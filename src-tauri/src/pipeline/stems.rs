use std::path::Path;
use std::process::Command;

use super::analysis;

/// Run Demucs on `source_wav`, writing 4 stems into `out_dir/{model}/{stem}.wav`.
/// We then move them into the flat `stems_dir` expected by the rest of the app.
pub fn separate(source_wav: &Path, stems_dir: &Path) -> Result<(), String> {
    // Demucs writes to: <out_dir>/htdemucs/<source_stem>/{bass,drums,vocals,other}.wav
    // We use a temp subdir then flatten.
    let tmp = stems_dir.parent()
        .ok_or("stems_dir has no parent")?
        .join("demucs_tmp");
    std::fs::create_dir_all(&tmp).map_err(|e| format!("mkdir demucs_tmp: {e}"))?;

    let project_dir = analysis::project_dir();
    let output = Command::new("poetry")
        .args([
            "run", "demucs",
            "--name", "htdemucs",
            "-o", tmp.to_str().ok_or("invalid tmp path")?,
            source_wav.to_str().ok_or("invalid source_wav path")?,
        ])
        .current_dir(&project_dir)
        .output()
        .map_err(|e| format!("poetry/demucs not found or failed to start: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("demucs failed: {stderr}"));
    }

    // Demucs output: tmp/htdemucs/<filename_without_ext>/{bass,drums,vocals,other}.wav
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
