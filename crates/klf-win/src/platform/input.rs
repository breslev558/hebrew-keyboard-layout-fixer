//! Simulated keys (SendInput). Every call reports whether Windows accepted ALL events
//! (0 when blocked, e.g. towards an administrator window).

use std::mem::size_of;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

const VK_C: u16 = 0x43;
const VK_V: u16 = 0x56;

fn key(vk: u16, up: bool, extended: bool) -> INPUT {
    // SAFETY: MapVirtualKeyW has no pointer arguments.
    let scan = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC) } as u16;
    let mut flags = 0;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn unicode(unit: u16, up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 0, wScan: unit, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

/// Sends all events at once. If only part went through, the keys pressed are released again
/// so that Ctrl/Shift never stay "stuck".
fn send(inputs: &[INPUT]) -> bool {
    if inputs.is_empty() {
        return true;
    }
    // SAFETY: valid slice, correct structure size.
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        return true;
    }
    if sent > 0 {
        // SAFETY: reading the union field we wrote (keyboard inputs only).
        let release: Vec<INPUT> = inputs
            .iter()
            .filter_map(|i| unsafe {
                let k = i.Anonymous.ki;
                if k.dwFlags & KEYEVENTF_KEYUP == 0 && k.dwFlags & KEYEVENTF_UNICODE == 0 {
                    Some(key(k.wVk, true, k.dwFlags & KEYEVENTF_EXTENDEDKEY != 0))
                } else {
                    None
                }
            })
            .collect();
        // SAFETY: as above.
        unsafe {
            SendInput(release.len() as u32, release.as_ptr(), size_of::<INPUT>() as i32);
        }
    }
    false
}

fn combo(modifier: u16, k: u16, extended: bool) -> bool {
    send(&[key(modifier, false, false), key(k, false, extended), key(k, true, extended), key(modifier, true, false)])
}

pub fn copy() -> bool {
    combo(VK_CONTROL, VK_C, false)
}

pub fn paste() -> bool {
    combo(VK_CONTROL, VK_V, false)
}

/// Shift+Home: Home is an extended key (without the flag it can become "7" of the keypad).
pub fn select_to_line_start() -> bool {
    combo(VK_SHIFT, VK_HOME, true)
}

pub fn home() -> bool {
    send(&[key(VK_HOME, false, true), key(VK_HOME, true, true)])
}

pub fn end() -> bool {
    send(&[key(VK_END, false, true), key(VK_END, true, true)])
}

/// ONE SendInput: `backspaces` x Backspace, then the text as Unicode characters (consoles).
pub fn replace(backspaces: usize, text: &str) -> bool {
    let mut v = Vec::with_capacity(backspaces * 2 + text.len() * 4);
    for _ in 0..backspaces {
        v.push(key(VK_BACK, false, false));
        v.push(key(VK_BACK, true, false));
    }
    for unit in text.encode_utf16() {
        v.push(unicode(unit, false));
        v.push(unicode(unit, true));
    }
    send(&v)
}

fn down(vk: u16) -> bool {
    // SAFETY: no pointers.
    (unsafe { GetAsyncKeyState(vk as i32) } as u16 & 0x8000) != 0
}

/// A mouse button, Tab or Enter is down now, or was pressed since the previous call (bit 0 of
/// GetAsyncKeyState: indicative only, any other caller resets it). Sampled every 5 ms during
/// the waits of a conversion (audit B3).
/// `hotkey_vk` is left out (a hotkey with Enter or Tab would always look like a move).
pub fn user_moving(hotkey_vk: u16) -> bool {
    [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_TAB, VK_RETURN].iter().filter(|k| **k != hotkey_vk).any(|k| {
        // SAFETY: no pointers.
        let s = unsafe { GetAsyncKeyState(*k as i32) } as u16;
        s & 0x8001 != 0
    })
}

// Every press of Caps Lock, seen by a low-level keyboard hook when Windows dispatches it,
// whatever window has the focus (GetKeyState alone can be stale, audit 1.3.7 P2).
// The hook only COUNTS Caps Lock presses: no other key is looked at or kept.
static CAPS_PRESSES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static CAPS_HOOK_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

