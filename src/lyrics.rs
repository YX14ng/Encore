//! The synced lyrics panel on the right.

use iced::widget::{container, text};
use iced::{Element, Fill};

use crate::{Message, theme};

pub const WIDTH: f32 = 300.0;

pub fn view() -> Element<'static, Message> {
    container(text("Lyrics"))
        .width(WIDTH)
        .height(Fill)
        .padding(16)
        .style(|_| theme::panel(theme::LYRICS))
        .into()
}
