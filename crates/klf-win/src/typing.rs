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
use std::sync::Mutex;
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
const UNDO: usize = 6;
const DOUBLE_SHIFT: usize = 7;
const REPLACE: usize = 8;
const REPL_REFRESH: usize = 9;
const VERIFY: usize = 10;
const CTRL_EN: usize = 11;
const CTRL_HE: usize = 12;
const GUARD: usize = 13;
const TIMING: usize = 14;
/// a pause longer than this starts a new run
const PAUSE_MS: u64 = 15_000;
/// a proposal stays valid this long
const PROPOSAL_MS: u64 = 5_000;

static DICT: Mutex<Option<Dict>> = Mutex::new(None);
/// date of the personal file when it was last read
static PERSONAL_AT: Mutex<Option<std::time::SystemTime>> = Mutex::new(None);

/// The lists are loaded (read in the hook without any lock).
static LISTS_READY: AtomicBool = AtomicBool::new(false);
/// 1.8.5: time spent in the keyboard hook, measured on every key (two clock reads), written to
/// debug.log every 500 keys: Windows removes a hook that answers too slowly, and typing waits for it.
static HOOK_KEYS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static HOOK_SUM_US: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static HOOK_MAX_US: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn qpc() -> (i64, i64) {
    use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
    let (mut c, mut f) = (0i64, 0i64);
    // SAFETY: plain out values.
    unsafe {
        QueryPerformanceCounter(&mut c);
        QueryPerformanceFrequency(&mut f);
    }
    (c, f.max(1))
}

/// 1.8.5: last event seen by the typing hooks (GetTickCount, ms)
static LAST_EVENT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn dict_ready_fast() -> bool {
    LISTS_READY.load(Ordering::SeqCst)
}

fn dict_ready() -> bool {
    DICT.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_some()
}

/// %APPDATA%\KeyboardLayoutFixer\my-words.txt: words of the user ("-word": never fix it).
pub fn personal_path() -> Option<std::path::PathBuf> {
    crate::util::app_dir().map(|d| d.join("my-words.txt"))
}

/// Reads the personal file again if it changed (cheap: one date check).
fn refresh_personal() {
    let Some(p) = personal_path() else { return };
    let meta = std::fs::metadata(&p).ok();
    let at = meta.as_ref().and_then(|m| m.modified().ok());
    let mut last = PERSONAL_AT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if *last == at {
        return;
    }
    if meta.as_ref().is_some_and(|m| m.len() > 256 * 1024) {
        log("Typing correction: my-words.txt larger than 256 KB, not read");
        return;
    }
    let text = match std::fs::read(&p) {
        Ok(b) => decode_text(&b),
        Err(_) if meta.is_none() => String::new(), // no file: no personal words
        Err(_) => return,                           // busy (Notepad, antivirus): tried again later
    };
    *last = at;
    if let Some(d) = DICT.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_mut() {
        d.set_personal(&text);
    }
}

/// UTF-8 (with or without BOM) or UTF-16 (Notepad "Unicode"), else read as Latin-1.
fn decode_text(b: &[u8]) -> String {
    if let Some(rest) = b.strip_prefix(&[0xFF, 0xFE]) {
        let u: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
        return String::from_utf16_lossy(&u);
    }
    if let Some(rest) = b.strip_prefix(&[0xFE, 0xFF]) {
        let u: Vec<u16> = rest.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect();
        return String::from_utf16_lossy(&u);
    }
    let b = b.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(b);
    match std::str::from_utf8(b) {
        Ok(s) => s.to_string(),
        Err(_) => b.iter().map(|&c| c as char).collect(),
    }
}

/// Adds lines to the personal file (UTF-8; a line break first if the file does not end with one).
/// 1.8.5: a quick second F10 put the original back: what the first one taught ("+word") is
/// taken back out of my-words.txt (a round trip teaches nothing).
pub fn forget_learned(typed: &str) {
    let Some(line) = t::learned_line(typed) else { return };
    let Some(p) = personal_path() else { return };
    let _w = PERSONAL_WRITE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Ok(current) = std::fs::read(&p) else { return };
    if current.len() > 256 * 1024 {
        return;
    }
    let text = decode_text(&current);
    if !text.lines().any(|l| l.trim().eq_ignore_ascii_case(&line)) {
        return;
    }
    let kept: Vec<&str> = text.lines().filter(|l| !l.trim().eq_ignore_ascii_case(&line)).collect();
    if std::fs::write(&p, kept.join("\r\n") + "\r\n").is_ok() {
        log("F10 round trip: the word learned by the first F10 was taken back");
        refresh_personal();
    }
}

fn append_personal(lines: &[String]) {
    let Some(p) = personal_path() else { return };
    let _w = PERSONAL_WRITE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let current = std::fs::read(&p).unwrap_or_default();
    if current.len() > 256 * 1024 || lines.is_empty() {
        return;
    }
    let known = decode_text(&current);
    let new: Vec<&String> = lines.iter().filter(|l| !known.lines().any(|k| k.trim().eq_ignore_ascii_case(l))).collect();
    if new.is_empty() {
        return;
    }
    let mut add = String::new();
    if !current.is_empty() && !current.ends_with(b"\n") {
        add.push_str("\r\n");
    }
    for l in new {
        add.push_str(l);
        add.push_str("\r\n");
    }
    // a file saved by Notepad as UTF-16 is rewritten in UTF-8 (the program reads both)
    let utf16 = current.starts_with(&[0xFF, 0xFE]) || current.starts_with(&[0xFE, 0xFF]);
    let result = if utf16 {
        std::fs::write(&p, format!("{}{add}", known.trim_end_matches(['\r', '\n']).to_string() + "\r\n"))
    } else {
        use std::io::Write;
        std::fs::OpenOptions::new().create(true).append(true).open(&p).and_then(|mut f| f.write_all(add.as_bytes()))
    };
    if result.is_err() {
        log("Typing correction: my-words.txt could not be written");
    }
    refresh_personal();
}

/// Tray menu "My words…": opens the personal file in Notepad (created with an explanation).
pub fn open_personal() {
    let Some(p) = personal_path() else { return };
    if !p.exists() {
        let l = app::lang();
        let _ = std::fs::write(&p, format!("# {}\r\n", text(l, Msg::TypingWordsHelp)));
    }
    let _ = std::process::Command::new("notepad.exe").arg(&p).spawn();
}
static LOADING: AtomicBool = AtomicBool::new(false);

struct Proposal {
    s: Suggestion,
    hwnd: usize,
    latin_hkl: Option<u32>,
    at: u64,
    /// the words the proposal is about (must still be the last ones typed)
    words: Vec<String>,
}

