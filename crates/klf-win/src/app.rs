//! The application: hidden top-level window (hotkey, tray icon, clipboard owner), tray menu,
//! worker thread for F10. The UI thread never blocks; F10 runs on a worker thread.

use crate::host::WinHost;
use crate::platform::{sys, target};
use crate::util::{log, settings_path, wide};
use crate::{bubble, settings_ui};
use klf_core::flow::{self, Outcome};
use klf_core::i18n::{text, text_with_key, Lang, Msg};
use klf_core::settings::{key_to_vk, Action, AppSettings, UiLanguage, KEY_NONE};
use std::cell::{Cell, RefCell};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::LoadIconWithScaleDown;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT};
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const WM_APP_TRAY: u32 = WM_APP + 1;
pub const WM_APP_DONE: u32 = WM_APP + 2;
/// Posted by a second copy started with --settings.
pub const WM_APP_SETTINGS: u32 = WM_APP + 3;
/// A hotkey pressed during a conversion, run once the conversion (and its clipboard restore) is over.
const WM_APP_PENDING: u32 = WM_APP + 4;
/// update (1.5.5): wparam 1 = a new version was found, 2 = installed (quit), 3 = failed
const WM_APP_UPDATE: u32 = WM_APP + 5;
const TIMER_UPDATE: usize = 5;
/// 1.6.0: first-run welcome (shown once the message loop runs)
const WM_APP_WELCOME: u32 = WM_APP + 6;

pub fn post_welcome() {
    let h = STATE.with(|s| s.hwnd.get());
    // SAFETY: posting a message without pointers to our window.
    unsafe { PostMessageW(h, WM_APP_WELCOME, 0, 0) };
}
static UPDATE_ERROR: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
/// one check / one installation at a time
static UPDATE_BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const MAIN_CLASS: &str = "KBLayoutFixerMain";
const HOTKEY_ID: i32 = 1;
const HOTKEY_TEST_ID: i32 = 2;
/// Second hotkey (F9): Caps Lock fix.
const HOTKEY_CASE_ID: i32 = 3;
const TIMER_RETRY: usize = 1;
const TIMER_TRAY: usize = 2;
/// Tray icon retries: every 10 s for 10 minutes, then only on TaskbarCreated.
const TRAY_RETRIES: u32 = 60;
const TRAY_ID: u32 = 1;
const NIN_KEYSELECT: u32 = NIN_SELECT | 1; // NINF_KEY = 1

const CMD_PAUSE: usize = 101;
const CMD_SETTINGS: usize = 102;
const CMD_ABOUT: usize = 103;
const CMD_EXIT: usize = 104;
const CMD_RULE_DISABLE: usize = 105;
const CMD_RULE_SELECTION: usize = 106;
const CMD_RULE_REMOVE: usize = 107;
const CMD_PAUSE10: usize = 108;
const CMD_RESTORE_CLIP: usize = 109;
const CMD_CHECK_UPDATES: usize = 110;
const CMD_INSTALL_UPDATE: usize = 111;
const CMD_PREVIEW: usize = 112;
// 1.6.1: anonymous problem reports (public install only)
const CMD_REPORT_SEND: usize = 113;
const CMD_REPORT_PROBLEM: usize = 114;
const CMD_REPORT_ALWAYS: usize = 115;
const CMD_REPORT_NEVER: usize = 116;
/// not a command: place of the "Problem reports" submenu
const MENU_REPORTS: usize = 199;
// 1.8.0: correction while typing (public install only)
const CMD_TYPING: usize = 117;
/// End of a timed pause (10 minutes).
const TIMER_PAUSE_END: usize = 3;
const TIMED_PAUSE_MS: u32 = 10 * 60 * 1000;
/// The administrator's settings are read again every 60 s (an administration tool writes them without a notice).
const TIMER_POLICY: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyState {
    Ready,
    Paused,
    Unavailable,
}

pub struct State {
    pub hwnd: Cell<HWND>,
    /// the user's own settings (settings.json)
    pub settings: RefCell<AppSettings>,
    /// what the administrator imposes (registry), read at start, every 60 s and when the settings open
    pub policy: RefCell<klf_core::school::Policy>,
    pub enabled: Cell<bool>,
    pub registered: Cell<bool>,
    pub busy: Arc<AtomicBool>,
    pub exit_requested: Cell<bool>,
    pub taskbar_created: Cell<u32>,
    pub tray_added: Cell<bool>,
    pub tray_retries: Cell<u32>,
}

thread_local! {
    /// Program of the last real window in the foreground (not ours, not the taskbar): the one the
    /// tray menu rules talk about. Name only (lower case), never a title.
    static LAST_APP: RefCell<String> = const { RefCell::new(String::new()) };
    /// "hh:mm" end of the timed pause (empty: no timed pause)
    static PAUSE_UNTIL: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Windows of the shell that take the foreground when the tray icon is clicked.
const SHELL_CLASSES: &[&str] = &["Shell_TrayWnd", "Shell_SecondaryTrayWnd", "NotifyIconOverflowWindow", "TopLevelWindowForOverflowXamlIsland", "Progman", "WorkerW", "XamlExplorerHostIslandWindow", "#32768"];
const SHELL_EXES: &[&str] = &["startmenuexperiencehost.exe", "searchhost.exe", "searchapp.exe", "shellexperiencehost.exe", "explorer.exe", "lockapp.exe", "textinputhost.exe", "applicationframehost.exe"];

unsafe extern "system" fn on_foreground(_hook: windows_sys::Win32::UI::Accessibility::HWINEVENTHOOK, _event: u32, hwnd: HWND, id_object: i32, _id_child: i32, _thread: u32, _time: u32) {
    // whole windows only (OBJID_WINDOW = 0)
    if id_object != 0 {
        return;
    }
    note_foreground(hwnd);
}

/// Remembers the program of a foreground window, unless it is the shell, ours, invisible, or a
/// suspended Store app (then the previous program is kept).
fn note_foreground(hwnd: HWND) {
    // SAFETY: IsWindowVisible has no pointers.
    if hwnd.is_null() || unsafe { IsWindowVisible(hwnd) } == 0 {
        return;
    }
    crate::typing::reset(); // another window: the words kept are forgotten
    let class = sys::class_name(hwnd);
    if SHELL_CLASSES.contains(&class.as_str()) {
        return;
    }
    crate::indicator::soon();
    let exe = target::exe_of(hwnd);
    if exe.is_empty() || SHELL_EXES.contains(&exe.as_str()) || exe == "keyboardlayoutfixer.exe" {
        return;
    }
    LAST_APP.with(|a| *a.borrow_mut() = exe);
}

/// Follows the foreground window (out of context: no DLL is injected anywhere).
fn watch_foreground() {
    use windows_sys::Win32::UI::Accessibility::SetWinEventHook;
    // SAFETY: callback of this module, called on this (UI) thread while it pumps messages.
    let h = unsafe { SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, null_mut(), Some(on_foreground), 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS) };
    if h.is_null() {
        log("Foreground window cannot be followed: the tray menu will not offer per-program rules");
    }
}

