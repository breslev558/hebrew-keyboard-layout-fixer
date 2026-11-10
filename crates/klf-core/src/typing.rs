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
    /// 1.8.5: technical words and brands (lower case -> as written: "github" -> "GitHub")
    tech: HashMap<String, String>,
}

/// 1.8.6: English of chat messages, without the apostrophe ("dont", "youre"): real words for
/// the program (never converted). Words of 1-2 letters ("im") are left to the usual rules.
pub const CHAT_EN: &[&str] = &[
    "dont", "cant", "wont", "isnt", "arent", "wasnt", "werent", "didnt", "doesnt", "hasnt", "havent", "hadnt", "couldnt", "wouldnt",
    "shouldnt", "youre", "theyre", "thats", "whats", "theres", "heres", "wheres", "ive", "youve", "weve", "theyve", "youll", "theyll",
    "itll", "hes", "shes", "lets", "aint", "gonna", "wanna", "gotta", "yeah", "nope", "lol", "omg", "btw", "thx", "pls", "plz",
];

/// 1.8.5: technical words and brands often typed with the wrong layout, missing from the word
/// lists or written with capitals inside: recognised, and written their usual way.
pub const TECH_WORDS: &[&str] = &[
    "GitHub", "ChatGPT", "WhatsApp", "YouTube", "Gmail", "Google", "Facebook", "Instagram", "TikTok", "Zoom", "Outlook", "Excel",
    "PowerPoint", "OneDrive", "Dropbox", "iPhone", "iPad", "Android", "Windows", "Linux", "Python", "JavaScript", "TypeScript", "Netflix",
    "Spotify", "LinkedIn", "Telegram", "Discord", "Slack", "PayPal", "Bitcoin", "Wikipedia", "Copilot", "Claude", "Gemini", "OpenAI",
    "Microsoft", "Samsung", "Chrome", "Firefox", "Waze", "Canva", "Notion", "Trello", "Jira", "Docker", "Kubernetes", "WordPress",
    "Shopify", "AliExpress", "eBay", "Amazon", "Bluetooth", "WiFi", "USB", "PDF", "HTML", "CSS", "SQL", "Gmail", "Hotmail", "Yahoo",
    "Skype", "Viber", "Signal", "Teams", "Photoshop", "AutoCAD", "Excel", "Zoom", "MacBook", "PlayStation", "Xbox", "Minecraft",
    "Fortnite", "Roblox", "Uber", "Airbnb", "Booking", "Waze", "Moovit", "Gett", "Wolt", "Bit", "Pango", "NetFree",
];

