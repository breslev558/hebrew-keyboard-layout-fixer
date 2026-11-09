//! Anonymous problem reports (1.6.1, PUBLIC version only, always with the user's consent unless
//! they chose "always send"). Pure and tested: what is sent is built HERE from a fixed list of
//! technical fields — never the typed text, never the clipboard, no identifier.

use crate::diag::Attempt;
use crate::i18n::Msg;

/// Where reports go (a small Cloudflare Worker, separate from any server of the project).
pub const REPORT_URL: &str = "https://klf-rapports.breslev558.workers.dev/rapport";
pub const MAX_COMMENT: usize = 500;
pub const MAX_BYTES: usize = 4096;
/// ReportMode: 0 = ask (default), 1 = always send without asking, 2 = never ask.
pub const ASK: u32 = 0;
pub const ALWAYS: u32 = 1;
pub const NEVER: u32 = 2;

/// Results worth a report: something went wrong on our side. Not a normal refusal, nor a limit
/// (clipboard too big) or keys held by another program.
pub fn reportable(msg: Option<Msg>) -> bool {
    matches!(msg, Some(Msg::ClipboardNotRestored | Msg::PasteFailed))
}

/// What to do with the answer of the receiver (None: network problem).
#[derive(Debug, PartialEq, Eq)]
pub enum Sent {
    Ok,
    /// refused by the receiver (4xx): never sent again
    Refused,
    /// network or server problem: kept for a new try from the menu
    Retry,
}

pub fn sent(status: Option<u32>) -> Sent {
    match status {
        Some(200..=299) => Sent::Ok,
        Some(400..=499) => Sent::Refused,
        _ => Sent::Retry,
    }
}

/// A crash is offered at most once a day (a program crashing at every start must not flood).
pub fn crash_offer_due(last_day: Option<u64>, today: u64) -> bool {
    last_day != Some(today)
}

/// A short technical name: letters, digits and . _ - only, at most `max` characters.
fn clean(s: &str, max: usize) -> String {
    let s = s.rsplit(['\\', '/']).next().unwrap_or("");
    s.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')).take(max).collect()
}

/// The JSON sent. `kind`: "erreur", "plantage" or "signalement"; `comment`: written by the user
/// in "Report a problem" (shown to them before sending), else empty.
pub fn build(kind: &str, version: &str, windows: &str, a: Option<&Attempt>, comment: &str) -> String {
    let mut v = serde_json::json!({
        "kind": clean(kind, 16),
        "version": clean(version, 24),
        "windows": clean(windows, 32),
    });
    if let Some(a) = a {
        v["msg"] = serde_json::Value::String(a.msg.map(|m| format!("{m:?}")).unwrap_or_default());
        v["action"] = serde_json::Value::String(a.action.map(|x| format!("{x:?}")).unwrap_or_default());
        v["step"] = serde_json::Value::String(a.stopped_at.map(|s| format!("{s:?}")).unwrap_or_default());
        v["total_ms"] = a.total_ms.into();
        v["steps"] = a.steps.iter().take(16).map(|(s, ms)| serde_json::json!([format!("{s:?}"), ms])).collect::<Vec<_>>().into();
        v["exe"] = clean(&a.exe, 64).into();
        v["window_class"] = clean(&a.window_class, 64).into();
        v["focus_class"] = clean(&a.focus_class, 64).into();
        v["selection"] = a.selection.into();
        v["chars"] = a.chars.map(|c| c as u64).into();
    }
    let c: String = comment.chars().filter(|c| !c.is_control() || *c == '\n').take(MAX_COMMENT).collect();
    if !c.trim().is_empty() {
        v["comment"] = c.trim().into();
    }
    let s = v.to_string();
    if s.len() > MAX_BYTES {
        // never larger than the limit: the comment is dropped first
        v.as_object_mut().map(|o| o.remove("comment"));
        return v.to_string();
    }
    s
}