/// The last fix, for 5 s: Backspace as the very next key undoes it and teaches "never again".
/// (control, length before the words in UTF-16 units, its last 64 units, words typed, fix typed)
type VerifyState = (usize, usize, String, String, String);

struct LastFix {
    /// undoing teaches "never fix these words" (not for a text replacement)
    learn: bool,
    typed: String,
    fixed: String,
    hwnd: usize,
    layout_back: Option<u32>,
    at: u64,
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
    static LAST_FIX: RefCell<Option<LastFix>> = const { RefCell::new(None) };
    static SWALLOW_BACK_UP: Cell<bool> = const { Cell::new(false) };
    static DSHIFT: RefCell<t::DoubleShift> = RefCell::new(t::DoubleShift::default());
    /// what the hooks serve (read in the hook: no settings lock there)
    static WANT_TYPING: Cell<bool> = const { Cell::new(false) };
    /// 1.8.2, text replacements on
    static WANT_REPL: Cell<bool> = const { Cell::new(false) };
    /// 1.8.5: left/right Ctrl alone switch the keyboard; chat guard on Enter
    static WANT_CTRLTAP: Cell<bool> = const { Cell::new(false) };
    static CTRLTAP: RefCell<t::CtrlTap> = RefCell::new(t::CtrlTap::default());
    static WANT_GUARD: Cell<bool> = const { Cell::new(false) };
    static IS_MESSENGER: Cell<bool> = const { Cell::new(false) };
    /// 1.8.6: (Run dialog, keyboard before, keyboard we chose)
    static RUN_DIALOG: Cell<(usize, u32, u32)> = const { Cell::new((0, 0, 0)) };
    /// key maps of the installed layouts (prepared outside the hook, for the chat guard)
    static KEYMAPS: RefCell<HashMap<u32, klf_core::layouts::KeyMap>> = RefCell::new(HashMap::new());
    /// (window, time): the next Enter there is let through (the user was warned)
    static GUARD_ARMED: Cell<Option<(usize, u64)>> = const { Cell::new(None) };
    static SWALLOW_ENTER_UP: Cell<bool> = const { Cell::new(false) };
    /// Enter is down (auto-repeat while held is not a new press)
    static ENTER_HELD: Cell<bool> = const { Cell::new(false) };
    static TOKEN: RefCell<klf_core::replace::Token> = RefCell::new(klf_core::replace::Token::default());
    static PENDING_REPL: RefCell<Option<(String, usize)>> = const { RefCell::new(None) };
    /// characters with Shift, for shortcuts such as "@@" (prepared outside the hook too)
    static MAPS_SHIFT: RefCell<HashMap<u32, HashMap<u32, char>>> = RefCell::new(HashMap::new());
    static TABLES_AT: Cell<u64> = const { Cell::new(0) };
    /// 1.8.5: (control, text before the words, words typed, fix typed) for the check after a fix
    /// (control, length of the text before the words in UTF-16 units, its last 64 units, words
    /// typed, fix typed)
    static VERIFY_STATE: RefCell<Option<VerifyState>> = const { RefCell::new(None) };
    /// user keys and clicks counted when the fix was typed (any since: no check, never undone)
    static VERIFY_KEYS: Cell<(u32, u32)> = const { Cell::new((0, 0)) };
    static MAPS_ALTGR: RefCell<HashMap<u32, HashMap<u32, char>>> = RefCell::new(HashMap::new());

    static WANT_DSHIFT: Cell<bool> = const { Cell::new(false) };
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
    let s = app::settings();
    let (typing, dshift) = (s.typing_suggestions && allowed(), s.double_shift_convert && allowed());
    WANT_TYPING.with(|w| w.set(typing));
    WANT_DSHIFT.with(|w| w.set(dshift));
    let repl = s.text_replacements && allowed();
    WANT_REPL.with(|w| w.set(repl));
    let ctrl = s.ctrl_tap_language && allowed();
    WANT_CTRLTAP.with(|w| w.set(ctrl));
    let guard = s.messenger_guard && typing;
    WANT_GUARD.with(|w| w.set(guard));
    if repl {
        refresh_replacements(true);
    }
    // 1.8.5: F10 "only the wrong words" needs the word lists too (no keyboard watching for it)
    if s.f10_only_wrong_words && allowed() && !dict_ready() {
        load_lists();
    }
    if !typing && !dshift && !repl && !ctrl {
        stop();
        app::refresh_dormant(None);
        return;
    }
    if typing && !dict_ready() {
        load_lists(); // the hooks start for double Shift meanwhile; the lists come later
    }
    start();
    app::refresh_dormant(None);
}

/// Clears the words kept (another window, click...).
pub fn reset() {
    BUF.with(|b| b.borrow_mut().clear());
    PENDING.with(|p| p.borrow_mut().take());
    PROPOSAL.with(|p| p.borrow_mut().take());
    LAST_FIX.with(|p| p.borrow_mut().take());
    DSHIFT.with(|d| *d.borrow_mut() = t::DoubleShift::default());
    TOKEN.with(|b| b.borrow_mut().interrupt()); // "ho" + click + "tel" is not the shortcut "tel"
    PENDING_REPL.with(|p| p.borrow_mut().take());
}

