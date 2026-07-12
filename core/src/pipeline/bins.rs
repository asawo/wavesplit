use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

/// Build a `Command` that does not spawn a console window on Windows.
///
/// On non-Windows platforms this is just `Command::new`. On Windows the app is
/// built with `windows_subsystem = "windows"`, which hides the app's own
/// console but not those of child processes — so each subprocess (yt-dlp,
/// ffmpeg, ffprobe, demucs) would flash a console window without this flag.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

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
