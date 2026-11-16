//! Encore's colors and shared styles.

use std::sync::RwLock;

use iced::widget::{button, container, slider, svg, toggler};
use iced::{Background, Color, Font, Theme, border};

/// The app's typeface, bundled in `assets/fonts/`.
pub const INTER: Font = Font::with_name("Inter");

/// The colors of the interface. Themes swap it while the app runs (see `themes.rs`); styles
/// read it when they are drawn, so a new palette shows on the next frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub sidebar: Color,
    pub center: Color,
    pub lyrics: Color,
    pub player: Color,
    /// Placeholder squares behind covers that have not loaded.
    pub hero: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub text_dim: Color,
    pub accent: Color,
    /// The color of hover tints and lines: white on dark themes, black on light ones.
    pub ink: Color,
    /// Whether iced's own widgets (scrollbars, text inputs) should use their light look.
    pub light: bool,
}

/// Encore's own dark palette, the default.
pub const ENCORE: Palette = Palette {
    sidebar: Color::from_rgb8(0x13, 0x13, 0x15),
    center: Color::from_rgb8(0x19, 0x19, 0x1c),
    lyrics: Color::from_rgb8(0x15, 0x15, 0x17),
    player: Color::from_rgb8(0x16, 0x16, 0x19),
    hero: Color::from_rgb8(0x24, 0x24, 0x24),
    text: Color::from_rgb8(0xf3, 0xf3, 0xf5),
    text_secondary: Color::from_rgb8(0x9b, 0x9b, 0xa4),
    text_dim: Color::from_rgb8(0x5f, 0x5f, 0x68),
    accent: Color::from_rgb8(0xff, 0x44, 0x59),
    ink: Color::WHITE,
    light: false,
};

/// The palette in use. A global because iced's style closures have no way to receive it, and
/// every view reads it while only the appearance settings write it.
static PALETTE: RwLock<Palette> = RwLock::new(ENCORE);

/// The palette in use.
pub fn palette() -> Palette {
    PALETTE.read().map(|palette| *palette).unwrap_or(ENCORE)
}

/// Switches the palette; the next frame draws with it.
pub fn set_palette(palette: Palette) {
    if let Ok(mut current) = PALETTE.write() {
        *current = palette;
    }
}

pub fn sidebar_bg() -> Color {
    palette().sidebar
}

pub fn center_bg() -> Color {
    palette().center
}

pub fn lyrics_bg() -> Color {
    palette().lyrics
}

pub fn player_bg() -> Color {
    palette().player
}

pub fn hero() -> Color {
    palette().hero
}

pub fn text_primary() -> Color {
    palette().text
}

pub fn text_secondary() -> Color {
    palette().text_secondary
}

pub fn text_dim() -> Color {
    palette().text_dim
}

pub fn accent() -> Color {
    palette().accent
}

/// Dividers and outlines.
pub fn line() -> Color {
    tint(0.07)
}

/// The ink color (white or black) at `alpha`, for hover and selection tints.
pub fn tint(alpha: f32) -> Color {
    Color {
        a: alpha,
        ..palette().ink
    }
}

/// A container style with a solid background and Encore's text color.
pub fn panel(background: Color) -> container::Style {
    container::background(background).color(text_primary())
}

/// A flat button that only shows a background when hovered or selected.
pub fn flat_button(status: button::Status, selected: bool) -> button::Style {
    let background = if selected {
        Some(Background::Color(tint(0.10)))
    } else if status == button::Status::Hovered {
        Some(Background::Color(tint(0.06)))
    } else {
        None
    };

    button::Style {
        background,
        text_color: text_primary(),
        border: border::rounded(7),
        ..button::Style::default()
    }
}

/// A square SVG icon painted in one color.
pub fn icon<'a>(bytes: &'static [u8], color: Color, size: f32) -> svg::Svg<'a> {
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
            backgrounds: (fill.into(), tint(0.14).into()),
            width: 4.0,
            border: border::rounded(4),
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius },
            background: text_primary().into(),
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
        style.background = accent().into();
    }
    style
}