/// Reads every installed layout once, outside the hook.
fn prepare_tables() {
    let (mut maps, mut shifted, mut altgr) = (HashMap::new(), HashMap::new(), HashMap::new());
    for hkl in layouts::installed() {
        if let Some(t) = layouts::read_layout(hkl) {
            maps.insert(hkl, t.into_iter().map(|k| (k.sc as u32, k.ch)).collect::<HashMap<u32, char>>());
        }
        if let Some(t) = layouts::read_layout_shifted(hkl) {
            shifted.insert(hkl, t.into_iter().map(|k| (k.sc as u32, k.ch)).collect::<HashMap<u32, char>>());
        }
        if let Some(t) = layouts::read_layout_altgr(hkl) {
            altgr.insert(hkl, t.into_iter().map(|k| (k.sc as u32, k.ch)).collect::<HashMap<u32, char>>());
        }
    }
    let keymaps: HashMap<u32, klf_core::layouts::KeyMap> = maps.keys().filter_map(|&h| match layouts::choose(Some(h)) {
        layouts::Choice::Map(m, _) => Some((h, m)),
        _ => None,
    }).collect();
    KEYMAPS.with(|k| *k.borrow_mut() = keymaps);
    MAPS.with(|m| *m.borrow_mut() = maps);
    MAPS_SHIFT.with(|m| *m.borrow_mut() = shifted);
    MAPS_ALTGR.with(|m| *m.borrow_mut() = altgr);
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
    LAST_EVENT.store(input::tick32(), Ordering::SeqCst);
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
            Some(d) => {
                *DICT.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(d);
                LISTS_READY.store(true, Ordering::SeqCst);
                *PERSONAL_AT.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
                true
            }
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
    let want_t = WANT_TYPING.with(|w| w.get()) && dict_ready_fast();
    let want_r = WANT_REPL.with(|w| w.get());
    if !want_t && !want_r {
        return false;
    }
    let now = tick();
    let (fg, hkl) = focus_and_layout();
    // Backspace right after a fix: undo it (and never fix those words again)
    let undo_ok = LAST_FIX.with(|f| f.borrow().as_ref().is_some_and(|x| now.saturating_sub(x.at) < PROPOSAL_MS && x.hwnd == fg));
    let plain_back = vk == VK_BACK as u32 && !down(VK_CONTROL) && !down(VK_MENU) && !down(VK_SHIFT) && !down(VK_LWIN) && !down(VK_RWIN);
    LAST_FIX.with(|f| {
        if !(undo_ok && plain_back) {
            f.borrow_mut().take(); // any other key, Ctrl+Backspace included: no undo any more
        }
    });
    if undo_ok && plain_back {
        SWALLOW_BACK_UP.with(|s| s.set(true));
        post(UNDO);
        return true;
    }
    let fresh = PROPOSAL.with(|p| p.borrow().as_ref().is_some_and(|x| now.saturating_sub(x.at) < PROPOSAL_MS && x.hwnd == fg));
    if want_t && fresh && vk == VK_TAB as u32 && !down(VK_SHIFT) && !down(VK_CONTROL) && !down(VK_MENU) && !down(VK_LWIN) && !down(VK_RWIN) {
        SWALLOW_TAB_UP.with(|s| s.set(true));
        post(FIX);
        return true;
    }
    // 1.8.5, chat guard: Enter alone in a chat program, with a sentence typed. Decided here, at
    // once (tables and lists prepared beforehand; the lists only if free this instant): a wrong
    // sentence holds this Enter back with a warning, and the user's NEXT Enter sends it. The
    // program never sends Enter itself.
    if vk == VK_RETURN as u32 && WANT_GUARD.with(|g| g.get()) && IS_MESSENGER.with(|m| m.get()) && !down(VK_SHIFT) && !down(VK_CONTROL) && !down(VK_MENU) && !down(VK_LWIN) && !down(VK_RWIN) {
        let armed = GUARD_ARMED.with(|g| g.take()).is_some_and(|(h, at)| h == fg && now.saturating_sub(at) < 10_000);
        if !armed && sentence_wrong_now(hkl) {
            GUARD_ARMED.with(|g| g.set(Some((fg, now))));
            SWALLOW_ENTER_UP.with(|s| s.set(true));
            BUF.with(|b| b.borrow_mut().clear());
            post(GUARD);
            return true;
        }
    }
    // any other key: the proposal and the words waiting for a check are dropped
    PROPOSAL.with(|p| p.borrow_mut().take());
    PENDING.with(|p| p.borrow_mut().take());
    PENDING_REPL.with(|p| p.borrow_mut().take());
    if now.saturating_sub(LAST_KEY.with(|l| l.replace(now))) > PAUSE_MS || BUF_HWND.with(|h| h.replace(fg)) != fg {
        BUF.with(|b| b.borrow_mut().clear());
        TOKEN.with(|b| b.borrow_mut().interrupt());
    }
    // modifiers alone change nothing (AltGr = left Ctrl + right Alt: its own presses must not
    // end the shortcut being typed); the key typed WITH them decides
    if matches!(vk as u16, VK_SHIFT | VK_LSHIFT | VK_RSHIFT | VK_CAPITAL | VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU | VK_RMENU | VK_LWIN | VK_RWIN) {
        return false;
    }
    let hebrew = hkl & 0x3FF == 0x0D; // primary language of the layout: Hebrew
    // AltGr (right Alt, which Windows sends with left Ctrl): characters such as @ on AZERTY
    let altgr = down(VK_RMENU) && down(VK_LCONTROL) && !down(VK_LMENU) && !down(VK_RCONTROL) && !down(VK_LWIN) && !down(VK_RWIN);
    if altgr {
        BUF.with(|b| b.borrow_mut().clear());
        if want_r && down(VK_SHIFT) {
            TOKEN.with(|b| b.borrow_mut().invalidate()); // AltGr+Shift: level not prepared
        } else if want_r {
            token_key(vk, sc, hkl, fg, true);
        }
        return false;
    }
    if down(VK_CONTROL) || down(VK_MENU) || down(VK_LWIN) || down(VK_RWIN) {
        BUF.with(|b| b.borrow_mut().clear());
        TOKEN.with(|b| b.borrow_mut().interrupt());
        return false;
    }
    if want_r {
        token_key(vk, sc, hkl, fg, false);
    }
    if !want_t {
        return false;
    }
    // Shift on the Hebrew layout, or Caps Lock there (it types Latin capitals): not tracked
    if hebrew && (down(VK_SHIFT) || caps_on()) {
        BUF.with(|b| b.borrow_mut().invalidate()); // what is on the screen is not what is tracked
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
                // , . ; / ' : Hebrew letters on the other layout, kept inside a word (1.8.2);
                // with Shift they are other signs (< > : ? ") and end the word
                Some(Some(c)) if c.is_alphabetic() || (t::WORD_SIGNS.contains(&c) && !down(VK_SHIFT)) => {
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

/// 1.8.2, the text replacements: what was typed since the last space (any visible character),
/// kept only while it is the start of one of the user's shortcuts.
fn token_key(vk: u32, sc: u32, hkl: u32, fg: usize, altgr: bool) {
    let inval = || TOKEN.with(|b| b.borrow_mut().invalidate());
    match vk as u16 {
        VK_BACK => TOKEN.with(|b| b.borrow_mut().backspace()),
        VK_SPACE => {
            if let Some(tok) = TOKEN.with(|b| b.borrow_mut().space()) {
                PENDING_REPL.with(|p| *p.borrow_mut() = Some((tok, fg)));
                post(REPLACE); // posted before CHECK: a replacement wins over a layout fix
            } else {
                post(REPL_REFRESH); // the list may have just been saved (in Notepad itself)
            }
        }
        // a new line or a new field: a shortcut may start right away
        VK_RETURN | VK_TAB => TOKEN.with(|b| b.borrow_mut().clear()),
        _ => {
            // Caps Lock outside English layouts changes more than letters (AZERTY digits, Latin
            // capitals on the Hebrew layout): not modelled, so no shortcut until the next space
            if caps_on() && hkl & 0x3FF != 0x09 {
                inval();
                return;
            }
            let shift = down(VK_SHIFT);
            let table = if altgr { &MAPS_ALTGR } else if shift { &MAPS_SHIFT } else { &MAPS };
            let ch = table.with(|m| m.try_borrow().ok().map(|m| m.get(&hkl).map(|t| t.get(&sc).copied())));
            match ch {
                // a layout added since the start: tables prepared outside the hook
                Some(None) => {
                    inval();
                    post(TABLES);
                }
                Some(Some(Some(c))) if !c.is_control() && !c.is_whitespace() => {
                    let caps = !altgr && c.is_alphabetic() && !klf_core::layouts::is_hebrew_letter(c) && caps_on();
                    let c = if caps { if shift { c.to_lowercase().next().unwrap_or(c) } else { c.to_uppercase().next().unwrap_or(c) } } else { c };
                    TOKEN.with(|b| b.borrow_mut().push(c));
                    // not the start of any shortcut (ordinary text, a password...): not kept
                    let known = TOKEN.with(|b| PREFIXES.try_lock().is_ok_and(|p| p.as_ref().is_some_and(|p| p.contains(b.borrow().as_str()))));
                    if !known {
                        inval();
                    }
                }
                // arrows, Home, Delete, a dead key...: the caret or the text may have changed
                _ => TOKEN.with(|b| b.borrow_mut().interrupt()),
            }
        }
    }
}

fn caps_on() -> bool {
    // SAFETY: no pointers.
    unsafe { GetKeyState(VK_CAPITAL as i32) & 1 != 0 }
}

unsafe extern "system" fn key_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    let (t0, f) = qpc();
    // SAFETY: same contract as the hook itself.
    let r = unsafe { key_proc_inner(code, w, l) };
    let us = ((qpc().0 - t0).max(0) as u64) * 1_000_000 / f as u64;
    HOOK_SUM_US.fetch_add(us, Ordering::Relaxed);
    HOOK_MAX_US.fetch_max(us, Ordering::Relaxed);
    if HOOK_KEYS.fetch_add(1, Ordering::Relaxed) + 1 >= 500 {
        post(TIMING);
    }
    r
}

unsafe fn key_proc_inner(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    LAST_EVENT.store(input::tick32(), Ordering::Relaxed);
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION, l points to a KBDLLHOOKSTRUCT.
        let k = unsafe { &*(l as *const KBDLLHOOKSTRUCT) };
        // our own typing (the fix) and other programs' injected keys are not the user's typing
        if k.flags & LLKHF_INJECTED == 0 {
            let msg = w as u32;
            if WANT_CTRLTAP.with(|c| c.get()) {
                let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                let ctrl = match k.vkCode as u16 {
                    VK_LCONTROL => Some(true),
                    VK_RCONTROL => Some(false),
                    _ => None,
                };
                if let Some(left) = CTRLTAP.with(|c| c.borrow_mut().key(ctrl, down, tick())) {
                    post(if left { CTRL_EN } else { CTRL_HE });
                }
            }
            if (msg == WM_KEYUP || msg == WM_SYSKEYUP) && k.vkCode == VK_RETURN as u32 {
                ENTER_HELD.with(|e| e.set(false));
                if SWALLOW_ENTER_UP.with(|s| s.replace(false)) {
                    return 1;
                }
            }
            // Enter held after it was held back: its repeats are held back too (never sent by
            // the repeat of the very press that was stopped)
            if (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) && k.vkCode == VK_RETURN as u32 && ENTER_HELD.with(|e| e.replace(true)) && SWALLOW_ENTER_UP.with(|s| s.get()) {
                return 1;
            }
            if WANT_DSHIFT.with(|d| d.get()) {
                let is_shift = matches!(k.vkCode as u16, VK_SHIFT | VK_LSHIFT | VK_RSHIFT);
                let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                let mods = matches!(k.vkCode as u16, VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU | VK_RMENU | VK_LWIN | VK_RWIN);
                // a key-up of another key, or Ctrl/Alt/Win held (Alt+Shift, Ctrl+Shift change
                // the layout or the direction): never a double Shift
                if (!is_shift && !down) || mods {
                    DSHIFT.with(|d| *d.borrow_mut() = t::DoubleShift::default());
                } else if DSHIFT.with(|d| d.borrow_mut().key(is_shift, down, tick()))
                    && !self::down(VK_CONTROL)
                    && !self::down(VK_MENU)
                    && !self::down(VK_LWIN)
                    && !self::down(VK_RWIN)
                {
                    post(DOUBLE_SHIFT);
                }
            }
            if (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) && on_key(k.vkCode, k.scanCode) {
                return 1;
            }
            if (msg == WM_KEYUP || msg == WM_SYSKEYUP) && k.vkCode == VK_TAB as u32 && SWALLOW_TAB_UP.with(|s| s.replace(false)) {
                return 1;
            }
            if (msg == WM_KEYUP || msg == WM_SYSKEYUP) && k.vkCode == VK_BACK as u32 && SWALLOW_BACK_UP.with(|s| s.replace(false)) {
                return 1;
            }
        }
    }
    // SAFETY: passes the event on.
    unsafe { CallNextHookEx(null_mut(), code, w, l) }
}

unsafe extern "system" fn mouse_proc(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    LAST_EVENT.store(input::tick32(), Ordering::Relaxed);
    // Ctrl with the wheel (zoom) or with a button is not a Ctrl tap
    if code == HC_ACTION as i32 && matches!(w as u32, WM_MOUSEWHEEL | WM_MOUSEHWHEEL | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN) {
        CTRLTAP.with(|c| c.borrow_mut().cancel());
    }
    if code == HC_ACTION as i32 && matches!(w as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
        reset(); // the caret may have moved: the words kept no longer end at the caret
        CTRLTAP.with(|c| c.borrow_mut().cancel()); // Ctrl+click is not a Ctrl tap
        DSHIFT.with(|d| *d.borrow_mut() = t::DoubleShift::default()); // Shift+click is not a double Shift
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
        TABLES => {
            // at most once every 2 s (a layout that cannot be read would ask at every key)
            let now = tick();
            if now.saturating_sub(TABLES_AT.with(|t| t.get())) >= 2_000 {
                TABLES_AT.with(|t| t.set(now));
                prepare_tables();
            }
        }
        DOUBLE_SHIFT => {
            // never in JetBrains IDEs, where double Shift opens "Search Everywhere"
            let fg = target::foreground();
            let exe = target::exe_of(fg);
            let jetbrains = (exe.to_ascii_lowercase().ends_with("64.exe") && t::is_code_editor(&exe)) || crate::platform::sys::class_name(fg) == "SunAwtFrame";
            if !jetbrains && !fullscreen_or_game() {
                app::convert_now();
            }
        }
        UNDO => undo(),
        REPLACE => replace_now(),
        REPL_REFRESH => refresh_replacements(false),
        VERIFY => verify_fix(),
        CTRL_EN | CTRL_HE if !fullscreen_or_game() => {
            let fg = target::foreground();
            let hkl = if code == CTRL_HE { layouts::hebrew_layout() } else { latin_layout(fg) };
            if let Some(h) = hkl {
                layouts::request(fg, h);
            }
        }
        CTRL_EN | CTRL_HE => {} // full-screen game or video: nothing
        GUARD => guard_check(),
        TIMING => {
            let n = HOOK_KEYS.swap(0, Ordering::Relaxed);
            let sum = HOOK_SUM_US.swap(0, Ordering::Relaxed);
            let max = HOOK_MAX_US.swap(0, Ordering::Relaxed);
            if n > 0 {
                log(&format!("Hook timing: {n} key events, average {} us, max {max} us", sum / n as u64));
            }
        }
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
    let fg = target::foreground();
    if fg as usize != hwnd || fullscreen_or_game() {
        return; // another window, or games / presentations / full-screen videos: never
    }
    // the window is checked BEFORE the lists are locked (target::info may pump messages)
    if !window_ok(fg, true) {
        return;
    }
    refresh_personal();
    let guard = DICT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(dict) = guard.as_ref() else { return };
    let (_, hkl) = focus_and_layout();
    let layouts::Choice::Map(map, latin_hkl) = layouts::choose(Some(hkl)) else { return }; // a 3rd layout: never
    let pairs: Vec<(String, String)> = words.iter().map(|w| (w.clone(), klf_core::convert::convert_with(&map, w, klf_core::settings::MixedTextMode::ToggleAll, true))).collect();
    let Some(s) = t::suggest_any(&pairs, hebrew, dict) else { return };
    drop(guard);
    let shown = if s.auto { s.text.trim_end().to_string() } else { format!("{}   Tab ↹", s.text.trim_end()) };
    crate::bubble::show_near_caret(&shown, s.to_hebrew, target::caret_point(fg), fg as usize);
    if app::settings().play_sound && !s.auto {
        crate::platform::sys::beep();
    }
    let auto = s.auto;
    PROPOSAL.with(|p| *p.borrow_mut() = Some(Proposal { s, hwnd: fg as usize, latin_hkl, at: tick(), words }));
    if auto {
        fix(); // a word taught with F10: fixed at once (Backspace undoes it)
    }
}

/// For Edit / RichEdit controls, the text just before the caret must be exactly what was typed
/// (dead keys, autocorrection, autocompletion... would make the erase hit other text). Other
/// controls cannot be read back: allowed.
pub fn text_before_caret_matches(expected: &str) -> bool {
    text_before_caret(expected, false)
}

/// 1.8.2, `word_start`: the expected text must also start a word (start of the field, or a
/// space / line break before it): "ho" + click + "tel" is not the shortcut "tel".
fn text_before_caret(expected: &str, word_start: bool) -> bool {
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
        if !before.ends_with(&exp) {
            return false;
        }
        !word_start || before[..before.len() - exp.len()].chars().next_back().is_none_or(char::is_whitespace)
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
    let layout_back = target::current_layout(fg).map(|h| h as u32);
    // 1.8.5: in a control that can be read back, what is before the words is remembered, to check
    // the result once typed (an autocompletion or a key typed meanwhile could spoil it)
    // (only the length and the end of the text before are kept: never the whole field)
    let check = edit_text_to_caret().map(|(focus, t)| {
        let units: Vec<u16> = t.encode_utf16().collect();
        let cut = units.len().saturating_sub(erase_text.encode_utf16().count());
        let tail_from = cut.saturating_sub(64);
        (focus, cut, String::from_utf16_lossy(&units[tail_from..cut]), erase_text.clone(), p.s.text.clone())
    });
    // counted BEFORE sending: a key typed while the fix is being typed is seen
    VERIFY_KEYS.with(|v| v.set(input::user_activity()));
    let owner = app::main_window() as usize;
    let verify = check.is_some();
    VERIFY_STATE.with(|v| *v.borrow_mut() = check);
    // typed from a worker thread (the hooks live on this one and must never wait); the check
    // comes 250 ms after the last character was sent
    input::replace_paced_then(p.s.erase, p.s.text.clone(), move || {
        if verify {
            std::thread::sleep(std::time::Duration::from_millis(250));
            // SAFETY: posting a message without pointers to our own window.
            unsafe { PostMessageW(owner as HWND, WM_APP_TYPING, VERIFY, 0) };
        }
    });
    LAST_FIX.with(|f| *f.borrow_mut() = Some(LastFix { learn: true, typed: erase_text.clone(), fixed: p.s.text.clone(), hwnd: fg as usize, layout_back, at: tick() }));
    let to = if p.s.keep_layout { None } else if p.s.to_hebrew { layouts::hebrew_layout() } else { p.latin_hkl };
    if let Some(h) = to {
        layouts::request(fg, h);
    }
    BUF.with(|b| b.borrow_mut().clear());
    if app::settings().typing_sound {
        crate::platform::sys::fix_sound();
    }
    log(&format!("Typing correction: {} word(s) fixed", p.s.text.split_whitespace().count()));
}

/// Backspace right after a fix: the text typed comes back, the keyboard too, and those words are
/// added to the personal file as "never fix".
fn undo() {
    let Some(f) = LAST_FIX.with(|f| f.borrow_mut().take()) else { return };
    let fg = target::foreground();
    if fg as usize != f.hwnd || !text_before_caret_matches(&f.fixed) || !input::replace(f.fixed.chars().count(), &f.typed) {
        // nothing undone: the Backspace the user pressed is given back
        let _ = input::replace(1, "");
        return;
    }
    if let Some(h) = f.layout_back {
        layouts::request(fg, h);
    }
    BUF.with(|b| b.borrow_mut().clear());
    if !f.learn {
        log("Text replacement undone");
        return;
    }
    let lines: Vec<String> = f.typed.split_whitespace().map(t::never_line).collect();
    append_personal(&lines);
    if app::settings().typing_sound {
        crate::platform::sys::undo_sound();
    }
    let l = app::lang();
    crate::bubble::show(text(l, Msg::TypingUndone), l.is_rtl(), target::caret_point(fg), fg as usize, true);
    log("Typing correction: fix undone, word(s) added to the personal list");
}

/// F10 fixed a single word: the "fix while typing" option learns it (added to my-words.txt as
/// "+word"), so it is fixed at once next time. Only when the option is on.
pub fn learn_from_f10(before: &str, after: &str) {
    if !(app::settings().typing_suggestions && allowed()) {
        return;
    }
    // never learned where the typing correction does not act (code editors, excluded programs)
    let info = target::info(target::foreground());
    let s = app::settings();
    if (s.typing_skip_code_editors && t::is_code_editor(&info.exe_name)) || s.rule_for(&info.exe_name) == Some(klf_core::settings::RuleMode::Disabled) || t::is_sensitive_app(&info.exe_name) || info.exe_name.is_empty() {
        return; // 1.8.4: nothing learned in a password manager or a remote desktop either
    }
    refresh_personal();
    let line = {
        let guard = DICT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(d) = guard.as_ref() else { return };
        t::f10_lesson_checked(before, after, Some(d))
    };
    // 1.8.6: learned only the second time ("?word date" the first time, forgotten after 30 days)
    if let Some(line) = line {
        learn_twice(&line[1..]);
    }
}

/// 1.8.6: one writer at a time for my-words.txt (learning, undo, round trip).
static PERSONAL_WRITE: Mutex<()> = Mutex::new(());

fn learn_twice(word: &str) {
    let Some(p) = personal_path() else { return };
    let _w = PERSONAL_WRITE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // a file that exists but cannot be read (shared, antivirus, OneDrive) is never replaced
    let current = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(_) => return,
    };
    if current.len() > 256 * 1024 {
        return;
    }
    let today = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| (d.as_secs() / 86_400) as u32).unwrap_or(0);
    let Some(new) = t::learn_step(&decode_text(&current), word, today) else { return };
    let tmp = p.with_extension("txt.learn.tmp");
    let written = (|| -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp)?;
        // UTF-8 with its mark: Notepad and every version of the program read it right
        f.write_all(b"\xEF\xBB\xBF")?;
        f.write_all(new.as_bytes())?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, &p)
    })();
    if written.is_ok() {
        refresh_personal();
    } else {
        let _ = std::fs::remove_file(&tmp);
        log("Typing correction: my-words.txt could not be written");
    }
}