unsafe extern "system" fn caps_hook(code: i32, wp: usize, lp: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{CallNextHookEx, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_SYSKEYDOWN};
    // trivial on purpose: a slow hook is skipped, then removed, by Windows
    if code >= 0 && (wp as u32 == WM_KEYDOWN || wp as u32 == WM_SYSKEYDOWN) {
        // SAFETY: for code >= 0, lp points to a KBDLLHOOKSTRUCT (contract of WH_KEYBOARD_LL).
        if unsafe { (*(lp as *const KBDLLHOOKSTRUCT)).vkCode } == VK_CAPITAL as u32 {
            CAPS_PRESSES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
    // SAFETY: passes the event on unchanged.
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wp, lp) }
}

/// Starts counting Caps Lock presses (once; UI thread, which pumps messages). false = no hook:
/// Caps Lock is then never touched after F9.
pub fn watch_caps_lock() -> bool {
    use std::sync::atomic::Ordering::SeqCst;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowsHookExW, WH_KEYBOARD_LL};
    if CAPS_HOOK_ON.load(SeqCst) {
        return true;
    }
    // SAFETY: valid callback of this module; thread 0 = global (required for a low-level hook).
    let h = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(caps_hook), GetModuleHandleW(std::ptr::null()), 0) };
    let ok = !h.is_null();
    CAPS_HOOK_ON.store(ok, SeqCst);
    ok
}

/// Caps Lock presses counted since start; None when the hook is not in place.
pub fn caps_presses() -> Option<u32> {
    use std::sync::atomic::Ordering::SeqCst;
    CAPS_HOOK_ON.load(SeqCst).then(|| CAPS_PRESSES.load(SeqCst))
}

/// Caps Lock is on NOW, as seen by the thread that receives the keys (the window `hwnd`): our
/// thread shares its input state for the time of the reading (AttachThreadInput), so a change
/// made by the user since F9 is seen (audit 1.3.5, N3). None if it cannot be read.
pub fn caps_lock_on_now(hwnd: windows_sys::Win32::Foundation::HWND) -> Option<bool> {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsHungAppWindow};
    // SAFETY: thread ids only; detached right after the reading.
    unsafe {
        // a frozen window would give a stale state
        if IsHungAppWindow(hwnd) != 0 {
            return None;
        }
        let them = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
        let me = GetCurrentThreadId();
        if them == 0 || AttachThreadInput(me, them, 1) == 0 {
            return None;
        }
        let on = (GetKeyState(VK_CAPITAL as i32) & 1) != 0;
        AttachThreadInput(me, them, 0);
        Some(on)
    }
}

/// One press of Caps Lock (switches it off when it was on).
pub fn press_caps_lock() -> bool {
    send(&[key(VK_CAPITAL, false, false), key(VK_CAPITAL, true, false)])
}

/// Shift, Ctrl, Alt or Win physically held now (Win+Right would move the window).
pub fn modifiers_down() -> bool {
    [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN].iter().any(|k| down(*k))
}

/// "Last word" (1.4.1 trial): Right (collapses the selection to the caret), then Shift+Left x
/// `n`, all in ONE SendInput (nothing typed by the user can come in between).
pub fn select_last(n: usize) -> bool {
    let mut v = Vec::with_capacity(4 + n * 2);
    v.push(key(VK_RIGHT, false, true));
    v.push(key(VK_RIGHT, true, true));
    v.push(key(VK_SHIFT, false, false));
    for _ in 0..n {
        v.push(key(VK_LEFT, false, true));
        v.push(key(VK_LEFT, true, true));
    }
    v.push(key(VK_SHIFT, true, false));
    send(&v)
}

/// `n` x Right arrow (extended key), in one SendInput.
pub fn right(n: usize) -> bool {
    let mut v = Vec::with_capacity(n * 2);
    for _ in 0..n {
        v.push(key(VK_RIGHT, false, true));
        v.push(key(VK_RIGHT, true, true));
    }
    send(&v)
}

/// Waits until the hotkey and Ctrl/Alt/Shift/Win are released; false after `timeout_ms`.
pub fn wait_keys_released(hotkey_vk: u16, timeout_ms: u64) -> bool {
    let mut waited = 0;
    loop {
        let held = (hotkey_vk != 0 && down(hotkey_vk))
            || [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN].iter().any(|k| down(*k));
        if !held {
            return true;
        }
        if waited >= timeout_ms {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        waited += 10;
    }
}
