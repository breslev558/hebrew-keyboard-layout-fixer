//! Export / import of the user's settings (1.5.0): a small JSON file without paths, names or
//! typed text. Import goes through the same tolerant reader as settings.json, shows the
//! differences first, and keeps a copy of the previous file (in klf-win).

use crate::school::Locked;
use crate::settings::AppSettings;

pub const FORMAT: &str = "KeyboardLayoutFixer-Profil";
pub const FORMAT_VERSION: u32 = 1;
/// Larger files are refused.
pub const MAX_BYTES: usize = 1024 * 1024;

pub fn export(user: &AppSettings, app_version: &str) -> String {
    let settings: serde_json::Value = serde_json::from_str(&user.to_json()).unwrap_or(serde_json::Value::Null);
    let v = serde_json::json!({ "Format": FORMAT, "FormatVersion": FORMAT_VERSION, "AppVersion": app_version, "Settings": settings });
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

/// Why an import is refused (shown in the interface language, audit A11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportError {
    TooBig,
    NotJson,
    UnknownFormat,
    /// a profile written by a newer version (FormatVersion higher than ours, audit A06)
    NewerFormat,
    Unreadable,
}

impl ImportError {
    pub fn msg(self) -> crate::i18n::Msg {
        use crate::i18n::Msg;
        match self {
            Self::TooBig => Msg::ImportTooBig,
            Self::NotJson => Msg::ImportNotJson,
            Self::UnknownFormat => Msg::ImportUnknownFormat,
            Self::NewerFormat => Msg::ImportNewerFormat,
            Self::Unreadable => Msg::ImportUnreadable,
        }
    }
}

/// A profile file, or a plain settings.json.
pub fn import(text: &str) -> Result<AppSettings, ImportError> {
    if text.len() > MAX_BYTES {
        return Err(ImportError::TooBig);
    }
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| ImportError::NotJson)?;
    let settings = match v.get("Format").and_then(|f| f.as_str()) {
        Some(FORMAT) => {
            match v.get("FormatVersion") {
                None => {}
                Some(n) => match n.as_u64() {
                    Some(n) if n <= u64::from(FORMAT_VERSION) => {}
                    Some(_) => return Err(ImportError::NewerFormat),
                    None => return Err(ImportError::Unreadable),
                },
            }
            v.get("Settings").cloned().ok_or(ImportError::Unreadable)?
        }
        Some(_) => return Err(ImportError::UnknownFormat),
        None => v,
    };
    if !settings.is_object() {
        return Err(ImportError::Unreadable);
    }
    let (s, ok) = AppSettings::from_json(&settings.to_string());
    if !ok {
        return Err(ImportError::Unreadable);
    }
    Ok(s)
}

