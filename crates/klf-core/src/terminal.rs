//! 1.9.5: F10 in Windows Terminal (option, on by default in the public version, never on a PC
//! with an administrator policy, where it is also hidden). A terminal has no selection to copy and Ctrl+C would stop the running program: the
//! keys the user just typed in the terminal are followed here (memory only, wiped on Enter, Tab,
//! arrows, click, another window, and within 20 s without a key), then F10 checks ON THE SCREEN that exactly
//! this text is just before the cursor, and replaces it with Backspaces and the converted text
//! (one SendInput, nothing else: no Ctrl+C, no clipboard, no Enter). Design reviewed by Fable.
//!
//! Only virtual keys are kept (never characters): they are turned into text at F10 time only.

/// At most this many keys are followed; beyond, the run is "broken" (nothing done).
pub const MAX_KEYS: usize = 200;
/// A run older than this (no key for 15 s) is forgotten.
pub const IDLE_MS: u64 = 15_000;

/// One key press of the user in the terminal: the virtual key, its scan code, Shift held, and the
/// keyboard layout (HKL) of the terminal at that moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct KeyStroke {
    pub vk: u16,
    pub scan: u16,
    pub shift: bool,
    pub hkl: usize,
}

/// What a key does to the run.
#[derive(Debug, PartialEq, Eq)]
pub enum KeyClass {
    /// a letter, digit, space or sign (OEM key): added
    Text,
    /// Backspace: the last key is taken away
    Backspace,
    /// Shift alone: nothing
    Ignored,
    /// anything else (Enter, Tab, Esc, arrows, F1-F24, keypad, IME, Caps Lock, Ctrl/Alt/Win…):
    /// the run is forgotten (refused by default)
    Reset,
}

/// `ctrl_alt_win`: Ctrl, Alt (AltGr = Ctrl+Alt) or Win was held with this key.
pub fn classify(vk: u16, ctrl_alt_win: bool) -> KeyClass {
    match vk {
        0x10 | 0xA0 | 0xA1 => KeyClass::Ignored, // Shift, left/right Shift
        _ if ctrl_alt_win => KeyClass::Reset,
        0x08 => KeyClass::Backspace,
        0x20 | 0x30..=0x39 | 0x41..=0x5A | 0xBA..=0xC0 | 0xDB..=0xDF | 0xE2 => KeyClass::Text,
        _ => KeyClass::Reset,
    }
}

/// What F10 can work on. (No Debug: a `{:?}` would write the keys to a journal)
#[derive(PartialEq, Eq)]
pub enum Typed {
    /// keys typed since the last reset (in one keyboard layout)
    Keys(Vec<KeyStroke>),
    /// the text a previous F10 put there (confirmed on the screen): a second F10 toggles it back
    Text(String),
}

/// The keys typed in the terminal. A fixed array (no allocation in the keyboard hook), zeroed
/// whenever it is emptied. (No Debug, see Typed.)
pub struct Tracker {
    keys: [KeyStroke; MAX_KEYS],
    len: usize,
    broken: bool,
    adopted: Option<String>,
    generation: u64,
    last_ms: u64,
}

impl Default for Tracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Tracker {
    pub const fn new() -> Self {
        Tracker { keys: [KeyStroke { vk: 0, scan: 0, shift: false, hkl: 0 }; MAX_KEYS], len: 0, broken: false, adopted: None, generation: 0, last_ms: 0 }
    }

    fn wipe(&mut self) {
        for k in self.keys.iter_mut() {
            // SAFETY: writing a plain value through a valid &mut (volatile: not optimized away).
            unsafe { std::ptr::write_volatile(k, KeyStroke::default()) };
        }
        self.len = 0;
        if let Some(t) = self.adopted.take() {
            wipe_string(t);
        }
    }

    /// Everything is forgotten; a new run can start.
    pub fn reset(&mut self) {
        self.wipe();
        self.broken = false;
        self.generation += 1;
    }

    /// Something not followed happened (key injected by another program, hook busy): nothing is
    /// done until the next reset.
    pub fn break_run(&mut self) {
        self.wipe();
        self.broken = true;
        self.generation += 1;
    }

    /// One key press of the user (`now`: milliseconds, any steady clock).
    pub fn key(&mut self, k: KeyStroke, ctrl_alt_win: bool, now: u64) {
        if (self.len > 0 || self.adopted.is_some() || self.broken) && now.saturating_sub(self.last_ms) > IDLE_MS {
            self.reset();
        }
        self.last_ms = now;
        match classify(k.vk, ctrl_alt_win) {
            KeyClass::Ignored => {}
            KeyClass::Reset => self.reset(),
            KeyClass::Backspace => {
                if self.broken {
                    return;
                }
                if self.adopted.is_some() || self.len == 0 {
                    // erasing what we do not follow (the converted text, or before the run)
                    self.break_run();
                    return;
                }
                self.len -= 1;
                self.keys[self.len] = KeyStroke::default();
                self.generation += 1;
            }
            KeyClass::Text => {
                if self.broken {
                    return;
                }
                if self.adopted.is_some() {
                    self.reset(); // typing after a conversion: a new run starts here
                }
                if self.len > 0 && self.keys[self.len - 1].hkl != k.hkl {
                    // the layout changed: only what follows is one run
                    self.reset();
                }
                if self.len == MAX_KEYS {
                    self.break_run();
                    return;
                }
                self.keys[self.len] = k;
                self.len += 1;
                self.generation += 1;
            }
        }
    }

