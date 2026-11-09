//! %APPDATA%\KeyboardLayoutFixer\settings.json — same file and same field names as the .NET versions
//! (DataContractJsonSerializer: enums written as numbers). Reading is tolerant: missing fields take
//! their default, enums accept a number or a name, unknown fields are ignored, invalid values are reset.

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MixedTextMode {
    #[default]
    ToggleAll,
    EnglishToHebrewOnly,
    HebrewToEnglishOnly,
}

impl MixedTextMode {
    fn from_number(n: i64) -> Option<Self> {
        match n {
            0 => Some(Self::ToggleAll),
            1 => Some(Self::EnglishToHebrewOnly),
            2 => Some(Self::HebrewToEnglishOnly),
            _ => None,
        }
    }
    fn from_name(s: &str) -> Option<Self> {
        match s {
            "ToggleAll" => Some(Self::ToggleAll),
            "EnglishToHebrewOnly" => Some(Self::EnglishToHebrewOnly),
            "HebrewToEnglishOnly" => Some(Self::HebrewToEnglishOnly),
            _ => None,
        }
    }
    pub fn as_number(self) -> i64 {
        self as i64
    }
}

/// Modifier flags, same values as RegisterHotKey (MOD_ALT=1, MOD_CONTROL=2, MOD_SHIFT=4, MOD_WIN=8).
pub const MOD_ALT: u32 = 1;
pub const MOD_CONTROL: u32 = 2;
pub const MOD_SHIFT: u32 = 4;
pub const MOD_WIN: u32 = 8;
pub const KNOWN_MODS: u32 = MOD_ALT | MOD_CONTROL | MOD_SHIFT | MOD_WIN;

/// HotkeyKey values of the .NET enum (= System.Windows.Input.Key numbers).
pub const KEY_NONE: u32 = 0;
pub const KEY_TAB: u32 = 3;
pub const KEY_ENTER: u32 = 6;
pub const KEY_SPACE: u32 = 18;
pub const KEY_A: u32 = 44;
pub const KEY_Z: u32 = 69;
pub const KEY_F1: u32 = 90;
pub const KEY_F10: u32 = 99;
pub const KEY_F12: u32 = 101;

pub fn key_is_known(k: u32) -> bool {
    matches!(k, KEY_TAB | KEY_ENTER | KEY_SPACE) || (KEY_A..=KEY_Z).contains(&k) || (KEY_F1..=KEY_F12).contains(&k)
}

pub fn key_is_function(k: u32) -> bool {
    (KEY_F1..=KEY_F12).contains(&k)
}

/// .NET key number -> Win32 virtual key code (0 if unknown).
pub fn key_to_vk(k: u32) -> u32 {
    match k {
        KEY_TAB => 0x09,
        KEY_ENTER => 0x0D,
        KEY_SPACE => 0x20,
        _ if (KEY_A..=KEY_Z).contains(&k) => 0x41 + (k - KEY_A),
        _ if (KEY_F1..=KEY_F12).contains(&k) => 0x70 + (k - KEY_F1),
        _ => 0,
    }
}

/// Win32 virtual key code -> .NET key number (None if the key cannot be used).
pub fn vk_to_key(vk: u32) -> Option<u32> {
    match vk {
        0x09 => Some(KEY_TAB),
        0x0D => Some(KEY_ENTER),
        0x20 => Some(KEY_SPACE),
        0x41..=0x5A => Some(KEY_A + (vk - 0x41)),
        0x70..=0x7B => Some(KEY_F1 + (vk - 0x70)),
        _ => None,
    }
}

pub fn key_name(k: u32) -> String {
    match k {
        KEY_TAB => "Tab".into(),
        KEY_ENTER => "Enter".into(),
        KEY_SPACE => "Space".into(),
        _ if (KEY_A..=KEY_Z).contains(&k) => char::from(b'A' + (k - KEY_A) as u8).to_string(),
        _ if (KEY_F1..=KEY_F12).contains(&k) => format!("F{}", k - KEY_F1 + 1),
        _ => "?".into(),
    }
}

/// Interface language: "auto" (Windows display language), "he", "fr", "en".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiLanguage {
    #[default]
    Auto,
    He,
    Fr,
    En,
}

