use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::constants::STEM_NAMES;
use crate::db::{self, Track};
use crate::paths;
use crate::pipeline::{self, Source};
use crate::AppState;

const ALLOWED_YOUTUBE_HOSTS: &[&str] = &[
    "youtube.com",
    "www.youtube.com",
    "youtu.be",
    "music.youtube.com",
];

fn validate_youtube_url(raw: &str) -> Result<(), String> {
    let parsed = url::Url::parse(raw).map_err(|_| "Invalid URL".to_string())?;
    if parsed.scheme() != "https" {
        return Err("URL must use the https scheme".to_string());
    }
    let host = parsed.host_str().unwrap_or("");
    if !ALLOWED_YOUTUBE_HOSTS.contains(&host) {
        return Err(format!(
            "URL host '{host}' is not an allowed YouTube domain"
        ));
    }
    Ok(())
}

struct TaskGuard {
    tasks: Arc<Mutex<HashMap<String, CancellationToken>>>,
    track_id: String,
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        if let Ok(mut tasks) = self.tasks.lock() {
            tasks.remove(&self.track_id);
        }
    }
}

fn spawn_pipeline<R: tauri::Runtime>(
    track_id: String,
    source: pipeline::Source,
    state: &AppState,
    app: AppHandle<R>,
    start_stage: pipeline::StartStage,
) {
    let token = CancellationToken::new();
    state
        .tasks
        .lock()
        .unwrap_or_else(|e| {
            tracing::warn!(track_id, "tasks mutex poisoned, recovering");
            e.into_inner()
        })
        .insert(track_id.clone(), token.clone());
    let tasks = Arc::clone(&state.tasks);
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    let demucs_dir = state.demucs_dir.clone();
    tokio::spawn(async move {
        let _guard = TaskGuard {
            tasks,
            track_id: track_id.clone(),
        };
        pipeline::run(
            track_id,
            source,
            db,
            data_dir,
            demucs_dir,
            token,
            app,
            start_stage,
        )
        .await;
    });
}

