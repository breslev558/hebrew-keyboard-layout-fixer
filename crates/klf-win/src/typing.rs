//! Correction while typing (1.8.0). OPTION, off by default, PUBLIC per-user install only (never
//! with an administrator policy: same rule as updates and reports).
//! - Keys are watched (low-level keyboard + mouse hooks) ONLY while the option is on.
//! - Only the last few words are kept, in memory; cleared on click, arrows, Enter, shortcuts,
//!   another window, a dead key, Shift on the Hebrew layout or a pause. Nothing is written or sent.
//! - A word is checked when Space is typed (a password without spaces is never checked).
//! - When the keys typed spell a real word on the other layout (and not on this one), a bubble
//!   shows the fix: Tab fixes the whole run and switches the keyboard, any other key ignores it.
//!   Before erasing, the text before the caret is read back when the control allows it (Edit,
//!   RichEdit) and must be exactly what was typed.
//! - The hook only looks up tables prepared beforehand (never reads a layout itself: a slow hook
//!   is removed by Windows).
//! - The word lists are downloaded once from the project's GitHub release, signed with the update
//!   key, and checked again (signature + SHA-256) each time they are read.

use crate::app;
use crate::platform::{input, layouts, target};
use crate::util::{log, wide};
use klf_core::i18n::{text, Msg};
use klf_core::typing::{self as t, Buffer, Dict, Suggestion};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// WPARAM: 1 check the words just finished, 2 fix (Tab), 3 lists ready, 4 lists not available,
/// 5 a layout without a prepared table was met (prepared outside the hook)
pub const WM_APP_TYPING: u32 = WM_APP + 8;
const CHECK: usize = 1;
const FIX: usize = 2;
const READY: usize = 3;
const FAILED: usize = 4;
const TABLES: usize = 5;
/// a pause longer than this starts a new run
const PAUSE_MS: u64 = 15_000;
/// a proposal stays valid this long
const PROPOSAL_MS: u64 = 5_000;

static DICT: OnceLock<Dict> = OnceLock::new();
static LOADING: AtomicBool = AtomicBool::new(false);

struct Proposal {
    s: Suggestion,
    hwnd: usize,
    latin_hkl: Option<u32>,
    at: u64,
    /// the words the proposal is about (must still be the last ones typed)
    words: Vec<String>,
}

thread_local! {
    static KHOOK: Cell<HHOOK> = const { Cell::new(null_mut()) };
    static MHOOK: Cell<HHOOK> = const { Cell::new(null_mut()) };
    static BUF: RefCell<Buffer> = RefCell::new(Buffer::default());
    /// window the words were typed in
    static BUF_HWND: Cell<usize> = const { Cell::new(0) };
    static LAST_KEY: Cell<u64> = const { Cell::new(0) };
    /// words waiting to be checked (filled by the hook, read by the UI message)
    static PENDING: RefCell<Option<(Vec<String>, bool, usize)>> = const { RefCell::new(None) };
    static PROPOSAL: RefCell<Option<Proposal>> = const { RefCell::new(None) };
    /// the Tab key-up that follows a swallowed Tab is swallowed too
    static SWALLOW_TAB_UP: Cell<bool> = const { Cell::new(false) };
    /// unshifted characters of each installed layout, by scan code (prepared outside the hook)
    static MAPS: RefCell<HashMap<u32, HashMap<u32, char>>> = RefCell::new(HashMap::new());
}