impl UiLanguage {
    pub fn code(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::He => "he",
            Self::Fr => "fr",
            Self::En => "en",
        }
    }
    pub fn from_code(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "he" | "iw" | "hebrew" => Self::He,
            "fr" | "french" => Self::Fr,
            "en" | "english" => Self::En,
            _ => Self::Auto,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    #[serde(rename = "HotkeyModifiers", deserialize_with = "de_u32")]
    pub hotkey_modifiers: u32,
    #[serde(rename = "HotkeyKey", deserialize_with = "de_key")]
    pub hotkey_key: u32,
    #[serde(rename = "MaxCharacterLimit", deserialize_with = "de_i64")]
    pub max_character_limit: i64,
    #[serde(rename = "MixedTextMode", deserialize_with = "de_mode", serialize_with = "ser_mode")]
    pub mixed_text_mode: MixedTextMode,
    #[serde(rename = "ReplaceCaps", deserialize_with = "de_bool_true")]
    pub replace_caps: bool,
    /// 1.7.1: on by default (after F10 the keyboard follows the language of the fixed text)
    #[serde(rename = "SwitchLanguageAfterConvert", deserialize_with = "de_bool_true")]
    pub switch_language_after_convert: bool,
    /// Kept for compatibility with the .NET file; the Rust version uses HKCU/HKLM Run instead.
    #[serde(rename = "RunAtStartup", deserialize_with = "de_bool_false")]
    pub run_at_startup: bool,
    // --- new in 1.3.0 (ignored by the .NET versions) ---
    #[serde(rename = "ShowBubble", deserialize_with = "de_bool_true")]
    pub show_bubble: bool,
    #[serde(rename = "PlaySound", deserialize_with = "de_bool_false")]
    pub play_sound: bool,
    // --- new in 1.6.0 ---
    /// the bubble shows what was converted ("akuo → שלום"); never saved anywhere else
    #[serde(rename = "ShowPreview", deserialize_with = "de_bool_true")]
    pub show_preview: bool,
    /// the welcome window (public install) was closed with "do not show again"
    #[serde(rename = "WelcomeDone", deserialize_with = "de_bool_false")]
    pub welcome_done: bool,
    // --- new in 1.6.1 : anonymous problem reports (public install only) ---
    /// 0 = ask after a real error (default), 1 = always send without asking, 2 = never ask
    #[serde(rename = "ReportMode", default)]
    pub report_mode: u32,
    // --- new in 1.5.5 : update in one click (public per-user install only) ---
    #[serde(rename = "CheckUpdates", deserialize_with = "de_bool_true")]
    pub check_updates: bool,
    /// a version refused or failed is not offered again for 7 days
    #[serde(rename = "UpdateSkipTag", default)]
    pub update_skip_tag: String,
    #[serde(rename = "UpdateSkipAt", default)]
    pub update_skip_at: u64,
    // --- new in 1.5.2 ---
    /// Where the bubble appears: BUBBLE_TOP (default), BUBBLE_BOTTOM or BUBBLE_CARET.
    #[serde(rename = "BubblePosition", deserialize_with = "de_bubble_position")]
    pub bubble_position: u32,
    #[serde(rename = "UiLanguage", deserialize_with = "de_lang", serialize_with = "ser_lang")]
    pub ui_language: UiLanguage,
    /// Off by default since 1.3.2 (audit: not yet tried in real consoles; "experimental").
    #[serde(rename = "ConsoleSupport", deserialize_with = "de_bool_false")]
    pub console_support: bool,
    /// Time between Ctrl+V and putting the user's clipboard back (slow PCs need more).
    #[serde(rename = "PasteDelayMs", deserialize_with = "de_i64")]
    pub paste_delay_ms: i64,
    /// Extra programs where F10 does nothing (exe names, e.g. "game.exe"); added to the built-in list.
    #[serde(rename = "ExcludedProcesses", deserialize_with = "de_strings")]
    pub excluded_processes: Vec<String>,
    // --- new in 1.3.3 ---
    /// Careful mode: F10 only converts a selection, never "caret -> start of the line".
    #[serde(rename = "RequireSelection", deserialize_with = "de_bool_false")]
    pub require_selection: bool,
    // --- new in 1.3.5 : Caps Lock fix (second hotkey) ---
    #[serde(rename = "CaseHotkeyModifiers", deserialize_with = "de_u32")]
    pub case_hotkey_modifiers: u32,
    /// KEY_NONE = Caps Lock fix switched off.
    #[serde(rename = "CaseHotkeyKey", deserialize_with = "de_key_or_none")]
    pub case_hotkey_key: u32,
    #[serde(rename = "TurnOffCapsLock", deserialize_with = "de_bool_true")]
    pub turn_off_caps_lock: bool,
    /// Which action this run does (set by the hotkey pressed, never saved).
    #[serde(skip)]
    pub action: Action,
    // --- new in 1.4.0 : rules per program (tray menu), format ready for the administrator profiles ---
    /// Per-program rules; they can only RESTRICT (never lift the password, administrator or
    /// console protections). "ExcludedProcesses" stays (older versions read it) = Disabled.
    #[serde(rename = "AppRules", deserialize_with = "de_rules")]
    pub app_rules: Vec<AppRule>,
    // --- new in 1.4.1 (trial, off by default) ---
    /// Nothing selected: only the last word (instead of caret -> start of the line), in the
    /// known text fields only.
    #[serde(rename = "LastWordOnly", deserialize_with = "de_bool_false")]
    pub last_word_only: bool,
    // --- new in 1.5.0 ---
    /// Small "HE / EN" sign when the keyboard language changes (off by default).
    #[serde(rename = "Indicator", deserialize_with = "de_bool_false")]
    pub indicator: bool,
    /// 0 (absent) = file written before 1.3.3, when ConsoleSupport was ON by default.
    #[serde(rename = "SettingsVersion", default, deserialize_with = "de_u32")]
    pub settings_version: u32,
    /// The file was from an older version and was adjusted: it should be written again.
    #[serde(skip)]
    pub migrated: bool,
}

