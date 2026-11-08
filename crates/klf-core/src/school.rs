//! Settings imposed by the administrator (1.5.0): read by klf-win from the registry key
//! HKLM\SOFTWARE\Policies\NOAM\KeyboardLayoutFixer (a standard user cannot write there), merged here
//! over the user's own settings. Pure code, tested on Linux. An invalid value is ignored and
//! noted (diagnostic), never a crash.

use crate::settings::{AppRule, AppSettings, MixedTextMode, RuleMode, UiLanguage, MAX_RULES};

/// One registry value as read by the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegValue {
    Dword(u32),
    Text(String),
    Lines(Vec<String>),
}

/// The settings the administrator imposes (None = left to the user).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Policy {
    pub require_selection: Option<bool>,
    pub turn_off_caps_lock: Option<bool>,
    pub console_support: Option<bool>,
    pub last_word_only: Option<bool>,
    pub indicator: Option<bool>,
    pub show_bubble: Option<bool>,
    pub bubble_position: Option<u32>,
    pub check_updates: Option<bool>,
    pub play_sound: Option<bool>,
    pub switch_language_after_convert: Option<bool>,
    pub max_character_limit: Option<i64>,
    pub paste_delay_ms: Option<i64>,
    pub mixed_text_mode: Option<MixedTextMode>,
    pub hotkey: Option<(u32, u32)>,
    pub case_hotkey: Option<(u32, u32)>,
    pub ui_language: Option<UiLanguage>,
    /// administrator rules: added to the user's, they win on the same program, cannot be removed
    pub app_rules: Vec<AppRule>,
    pub excluded_processes: Vec<String>,
    /// values that were present but could not be used (name: why)
    pub ignored: Vec<String>,
}

impl Policy {
    pub fn is_empty(&self) -> bool {
        *self == Policy { ignored: self.ignored.clone(), ..Default::default() } && self.ignored.is_empty()
    }
}

/// Which settings are imposed (the settings window greys them out).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Locked {
    pub require_selection: bool,
    pub turn_off_caps_lock: bool,
    pub console_support: bool,
    pub last_word_only: bool,
    pub indicator: bool,
    pub show_bubble: bool,
    pub bubble_position: bool,
    pub check_updates: bool,
    pub play_sound: bool,
    pub switch_language_after_convert: bool,
    pub max_character_limit: bool,
    pub paste_delay_ms: bool,
    pub mixed_text_mode: bool,
    pub hotkey: bool,
    pub case_hotkey: bool,
    pub ui_language: bool,
    pub excluded_processes: bool,
}

impl Locked {
    pub fn any(&self) -> bool {
        *self != Locked::default()
    }
}

fn bool_of(name: &str, v: &RegValue, ignored: &mut Vec<String>) -> Option<bool> {
    match v {
        RegValue::Dword(0) => Some(false),
        RegValue::Dword(1) => Some(true),
        _ => {
            ignored.push(format!("{name} : 0 ou 1 attendu"));
            None
        }
    }
}

fn number_of(name: &str, v: &RegValue, ok: impl Fn(i64) -> bool, ignored: &mut Vec<String>) -> Option<i64> {
    match v {
        RegValue::Dword(n) if ok(*n as i64) => Some(*n as i64),
        _ => {
            ignored.push(format!("{name} : valeur hors limites"));
            None
        }
    }
}

