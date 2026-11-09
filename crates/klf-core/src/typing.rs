//! Correction while typing (1.8.0, OPTION off by default, public version only): pure decisions.
//! The program keeps only the last few words typed (memory only, cleared on click, arrows,
//! shortcuts, Enter, app switch) and proposes a fix only when the reading of those keys on the
//! other layout is a REAL word and what was typed is not: names, brands and typos are left alone.
//! The word lists are separate data files (see THIRD_PARTY_NOTICES.md), never compiled in.

use std::collections::HashSet;

/// Hebrew one-letter prefixes (ו ה ש ב ל מ כ): "והלכתי" is a word if "הלכתי" is.
const HE_PREFIXES: &[char] = &['ו', 'ה', 'ש', 'ב', 'ל', 'מ', 'כ'];
/// At most this many words are kept and fixed together.
pub const MAX_WORDS: usize = 6;
/// A word longer than this is not a typing mistake worth a proposal.
pub const MAX_WORD_CHARS: usize = 30;

#[derive(Debug, Default)]
pub struct Dict {
    he: HashSet<String>,
    en: HashSet<String>,
}

impl Dict {
    /// Lists of "word [count]" lines, most frequent first; only the first `max` words are kept.
    pub fn from_lists(he: &str, en: &str, max: usize) -> Dict {
        let take = |t: &str, lower: bool| -> HashSet<String> {
            t.lines()
                .filter_map(|l| l.split_whitespace().next())
                .filter(|w| !w.is_empty() && w.chars().count() <= MAX_WORD_CHARS)
                .take(max)
                .map(|w| if lower { w.to_lowercase() } else { w.to_string() })
                .collect()
        };
        Dict { he: take(he, false), en: take(en, true) }
    }
    pub fn is_empty(&self) -> bool {
        self.he.is_empty() || self.en.is_empty()
    }
    /// The word itself is in the Hebrew list (no prefix removed): for a proposal, which must be
    /// sure (prefixed forms of common words are in the frequency list anyway).
    pub fn is_he_exact(&self, w: &str) -> bool {
        self.he.contains(w)
    }
    pub fn is_en(&self, w: &str) -> bool {
        self.en.contains(&w.to_lowercase())
    }
    pub fn is_he(&self, w: &str) -> bool {
        if self.he.contains(w) {
            return true;
        }
        // up to two prefixes ("ושב...", "וה...")
        let mut s = w;
        for _ in 0..2 {
            let mut it = s.chars();
            match it.next() {
                Some(c) if HE_PREFIXES.contains(&c) && it.as_str().chars().count() >= 2 => {
                    s = it.as_str();
                    if self.he.contains(s) {
                        return true;
                    }
                }
                _ => break,
            }
        }
        false
    }
}

/// The fix proposed: erase `erase` characters before the caret (the words and the space typed
/// after them) and type `text` instead; `to_hebrew`: the keyboard then switches to Hebrew.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub erase: usize,
    pub text: String,
    pub to_hebrew: bool,
    /// the keyboard is left as it is (a capitals fix, not a layout fix)
    pub keep_layout: bool,
}

fn letters_only(w: &str, hebrew: bool) -> bool {
    !w.is_empty() && w.chars().all(|c| if hebrew { crate::layouts::is_hebrew_letter(c) || c == '\'' || c == '"' } else { c.is_alphabetic() && !crate::layouts::is_hebrew_letter(c) || c == '\'' })
}

/// `words`: the last words typed, oldest first, each as (typed, reading on the other layout);
/// they were all typed with the same layout, `hebrew_active` telling which. A space was just
/// typed after the last one.
pub fn suggest(words: &[(String, String)], hebrew_active: bool, dict: &Dict) -> Option<Suggestion> {
    if dict.is_empty() || words.is_empty() {
        return None;
    }
    let wrong = |typed: &str, read: &str| -> bool {
        if hebrew_active {
            letters_only(read, false) && !dict.is_he(typed) && dict.is_en(read)
        } else {
            letters_only(read, true) && !dict.is_en(typed) && dict.is_he_exact(read)
        }
    };
    let len = |s: &str| s.chars().count();
    let n = words.len();
    let flags: Vec<bool> = words.iter().map(|(t, r)| len(t) >= 2 && wrong(t, r)).collect();
    // the last word must be clearly wrong: 3 letters or more, or 2 letters next to a wrong word
    let (lt, _) = &words[n - 1];
    if !flags[n - 1] || (len(lt) < 3 && !(n >= 2 && flags[n - 2] && len(&words[n - 2].0) >= 3)) {
        return None;
    }
    let mut start = n - 1;
    while start > 0 && flags[start - 1] {
        start -= 1;
    }
    let run = &words[start..];
    let erase = run.iter().map(|(t, _)| len(t)).sum::<usize>() + run.len(); // each word + its space
    let text = run.iter().map(|(_, r)| r.as_str()).collect::<Vec<_>>().join(" ") + " ";
    Some(Suggestion { erase, text, to_hebrew: !hebrew_active, keep_layout: false })
}

