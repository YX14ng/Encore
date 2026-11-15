//! The player bar at the bottom of the center column.

use iced::widget::{button, column, container, image, row, slider, space, text};
use iced::{Center, Color, Element, Fill, border};

use crate::clock::{self, Playback};
use crate::queue::Repeat;
use crate::rating::Rating;
use crate::song::Song;
use crate::{Message, theme};

/// Below this width the player bar puts the song above the buttons, so neither is squeezed.
pub const COMPACT_WIDTH: f32 = 1000.0;

/// The state of the player's buttons and sliders.
#[derive(Debug, Clone, Copy)]
pub struct Controls {
    /// How the playing song is rated; the buttons only work with `can_rate`.
    pub rating: crate::rating::Rating,
    pub can_rate: bool,
    /// Whether the music video shows, and whether a YouTube song plays so it can.
    pub video: bool,
    pub can_video: bool,
    /// The bar is narrow: the song goes on a row of its own above the buttons.
    pub compact: bool,
    /// Where the progress bar is being dragged to, in seconds.
    pub seeking: Option<f32>,
    /// From 0 to 100.
    pub volume: f32,
    pub muted: bool,
    pub shuffle: bool,
    pub repeat: Repeat,
}

/// The progress bar, the transport buttons and the card of the track that is playing.
/// `format` is the playing file's format ("OPUS 152"), shown next to the song when known.
pub fn view<'a>(
    song: &'a Song,
    playback: &'a Playback,
    controls: Controls,
    format: Option<&'a str>,
) -> Element<'a, Message> {
    let divider = container(space())
        .width(Fill)
        .height(1)
        .style(|_| container::background(theme::line()));

    let content = column![
        progress(playback, controls.seeking),
        buttons(song, playback, controls, format)
    ]
    .spacing(10)
    .padding([10, 20]);

    container(column![divider, content])
        .width(Fill)
        .style(|_| container::background(theme::player_bg()))
        .into()
}

/// The progress bar: drag or click it to jump through the song.
pub(crate) fn progress(playback: &Playback, seeking: Option<f32>) -> Element<'static, Message> {
    let length = playback.duration.max(1) as f32;
    let position = seeking
        .unwrap_or(playback.elapsed as f32)
        .clamp(0.0, length);
    let elapsed = position as u32;
    let left = clock::remaining(elapsed, playback.duration);

    let bar = slider(0.0..=length, position, Message::Seeking)
        .on_release(Message::SeekDone)
        .height(14)
        .style(|_, status| theme::thin_slider(status, theme::text_primary()));

    row![
        time_label(clock::format_time(elapsed)),
        bar,
        time_label(format!("-{}", clock::format_time(left))),
    ]
    .spacing(12)
    .align_y(Center)
    .into()
}

fn time_label(label: String) -> Element<'static, Message> {
    text(label).size(11).color(theme::text_secondary()).into()
}

/// Shuffle, previous, play or pause, next, repeat, like and dislike.
pub(crate) fn transport(playback: &Playback, controls: Controls) -> Element<'static, Message> {
    let play_icon: &'static [u8] = if playback.is_playing {
        include_bytes!("../assets/icons/pause.svg")
    } else {
        include_bytes!("../assets/icons/play.svg")
    };
    let rating_color = |rating| {
        if controls.rating == rating {
            theme::accent()
        } else {
            theme::text_secondary()
        }
    };
    let rate = |rating| {
        controls
            .can_rate
            .then_some(Message::Rating(crate::rating::Event::Pressed(rating)))
    };

    let repeat_icon: &'static [u8] = if controls.repeat == Repeat::One {
        include_bytes!("../assets/icons/repeat-one.svg")
    } else {
        include_bytes!("../assets/icons/repeat.svg")
    };
    row![
        icon_button(
            include_bytes!("../assets/icons/shuffle.svg"),
            toggle_color(controls.shuffle),
            18.0,
            Some(Message::ToggleShuffle)
        ),
        icon_button(
            include_bytes!("../assets/icons/previous.svg"),
            theme::text_primary(),
            18.0,
            Some(Message::Previous)
        ),
        icon_button(
            play_icon,
            theme::text_primary(),
            26.0,
            Some(Message::TogglePlay)
        ),
        icon_button(
            include_bytes!("../assets/icons/next.svg"),
            theme::text_primary(),
            18.0,
            Some(Message::Next)
        ),
        icon_button(
            repeat_icon,
            toggle_color(controls.repeat != Repeat::Off),
            18.0,
            Some(Message::CycleRepeat)
        ),
        icon_button(
            include_bytes!("../assets/icons/dislike.svg"),
            rating_color(Rating::Dislike),
            18.0,
            rate(Rating::Dislike)
        ),
        icon_button(
            include_bytes!("../assets/icons/like.svg"),
            rating_color(Rating::Like),
            18.0,
            rate(Rating::Like)
        ),
    ]
    .spacing(2)
    .align_y(Center)
    .into()
}

