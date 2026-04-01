use std::path::Path;
use std::process::Command;

use super::bins;

/// For a YouTube URL: download audio and convert to WAV at `dest`.
pub fn from_youtube(url: &str, dest: &Path) -> Result<(), String> {
    let ffmpeg = bins::resolve("ffmpeg");
    let output = Command::new(bins::resolve("yt-dlp"))
        .args([
            "--ffmpeg-location", ffmpeg.to_str().ok_or("invalid ffmpeg path")?,
            "-x",
            "--audio-format", "wav",
            "--audio-quality", "0",
            "-o", dest.to_str().ok_or("invalid dest path")?,
            url,
        ])
        .output()
        .map_err(|e| format!("yt-dlp not found or failed to start: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp failed: {stderr}"));
    }
    Ok(())
}

/// For a local file: convert to WAV at `dest` via ffmpeg.
/// If the file is already a WAV, copies it directly.
pub fn from_local(src: &Path, dest: &Path) -> Result<(), String> {
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    if ext == "wav" {
        std::fs::copy(src, dest)
            .map_err(|e| format!("failed to copy WAV: {e}"))?;
    } else {
        let output = Command::new(bins::resolve("ffmpeg"))
            .args([
                "-y",
                "-i", src.to_str().ok_or("invalid src path")?,
                "-ar", "44100",
                "-ac", "2",
                dest.to_str().ok_or("invalid dest path")?,
            ])
            .output()
            .map_err(|e| format!("ffmpeg not found or failed to start: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("ffmpeg failed: {stderr}"));
        }
    }
    Ok(())
}

/// Extract a title from a YouTube URL using yt-dlp (best-effort).
pub fn youtube_title(url: &str) -> Option<String> {
    let ffmpeg = bins::resolve("ffmpeg");
    let output = Command::new(bins::resolve("yt-dlp"))
        .args(["--ffmpeg-location", ffmpeg.to_str()?, "--print", "title", "--no-download", url])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Derive a display title from a local file path.
pub fn local_title(src: &Path) -> String {
    src.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}
