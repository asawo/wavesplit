mod analysis;
pub mod download;
mod stems;

use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

use crate::db;
use crate::paths;

const EVENT: &str = "pipeline";

#[derive(Clone, serde::Serialize)]
struct PipelineEvent<'a> {
    track_id: &'a str,
    stage: &'a str,
    status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

fn emit(app: &AppHandle, track_id: &str, stage: &str, status: &str, message: Option<String>) {
    let _ = app.emit(EVENT, PipelineEvent { track_id, stage, status, message });
}

pub enum Source {
    Youtube(String),
    Local(std::path::PathBuf),
}

/// Run the full pipeline for a track: download → stems → analysis.
/// Each stage updates the DB status and emits a Tauri event.
pub async fn run(
    track_id: String,
    source: Source,
    db: Arc<Mutex<Connection>>,
    data_dir: std::path::PathBuf,
    app: AppHandle,
) {
    let source_wav = paths::source_wav(&data_dir, &track_id);
    let stems_dir = paths::stems_dir(&data_dir, &track_id);
    let analysis_dir = paths::analysis_dir(&data_dir, &track_id);

    // --- Stage 1: download ---
    emit(&app, &track_id, "download", "started", None);
    let dl_result = tokio::task::spawn_blocking({
        let source_wav = source_wav.clone();
        move || match &source {
            Source::Youtube(url) => download::from_youtube(url, &source_wav),
            Source::Local(path) => download::from_local(path, &source_wav),
        }
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()));

    {
        let conn = db.lock().unwrap();
        match &dl_result {
            Ok(_) => { let _ = db::update_status(&conn, &track_id, "status_download", "done", None); }
            Err(e) => { let _ = db::update_status(&conn, &track_id, "status_download", "error", Some(e)); }
        }
    }
    match dl_result {
        Ok(_) => emit(&app, &track_id, "download", "done", None),
        Err(e) => { emit(&app, &track_id, "download", "error", Some(e)); return; }
    }

    // --- Stage 2: stems ---
    emit(&app, &track_id, "stems", "started", None);
    let stems_result = tokio::task::spawn_blocking({
        let source_wav = source_wav.clone();
        let stems_dir = stems_dir.clone();
        move || stems::separate(&source_wav, &stems_dir)
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()));

    {
        let conn = db.lock().unwrap();
        match &stems_result {
            Ok(_) => { let _ = db::update_status(&conn, &track_id, "status_stems", "done", None); }
            Err(e) => { let _ = db::update_status(&conn, &track_id, "status_stems", "error", Some(e)); }
        }
    }
    match stems_result {
        Ok(_) => emit(&app, &track_id, "stems", "done", None),
        Err(e) => { emit(&app, &track_id, "stems", "error", Some(e)); return; }
    }

    // TODO: re-enable analysis once beat/note detection is ready (MVP v2)
    // emit(&app, &track_id, "analysis", "started", None);
    // let analysis_result = tokio::task::spawn_blocking({
    //     let stems_dir = stems_dir.clone();
    //     let analysis_dir = analysis_dir.clone();
    //     move || analysis::run(&stems_dir, &analysis_dir)
    // })
    // .await
    // .unwrap_or_else(|e| Err(e.to_string()));
    // {
    //     let conn = db.lock().unwrap();
    //     match &analysis_result {
    //         Ok(_) => { let _ = db::update_status(&conn, &conn, "status_analysis", "done", None); }
    //         Err(e) => { let _ = db::update_status(&conn, &track_id, "status_analysis", "error", Some(e)); }
    //     }
    // }
    // match analysis_result {
    //     Ok(_) => emit(&app, &track_id, "analysis", "done", None),
    //     Err(e) => emit(&app, &track_id, "analysis", "error", Some(e)),
    // }
    {
        let conn = db.lock().unwrap();
        let _ = db::update_status(&conn, &track_id, "status_analysis", "done", None);
    }
    emit(&app, &track_id, "analysis", "done", None);
}