/// 1.8.6: the Run dialog (Win+R) gets the English keyboard, with the one-language option.
pub fn run_dialog(hwnd: HWND, class: &str) {
    if class != "#32770" || !app::settings().one_language_programs || !allowed() {
        return;
    }
    if target::exe_of(hwnd) != "explorer.exe" {
        return;
    }
    // the Run dialog: a dialog of the shell whose command line is the combo box 12298
    // SAFETY: plain window handle.
    if unsafe { GetDlgItem(hwnd, 12298) }.is_null() {
        return;
    }
    // once per dialog (the user may switch back to Hebrew on purpose)
    if RUN_DIALOG.with(|r| r.get().0) == hwnd as usize {
        return;
    }
    let before = target::current_layout(hwnd).map(|c| c as u32);
    if let Some(h) = latin_layout(hwnd) {
        if before != Some(h) {
            layouts::request(hwnd, h);
            RUN_DIALOG.with(|r| r.set((hwnd as usize, before.unwrap_or(0), h)));
            log("Run dialog: keyboard switched to English");
        }
    }
}

/// 1.8.6: leaving the Run dialog: the keyboard it had before comes back, if it is still the one
/// we chose (the user did not change it).
pub fn left_run_dialog(fg: HWND) {
    let (dlg, before, ours) = RUN_DIALOG.with(|r| r.get());
    if dlg == 0 || dlg == fg as usize {
        return;
    }
    RUN_DIALOG.with(|r| r.set((0, 0, 0)));
    if before != 0 && target::current_layout(fg).map(|c| c as u32) == Some(ours) {
        layouts::request(fg, before);
    }
}

