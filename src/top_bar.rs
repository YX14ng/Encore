//! The bar above the center column, with navigation and panel toggles.

use iced::Element;
use iced::widget::{button, row, space};

use crate::Message;

/// Back and forward, the panel toggles and the shortcuts on the right.
pub fn view() -> Element<'static, Message> {
    row![
        button("‹"),
        button("›"),
        button("Sidebar").on_press(Message::ToggleSidebar),
        button("···"),
        space::horizontal(),
        button("Together"),
        button("Cast"),
        button("Lyrics").on_press(Message::ToggleLyrics),
        button("Queue"),
    ]
    .spacing(4)
    .padding(10)
    .into()
}
