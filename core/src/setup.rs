use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

/// Update this to your GitHub repo (owner/name).
const GITHUB_REPO: &str = "asawo/wavesplit";
const RELEASE_TAG: &str = "demucs-sidecar";

async fn fetch_expected_sha256(client: &reqwest::Client, asset: &str) -> Result<String, String> {
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
        return Err(format!(
            "failed to fetch checksums.txt: HTTP {}",
            response.status()
        ));
    }
    let body = response
        .text()
        .await
        .map_err(|e| format!("failed to read checksums.txt: {e}"))?;
    find_checksum(&body, asset).ok_or_else(|| format!("asset '{asset}' not found in checksums.txt"))
}

/// Look up the SHA-256 hash for `asset` in a shasum-style checksums file.
///
/// Handles both output modes shasum tools emit:
/// - text mode: `"{hash}  {filename}"` (two spaces) — coreutils on macOS/Linux
/// - binary mode: `"{hash} *{filename}"` (one space + `*`) — Git-for-Windows'
///   `sha256sum`, which is what our Windows CI runner produces
///
/// Splitting on the first whitespace run and stripping a leading `*` marker
/// covers both, so a Windows sidecar entry is no longer missed.
fn find_checksum(body: &str, asset: &str) -> Option<String> {
    for line in body.lines() {
        let line = line.trim();
        if let Some((hash, rest)) = line.split_once(char::is_whitespace) {
            let filename = rest.trim_start().trim_start_matches('*');
            if filename == asset {
                return Some(hash.trim().to_string());
            }
        }
    }
    None
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

#[cfg(target_os = "macos")]
fn remove_quarantine(dest: &Path) -> Result<(), String> {
    let path_str = dest.to_str().ok_or_else(|| {
        format!(
            "cannot remove quarantine: path contains non-UTF-8 characters: {}",
            dest.to_string_lossy()
        )
    })?;

    let output = std::process::Command::new("xattr")
        .args(["-d", "com.apple.quarantine", path_str])
        .output()
        .map_err(|e| format!("xattr failed to start: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // A freshly-extracted binary may not carry the quarantine attribute.
        // xattr exits non-zero with "No such xattr" — treat that as a no-op success.
        if stderr.contains("No such xattr") {
            return Ok(());
        }
        return Err(format!("xattr failed: {stderr}"));
    }

    Ok(())
}

pub async fn download(demucs_dir: &Path, app: &AppHandle) -> Result<(), String> {
    std::fs::create_dir_all(demucs_dir).map_err(|e| format!("failed to create demucs dir: {e}"))?;

    let client = reqwest::Client::new();
    let expected = fetch_expected_sha256(&client, asset_name()).await?;

    let url = format!(
        "https://github.com/{}/releases/download/{}/{}",
        GITHUB_REPO,
        RELEASE_TAG,
        asset_name()
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

        if let Err(e) = app.emit(
            "setup:progress",
            DownloadProgress {
                downloaded_mb: downloaded as f64 / 1_048_576.0,
                total_mb: total_bytes.map(|t| t as f64 / 1_048_576.0),
                percent: total_bytes.map(|t| (downloaded * 100 / t) as u32),
            },
        ) {
            tracing::trace!(error = %e, "setup:progress emit failed");
        }
    }

    use tokio::io::AsyncWriteExt;
    file.flush()
        .await
        .map_err(|e| format!("flush error: {e}"))?;
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
    remove_quarantine(&dest)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::find_checksum;

    // A checksums.txt exactly as produced by the sidecar CI: macOS/Linux lines
    // are coreutils text mode (two spaces); the Windows line is Git-for-Windows
    // binary mode (one space + `*`).
    const MIXED_CHECKSUMS: &str = "\
3667b7b3fd64ea27f07b438514a75201cc6de094f6058fa76c8a121a9913646f  demucs-macos-arm64
5b47f31032a433b9fb544e03f13096dbab763b493e8ded99d68b42bfdc2a7e73  demucs-macos-x86_64
65e2c3a8439719cfcb5d9c17f32c472672c54e8c833cf416524f82e29faceb2b  demucs-linux-x86_64
81304de02cab6439b8dc72c82e09c7bf18c856f1afe66223d44de30ed5892e90 *demucs-windows-x86_64.exe
";

    #[test]
    fn find_checksum_parses_text_mode_two_spaces() {
        assert_eq!(
            find_checksum(MIXED_CHECKSUMS, "demucs-macos-arm64").as_deref(),
            Some("3667b7b3fd64ea27f07b438514a75201cc6de094f6058fa76c8a121a9913646f")
        );
    }

    #[test]
    fn find_checksum_parses_binary_mode_asterisk() {
        // Regression: the Windows line uses "<hash> *<file>" (one space + '*'),
        // which the old two-space split missed entirely.
        assert_eq!(
            find_checksum(MIXED_CHECKSUMS, "demucs-windows-x86_64.exe").as_deref(),
            Some("81304de02cab6439b8dc72c82e09c7bf18c856f1afe66223d44de30ed5892e90")
        );
    }

    #[test]
    fn find_checksum_handles_crlf_line_endings() {
        let body = "aa11  demucs-linux-x86_64\r\nbb22 *demucs-windows-x86_64.exe\r\n";
        assert_eq!(
            find_checksum(body, "demucs-windows-x86_64.exe").as_deref(),
            Some("bb22")
        );
    }

    #[test]
    fn find_checksum_returns_none_for_missing_asset() {
        assert_eq!(
            find_checksum(MIXED_CHECKSUMS, "demucs-freebsd-x86_64"),
            None
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn remove_quarantine_errors_on_non_utf8_path() {
        use super::remove_quarantine;
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        use std::path::Path;

        let non_utf8 = OsStr::from_bytes(&[0x80, 0x81]); // invalid UTF-8
        let path = Path::new(non_utf8);
        let result = remove_quarantine(path);
        assert!(result.is_err(), "expected Err for non-UTF8 path, got Ok");
        assert!(result.unwrap_err().contains("non-UTF-8"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn remove_quarantine_errors_when_xattr_fails() {
        use super::remove_quarantine;
        use std::path::Path;

        let result = remove_quarantine(Path::new("/nonexistent/path/demucs"));
        assert!(result.is_err(), "expected Err when xattr fails, got Ok");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn remove_quarantine_succeeds_when_attribute_missing() {
        use super::remove_quarantine;

        // A real file without the quarantine attribute — what we get after
        // bsdtar-extracting a tarball locally. xattr -d exits non-zero with
        // "No such xattr", which we should treat as success.
        let tmp = std::env::temp_dir().join(format!(
            "wavesplit_quarantine_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::write(&tmp, b"placeholder").expect("write tmp file");
        let result = remove_quarantine(&tmp);
        let _ = std::fs::remove_file(&tmp);
        assert!(
            result.is_ok(),
            "expected Ok when quarantine attr is missing, got {result:?}"
        );
    }
}
