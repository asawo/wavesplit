mod commands;
mod db;
mod paths;
mod pipeline;

use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use tauri::Manager;

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub data_dir: std::path::PathBuf,
}

#[tauri::command]
fn list_tracks(state: tauri::State<AppState>) -> Result<Vec<db::Track>, String> {
    let conn = state.db.lock().unwrap();
    db::list_tracks(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_track(id: String, state: tauri::State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().unwrap();
    db::delete_track(&conn, &id).map_err(|e| e.to_string())?;
    drop(conn);
    let track_dir = paths::track_dir(&state.data_dir, &id);
    if track_dir.exists() {
        std::fs::remove_dir_all(&track_dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(data_dir.join("tracks"))?;

            let db_path = data_dir.join("wavesplit.db");
            let conn = db::open(&db_path)?;

            let incomplete = db::incomplete_tracks(&conn).unwrap_or_default();
            if !incomplete.is_empty() {
                eprintln!("[wavesplit] {} track(s) have incomplete pipeline state", incomplete.len());
            }

            app.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
                data_dir,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_tracks,
            delete_track,
            commands::add_track_youtube,
            commands::add_track_local,
            commands::export_stems,
            commands::update_track_meta,
            commands::open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
