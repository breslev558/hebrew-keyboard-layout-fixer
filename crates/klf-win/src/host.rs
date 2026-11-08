//! `klf_core::flow::Host` for the real Windows (used on the worker thread).

use crate::platform::{clipboard, console, input, layouts, target};
use klf_core::layouts::KeyMap;
use klf_core::flow::{Capture, ClipState, Host, PublishResult, RestoreResult, SaveResult, StillOurs, TargetInfo};
use std::cell::{Cell, RefCell};
use windows_sys::Win32::Foundation::HWND;

pub struct WinHost {
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
    pub fn new(owner: usize, hotkey_vk: u16, caps_at_hotkey: (Option<bool>, Option<u32>), start_fg: usize) -> Self {
        Self {
            owner: owner as HWND,
            hotkey_vk,
            snapshot: None,
            target: Cell::new(std::ptr::null_mut()),
            focus: Cell::new(std::ptr::null_mut()),
            pids: RefCell::new(Vec::new()),
            caret: Cell::new(None),
            moved: Cell::new({
                // the first call clears the "pressed since last call" bits left from before F10
                input::user_moving(hotkey_vk);
                false
            }),
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
    let r = clipboard::restore(owner as HWND, &snap, &st);
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
        let same_focus = target::focus_of(self.target.get()) == self.focus.get();
        // the caret, when remembered, must not have moved (other field of the same window)
        let same_caret = match self.caret.get() {
            Some(before) => target::caret_of(self.target.get()) == before,
            None => true,
        };
        same_focus && same_caret
    }
    fn user_moving(&self) -> bool {
        if input::user_moving(self.hotkey_vk) {
            self.moved.set(true);
        }
        self.moved.get()
    }
    fn remember_caret(&self) {
        self.caret.set(Some(target::caret_of(self.target.get())));
    }
    fn target_info(&self, hwnd: usize) -> TargetInfo {
        self.target.set(h(hwnd));
        self.focus.set(target::focus_of(h(hwnd)));
        *self.pids.borrow_mut() = target::pids_of(h(hwnd));
        target::info(h(hwnd))
    }
    fn wait_keys_released(&self) -> bool {
        input::wait_keys_released(self.hotkey_vk, 1500)
    }
    fn pause(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
    /// Sleeps while watching the mouse buttons, Tab and Enter every 5 ms (audit B3).
    fn sleep(&self, ms: u64) {
        let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
        loop {
            if input::user_moving(self.hotkey_vk) {
                self.moved.set(true);
            }
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
        // one press ONLY if it was on at F9, is still on now, and Caps Lock was not pressed in
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
}