/// "SHalom" -> "Shalom": two capitals then only small letters (4 letters or more, Latin), the
/// usual slip of Shift. Not for words in capitals, nor mixed forms such as "iPhone" or "McD".
pub fn two_capitals(word: &str) -> Option<String> {
    let c: Vec<char> = word.chars().collect();
    // 4 letters or more: "PCs", "IDs", "MHz", "GHz" stay as they are
    if c.len() < 4 || !c.iter().all(|x| x.is_alphabetic() && !crate::layouts::is_hebrew_letter(*x)) {
        return None;
    }
    if c[0].is_uppercase() && c[1].is_uppercase() && c[2..].iter().all(|x| x.is_lowercase()) {
        let mut out = String::new();
        out.push(c[0]);
        out.extend(c[1].to_lowercase());
        out.extend(c[2..].iter());
        return Some(out);
    }
    None
}

/// After a Space: a layout fix if there is one, else a "TWo capitals" fix of the last word.
pub fn suggest_any(words: &[(String, String)], hebrew_active: bool, dict: &Dict) -> Option<Suggestion> {
    if let Some(s) = suggest(words, hebrew_active, dict) {
        return Some(s);
    }
    let (last, _) = words.last()?;
    if hebrew_active {
        return None;
    }
    let fixed = two_capitals(last)?;
    Some(Suggestion { erase: last.chars().count() + 1, text: format!("{fixed} "), to_hebrew: false, keep_layout: true })
}

/// The words being typed: letters are added, Backspace removes, Space ends a word. Anything
/// else (click, arrows, Enter, shortcuts, another window, a pause) clears everything.
#[derive(Debug, Default)]
pub struct Buffer {
    current: String,
    words: Vec<String>,
    hebrew: Option<bool>,
}

impl Buffer {
    pub fn clear(&mut self) {
        self.current.clear();
        self.words.clear();
        self.hebrew = None;
    }
    /// A character typed with the Hebrew (`hebrew`) or the Latin layout.
    pub fn push(&mut self, c: char, hebrew: bool) {
        if self.hebrew.is_some_and(|h| h != hebrew) {
            self.clear(); // the layout changed: a new run starts
        }
        self.hebrew = Some(hebrew);
        if self.current.chars().count() >= MAX_WORD_CHARS {
            self.clear();
            return;
        }
        self.current.push(c);
    }
    pub fn backspace(&mut self) {
        if self.current.pop().is_none() {
            // back into the previous word: simpler and safer to start again
            self.clear();
        }
    }
    /// Nothing typed after the last space, and the last words are exactly `words`.
    pub fn ends_with_words(&self, words: &[String]) -> bool {
        self.current.is_empty() && self.words.len() >= words.len() && self.words[self.words.len() - words.len()..] == *words
    }
    /// Space typed: the current word ends. Some(words, hebrew) when there is something to check.
    pub fn space(&mut self) -> Option<(Vec<String>, bool)> {
        if self.current.is_empty() {
            self.clear(); // two spaces: start again
            return None;
        }
        self.words.push(std::mem::take(&mut self.current));
        if self.words.len() > MAX_WORDS {
            self.words.remove(0);
        }
        Some((self.words.clone(), self.hebrew?))
    }
}

/// Where the word lists are: a dedicated GitHub release (not "latest", so later releases need not
/// carry them), signed with the update key.
pub const WORDS_BASE: &str = "https://github.com/breslev558/hebrew-keyboard-layout-fixer/releases/download/words-1/";
pub const WORDS_MANIFEST: &str = "KeyboardLayoutFixer-words.manifest";
pub const WORDS_HE: &str = "KeyboardLayoutFixer-words-he.txt";
pub const WORDS_EN: &str = "KeyboardLayoutFixer-words-en.txt";
pub const MAX_WORDS_BYTES: usize = 2 * 1024 * 1024;
/// words kept from each list (most frequent first)
pub const DICT_SIZE: usize = 40_000;
const MANIFEST_HEADER: &str = "KeyboardLayoutFixer words 1";

