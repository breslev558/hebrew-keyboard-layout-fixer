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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::BUBBLE_TOP;
    const WA: (i32, i32, i32, i32) = (0, 0, 1920, 1040); // 1080p, taskbar 40 px at the bottom

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