/// Where typing help may act: not a password field, not a program run as administrator, not a
/// console, not a program the user or the school excluded, and (for layout fixes) not a code
/// editor. Read at the moment of acting, for every word.
fn window_ok(fg: HWND, layout_fix: bool) -> bool {
    let info = target::info(fg);
    // 1.8.4: a program whose name cannot be read is not known to be safe
    if info.exe_name.is_empty() {
        return false;
    }
    if info.password_field || info.elevated == Some(true) || klf_core::policy::console_kind(&info.window_class).is_some() || !klf_core::policy::rule_allowed_for(&info.exe_name) {
        return false;
    }
    if app::settings().rule_for(&info.exe_name) == Some(klf_core::settings::RuleMode::Disabled) || t::is_sensitive_app(&info.exe_name) {
        return false; // 1.8.4: password managers, crypto wallets, remote desktop
    }
    !(layout_fix && app::settings().typing_skip_code_editors && t::is_code_editor(&info.exe_name))
}

/// 1.8.2, for the tray icon: why the typing help is asleep in this window, if it is.
pub fn dormant_reason(fg: HWND) -> Option<Msg> {
    if !active() || !WANT_TYPING.with(|w| w.get()) || fg.is_null() {
        return None;
    }
    let exe = target::exe_of(fg);
    if t::is_sensitive_app(&exe) {
        return Some(Msg::DormantSensitive);
    }
    if fullscreen_or_game() {
        return Some(Msg::DormantFullscreen);
    }
    if app::settings().rule_for(&exe) == Some(klf_core::settings::RuleMode::Disabled) || !klf_core::policy::rule_allowed_for(&exe) {
        return Some(Msg::DormantExcluded);
    }
    if app::settings().typing_skip_code_editors && t::is_code_editor(&exe) {
        return Some(Msg::DormantCodeEditor);
    }
    None
}

