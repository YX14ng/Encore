//! The bar above the center column, with navigation and panel toggles.

use iced::widget::{button, row, space};
use iced::{Center, Element};

use crate::{Message, theme};

/// Back and forward, the panel toggles and the shortcuts on the right. Back is active while a
/// search or a collection is open.
/// The sidebar and lyrics buttons are highlighted while their panel is shown.
pub fn view(show_sidebar: bool, show_lyrics: bool, can_go_back: bool) -> Element<'static, Message> {
    row![
        tool(include_bytes!("../assets/icons/more.svg"), None, false),
        tool(
            include_bytes!("../assets/icons/chevron-left.svg"),
            can_go_back.then_some(Message::Back),
            false
        ),
        tool(
            include_bytes!("../assets/icons/chevron-right.svg"),
            None,
            false
        ),
        tool(
            include_bytes!("../assets/icons/sidebar.svg"),
            Some(Message::ToggleSidebar),
            show_sidebar
        ),
        space::horizontal(),
        tool(include_bytes!("../assets/icons/together.svg"), None, false),
        tool(include_bytes!("../assets/icons/cast.svg"), None, false),
        tool(
            include_bytes!("../assets/icons/lyrics.svg"),
            Some(Message::ToggleLyrics),
            show_lyrics
        ),
        tool(include_bytes!("../assets/icons/queue.svg"), None, false),
    ]
    .spacing(4)
    .padding([10, 14])
    .align_y(Center)
    .into()
}

/// A small square button with an icon, highlighted while `active`.
fn tool(
    bytes: &'static [u8],
    on_press: Option<Message>,
    active: bool,
) -> Element<'static, Message> {
    let color = if active {
        theme::TEXT
    } else {
        theme::TEXT_SECONDARY
    };

    button(theme::icon(bytes, color, 18.0))
        .on_press_maybe(on_press)
        .padding([6, 8])
        .style(move |_, status| theme::flat_button(status, active))
        .into()
}
