//! The player bar at the bottom of the center column.

use iced::widget::{button, column, container, image, row, slider, space, text};
use iced::{Center, Color, Element, Fill, border};

use crate::clock::{self, Playback};
use crate::queue::Repeat;
use crate::song::Song;
use crate::{Message, theme};

/// The state of the player's buttons and sliders.
#[derive(Debug, Clone, Copy)]
pub struct Controls {
    pub liked: bool,
    /// Where the progress bar is being dragged to, in seconds.
    pub seeking: Option<f32>,
    /// From 0 to 100.
    pub volume: f32,
    pub muted: bool,
    pub shuffle: bool,
    pub repeat: Repeat,
}

/// The progress bar, the transport buttons and the card of the track that is playing.
pub fn view<'a>(
    song: &'a Song,
    playback: &'a Playback,
    controls: Controls,
) -> Element<'a, Message> {
    let divider = container(space())
        .width(Fill)
        .height(1)
        .style(|_| container::background(theme::line()));

    let content = column![
        progress(playback, controls.seeking),
        buttons(song, playback, controls)
    ]
    .spacing(10)
    .padding([10, 20]);

    container(column![divider, content])
        .width(Fill)
        .style(|_| container::background(theme::player_bg()))
        .into()
}

/// The progress bar: drag or click it to jump through the song.
fn progress(playback: &Playback, seeking: Option<f32>) -> Element<'static, Message> {
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

fn buttons<'a>(song: &'a Song, playback: &'a Playback, controls: Controls) -> Element<'a, Message> {
    let play_icon: &'static [u8] = if playback.is_playing {
        include_bytes!("../assets/icons/pause.svg")
    } else {
        include_bytes!("../assets/icons/play.svg")
    };
    let like_color = if controls.liked {
        theme::accent()
    } else {
        theme::text_secondary()
    };

    let repeat_icon: &'static [u8] = if controls.repeat == Repeat::One {
        include_bytes!("../assets/icons/repeat-one.svg")
    } else {
        include_bytes!("../assets/icons/repeat.svg")
    };
    let volume_icon: &'static [u8] = if controls.muted {
        include_bytes!("../assets/icons/volume-muted.svg")
    } else {
        include_bytes!("../assets/icons/volume.svg")
    };

    let transport = row![
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
            include_bytes!("../assets/icons/like.svg"),
            like_color,
            18.0,
            Some(Message::ToggleLike)
        ),
    ]
    .spacing(2)
    .align_y(Center);

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
            Some(Message::OpenAudio)
        ),
        icon_button(
            include_bytes!("../assets/icons/miniplayer.svg"),
            theme::text_secondary(),
            18.0,
            None
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
            .style(|_| theme::panel(theme::hero()))
            .into(),
    };

    let details = column![
        text(&song.title).size(14),
        text(format!("{} \u{b7} {}", song.artist, song.album))
            .size(12)
            .color(theme::text_secondary()),
    ]
    .spacing(2)
    .width(Fill);

    let badge = container(text("OPUS 160").size(10).color(theme::text_secondary()))
        .padding([2, 6])
        .style(|_| {
            container::Style::default().border(border::rounded(5).color(theme::line()).width(1))
        });

    let card = container(row![cover, details, badge].spacing(11).align_y(Center))
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