fn tick() -> u64 {
    // SAFETY: no arguments.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}

/// Allowed on this PC: public per-user install, no administrator policy.
pub fn allowed() -> bool {
    crate::update::allowed(true)
}

pub fn active() -> bool {
    !KHOOK.with(|h| h.get()).is_null()
}

/// Settings changed or program started (UI thread): hooks on or off as the option says.
pub fn apply() {
    let want = app::settings().typing_suggestions && allowed();
    if !want {
        stop();
        return;
    }
    if DICT.get().is_some() {
        start();
    } else {
        load_lists();
    }
}

/// Clears the words kept (another window, click...).
pub fn reset() {
    BUF.with(|b| b.borrow_mut().clear());
    PENDING.with(|p| p.borrow_mut().take());
    PROPOSAL.with(|p| p.borrow_mut().take());
}

/// Reads every installed layout once, outside the hook.
fn prepare_tables() {
    let mut maps = HashMap::new();
    for hkl in layouts::installed() {
        if let Some(t) = layouts::read_layout(hkl) {
            maps.insert(hkl, t.into_iter().map(|k| (k.sc as u32, k.ch)).collect::<HashMap<u32, char>>());
        }
    }
    MAPS.with(|m| *m.borrow_mut() = maps);
}

fn start() {
    if active() {
        return;
    }
    prepare_tables();
    // SAFETY: hook procedures of this module; the module handle is our exe.
    unsafe {
        let k = SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_proc), app::instance(), 0);
        let m = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), app::instance(), 0);
        if k.is_null() {
            log("Typing correction: keyboard hook refused");
            if !m.is_null() {
                UnhookWindowsHookEx(m);
            }
            return;
        }
        KHOOK.with(|h| h.set(k));
        MHOOK.with(|h| h.set(m));
    }
    log("Typing correction on");
}

pub fn stop() {
    reset();
    for h in [KHOOK.with(|h| h.replace(null_mut())), MHOOK.with(|h| h.replace(null_mut()))] {
        if !h.is_null() {
            // SAFETY: a hook we installed.
            unsafe { UnhookWindowsHookEx(h) };
        }
    }
}

fn post(code: usize) {
    // SAFETY: posting a message without pointers to our own window.
    unsafe { PostMessageW(app::main_window(), WM_APP_TYPING, code, 0) };
}

// ---------------------------------------------------------------------------------------------
// word lists: local copy (checked again), else downloaded once (signed)

struct LoadingGuard;
impl Drop for LoadingGuard {
    fn drop(&mut self) {
        LOADING.store(false, Ordering::SeqCst);
    }
}

fn load_lists() {
    if LOADING.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new().name("typing-lists".into()).spawn(|| {
        let _guard = LoadingGuard;
        let ok = match read_or_download() {
            Some(d) => DICT.set(d).is_ok() || DICT.get().is_some(),
            None => false,
        };
        post(if ok { READY } else { FAILED });
    });
}

/// The manifest is signed; each list must match its SHA-256 and size in it.
fn verified(man: &[u8], sig: &[u8], he: &[u8], en: &[u8]) -> bool {
    let check = || -> Option<bool> {
        let sig = klf_core::update::parse_sig(std::str::from_utf8(sig).ok()?)?;
        if !crate::update::signature_ok(man, &sig) {
            return Some(false);
        }
        let m = t::parse_manifest(std::str::from_utf8(man).ok()?)?;
        let ok = |b: &[u8], (sha, size): &(String, u64)| b.len() as u64 == *size && crate::update::sha256(b).is_some_and(|h| crate::update::hex(&h).eq_ignore_ascii_case(sha));
        Some(ok(he, &m.he) && ok(en, &m.en))
    };
    check().unwrap_or(false)
}

