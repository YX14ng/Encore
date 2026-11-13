//! The synced lyrics panel on the right.

use iced::font::Weight;
use iced::gradient::Linear;
use iced::widget::{Column, column, container, pin, space, stack, text};
use iced::{Degrees, Element, Fill, Font};

use crate::{Message, theme};

pub const WIDTH: f32 = 300.0;

/// The height of every lyric row. Fixed so the scroll position of any line is easy to compute.
const ROW_HEIGHT: f32 = 88.0;
/// How far from the top of the panel the current line sits.
const FOCUS_OFFSET: f32 = 140.0;
/// The height of the fade at the top and bottom of the panel.
const FADE_HEIGHT: f32 = 90.0;

/// One line of synced lyrics: when it starts and what it says.
pub struct LyricLine {
    pub start: u32,
    pub text: String,
}

/// Reads an LRC file (`[mm:ss.cc] text` per line) into lines sorted by start time.
/// Metadata like `[ar:Artist]`, empty lines and lines with a broken time are skipped.
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut lines: Vec<LyricLine> = text.lines().filter_map(parse_lrc_line).collect();
    lines.sort_by_key(|line| line.start);
    lines
}

fn parse_lrc_line(line: &str) -> Option<LyricLine> {
    let rest = line.strip_prefix('[')?;
    let (stamp, words) = rest.split_once(']')?;
    let (minutes, seconds) = stamp.split_once(':')?;
    let minutes = minutes.parse::<u32>().ok()?;
    let whole_seconds = seconds.split('.').next()?.parse::<u32>().ok()?;
    let words = words.trim();
    if words.is_empty() {
        return None;
    }

    Some(LyricLine {
        start: minutes * 60 + whole_seconds,
        text: words.to_string(),
    })
}

/// The index of the line being sung at `elapsed` seconds, or `None` before the first one.
pub fn current_line(lines: &[LyricLine], elapsed: u32) -> Option<usize> {
    lines.iter().rposition(|line| line.start <= elapsed)
}

/// Every line of the track, scrolled so the line at `scroll` (a line index, animated between
/// whole numbers) sits near the top, with the current line bright and fades at both edges.
pub fn view(lines: &[LyricLine], current: Option<usize>, scroll: f32) -> Element<'_, Message> {
    let content: Element<'_, Message> = if lines.is_empty() {
        container(text("No lyrics for this track").color(theme::TEXT_SECONDARY))
            .padding([48, 26])
            .into()
    } else {
        let rows = lines.iter().enumerate().map(|(index, line)| {
            container(lyric(line, Some(index) == current))
                .height(ROW_HEIGHT)
                .clip(true)
                .into()
        });
        let list = Column::with_children(rows).width(WIDTH).padding([0, 26]);

        pin(list).y(FOCUS_OFFSET - scroll * ROW_HEIGHT).into()
    };

    let fades = column![fade(Degrees(0.0)), space::vertical(), fade(Degrees(180.0))];

    container(stack![content, fades])
        .width(WIDTH)
        .height(Fill)
        .clip(true)
        .style(|_| theme::panel(theme::LYRICS))
        .into()
}

/// A band that goes from the panel color to transparent, pointing at `direction`.
fn fade(direction: Degrees) -> Element<'static, Message> {
    container(space())
        .width(Fill)
        .height(FADE_HEIGHT)
        .style(move |_| {
            let gradient = Linear::new(direction)
                .add_stop(0.0, theme::LYRICS.scale_alpha(0.0))
                .add_stop(1.0, theme::LYRICS);
            container::background(gradient)
        })
        .into()
}

fn lyric(line: &LyricLine, is_current: bool) -> Element<'_, Message> {
    let color = if is_current {
        theme::TEXT
    } else {
        theme::TEXT.scale_alpha(0.3)
    };
    let font = if is_current {
        Font {
            weight: Weight::Bold,
            ..theme::INTER
        }
    } else {
        Font {
            weight: Weight::Semibold,
            ..Font::MONOSPACE
        }
    };

    text(&line.text)
        .size(if is_current { 26 } else { 22 })
        .color(color)
        .font(font)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(start: u32) -> LyricLine {
        LyricLine {
            start,
            text: String::from("la"),
        }
    }

    #[test]
    fn parses_a_timed_line() {
        let lines = parse_lrc("[01:10.00] Las calles");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].start, 70);
        assert_eq!(lines[0].text, "Las calles");
    }

    #[test]
    fn skips_metadata_and_broken_times() {
        assert!(parse_lrc("[ar:Los Satélites]").is_empty());
        assert!(parse_lrc("[xx:10.00] hola").is_empty());
        assert!(parse_lrc("[01:10.00]   ").is_empty());
        assert!(parse_lrc("no brackets at all").is_empty());
    }

    #[test]
    fn sorts_lines_that_are_out_of_order() {
        let lines = parse_lrc("[01:16.00] b\n[01:10.00] a");
        assert_eq!(lines[0].text, "a");
        assert_eq!(lines[1].text, "b");
    }

    #[test]
    fn reads_the_sample_file_in_order() {
        let lines = parse_lrc(include_str!("../assets/mock/luz-de-neon.lrc"));
        assert_eq!(lines.len(), 14);
        assert!(lines.windows(2).all(|pair| pair[0].start <= pair[1].start));
    }

    #[test]
    fn finds_the_current_line() {
        let lines = [line(70), line(76), line(83)];
        assert_eq!(current_line(&lines, 69), None);
        assert_eq!(current_line(&lines, 70), Some(0));
        assert_eq!(current_line(&lines, 75), Some(0));
        assert_eq!(current_line(&lines, 76), Some(1));
        assert_eq!(current_line(&lines, 999), Some(2));
    }

    #[test]
    fn no_lines_means_no_current_line() {
        assert_eq!(current_line(&[], 50), None);
    }
}