thread_local! {
    pub static STATE: State = State {
        hwnd: Cell::new(null_mut()),
        settings: RefCell::new(AppSettings::default()),
        policy: RefCell::new(klf_core::school::Policy::default()),
        enabled: Cell::new(true),
        registered: Cell::new(false),
        busy: Arc::new(AtomicBool::new(false)),
        exit_requested: Cell::new(false),
        taskbar_created: Cell::new(0),
        tray_added: Cell::new(false),
        tray_retries: Cell::new(0),
    };
}

/// The settings in force: the user's, with what the administrator imposes on top.
pub fn settings() -> AppSettings {
    STATE.with(|s| klf_core::school::effective(&s.settings.borrow(), &s.policy.borrow()).0)
}

/// The user's own settings (what settings.json holds and the settings window saves).
pub fn user_settings() -> AppSettings {
    STATE.with(|s| s.settings.borrow().clone())
}

pub fn policy() -> klf_core::school::Policy {
    STATE.with(|s| s.policy.borrow().clone())
}

/// Which settings the administrator imposes.
pub fn locked() -> klf_core::school::Locked {
    STATE.with(|s| klf_core::school::effective(&s.settings.borrow(), &s.policy.borrow()).1)
}

/// Reads the administrator's settings again; when they changed: keys registered again, indicator and
/// settings window updated. Returns true if something changed.
pub fn reload_policy() -> bool {
    let p = klf_core::school::from_values(&crate::platform::school::read());
    let changed = STATE.with(|s| *s.policy.borrow() != p);
    if !changed {
        return false;
    }
    if p.is_empty() {
        log("School settings: none");
    } else {
        log(&format!("School settings: {}", klf_core::school::report_lines(&p).join(" ; ")));
    }
    let hwnd = STATE.with(|s| s.hwnd.get());
    let before = settings();
    STATE.with(|s| *s.policy.borrow_mut() = p);
    let after = settings();
    if !hwnd.is_null() {
        let keys_changed = (before.hotkey_modifiers, before.hotkey_key, before.case_hotkey_modifiers, before.case_hotkey_key) != (after.hotkey_modifiers, after.hotkey_key, after.case_hotkey_modifiers, after.case_hotkey_key);
        if keys_changed && hotkey_state() != HotkeyState::Paused {
            unregister_hotkey();
            register_hotkey();
        }
        crate::indicator::configure();
        update_tray();
        settings_ui::refresh_after_policy();
    }
    true
}

pub fn lang() -> Lang {
    lang_of(&settings())
}

pub fn lang_of(s: &AppSettings) -> Lang {
    match s.ui_language {
        UiLanguage::He => Lang::He,
        UiLanguage::Fr => Lang::Fr,
        UiLanguage::En => Lang::En,
        UiLanguage::Auto => Lang::from_langid(sys::ui_langid()),
    }
}

pub fn hotkey_state() -> HotkeyState {
    STATE.with(|s| {
        if !s.enabled.get() {
            HotkeyState::Paused
        } else if s.registered.get() {
            HotkeyState::Ready
        } else {
            HotkeyState::Unavailable
        }
    })
}

/// A conversion is running (the copy buttons of the settings window wait).
pub fn busy() -> bool {
    STATE.with(|s| s.busy.load(Ordering::SeqCst))
}

pub fn key_name() -> String {
    settings().hotkey_description()
}

pub fn instance() -> HINSTANCE {
    // SAFETY: null = our own module.
    unsafe { GetModuleHandleW(null()) }
}

// ---------------------------------------------------------------------------------------------
// settings file

pub fn load_settings() -> AppSettings {
    // at every start (idempotent, decided by the content): a journal of the original program
    // with typed texts never stays, even on a PC already moved to 1.3.x
    purge_original_log();
    let Some(path) = settings_path() else { return AppSettings::default() };
    match std::fs::read_to_string(&path) {
        Ok(t) => {
            let (s, ok) = AppSettings::from_json(&t);
            if !ok {
                log("settings.json unreadable: defaults used, file left as it is");
            } else if s.migrated {
                log("settings.json from an older version (before 1.3.3): console support switched off once, Ctrl+Q of the original program replaced by F10");
                save_settings(&s);
            }
            s
        }
        Err(_) => {
            let s = AppSettings::default();
            if !path.exists() {
                save_settings(&s);
            }
            s
        }
    }
}

/// The ORIGINAL program wrote every captured/converted text in debug.log: such a journal is
/// deleted once, when its settings are taken over (our own journal never contains text).
fn purge_original_log() {
    let Some(dir) = crate::util::app_dir() else { return };
    for name in ["debug.log", "debug.log.old"] {
        let p = dir.join(name);
        // read line by line (an old journal can be large), stops at the first typed text
        let has_text = std::fs::File::open(&p).map(|f| {
            use std::io::BufRead;
            let mut r = std::io::BufReader::new(f);
            let mut line = Vec::new();
            while matches!(r.read_until(b'\n', &mut line), Ok(n) if n > 0) {
                let t = String::from_utf8_lossy(&line);
                if t.contains("Captured text:") || t.contains("Converted text:") || t.contains("captured: '") {
                    return true;
                }
                line.clear();
            }
            false
        });
        if has_text.unwrap_or(false) {
            match std::fs::remove_file(&p) {
                Ok(()) => log(&format!("{name} of the original program deleted (it contained the typed texts)")),
                Err(e) => log(&format!("{name} of the original program (typed texts) NOT deleted: {e}")),
            }
        }
    }
}

/// Writes settings.json through a temporary file then a rename (MoveFileExW REPLACE_EXISTING):
/// a cut in the middle never leaves a half-written file.
pub fn save_settings(s: &AppSettings) -> bool {
    use std::io::Write;
    let Some(path) = settings_path() else { return false };
    let tmp = path.with_extension("json.tmp");
    let written = (|| -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(s.to_json().as_bytes())?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, &path)
    })();
    match written {
        Ok(()) => true,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            log(&format!("settings.json could not be written: {e}"));
            false
        }
    }
}

