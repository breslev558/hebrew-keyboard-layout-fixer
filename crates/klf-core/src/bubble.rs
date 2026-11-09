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
/// characters (then "…"), each wrapped in Unicode isolates so that Hebrew and Latin text keep
/// their own direction whatever the interface language (1.6.0).
pub fn preview_text(before: &str, after: &str, max: usize) -> String {
    fn part(t: &str, max: usize) -> String {
        let one: String = t.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut c: Vec<char> = one.chars().filter(|ch| !ch.is_control() && !('\u{2066}'..='\u{2069}').contains(ch) && !('\u{202a}'..='\u{202e}').contains(ch) && !matches!(ch, '\u{200e}' | '\u{200f}' | '\u{061c}')).collect();
        if c.len() > max {
            c.truncate(max.saturating_sub(1));
            c.push('…');
        }
        c.into_iter().collect()
    }
    // the whole line left to right (before, then after), whatever the interface direction
    format!("\u{2066}\u{2068}{}\u{2069} → \u{2068}{}\u{2069}\u{2069}", part(before, max), part(after, max))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::BUBBLE_TOP;
    const WA: (i32, i32, i32, i32) = (0, 0, 1920, 1040); // 1080p, taskbar 40 px at the bottom

    #[test]
    fn preview_is_short_one_line_and_isolated() {
        assert_eq!(preview_text("akuo", "שלום", 24), "\u{2066}\u{2068}akuo\u{2069} → \u{2068}שלום\u{2069}\u{2069}");
        assert!(!preview_text("a\u{200f}b", "c", 24).contains('\u{200f}'));
        let p = preview_text("a\nb\t  c", "x", 24);
        assert!(p.contains("a b c"), "{p}");
        let long = "x".repeat(50);
        assert_eq!(preview_text(&long, "y", 10).chars().filter(|c| *c == 'x').count(), 9);
        assert!(preview_text(&long, "y", 10).contains('…'));
        // a text cannot break the layout with its own direction marks
        assert!(!preview_text("a\u{202e}b\u{2069}", "c", 24).contains('\u{202e}'));
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
