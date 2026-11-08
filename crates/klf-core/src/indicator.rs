//! "HE / EN" sign when the keyboard language changes (1.5.0, off by default): when to show it.
//! Pure code, tested on Linux; klf-win reads the layout and Caps Lock and shows the sign.
//! In doubt, nothing is shown.

/// What to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sign {
    /// keyboard layout (low 32 bits of the HKL)
    Layout(u32),
    /// Caps Lock read again after a press: on / off
    CapsLock(bool),
}

/// Short tag of a layout: HE, EN, FR, or None (klf-win asks Windows for the ISO code then).
pub fn tag(hkl: u32) -> Option<&'static str> {
    match hkl & 0x3FF {
        0x0D => Some("HE"),
        0x09 => Some("EN"),
        0x0C => Some("FR"),
        _ => None,
    }
}

/// Memory of the indicator between two readings.
#[derive(Debug, Clone, Default)]
pub struct State {
    last_layout: Option<u32>,
    last_caps_presses: Option<u32>,
    last_shown: Option<(Sign, u64)>,
    /// no sign until this time (a conversion bubble is on screen)
    quiet_until: u64,
}

/// Same sign within this time: not shown again.
pub const REPEAT_MS: u64 = 2000;

impl State {
    /// After a conversion: its bubble has priority, the language it switched to is not shown.
    pub fn quiet_for(&mut self, now_ms: u64, ms: u64) {
        self.quiet_until = now_ms + ms;
    }

    /// One reading: layout of the foreground window (None = unknown) and Caps Lock presses seen
    /// by the hook (None = not followed). Returns Some(true) when Caps Lock was pressed: klf-win
    /// then reads its state a moment later and calls `caps_read`.
    pub fn reading(&mut self, layout: Option<u32>, caps_presses: Option<u32>, now_ms: u64) -> (Option<Sign>, bool) {
        let mut caps_pressed = false;
        if let (Some(c), Some(last)) = (caps_presses, self.last_caps_presses) {
            caps_pressed = c != last;
        }
        if caps_presses.is_some() {
            self.last_caps_presses = caps_presses;
        }
        let mut sign = None;
        if let Some(l) = layout {
            if self.last_layout.is_some_and(|x| x != l) {
                sign = self.show(Sign::Layout(l), now_ms);
            }
            self.last_layout = Some(l);
        }
        (sign, caps_pressed)
    }

    /// Caps Lock state read after a press (None = unknown: nothing shown).
    pub fn caps_read(&mut self, on: Option<bool>, now_ms: u64) -> Option<Sign> {
        on.and_then(|o| self.show(Sign::CapsLock(o), now_ms))
    }

    fn show(&mut self, s: Sign, now_ms: u64) -> Option<Sign> {
        if now_ms < self.quiet_until {
            return None;
        }
        if let Some((last, t)) = self.last_shown {
            if last == s && now_ms.saturating_sub(t) < REPEAT_MS {
                return None;
            }
        }
        self.last_shown = Some((s, now_ms));
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const HE: u32 = 0x040D_040D;
    const EN: u32 = 0x0409_0409;

    #[test]
    fn a_change_of_language_is_shown_once() {
        let mut s = State::default();
        assert_eq!(s.reading(Some(EN), Some(0), 0), (None, false), "first reading: nothing (no change seen)");
        assert_eq!(s.reading(Some(EN), Some(0), 500), (None, false));
        assert_eq!(s.reading(Some(HE), Some(0), 1000), (Some(Sign::Layout(HE)), false));
        assert_eq!(s.reading(Some(HE), Some(0), 1500), (None, false));
        // back and forth quickly: each change is shown (the latest language matters)
        assert_eq!(s.reading(Some(EN), Some(0), 1600).0, Some(Sign::Layout(EN)));
        assert_eq!(s.reading(Some(HE), Some(0), 1700).0, Some(Sign::Layout(HE)));
        // the same sign right after (another window with the same language): not again
        assert_eq!(s.caps_read(None, 1800), None);
        let mut t = State::default();
        t.reading(Some(EN), None, 0);
        assert_eq!(t.reading(Some(HE), None, 100).0, Some(Sign::Layout(HE)));
        t.reading(Some(EN), None, 150); // shown EN
        assert_eq!(t.reading(Some(EN), None, 200).0, None);
    }

    #[test]
    fn unknown_layout_shows_nothing_and_caps_lock_is_read_after_a_press() {
        let mut s = State::default();
        s.reading(Some(EN), Some(3), 0);
        assert_eq!(s.reading(None, Some(3), 500), (None, false));
        assert_eq!(s.reading(Some(EN), Some(4), 1000), (None, true), "a press: read the state");
        assert_eq!(s.caps_read(Some(true), 1060), Some(Sign::CapsLock(true)));
        assert_eq!(s.caps_read(None, 1100), None, "unknown state: nothing");
        assert_eq!(s.reading(Some(EN), None, 1500), (None, false), "hook missing: no Caps Lock sign");
    }

    #[test]
    fn quiet_after_a_conversion() {
        let mut s = State::default();
        s.reading(Some(EN), None, 0);
        s.quiet_for(100, 2000);
        assert_eq!(s.reading(Some(HE), None, 600).0, None, "the conversion bubble has priority");
        assert_eq!(s.reading(Some(EN), None, 2500).0, Some(Sign::Layout(EN)));
        assert_eq!(tag(HE), Some("HE"));
        assert_eq!(tag(0x0419_0419), None);
    }
}
