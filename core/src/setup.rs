use std::path::{Path, PathBuf};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

/// Update this to your GitHub repo (owner/name).
const GITHUB_REPO: &str = "asawo/wavesplit";
const RELEASE_TAG: &str = "demucs-sidecar";

async fn fetch_expected_sha256(
    client: &reqwest::Client,
    asset: &str,
) -> Result<String, String> {
    let url = format!(
        "https://github.com/{}/releases/download/{}/checksums.txt",
        GITHUB_REPO, RELEASE_TAG
    );
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("failed to fetch checksums.txt: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("failed to fetch checksums.txt: HTTP {}", response.status()));
    }
    let body = response
        .text()
        .await
        .map_err(|e| format!("failed to read checksums.txt: {e}"))?;
    for line in body.lines() {
        // Standard shasum format: "{hash}  {filename}" (two spaces)
        if let Some((hash, filename)) = line.split_once("  ") {
            if filename.trim() == asset {
                return Ok(hash.trim().to_string());
            }
        }
    }
    Err(format!("asset '{asset}' not found in checksums.txt"))
}

fn asset_name() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "demucs-macos-arm64";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "demucs-macos-x86_64";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "demucs-linux-x86_64";
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "demucs-windows-x86_64.exe";
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

/// Returns the path to the demucs binary on PATH, if found.
fn find_on_path() -> Option<PathBuf> {
    which::which("demucs").ok()
}

/// True if demucs is available — either as a bundled binary or on PATH.
pub fn is_available(demucs_dir: &Path) -> bool {
    binary_path(demucs_dir).exists() || find_on_path().is_some()
}

/// Returns the binary to invoke: bundled binary if downloaded, otherwise the one on PATH.
/// Returns None if demucs is not available at all.
pub fn resolve_binary(demucs_dir: &Path) -> Option<PathBuf> {
    let bundled = binary_path(demucs_dir);
    if bundled.exists() {
        return Some(bundled);
    }
    find_on_path()
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

    let client = reqwest::Client::new();
    let expected = fetch_expected_sha256(&client, asset_name()).await?;

    let url = format!(
        "https://github.com/{}/releases/download/{}/{}",
        GITHUB_REPO, RELEASE_TAG, asset_name()
    );
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
    let mut hasher = Sha256::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("download error: {e}"))?;
        downloaded += chunk.len() as u64;
        hasher.update(&chunk);

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

    let actual = hex::encode(hasher.finalize());
    if actual != expected {
        tokio::fs::remove_file(&tmp).await.ok();
        return Err(format!(
            "checksum mismatch for {}: expected {expected}, got {actual}",
            asset_name()
        ));
    }

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

    // macOS marks files downloaded via HTTP with a quarantine attribute that
    // Gatekeeper uses to block unsigned executables. Strip it so the binary
    // can actually be invoked after download.
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("xattr")
            .args(["-d", "com.apple.quarantine", dest.to_str().unwrap_or("")])
            .output()
            .ok();
    }

    Ok(())
}
