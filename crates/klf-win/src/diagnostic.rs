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

/// The last attempt: time, bubble message, plain explanation (None: none since the start).
pub fn last() -> Option<(String, Option<klf_core::i18n::Msg>, String)> {
    let g = RING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    g.as_ref().and_then(|r| r.last()).map(|a| (a.when.clone(), a.msg, klf_core::diag::explain(a)))
}

fn state_text(registered: bool, paused: bool) -> String {
    if paused {
        "en pause".into()
    } else if registered {
        "enregistrée".into()
    } else {
        "occupée par un autre programme".into()
    }
}

/// The full report (UI thread: reads the settings and the hotkey states).
pub fn report() -> String {
    let s = crate::app::settings();
    let paused = crate::app::hotkey_state() == crate::app::HotkeyState::Paused;
    let ready = crate::app::hotkey_state() == crate::app::HotkeyState::Ready;
    let case_key = if s.case_hotkey_key == klf_core::settings::KEY_NONE { "coupée".to_string() } else { s.case_hotkey_description() };
    let env = Environment {
        version: klf_core::VERSION.to_string(),
        windows: crate::platform::sys::windows_version(),
        ui_language: format!("{:?}", crate::app::lang()),
        layouts: crate::platform::layouts::installed(),
        hotkey: s.hotkey_description(),
        hotkey_state: state_text(ready, paused),
        case_hotkey: case_key,
        case_hotkey_state: if s.case_hotkey_key == klf_core::settings::KEY_NONE { "coupée".into() } else { state_text(crate::app::case_hotkey_ok(), paused) },
        settings: vec![
            ("sens".into(), format!("{:?}", s.mixed_text_mode)),
            ("mode prudent".into(), s.require_selection.to_string()),
            ("éteindre Verr. Maj après F9".into(), s.turn_off_caps_lock.to_string()),
            ("console expérimentale".into(), s.console_support.to_string()),
            ("changer la langue après".into(), s.switch_language_after_convert.to_string()),
            ("limite de caractères".into(), s.max_character_limit.to_string()),
            ("délai avant remise (ms)".into(), s.paste_delay_ms.to_string()),
            ("programmes exclus".into(), s.excluded_processes.len().to_string()),
            ("dernier mot (essai)".into(), s.last_word_only.to_string()),
            ("indicateur HE/EN".into(), s.indicator.to_string()),
            ("règles par programme".into(), s.app_rules.len().to_string()),
        ]
        .into_iter()
        .chain(klf_core::school::report_lines(&crate::app::policy()).into_iter().map(|l| ("imposé par l'administrateur".to_string(), l)))
        .collect(),
    };
    let g = RING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let empty = Ring::default();
    klf_core::diag::report(&env, g.as_ref().unwrap_or(&empty))
}