/// Applies new USER settings (settings window, menu, import): the administrator's settings stay on
/// top; hotkey re-registered, indicator and tray updated.
pub fn apply_settings(new: AppSettings) {
    STATE.with(|s| *s.settings.borrow_mut() = new);
    if hotkey_state() != HotkeyState::Paused {
        unregister_hotkey();
        register_hotkey();
    }
    crate::indicator::configure();
    update_tray();
    crate::typing::apply();
}

// ---------------------------------------------------------------------------------------------
// hotkey

thread_local! {
    /// last result of the Caps Lock hotkey registration (0 = ok), to log changes only
    static CASE_STATE: Cell<u32> = const { Cell::new(u32::MAX) };
    /// the Caps Lock hotkey is registered, or switched off (nothing missing)
    static CASE_OK: Cell<bool> = const { Cell::new(true) };
}

/// The Caps Lock hotkey is registered or switched off (false = wanted but taken by another program).
pub fn case_hotkey_ok() -> bool {
    CASE_OK.with(|c| c.get())
}

/// The second hotkey (Caps Lock fix, F9). Its failure never stops F10; written in the log.
fn register_case_hotkey() {
    let s = settings();
    let hwnd = STATE.with(|st| st.hwnd.get());
    // SAFETY: our window, on its own thread (an id registered twice would be kept twice).
    unsafe { UnregisterHotKey(hwnd, HOTKEY_CASE_ID) };
    if s.case_hotkey_key == KEY_NONE {
        CASE_OK.with(|c| c.set(true));
        return;
    }
    let vk = key_to_vk(s.case_hotkey_key);
    // SAFETY: our window, on its own thread.
    // SAFETY: our window, on its own thread.
    let ok = vk != 0 && unsafe { RegisterHotKey(hwnd, HOTKEY_CASE_ID, s.case_hotkey_modifiers | MOD_NOREPEAT, vk) } != 0;
    CASE_OK.with(|c| c.set(ok));
    // SAFETY: right after the call (meaningless when vk == 0, then 0 is written).
    let err = if ok || vk == 0 { 0 } else { unsafe { GetLastError() } };
    // written once per state change (the retry timer calls this every 30 s)
    let state = if ok { 0 } else { err.max(1) };
    if CASE_STATE.with(|c| c.replace(state)) != state {
        if ok {
            log(&format!("Caps Lock hotkey registered: {}", s.case_hotkey_description()));
        } else {
            log(&format!("Caps Lock hotkey {} registration failed: error {err} (F10 still works; tried again every 30 s)", s.case_hotkey_description()));
        }
    }
}

pub fn register_hotkey() -> bool {
    let s = settings();
    let hwnd = STATE.with(|st| st.hwnd.get());
    let vk = key_to_vk(s.hotkey_key);
    register_case_hotkey();
    if !crate::platform::input::watch_caps_lock() {
        log("Caps Lock presses cannot be followed: Caps Lock will be left as it is after F9");
    }
    // SAFETY: our window, on its own thread (unregistered first: never kept twice).
    unsafe { UnregisterHotKey(hwnd, HOTKEY_ID) };
    // SAFETY: our window, on its own thread.
    let ok = vk != 0 && unsafe { RegisterHotKey(hwnd, HOTKEY_ID, s.hotkey_modifiers | MOD_NOREPEAT, vk) } != 0;
    STATE.with(|st| st.registered.set(ok));
    if ok {
        // the retry timer stays on while the Caps Lock hotkey is missing (audit 1.3.5, N4)
        // SAFETY: our window.
        unsafe {
            if case_hotkey_ok() {
                KillTimer(hwnd, TIMER_RETRY);
            } else {
                SetTimer(hwnd, TIMER_RETRY, 30_000, None);
            }
        }
        log(&format!("Hotkey registered: {}", s.hotkey_description()));
    } else {
        // SAFETY: GetLastError right after the call.
        let err = unsafe { GetLastError() };
        let why = match err {
            1409 => " = key already used by another program",
            _ => "",
        };
        log(&format!("Hotkey registration failed: error {err}{why} (retry every 30 s)"));
        // another program may hold F10 at logon: retry every 30 s
        // SAFETY: our window.
        unsafe {
            SetTimer(hwnd, TIMER_RETRY, 30_000, None);
        }
    }
    update_tray();
    ok
}

/// Is this hotkey free? Tried with a temporary id, then released at once.
pub fn hotkey_free(mods: u32, key: u32) -> bool {
    let hwnd = STATE.with(|st| st.hwnd.get());
    let vk = key_to_vk(key);
    // SAFETY: our window, on its own thread.
    unsafe {
        if vk == 0 || RegisterHotKey(hwnd, HOTKEY_TEST_ID, mods | MOD_NOREPEAT, vk) == 0 {
            return false;
        }
        UnregisterHotKey(hwnd, HOTKEY_TEST_ID);
    }
    true
}

pub fn unregister_hotkey() {
    let hwnd = STATE.with(|st| st.hwnd.get());
    // SAFETY: our window, on its own thread.
    unsafe {
        KillTimer(hwnd, TIMER_RETRY);
        UnregisterHotKey(hwnd, HOTKEY_ID);
        UnregisterHotKey(hwnd, HOTKEY_CASE_ID);
    }
    STATE.with(|st| st.registered.set(false));
}

/// Pause for 10 minutes: the keys are given back to the other programs, then taken again; if
/// one is taken meanwhile, it is said (bubble) and tried again every 30 s.
pub fn pause_for_a_while() {
    let hwnd = STATE.with(|st| st.hwnd.get());
    if hotkey_state() != HotkeyState::Paused {
        toggle_pause();
    }
    // SAFETY: our window.
    unsafe { SetTimer(hwnd, TIMER_PAUSE_END, TIMED_PAUSE_MS, None) };
    let end = {
        let now = sys::local_clock(); // "hh:mm:ss"
        let h: u32 = now.get(0..2).and_then(|x| x.parse().ok()).unwrap_or(0);
        let m: u32 = now.get(3..5).and_then(|x| x.parse().ok()).unwrap_or(0);
        let t = (h * 60 + m + 10) % (24 * 60);
        format!("{:02}:{:02}", t / 60, t % 60)
    };
    log(&format!("Paused for 10 minutes (until {end})"));
    PAUSE_UNTIL.with(|p| *p.borrow_mut() = end);
}

