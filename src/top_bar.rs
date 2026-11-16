//! The bar above the center column, with navigation and panel toggles.

use iced::widget::{button, row, space};
use iced::{Center, Element};

use crate::layout::Panel;
use crate::{Message, theme};

/// Back and forward, the panel toggles and the shortcuts on the right. Back is active away from
/// Home, Forward after going back. The sidebar, lyrics and queue buttons are highlighted while
/// their panel is shown, and the together button while a session runs.
pub fn view(
    show_sidebar: bool,
    panel: Option<Panel>,
    can_go_back: bool,
    can_go_forward: bool,
    together: bool,
) -> Element<'static, Message> {
    row![
        tool(
            include_bytes!("../assets/icons/chevron-left.svg"),
            can_go_back.then_some(Message::Back),
            false
        ),
        tool(
            include_bytes!("../assets/icons/chevron-right.svg"),
            can_go_forward.then_some(Message::Forward),
            false
        ),
        tool(
            include_bytes!("../assets/icons/sidebar.svg"),
            Some(Message::ToggleSidebar),
            show_sidebar
        ),
        space::horizontal(),
        tool(
            include_bytes!("../assets/icons/settings.svg"),
            Some(Message::OpenSettings(crate::settings_view::Tab::Appearance)),
            false
        ),
        tool(
            include_bytes!("../assets/icons/together.svg"),
            Some(Message::OpenTogether),
            together
        ),
        tool(
            include_bytes!("../assets/icons/lyrics.svg"),
            Some(Message::TogglePanel(Panel::Lyrics)),
            panel == Some(Panel::Lyrics)
        ),
        tool(
            include_bytes!("../assets/icons/queue.svg"),
            Some(Message::TogglePanel(Panel::Queue)),
            panel == Some(Panel::Queue)
        ),
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
        theme::text_primary()
    } else {
        theme::text_secondary()
    };

    button(theme::icon(bytes, color, 18.0))
        .on_press_maybe(on_press)
        .padding([6, 8])
        .style(move |_, status| theme::flat_button(status, active))
        .into()
}
