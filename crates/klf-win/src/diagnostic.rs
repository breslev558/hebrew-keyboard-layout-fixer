//! The last F10/F9 attempts, in memory only (never written to disk), and the diagnostic report.

use klf_core::diag::{Attempt, Environment, Ring, Trace};
use klf_core::flow::Outcome;
use klf_core::settings::Action;
use std::sync::Mutex;

static RING: Mutex<Option<Ring>> = Mutex::new(None);

/// Notes one attempt (worker thread, after the run).
pub fn record(action: Action, outcome: &Outcome, mut trace: Trace, now_ms: u64, layout: Option<u32>) {
    let (stopped_at, total_ms) = trace.finish(now_ms);
    let info = trace.info.take().unwrap_or_default();
    let a = Attempt {
        when: crate::platform::sys::local_clock(),
        action: Some(action),
        msg: outcome.msg,
        log: outcome.log.clone(),
        stopped_at,
        total_ms,
        steps: std::mem::take(&mut trace.steps),
        exe: info.exe_name,
        window_class: info.window_class,
        focus_class: info.focus_class,
        selection: klf_core::diag::selection_text(info.selection),
        chars: trace.chars,
        layout,
        latin: trace.latin,
    };
    let mut g = RING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    g.get_or_insert_with(Ring::default).push(a);
}

/// A copy of the last attempt (for an anonymous report: klf-core/report.rs keeps only fixed fields).
pub fn last_attempt() -> Option<Attempt> {
    let g = RING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    g.as_ref().and_then(|r| r.last()).cloned()
}

/// The last attempt: time, bubble message, plain explanation (None: none since the start).
pub fn last() -> Option<(String, Option<klf_core::i18n::Msg>, String)> {
    let g = RING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    g.as_ref().and_then(|r| r.last()).map(|a| (a.when.clone(), a.msg, klf_core::diag::explain(a, crate::app::lang())))
}

fn state_text(registered: bool, paused: bool, fr: bool) -> String {
    match (paused, registered, fr) {
        (true, _, true) => "en pause",
        (true, _, false) => "paused",
        (false, true, true) => "enregistrée",
        (false, true, false) => "registered",
        (false, false, true) => "occupée par un autre programme",
        (false, false, false) => "used by another program",
    }
    .into()
}

/// The full report (UI thread: reads the settings and the hotkey states).
pub fn report() -> String {
    let s = crate::app::settings();
    let paused = crate::app::hotkey_state() == crate::app::HotkeyState::Paused;
    let ready = crate::app::hotkey_state() == crate::app::HotkeyState::Ready;
    // 1.9.1: French, or English for the Hebrew and English interfaces (technical text, left to right)
    let lang = crate::app::lang();
    let fr = lang == klf_core::i18n::Lang::Fr;
    let t = |f: &str, e: &str| -> String { (if fr { f } else { e }).to_string() };
    let off = t("coupée", "off");
    let case_key = if s.case_hotkey_key == klf_core::settings::KEY_NONE { off.clone() } else { s.case_hotkey_description() };
    let env = Environment {
        version: klf_core::VERSION.to_string(),
        windows: crate::platform::sys::windows_version(),
        ui_language: format!("{:?}", crate::app::lang()),
        layouts: crate::platform::layouts::installed(),
        hotkey: s.hotkey_description(),
        hotkey_state: state_text(ready, paused, fr),
        case_hotkey: case_key,
        case_hotkey_state: if s.case_hotkey_key == klf_core::settings::KEY_NONE { off } else { state_text(crate::app::case_hotkey_ok(), paused, fr) },
        settings: vec![
            (t("sens", "direction"), format!("{:?}", s.mixed_text_mode)),
            (t("mode prudent", "careful mode"), s.require_selection.to_string()),
            (t("éteindre Verr. Maj après F9", "Caps Lock off after F9"), s.turn_off_caps_lock.to_string()),
            (t("console expérimentale", "experimental console"), s.console_support.to_string()),
            (t("changer la langue après", "switch language after"), s.switch_language_after_convert.to_string()),
            (t("limite de caractères", "character limit"), s.max_character_limit.to_string()),
            (t("délai avant remise (ms)", "delay before restore (ms)"), s.paste_delay_ms.to_string()),
            (t("programmes exclus", "excluded programs"), s.excluded_processes.len().to_string()),
            (t("dernier mot (essai)", "last word (trial)"), s.last_word_only.to_string()),
            (t("indicateur HE/EN", "HE/EN indicator"), s.indicator.to_string()),
            (t("règles par programme", "rules per program"), s.app_rules.len().to_string()),
        ]
        .into_iter()
        .chain(klf_core::school::report_lines(&crate::app::policy()).into_iter().map(|l| (t("imposé par l'administrateur", "enforced by the administrator"), l)))
        .collect(),
    };
    let g = RING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let empty = Ring::default();
    klf_core::diag::report_in(&env, g.as_ref().unwrap_or(&empty), lang)
}
