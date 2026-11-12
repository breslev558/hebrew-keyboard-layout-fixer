//! "Why did it not work?": the last attempts, kept in MEMORY only (never on disk), and the
//! text of "Copy the diagnostic information". Never any typed text, clipboard content, window
//! title, user name or path: only technical facts (program name, window class, step, times).
//! What is not known is written "inconnu".

use crate::i18n::Msg;
use crate::settings::Action;
use std::collections::VecDeque;

/// How many attempts are kept.
pub const KEPT: usize = 10;

/// Steps of one F10/F9 press, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Checks,
    KeysReleased,
    ClipboardSave,
    Copy,
    LineStart,
    Convert,
    Publish,
    Verify,
    Paste,
    WaitBeforeRestore,
    Restore,
    Console,
}

impl Step {
    pub fn name(self) -> &'static str {
        self.name_in(true)
    }
    /// 1.9.1: French (`fr`) or English (the copied report follows the interface: French, or English
    /// for the Hebrew and English interfaces, technical text being read left to right).
    pub fn name_in(self, fr: bool) -> &'static str {
        if !fr {
            return match self {
                Self::Checks => "checks",
                Self::KeysReleased => "keys released",
                Self::ClipboardSave => "clipboard saved",
                Self::Copy => "copy (Ctrl+C)",
                Self::LineStart => "selection to the start of the line",
                Self::Convert => "conversion",
                Self::Publish => "fixed text put in the clipboard",
                Self::Verify => "check before pasting",
                Self::Paste => "paste (Ctrl+V)",
                Self::WaitBeforeRestore => "wait before restoring",
                Self::Restore => "clipboard restored",
                Self::Console => "console",
            };
        }
        match self {
            Self::Checks => "vérifications",
            Self::KeysReleased => "touches relâchées",
            Self::ClipboardSave => "sauvegarde du presse-papiers",
            Self::Copy => "copie (Ctrl+C)",
            Self::LineStart => "sélection jusqu'au début de la ligne",
            Self::Convert => "conversion",
            Self::Publish => "texte corrigé mis dans le presse-papiers",
            Self::Verify => "vérification avant collage",
            Self::Paste => "collage (Ctrl+V)",
            Self::WaitBeforeRestore => "attente avant remise",
            Self::Restore => "remise du presse-papiers",
            Self::Console => "console",
        }
    }
}

/// Times of the steps of one attempt. The host gives a monotonic clock in milliseconds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trace {
    started: Option<u64>,
    current: Option<(Step, u64)>,
    pub steps: Vec<(Step, u32)>,
    /// what was known about the target window
    pub info: Option<crate::flow::TargetInfo>,
    /// number of characters converted
    pub chars: Option<usize>,
    /// Latin layout used by the conversion
    pub latin: Option<&'static str>,
}

impl Trace {
    /// The previous step ends, `step` starts now.
    pub fn mark(&mut self, step: Step, now_ms: u64) {
        if self.started.is_none() {
            self.started = Some(now_ms);
        }
        if let Some((s, t)) = self.current.take() {
            self.steps.push((s, now_ms.saturating_sub(t).min(u32::MAX as u64) as u32));
        }
        self.current = Some((step, now_ms));
    }
    /// The last step ends; returns the step where the attempt stopped and the total time.
    pub fn finish(&mut self, now_ms: u64) -> (Option<Step>, u32) {
        let last = self.current.map(|(s, _)| s);
        if let Some((s, t)) = self.current.take() {
            self.steps.push((s, now_ms.saturating_sub(t).min(u32::MAX as u64) as u32));
        }
        let total = self.started.map(|t| now_ms.saturating_sub(t).min(u32::MAX as u64) as u32).unwrap_or(0);
        (last, total)
    }
}

/// What is known about the selection before the copy.
pub fn selection_text(sel: Option<crate::policy::Selection>) -> &'static str {
    match sel {
        Some(crate::policy::Selection::Empty) => "rien de sélectionné",
        Some(crate::policy::Selection::NonEmpty) => "texte sélectionné",
        _ => "inconnue",
    }
}

