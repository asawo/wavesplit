use std::path::Path;

use super::bins;

/// yt-dlp args for downloading `url` as a WAV at `dest`.
fn download_args<'a>(ffmpeg_dir: &'a str, dest: &'a str, url: &'a str) -> Vec<&'a str> {
    vec![
        "--no-playlist",
        "--ffmpeg-location",
        ffmpeg_dir,
        "-x",
        "--audio-format",
        "wav",
        "--audio-quality",
        "0",
        "-o",
        dest,
        url,
    ]
}

/// yt-dlp args for printing the title of `url` without downloading it.
fn title_args<'a>(ffmpeg_dir: &'a str, url: &'a str) -> Vec<&'a str> {
    vec![
        "--no-playlist",
        "--ffmpeg-location",
        ffmpeg_dir,
        "--print",
        "title",
        "--no-download",
        url,
    ]
}

/// For a YouTube URL: download audio and convert to WAV at `dest`.
pub fn from_youtube(url: &str, dest: &Path) -> Result<(), String> {
    let ffmpeg = bins::resolve("ffmpeg");
    let ffmpeg_dir = ffmpeg
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or(ffmpeg.clone());
    let output = bins::command(bins::resolve("yt-dlp"))
        .args(download_args(
            ffmpeg_dir.to_str().ok_or("invalid ffmpeg path")?,
            dest.to_str().ok_or("invalid dest path")?,
            url,
        ))
        .output()
        .map_err(|e| format!("yt-dlp not found or failed to start: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "yt-dlp failed (ffmpeg-location={:?}): {stderr}",
            ffmpeg_dir
        ));
    }
    Ok(())
}

/// For a local file: convert to WAV at `dest` via ffmpeg.
/// If the file is already a WAV, copies it directly.
pub fn from_local(src: &Path, dest: &Path) -> Result<(), String> {
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "wav" {
        std::fs::copy(src, dest).map_err(|e| format!("failed to copy WAV: {e}"))?;
    } else {
        let output = bins::command(bins::resolve("ffmpeg"))
            .args([
                "-y",
                "-i",
                src.to_str().ok_or("invalid src path")?,
                "-ar",
                "44100",
                "-ac",
                "2",
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
    let ffmpeg_dir = ffmpeg
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or(ffmpeg.clone());
    let output = bins::command(bins::resolve("yt-dlp"))
        .args(title_args(ffmpeg_dir.to_str()?, url))
        .output()
        .map_err(|e| {
            tracing::debug!(error = %e, "yt-dlp title extraction failed");
            e
        })
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

/// Probe the duration of a media file in milliseconds via ffprobe.
///
/// Returns `Ok(None)` when ffprobe ran but the file has no parseable duration
/// (e.g. format=duration was "N/A"). Returns `Err` for execution failures
/// — callers should treat this as a non-fatal best-effort and continue.
pub fn probe_duration_ms(src: &Path) -> Result<Option<i64>, String> {
    let output = bins::command(bins::resolve("ffprobe"))
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            src.to_str().ok_or("invalid src path")?,
        ])
        .output()
        .map_err(|e| format!("ffprobe not found or failed to start: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffprobe failed: {stderr}"));
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("N/A") {
        return Ok(None);
    }
    let seconds: f64 = trimmed
        .parse()
        .map_err(|e| format!("ffprobe returned unparseable duration {trimmed:?}: {e}"))?;
    if !seconds.is_finite() || seconds < 0.0 {
        return Ok(None);
    }
    Ok(Some((seconds * 1000.0).round() as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WATCH_URL_IN_PLAYLIST: &str = "https://www.youtube.com/watch?v=abc&list=PLxyz";

    #[test]
    fn download_args_ignore_playlist_in_url() {
        let args = download_args("/bin", "/tmp/source.wav", WATCH_URL_IN_PLAYLIST);
        assert!(args.contains(&"--no-playlist"), "got: {args:?}");
    }

    #[test]
    fn title_args_ignore_playlist_in_url() {
        let args = title_args("/bin", WATCH_URL_IN_PLAYLIST);
        assert!(args.contains(&"--no-playlist"), "got: {args:?}");
    }
}
