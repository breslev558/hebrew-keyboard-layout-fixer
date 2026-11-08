//! Hebrew <-> English keyboard layout conversion (standard Windows Hebrew layout + US English).
//! Ported 1:1 from KeyboardLayoutConverter.cs (1.2.9-noam5), same behaviour, same tests.

use crate::layouts::{KeyMap, LatinKind};
use crate::settings::MixedTextMode;

/// English key -> Hebrew character (the order matters for the reverse table:
/// the FIRST English key wins, e.g. 'ת' -> ',' and not another key).
const EN_TO_HE: &[(char, char)] = &[
    ('q', '/'), ('Q', '/'),
    ('w', '\''), ('W', '\''),
    ('e', 'ק'), ('E', 'ק'),
    ('r', 'ר'), ('R', 'ר'),
    ('t', 'א'), ('T', 'א'),
    ('y', 'ט'), ('Y', 'ט'),
    ('u', 'ו'), ('U', 'ו'),
    ('i', 'ן'), ('I', 'ן'),
    ('o', 'ם'), ('O', 'ם'),
    ('p', 'פ'), ('P', 'פ'),
    ('a', 'ש'), ('A', 'ש'),
    ('s', 'ד'), ('S', 'ד'),
    ('d', 'ג'), ('D', 'ג'),
    ('f', 'כ'), ('F', 'כ'),
    ('g', 'ע'), ('G', 'ע'),
    ('h', 'י'), ('H', 'י'),
    ('j', 'ח'), ('J', 'ח'),
    ('k', 'ל'), ('K', 'ל'),
    ('l', 'ך'), ('L', 'ך'),
    ('z', 'ז'), ('Z', 'ז'),
    ('x', 'ס'), ('X', 'ס'),
    ('c', 'ב'), ('C', 'ב'),
    ('v', 'ה'), ('V', 'ה'),
    ('b', 'נ'), ('B', 'נ'),
    ('n', 'מ'), ('N', 'מ'),
    ('m', 'צ'), ('M', 'צ'),
    // punctuation
    (',', 'ת'),
    ('.', 'ץ'),
    ('/', '.'),
    (';', 'ף'),
    ('\'', ','),
    ('[', ']'),
    (']', '['),
    ('-', '-'),
    ('=', '='),
    ('`', ';'), // NOAM : Hebrew layout gives ';' on the ` key
];

/// The fixed US <-> Hebrew map (the proven table above).
pub fn us_map() -> KeyMap {
    KeyMap::from_pairs(LatinKind::Us, EN_TO_HE.to_vec())
}

/// Latin-script letter (ASCII and accented: é, ç... for AZERTY), counted for the majority rule.
fn is_latin_letter(c: char) -> bool {
    c.is_ascii_alphabetic() || (('\u{00C0}'..='\u{024F}').contains(&c) && c.is_alphabetic())
}

fn is_hebrew_block(c: char) -> bool {
    ('\u{0590}'..='\u{05FF}').contains(&c)
}

/// Hebrew LETTERS only (not the niqqud kept after a conversion): decides which keyboard to
/// switch to after converting (audit: "שָׁלוֹם" -> "aָׁkuֹo" is English).
pub fn contains_hebrew(text: &str) -> bool {
    text.chars().any(|c| ('\u{05D0}'..='\u{05EA}').contains(&c) || ('\u{05F0}'..='\u{05F2}').contains(&c))
}

/// Converts the text with the US table. Characters without a mapping (digits, emoji, niqqud,
/// spaces...) are kept.
pub fn convert(text: &str, mode: MixedTextMode, replace_caps: bool) -> String {
    convert_with(&us_map(), text, mode, replace_caps)
}

/// Converts the text with a key map (US fixed table, or French AZERTY built from Windows).
pub fn convert_with(map: &KeyMap, text: &str, mode: MixedTextMode, replace_caps: bool) -> String {
    // ToggleAll: the majority language decides the direction of ambiguous signs (' / . , ; ...)
    let predominantly_english = if mode == MixedTextMode::ToggleAll {
        let en = text.chars().filter(|c| is_latin_letter(*c)).count();
        let he = text.chars().filter(|c| is_hebrew_block(*c)).count();
        en >= he
    } else {
        false
    };

    text.chars()
        .map(|c| {
            if !replace_caps && c.is_uppercase() && !is_hebrew_block(c) {
                return c;
            }
            match mode {
                MixedTextMode::ToggleAll => match (map.latin_to_he(c), map.he_to_latin(c)) {
                    (Some(h), Some(e)) => {
                        if predominantly_english {
                            h
                        } else {
                            e
                        }
                    }
                    (Some(h), None) => h,
                    (None, Some(e)) => e,
                    (None, None) => c,
                },
                MixedTextMode::EnglishToHebrewOnly => map.latin_to_he(c).unwrap_or(c),
                MixedTextMode::HebrewToEnglishOnly => map.he_to_latin(c).unwrap_or(c),
            }
        })
        .collect()
}

/// Text typed with Caps Lock on: every Latin letter changes case ("hELLO wORLD" -> "Hello
/// World"). Pressing again on the same (re-selected) text undoes it. A letter is changed only if it becomes ONE character and
/// comes back exactly: ß, İ, ı, ſ... stay as they are. Hebrew, digits and signs have
/// no case and are kept, so mixed text is safe.
pub fn swap_case(text: &str) -> String {
    text.chars().map(swap_char).collect()
}

/// Latin letters handled by F9 (ASCII, Latin-1, Latin Extended-A/B).
fn in_scope(c: char) -> bool {
    c.is_ascii_alphabetic() || ('\u{00C0}'..='\u{024F}').contains(&c)
}

