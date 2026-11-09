//! Process-level helpers: time, single instance, registry (autostart), sound, process names, elevation.

use crate::util::wide;
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Globalization::GetUserDefaultUILanguage;
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
use windows_sys::Win32::UI::WindowsAndMessaging::MB_OK;

/// High contrast is on (Windows accessibility): only system colours are used then.
pub fn high_contrast() -> bool {
    use windows_sys::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETHIGHCONTRAST};
    let mut hc = HIGHCONTRASTW { cbSize: std::mem::size_of::<HIGHCONTRASTW>() as u32, dwFlags: 0, lpszDefaultScheme: null_mut() };
    // SAFETY: structure of the right size, only written.
    let ok = unsafe { SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cbSize, (&mut hc as *mut HIGHCONTRASTW).cast(), 0) };
    ok != 0 && hc.dwFlags & HCF_HIGHCONTRASTON != 0
}

/// Local time "hh:mm:ss" (diagnostic).
pub fn local_clock() -> String {
    // SAFETY: GetLocalTime only writes the structure.
    let t = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    format!("{:02}:{:02}:{:02}", t.wHour, t.wMinute, t.wSecond)
}

fn reg_string(path: &str, name: &str) -> Option<String> {
    let key = wide(path);
    let val = wide(name);
    let mut buf = [0u16; 256];
    let mut size = (buf.len() * 2) as u32;
    // SAFETY: valid strings; size in bytes of our buffer; RRF_RT_REG_SZ checks the type.
    let r = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), val.as_ptr(), RRF_RT_REG_SZ, null_mut(), buf.as_mut_ptr().cast(), &mut size) };
    (r == ERROR_SUCCESS).then(|| crate::util::from_wide(&buf))
}

fn reg_dword(path: &str, name: &str) -> Option<u32> {
    let key = wide(path);
    let val = wide(name);
    let mut v: u32 = 0;
    let mut size = 4u32;
    // SAFETY: as above, 4-byte value.
    let r = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), val.as_ptr(), RRF_RT_REG_DWORD, null_mut(), (&mut v as *mut u32).cast(), &mut size) };
    (r == ERROR_SUCCESS).then_some(v)
}

/// Windows build number (0 if unreadable).
pub fn windows_build() -> u32 {
    reg_string("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "CurrentBuildNumber").and_then(|b| b.trim().parse().ok()).unwrap_or(0)
}

/// "Windows 11 (25H2, build 26200.1234)", None if unreadable (diagnostic).
pub fn windows_version() -> Option<String> {
    const CV: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion";
    let build: u32 = reg_string(CV, "CurrentBuildNumber")?.trim().parse().ok()?;
    let name = if build >= 22000 { "Windows 11" } else { "Windows 10" };
    let display = reg_string(CV, "DisplayVersion").unwrap_or_default();
    let ubr = reg_dword(CV, "UBR").map(|u| format!(".{u}")).unwrap_or_default();
    Some(if display.is_empty() { format!("{name} (build {build}{ubr})") } else { format!("{name} ({display}, build {build}{ubr})") })
}

pub fn local_time_string() -> String {
    // SAFETY: GetLocalTime only writes the structure.
    let t = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond, t.wMilliseconds
    )
}

/// One copy per user session (same mutex name as the .NET versions, so that both never run
/// together). With `wait_ms` (for --restart) the old copy is given time to quit.
pub struct SingleInstance(HANDLE);

impl SingleInstance {
    pub fn acquire(wait_ms: u32) -> Option<SingleInstance> {
        let name = wide("Local\\KeyboardLayoutFixer_SingleInstance");
        // SAFETY: valid NUL-terminated name; the handle is closed in Drop.
        unsafe {
            let h = CreateMutexW(null(), 0, name.as_ptr());
            if h.is_null() {
                return None;
            }
            match WaitForSingleObject(h, wait_ms) {
                WAIT_OBJECT_0 | WAIT_ABANDONED => Some(SingleInstance(h)),
                _ => {
                    CloseHandle(h);
                    None
                }
            }
        }
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        // SAFETY: we own the mutex and the handle.
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}

pub fn ui_langid() -> u16 {
    // SAFETY: no arguments.
    unsafe { GetUserDefaultUILanguage() }
}

pub fn beep() {
    // SAFETY: no pointers.
    unsafe {
        MessageBeep(MB_OK);
    }
}

/// 1.8.2: the short sound after a fix while typing: a standard Windows sound ("Notification"
/// event of the user's sound scheme), as a system sound: silent when Windows sounds are off or
/// muted, nothing when no sound is set. Played on a thread of its own, so that neither loading
/// the sound nor the audio service can ever slow down typing (the keyboard hook runs on the UI
/// thread); a sound already playing is not doubled.
pub fn fix_sound() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static PLAYING: AtomicBool = AtomicBool::new(false);
    if PLAYING.swap(true, Ordering::SeqCst) {
        return;
    }
    let spawned = std::thread::Builder::new().name("klf-sound".into()).spawn(|| {
        use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_NODEFAULT, SND_SYSTEM};
        let alias = wide("SystemNotification");
        // SAFETY: NUL-terminated alias that lives for the (synchronous) call.
        unsafe {
            PlaySoundW(alias.as_ptr(), null_mut(), SND_ALIAS | SND_NODEFAULT | SND_SYSTEM);
        }
        PLAYING.store(false, Ordering::SeqCst);
    });
    if spawned.is_err() {
        PLAYING.store(false, Ordering::SeqCst);
    }
}

