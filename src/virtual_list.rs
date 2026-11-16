//! Long lists that only build the rows in sight. iced lays out and draws every widget of a view
//! each time it is rebuilt, so a playlist of a thousand songs would cost tens of milliseconds
//! a frame; instead, the rows above and below the visible part become empty space of the same
//! height. Rows must all be `row_height` tall (spacing included).

use std::ops::Range;

use iced::widget::{Column, Space};
use iced::{Element, Fill};

/// Rows built beyond the visible ones on each side, so a quick scroll does not show gaps and a
/// header of a slightly different height than guessed is covered.
const OVERSCAN: usize = 15;

/// Where a list sits in its scrollable and how much of it shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scroll {
    /// How far the scrollable is scrolled.
    pub offset: f32,
    /// How tall the visible part is.
    pub height: f32,
}

impl Default for Scroll {
    fn default() -> Self {
        Self {
            offset: 0.0,
            height: 1000.0,
        }
    }
}

/// The rows of a list of `count` rows, `row_height` tall and starting `top` pixels into the
/// scrollable, that are in sight (plus the overscan).
pub fn visible(scroll: Scroll, top: f32, row_height: f32, count: usize) -> Range<usize> {
    let first = ((scroll.offset - top) / row_height).floor().max(0.0) as usize;
    let shown = (scroll.height / row_height).ceil() as usize + 1;
    let start = first.saturating_sub(OVERSCAN).min(count);
    let end = (first + shown + OVERSCAN).min(count);
    start..end
}

/// The list: space for the rows before `rows`, the rows themselves, space for the rows after.
pub fn view<'a, Message: 'a>(
    count: usize,
    rows: Range<usize>,
    row_height: f32,
    row: impl Fn(usize) -> Element<'a, Message>,
) -> Element<'a, Message> {
    let before = rows.start as f32 * row_height;
    let after = count.saturating_sub(rows.end) as f32 * row_height;
    Column::new()
        .push((before > 0.0).then(|| Space::new().width(Fill).height(before)))
        .extend(rows.map(row))
        .push((after > 0.0).then(|| Space::new().width(Fill).height(after)))
        .width(Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_rows_in_sight_and_a_margin() {
        let at = |offset| Scroll {
            offset,
            height: 580.0,
        };
        // At the top: the first rows only.
        assert_eq!(visible(at(0.0), 200.0, 58.0, 1000), 0..26);
        // Scrolled to row 100 or so.
        let rows = visible(at(200.0 + 100.0 * 58.0), 200.0, 58.0, 1000);
        assert_eq!(rows, 85..126);
        // Near the end, and lists shorter than the screen.
        assert_eq!(visible(at(1e6), 200.0, 58.0, 1000), 1000..1000);
        assert_eq!(visible(at(0.0), 200.0, 58.0, 4), 0..4);
    }
}