// ---------------------------------------------------------------------------------------------
// 1.8.2, text replacements (replacements.txt, empty by default)

/// (file date, list)
type Replacements = (Option<std::time::SystemTime>, HashMap<String, String>);
static REPL: Mutex<Option<Replacements>> = Mutex::new(None);
/// every start of every shortcut, read by the hook with try_lock (never waits)
static PREFIXES: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);
static REPL_READING: AtomicBool = AtomicBool::new(false);
/// when the file date was last looked at (ms)
static REPL_CHECKED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn replacements_path() -> Option<std::path::PathBuf> {
    crate::util::app_dir().map(|d| d.join("replacements.txt"))
}

/// Reads the list again if the file changed (the date is looked at every 3 s at most, or now
/// with `force`), on a thread of its own: a slow disk or network profile never holds the UI
/// thread, which runs the keyboard hook. A file that cannot be read (being saved) keeps the
/// previous list; a file too big gives no list.
pub fn refresh_replacements(force: bool) {
    let now = tick();
    if !force && now.saturating_sub(REPL_CHECKED.load(Ordering::Relaxed)) < 3_000 {
        return;
    }
    if REPL_READING.swap(true, Ordering::SeqCst) {
        return;
    }
    REPL_CHECKED.store(now, Ordering::Relaxed);
    let Some(p) = replacements_path() else {
        REPL_READING.store(false, Ordering::SeqCst);
        return;
    };
    let started = std::thread::Builder::new().name("klf-replacements".into()).spawn(move || {
        let meta = std::fs::metadata(&p).ok();
        let at = meta.as_ref().and_then(|m| m.modified().ok());
        let same = REPL.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().is_some_and(|(t, _)| *t == at);
        if !same {
            let list = match meta {
                None => Some(HashMap::new()), // no file: no replacement
                Some(m) if m.len() > 256 * 1024 => {
                    log("Text replacements: replacements.txt is larger than 256 KB, not used");
                    Some(HashMap::new())
                }
                Some(_) => std::fs::read(&p).ok().map(|b| klf_core::replace::parse(&decode_text(&b))),
            };
            if let Some(list) = list {
                let prefixes = klf_core::replace::prefixes(&list);
                *REPL.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((at, list));
                *PREFIXES.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(prefixes);
            }
        }
        REPL_READING.store(false, Ordering::SeqCst);
    });
    if started.is_err() {
        REPL_READING.store(false, Ordering::SeqCst);
    }
}