/// Full path of the process exe (None if access is refused).
pub fn process_path(pid: u32) -> Option<String> {
    // SAFETY: handle checked and closed; buffer size passed.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        Some(crate::util::prefix(&buf, len as usize))
    }
}

/// Some(true) if the process runs elevated (administrator); None if it cannot be checked.
pub fn process_elevated(pid: u32) -> Option<bool> {
    // SAFETY: handles checked and closed; the output size matches TOKEN_ELEVATION.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut token: HANDLE = null_mut();
        let mut result = None;
        if OpenProcessToken(h, TOKEN_QUERY, &mut token) != 0 {
            let mut e = TOKEN_ELEVATION { TokenIsElevated: 0 };
            let mut ret = 0u32;
            if GetTokenInformation(
                token,
                TokenElevation,
                &mut e as *mut _ as *mut _,
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut ret,
            ) != 0
            {
                result = Some(e.TokenIsElevated != 0);
            }
            CloseHandle(token);
        }
        CloseHandle(h);
        result
    }
}

pub fn current_process_elevated() -> bool {
    // SAFETY: no pointers.
    process_elevated(unsafe { GetCurrentProcessId() }).unwrap_or(false)
}

// ---- autostart (Run keys) ----

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "KeyboardLayoutFixer";

fn reg_value_exists(root: HKEY, flags: REG_SAM_FLAGS) -> bool {
    let key = wide(RUN_KEY);
    let val = wide(RUN_VALUE);
    // SAFETY: valid strings; key closed.
    unsafe {
        let mut hk: HKEY = null_mut();
        if RegOpenKeyExW(root, key.as_ptr(), 0, KEY_QUERY_VALUE | flags, &mut hk) != ERROR_SUCCESS {
            return false;
        }
        let r = RegQueryValueExW(hk, val.as_ptr(), null(), null_mut(), null_mut(), null_mut());
        RegCloseKey(hk);
        r == ERROR_SUCCESS
    }
}

/// Installed for the whole PC by Installer.cmd (HKLM Run): the administrator manages the autostart.
pub fn autostart_managed() -> bool {
    reg_value_exists(HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY)
}

pub fn autostart_user() -> bool {
    reg_value_exists(HKEY_CURRENT_USER, 0)
}

pub fn set_autostart_user(on: bool, exe_path: &str) -> bool {
    let key = wide(RUN_KEY);
    let val = wide(RUN_VALUE);
    // SAFETY: valid strings; data length in bytes including the NUL; key closed.
    unsafe {
        let mut hk: HKEY = null_mut();
        if RegCreateKeyExW(HKEY_CURRENT_USER, key.as_ptr(), 0, null(), 0, KEY_SET_VALUE, null(), &mut hk, null_mut())
            != ERROR_SUCCESS
        {
            return false;
        }
        let r = if on {
            let data = wide(&format!("\"{exe_path}\""));
            RegSetValueExW(hk, val.as_ptr(), 0, REG_SZ, data.as_ptr() as *const u8, (data.len() * 2) as u32)
        } else {
            let r = RegDeleteValueW(hk, val.as_ptr());
            if r == ERROR_FILE_NOT_FOUND {
                ERROR_SUCCESS
            } else {
                r
            }
        };
        RegCloseKey(hk);
        r == ERROR_SUCCESS
    }
}

pub fn exe_path() -> String {
    std::env::current_exe().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

pub fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: buffer size passed.
    let n = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    if n <= 0 {
        return String::new();
    }
    crate::util::prefix(&buf, n as usize)
}
