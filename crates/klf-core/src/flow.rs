//! What F10 does, step by step, written against a `Host` trait so that the whole sequence
//! (checks, give-ups, clipboard restore) is tested on Linux with a simulated Windows.

use crate::convert::{contains_hebrew, convert_with, swap_case, us_map};
use crate::diag::{Step, Trace};
use crate::layouts::KeyMap;
use crate::i18n::Msg;
use crate::policy::{self, ConsoleKind, ConsoleReader, Selection};
use crate::settings::{Action, AppSettings, MixedTextMode, RuleMode};

/// What we know about the window that had the focus when F10 was pressed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TargetInfo {
    pub window_class: String,
    pub exe_name: String,
    /// Some(true) if the program runs as administrator; None if it could not be checked.
    pub elevated: Option<bool>,
    pub focus_class: String,
    pub password_field: bool,
    pub selection: Option<Selection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveResult {
    /// Everything saved AND the clipboard emptied, in the same opening (nobody can copy in between).
    Complete,
    Busy,
    TooBig,
    /// A format the program can save could not be read: the clipboard is left untouched.
    ReadError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreResult {
    Restored,
    KeptUserCopy,
    Failed,
}

/// Right before Ctrl+V: is our converted text still in the clipboard?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StillOurs {
    Yes,
    /// someone copied something new: keep it
    No,
    /// the clipboard could not be opened (Win+V service, a clipboard manager reading it):
    /// nothing pasted, the user's clipboard is put back as usual
    Busy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishResult {
    /// Our text is in the clipboard; sequence number read right after closing it.
    Done(u32),
    /// Someone copied something new since the capture: nothing replaced.
    NewCopy,
    /// Failed before anything changed in the clipboard.
    Failed,
    /// Failed AFTER our EmptyClipboard: the clipboard holds a partial state of ours.
    FailedAfterEmpty,
}

/// What the clipboard held when the application's copy was read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capture {
    pub text: Option<String>,
    pub seq: u32,
    /// GetClipboardOwner() as a number (0 = no owner window).
    pub owner: usize,
    /// The owner window belongs to another process than the target window (and its child
    /// windows): someone else copied (audit A1). False when it cannot be told (no owner).
    pub foreign_owner: bool,
    /// The text is larger than the reading limit (audit B10): not read.
    pub too_big: bool,
    /// The clipboard could not be opened to read it.
    pub busy: bool,
    /// The formats listed by EnumClipboardFormats, in order.
    pub formats: Vec<u32>,
}

/// What the clipboard looks like now (read while it is open, so it cannot change meanwhile).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipObs {
    pub seq: u32,
    /// GetClipboardOwner() is our window (the last EmptyClipboard was ours).
    pub owner_is_us: bool,
    /// GetClipboardOwner() as a number (0 = no owner window).
    pub owner: usize,
    /// None also when the text is too big to read: then it is never "ours".
    pub text: Option<String>,
    /// The formats listed by EnumClipboardFormats, in order.
    pub formats: Vec<u32>,
    /// A format other than text (CF_UNICODETEXT and the CF_TEXT/CF_OEMTEXT/CF_LOCALE Windows
    /// derives from it) and our "not in history" marks.
    pub foreign_formats: bool,
}

/// Where the operation stands, to know what the clipboard should contain (provenance).
/// A sequence number proves nothing changed. Office and the Win+V service render delayed
/// formats later and move the number WITHOUT changing the owner window: the fallback is
/// "same text AND same owner window" (audit B1), never equal text alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipState {
    /// sequence number right after our EmptyClipboard, read after closing: for the log only,
    /// never trusted to decide (audit 1.3.2, R2)
    pub seq_clear: u32,
    /// sequence number when the application's copy was read (text or not)
    pub seq_cap: Option<u32>,
    pub captured: Option<String>,
    /// 1.8.6: the text came from the user's own selection (not Shift+Home)
    pub user_selected: bool,
    /// owner window of the application's copy (0 = none)
    pub owner_cap: Option<usize>,
    /// formats of the application's copy
    pub formats_cap: Option<Vec<u32>>,
    /// sequence number right after our converted text was put in
    pub seq_set: Option<u32>,
    pub converted: Option<String>,
    /// someone copied something new before we replaced it: nothing pasted, nothing restored
    pub new_copy_seen: bool,
}

/// The clipboard still holds the application's copy made for our Ctrl+C: same sequence
/// number (read INSIDE the opening that read the copy), or the same (non-null) owner window
/// with the same text (Office renders delayed formats later and moves the number). Without
/// any text, the list of formats must also be the same (audit 1.3.2, R3). Known limit: a new
/// copy of the SAME text from the SAME window within those milliseconds is not told apart
///.
fn is_app_copy(st: &ClipState, now: &ClipObs) -> bool {
    let Some(seq) = st.seq_cap else { return false };
    if now.seq == seq {
        return true;
    }
    let same_owner = matches!(st.owner_cap, Some(o) if o != 0 && o == now.owner);
    match (&st.captured, &now.text) {
        (Some(c), Some(t)) => same_owner && c == t,
        (None, None) => same_owner && st.formats_cap.as_ref() == Some(&now.formats),
        _ => false,
    }
}

/// May our converted text replace what is in the clipboard? Only the application's copy made
/// for our Ctrl+C may be replaced.
pub fn may_publish(st: &ClipState, now: &ClipObs) -> bool {
    is_app_copy(st, now)
}

/// Is our converted text still what Ctrl+V will paste? Checked right before Ctrl+V (audit A2).
/// The owner window decides, never a sequence number read after closing (audit 1.3.2, R2).
pub fn still_ours(st: &ClipState, now: &ClipObs) -> bool {
    st.seq_set.is_some() && now.owner_is_us && st.converted.is_some() && now.text == st.converted && !now.foreign_formats
}

/// May the user's saved clipboard be put back? Only if what is there now is ours (or the
/// application's copy made for us): anything else is a new copy of the user, which is kept.
pub fn may_restore(st: &ClipState, now: &ClipObs) -> bool {
    if st.new_copy_seen {
        return false;
    }
    // the last EmptyClipboard was ours: our own state, even half-written by a publish or a
    // restore that failed (audit A3, 1.3.2 R1). A real copy always empties with another owner.
    if now.owner_is_us {
        return true;
    }
    // we published and someone else emptied it since: their copy
    if st.seq_set.is_some() {
        return false;
    }
    // the application's copy made for us, or (nothing captured) someone else's
    st.seq_cap.is_some() && is_app_copy(st, now)
}

pub trait Host {
    fn foreground(&self) -> usize;
    /// 1.8.5, F10 "only the wrong words": the text with only its words typed on the wrong layout
    /// converted, when the word lists are there (public version); None = convert everything.
    fn wrong_words_only(&self, _text: &str, _map: &KeyMap, _replace_caps: bool) -> Option<String> {
        None
    }
    /// 1.8.5: Ctrl+V was just sent with `after` replacing `before` (the clipboard path): a quick
    /// next F10 may toggle it back without the clipboard.
    fn pasted(&self, _before: &str, _after: &str, _selected: bool) {}
    /// 1.8.6: the text just pasted is selected again (Shift+Left `n` times).
    fn send_select_back(&self, _n: usize) -> bool {
        false
    }
    /// 1.8.5: Shift+Up then Shift+Home (the selection grows by one DISPLAYED line, in a field that
    /// wraps a long line); false = not sent / not supported.
    fn send_extend_up(&self) -> bool {
        false
    }
    /// 1.8.5: Shift+Down (undoes one `send_extend_up`).
    fn send_shrink_down(&self) -> bool {
        false
    }
    /// 1.8.5: the selection is collapsed back to the caret (Right arrow), after an aborted
    /// paragraph extension.
    fn send_collapse(&self) -> bool {
        false
    }
    /// 1.8.5: keys and clicks of the user counted so far (None: not counted); any change during
    /// the paragraph extension stops it.
    fn user_activity(&self) -> Option<(u32, u32)> {
        None
    }
    /// The window in the foreground when the hotkey was pressed (read on the UI thread before
    /// the work starts, audit A03); a different window later stops the conversion.
    fn initial_target(&self) -> usize {
        self.foreground()
    }
    /// The focused control of the target is still the one seen by `target_info`.
    fn focus_unchanged(&self) -> bool;
    /// A mouse button, Tab or Enter was pressed at any moment since F10 (sampled during every
    /// wait): the user may be moving to another field of the same window (browsers: all
    /// fields share one window handle).
    fn user_moving(&self) -> bool;
    /// Remembers where the text caret is (after the last Ctrl+C); `focus_unchanged` then also
    /// checks it did not move (other field of the same window).
    fn remember_caret(&self);
    fn target_info(&self, hwnd: usize) -> TargetInfo;
    /// Waits (max 1.5 s) until the hotkey and Ctrl/Alt/Shift/Win are released.
    fn wait_keys_released(&self) -> bool;
    fn sleep(&self, ms: u64);
    /// Plain wait, without watching the user (after Ctrl+V: the destination is settled).
    fn pause(&self, ms: u64) {
        self.sleep(ms)
    }
    /// Saves the whole clipboard then empties it, in ONE opening. Returns the sequence number
    /// read right after closing.
    fn clipboard_save_and_clear(&mut self) -> (SaveResult, u32);
    fn clipboard_seq(&self) -> u32;
    /// Text, sequence number and owner of the clipboard, read in one opening.
    fn clipboard_capture(&self) -> Capture;
    /// Puts `text` in the clipboard if `may_publish` allows it (checked in the same opening).
    fn clipboard_publish(&mut self, text: &str, st: &ClipState) -> PublishResult;
    /// `still_ours` checked in one opening, right before Ctrl+V.
    fn clipboard_still_ours(&self, st: &ClipState) -> StillOurs;
    /// Puts the saved clipboard back if `may_restore` allows it (checked in the same opening).
    /// The saved copy is kept after `Failed` (a retry is possible).
    fn clipboard_restore(&mut self, st: &ClipState) -> RestoreResult;
    /// Drops the saved clipboard without putting it back (the user copied something new).
    fn clipboard_forget(&mut self);
    /// The restore failed after every retry: the saved copy is kept a few minutes IN MEMORY
    /// only, so that the user can put it back later from the tray menu (audit A08).
    fn clipboard_park(&mut self, _st: &ClipState) {}
    fn send_copy(&self) -> bool;
    fn send_select_to_line_start(&self) -> bool;
    fn send_paste(&self) -> bool;
    fn switch_language(&self, hwnd: usize, to_hebrew: bool);
    /// Puts back the keyboard language the target had before `switch_language`.
    fn restore_language(&self, hwnd: usize);
    /// Switches Caps Lock off if it is on (after an F9 fix).
    fn caps_lock_off(&self);
    /// Monotonic clock in milliseconds (diagnostic times only, never decides anything).
    fn now_ms(&self) -> u64 {
        0
    }
    /// The key map for the layout active in `hwnd`: the fixed US table, or French AZERTY built
    /// from Windows; Err(name of the layout) when the active layout is not supported. The host
    /// remembers the Latin layout chosen, for `switch_language`.
    fn key_map(&mut self, _hwnd: usize) -> Result<KeyMap, String> {
        Ok(us_map())
    }
    /// Shift, Ctrl or Alt PHYSICALLY held now ("last word": Right would extend the selection).
    fn modifiers_down(&self) -> bool {
        false
    }
    /// "Last word": Right (collapses the Shift+Home selection to the caret), then Shift+Left x
    /// `n`, in one SendInput.
    fn send_select_last(&self, _n: usize) -> bool {
        false
    }

    // --- classic console (conhost) ---
    fn console_attach(&mut self, hwnd: usize) -> bool;
    fn console_detach(&mut self);
    fn console_processes(&self) -> Vec<String>;
    fn console_input_mode(&self) -> Option<u32>;
    fn console_selection_active(&self) -> bool;
    /// Cursor position (x, y) and buffer width.
    fn console_cursor(&self) -> Option<(i32, i32, i32)>;
    /// Reads `count` cells starting at (x, y), wrapping lines.
    fn console_read(&self, x: i32, y: i32, count: usize) -> Option<String>;
    fn send_home(&self) -> bool;
    fn send_end(&self) -> bool;
    /// `n` x Right arrow (puts the console cursor back where it was).
    fn send_right(&self, n: usize) -> bool;
    /// ONE SendInput: `backspaces` x Backspace then `text` as Unicode characters.
    fn send_replace(&self, backspaces: usize, text: &str) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Message for the bubble (None = say nothing).
    pub msg: Option<Msg>,
    /// Line for debug.log (never the text itself).
    pub log: String,
    /// 1.6.0: what was converted (before, after), for the bubble only: kept in memory, never
    /// written to a log or a file. Only set for a real conversion (never for a refused field).
    pub preview: Option<(String, String)>,
}

