mod analysis;
pub mod bins;
pub mod download;
mod stems;

use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Runtime};
use tokio_util::sync::CancellationToken;

use crate::db;
use crate::paths;
use crate::setup;

const EVENT: &str = "pipeline";

/// Acquire the DB mutex. Returns `Err` if the mutex is poisoned so the pipeline
/// can emit an error event and abort rather than silently continuing with
/// potentially corrupt state.
fn lock_db(db: &Mutex<Connection>) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
    db.lock().map_err(|_| "database mutex is poisoned".to_string())
}

/// Acquire the DB mutex or emit a stage error and return from the caller.
macro_rules! lock_or_abort {
    ($db:expr, $app:expr, $track_id:expr, $stage:literal) => {
        match lock_db($db) {
            Ok(c) => c,
            Err(e) => { emit($app, $track_id, $stage, "error", Some(e)); return; }
        }
    };
}

/// Write the stage result to the DB: "done" on success, "error" + message on failure.
/// Returns `Err` if the DB write itself fails so the caller can emit an error event and abort.
fn commit_result(conn: &Connection, track_id: &str, field: db::Stage, result: &Result<(), String>) -> Result<(), String> {
    match result {
        Ok(_) => db::update_status(conn, track_id, field, db::StageStatus::Done, None).map_err(|e| e.to_string()),
        Err(e) => db::update_status(conn, track_id, field, db::StageStatus::Error, Some(e)).map_err(|e| e.to_string()),
    }
}

#[derive(Clone, serde::Serialize)]
struct PipelineEvent<'a> {
    track_id: &'a str,
    stage: &'a str,
    status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

fn emit<R: Runtime>(app: &AppHandle<R>, track_id: &str, stage: &str, status: &str, message: Option<String>) {
    if let Err(e) = app.emit(EVENT, PipelineEvent { track_id, stage, status, message }) {
        // TODO: replace with structured logging once #22 lands
        eprintln!("[pipeline] emit failed for track={track_id} stage={stage}: {e}");
    }
}

pub enum Source {
    Youtube(String),
    Local(std::path::PathBuf),
}

pub enum StartStage {
    Download,
    Stems,
    Analysis,
}