/// Differences between the current settings and the imported ones, in plain French; a setting
/// imposed by the administrator is marked "(sans effet : géré par l'administrateur)".
pub fn differences(old: &AppSettings, new: &AppSettings, locked: &Locked) -> Vec<String> {
    let mut v = Vec::new();
    let mut add = |label: &str, a: String, b: String, l: bool| {
        if a != b {
            v.push(format!("{label} : {a} → {b}{}", if l { " (sans effet : géré par l'administrateur)" } else { "" }));
        }
    };
    add("touche langue", old.hotkey_description(), new.hotkey_description(), locked.hotkey);
    add("touche majuscules", old.case_hotkey_description(), new.case_hotkey_description(), locked.case_hotkey);
    add("majuscules converties (ReplaceCaps)", old.replace_caps.to_string(), new.replace_caps.to_string(), false);
    add("sens", format!("{:?}", old.mixed_text_mode), format!("{:?}", new.mixed_text_mode), locked.mixed_text_mode);
    add("mode prudent", old.require_selection.to_string(), new.require_selection.to_string(), locked.require_selection);
    add("dernier mot", old.last_word_only.to_string(), new.last_word_only.to_string(), locked.last_word_only);
    add("éteindre Verr. Maj", old.turn_off_caps_lock.to_string(), new.turn_off_caps_lock.to_string(), locked.turn_off_caps_lock);
    add("console", old.console_support.to_string(), new.console_support.to_string(), locked.console_support);
    add("indicateur", old.indicator.to_string(), new.indicator.to_string(), locked.indicator);
    add("bulle", old.show_bubble.to_string(), new.show_bubble.to_string(), locked.show_bubble);
    add("position de la bulle", old.bubble_position.to_string(), new.bubble_position.to_string(), locked.bubble_position);
    add("son", old.play_sound.to_string(), new.play_sound.to_string(), locked.play_sound);
    add("changer la langue après", old.switch_language_after_convert.to_string(), new.switch_language_after_convert.to_string(), locked.switch_language_after_convert);
    add("limite", old.max_character_limit.to_string(), new.max_character_limit.to_string(), locked.max_character_limit);
    add("délai (ms)", old.paste_delay_ms.to_string(), new.paste_delay_ms.to_string(), locked.paste_delay_ms);
    add("langue de l'interface", old.ui_language.code().to_string(), new.ui_language.code().to_string(), locked.ui_language);
    add("programmes exclus", old.excluded_processes.join(", "), new.excluded_processes.join(", "), false);
    let rules = |s: &AppSettings| s.app_rules.iter().map(|r| format!("{}={:?}", r.process, r.mode)).collect::<Vec<_>>().join(", ");
    add("règles", rules(old), rules(new), false);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::RuleMode;

    #[test]
    fn audit_a06_replace_caps_alone_is_a_difference() {
        let old = AppSettings::default();
        let new = AppSettings { replace_caps: !old.replace_caps, ..old.clone() };
        let d = differences(&old, &new, &Locked::default());
        assert_eq!(d.len(), 1, "{d:?}");
        assert!(d[0].contains("ReplaceCaps"));
    }

    #[test]
    fn audit_a06_newer_format_version_is_refused() {
        let ok = format!(r#"{{"Format":"{FORMAT}","FormatVersion":1,"Settings":{{}}}}"#);
        assert!(import(&ok).is_ok());
        let newer = format!(r#"{{"Format":"{FORMAT}","FormatVersion":2,"Settings":{{}}}}"#);
        assert_eq!(import(&newer), Err(ImportError::NewerFormat));
        let bad = format!(r#"{{"Format":"{FORMAT}","FormatVersion":"x","Settings":{{}}}}"#);
        assert_eq!(import(&bad), Err(ImportError::Unreadable));
    }

    #[test]
    fn audit_a11_import_errors_have_a_translated_text() {
        for l in [crate::i18n::Lang::He, crate::i18n::Lang::Fr, crate::i18n::Lang::En] {
            for e in [ImportError::TooBig, ImportError::NotJson, ImportError::UnknownFormat, ImportError::NewerFormat, ImportError::Unreadable] {
                assert!(!crate::i18n::text(l, e.msg()).is_empty());
            }
        }
        assert_eq!(import("not json"), Err(ImportError::NotJson));
    }

    #[test]
    fn export_then_import_gives_the_same_settings() {
        let mut s = AppSettings { require_selection: true, indicator: true, ..AppSettings::default() };
        s.set_rule("winword.exe", Some(RuleMode::SelectionOnly));
        let text = export(&s, "1.5.0");
        assert!(text.contains("\"Format\": \"KeyboardLayoutFixer-Profil\""));
        assert!(!text.contains("\\\\"), "no path in a profile");
        assert_eq!(import(&text).unwrap(), s);
    }

    #[test]
    fn a_plain_settings_file_is_accepted_and_bad_files_are_refused() {
        let s = import(r#"{"RequireSelection": true}"#).unwrap();
        assert!(s.require_selection);
        assert!(import("not json").is_err());
        assert!(import(r#"{"Format": "autre chose"}"#).is_err());
        assert!(import(r#"[1, 2]"#).is_err());
        assert!(import(&" ".repeat(MAX_BYTES + 1)).is_err());
    }

    #[test]
    fn differences_mark_what_the_school_imposes() {
        let old = AppSettings::default();
        let new = AppSettings { require_selection: true, paste_delay_ms: 2000, ..AppSettings::default() };
        let locked = Locked { paste_delay_ms: true, ..Locked::default() };
        let d = differences(&old, &new, &locked);
        assert_eq!(d.len(), 2, "{d:?}");
        assert!(d.iter().any(|x| x.starts_with("mode prudent : false → true")));
        assert!(d.iter().any(|x| x.contains("délai (ms) : 1000 → 2000 (sans effet")));
        assert!(differences(&old, &old, &locked).is_empty());
    }
}
