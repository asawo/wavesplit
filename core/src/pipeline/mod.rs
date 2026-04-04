mod analysis;
pub mod bins;
pub mod download;
mod stems;

use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use crate::db;
use crate::paths;
use crate::setup;

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
/// Returns early (silently) if the cancellation token is triggered.
pub async fn run(
    track_id: String,
    source: Source,
    db: Arc<Mutex<Connection>>,
    data_dir: std::path::PathBuf,
    demucs_dir: std::path::PathBuf,
    token: CancellationToken,
    app: AppHandle,
) {
    let source_wav = paths::source_wav(&data_dir, &track_id);
    let stems_dir = paths::stems_dir(&data_dir, &track_id);
    let Some(demucs_bin) = setup::resolve_binary(&demucs_dir) else {
        emit(&app, &track_id, "stems", "error", Some("demucs not found".to_string()));
        return;
    };
    let cache_dir = setup::cache_dir(&demucs_dir);

    // --- Stage 1: download ---
    if token.is_cancelled() { return; }
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
        let conn = db.lock().unwrap_or_else(|e| e.into_inner());
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
    if token.is_cancelled() { return; }
    emit(&app, &track_id, "stems", "started", None);
    let stems_result = tokio::task::spawn_blocking({
        let source_wav = source_wav.clone();
        let stems_dir = stems_dir.clone();
        move || stems::separate(&source_wav, &stems_dir, &demucs_bin, &cache_dir)
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()));

    {
        let conn = db.lock().unwrap_or_else(|e| e.into_inner());
        match &stems_result {
            Ok(_) => { let _ = db::update_status(&conn, &track_id, "status_stems", "done", None); }
            Err(e) => { let _ = db::update_status(&conn, &track_id, "status_stems", "error", Some(e)); }
        }
    }
    match stems_result {
        Ok(_) => emit(&app, &track_id, "stems", "done", None),
        Err(e) => { emit(&app, &track_id, "stems", "error", Some(e)); return; }
    }

    // --- Stage 3: analysis ---
    if token.is_cancelled() { return; }
    // TODO: re-enable analysis once beat/note detection is ready (MVP v2)
    {
        let conn = db.lock().unwrap_or_else(|e| e.into_inner());
        let _ = db::update_status(&conn, &track_id, "status_analysis", "done", None);
    }
    emit(&app, &track_id, "analysis", "done", None);
}
