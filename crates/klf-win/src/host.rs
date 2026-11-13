//! `klf_core::flow::Host` for the real Windows (used on the worker thread).

use crate::platform::{clipboard, console, input, layouts, target};
use klf_core::layouts::KeyMap;
use klf_core::flow::{Capture, ClipState, Host, PublishResult, RestoreResult, SaveResult, StillOurs, TargetInfo};
use std::cell::{Cell, RefCell};
use windows_sys::Win32::Foundation::HWND;

pub struct WinHost {
    /// 1.8.7: take an autocompletion suggestion away before copying (decided on the UI thread)
    pub drop_suggestion: bool,
    owner: HWND,
    hotkey_vk: u16,
    snapshot: Option<clipboard::Snapshot>,
    target: Cell<HWND>,
    focus: Cell<HWND>,
    /// processes allowed to own the clipboard after our Ctrl+C
    pids: RefCell<Vec<u32>>,
    /// caret remembered after the last Ctrl+C (None: not remembered or no caret)
    caret: Cell<Option<Option<target::Caret>>>,
    /// a mouse button, Tab or Enter was seen during the operation (audit B3)
    moved: Cell<bool>,
    /// 1.9.4: user keys and clicks counted by the hooks at F10 (None: hooks not working); a key or
    /// click is "during the fix" only if these counters moved, never from a flag set before F10
    activity_at_start: Cell<Option<(u32, u32)>>,
    /// 1.9.4: event time of the hotkey's press (keyboard hook), to date the clicks and keys
    hotkey_at: Option<u32>,
    /// Caps Lock at the hotkey (UI thread): state read on the target (None = unknown) and
    /// number of Caps Lock presses counted so far (None = not followed)
    caps_at_hotkey: (Option<bool>, Option<u32>),
    /// keyboard layout of the target before `switch_language` (to put it back)
    lang_before: Cell<Option<usize>>,
    console: Option<console::Session>,
    /// diagnostic clock
    started: std::time::Instant,
    /// layout of the target at the press, and the Latin layout chosen for the conversion
    pub layout_seen: Cell<Option<u32>>,
    latin_hkl: Cell<Option<u32>>,
    /// window in the foreground at the hotkey (UI thread, audit A03)
    start_fg: usize,
}

impl WinHost {
    /// `owner`: our hidden window (clipboard owner).
    /// 1.9.3: Chrome/Edge/Electron moved their focus to (or from) the child window they create for
    /// accessibility, inside the target window: the same field (see policy::same_chromium_field).
    /// The new focus is then the one checked from now on. Only window classes are logged.
    fn chromium_focus_swap(&self, now: HWND) -> bool {
        let (before, top) = (self.focus.get(), self.target.get());
        if before.is_null() || now.is_null() {
            return false;
        }
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetParent, IsChild};
        // SAFETY: plain window handles (a closed window only gives false / null).
        let inside = |w: HWND| w == top || unsafe { IsChild(top, w) } != 0;
        // one is the direct parent of the other (the page window inside its browser window)
        // SAFETY: as above.
        let parent_child = unsafe { GetParent(now) == before || GetParent(before) == now };
        let (cb, cn) = (crate::platform::sys::class_name(before), crate::platform::sys::class_name(now));
        let same = inside(before) && inside(now) && parent_child && klf_core::policy::same_chromium_field(&cb, &cn);
        crate::util::log(&format!("Focus moved {cb} -> {cn}: {}", if same { "same page, accepted" } else { "other field" }));
        if same {
            self.focus.set(now);
            // the caret, if remembered, belongs to the window that had the focus: remembered again
            if self.caret.get().is_some() {
                self.remember_caret();
            }
        }
        same
    }
    /// 1.9.4: a click or key of the user DURING the fix (after F10), three ways: held down now,
    /// counted by the hooks since the press, or the "pressed since the last call" flag of Windows
    /// (always read too: a safety net when a hook misses an event; all its flags are cleared at
    /// the press, so it no longer reports a click or Enter from BEFORE F10, the cancelled F10s of
    /// 12/11). The first cause is written to the journal.
    fn watch_user(&self) {
        if self.moved.get() {
            return;
        }
        let flag = input::pressed_since_last_call(self.hotkey_vk);
        // dated by the hooks (any click or key at or after the press, whatever the order of arrival),
        // else counted since the press (counters read on the UI thread at WM_HOTKEY)
        // (both when both exist: a click injected with its own time, remote control, tablet, is
        // still counted)
        let by_count = self.activity_at_start.get().and_then(|start| {
            let now = input::user_activity();
            if now.1 != start.1 {
                Some("mouse click")
            } else if now.0 != start.0 {
                Some("key pressed")
            } else {
                None
            }
        });
        let counted = self.hotkey_at.and_then(input::input_since).or(by_count);
        let why = input::held_now(self.hotkey_vk).or(counted).or(flag);
        if let Some(why) = why {
            self.moved.set(true);
            crate::util::log(&format!("User input during the fix: {why}"));
        }
    }
    pub fn new(owner: usize, hotkey_vk: u16, caps_at_hotkey: (Option<bool>, Option<u32>), start_fg: usize, activity_at_hotkey: Option<(u32, u32)>, hotkey_at: Option<u32>) -> Self {
        Self {
            drop_suggestion: false,
            owner: owner as HWND,
            hotkey_vk,
            snapshot: None,
            target: Cell::new(std::ptr::null_mut()),
            focus: Cell::new(std::ptr::null_mut()),
            pids: RefCell::new(Vec::new()),
            caret: Cell::new(None),
            // (the "pressed since last call" flags were all cleared at the press, app.rs, 1.9.4)
            moved: Cell::new(false),
            activity_at_start: Cell::new(activity_at_hotkey),
            hotkey_at,
            lang_before: Cell::new(None),
            caps_at_hotkey,
            console: None,
            started: std::time::Instant::now(),
            layout_seen: Cell::new(None),
            latin_hkl: Cell::new(None),
            start_fg,
        }
    }
}