fn read_or_download() -> Option<Dict> {
    let dir = crate::util::app_dir()?;
    let names = ["words.manifest", "words.manifest.sig", "words-he.txt", "words-en.txt"];
    let local: Vec<Option<Vec<u8>>> = names.iter().map(|n| std::fs::read(dir.join(n)).ok()).collect();
    if let [Some(m), Some(s), Some(h), Some(e)] = &local[..] {
        if verified(m, s, h, e) {
            let d = Dict::from_lists(std::str::from_utf8(h).ok()?, std::str::from_utf8(e).ok()?, t::DICT_SIZE);
            if !d.is_empty() {
                return Some(d);
            }
        }
        log("Typing correction: local word lists changed or damaged, downloaded again");
    }
    let get = |name: &str, max: usize| crate::update::get(&format!("{}{}", t::WORDS_BASE, name), max);
    let man = get(t::WORDS_MANIFEST, 1024)?;
    let sig = get(&format!("{}.sig", t::WORDS_MANIFEST), 1024)?;
    let he = get(t::WORDS_HE, t::MAX_WORDS_BYTES)?;
    let en = get(t::WORDS_EN, t::MAX_WORDS_BYTES)?;
    if !verified(&man, &sig, &he, &en) {
        log("Typing correction: word lists signature or checksum NOT valid, not used");
        return None;
    }
    for (n, b) in names.iter().zip([&man, &sig, &he, &en]) {
        let _ = std::fs::write(dir.join(n), b);
    }
    log("Typing correction: word lists downloaded and checked");
    let d = Dict::from_lists(std::str::from_utf8(&he).ok()?, std::str::from_utf8(&en).ok()?, t::DICT_SIZE);
    (!d.is_empty()).then_some(d)
}

// ---------------------------------------------------------------------------------------------
// hooks (called on the UI thread, must stay short: lookups only)

fn down(vk: u16) -> bool {
    // SAFETY: no pointers.
    unsafe { GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0 }
}

/// Window and keyboard layout of the control that has the focus (right for UWP hosts too).
fn focus_and_layout() -> (usize, u32) {
    // SAFETY: structure sized; plain calls.
    unsafe {
        let mut gi: GUITHREADINFO = std::mem::zeroed();
        gi.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        let fg = GetForegroundWindow();
        let w = if GetGUIThreadInfo(0, &mut gi) != 0 && !gi.hwndFocus.is_null() { gi.hwndFocus } else { fg };
        let tid = GetWindowThreadProcessId(w, null_mut());
        (fg as usize, GetKeyboardLayout(tid) as usize as u32)
    }
}

/// true: the key is swallowed (Tab accepting a proposal).
fn on_key(vk: u32, sc: u32) -> bool {
    let now = tick();
    let (fg, hkl) = focus_and_layout();
    let fresh = PROPOSAL.with(|p| p.borrow().as_ref().is_some_and(|x| now.saturating_sub(x.at) < PROPOSAL_MS && x.hwnd == fg));
    if fresh && vk == VK_TAB as u32 && !down(VK_SHIFT) && !down(VK_CONTROL) && !down(VK_MENU) && !down(VK_LWIN) && !down(VK_RWIN) {
        SWALLOW_TAB_UP.with(|s| s.set(true));
        post(FIX);
        return true;
    }
    // any other key: the proposal and the words waiting for a check are dropped
    PROPOSAL.with(|p| p.borrow_mut().take());
    PENDING.with(|p| p.borrow_mut().take());
    if now.saturating_sub(LAST_KEY.with(|l| l.replace(now))) > PAUSE_MS || BUF_HWND.with(|h| h.replace(fg)) != fg {
        BUF.with(|b| b.borrow_mut().clear());
    }
    if matches!(vk as u16, VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CAPITAL) {
        return false;
    }
    let hebrew = hkl & 0x3FF == 0x0D; // primary language of the layout: Hebrew
    if down(VK_CONTROL) || down(VK_MENU) || down(VK_LWIN) || down(VK_RWIN) || (hebrew && down(VK_SHIFT)) {
        BUF.with(|b| b.borrow_mut().clear());
        return false;
    }
    match vk as u16 {
        VK_BACK => BUF.with(|b| b.borrow_mut().backspace()),
        VK_SPACE => {
            if let Some((w, h)) = BUF.with(|b| b.borrow_mut().space()) {
                PENDING.with(|p| *p.borrow_mut() = Some((w, h, fg)));
                post(CHECK);
            }
        }
        _ => {
            let ch = MAPS.with(|m| m.try_borrow().ok().and_then(|m| m.get(&hkl).map(|t| t.get(&sc).copied())));
            match ch {
                None => {
                    // a layout added since the start: tables prepared outside the hook
                    BUF.with(|b| b.borrow_mut().clear());
                    post(TABLES);
                }
                Some(Some(c)) if c.is_alphabetic() || c == '\'' => {
                    let caps = !hebrew && (down(VK_SHIFT) ^ caps_on());
                    let c = if caps { c.to_uppercase().next().unwrap_or(c) } else { c };
                    BUF.with(|b| b.borrow_mut().push(c, hebrew));
                }
                // Enter, Tab, arrows, digits, signs, dead keys...
                Some(_) => BUF.with(|b| b.borrow_mut().clear()),
            }
        }
    }
    false
}

