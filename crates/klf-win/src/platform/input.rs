//! Simulated keys (SendInput). Every call reports whether Windows accepted ALL events
//! (0 when blocked, e.g. towards an administrator window).

use std::mem::size_of;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

const VK_C: u16 = 0x43;
const VK_V: u16 = 0x56;

/// 1.8.5: every key this program sends carries this value, so that keys sent by other programs
/// (on-screen keyboard, AutoHotkey, remote tools) are still counted as the user's.
pub const OUR_MARK: usize = 0x4B4C_4658; // "KLFX"

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
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: OUR_MARK } },
    }
}

fn unicode(unit: u16, up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 0, wScan: unit, dwFlags: flags, time: 0, dwExtraInfo: OUR_MARK } },
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

/// 1.8.5: Shift+Up then Shift+Home, in ONE SendInput (the selection grows by one displayed line).
pub fn select_up_to_line_start() -> bool {
    send(&[
        key(VK_SHIFT, false, false),
        key(VK_UP, false, true),
        key(VK_UP, true, true),
        key(VK_HOME, false, true),
        key(VK_HOME, true, true),
        key(VK_SHIFT, true, false),
    ])
}

/// 1.8.7: Delete (takes an autocompletion suggestion away).
pub fn delete_key() -> bool {
    send(&[key(VK_DELETE, false, true), key(VK_DELETE, true, true)])
}

/// 1.8.6: Shift+Left `n` times (the text just pasted selected again).
pub fn select_back(n: usize) -> bool {
    let mut v = Vec::with_capacity(2 + n * 2);
    v.push(key(VK_SHIFT, false, false));
    for _ in 0..n {
        v.push(key(VK_LEFT, false, true));
        v.push(key(VK_LEFT, true, true));
    }
    v.push(key(VK_SHIFT, true, false));
    send(&v)
}

/// 1.8.5: Right arrow: the selection collapses to its end (where the caret was).
pub fn collapse_right() -> bool {
    send(&[key(VK_RIGHT, false, true), key(VK_RIGHT, true, true)])
}

