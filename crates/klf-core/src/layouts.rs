//! Keyboard layouts: which ones are supported, and the Hebrew <-> Latin key map built from two
//! layouts read from Windows (French AZERTY), paired by PHYSICAL key (scan code): AZERTY moves
//! the virtual-key codes (the physical Q key gives VK_A), the scan code does not move.
//!
//! The US map stays the fixed, proven table of convert.rs; a built map is used only for the
//! French AZERTY layout, and only if it passes the checks of `build_keymap`.

/// One character of a layout: the unshifted level of the key with this scan code (Shift is
/// never used: the Hebrew layout gives Latin capitals there). Dead keys and AltGr are never read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyChar {
    pub sc: u16,
    pub ch: char,
}

/// Latin layouts the conversion knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LatinKind {
    /// English (United States): fixed table.
    #[default]
    Us,
    /// French (France), AZERTY: table built from the installed layout.
    FrenchAzerty,
    /// 1.8.2: another Latin layout whose letters are NOT on the US keys (German or Swiss QWERTZ,
    /// Belgian AZERTY...): table built from the installed layout, as for French AZERTY.
    OtherFromWindows,
}

impl LatinKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Us => "anglais (États-Unis)",
            Self::FrenchAzerty => "français (AZERTY)",
            Self::OtherFromWindows => "autre clavier latin (table de Windows)",
        }
    }
}

/// What a keyboard layout (HKL) of Windows is, for the conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutKind {
    HebrewStandard,
    Latin(LatinKind),
    /// Any other layout: announced as not supported.
    Unsupported,
}

/// Classifies a keyboard layout handle by its exact value: 0x040D040D Hebrew (the classic
/// Windows layout "Hebrew"; not "Hebrew (Standard)" SI-1452), 0x04090409 US, 0x040C040C French
/// (France) AZERTY. Any other layout is checked by its letters (klf-win `choose`): a Hebrew
/// variant with the same letter keys works as Hebrew, a Latin layout uses the fixed US table
/// (as before 1.4.0), anything else (Russian, Arabic...) is refused.
pub fn classify_hkl(hkl: u32) -> LayoutKind {
    match hkl {
        0x040D_040D => LayoutKind::HebrewStandard,
        0x0409_0409 => LayoutKind::Latin(LatinKind::Us),
        0x040C_040C => LayoutKind::Latin(LatinKind::FrenchAzerty),
        _ => LayoutKind::Unsupported,
    }
}

/// Short French name of a layout handle, for the settings window and the diagnostic.
pub fn hkl_name(hkl: u32) -> String {
    let lang = hkl & 0xFFFF;
    let known = match classify_hkl(hkl) {
        LayoutKind::HebrewStandard => return "hébreu".into(),
        LayoutKind::Latin(k) => return k.name().into(),
        LayoutKind::Unsupported => match lang {
            0x040D => "hébreu (variante, par ex. SI-1452)",
            0x0409 => "anglais (États-Unis, variante)",
            0x0809 => "anglais (Royaume-Uni)",
            0x040C => "français (variante)",
            0x080C => "français (Belgique)",
            0x0C0C => "français (Canada)",
            0x100C => "français (Suisse)",
            0x0407 => "allemand",
            0x0419 => "russe",
            0x0401 => "arabe",
            _ => "",
        },
    };
    if known.is_empty() {
        format!("clavier 0x{hkl:08X}")
    } else {
        format!("{known} (0x{hkl:08X})")
    }
}

/// Hebrew <-> Latin character pairs. `pairs` is in priority order: for the reverse direction
/// the FIRST pair wins (same rule as the fixed table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyMap {
    pub latin: LatinKind,
    pairs: Vec<(char, char)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapError {
    /// A Hebrew letter has no key, or two Hebrew letters share one Latin character.
    Incomplete(String),
}

/// The 27 Hebrew letters (22 + 5 final forms).
pub fn is_hebrew_letter(c: char) -> bool {
    ('\u{05D0}'..='\u{05EA}').contains(&c)
}

