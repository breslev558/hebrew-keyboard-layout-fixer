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

/// A profile file, or a plain settings.json. Err(reason in French).
pub fn import(text: &str) -> Result<AppSettings, String> {
    if text.len() > MAX_BYTES {
        return Err("fichier trop gros (plus de 1 Mo)".into());
    }
    let v: serde_json::Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|_| "ce n'est pas un fichier JSON lisible".to_string())?;
    let settings = match v.get("Format").and_then(|f| f.as_str()) {
        Some(FORMAT) => v.get("Settings").cloned().ok_or_else(|| "profil sans réglages".to_string())?,
        Some(_) => return Err("format de fichier inconnu".into()),
        None => v,
    };
    if !settings.is_object() {
        return Err("réglages illisibles".into());
    }
    let (s, ok) = AppSettings::from_json(&settings.to_string());
    if !ok {
        return Err("réglages illisibles".into());
    }
    Ok(s)
}

/// Differences between the current settings and the imported ones, in plain French; a setting
/// imposed by the school is marked "(sans effet : géré par l'école)".
pub fn differences(old: &AppSettings, new: &AppSettings, locked: &Locked) -> Vec<String> {
    let mut v = Vec::new();
    let mut add = |label: &str, a: String, b: String, l: bool| {
        if a != b {
            v.push(format!("{label} : {a} → {b}{}", if l { " (sans effet : géré par l'école)" } else { "" }));
        }
    };
    add("touche langue", old.hotkey_description(), new.hotkey_description(), locked.hotkey);
    add("touche majuscules", old.case_hotkey_description(), new.case_hotkey_description(), locked.case_hotkey);
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
