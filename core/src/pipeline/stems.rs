use std::path::Path;
use std::process::Command;

/// Run Demucs on `source_wav`, writing 4 stems into `stems_dir`.
/// `demucs_bin` is the path to the frozen demucs executable.
/// `cache_dir` is used as TORCH_HOME so model weights are stored in the app
/// data directory rather than the user's home directory.
pub fn separate(
    source_wav: &Path,
    stems_dir: &Path,
    demucs_bin: &Path,
    cache_dir: &Path,
) -> Result<(), String> {
    // Demucs writes to: <tmp>/htdemucs/<source_stem>/{bass,drums,vocals,other}.wav
    let tmp = stems_dir
        .parent()
        .ok_or("stems_dir has no parent")?
        .join("demucs_tmp");
    std::fs::create_dir_all(&tmp).map_err(|e| format!("mkdir demucs_tmp: {e}"))?;
    std::fs::create_dir_all(cache_dir).map_err(|e| format!("mkdir cache_dir: {e}"))?;

    let output = Command::new(demucs_bin)
        .args([
            "--name", "htdemucs",
            "-o", tmp.to_str().ok_or("invalid tmp path")?,
            source_wav.to_str().ok_or("invalid source_wav path")?,
        ])
        .env("TORCH_HOME", cache_dir)
        .output()
        .map_err(|e| format!("demucs failed to start: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("demucs failed: {stderr}"));
    }

    let source_stem = source_wav
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("invalid source filename")?;

    let demucs_out = tmp.join("htdemucs").join(source_stem);
    std::fs::create_dir_all(stems_dir).map_err(|e| format!("mkdir stems_dir: {e}"))?;

    for stem_name in &["bass", "drums", "vocals", "other"] {
        let src = demucs_out.join(format!("{stem_name}.wav"));
        let dst = stems_dir.join(format!("{stem_name}.wav"));
        std::fs::rename(&src, &dst)
            .map_err(|e| format!("failed to move {stem_name}.wav: {e}"))?;
    }

    std::fs::remove_dir_all(&tmp).map_err(|e| format!("cleanup demucs_tmp: {e}"))?;

    Ok(())
}