impl KeyMap {
    pub fn from_pairs(latin: LatinKind, pairs: Vec<(char, char)>) -> Self {
        Self { latin, pairs }
    }
    pub fn latin_to_he(&self, c: char) -> Option<char> {
        self.pairs.iter().find(|(l, _)| *l == c).map(|(_, h)| *h)
    }
    pub fn he_to_latin(&self, c: char) -> Option<char> {
        self.pairs.iter().find(|(_, h)| *h == c).map(|(l, _)| *l)
    }
    pub fn pairs(&self) -> &[(char, char)] {
        &self.pairs
    }
}

/// Builds the map from the unshifted level of two layouts, key by key (same scan code).
/// Rules: never a digit on either side (AZERTY "é" is the key of Hebrew "2": the
/// numbers would break); a pair is kept only if the Hebrew side is a Hebrew letter or the
/// Latin side is a letter; identical pairs are useless. Upper case: each Latin letter gets the
/// Hebrew letter of its lower case (as the fixed US table does). Checked: all 27 Hebrew
/// letters present, each with its own Latin character.
pub fn build_keymap(latin_kind: LatinKind, latin: &[KeyChar], hebrew: &[KeyChar]) -> Result<KeyMap, MapError> {
    let mut pairs: Vec<(char, char)> = Vec::new();
    for l in latin {
        let Some(h) = hebrew.iter().find(|k| k.sc == l.sc) else { continue };
        let (lc, hc) = (l.ch, h.ch);
        if lc == hc || lc.is_ascii_digit() || hc.is_ascii_digit() || lc.is_control() || hc.is_control() || lc.is_whitespace() || hc.is_whitespace() {
            continue;
        }
        if !(is_hebrew_letter(hc) || lc.is_alphabetic()) {
            continue;
        }
        if lc.is_alphabetic() && is_hebrew_letter(lc) {
            continue; // a Hebrew letter on the "Latin" layout: not a Latin layout
        }
        if !pairs.iter().any(|(a, _)| *a == lc) {
            pairs.push((lc, hc));
        }
    }
    // upper case of each Latin letter -> same Hebrew character (after all lower-case pairs, so
    // that the reverse direction gives the lower case)
    let lower: Vec<(char, char)> = pairs.clone();
    for (lc, hc) in lower {
        if !lc.is_lowercase() {
            continue;
        }
        let mut up = lc.to_uppercase();
        let (Some(u), None) = (up.next(), up.next()) else { continue };
        let mut back = u.to_lowercase();
        if (back.next(), back.next()) != (Some(lc), None) || pairs.iter().any(|(a, _)| *a == u) {
            continue;
        }
        pairs.push((u, hc));
    }
    // checks: every Hebrew letter, each with its own Latin character
    let mut missing = String::new();
    for h in '\u{05D0}'..='\u{05EA}' {
        let partners: Vec<char> = pairs.iter().filter(|(l, x)| *x == h && !l.is_uppercase()).map(|(l, _)| *l).collect();
        if partners.len() != 1 {
            missing.push(h);
        }
    }
    if !missing.is_empty() {
        return Err(MapError::Incomplete(format!("lettres hébraïques sans touche unique : {missing}")));
    }
    Ok(KeyMap { latin: latin_kind, pairs })
}

/// A Hebrew layout whose 27 letters are on the same keys as the classic Windows layout (a
/// variant such as SI-1452 moves only signs): it converts like the classic one.
pub fn same_hebrew_letters(table: &[KeyChar]) -> bool {
    let reference = reference::hebrew();
    ('\u{05D0}'..='\u{05EA}').all(|h| {
        let want = reference.iter().find(|k| k.ch == h).map(|k| k.sc);
        want.is_some() && table.iter().find(|k| k.ch == h).map(|k| k.sc) == want
    })
}

/// 1.8.2: the 26 letters a-z are on the same keys as US English (UK, US-International, Dutch...):
/// the fixed US table is right for it, as before. Otherwise (QWERTZ, AZERTY) it is not.
pub fn qwerty_letters(table: &[KeyChar]) -> bool {
    let us = reference::us();
    ('a'..='z').all(|c| {
        let want = us.iter().find(|k| k.ch == c).map(|k| k.sc);
        want.is_some() && table.iter().find(|k| k.ch == c).map(|k| k.sc) == want
    })
}