impl Outcome {
    fn say(msg: Msg, log: impl Into<String>) -> Self {
        Self { msg: Some(msg), log: log.into(), preview: None }
    }
    fn quiet(log: impl Into<String>) -> Self {
        Self { msg: None, log: log.into(), preview: None }
    }
}

pub const COPY_WAIT_MS: u64 = 1500;
const RESTORE_RETRIES: usize = 3;
const NO_TEXT_LOG: &str = "Clipboard changed but contains no text (image/files?)";
const POLL_MS: u64 = 20;
/// 1.5.1: the copy is watched every 5 ms (reading the sequence
/// number costs nothing), then it must stay unchanged for SETTLE_MS (cap SETTLE_MAX_MS) instead
/// of a fixed 100 ms: an application still writing its formats moves the number.
const COPY_POLL_MS: u64 = 5;
const SETTLE_MS: u64 = 15;
const SETTLE_MAX_MS: u64 = 100;
/// After the hotkey and modifiers are released: the key-up reaches the application first (the
/// SendInput events are queued after it; 20 ms margin for a slow application, 50 before 1.5.1).
const AFTER_RELEASE_MS: u64 = 20;
pub const CONSOLE_MAX_CHARS: usize = 300;

/// 1.8.3: Latin text all in capitals (2 letters or more, none small), such as "AKUO": typed with
/// Caps Lock on. When F10 turns it into Hebrew, Caps Lock is switched off too (it is on, or the
/// next Hebrew words would come out as Latin capitals again).
pub fn typed_with_caps_lock(text: &str) -> bool {
    let latin: Vec<char> = text.chars().filter(|c| c.is_alphabetic() && !crate::layouts::is_hebrew_letter(*c)).collect();
    latin.len() >= 2 && latin.iter().all(|c| c.is_uppercase())
}

/// The whole text was Latin capitals (no Hebrew letter in it) and the result is Hebrew with no
/// Latin capital left: the same rule for the clipboard and the console paths.
pub fn caps_fixed_to_hebrew(text: &str, out: &str) -> bool {
    typed_with_caps_lock(text) && !contains_hebrew(text) && contains_hebrew(out) && !out.chars().any(|c| c.is_alphabetic() && c.is_uppercase() && !crate::layouts::is_hebrew_letter(c))
}

/// The keyboard language to switch to after a conversion: Hebrew if the result has Hebrew. With
/// only some words converted (1.8.5), the language of the LAST word that changed (the one the user
/// was typing when F10 was pressed).
pub fn language_after(text: &str, out: &str) -> bool {
    let a: Vec<&str> = text.split_whitespace().collect();
    let b: Vec<&str> = out.split_whitespace().collect();
    if a.len() == b.len() {
        if let Some((_, w)) = a.iter().zip(&b).rev().find(|(x, y)| x != y && y.chars().any(char::is_alphabetic)) {
            return contains_hebrew(w);
        }
    }
    contains_hebrew(out)
}

/// The conversion of this run: keyboard layout (F10) or Caps Lock case swap (F9).
fn transform(host: &dyn Host, s: &AppSettings, map: &KeyMap, text: &str) -> String {
    match s.action {
        // 1.8.5: only the wrong words (option, word lists needed); everything when none is found
        Action::Layout if s.f10_only_wrong_words && s.mixed_text_mode == MixedTextMode::ToggleAll => {
            host.wrong_words_only(text, map, s.replace_caps).unwrap_or_else(|| convert_with(map, text, s.mixed_text_mode, s.replace_caps))
        }
        Action::Layout => convert_with(map, text, s.mixed_text_mode, s.replace_caps),
        Action::Case => swap_case(text),
    }
}

pub fn run(host: &mut dyn Host, s: &AppSettings) -> Outcome {
    run_traced(host, s, &mut Trace::default())
}

/// `run`, with the time of each step and what was known noted in `trace` (diagnostic).
pub fn run_traced(host: &mut dyn Host, s: &AppSettings, trace: &mut Trace) -> Outcome {
    trace.mark(Step::Checks, host.now_ms());
    let target = host.initial_target();
    if target == 0 {
        return Outcome::quiet("No active window, nothing done");
    }
    let info = host.target_info(target);
    trace.info = Some(info.clone());

    if policy::is_excluded(&info.exe_name, &s.excluded_processes) {
        return Outcome::say(Msg::ExcludedProgram, format!("Excluded program ({}), nothing done", info.exe_name));
    }
    // rules per program (tray menu): they can only restrict; the protections below still apply
    let rule = s.rule_for(&info.exe_name);
    if rule == Some(RuleMode::Disabled) {
        return Outcome::say(Msg::ExcludedProgram, format!("Disabled by a rule for {}, nothing done", info.exe_name));
    }
    let careful;
    let s: &AppSettings = if rule == Some(RuleMode::SelectionOnly) && !s.require_selection {
        careful = AppSettings { require_selection: true, ..s.clone() };
        &careful
    } else {
        s
    };
    if info.elevated == Some(true) {
        return Outcome::say(Msg::AdminWindow, "Administrator window, nothing done");
    }
    // keyboard layout of the target: refused if not supported (F9 does not depend on it)
    let map = if s.action == Action::Layout {
        match host.key_map(target) {
            Ok(m) => m,
            Err(name) => return Outcome::say(Msg::LayoutUnsupported, format!("Active keyboard layout not supported ({name}), nothing done")),
        }
    } else {
        us_map()
    };
    if s.action == Action::Layout {
        trace.latin = Some(map.latin.name()); // F9 does not use a key map
    }
    if let Some(kind) = policy::console_kind(&info.window_class) {
        return match kind {
            // careful mode: the console path reads the whole typed line (no selection): refused
            ConsoleKind::Classic if s.console_support && s.require_selection => {
                Outcome::say(Msg::SelectText, "Console and careful mode on: nothing done")
            }
            ConsoleKind::Classic if s.console_support => {
                trace.mark(Step::Console, host.now_ms());
                run_console(host, s, &map, target)
            }
            _ => Outcome::say(Msg::ConsoleUnsupported, format!("Console window ({}), nothing done", info.window_class)),
        };
    }
    if info.password_field {
        return Outcome::say(Msg::PasswordField, "Password field, nothing done");
    }
    trace.mark(Step::KeysReleased, host.now_ms());
    if !host.wait_keys_released() {
        return Outcome::say(Msg::KeysHeld, "Keys still held after 1.5 s, nothing done");
    }
    host.sleep(AFTER_RELEASE_MS); // the hotkey key-up reaches the application first
    if host.foreground() != target {
        return Outcome::say(Msg::WindowChanged, "Target window changed, nothing done");
    }

    trace.mark(Step::ClipboardSave, host.now_ms());
    // The user's clipboard is saved COMPLETELY and emptied in the same opening, or nothing is done.
    let mut st = ClipState::default();
    match host.clipboard_save_and_clear() {
        (SaveResult::Complete, seq) => st.seq_clear = seq,
        (SaveResult::Busy, _) => return Outcome::say(Msg::ClipboardBusy, "Clipboard busy, nothing done"),
        (SaveResult::TooBig, _) => return Outcome::say(Msg::ClipboardTooBig, "Clipboard too big to save, nothing done"),
        (SaveResult::ReadError, _) => return Outcome::say(Msg::ClipboardBusy, "Clipboard could not be read completely, nothing done"),
    }

    let outcome = convert_window(host, s, &map, target, &info, &mut st, trace);

    // ALWAYS: put the user's clipboard back, unless they copied something new meanwhile
    let mut log = outcome.log;
    if st.new_copy_seen {
        log.push_str(" | new copy during conversion: kept, nothing restored");
        host.clipboard_forget();
        return Outcome { msg: outcome.msg, log, preview: outcome.preview };
    }
    // a busy clipboard is retried (provenance checked again each time); a failure is SAID (audit B2)
    trace.mark(Step::Restore, host.now_ms());
    let mut restore = host.clipboard_restore(&st);
    for _ in 0..RESTORE_RETRIES {
        if restore != RestoreResult::Failed {
            break;
        }
        host.pause(200);
        restore = host.clipboard_restore(&st);
    }
    let mut msg = outcome.msg;
    match restore {
        RestoreResult::Restored => {}
        RestoreResult::KeptUserCopy => log.push_str(" | clipboard changed by someone else meanwhile: kept"),
        RestoreResult::Failed => {
            log.push_str(" | clipboard restore FAILED (kept in memory for the tray menu)");
            host.clipboard_park(&st);
            msg = Some(Msg::ClipboardNotRestored);
        }
    }
    // the preview is shown only if everything went well (the user's clipboard is back)
    let preview = if msg == Some(Msg::Converted) { outcome.preview } else { None };
    Outcome { msg, log, preview }
}

enum Copy {
    Text(String),
    NoChange,
    NoText,
    Stopped(Outcome),
}

/// Ctrl+C in the target, then waits for a REAL clipboard change.
fn copy_selection(host: &mut dyn Host, target: usize, st: &mut ClipState) -> Copy {
    if host.foreground() != target {
        return Copy::Stopped(Outcome::say(Msg::WindowChanged, "Target window changed before Ctrl+C"));
    }
    let seq = host.clipboard_seq();
    if !host.send_copy() {
        // part of Ctrl+C may have gone through: what is in the clipboard now is the reference
        note_capture(st, &host.clipboard_capture());
        return Copy::Stopped(Outcome::say(Msg::KeysBlocked, "Ctrl+C not sent"));
    }
    let mut waited = 0;
    loop {
        if host.clipboard_seq() != seq {
            break;
        }
        if waited >= COPY_WAIT_MS {
            return Copy::NoChange;
        }
        if host.foreground() != target {
            note_capture(st, &host.clipboard_capture());
            return Copy::Stopped(Outcome::say(Msg::WindowChanged, "Target window changed during Ctrl+C"));
        }
        host.sleep(COPY_POLL_MS);
        waited += COPY_POLL_MS;
    }
    // let the application finish writing all its formats: the sequence number must stay the
    // same for SETTLE_MS (at most SETTLE_MAX_MS); provenance is checked again before publishing
    let mut last = host.clipboard_seq();
    let (mut quiet, mut spent) = (0, 0);
    while quiet < SETTLE_MS && spent < SETTLE_MAX_MS {
        host.sleep(COPY_POLL_MS);
        spent += COPY_POLL_MS;
        let now = host.clipboard_seq();
        if now == last {
            quiet += COPY_POLL_MS;
        } else {
            last = now;
            quiet = 0;
        }
    }
    let cap = host.clipboard_capture();
    if cap.busy {
        note_capture(st, &cap);
        return Copy::Stopped(Outcome::say(Msg::ClipboardBusy, "Clipboard busy when reading the copy"));
    }
    if cap.foreign_owner {
        // written by ANOTHER program (the user copied elsewhere): never converted nor pasted,
        // and left in place (audit A1)
        st.new_copy_seen = true;
        return Copy::Stopped(Outcome::say(Msg::NewCopyKept, "Clipboard written by another program after Ctrl+C: nothing pasted"));
    }
    note_capture(st, &cap);
    if cap.too_big {
        return Copy::Stopped(Outcome::say(Msg::TooLong, "Copied text larger than the reading limit"));
    }
    match cap.text {
        Some(t) if !t.is_empty() => Copy::Text(t),
        _ => Copy::NoText,
    }
}

fn note_capture(st: &mut ClipState, cap: &Capture) {
    st.seq_cap = Some(cap.seq);
    st.owner_cap = Some(cap.owner);
    st.formats_cap = Some(cap.formats.clone());
}

