//! Where the bubble goes (1.5.2). Pure, tested here; drawn by klf-win/src/bubble.rs.

use crate::settings::{BUBBLE_BOTTOM, BUBBLE_CARET};

/// Position of a bubble of size `size` in the work area `wa` (left, top, right, bottom).
/// `caret`: screen point under the text caret, if known. `m`: margin from the edge; `gap`: space
/// under the caret; `near`: distance under which the caret counts as "under the bubble" (then the
/// centred bubble moves to the other edge, so it never hides the text just corrected).
pub fn place(pos: u32, caret: Option<(i32, i32)>, size: (i32, i32), wa: (i32, i32, i32, i32), m: i32, gap: i32, near: i32) -> (i32, i32) {
    let (w, h) = size;
    let (left, top_edge, right, bottom_edge) = wa;
    let centre_x = left + (right - left - w) / 2;
    let top = top_edge + m;
    let bottom = bottom_edge - h - m;
    let (x, mut y) = match (pos, caret) {
        // near the caret (as before 1.5.2): under it, above it when there is no room below
        (BUBBLE_CARET, Some((cx, cy))) => (cx, if cy + gap + h > bottom_edge { cy - h - 3 * gap } else { cy + gap }),
        (BUBBLE_BOTTOM, _) => (centre_x, bottom),
        // top, unknown value, or "near the caret" with the caret unknown
        _ => (centre_x, top),
    };
    if pos != BUBBLE_CARET || caret.is_none() {
        if let Some((_, cy)) = caret {
            if cy >= y - near && cy <= y + h + near {
                y = if y == top { bottom } else { top };
            }
        }
    }
    (x.clamp(left, (right - w).max(left)), y.clamp(top_edge, (bottom_edge - h).max(top_edge)))
}

/// Bubble text for a conversion: "before → after", each part on one line, at most `max`
/// characters (then "…"), drawn as a LEFT-TO-RIGHT line whatever the interface language.
/// 1.8.4: only the invisible marks LRM / RLM (U+200E / U+200F) keep each part in its own
/// direction: the isolates used before (U+2066–U+2069) were drawn as boxes "FSI", "LRI", "PDI"
/// by Windows on some PCs. A Hebrew part is framed by RLM, a Latin part by LRM, so that its
/// signs (".", ",", "?") stay at its own end and the arrow stays between the two parts.
pub fn preview_text(before: &str, after: &str, max: usize) -> String {
    fn part(t: &str, max: usize) -> String {
        let one: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut c: Vec<char> = one.chars().filter(|ch| !is_direction_mark(*ch)).collect();
        if c.len() > max {
            c.truncate(max.saturating_sub(1));
            c.push('…');
        }
        let s: String = c.into_iter().collect();
        let he = s.chars().filter(|c| crate::layouts::is_hebrew_letter(*c)).count();
        let la = s.chars().filter(|c| c.is_alphabetic() && !crate::layouts::is_hebrew_letter(*c)).count();
        let m = if he > la { '\u{200F}' } else { '\u{200E}' };
        format!("{m}{s}{m}")
    }
    format!("\u{200E}{} \u{2192} {}\u{200E}", part(before, max), part(after, max))
}