fn end_timed_pause(hwnd: HWND) {
    // SAFETY: our window.
    unsafe { KillTimer(hwnd, TIMER_PAUSE_END) };
    PAUSE_UNTIL.with(|p| p.borrow_mut().clear());
    if hotkey_state() == HotkeyState::Paused {
        toggle_pause();
        if !STATE.with(|s| s.registered.get()) {
            announce_hotkey_unavailable();
        }
    }
}

/// Adds, changes or removes the rule of a program (tray menu). A rule of the administrator cannot be
/// changed here.
fn set_rule(exe: &str, mode: Option<klf_core::settings::RuleMode>) {
    if klf_core::school::school_rule(&policy(), exe) {
        return;
    }
    let mut s = user_settings();
    if !s.set_rule(exe, mode) {
        log(&format!("Rule not allowed for {exe}"));
        return;
    }
    if save_settings(&s) {
        log(&format!("Rule for {exe}: {mode:?}"));
        apply_settings(s);
        settings_ui::refresh_status();
    } else {
        log("Rule could not be saved (settings.json)");
    }
}

/// Pause gives the key back to the other programs (UnregisterHotKey); resume takes it again.
pub fn toggle_pause() {
    let enabled = STATE.with(|s| {
        let e = !s.enabled.get();
        s.enabled.set(e);
        e
    });
    log(if enabled { "App enabled" } else { "App disabled" });
    if enabled {
        register_hotkey();
    } else {
        unregister_hotkey();
        update_tray();
    }
    settings_ui::refresh_status();
}

// ---------------------------------------------------------------------------------------------
// tray icon

fn load_icon(id: u16, size: i32) -> HICON {
    let mut icon: HICON = null_mut();
    // SAFETY: resource id as MAKEINTRESOURCE; icon written.
    unsafe {
        if LoadIconWithScaleDown(instance(), id as usize as *const u16, size, size, &mut icon) < 0 || icon.is_null() {
            icon = LoadIconW(null_mut(), IDI_APPLICATION);
        }
    }
    icon
}

pub fn app_icon(size: i32) -> HICON {
    load_icon(1, size)
}

/// Copies `s` into a fixed buffer, truncated, always NUL-terminated.
fn copy_wide(dst: &mut [u16], s: &str) {
    let max = dst.len().saturating_sub(1);
    let mut n = 0;
    for (d, c) in dst.iter_mut().zip(s.encode_utf16().take(max)) {
        *d = c;
        n += 1;
    }
    if let Some(z) = dst.get_mut(n) {
        *z = 0;
    }
}

fn tray_data() -> NOTIFYICONDATAW {
    let hwnd = STATE.with(|s| s.hwnd.get());
    let l = lang();
    let key = key_name();
    let (tip, icon_id) = match hotkey_state() {
        HotkeyState::Ready => (text_with_key(l, Msg::TipReady, &key), 1),
        HotkeyState::Paused => (text_with_key(l, Msg::TipPaused, &key), 2),
        HotkeyState::Unavailable => (text_with_key(l, Msg::TipUnavailable, &key), 2),
    };
    // SAFETY: GetSystemMetrics has no pointers.
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) };
    // SAFETY: zeroed is a valid NOTIFYICONDATAW.
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ID;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
    nid.uCallbackMessage = WM_APP_TRAY;
    nid.hIcon = load_icon(icon_id, size);
    copy_wide(&mut nid.szTip, &tip);
    nid
}

fn add_tray() -> bool {
    let mut nid = tray_data();
    // SAFETY: valid structure.
    let ok = unsafe {
        let mut ok = Shell_NotifyIconW(NIM_ADD, &nid) != 0;
        // documented case: a busy shell reports ERROR_TIMEOUT although the icon was added
        if !ok && GetLastError() == ERROR_TIMEOUT {
            ok = Shell_NotifyIconW(NIM_MODIFY, &nid) != 0;
        }
        if ok {
            nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &nid);
        }
        DestroyIcon(nid.hIcon); // the shell keeps its own copy
        ok
    };
    STATE.with(|s| s.tray_added.set(ok));
    ok
}

/// Adds the tray icon; if the taskbar is not ready (logon) or refuses, retries every 10 s
/// (10 minutes), and again at each TaskbarCreated. F10 works without the icon.
pub fn ensure_tray() {
    let hwnd = STATE.with(|s| s.hwnd.get());
    if add_tray() {
        // SAFETY: our window.
        unsafe { KillTimer(hwnd, TIMER_TRAY) };
        log("Tray icon ready");
        return;
    }
    let n = STATE.with(|s| {
        let n = s.tray_retries.get() + 1;
        s.tray_retries.set(n);
        n
    });
    if n == 1 {
        log("Tray icon not added yet (taskbar not ready?): retry every 10 s; settings: Start menu shortcut");
    }
    // SAFETY: our window.
    unsafe {
        if n <= TRAY_RETRIES {
            SetTimer(hwnd, TIMER_TRAY, 10_000, None);
        } else {
            KillTimer(hwnd, TIMER_TRAY);
            log("Tray icon still missing after 10 minutes: waiting for TaskbarCreated");
        }
    }
}

/// Bubble at start when the hotkey is taken (the tray icon may not be there yet).
pub fn announce_hotkey_unavailable() {
    let s = settings();
    if s.show_bubble {
        let l = lang_of(&s);
        bubble::show(&text_with_key(l, Msg::HotkeyBusy, &s.hotkey_description()), l.is_rtl(), None, 0, false);
    }
}