fn convert_window(
    host: &mut dyn Host,
    s: &AppSettings,
    map: &KeyMap,
    target: usize,
    info: &TargetInfo,
    st: &mut ClipState,
    trace: &mut Trace,
) -> Outcome {
    let selection = info.selection.unwrap_or(Selection::Unknown);
    // "last word": the word to convert (the raw copy stays in st.captured)
    let mut to_convert: Option<String> = None;

    if selection != Selection::Empty {
        trace.mark(Step::Copy, host.now_ms());
        match copy_selection(host, target, st) {
            Copy::Text(t) => {
                let suspect = policy::line_copy_suspect(&t, selection, &info.focus_class);
                st.captured = Some(t);
                st.user_selected = true;
                if suspect {
                    // VS Code / Excel copied the whole line, cell or range: pasting would
                    // duplicate it, or replace Excel formulas by text
                    return Outcome::say(Msg::SelectText, "Whole line/cell/range copied, nothing pasted");
                }
            }
            Copy::NoText => return Outcome::say(Msg::NothingToConvert, NO_TEXT_LOG),
            Copy::Stopped(o) => return o,
            Copy::NoChange => {}
        }
    }

    if st.captured.is_none() && s.require_selection {
        // careful mode: never the "caret -> start of the line" fallback
        return Outcome::say(Msg::SelectText, "Nothing selected and careful mode on: nothing done");
    }
    if st.captured.is_none() {
        // nothing selected (or the application refuses copying): caret -> start of the line
        if host.foreground() != target || !host.focus_unchanged() || host.user_moving() {
            return Outcome::say(Msg::WindowChanged, "Target window or field changed, stopped");
        }
        trace.mark(Step::LineStart, host.now_ms());
        if !host.send_select_to_line_start() {
            return Outcome::say(Msg::KeysBlocked, "Shift+Home not sent, stopped");
        }
        match copy_selection(host, target, st) {
            Copy::Text(t) => {
                let multi = policy::contains_line_break(&t);
                st.captured = Some(t);
                if multi {
                    return Outcome::say(Msg::SelectText, "Shift+Home copied several lines, nothing pasted");
                }
                // 1.8.5: in a field that wraps a long line (chat boxes, Electron, browsers), Shift+Home
                // stops at the start of the DISPLAYED line: the selection grows one displayed line
                // at a time while it stays in the same real line (no line break) and still grows
                if !(s.last_word_only && policy::last_word_field(&info.window_class, &info.focus_class)) && policy::paragraph_field(&info.focus_class, &info.exe_name) {
                    if let Err(o) = extend_to_paragraph(host, target, st, s) {
                        return o;
                    }
                }
            }
            Copy::NoChange => return Outcome::say(Msg::NothingToConvert, "No clipboard change after Ctrl+C (no text?)"),
            Copy::NoText => return Outcome::say(Msg::NothingToConvert, NO_TEXT_LOG),
            Copy::Stopped(o) => return o,
        }
        // 1.4.1 trial: only the last word of that line, in the known text fields
        if s.last_word_only && s.action == Action::Layout && policy::last_word_field(&info.window_class, &info.focus_class) {
            match select_last_word(host, target, st, trace) {
                Ok(word) => to_convert = Some(word),
                Err(o) => return o,
            }
        }
    }

    let captured = st.captured.clone();
    let text = match to_convert.as_deref().or(captured.as_deref()) {
        Some(t) => t,
        None => return Outcome::say(Msg::NothingToConvert, "No text captured"),
    };
    host.remember_caret(); // after the last Ctrl+C (Shift+Home moves the caret on purpose)
    trace.mark(Step::Convert, host.now_ms());
    let n = text.chars().count();
    trace.chars = Some(n);
    if s.max_character_limit > 0 && n as i64 > s.max_character_limit {
        return Outcome::say(Msg::TooLong, format!("Text exceeds limit ({n} > {})", s.max_character_limit));
    }
    let out = transform(&*host, s, map, text);
    if out == text {
        return Outcome::say(Msg::NothingToConvert, "Text unchanged");
    }
    let to_hebrew = language_after(text, &out);
    st.converted = Some(out.clone());

    trace.mark(Step::Publish, host.now_ms());
    match host.clipboard_publish(&out, st) {
        PublishResult::Done(seq) => st.seq_set = Some(seq),
        PublishResult::NewCopy => {
            st.new_copy_seen = true;
            return Outcome::say(Msg::NewCopyKept, "New copy in the clipboard before publishing: NOT pasted");
        }
        PublishResult::Failed => return Outcome::say(Msg::PasteFailed, "Converted text could not be put in the clipboard, NOT pasted"),
        PublishResult::FailedAfterEmpty => {
            return Outcome::say(Msg::PasteFailed, "Converted text only partly put in the clipboard, NOT pasted")
        }
    }
    trace.mark(Step::Verify, host.now_ms());
    host.sleep(10);
    if host.foreground() != target || !host.focus_unchanged() || host.user_moving() {
        return Outcome::say(Msg::WindowChanged, "Target window or field changed, NOT pasted");
    }
    match host.clipboard_still_ours(st) {
        StillOurs::Yes => {}
        StillOurs::No => {
            // the user copied something between our publish and Ctrl+V (audit A2): keep it
            st.new_copy_seen = true;
            return Outcome::say(Msg::NewCopyKept, "New copy in the clipboard before Ctrl+V: NOT pasted");
        }
        // only busy: nothing pasted, the user's clipboard is put back (with retries)
        StillOurs::Busy => return Outcome::say(Msg::ClipboardBusy, "Clipboard busy right before Ctrl+V: NOT pasted"),
    }
    // only once the paste is going ahead (audit 1.3.2 R6), and posted BEFORE Ctrl+V so that the
    // order is certain (Word tags the text with the new language); if Ctrl+V cannot be sent,
    // the previous language is put back at once (audit 1.3.3 F4).
    trace.mark(Step::Paste, host.now_ms());
    let switch = s.switch_language_after_convert && s.action == Action::Layout;
    if switch {
        host.switch_language(target, to_hebrew);
    }
    if !host.send_paste() {
        if switch {
            host.restore_language(target);
        }
        return Outcome::say(Msg::KeysBlocked, "Ctrl+V not sent, NOT pasted");
    }
    if s.action == Action::Layout {
        host.pasted(text, &out, false);
    }
    // 1.8.6: a text the user had selected is selected again after the fix, once the paste is
    // done (after the wait below): short, one line, left to right only (Shift+Left goes the other
    // way in Hebrew text), same window and field
    let reselect = st.user_selected && s.action == Action::Layout && out.chars().count() <= 100 && !out.chars().any(char::is_control) && crate::replace::simple_text(&out) && !contains_hebrew(&out) && !out.chars().any(|c| ('\u{0590}'..='\u{08FF}').contains(&c));
    if s.turn_off_caps_lock && (s.action == Action::Case || (s.action == Action::Layout && caps_fixed_to_hebrew(text, &out))) {
        host.caps_lock_off();
    }
    trace.mark(Step::WaitBeforeRestore, host.now_ms());
    host.pause(s.paste_delay_ms.max(0) as u64); // a slow PC must have read the clipboard before restore
    if reselect && host.foreground() == target && host.focus_unchanged() && !host.user_moving() {
        host.send_select_back(out.chars().count());
    }
    Outcome { preview: Some((text.to_string(), out)), ..Outcome::say(Msg::Converted, format!("Ctrl+V sent with {n} converted characters")) }
}

/// "Only the last word" (1.4.1 trial): the line copied with Shift+Home gives the last
/// word; Right + Shift+Left x n select it; a second copy MUST give exactly that piece (quotes
/// normalised), otherwise nothing is pasted. Any doubt = refusal, the clipboard is put back.
/// Returns the text to convert (quotes normalised). `st.captured` always keeps the RAW text of
/// the last copy: the clipboard checks compare it with what is really there.
/// 1.8.5: at most this many displayed lines are added (a paragraph longer than that: its end only).
const PARAGRAPH_LINES: usize = 30;
/// ...and at most this long (ms) in all.
const PARAGRAPH_MS: u64 = 1_500;

fn extend_to_paragraph(host: &mut dyn Host, target: usize, st: &mut ClipState, s: &AppSettings) -> Result<(), Outcome> {
    let limit = if s.max_character_limit > 0 { s.max_character_limit as usize } else { 5_000 };
    let activity = host.user_activity();
    let started = host.now_ms();
    // one displayed line back, then the selection must be exactly `before` again (the caret may
    // land elsewhere: right-to-left lines, indents, "smart Home"); otherwise nothing is pasted
    let step_back = |host: &mut dyn Host, st: &mut ClipState, before: String| -> Result<(), Outcome> {
        if !host.send_shrink_down() {
            host.send_collapse();
            return Err(Outcome::say(Msg::KeysBlocked, "Shift+Down not sent, stopped"));
        }
        match copy_selection(host, target, st) {
            Copy::Text(t) if t == before => {
                st.captured = Some(t);
                Ok(())
            }
            Copy::Stopped(o) => {
                host.send_collapse();
                Err(o)
            }
            _ => {
                host.send_collapse();
                Err(Outcome::say(Msg::SelectText, "Paragraph: the selection could not be put back exactly, nothing pasted"))
            }
        }
    };
    for _ in 0..PARAGRAPH_LINES {
        let before = st.captured.clone().unwrap_or_default();
        // a key typed or a click, a move, too long: stop where we are (the text selected now is `before`)
        if host.foreground() != target || !host.focus_unchanged() || host.user_moving() || host.user_activity() != activity {
            host.send_collapse();
            return Err(Outcome::say(Msg::WindowChanged, "Target window or field changed, stopped"));
        }
        if host.now_ms().saturating_sub(started) > PARAGRAPH_MS {
            return Ok(());
        }
        if !host.send_extend_up() {
            return Ok(()); // not supported: the displayed line only, as before
        }
        match copy_selection(host, target, st) {
            Copy::Text(t) if policy::contains_line_break(&t) => return step_back(host, st, before),
            Copy::Text(t) if t == before => {
                st.captured = Some(t); // start of the field: complete
                return Ok(());
            }
            Copy::Text(t) if t.chars().count() > limit || !t.ends_with(&before) || t.chars().count() < before.chars().count() => {
                // too long, or it did not grow backwards from the same end: one step back
                return step_back(host, st, before);
            }
            Copy::Text(t) => st.captured = Some(t),
            // the copy failed after the selection moved: back one step and checked
            Copy::NoChange | Copy::NoText => return step_back(host, st, before),
            Copy::Stopped(o) => {
                host.send_collapse();
                return Err(o);
            }
        }
    }
    Ok(())
}

fn select_last_word(host: &mut dyn Host, target: usize, st: &mut ClipState, trace: &mut Trace) -> Result<String, Outcome> {
    use crate::words::{last_word, normalize_quotes, WordRefusal};
    let line = st.captured.clone().unwrap_or_default();
    let w = match last_word(&line) {
        Ok(w) => w,
        Err(WordRefusal::NoWord) => return Err(Outcome::say(Msg::NothingToConvert, "Last word: nothing before the caret")),
        Err(r) => return Err(Outcome::say(Msg::SelectText, format!("Last word: {r:?}, select the text"))),
    };
    if host.modifiers_down() {
        return Err(Outcome::say(Msg::KeysHeld, "Last word: Shift/Ctrl/Alt held, nothing done"));
    }
    if host.foreground() != target || !host.focus_unchanged() || host.user_moving() {
        return Err(Outcome::say(Msg::WindowChanged, "Last word: window or field changed, stopped"));
    }
    trace.mark(Step::LineStart, host.now_ms());
    host.sleep(30); // the application finishes the copy before the keys
    if !host.send_select_last(w.count) {
        return Err(Outcome::say(Msg::KeysBlocked, "Last word: keys not sent, stopped"));
    }
    trace.mark(Step::Copy, host.now_ms());
    match copy_selection(host, target, st) {
        Copy::Text(t) if normalize_quotes(&t) == normalize_quotes(&w.piece) => {
            let word = normalize_quotes(&t);
            st.captured = Some(t);
            Ok(word)
        }
        // a selection other than the word (right-to-left paragraph, visual caret, suggestion
        // list...): never pasted; the clipboard holds that piece, `captured` says so, so that
        // the user's clipboard is put back
        Copy::Text(t) => {
            st.captured = Some(t);
            Err(Outcome::say(Msg::SelectText, "Last word: the selection is not the expected word (right-to-left text?), nothing pasted"))
        }
        Copy::NoChange | Copy::NoText => Err(Outcome::say(Msg::SelectText, "Last word: no copy of the word, nothing pasted")),
        Copy::Stopped(o) => Err(o),
    }
}

// ---------------------------------------------------------------------------------------------
// Classic console: no clipboard, no Ctrl+C. Only line-editing keys (Home, End, Backspace) and
// Unicode characters are sent, and only when cmd/PowerShell is waiting for a command.

fn run_console(host: &mut dyn Host, s: &AppSettings, map: &KeyMap, target: usize) -> Outcome {
    if !host.wait_keys_released() {
        return Outcome::say(Msg::KeysHeld, "Keys still held after 1.5 s, nothing done");
    }
    host.sleep(50);
    if host.foreground() != target {
        return Outcome::say(Msg::WindowChanged, "Target window changed, nothing done");
    }
    if !host.console_attach(target) {
        return Outcome::say(Msg::ConsoleBusy, "Console: could not attach");
    }
    let o = console_inner(host, s, map, target);
    host.console_detach();
    o
}

