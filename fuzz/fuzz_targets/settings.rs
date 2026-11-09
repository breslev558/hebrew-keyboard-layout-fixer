//! Any settings or profile file: reading it never panics, and what is read is always valid
//! (saved again and read back identically).
#![no_main]
use klf_core::bubble::preview_text;
use klf_core::profile::import;
use klf_core::settings::AppSettings;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    let (s, _) = AppSettings::from_json(data);
    let (again, ok) = AppSettings::from_json(&s.to_json());
    assert!(ok, "our own file must be readable");
    assert_eq!(s.to_json(), again.to_json(), "saved settings change when read back");
    let _ = import(data);
    let (a, b) = data.split_at(data.char_indices().nth(data.chars().count() / 2).map_or(0, |(i, _)| i));
    let p = preview_text(a, b, 24);
    assert!(!p.contains(['\u{202e}', '\u{202d}']), "bidi override left in the bubble");
});