/// Second copy started with --settings: asks the copy already running in this session to open
/// its settings window (works even when its tray icon is missing).
pub fn open_settings_in_running_copy() {
    let class = wide(MAIN_CLASS);
    // SAFETY: valid class name; pid written; lets that copy bring its window to the front.
    unsafe {
        let hwnd = FindWindowW(class.as_ptr(), null());
        if hwnd.is_null() {
            return;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        AllowSetForegroundWindow(pid);
        PostMessageW(hwnd, WM_APP_SETTINGS, 0, 0);
    }
}

pub fn update_tray() {
    if STATE.with(|s| s.hwnd.get().is_null()) {
        return;
    }
    let nid = tray_data();
    // SAFETY: valid structure; the shell keeps its own copy of the icon.
    unsafe {
        Shell_NotifyIconW(NIM_MODIFY, &nid);
        DestroyIcon(nid.hIcon);
    }
}

pub fn main_window() -> HWND {
    STATE.with(|s| s.hwnd.get())
}

/// Windows notification next to the icon: "Send an anonymous report? Click here." (1.6.1)
pub fn show_report_notification() -> bool {
    if !STATE.with(|s| s.tray_added.get()) {
        return false; // no icon yet: the report stays offered in the menu
    }
    let l = lang();
    // SAFETY: zeroed is a valid NOTIFYICONDATAW; only the notification fields are changed.
    unsafe {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = main_window();
        nid.uID = TRAY_ID;
        nid.uFlags = NIF_INFO;
        nid.dwInfoFlags = NIIF_INFO | NIIF_RESPECT_QUIET_TIME;
        copy_wide(&mut nid.szInfoTitle, text(l, Msg::ReportAskTitle));
        copy_wide(&mut nid.szInfo, text(l, Msg::ReportAskText));
        Shell_NotifyIconW(NIM_MODIFY, &nid) != 0
    }
}

fn remove_tray() {
    let nid = tray_data();
    // SAFETY: valid structure.
    unsafe {
        Shell_NotifyIconW(NIM_DELETE, &nid);
        DestroyIcon(nid.hIcon);
    }
}

fn show_menu(hwnd: HWND) {
    let l = lang();
    let paused = hotkey_state() == HotkeyState::Paused;
    let s = settings();
    let app = LAST_APP.with(|a| a.borrow().clone());
    let until = PAUSE_UNTIL.with(|p| p.borrow().clone());
    let mut items: Vec<(usize, String, bool)> = vec![(CMD_SETTINGS, text(l, Msg::MenuSettings).to_string(), false), (0, String::new(), false)];
    let school = klf_core::school::school_rule(&policy(), &app);
    if klf_core::policy::rule_allowed_for(&app) {
        let rule = s.rule_for(&app);
        // shown as it is, shortened, & doubled (a menu would take it as a shortcut letter)
        let shown: String = if app.chars().count() > 40 { app.chars().take(39).chain(std::iter::once('…')).collect() } else { app.clone() };
        let shown = shown.replace('&', "&&");
        let with_app = |m: Msg| text(l, m).replace("{app}", &shown);
        items.push((CMD_RULE_DISABLE, with_app(Msg::MenuRuleDisable), rule == Some(klf_core::settings::RuleMode::Disabled)));
        items.push((CMD_RULE_SELECTION, with_app(Msg::MenuRuleSelection), rule == Some(klf_core::settings::RuleMode::SelectionOnly)));
        if rule.is_some() && !school {
            items.push((CMD_RULE_REMOVE, with_app(Msg::MenuRuleRemove), false));
        }
        items.push((0, String::new(), false));
    }
    if paused {
        let t = if until.is_empty() { text(l, Msg::MenuResume).to_string() } else { text(l, Msg::MenuResumeAt).replace("{time}", &until) };
        items.push((CMD_PAUSE, t, false));
    } else {
        items.push((CMD_PAUSE10, text(l, Msg::MenuPause10).to_string(), false));
        items.push((CMD_PAUSE, text(l, Msg::MenuPause).to_string(), false));
    }
    if crate::update::allowed(true) {
        let on = settings().check_updates;
        if on {
            if let Some(r) = crate::update::OFFER.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().filter(|_| !UPDATE_BUSY.load(Ordering::SeqCst)) {
                items.push((CMD_INSTALL_UPDATE, text(l, Msg::MenuInstallUpdate).replace("{version}", r.tag.trim_start_matches('v')), false));
            }
        }
        items.push((CMD_CHECK_UPDATES, text(l, Msg::MenuCheckUpdates).to_string(), on));
    }
    if settings().show_bubble {
        items.push((CMD_PREVIEW, text(l, Msg::MenuPreview).to_string(), settings().show_preview));
    }
    if crate::host::parked_available() {
        items.push((CMD_RESTORE_CLIP, text(l, Msg::MenuRestoreClipboard).to_string(), false));
    }
    if crate::typing::allowed() {
        items.push((CMD_TYPING, text(l, Msg::MenuTyping).to_string(), s.typing_suggestions));
    }
    let reports = crate::report::enabled();
    if reports {
        if crate::report::pending() {
            items.push((CMD_REPORT_SEND, text(l, Msg::MenuSendReport).to_string(), false));
        }
        items.push((MENU_REPORTS, text(l, Msg::MenuReports).to_string(), false));
    }
    items.push((CMD_ABOUT, text(l, Msg::MenuAbout).to_string(), false));
    items.push((0, String::new(), false));
    items.push((CMD_EXIT, text(l, Msg::MenuExit).to_string(), false));
    MENU_APP.with(|m| *m.borrow_mut() = app);
    // SAFETY: menu created and destroyed here; strings live until TrackPopupMenu returns.
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        let strings: Vec<(usize, Vec<u16>, bool)> = items.iter().map(|(id, t, c)| (*id, wide(t), *c)).collect();
        for (id, w, check) in &strings {
            if *id == 0 {
                AppendMenuW(menu, MF_SEPARATOR, 0, null());
            } else if *id == MENU_REPORTS {
                let sub = CreatePopupMenu();
                if !sub.is_null() {
                    let mode = s.report_mode;
                    let subs = [
                        (CMD_REPORT_PROBLEM, wide(text(l, Msg::MenuReportProblem)), false),
                        (CMD_REPORT_ALWAYS, wide(text(l, Msg::MenuReportAlways)), mode == klf_core::report::ALWAYS),
                        (CMD_REPORT_NEVER, wide(text(l, Msg::MenuReportNever)), mode == klf_core::report::NEVER),
                    ];
                    for (sid, sw, sc) in &subs {
                        AppendMenuW(sub, MF_STRING | if *sc { MF_CHECKED } else { 0 }, *sid, sw.as_ptr());
                    }
                    // the submenu is destroyed with the menu
                    AppendMenuW(menu, MF_STRING | MF_POPUP, sub as usize, w.as_ptr());
                }
            } else {
                // a rule of the administrator is shown, ticked, but cannot be changed here
                let grey = (school && matches!(*id, CMD_RULE_DISABLE | CMD_RULE_SELECTION)) || (*id == CMD_PREVIEW && locked().show_preview);
                AppendMenuW(menu, MF_STRING | if *check { MF_CHECKED } else { 0 } | if grey { MF_GRAYED } else { 0 }, *id, w.as_ptr());
            }
        }
        SetMenuDefaultItem(menu, CMD_SETTINGS as u32, 0);
        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        // known Win32 rule: the window must be in the foreground, or the menu does not close
        SetForegroundWindow(hwnd);
        let mut flags = TPM_RIGHTBUTTON | TPM_BOTTOMALIGN;
        if l.is_rtl() {
            flags |= TPM_LAYOUTRTL | TPM_RIGHTALIGN;
        }
        TrackPopupMenu(menu, flags, pt.x, pt.y, 0, hwnd, null());
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
    }
}

