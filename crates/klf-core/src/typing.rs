//! Correction while typing (1.8.0, OPTION off by default, public version only): pure decisions.
//! The program keeps only the last few words typed (memory only, cleared on click, arrows,
//! shortcuts, Enter, app switch) and proposes a fix only when the reading of those keys on the
//! other layout is a REAL word and what was typed is not: names, brands and typos are left alone.
//! The word lists are separate data files (see THIRD_PARTY_NOTICES.md), never compiled in.

use std::collections::{HashMap, HashSet};

/// Hebrew one-letter prefixes (ו ה ש ב ל מ כ): "והלכתי" is a word if "הלכתי" is.
const HE_PREFIXES: &[char] = &['ו', 'ה', 'ש', 'ב', 'ל', 'מ', 'כ'];
/// At most this many words are kept and fixed together.
pub const MAX_WORDS: usize = 6;
/// A word longer than this is not a typing mistake worth a proposal.
pub const MAX_WORD_CHARS: usize = 30;

#[derive(Debug, Default)]
pub struct Dict {
    /// word -> rank (0 = most frequent)
    he: HashMap<String, u32>,
    en: HashMap<String, u32>,
    /// 1.8.1, personal dictionary (priority over the lists): words of the user, valid in either
    /// language; and words never to fix (learned when a fix is undone), in lower case
    personal: HashSet<String>,
    never: HashSet<String>,
    /// words the user fixed with F10 ("+word"): fixed at once next time
    learned: HashSet<String>,
    /// 1.8.2, letter models built from the lists, for words in no list
    he_model: crate::ngram::Model,
    en_model: crate::ngram::Model,
    /// (gap, min), NGRAM_GAP and NGRAM_MIN; changed only by the measurement
    ngram: (f64, f64),
}

/// 1.8.2, a word in no list: proposed only when the reading looks much more like its language
/// than what was typed looks like its own (average log-probability per letter), 5 letters or more.
pub const NGRAM_GAP: f64 = 1.5;
pub const NGRAM_MIN: f64 = -3.0;
pub const NGRAM_MIN_CHARS: usize = 5;
/// ...and what was typed must look bad in its own language (typos of real words do not).
pub const NGRAM_OWN_MAX: f64 = -3.6;
/// 1.8.2, short words fixed with the next one: rare on this layout, among the most common on the other.
pub const SHORT_OWN_RANK: u32 = 3_000;
pub const SHORT_OTHER_RANK: u32 = 300;
/// A Hebrew word with prefixes ("וכשהלכתי"): what was typed must also look less like English.
pub const PREFIX_GAP: f64 = 1.0;

/// The personal file: one word per line; "-word" = never fix this word as typed; "+word" = a word
/// fixed with F10, fixed at once next time. "#" comments.
pub fn parse_personal(text: &str) -> (HashSet<String>, HashSet<String>) {
    let p = parse_personal_full(text);
    (p.0, p.1)
}

pub fn parse_personal_full(text: &str) -> (HashSet<String>, HashSet<String>, HashSet<String>) {
    let (mut known, mut never, mut learned) = (HashSet::new(), HashSet::new(), HashSet::new());
    for l in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).take(10_000) {
        let (set, w) = match (l.strip_prefix('-'), l.strip_prefix('+')) {
            (Some(w), _) => (&mut never, w.trim()),
            (_, Some(w)) => (&mut learned, w.trim()),
            _ => (&mut known, l),
        };
        if !w.is_empty() {
            set.insert(w.to_lowercase());
        }
    }
    // a word never to fix wins over a learned one
    learned.retain(|w| !never.contains(w));
    (known, never, learned)
}

/// Line added to the personal file when a single word was fixed with F10.
pub fn learned_line(typed: &str) -> Option<String> {
    let w = typed.trim();
    (w.chars().count() >= 2 && w.chars().count() <= MAX_WORD_CHARS && w.chars().all(|c| c.is_alphabetic() || c == '\'')).then(|| format!("+{}", w.to_lowercase()))
}

/// Line added to the personal file when a fix is undone.
pub fn never_line(typed: &str) -> String {
    format!("-{}", typed.trim().to_lowercase())
}