/// Builds the policy from the values found under the key (name, value), names compared
/// without case. Unknown names are noted and ignored.
pub fn from_values(values: &[(String, RegValue)]) -> Policy {
    let mut p = Policy::default();
    let mut ign = Vec::new();
    let mut hk: (Option<u32>, Option<u32>) = (None, None);
    let mut chk: (Option<u32>, Option<u32>) = (None, None);
    for (name, v) in values {
        match name.to_ascii_lowercase().as_str() {
            "requireselection" => p.require_selection = bool_of(name, v, &mut ign),
            "turnoffcapslock" => p.turn_off_caps_lock = bool_of(name, v, &mut ign),
            "consolesupport" => p.console_support = bool_of(name, v, &mut ign),
            "lastwordonly" => p.last_word_only = bool_of(name, v, &mut ign),
            "indicator" => p.indicator = bool_of(name, v, &mut ign),
            "showbubble" => p.show_bubble = bool_of(name, v, &mut ign),
            "playsound" => p.play_sound = bool_of(name, v, &mut ign),
            "checkupdates" => p.check_updates = bool_of(name, v, &mut ign),
            "bubbleposition" => match v {
                RegValue::Dword(n) if *n <= crate::settings::BUBBLE_CARET => p.bubble_position = Some(*n),
                _ => ign.push(format!("{name} : 0, 1 ou 2 attendu")),
            },
            "switchlanguageafterconvert" => p.switch_language_after_convert = bool_of(name, v, &mut ign),
            "maxcharacterlimit" => p.max_character_limit = number_of(name, v, |n| n >= 0, &mut ign),
            "pastedelayms" => p.paste_delay_ms = number_of(name, v, |n| (100..=10_000).contains(&n), &mut ign),
            "mixedtextmode" => {
                p.mixed_text_mode = match v {
                    RegValue::Dword(0) => Some(MixedTextMode::ToggleAll),
                    RegValue::Dword(1) => Some(MixedTextMode::EnglishToHebrewOnly),
                    RegValue::Dword(2) => Some(MixedTextMode::HebrewToEnglishOnly),
                    _ => {
                        ign.push(format!("{name} : 0, 1 ou 2 attendu"));
                        None
                    }
                }
            }
            "hotkeymodifiers" => hk.0 = dword(name, v, &mut ign),
            "hotkeykey" => hk.1 = dword(name, v, &mut ign),
            "casehotkeymodifiers" => chk.0 = dword(name, v, &mut ign),
            "casehotkeykey" => chk.1 = dword(name, v, &mut ign),
            "uilanguage" => match v {
                RegValue::Text(t) if matches!(t.to_ascii_lowercase().as_str(), "auto" | "he" | "fr" | "en") => p.ui_language = Some(UiLanguage::from_code(t)),
                _ => ign.push(format!("{name} : auto, he, fr ou en attendu")),
            },
            "apprules" => match v {
                RegValue::Lines(lines) => {
                    for l in lines {
                        let Some((proc_, mode)) = l.split_once('=') else {
                            ign.push(format!("AppRules : ligne « {l} » sans « = »"));
                            continue;
                        };
                        let mode = match mode.trim().to_ascii_lowercase().as_str() {
                            "disabled" => RuleMode::Disabled,
                            "selectiononly" => RuleMode::SelectionOnly,
                            _ => {
                                ign.push(format!("AppRules : mode inconnu « {} »", mode.trim()));
                                continue;
                            }
                        };
                        let proc_ = crate::policy::exe_base_name(proc_.trim());
                        if !crate::policy::rule_allowed_for(&proc_) {
                            ign.push(format!("AppRules : programme non permis « {proc_} »"));
                            continue;
                        }
                        p.app_rules.retain(|r| r.process != proc_);
                        // audit A05: the user's values (HKCU) are read first, the machine's (HKLM)
                        // last: when the list is full, the oldest rule (a user one) gives way
                        if p.app_rules.len() >= MAX_RULES {
                            p.app_rules.remove(0);
                            if !ign.iter().any(|m| m.starts_with("AppRules : limite")) {
                                ign.push(format!("AppRules : limite de {MAX_RULES} règles atteinte, les plus anciennes (utilisateur) sont retirées"));
                            }
                        }
                        p.app_rules.push(AppRule { process: proc_, mode });
                    }
                }
                _ => ign.push(format!("{name} : liste (REG_MULTI_SZ) attendue")),
            },
            "excludedprocesses" => match v {
                RegValue::Lines(lines) => {
                    for l in lines.iter().map(|l| l.trim().to_lowercase()).filter(|l| !l.is_empty()) {
                        if !p.excluded_processes.contains(&l) {
                            p.excluded_processes.push(l);
                        }
                    }
                }
                _ => ign.push(format!("{name} : liste (REG_MULTI_SZ) attendue")),
            },
            _ => ign.push(format!("{name} : nom inconnu")),
        }
    }
    // a key needs both values, and must be allowed (same rules as the settings window)
    for (label, pair, slot, allow_none) in [("Hotkey", hk, &mut p.hotkey, false), ("CaseHotkey", chk, &mut p.case_hotkey, true)] {
        match pair {
            (None, None) => {}
            (m, Some(k)) => {
                let m = m.unwrap_or(0);
                if (allow_none && k == crate::settings::KEY_NONE) || crate::settings::hotkey_allowed(m, k) {
                    *slot = Some((m, k));
                } else {
                    ign.push(format!("{label} : touche non permise"));
                }
            }
            (Some(_), None) => ign.push(format!("{label}Key manquant")),
        }
    }
    p.ignored = ign;
    p
}

