use iced::widget::{button, column, text};
use iced::{Center, Element};

use crate::Message;
use crate::clock::{self, Playback};

pub fn view(playback: &Playback) -> Element<'_, Message> {
    let label = if playback.is_playing { "Pause" } else { "Play" };
    let elapsed = playback.elapsed;
    let left = clock::remaining(elapsed, playback.duration);

    column![
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
