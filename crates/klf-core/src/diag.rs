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

/// Plain explanation of a refusal, in French, with what to do (shown in the settings window and
/// at the top of the copied report).
pub fn explain(a: &Attempt) -> String {
    use Msg::*;
    let why = match a.msg {
        None => "Aucune fenêtre active : rien n'a été fait.",
        Some(Converted) => "La dernière correction a réussi.",
        Some(NothingToConvert) => "Rien à corriger : le texte copié ne change pas, ou aucun texte n'a pu être copié (image, fichier, ou programme qui refuse Ctrl+C).",
        Some(SelectText) => "Sélectionnez le texte puis réessayez (mode prudent, console, ou programme qui copie la ligne entière comme Excel ou VS Code).",
        Some(ConsoleUnsupported) => "Fenêtre de console non prise en charge (Terminal Windows, ou console désactivée dans les réglages).",
        Some(ConsoleBusy) => "La console était occupée : rien n'a été changé.",
        Some(AdminWindow) => "Fenêtre ouverte en administrateur : Windows interdit d'y envoyer des touches.",
        Some(PasswordField) => "Champ de mot de passe : jamais corrigé, par sécurité.",
        Some(ClipboardBusy) => "Le presse-papiers était occupé par un autre programme (gestionnaire de presse-papiers, historique Win+V…). Réessayez.",
        Some(ClipboardTooBig) => "Le presse-papiers contient quelque chose de trop gros pour être sauvegardé : rien n'a été fait, pour ne pas le perdre.",
        Some(WindowChanged) => "La fenêtre ou le champ a changé pendant la correction (clic, Tab…) : arrêté par prudence.",
        Some(ExcludedProgram) => "Ce programme est exclu (liste des programmes exclus ou règle de l'application).",
        Some(KeysHeld) => "Des touches étaient encore enfoncées : relâchez tout puis réessayez.",
        Some(TooLong) => "Texte plus long que la limite réglée.",
        Some(PasteFailed) => "Le texte corrigé n'a pas pu être mis dans le presse-papiers : rien n'a été collé.",
        Some(KeysBlocked) => "Windows a bloqué les touches simulées (fenêtre protégée ou programme de sécurité).",
        Some(NewCopyKept) => "Une autre copie a eu lieu pendant la correction : rien n'a été collé, la copie a été gardée.",
        Some(ClipboardNotRestored) => "La correction a eu lieu, mais le presse-papiers d'avant n'a pas pu être remis.",
        Some(LayoutUnsupported) => "Le clavier actif n'est ni hébreu ni latin (russe, arabe…), c'est une variante hébraïque aux lettres déplacées, ou un clavier français AZERTY dont la table n'a pas pu être construite (voir le journal) : rien n'a été fait.",
        Some(_) => "Raison inconnue.",
    };
    why.to_string()
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

/// Lines describing the installed layouts and whether each is supported.
pub fn layout_lines(layouts: &[u32]) -> Vec<String> {
    use crate::layouts::{classify_hkl, hkl_name, LayoutKind};
    if layouts.is_empty() {
        return vec!["  inconnus".into()];
    }
    layouts
        .iter()
        .map(|h| {
            let ok = match classify_hkl(*h) {
                LayoutKind::Unsupported => "NON pris en charge",
                _ => "pris en charge",
            };
            format!("  - {} : {ok}", hkl_name(*h))
        })
        .collect()
}

/// Problems with the installed layouts, in plain words (empty = nothing to say).
pub fn layout_warnings(layouts: &[u32]) -> Vec<String> {
    use crate::layouts::{classify_hkl, hkl_name, LayoutKind};
    let mut out = Vec::new();
    if layouts.is_empty() {
        return out;
    }
    if !layouts.iter().any(|h| classify_hkl(*h) == LayoutKind::HebrewStandard) {
        out.push("Pas de clavier hébreu classique installé : la correction reste possible (une variante dont les lettres sont aux mêmes places est acceptée), mais le changement de langue ne pourra pas viser ce clavier.".into());
    }
    if !layouts.iter().any(|h| matches!(classify_hkl(*h), LayoutKind::Latin(_))) {
        out.push("Aucun clavier latin pris en charge (anglais États-Unis ou français AZERTY) : la table anglais (États-Unis) est utilisée.".into());
    }
    for h in layouts.iter().filter(|h| classify_hkl(**h) == LayoutKind::Unsupported) {
        out.push(format!("{} : non pris en charge par son nom ; s'il est latin, la table anglais (États-Unis) est utilisée (ponctuation possiblement fausse) ; une variante hébraïque est acceptée si ses lettres sont aux mêmes places ; sinon (russe, arabe…) F10 ne fait rien quand il est actif.", hkl_name(*h)));
    }
    out
}

/// The text of "Copy the diagnostic information".
pub fn report(env: &Environment, ring: &Ring) -> String {
    let mut r = String::new();
    r.push_str("Keyboard Layout Fixer NOAM - informations de diagnostic\n");
    r.push_str("(aucun texte tapé, aucun contenu du presse-papiers, aucun titre de fenêtre ni nom de compte)\n\n");
    r.push_str(&format!("Version : {}\n", or_unknown(&env.version)));
    r.push_str(&format!("Windows : {}\n", env.windows.as_deref().unwrap_or("inconnu")));
    r.push_str(&format!("Langue de l'interface : {}\n", or_unknown(&env.ui_language)));
    r.push_str(&format!("Touche langue : {} ({})\n", or_unknown(&env.hotkey), or_unknown(&env.hotkey_state)));
    r.push_str(&format!("Touche majuscules : {} ({})\n", or_unknown(&env.case_hotkey), or_unknown(&env.case_hotkey_state)));
    r.push_str("Claviers installés :\n");
    for l in layout_lines(&env.layouts) {
        r.push_str(&l);
        r.push('\n');
    }
    for w in layout_warnings(&env.layouts) {
        r.push_str("  ! ");
        r.push_str(&w);
        r.push('\n');
    }
    if !env.settings.is_empty() {
        r.push_str("Réglages :\n");
        for (k, v) in &env.settings {
            r.push_str(&format!("  {k} = {v}\n"));
        }
    }
    r.push_str(&format!("\nDernières tentatives ({}, la plus récente en dernier) :\n", ring.len()));
    if ring.is_empty() {
        r.push_str("  aucune depuis le démarrage du programme\n");
    }
    for (i, a) in ring.iter().enumerate() {
        let action = match a.action {
            Some(Action::Layout) => "langue",
            Some(Action::Case) => "majuscules",
            None => "inconnue",
        };
        r.push_str(&format!("{}. {} - touche {action} - {}\n", i + 1, or_unknown(&a.when), explain(a)));
        r.push_str(&format!("   programme : {} ; fenêtre : {} ; champ : {}\n", or_unknown(&a.exe), or_unknown(&a.window_class), or_unknown(&a.focus_class)));
        r.push_str(&format!(
            "   sélection : {} ; caractères : {} ; clavier : {} ; table : {}\n",
            or_unknown(a.selection),
            a.chars.map(|n| n.to_string()).unwrap_or_else(|| "inconnu".into()),
            a.layout.map(crate::layouts::hkl_name).unwrap_or_else(|| "inconnu".into()),
            a.latin.unwrap_or("inconnue"),
        ));
        r.push_str(&format!("   arrêt à l'étape : {} ; durée totale : {} ms\n", a.stopped_at.map(Step::name).unwrap_or("inconnue"), a.total_ms));
        if !a.steps.is_empty() {
            let s: Vec<String> = a.steps.iter().map(|(st, ms)| format!("{} {ms} ms", st.name())).collect();
            r.push_str(&format!("   étapes : {}\n", s.join(", ")));
        }
        r.push_str(&format!("   journal : {}\n", a.log));
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
    fn every_bubble_message_has_an_explanation() {
        for m in crate::i18n::ALL_MSGS {
            let a = Attempt { msg: Some(*m), ..Default::default() };
            let e = explain(&a);
            assert!(!e.is_empty());
        }
    }
}
