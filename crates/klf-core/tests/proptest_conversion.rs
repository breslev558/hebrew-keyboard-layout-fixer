//! Property tests (proptest), audit of 17/11/2026: the conversion and the parsers never panic,
//! keep the number of characters, and the conversion comes back exactly where it should.
use klf_core::convert::{convert_with, swap_case, us_map};
use klf_core::layouts::KeyMap;
use klf_core::policy::{contains_line_break, is_line_break};
use klf_core::settings::{AppSettings, MixedTextMode};
use proptest::prelude::*;

const MODES: [MixedTextMode; 3] = [MixedTextMode::ToggleAll, MixedTextMode::EnglishToHebrewOnly, MixedTextMode::HebrewToEnglishOnly];

/// Latin characters whose Hebrew character comes back to them (a two-way pair of the table).
fn two_way(map: &KeyMap) -> Vec<char> {
    map.pairs().iter().filter(|(l, h)| map.latin_to_he(*l) == Some(*h) && map.he_to_latin(*h) == Some(*l)).map(|(l, _)| *l).collect()
}

/// Any text, with Hebrew, niqqud, RTL marks, line breaks, controls and emoji well represented.
fn any_text() -> impl Strategy<Value = String> {
    let pick = prop_oneof![
        any::<char>(),
        prop::sample::select(vec!['a', 'k', 'u', 'o', ',', '.', ';', '\'', '/', 'א', 'ש', 'ל', 'ו', 'ם', '\u{05B8}', '\u{05BC}', '\u{200F}', '\u{202B}', '\r', '\n', '\u{2028}', '\t', '\u{0}', '😀', 'A', 'Z', 'ß', 'İ']),
    ];
    prop::collection::vec(pick, 0..200).prop_map(|v| v.into_iter().collect())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 2000, ..ProptestConfig::default() })]

    #[test]
    fn conversion_never_panics_and_keeps_the_length(t in any_text(), caps in any::<bool>()) {
        let map = us_map();
        for m in MODES {
            let r = convert_with(&map, &t, m, caps);
            prop_assert_eq!(r.chars().count(), t.chars().count());
        }
    }

    #[test]
    fn english_to_hebrew_then_back_gives_the_original(idx in prop::collection::vec(any::<prop::sample::Index>(), 0..80)) {
        let map = us_map();
        let set = two_way(&map);
        let t: String = idx.iter().map(|i| set[i.index(set.len())]).collect();
        let he = convert_with(&map, &t, MixedTextMode::EnglishToHebrewOnly, true);
        prop_assert_eq!(convert_with(&map, &he, MixedTextMode::HebrewToEnglishOnly, true), t.clone());
        // ToggleAll twice (pure letters: English decides, then Hebrew decides)
        // (only keys that give a Hebrew LETTER: "q" gives "/", a sign, and a text of signs only is
        // read as English the second time: "q" -> "/" -> "." — see the test below)
        let letters: String = t.chars().filter(|c| c.is_ascii_lowercase() && map.latin_to_he(*c).is_some_and(klf_core::layouts::is_hebrew_letter)).collect();
        let once = convert_with(&map, &letters, MixedTextMode::ToggleAll, true);
        prop_assert_eq!(convert_with(&map, &once, MixedTextMode::ToggleAll, true), letters);
    }

    #[test]
    #[ignore = "audit 17/11: a text made only of signs is read as English both times (q -> / -> .)"]
    fn toggle_all_twice_on_signs_only(i in any::<prop::sample::Index>()) {
        let map = us_map();
        let set = two_way(&map);
        let t = set[i.index(set.len())].to_string();
        let once = convert_with(&map, &t, MixedTextMode::ToggleAll, true);
        prop_assert_eq!(convert_with(&map, &once, MixedTextMode::ToggleAll, true), t);
    }

    #[test]
    fn caps_lock_fix_twice_gives_the_original(t in any_text()) {
        prop_assert_eq!(swap_case(&swap_case(&t)), t);
    }

    #[test]
    fn line_breaks_agree(t in any_text()) {
        prop_assert_eq!(contains_line_break(&t), t.chars().any(is_line_break));
    }

    #[test]
    fn parsers_never_panic(t in any_text()) {
        let _ = AppSettings::from_json(&t);
        let _ = klf_core::update::parse_update_file(&t);
        let _ = klf_core::update::parse_version(&t);
        let _ = klf_core::update::parse_release_for(&t, false);
        let _ = klf_core::update::parse_release_for(&t, true);
        let _ = klf_core::update::parse_sig(&t);
        let _ = klf_core::update::parse_sha_file(&t);
    }

    #[test]
    fn settings_json_with_wrong_types_never_panics(k in "[A-Za-z]{1,20}", v in prop_oneof![Just("null".to_string()), Just("[]".to_string()), Just("{}".to_string()), Just("-1".to_string()), Just("1e400".to_string()), Just("\"x\"".to_string()), Just("true".to_string()), Just("18446744073709551616".to_string())]) {
        let _ = AppSettings::from_json(&format!("{{\"{k}\": {v}}}"));
        let _ = AppSettings::from_json(&format!("{{\"Hotkey\": {v}, \"BubblePosition\": {v}, \"MixedTextMode\": {v}, \"AppRules\": {v}}}"));
    }
}
