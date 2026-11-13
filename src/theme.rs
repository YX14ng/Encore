//! Encore's colors and shared styles.

use iced::widget::{button, container};
use iced::{Background, Color, border};

pub const SIDEBAR: Color = Color::from_rgb8(0x13, 0x13, 0x15);
pub const CENTER: Color = Color::from_rgb8(0x19, 0x19, 0x1c);
pub const LYRICS: Color = Color::from_rgb8(0x15, 0x15, 0x17);
pub const TEXT: Color = Color::from_rgb8(0xf3, 0xf3, 0xf5);
pub const TEXT_SECONDARY: Color = Color::from_rgb8(0x9b, 0x9b, 0xa4);
pub const TEXT_DIM: Color = Color::from_rgb8(0x5f, 0x5f, 0x68);
pub const ACCENT: Color = Color::from_rgb8(0xff, 0x44, 0x59);
pub const LINE: Color = Color::from_rgba8(255, 255, 255, 0.07);

/// A container style with a solid background and Encore's text color.
pub fn panel(background: Color) -> container::Style {
    container::background(background).color(TEXT)
}

/// A flat button that only shows a background when hovered or selected.
pub fn flat_button(status: button::Status, selected: bool) -> button::Style {
    let background = if selected {
        Some(Background::Color(Color::from_rgba8(255, 255, 255, 0.10)))
    } else if status == button::Status::Hovered {
        Some(Background::Color(Color::from_rgba8(255, 255, 255, 0.06)))
    } else {
        None
    };

    button::Style {
        background,
        text_color: TEXT,
        border: border::rounded(7),
        ..button::Style::default()
    }
}