/// The signed manifest: SHA-256 and size of the two lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordsManifest {
    pub he: (String, u64),
    pub en: (String, u64),
}

pub fn manifest_text(he_sha: &str, he_size: u64, en_sha: &str, en_size: u64) -> String {
    format!("{MANIFEST_HEADER}\nhe {} {he_size}\nen {} {en_size}\n", he_sha.to_ascii_lowercase(), en_sha.to_ascii_lowercase())
}

pub fn parse_manifest(text: &str) -> Option<WordsManifest> {
    if text.len() > 512 {
        return None;
    }
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    if lines.next()? != MANIFEST_HEADER {
        return None;
    }
    let (mut he, mut en) = (None, None);
    for l in lines {
        let mut p = l.split_whitespace();
        let (k, sha, size) = (p.next()?, crate::update::parse_sha_file(p.next()?)?, p.next()?.parse::<u64>().ok()?);
        if p.next().is_some() || size == 0 || size > MAX_WORDS_BYTES as u64 {
            return None;
        }
        match k {
            "he" if he.is_none() => he = Some((sha, size)),
            "en" if en.is_none() => en = Some((sha, size)),
            _ => return None,
        }
    }
    Some(WordsManifest { he: he?, en: en? })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict() -> Dict {
        Dict::from_lists("שלום 10\nמה 9\nאתה 8\nהלכתי 7\nאוכל 6\nכלב 5\n", "hello 10\nwant 9\nto 8\ngrab 7\nlunch 6\ntoday 5\ngithub 4\nthe 3\n", 1000)
    }
    fn w(t: &str, r: &str) -> (String, String) {
        (t.to_string(), r.to_string())
    }

    #[test]
    fn latin_keys_that_spell_a_hebrew_word() {
        let s = suggest(&[w("akuo", "שלום")], false, &dict()).unwrap();
        assert_eq!(s, Suggestion { erase: 5, text: "שלום ".into(), to_hebrew: true, keep_layout: false });
    }

    #[test]
    fn hebrew_keys_that_spell_an_english_word_and_the_whole_run() {
        let words = [w("שגשנ", "grab"), w("ךוmבי", "lunch"), w("אםגשט", "today")];
        let s = suggest(&words, true, &dict()).unwrap();
        assert_eq!(s.text, "grab lunch today ");
        assert_eq!(s.erase, 4 + 5 + 5 + 3);
        assert!(!s.to_hebrew);
    }

    #[test]
    fn names_brands_typos_and_real_words_are_left_alone() {
        let d = dict();
        assert!(suggest(&[w("github", "עןאוהנ")], false, &d).is_none(), "an English word is never touched");
        assert!(suggest(&[w("teh", "אקי")], false, &d).is_none(), "a typo whose other reading is no word");
        assert!(suggest(&[w("שלום", "akuo")], true, &d).is_none(), "a real Hebrew word stays");
        assert!(suggest(&[w("ab", "שנ")], false, &d).is_none(), "two letters alone: never");
    }

    #[test]
    fn hebrew_prefixes_count_as_words() {
        let d = dict();
        assert!(d.is_he("והלכתי") && !d.is_he("וכשהלכתי") && d.is_he("שהלכתי"));
        assert!(suggest(&[w("וקט", "and")], true, &d).is_none(), "not an English word: nothing");
    }

    #[test]
    fn two_capitals_slip() {
        assert_eq!(two_capitals("SHalom").as_deref(), Some("Shalom"));
        assert_eq!(two_capitals("HEllo").as_deref(), Some("Hello"));
        for w in ["SHALOM", "iPhone", "McDonald", "OK", "Shalom", "shalom", "SHa1om", "PCs", "IDs", "MHz", "GHz"] {
            assert_eq!(two_capitals(w), None, "{w}");
        }
        let s = suggest_any(&[w("HEllo", "ןקךךם")], false, &dict()).unwrap();
        assert_eq!((s.erase, s.text.as_str(), s.keep_layout), (6, "Hello ", true));
        assert!(suggest_any(&[w("HEllo", "x")], true, &dict()).is_none(), "Hebrew layout: no capitals fix");
    }

    #[test]
    fn signed_manifest_of_the_word_lists() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let m = parse_manifest(&manifest_text(&a, 300_000, &b, 250_000)).unwrap();
        assert_eq!(m, WordsManifest { he: (a.clone(), 300_000), en: (b.clone(), 250_000) });
        assert!(parse_manifest(&format!("he {a} 1\nen {b} 1")).is_none(), "header required");
        assert!(parse_manifest(&manifest_text(&a, 9_000_000, &b, 1)).is_none(), "too big");
        assert!(parse_manifest(&format!("{}he {a} 1\n", manifest_text(&a, 1, &b, 1))).is_none(), "no duplicate");
    }

    #[test]
    fn buffer_keeps_only_a_short_run_and_clears_on_change() {
        let mut b = Buffer::default();
        for c in "akuo".chars() {
            b.push(c, false);
        }
        assert_eq!(b.space(), Some((vec!["akuo".to_string()], false)));
        assert!(b.ends_with_words(&["akuo".to_string()]));
        b.push('x', false);
        assert!(!b.ends_with_words(&["akuo".to_string()]), "a letter typed after the space: the proposal is stale");
        b.backspace();
        b.push('ש', true);
        b.push('ל', true);
        assert_eq!(b.space(), Some((vec!["של".to_string()], true)), "another layout: a new run");
        assert_eq!(b.space(), None, "two spaces: cleared");
        b.push('a', false);
        b.backspace();
        b.backspace();
        assert_eq!(b.space(), None);
        for i in 0..10 {
            b.push('x', false);
            let r = b.space().unwrap();
            assert!(r.0.len() <= MAX_WORDS, "{i}");
        }
    }
}

