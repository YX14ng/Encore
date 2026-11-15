//! Lyrics: parsing LRC files and drawing the panel on the right (and its larger version in the
//! immersive view). Fetching them lives in `lyrics_source.rs`.

use iced::font::Weight;
use iced::gradient::Linear;
use iced::widget::{Column, column, container, pin, scrollable, space, stack, text};
use iced::{Color, Degrees, Element, Fill, Font};
use rust_i18n::t;
use serde::{Deserialize, Serialize};

use crate::{Message, theme};

pub const WIDTH: f32 = 300.0;

/// How far from the top of the panel the current line sits, as a share of its height.
const FOCUS: f32 = 0.18;
/// The height of the fade at the top and bottom of the panel.
const FADE_HEIGHT: f32 = 90.0;

/// One line of synced lyrics: when it starts (in milliseconds) and what it says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LyricLine {
    pub start_ms: u32,
    pub text: String,
}

/// The lyrics of the song that is playing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lyrics {
    /// Still looking for them.
    Loading,
    /// Lines with times, highlighted as the song plays.
    Synced(Vec<LyricLine>),
    /// Lines without times, shown as plain text.
    Plain(Vec<String>),
    /// The song has no words.
    Instrumental,
    /// Nothing was found.
    Missing,
}

impl Lyrics {
    /// The synced lines, if these lyrics have times.
    pub fn synced(&self) -> &[LyricLine] {
        match self {
            Lyrics::Synced(lines) => lines,
            _ => &[],
        }
    }
}

/// How big the lyrics are drawn.
#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub width: f32,
    pub height_hint: f32,
    pub current_size: f32,
    pub other_size: f32,
    pub row_height: f32,
    /// Whether the panel fades into its background at the top and bottom.
    pub fade: bool,
}

/// The panel at the right of the window.
pub const PANEL: Style = Style {
    width: WIDTH,
    height_hint: 760.0,
    current_size: 26.0,
    other_size: 22.0,
    row_height: 88.0,
    fade: true,
};

/// The lyrics tab of the now playing page.
pub const PAGE: Style = Style {
    width: 420.0,
    height_hint: 760.0,
    current_size: 26.0,
    other_size: 22.0,
    row_height: 88.0,
    fade: false,
};

/// The large lyrics of the immersive view.
pub const IMMERSIVE: Style = Style {
    width: 620.0,
    height_hint: 900.0,
    current_size: 40.0,
    other_size: 32.0,
    row_height: 118.0,
    fade: false,
};

/// Reads an LRC file (`[mm:ss.cc] text` per line) into lines sorted by start time.
/// Metadata like `[ar:Artist]`, empty lines and lines with a broken time are skipped.
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let mut lines: Vec<LyricLine> = text.lines().filter_map(parse_lrc_line).collect();
    lines.sort_by_key(|line| line.start_ms);
    lines
}

fn parse_lrc_line(line: &str) -> Option<LyricLine> {
    let rest = line.strip_prefix('[')?;
    let (stamp, words) = rest.split_once(']')?;
    let (minutes, seconds) = stamp.split_once(':')?;
    let minutes = minutes.parse::<u32>().ok()?;
    let (whole, fraction) = seconds.split_once('.').unwrap_or((seconds, "0"));
    let whole = whole.parse::<u32>().ok()?;
    let millis = fraction_to_millis(fraction)?;
    let words = words.trim();
    if words.is_empty() {
        return None;
    }

    Some(LyricLine {
        start_ms: (minutes * 60 + whole) * 1000 + millis,
        text: words.to_string(),
    })
}

/// "64" (hundredths) → 640 ms, "5" (tenths) → 500 ms, "123" → 123 ms.
fn fraction_to_millis(fraction: &str) -> Option<u32> {
    if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let digits: String = fraction.chars().chain("000".chars()).take(3).collect();
    digits.parse().ok()
}

/// The index of the line being sung at `elapsed_ms`, or `None` before the first one.
pub fn current_line(lines: &[LyricLine], elapsed_ms: u32) -> Option<usize> {
    lines.iter().rposition(|line| line.start_ms <= elapsed_ms)
}