/// The user's clipboard that could not be put back (audit A08): memory only, 10 minutes,
/// put back only on request from the tray menu, and only if our text is still in the clipboard.
static PARKED: std::sync::Mutex<Option<(clipboard::Snapshot, ClipState, std::time::Instant)>> = std::sync::Mutex::new(None);
const PARK_SECONDS: u64 = 600;

/// A kept clipboard can be put back (shown in the tray menu).
pub fn parked_available() -> bool {
    let mut p = PARKED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if p.as_ref().is_some_and(|(_, _, at)| at.elapsed().as_secs() >= PARK_SECONDS) {
        *p = None; // expired: forgotten
    }
    p.is_some()
}

/// Puts the kept clipboard back (tray menu). None: nothing kept (or expired).
pub fn restore_parked(owner: usize) -> Option<RestoreResult> {
    if !parked_available() {
        return None;
    }
    let mut p = PARKED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let (snap, st, at) = p.take()?;
    let r = clipboard::restore_on_request(owner as HWND, &snap);
    if r == RestoreResult::Failed {
        *p = Some((snap, st, at)); // still busy: kept for another try
    }
    Some(r)
}

fn h(hwnd: usize) -> HWND {
    hwnd as HWND
}

impl Host for WinHost {
    fn foreground(&self) -> usize {
        target::foreground() as usize
    }
    fn initial_target(&self) -> usize {
        if self.start_fg != 0 { self.start_fg } else { self.foreground() }
    }
    fn focus_unchanged(&self) -> bool {
        // unknown -> known (or the reverse) is a change; unknown both times stays "unchanged",
        // or programs without a focused control would lose F10
        let now = target::focus_of(self.target.get());
        let same_focus = now == self.focus.get() || self.chromium_focus_swap(now);
        // the caret, when remembered, must not have moved (other field of the same window)
        let same_caret = match self.caret.get() {
            Some(before) => target::caret_of(self.target.get()) == before,
            None => true,
        };
        // 1.9.3: which of the two changed is written to the journal (window classes only)
        if same_focus && !same_caret {
            crate::util::log("Text caret moved (other field?)");
        }
        same_focus && same_caret
    }
    fn wait_keys_released(&self, hotkey_too: bool) -> bool {
        input::wait_keys_released(if hotkey_too { self.hotkey_vk } else { 0 }, 1500)
    }
    fn user_moving(&self) -> bool {
        self.watch_user();
        self.moved.get()
    }
    fn remember_caret(&self) {
        self.caret.set(Some(target::caret_of(self.target.get())));
    }
    fn target_info(&self, hwnd: usize) -> TargetInfo {
        self.target.set(h(hwnd));
        self.focus.set(target::focus_of(h(hwnd)));
        *self.pids.borrow_mut() = target::pids_of(h(hwnd));
        let mut info = target::info(h(hwnd));
        // 1.9.2: in a web page or an Electron app the selection is asked to UI Automation (60 ms at
        // most): nothing selected = no Ctrl+C to wait for, the line is taken at once
        if matches!(info.selection, None | Some(klf_core::policy::Selection::Unknown)) && klf_core::policy::copies_nothing_without_selection(&info.focus_class) {
            info.selection = match crate::platform::uia::selection_empty(60) {
                Some(true) => Some(klf_core::policy::Selection::Empty),
                Some(false) => Some(klf_core::policy::Selection::NonEmpty),
                None => info.selection,
            };
        }
        info
    }
    fn pause(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
    /// Sleeps while watching the mouse buttons, Tab and Enter every 5 ms (audit B3).
    fn sleep(&self, ms: u64) {
        let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        loop {
            self.watch_user();
            let now = std::time::Instant::now();
            if now >= end {
                break;
            }
            std::thread::sleep((end - now).min(std::time::Duration::from_millis(5)));
        }
    }
    fn clipboard_save_and_clear(&mut self) -> (SaveResult, u32) {
        // a new conversion: an older kept copy no longer makes sense
        *PARKED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        let (r, seq, snap) = clipboard::save_and_clear(self.owner);
        self.snapshot = snap;
        (r, seq)
    }
    fn clipboard_seq(&self) -> u32 {
        clipboard::sequence()
    }
    fn clipboard_capture(&self) -> Capture {
        clipboard::capture(self.owner, &self.pids.borrow())
    }
    fn clipboard_publish(&mut self, text: &str, st: &ClipState) -> PublishResult {
        clipboard::publish(self.owner, text, st)
    }
    fn clipboard_still_ours(&self, st: &ClipState) -> StillOurs {
        clipboard::is_still_ours(self.owner, st)
    }
    fn clipboard_restore(&mut self, st: &ClipState) -> RestoreResult {
        // the saved copy is dropped only once the question is settled (audit B2)
        let r = match &self.snapshot {
            Some(s) => clipboard::restore(self.owner, s, st),
            None => RestoreResult::Failed,
        };
        if r != RestoreResult::Failed {
            self.snapshot = None;
        }
        r
    }
    fn clipboard_forget(&mut self) {
        self.snapshot = None;
    }
    fn clipboard_park(&mut self, st: &ClipState) {
        if let Some(snap) = self.snapshot.take() {
            if snap.is_empty() {
                return; // 1.8.8: an empty clipboard is not kept (a click would only empty the current one)
            }
            if snap.secret {
                // a password manager's copy is never kept for later (it would come back after the
                // manager cleared it)
                crate::util::log("Clipboard marked as secret by its program: not kept for later");
                return;
            }
            *PARKED.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((snap, st.clone(), std::time::Instant::now()));
        }
    }
    fn send_copy(&self) -> bool {
        input::copy()
    }
    fn send_select_to_line_start(&self) -> bool {
        input::select_to_line_start()
    }
    fn send_paste(&self) -> bool {
        input::paste()
    }
    fn wrong_words_only(&self, text: &str, map: &klf_core::layouts::KeyMap, replace_caps: bool) -> Option<String> {
        crate::typing::wrong_words_only(text, map, replace_caps)
    }
    fn pasted(&self, before: &str, after: &str, selected: bool) {
        crate::app::remember_paste(self.target.get() as usize, self.focus.get() as usize, before, after, selected);
    }
    fn send_select_back(&self, n: usize) -> bool {
        input::select_back(n)
    }
    fn switch_language(&self, hwnd: usize, to_hebrew: bool) {
        self.lang_before.set(target::current_layout(h(hwnd)));
        // the exact layouts of the conversion: standard Hebrew, or the Latin layout used
        let wanted = if to_hebrew { layouts::hebrew_layout() } else { self.latin_hkl.get() };
        if !wanted.is_some_and(|k| layouts::request(h(hwnd), k)) {
            target::switch_language(h(hwnd), to_hebrew)
        }
    }
    fn modifiers_down(&self) -> bool {
        input::modifiers_down()
    }
    fn before_copy(&self) {
        // checked again now (the suggestion may have changed); Delete with no modifier held
        if self.drop_suggestion && crate::app::autocomplete_suffix(self.target.get()) && input::delete_key() {
            crate::util::log("Autocompletion suggestion removed before converting");
        }
    }
    fn send_extend_up(&self) -> bool {
        input::select_up_to_line_start()
    }
    fn send_shrink_down(&self) -> bool {
        input::select_down()
    }
    fn send_collapse(&self) -> bool {
        input::collapse_right()
    }
    fn user_activity(&self) -> Option<(u32, u32)> {
        input::activity_watched().then(input::user_activity)
    }
    fn send_select_last(&self, n: usize) -> bool {
        input::select_last(n)
    }
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
    fn key_map(&mut self, hwnd: usize) -> Result<KeyMap, String> {
        let active = target::current_layout(h(hwnd)).map(|k| k as u32);
        self.layout_seen.set(active);
        match layouts::choose(active) {
            layouts::Choice::Map(m, latin) => {
                self.latin_hkl.set(latin);
                Ok(m)
            }
            layouts::Choice::Unsupported(name) => Err(name),
        }
    }
    fn caps_lock_off(&self) {
        // one press ONLY if it was on at the hotkey, is still on now, and Caps Lock was not pressed in
        // between (seen by the hook): otherwise it is left as it is, never a press "to be sure"
        // (audit 1.3.7, P2; rule tested in klf_core::policy).
        let (was, presses_before) = self.caps_at_hotkey;
        if was != Some(true) {
            if was.is_none() {
                crate::util::log("Caps Lock state unknown at the hotkey: left as it is");
            }
            return;
        }
        let now = input::caps_lock_on_now(self.target.get());
        let presses_now = input::caps_presses();
        if klf_core::policy::caps_lock_press_needed(was, now, presses_before, presses_now) {
            if !input::press_caps_lock() {
                crate::util::log("Caps Lock could not be switched off (keys blocked)");
            }
        } else {
            crate::util::log("Caps Lock left as it is (state not certain or changed by the user)");
        }
    }
    fn restore_language(&self, hwnd: usize) {
        if let Some(hkl) = self.lang_before.take() {
            target::request_layout(h(hwnd), hkl);
        }
    }
    fn console_attach(&mut self, hwnd: usize) -> bool {
        self.console = console::Session::attach(h(hwnd));
        self.console.is_some()
    }
    fn console_detach(&mut self) {
        self.console = None; // Drop -> FreeConsole
    }
    fn console_processes(&self) -> Vec<String> {
        self.console.as_ref().map(|c| c.processes()).unwrap_or_default()
    }
    fn console_input_mode(&self) -> Option<u32> {
        self.console.as_ref()?.input_mode()
    }
    fn console_truetype(&self) -> Option<bool> {
        self.console.as_ref()?.truetype()
    }
    fn console_selection_active(&self) -> bool {
        self.console.as_ref().map(|c| c.selection_active()).unwrap_or(true)
    }
    fn console_cursor(&self) -> Option<(i32, i32, i32)> {
        self.console.as_ref()?.cursor()
    }
    fn console_read(&self, x: i32, y: i32, count: usize) -> Option<String> {
        self.console.as_ref()?.read(x, y, count)
    }
    fn send_home(&self) -> bool {
        input::home()
    }
    fn send_end(&self) -> bool {
        input::end()
    }
    fn send_right(&self, n: usize) -> bool {
        input::right(n)
    }
    fn send_replace(&self, backspaces: usize, text: &str) -> bool {
        input::replace(backspaces, text)
    }
    fn terminal_typed(&self) -> Option<(String, u64)> {
        let Some((typed, generation)) = input::terminal_snapshot() else {
            crate::util::log(&format!("Terminal: {}", input::terminal_why_not()));
            return None;
        };
        match typed {
            klf_core::terminal::Typed::Text(t) => Some((t, generation)),
            klf_core::terminal::Typed::Keys(mut keys) => {
                // (checked against the screen anyway: a wrong guess is refused there)
                let caps = input::caps_lock_guess(self.target.get());
                let text = caps.and_then(|caps| layouts::keys_to_text(&keys, caps));
                if text.is_none() {
                    crate::util::log(if caps.is_none() { "Terminal: Caps Lock state unknown" } else { "Terminal: a key gives no single character (dead key?)" });
                }
                for k in keys.iter_mut() {
                    // SAFETY: plain value through a valid &mut (volatile: not optimized away).
                    unsafe { std::ptr::write_volatile(k, klf_core::terminal::KeyStroke::default()) };
                }
                text.map(|t| (t, generation))
            }
        }
    }
    fn terminal_generation(&self) -> u64 {
        input::terminal_generation()
    }
    fn terminal_before_caret(&self, n: usize) -> Result<String, &'static str> {
        let mut pid = 0u32;
        // SAFETY: the pid is written.
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(self.target.get(), &mut pid) };
        crate::platform::uia::terminal_before_caret(n, pid, 150)
    }
    fn terminal_adopt(&self, text: &str) {
        input::terminal_adopt(text);
    }
    fn terminal_forget(&self) {
        input::terminal_forget();
    }
}