fn caps_on() -> bool {
    // SAFETY: no pointers.
    unsafe { GetKeyState(VK_CAPITAL as i32) & 1 != 0 }
}

unsafe extern "system" fn key_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION, l points to a KBDLLHOOKSTRUCT.
        let k = unsafe { &*(l as *const KBDLLHOOKSTRUCT) };
        // our own typing (the fix) and other programs' injected keys are not the user's typing
        if k.flags & LLKHF_INJECTED == 0 {
            let msg = w as u32;
            if (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) && on_key(k.vkCode, k.scanCode) {
                return 1;
            }
            if (msg == WM_KEYUP || msg == WM_SYSKEYUP) && k.vkCode == VK_TAB as u32 && SWALLOW_TAB_UP.with(|s| s.replace(false)) {
                return 1;
            }
        }
    }
    // SAFETY: passes the event on.
    unsafe { CallNextHookEx(null_mut(), code, w, l) }
}

unsafe extern "system" fn mouse_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && matches!(w as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
        reset(); // the caret may have moved: the words kept no longer end at the caret
    }
    // SAFETY: passes the event on.
    unsafe { CallNextHookEx(null_mut(), code, w, l) }
}

// ---------------------------------------------------------------------------------------------
// UI thread messages

pub fn on_message(code: WPARAM) {
    match code {
        CHECK => check(),
        FIX => fix(),
        TABLES => prepare_tables(),
        READY => {
            if app::settings().typing_suggestions && allowed() {
                start();
                let l = app::lang();
                crate::bubble::show(text(l, Msg::TypingReady), l.is_rtl(), None, 0, true);
            }
        }
        _ => {
            let l = app::lang();
            crate::bubble::show(text(l, Msg::TypingFailed), l.is_rtl(), None, 0, false);
        }
    }
}

/// A full-screen program (game, video, presentation).
fn fullscreen_or_game() -> bool {
    use windows_sys::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN};
    let mut st = 0;
    // SAFETY: the state is written.
    let r = unsafe { SHQueryUserNotificationState(&mut st) };
    // busy (full-screen program), Direct3D full screen (games), presentation mode
    r == 0 && matches!(st, QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE)
}

fn check() {
    let Some((words, hebrew, hwnd)) = PENDING.with(|p| p.borrow_mut().take()) else { return };
    let Some(dict) = DICT.get() else { return };
    let fg = target::foreground();
    if fg as usize != hwnd || fullscreen_or_game() {
        return; // another window, or games / presentations / full-screen videos: never
    }
    let info = target::info(fg);
    if info.password_field || info.elevated == Some(true) || klf_core::policy::console_kind(&info.window_class).is_some() || !klf_core::policy::rule_allowed_for(&info.exe_name) {
        return;
    }
    if app::settings().rule_for(&info.exe_name) == Some(klf_core::settings::RuleMode::Disabled) {
        return;
    }
    let (_, hkl) = focus_and_layout();
    let layouts::Choice::Map(map, latin_hkl) = layouts::choose(Some(hkl)) else { return }; // a 3rd layout: never
    let pairs: Vec<(String, String)> = words.iter().map(|w| (w.clone(), klf_core::convert::convert_with(&map, w, klf_core::settings::MixedTextMode::ToggleAll, true))).collect();
    let Some(s) = t::suggest_any(&pairs, hebrew, dict) else { return };
    let shown = format!("{}   Tab ↹", s.text.trim_end());
    crate::bubble::show_near_caret(&shown, s.to_hebrew, target::caret_point(fg), fg as usize);
    if app::settings().play_sound {
        crate::platform::sys::beep();
    }
    PROPOSAL.with(|p| *p.borrow_mut() = Some(Proposal { s, hwnd: fg as usize, latin_hkl, at: tick(), words }));
}