fn buttons<'a>(
    song: &'a Song,
    playback: &'a Playback,
    controls: Controls,
    format: Option<&'a str>,
) -> Element<'a, Message> {
    let volume_icon: &'static [u8] = if controls.muted {
        include_bytes!("../assets/icons/volume-muted.svg")
    } else {
        include_bytes!("../assets/icons/volume.svg")
    };

    let transport = transport(playback, controls);

    let extras = row![
        icon_button(
            volume_icon,
            theme::text_secondary(),
            18.0,
            Some(Message::ToggleMute)
        ),
        volume_bar(controls),
        icon_button(
            include_bytes!("../assets/icons/equalizer.svg"),
            theme::text_secondary(),
            18.0,
            Some(Message::OpenSettings(crate::settings_view::Tab::Audio))
        ),
        icon_button(
            include_bytes!("../assets/icons/video.svg"),
            toggle_color(controls.video),
            18.0,
            controls.can_video.then_some(Message::ToggleVideo)
        ),
        icon_button(
            include_bytes!("../assets/icons/miniplayer.svg"),
            theme::text_secondary(),
            18.0,
            Some(Message::ToggleMini)
        ),
        icon_button(
            include_bytes!("../assets/icons/fullscreen.svg"),
            theme::text_secondary(),
            18.0,
            Some(Message::ToggleImmersive)
        ),
    ]
    .spacing(8)
    .align_y(Center);

    if controls.compact {
        return column![
            now_playing(song, format),
            row![transport, space::horizontal(), extras].align_y(Center)
        ]
        .spacing(8)
        .into();
    }
    row![transport, now_playing(song, format), extras]
        .spacing(14)
        .align_y(Center)
        .into()
}

fn now_playing<'a>(song: &'a Song, format: Option<&'a str>) -> Element<'a, Message> {
    let cover: Element<'_, Message> = match &song.cover {
        Some(handle) => image(handle.clone())
            .width(40)
            .height(40)
            .border_radius(5)
            .into(),
        None => container(space())
            .width(40)
            .height(40)
            .style(|_| theme::panel(theme::hero()))
            .into(),
    };

    let details = column![
        text(&song.title)
            .size(14)
            .wrapping(iced::widget::text::Wrapping::None),
        text(format!("{} \u{b7} {}", song.artist, song.album))
            .size(12)
            .color(theme::text_secondary())
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(2)
    .width(Fill)
    .clip(true);

    let badge = format.map(|format| {
        container(text(format).size(10).color(theme::text_secondary()))
            .padding([2, 6])
            .style(|_| {
                container::Style::default().border(border::rounded(5).color(theme::line()).width(1))
            })
    });

    let card = container(row![cover, details].push(badge).spacing(11).align_y(Center))
        .max_width(460)
        .padding([6, 10])
        .style(|_| {
            container::background(theme::tint(0.05))
                .border(border::rounded(9).color(theme::line()).width(1))
        });

    container(card).center_x(Fill).into()
}

fn volume_bar(controls: Controls) -> Element<'static, Message> {
    let volume = if controls.muted { 0.0 } else { controls.volume };
    slider(0.0..=100.0, volume, Message::VolumeChanged)
        .on_release(Message::VolumeDone)
        .width(90)
        .height(14)
        .style(|_, status| theme::thin_slider(status, theme::text_secondary()))
        .into()
}

/// The accent color for a button that is switched on.
fn toggle_color(on: bool) -> Color {
    if on {
        theme::accent()
    } else {
        theme::text_secondary()
    }
}

/// A round button with an icon. Without a message it is drawn but does nothing.
pub(crate) fn icon_button(
    bytes: &'static [u8],
    color: Color,
    size: f32,
    on_press: Option<Message>,
) -> Element<'static, Message> {
    button(theme::icon(bytes, color, size))
        .on_press_maybe(on_press)
        .padding(8)
        .style(|_, status| button::Style {
            border: border::rounded(20),
            ..theme::flat_button(status, false)
        })
        .into()
}
