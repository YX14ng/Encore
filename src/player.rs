//! The player bar at the bottom of the center column.

use iced::widget::{button, column, container, image, progress_bar, row, space, text};
use iced::{Background, Center, Color, Element, Fill, border};

use crate::clock::{self, Playback};
use crate::song::Song;
use crate::{Message, theme};

/// The progress bar, the transport buttons and the card of the track that is playing.
pub fn view<'a>(song: &'a Song, playback: &'a Playback, liked: bool) -> Element<'a, Message> {
    let divider = container(space())
        .width(Fill)
        .height(1)
        .style(|_| container::background(theme::LINE));

    let content = column![progress(playback), controls(song, playback, liked)]
        .spacing(10)
        .padding([10, 20]);

    container(column![divider, content])
        .width(Fill)
        .style(|_| container::background(Color::from_rgb8(0x16, 0x16, 0x19)))
        .into()
}

fn progress(playback: &Playback) -> Element<'_, Message> {
    let elapsed = playback.elapsed;
    let left = clock::remaining(elapsed, playback.duration);

    let bar = progress_bar(0.0..=playback.duration as f32, elapsed as f32)
        .girth(4)
        .style(|_| progress_bar::Style {
            background: Background::Color(Color::from_rgba8(255, 255, 255, 0.14)),
            bar: Background::Color(theme::TEXT),
            border: border::rounded(4),
        });

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
    text(label).size(11).color(theme::TEXT_SECONDARY).into()
}

fn controls<'a>(song: &'a Song, playback: &'a Playback, liked: bool) -> Element<'a, Message> {
    let play_icon: &'static [u8] = if playback.is_playing {
        include_bytes!("../assets/icons/pause.svg")
    } else {
        include_bytes!("../assets/icons/play.svg")
    };
    let like_color = if liked {
        theme::ACCENT
    } else {
        theme::TEXT_SECONDARY
    };

    let transport = row![
        icon_button(
            include_bytes!("../assets/icons/shuffle.svg"),
            theme::TEXT_SECONDARY,
            18.0,
            None
        ),
        icon_button(
            include_bytes!("../assets/icons/previous.svg"),
            theme::TEXT,
            18.0,
            None
        ),
        icon_button(play_icon, theme::TEXT, 26.0, Some(Message::TogglePlay)),
        icon_button(
            include_bytes!("../assets/icons/next.svg"),
            theme::TEXT,
            18.0,
            Some(Message::Next)
        ),
        icon_button(
            include_bytes!("../assets/icons/repeat.svg"),
            theme::TEXT_SECONDARY,
            18.0,
            None
        ),
        icon_button(
            include_bytes!("../assets/icons/like.svg"),
            like_color,
            18.0,
            Some(Message::ToggleLike)
        ),
    ]
    .spacing(2)
    .align_y(Center);

    let extras = row![
        theme::icon(
            include_bytes!("../assets/icons/volume.svg"),
            theme::TEXT_SECONDARY,
            18.0
        ),
        volume_bar(),
        icon_button(
            include_bytes!("../assets/icons/miniplayer.svg"),
            theme::TEXT_SECONDARY,
            18.0,
            None
        ),
        icon_button(
            include_bytes!("../assets/icons/fullscreen.svg"),
            theme::TEXT_SECONDARY,
            18.0,
            None
        ),
    ]
    .spacing(8)
    .align_y(Center);

    row![transport, now_playing(song), extras]
        .spacing(14)
        .align_y(Center)
        .into()
}

fn now_playing(song: &Song) -> Element<'_, Message> {
    let cover: Element<'_, Message> = match &song.cover {
        Some(handle) => image(handle.clone())
            .width(40)
            .height(40)
            .border_radius(5)
            .into(),
        None => container(space())
            .width(40)
            .height(40)
            .style(|_| theme::panel(theme::HERO))
            .into(),
    };

    let details = column![
        text(&song.title).size(14),
        text(format!("{} \u{b7} {}", song.artist, song.album))
            .size(12)
            .color(theme::TEXT_SECONDARY),
    ]
    .spacing(2)
    .width(Fill);

    let badge = container(text("OPUS 160").size(10).color(theme::TEXT_SECONDARY))
        .padding([2, 6])
        .style(|_| {
            container::Style::default().border(border::rounded(5).color(theme::LINE).width(1))
        });

    let card = container(row![cover, details, badge].spacing(11).align_y(Center))
        .max_width(460)
        .padding([6, 10])
        .style(|_| {
            container::background(Color::from_rgba8(255, 255, 255, 0.05))
                .border(border::rounded(9).color(theme::LINE).width(1))
        });

    container(card).center_x(Fill).into()
}

fn volume_bar() -> Element<'static, Message> {
    progress_bar(0.0..=1.0, 0.62)
        .length(90)
        .girth(4)
        .style(|_| progress_bar::Style {
            background: Background::Color(Color::from_rgba8(255, 255, 255, 0.14)),
            bar: Background::Color(theme::TEXT_SECONDARY),
            border: border::rounded(4),
        })
        .into()
}

/// A round button with an icon. Without a message it is drawn but does nothing.
fn icon_button(
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