/// After a crash of the previous session: only where it stopped in our code ("host.rs:123",
/// written by the panic hook), never the panic message.
pub fn build_crash(version: &str, windows: &str, location: &str) -> String {
    let mut v: serde_json::Value = serde_json::from_str(&build("plantage", version, windows, None, "")).unwrap_or_default();
    v["step"] = clean(&location.replace(':', "-"), 80).into();
    v.to_string()
}

/// The report as shown to the user before sending (one field per line, Windows line ends).
pub fn pretty(json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(json).ok().and_then(|v| serde_json::to_string_pretty(&v).ok()).unwrap_or_default().replace('\n', "\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diag::{Attempt, Step};

    fn attempt() -> Attempt {
        Attempt {
            msg: Some(Msg::ClipboardNotRestored),
            log: "Ctrl+V sent with 4 converted characters SECRET-TEXT".into(),
            stopped_at: Some(Step::Restore),
            total_ms: 1234,
            steps: vec![(Step::Copy, 40), (Step::Restore, 900)],
            exe: r"C:\Users\Moshe\AppData\winword.exe".into(),
            window_class: "OpusApp".into(),
            focus_class: "_WwG".into(),
            selection: "sélection",
            chars: Some(4),
            ..Default::default()
        }
    }

    #[test]
    fn only_fixed_technical_fields_are_sent() {
        let j = build("erreur", "1.6.1", "10.0.26100", Some(&attempt()), "");
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["msg"], "ClipboardNotRestored");
        assert_eq!(v["step"], "Restore");
        assert_eq!(v["exe"], "winword.exe", "program name only, never the path");
        assert!(!j.contains("SECRET-TEXT"), "the log line is never sent");
        assert!(!j.contains("Moshe") && !j.contains("Users"), "no path, no user name: {j}");
        assert!(v.get("comment").is_none());
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        for k in keys {
            assert!(["kind", "version", "windows", "msg", "action", "step", "total_ms", "steps", "exe", "window_class", "focus_class", "selection", "chars"].contains(&k.as_str()), "unexpected field {k}");
        }
    }

    #[test]
    fn comment_is_kept_short_and_the_size_is_capped() {
        let j = build("signalement", "1.6.1", "10", None, "  ça ne marche pas dans Word  ");
        assert_eq!(serde_json::from_str::<serde_json::Value>(&j).unwrap()["comment"], "ça ne marche pas dans Word");
        let long = "x".repeat(5000);
        let j = build("signalement", "1.6.1", "10", None, &long);
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["comment"].as_str().unwrap().chars().count(), MAX_COMMENT);
        assert!(j.len() <= MAX_BYTES);
    }

    #[test]
    fn a_crash_report_says_only_where_it_stopped() {
        let j = build_crash("1.6.1", "10.0.22631", "crates\\klf-win\\src\\host.rs:812:9");
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["kind"], "plantage");
        assert_eq!(v["step"], "host.rs-812-9");
        assert!(pretty(&j).contains("\r\n  \"kind\": \"plantage\""));
    }

    #[test]
    fn which_results_ask_for_a_report() {
        assert!(reportable(Some(Msg::ClipboardNotRestored)));
        assert!(!reportable(Some(Msg::Converted)));
        assert!(!reportable(Some(Msg::PasswordField)), "a normal refusal is not a bug");
        assert!(!reportable(None));
        assert!(!reportable(Some(Msg::ClipboardTooBig)), "a limit, not a bug");
    }

    #[test]
    fn receiver_answers() {
        assert_eq!(sent(Some(204)), Sent::Ok);
        assert_eq!(sent(Some(413)), Sent::Refused, "a refused report is not tried again");
        assert_eq!(sent(Some(503)), Sent::Retry);
        assert_eq!(sent(None), Sent::Retry);
        assert!(crash_offer_due(None, 20400) && crash_offer_due(Some(20399), 20400));
        assert!(!crash_offer_due(Some(20400), 20400));
    }
}
