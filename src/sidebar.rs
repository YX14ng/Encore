//! The library sidebar on the left.

use iced::widget::{container, text};
use iced::{Element, Fill};

use crate::{Message, theme};

pub const WIDTH: f32 = 236.0;

pub fn view() -> Element<'static, Message> {
    container(text("Sidebar"))
        .width(WIDTH)
        .height(Fill)
        .padding(16)
        .style(|_| theme::panel(theme::SIDEBAR))
        .into()
}