/// Moves the cursor with Home or End, waits until it stops moving, returns the new position.
/// None if the key was not sent or the cursor never became stable (audit A5).
fn move_and_read(host: &mut dyn Host, home: bool) -> Option<(i32, i32, i32)> {
    let sent = if home { host.send_home() } else { host.send_end() };
    if !sent {
        return None;
    }
    let mut last = host.console_cursor()?;
    let mut stable = 0;
    let mut waited = 0;
    while stable < 3 {
        if waited >= 400 {
            return None;
        }
        host.sleep(POLL_MS);
        waited += POLL_MS;
        let now = host.console_cursor()?;
        if now == last {
            stable += 1;
        } else {
            stable = 0;
            last = now;
        }
    }
    Some(last)
}

/// What the console looked like when the line was read; checked again right before the
/// Backspaces, so that nothing is erased at another place (audit A5).
struct ConsoleView {
    procs: Vec<String>,
    mode: u32,
    home: (i32, i32, i32),
    end: (i32, i32, i32),
    count: usize,
    text: String,
}

fn console_unchanged(host: &mut dyn Host, target: usize, v: &ConsoleView) -> bool {
    host.foreground() == target
        && !host.console_selection_active()
        && host.console_cursor() == Some(v.end)
        && host.console_input_mode() == Some(v.mode)
        && host.console_processes() == v.procs
        && host.console_read(v.home.0, v.home.1, v.count).as_deref() == Some(v.text.as_str())
}

fn console_inner(host: &mut dyn Host, s: &AppSettings, map: &KeyMap, target: usize) -> Outcome {
    let procs = host.console_processes();
    let mode = match host.console_input_mode() {
        Some(m) => m,
        None => return Outcome::say(Msg::ConsoleBusy, "Console: input mode unknown"),
    };
    let reader = match policy::console_reader(&procs, mode) {
        Some(r) => r,
        None => return Outcome::say(Msg::ConsoleBusy, format!("Console busy (mode 0x{mode:X}, {} processes), nothing done", procs.len())),
    };
    if host.console_selection_active() {
        return Outcome::say(Msg::ConsoleBusy, "Console in selection mode, nothing done");
    }
    let start = match host.console_cursor() {
        Some(c) => c,
        None => return Outcome::say(Msg::ConsoleBusy, "Console: cursor unknown"),
    };
    let width = start.2;
    if width <= 0 {
        return Outcome::say(Msg::ConsoleBusy, "Console: bad buffer width");
    }
    if host.foreground() != target {
        return Outcome::say(Msg::WindowChanged, "Target window changed, nothing done");
    }
    let Some(home) = move_and_read(host, true) else {
        return Outcome::say(Msg::KeysBlocked, "Console: Home not sent or cursor not stable");
    };
    // from here the cursor has moved: any exit without replacement puts it back where it was
    match console_line(host, s, map, target, procs, mode, reader, start, home) {
        Ok(o) => o,
        Err(o) => {
            put_cursor_back(host, target, start, home);
            o
        }
    }
}

fn idx(p: (i32, i32, i32), width: i32) -> i64 {
    p.1 as i64 * width as i64 + p.0 as i64
}

/// Home, then Right x (start - home): only if the console is still ours and the distance is sane.
fn put_cursor_back(host: &mut dyn Host, target: usize, start: (i32, i32, i32), home: (i32, i32, i32)) {
    let width = start.2;
    let d = idx(start, width) - idx(home, width);
    if d < 0 || d > width as i64 * 4 || host.foreground() != target || host.console_selection_active() {
        return;
    }
    if host.send_home() && d > 0 {
        host.send_right(d as usize);
    }
}