fn swap_char(c: char) -> char {
    if !in_scope(c) {
        return c;
    }
    // exactly one character, and the reverse gives back exactly `c`
    fn single(mut it: impl Iterator<Item = char>) -> Option<char> {
        let x = it.next()?;
        it.next().is_none().then_some(x)
    }
    let swapped = if c.is_lowercase() {
        single(c.to_uppercase()).filter(|x| single(x.to_lowercase()) == Some(c))
    } else if c.is_uppercase() {
        single(c.to_lowercase()).filter(|x| single(x.to_uppercase()) == Some(c))
    } else {
        None
    };
    // the partner must be handled too, or the second press could not come back (audit 1.3.5, N2)
    swapped.filter(|x| in_scope(*x)).unwrap_or(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn t(s: &str) -> String {
        convert(s, MixedTextMode::ToggleAll, true)
    }

    #[test]
    fn same_results_as_csharp_tests() {
        assert_eq!(t("akuo"), "שלום");
        assert_eq!(t("שלום"), "akuo");
        assert_eq!(t("hello"), "יקךךם");
        assert_eq!(convert("Hello", MixedTextMode::ToggleAll, false), "Hקךךם");
        assert_eq!(t("akuo 123 😀"), "שלום 123 😀");
        assert_eq!(t("יקךךם 'םרךג"), "hello world");
        assert_eq!(t("akuo' nv"), "שלום, מה");
        assert_eq!(convert("akuo שלום", MixedTextMode::EnglishToHebrewOnly, true), "שלום שלום");
        assert_eq!(convert("akuo שלום", MixedTextMode::HebrewToEnglishOnly, true), "akuo akuo");
        assert_eq!(t(""), "");
        // known limit, documented: converting twice is not an undo for punctuation alone
        assert_eq!(t("q"), "/");
        assert_eq!(t("/"), ".");
        assert_eq!(t(&t("akuo")), "akuo");
    }

    #[test]
    fn niqqud_digits_parentheses_newlines_kept() {
        // niqqud (U+05B0..U+05C7) has no key mapping: kept as is
        assert_eq!(t("שָׁלוֹם"), "aָׁkuֹo");
        assert_eq!(t("(akuo)"), "(שלום)");
        assert_eq!(t("akuo\r\nakuo"), "שלום\r\nשלום");
        assert_eq!(t("2026"), "2026");
    }

    #[test]
    fn every_letter_round_trips() {
        // q and w give / and ' (the known punctuation limit, see above)
        for c in ('a'..='z').filter(|c| *c != 'q' && *c != 'w') {
            let s = c.to_string();
            assert_eq!(t(&t(&s)), s, "letter {c}");
        }
    }

    #[test]
    fn brackets_swapped_backtick() {
        assert_eq!(t("[a]"), "]ש[");
        assert_eq!(t("`a"), ";ש");
    }

    #[test]
    fn hebrew_letters_decide_the_keyboard() {
        assert!(contains_hebrew("שלום"));
        assert!(!contains_hebrew("a\u{05B8}\u{05C1}ku\u{05B9}o"), "niqqud alone is not Hebrew text");
    }

    #[test]
    fn caps_lock_text_is_swapped_and_swapping_twice_gives_it_back() {
        assert_eq!(swap_case("hELLO wORLD"), "Hello World");
        assert_eq!(swap_case("HELLO"), "hello");
        assert_eq!(swap_case("שלום aBC 123 !?"), "שלום Abc 123 !?", "Hebrew, digits, signs kept");
        assert_eq!(swap_case("éCOLE"), "École");
        for t in ["Straße", "İstanbul", "ıi", "ſ", "\u{212A}elvin", "שָׁלוֹם", "NASA iPhone"] {
            assert_eq!(swap_case(&swap_case(t)), t, "{t}");
        }
        assert_eq!(swap_case("ß"), "ß");
        assert_eq!(swap_case("İ"), "İ");
    }

    /// Audit 1.3.5 (N2): pressing F9 twice gives back EVERY character.
    #[test]
    fn swap_twice_restores_every_unicode_scalar() {
        let mut failures = Vec::new();
        for n in 0..=0x10ffff {
            let Some(c) = char::from_u32(n) else { continue };
            let original = c.to_string();
            let once = swap_case(&original);
            let twice = swap_case(&once);
            if twice != original {
                failures.push(format!("U+{n:04X} {original} -> {once} -> {twice}"));
            }
        }
        assert!(failures.is_empty(), "{} non-reversible characters: {}", failures.len(), failures.join(", "));
    }

    #[test]
    fn azerty_words_convert_both_ways() {
        use crate::layouts::{build_keymap, reference};
        let m = build_keymap(LatinKind::FrenchAzerty, &reference::french_azerty(), &reference::hebrew()).unwrap();
        let t = |s: &str| convert_with(&m, s, MixedTextMode::ToggleAll, true);
        // "שלום" typed on AZERTY = physical keys A ; K U O -> q k u o in AZERTY? no: physical
        // keys of ש ל ו ם are A, K, U, O: AZERTY gives q, k, u, o
        assert_eq!(t("qkuo"), "שלום");
        assert_eq!(t("שלום"), "qkuo");
        // "bonjour" typed with the Hebrew layout: physical keys B O N J O U R
        assert_eq!(t("נםמחםור"), "bonjour");
        assert_eq!(t("bonjour"), "נםמחםור");
        // digits and accented letters without a Hebrew key stay
        assert_eq!(t("qkuo 2026 é"), "שלום 2026 é");
        assert_eq!(t(&t("qkuo")), "qkuo");
    }
}