#[derive(serde::Serialize)]
pub struct AddTrackResult {
    pub id: String,
    pub duplicate: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct StemPaths {
    pub vocals: String,
    pub drums: String,
    pub bass: String,
    pub other: String,
}

#[tauri::command]
pub fn get_stem_paths(
    track_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<StemPaths, String> {
    let track_id = paths::parse_track_id(&track_id)?;
    {
        let conn = state
            .db
            .lock()
            .map_err(|_| "database unavailable".to_string())?;
        if db::get_track(&conn, &track_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err("track not found".into());
        }
    }
    let stems = paths::stems_dir(&state.data_dir, &track_id);
    let p = |name: &str| stems.join(name).to_string_lossy().into_owned();
    Ok(StemPaths {
        vocals: p("vocals.wav"),
        drums: p("drums.wav"),
        bass: p("bass.wav"),
        other: p("other.wav"),
    })
}

#[tauri::command]
pub async fn add_track_youtube(
    url: String,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AddTrackResult, String> {
    validate_youtube_url(&url)?;
    {
        let conn = state
            .db
            .lock()
            .map_err(|_| "database unavailable".to_string())?;
        if let Some(existing) = db::find_by_url(&conn, &url).map_err(|e| e.to_string())? {
            return Ok(AddTrackResult {
                id: existing.id,
                duplicate: true,
            });
        }
    }
    let title = pipeline::download::youtube_title(&url).unwrap_or_else(|| {
        tracing::info!("failed to extract YouTube title, falling back to URL");
        url.clone()
    });
    let id = add_track(
        Source::Youtube(url.clone()),
        title,
        Some(url),
        None,
        app,
        state,
    )
    .await?;
    Ok(AddTrackResult {
        id,
        duplicate: false,
    })
}

// Keep in sync with ACCEPTED_AUDIO_EXTENSIONS in ui/lib/importTrack.ts.
const ACCEPTED_LOCAL_EXTENSIONS: [&str; 6] = ["mp3", "wav", "flac", "m4a", "aac", "ogg"];

/// Canonicalizes and validates a caller-supplied local import path. Canonicalization requires
/// the path to exist on disk, which rejects protocol-like strings (`http://...`) and traversal
/// sequences that don't resolve to a real file; it also resolves symlinks, so the extension and
/// regular-file checks below apply to the real target rather than a symlink pointing elsewhere.
fn validate_local_source(raw: &str) -> Result<std::path::PathBuf, String> {
    if raw.is_empty() || raw.contains("://") {
        return Err("invalid local file path".to_string());
    }
    let canonical = std::path::Path::new(raw)
        .canonicalize()
        .map_err(|_| "file not found".to_string())?;
    if !canonical.is_file() {
        return Err("not a regular file".to_string());
    }
    let ext = canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !ACCEPTED_LOCAL_EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!("unsupported file type: .{ext}"));
    }
    Ok(canonical)
}

#[tauri::command]
pub async fn add_track_local<R: tauri::Runtime>(
    path: String,
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<AddTrackResult, String> {
    let canonical = validate_local_source(&path)?;
    let canonical_str = canonical.to_string_lossy().into_owned();
    {
        let conn = state
            .db
            .lock()
            .map_err(|_| "database unavailable".to_string())?;
        if let Some(existing) =
            db::find_by_path(&conn, &canonical_str).map_err(|e| e.to_string())?
        {
            return Ok(AddTrackResult {
                id: existing.id,
                duplicate: true,
            });
        }
    }
    let title = pipeline::download::local_title(&canonical);
    let id = add_track(
        Source::Local(canonical),
        title,
        None,
        Some(canonical_str),
        app,
        state,
    )
    .await?;
    Ok(AddTrackResult {
        id,
        duplicate: false,
    })
}

async fn add_track<R: tauri::Runtime>(
    source: Source,
    title: String,
    source_url: Option<String>,
    source_path: Option<String>,
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let source_type = if source_url.is_some() {
        "youtube"
    } else {
        "local"
    };

    // Create track directories
    std::fs::create_dir_all(paths::track_dir(&state.data_dir, &id)).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(paths::stems_dir(&state.data_dir, &id)).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(paths::analysis_dir(&state.data_dir, &id))
        .map_err(|e| e.to_string())?;

    // Insert DB row
    {
        let conn = state
            .db
            .lock()
            .map_err(|_| "database unavailable".to_string())?;
        let order = db::next_sort_order(&conn).map_err(|e| e.to_string())?;
        db::insert_track(
            &conn,
            &Track {
                id: id.clone(),
                title,
                source_type: source_type.to_string(),
                source_url,
                source_path,
                created_at: Utc::now().to_rfc3339(),
                sort_order: order,
                duration_ms: None,
                status_download: "pending".into(),
                status_stems: "pending".into(),
                status_analysis: "pending".into(),
                error_message: None,
                export_path: None,
                artist: None,
            },
        )
        .map_err(|e| e.to_string())?;
    }

    spawn_pipeline(
        id.clone(),
        source,
        &state,
        app,
        pipeline::StartStage::Download,
    );

    Ok(id)
}

#[tauri::command]
pub fn export_stems(
    track_id: String,
    dest_dir: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let track_id = paths::parse_track_id(&track_id)?;
    {
        let conn = state
            .db
            .lock()
            .map_err(|_| "database unavailable".to_string())?;
        if db::get_track(&conn, &track_id)
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Err("track not found — it may have been deleted".into());
        }
    }
    let stems_dir = paths::stems_dir(&state.data_dir, &track_id);
    if !stems_dir.exists() {
        return Err("track data no longer available — it may have been deleted".into());
    }
    let dest = std::path::PathBuf::from(&dest_dir);

    let mut exported = Vec::new();

    let source_wav = paths::source_wav(&state.data_dir, &track_id);
    if source_wav.exists() {
        let dst = dest.join("source.wav");
        std::fs::copy(&source_wav, &dst).map_err(|e| format!("failed to copy source.wav: {e}"))?;
        exported.push("source.wav".to_string());
    }

    for stem in STEM_NAMES {
        let src = stems_dir.join(format!("{stem}.wav"));
        if src.exists() {
            let dst = dest.join(format!("{stem}.wav"));
            std::fs::copy(&src, &dst).map_err(|e| format!("failed to copy {stem}.wav: {e}"))?;
            exported.push(format!("{stem}.wav"));
        }
    }

    if exported.is_empty() {
        return Err("No stem files found for this track".into());
    }

    let conn = state
        .db
        .lock()
        .map_err(|_| "database unavailable".to_string())?;
    db::set_export_path(&conn, &track_id, &dest_dir).map_err(|e| e.to_string())?;

    Ok(exported)
}

#[tauri::command]
pub fn update_track_meta(
    id: String,
    title: String,
    artist: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let id = paths::parse_track_id(&id)?;
    let conn = state
        .db
        .lock()
        .map_err(|_| "database unavailable".to_string())?;
    let rows =
        db::update_track_meta(&conn, &id, &title, artist.as_deref()).map_err(|e| e.to_string())?;
    if rows == 0 {
        return Err("track not found".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    let canonical = p.canonicalize().map_err(|e| e.to_string())?;
    tauri_plugin_opener::open_path(canonical, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn retry_track<R: tauri::Runtime>(
    id: String,
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let id = paths::parse_track_id(&id)?;
    // Don't retry if already running
    if state
        .tasks
        .lock()
        .map_err(|_| "tasks unavailable".to_string())?
        .contains_key(&id)
    {
        return Err("track is already processing".to_string());
    }

    let track = {
        let conn = state
            .db
            .lock()
            .map_err(|_| "database unavailable".to_string())?;
        db::get_track(&conn, &id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "track not found".to_string())?
    };

    let (start_stage, reset_download, reset_stems) = if track.status_download != "done" {
        (pipeline::StartStage::Download, true, true)
    } else if track.status_stems != "done" {
        (pipeline::StartStage::Stems, false, true)
    } else {
        (pipeline::StartStage::Analysis, false, false)
    };

    {
        let conn = state
            .db
            .lock()
            .map_err(|_| "database unavailable".to_string())?;
        db::reset_for_retry(&conn, &id, reset_download, reset_stems).map_err(|e| e.to_string())?;
    }

    let source = match track.source_type.as_str() {
        "youtube" => {
            let url = track
                .source_url
                .ok_or_else(|| "missing source URL".to_string())?;
            pipeline::Source::Youtube(url)
        }
        _ => {
            let path = track
                .source_path
                .ok_or_else(|| "missing source path".to_string())?;
            pipeline::Source::Local(std::path::PathBuf::from(path))
        }
    };

    spawn_pipeline(id, source, &state, app, start_stage);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use tauri::{test::mock_app, Manager};
    use tokio_util::sync::CancellationToken;

    #[test]
    fn validate_youtube_url_accepts_valid() {
        let valid = [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ",
            "https://youtube.com/watch?v=dQw4w9WgXcQ",
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ",
        ];
        for url in &valid {
            assert!(validate_youtube_url(url).is_ok(), "should accept: {url}");
        }
    }

    #[test]
    fn validate_youtube_url_rejects_invalid() {
        let invalid = [
            ("http://www.youtube.com/watch?v=x", "non-https scheme"),
            ("https://evil.com/watch?v=x", "foreign host"),
            ("https://192.168.1.1/foo", "IP address"),
            ("not a url at all", "garbage input"),
            ("file:///etc/passwd", "file scheme"),
            ("https://www.youtube.com.evil.com/x", "lookalike domain"),
        ];
        for (url, label) in &invalid {
            assert!(
                validate_youtube_url(url).is_err(),
                "should reject ({label}): {url}"
            );
        }
    }

    #[tokio::test]
    async fn spawn_pipeline_adds_and_removes_task() {
        let db = Arc::new(Mutex::new(
            crate::db::open(std::path::Path::new(":memory:")).unwrap(),
        ));
        let tasks: Arc<Mutex<HashMap<String, CancellationToken>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let state = AppState {
            db: Arc::clone(&db),
            data_dir: std::path::PathBuf::from("/tmp"),
            demucs_dir: std::path::PathBuf::from("/tmp"),
            tasks: Arc::clone(&tasks),
        };

        let app = mock_app();
        let id = "test-spawn-id".to_string();

        spawn_pipeline(
            id.clone(),
            pipeline::Source::Local(std::path::PathBuf::from("/dev/null")),
            &state,
            app.handle().clone(),
            pipeline::StartStage::Analysis,
        );

        assert!(
            tasks.lock().unwrap().contains_key(&id),
            "pipeline task should be added to map on spawn"
        );

        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(5);
        while tasks.lock().unwrap().contains_key(&id) {
            if start.elapsed() > timeout {
                panic!("pipeline task did not complete within timeout");
            }
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn retry_track_rejects_running_track() {
        let db = Arc::new(Mutex::new(
            crate::db::open(std::path::Path::new(":memory:")).unwrap(),
        ));
        let tasks: Arc<Mutex<HashMap<String, CancellationToken>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let app_state = AppState {
            db: Arc::clone(&db),
            data_dir: std::path::PathBuf::from("/tmp"),
            demucs_dir: std::path::PathBuf::from("/tmp"),
            tasks: Arc::clone(&tasks),
        };

        let track_id = "22222222-2222-2222-2222-222222222222";
        tasks
            .lock()
            .unwrap()
            .insert(track_id.to_string(), CancellationToken::new());

        let app = mock_app();
        app.manage(app_state);
        let state = app.state::<AppState>();

        let result = retry_track(track_id.to_string(), app.handle().clone(), state).await;

        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("already processing"),
            "should reject already running track"
        );
    }

    #[tokio::test]
    async fn spawn_pipeline_logs_warning_on_poisoned_mutex() {
        let tasks: Arc<Mutex<HashMap<String, CancellationToken>>> =
            Arc::new(Mutex::new(HashMap::new()));
        // Poison the mutex
        let tasks2 = Arc::clone(&tasks);
        let _ = std::thread::spawn(move || {
            let _guard = tasks2.lock().unwrap();
            panic!("intentional panic to poison mutex");
        })
        .join();

        let (capture, _guard) = crate::test_support::TracingCapture::new();

        let db = Arc::new(Mutex::new(
            crate::db::open(std::path::Path::new(":memory:")).unwrap(),
        ));
        let state = AppState {
            db,
            data_dir: std::path::PathBuf::from("/tmp"),
            demucs_dir: std::path::PathBuf::from("/tmp"),
            tasks: Arc::clone(&tasks),
        };

        let app = tauri::test::mock_app();
        let id = "test-poison-id".to_string();

        spawn_pipeline(
            id.clone(),
            pipeline::Source::Local(std::path::PathBuf::from("/dev/null")),
            &state,
            app.handle().clone(),
            pipeline::StartStage::Analysis,
        );

        assert!(
            capture.contains("WARN", "tasks mutex poisoned"),
            "expected warning about poisoned mutex, got: {:?}",
            capture.events()
        );

        // Should still have inserted the task despite the poison
        let guard = tasks.lock().unwrap_or_else(|e| e.into_inner());
        assert!(
            guard.contains_key(&id),
            "task should be in map despite poison"
        );
    }

    #[tokio::test]
    async fn retry_track_spawns_for_error_track() {
        let db = Arc::new(Mutex::new(
            crate::db::open(std::path::Path::new(":memory:")).unwrap(),
        ));
        let tasks: Arc<Mutex<HashMap<String, CancellationToken>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let app_state = AppState {
            db: Arc::clone(&db),
            data_dir: std::path::PathBuf::from("/tmp"),
            demucs_dir: std::path::PathBuf::from("/tmp"),
            tasks: Arc::clone(&tasks),
        };

        let track_id = "33333333-3333-3333-3333-333333333333";
        {
            let conn = db.lock().unwrap();
            crate::db::insert_track(
                &conn,
                &crate::db::Track {
                    id: track_id.to_string(),
                    title: "Retry Test".to_string(),
                    source_type: "youtube".to_string(),
                    source_url: Some("https://youtube.com/watch?v=test".to_string()),
                    source_path: None,
                    created_at: "2024-01-01T00:00:00Z".to_string(),
                    sort_order: 1,
                    duration_ms: None,
                    status_download: "done".to_string(),
                    status_stems: "done".to_string(),
                    status_analysis: "error".to_string(),
                    error_message: Some("analysis failed".to_string()),
                    export_path: None,
                    artist: None,
                },
            )
            .unwrap();
        }

        let app = mock_app();
        app.manage(app_state);
        let state = app.state::<AppState>();

        let result = retry_track(track_id.to_string(), app.handle().clone(), state).await;

        assert!(result.is_ok(), "retry should succeed: {:?}", result.err());

        assert!(
            tasks.lock().unwrap().contains_key(track_id),
            "pipeline task should be spawned"
        );

        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(5);
        while tasks.lock().unwrap().contains_key(track_id) {
            if start.elapsed() > timeout {
                panic!("pipeline task did not complete within timeout");
            }
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn task_removed_from_map_on_pipeline_panic() {
        let tasks: Arc<Mutex<HashMap<String, CancellationToken>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let track_id = "test-track-id".to_string();
        let token = CancellationToken::new();

        tasks.lock().unwrap().insert(track_id.clone(), token);

        let tasks_clone = Arc::clone(&tasks);
        let tid = track_id.clone();
        let handle = tokio::spawn(async move {
            let _guard = TaskGuard {
                tasks: tasks_clone,
                track_id: tid,
            };
            panic!("simulated pipeline panic");
        });
        let _ = handle.await; // absorb JoinError

        assert!(
            !tasks.lock().unwrap().contains_key(&track_id),
            "task entry should be removed even after a panic"
        );
    }

    fn mem_state() -> AppState {
        AppState {
            db: Arc::new(Mutex::new(
                crate::db::open(std::path::Path::new(":memory:")).unwrap(),
            )),
            data_dir: std::path::PathBuf::from("/tmp"),
            demucs_dir: std::path::PathBuf::from("/tmp"),
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[test]
    fn get_stem_paths_rejects_malformed_id() {
        let app = mock_app();
        app.manage(mem_state());
        let result = get_stem_paths("../../etc".to_string(), app.state::<AppState>());
        assert!(result.is_err());
    }

    #[test]
    fn get_stem_paths_rejects_unknown_track() {
        let app = mock_app();
        app.manage(mem_state());
        let id = "44444444-4444-4444-4444-444444444444".to_string();
        let result = get_stem_paths(id, app.state::<AppState>());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn get_stem_paths_returns_paths_for_existing_track() {
        let app = mock_app();
        app.manage(mem_state());
        let id = "55555555-5555-5555-5555-555555555555";
        {
            let state = app.state::<AppState>();
            let conn = state.db.lock().unwrap();
            let track = crate::db::Track {
                id: id.to_string(),
                title: "T".to_string(),
                source_type: "local".to_string(),
                source_url: None,
                source_path: None,
                created_at: "2024-01-01T00:00:00Z".to_string(),
                sort_order: 1,
                duration_ms: None,
                status_download: "done".to_string(),
                status_stems: "done".to_string(),
                status_analysis: "done".to_string(),
                error_message: None,
                export_path: None,
                artist: None,
            };
            crate::db::insert_track(&conn, &track).unwrap();
        }
        let result = get_stem_paths(id.to_string(), app.state::<AppState>()).unwrap();
        assert!(result.bass.ends_with("bass.wav"));
        assert!(result.bass.contains(id));
    }

    #[test]
    fn export_stems_rejects_malformed_id() {
        let app = mock_app();
        app.manage(mem_state());
        let result = export_stems(
            "/etc/passwd".to_string(),
            std::env::temp_dir().to_string_lossy().into_owned(),
            app.state::<AppState>(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn update_track_meta_rejects_malformed_id() {
        let app = mock_app();
        app.manage(mem_state());
        let result = update_track_meta(
            "../../evil".to_string(),
            "Title".to_string(),
            None,
            app.state::<AppState>(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn update_track_meta_rejects_unknown_track() {
        let app = mock_app();
        app.manage(mem_state());
        let id = "66666666-6666-6666-6666-666666666666".to_string();
        let result = update_track_meta(id, "Title".to_string(), None, app.state::<AppState>());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn retry_track_rejects_malformed_id() {
        let app = mock_app();
        app.manage(mem_state());
        let state = app.state::<AppState>();
        let result = retry_track("../../evil".to_string(), app.handle().clone(), state).await;
        assert!(result.is_err());
    }

    #[test]
    fn validate_local_source_rejects_missing_file() {
        assert!(validate_local_source("/nonexistent/wavesplit-test/does-not-exist.wav").is_err());
    }

    #[test]
    fn validate_local_source_rejects_protocol_like_input() {
        assert!(validate_local_source("http://example.com/track.mp3").is_err());
    }

    #[test]
    fn validate_local_source_rejects_directory() {
        let dir = std::env::temp_dir();
        assert!(validate_local_source(&dir.to_string_lossy()).is_err());
    }

    #[test]
    fn validate_local_source_rejects_disallowed_extension() {
        let path = std::env::temp_dir().join("wavesplit-test-validate-local-source.exe");
        std::fs::write(&path, b"not audio").unwrap();
        let result = validate_local_source(&path.to_string_lossy());
        let _ = std::fs::remove_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn validate_local_source_accepts_and_canonicalizes_valid_file() {
        let path = std::env::temp_dir().join("wavesplit-test-validate-local-source.wav");
        std::fs::write(&path, b"not real audio, just bytes").unwrap();
        let expected = path.canonicalize().unwrap();
        let result = validate_local_source(&path.to_string_lossy());
        let _ = std::fs::remove_file(&path);
        assert_eq!(result.unwrap(), expected);
    }

    #[tokio::test]
    async fn add_track_local_rejects_disallowed_extension() {
        let bad_file = std::env::temp_dir().join("wavesplit-test-add-track-local.exe");
        std::fs::write(&bad_file, b"not audio").unwrap();

        let app = mock_app();
        app.manage(mem_state());
        let state = app.state::<AppState>();

        let result = add_track_local(
            bad_file.to_string_lossy().into_owned(),
            app.handle().clone(),
            state,
        )
        .await;
        let _ = std::fs::remove_file(&bad_file);

        assert!(result.is_err());
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        assert!(
            crate::db::list_tracks(&conn).unwrap().is_empty(),
            "no track row should be inserted for a rejected import"
        );
    }

    #[tokio::test]
    async fn add_track_local_canonicalizes_and_dedupes_by_real_path() {
        let wav_file = std::env::temp_dir().join("wavesplit-test-add-track-local-dedupe.wav");
        std::fs::write(&wav_file, b"not real audio, just bytes").unwrap();
        let canonical = wav_file.canonicalize().unwrap();

        let app = mock_app();
        app.manage(mem_state());

        let first = add_track_local(
            wav_file.to_string_lossy().into_owned(),
            app.handle().clone(),
            app.state::<AppState>(),
        )
        .await
        .expect("first import should succeed");
        assert!(!first.duplicate);

        let second = add_track_local(
            wav_file.to_string_lossy().into_owned(),
            app.handle().clone(),
            app.state::<AppState>(),
        )
        .await
        .expect("second import should be detected as duplicate");
        assert!(second.duplicate);
        assert_eq!(second.id, first.id);

        {
            let state = app.state::<AppState>();
            let conn = state.db.lock().unwrap();
            let stored = crate::db::get_track(&conn, &first.id).unwrap().unwrap();
            assert_eq!(
                stored.source_path.as_deref(),
                Some(canonical.to_string_lossy().as_ref())
            );
        }

        let _ = std::fs::remove_file(&wav_file);
    }
}