pub const SETTINGS_VERSION: u32 = 4;
/// Bubble position (1.5.2): centre top of the work area of the target's screen,
/// centre bottom, or near the text caret (falls back to the centre top when it is unknown).
pub const BUBBLE_TOP: u32 = 0;
pub const BUBBLE_BOTTOM: u32 = 1;
pub const BUBBLE_CARET: u32 = 2;
/// At most this many rules are kept.
pub const MAX_RULES: usize = 200;

/// What a rule does in one program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleMode {
    /// F10/F9 do nothing in this program.
    Disabled,
    /// Only a selected text is converted (careful mode for this program).
    SelectionOnly,
}

/// One rule: program name (lower case, no path) and what it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppRule {
    #[serde(rename = "Process")]
    pub process: String,
    #[serde(rename = "Mode")]
    pub mode: RuleMode,
}
pub const KEY_F9: u32 = 98;
pub const KEY_Q: u32 = 60;

/// What a hotkey does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Action {
    /// F10: text typed with the wrong keyboard (Hebrew <-> English)
    #[default]
    Layout,
    /// F9: text typed with Caps Lock on (case of every Latin letter swapped)
    Case,
}

pub const DEFAULT_PASTE_DELAY_MS: i64 = 1000;

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hotkey_modifiers: 0,
            hotkey_key: KEY_F10,
            max_character_limit: 0,
            mixed_text_mode: MixedTextMode::ToggleAll,
            replace_caps: true,
            switch_language_after_convert: true,
            run_at_startup: false,
            show_bubble: true,
            play_sound: false,
            bubble_position: BUBBLE_TOP,
            check_updates: true,
            show_preview: true,
            welcome_done: false,
            report_mode: 0,
            update_skip_tag: String::new(),
            update_skip_at: 0,
            ui_language: UiLanguage::Auto,
            console_support: false,
            paste_delay_ms: DEFAULT_PASTE_DELAY_MS,
            excluded_processes: Vec::new(),
            app_rules: Vec::new(),
            last_word_only: false,
            indicator: false,
            require_selection: false,
            case_hotkey_modifiers: 0,
            case_hotkey_key: KEY_F9,
            turn_off_caps_lock: true,
            action: Action::Layout,
            settings_version: SETTINGS_VERSION,
            migrated: false,
        }
    }
}

impl AppSettings {
    /// Reads the JSON text; anything unreadable gives the defaults (never an error).
    pub fn from_json(text: &str) -> (Self, bool) {
        let trimmed = text.trim_start_matches('\u{feff}');
        match serde_json::from_str::<AppSettings>(trimmed) {
            Ok(mut s) => {
                if s.settings_version < 2 || s.settings_version == u32::MAX {
                    // 1.3.0/1.3.1 wrote ConsoleSupport=true as their default: the experimental
                    // console path is switched off once (audit 1.3.2); the user can switch it on again
                    s.console_support = false;
                    // file of the ORIGINAL program (RavidSaadia 1.2.8, default Ctrl+Q): the NOAM version
                    // key F10 is used instead (the original Ctrl+Q setting is never carried over)
                    if (s.hotkey_modifiers, s.hotkey_key) == (MOD_CONTROL, KEY_Q) {
                        s.hotkey_modifiers = 0;
                        s.hotkey_key = KEY_F10;
                    }
                }
                // 2 -> 3: nothing to change (AppRules is new and empty)
                // 3 -> 4 (1.7.1): "switch the keyboard language after the fix" becomes the default.
                // Older files always wrote the old default (off), so an unchanged file cannot be
                // told from a deliberate choice: it is switched on once; a choice made from 1.7.1
                // on (version 4) is never changed again.
                if s.settings_version < 4 || s.settings_version == u32::MAX {
                    s.switch_language_after_convert = true;
                }
                if s.settings_version != SETTINGS_VERSION {
                    s.settings_version = SETTINGS_VERSION;
                    s.migrated = true;
                }
                s.validate();
                (s, true)
            }
            Err(_) => (Self::default(), false),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| String::from("{}"))
    }