    /// 1.9.5: called every few seconds: a run left for 15 s is wiped even without a next key
    /// (a password typed without Enter).
    pub fn expire(&mut self, now: u64) {
        if (self.len > 0 || self.adopted.is_some() || self.broken) && now.saturating_sub(self.last_ms) > IDLE_MS {
            self.reset();
        }
    }

    /// Changes each time the run changes.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// What F10 can convert now, with the generation; None if nothing is followed.
    pub fn snapshot(&self, now: u64) -> Option<(Typed, u64)> {
        if self.broken || now.saturating_sub(self.last_ms) > IDLE_MS {
            return None;
        }
        if let Some(t) = &self.adopted {
            return Some((Typed::Text(t.clone()), self.generation));
        }
        (self.len > 0).then(|| (Typed::Keys(self.keys[..self.len].to_vec()), self.generation))
    }

    /// The converted text was confirmed on the screen: it replaces the run (a second F10 toggles).
    pub fn adopt(&mut self, text: &str, now: u64) {
        self.wipe();
        self.broken = false;
        self.adopted = Some(text.to_string());
        self.last_ms = now;
        self.generation += 1;
    }
}

/// Overwrites a string's memory before dropping it (typed text, possibly a secret).
pub fn wipe_string(s: String) {
    let mut b = s.into_bytes();
    for x in b.iter_mut() {
        // SAFETY: writing a plain value through a valid &mut (volatile: not optimized away).
        unsafe { std::ptr::write_volatile(x, 0) };
    }
}

/// A character that may be sent to a terminal. An allow-list: printable ASCII, the
/// printable Latin-1 letters and signs of the Latin layouts (AZERTY é, à, ç, §…; not the soft
/// hyphen), the Hebrew letters, maqaf, geresh, gershayim, the shekel and euro signs. All one
/// column wide; no combining mark, no direction mark, no control character.
pub fn printable(c: char) -> bool {
    matches!(c as u32, 0x20..=0x7E | 0xA1..=0xAC | 0xAE..=0xFF | 0x05D0..=0x05EA | 0x05BE | 0x05F3 | 0x05F4 | 0x20AA | 0x20AC)
}

/// Why a typed or converted text is not sent.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Empty,
    TooLong,
    NotPrintable,
    NoLetter,
}

/// The text may be erased and sent: 1 to MAX_KEYS printable characters, at least one letter.
pub fn check_text(t: &str) -> Result<usize, Refusal> {
    let n = t.chars().count();
    if n == 0 {
        return Err(Refusal::Empty);
    }
    if n > MAX_KEYS {
        return Err(Refusal::TooLong);
    }
    if !t.chars().all(printable) {
        return Err(Refusal::NotPrintable);
    }
    if !t.chars().any(char::is_alphabetic) {
        return Err(Refusal::NoLetter);
    }
    Ok(n)
}