fn replacement_for(tok: &str) -> Option<String> {
    let g = REPL.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    g.as_ref().and_then(|(_, m)| m.get(tok).cloned())
}

/// Another program came to the front (from Notepad after editing the list, for instance).
pub fn on_foreground() {
    if WANT_REPL.with(|w| w.get()) {
        refresh_replacements(false);
    }
    let fg = target::foreground();
    let exe = target::exe_of(fg);
    layouts::note_active(target::current_layout(fg).map(|h| h as u32)); // 1.8.6: last Latin layout
    IS_MESSENGER.with(|m| m.set(t::is_messenger(&exe)));
    GUARD_ARMED.with(|g| g.set(None));
    SWALLOW_ENTER_UP.with(|s| s.set(false));
    // 1.8.5: one-language programs get their keyboard
    let s = app::settings();
    if s.one_language_programs && allowed() {
        if let Some(&hebrew) = one_language_list().get(&exe.to_ascii_lowercase()) {
            let want = if hebrew { layouts::hebrew_layout() } else { latin_layout(fg) };
            if let Some(h) = want {
                if target::current_layout(fg).map(|c| c as u32) != Some(h) {
                    layouts::request(fg, h);
                    log(&format!("One-language program {exe}: keyboard switched"));
                }
            }
        }
    }
}

/// The Latin layout to switch to (the one F10 would use), if any is installed.
fn latin_layout(fg: HWND) -> Option<u32> {
    match layouts::choose(target::current_layout(fg).map(|h| h as u32)) {
        layouts::Choice::Map(_, l) => l,
        _ => layouts::installed().into_iter().find(|h| h & 0x3FF != 0x0D),
    }
}

pub fn one_language_path() -> Option<std::path::PathBuf> {
    crate::util::app_dir().map(|d| d.join("one-language-programs.txt"))
}

/// The user's list, or the default one (terminals in English) while there is no file.
fn one_language_list() -> HashMap<String, bool> {
    let text = one_language_path().and_then(|p| std::fs::read(p).ok()).filter(|b| b.len() <= 64 * 1024).map(|b| decode_text(&b));
    t::parse_one_language(text.as_deref().unwrap_or(t::ONE_LANGUAGE_DEFAULT))
}

pub fn open_one_language() {
    let Some(p) = one_language_path() else { return };
    if !p.exists() {
        let l = app::lang();
        let _ = std::fs::write(&p, format!("# {}\r\n{}", text(l, Msg::OneLanguageHelp), t::ONE_LANGUAGE_DEFAULT));
    }
    let _ = std::process::Command::new("notepad.exe").arg(&p).spawn();
}

/// 1.8.5, chat guard, in the hook: the sentence being typed looks typed in the wrong language
/// (word lists only if free this instant: never waits).
fn sentence_wrong_now(hkl: u32) -> bool {
    let Some((words, hebrew)) = BUF.with(|b| b.borrow().snapshot()) else { return false };
    let Ok(guard) = DICT.try_lock() else { return false };
    let Some(dict) = guard.as_ref() else { return false };
    KEYMAPS.with(|k| {
        let Ok(k) = k.try_borrow() else { return false };
        let Some(map) = k.get(&hkl) else { return false };
        let pairs: Vec<(String, String)> = words.iter().map(|w| (w.clone(), klf_core::convert::convert_with(map, w, klf_core::settings::MixedTextMode::ToggleAll, true))).collect();
        t::sentence_wrong(&pairs, hebrew, dict)
    })
}

/// 1.8.5, chat guard: the warning, shown outside the hook.
fn guard_check() {
    let fg = target::foreground();
    let l = app::lang();
    crate::bubble::show_near_caret(text(l, Msg::MessengerWarning), l.is_rtl(), target::caret_point(fg), fg as usize);
    log("Chat guard: Enter held back, the sentence looks typed in the wrong language");
}

/// 1.8.5: the typing hooks are put back if Windows removed them (silently, when a hook was too
/// slow once). false = they should be there and could not be put back.
/// 1.8.6: when the hook brake stopped fixing while typing (0 = not braked); it comes back by
/// itself after 15 minutes.
static BRAKED_AT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn check_hooks() -> bool {
    let braked = BRAKED_AT.load(Ordering::SeqCst);
    if braked != 0 {
        if tick().saturating_sub(braked) < 15 * 60_000 {
            return false; // still braked: the alert stays, no new bubble
        }
        BRAKED_AT.store(0, Ordering::SeqCst);
        log("Safety brake over: fixing while typing comes back");
        apply();
        return true;
    }
    if !active() || !input::hook_lost(LAST_EVENT.load(Ordering::SeqCst)) || input::foreground_hides_input() {
        return true;
    }
    // at most once every 30 s (never a loop); the time is written only when repairing
    static LAST_FIX: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let now = tick();
    let last = LAST_FIX.load(Ordering::SeqCst);
    if last != 0 && now.saturating_sub(last) < 30_000 {
        return true;
    }
    LAST_FIX.store(now, Ordering::SeqCst);
    // 1.8.6, safety brake: removed 3 times within 5 minutes = the hook is too slow on this PC;
    // fixing while typing stops (with a warning) rather than coming back again and again
    static REPAIRS: std::sync::Mutex<Vec<u64>> = std::sync::Mutex::new(Vec::new());
    let many = {
        let mut r = REPAIRS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        r.retain(|x| now.saturating_sub(*x) < 300_000);
        r.push(now);
        r.len() >= 3
    };
    if many {
        log("Typing hooks removed by Windows 3 times in 5 minutes: fixing while typing stopped for 15 minutes (safety brake)");
        stop();
        BRAKED_AT.store(now, Ordering::SeqCst);
        REPAIRS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clear();
        let l = app::lang();
        crate::bubble::show(text(l, Msg::HookBrake), l.is_rtl(), None, 0, false);
        return false;
    }
    log("Typing hooks removed by Windows: put back");
    stop();
    start();
    active()
}