/// 1.8.2, a word in no list: proposed only when the reading looks much more like its language
/// than what was typed looks like its own (average log-probability per letter), 5 letters or more.
pub const NGRAM_GAP: f64 = 1.5;
pub const NGRAM_MIN: f64 = -3.0;
pub const NGRAM_MIN_CHARS: usize = 5;
/// ...and what was typed must look bad in its own language (typos of real words do not).
pub const NGRAM_OWN_MAX: f64 = -3.6;
/// 1.8.4, a Hebrew word in no list that looks like Hebrew this much keeps its place against an
/// English reading of this rank or more (measured on real sentences).
pub const HE_LOOKS_OK: f64 = -2.4;
pub const HE_LOOKS_OK_RANK: u32 = 300;
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
    // "?word date" (1.8.6): seen once, not yet learned: not used
    for l in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with('?')).take(10_000) {
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

/// 1.8.6, cautious learning: a word fixed with F10 is learned ("+word") only the SECOND time; the
/// first time it is noted as "?word YYYY-MM-DD", forgotten after 30 days if not confirmed.
/// Returns the new file text, or None when nothing changes. `today` = days since 1970.
pub fn learn_step(text: &str, typed: &str, today: u32) -> Option<String> {
    let line = learned_line(typed)?;
    let word = &line[1..];
    let lines: Vec<&str> = text.lines().collect();
    if lines.iter().any(|l| l.trim().eq_ignore_ascii_case(&line)) {
        return None; // already learned
    }
    let pending = |l: &str| l.trim().strip_prefix('?').and_then(|r| r.split_whitespace().next()).is_some_and(|w| w.eq_ignore_ascii_case(word));
    let mut out: Vec<String> = lines.iter().filter(|l| !pending(l) && !pending_expired(l, today)).map(|l| l.to_string()).collect();
    // the second time must be another day (three F10 in a row are not two lessons)
    let seen_before_today = lines.iter().any(|l| pending(l) && l.split_whitespace().nth(1).and_then(day_number).is_some_and(|d| d < today));
    if lines.iter().any(|l| pending(l)) && !seen_before_today {
        return None; // already noted today
    }
    if seen_before_today {
        out.push(line.clone()); // second time, another day: learned
    } else {
        out.push(format!("?{word} {}", day_text(today)));
    }
    Some(out.join("\r\n") + "\r\n")
}

/// A "?word date" line older than 30 days.
fn pending_expired(l: &str, today: u32) -> bool {
    let Some(rest) = l.trim().strip_prefix('?') else { return false };
    rest.split_whitespace().nth(1).and_then(day_number).is_some_and(|d| today.saturating_sub(d) > 30)
}

/// Days since 1970-01-01 <-> "YYYY-MM-DD" (proleptic Gregorian; no time zone needed).
pub fn day_text(days: u32) -> String {
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn day_number(t: &str) -> Option<u32> {
    let mut p = t.split('-');
    let (y, m, d): (i64, i64, i64) = (p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next()?.parse().ok()?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || p.next().is_some() {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    u32::try_from(era * 146_097 + doe - 719_468).ok()
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
        let mut en_map = take(en, true);
        for w in CHAT_EN {
            en_map.entry(w.to_string()).or_insert(9_000);
        }
        let mut tech = HashMap::new();
        for w in TECH_WORDS {
            let l = w.to_lowercase();
            // only words that are not ordinary English words (bit, signal, teams, windows, zoom...
            // keep their place and their small letters: "a bit", "two teams")
            // ...and not the short ones (USB, PDF, Bit...): too close to typos and short words
            if en_map.contains_key(&l) || l.chars().count() < 5 {
                continue;
            }
            en_map.insert(l.clone(), 2_000);
            if w.chars().any(|c| c.is_uppercase()) {
                tech.insert(l, w.to_string());
            }
        }
        Dict { he: take(he, false), en: en_map, personal: HashSet::new(), never: HashSet::new(), learned: HashSet::new(), he_model: model(he), en_model: model(en), ngram: (NGRAM_GAP, NGRAM_MIN), tech }
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
    /// 1.8.5: a technical word or brand as usually written ("github" -> "GitHub"), else as is.
    pub fn display(&self, w: &str) -> String {
        // only a whole lower-case word, without the signs around it
        let core = w.trim_matches(|c: char| !c.is_alphanumeric());
        match self.tech.get(core) {
            Some(d) if core == core.to_lowercase() => w.replacen(core, d, 1),
            _ => w.to_string(),
        }
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
    /// 1.8.4: the word looks like a word of that language (average log-probability per letter).
    pub fn looks_like(&self, w: &str, hebrew: bool, min: f64) -> bool {
        let m = if hebrew { &self.he_model } else { &self.en_model };
        !m.is_empty() && m.score(w) >= min
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

/// 1.8.4: programs where typing help never acts (F10 still works): password managers and crypto
/// wallets (their fields are not always seen as password fields), and remote desktop clients
/// (the keys go to another PC: a fix could land in the wrong place).
pub const SENSITIVE_APPS: &[&str] = &[
    // password managers
    "keepass.exe", "keepassxc.exe", "keeweb.exe", "bitwarden.exe", "1password.exe", "lastpass.exe", "dashlane.exe", "proton pass.exe",
    "protonpass.exe", "enpass.exe", "roboform.exe", "nordpass.exe", "keeper password manager.exe", "keeperpasswordmanager.exe", "pwsafe.exe",
    "stpass.exe", "authy desktop.exe", "passwordsafe.exe", "psono.exe",
    // crypto wallets
    "exodus.exe", "electrum.exe", "ledger live.exe", "trezor suite.exe", "atomic wallet.exe", "bitcoin-qt.exe", "monero-wallet-gui.exe",
    "sparrow.exe", "wassabee.exe", "guarda.exe", "coinomi.exe", "trust wallet.exe",
    // remote desktop
    "mstsc.exe", "msrdc.exe", "msrdcw.exe", "anydesk.exe", "rustdesk.exe", "teamviewer.exe", "parsecd.exe", "vncviewer.exe", "tvnviewer.exe",
    "mremoteng.exe", "royalts.exe", "remotedesktopmanager.exe", "remotedesktopmanager64.exe", "nxplayer.exe", "supremo.exe", "aeroadmin.exe",
    "strwinclt.exe", "rdclient.windows.exe", "windows365.exe", "wfica32.exe", "cdviewer.exe", "vmware-view.exe", "horizon_client.exe",
    "vmconnect.exe", "vmware.exe", "vmplayer.exe", "virtualboxvm.exe", "quickassist.exe", "workspaces.exe", "screenconnect.windowsclient.exe",
    "moonlight.exe",
];
// Not covered (shown in a browser tab): Chrome Remote Desktop, web vaults and password manager
// extensions; the password-field check still applies there.

pub fn is_sensitive_app(exe: &str) -> bool {
    // the file name only, whatever the case ("C:\\...\\KeePass.exe")
    let e = exe.rsplit(['\\', '/']).next().unwrap_or(exe).to_lowercase();
    SENSITIVE_APPS.iter().any(|x| *x == e) || (e.ends_with(".exe") && ["electrum", "keepass", "bitwarden", "1password"].iter().any(|p| e.starts_with(p)))
}

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

/// 1.8.6: a final form (ך ם ן ף ץ) followed by another Hebrew letter in the same word.
pub fn has_inner_final(w: &str) -> bool {
    let c: Vec<char> = w.chars().collect();
    c.windows(2).any(|p| matches!(p[0], 'ך' | 'ם' | 'ן' | 'ף' | 'ץ') && crate::layouts::is_hebrew_letter(p[1]))
}

fn typed_is_word_he(dict: &Dict, w: &str) -> bool {
    dict.is_he(w)
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

/// One word typed with the Hebrew (`hebrew_active`) or the Latin layout, and its reading on the
/// other layout: was it typed with the wrong layout? (1.8.5: shared by the proposals while typing
/// and by F10 "only the wrong words".)
pub fn word_wrong(dict: &Dict, typed: &str, read: &str, hebrew_active: bool) -> bool {
    // personal dictionary first: a word of the user, or a word never to fix, is never wrong;
    // a reading that is a word of the user counts as a real word
    if dict.never(typed) || dict.personal(typed) || (!hebrew_active && (code_like(typed) || all_caps(typed))) {
        return false;
    }
    // 1.8.2: a word followed by a sign ("hello," "שלום."): the word itself decides; the keys
    // , . ; / ' are Hebrew letters on the other layout ("ac," = שבת), so they stay in the word
    // (on the Hebrew layout "/" and "'" are the letters q and w, not signs)
    let core = if hebrew_active { typed.trim_end_matches([',', '.', ';']) } else { typed.trim_end_matches(WORD_SIGNS) };
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
    // 1.8.6: a final letter (ך ם ן ף ץ) inside a word does not exist in Hebrew: Latin text typed
    // on the Hebrew layout ("ןא" = "it", "ךםמע" = "long"), if the reading is Latin letters
    // ...but only when the reading is a known English word (a typo such as "הןא" for "הוא" also
    // has a final letter inside, and its reading is no word)
    if hebrew_active && has_inner_final(core) && !typed_is_word_he(dict, core) {
        let r = read.trim_end_matches(['.', ',', ';']);
        if letters_only(r, false) && dict.common_enough(r, false) && !dict.personal(core) {
            return true;
        }
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
        // 1.8.4 (real sentences): a rare Hebrew word that looks like Hebrew ("פרם", "נשאי")
        // whose reading is a less common English word ("pro", "bath") is left alone
        if hebrew_active && dict.rank(read, false).is_some_and(|r| r >= HE_LOOKS_OK_RANK) && dict.looks_like(core, true, HE_LOOKS_OK) {
            return false;
        }
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
}

/// 1.8.2: a short word that happens to be a rare word here ("ak", "gk") but a very common one on
/// the other layout (של, על): fixed together with the wrong word next to it.
pub fn short_word_follows(dict: &Dict, t: &str, r: &str, hebrew_active: bool) -> bool {
    let n = t.chars().count();
    (2..=3).contains(&n) && !dict.never(t) && !dict.personal(t) && !(!hebrew_active && (all_caps(t) || code_like(t))) && letters_only(t, hebrew_active) && dict.rank(t, hebrew_active).is_none_or(|k| k >= SHORT_OWN_RANK) && dict.rank(r, !hebrew_active).is_some_and(|k| k < SHORT_OTHER_RANK)
}

/// 1.8.5, F10 "only the wrong words": in a text where some words were typed with the right
/// layout and others with the wrong one, only the wrong ones are converted (each word judged on
/// its own script, then short words next to a wrong word follow it). None when no word looks
/// wrong: the caller then converts everything, as before (never stuck).
pub fn convert_wrong_words(text: &str, dict: &Dict, convert: impl Fn(&str) -> String) -> Option<String> {
    if dict.is_empty() {
        return None;
    }
    // words and the blanks between them, kept exactly
    let mut parts: Vec<(String, bool)> = Vec::new(); // (text, is_word)
    for c in text.chars() {
        let w = !c.is_whitespace();
        match parts.last_mut() {
            Some((t, iw)) if *iw == w => t.push(c),
            _ => parts.push((c.to_string(), w)),
        }
    }
    let words: Vec<usize> = (0..parts.len()).filter(|&i| parts[i].1).collect();
    let info: Vec<(String, bool)> = words.iter().map(|&i| {
        let t = &parts[i].0;
        let he = t.chars().filter(|c| crate::layouts::is_hebrew_letter(*c)).count();
        let la = t.chars().filter(|c| c.is_alphabetic() && !crate::layouts::is_hebrew_letter(*c)).count();
        (convert(t), he > la)
    }).collect();
    // each word is judged without the signs around it that are signs on BOTH layouts ("?", "!",
    // quotes, brackets, "/" = "."), keeping those that are letters on the other one ("," = ת)
    // (on the Hebrew layout "'" and "/" are the letters w and q)
    let core_of = |t: &str, he: bool| -> String {
        let sign = |c: char| !c.is_alphabetic() && !(he && matches!(c, '\'' | '/')) && !convert(&c.to_string()).chars().any(char::is_alphabetic);
        t.trim_matches(sign).to_string()
    };
    let letters = |t: &str| t.chars().filter(|c| c.is_alphabetic()).count();
    // typed on the Hebrew layout, a final ץ ת ף may be the "." "," ";" ending an English word
    // ("נוממטץ" = "bunny."): the word is judged with and without it
    let judge = |c: &str, he: bool| -> bool {
        if letters(c) >= 2 && word_wrong(dict, c, &convert(c), he) {
            return true;
        }
        if he {
            let r = convert(c);
            let rc = r.trim_end_matches([',', '.', ';']);
            let n = r.chars().count() - rc.chars().count();
            if n > 0 {
                let tc: String = c.chars().take(c.chars().count().saturating_sub(n)).collect();
                return letters(&tc) >= 2 && word_wrong(dict, &tc, rc, he);
            }
        }
        false
    };
    let mut flags: Vec<bool> = words.iter().zip(&info).map(|(&i, (_, he))| judge(&core_of(&parts[i].0, *he), *he)).collect();
    if !flags.iter().any(|f| *f) {
        return None;
    }
    // short words next to a wrong word of the same script follow it (a few passes, both sides)
    for _ in 0..3 {
        for k in 0..flags.len() {
            if flags[k] {
                continue;
            }
            let he = &info[k].1;
            let t = &core_of(&parts[words[k]].0, *he);
            let r = &convert(t);
            let near = (k > 0 && flags[k - 1] && info[k - 1].1 == *he) || (k + 1 < flags.len() && flags[k + 1] && info[k + 1].1 == *he);
            let one_letter = letters(t) == 1 && t.chars().count() <= 2 && dict.rank(r.trim_matches(|c: char| !c.is_alphabetic()), !*he).is_some_and(|x| x < SHORT_OTHER_RANK) && dict.rank(t, *he).is_none_or(|x| x >= SHORT_OWN_RANK);
            // between two wrong words, a short word whose reading is far more frequent ("שרק",
            // rank 1 844, between two wrong words reads "are", rank 30)
            let both = k > 0 && k + 1 < flags.len() && flags[k - 1] && flags[k + 1] && info[k - 1].1 == *he && info[k + 1].1 == *he;
            let sandwiched = both && letters(t) <= 3 && dict.rank(r, !*he).is_some_and(|x| dict.rank(t, *he).is_none_or(|o| x.saturating_mul(20) < o));
            if near && (short_word_follows(dict, t, r, *he) || one_letter || sandwiched) {
                flags[k] = true;
            }
        }
    }
    // a run of words of the same script where most words are wrong: the whole run was typed on
    // the wrong layout, its short ambiguous words too ("אם" = "to", "ש" = "a" in English text)
    let mut k = 0;
    while k < flags.len() {
        let mut e = k;
        while e + 1 < flags.len() && info[e + 1].1 == info[k].1 {
            e += 1;
        }
        let run: Vec<usize> = (k..=e).filter(|&j| letters(&parts[words[j]].0) >= 1).collect();
        let wrong = run.iter().filter(|&&j| flags[j]).count();
        if run.len() >= 3 && wrong * 10 >= run.len() * 6 {
            for &j in &run {
                let t = &parts[words[j]].0;
                // never a word the user protects, capitals or code-like words
                if !dict.never(t) && !dict.personal(t) && !(!info[j].1 && (all_caps(t) || code_like(t))) {
                    flags[j] = true;
                }
            }
        }
        k = e + 1;
    }
    let mut out = String::with_capacity(text.len());
    let mut k = 0;
    for (t, is_word) in &parts {
        if *is_word {
            if flags[k] {
                // a Hebrew-layout word read as Latin: brands written their usual way
                out.push_str(&if info[k].1 { dict.display(&info[k].0) } else { info[k].0.clone() });
            } else {
                out.push_str(t);
            }
            k += 1;
        } else {
            out.push_str(t);
        }
    }
    Some(out)
}

/// `words`: the last words typed, oldest first, each as (typed, reading on the other layout);
/// they were all typed with the same layout, `hebrew_active` telling which. A space was just
/// typed after the last one.
pub fn suggest(words: &[(String, String)], hebrew_active: bool, dict: &Dict) -> Option<Suggestion> {
    if dict.is_empty() || words.is_empty() {
        return None;
    }
    // while typing, an abbreviation with a geresh (ים' דק') is left alone when the word without
    // it is Hebrew (F10 "only the wrong words" still judges it with its neighbours)
    let wrong = |typed: &str, read: &str| !(hebrew_active && typed.ends_with('\'') && dict.is_he(typed.trim_end_matches('\''))) && word_wrong(dict, typed, read, hebrew_active);
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
    let soft = |(t, r): &(String, String)| short_word_follows(dict, t, r, hebrew_active);
    while start > 0 && (flags[start - 1] || soft(&words[start - 1])) {
        start -= 1;
    }
    let run = &words[start..];
    let erase = run.iter().map(|(t, _)| len(t)).sum::<usize>() + run.len(); // each word + its space
    let text = run.iter().map(|(_, r)| if hebrew_active { dict.display(r) } else { r.clone() }).collect::<Vec<_>>().join(" ") + " ";
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
    // 1.8.4: only when the fixed word is a word ("SHalom") or one of the user's, not "DJing"
    if !dict.is_empty() && !dict.is_en(&fixed) && !dict.personal(&fixed) {
        return None;
    }
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
    /// 1.8.5: the words of the sentence being typed, the current one included (for the chat
    /// guard on Enter); None when nothing is tracked.
    pub fn snapshot(&self) -> Option<(Vec<String>, bool)> {
        if self.broken {
            return None;
        }
        let mut w = self.words.clone();
        if !self.current.is_empty() {
            w.push(self.current.clone());
        }
        (!w.is_empty()).then_some((w, self.hebrew?))
    }
    /// Nothing typed yet in the current word.
    pub fn is_empty_word(&self) -> bool {
        self.current.is_empty()
    }
    /// The layout of the words being typed, if any.
    pub fn is_hebrew(&self) -> Option<bool> {
        self.hebrew
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

/// 1.8.5, option: a quick tap on LEFT Ctrl alone = English keyboard, on RIGHT Ctrl alone = Hebrew.
/// Any other key or click in between, or Ctrl held longer (a shortcut), cancels it.
#[derive(Debug, Default)]
pub struct CtrlTap {
    down: Option<(bool, u64)>,
}

impl CtrlTap {
    /// One key event: `ctrl` = Some(true) left Ctrl, Some(false) right Ctrl, None another key.
    /// Some(true) = switch to English, Some(false) = switch to Hebrew.
    pub fn key(&mut self, ctrl: Option<bool>, down: bool, now: u64) -> Option<bool> {
        match (ctrl, down) {
            (None, true) => {
                self.down = None; // a shortcut (Ctrl+C...), AltGr (Ctrl + right Alt)...
                None
            }
            (None, false) => None,
            (Some(left), true) => {
                if self.down.is_none_or(|(l, _)| l != left) {
                    self.down = Some((left, now)); // key repeat while held: ignored
                }
                None
            }
            (Some(left), false) => match self.down.take() {
                Some((l, at)) if l == left && now.saturating_sub(at) <= TAP_MS => Some(left),
                _ => None,
            },
        }
    }
    pub fn cancel(&mut self) {
        self.down = None;
    }
}

/// 1.8.5: chat programs where Enter sends the message (option: a first Enter is held back when
/// the whole sentence is in the wrong language).
pub const MESSENGERS: &[&str] = &["whatsapp.exe", "whatsapp.root.exe", "telegram.exe", "ms-teams.exe", "teams.exe", "signal.exe", "discord.exe", "slack.exe", "viber.exe", "element.exe"];

pub fn is_messenger(exe: &str) -> bool {
    MESSENGERS.iter().any(|m| m.eq_ignore_ascii_case(exe))
}

/// 1.8.5: the sentence about to be sent looks typed in the wrong language: at least 2 words
/// and most of them wrong, or a single long wrong word.
pub fn sentence_wrong(pairs: &[(String, String)], hebrew_active: bool, dict: &Dict) -> bool {
    let words: Vec<&(String, String)> = pairs.iter().filter(|(t, _)| t.chars().filter(|c| c.is_alphabetic()).count() >= 2).collect();
    let wrong = words.iter().filter(|(t, r)| word_wrong(dict, t, r, hebrew_active)).count();
    match words.len() {
        0 => false,
        1 => wrong == 1 && words[0].0.chars().count() >= 4,
        n => wrong * 2 > n,
    }
}

/// 1.8.5, option: programs that use one language only get that keyboard when they come to the
/// front. The list is the user's file, one program per line, "program.exe" (English) or
/// "program.exe = he" (Hebrew); these are written when the option is first turned on.
pub const ONE_LANGUAGE_DEFAULT: &str = "windowsterminal.exe\r\npowershell.exe\r\npwsh.exe\r\ncmd.exe\r\nconhost.exe\r\nputty.exe\r\nmintty.exe\r\n";

/// Parsed list: exe (lower case) -> true for Hebrew, false for English.
pub fn parse_one_language(text: &str) -> HashMap<String, bool> {
    let mut m = HashMap::new();
    for l in text.trim_start_matches('\u{feff}').lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).take(500) {
        let (exe, lang) = match l.split_once('=') {
            Some((e, g)) => (e.trim(), g.trim().to_ascii_lowercase()),
            None => (l, String::new()),
        };
        let exe = exe.to_ascii_lowercase();
        if exe.ends_with(".exe") && !exe.contains(['\\', '/']) && exe.len() <= 80 {
            m.insert(exe, matches!(lang.as_str(), "he" | "hebrew" | "hébreu" | "עברית"));
        }
    }
    m
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

/// 1.8.4, false proposals on REAL sentences typed correctly (not shipped: a measurement run by
/// hand). KLF_SENTENCES = a folder with he.txt and en.txt, one sentence per line (Tatoeba, CC BY
/// 2.0 FR, used for testing only). Each sentence is typed key by key as the program sees it:
/// letters (and the sign keys) make words, Space checks, anything else starts again; Hebrew
/// letters are typed on the Hebrew layout, Latin ones on the Latin layout.
#[cfg(test)]
mod sentences {
    use super::*;
    use crate::convert::{convert_with, us_map};
    use crate::settings::MixedTextMode;

    /// What the hook sees for a character: (char, Hebrew layout). Letters typable on a US or Hebrew
    /// keyboard only; the sign keys , . ; / ' anywhere in a word (as the hook keeps them); ’ = '.
    fn key_of(c: char, buf: &Buffer) -> Option<(char, bool)> {
        let c = if c == '’' { '\'' } else { c };
        if crate::layouts::is_hebrew_letter(c) {
            return Some((c, true));
        }
        if c.is_ascii_alphabetic() {
            return Some((c, false));
        }
        WORD_SIGNS.contains(&c).then(|| (c, buf.is_hebrew().unwrap_or(false)))
    }

    #[test]
    #[ignore]
    fn sentences_measure() {
        let Ok(dir) = std::env::var("KLF_SENTENCES") else { return };
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/words/");
        let read = |p: &str| std::fs::read_to_string(p).ok();
        let (Some(he), Some(en)) = (read(&format!("{root}KeyboardLayoutFixer-words-he.txt")), read(&format!("{root}KeyboardLayoutFixer-words-en.txt"))) else { return };
        let d = Dict::from_lists(&he, &en, DICT_SIZE);
        let m = us_map();
        let show = std::env::var("KLF_SHOW").is_ok();
        for (name, file) in [("hébreu", "he.txt"), ("anglais", "en.txt")] {
            let Some(text) = read(&format!("{dir}/{file}")) else { continue };
            let (mut n, mut bad, mut words, mut props) = (0usize, 0usize, 0usize, 0usize);
            let mut shown = 0;
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                n += 1;
                let mut buf = Buffer::default();
                let mut wrong_here = false;
                for c in line.chars().chain(std::iter::once(' ')) {
                    if c == ' ' {
                        if let Some((w, h)) = buf.space() {
                            words += 1;
                            let pairs: Vec<(String, String)> = w.iter().map(|x| (x.clone(), convert_with(&m, x, MixedTextMode::ToggleAll, true))).collect();
                            if let Some(sg) = suggest_any(&pairs, h, &d) {
                                props += 1;
                                wrong_here = true;
                                if show && shown < 60 {
                                    shown += 1;
                                    println!("  {name} : « {line} » -> {:?}", sg.text);
                                }
                            }
                        }
                    } else if let Some(k) = key_of(c, &buf) {
                        buf.push(k.0, k.1);
                    } else if c == '"' {
                        buf.invalidate(); // Shift on the Hebrew layout (צה"ל): the hook does the same
                    } else {
                        buf.clear();
                    }
                }
                if wrong_here {
                    bad += 1;
                }
            }
            // the same sentences typed on the WRONG layout: how many words get a proposal
            let (mut wn, mut wfixed) = (0usize, 0usize);
            for line in text.lines().filter(|l| !l.trim().is_empty()).step_by(4) {
                let typed = convert_with(&m, &line.replace('’', "'"), MixedTextMode::ToggleAll, true);
                let mut buf = Buffer::default();
                for c in typed.chars().chain(std::iter::once(' ')) {
                    if c == ' ' {
                        if let Some((w, h)) = buf.space() {
                            wn += 1;
                            let pairs: Vec<(String, String)> = w.iter().map(|x| (x.clone(), convert_with(&m, x, MixedTextMode::ToggleAll, true))).collect();
                            if let Some(sg) = suggest_any(&pairs, h, &d) {
                                wfixed += sg.text.split_whitespace().count().min(w.len());
                                buf.clear(); // accepted with Tab
                            }
                        }
                    } else if let Some(k) = key_of(c, &buf) {
                        buf.push(k.0, k.1);
                    } else {
                        buf.clear();
                    }
                }
            }
            println!("{name} : mêmes phrases tapées sur le MAUVAIS clavier (1 sur 4) : {wn} mots, corrigés {:.1} %", 100.0 * wfixed as f64 / wn.max(1) as f64);
            // F10 "only the wrong words": first half right, second half on the wrong layout
            let (mut mn, mut mok, mut mnone, mut mfull, mut mcase) = (0usize, 0usize, 0usize, 0usize, 0usize);
            for line in text.lines().filter(|l| l.split_whitespace().count() >= 4).step_by(4) {
                let words: Vec<&str> = line.split_whitespace().collect();
                let half = words.len() / 2;
                let typed = format!("{} {}", words[..half].join(" "), words[half..].iter().map(|w| convert_with(&m, w, MixedTextMode::ToggleAll, true)).collect::<Vec<_>>().join(" "));
                let want = words.join(" ");
                mn += 1;
                let got = convert_wrong_words(&typed, &d, |w| convert_with(&m, w, MixedTextMode::ToggleAll, true));
                if got.as_deref() == Some(want.as_str()) {
                    mok += 1;
                }
                // capitals cannot come back from Hebrew letters ("איק" = "the", not "The")
                if got.as_deref().map(str::to_lowercase) == Some(want.to_lowercase()) {
                    mcase += 1;
                } else if show && mn < 40 {
                    println!("  mélangée : {want:?}\n    tapé : {typed:?}\n    F10  : {got:?}");
                }
                // a sentence typed right: nothing to change
                match convert_wrong_words(&want, &d, |w| convert_with(&m, w, MixedTextMode::ToggleAll, true)) {
                    None => mnone += 1,
                    Some(_) => mfull += 1,
                }
            }
            println!("{name} : F10 « seulement les mots faux », {mn} phrases mélangées (1re moitié juste, 2e moitié sur le mauvais clavier) : entièrement justes {:.1} % ({:.1} % aux majuscules près) ; phrases justes touchées à tort {:.2} %", 100.0 * mok as f64 / mn.max(1) as f64, 100.0 * mcase as f64 / mn.max(1) as f64, 100.0 * mfull as f64 / (mnone + mfull).max(1) as f64);
            println!("{name} : {n} phrases bien tapées, {words} mots vérifiés ; phrases avec une fausse proposition : {bad} ({:.2} %) ; propositions par mot : {:.3} %", 100.0 * bad as f64 / n.max(1) as f64, 100.0 * props as f64 / words.max(1) as f64);
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
    fn sensitive_apps_left_alone() {
        assert!(is_sensitive_app("KeePassXC.exe") && is_sensitive_app("mstsc.exe") && is_sensitive_app("electrum-4.5.8.exe") && is_sensitive_app("AnyDesk.exe"));
        assert!(is_sensitive_app("C:\\Tools\\KeePass-2.57.exe") && is_sensitive_app("RdClient.Windows.exe") && is_sensitive_app("Windows365.exe"));
        assert!(!is_sensitive_app("winword.exe") && !is_sensitive_app("notepad.exe"));
        // "DJing": two capitals left alone when the fixed word is no word
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../");
        let read = |p: &str| std::fs::read_to_string(format!("{root}{p}")).ok();
        let (Some(he), Some(en)) = (read("data/words/KeyboardLayoutFixer-words-he.txt"), read("data/words/KeyboardLayoutFixer-words-en.txt")) else { return };
        let d = Dict::from_lists(&he, &en, DICT_SIZE);
        assert!(suggest_any(&[("DJing".into(), "גחןמע".into())], false, &d).is_none());
        assert!(suggest_any(&[("HEllo".into(), "יקךךם".into())], false, &d).is_some_and(|s| s.text == "Hello "));
    }

    #[test]
    fn tech_words_written_their_way() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../");
        let read = |p: &str| std::fs::read_to_string(format!("{root}{p}")).ok();
        let (Some(he), Some(en)) = (read("data/words/KeyboardLayoutFixer-words-he.txt"), read("data/words/KeyboardLayoutFixer-words-en.txt")) else { return };
        let d = Dict::from_lists(&he, &en, DICT_SIZE);
        let m = us_map();
        let conv = |s: &str| convert_with(&m, s, MixedTextMode::ToggleAll, true);
        for w in ["github", "chatgpt", "whatsapp"] {
            let typed = conv(w);
            let s = suggest(&[(typed.clone(), conv(&typed))], true, &d).unwrap_or_else(|| panic!("{w}"));
            assert_eq!(s.text.trim().to_lowercase(), w);
            assert!(s.text.chars().any(|c| c.is_uppercase()), "{}", s.text);
        }
        assert_eq!(d.display("github,"), "GitHub,");
        assert_eq!(d.display("hello"), "hello");
    }

    #[test]
    fn cautious_learning_and_dates() {
        assert_eq!(day_text(0), "1970-01-01");
        assert_eq!(day_number("2026-11-10"), Some(20_767));
        assert_eq!(day_text(20_767), "2026-11-10");
        let t1 = learn_step("", "akuo", 20_767).unwrap();
        assert_eq!(t1, "?akuo 2026-11-10\r\n");
        assert!(parse_personal_full(&t1).2.is_empty(), "seen once: not learned");
        assert_eq!(learn_step(&t1, "akuo", 20_767), None, "same day: not a second lesson");
        let t2 = learn_step(&t1, "akuo", 20_768).unwrap();
        assert_eq!(t2, "+akuo\r\n");
        assert_eq!(learn_step(&t2, "akuo", 20_769), None);
        // forgotten after 30 days without a second time
        let t3 = learn_step("?old 2026-09-01\r\nmine\r\n", "akuo", 20_767).unwrap();
        assert_eq!(t3, "mine\r\n?akuo 2026-11-10\r\n");
    }

    #[test]
    fn inner_final_letters_and_chat_english() {
        assert!(has_inner_final("ןא") && has_inner_final("ךםמע") && !has_inner_final("שלום") && !has_inner_final("ים"));
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../");
        let read = |p: &str| std::fs::read_to_string(format!("{root}{p}")).ok();
        let (Some(he), Some(en)) = (read("data/words/KeyboardLayoutFixer-words-he.txt"), read("data/words/KeyboardLayoutFixer-words-en.txt")) else { return };
        let d = Dict::from_lists(&he, &en, DICT_SIZE);
        let m = us_map();
        let conv = |s: &str| convert_with(&m, s, MixedTextMode::ToggleAll, true);
        // "dont" typed in English is a word now: never converted
        assert!(suggest(&[("dont".into(), conv("dont"))], false, &d).is_none());
        // a final letter inside: Latin typed on the Hebrew layout
        for w in ["long", "into", "lion"] {
            let typed = conv(w);
            if has_inner_final(&typed) {
                assert!(word_wrong(&d, &typed, w, true), "{w} -> {typed}");
            }
        }
    }

    /// 1.8.6: precision gate run on every change (CI): the word lists of the repository, a sample.
    #[test]
    fn precision_gate() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../");
        let read = |p: &str| std::fs::read_to_string(format!("{root}{p}")).ok();
        let (Some(he), Some(en)) = (read("data/words/KeyboardLayoutFixer-words-he.txt"), read("data/words/KeyboardLayoutFixer-words-en.txt")) else {
            assert!(std::env::var("CI").is_err(), "word lists missing: the precision gate must run in CI");
            return;
        };
        let d = Dict::from_lists(&he, &en, DICT_SIZE);
        let m = us_map();
        let conv = |s: &str| convert_with(&m, s, MixedTextMode::ToggleAll, true);
        for (name, list, hebrew_word, min_fix) in [("he", &he, true, 85.0), ("en", &en, false, 90.0)] {
            let words: Vec<&str> = list.lines().take(DICT_SIZE).filter_map(|l| l.split_whitespace().next()).filter(|w| w.chars().count() >= 3).step_by(7).collect();
            let (mut fp, mut fix) = (0, 0);
            for w in &words {
                let other = conv(w);
                if suggest(&[(w.to_string(), other.clone())], hebrew_word, &d).is_some() {
                    fp += 1;
                }
                if suggest(&[(other.clone(), w.to_string())], !hebrew_word, &d).is_some_and(|s| s.text.trim().eq_ignore_ascii_case(w)) {
                    fix += 1;
                }
            }
            let n = words.len() as f64;
            assert!(100.0 * fix as f64 / n >= min_fix, "{name}: fixed {:.1} % < {min_fix} %", 100.0 * fix as f64 / n);
            assert_eq!(fp, 0, "{name}: false proposals on words of the list");
        }
    }

    #[test]
    fn ctrl_taps_and_lists() {
        let mut c = CtrlTap::default();
        assert_eq!(c.key(Some(true), true, 0), None);
        assert_eq!(c.key(Some(true), false, 100), Some(true), "left Ctrl tap = English");
        assert_eq!(c.key(Some(false), true, 200), None);
        assert_eq!(c.key(Some(false), false, 250), Some(false), "right Ctrl tap = Hebrew");
        // Ctrl+C, held Ctrl, AltGr: nothing
        c.key(Some(true), true, 0);
        c.key(None, true, 50);
        assert_eq!(c.key(Some(true), false, 100), None);
        c.key(Some(true), true, 0);
        assert_eq!(c.key(Some(true), false, 900), None);
        let m = parse_one_language("# x\nWindowsTerminal.exe\nnotepad.exe = he\nC:\\bad.exe\nnot-an-exe\n");
        assert_eq!(m.get("windowsterminal.exe"), Some(&false));
        assert_eq!(m.get("notepad.exe"), Some(&true));
        assert_eq!(m.len(), 2);
        assert_eq!(parse_one_language(ONE_LANGUAGE_DEFAULT).len(), 7);
        assert!(is_messenger("WhatsApp.exe") && !is_messenger("notepad.exe"));
    }

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
    fn f10_only_wrong_words() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../");
        let read = |p: &str| std::fs::read_to_string(format!("{root}{p}")).ok();
        let (Some(he), Some(en)) = (read("data/words/KeyboardLayoutFixer-words-he.txt"), read("data/words/KeyboardLayoutFixer-words-en.txt")) else { return };
        let d = Dict::from_lists(&he, &en, DICT_SIZE);
        let m = us_map();
        let conv = |s: &str| convert_with(&m, s, MixedTextMode::ToggleAll, true);
        // started in Hebrew, went on with the Latin layout
        let typed = format!("שלום לכולם, {}", conv("מה שלומכם היום"));
        assert_eq!(convert_wrong_words(&typed, &d, conv).as_deref(), Some("שלום לכולם, מה שלומכם היום"));
        // started in English, went on with the Hebrew layout
        let typed = format!("hello my friends {}", conv("how are you today"));
        assert_eq!(convert_wrong_words(&typed, &d, conv).as_deref(), Some("hello my friends how are you today"));
        // blanks and line breaks kept exactly
        let typed = format!("שלום\r\n  {}", conv("מה שלומך"));
        assert_eq!(convert_wrong_words(&typed, &d, conv).as_deref(), Some("שלום\r\n  מה שלומך"));
        // nothing wrong: None (the caller converts everything, as before)
        assert_eq!(convert_wrong_words("שלום לכולם", &d, conv), None);
        assert_eq!(convert_wrong_words("hello world", &d, conv), None);
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
