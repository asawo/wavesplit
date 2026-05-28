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
        .unwrap_or_else(|e| e.into_inner())
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

#[derive(serde::Serialize)]
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
    let title = pipeline::download::youtube_title(&url).unwrap_or_else(|| url.clone());
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

#[tauri::command]
pub async fn add_track_local(
    path: String,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AddTrackResult, String> {
    {
        let conn = state
            .db
            .lock()
            .map_err(|_| "database unavailable".to_string())?;
        if let Some(existing) = db::find_by_path(&conn, &path).map_err(|e| e.to_string())? {
            return Ok(AddTrackResult {
                id: existing.id,
                duplicate: true,
            });
        }
    }
    let src = std::path::PathBuf::from(&path);
    let title = pipeline::download::local_title(&src);
    let id = add_track(Source::Local(src), title, None, Some(path), app, state).await?;
    Ok(AddTrackResult {
        id,
        duplicate: false,
    })
}

async fn add_track(
    source: Source,
    title: String,
    source_url: Option<String>,
    source_path: Option<String>,
    app: AppHandle,
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
    let stems_dir = paths::stems_dir(&state.data_dir, &track_id);
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
    let conn = state
        .db
        .lock()
        .map_err(|_| "database unavailable".to_string())?;
    db::update_track_meta(&conn, &id, &title, artist.as_deref()).map_err(|e| e.to_string())
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

        tasks
            .lock()
            .unwrap()
            .insert("test-retry-id".to_string(), CancellationToken::new());

        let app = mock_app();
        app.manage(app_state);
        let state = app.state::<AppState>();

        let result = retry_track("test-retry-id".to_string(), app.handle().clone(), state).await;

        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("already processing"),
            "should reject already running track"
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

        {
            let conn = db.lock().unwrap();
            crate::db::insert_track(
                &conn,
                &crate::db::Track {
                    id: "test-retry-spawn".to_string(),
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

        let result = retry_track("test-retry-spawn".to_string(), app.handle().clone(), state).await;

        assert!(result.is_ok(), "retry should succeed: {:?}", result.err());

        assert!(
            tasks.lock().unwrap().contains_key("test-retry-spawn"),
            "pipeline task should be spawned"
        );

        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(5);
        while tasks.lock().unwrap().contains_key("test-retry-spawn") {
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
}