/// A Latin layout: at least 20 of its letter keys give an ASCII letter (UK, US-International,
/// German...; not Russian or Arabic).
pub fn is_latin_table(table: &[KeyChar]) -> bool {
    table.iter().filter(|k| (0x10..=0x32).contains(&k.sc) && k.ch.is_ascii_alphabetic()).count() >= 20
}

/// Reference tables (unshifted level, by scan code), written from the Windows layouts kbdus,
/// kbdfr and kbdheb. Used by the tests, and to check that a layout read from Windows looks like
/// the one expected.
pub mod reference {
    use super::KeyChar;

    fn table(rows: &[(u16, char)]) -> Vec<KeyChar> {
        rows.iter().map(|&(sc, ch)| KeyChar { sc, ch }).collect()
    }

    pub fn us() -> Vec<KeyChar> {
        table(&[
            (0x29, '`'), (0x02, '1'), (0x03, '2'), (0x04, '3'), (0x05, '4'), (0x06, '5'), (0x07, '6'),
            (0x08, '7'), (0x09, '8'), (0x0A, '9'), (0x0B, '0'), (0x0C, '-'), (0x0D, '='),
            (0x10, 'q'), (0x11, 'w'), (0x12, 'e'), (0x13, 'r'), (0x14, 't'), (0x15, 'y'), (0x16, 'u'),
            (0x17, 'i'), (0x18, 'o'), (0x19, 'p'), (0x1A, '['), (0x1B, ']'), (0x2B, '\\'),
            (0x1E, 'a'), (0x1F, 's'), (0x20, 'd'), (0x21, 'f'), (0x22, 'g'), (0x23, 'h'), (0x24, 'j'),
            (0x25, 'k'), (0x26, 'l'), (0x27, ';'), (0x28, '\''),
            (0x2C, 'z'), (0x2D, 'x'), (0x2E, 'c'), (0x2F, 'v'), (0x30, 'b'), (0x31, 'n'), (0x32, 'm'),
            (0x33, ','), (0x34, '.'), (0x35, '/'), (0x56, '\\'),
        ])
    }

    /// French (France) AZERTY. The dead key ^ (0x1A) is absent: Windows reports it as a dead
    /// key, which is never used.
    pub fn french_azerty() -> Vec<KeyChar> {
        table(&[
            (0x29, '²'), (0x02, '&'), (0x03, 'é'), (0x04, '"'), (0x05, '\''), (0x06, '('), (0x07, '-'),
            (0x08, 'è'), (0x09, '_'), (0x0A, 'ç'), (0x0B, 'à'), (0x0C, ')'), (0x0D, '='),
            (0x10, 'a'), (0x11, 'z'), (0x12, 'e'), (0x13, 'r'), (0x14, 't'), (0x15, 'y'), (0x16, 'u'),
            (0x17, 'i'), (0x18, 'o'), (0x19, 'p'), (0x1B, '$'), (0x2B, '*'),
            (0x1E, 'q'), (0x1F, 's'), (0x20, 'd'), (0x21, 'f'), (0x22, 'g'), (0x23, 'h'), (0x24, 'j'),
            (0x25, 'k'), (0x26, 'l'), (0x27, 'm'), (0x28, 'ù'),
            (0x2C, 'w'), (0x2D, 'x'), (0x2E, 'c'), (0x2F, 'v'), (0x30, 'b'), (0x31, 'n'), (0x32, ','),
            (0x33, ';'), (0x34, ':'), (0x35, '!'), (0x56, '<'),
        ])
    }