/// For Edit / RichEdit controls, the text just before the caret must be exactly what was typed
/// (dead keys, autocorrection, autocompletion... would make the erase hit other text). Other
/// controls cannot be read back: allowed.
fn text_before_caret_matches(expected: &str) -> bool {
    let (focus, _) = {
        // SAFETY: structure sized.
        unsafe {
            let mut gi: GUITHREADINFO = std::mem::zeroed();
            gi.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
            if GetGUIThreadInfo(0, &mut gi) == 0 || gi.hwndFocus.is_null() {
                return true;
            }
            (gi.hwndFocus, ())
        }
    };
    let class = crate::platform::sys::class_name(focus);
    if !(class.eq_ignore_ascii_case("Edit") || class.to_ascii_lowercase().starts_with("richedit")) {
        return true;
    }
    // SAFETY: EM_GETSEL / WM_GETTEXTLENGTH / WM_GETTEXT on an Edit control (marshalled by Windows
    // across processes); the buffer size is passed.
    unsafe {
        let (mut start, mut end) = (0u32, 0u32);
        SendMessageW(focus, 0x00B0, &mut start as *mut u32 as WPARAM, &mut end as *mut u32 as LPARAM); // EM_GETSEL
        let len = SendMessageW(focus, WM_GETTEXTLENGTH, 0, 0).max(0) as usize;
        if start != end || len > 1_000_000 || end as usize > len {
            return false;
        }
        let mut buf = vec![0u16; len + 1];
        let n = SendMessageW(focus, WM_GETTEXT, buf.len(), buf.as_mut_ptr() as LPARAM).max(0) as usize;
        let before = String::from_utf16_lossy(&buf[..(end as usize).min(n)]);
        let exp: Vec<u16> = wide(expected);
        let exp = String::from_utf16_lossy(&exp[..exp.len().saturating_sub(1)]);
        before.ends_with(&exp)
    }
}

fn fix() {
    let Some(p) = PROPOSAL.with(|p| p.borrow_mut().take()) else { return };
    let fg = target::foreground();
    if fg as usize != p.hwnd {
        return; // another window: nothing is typed
    }
    // the words of the proposal must still be the last ones typed, with nothing after the space
    let still = BUF.with(|b| b.borrow().ends_with_words(&p.words));
    let typed = p.words.join(" ") + " ";
    let erase_text: String = typed.chars().rev().take(p.s.erase).collect::<Vec<_>>().into_iter().rev().collect();
    if !still || !text_before_caret_matches(&erase_text) {
        log("Typing correction: text changed before Tab, nothing done");
        BUF.with(|b| b.borrow_mut().clear());
        return;
    }
    if !input::replace(p.s.erase, &p.s.text) {
        log("Typing correction: the fix could not be typed");
        return;
    }
    let to = if p.s.keep_layout { None } else if p.s.to_hebrew { layouts::hebrew_layout() } else { p.latin_hkl };
    if let Some(h) = to {
        layouts::request(fg, h);
    }
    BUF.with(|b| b.borrow_mut().clear());
    log(&format!("Typing correction: {} word(s) fixed", p.s.text.split_whitespace().count()));
}
