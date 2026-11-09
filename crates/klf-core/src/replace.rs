//! 1.8.2, the user's text replacements (OPTION, off by default, public version only): "@@" typed
//! then Space becomes an email address. Nothing is built in: the list is empty until the user
//! writes it (one "shortcut = text" per line in replacements.txt).

use std::collections::HashMap;

pub const MAX_KEY_CHARS: usize = 32;
pub const MAX_VALUE_CHARS: usize = 1_000;
pub const MAX_ENTRIES: usize = 500;

/// "shortcut = text" lines; comments start with "# " (so "#sig = ..." is a shortcut). The
/// separator is " = " when present (a shortcut may then contain "=", such as "=>"), else the
/// first "=". A shortcut has no space, 2 to 32 characters; the text is one line (no line
/// breaks: Enter would send a chat message). The first line for a shortcut wins.
pub fn parse(text: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    for l in text.lines().map(str::trim).filter(|l| !l.is_empty() && *l != "#" && !l.starts_with("# ")) {
        if m.len() >= MAX_ENTRIES {
            break;
        }
        let Some((k, v)) = l.split_once(" = ").or_else(|| l.split_once('=')) else { continue };
        let (k, v) = (k.trim(), v.trim());
        let kn = k.chars().count();
        if !(2..=MAX_KEY_CHARS).contains(&kn) || k.chars().any(|c| c.is_whitespace() || c.is_control()) {
            continue;
        }
        if v.is_empty() || v.chars().count() > MAX_VALUE_CHARS || v.chars().any(char::is_control) || v == k {
            continue;
        }
        m.entry(k.to_string()).or_insert_with(|| v.to_string());
    }
    m
}

/// Every prefix of every shortcut: the token is kept only while it can still become one (any
/// other text typed, a password for instance, is not kept at all).
pub fn prefixes(m: &HashMap<String, String>) -> std::collections::HashSet<String> {
    let mut p = std::collections::HashSet::new();
    for k in m.keys() {
        let mut s = String::new();
        for c in k.chars() {
            s.push(c);
            p.insert(s.clone());
        }
    }
    p
}

/// A text whose characters are each erased by one Backspace in every program: no character
/// outside the basic plane (emoji), no joiner, variation selector or combining mark (niqqud,
/// accents typed apart). Only such a replacement can be undone safely.
pub fn simple_text(v: &str) -> bool {
    v.chars().all(|c| {
        let u = c as u32;
        u <= 0xFFFF
            && !(0x0300..=0x036F).contains(&u)
            && !(0x0483..=0x0489).contains(&u)
            && !(0x0591..=0x05C7).contains(&u)
            && !(0x0610..=0x061A).contains(&u)
            && !(0x064B..=0x065F).contains(&u)
            && !(0x1AB0..=0x1AFF).contains(&u)
            && !(0x1DC0..=0x1DFF).contains(&u)
            && !(0x200B..=0x200F).contains(&u)
            && !(0x20D0..=0x20FF).contains(&u)
            && !(0xFE00..=0xFE0F).contains(&u)
            && !(0xFE20..=0xFE2F).contains(&u)
    })
}

/// Tracks what was typed since the last space, any visible character (letters, digits, signs).
#[derive(Debug, Default)]
pub struct Token {
    current: String,
}

impl Token {
    pub fn clear(&mut self) {
        self.current.clear();
    }
    /// Something happened in the middle of a word (a click, a shortcut, a pause, another window):
    /// what comes next is not a whole shortcut ("ho" + click + "tel" is not "tel"), until a space.
    /// Before anything was typed, nothing to invalidate: a shortcut may start right away.
    pub fn interrupt(&mut self) {
        if !self.current.is_empty() {
            self.invalidate();
        }
    }
    /// No shortcut possible until the next space.
    pub fn invalidate(&mut self) {
        self.current.clear();
        self.current.push('\u{0}');
    }
    pub fn push(&mut self, c: char) {
        if self.current.contains('\u{0}') {
            return;
        }
        if self.current.chars().count() >= MAX_KEY_CHARS {
            // longer than any shortcut: nothing to replace until the next space
            self.current.push('\u{0}');
            return;
        }
        self.current.push(c);
    }
    pub fn backspace(&mut self) {
        if self.current.contains('\u{0}') {
            return; // stays invalid until the next space ("hor" + Backspace + "tel" is not "tel")
        }
        if self.current.pop().is_none() {
            self.clear();
        }
    }
    pub fn as_str(&self) -> &str {
        &self.current
    }
    /// Space typed: the shortcut just finished, if any (and the token starts again).
    pub fn space(&mut self) -> Option<String> {
        let t = std::mem::take(&mut self.current);
        (!t.is_empty() && !t.contains('\u{0}')).then_some(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lines() {
        let m = parse("# comment\n#\n@@ = moi@exemple.com\n;sig=Sam\nbad line\na = too short\n@@ = second\nx y = space\n;e = \n;same = ;same\n");
        assert_eq!(m.get("@@").map(String::as_str), Some("moi@exemple.com"));
        assert_eq!(m.get(";sig").map(String::as_str), Some("Sam"));
        assert_eq!(m.len(), 2);
        assert!(parse("").is_empty());
        let many: String = (0..600).map(|i| format!("k{i} = v\n")).collect();
        assert_eq!(parse(&many).len(), MAX_ENTRIES);
        // a text with "=" inside is kept whole; "#sig" and "=>" are shortcuts
        assert_eq!(parse("@m = a=b").get("@m").map(String::as_str), Some("a=b"));
        assert_eq!(parse("#sig = Y").get("#sig").map(String::as_str), Some("Y"));
        assert_eq!(parse("=> = ⇒").get("=>").map(String::as_str), Some("⇒"));
        let p = prefixes(&parse("@@ = x\n;sig = y"));
        assert!(p.contains("@") && p.contains("@@") && p.contains(";s") && !p.contains("x"));
        assert!(simple_text("moi@exemple.com שלום"));
        assert!(!simple_text("👍") && !simple_text("שָׁלוֹם") && !simple_text("e\u{301}"));
    }

    #[test]
    fn token() {
        let mut t = Token::default();
        for c in "@@".chars() {
            t.push(c);
        }
        assert_eq!(t.space().as_deref(), Some("@@"));
        assert_eq!(t.space(), None);
        for c in "@x".chars() {
            t.push(c);
        }
        t.backspace();
        t.push('@');
        assert_eq!(t.space().as_deref(), Some("@@"));
        for _ in 0..40 {
            t.push('a');
        }
        assert_eq!(t.space(), None);
        t.push('a');
        t.push('b');
        assert_eq!(t.space().as_deref(), Some("ab"));
        // "ho" + click + "tel": not the shortcut "tel"
        for c in "ho".chars() {
            t.push(c);
        }
        t.interrupt();
        for c in "tel".chars() {
            t.push(c);
        }
        assert_eq!(t.space(), None);
        // invalid, then Backspace: still invalid
        t.push('h');
        t.invalidate();
        t.backspace();
        for c in "tel".chars() {
            t.push(c);
        }
        assert_eq!(t.space(), None);
        // a click before anything typed: the shortcut still works
        t.interrupt();
        for c in "tel".chars() {
            t.push(c);
        }
        assert_eq!(t.space().as_deref(), Some("tel"));
    }
}
