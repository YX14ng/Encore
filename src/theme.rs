//! Encore's colors and shared styles.

use iced::Color;
use iced::widget::container;

pub const SIDEBAR: Color = Color::from_rgb8(0x13, 0x13, 0x15);
pub const CENTER: Color = Color::from_rgb8(0x19, 0x19, 0x1c);
pub const LYRICS: Color = Color::from_rgb8(0x15, 0x15, 0x17);
pub const TEXT: Color = Color::from_rgb8(0xf3, 0xf3, 0xf5);

/// A container style with a solid background and Encore's text color.
pub fn panel(background: Color) -> container::Style {
    container::background(background).color(TEXT)
}