    /// Same rules as AppSettings.Validate() of 1.2.9-noam5, plus the new fields.
    pub fn validate(&mut self) {
        if self.hotkey_modifiers & !KNOWN_MODS != 0 {
            self.hotkey_modifiers = 0;
            self.hotkey_key = KEY_F10;
        }
        if self.hotkey_key == KEY_NONE || !key_is_known(self.hotkey_key) {
            self.hotkey_key = KEY_F10;
        }
        // a function key alone (F1-F11) is allowed; other keys need Ctrl, Alt or Win
        // (Shift+K would take the capital K away from the user)
        if !key_is_function(self.hotkey_key) && self.hotkey_modifiers & (MOD_CONTROL | MOD_ALT | MOD_WIN) == 0 {
            self.hotkey_modifiers |= MOD_CONTROL;
        }
        if !hotkey_allowed(self.hotkey_modifiers, self.hotkey_key) {
            self.hotkey_modifiers = 0;
            self.hotkey_key = KEY_F10;
        }
        // second hotkey: same rules; switched off if not allowed or the same as the first one
        if self.case_hotkey_key != KEY_NONE
            && (self.case_hotkey_modifiers & !KNOWN_MODS != 0
                || !hotkey_allowed(self.case_hotkey_modifiers, self.case_hotkey_key)
                || (self.case_hotkey_modifiers, self.case_hotkey_key) == (self.hotkey_modifiers, self.hotkey_key))
        {
            self.case_hotkey_modifiers = 0;
            self.case_hotkey_key = KEY_NONE;
        }
        if self.max_character_limit < 0 {
            self.max_character_limit = 0;
        }
        if !(100..=10_000).contains(&self.paste_delay_ms) {
            self.paste_delay_ms = DEFAULT_PASTE_DELAY_MS;
        }
        self.excluded_processes.retain(|p| !p.trim().is_empty());
        for p in self.excluded_processes.iter_mut() {
            *p = p.trim().to_lowercase();
        }
        // rules: program name only (no path), lower case, one rule per program (the last
        // one wins), never our own program, at most MAX_RULES
        let mut rules: Vec<AppRule> = Vec::new();
        for r in self.app_rules.drain(..) {
            let p = crate::policy::exe_base_name(r.process.trim());
            if !crate::policy::rule_allowed_for(&p) {
                continue;
            }
            rules.retain(|x| x.process != p);
            rules.push(AppRule { process: p, mode: r.mode });
        }
        rules.truncate(MAX_RULES);
        self.app_rules = rules;
    }

    /// The rule of a program (None: no rule).
    pub fn rule_for(&self, exe_name: &str) -> Option<RuleMode> {
        let n = crate::policy::exe_base_name(exe_name);
        self.app_rules.iter().find(|r| r.process == n).map(|r| r.mode)
    }

    /// Sets (Some) or removes (None) the rule of a program; false if not allowed for it.
    pub fn set_rule(&mut self, exe_name: &str, mode: Option<RuleMode>) -> bool {
        let n = crate::policy::exe_base_name(exe_name.trim());
        if !crate::policy::rule_allowed_for(&n) {
            return false;
        }
        self.app_rules.retain(|r| r.process != n);
        if let Some(m) = mode {
            if self.app_rules.len() >= MAX_RULES {
                return false;
            }
            self.app_rules.push(AppRule { process: n, mode: m });
        }
        true
    }

    pub fn hotkey_description(&self) -> String {
        hotkey_description(self.hotkey_modifiers, self.hotkey_key)
    }

    pub fn case_hotkey_description(&self) -> String {
        hotkey_description(self.case_hotkey_modifiers, self.case_hotkey_key)
    }
}

