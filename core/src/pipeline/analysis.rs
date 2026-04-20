use std::path::Path;
use std::process::Command;

/// Resolve the Poetry project directory containing pyproject.toml and analyze.py.
/// In dev: core/target/debug/wavesplit → ../../../python/
pub fn project_dir() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
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
/// Produces `analysis/analysis.json`.
pub fn run(stems_dir: &Path, source_wav: &Path, analysis_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(analysis_dir).map_err(|e| format!("mkdir analysis_dir: {e}"))?;

    let project_dir = project_dir();
    let script = project_dir.join("analyze.py");
    if !script.exists() {
        return Err(format!("analyze.py not found at {}", script.display()));
    }

    let output = Command::new("poetry")
        .args([
            "run",
            "python3",
            script.to_str().ok_or("invalid script path")?,
            stems_dir.to_str().ok_or("invalid stems_dir path")?,
            source_wav.to_str().ok_or("invalid source_wav path")?,
            analysis_dir.to_str().ok_or("invalid analysis_dir path")?,
        ])
        .current_dir(&project_dir)
        .output()
        .map_err(|e| format!("poetry not found or failed to start: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if !stderr.trim().is_empty() {
            stderr.trim().to_string()
        } else if !stdout.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            format!("exit status: {}", output.status)
        };
        return Err(format!("analyze.py failed: {detail}"));
    }

    Ok(())
}
