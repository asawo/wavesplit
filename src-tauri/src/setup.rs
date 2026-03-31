use std::path::{Path, PathBuf};
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};

/// Update this to your GitHub repo (owner/name).
const GITHUB_REPO: &str = "arthurlechte/wavesplit";
const RELEASE_TAG: &str = "demucs-sidecar";

fn asset_name() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "demucs-aarch64-apple-darwin";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "demucs-x86_64-apple-darwin";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "demucs-x86_64-unknown-linux-gnu";
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "demucs-x86_64-pc-windows-msvc.exe";
}

pub fn binary_path(demucs_dir: &Path) -> PathBuf {
    #[cfg(windows)]
    return demucs_dir.join("demucs.exe");
    #[cfg(not(windows))]
    demucs_dir.join("demucs")
}

pub fn cache_dir(demucs_dir: &Path) -> PathBuf {
    demucs_dir.join("cache")
}

pub fn is_available(demucs_dir: &Path) -> bool {
    binary_path(demucs_dir).exists()
}

#[derive(Clone, serde::Serialize)]
struct DownloadProgress {
    downloaded_mb: f64,
    total_mb: Option<f64>,
    percent: Option<u32>,
}

pub async fn download(demucs_dir: &Path, app: &AppHandle) -> Result<(), String> {
    std::fs::create_dir_all(demucs_dir)
        .map_err(|e| format!("failed to create demucs dir: {e}"))?;

    let url = format!(
        "https://github.com/{}/releases/download/{}/{}",
        GITHUB_REPO, RELEASE_TAG, asset_name()
    );

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("download failed: HTTP {}", response.status()));
    }

    let total_bytes = response.content_length();
    let dest = binary_path(demucs_dir);
    let tmp = dest.with_extension("tmp");

    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| format!("failed to create temp file: {e}"))?;

    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("download error: {e}"))?;
        downloaded += chunk.len() as u64;

        use tokio::io::AsyncWriteExt;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("write error: {e}"))?;

        let _ = app.emit("setup:progress", DownloadProgress {
            downloaded_mb: downloaded as f64 / 1_048_576.0,
            total_mb: total_bytes.map(|t| t as f64 / 1_048_576.0),
            percent: total_bytes.map(|t| (downloaded * 100 / t) as u32),
        });
    }

    use tokio::io::AsyncWriteExt;
    file.flush().await.map_err(|e| format!("flush error: {e}"))?;
    drop(file);

    tokio::fs::rename(&tmp, &dest)
        .await
        .map_err(|e| format!("failed to move file: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest)
            .map_err(|e| e.to_string())?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms).map_err(|e| e.to_string())?;
    }

    Ok(())
}