/// Ctrl+C / Ctrl+V / Ctrl+X / Ctrl+A / Ctrl+Z are needed by the program itself and by the user.
/// The Windows hotkey box cannot show Win: when the key and the other modifiers are unchanged,
/// Win is kept (both hotkeys, audit A07).
pub fn keep_win(old_mods: u32, old_key: u32, vk: u32, mods: u32) -> u32 {
    if old_mods & MOD_WIN != 0 && key_to_vk(old_key) == vk && mods == old_mods & !MOD_WIN {
        mods | MOD_WIN
    } else {
        mods
    }
}

pub fn hotkey_allowed(mods: u32, key: u32) -> bool {
    if !key_is_known(key) || mods & !KNOWN_MODS != 0 {
        return false;
    }
    // F12 is reserved by Windows for the debugger (RegisterHotKey documentation), with any modifier
    if key == KEY_F12 {
        return false;
    }
    if !key_is_function(key) && mods & (MOD_CONTROL | MOD_ALT | MOD_WIN) == 0 {
        return false;
    }
    let reserved = matches!(key_name(key).as_str(), "C" | "V" | "X" | "A" | "Z");
    !(reserved && mods & !MOD_SHIFT == MOD_CONTROL)
}

pub fn hotkey_description(mods: u32, key: u32) -> String {
    let mut parts: Vec<String> = Vec::new();
    if mods & MOD_CONTROL != 0 {
        parts.push("Ctrl".into());
    }
    if mods & MOD_ALT != 0 {
        parts.push("Alt".into());
    }
    if mods & MOD_SHIFT != 0 {
        parts.push("Shift".into());
    }
    if mods & MOD_WIN != 0 {
        parts.push("Win".into());
    }
    parts.push(key_name(key));
    parts.join("+")
}

// ---- tolerant field readers (a wrong type gives the default instead of failing the whole file) ----

fn de_value<'de, D: Deserializer<'de>>(d: D) -> Result<serde_json::Value, D::Error> {
    serde_json::Value::deserialize(d)
}

fn de_bubble_position<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let v = de_value(d)?;
    Ok(v.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n <= BUBBLE_CARET).unwrap_or(BUBBLE_TOP))
}

fn de_u32<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let v = de_value(d)?;
    Ok(v.as_u64().and_then(|n| u32::try_from(n).ok()).unwrap_or(u32::MAX))
}

fn de_key<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let v = de_value(d)?;
    if let Some(n) = v.as_u64() {
        return Ok(u32::try_from(n).unwrap_or(KEY_NONE));
    }
    if let Some(s) = v.as_str() {
        for k in [KEY_TAB, KEY_ENTER, KEY_SPACE].into_iter().chain(KEY_A..=KEY_Z).chain(KEY_F1..=KEY_F12) {
            if key_name(k).eq_ignore_ascii_case(s) {
                return Ok(k);
            }
        }
    }
    Ok(KEY_NONE)
}

/// Like de_key, but a missing/invalid value or 0 means "off"; absent field = F9 (struct default).
fn de_key_or_none<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    de_key(d)
}

fn de_i64<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    let v = de_value(d)?;
    Ok(v.as_i64().unwrap_or(-1))
}

fn de_bool(v: serde_json::Value, default: bool) -> bool {
    match v {
        serde_json::Value::Bool(b) => b,
        serde_json::Value::Number(n) => n.as_i64().map(|x| x != 0).unwrap_or(default),
        serde_json::Value::String(s) => match s.to_ascii_lowercase().as_str() {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => default,
        },
        _ => default,
    }
}

fn de_bool_true<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(de_bool(de_value(d)?, true))
}

fn de_bool_false<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    Ok(de_bool(de_value(d)?, false))
}

fn de_mode<'de, D: Deserializer<'de>>(d: D) -> Result<MixedTextMode, D::Error> {
    let v = de_value(d)?;
    let m = match &v {
        serde_json::Value::Number(n) => n.as_i64().and_then(MixedTextMode::from_number),
        serde_json::Value::String(s) => MixedTextMode::from_name(s),
        _ => None,
    };
    Ok(m.unwrap_or_default())
}

fn ser_mode<S: serde::Serializer>(m: &MixedTextMode, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_i64(m.as_number())
}

fn de_lang<'de, D: Deserializer<'de>>(d: D) -> Result<UiLanguage, D::Error> {
    let v = de_value(d)?;
    Ok(v.as_str().map(UiLanguage::from_code).unwrap_or_default())
}

fn ser_lang<S: serde::Serializer>(l: &UiLanguage, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(l.code())
}