/// 1.8.5: Shift+Down (one displayed line back).
pub fn select_down() -> bool {
    combo(VK_SHIFT, VK_DOWN, true)
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

/// 1.8.5: the same, at a gentle pace (the Windows 11 Notepad loses characters sent too fast):
/// the Backspaces, a short pause, then the text 8 characters at a time. A few milliseconds in all
/// for a word. false (nothing more sent) as soon as one part is refused.
pub fn replace_paced(backspaces: usize, text: &str) -> bool {
    let mut v = Vec::with_capacity(backspaces * 2);
    for _ in 0..backspaces {
        v.push(key(VK_BACK, false, false));
        v.push(key(VK_BACK, true, false));
    }
    if !v.is_empty() && !send(&v) {
        return false;
    }
    std::thread::sleep(std::time::Duration::from_millis(8));
    let units: Vec<u16> = text.encode_utf16().collect();
    for chunk in units.chunks(8) {
        let mut v = Vec::with_capacity(chunk.len() * 2);
        for &u in chunk {
            v.push(unicode(u, false));
            v.push(unicode(u, true));
        }
        if !send(&v) {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    true
}

/// 1.8.5: `replace_paced` on a worker thread (the UI thread runs the keyboard hooks and must
/// never sleep). Calls are serialised: one sending at a time, in order.
pub fn replace_paced_async(backspaces: usize, text: String) {
    replace_paced_then(backspaces, text, || {});
}

static TYPING: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// 1.8.6: text is being typed by a worker thread right now.
pub fn typing_in_progress() -> bool {
    TYPING.load(std::sync::atomic::Ordering::SeqCst) > 0
}

pub fn replace_paced_then(backspaces: usize, text: String, then: impl FnOnce() + Send + 'static) {
    use std::sync::atomic::Ordering::SeqCst;
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TYPING.fetch_add(1, SeqCst);
    let started = std::thread::Builder::new().name("klf-type".into()).spawn(move || {
        let _g = ONE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let ok = replace_paced(backspaces, &text);
        TYPING.fetch_sub(1, SeqCst);
        if ok {
            then();
        }
    });
    if started.is_err() {
        TYPING.fetch_sub(1, SeqCst);
    }
}

/// 1.8.5: the counting hook may have missed input (removed by Windows and not yet put back).
pub fn counting_unsure() -> bool {
    use std::sync::atomic::Ordering::SeqCst;
    !CAPS_HOOK_ON.load(SeqCst) || hook_lost(LAST_EVENT.load(SeqCst))
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
    let clicked = [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_TAB, VK_RETURN].iter().filter(|k| **k != hotkey_vk).any(|k| {
        // SAFETY: no pointers.
        let s = unsafe { GetAsyncKeyState(*k as i32) } as u16;
        s & 0x8001 != 0
    });
    // arrows and page keys held down by the user (audit A03): only "down now" (bit 15), because
    // the "only the last word" mode sends Shift+Left/Right itself (key up sent at once)
    clicked
        || [VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN, VK_PRIOR, VK_NEXT].iter().filter(|k| **k != hotkey_vk).any(|k| {
            // SAFETY: no pointers.
            (unsafe { GetAsyncKeyState(*k as i32) } as u16) & 0x8000 != 0
        })
}

// Every press of Caps Lock, seen by a low-level keyboard hook when Windows dispatches it,
// whatever window has the focus (GetKeyState alone can be stale, audit 1.3.7 P2).
// The hook only COUNTS Caps Lock presses: no other key is looked at or kept.
static CAPS_PRESSES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// 1.8.5: how many keys the USER pressed (not injected; the layout hotkey, modifiers and Caps Lock
/// left out) and how
/// many mouse clicks: only counted, never which key. A quick second F10 toggles the text just
/// pasted only if nothing but the hotkey was pressed in between.
static USER_KEYS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static USER_CLICKS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
/// the layout hotkey (F10 by default): its own presses are not "other keys"
static HOTKEY_VK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn set_hotkey_vk(vk: u32) {
    HOTKEY_VK.store(vk, std::sync::atomic::Ordering::SeqCst);
}

/// 1.8.5: last event seen by the hooks of this module (GetTickCount, ms) and their handles, to see
/// whether Windows removed them (it does, silently, when a hook answers too slowly).
static LAST_EVENT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static CAPS_HHOOK: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static CLICK_HHOOK: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn tick32() -> u32 {
    // SAFETY: no arguments.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount() }
}

/// When the last keyboard or mouse input of the session happened (GetTickCount, ms).
pub fn last_input_tick() -> Option<u32> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut li = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY: structure sized.
    (unsafe { GetLastInputInfo(&mut li) } != 0).then_some(li.dwTime)
}

/// A hook that saw nothing for more than 2 s while the session had input after its last event:
/// Windows removed it. (Low-level hooks see every input, injected or not.)
pub fn hook_lost(last_event: u32) -> bool {
    let Some(input) = last_input_tick() else { return false };
    let since_input = tick32().wrapping_sub(input);
    let since_event = tick32().wrapping_sub(last_event);
    // the latest input is more recent than the hook's latest event, by more than 2 s, and that
    // input itself is at least 1 s old (the hook had time to see it)
    since_event > since_input.saturating_add(2_000) && (1_000..600_000).contains(&since_input)
}

/// The window in front receives input our hooks never see (a program run as administrator:
/// Windows keeps its input from us), so "no event" proves nothing.
pub fn foreground_hides_input() -> bool {
    let fg = crate::platform::target::foreground();
    fg.is_null() || crate::platform::target::info(fg).elevated == Some(true)
}

/// The Caps Lock / click hooks are reinstalled if Windows removed them (at most every 30 s).
/// false = could not.
pub fn check_hooks() -> bool {
    use std::sync::atomic::Ordering::SeqCst;
    use windows_sys::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx;
    static LAST_FIX: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    // wanted but absent (a reinstall failed): still an alert, tried again every 30 s
    let absent = WANTED.load(SeqCst) && !CAPS_HOOK_ON.load(SeqCst);
    if !absent && (!CAPS_HOOK_ON.load(SeqCst) || !hook_lost(LAST_EVENT.load(SeqCst)) || foreground_hides_input()) {
        return true;
    }
    let now = tick32();
    let last = LAST_FIX.load(SeqCst);
    if last != 0 && now.wrapping_sub(last) < 30_000 {
        return !absent; // repaired less than 30 s ago: not again yet (never a loop)
    }
    LAST_FIX.store(now, SeqCst);
    for h in [CAPS_HHOOK.swap(0, SeqCst), CLICK_HHOOK.swap(0, SeqCst)] {
        if h != 0 {
            // SAFETY: a hook of ours (already removed by Windows: the call just fails).
            unsafe { UnhookWindowsHookEx(h as _) };
        }
    }
    CAPS_HOOK_ON.store(false, SeqCst);
    CLICK_HOOK_ON.store(false, SeqCst);
    LAST_EVENT.store(tick32(), SeqCst);
    crate::util::log("Keyboard hook (Caps Lock, clicks) removed by Windows: put back");
    watch_caps_lock()
}

/// 1.8.7: the last key the user pressed (the hotkey and modifiers aside) typed a character, and no
/// click came after it.
static LAST_CHAR: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn last_key_was_character() -> bool {
    LAST_CHAR.load(std::sync::atomic::Ordering::Relaxed)
}

pub fn user_activity() -> (u32, u32) {
    use std::sync::atomic::Ordering::SeqCst;
    (USER_KEYS.load(SeqCst), USER_CLICKS.load(SeqCst))
}

unsafe extern "system" fn click_hook(code: i32, wp: usize, lp: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{CallNextHookEx, WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN};
    LAST_EVENT.store(tick32(), std::sync::atomic::Ordering::Relaxed);
    if code >= 0 && matches!(wp as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
        USER_CLICKS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        LAST_CHAR.store(false, std::sync::atomic::Ordering::Relaxed);
    }
    // SAFETY: passes the event on unchanged.
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wp, lp) }
}
static CAPS_HOOK_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CLICK_HOOK_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// the hooks were installed once (so their absence is an alert)
static WANTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Both counters are working (the quick toggle is allowed only then).
pub fn activity_watched() -> bool {
    use std::sync::atomic::Ordering::SeqCst;
    CAPS_HOOK_ON.load(SeqCst) && CLICK_HOOK_ON.load(SeqCst)
}

