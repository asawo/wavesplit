#![allow(dead_code)] // stubbed — will be used in MVP v2 (beat/note analysis)

use std::path::Path;
use std::process::Command;

/// Resolve the Poetry project directory containing pyproject.toml and analyze.py.
/// In dev: core/target/debug/wavesplit → ../../../python/
pub fn project_dir() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to get current exe path");
        std::path::PathBuf::new()
    });
    // dev path: core/target/debug/wavesplit → up 4 levels to repo root, then python/
    let dev_path = exe
        .parent()
        .unwrap_or(Path::new("."))
        .parent()
        .unwrap_or(Path::new("."))
        .parent()
        .unwrap_or(Path::new("."))
        .parent()
        .unwrap_or(Path::new("."))
        .join("python");
    if dev_path.join("pyproject.toml").exists() {
        return dev_path;
    }
    // production: script lives next to the binary
    exe.parent().unwrap_or(Path::new(".")).to_path_buf()
}

/// Run the Python analysis script via `poetry run` inside the analysis project.
/// Produces `analysis/timing.json` and `analysis/stems.json`.
pub fn run(stems_dir: &Path, analysis_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(analysis_dir).map_err(|e| format!("mkdir analysis_dir: {e}"))?;

    let project_dir = project_dir();
    let script = project_dir.join("analyze.py");
    if !script.exists() {
        return Err(format!("analyze.py not found at {}", script.display()));
    }

    let status = Command::new("poetry")
        .args([
            "run",
            "python3",
            script.to_str().ok_or("invalid script path")?,
            stems_dir.to_str().ok_or("invalid stems_dir path")?,
            analysis_dir.to_str().ok_or("invalid analysis_dir path")?,
        ])
        .current_dir(&project_dir)
        .status()
        .map_err(|e| format!("poetry not found or failed to start: {e}"))?;

    if !status.success() {
        return Err(format!("analyze.py exited with status {status}"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_dir_returns_non_empty_path() {
        let dir = project_dir();
        assert!(
            !dir.as_os_str().is_empty(),
            "project_dir should return a non-empty path"
        );
    }

    #[test]
    fn project_dir_does_not_log_warning_when_exe_available() {
        let (capture, _guard) = crate::test_support::TracingCapture::new();
        let _dir = project_dir();
        assert!(
            !capture.contains("WARN", "failed to get current exe path"),
            "expected no warning on happy path, got: {:?}",
            capture.events()
        );
    }

    #[test]
    fn run_errors_when_script_missing() {
        let project_dir = project_dir();
        // Use the project dir's parent as a fake stems/analysis dir to ensure
        // the script path won't match.
        let fake_dir = project_dir.join("nonexistent_subdir_xyz");
        let result = run(Path::new("/tmp"), &fake_dir);
        assert!(
            result.is_err(),
            "expected Err when analyze.py is missing, got Ok"
        );
        assert!(
            result.unwrap_err().contains("analyze.py not found"),
            "error should mention missing script"
        );
    }
}