/// One F10/F9 press.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attempt {
    /// local time "hh:mm:ss", given by the host
    pub when: String,
    pub action: Option<Action>,
    /// the bubble message (None: nothing said, e.g. no active window)
    pub msg: Option<Msg>,
    /// technical line of debug.log (never the text itself)
    pub log: String,
    /// step where it stopped (Paste/WaitBeforeRestore/Restore for a success)
    pub stopped_at: Option<Step>,
    pub total_ms: u32,
    pub steps: Vec<(Step, u32)>,
    pub exe: String,
    pub window_class: String,
    pub focus_class: String,
    pub selection: &'static str,
    /// number of characters converted (never the text)
    pub chars: Option<usize>,
    /// keyboard layout of the target at the press (low 32 bits of the HKL)
    pub layout: Option<u32>,
    /// Latin layout used for the conversion
    pub latin: Option<&'static str>,
}

/// The last attempts, most recent last.
#[derive(Debug, Clone, Default)]
pub struct Ring {
    items: VecDeque<Attempt>,
}

impl Ring {
    pub fn push(&mut self, a: Attempt) {
        if self.items.len() == KEPT {
            self.items.pop_front();
        }
        self.items.push_back(a);
    }
    pub fn last(&self) -> Option<&Attempt> {
        self.items.back()
    }
    pub fn iter(&self) -> impl Iterator<Item = &Attempt> {
        self.items.iter()
    }
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Plain explanation of a refusal, with what to do, in the language of the interface (1.9.1:
/// it was always French; shown in the settings window and at the top of the copied report).
pub fn explain(a: &Attempt, lang: crate::i18n::Lang) -> String {
    use crate::i18n::Lang;
    use Msg::*;
    let (fr, en, he) = match a.msg {
        None => ("Aucune fenêtre active : rien n'a été fait.", "No active window: nothing was done.", "אין חלון פעיל: לא נעשה דבר."),
        Some(Converted) => ("La dernière correction a réussi.", "The last fix worked.", "התיקון האחרון הצליח."),
        Some(NothingToConvert) => ("Rien à corriger : le texte copié ne change pas, ou aucun texte n'a pu être copié (image, fichier, ou programme qui refuse Ctrl+C).", "Nothing to fix: the copied text does not change, or no text could be copied (image, file, or a program that refuses Ctrl+C).", "אין מה לתקן: הטקסט שהועתק נשאר כמו שהוא, או שלא ניתן היה להעתיק טקסט (תמונה, קובץ, או תוכנה שלא מאפשרת העתקה עם Ctrl+C).\u{200F}"),
        Some(SelectText) => ("Sélectionnez le texte puis réessayez (mode prudent, console, ou programme qui copie la ligne entière comme Excel ou VS Code).", "Select the text and try again (careful mode, console, or a program that copies the whole line, like Excel or VS Code).", "סמנו את הטקסט ונסו שוב (מצב זהיר, שורת פקודה, או תוכנה שמעתיקה את כל השורה, כמו Excel או VS Code).\u{200F}"),
        Some(ConsoleUnsupported) => ("Fenêtre de console non prise en charge (Terminal Windows, ou console désactivée dans les réglages).", "Console window not supported (Windows Terminal, or console support turned off in the settings).", "שורת פקודה שאינה נתמכת (Windows Terminal, או שהתמיכה בשורת הפקודה כבויה בהגדרות).\u{200F}"),
        Some(ConsoleBusy) => ("La console était occupée : rien n'a été changé.", "The console was busy: nothing was changed.", "שורת הפקודה הייתה עסוקה: לא שונה דבר."),
        Some(AdminWindow) => ("Fenêtre ouverte en administrateur : Windows interdit d'y envoyer des touches.", "Window running as administrator: Windows does not allow sending keys to it.", "החלון פתוח בהרשאות מנהל: Windows לא מאפשרת לשלוח אליו הקשות."),
        Some(PasswordField) => ("Champ de mot de passe : jamais corrigé, par sécurité.", "Password field: never fixed, for safety.", "שדה סיסמה: אף פעם לא מתוקן, מטעמי אבטחה."),
        Some(ClipboardBusy) => ("Le presse-papiers était occupé par un autre programme (gestionnaire de presse-papiers, historique Win+V…). Réessayez.", "The clipboard was busy with another program (clipboard manager, Win+V history…). Try again.", "לוח ההעתקה היה בשימוש של תוכנה אחרת (מנהל לוח העתקה, היסטוריית Win+V…). נסו שוב."),
        Some(ClipboardTooBig) => ("Le presse-papiers contient quelque chose de trop gros pour être sauvegardé : rien n'a été fait, pour ne pas le perdre.", "The clipboard holds something too big to save: nothing was done, so that it is not lost.", "בלוח ההעתקה יש משהו גדול מדי לשמירה: לא נעשה דבר, כדי לא לאבד אותו."),
        Some(WindowChanged) => ("La fenêtre ou le champ a changé pendant la correction : arrêté par prudence.", "The window or the field changed during the fix: stopped to be safe.", "החלון או השדה התחלפו במהלך התיקון: התיקון נעצר ליתר ביטחון."),
        Some(UserMoved) => ("Un clic ou une touche (Tab, Entrée, flèche…) a eu lieu pendant la correction : arrêté par prudence. Réessayez sans toucher la souris ni le clavier.", "A click or a key (Tab, Enter, arrow…) came during the fix: stopped to be safe. Try again without touching the mouse or the keyboard.", "במהלך התיקון נלחץ לחצן עכבר או מקש (Tab, Enter, חץ…): התיקון נעצר ליתר ביטחון. נסו שוב בלי לגעת בעכבר ובמקלדת."),
        Some(ExcludedProgram) => ("Ce programme est exclu (liste des programmes exclus ou règle de l'application).", "This program is excluded (excluded programs list, or a rule for this program).", "התוכנה הזו לא נכללת בתיקון (היא ברשימת התוכנות המוחרגות, או שנקבע לה כלל מיוחד)."),
        Some(KeysHeld) => ("Des touches étaient encore enfoncées : relâchez tout puis réessayez.", "Keys were still held down: release everything and try again.", "מקשים עדיין היו לחוצים: שחררו הכול ונסו שוב."),
        Some(TooLong) => ("Texte plus long que la limite réglée.", "Text longer than the limit set.", "הטקסט ארוך מהמגבלה שנקבעה."),
        Some(PasteFailed) => ("Le texte corrigé n'a pas pu être mis dans le presse-papiers : rien n'a été collé.", "The fixed text could not be put in the clipboard: nothing was pasted.", "לא ניתן היה להעתיק את הטקסט המתוקן ללוח: שום דבר לא הודבק."),
        Some(KeysBlocked) => ("Windows a bloqué les touches simulées (fenêtre protégée ou programme de sécurité).", "Windows blocked the simulated keys (protected window or security program).", "ההקשות שהתוכנה שולחת נחסמו על ידי Windows (חלון מוגן או תוכנת אבטחה)."),
        Some(NewCopyKept) => ("Une autre copie a eu lieu pendant la correction : rien n'a été collé, la copie a été gardée.", "Another copy happened during the fix: nothing was pasted, and that copy was kept.", "בוצעה העתקה אחרת במהלך התיקון: שום דבר לא הודבק, ומה שהועתק נשאר בלוח."),
        Some(ClipboardNotRestored) => ("La correction a eu lieu, mais le presse-papiers d'avant n'a pas pu être remis.", "The fix was done, but the previous clipboard could not be put back.", "התיקון בוצע, אבל לא ניתן היה להחזיר ללוח את התוכן הקודם שלו."),
        Some(LayoutUnsupported) => ("Le clavier actif n'est ni hébreu ni latin (russe, arabe…), c'est une variante hébraïque aux lettres déplacées, ou un clavier français AZERTY dont la table n'a pas pu être construite (voir le journal) : rien n'a été fait.", "The active keyboard is neither Hebrew nor Latin (Russian, Arabic…), is a Hebrew variant with moved letters, or is a French AZERTY layout whose table could not be built (see the log): nothing was done.", "פריסת המקלדת הפעילה אינה עברית ואינה באותיות לטיניות (רוסית, ערבית…), או שזו פריסה עברית שבה האותיות נמצאות במקומות אחרים, או מקלדת צרפתית AZERTY שלא ניתן היה לבנות עבורה את טבלת המקשים (פרטים ביומן): לא נעשה דבר."),
        Some(_) => ("Raison inconnue.", "Unknown reason.", "סיבה לא ידועה."),
    };
    match lang {
        Lang::Fr => fr,
        Lang::En => en,
        Lang::He => he,
    }
    .to_string()
}

/// Facts about the PC and the program, given by the host (all optional: "inconnu").
#[derive(Debug, Clone, Default)]
pub struct Environment {
    pub version: String,
    pub windows: Option<String>,
    pub ui_language: String,
    /// installed keyboard layouts (low 32 bits of each HKL), in Windows order
    pub layouts: Vec<u32>,
    pub hotkey: String,
    pub hotkey_state: String,
    pub case_hotkey: String,
    pub case_hotkey_state: String,
    pub settings: Vec<(String, String)>,
}

fn or_unknown(s: &str) -> &str {
    if s.is_empty() { "inconnu" } else { s }
}

fn or_unknown_in(s: &str, fr: bool) -> &str {
    if !s.is_empty() {
        s
    } else if fr {
        "inconnu"
    } else {
        "unknown"
    }
}

/// The selection as noted at the time (French words), in the language of the report.
fn selection_in(s: &str, fr: bool) -> &str {
    if fr {
        return or_unknown(s);
    }
    match s {
        "rien de sélectionné" => "nothing selected",
        "texte sélectionné" => "text selected",
        _ => "unknown",
    }
}

/// Lines describing the installed layouts and whether each is supported.
pub fn layout_lines(layouts: &[u32]) -> Vec<String> {
    layout_lines_in(layouts, true)
}

pub fn layout_lines_in(layouts: &[u32], fr: bool) -> Vec<String> {
    use crate::layouts::{classify_hkl, hkl_name, LayoutKind};
    if layouts.is_empty() {
        return vec![(if fr { "  inconnus" } else { "  unknown" }).into()];
    }
    layouts
        .iter()
        .map(|h| {
            let ok = match (classify_hkl(*h), fr) {
                (LayoutKind::Unsupported, true) => "NON pris en charge",
                (_, true) => "pris en charge",
                (LayoutKind::Unsupported, false) => "NOT supported",
                (_, false) => "supported",
            };
            format!("  - {} : {ok}", hkl_name(*h))
        })
        .collect()
}

/// Problems with the installed layouts, in plain words (empty = nothing to say).
pub fn layout_warnings(layouts: &[u32]) -> Vec<String> {
    layout_warnings_in(layouts, true)
}

pub fn layout_warnings_in(layouts: &[u32], fr: bool) -> Vec<String> {
    use crate::layouts::{classify_hkl, hkl_name, LayoutKind};
    let mut out = Vec::new();
    if layouts.is_empty() {
        return out;
    }
    if !layouts.iter().any(|h| classify_hkl(*h) == LayoutKind::HebrewStandard) {
        out.push((if fr { "Pas de clavier hébreu classique installé : la correction reste possible (une variante dont les lettres sont aux mêmes places est acceptée), mais le changement de langue ne pourra pas viser ce clavier." } else { "No standard Hebrew keyboard installed: fixing still works (a variant with the letters in the same places is accepted), but switching the language cannot select that keyboard." }).into());
    }
    if !layouts.iter().any(|h| matches!(classify_hkl(*h), LayoutKind::Latin(_))) {
        out.push((if fr { "Aucun clavier latin pris en charge (anglais États-Unis ou français AZERTY) : la table anglais (États-Unis) est utilisée." } else { "No supported Latin keyboard (English United States or French AZERTY): the English (United States) table is used." }).into());
    }
    for h in layouts.iter().filter(|h| classify_hkl(**h) == LayoutKind::Unsupported) {
        out.push(if fr {
            format!("{} : non pris en charge par son nom ; s'il est latin, la table anglais (États-Unis) est utilisée (ponctuation possiblement fausse) ; une variante hébraïque est acceptée si ses lettres sont aux mêmes places ; sinon (russe, arabe…) F10 ne fait rien quand il est actif.", hkl_name(*h))
        } else {
            format!("{}: not supported by its name; if it is Latin, the English (United States) table is used (punctuation may be wrong); a Hebrew variant is accepted if its letters are in the same places; otherwise (Russian, Arabic…) F10 does nothing while it is active.", hkl_name(*h))
        });
    }
    out
}

/// The text of "Copy the diagnostic information": French, or English (1.9.1: for the Hebrew and
/// English interfaces; this technical text is shown left to right).
pub fn report(env: &Environment, ring: &Ring) -> String {
    report_in(env, ring, crate::i18n::Lang::Fr)
}

pub fn report_in(env: &Environment, ring: &Ring, lang: crate::i18n::Lang) -> String {
    let fr = lang == crate::i18n::Lang::Fr;
    let t = |f: &'static str, e: &'static str| if fr { f } else { e };
    let u = |s: &str| -> String { or_unknown_in(s, fr).to_string() };
    let mut r = String::new();
    r.push_str(t("Keyboard Layout Fixer - informations de diagnostic\n", "Keyboard Layout Fixer - diagnostic information\n"));
    r.push_str(t("(aucun texte tapé, aucun contenu du presse-papiers, aucun titre de fenêtre ni nom de compte)\n\n", "(no typed text, no clipboard content, no window title, no account name)\n\n"));
    r.push_str(&format!("Version : {}\n", u(&env.version)));
    r.push_str(&format!("Windows : {}\n", env.windows.clone().unwrap_or_else(|| u(""))));
    r.push_str(&format!("{} : {}\n", t("Langue de l'interface", "Interface language"), u(&env.ui_language)));
    r.push_str(&format!("{} : {} ({})\n", t("Touche langue", "Language key"), u(&env.hotkey), u(&env.hotkey_state)));
    r.push_str(&format!("{} : {} ({})\n", t("Touche majuscules", "Caps key"), u(&env.case_hotkey), u(&env.case_hotkey_state)));
    r.push_str(t("Claviers installés :\n", "Installed keyboards:\n"));
    for l in layout_lines_in(&env.layouts, fr) {
        r.push_str(&l);
        r.push('\n');
    }
    for w in layout_warnings_in(&env.layouts, fr) {
        r.push_str("  ! ");
        r.push_str(&w);
        r.push('\n');
    }
    if !env.settings.is_empty() {
        r.push_str(t("Réglages :\n", "Settings:\n"));
        for (k, v) in &env.settings {
            r.push_str(&format!("  {k} = {v}\n"));
        }
    }
    r.push_str(&format!("\n{} ({}, {}) :\n", t("Dernières tentatives", "Last attempts"), ring.len(), t("la plus récente en dernier", "most recent last")));
    if ring.is_empty() {
        r.push_str(t("  aucune depuis le démarrage du programme\n", "  none since the program started\n"));
    }
    for (i, a) in ring.iter().enumerate() {
        let action = match a.action {
            Some(Action::Layout) => t("langue", "language"),
            Some(Action::Case) => t("majuscules", "caps"),
            None => t("inconnue", "unknown"),
        };
        r.push_str(&format!("{}. {} - {} {action} - {}\n", i + 1, u(&a.when), t("touche", "key"), explain(a, if fr { crate::i18n::Lang::Fr } else { crate::i18n::Lang::En })));
        r.push_str(&format!("   {} : {} ; {} : {} ; {} : {}\n", t("programme", "program"), u(&a.exe), t("fenêtre", "window"), u(&a.window_class), t("champ", "field"), u(&a.focus_class)));
        r.push_str(&format!(
            "   {} : {} ; {} : {} ; {} : {} ; {} : {}\n",
            t("sélection", "selection"),
            selection_in(a.selection, fr),
            t("caractères", "characters"),
            a.chars.map(|n| n.to_string()).unwrap_or_else(|| u("")),
            t("clavier", "keyboard"),
            a.layout.map(crate::layouts::hkl_name).unwrap_or_else(|| u("")),
            t("table", "table"),
            a.latin.map(str::to_string).unwrap_or_else(|| u("")),
        ));
        r.push_str(&format!("   {} : {} ; {} : {} ms\n", t("arrêt à l'étape", "stopped at step"), a.stopped_at.map(|s| s.name_in(fr).to_string()).unwrap_or_else(|| u("")), t("durée totale", "total time"), a.total_ms));
        if !a.steps.is_empty() {
            let s: Vec<String> = a.steps.iter().map(|(st, ms)| format!("{} {ms} ms", st.name_in(fr))).collect();
            r.push_str(&format!("   {} : {}\n", t("étapes", "steps"), s.join(", ")));
        }
        r.push_str(&format!("   {} : {}\n", t("journal", "log"), a.log));
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_measures_each_step() {
        let mut t = Trace::default();
        t.mark(Step::Checks, 100);
        t.mark(Step::Copy, 130);
        t.mark(Step::Paste, 400);
        let (last, total) = t.finish(1400);
        assert_eq!(last, Some(Step::Paste));
        assert_eq!(total, 1300);
        assert_eq!(t.steps, vec![(Step::Checks, 30), (Step::Copy, 270), (Step::Paste, 1000)]);
    }

    #[test]
    fn ring_keeps_the_last_ten() {
        let mut r = Ring::default();
        for i in 0..15 {
            r.push(Attempt { when: format!("{i}"), ..Default::default() });
        }
        assert_eq!(r.len(), KEPT);
        assert_eq!(r.iter().next().unwrap().when, "5");
        assert_eq!(r.last().unwrap().when, "14");
    }

    #[test]
    fn report_says_unknown_and_never_contains_text() {
        let mut ring = Ring::default();
        ring.push(Attempt {
            when: "10:00:00".into(),
            action: Some(Action::Layout),
            msg: Some(Msg::PasswordField),
            log: "Password field, nothing done".into(),
            exe: "chrome.exe".into(),
            selection: "inconnue",
            ..Default::default()
        });
        let env = Environment { version: "1.4.0".into(), layouts: vec![0x040D_040D, 0x0809_0809], ..Default::default() };
        let r = report(&env, &ring);
        assert!(r.contains("Windows : inconnu"));
        assert!(r.contains("Champ de mot de passe"));
        assert!(r.contains("anglais (Royaume-Uni) (0x08090809) : NON pris en charge"));
        assert!(r.contains("table anglais (États-Unis) est utilisée"));
        assert!(r.contains("caractères : inconnu"));
        assert!(r.contains("Aucun clavier latin pris en charge"));
    }

    #[test]
    fn report_and_explanations_follow_the_interface() {
        let mut ring = Ring::default();
        ring.push(Attempt { when: "10:00:00".into(), action: Some(Action::Layout), msg: Some(Msg::PasswordField), selection: "texte sélectionné", ..Default::default() });
        let env = Environment { version: "1.9.1".into(), layouts: vec![0x0809_0809], ..Default::default() };
        let en = report_in(&env, &ring, crate::i18n::Lang::En);
        assert!(en.contains("diagnostic information") && en.contains("Windows : unknown") && en.contains("Password field") && en.contains("text selected"), "{en}");
        assert!(en.contains("NOT supported") && !en.contains("inconnu"), "{en}");
        // the Hebrew interface gets the English report (read left to right) but a Hebrew explanation
        let he = report_in(&env, &ring, crate::i18n::Lang::He);
        assert!(he.contains("diagnostic information"));
        let heb = |c: char| ('\u{0590}'..='\u{05FF}').contains(&c);
        for m in crate::i18n::ALL_MSGS.iter().copied().map(Some).chain([None]) {
            let a = Attempt { msg: m, ..Default::default() };
            let e = explain(&a, crate::i18n::Lang::He);
            assert!(e.chars().any(heb), "{m:?}");
            let first = e.chars().next().unwrap();
            assert!(!first.is_ascii_alphanumeric(), "{m:?} starts with Latin: {e}");
            assert!(!explain(&a, crate::i18n::Lang::En).chars().any(heb), "{m:?}");
        }
    }

    #[test]
    fn every_bubble_message_has_an_explanation() {
        for m in crate::i18n::ALL_MSGS {
            let a = Attempt { msg: Some(*m), ..Default::default() };
            let e = explain(&a, crate::i18n::Lang::Fr);
            assert!(!e.is_empty());
        }
    }
}