    /// Hebrew (0x040D040D), unshifted level.
    pub fn hebrew() -> Vec<KeyChar> {
        table(&[
            (0x29, ';'), (0x02, '1'), (0x03, '2'), (0x04, '3'), (0x05, '4'), (0x06, '5'), (0x07, '6'),
            (0x08, '7'), (0x09, '8'), (0x0A, '9'), (0x0B, '0'), (0x0C, '-'), (0x0D, '='),
            (0x10, '/'), (0x11, '\''), (0x12, 'ק'), (0x13, 'ר'), (0x14, 'א'), (0x15, 'ט'), (0x16, 'ו'),
            (0x17, 'ן'), (0x18, 'ם'), (0x19, 'פ'), (0x1A, ']'), (0x1B, '['), (0x2B, '\\'),
            (0x1E, 'ש'), (0x1F, 'ד'), (0x20, 'ג'), (0x21, 'כ'), (0x22, 'ע'), (0x23, 'י'), (0x24, 'ח'),
            (0x25, 'ל'), (0x26, 'ך'), (0x27, 'ף'), (0x28, ','),
            (0x2C, 'ז'), (0x2D, 'ס'), (0x2E, 'ב'), (0x2F, 'ה'), (0x30, 'נ'), (0x31, 'מ'), (0x32, 'צ'),
            (0x33, 'ת'), (0x34, 'ץ'), (0x35, '.'), (0x56, '\\'),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn azerty() -> KeyMap {
        build_keymap(LatinKind::FrenchAzerty, &reference::french_azerty(), &reference::hebrew()).expect("AZERTY map")
    }

    #[test]
    fn hkl_classification() {
        assert_eq!(classify_hkl(0x040D_040D), LayoutKind::HebrewStandard);
        assert_eq!(classify_hkl(0x0409_0409), LayoutKind::Latin(LatinKind::Us));
        assert_eq!(classify_hkl(0x040C_040C), LayoutKind::Latin(LatinKind::FrenchAzerty));
        for other in [0x080C_080Cu32, 0x0809_0809, 0xF002_040D, 0x0C0C_0C0C, 0x0407_0407, 0] {
            assert_eq!(classify_hkl(other), LayoutKind::Unsupported, "0x{other:08X}");
        }
        assert_eq!(hkl_name(0x0809_0809), "anglais (Royaume-Uni) (0x08090809)");
        assert_eq!(hkl_name(0x1234_5678), "clavier 0x12345678");
    }

    #[test]
    fn built_us_map_agrees_with_the_fixed_table_on_letters() {
        let m = build_keymap(LatinKind::Us, &reference::us(), &reference::hebrew()).expect("US map");
        let fixed = crate::convert::us_map();
        for c in 'a'..='z' {
            assert_eq!(m.latin_to_he(c), fixed.latin_to_he(c), "letter {c}");
        }
        for h in '\u{05D0}'..='\u{05EA}' {
            assert_eq!(m.he_to_latin(h), fixed.he_to_latin(h), "Hebrew {h}");
        }
    }

    #[test]
    fn azerty_pairs_follow_the_physical_keys() {
        let m = azerty();
        // physical Q key: AZERTY a, Hebrew /
        assert_eq!(m.latin_to_he('a'), Some('/'));
        assert_eq!(m.latin_to_he('z'), Some('\''));
        assert_eq!(m.latin_to_he('q'), Some('ש'));
        assert_eq!(m.latin_to_he('w'), Some('ז'));
        assert_eq!(m.latin_to_he('m'), Some('ף'));
        assert_eq!(m.latin_to_he(';'), Some('ת'));
        assert_eq!(m.latin_to_he(':'), Some('ץ'));
        assert_eq!(m.latin_to_he(','), Some('צ'));
        assert_eq!(m.latin_to_he('ù'), Some(','));
        assert_eq!(m.he_to_latin('ש'), Some('q'));
        assert_eq!(m.he_to_latin(','), Some('ù'));
        assert_eq!(m.latin_to_he('Q'), Some('ש'), "upper case");
        assert_eq!(m.he_to_latin('ש'), Some('q'), "reverse gives the lower case");
    }

    #[test]
    fn azerty_digits_dead_keys_and_symbols_are_never_mapped() {
        let m = azerty();
        for c in ['é', 'è', 'ç', 'à', '&', '"', '(', '-', '_', ')', '=', '²', '$', '*', '!', '<', '^'] {
            assert_eq!(m.latin_to_he(c), None, "{c} must stay as it is");
        }
        for d in '0'..='9' {
            assert_eq!(m.he_to_latin(d), None, "digit {d} must stay");
            assert_eq!(m.latin_to_he(d), None);
        }
    }

    #[test]
    fn a_layout_missing_a_hebrew_letter_is_refused() {
        let mut he = reference::hebrew();
        he.retain(|k| k.ch != 'ש');
        assert!(matches!(build_keymap(LatinKind::FrenchAzerty, &reference::french_azerty(), &he), Err(MapError::Incomplete(_))));
        // two Hebrew letters on one key would make the reverse ambiguous
        let mut fr = reference::french_azerty();
        for k in fr.iter_mut() {
            if k.sc == 0x1F {
                k.ch = 'q'; // s replaced by a second q
            }
        }
        assert!(build_keymap(LatinKind::FrenchAzerty, &fr, &reference::hebrew()).is_err());
    }

    #[test]
    fn hebrew_variants_and_latin_layouts_are_recognised_by_their_letters() {
        let mut si1452 = reference::hebrew();
        for k in si1452.iter_mut() {
            if k.sc == 0x10 {
                k.ch = '\u{05F3}'; // a sign moved: letters unchanged
            }
        }
        assert!(same_hebrew_letters(&si1452));
        let mut swapped = reference::hebrew();
        for k in swapped.iter_mut() {
            match k.ch {
                'ש' => k.ch = 'ד',
                'ד' => k.ch = 'ש',
                _ => {}
            }
        }
        assert!(!same_hebrew_letters(&swapped), "letters moved: not the same layout");
        assert!(is_latin_table(&reference::us()));
        assert!(is_latin_table(&reference::french_azerty()));
        assert!(!is_latin_table(&reference::hebrew()));
        let russian: Vec<KeyChar> = reference::us().iter().map(|k| KeyChar { sc: k.sc, ch: if k.ch.is_ascii_alphabetic() { 'ж' } else { k.ch } }).collect();
        assert!(!is_latin_table(&russian));
    }

    #[test]
    fn qwertz_gets_its_own_table() {
        assert!(qwerty_letters(&reference::us()));
        let mut de = reference::us();
        for k in de.iter_mut() {
            k.ch = match k.ch { 'y' => 'z', 'z' => 'y', c => c };
        }
        assert!(!qwerty_letters(&de));
        assert!(!qwerty_letters(&reference::french_azerty()));
        let m = build_keymap(LatinKind::OtherFromWindows, &de, &reference::hebrew()).unwrap();
        // a fuller German table (kbdgr, unshifted): ß ü + ö ä # - and the dead keys ^ ´ left out
        let mut gr: Vec<KeyChar> = reference::us().into_iter().filter(|k| k.sc != 0x29 && k.sc != 0x0D).collect();
        for k in gr.iter_mut() {
            k.ch = match k.sc { 0x15 => 'z', 0x2C => 'y', 0x0C => 'ß', 0x1A => 'ü', 0x1B => '+', 0x27 => 'ö', 0x28 => 'ä', 0x2B => '#', 0x35 => '-', _ => k.ch };
        }
        let g = build_keymap(LatinKind::OtherFromWindows, &gr, &reference::hebrew()).unwrap();
        assert_eq!(g.latin_to_he('z'), Some('ט'));
        assert_eq!(g.latin_to_he('m'), Some('צ'));
        // digits never paired (the Hebrew layout has digits there too)
        assert!(g.pairs().iter().all(|(l, h)| !l.is_ascii_digit() && !h.is_ascii_digit()));
        // the key of US "y" (Hebrew ט) gives "z" on QWERTZ
        assert_eq!(m.latin_to_he('z'), Some('ט'));
        assert_eq!(m.latin_to_he('y'), Some('ז'));
        assert_eq!(m.he_to_latin('ש'), Some('a'));
    }

}