unsafe extern "system" fn caps_hook(code: i32, wp: usize, lp: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::{CallNextHookEx, KBDLLHOOKSTRUCT, WM_KEYDOWN, WM_SYSKEYDOWN};
    // trivial on purpose: a slow hook is skipped, then removed, by Windows
    LAST_EVENT.store(tick32(), std::sync::atomic::Ordering::Relaxed);
    if code >= 0 && (wp as u32 == WM_KEYDOWN || wp as u32 == WM_SYSKEYDOWN) {
        // SAFETY: for code >= 0, lp points to a KBDLLHOOKSTRUCT (contract of WH_KEYBOARD_LL).
        // SAFETY: same structure.
        let k = unsafe { &*(lp as *const KBDLLHOOKSTRUCT) };
        if k.vkCode == VK_CAPITAL as u32 {
            CAPS_PRESSES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        } else if (k.flags & windows_sys::Win32::UI::WindowsAndMessaging::LLKHF_INJECTED == 0 || k.dwExtraInfo != OUR_MARK)
            && k.vkCode != HOTKEY_VK.load(std::sync::atomic::Ordering::SeqCst)
            && !matches!(k.vkCode as u16, VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU | VK_RMENU | VK_LWIN | VK_RWIN)
        {
            USER_KEYS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // a character key (letters, digits, signs, space) without Ctrl/Alt: typing; anything
            // else (arrows, End, Home, Delete...) or Shift+navigation: not
            let ch = matches!(k.vkCode, 0x20 | 0x30..=0x39 | 0x41..=0x5A | 0x60..=0x6F | 0xBA..=0xC0 | 0xDB..=0xDF | 0xE2);
            let ctrl_alt = [VK_CONTROL, VK_MENU].iter().any(|v| {
                // SAFETY: no pointers.
                (unsafe { GetAsyncKeyState(*v as i32) } as u16) & 0x8000 != 0
            });
            LAST_CHAR.store(ch && !ctrl_alt, std::sync::atomic::Ordering::Relaxed);
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
    WANTED.store(true, SeqCst);
    CAPS_HHOOK.store(h as usize, SeqCst);
    // 1.8.5: clicks counted too (see USER_CLICKS)
    // SAFETY: valid callback of this module; thread 0 = global (required for a low-level hook).
    let m = unsafe { SetWindowsHookExW(windows_sys::Win32::UI::WindowsAndMessaging::WH_MOUSE_LL, Some(click_hook), GetModuleHandleW(std::ptr::null()), 0) };
    if !ok && !m.is_null() {
        // the keyboard hook failed: the mouse one is not kept alone (never piled up on retries)
        // SAFETY: the hook just installed.
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(m) };
    }
    let m = if ok { m } else { std::ptr::null_mut() };
    CLICK_HHOOK.store(m as usize, SeqCst);
    CLICK_HOOK_ON.store(!m.is_null(), SeqCst);
    LAST_EVENT.store(tick32(), SeqCst);
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