thread_local! {
    /// program named in the menu that was shown (the rule commands act on it)
    static MENU_APP: RefCell<String> = const { RefCell::new(String::new()) };
}

pub fn show_about(owner: HWND) {
    let l = lang();
    let body = text_with_key(l, Msg::AboutText, &key_name()).replace("{version}", klf_core::VERSION);
    let title = wide(text(l, Msg::WindowTitle));
    let body = wide(&body);
    let mut flags = MB_OK | MB_ICONINFORMATION;
    if l.is_rtl() {
        flags |= MB_RTLREADING | MB_RIGHT;
    }
    // SAFETY: valid strings.
    unsafe {
        MessageBoxW(owner, body.as_ptr(), title.as_ptr(), flags);
    }
}

fn request_exit(hwnd: HWND) {
    // never quit in the middle of a conversion (the clipboard would not be put back)
    if STATE.with(|s| s.busy.load(Ordering::SeqCst)) {
        STATE.with(|s| s.exit_requested.set(true));
    } else {
        // SAFETY: our window.
        unsafe {
            DestroyWindow(hwnd);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// F10

struct Done {
    outcome: Outcome,
    caret: Option<(i32, i32)>,
    target: usize,
}

fn on_hotkey(hwnd: HWND, action: Action) {
    start_hotkey(hwnd, action, false);
}

/// `delayed`: a press kept during the previous conversion (1.5.1): run in careful mode (only a
/// selection, never "caret -> start of the line": the text just pasted is not selected, it
/// would be converted back), and never into the settings window boxes.
fn start_hotkey(hwnd: HWND, action: Action, delayed: bool) {
    if delayed && settings_ui::hotkey_box_has_focus() {
        return;
    }
    if settings_ui::hotkey_box_has_focus() {
        // the user is choosing a new key in the settings window: show the key, do not convert
        settings_ui::put_pressed_hotkey_in_box(action);
        return;
    }
    // trying F10/F9 in the converter text of the settings window: done in the field itself,
    // without the clipboard (our window would be both the target and the clipboard owner)
    if settings_ui::convert_in_try_field(action) {
        return;
    }
    let (enabled, busy) = STATE.with(|s| (s.enabled.get(), s.busy.clone()));
    if !enabled {
        return;
    }
    if busy.swap(true, Ordering::SeqCst) {
        // 1.5.1: a press during a conversion (often during the 1 s wait before the
        // clipboard is put back) is no longer lost: it runs right after, once the user's clipboard
        // is back (never before: we would save our own converted text as theirs). One at most.
        PENDING.with(|p| p.set(Some((action, target::foreground() as usize))));
        return;
    }
    // a real conversion starts: nothing older is kept
    if !delayed {
        PENDING.with(|p| p.set(None));
    }
    // no language/Caps Lock sign during the conversion (its own switch, F9's Caps Lock press)
    crate::indicator::after_conversion();
    let mut s = settings();
    s.action = action;
    if delayed {
        s.require_selection = true;
    }
    let owner = hwnd as usize;
    let vk = key_to_vk(if action == Action::Case { s.case_hotkey_key } else { s.hotkey_key }) as u16;
    // Caps Lock at F9: state read on the window that receives the keys + count of Caps Lock
    // presses so far (UI thread, before any key is sent; audit 1.3.7 P2)
    // the target is the window in the foreground NOW, at the hotkey (audit A03)
    let fg_at_hotkey = target::foreground() as usize;
    let caps_at_hotkey = if action == Action::Case && s.turn_off_caps_lock {
        let fg = fg_at_hotkey as HWND;
        let state = if fg.is_null() { None } else { crate::platform::input::caps_lock_on_now(fg) };
        (state, crate::platform::input::caps_presses())
    } else {
        (None, None)
    };
    let spawned = std::thread::Builder::new().name("f10".into()).spawn(move || {
        log(if s.action == Action::Case { "Caps Lock hotkey pressed" } else { "Hotkey pressed" });
        let fg = fg_at_hotkey as HWND;
        let caret = if fg.is_null() { None } else { target::caret_point(fg) };
        let mut host = WinHost::new(owner, vk, caps_at_hotkey, fg_at_hotkey);
        let mut trace = klf_core::diag::Trace::default();
        let outcome = flow::run_traced(&mut host, &s, &mut trace);
        {
            use klf_core::flow::Host;
            crate::diagnostic::record(s.action, &outcome, trace, host.now_ms(), host.layout_seen.get());
        }
        // the result goes through an internal slot; the message only wakes the UI thread
        // (no pointer in a window message: another program could post a fake one)
        *DONE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Done { outcome, caret, target: fg as usize });
        // SAFETY: posting a message without pointers.
        if unsafe { PostMessageW(owner as HWND, WM_APP_DONE, 0, 0) } == 0 {
            *DONE.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            busy.store(false, Ordering::SeqCst);
        }
    });
    if spawned.is_err() {
        STATE.with(|s| s.busy.store(false, Ordering::SeqCst));
        log("Could not start the worker thread");
    }
}

fn sys_tick() -> u64 {
    // SAFETY: no arguments.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}

/// Looks for an update on a worker thread (public per-user install, setting on, no policy).
fn start_update_check(hwnd: HWND) {
    let s = settings();
    if !crate::update::allowed(s.check_updates) {
        return;
    }
    if UPDATE_BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let owner = hwnd as usize;
    let _ = std::thread::Builder::new().name("update-check".into()).spawn(move || {
        let found = crate::update::check(&s.update_skip_tag, s.update_skip_at);
        UPDATE_BUSY.store(false, Ordering::SeqCst);
        if let Some(r) = found {
            log(&format!("Update available: {}", r.tag));
            *crate::update::OFFER.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(r);
            // SAFETY: posting a message without pointers.
            unsafe { PostMessageW(owner as HWND, WM_APP_UPDATE, 1, 0) };
        }
    });
}

/// Installs the offered update (tray menu click): download + checks + swap on a worker thread.
fn start_update_install(hwnd: HWND) {
    let Some(r) = crate::update::OFFER.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone() else { return };
    if UPDATE_BUSY.swap(true, Ordering::SeqCst) {
        return; // already installing (second click)
    }
    let l = lang();
    bubble::show(text(l, Msg::UpdateInstalling), l.is_rtl(), None, 0, false);
    let owner = hwnd as usize;
    let _ = std::thread::Builder::new().name("update-install".into()).spawn(move || {
        let code = match crate::update::install(&r) {
            Ok(()) => 2,
            Err(crate::update::Failure::Network(e)) => {
                log(&format!("Update {}: network problem, tried again later: {e}", r.tag));
                UPDATE_BUSY.store(false, Ordering::SeqCst);
                4
            }
            Err(crate::update::Failure::Refused(e)) => {
                log(&format!("Update {} FAILED: {e}", r.tag));
                *UPDATE_ERROR.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = r.tag.clone();
                UPDATE_BUSY.store(false, Ordering::SeqCst);
                3
            }
        };
        // SAFETY: posting a message without pointers.
        unsafe { PostMessageW(owner as HWND, WM_APP_UPDATE, code, 0) };
    });
}

fn on_update_message(hwnd: HWND, code: WPARAM) {
    let l = lang();
    match code {
        1 => {
            let tag = crate::update::OFFER.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().map(|r| r.tag.trim_start_matches('v').to_string());
            if let Some(v) = tag {
                bubble::show(&text(l, Msg::UpdateAvailable).replace("{version}", &v), l.is_rtl(), None, 0, true);
            }
        }
        2 => request_exit(hwnd), // the new copy is waiting for us to quit (--update)
        4 => bubble::show(text(l, Msg::UpdateFailed), l.is_rtl(), None, 0, false), // network: offered again later
        _ => {
            // the failed version is not offered again for 7 days (no loop)
            let tag = std::mem::take(&mut *UPDATE_ERROR.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
            *crate::update::OFFER.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            let mut s = user_settings();
            s.update_skip_tag = tag;
            s.update_skip_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            if save_settings(&s) {
                apply_settings(s);
            }
            bubble::show(text(l, Msg::UpdateFailed), l.is_rtl(), None, 0, false);
        }
    }
}

/// Result of the last conversion, taken by the UI thread on WM_APP_DONE (one at a time).
static DONE: std::sync::Mutex<Option<Done>> = std::sync::Mutex::new(None);

thread_local! {
    /// (action, window that was in the foreground when it was pressed)
    static PENDING: Cell<Option<(Action, usize)>> = const { Cell::new(None) };
}

fn on_done(hwnd: HWND) {
    let Some(done) = DONE.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take() else { return };
    crate::indicator::after_conversion();
    STATE.with(|s| s.busy.store(false, Ordering::SeqCst));
    // the kept press stays in PENDING until WM_APP_PENDING takes it
    if PENDING.with(|p| p.get()).is_some() && !STATE.with(|s| s.exit_requested.get()) {
        // SAFETY: posting a message without pointers to our own window.
        unsafe { PostMessageW(hwnd, WM_APP_PENDING, 0, 0) };
    }
    log(&done.outcome.log);
    crate::report::after_conversion(hwnd, done.outcome.msg);
    let s = settings();
    if let Some(msg) = done.outcome.msg {
        if s.show_bubble {
            let l = lang_of(&s);
            // 1.6.0: what was converted ("akuo → שלום"), from memory only, if the user wants it
            let shown = match (&done.outcome.preview, msg == Msg::Converted && s.show_preview) {
                (Some((before, after)), true) => klf_core::bubble::preview_text(before, after, 24),
                _ => text_with_key(l, msg, &s.hotkey_description()),
            };
            bubble::show(&shown, l.is_rtl(), done.caret, done.target, msg == Msg::Converted);
        }
        if s.play_sound && msg == Msg::Converted {
            sys::beep();
        }
    }
    if STATE.with(|s| s.exit_requested.get()) {
        // SAFETY: our window.
        unsafe {
            DestroyWindow(hwnd);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// window procedure

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY if wparam as i32 == HOTKEY_ID => {
            on_hotkey(hwnd, Action::Layout);
            0
        }
        WM_HOTKEY if wparam as i32 == HOTKEY_CASE_ID => {
            on_hotkey(hwnd, Action::Case);
            0
        }
        WM_APP_PENDING => {
            // the action comes from PENDING only (a message posted by someone else does nothing)
            if let Some((a, window)) = PENDING.with(|p| p.take()) {
                if target::foreground() as usize == window {
                    log("Hotkey pressed during the previous conversion: run now (selection only)");
                    start_hotkey(hwnd, a, true);
                } else {
                    log("Hotkey pressed during the previous conversion: window changed, dropped");
                }
            }
            0
        }
        WM_APP_DONE => {
            on_done(hwnd);
            0
        }
        WM_APP_TRAY => {
            // NOTIFYICON_VERSION_4: the event is in LOWORD(lParam)
            match (lparam as u32) & 0xFFFF {
                // version 4 also sends WM_RBUTTONUP / WM_LBUTTONUP: reacting to both would open twice
                WM_CONTEXTMENU => show_menu(hwnd),
                NIN_SELECT | NIN_KEYSELECT => settings_ui::open(),
                // the only notification we show: "Send an anonymous report?" (1.6.1)
                NIN_BALLOONUSERCLICK => crate::report::send_pending(hwnd),
                _ => {}
            }
            0
        }
        WM_COMMAND => {
            match wparam & 0xFFFF {
                CMD_PAUSE => {
                    if hotkey_state() == HotkeyState::Paused {
                        // resuming by hand ends a timed pause too
                        // SAFETY: our window.
                        unsafe { KillTimer(hwnd, TIMER_PAUSE_END) };
                        PAUSE_UNTIL.with(|p| p.borrow_mut().clear());
                    }
                    toggle_pause()
                }
                CMD_PAUSE10 => pause_for_a_while(),
                CMD_REPORT_SEND => crate::report::send_pending(hwnd),
                CMD_TYPING if crate::typing::allowed() => {
                    let mut s = user_settings();
                    s.typing_suggestions = !s.typing_suggestions;
                    if save_settings(&s) {
                        apply_settings(s);
                    }
                }
                CMD_REPORT_PROBLEM => crate::report::open(),
                CMD_REPORT_ALWAYS => crate::report::set_mode(klf_core::report::ALWAYS),
                CMD_REPORT_NEVER => crate::report::set_mode(klf_core::report::NEVER),
                CMD_CHECK_UPDATES => {
                    let mut s = user_settings();
                    s.check_updates = !s.check_updates;
                    if save_settings(&s) {
                        apply_settings(s);
                    }
                }
                CMD_INSTALL_UPDATE if !busy() => start_update_install(hwnd),
                CMD_PREVIEW if !locked().show_preview => {
                    let mut s = user_settings();
                    s.show_preview = !s.show_preview;
                    if save_settings(&s) {
                        apply_settings(s);
                    }
                }
                CMD_RESTORE_CLIP if !busy() => {
                    use klf_core::flow::RestoreResult;
                    let l = lang();
                    let m = match crate::host::restore_parked(hwnd as usize) {
                        Some(RestoreResult::Restored) => Msg::ClipboardRestoredLater,
                        Some(RestoreResult::KeptUserCopy) => Msg::ClipboardNotRestoredNewCopy,
                        Some(RestoreResult::Failed) => Msg::ClipboardNotRestored,
                        None => Msg::ClipboardNotRestoredNewCopy,
                    };
                    log(&format!("Kept clipboard put back on request: {m:?}"));
                    bubble::show(text(l, m), l.is_rtl(), None, 0, m == Msg::ClipboardRestoredLater);
                }
                CMD_RULE_DISABLE | CMD_RULE_SELECTION | CMD_RULE_REMOVE => {
                    use klf_core::settings::RuleMode;
                    let app = MENU_APP.with(|m| m.borrow().clone());
                    let mode = match wparam & 0xFFFF {
                        CMD_RULE_DISABLE => Some(RuleMode::Disabled),
                        CMD_RULE_SELECTION => Some(RuleMode::SelectionOnly),
                        _ => None,
                    };
                    // clicking a ticked rule again removes it
                    let mode = if mode.is_some() && settings().rule_for(&app) == mode { None } else { mode };
                    set_rule(&app, mode);
                }
                CMD_SETTINGS => settings_ui::open(),
                CMD_ABOUT => show_about(null_mut()),
                CMD_EXIT => request_exit(hwnd),
                _ => {}
            }
            0
        }
        WM_APP_SETTINGS => {
            settings_ui::open();
            0
        }
        WM_CLOSE => {
            // "taskkill" without /f (installer): quit cleanly, after a conversion in progress
            log("Close requested (taskkill / installer)");
            request_exit(hwnd);
            0
        }
        WM_TIMER if crate::indicator::on_timer(wparam) => 0,
        WM_TIMER if wparam == TIMER_POLICY => {
            reload_policy();
            crate::host::parked_available(); // a kept clipboard is forgotten after 10 minutes, even unseen
            0
        }
        WM_TIMER if wparam == TIMER_UPDATE => {
            // then once a day, a little later each time (not every PC at the same second)
            let jitter = (sys_tick() % 3_600_000) as u32;
            // SAFETY: our window.
            unsafe { SetCoalescableTimer(hwnd, TIMER_UPDATE, 86_400_000 + jitter, None, 60_000) };
            start_update_check(hwnd);
            0
        }
        WM_APP_UPDATE => {
            on_update_message(hwnd, wparam);
            0
        }
        crate::typing::WM_APP_TYPING => {
            crate::typing::on_message(wparam);
            0
        }
        crate::report::WM_APP_REPORT => {
            crate::report::on_result(wparam);
            0
        }
        WM_APP_WELCOME => {
            crate::welcome::maybe_show(hwnd);
            0
        }
        WM_TIMER if wparam == TIMER_PAUSE_END => {
            end_timed_pause(hwnd);
            0
        }
        WM_TIMER if wparam == TIMER_TRAY => {
            ensure_tray();
            0
        }
        WM_TIMER if wparam == TIMER_RETRY => {
            if STATE.with(|s| s.enabled.get()) {
                if !STATE.with(|s| s.registered.get()) {
                    if register_hotkey() {
                        log("Hotkey registered on retry");
                    }
                } else if !case_hotkey_ok() {
                    // F10 works: only the Caps Lock hotkey is tried again (F10 left as it is)
                    register_case_hotkey();
                    if case_hotkey_ok() {
                        // SAFETY: our window.
                        unsafe { KillTimer(hwnd, TIMER_RETRY) };
                    }
                }
                settings_ui::refresh_status();
            }
            0
        }
        WM_DESTROY => {
            unregister_hotkey();
            remove_tray();
            // SAFETY: ends the message loop.
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => {
            if crate::indicator::shell_message(msg, wparam) {
                return 0;
            }
            // explorer.exe restarted: the tray icon must be added again
            let tc = STATE.with(|s| s.taskbar_created.get());
            if tc != 0 && msg == tc {
                STATE.with(|s| s.tray_retries.set(0));
                ensure_tray();
                return 0;
            }
            // SAFETY: default handling.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

/// Hidden top-level window (never shown): a message-only window would not receive the
/// TaskbarCreated broadcast.
pub fn create_main_window() -> Option<HWND> {
    let class = wide(MAIN_CLASS);
    let title = wide("Keyboard Layout Fixer");
    // SAFETY: class registered once; strings outlive the calls.
    unsafe {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: instance(),
            lpszClassName: class.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassExW(&wc);
        let hwnd = CreateWindowExW(WS_EX_TOOLWINDOW, class.as_ptr(), title.as_ptr(), WS_POPUP, 0, 0, 0, 0, null_mut(), null_mut(), instance(), null());
        if hwnd.is_null() {
            return None;
        }
        let tc = wide("TaskbarCreated");
        let id = RegisterWindowMessageW(tc.as_ptr());
        STATE.with(|s| {
            s.hwnd.set(hwnd);
            s.taskbar_created.set(id);
        });
        watch_foreground();
        note_foreground(GetForegroundWindow());
        SetCoalescableTimer(hwnd, TIMER_POLICY, 60_000, None, 5_000);
        // update (public per-user install only): first look 2 minutes after the start
        crate::update::cleanup();
        SetCoalescableTimer(hwnd, TIMER_UPDATE, 120_000, None, 30_000);
        Some(hwnd)
    }
}

pub fn run_message_loop() {
    // SAFETY: standard loop; IsDialogMessage gives Tab navigation in the settings window.
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            let dlg = settings_ui::window();
            // Ctrl+Tab changes the tab of the settings window (before IsDialogMessage, which
            // would take it as Tab)
            if settings_ui::handle_ctrl_tab(&msg) {
                continue;
            }
            if !dlg.is_null() && IsDialogMessageW(dlg, &msg) != 0 {
                continue;
            }
            let rep = crate::report::window();
            if !rep.is_null() && IsDialogMessageW(rep, &msg) != 0 {
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
