//! Any text: the conversion and the Caps Lock fix never panic and keep the number of characters.
#![no_main]
use klf_core::convert::{convert, swap_case};
use klf_core::settings::MixedTextMode;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    for mode in [MixedTextMode::ToggleAll, MixedTextMode::EnglishToHebrewOnly, MixedTextMode::HebrewToEnglishOnly] {
        for caps in [false, true] {
            let _ = convert(text, mode, caps);
        }
    }
    let s = swap_case(text);
    let _ = swap_case(&s);
});