fn dword(name: &str, v: &RegValue, ignored: &mut Vec<String>) -> Option<u32> {
    match v {
        RegValue::Dword(n) => Some(*n),
        _ => {
            ignored.push(format!("{name} : nombre (REG_DWORD) attendu"));
            None
        }
    }
}

/// The settings in force: the user's, then what the administrator imposes on top.
pub fn effective(user: &AppSettings, p: &Policy) -> (AppSettings, Locked) {
    let mut s = user.clone();
    let mut l = Locked::default();
    macro_rules! take {
        ($f:ident) => {
            if let Some(v) = p.$f {
                s.$f = v;
                l.$f = true;
            }
        };
    }
    take!(require_selection);
    take!(turn_off_caps_lock);
    take!(console_support);
    take!(last_word_only);
    take!(indicator);
    take!(show_bubble);
    take!(bubble_position);
    take!(check_updates);
    take!(play_sound);
    take!(switch_language_after_convert);
    take!(max_character_limit);
    take!(paste_delay_ms);
    take!(mixed_text_mode);
    take!(ui_language);
    if let Some((m, k)) = p.hotkey {
        s.hotkey_modifiers = m;
        s.hotkey_key = k;
        l.hotkey = true;
    }
    if let Some((m, k)) = p.case_hotkey {
        s.case_hotkey_modifiers = m;
        s.case_hotkey_key = k;
        l.case_hotkey = true;
    }
    // the same key for both: the administrator's language key wins, the case key is switched off
    if s.case_hotkey_key != crate::settings::KEY_NONE && (s.case_hotkey_modifiers, s.case_hotkey_key) == (s.hotkey_modifiers, s.hotkey_key) {
        s.case_hotkey_key = crate::settings::KEY_NONE;
        s.case_hotkey_modifiers = 0;
    }
    for r in &p.app_rules {
        s.app_rules.retain(|x| x.process != r.process);
        s.app_rules.push(r.clone());
    }
    if !p.excluded_processes.is_empty() {
        for e in &p.excluded_processes {
            if !s.excluded_processes.contains(e) {
                s.excluded_processes.push(e.clone());
            }
        }
        l.excluded_processes = true;
    }
    (s, l)
}

/// The rule of a program comes from the administrator (cannot be changed from the tray menu).
pub fn school_rule(p: &Policy, exe: &str) -> bool {
    let n = crate::policy::exe_base_name(exe);
    p.app_rules.iter().any(|r| r.process == n)
}

