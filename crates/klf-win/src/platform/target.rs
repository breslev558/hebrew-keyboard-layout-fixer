//! The window that had the focus: class, program, elevation, focused control, selection, password,
//! caret position (for the bubble), keyboard language switch.

use super::sys;
use klf_core::flow::TargetInfo;
use klf_core::policy::{self, Selection};
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayoutList, HKL};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub fn foreground() -> HWND {
    // SAFETY: no arguments.
    unsafe { GetForegroundWindow() }
}

fn thread_info(tid: u32) -> Option<GUITHREADINFO> {
    // SAFETY: cbSize set; the structure is only written.
    unsafe {
        let mut gi: GUITHREADINFO = std::mem::zeroed();
        gi.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(tid, &mut gi) != 0 {
            Some(gi)
        } else {
            None
        }
    }
}

const ES_PASSWORD: u32 = 0x0020;
const EM_GETSEL: u32 = 0x00B0;

/// EM_GETSEL with null pointers is the only safe form between processes (no pointer to marshal).
fn edit_selection(hwnd: HWND) -> Selection {
    let mut result: usize = 0;
    // SAFETY: SendMessageTimeoutW with a timeout: a frozen program cannot block us.
    let ok = unsafe { SendMessageTimeoutW(hwnd, EM_GETSEL, 0, 0, SMTO_ABORTIFHUNG | SMTO_BLOCK, 250, &mut result) };
    if ok == 0 {
        return Selection::Unknown;
    }
    policy::selection_from_em_getsel(result)
}

fn is_edit_class(class: &str) -> bool {
    let c = class.to_ascii_lowercase();
    c == "edit" || c.starts_with("richedit")
}

/// Focused control of the thread owning `hwnd` (null if unknown).
pub fn focus_of(hwnd: HWND) -> HWND {
    // SAFETY: pid not needed (null).
    let tid = unsafe { GetWindowThreadProcessId(hwnd, null_mut()) };
    thread_info(tid).map(|g| g.hwndFocus).unwrap_or(null_mut())
}

/// Process of a window; for a Store app, the process of the app itself: the frame host
/// (ApplicationFrameHost.exe, shared by every Store app) holds a child CoreWindow of the app.
pub fn real_pid(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    // SAFETY: pid is written.
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    let exe = sys::process_path(pid).map(|p| policy::exe_base_name(&p)).unwrap_or_default();
    if exe != "applicationframehost.exe" {
        return pid;
    }
    struct Find {
        host: u32,
        found: u32,
    }
    unsafe extern "system" fn each(child: HWND, lparam: LPARAM) -> windows_sys::core::BOOL {
        // SAFETY: lparam is the &mut Find passed below, alive during EnumChildWindows.
        let f = unsafe { &mut *(lparam as *mut Find) };
        if sys::class_name(child) == "Windows.UI.Core.CoreWindow" {
            let mut p = 0u32;
            // SAFETY: p written.
            unsafe { GetWindowThreadProcessId(child, &mut p) };
            if p != 0 && p != f.host {
                f.found = p;
                return 0;
            }
        }
        1
    }
    let mut f = Find { host: pid, found: 0 };
    // SAFETY: callback above; `f` outlives the call.
    unsafe { EnumChildWindows(hwnd, Some(each), &mut f as *mut Find as LPARAM) };
    if f.found != 0 { f.found } else { pid }
}

/// Program name (lower case, no path) of a window, Store apps included; empty if unknown.
pub fn exe_of(hwnd: HWND) -> String {
    sys::process_path(real_pid(hwnd)).map(|p| policy::exe_base_name(&p)).unwrap_or_default()
}

pub fn info(hwnd: HWND) -> TargetInfo {
    let mut _frame_pid = 0u32;
    // SAFETY: pid is written.
    let tid = unsafe { GetWindowThreadProcessId(hwnd, &mut _frame_pid) };
    let pid = real_pid(hwnd);
    let window_class = sys::class_name(hwnd);
    let exe_name = sys::process_path(pid).map(|p| policy::exe_base_name(&p)).unwrap_or_default();
    // checking elevation only matters if we are not elevated ourselves
    let elevated = if sys::current_process_elevated() { Some(false) } else { sys::process_elevated(pid) };
    let mut out = TargetInfo { window_class, exe_name, elevated, ..Default::default() };
    if let Some(gi) = thread_info(tid) {
        let focus = gi.hwndFocus;
        if !focus.is_null() {
            out.focus_class = sys::class_name(focus);
            if is_edit_class(&out.focus_class) {
                // SAFETY: reading the style of a window is allowed across processes.
                let style = unsafe { GetWindowLongW(focus, GWL_STYLE) } as u32;
                // Edit and RichEdit controls (browsers, WPF and others: their password
                // fields refuse Ctrl+C, so nothing is copied there)
                out.password_field = style & ES_PASSWORD != 0;
                out.selection = Some(edit_selection(focus));
            }
        }
    }
    out
}

