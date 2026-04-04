mod commands;
mod db;
mod paths;
mod pipeline;
mod setup;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub data_dir: std::path::PathBuf,
    /// Directory that holds the frozen demucs binary and its model cache.
    /// Default: {app_data}/demucs/
    pub demucs_dir: std::path::PathBuf,
    /// Cancellation tokens for in-flight pipeline tasks, keyed by track ID.
    pub tasks: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

#[tauri::command]
fn list_tracks(state: tauri::State<AppState>) -> Result<Vec<db::Track>, String> {
    let conn = state.db.lock().map_err(|_| "database unavailable".to_string())?;
    db::list_tracks(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_track(id: String, state: tauri::State<AppState>) -> Result<(), String> {
    // Cancel any in-flight pipeline task before touching the filesystem.
    if let Ok(mut tasks) = state.tasks.lock() {
        if let Some(token) = tasks.remove(&id) {
            token.cancel();
        }
    }
    let conn = state.db.lock().map_err(|_| "database unavailable".to_string())?;
    db::delete_track(&conn, &id).map_err(|e| e.to_string())?;
    drop(conn);
    let track_dir = paths::track_dir(&state.data_dir, &id);
    if track_dir.exists() {
        std::fs::remove_dir_all(&track_dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn check_demucs(state: tauri::State<AppState>) -> bool {
    setup::is_available(&state.demucs_dir)
}

#[tauri::command]
async fn download_demucs(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let demucs_dir = state.demucs_dir.clone();
    setup::download(&demucs_dir, &app).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(data_dir.join("tracks"))?;

            let demucs_dir = data_dir.join("demucs");
            std::fs::create_dir_all(&demucs_dir)?;

            let db_path = data_dir.join("wavesplit.db");
            let conn = db::open(&db_path)?;

            let incomplete = db::incomplete_tracks(&conn).unwrap_or_default();
            if !incomplete.is_empty() {
                eprintln!("[wavesplit] {} track(s) have incomplete pipeline state", incomplete.len());
            }

            app.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
                data_dir,
                demucs_dir,
                tasks: Arc::new(Mutex::new(HashMap::new())),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_tracks,
            delete_track,
            check_demucs,
            download_demucs,
            commands::add_track_youtube,
            commands::add_track_local,
            commands::export_stems,
            commands::update_track_meta,
            commands::open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