/// Run the pipeline for a track starting from `start_stage`: download → stems → analysis.
/// Each stage updates the DB status and emits a Tauri event.
/// Returns early (silently) if the cancellation token is triggered.
#[allow(clippy::too_many_arguments)]
pub async fn run<R: Runtime>(
    track_id: String,
    source: Source,
    db: Arc<Mutex<Connection>>,
    data_dir: std::path::PathBuf,
    demucs_dir: std::path::PathBuf,
    token: CancellationToken,
    app: AppHandle<R>,
    start_stage: StartStage,
) {
    let source_wav = paths::source_wav(&data_dir, &track_id);
    let stems_dir = paths::stems_dir(&data_dir, &track_id);

    // --- Stage 1: download ---
    if matches!(start_stage, StartStage::Download) {
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
            let conn = lock_or_abort!(&db, &app, &track_id, "download");
            if let Err(e) = commit_result(&conn, &track_id, db::Stage::Download, &dl_result) {
                emit(&app, &track_id, "download", "error", Some(format!("failed to write status: {e}")));
                return;
            }
        }
        match dl_result {
            Ok(_) => emit(&app, &track_id, "download", "done", None),
            Err(e) => { emit(&app, &track_id, "download", "error", Some(e)); return; }
        }
    }

    // --- Stage 2: stems ---
    if matches!(start_stage, StartStage::Download | StartStage::Stems) {
        let Some(demucs_bin) = setup::resolve_binary(&demucs_dir) else {
            emit(&app, &track_id, "stems", "error", Some("demucs not found".to_string()));
            return;
        };
        let cache_dir = setup::cache_dir(&demucs_dir);

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
            let conn = lock_or_abort!(&db, &app, &track_id, "stems");
            if let Err(e) = commit_result(&conn, &track_id, db::Stage::Stems, &stems_result) {
                emit(&app, &track_id, "stems", "error", Some(format!("failed to write status: {e}")));
                return;
            }
        }
        match stems_result {
            Ok(_) => emit(&app, &track_id, "stems", "done", None),
            Err(e) => { emit(&app, &track_id, "stems", "error", Some(e)); return; }
        }
    }

    // --- Stage 3: analysis ---
    if token.is_cancelled() { return; }
    // TODO: re-enable analysis once beat/note detection is ready (MVP v2)
    {
        let conn = lock_or_abort!(&db, &app, &track_id, "analysis");
        if let Err(e) = db::update_status(&conn, &track_id, db::Stage::Analysis, db::StageStatus::Done, None) {
            emit(&app, &track_id, "analysis", "error", Some(format!("failed to write status: {e}")));
            return;
        }
    }
    emit(&app, &track_id, "analysis", "done", None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Arc;

    fn open_mem() -> Connection {
        crate::db::open(Path::new(":memory:")).unwrap()
    }

    fn insert_pending(conn: &Connection, id: &str) {
        crate::db::insert_track(conn, &crate::db::Track {
            id: id.to_string(),
            title: "t".to_string(),
            source_type: "youtube".to_string(),
            source_url: None,
            source_path: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            sort_order: 1,
            duration_ms: None,
            status_download: "pending".to_string(),
            status_stems: "pending".to_string(),
            status_analysis: "pending".to_string(),
            error_message: None,
            export_path: None,
            artist: None,
        }).unwrap();
    }

    #[test]
    fn lock_db_returns_err_on_poisoned_mutex() {
        let db: Arc<Mutex<Connection>> = Arc::new(Mutex::new(open_mem()));
        let db2 = Arc::clone(&db);
        // Poison the mutex by panicking while holding the lock
        let _ = std::thread::spawn(move || {
            let _guard = db2.lock().unwrap();
            panic!("intentional panic to poison mutex");
        })
        .join();

        let result = lock_db(&db);
        assert_eq!(result.unwrap_err(), "database mutex is poisoned");
    }

    #[test]
    fn commit_result_ok_sets_done() {
        let conn = open_mem();
        insert_pending(&conn, "t1");
        assert!(commit_result(&conn, "t1", db::Stage::Download, &Ok(())).is_ok());
        let track = crate::db::get_track(&conn, "t1").unwrap().unwrap();
        assert_eq!(track.status_download, "done");
        assert_eq!(track.error_message, None);
    }

    #[test]
    fn commit_result_err_sets_error_and_message() {
        let conn = open_mem();
        insert_pending(&conn, "t2");
        assert!(commit_result(&conn, "t2", db::Stage::Stems, &Err("demucs crashed".to_string())).is_ok());
        let track = crate::db::get_track(&conn, "t2").unwrap().unwrap();
        assert_eq!(track.status_stems, "error");
        assert_eq!(track.error_message.as_deref(), Some("demucs crashed"));
    }

/// Simulate a DB failure mid-pipeline: drop the schema so the analysis
    /// status write fails, then verify the pipeline emits an error event
    /// rather than hanging or silently completing.
    #[tokio::test]
    async fn run_emits_error_event_when_db_write_fails() {
        use tauri::Listener;

        let app = tauri::test::mock_builder()
            .build(tauri::generate_context!())
            .unwrap();
        let handle = app.handle().clone();

        let (tx, rx) = std::sync::mpsc::channel::<String>();
        handle.listen("pipeline", move |e| {
            let _ = tx.send(e.payload().to_string());
        });

        // Destroy the schema so every DB write will fail with "no such table"
        let conn = crate::db::open(std::path::Path::new(":memory:")).unwrap();
        conn.execute("DROP TABLE tracks", []).unwrap();
        let db = Arc::new(Mutex::new(conn));

        // StartStage::Analysis skips download and stems (no yt-dlp / demucs needed)
        run(
            "t-fail".to_string(),
            Source::Local(std::path::PathBuf::from("/dev/null")),
            db,
            std::path::PathBuf::from("/tmp"),
            std::path::PathBuf::from("/tmp"),
            CancellationToken::new(),
            handle,
            StartStage::Analysis,
        )
        .await;

        // Events are dispatched synchronously in MockRuntime, so they are
        // already in the channel by the time run() returns.
        let payloads: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();

        assert!(
            payloads.iter().any(|p| {
                p.contains(r#""stage":"analysis""#) && p.contains(r#""status":"error""#)
            }),
            "expected analysis/error pipeline event, got: {payloads:?}"
        );
    }
}