/// Controls and every direction character (marks, embeddings, overrides, isolates, Arabic
/// letter mark): taken out of a text before it is shown.
pub fn is_direction_mark(c: char) -> bool {
    c.is_control() || ('\u{2066}'..='\u{2069}').contains(&c) || ('\u{202A}'..='\u{202E}').contains(&c) || matches!(c, '\u{200E}' | '\u{200F}' | '\u{061C}')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::BUBBLE_TOP;
    const WA: (i32, i32, i32, i32) = (0, 0, 1920, 1040); // 1080p, taskbar 40 px at the bottom

    #[test]
    fn preview_is_short_one_line_and_has_no_isolates() {
        assert_eq!(preview_text("akuo", "שלום", 24), "\u{200E}\u{200E}akuo\u{200E} \u{2192} \u{200F}שלום\u{200F}\u{200E}");
        assert_eq!(preview_text("שלום.", "akuo/", 24), "\u{200E}\u{200F}שלום.\u{200F} \u{2192} \u{200E}akuo/\u{200E}\u{200E}");
        // mixed texts, signs and digits: never an isolate, embedding or override; marks balanced
        for (b, a) in [("akuo, nv vekl?", "שלום, מה הקול?"), ("12 akuo 34", "12 שלום 34"), ("(akuo)", "(שלום)"), ("ac,", "שבת"), ("שלום world", "akuo עםרךג")] {
            let p = preview_text(b, a, 24);
            assert!(!p.chars().any(|c| ('\u{2066}'..='\u{2069}').contains(&c) || ('\u{202A}'..='\u{202E}').contains(&c)), "{p:?}");
            assert_eq!(p.matches('\u{2192}').count(), 1);
            assert!(p.starts_with('\u{200E}') && p.ends_with('\u{200E}'));
        }
        assert!(!preview_text("a\u{200f}b", "c", 24).contains("a\u{200f}b"));
        let p = preview_text("a\nb\t  c", "x", 24);
        assert!(p.contains("a b c"), "{p}");
        let long = "x".repeat(50);
        assert_eq!(preview_text(&long, "y", 10).chars().filter(|c| *c == 'x').count(), 9);
        assert!(preview_text(&long, "y", 10).contains('…'));
        // a text cannot break the layout with its own direction characters
        let p = preview_text("a\u{202e}b\u{2069}\u{2066}", "c\u{2067}", 24);
        assert!(!p.chars().any(|c| ('\u{2066}'..='\u{2069}').contains(&c) || ('\u{202A}'..='\u{202E}').contains(&c)), "{p:?}");
    }

    #[test]
    fn centred_top_by_default_and_bottom_on_request() {
        assert_eq!(place(BUBBLE_TOP, None, (200, 40), WA, 12, 6, 24), (860, 12));
        assert_eq!(place(BUBBLE_BOTTOM, None, (200, 40), WA, 12, 6, 24), (860, 988));
        assert_eq!(place(99, None, (200, 40), WA, 12, 6, 24), (860, 12), "unknown value = top");
    }

    #[test]
    fn never_on_the_corrected_text() {
        // caret in an address bar at the top -> the bubble goes to the bottom
        assert_eq!(place(BUBBLE_TOP, Some((500, 40)), (200, 40), WA, 12, 6, 24).1, 988);
        // caret in a chat box at the bottom, "bottom" chosen -> top
        assert_eq!(place(BUBBLE_BOTTOM, Some((500, 1000)), (200, 40), WA, 12, 6, 24).1, 12);
        // caret in the middle: no move
        assert_eq!(place(BUBBLE_TOP, Some((500, 500)), (200, 40), WA, 12, 6, 24).1, 12);
    }

    #[test]
    fn near_the_caret_and_its_fallback() {
        assert_eq!(place(BUBBLE_CARET, Some((500, 500)), (200, 40), WA, 12, 6, 24), (500, 506));
        // no room below: above
        assert_eq!(place(BUBBLE_CARET, Some((500, 1020)), (200, 40), WA, 12, 6, 24), (500, 962));
        // right edge: kept on screen
        assert_eq!(place(BUBBLE_CARET, Some((1900, 500)), (200, 40), WA, 12, 6, 24).0, 1720);
        // caret unknown: centre top
        assert_eq!(place(BUBBLE_CARET, None, (200, 40), WA, 12, 6, 24), (860, 12));
    }

    #[test]
    fn second_screen_on_the_left_and_tiny_area() {
        assert_eq!(place(BUBBLE_TOP, None, (200, 40), (-1280, 0, 0, 984), 12, 6, 24), (-740, 12));
        assert_eq!(place(BUBBLE_TOP, None, (300, 40), (0, 0, 200, 30), 12, 6, 24), (0, 0));
    }
}