/// The lyrics in `style`: synced ones scrolled so the line at `scroll` (a line index, animated
/// between whole numbers) sits near the top, plain ones as a scrollable text, or a note.
pub fn view(
    lyrics: &Lyrics,
    current: Option<usize>,
    scroll: f32,
    style: Style,
) -> Element<'_, Message> {
    let content: Element<'_, Message> = match lyrics {
        Lyrics::Synced(lines) if !lines.is_empty() => {
            let rows = lines.iter().enumerate().map(|(index, line)| {
                container(lyric(&line.text, Some(index) == current, style))
                    .height(style.row_height)
                    .clip(true)
                    .into()
            });
            let list = Column::with_children(rows)
                .width(style.width)
                .padding([0, 26]);
            let offset = style.height_hint * FOCUS - scroll * style.row_height;
            pin(list).y(offset).into()
        }
        Lyrics::Plain(lines) => {
            let rows = lines.iter().map(|line| {
                text(line)
                    .size(style.other_size * 0.8)
                    .color(theme::text_primary().scale_alpha(0.85))
                    .into()
            });
            scrollable(
                Column::with_children(rows)
                    .spacing(10)
                    .padding([48, 26])
                    .width(style.width),
            )
            .height(Fill)
            .into()
        }
        Lyrics::Loading => note(t!("lyrics.looking")),
        Lyrics::Instrumental => note(t!("lyrics.instrumental")),
        Lyrics::Missing | Lyrics::Synced(_) => note(t!("lyrics.missing")),
    };

    let fade_into = style.fade.then(theme::lyrics_bg);
    let layered: Element<'_, Message> = match fade_into {
        Some(color) => stack![
            content,
            column![
                fade(Degrees(0.0), color),
                space::vertical(),
                fade(Degrees(180.0), color)
            ]
        ]
        .into(),
        None => content,
    };

    let panel = container(layered)
        .width(style.width)
        .height(Fill)
        .clip(true);
    match fade_into {
        Some(color) => panel.style(move |_| theme::panel(color)).into(),
        None => panel.into(),
    }
}

fn note<'a>(message: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    container(text(message).color(theme::text_secondary()))
        .padding([48, 26])
        .into()
}

/// A band that goes from `color` to transparent, pointing at `direction`.
fn fade(direction: Degrees, color: Color) -> Element<'static, Message> {
    container(space())
        .width(Fill)
        .height(FADE_HEIGHT)
        .style(move |_| {
            let gradient = Linear::new(direction)
                .add_stop(0.0, color.scale_alpha(0.0))
                .add_stop(1.0, color);
            container::background(gradient)
        })
        .into()
}

fn lyric(line: &str, is_current: bool, style: Style) -> Element<'_, Message> {
    let color = if is_current {
        theme::text_primary()
    } else {
        theme::text_primary().scale_alpha(0.3)
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

    text(line)
        .size(if is_current {
            style.current_size
        } else {
            style.other_size
        })
        .color(color)
        .font(font)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(start_ms: u32) -> LyricLine {
        LyricLine {
            start_ms,
            text: String::from("la"),
        }
    }

    #[test]
    fn parses_a_timed_line_to_the_millisecond() {
        let lines = parse_lrc("[00:30.64] Like the legend of the phoenix");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].start_ms, 30_640);
        assert_eq!(lines[0].text, "Like the legend of the phoenix");
    }

    #[test]
    fn reads_tenths_hundredths_and_thousandths() {
        assert_eq!(parse_lrc("[01:10.5] a")[0].start_ms, 70_500);
        assert_eq!(parse_lrc("[01:10.05] a")[0].start_ms, 70_050);
        assert_eq!(parse_lrc("[01:10.123] a")[0].start_ms, 70_123);
        assert_eq!(parse_lrc("[01:10] a")[0].start_ms, 70_000);
    }

    #[test]
    fn skips_metadata_and_broken_times() {
        assert!(parse_lrc("[ar:Los Satélites]").is_empty());
        assert!(parse_lrc("[xx:10.00] hola").is_empty());
        assert!(parse_lrc("[01:10.x0] hola").is_empty());
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
        assert!(
            lines
                .windows(2)
                .all(|pair| pair[0].start_ms <= pair[1].start_ms)
        );
    }

    #[test]
    fn finds_the_current_line() {
        let lines = [line(70_000), line(76_000), line(83_000)];
        assert_eq!(current_line(&lines, 69_999), None);
        assert_eq!(current_line(&lines, 70_000), Some(0));
        assert_eq!(current_line(&lines, 75_999), Some(0));
        assert_eq!(current_line(&lines, 76_000), Some(1));
        assert_eq!(current_line(&lines, 999_000), Some(2));
    }

    #[test]
    fn no_lines_means_no_current_line() {
        assert_eq!(current_line(&[], 50_000), None);
    }
}
