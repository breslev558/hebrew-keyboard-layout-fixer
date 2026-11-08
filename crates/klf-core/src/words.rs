//! "Only the last word" (1.4.1 trial option, off by default): which part of the line copied with
//! Shift+Home is the last word. Pure code, tested on Linux.
//!
//! The last word is the last run of non-blank characters; blanks after it belong to the
//! selection but do not change. No cut on punctuation: with the English layout , . ; ' are
//! Hebrew letters (ת ץ ף ,) and w gives the geresh, so "akuo'" typed in English is "שלום,".

/// Longest piece handled (Shift+Left is sent once per character).
pub const MAX_CHARS: usize = 100;

/// Why the last word cannot be selected safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordRefusal {
    /// nothing but blanks before the caret
    NoWord,
    /// a combining mark (niqqud...), a character outside the BMP (emoji) or a tab: the caret
    /// moves by grapheme, the count of Shift+Left would be wrong
    Complex,
    TooLong,
}

/// The piece to select, counted back from the caret: `count` characters (word + blanks after
/// it), `word` is the part that will be converted (the blanks stay as they are).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastWord {
    pub piece: String,
    pub count: usize,
}

/// Word's automatic typographic quotes back to the keys typed: ' and ".
pub fn normalize_quotes(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201C}' | '\u{201D}' => '"',
            _ => c,
        })
        .collect()
}

fn is_blank(c: char) -> bool {
    c == ' ' || c == '\u{00A0}' || c == '\u{202F}'
}

/// Invisible format or control characters (LRM, RLM, ZWJ, ZWNJ...): refused.
fn is_format(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}') || c.is_control()
}

fn is_combining(c: char) -> bool {
    // Hebrew points and cantillation, and the general combining blocks
    ('\u{0591}'..='\u{05C7}').contains(&c)
        || ('\u{0300}'..='\u{036F}').contains(&c)
        || ('\u{1AB0}'..='\u{1AFF}').contains(&c)
        || ('\u{1DC0}'..='\u{1DFF}').contains(&c)
        || ('\u{20D0}'..='\u{20FF}').contains(&c)
        || ('\u{FE20}'..='\u{FE2F}').contains(&c)
        || c == '\u{200D}'
        || ('\u{FE00}'..='\u{FE0F}').contains(&c)
}

/// The last word of `line` (the text from the start of the line to the caret).
pub fn last_word(line: &str) -> Result<LastWord, WordRefusal> {
    let chars: Vec<char> = line.chars().collect();
    let mut end = chars.len();
    while end > 0 && is_blank(chars[end - 1]) {
        end -= 1;
    }
    if end == 0 {
        return Err(WordRefusal::NoWord);
    }
    let mut start = end;
    while start > 0 && !chars[start - 1].is_whitespace() && !is_blank(chars[start - 1]) {
        start -= 1;
    }
    if start == end {
        return Err(WordRefusal::Complex); // a tab or another blank right before the caret
    }
    let piece: String = chars[start..].iter().collect();
    if piece.chars().any(|c| is_combining(c) || (c as u32) > 0xFFFF || is_format(c)) {
        return Err(WordRefusal::Complex);
    }
    let count = chars.len() - start;
    if count > MAX_CHARS {
        return Err(WordRefusal::TooLong);
    }
    Ok(LastWord { piece, count })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lw(s: &str) -> (String, usize) {
        let w = last_word(s).unwrap();
        (w.piece, w.count)
    }

    #[test]
    fn last_word_of_a_line() {
        assert_eq!(lw("bonjour à tous akuo"), ("akuo".into(), 4));
        assert_eq!(lw("שלום לכם nv"), ("nv".into(), 2));
        assert_eq!(lw("akuo"), ("akuo".into(), 4));
    }

    #[test]
    fn blanks_after_the_word_are_selected_but_kept() {
        assert_eq!(lw("hello akuo  "), ("akuo  ".into(), 6));
    }

    #[test]
    fn punctuation_stays_in_the_word_hebrew_letters_on_english_keys() {
        // , . ; ' are Hebrew letters typed with the English layout
        assert_eq!(lw("text akuo'"), ("akuo'".into(), 5));
        assert_eq!(lw("text ,ehu"), (",ehu".into(), 4));
        assert_eq!(lw("hi ג'ינס"), ("ג'ינס".into(), 5));
        assert_eq!(lw("ארה״ב"), ("ארה״ב".into(), 5));
    }

    #[test]
    fn risky_pieces_are_refused() {
        assert_eq!(last_word("   "), Err(WordRefusal::NoWord));
        assert_eq!(last_word(""), Err(WordRefusal::NoWord));
        assert_eq!(last_word("x שָׁלוֹם"), Err(WordRefusal::Complex), "niqqud: the caret moves by grapheme");
        assert_eq!(last_word("x akuo😀"), Err(WordRefusal::Complex), "emoji outside the BMP");
        assert_eq!(last_word(&"a".repeat(101)), Err(WordRefusal::TooLong));
        assert!(last_word(&"a".repeat(100)).is_ok());
    }

    #[test]
    fn a_tab_before_the_word_is_a_separator() {
        assert_eq!(lw("x\takuo"), ("akuo".into(), 4));
    }

    #[test]
    fn word_quotes_nbsp_and_invisible_marks() {
        assert_eq!(normalize_quotes("akuo\u{2019}"), "akuo'");
        assert_eq!(normalize_quotes("\u{201C}x\u{201D}"), "\"x\"");
        assert_eq!(lw("hello\u{00A0}akuo"), ("akuo".into(), 4), "no-break space is a blank");
        assert_eq!(last_word("x a\u{200F}kuo"), Err(WordRefusal::Complex), "RLM refused");
    }
}