/// The text read on the screen just before the cursor (line breaks of the display ignored) ends
/// with exactly `typed`.
/// (Compared from the end, without copying the screen text.)
pub fn shown(screen_before_caret: &str, typed: &str) -> bool {
    if typed.is_empty() {
        return false;
    }
    let mut screen = screen_before_caret.chars().rev().filter(|c| *c != '\r' && *c != '\n');
    typed.chars().rev().all(|t| screen.next() == Some(t))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(vk: u16) -> KeyStroke {
        KeyStroke { vk, scan: 1, shift: false, hkl: 0x0409 }
    }
    fn keys(t: &Tracker) -> Vec<u16> {
        match t.snapshot(0) {
            Some((Typed::Keys(v), _)) => v.iter().map(|k| k.vk).collect(),
            _ => vec![],
        }
    }

    #[test]
    fn letters_digits_space_and_signs_are_kept_anything_else_resets() {
        let mut t = Tracker::new();
        for vk in [0x41, 0x4B, 0x20, 0x31, 0xBA, 0xDE] {
            t.key(k(vk), false, 0);
        }
        assert_eq!(keys(&t), vec![0x41, 0x4B, 0x20, 0x31, 0xBA, 0xDE]);
        t.key(k(0x10), false, 0); // Shift alone: nothing
        assert_eq!(keys(&t).len(), 6);
        // Enter, Tab, Esc, arrows, Home, Delete, F8, keypad, IME, Caps Lock: reset
        for vk in [0x0D, 0x09, 0x1B, 0x25, 0x24, 0x2E, 0x77, 0x61, 0xE5, 0x14, 0x11, 0x12, 0x5B] {
            t.key(k(0x41), false, 0);
            t.key(k(vk), false, 0);
            assert!(t.snapshot(0).is_none(), "vk {vk:#x} must reset");
        }
        // a letter with Ctrl/Alt/Win (Ctrl+V, Ctrl+R, AltGr…) resets
        t.key(k(0x41), false, 0);
        t.key(k(0x56), true, 0);
        assert!(t.snapshot(0).is_none());
    }

    #[test]
    fn backspace_takes_away_and_breaks_when_nothing_is_left() {
        let mut t = Tracker::new();
        t.key(k(0x41), false, 0);
        t.key(k(0x42), false, 0);
        t.key(k(0x08), false, 0);
        assert_eq!(keys(&t), vec![0x41]);
        t.key(k(0x08), false, 0);
        t.key(k(0x08), false, 0); // erases what was there before the run: not followed
        t.key(k(0x43), false, 0);
        assert!(t.snapshot(0).is_none(), "broken until the next reset");
        t.key(k(0x0D), false, 0);
        t.key(k(0x43), false, 0);
        assert_eq!(keys(&t), vec![0x43]);
    }

    #[test]
    fn a_layout_change_starts_a_new_run_and_the_run_has_a_limit() {
        let mut t = Tracker::new();
        t.key(k(0x41), false, 0);
        t.key(KeyStroke { hkl: 0x040D, ..k(0x42) }, false, 0);
        assert_eq!(keys(&t), vec![0x42]);
        let mut t = Tracker::new();
        for _ in 0..MAX_KEYS {
            t.key(k(0x41), false, 0);
        }
        assert_eq!(keys(&t).len(), MAX_KEYS);
        t.key(k(0x41), false, 0);
        assert!(t.snapshot(0).is_none());
    }

    #[test]
    fn idle_runs_are_forgotten_and_generation_moves() {
        let mut t = Tracker::new();
        t.key(k(0x41), false, 1_000);
        assert!(t.snapshot(1_000 + IDLE_MS).is_some());
        assert!(t.snapshot(1_001 + IDLE_MS).is_none());
        let now = 2_000 + IDLE_MS;
        t.key(k(0x42), false, now);
        assert!(matches!(t.snapshot(now), Some((Typed::Keys(ref v), _)) if v.len() == 1 && v[0].vk == 0x42), "the old key was forgotten");
        let g = t.generation();
        t.key(k(0x10), false, now);
        assert_eq!(t.generation(), g, "Shift alone changes nothing");
        t.key(k(0x43), false, now);
        assert_ne!(t.generation(), g);
    }

    #[test]
    fn a_run_left_alone_expires_without_a_next_key() {
        let mut t = Tracker::new();
        t.key(k(0x41), false, 1_000);
        t.expire(1_000 + IDLE_MS);
        assert!(t.snapshot(1_000 + IDLE_MS).is_some());
        t.expire(1_001 + IDLE_MS);
        assert!(t.snapshot(0).is_none() && t.len == 0 && t.keys[0] == KeyStroke::default(), "wiped");
    }

    #[test]
    fn adopted_text_toggles_back_and_typing_after_it_starts_again() {
        let mut t = Tracker::new();
        t.key(k(0x41), false, 0);
        t.adopt("שלום", 0);
        assert!(matches!(t.snapshot(0), Some((Typed::Text(ref s), _)) if s == "שלום"));
        t.key(k(0x42), false, 0);
        assert_eq!(keys(&t), vec![0x42]);
        t.adopt("x", 0);
        t.key(k(0x08), false, 0); // erasing the converted text: not followed
        assert!(t.snapshot(0).is_none());
    }

    #[test]
    fn only_simple_one_column_characters_are_sent() {
        assert_eq!(check_text("akuo"), Ok(4));
        assert_eq!(check_text("שלום עולם"), Ok(9));
        assert_eq!(check_text("git commit -m ,"), Ok(15));
        assert_eq!(check_text(""), Err(Refusal::Empty));
        assert_eq!(check_text("123 ,."), Err(Refusal::NoLetter));
        assert_eq!(check_text("été à ç"), Ok(7), "AZERTY letters");
        for bad in ["a\u{A0}b", "a\nb", "a\rb", "a\tb", "a\u{1b}b", "שָׁלוֹם", "a\u{200F}b", "日本", "a😀", "a\u{AD}b", "ab\u{0E31}", "\u{0644}\u{064E}"] {
            assert_eq!(check_text(bad), Err(Refusal::NotPrintable), "{bad:?}");
        }
        assert_eq!(check_text(&"a".repeat(MAX_KEYS + 1)), Err(Refusal::TooLong));
    }

    #[test]
    fn the_typed_text_must_be_exactly_before_the_cursor() {
        assert!(shown("PS C:\\> akuo", "akuo"));
        assert!(shown("PS C:\\> ak\r\nuo", "akuo"), "a wrapped line");
        assert!(!shown("PS C:\\> ****", "akuo"), "secret prompt: stars, not the text");
        assert!(!shown("PS C:\\> ", "akuo"), "no echo (password)");
        assert!(!shown("akuo x", "akuo"), "text after the typed run: the cursor moved");
        assert!(!shown("anything", ""));
    }
}
