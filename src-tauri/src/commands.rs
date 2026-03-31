use std::sync::Arc;
use tauri::AppHandle;
use uuid::Uuid;
use chrono::Utc;

use crate::db::{self, Track};
use crate::paths;
use crate::pipeline::{self, Source};
use crate::AppState;

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
        let conn = state.db.lock().unwrap();
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
        }).map_err(|e| e.to_string())?;
    }

    // Spawn pipeline in background
    let db = Arc::clone(&state.db);
    let data_dir = state.data_dir.clone();
    let track_id = id.clone();
    tokio::spawn(pipeline::run(track_id, source, db, data_dir, app));

    Ok(id)
}

#[tauri::command]
pub fn export_stems(track_id: String, dest_dir: String, state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let stems_dir = paths::stems_dir(&state.data_dir, &track_id);
    let dest = std::path::PathBuf::from(&dest_dir);

    let mut exported = Vec::new();
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

    Ok(exported)
}