impl Dict {
    /// Lists of "word [count]" lines, most frequent first; only the first `max` words are kept.
    pub fn from_lists(he: &str, en: &str, max: usize) -> Dict {
        let take = |t: &str, lower: bool| -> HashMap<String, u32> {
            let mut m = HashMap::new();
            for (i, w) in t.lines().filter_map(|l| l.split_whitespace().next()).filter(|w| !w.is_empty() && w.chars().count() <= MAX_WORD_CHARS).take(max).enumerate() {
                m.entry(if lower { w.to_lowercase() } else { w.to_string() }).or_insert(i as u32);
            }
            m
        };
        let model = |t: &str| crate::ngram::Model::train(t.lines().filter_map(|l| l.split_whitespace().next()).filter(|w| w.chars().count() <= MAX_WORD_CHARS).take(max));
        Dict { he: take(he, false), en: take(en, true), personal: HashSet::new(), never: HashSet::new(), learned: HashSet::new(), he_model: model(he), en_model: model(en), ngram: (NGRAM_GAP, NGRAM_MIN) }
    }
    /// The personal dictionary (replaces the previous one).
    pub fn set_personal(&mut self, text: &str) {
        let (k, n, l) = parse_personal_full(text);
        self.personal = k;
        self.never = n;
        self.learned = l;
    }
    fn learned(&self, w: &str) -> bool {
        self.learned.contains(&w.to_lowercase())
    }
    fn personal(&self, w: &str) -> bool {
        self.personal.contains(&w.to_lowercase())
    }
    fn never(&self, w: &str) -> bool {
        self.never.contains(&w.to_lowercase())
    }
    pub fn is_empty(&self) -> bool {
        self.he.is_empty() || self.en.is_empty()
    }
    /// The word itself is in the Hebrew list (no prefix removed): for a proposal, which must be
    /// sure (prefixed forms of common words are in the frequency list anyway).
    pub fn is_he_exact(&self, w: &str) -> bool {
        self.he.contains_key(w)
    }
    /// Rank in the list of that language (0 = most frequent).
    pub fn rank(&self, w: &str, hebrew: bool) -> Option<u32> {
        if hebrew { self.he.get(w).copied() } else { self.en.get(&w.to_lowercase()).copied() }
    }
    pub fn is_en(&self, w: &str) -> bool {
        self.en.contains_key(&w.to_lowercase())
    }
    /// A reading worth a proposal: short words must be common (3 letters: the 5 000 most
    /// frequent, 4 letters: 20 000), as rare short words (names, slang) collide with typos.
    pub fn common_enough(&self, read: &str, hebrew: bool) -> bool {
        let rank = if hebrew { self.he.get(read) } else { self.en.get(&read.to_lowercase()) };
        let Some(&r) = rank else { return false };
        match read.chars().count() {
            0..=3 => r < 5_000,
            4 => r < 20_000,
            _ => true,
        }
    }
    /// 1.8.2, a Hebrew reading made of one or two prefixes and a common word ("וכשהלכתי" is not
    /// in the list but "הלכתי" is): 3 letters or more left, and that word common enough.
    pub fn is_he_prefixed(&self, w: &str) -> bool {
        let mut s = w;
        for _ in 0..2 {
            let mut it = s.chars();
            match it.next() {
                Some(c) if HE_PREFIXES.contains(&c) && it.as_str().chars().count() >= 3 => {
                    s = it.as_str();
                    if self.he.get(s).is_some_and(|&r| r < 20_000) {
                        return true;
                    }
                }
                _ => break,
            }
        }
        false
    }
    /// 1.8.2, both words in no list: does the reading look like a word of its language and what
    /// was typed like nothing of its own?
    pub fn looks_swapped(&self, typed: &str, read: &str, hebrew_typed: bool, gap: f64, min: f64) -> bool {
        if read.chars().count() < NGRAM_MIN_CHARS || self.he_model.is_empty() || self.en_model.is_empty() {
            return false;
        }
        let (own, other) = if hebrew_typed { (&self.he_model, &self.en_model) } else { (&self.en_model, &self.he_model) };
        let (r, t) = (other.score(read), own.score(typed));
        r >= min && r - t >= gap && t <= NGRAM_OWN_MAX
    }
    pub fn is_he(&self, w: &str) -> bool {
        if self.he.contains_key(w) {
            return true;
        }
        // up to two prefixes ("ושב...", "וה...")
        let mut s = w;
        for _ in 0..2 {
            let mut it = s.chars();
            match it.next() {
                Some(c) if HE_PREFIXES.contains(&c) && it.as_str().chars().count() >= 2 => {
                    s = it.as_str();
                    if self.he.contains_key(s) {
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
    /// fixed without asking (a word the user taught with F10)
    pub auto: bool,
}

/// Code editors: no correction while typing there by default (F10 still works).
pub const CODE_EDITORS: &[&str] = &[
    "code.exe", "code - insiders.exe", "cursor.exe", "windsurf.exe", "devenv.exe", "sublime_text.exe", "notepad++.exe",
    "idea64.exe", "pycharm64.exe", "webstorm64.exe", "clion64.exe", "rider64.exe", "goland64.exe", "phpstorm64.exe",
    "rubymine64.exe", "datagrip64.exe", "studio64.exe", "rustrover64.exe", "dataspell64.exe", "aqua64.exe", "idea.exe", "pycharm.exe", "fleet.exe", "zed.exe", "atom.exe", "eclipse.exe", "vim.exe", "gvim.exe",
];

pub fn is_code_editor(exe: &str) -> bool {
    CODE_EDITORS.iter().any(|e| e.eq_ignore_ascii_case(exe))
}

/// A conversion made with F10 that the typing option may learn: one word, letters only, one
/// side Hebrew and the other Latin. Returns the line for the personal file.
pub fn f10_lesson(before: &str, after: &str) -> Option<String> {
    f10_lesson_checked(before, after, None)
}

/// With the lists: only when what was typed is not a word and the result is one (an F10 that
/// goes back, a part of a word or a real word converted by mistake teaches nothing), 3 letters
/// or more, and never a word of the personal list.
pub fn f10_lesson_checked(before: &str, after: &str, dict: Option<&Dict>) -> Option<String> {
    let (b, a) = (before.trim(), after.trim());
    if b.chars().count() < 3 {
        return None;
    }
    if let Some(d) = dict {
        let b_he = b.chars().any(crate::layouts::is_hebrew_letter);
        let b_word = if b_he { d.is_he(b) } else { d.is_en(b) };
        let a_word = if b_he { d.is_en(a) } else { d.is_he_exact(a) } || d.personal(a);
        if b_word || !a_word || d.personal(b) || d.never(b) {
            return None;
        }
    }
    let he = |s: &str| s.chars().all(|c| crate::layouts::is_hebrew_letter(c) || c == '\'' || c == '"');
    let la = |s: &str| s.chars().all(|c| (c.is_alphabetic() && !crate::layouts::is_hebrew_letter(c)) || c == '\'');
    let (b, a) = (before.trim(), after.trim());
    if b.contains(char::is_whitespace) || a.contains(char::is_whitespace) || !((he(b) && la(a)) || (la(b) && he(a))) {
        return None;
    }
    learned_line(b)
}

/// Keys that are Hebrew letters on one layout and signs on the other: kept inside a word.
pub const WORD_SIGNS: &[char] = &[',', '.', ';', '/', '\''];

/// Inside a word typed on the Latin layout only "," (ת) and "'" may appear (ץ and ף, from "." and
/// ";", are final letters); on the Hebrew layout only "/" (q) and "'" (w).
fn interior_signs_ok(core: &str, hebrew_typed: bool) -> bool {
    let allowed: &[char] = if hebrew_typed { &['/', '\''] } else { &[',', '\''] };
    core.chars().all(|c| !WORD_SIGNS.contains(&c) || allowed.contains(&c))
}

/// "NASA", "AKUO": capitals only (an acronym, or Caps Lock): never fixed while typing.
fn all_caps(w: &str) -> bool {
    let letters: Vec<char> = w.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase())
}

/// Looks like code or a name made of parts: "getElementById", "iPhone", "McDonald".
fn code_like(w: &str) -> bool {
    let letters: Vec<char> = w.chars().filter(|c| c.is_alphabetic()).collect();
    letters.iter().skip(1).any(|c| c.is_uppercase()) && !letters.iter().all(|c| c.is_uppercase())
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
        // personal dictionary first: a word of the user, or a word never to fix, is never wrong;
        // a reading that is a word of the user counts as a real word
        if dict.never(typed) || dict.personal(typed) || (!hebrew_active && (code_like(typed) || all_caps(typed))) {
            return false;
        }
        // 1.8.2: a word followed by a sign ("hello," "שלום."): the word itself decides; the keys
        // , . ; / ' are Hebrew letters on the other layout ("ac," = שבת), so they stay in the word
        let core = typed.trim_end_matches(WORD_SIGNS);
        let signs_after = core != typed;
        if signs_after && dict.personal(core) {
            return false;
        }
        // (an English word of the list stays, unless the reading is a word at least 4 times more
        // frequent: "ac," = שבת, rank 2 634, rather than "ac", rank 11 948)
        let read_rank = dict.rank(read, !hebrew_active).filter(|_| dict.common_enough(read, !hebrew_active));
        if signs_after && (if hebrew_active { dict.is_he(core) } else { dict.rank(core, false).is_some_and(|k| read_rank.is_none_or(|r| r.saturating_mul(4) >= k)) }) {
            return false;
        }
        // taught with F10: wrong whatever the lists say
        if dict.learned(typed) && letters_only(read, !hebrew_active) {
            return true;
        }
        let typed_is_word = if hebrew_active { dict.is_he(typed) } else { dict.is_en(typed) };
        if dict.personal(read) && letters_only(read, !hebrew_active) && !typed_is_word {
            return true;
        }
        if typed_is_word || !letters_only(read, !hebrew_active) {
            return false;
        }
        if dict.common_enough(read, !hebrew_active) {
            return true;
        }
        // 1.8.2: the reading is in no list (a rare word, a Hebrew word with prefixes)
        let read_listed = if hebrew_active { dict.is_en(read) } else { dict.is_he_exact(read) };
        if read_listed {
            return false; // a rare short word of the list: left alone, as before
        }
        // "hello,world", "yes,no": words of the list joined by a comma
        if !hebrew_active && core.contains(',') && core.split(',').all(|p| p.is_empty() || dict.is_en(p)) {
            return false;
        }
        // a sign INSIDE what was typed that is no letter on the other layout ("file.txt",
        // "google.com"): the letter models would be fooled, so no proposal without the lists
        if !interior_signs_ok(core, hebrew_active) {
            return false;
        }
        // what was typed is judged without the signs after it ("kauai," as "kauai"): a word and
        // a comma is never less English than the word; the reading keeps them (ת ץ ף...)
        (!hebrew_active && dict.ngram.0.is_finite() && dict.is_he_prefixed(read) && dict.looks_swapped(core, read, false, PREFIX_GAP, f64::NEG_INFINITY))
            || (!core.is_empty() && dict.looks_swapped(core, read, hebrew_active, dict.ngram.0, dict.ngram.1))
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
    // 1.8.2: a short word that happens to be a rare word here ("ak", "gk") but a very common one
    // on the other layout (של, על) joins the run when the word after it is wrong
    let soft = |(t, r): &(String, String)| -> bool {
        let n = len(t);
        (2..=3).contains(&n) && !dict.never(t) && !dict.personal(t) && !(!hebrew_active && (all_caps(t) || code_like(t))) && letters_only(t, hebrew_active) && dict.rank(t, hebrew_active).is_none_or(|k| k >= SHORT_OWN_RANK) && dict.rank(r, !hebrew_active).is_some_and(|k| k < SHORT_OTHER_RANK)
    };
    while start > 0 && (flags[start - 1] || soft(&words[start - 1])) {
        start -= 1;
    }
    let run = &words[start..];
    let erase = run.iter().map(|(t, _)| len(t)).sum::<usize>() + run.len(); // each word + its space
    let text = run.iter().map(|(_, r)| r.as_str()).collect::<Vec<_>>().join(" ") + " ";
    // a single word taught with F10 is fixed at once (Backspace still undoes it)
    let auto = run.len() == 1 && dict.learned(&run[0].0);
    Some(Suggestion { erase, text, to_hebrew: !hebrew_active, keep_layout: false, auto })
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
    if hebrew_active || dict.never(last) {
        return None;
    }
    let fixed = two_capitals(last)?;
    Some(Suggestion { erase: last.chars().count() + 1, text: format!("{fixed} "), to_hebrew: false, keep_layout: true, auto: false })
}

/// The words being typed: letters are added, Backspace removes, Space ends a word. Anything
/// else (click, arrows, Enter, shortcuts, another window, a pause) clears everything.
#[derive(Debug, Default)]
pub struct Buffer {
    current: String,
    words: Vec<String>,
    hebrew: Option<bool>,
    /// 1.8.2: something not tracked was typed in this word (Caps Lock or Shift on the Hebrew
    /// layout): this word is not judged, and the run starts again after the next space
    broken: bool,
}

impl Buffer {
    pub fn clear(&mut self) {
        self.current.clear();
        self.words.clear();
        self.hebrew = None;
        self.broken = false;
    }
    /// Text that is not tracked was typed: nothing is judged until the next space.
    pub fn invalidate(&mut self) {
        self.clear();
        self.broken = true;
    }
    /// A character typed with the Hebrew (`hebrew`) or the Latin layout.
    pub fn push(&mut self, c: char, hebrew: bool) {
        if self.broken {
            return;
        }
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
        if self.broken {
            return;
        }
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
        if self.broken {
            self.clear();
            return None;
        }
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

/// 1.8.1, option: two quick taps on Shift convert like F10 (handy on laptops where F10 needs Fn).
/// A tap is Shift pressed and released within `TAP_MS` with no other key; two taps must follow
/// within `GAP_MS`. Any other key, or Shift held longer (typing a capital), starts again.
pub const TAP_MS: u64 = 300;
pub const GAP_MS: u64 = 400;

#[derive(Debug, Default)]
pub struct DoubleShift {
    down_at: Option<u64>,
    last_tap: Option<u64>,
}

impl DoubleShift {
    /// One key event; true when the second quick tap ends.
    pub fn key(&mut self, is_shift: bool, down: bool, now: u64) -> bool {
        if !is_shift {
            if down {
                *self = DoubleShift::default();
            }
            return false;
        }
        if down {
            if self.down_at.is_none() {
                self.down_at = Some(now); // key repeat while held: ignored
            }
            return false;
        }
        let Some(d) = self.down_at.take() else { return false };
        if now.saturating_sub(d) > TAP_MS {
            self.last_tap = None; // held: a capital letter, not a tap
            return false;
        }
        match self.last_tap {
            Some(l) if now.saturating_sub(l) <= GAP_MS => {
                self.last_tap = None;
                true
            }
            _ => {
                self.last_tap = Some(now);
                false
            }
        }
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
        assert_eq!(s, Suggestion { erase: 5, text: "שלום ".into(), to_hebrew: true, keep_layout: false, auto: false });
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
    fn personal_dictionary_has_priority_and_undo_teaches() {
        let mut d = dict();
        assert!(suggest(&[w("akuo", "שלום")], false, &d).is_some());
        d.set_personal(&format!("# my words\nakuo\n{}\nבריסלב\n", never_line("XXX")));
        assert!(suggest(&[w("akuo", "שלום")], false, &d).is_none(), "a word of the user is never fixed");
        assert!(suggest(&[w("cr,xkc", "בריסלב")], false, &d).is_some(), "a personal word counts as a real word");
        d.set_personal(&never_line("Akuo"));
        assert!(suggest(&[w("akuo", "שלום")], false, &d).is_none(), "after an undo: never again (case ignored)");
        assert_eq!(parse_personal("-\n  \nא\n").0.len(), 1);
    }

    #[test]
    fn f10_teaches_and_code_is_left_alone() {
        let mut d = dict();
        // "cr,xkc" (בריסלב) is in no list: nothing, until the user fixes it once with F10
        assert!(suggest(&[w("cr,xkc", "בריסלב")], false, &d).is_none());
        d.set_personal(&learned_line("cr,xkc").unwrap_or_default());
        assert_eq!(learned_line("akuo").as_deref(), Some("+akuo"));
        assert_eq!(learned_line("a b"), None, "a single word only");
        // learned_line refuses signs: "cr,xkc" itself is not taught that way; the word taught is a letters word
        d.set_personal("+zzzz\n");
        let s = suggest(&[w("zzzz", "זזזז")], false, &d).unwrap();
        assert!(s.auto, "taught with F10: fixed at once");
        d.set_personal("+zzzz\n-zzzz\n");
        assert!(suggest(&[w("zzzz", "זזזז")], false, &d).is_none(), "never wins over learned");
        assert!(suggest(&[w("getElementById", "שלום")], false, &dict()).is_none(), "code-like word: never");
        assert!(!code_like("CR,XKC"), "Caps Lock: not code");
    }

    #[test]
    fn what_f10_teaches() {
        assert_eq!(f10_lesson("akuo", "שלום").as_deref(), Some("+akuo"));
        assert_eq!(f10_lesson("שלום", "akuo").as_deref(), Some("+שלום"));
        assert_eq!(f10_lesson("akuo kuo", "שלום לום"), None, "one word only");
        let d = dict();
        assert_eq!(f10_lesson_checked("akuo", "שלום", Some(&d)).as_deref(), Some("+akuo"));
        assert_eq!(f10_lesson_checked("שלום", "akuo", Some(&d)), None, "an F10 that goes back teaches nothing");
        assert_eq!(f10_lesson_checked("kuo", "לום", Some(&d)), None, "a part of a word teaches nothing");
        assert_eq!(f10_lesson_checked("ab", "שנ", Some(&d)), None, "too short");
        assert_eq!(f10_lesson("hELLO", "Hello"), None, "a Caps Lock fix is not a layout lesson");
        assert!(is_code_editor("Code.exe") && !is_code_editor("winword.exe"));
    }

    #[test]
    fn double_shift_only_for_two_quick_lonely_taps() {
        let mut d = DoubleShift::default();
        assert!(!d.key(true, true, 0) && !d.key(true, false, 80));
        assert!(!d.key(true, true, 200) && d.key(true, false, 260), "two quick taps");
        // Shift held to type a capital, then a tap: nothing
        assert!(!d.key(true, true, 1000) && !d.key(true, false, 1500) && !d.key(true, true, 1600) && !d.key(true, false, 1650));
        // a letter between the taps: nothing
        let mut d = DoubleShift::default();
        d.key(true, true, 0);
        d.key(true, false, 50);
        d.key(false, true, 100);
        assert!(!d.key(true, true, 150) && !d.key(true, false, 200));
        // taps too far apart
        let mut d = DoubleShift::default();
        d.key(true, true, 0);
        d.key(true, false, 50);
        assert!(!d.key(true, true, 600) && !d.key(true, false, 650));
        // key repeat while held does not count as a second tap
        let mut d = DoubleShift::default();
        d.key(true, true, 0);
        d.key(true, true, 30);
        assert!(!d.key(true, false, 60));
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
        // 1.8.2, short words that join a run: how many words of each list can do it
        for (name, list, hebrew_word) in [("hébreu", &he, true), ("anglais", &en, false)] {
            let soft: Vec<String> = list.lines().filter(|w| (2..=3).contains(&w.chars().count())).filter(|w| d.rank(w, hebrew_word).is_none_or(|k| k >= SHORT_OWN_RANK) && d.rank(&conv(w), !hebrew_word).is_some_and(|k| k < SHORT_OTHER_RANK)).map(String::from).collect();
            rows.push(format!("{name} : {} mots courts rares qui peuvent suivre un mot corrigé : {}", soft.len(), soft.iter().take(30).cloned().collect::<Vec<_>>().join(" ")));
        }
        // 1.8.2, words followed by a sign (, . ;) typed on the RIGHT keyboard: nothing to propose
        for (name, list, hebrew_word) in [("hébreu", &he, true), ("anglais", &en, false)] {
            let mut words: Vec<String> = list.lines().filter(|w| w.chars().count() >= 2).map(String::from).collect();
            if let Ok(dir) = std::env::var("KLF_WORDS_50K") {
                if let Some(t) = read(&format!("{dir}/{}", if hebrew_word { "he_50k.txt" } else { "en_50k.txt" })) {
                    words.extend(t.lines().skip(40_000).filter_map(|l| l.split_whitespace().next()).filter(|w| w.chars().count() >= 2 && w.chars().all(|c| c.is_alphabetic())).map(String::from));
                }
            }
            let (mut fp, mut n) = (0, 0);
            for w in words.iter().step_by(3) {
                for sign in [",", ".", ";"] {
                    let t = format!("{w}{sign}");
                    n += 1;
                    if suggest(&[(t.clone(), conv(&t))], hebrew_word, &d).is_some() {
                        fp += 1;
                        if std::env::var("KLF_SHOW").is_ok() {
                            println!("  mot + signe pris pour une erreur : {t} -> {}", conv(&t));
                        }
                    }
                }
            }
            rows.push(format!("{name} : {n} mots suivis de , . ; ; fausses alertes : {:.2} %", 100.0 * fp as f64 / n.max(1) as f64));
        }
        // typos (a letter dropped, two letters swapped) typed on the RIGHT keyboard: nothing to propose
        for (name, list, hebrew_word) in [("hébreu", &he, true), ("anglais", &en, false)] {
            let (mut fp, mut n) = (0, 0);
            for w in list.lines().filter(|w| w.chars().count() >= 5).step_by(4) {
                let c: Vec<char> = w.chars().collect();
                let k = c.len() / 2;
                let mut dropped = c.clone();
                dropped.remove(k);
                let mut swapped = c.clone();
                swapped.swap(k - 1, k);
                for t in [dropped, swapped] {
                    let t: String = t.into_iter().collect();
                    if t == w || (if hebrew_word { d.is_he(&t) } else { d.is_en(&t) }) {
                        continue;
                    }
                    n += 1;
                    if suggest(&[(t.clone(), conv(&t))], hebrew_word, &d).is_some() {
                        fp += 1;
                        if std::env::var("KLF_SHOW").is_ok() {
                            println!("  faute de frappe prise pour une erreur de clavier : {t} -> {}", conv(&t));
                        }
                    }
                }
            }
            let mut d0 = Dict::from_lists(&he, &en, DICT_SIZE);
            d0.ngram = (f64::INFINITY, 0.0);
            let mut fp0 = 0;
            for w in list.lines().filter(|w| w.chars().count() >= 5).step_by(4) {
                let c: Vec<char> = w.chars().collect();
                let k = c.len() / 2;
                let mut dropped = c.clone();
                dropped.remove(k);
                let mut swapped = c.clone();
                swapped.swap(k - 1, k);
                for t in [dropped, swapped] {
                    let t: String = t.into_iter().collect();
                    if t != w && !(if hebrew_word { d.is_he(&t) } else { d.is_en(&t) }) && suggest(&[(t.clone(), conv(&t))], hebrew_word, &d0).is_some() {
                        fp0 += 1;
                    }
                }
            }
            rows.push(format!("{name} : (sans modèle de lettres ni préfixes : {:.2} %)", 100.0 * fp0 as f64 / n.max(1) as f64));
            rows.push(format!("{name} : {n} fautes de frappe (lettre oubliée, lettres inversées) ; fausses alertes : {:.2} %", 100.0 * fp as f64 / n.max(1) as f64));
        }
        // rarer words, NOT in the lists (ranks 40 001 to 50 000 of the source files), if present:
        // 40 001-45 000 to choose the thresholds, 45 001-50 000 to measure (never seen before)
        if let Ok(dir) = std::env::var("KLF_WORDS_50K") {
            let mut d = d;
            let sweep = std::env::var("KLF_SWEEP").is_ok();
            for (name, file, hebrew_word) in [("hébreu rare", "he_50k.txt", true), ("anglais rare", "en_50k.txt", false)] {
                let Some(t) = read(&format!("{dir}/{file}")) else { continue };
                let all: Vec<String> = t.lines().skip(40_000).filter_map(|l| l.split_whitespace().next()).filter(|w| w.chars().count() >= 3 && w.chars().all(|c| c.is_alphabetic())).map(String::from).collect();
                let half = all.len() / 2;
                let rate = |d: &Dict, words: &[String]| {
                    let (mut fp, mut fix) = (0, 0);
                    for w in words {
                        let other = conv(w);
                        if suggest(&[(w.clone(), other.clone())], hebrew_word, d).is_some() {
                            fp += 1;
                            if std::env::var("KLF_SHOW").is_ok() {
                                println!("  fausse alerte : {w} -> {other}");
                            }
                        }
                        if suggest(&[(other.clone(), w.clone())], !hebrew_word, d).is_some_and(|s| s.text.trim() == w) {
                            fix += 1;
                        }
                    }
                    let n = words.len().max(1) as f64;
                    (100.0 * fix as f64 / n, 100.0 * fp as f64 / n)
                };
                if sweep {
                    for gap in [1.0, 1.5, 2.0, 2.5, 3.0] {
                        for min in [-2.2, -2.4, -2.6, -2.8, -3.0, -9.0] {
                            d.ngram = (gap, min);
                            let (f, p) = rate(&d, &all[..half]);
                            println!("{name} choix gap {gap} min {min} : corrigés {f:.1} % fausses {p:.2} %");
                        }
                    }
                    d.ngram = (NGRAM_GAP, NGRAM_MIN);
                }
                d.ngram = (f64::INFINITY, 0.0);
                let (f0, p0) = rate(&d, &all[half..]);
                d.ngram = (NGRAM_GAP, NGRAM_MIN);
                let (f, p) = rate(&d, &all[half..]);
                rows.push(format!("{name} : {} mots absents des listes (rangs 45 001-50 000) ; corrigés : {f:.1} % (sans modèle de lettres {f0:.1} %) ; fausses alertes : {p:.2} % (sans {p0:.2} %)", all.len() - half));
            }
        }
        for r in rows {
            println!("{r}");
        }
    }
}

/// tests/typing-cases.tsv: real cases reported by users, checked with the real word lists.
#[cfg(test)]
mod cases {
    use super::*;
    use crate::convert::{convert_with, us_map};
    use crate::settings::MixedTextMode;

    #[test]
    fn short_words_fixed_with_the_next_one() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../");
        let read = |p: &str| std::fs::read_to_string(format!("{root}{p}")).ok();
        let (Some(he), Some(en)) = (read("data/words/KeyboardLayoutFixer-words-he.txt"), read("data/words/KeyboardLayoutFixer-words-en.txt")) else { return };
        let d = Dict::from_lists(&he, &en, DICT_SIZE);
        let m = us_map();
        let pairs = |s: &str| s.split(' ').map(|w| (w.to_string(), convert_with(&m, w, MixedTextMode::ToggleAll, true))).collect::<Vec<_>>();
        // "kt" (לא) alone is never fixed; with a wrong word after it, the whole run is
        assert!(suggest(&pairs("kt"), false, &d).is_none());
        assert_eq!(suggest(&pairs("kt hsg,h"), false, &d).map(|s| s.text), Some("לא ידעתי ".to_string()));
        let s = suggest(&pairs("ak gk akuo"), false, &d).expect("run fixed");
        assert_eq!(s.text, "של על שלום ");
        // an English word before stays: "is akuo" fixes only akuo
        assert_eq!(suggest(&pairs("is akuo"), false, &d).map(|s| s.text), Some("שלום ".to_string()));
    }

    #[test]
    fn typing_cases() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../");
        let read = |p: &str| std::fs::read_to_string(format!("{root}{p}")).ok();
        let (Some(he), Some(en), Some(cases)) = (read("data/words/KeyboardLayoutFixer-words-he.txt"), read("data/words/KeyboardLayoutFixer-words-en.txt"), read("tests/typing-cases.tsv")) else { return };
        let d = Dict::from_lists(&he, &en, DICT_SIZE);
        let m = us_map();
        let mut bad = Vec::new();
        for l in cases.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
            let c: Vec<&str> = l.split('\t').collect();
            assert!(c.len() >= 3, "bad line: {l}");
            let (expect, hebrew, typed) = (c[0], c[1] == "he", c[2]);
            let read = convert_with(&m, typed, MixedTextMode::ToggleAll, true);
            let got = suggest_any(&[(typed.to_string(), read.clone())], hebrew, &d);
            let ok = match expect {
                "fix" => got.as_ref().is_some_and(|s| c.get(3).is_none_or(|w| s.text.trim() == w.trim())),
                "keep" => got.is_none(),
                other => panic!("expect must be fix or keep, not {other}"),
            };
            if !ok {
                bad.push(format!("{l}  ->  {:?}", got.map(|s| s.text)));
            }
        }
        assert!(bad.is_empty(), "cases not met:\n{}", bad.join("\n"));
    }
}
