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
/// a pause longer than this starts a new run
const PAUSE_MS: u64 = 15_000;
/// a proposal stays valid this long
const PROPOSAL_MS: u64 = 5_000;

static DICT: Mutex<Option<Dict>> = Mutex::new(None);
/// date of the personal file when it was last read
static PERSONAL_AT: Mutex<Option<std::time::SystemTime>> = Mutex::new(None);

/// The lists are loaded (read in the hook without any lock).
static LISTS_READY: AtomicBool = AtomicBool::new(false);

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
fn append_personal(lines: &[String]) {
    let Some(p) = personal_path() else { return };
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
    static TOKEN: RefCell<klf_core::replace::Token> = RefCell::new(klf_core::replace::Token::default());
    static PENDING_REPL: RefCell<Option<(String, usize)>> = const { RefCell::new(None) };
    /// characters with Shift, for shortcuts such as "@@" (prepared outside the hook too)
    static MAPS_SHIFT: RefCell<HashMap<u32, HashMap<u32, char>>> = RefCell::new(HashMap::new());
    static TABLES_AT: Cell<u64> = const { Cell::new(0) };
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
    if repl {
        refresh_replacements(true);
    }
    if !typing && !dshift && !repl {
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
    if code == HC_ACTION as i32 {
        // SAFETY: for HC_ACTION, l points to a KBDLLHOOKSTRUCT.
        let k = unsafe { &*(l as *const KBDLLHOOKSTRUCT) };
        // our own typing (the fix) and other programs' injected keys are not the user's typing
        if k.flags & LLKHF_INJECTED == 0 {
            let msg = w as u32;
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
    if code == HC_ACTION as i32 && matches!(w as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
        reset(); // the caret may have moved: the words kept no longer end at the caret
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
fn text_before_caret_matches(expected: &str) -> bool {
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
    if !input::replace(p.s.erase, &p.s.text) {
        log("Typing correction: the fix could not be typed");
        return;
    }
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
    if (s.typing_skip_code_editors && t::is_code_editor(&info.exe_name)) || s.rule_for(&info.exe_name) == Some(klf_core::settings::RuleMode::Disabled) {
        return;
    }
    refresh_personal();
    let line = {
        let guard = DICT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(d) = guard.as_ref() else { return };
        t::f10_lesson_checked(before, after, Some(d))
    };
    if let Some(line) = line {
        append_personal(&[line]);
    }
}

/// Where typing help may act: not a password field, not a program run as administrator, not a
/// console, not a program the user or the school excluded, and (for layout fixes) not a code
/// editor. Read at the moment of acting, for every word.
fn window_ok(fg: HWND, layout_fix: bool) -> bool {
    let info = target::info(fg);
    if info.password_field || info.elevated == Some(true) || klf_core::policy::console_kind(&info.window_class).is_some() || !klf_core::policy::rule_allowed_for(&info.exe_name) {
        return false;
    }
    if app::settings().rule_for(&info.exe_name) == Some(klf_core::settings::RuleMode::Disabled) {
        return false;
    }
    !(layout_fix && app::settings().typing_skip_code_editors && t::is_code_editor(&info.exe_name))
}

/// 1.8.2, for the tray icon: why the typing help is asleep in this window, if it is.
pub fn dormant_reason(fg: HWND) -> Option<Msg> {
    if !active() || !WANT_TYPING.with(|w| w.get()) || fg.is_null() {
        return None;
    }
    if fullscreen_or_game() {
        return Some(Msg::DormantFullscreen);
    }
    let exe = target::exe_of(fg);
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
