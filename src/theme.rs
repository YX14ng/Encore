//! Encore's colors and shared styles.

use iced::widget::{button, container, slider, svg, toggler};
use iced::{Background, Color, Font, Theme, border};

/// The app's typeface, bundled in `assets/fonts/`.
pub const INTER: Font = Font::with_name("Inter");

pub const SIDEBAR: Color = Color::from_rgb8(0x13, 0x13, 0x15);
pub const CENTER: Color = Color::from_rgb8(0x19, 0x19, 0x1c);
pub const LYRICS: Color = Color::from_rgb8(0x15, 0x15, 0x17);
pub const TEXT: Color = Color::from_rgb8(0xf3, 0xf3, 0xf5);
pub const TEXT_SECONDARY: Color = Color::from_rgb8(0x9b, 0x9b, 0xa4);
pub const TEXT_DIM: Color = Color::from_rgb8(0x5f, 0x5f, 0x68);
pub const ACCENT: Color = Color::from_rgb8(0xff, 0x44, 0x59);
pub const LINE: Color = Color::from_rgba8(255, 255, 255, 0.07);
pub const HERO: Color = Color::from_rgb8(0x24, 0x24, 0x24);

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

/// A square SVG icon painted in one color.
pub fn icon(bytes: &'static [u8], color: Color, size: f32) -> svg::Svg<'static> {
    svg(svg::Handle::from_memory(bytes))
        .width(size)
        .height(size)
        .style(move |_, _| svg::Style { color: Some(color) })
}

/// A slim slider like Cider's: a thin rail whose handle shows on hover and while dragging.
pub fn thin_slider(status: slider::Status, fill: Color) -> slider::Style {
    let radius = match status {
        slider::Status::Active => 0.0,
        slider::Status::Hovered | slider::Status::Dragged => 6.0,
    };
    slider::Style {
        rail: slider::Rail {
            backgrounds: (fill.into(), Color::from_rgba8(255, 255, 255, 0.14).into()),
            width: 4.0,
            border: border::rounded(4),
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius },
            background: TEXT.into(),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    }
}

/// An on/off switch in the accent color.
pub fn switch(theme: &Theme, status: toggler::Status) -> toggler::Style {
    let mut style = toggler::default(theme, status);
    if let toggler::Status::Active { is_toggled: true }
    | toggler::Status::Hovered { is_toggled: true } = status
    {
        style.background = ACCENT.into();
    }
    style
}
