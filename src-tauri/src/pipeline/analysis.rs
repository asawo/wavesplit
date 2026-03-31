use std::path::Path;
use std::process::Command;

/// Resolve the Poetry project directory containing pyproject.toml and analyze.py.
/// In dev: src-tauri/target/debug/wavesplit → ../../../src/analysis/
pub fn project_dir() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    // dev path: src-tauri/target/debug/wavesplit → up 4 levels to repo root, then src/analysis
    let dev_path = exe
        .parent().unwrap_or(Path::new("."))
        .parent().unwrap_or(Path::new("."))
        .parent().unwrap_or(Path::new("."))
        .parent().unwrap_or(Path::new("."))
        .join("src/analysis");
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
            "run", "python3",
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