/// Every minute (UI thread).
pub fn periodic() {
    if WANT_REPL.with(|w| w.get()) {
        refresh_replacements(false);
    }
}

pub fn open_replacements() {
    let Some(p) = replacements_path() else { return };
    if !p.exists() {
        let l = app::lang();
        let _ = std::fs::write(&p, format!("# {}\r\n", text(l, Msg::ReplacementsHelp)));
    }
    let _ = std::process::Command::new("notepad.exe").arg(&p).spawn();
}

fn replace_now() {
    let Some((tok, hwnd)) = PENDING_REPL.with(|p| p.borrow_mut().take()) else { return };
    let fg = target::foreground();
    if fg as usize != hwnd || fullscreen_or_game() {
        return;
    }
    let Some(value) = replacement_for(&tok) else { return };
    if !window_ok(fg, false) {
        return;
    }
    let typed = format!("{tok} ");
    if !text_before_caret(&typed, true) {
        log("Text replacement: text changed, nothing done");
        return;
    }
    let fixed = format!("{value} ");
    if !input::replace(typed.chars().count(), &fixed) {
        log("Text replacement: could not be typed");
        return;
    }
    // the layout check for the same space is dropped: the text is no longer what was typed
    PENDING.with(|p| p.borrow_mut().take());
    PROPOSAL.with(|p| p.borrow_mut().take());
    BUF.with(|b| b.borrow_mut().clear());
    // undone with Backspace only when each character is one Backspace in every program (no
    // emoji, joiner or combining mark): else the undo could erase text before the replacement
    if klf_core::replace::simple_text(&fixed) {
        LAST_FIX.with(|f| *f.borrow_mut() = Some(LastFix { learn: false, typed, fixed, hwnd: fg as usize, layout_back: None, at: tick() }));
    } else {
        LAST_FIX.with(|f| f.borrow_mut().take());
    }
    log("Text replacement made");
}

/// 1.8.5, F10 "only the wrong words" (public version, option): the text with only its words typed
/// on the wrong layout converted; None without the lists or when no word looks wrong.
pub fn wrong_words_only(text: &str, map: &klf_core::layouts::KeyMap, replace_caps: bool) -> Option<String> {
    if !allowed() {
        return None;
    }
    refresh_personal();
    let guard = DICT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let dict = guard.as_ref()?;
    t::convert_wrong_words(text, dict, |w| klf_core::convert::convert_with(map, w, klf_core::settings::MixedTextMode::ToggleAll, replace_caps))
}

/// 1.8.5: the focused Edit / RichEdit control and its text up to the caret (no selection); None
/// for any other control (it cannot be read back).
fn edit_text_to_caret() -> Option<(usize, String)> {
    // SAFETY: structure sized; EM_GETSEL / WM_GETTEXTLENGTH / WM_GETTEXT are marshalled by
    // Windows for Edit and RichEdit controls; the buffer size is passed.
    unsafe {
        let mut gi: GUITHREADINFO = std::mem::zeroed();
        gi.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(0, &mut gi) == 0 || gi.hwndFocus.is_null() {
            return None;
        }
        let focus = gi.hwndFocus;
        let class = crate::platform::sys::class_name(focus);
        // plain Edit only: RichEdit counts a line break as 1 in its positions but returns CR LF
        // in its text, so positions and text would not match
        if !class.eq_ignore_ascii_case("Edit") {
            return None;
        }
        // a program that does not answer within 200 ms is not waited for (the hooks live here)
        let ask = |msg: u32, w: WPARAM, l: LPARAM| -> Option<usize> {
            let mut r: usize = 0;
            (SendMessageTimeoutW(focus, msg, w, l, SMTO_ABORTIFHUNG | SMTO_BLOCK, 200, &mut r) != 0).then_some(r)
        };
        let (mut start, mut end) = (0u32, 0u32);
        ask(0x00B0, &mut start as *mut u32 as WPARAM, &mut end as *mut u32 as LPARAM)?; // EM_GETSEL
        let len = ask(WM_GETTEXTLENGTH, 0, 0)?;
        if start != end || len > 100_000 || end as usize > len {
            return None;
        }
        let mut buf = vec![0u16; len + 1];
        let n = ask(WM_GETTEXT, buf.len(), buf.as_mut_ptr() as LPARAM)?;
        Some((focus as usize, String::from_utf16_lossy(&buf[..(end as usize).min(n)])))
    }
}

/// 1.8.5: 250 ms after a fix while typing, in a control that can be read back: the text must be
/// exactly what was before + the fix. Otherwise, if what was before is intact, the part after it
/// is replaced by the words as typed (one undoable replacement), and the user is told.
fn verify_fix() {
    let Some((focus, before_len, before_tail, typed, fixed)) = VERIFY_STATE.with(|v| v.borrow_mut().take()) else { return };
    // the user typed or clicked since: the text is theirs now, nothing is touched
    if input::user_activity() != VERIFY_KEYS.with(|v| v.get()) {
        return;
    }
    let Some((f, now)) = edit_text_to_caret() else { return };
    if f != focus {
        return;
    }
    let units: Vec<u16> = now.encode_utf16().collect();
    let fixed_units: Vec<u16> = fixed.encode_utf16().collect();
    let tail: Vec<u16> = before_tail.encode_utf16().collect();
    let before_intact = units.len() >= before_len && units[..before_len].ends_with(&tail);
    if before_intact && units[before_len..] == fixed_units[..] {
        return; // exactly the fix: all good
    }
    if !before_intact {
        log("Typing correction: text changed around the fix, left as it is");
        return;
    }
    let start = before_len;
    let end = units.len();
    let w = wide(&typed);
    // SAFETY: EM_SETSEL / EM_REPLACESEL on an Edit control; the string is NUL-terminated and lives
    // for the call.
    unsafe {
        SendMessageW(focus as HWND, 0x00B1, start, end as LPARAM); // EM_SETSEL
        SendMessageW(focus as HWND, 0x00C2, 1, w.as_ptr() as LPARAM); // EM_REPLACESEL, undoable
    }
    LAST_FIX.with(|f| f.borrow_mut().take());
    log("Typing correction: the result was not the fix (autocompletion?), the words as typed were put back");
    let l = app::lang();
    crate::bubble::show(text(l, Msg::TypingRestored), l.is_rtl(), None, 0, false);
}