/// Lines for the diagnostic report.
pub fn report_lines(p: &Policy) -> Vec<String> {
    let mut v = Vec::new();
    if p.is_empty() {
        v.push("aucun".to_string());
        return v;
    }
    macro_rules! show {
        ($f:ident, $label:expr) => {
            if let Some(x) = &p.$f {
                v.push(format!("{} = {:?}", $label, x));
            }
        };
    }
    show!(require_selection, "RequireSelection");
    show!(turn_off_caps_lock, "TurnOffCapsLock");
    show!(console_support, "ConsoleSupport");
    show!(last_word_only, "LastWordOnly");
    show!(indicator, "Indicator");
    show!(show_bubble, "ShowBubble");
    show!(bubble_position, "BubblePosition");
    show!(check_updates, "CheckUpdates");
    show!(play_sound, "PlaySound");
    show!(switch_language_after_convert, "SwitchLanguageAfterConvert");
    show!(max_character_limit, "MaxCharacterLimit");
    show!(paste_delay_ms, "PasteDelayMs");
    show!(mixed_text_mode, "MixedTextMode");
    show!(hotkey, "Hotkey (modificateurs, touche)");
    show!(case_hotkey, "CaseHotkey (modificateurs, touche)");
    show!(ui_language, "UiLanguage");
    for r in &p.app_rules {
        v.push(format!("AppRules : {} = {:?}", r.process, r.mode));
    }
    for e in &p.excluded_processes {
        v.push(format!("ExcludedProcesses : {e}"));
    }
    for i in &p.ignored {
        v.push(format!("IGNORÉ : {i}"));
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vals(v: &[(&str, RegValue)]) -> Vec<(String, RegValue)> {
        v.iter().map(|(n, x)| (n.to_string(), x.clone())).collect()
    }

    #[test]
    fn audit_a05_machine_rule_survives_200_user_rules() {
        let user: Vec<String> = (0..MAX_RULES).map(|i| format!("prog{i}.exe=Disabled")).collect();
        // HKCU is read first, HKLM last (platform/school.rs)
        let p = from_values(&vals(&[("AppRules", RegValue::Lines(user)), ("AppRules", RegValue::Lines(vec!["machine.exe=SelectionOnly".into()]))]));
        assert_eq!(p.app_rules.len(), MAX_RULES);
        assert!(p.app_rules.iter().any(|r| r.process == "machine.exe" && r.mode == RuleMode::SelectionOnly));
        assert_eq!(p.ignored.iter().filter(|m| m.contains("limite")).count(), 1, "{:?}", p.ignored);
    }

    #[test]
    fn school_bubble_position_is_locked_and_checked() {
        let p = from_values(&vals(&[("BubblePosition", RegValue::Dword(1))]));
        let (s, l) = effective(&AppSettings::default(), &p);
        assert_eq!(s.bubble_position, crate::settings::BUBBLE_BOTTOM);
        assert!(l.bubble_position);
        let p = from_values(&vals(&[("BubblePosition", RegValue::Dword(7))]));
        assert_eq!(p.bubble_position, None);
        assert_eq!(p.ignored.len(), 1);
    }

    #[test]
    fn school_values_override_the_user_and_are_locked() {
        let p = from_values(&vals(&[
            ("RequireSelection", RegValue::Dword(1)),
            ("pastedelayms", RegValue::Dword(1500)),
            ("UiLanguage", RegValue::Text("he".into())),
            ("AppRules", RegValue::Lines(vec!["WINWORD.EXE=SelectionOnly".into(), "jeu.exe=Disabled".into()])),
        ]));
        assert!(p.ignored.is_empty(), "{:?}", p.ignored);
        let mut user = AppSettings::default();
        user.set_rule("winword.exe", Some(RuleMode::Disabled));
        user.set_rule("notepad.exe", Some(RuleMode::SelectionOnly));
        let (s, l) = effective(&user, &p);
        assert!(s.require_selection && l.require_selection);
        assert_eq!(s.paste_delay_ms, 1500);
        assert_eq!(s.ui_language, UiLanguage::He);
        assert!(!l.hotkey && !l.indicator);
        assert_eq!(s.rule_for("winword.exe"), Some(RuleMode::SelectionOnly), "the school rule wins");
        assert_eq!(s.rule_for("jeu.exe"), Some(RuleMode::Disabled));
        assert_eq!(s.rule_for("notepad.exe"), Some(RuleMode::SelectionOnly), "the user's other rules stay");
        assert!(school_rule(&p, "C:\\x\\WinWord.exe"));
        assert!(!school_rule(&p, "notepad.exe"));
    }

    #[test]
    fn invalid_values_are_ignored_and_noted() {
        let p = from_values(&vals(&[
            ("RequireSelection", RegValue::Dword(7)),
            ("PasteDelayMs", RegValue::Dword(50)),
            ("HotkeyKey", RegValue::Dword(101)), // F12: not allowed
            ("AppRules", RegValue::Lines(vec!["explorer.exe=Disabled".into(), "x.exe=Maybe".into(), "nonsense".into()])),
            ("Bizarre", RegValue::Dword(1)),
            ("UiLanguage", RegValue::Text("de".into())),
        ]));
        assert_eq!(p.require_selection, None);
        assert_eq!(p.paste_delay_ms, None);
        assert_eq!(p.hotkey, None);
        assert!(p.app_rules.is_empty());
        assert_eq!(p.ignored.len(), 8, "{:?}", p.ignored);
        let (s, l) = effective(&AppSettings::default(), &p);
        assert_eq!(s, AppSettings::default());
        assert!(!l.any());
    }

    #[test]
    fn school_language_key_and_same_case_key() {
        let p = from_values(&vals(&[("HotkeyModifiers", RegValue::Dword(0)), ("HotkeyKey", RegValue::Dword(98))])); // F9
        let (s, l) = effective(&AppSettings::default(), &p);
        assert!(l.hotkey);
        assert_eq!(s.hotkey_key, 98);
        assert_eq!(s.case_hotkey_key, crate::settings::KEY_NONE, "F9 taken by the language key: case key off");
    }

    #[test]
    fn report_lists_imposed_and_ignored() {
        let p = from_values(&vals(&[("Indicator", RegValue::Dword(0)), ("Oops", RegValue::Dword(1))]));
        let r = report_lines(&p);
        assert!(r.iter().any(|x| x.starts_with("Indicator")));
        assert!(r.iter().any(|x| x.starts_with("IGNORÉ")));
        assert_eq!(report_lines(&Policy::default()), vec!["aucun".to_string()]);
    }
}
