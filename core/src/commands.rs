use std::sync::Arc;
use tauri::AppHandle;
use uuid::Uuid;
use chrono::Utc;
use tokio_util::sync::CancellationToken;

use crate::db::{self, Track};
use crate::paths;
use crate::pipeline::{self, Source};
use crate::AppState;

#[derive(serde::Serialize)]
pub struct StemPaths {
    pub vocals: String,
    pub drums: String,
    pub bass: String,
    pub other: String,
}

#[tauri::command]
pub fn get_stem_paths(track_id: String, state: tauri::State<'_, AppState>) -> Result<StemPaths, String> {
    let stems = paths::stems_dir(&state.data_dir, &track_id);
    let p = |name: &str| stems.join(name).to_string_lossy().into_owned();
    Ok(StemPaths {
        vocals: p("vocals.wav"),
        drums:  p("drums.wav"),
        bass:   p("bass.wav"),
        other:  p("other.wav"),
    })
}

#[tauri::command]
pub async fn add_track_youtube(
    url: String,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let title = pipeline::download::youtube_title(&url)
        .unwrap_or_else(|| url.clone());
    add_track(Source::Youtube(url.clone()), title, Some(url), None, app, state).await
}

#[tauri::command]
pub async fn add_track_local(
    path: String,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let src = std::path::PathBuf::from(&path);
    let title = pipeline::download::local_title(&src);
    add_track(Source::Local(src), title, None, Some(path), app, state).await
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
    let source_type = if source_url.is_some() { "youtube" } else { "local" };

    // Create track directories
    std::fs::create_dir_all(paths::track_dir(&state.data_dir, &id)).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(paths::stems_dir(&state.data_dir, &id)).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(paths::analysis_dir(&state.data_dir, &id)).map_err(|e| e.to_string())?;

    // Insert DB row
    {
        let conn = state.db.lock().map_err(|_| "database unavailable".to_string())?;
        let order = db::next_sort_order(&conn).map_err(|e| e.to_string())?;
        db::insert_track(&conn, &Track {
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
        }).map_err(|e| e.to_string())?;
    }

    // Spawn pipeline in background with a cancellation token.
    let token = CancellationToken::new();
    {
        let mut tasks = state.tasks.lock().map_err(|_| "tasks unavailable".to_string())?;
        tasks.insert(id.clone(), token.clone());
    }
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    let demucs_dir = state.demucs_dir.clone();
    let tasks = Arc::clone(&state.tasks);
    let track_id = id.clone();
    tokio::spawn(async move {
        pipeline::run(track_id.clone(), source, db, data_dir, demucs_dir, token, app, pipeline::StartStage::Download).await;
        if let Ok(mut tasks) = tasks.lock() {
            tasks.remove(&track_id);
        }
    });

    Ok(id)
}

#[tauri::command]
pub fn export_stems(track_id: String, dest_dir: String, state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let stems_dir = paths::stems_dir(&state.data_dir, &track_id);
    let dest = std::path::PathBuf::from(&dest_dir);

    let mut exported = Vec::new();

    let source_wav = paths::source_wav(&state.data_dir, &track_id);
    if source_wav.exists() {
        let dst = dest.join("source.wav");
        std::fs::copy(&source_wav, &dst).map_err(|e| format!("failed to copy source.wav: {e}"))?;
        exported.push("source.wav".to_string());
    }

    for stem in &["bass", "drums", "vocals", "other"] {
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

    let conn = state.db.lock().map_err(|_| "database unavailable".to_string())?;
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
    let conn = state.db.lock().map_err(|_| "database unavailable".to_string())?;
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
pub async fn retry_track(
    id: String,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Don't retry if already running
    if state.tasks.lock().map_err(|_| "tasks unavailable".to_string())?.contains_key(&id) {
        return Err("track is already processing".to_string());
    }

    let track = {
        let conn = state.db.lock().map_err(|_| "database unavailable".to_string())?;
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
        let conn = state.db.lock().map_err(|_| "database unavailable".to_string())?;
        db::reset_for_retry(&conn, &id, reset_download, reset_stems).map_err(|e| e.to_string())?;
    }

    let source = match track.source_type.as_str() {
        "youtube" => {
            let url = track.source_url.ok_or_else(|| "missing source URL".to_string())?;
            pipeline::Source::Youtube(url)
        }
        _ => {
            let path = track.source_path.ok_or_else(|| "missing source path".to_string())?;
            pipeline::Source::Local(std::path::PathBuf::from(path))
        }
    };

    let token = CancellationToken::new();
    {
        let mut tasks = state.tasks.lock().map_err(|_| "tasks unavailable".to_string())?;
        tasks.insert(id.clone(), token.clone());
    }
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    let demucs_dir = state.demucs_dir.clone();
    let tasks = Arc::clone(&state.tasks);
    let track_id = id.clone();
    tokio::spawn(async move {
        pipeline::run(track_id.clone(), source, db, data_dir, demucs_dir, token, app, start_stage).await;
        if let Ok(mut tasks) = tasks.lock() {
            tasks.remove(&track_id);
        }
    });

    Ok(())
}