/// Measure on the real lists (data/words + the 50k source files if present):
/// cargo test -p klf-core --release typing_measure -- --ignored --nocapture
#[cfg(test)]
mod measure {
    use super::*;
    use crate::convert::{convert_with, us_map};
    use crate::settings::MixedTextMode;

    fn read(p: &str) -> Option<String> {
        std::fs::read_to_string(p).ok()
    }

    #[test]
    #[ignore]
    fn typing_measure() {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/words/");
        let (Some(he), Some(en)) = (read(&format!("{base}KeyboardLayoutFixer-words-he.txt")), read(&format!("{base}KeyboardLayoutFixer-words-en.txt"))) else { return };
        let d = Dict::from_lists(&he, &en, DICT_SIZE);
        let m = us_map();
        let conv = |s: &str| convert_with(&m, s, MixedTextMode::ToggleAll, true);
        let mut rows = Vec::new();
        for (name, list, hebrew_word) in [("hébreu", &he, true), ("anglais", &en, false)] {
            let words: Vec<&str> = list.lines().filter(|w| w.chars().count() >= 3).collect();
            let (mut fp, mut fix, mut n) = (0, 0, 0);
            for w in &words {
                n += 1;
                let other = conv(w);
                // typed correctly: nothing must be proposed
                if suggest(&[(w.to_string(), other.clone())], hebrew_word, &d).is_some() {
                    fp += 1;
                }
                // typed with the wrong layout: the fix should be proposed
                if suggest(&[(other.clone(), w.to_string())], !hebrew_word, &d).is_some_and(|s| s.text.trim() == *w) {
                    fix += 1;
                }
            }
            rows.push(format!("{name} : {n} mots (3 lettres et plus) ; corrigés si tapés sur le mauvais clavier : {:.1} % ; fausses alertes si tapés correctement : {:.2} %", 100.0 * fix as f64 / n as f64, 100.0 * fp as f64 / n as f64));
        }
        // rarer words, NOT in the lists (ranks 40 001 to 50 000 of the source files), if present
        if let Ok(dir) = std::env::var("KLF_WORDS_50K") {
            for (name, file, hebrew_word) in [("hébreu rare", "he_50k.txt", true), ("anglais rare", "en_50k.txt", false)] {
                let Some(t) = read(&format!("{dir}/{file}")) else { continue };
                let words: Vec<String> = t.lines().skip(40_000).filter_map(|l| l.split_whitespace().next()).filter(|w| w.chars().count() >= 3 && w.chars().all(|c| c.is_alphabetic())).map(String::from).collect();
                let (mut fp, mut fix) = (0, 0);
                for w in &words {
                    let other = conv(w);
                    if suggest(&[(w.clone(), other.clone())], hebrew_word, &d).is_some() {
                        fp += 1;
                    }
                    if suggest(&[(other.clone(), w.clone())], !hebrew_word, &d).is_some() {
                        fix += 1;
                    }
                }
                let n = words.len().max(1);
                rows.push(format!("{name} : {n} mots absents des listes ; corrigés : {:.1} % ; fausses alertes : {:.2} %", 100.0 * fix as f64 / n as f64, 100.0 * fp as f64 / n as f64));
            }
        }
        for r in rows {
            println!("{r}");
        }
    }
}
