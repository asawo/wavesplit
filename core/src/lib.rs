mod commands;
pub mod constants;
mod db;
mod paths;
mod pipeline;
mod setup;

use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub data_dir: std::path::PathBuf,
    /// Directory that holds the frozen demucs binary and its model cache.
    /// Default: {app_data}/demucs/
    pub demucs_dir: std::path::PathBuf,
    /// Cancellation tokens for in-flight pipeline tasks, keyed by track ID.
    pub tasks: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

/// Holds the tracing-appender guard so log buffers flush on shutdown.
#[allow(dead_code)]
struct LogGuard(tracing_appender::non_blocking::WorkerGuard);

#[tauri::command]
fn list_tracks(state: tauri::State<AppState>) -> Result<Vec<db::Track>, String> {
    let conn = state
        .db
        .lock()
        .map_err(|_| "database unavailable".to_string())?;
    db::list_tracks(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_track(id: String, state: tauri::State<AppState>) -> Result<(), String> {
    let id = paths::parse_track_id(&id)?;
    // Cancel any in-flight pipeline task before touching the filesystem.
    if let Ok(mut tasks) = state.tasks.lock() {
        if let Some(token) = tasks.remove(&id) {
            token.cancel();
        }
    }
    let conn = state
        .db
        .lock()
        .map_err(|_| "database unavailable".to_string())?;
    let rows = db::delete_track(&conn, &id).map_err(|e| e.to_string())?;
    drop(conn);
    if rows == 0 {
        return Err("track not found".to_string());
    }
    let track_dir = paths::track_dir(&state.data_dir, &id);
    // If a symlink was somehow planted at the track dir path, unlink the entry itself rather
    // than recursing through it — remove_dir_all should never follow a top-level symlink out
    // of the tracks root. On Windows, directory-type reparse points must be removed via
    // RemoveDirectoryW (`remove_dir`), not DeleteFileW (`remove_file`); symlink_metadata's
    // `is_dir()` reports that without following the link (on Unix a symlink never reports as
    // a dir here, so this always falls through to `remove_file` there, which is correct).
    match std::fs::symlink_metadata(&track_dir) {
        Ok(meta) if meta.file_type().is_symlink() => {
            if meta.is_dir() {
                std::fs::remove_dir(&track_dir).map_err(|e| e.to_string())?;
            } else {
                std::fs::remove_file(&track_dir).map_err(|e| e.to_string())?;
            }
        }
        Ok(_) => std::fs::remove_dir_all(&track_dir).map_err(|e| e.to_string())?,
        Err(_) => {}
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
pub fn run(ctx: tauri::Context) {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(data_dir.join("tracks"))?;

            let log_dir = app
                .path()
                .app_log_dir()
                .unwrap_or_else(|_| data_dir.join("logs"));
            std::fs::create_dir_all(&log_dir).ok();
            let file_appender = tracing_appender::rolling::daily(&log_dir, "wavesplit.log");
            let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
            registry()
                .with(
                    fmt::Layer::new()
                        .json()
                        .with_writer(non_blocking)
                        .with_target(true)
                        .with_thread_ids(true),
                )
                .with(
                    fmt::Layer::new()
                        .with_writer(std::io::stderr)
                        .with_target(true)
                        .with_thread_ids(true),
                )
                .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
                .init();
            app.manage(LogGuard(guard));

            let demucs_dir = data_dir.join("demucs");
            std::fs::create_dir_all(&demucs_dir)?;

            let db_path = data_dir.join("wavesplit.db");
            let conn = db::open(&db_path)?;

            if let Err(e) = db::mark_interrupted(&conn) {
                tracing::warn!(error = %e, "failed to mark interrupted tracks");
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
            commands::retry_track,
            commands::get_stem_paths,
        ])
        .run(ctx)
        .expect("error while running tauri application");
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{Arc, Mutex};
    use tracing::field::{Field, Visit};
    use tracing::Event;
    use tracing::Subscriber;
    use tracing_subscriber::layer::Context;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::Layer;

    #[derive(Clone, Default)]
    pub struct TracingCapture {
        events: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl TracingCapture {
        pub fn new() -> (Self, tracing::subscriber::DefaultGuard) {
            let this = Self::default();
            let layer = CaptureLayer {
                capture: this.clone(),
            };
            let subscriber = tracing_subscriber::registry().with(layer);
            let guard = tracing::subscriber::set_default(subscriber);
            (this, guard)
        }

        pub fn events(&self) -> Vec<(String, String)> {
            self.events.lock().unwrap().clone()
        }

        pub fn contains(&self, level: &str, needle: &str) -> bool {
            self.events()
                .iter()
                .any(|(lvl, msg)| lvl == level && msg.contains(needle))
        }
    }

    struct CaptureLayer {
        capture: TracingCapture,
    }

    struct CaptureVisitor(String);

    impl Visit for CaptureVisitor {
        fn record_str(&mut self, field: &Field, value: &str) {
            if !self.0.is_empty() {
                self.0.push(' ');
            }
            use std::fmt::Write;
            write!(self.0, "{}={:?}", field.name(), value).unwrap();
        }

        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            if !self.0.is_empty() {
                self.0.push(' ');
            }
            use std::fmt::Write;
            write!(self.0, "{}={:?}", field.name(), value).unwrap();
        }
    }

    impl<S: Subscriber> Layer<S> for CaptureLayer {
        fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
            let mut visitor = CaptureVisitor(String::new());
            event.record(&mut visitor);
            let level = event.metadata().level().to_string();
            let mut events = self.capture.events.lock().unwrap();
            events.push((level, visitor.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::{test::mock_app, Manager};

    fn temp_data_dir(label: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("wavesplit-lib-test-{label}-{nanos}"));
        std::fs::create_dir_all(dir.join("tracks")).unwrap();
        dir
    }

    fn state_with_data_dir(data_dir: std::path::PathBuf) -> AppState {
        AppState {
            db: Arc::new(Mutex::new(
                db::open(std::path::Path::new(":memory:")).unwrap(),
            )),
            data_dir,
            demucs_dir: std::path::PathBuf::from("/tmp"),
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[test]
    fn delete_track_rejects_traversal_id() {
        let data_dir = temp_data_dir("traversal");
        let app = mock_app();
        app.manage(state_with_data_dir(data_dir.clone()));
        let result = delete_track("../../evil".to_string(), app.state::<AppState>());
        assert!(result.is_err());
        assert!(
            data_dir.join("tracks").read_dir().unwrap().next().is_none(),
            "tracks dir should be untouched"
        );
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn delete_track_rejects_absolute_path_id() {
        let data_dir = temp_data_dir("abs-path");
        let app = mock_app();
        app.manage(state_with_data_dir(data_dir.clone()));
        let result = delete_track("/etc/passwd".to_string(), app.state::<AppState>());
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn delete_track_returns_not_found_for_unknown_uuid() {
        let data_dir = temp_data_dir("unknown-uuid");
        let app = mock_app();
        app.manage(state_with_data_dir(data_dir.clone()));
        let id = "77777777-7777-7777-7777-777777777777".to_string();
        let result = delete_track(id, app.state::<AppState>());
        assert!(result.unwrap_err().contains("not found"));
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn delete_track_removes_existing_track() {
        let data_dir = temp_data_dir("removes-existing");
        let id = "88888888-8888-8888-8888-888888888888";
        let track_dir = data_dir.join("tracks").join(id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("source.wav"), b"stub").unwrap();

        let app = mock_app();
        app.manage(state_with_data_dir(data_dir.clone()));
        {
            let state = app.state::<AppState>();
            let conn = state.db.lock().unwrap();
            db::insert_track(
                &conn,
                &db::Track {
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
                },
            )
            .unwrap();
        }

        let result = delete_track(id.to_string(), app.state::<AppState>());
        assert!(result.is_ok(), "{:?}", result.err());
        assert!(!track_dir.exists());
        {
            let state = app.state::<AppState>();
            let conn = state.db.lock().unwrap();
            assert!(db::get_track(&conn, id).unwrap().is_none());
        }
        let _ = std::fs::remove_dir_all(&data_dir);
    }

    #[cfg(unix)]
    #[test]
    fn delete_track_unlinks_symlink_without_following_it() {
        use std::os::unix::fs::symlink;

        let data_dir = temp_data_dir("symlink-escape");
        let id = "99999999-9999-9999-9999-999999999999";

        let target_dir = std::env::temp_dir().join(format!(
            "wavesplit-lib-test-symlink-target-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&target_dir).unwrap();
        let marker = target_dir.join("marker.txt");
        std::fs::write(&marker, b"do not delete me").unwrap();

        let track_dir = data_dir.join("tracks").join(id);
        symlink(&target_dir, &track_dir).unwrap();

        let app = mock_app();
        app.manage(state_with_data_dir(data_dir.clone()));
        {
            let state = app.state::<AppState>();
            let conn = state.db.lock().unwrap();
            db::insert_track(
                &conn,
                &db::Track {
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
                },
            )
            .unwrap();
        }

        let result = delete_track(id.to_string(), app.state::<AppState>());
        assert!(result.is_ok(), "{:?}", result.err());
        assert!(
            !track_dir.exists(),
            "symlink at the track dir path should be removed"
        );
        assert!(
            marker.exists(),
            "the symlink target's contents must not be touched"
        );

        let _ = std::fs::remove_dir_all(&data_dir);
        let _ = std::fs::remove_dir_all(&target_dir);
    }

    // Windows distinguishes directory-type reparse points from file symlinks at the deletion
    // API level (RemoveDirectoryW vs DeleteFileW); this mirrors the Unix test above using
    // `symlink_dir` to make sure that distinction is handled.
    #[cfg(windows)]
    #[test]
    fn delete_track_unlinks_directory_symlink_without_following_it() {
        use std::os::windows::fs::symlink_dir;

        let data_dir = temp_data_dir("symlink-escape-windows");
        let id = "99999999-9999-9999-9999-999999999998";

        let target_dir = std::env::temp_dir().join(format!(
            "wavesplit-lib-test-symlink-target-windows-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&target_dir).unwrap();
        let marker = target_dir.join("marker.txt");
        std::fs::write(&marker, b"do not delete me").unwrap();

        let track_dir = data_dir.join("tracks").join(id);
        // Directory symlink creation requires developer mode or admin privileges on Windows;
        // skip rather than fail the suite in restricted CI/dev environments.
        if symlink_dir(&target_dir, &track_dir).is_err() {
            let _ = std::fs::remove_dir_all(&data_dir);
            let _ = std::fs::remove_dir_all(&target_dir);
            return;
        }

        let app = mock_app();
        app.manage(state_with_data_dir(data_dir.clone()));
        {
            let state = app.state::<AppState>();
            let conn = state.db.lock().unwrap();
            db::insert_track(
                &conn,
                &db::Track {
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
                },
            )
            .unwrap();
        }

        let result = delete_track(id.to_string(), app.state::<AppState>());
        assert!(result.is_ok(), "{:?}", result.err());
        assert!(
            !track_dir.exists(),
            "directory symlink at the track dir path should be removed"
        );
        assert!(
            marker.exists(),
            "the symlink target's contents must not be touched"
        );

        let _ = std::fs::remove_dir_all(&data_dir);
        let _ = std::fs::remove_dir_all(&target_dir);
    }
}
