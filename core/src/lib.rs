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
