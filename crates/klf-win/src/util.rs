//! Small helpers: UTF-16 strings, paths, debug.log (never contains typed or copied text).

use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// UTF-16 with the final NUL, for Win32 *W functions.
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Reads a NUL-terminated UTF-16 buffer.
pub fn from_wide(buf: &[u16]) -> String {
    String::from_utf16_lossy(buf.split(|&c| c == 0).next().unwrap_or_default())
}

/// The first `n` units of a buffer filled by Windows (never panics, even if `n` is wrong).
pub fn prefix(buf: &[u16], n: usize) -> String {
    from_wide(buf.get(..n.min(buf.len())).unwrap_or_default())
}

/// %APPDATA%\KeyboardLayoutFixer (same folder as the .NET versions: settings.json + debug.log).
pub fn app_dir() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")?;
    let dir = PathBuf::from(base).join("KeyboardLayoutFixer");
    let _ = fs::create_dir_all(&dir);
    Some(dir)
}

pub fn settings_path() -> Option<PathBuf> {
    app_dir().map(|d| d.join("settings.json"))
}

/// Same file and same first messages as 1.2.x ("Hotkey registered: F10"...): the school's
/// check script reads them. Kept under 256 KB (+ .old).
pub fn log(message: &str) {
    let Some(dir) = app_dir() else { return };
    let path = dir.join("debug.log");
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > 256 * 1024 {
            let old = dir.join("debug.log.old");
            let _ = fs::remove_file(&old);
            let _ = fs::rename(&path, &old);
        }
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "[{}] {}", crate::platform::sys::local_time_string(), message);
    }
}