/// A list of rules; a rule that cannot be read is left out (never the whole file).
fn de_rules<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<AppRule>, D::Error> {
    let v = de_value(d)?;
    Ok(v.as_array().map(|a| a.iter().filter_map(|x| serde_json::from_value::<AppRule>(x.clone()).ok()).collect()).unwrap_or_default())
}

fn de_strings<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let v = de_value(d)?;
    Ok(match v {
        serde_json::Value::Array(a) => a.into_iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bubble_position_defaults_to_top_and_rejects_bad_values() {
        assert_eq!(AppSettings::default().bubble_position, BUBBLE_TOP);
        assert_eq!(AppSettings::from_json("{}").0.bubble_position, BUBBLE_TOP);
        assert_eq!(AppSettings::from_json("{\"BubblePosition\":2}").0.bubble_position, BUBBLE_CARET);
        assert_eq!(AppSettings::from_json("{\"BubblePosition\":9}").0.bubble_position, BUBBLE_TOP);
        assert_eq!(AppSettings::from_json("{\"BubblePosition\":\"x\"}").0.bubble_position, BUBBLE_TOP);
        let (s, _) = AppSettings::from_json("{\"BubblePosition\":1}");
        assert!(s.to_json().contains("\"BubblePosition\": 1"));
    }

    #[test]
    fn audit_a07_win_is_kept_when_nothing_changed() {
        let f9 = key_to_vk(KEY_F9);
        assert_eq!(keep_win(MOD_WIN, KEY_F9, f9, 0), MOD_WIN, "Win+F9 unchanged stays Win+F9");
        assert_eq!(keep_win(MOD_WIN | MOD_CONTROL, KEY_F9, f9, MOD_CONTROL), MOD_WIN | MOD_CONTROL);
        assert_eq!(keep_win(MOD_WIN, KEY_F9, key_to_vk(KEY_F10), 0), 0, "another key: Win not added");
        assert_eq!(keep_win(MOD_WIN, KEY_F9, f9, MOD_CONTROL), MOD_CONTROL, "other modifiers changed: Win not added");
        assert_eq!(keep_win(0, KEY_F9, f9, 0), 0);
    }

    fn v(f: impl FnOnce(&mut AppSettings)) -> AppSettings {
        let mut s = AppSettings::default();
        f(&mut s);
        s.validate();
        s
    }

    #[test]
    fn defaults() {
        let d = AppSettings::default();
        assert_eq!(d.hotkey_key, KEY_F10);
        assert_eq!(d.hotkey_modifiers, 0);
        assert_eq!(d.mixed_text_mode, MixedTextMode::ToggleAll);
        assert!(d.replace_caps && d.show_bubble && !d.play_sound);
        assert_eq!(d.hotkey_description(), "F10");
    }

    #[test]
    fn validate_like_csharp() {
        let bad = v(|s| {
            s.hotkey_modifiers = 0x30;
            s.hotkey_key = 54; // K
        });
        assert_eq!((bad.hotkey_modifiers, bad.hotkey_key), (0, KEY_F10));
        let ok = v(|s| {
            s.hotkey_modifiers = MOD_CONTROL | MOD_SHIFT;
            s.hotkey_key = 54;
        });
        assert_eq!((ok.hotkey_modifiers, ok.hotkey_key), (MOD_CONTROL | MOD_SHIFT, 54));
        assert_eq!(ok.hotkey_description(), "Ctrl+Shift+K");
        assert_eq!(v(|s| s.hotkey_key = 54).hotkey_modifiers, MOD_CONTROL);
        assert_eq!(v(|s| { s.hotkey_key = 54; s.hotkey_modifiers = MOD_SHIFT }).hotkey_modifiers, MOD_CONTROL | MOD_SHIFT);
        let c = v(|s| { s.hotkey_key = 46; s.hotkey_modifiers = MOD_CONTROL }); // Ctrl+C
        assert_eq!((c.hotkey_modifiers, c.hotkey_key), (0, KEY_F10));
        assert!(hotkey_allowed(MOD_CONTROL | MOD_ALT, 46));
        assert!(!hotkey_allowed(MOD_SHIFT, 54));
        assert!(hotkey_allowed(0, KEY_F10));
        assert_eq!(v(|s| s.hotkey_key = 12345).hotkey_key, KEY_F10);
        assert_eq!(v(|s| s.max_character_limit = -5).max_character_limit, 0);
        assert_eq!(v(|s| s.paste_delay_ms = 5).paste_delay_ms, DEFAULT_PASTE_DELAY_MS);
    }

    #[test]
    fn reads_the_dotnet_file() {
        // file written by 1.2.8/1.2.9 (DataContractJsonSerializer)
        let net = "\u{feff}{\n  \"HotkeyKey\":99,\"HotkeyModifiers\":0,\"MaxCharacterLimit\":0,\"MixedTextMode\":0,\n  \"ReplaceCaps\":true,\"RunAtStartup\":false,\"SwitchLanguageAfterConvert\":false\n}";
        let (s, ok) = AppSettings::from_json(net);
        assert!(ok);
        assert!(s.migrated, "a .NET file has no SettingsVersion: written again once");
        assert_eq!(AppSettings { migrated: false, ..s }, AppSettings::default());
    }

    #[test]
    fn tolerant_reading() {
        let (s, _) = AppSettings::from_json("{\"MixedTextMode\":999,\"HotkeyModifiers\":64,\"HotkeyKey\":95}");
        assert_eq!(s.mixed_text_mode, MixedTextMode::ToggleAll);
        assert_eq!((s.hotkey_modifiers, s.hotkey_key), (0, KEY_F10));
        let (s, _) = AppSettings::from_json("{\"MixedTextMode\":\"HebrewToEnglishOnly\",\"HotkeyKey\":\"F9\",\"ReplaceCaps\":\"false\"}");
        assert_eq!(s.mixed_text_mode, MixedTextMode::HebrewToEnglishOnly);
        assert_eq!(s.hotkey_key, 98);
        assert!(!s.replace_caps);
        let (s, ok) = AppSettings::from_json("{}");
        assert!(ok && s.replace_caps && s.hotkey_key == KEY_F10);
        let (s, ok) = AppSettings::from_json("not json");
        assert!(!ok && s == AppSettings::default());
        let (s, _) = AppSettings::from_json("{\"HotkeyKey\":\"xyz\",\"ExcludedProcesses\":[\" Game.EXE \", 3, \"\"]}");
        assert_eq!(s.hotkey_key, KEY_F10);
        assert_eq!(s.excluded_processes, vec!["game.exe".to_string()]);
    }

    #[test]
    fn written_file_is_readable_by_dotnet_rules() {
        let s = AppSettings::default();
        let json = s.to_json();
        assert!(json.contains("\"MixedTextMode\": 0"));
        assert!(json.contains("\"HotkeyKey\": 99"));
        assert_eq!(AppSettings::from_json(&json).0, s);
    }

    #[test]
    fn f12_refused() {
        assert!(!hotkey_allowed(0, KEY_F12));
        assert!(!hotkey_allowed(MOD_CONTROL, KEY_F12));
        assert!(hotkey_allowed(0, KEY_F12 - 1));
        let (s, _) = AppSettings::from_json(r#"{"HotkeyModifiers":0,"HotkeyKey":"F12"}"#);
        assert_eq!(s.hotkey_key, KEY_F10);
    }

    #[test]
    fn vk_mapping() {
        assert_eq!(key_to_vk(KEY_F10), 0x79);
        assert_eq!(key_to_vk(KEY_A), 0x41);
        assert_eq!(vk_to_key(0x79), Some(KEY_F10));
        assert_eq!(vk_to_key(0x30), None);
        for k in (KEY_A..=KEY_Z).chain(KEY_F1..=KEY_F12) {
            assert_eq!(vk_to_key(key_to_vk(k)), Some(k));
        }
    }

    #[test]
    fn old_files_get_the_console_switched_off_once() {
        let (s, ok) = AppSettings::from_json(r#"{"HotkeyKey":"F10","ConsoleSupport":true}"#);
        assert!(ok && !s.console_support && s.migrated && s.settings_version == SETTINGS_VERSION);
        let (s, _) = AppSettings::from_json(&s.to_json().replace("\"ConsoleSupport\": false", "\"ConsoleSupport\": true"));
        assert!(s.console_support && !s.migrated, "a choice made after the migration is kept");
        assert!(!AppSettings::default().require_selection);
    }

    #[test]
    fn caps_lock_hotkey_defaults_and_rules() {
        let d = AppSettings::default();
        assert_eq!((d.case_hotkey_modifiers, d.case_hotkey_key, d.turn_off_caps_lock), (0, KEY_F9, true));
        assert_eq!(d.case_hotkey_description(), "F9");
        // same as the main hotkey -> switched off
        let (s, _) = AppSettings::from_json(r#"{"HotkeyKey":"F9","CaseHotkeyKey":"F9","SettingsVersion":2}"#);
        assert_eq!(s.case_hotkey_key, KEY_NONE);
        // switched off on purpose stays off
        let (s, _) = AppSettings::from_json(r#"{"CaseHotkeyKey":0,"SettingsVersion":2}"#);
        assert_eq!(s.case_hotkey_key, KEY_NONE);
        // .NET file (no field): F9
        let (s, _) = AppSettings::from_json(r#"{"HotkeyKey":99}"#);
        assert_eq!(s.case_hotkey_key, KEY_F9);
        // F12 refused
        let (s, _) = AppSettings::from_json(r#"{"CaseHotkeyKey":"F12","SettingsVersion":2}"#);
        assert_eq!(s.case_hotkey_key, KEY_NONE);
    }

    #[test]
    fn original_ctrl_q_file_gets_f10() {
        // settings.json of the original program (RavidSaadia 1.2.8)
        let orig = r#"{ "HotkeyModifiers": 2, "HotkeyKey": 60, "MaxCharacterLimit": 0, "MixedTextMode": 0, "ReplaceCaps": true, "SwitchLanguageAfterConvert": true, "RunAtStartup": true }"#;
        let (s, ok) = AppSettings::from_json(orig);
        assert!(ok && s.migrated);
        assert_eq!((s.hotkey_modifiers, s.hotkey_key), (0, KEY_F10));
        assert!(s.switch_language_after_convert, "the other choices are kept");
        // a user who chose Ctrl+Q AFTER the migration keeps it
        let (s, _) = AppSettings::from_json(r#"{"HotkeyModifiers":2,"HotkeyKey":60,"SettingsVersion":2}"#);
        assert_eq!((s.hotkey_modifiers, s.hotkey_key), (MOD_CONTROL, KEY_Q));
    }

    #[test]
    fn rules_are_cleaned_and_version_3_keeps_the_console_choice() {
        let (s, ok) = AppSettings::from_json(r#"{"SettingsVersion":2,"ConsoleSupport":true,"AppRules":[
            {"Process":"C:\\Program Files\\Office\\WINWORD.EXE","Mode":"Disabled"},
            {"Process":"winword.exe","Mode":"SelectionOnly"},
            {"Process":"explorer.exe","Mode":"Disabled"},
            {"Process":"KeyboardLayoutFixer.exe","Mode":"Disabled"},
            {"Process":"x.exe","Mode":"Nonsense"},
            {"Process":"","Mode":"Disabled"}]}"#);
        assert!(ok);
        assert!(s.console_support, "2 -> 3 does not switch the console off again");
        assert!(s.migrated);
        assert_eq!(s.settings_version, SETTINGS_VERSION);
        assert_eq!(s.app_rules, vec![AppRule { process: "winword.exe".into(), mode: RuleMode::SelectionOnly }]);
        assert_eq!(s.rule_for("WinWord.exe"), Some(RuleMode::SelectionOnly));
        let mut s2 = s.clone();
        assert!(!s2.set_rule("explorer.exe", Some(RuleMode::Disabled)));
        assert!(s2.set_rule("winword.exe", None));
        assert!(s2.app_rules.is_empty());
        // written back and read again: same rules
        let (s3, _) = AppSettings::from_json(&s.to_json());
        assert_eq!(s3.app_rules, s.app_rules);
    }

    #[test]
    fn version_1_file_still_switches_the_console_off_once() {
        let (s, _) = AppSettings::from_json(r#"{"ConsoleSupport":true,"HotkeyModifiers":2,"HotkeyKey":60}"#);
        assert!(!s.console_support);
        assert_eq!(s.hotkey_key, KEY_F10);
    }

    #[test]
    fn keyboard_follows_the_fixed_text_by_default_from_1_7_1() {
        assert!(AppSettings::default().switch_language_after_convert, "new install: on");
        let (s, _) = AppSettings::from_json(r#"{"SwitchLanguageAfterConvert":false,"SettingsVersion":3}"#);
        assert!(s.switch_language_after_convert, "file of 1.6.x/1.7.0: switched on once");
        assert_eq!(s.settings_version, SETTINGS_VERSION);
        let (s, _) = AppSettings::from_json(r#"{"SwitchLanguageAfterConvert":false,"SettingsVersion":4}"#);
        assert!(!s.switch_language_after_convert, "a choice made with 1.7.1 or later is kept");
        let (again, _) = AppSettings::from_json(&s.to_json());
        assert!(!again.switch_language_after_convert, "and stays off after saving");
        let (s, _) = AppSettings::from_json(r#"{"SettingsVersion":4}"#);
        assert!(s.switch_language_after_convert, "missing value: the default");
    }
}
