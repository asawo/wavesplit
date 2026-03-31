use std::path::PathBuf;

/// Resolve a sidecar binary path.
///
/// In production, Tauri bundles sidecars next to the executable. We check
/// there first. In dev mode the sidecar won't be present, so we fall back
/// to the bare binary name and rely on PATH.
pub fn resolve(name: &str) -> PathBuf {
    #[cfg(windows)]
    let filename = format!("{name}.exe");
    #[cfg(not(windows))]
    let filename = name.to_string();

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(&filename);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    // Dev fallback: expect the tool on PATH
    PathBuf::from(name)
}
