use std::path::{Path, PathBuf};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

/// Update this to your GitHub repo (owner/name).
const GITHUB_REPO: &str = "asawo/wavesplit";
const RELEASE_TAG: &str = "demucs-sidecar";

/// SHA-256 digests for each platform binary.
/// Update these each time build-demucs-sidecar.yml produces new binaries.
/// The workflow now prints the hash for each asset after upload — copy it here.
fn expected_sha256() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "0ddc40c14ce5d61a4d0ec8d6ba6af387665601133a36fe14c68086bffeefab31";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "a921410c07faebb6dc4fb61f1ce519a1e9719defc2656eaa789ab68ac8e0c35d";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "c92dbbeab6e2f4c8d83cb0babfbfd18110e3c7a43bd2f1938bb618b138681c5b";
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "f5d8613d97c7168c3a5be2160ef2ce3117bbed264dba57ee218344e9d6e403f4";
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
    let expected = expected_sha256();
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
