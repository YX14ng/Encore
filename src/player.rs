//! The player bar at the bottom of the center column.
use iced::widget::{button, column, text};
use iced::{Center, Element};

use crate::Message;
use crate::clock::{self, Playback};
use crate::mock::Track;

pub fn view<'a>(track: &'a Track, playback: &'a Playback) -> Element<'a, Message> {
    let label = if playback.is_playing { "Pause" } else { "Play" };
    let elapsed = playback.elapsed;
    let left = clock::remaining(elapsed, playback.duration);

    column![
        text(&track.title).size(16),
        text(format!("{} · {}", track.artist, track.album)).size(13),
        text(format!(
            "{} / -{}",
            clock::format_time(elapsed),
            clock::format_time(left)
        )),
        button(label).on_press(Message::TogglePlay),
    ]
    .spacing(8)
    .padding(16)
    .align_x(Center)
    .into()
}