#[allow(clippy::too_many_arguments)]
fn console_line(
    host: &mut dyn Host,
    s: &AppSettings,
    map: &KeyMap,
    target: usize,
    procs: Vec<String>,
    mode: u32,
    reader: ConsoleReader,
    start: (i32, i32, i32),
    home: (i32, i32, i32),
) -> Result<Outcome, Outcome> {
    let width = start.2;
    if host.foreground() != target {
        return Err(Outcome::say(Msg::WindowChanged, "Target window changed, stopped"));
    }
    let Some(end) = move_and_read(host, false) else {
        return Err(Outcome::say(Msg::KeysBlocked, "Console: End not sent or cursor not stable"));
    };
    let (h, e, c) = (idx(home, width), idx(end, width), idx(start, width));
    if !(h <= c && c <= e) || e - h > (width as i64) * 4 || end.2 != width || home.2 != width {
        return Err(Outcome::say(Msg::ConsoleBusy, "Console: line positions not coherent, nothing done"));
    }
    let count = (e - h) as usize;
    if count == 0 {
        return Err(Outcome::say(Msg::NothingToConvert, "Console: empty line"));
    }
    let text = match host.console_read(home.0, home.1, count) {
        Some(t) => t,
        None => return Err(Outcome::say(Msg::ConsoleBusy, "Console: line could not be read")),
    };
    let max = if s.max_character_limit > 0 { (s.max_character_limit as usize).min(CONSOLE_MAX_CHARS) } else { CONSOLE_MAX_CHARS };
    if !policy::console_text_ok(&text, max) || text.chars().count() != count {
        return Err(Outcome::say(Msg::ConsoleBusy, "Console: line has unsupported characters or is too long, nothing done"));
    }
    let out = transform(&*host, s, map, &text);
    if out == text {
        return Err(Outcome::say(Msg::NothingToConvert, "Console: text unchanged"));
    }
    let caps_typed = caps_fixed_to_hebrew(&text, &out);
    let view = ConsoleView { procs, mode, home, end, count, text };
    if !console_unchanged(host, target, &view) {
        return Err(Outcome::say(Msg::WindowChanged, "Console: cursor, line, mode or programs changed, NOT replaced"));
    }
    // the cursor is at the end of the input: `count` Backspaces erase exactly the input
    if !host.send_replace(count, &out) {
        return Ok(Outcome::say(Msg::KeysBlocked, "Console: keys not sent"));
    }
    if s.switch_language_after_convert && s.action == Action::Layout {
        host.switch_language(target, contains_hebrew(&out));
    }
    if s.turn_off_caps_lock && (s.action == Action::Case || (s.action == Action::Layout && caps_typed)) {
        host.caps_lock_off();
    }
    let who = match reader {
        ConsoleReader::CmdLine => "cmd",
        ConsoleReader::PsReadLine => "PowerShell",
    };
    Ok(Outcome::say(Msg::Converted, format!("Console ({who}): {count} characters replaced")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    /// Simulated clipboard content: a text and/or a non-text item (image, files...).
    #[derive(Debug, Clone, Default, PartialEq)]
    struct Clip {
        text: Option<String>,
        image: bool,
    }

    /// Owners of the simulated clipboard: our hidden window, the target application, another program.
    const US: usize = 1;
    const APP: usize = 2;
    const OTHER: usize = 3;

    /// CF_UNICODETEXT for a text, CF_DIB for an image.
    fn formats_of(c: &Clip) -> Vec<u32> {
        let mut v = Vec::new();
        if c.text.is_some() {
            v.push(13);
        }
        if c.image {
            v.push(8);
        }
        v
    }

    fn text(t: &str) -> Clip {
        Clip { text: Some(t.into()), image: false }
    }

    /// A simulated Windows: a text field with an optional selection, a clipboard with a sequence
    /// number and an owner, a foreground window, and a user who may copy at any moment.
    struct Mock {
        /// 1.8.5: what wrong_words_only returns
        smart: Option<String>,
        /// 1.8.5: displayed lines ABOVE the caret line in the same paragraph (nearest first), then
        /// whether a real line break comes before them; `up` = how many were added
        above: Vec<String>,
        break_above: bool,
        up: Cell<usize>,
        fg: Cell<usize>,
        /// window in the foreground at the hotkey (0 = same as `fg`), audit A03
        start_fg: usize,
        info: TargetInfo,
        keys_ok: bool,
        save: SaveResult,
        clip: RefCell<Clip>,
        seq: Cell<u32>,
        owner: Cell<usize>,
        saved: Option<Clip>,
        /// what Ctrl+C copies (None = application copies nothing)
        selected: RefCell<Option<String>>,
        /// the application copies an image (no text) for Ctrl+C
        copy_image: bool,
        /// what Shift+Home selects
        line: Option<String>,
        copy_ok: bool,
        paste_ok: bool,
        set_ok: bool,
        /// injections (audit 1.3.1): publish fails after emptying, restore always fails,
        /// the user moves the console cursor while the line is read
        audit_partial_publish: bool,
        audit_restore_failure: bool,
        audit_move_console_cursor: bool,
        /// the clipboard is busy for the first N restore attempts
        restore_busy: Cell<u32>,
        /// the clipboard cannot be opened right before Ctrl+V
        still_busy: bool,
        /// injections (audit 1.3.2): a restore that writes the image then fails; another
        /// program copying between our publish and the sequence read
        audit_partial_rich_restore: Cell<bool>,
        audit_copy_before_seq: bool,
        pasted: RefCell<Option<String>>,
        restored: Cell<bool>,
        /// at the first sleep after this event, the user copies this (in another program)
        user_copy: Option<(&'static str, Clip)>,
        /// at the first sleep after this event, Office/Win+V renders a delayed format:
        /// the sequence number moves, the content does not
        seq_bump_after: Option<&'static str>,
        switch_window_after_sleeps: Cell<i32>,
        focus_moved: Cell<bool>,
        moving: Cell<bool>,
        events: RefCell<Vec<String>>,
        // console
        c_procs: Vec<String>,
        c_mode: u32,
        c_line: String,
        c_prompt: i32,
        c_cursor: Cell<(i32, i32, i32)>,
        c_selection: bool,
        c_replaced: RefCell<Option<(usize, String)>>,
        attached: Cell<bool>,
        /// key map given by key_map (None = the US table)
        map: Option<Result<KeyMap, String>>,
        clock: Cell<u64>,
        map_asked: Cell<bool>,
        /// 1.4.1: modifiers held; how the application selects with Shift+Left (None = the last
        /// n characters of the line, Some(x) = this text instead, e.g. a right-to-left field)
        mods_down: bool,
        word_selects: Option<String>,
        /// Office delayed rendering right after the n-th capture (the sequence number moves)
        bump_after_capture: Option<usize>,
        /// 1.5.1: the application writes its text again at the first sleep after Ctrl+C (two
        /// steps), as its own copy
        late_text: Option<&'static str>,
        /// the late text arrives only at this sleep after Ctrl+C (1 = first)
        late_at_sleep: usize,
        /// the sequence number moves at every sleep after Ctrl+C, by this owner (APP: same text;
        /// OTHER: someone else copying the same text)
        churn: Option<usize>,
        sleeps_after_copy: Cell<usize>,
        sleeps_before_capture: Cell<Option<usize>>,
        captures: Cell<usize>,
    }

    impl Mock {
        fn new() -> Self {
            Self {
                smart: None,
                above: Vec::new(),
                break_above: false,
                up: Cell::new(0),
                fg: Cell::new(7),
                start_fg: 0,
                info: TargetInfo { window_class: "Notepad".into(), exe_name: "notepad.exe".into(), elevated: Some(false), ..Default::default() },
                keys_ok: true,
                save: SaveResult::Complete,
                clip: RefCell::new(text("USER CLIPBOARD")),
                seq: Cell::new(1),
                owner: Cell::new(OTHER),
                saved: None,
                selected: RefCell::new(Some("akuo".into())),
                copy_image: false,
                line: Some("akuo".into()),
                copy_ok: true,
                paste_ok: true,
                set_ok: true,
                audit_partial_publish: false,
                audit_restore_failure: false,
                audit_move_console_cursor: false,
                restore_busy: Cell::new(0),
                still_busy: false,
                audit_partial_rich_restore: Cell::new(false),
                audit_copy_before_seq: false,
                pasted: RefCell::new(None),
                restored: Cell::new(false),
                user_copy: None,
                seq_bump_after: None,
                switch_window_after_sleeps: Cell::new(-1),
                focus_moved: Cell::new(false),
                moving: Cell::new(false),
                events: RefCell::new(vec![]),
                c_procs: vec!["cmd.exe".into()],
                c_mode: 0x1F7,
                c_line: "akuo".into(),
                c_prompt: 3, // "C:\>" ends at column 3, input starts at 4
                c_cursor: Cell::new((8, 10, 120)),
                c_selection: false,
                c_replaced: RefCell::new(None),
                attached: Cell::new(false),
                map: None,
                clock: Cell::new(0),
                map_asked: Cell::new(false),
                mods_down: false,
                word_selects: None,
                bump_after_capture: None,
                late_text: None,
                late_at_sleep: 1,
                churn: None,
                sleeps_after_copy: Cell::new(0),
                sleeps_before_capture: Cell::new(None),
                captures: Cell::new(0),
            }
        }
        fn write_clip(&self, c: Clip, owner: usize) {
            *self.clip.borrow_mut() = c;
            self.seq.set(self.seq.get() + 1);
            self.owner.set(owner);
        }
        fn ev(&self, e: &str) {
            self.events.borrow_mut().push(e.into());
        }
        fn happened(&self, e: &str) -> bool {
            self.events.borrow().iter().any(|x| x == e)
        }
        fn obs(&self) -> ClipObs {
            let c = self.clip.borrow();
            ClipObs { seq: self.seq.get(), owner_is_us: self.owner.get() == US, owner: self.owner.get(), text: c.text.clone(), formats: formats_of(&c), foreign_formats: c.image }
        }
        fn clip_text(&self) -> Option<String> {
            self.clip.borrow().text.clone()
        }
        /// Time passes (a sleep, or just before we open the clipboard): the user's copy and the
        /// delayed rendering happen at the first occasion after their event.
        fn tick(&self) {
            if self.happened("copy") && !self.happened("capture") {
                if let Some(owner) = self.churn {
                    let t = self.clip_text();
                    self.write_clip(Clip { text: t, image: false }, owner);
                }
            }
            if let Some(t) = self.late_text {
                if self.happened("copy") && !self.happened("late") && self.sleeps_after_copy.get() >= self.late_at_sleep {
                    self.ev("late");
                    self.write_clip(text(t), APP);
                }
            }
            if let Some((after, c)) = &self.user_copy {
                if self.happened(after) && !self.happened("usercopy") {
                    self.ev("usercopy");
                    self.write_clip(c.clone(), OTHER);
                }
            }
            if let Some(after) = self.seq_bump_after {
                if self.happened(after) && !self.happened("render") {
                    self.ev("render");
                    self.seq.set(self.seq.get() + 1);
                }
            }
        }
    }

    impl Host for Mock {
        fn foreground(&self) -> usize {
            self.fg.get()
        }
        fn wrong_words_only(&self, _text: &str, _map: &KeyMap, _replace_caps: bool) -> Option<String> {
            self.smart.clone()
        }
        fn send_extend_up(&self) -> bool {
            self.ev("extend-up");
            let n = (self.up.get() + 1).min(self.above.len() + 1);
            self.up.set(n);
            let line = self.line.clone().unwrap_or_default();
            let k = n.min(self.above.len());
            let mut t: String = self.above[..k].iter().rev().cloned().collect::<String>() + &line;
            if n > self.above.len() && self.break_above {
                t = format!("previous paragraph\r\n{t}");
            }
            *self.selected.borrow_mut() = Some(t);
            true
        }
        fn send_collapse(&self) -> bool {
            self.ev("collapse");
            true
        }
        fn send_select_back(&self, n: usize) -> bool {
            self.ev(&format!("select-back-{n}"));
            true
        }
        fn send_shrink_down(&self) -> bool {
            self.ev("shrink-down");
            let n = self.up.get().saturating_sub(1);
            self.up.set(n);
            let line = self.line.clone().unwrap_or_default();
            *self.selected.borrow_mut() = Some(self.above[..n.min(self.above.len())].iter().rev().cloned().collect::<String>() + &line);
            true
        }
        fn initial_target(&self) -> usize {
            if self.start_fg != 0 { self.start_fg } else { self.fg.get() }
        }
        fn focus_unchanged(&self) -> bool {
            !self.focus_moved.get()
        }
        fn user_moving(&self) -> bool {
            self.moving.get()
        }
        fn remember_caret(&self) {}
        fn target_info(&self, _: usize) -> TargetInfo {
            self.info.clone()
        }
        fn wait_keys_released(&self) -> bool {
            self.keys_ok
        }
        fn sleep(&self, _ms: u64) {
            if self.happened("copy") {
                self.sleeps_after_copy.set(self.sleeps_after_copy.get() + 1);
            }
            let n = self.switch_window_after_sleeps.get();
            if n == 0 {
                self.fg.set(99);
            }
            if n >= 0 {
                self.switch_window_after_sleeps.set(n - 1);
            }
            self.tick();
        }
        fn clipboard_save_and_clear(&mut self) -> (SaveResult, u32) {
            if self.save == SaveResult::Complete {
                self.saved = Some(self.clip.borrow().clone());
                self.write_clip(Clip::default(), US);
                self.ev("clear");
            }
            (self.save, self.seq.get())
        }
        fn clipboard_seq(&self) -> u32 {
            self.seq.get()
        }
        fn clipboard_capture(&self) -> Capture {
            self.ev("capture");
            if self.sleeps_before_capture.get().is_none() {
                self.sleeps_before_capture.set(Some(self.sleeps_after_copy.get()));
            }
            let owner = self.owner.get();
            let c = Capture { text: self.clip_text(), seq: self.seq.get(), owner, foreign_owner: owner == OTHER, too_big: false, busy: false, formats: formats_of(&self.clip.borrow()) };
            self.captures.set(self.captures.get() + 1);
            if self.bump_after_capture == Some(self.captures.get()) {
                self.seq.set(self.seq.get() + 1);
            }
            c
        }
        fn clipboard_publish(&mut self, t: &str, st: &ClipState) -> PublishResult {
            self.tick();
            self.ev("publish");
            if !self.set_ok {
                if self.audit_partial_publish {
                    self.write_clip(Clip::default(), US);
                    return PublishResult::FailedAfterEmpty;
                }
                return PublishResult::Failed;
            }
            if !may_publish(st, &self.obs()) {
                return PublishResult::NewCopy;
            }
            self.write_clip(text(t), US);
            if self.audit_copy_before_seq {
                self.write_clip(text("UNRELATED COPY"), OTHER);
            }
            PublishResult::Done(self.seq.get())
        }
        fn clipboard_still_ours(&self, st: &ClipState) -> StillOurs {
            self.tick();
            if self.still_busy {
                return StillOurs::Busy;
            }
            if still_ours(st, &self.obs()) { StillOurs::Yes } else { StillOurs::No }
        }
        fn clipboard_restore(&mut self, st: &ClipState) -> RestoreResult {
            if self.audit_partial_rich_restore.replace(false) {
                self.write_clip(Clip { text: None, image: true }, US);
                return RestoreResult::Failed;
            }
            if self.audit_restore_failure {
                return RestoreResult::Failed;
            }
            if self.restore_busy.get() > 0 {
                self.restore_busy.set(self.restore_busy.get() - 1);
                return RestoreResult::Failed;
            }
            self.tick();
            if !may_restore(st, &self.obs()) {
                return RestoreResult::KeptUserCopy;
            }
            let s = self.saved.clone().unwrap_or_default();
            self.write_clip(s, US);
            self.restored.set(true);
            RestoreResult::Restored
        }
        fn clipboard_forget(&mut self) {
            self.saved = None;
        }
        fn clipboard_park(&mut self, _st: &ClipState) {
            self.ev("park");
        }
        fn send_copy(&self) -> bool {
            self.ev("copy");
            if self.copy_ok {
                if self.copy_image {
                    self.write_clip(Clip { text: None, image: true }, APP);
                } else if let Some(t) = self.selected.borrow().clone() {
                    self.write_clip(text(&t), APP);
                }
            }
            self.copy_ok
        }
        fn send_select_to_line_start(&self) -> bool {
            self.ev("shifthome");
            *self.selected.borrow_mut() = self.line.clone();
            true
        }
        fn send_paste(&self) -> bool {
            self.ev("paste");
            if self.paste_ok {
                *self.pasted.borrow_mut() = self.clip_text();
            }
            self.paste_ok
        }
        fn switch_language(&self, _: usize, to_hebrew: bool) {
            self.ev(if to_hebrew { "lang-he" } else { "lang-en" });
        }
        fn restore_language(&self, _: usize) {
            self.ev("lang-restore");
        }
        fn caps_lock_off(&self) {
            self.ev("caps-off");
        }
        fn modifiers_down(&self) -> bool {
            self.mods_down
        }
        fn send_select_last(&self, n: usize) -> bool {
            self.ev("select-last");
            let line = self.line.clone().unwrap_or_default();
            let chars: Vec<char> = line.chars().collect();
            let piece: String = chars[chars.len().saturating_sub(n)..].iter().collect();
            *self.selected.borrow_mut() = Some(self.word_selects.clone().unwrap_or(piece));
            true
        }
        fn now_ms(&self) -> u64 {
            self.clock.set(self.clock.get() + 10);
            self.clock.get()
        }
        fn key_map(&mut self, _: usize) -> Result<KeyMap, String> {
            self.map_asked.set(true);
            self.map.clone().unwrap_or_else(|| Ok(us_map()))
        }
        fn console_attach(&mut self, _: usize) -> bool {
            self.attached.set(true);
            true
        }
        fn console_detach(&mut self) {
            self.attached.set(false);
        }
        fn console_processes(&self) -> Vec<String> {
            self.c_procs.clone()
        }
        fn console_input_mode(&self) -> Option<u32> {
            Some(self.c_mode)
        }
        fn console_selection_active(&self) -> bool {
            self.c_selection
        }
        fn console_cursor(&self) -> Option<(i32, i32, i32)> {
            Some(self.c_cursor.get())
        }
        fn console_read(&self, x: i32, _y: i32, count: usize) -> Option<String> {
            if self.audit_move_console_cursor {
                let c = self.c_cursor.get();
                self.c_cursor.set((self.c_prompt + 1, c.1, c.2));
            }
            let start = (x - self.c_prompt - 1) as usize;
            Some(self.c_line.chars().skip(start).take(count).collect())
        }
        fn send_home(&self) -> bool {
            let c = self.c_cursor.get();
            self.c_cursor.set((self.c_prompt + 1, c.1, c.2));
            true
        }
        fn send_end(&self) -> bool {
            let c = self.c_cursor.get();
            self.c_cursor.set((self.c_prompt + 1 + self.c_line.chars().count() as i32, c.1, c.2));
            true
        }
        fn send_right(&self, n: usize) -> bool {
            let c = self.c_cursor.get();
            self.c_cursor.set((c.0 + n as i32, c.1, c.2));
            true
        }
        fn send_replace(&self, n: usize, text: &str) -> bool {
            *self.c_replaced.borrow_mut() = Some((n, text.into()));
            true
        }
    }

    /// Settings of the tests: defaults, console path switched on (off by default since 1.3.2).
    fn settings() -> AppSettings {
        AppSettings { console_support: true, ..AppSettings::default() }
    }

    fn go(m: &mut Mock) -> Outcome {
        run(m, &settings())
    }

    #[test]
    fn selection_converted_and_clipboard_restored() {
        let mut m = Mock::new();
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום"));
        assert!(m.restored.get());
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn no_selection_uses_shift_home() {
        let mut m = Mock::new();
        *m.selected.borrow_mut() = None;
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert!(m.events.borrow().contains(&"shifthome".to_string()));
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn known_empty_selection_goes_straight_to_shift_home() {
        let mut m = Mock::new();
        m.info.selection = Some(Selection::Empty);
        go(&mut m);
        assert_eq!(m.events.borrow()[1], "shifthome", "right after emptying the clipboard");
    }

    #[test]
    fn vs_code_line_copy_is_not_pasted() {
        let mut m = Mock::new();
        m.info.window_class = "Chrome_WidgetWin_1".into();
        *m.selected.borrow_mut() = Some("let x = akuo;\n".into());
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::SelectText));
        assert!(m.pasted.borrow().is_none());
        assert!(m.restored.get(), "user clipboard must be put back");
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn word_paragraph_with_line_break_is_converted() {
        let mut m = Mock::new();
        m.info.focus_class = "_WwG".into();
        *m.selected.borrow_mut() = Some("akuo\r\n".into());
        assert_eq!(go(&mut m).msg, Some(Msg::Converted));
    }

    #[test]
    fn busy_or_too_big_clipboard_touches_nothing() {
        for (r, msg) in [(SaveResult::Busy, Msg::ClipboardBusy), (SaveResult::TooBig, Msg::ClipboardTooBig)] {
            let mut m = Mock::new();
            m.save = r;
            assert_eq!(go(&mut m).msg, Some(msg));
            assert!(m.events.borrow().is_empty(), "no key sent");
            assert_eq!(m.seq.get(), 1, "clipboard untouched");
        }
    }

    #[test]
    fn checks_before_anything() {
        let mut m = Mock::new();
        m.info.elevated = Some(true);
        assert_eq!(go(&mut m).msg, Some(Msg::AdminWindow));
        let mut m = Mock::new();
        m.info.elevated = None; // unknown -> go on
        assert_eq!(go(&mut m).msg, Some(Msg::Converted));
        let mut m = Mock::new();
        m.info.password_field = true;
        assert_eq!(go(&mut m).msg, Some(Msg::PasswordField));
        let mut m = Mock::new();
        m.info.exe_name = "C:\\x\\RustDesk.exe".into();
        assert_eq!(go(&mut m).msg, Some(Msg::ExcludedProgram));
        let mut m = Mock::new();
        m.keys_ok = false;
        assert_eq!(go(&mut m).msg, Some(Msg::KeysHeld));
        let mut m = Mock::new();
        m.fg.set(0);
        assert_eq!(go(&mut m).msg, None);
        assert_eq!(Mock::new().seq.get(), 1);
    }

    #[test]
    fn window_change_never_pastes_and_restores() {
        for after in 0..12 {
            let mut m = Mock::new();
            m.switch_window_after_sleeps.set(after);
            let o = go(&mut m);
            if o.msg == Some(Msg::Converted) {
                continue; // the change came after the paste
            }
            assert!(m.pasted.borrow().is_none(), "after {after}: {}", o.log);
            if m.seq.get() > 1 {
                assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"), "after {after}: {}", o.log);
            }
        }
    }

    #[test]
    fn other_field_of_same_window_never_pasted() {
        let mut m = Mock::new();
        m.focus_moved.set(true);
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::WindowChanged), "{}", o.log);
        assert!(m.pasted.borrow().is_none());
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn user_copy_during_operation_is_kept() {
        // text or image, copied by the user at any moment after our capture: always kept
        for after in ["capture", "publish", "paste"] {
            for c in [text("NEW USER COPY"), Clip { text: None, image: true }, text("akuo")] {
                let mut m = Mock::new();
                m.user_copy = Some((after, c.clone()));
                let o = go(&mut m);
                assert!(m.happened("usercopy"), "{after}");
                if after == "capture" {
                    // copied before our text was put in: nothing pasted at all
                    assert!(m.pasted.borrow().is_none(), "{after} {c:?}: {}", o.log);
                    assert_eq!(o.msg, Some(Msg::NewCopyKept), "{}", o.log);
                }
                assert_eq!(*m.clip.borrow(), c, "{after} {c:?}: {}", o.log);
                assert!(!m.restored.get(), "{after} {c:?}");
            }
        }
    }

    #[test]
    fn user_copies_same_text_as_converted_after_paste_is_kept() {
        // same text as ours, but a new copy (sequence moved, owner not us): kept
        let mut m = Mock::new();
        m.user_copy = Some(("paste", text("שלום")));
        go(&mut m);
        assert!(!m.restored.get());
        assert_eq!(m.clip_text().as_deref(), Some("שלום"));
    }

    #[test]
    fn delayed_rendering_does_not_cancel() {
        // Office / Win+V move the sequence number after the capture without a new copy
        let mut m = Mock::new();
        m.seq_bump_after = Some("capture");
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert!(m.restored.get(), "{}", o.log);
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn app_copied_image_then_user_copies_image_is_kept() {
        let mut m = Mock::new();
        m.copy_image = true;
        m.user_copy = Some(("capture", Clip { text: None, image: true }));
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::NothingToConvert));
        assert!(!m.restored.get(), "user's new image must stay: {}", o.log);
        // without the user's copy, the application's image is replaced by the saved clipboard
        let mut m = Mock::new();
        m.copy_image = true;
        go(&mut m);
        assert!(m.restored.get());
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn saved_image_comes_back() {
        let mut m = Mock::new();
        *m.clip.borrow_mut() = Clip { text: None, image: true };
        assert_eq!(go(&mut m).msg, Some(Msg::Converted));
        assert_eq!(*m.clip.borrow(), Clip { text: None, image: true });
    }

    #[test]
    fn read_error_touches_nothing() {
        let mut m = Mock::new();
        m.save = SaveResult::ReadError;
        assert_eq!(go(&mut m).msg, Some(Msg::ClipboardBusy));
        assert!(m.events.borrow().is_empty());
        assert_eq!(m.seq.get(), 1);
    }

    #[test]
    fn user_moving_never_pastes() {
        let mut m = Mock::new();
        m.moving.set(true);
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::WindowChanged), "{}", o.log);
        assert!(m.pasted.borrow().is_none());
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn excel_range_and_code_editor_untouched() {
        let mut m = Mock::new();
        m.info.focus_class = "EXCEL7".into();
        *m.selected.borrow_mut() = Some("akuo\tbla\r\nx\ty\r\n".into());
        assert_eq!(go(&mut m).msg, Some(Msg::SelectText));
        assert!(m.pasted.borrow().is_none());
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
        let mut m = Mock::new();
        m.info.exe_name = "C:\\x\\Code.exe".into();
        assert_eq!(go(&mut m).msg, Some(Msg::ExcludedProgram));
        assert!(m.events.borrow().is_empty(), "no Ctrl+C sent in VS Code");
    }

    #[test]
    fn provenance_rules() {
        let obs = |seq, owner: usize, t: Option<&str>, foreign: bool| ClipObs {
            seq,
            owner_is_us: owner == US,
            owner,
            text: t.map(String::from),
            formats: if foreign { vec![13, 8] } else { vec![13] },
            foreign_formats: foreign,
        };
        let mut st = ClipState { seq_clear: 10, ..Default::default() };
        // nothing captured: only our own emptied clipboard is put back
        assert!(may_restore(&st, &obs(10, US, None, false)));
        assert!(!may_restore(&st, &obs(10, OTHER, Some("x"), false)), "same number, other owner: never trusted (R2)");
        assert!(!may_publish(&st, &obs(10, US, None, false)));
        // captured from the application
        st.seq_cap = Some(20);
        st.owner_cap = Some(APP);
        st.formats_cap = Some(vec![13, 8]);
        st.captured = Some("akuo".into());
        assert!(may_publish(&st, &obs(20, APP, Some("akuo"), true)));
        assert!(may_publish(&st, &obs(23, APP, Some("akuo"), true)), "delayed rendering: same owner, same text");
        assert!(!may_publish(&st, &obs(23, OTHER, Some("akuo"), true)), "same text, other owner (audit B1)");
        assert!(!may_publish(&st, &obs(23, APP, Some("other"), false)));
        assert!(!may_publish(&st, &obs(23, OTHER, None, true)), "new image");
        assert!(may_restore(&st, &obs(20, APP, None, true)));
        assert!(!may_restore(&st, &obs(21, OTHER, None, true)));
        let null_owner = ClipState { owner_cap: Some(0), ..st.clone() };
        assert!(!may_publish(&null_owner, &obs(23, 0, Some("akuo"), false)), "no owner: equal text proves nothing");
        // no text both times: the list of formats must match too (R3)
        let img = ClipState { captured: None, formats_cap: Some(vec![8]), ..st.clone() };
        let pic = |f: Vec<u32>| ClipObs { seq: 24, owner_is_us: false, owner: APP, text: None, formats: f, foreign_formats: true };
        assert!(may_restore(&img, &pic(vec![8])));
        assert!(!may_restore(&img, &pic(vec![8, 15])), "other formats from the same window: a new copy");
        // published
        st.seq_set = Some(30);
        st.converted = Some("שלום".into());
        assert!(still_ours(&st, &obs(31, US, Some("שלום"), false)));
        assert!(!still_ours(&st, &obs(30, OTHER, Some("x"), false)), "same number read after closing, other owner (R2)");
        assert!(may_restore(&st, &obs(31, US, None, true)), "our own partial restore (R1)");
        assert!(!may_restore(&st, &obs(30, OTHER, Some("שלום"), false)), "someone else's copy is kept");
        st.new_copy_seen = true;
        assert!(!may_restore(&st, &obs(30, US, Some("שלום"), false)));
    }

    #[test]
    fn failures_are_reported_and_clipboard_restored() {
        let mut m = Mock::new();
        m.set_ok = false;
        assert_eq!(go(&mut m).msg, Some(Msg::PasteFailed));
        assert!(m.pasted.borrow().is_none());
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
        let mut m = Mock::new();
        m.copy_ok = false;
        assert_eq!(go(&mut m).msg, Some(Msg::KeysBlocked));
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
        let mut m = Mock::new();
        m.paste_ok = false;
        assert_eq!(go(&mut m).msg, Some(Msg::KeysBlocked));
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn unchanged_and_too_long() {
        let mut m = Mock::new();
        *m.selected.borrow_mut() = Some("123".into());
        assert_eq!(go(&mut m).msg, Some(Msg::NothingToConvert));
        assert!(m.pasted.borrow().is_none());
        let mut m = Mock::new();
        let s = AppSettings { max_character_limit: 2, ..AppSettings::default() };
        assert_eq!(run(&mut m, &s).msg, Some(Msg::TooLong));
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn language_switch_right_after_paste() {
        let mut m = Mock::new();
        let s = AppSettings { switch_language_after_convert: true, ..AppSettings::default() };
        run(&mut m, &s);
        let ev = m.events.borrow();
        let l = ev.iter().position(|e| e == "lang-he").unwrap();
        let p = ev.iter().position(|e| e == "paste").unwrap();
        assert!(l < p, "the keyboard changes right before Ctrl+V (order certain)");
    }

    // ---- consoles ----

    #[test]
    fn audit_a03_window_changed_before_the_work_starts_stops_without_ctrl_c() {
        // F10 pressed in window 5, but window 7 is in the foreground when the work starts
        let mut m = Mock { start_fg: 5, ..Mock::new() };
        let o = run(&mut m, &AppSettings::default());
        assert_eq!(o.msg, Some(Msg::WindowChanged), "{}", o.log);
        assert!(!m.events.borrow().contains(&"copy".to_string()), "never Ctrl+C in the other window");
        assert!(!m.events.borrow().contains(&"paste".to_string()));
    }

    fn console() -> Mock {
        let mut m = Mock::new();
        m.info.window_class = "ConsoleWindowClass".into();
        m.info.exe_name = "cmd.exe".into();
        m
    }

    #[test]
    fn console_cmd_line_replaced_without_clipboard() {
        let mut m = console();
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.c_replaced.borrow().clone(), Some((4, "שלום".to_string())));
        assert_eq!(m.seq.get(), 1, "clipboard never touched");
        assert!(!m.events.borrow().contains(&"copy".to_string()), "never Ctrl+C");
        assert!(!m.attached.get(), "detached at the end");
    }

    #[test]
    fn console_running_program_untouched() {
        let mut m = console();
        m.c_procs = vec!["cmd.exe".into(), "python.exe".into()];
        assert_eq!(go(&mut m).msg, Some(Msg::ConsoleBusy));
        assert!(m.c_replaced.borrow().is_none());
        let mut m = console();
        m.c_selection = true;
        assert_eq!(go(&mut m).msg, Some(Msg::ConsoleBusy));
        let mut m = console();
        m.c_mode = 0x80; // raw mode but cmd only: unknown reader
        assert_eq!(go(&mut m).msg, Some(Msg::ConsoleBusy));
        assert!(m.c_replaced.borrow().is_none());
    }

    #[test]
    fn console_powershell_and_options() {
        let mut m = console();
        m.c_procs = vec!["powershell.exe".into()];
        m.c_mode = 0x80 | 0x10;
        m.c_prompt = 6;
        m.c_cursor.set((11, 3, 120));
        m.c_line = "יקךךם".into();
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.c_replaced.borrow().clone(), Some((5, "hello".to_string())));
        let mut m = console();
        let mut s = settings();
        s.console_support = false;
        assert_eq!(run(&mut m, &s).msg, Some(Msg::ConsoleUnsupported));
        let mut m = console();
        m.info.window_class = "CASCADIA_HOSTING_WINDOW_CLASS".into();
        assert_eq!(go(&mut m).msg, Some(Msg::ConsoleUnsupported));
        assert!(m.events.borrow().is_empty());
    }

    #[test]
    fn console_empty_or_incoherent() {
        let mut m = console();
        m.c_line = String::new();
        m.c_cursor.set((4, 10, 120));
        assert_eq!(go(&mut m).msg, Some(Msg::NothingToConvert));
        let mut m = console();
        m.c_cursor.set((2, 10, 120)); // cursor BEFORE the input start: not coherent
        assert_eq!(go(&mut m).msg, Some(Msg::ConsoleBusy));
        assert!(m.c_replaced.borrow().is_none());
    }
    // Audit tests: expected user-facing safety properties, deliberately not implementation mirrors.
    #[test]
    fn audit_new_copy_before_capture_must_not_be_pasted() {
        let mut m = Mock::new();
        m.user_copy = Some(("copy", text("NEW USER COPY")));
        let o = go(&mut m);
        assert!(m.pasted.borrow().is_none(), "Other clipboard content was pasted: {:?}; {}", m.pasted.borrow(), o.log);
        assert_eq!(m.clip_text().as_deref(), Some("NEW USER COPY"));
    }

    #[test]
    fn audit_new_copy_between_publish_and_paste_must_cancel() {
        let mut m = Mock::new();
        m.user_copy = Some(("publish", text("NEW USER COPY")));
        let o = go(&mut m);
        assert!(m.pasted.borrow().is_none(), "Other clipboard content was pasted: {:?}; {}", m.pasted.borrow(), o.log);
    }

    #[test]
    fn audit_same_text_new_copy_before_publish_must_be_kept() {
        let mut m = Mock::new();
        m.user_copy = Some(("capture", text("akuo")));
        let o = go(&mut m);
        assert_eq!(m.clip_text().as_deref(), Some("akuo"), "{}", o.log);
    }

    #[test]
    fn audit_partial_publish_failure_must_restore_original() {
        let mut m = Mock::new();
        m.set_ok = false;
        m.audit_partial_publish = true;
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::PasteFailed));
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"), "{}", o.log);
    }

    #[test]
    fn audit_a08_failed_restore_keeps_the_copy_for_the_menu() {
        let mut m = Mock::new();
        m.audit_restore_failure = true;
        let o = run(&mut m, &AppSettings::default());
        assert_eq!(o.msg, Some(Msg::ClipboardNotRestored));
        assert_eq!(m.events.borrow().iter().filter(|e| *e == "park").count(), 1, "kept once, for the tray menu");
        let mut ok = Mock::new();
        run(&mut ok, &AppSettings::default());
        assert!(!ok.events.borrow().contains(&"park".to_string()), "nothing kept when the restore worked");
    }

    #[test]
    fn audit_restore_failure_must_not_show_success() {
        let mut m = Mock::new();
        m.audit_restore_failure = true;
        let o = go(&mut m);
        assert_ne!(o.msg, Some(Msg::Converted), "User sees success despite: {}", o.log);
    }

    #[test]
    fn audit_console_cursor_changed_after_read_must_cancel() {
        let mut m = console();
        m.audit_move_console_cursor = true;
        let o = go(&mut m);
        assert!(m.c_replaced.borrow().is_none(), "Replacement sent at another cursor position: {}", o.log);
    }

    #[test]
    fn audit_console_no_change_must_keep_cursor_position() {
        let mut m = console();
        m.c_line = "12345".into();
        m.c_cursor.set((6, 10, 120));
        let before = m.c_cursor.get();
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::NothingToConvert));
        assert_eq!(m.c_cursor.get(), before, "No conversion should preserve the cursor");
    }

    #[test]
    fn audit_new_foreign_rich_copy_is_not_ours() {
        let st = ClipState { seq_cap: Some(20), captured: Some("akuo".into()), ..Default::default() };
        let now = ClipObs { seq: 30, owner_is_us: false, owner: OTHER, text: Some("akuo".into()), formats: vec![13, 8], foreign_formats: true };
        assert!(!may_publish(&st, &now), "Equal text does not establish the provenance of a new rich copy");
    }


    // ---- 1.3.2 ----

    #[test]
    fn busy_clipboard_restore_is_retried() {
        let mut m = Mock::new();
        m.restore_busy.set(2);
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn console_is_off_by_default() {
        let mut m = console();
        assert_eq!(run(&mut m, &AppSettings::default()).msg, Some(Msg::ConsoleUnsupported));
        assert!(m.events.borrow().is_empty() && m.c_replaced.borrow().is_none());
    }

    #[test]
    fn partial_publish_failure_restores_the_user_clipboard() {
        let mut m = Mock::new();
        m.set_ok = false;
        m.audit_partial_publish = true;
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::PasteFailed));
        assert!(m.pasted.borrow().is_none());
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"), "{}", o.log);
    }

    #[test]
    fn busy_clipboard_before_paste_restores_and_pastes_nothing() {
        let mut m = Mock::new();
        m.still_busy = true;
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::ClipboardBusy), "{}", o.log);
        assert!(m.pasted.borrow().is_none());
        assert!(m.restored.get(), "user clipboard put back: {}", o.log);
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn image_copied_for_us_then_rendered_late_is_still_replaced_by_the_saved_clipboard() {
        let mut m = Mock::new();
        m.copy_image = true;
        m.seq_bump_after = Some("capture");
        go(&mut m);
        assert!(m.restored.get());
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    // ---- audit 1.3.2 (contre-audit) ----

    #[test]
    fn audit132_partial_rich_restore_must_not_be_mistaken_for_user_copy() {
        let mut m = Mock::new();
        let original = Clip { text: Some("USER CLIPBOARD".into()), image: true };
        *m.clip.borrow_mut() = original.clone();
        m.audit_partial_rich_restore.set(true);
        let o = go(&mut m);
        assert_eq!(*m.clip.borrow(), original, "Partial restoration must be retried, not discarded: {}", o.log);
    }

    #[test]
    fn audit132_copy_after_publish_close_before_sequence_read_must_cancel() {
        let mut m = Mock::new();
        m.audit_copy_before_seq = true;
        let o = go(&mut m);
        assert!(m.pasted.borrow().is_none(), "Unrelated content pasted: {:?}; {}", m.pasted.borrow(), o.log);
        assert_eq!(m.clip_text().as_deref(), Some("UNRELATED COPY"));
    }

    /// Adapted: with a TEXT, "same window + same text" stays accepted, because
    /// Office may publish its object again with another list of formats (delayed rendering),
    /// and refusing would drop the user's saved clipboard. Known, documented limit. Without
    /// text, a different list of formats is a new copy.
    #[test]
    fn audit132_same_owner_does_not_prove_same_copy() {
        let st = ClipState { seq_cap: Some(20), owner_cap: Some(APP), captured: Some("akuo".into()), ..Default::default() };
        let now = ClipObs { seq: 30, owner_is_us: false, owner: APP, text: Some("akuo".into()), formats: vec![13, 8], foreign_formats: true };
        assert!(may_publish(&st, &now), "accepted limit with a text (documented)");
        let st = ClipState { seq_cap: Some(20), owner_cap: Some(APP), captured: None, formats_cap: Some(vec![8]), ..Default::default() };
        let now = ClipObs { seq: 30, owner_is_us: false, owner: APP, text: None, formats: vec![8, 15], foreign_formats: true };
        assert!(!may_restore(&st, &now), "a fresh non-text copy from the same window must be preserved");
    }

    #[test]
    fn audit132_cancelled_publish_must_not_switch_keyboard() {
        let mut m = Mock::new();
        m.user_copy = Some(("capture", text("NEW USER COPY")));
        let mut s = settings();
        s.switch_language_after_convert = true;
        let o = run(&mut m, &s);
        assert_eq!(o.msg, Some(Msg::NewCopyKept));
        assert!(!m.happened("lang-he") && !m.happened("lang-en"), "Cancelled operation changed the keyboard");
    }

    #[test]
    fn careful_mode_never_selects_to_line_start() {
        let mut m = Mock::new();
        *m.selected.borrow_mut() = None;
        let mut s = settings();
        s.require_selection = true;
        assert_eq!(run(&mut m, &s).msg, Some(Msg::SelectText));
        assert!(!m.happened("shifthome"));
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    // ---- audit 1.3.3 ----

    #[test]
    fn audit133_failed_paste_should_not_change_keyboard() {
        let mut m = Mock::new();
        m.paste_ok = false;
        let mut s = settings();
        s.switch_language_after_convert = true;
        let o = run(&mut m, &s);
        assert_eq!(o.msg, Some(Msg::KeysBlocked));
        // adapted: the language is asked before Ctrl+V, then put back at once
        let ev = m.events.borrow();
        let he = ev.iter().position(|e| e == "lang-he").expect("asked before Ctrl+V");
        let back = ev.iter().position(|e| e == "lang-restore").expect("The paste was blocked, but the keyboard stayed changed");
        assert!(he < back);
    }

    /// Accepted limit (R3): kept as a reminder, not counted as passing.
    #[test]
    #[ignore = "limite acceptee R3 : meme fenetre + meme texte non distingues"]
    fn audit133_original_r3_strict_property_remains_unfulfilled() {
        let st = ClipState { seq_cap: Some(20), owner_cap: Some(APP), captured: Some("akuo".into()), ..Default::default() };
        let now = ClipObs { seq: 30, owner_is_us: false, owner: APP, text: Some("akuo".into()), formats: vec![13, 8], foreign_formats: true };
        assert!(!may_publish(&st, &now));
    }

    /// Documents the remaining image ambiguity (same owner, same formats): accepted.
    #[test]
    fn audit133_image_with_same_owner_and_formats_is_still_accepted() {
        let st = ClipState { seq_cap: Some(20), owner_cap: Some(APP), captured: None, formats_cap: Some(vec![8]), ..Default::default() };
        let now = ClipObs { seq: 30, owner_is_us: false, owner: APP, text: None, formats: vec![8], foreign_formats: true };
        assert!(may_restore(&st, &now));
    }

    #[test]
    fn audit133_require_selection_must_also_protect_console() {
        let mut m = console();
        let mut s = settings();
        s.require_selection = true;
        let o = run(&mut m, &s);
        assert!(m.c_replaced.borrow().is_none(), "RequireSelection changed an unselected console command: {}", o.log);
    }

    // ---- 1.8.6 : the user's selection is selected again after the fix ----

    #[test]
    fn selection_kept_after_f10() {
        let mut m = Mock::new();
        *m.selected.borrow_mut() = Some("שלום".into());
        run(&mut m, &settings());
        assert!(m.happened("select-back-4"), "a selected word is selected again");
        // a Hebrew result: Shift+Left would go the other way, never selected again
        let mut m = Mock::new();
        *m.selected.borrow_mut() = Some("akuo".into());
        run(&mut m, &settings());
        assert!(!m.events.borrow().iter().any(|e| e.starts_with("select-back")));
        // without a selection (Shift+Home): nothing selected afterwards
        let mut m = Mock::new();
        *m.selected.borrow_mut() = None;
        m.line = Some("akuo".into());
        run(&mut m, &settings());
        assert!(!m.events.borrow().iter().any(|e| e.starts_with("select-back")));
    }

    // ---- 1.8.5 : no selection = the whole paragraph, in a field that wraps ----

    #[test]
    fn no_selection_takes_the_whole_wrapped_paragraph() {
        // a paragraph displayed on 3 lines, a real line break before it
        let mut m = Mock::new();
        *m.selected.borrow_mut() = None;
        m.info.focus_class = "Chrome_RenderWidgetHostHWND".into();
        m.line = Some("vk ".into());
        m.above = vec!["nv ".into(), "akuo ".into()];
        m.break_above = true;
        let o = run(&mut m, &settings());
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום מה הל "));
        assert!(m.happened("shrink-down"), "one step back after the real line break");
        // the paragraph starts the field: no line break, the selection stops growing
        let mut m = Mock::new();
        *m.selected.borrow_mut() = None;
        m.info.focus_class = "Chrome_RenderWidgetHostHWND".into();
        m.line = Some("nv".into());
        m.above = vec!["akuo ".into()];
        let o = run(&mut m, &settings());
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום מה"), "{}", o.log);
        assert!(!m.happened("shrink-down"));
        // a single displayed line: as before
        let mut m = Mock::new();
        *m.selected.borrow_mut() = None;
        m.info.focus_class = "Chrome_RenderWidgetHostHWND".into();
        m.line = Some("akuo".into());
        run(&mut m, &settings());
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום"));
    }

    // ---- 1.8.5 : F10 only the wrong words ----

    #[test]
    fn f10_only_wrong_words_when_asked() {
        let on = AppSettings { f10_only_wrong_words: true, ..settings() };
        // the option on and an answer: only the wrong words are pasted; the keyboard follows the last
        let mut m = Mock::new();
        m.smart = Some("שלום לכולם מה נשמע".into());
        *m.selected.borrow_mut() = Some("שלום לכולם nv bang".into());
        let o = run(&mut m, &on);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום לכולם מה נשמע"));
        // no answer (no word lists, nothing found wrong): everything is converted, as before
        let mut m = Mock::new();
        *m.selected.borrow_mut() = Some("akuo".into());
        run(&mut m, &on);
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום"));
        // option off: never asked
        let mut m = Mock::new();
        m.smart = Some("x".into());
        *m.selected.borrow_mut() = Some("akuo".into());
        run(&mut m, &settings());
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום"));
        assert!(language_after("hello akuo", "hello שלום"));
        assert!(!language_after("שלום ים' שרק", "שלום how are"), "last changed word is English");
    }

    // ---- 1.8.3 : F10 on text typed with Caps Lock ----

    #[test]
    fn f10_on_capitals_to_hebrew_turns_caps_lock_off() {
        assert!(typed_with_caps_lock("AKUO"));
        assert!(typed_with_caps_lock("AKUO 123"));
        assert!(!typed_with_caps_lock("Akuo") && !typed_with_caps_lock("A") && !typed_with_caps_lock("akuo") && !typed_with_caps_lock("שלום"));
        assert!(caps_fixed_to_hebrew("AKUO", "שלום"));
        assert!(!caps_fixed_to_hebrew("שלום AB", "akuo שב"), "mixed text: Caps Lock left alone");
        assert!(!caps_fixed_to_hebrew("AKUO", "AKUO"), "capitals left in the result");
        let mut m = Mock::new();
        *m.selected.borrow_mut() = Some("AKUO".into());
        let o = run(&mut m, &settings());
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום"));
        assert!(m.happened("caps-off"), "Caps Lock switched off after AKUO -> שלום");
        // small letters, or Hebrew to Latin: Caps Lock left alone
        let mut m = Mock::new();
        *m.selected.borrow_mut() = Some("akuo".into());
        run(&mut m, &settings());
        assert!(!m.happened("caps-off"));
        let mut m = Mock::new();
        *m.selected.borrow_mut() = Some("שלום".into());
        run(&mut m, &settings());
        assert!(!m.happened("caps-off"));
        // the user's choice "do not switch Caps Lock off" is kept
        let mut m = Mock::new();
        *m.selected.borrow_mut() = Some("AKUO".into());
        run(&mut m, &AppSettings { turn_off_caps_lock: false, ..settings() });
        assert!(!m.happened("caps-off"));
    }

    // ---- 1.3.5 : Caps Lock fix (F9) ----

    fn caps(m: &mut Mock, selected: &str) -> Outcome {
        *m.selected.borrow_mut() = Some(selected.into());
        let s = AppSettings { action: Action::Case, switch_language_after_convert: true, ..settings() };
        run(m, &s)
    }

    #[test]
    fn f9_swaps_case_keeps_keyboard_and_turns_caps_lock_off() {
        let mut m = Mock::new();
        let o = caps(&mut m, "hELLO wORLD");
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.pasted.borrow().as_deref(), Some("Hello World"));
        assert!(m.happened("caps-off"));
        assert!(!m.happened("lang-he") && !m.happened("lang-en"), "F9 never changes the keyboard language");
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"), "user clipboard put back");
    }

    #[test]
    fn f9_same_protections_as_f10() {
        let mut m = Mock::new();
        m.info.password_field = true;
        assert_eq!(caps(&mut m, "aBC").msg, Some(Msg::PasswordField));
        let mut m = Mock::new();
        m.info.exe_name = "C:\\x\\Code.exe".into();
        assert_eq!(caps(&mut m, "aBC").msg, Some(Msg::ExcludedProgram));
        let mut m = Mock::new();
        assert_eq!(caps(&mut m, "123 שלום").msg, Some(Msg::NothingToConvert), "nothing with a case");
        assert!(!m.happened("caps-off"), "Caps Lock left alone when nothing was pasted");
        let mut m = Mock::new();
        m.paste_ok = false;
        assert_eq!(caps(&mut m, "aBC").msg, Some(Msg::KeysBlocked));
        assert!(!m.happened("caps-off"));
    }

    #[test]
    fn f9_in_console_swaps_the_line() {
        let mut m = console();
        m.c_line = "dIR".into();
        m.c_cursor.set((7, 10, 120));
        let s = AppSettings { action: Action::Case, ..settings() };
        let o = run(&mut m, &s);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.c_replaced.borrow().clone(), Some((3, "Dir".to_string())));
    }

    // ---------------------------------------------------------------- 1.4.0: layouts, diagnostic

    #[test]
    fn unsupported_layout_is_refused_before_the_clipboard_is_touched() {
        let mut m = Mock::new();
        m.map = Some(Err("allemand (0x04070407)".into()));
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::LayoutUnsupported), "{}", o.log);
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
        assert!(m.pasted.borrow().is_none());
        assert!(m.saved.is_none(), "the clipboard was never saved");
    }

    #[test]
    fn caps_fix_does_not_depend_on_the_layout() {
        let mut m = Mock::new();
        m.map = Some(Err("allemand".into()));
        *m.selected.borrow_mut() = Some("hELLO".into());
        let s = AppSettings { action: Action::Case, ..settings() };
        let o = run(&mut m, &s);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.pasted.borrow().as_deref(), Some("Hello"));
        assert!(!m.map_asked.get(), "F9 never asks for the layout");
    }

    #[test]
    fn azerty_map_is_used_when_the_host_gives_it() {
        use crate::layouts::{build_keymap, reference, LatinKind};
        let mut m = Mock::new();
        m.map = Some(Ok(build_keymap(LatinKind::FrenchAzerty, &reference::french_azerty(), &reference::hebrew()).unwrap()));
        *m.selected.borrow_mut() = Some("qkuo".into());
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום"));
    }

    #[test]
    fn trace_notes_every_step_of_a_success_and_never_the_text() {
        let mut m = Mock::new();
        let mut t = Trace::default();
        let o = run_traced(&mut m, &settings(), &mut t);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        let (last, total) = t.finish(m.now_ms());
        assert_eq!(last, Some(Step::Restore));
        assert!(total > 0);
        let names: Vec<Step> = t.steps.iter().map(|(s, _)| *s).collect();
        for st in [Step::Checks, Step::KeysReleased, Step::ClipboardSave, Step::Copy, Step::Convert, Step::Publish, Step::Verify, Step::Paste, Step::WaitBeforeRestore, Step::Restore] {
            assert!(names.contains(&st), "{st:?} missing in {names:?}");
        }
        assert_eq!(t.chars, Some(4));
        assert_eq!(t.latin, Some("anglais (États-Unis)"));
        assert_eq!(t.info.as_ref().map(|i| i.exe_name.as_str()), Some("notepad.exe"));
    }

    #[test]
    fn rule_disabled_stops_and_selection_only_is_careful_mode() {
        let mut m = Mock::new();
        let mut s = settings();
        assert!(s.set_rule("Notepad.EXE", Some(RuleMode::Disabled)));
        let o = run(&mut m, &s);
        assert_eq!(o.msg, Some(Msg::ExcludedProgram), "{}", o.log);
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
        // selection only: nothing selected -> no "start of the line" fallback
        let mut m = Mock::new();
        *m.selected.borrow_mut() = None;
        m.line = Some("akuo".into());
        assert!(s.set_rule("notepad.exe", Some(RuleMode::SelectionOnly)));
        let o = run(&mut m, &s);
        assert_eq!(o.msg, Some(Msg::SelectText), "{}", o.log);
        // a selected text is still converted
        let mut m = Mock::new();
        let o = run(&mut m, &s);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
    }

    #[test]
    fn a_rule_never_lifts_a_protection() {
        // no rule mode can make a password field or an administrator window convertible
        for mode in [RuleMode::Disabled, RuleMode::SelectionOnly] {
            let mut m = Mock::new();
            m.info.password_field = true;
            let mut s = settings();
            s.set_rule("notepad.exe", Some(mode));
            let o = run(&mut m, &s);
            assert!(matches!(o.msg, Some(Msg::PasswordField) | Some(Msg::ExcludedProgram)), "{:?}", o.msg);
            assert!(m.pasted.borrow().is_none());
        }
    }

    // ---------------------------------------------------------------- 1.4.1: last word (trial)

    fn last_word_mock(line: &str, focus: &str) -> Mock {
        let mut m = Mock::new();
        *m.selected.borrow_mut() = None;
        m.info.selection = Some(Selection::Empty);
        m.info.focus_class = focus.into();
        m.line = Some(line.into());
        m
    }

    fn last_word_settings() -> AppSettings {
        AppSettings { last_word_only: true, ..settings() }
    }

    #[test]
    fn only_the_last_word_is_converted_and_checked_by_a_second_copy() {
        let mut m = last_word_mock("bonjour akuo", "Edit");
        let o = run(&mut m, &last_word_settings());
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום"));
        assert!(m.events.borrow().iter().any(|e| e == "select-last"));
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn off_by_default_the_whole_line_as_before() {
        let mut m = last_word_mock("akuo akuo", "Edit");
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום שלום"));
        assert!(!m.events.borrow().iter().any(|e| e == "select-last"));
    }

    #[test]
    fn a_wrong_selection_is_never_pasted() {
        // right-to-left paragraph: Shift+Left selected something else
        let mut m = last_word_mock("bonjour akuo", "_WwG");
        m.word_selects = Some("bonjour".into());
        let o = run(&mut m, &last_word_settings());
        assert_eq!(o.msg, Some(Msg::SelectText), "{}", o.log);
        assert!(m.pasted.borrow().is_none());
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn excel_unknown_fields_held_keys_and_niqqud_are_safe() {
        // Excel: the usual "start of the line" path (no Shift+Left in a grid)
        let mut m = last_word_mock("x akuo", "EXCEL7");
        let _ = run(&mut m, &last_word_settings());
        assert!(!m.events.borrow().iter().any(|e| e == "select-last"));
        // modifiers held: refused
        let mut m = last_word_mock("x akuo", "Edit");
        m.mods_down = true;
        let o = run(&mut m, &last_word_settings());
        assert_eq!(o.msg, Some(Msg::KeysHeld), "{}", o.log);
        assert!(m.pasted.borrow().is_none());
        // niqqud: the caret moves by grapheme -> refused
        let mut m = last_word_mock("x שָׁלוֹם", "Edit");
        let o = run(&mut m, &last_word_settings());
        assert_eq!(o.msg, Some(Msg::SelectText), "{}", o.log);
        assert!(m.pasted.borrow().is_none());
    }

    #[test]
    fn word_typographic_quote_is_the_geresh_key() {
        // Word turned the ' typed on the w key into a typographic quote
        let mut m = last_word_mock("text akuo\u{2019}", "_WwG");
        let o = run(&mut m, &last_word_settings());
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום,"));
    }

    #[test]
    fn word_quote_and_office_delayed_rendering_keep_the_users_clipboard() {
        // G1: Word renders late (sequence moves after the 2nd capture) and the word has ’
        let mut m = last_word_mock("text akuo\u{2019}", "_WwG");
        m.bump_after_capture = Some(2);
        let o = run(&mut m, &last_word_settings());
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום,"));
        assert!(m.restored.get(), "{}", o.log);
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn wrong_selection_with_delayed_rendering_still_restores_the_clipboard() {
        // G2: right-to-left paragraph, Shift+Left selected something else, Office renders late
        let mut m = last_word_mock("bonjour akuo", "_WwG");
        m.word_selects = Some("our ak".into());
        m.bump_after_capture = Some(2);
        let o = run(&mut m, &last_word_settings());
        assert_eq!(o.msg, Some(Msg::SelectText), "{}", o.log);
        assert!(m.pasted.borrow().is_none());
        assert!(m.restored.get(), "{}", o.log);
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn a_tab_right_before_the_caret_is_refused_at_once() {
        let mut m = last_word_mock("akuo\t", "Edit");
        let o = run(&mut m, &last_word_settings());
        assert_eq!(o.msg, Some(Msg::SelectText), "{}", o.log);
        assert!(!m.events.borrow().iter().any(|e| e == "select-last"), "no key sent");
    }

    // ---------------------------------------------------------------- 1.5.1: speed

    #[test]
    fn an_application_writing_its_copy_in_two_steps_is_waited_for() {
        // first write "ak", then the full text: the second one must be converted
        let mut m = Mock::new();
        *m.selected.borrow_mut() = Some("ak".into());
        m.late_text = Some("akuo");
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום"));
        assert_eq!(m.clip_text().as_deref(), Some("USER CLIPBOARD"));
    }

    #[test]
    fn settle_and_release_waits_are_short() {
        const _: () = assert!(COPY_POLL_MS <= 5 && SETTLE_MS <= 20 && SETTLE_MAX_MS <= 100 && AFTER_RELEASE_MS <= 20);
    }

    #[test]
    fn g1_waits_for_a_text_written_later_than_the_first_sleep() {
        let mut m = Mock::new();
        *m.selected.borrow_mut() = Some("ak".into());
        m.late_text = Some("akuo");
        m.late_at_sleep = 3; // after 2 quiet sleeps (10 ms): still within the 15 ms window
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        assert_eq!(m.pasted.borrow().as_deref(), Some("שלום"));
    }

    #[test]
    fn g1_never_waits_more_than_its_cap_and_provenance_still_decides() {
        // the application keeps moving the sequence number (same text, same owner): bounded wait
        let mut m = Mock::new();
        m.churn = Some(APP);
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::Converted), "{}", o.log);
        let n = m.sleeps_before_capture.get().unwrap();
        assert!(n <= (SETTLE_MAX_MS / COPY_POLL_MS) as usize + 1, "{n} sleeps before the capture");
        // another program writes during that time: never converted nor pasted
        let mut m = Mock::new();
        m.churn = Some(OTHER);
        let o = go(&mut m);
        assert_eq!(o.msg, Some(Msg::NewCopyKept), "{}", o.log);
        assert!(m.pasted.borrow().is_none());
    }
}