/// Processes allowed to own the clipboard after our Ctrl+C: the target window, its focused
/// control and its child windows (a UWP app's CoreWindow lives under ApplicationFrameHost).
pub fn pids_of(hwnd: HWND) -> Vec<u32> {
    unsafe extern "system" fn collect(child: HWND, lparam: LPARAM) -> windows_sys::core::BOOL {
        // SAFETY: lparam is the &mut Vec<u32> passed below, alive during EnumChildWindows.
        let v = unsafe { &mut *(lparam as *mut Vec<u32>) };
        let mut pid = 0u32;
        // SAFETY: pid written.
        unsafe { GetWindowThreadProcessId(child, &mut pid) };
        if pid != 0 && !v.contains(&pid) && v.len() < 64 {
            v.push(pid);
        }
        1
    }
    let mut v: Vec<u32> = Vec::new();
    for w in [hwnd, focus_of(hwnd)] {
        let mut pid = 0u32;
        // SAFETY: pid written.
        unsafe { GetWindowThreadProcessId(w, &mut pid) };
        if pid != 0 && !v.contains(&pid) {
            v.push(pid);
        }
    }
    // SAFETY: callback above; the Vec outlives the call.
    unsafe { EnumChildWindows(hwnd, Some(collect), &mut v as *mut Vec<u32> as LPARAM) };
    v
}

/// Caret window and rectangle (left, top, right, bottom).
pub type Caret = (usize, i32, i32, i32, i32);

/// Window and rectangle of the text caret of the thread owning `hwnd` (None if no caret).
pub fn caret_of(hwnd: HWND) -> Option<Caret> {
    // SAFETY: pid not needed (null).
    let tid = unsafe { GetWindowThreadProcessId(hwnd, null_mut()) };
    let gi = thread_info(tid)?;
    if gi.hwndCaret.is_null() {
        return None;
    }
    let r = gi.rcCaret;
    Some((gi.hwndCaret as usize, r.left, r.top, r.right, r.bottom))
}

/// Screen position under the text caret of the foreground thread (for the bubble).
pub fn caret_point(hwnd: HWND) -> Option<(i32, i32)> {
    // SAFETY: pid not needed (null).
    let tid = unsafe { GetWindowThreadProcessId(hwnd, null_mut()) };
    let gi = thread_info(tid)?;
    if gi.hwndCaret.is_null() {
        return None;
    }
    let mut p = POINT { x: gi.rcCaret.left, y: gi.rcCaret.bottom };
    // SAFETY: valid window and point.
    if unsafe { ClientToScreen(gi.hwndCaret, &mut p) } == 0 || (p.x == 0 && p.y == 0) {
        return None;
    }
    Some((p.x, p.y))
}

/// Asks the window to switch to the Hebrew or English keyboard (WM_INPUTLANGCHANGEREQUEST),
/// better than a simulated Alt+Shift: Word then tags the text with the right language.
/// Keyboard layout (HKL) of the thread owning `hwnd`.
pub fn current_layout(hwnd: HWND) -> Option<usize> {
    // SAFETY: pid not needed (null).
    let tid = unsafe { GetWindowThreadProcessId(hwnd, null_mut()) };
    if tid == 0 {
        return None; // GetKeyboardLayout(0) would be OUR thread's layout
    }
    // SAFETY: a thread id.
    let h = unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout(tid) };
    (!h.is_null()).then_some(h as usize)
}

/// Asks the window to switch to this keyboard layout.
pub fn request_layout(hwnd: HWND, hkl: usize) {
    // SAFETY: posting a message, no pointers.
    unsafe {
        PostMessageW(hwnd, WM_INPUTLANGCHANGEREQUEST, 0, hkl as isize);
    }
}

pub fn switch_language(hwnd: HWND, to_hebrew: bool) {
    let mut list: [HKL; 32] = [null_mut(); 32];
    // SAFETY: buffer size passed.
    let n = unsafe { GetKeyboardLayoutList(list.len() as i32, list.as_mut_ptr()) };
    let wanted: u16 = if to_hebrew { 0x0D } else { 0x09 };
    let found = list.iter().take(n.max(0) as usize).find(|h| ((**h as usize) & 0x3FF) as u16 == wanted);
    if let Some(hkl) = found {
        // SAFETY: posting a message, no pointers.
        unsafe {
            PostMessageW(hwnd, WM_INPUTLANGCHANGEREQUEST, 0, *hkl as isize);
        }
    }
}
