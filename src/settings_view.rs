//! The settings screen: a tab bar over the appearance, audio and plugin settings. The appearance
//! tab (themes, accent color, custom theme files) lives here, the audio tab in `audio_view.rs`
//! and the plugins tab in `plugins.rs`.

use iced::font::Weight;
use iced::widget::{Row, button, column, container, row, scrollable, space, text};
use iced::{Center, Color, Element, Fill, Font, border};

use crate::themes::{ACCENTS, Accent, Appearance, Theme, themes_dir};
use crate::{Message, theme};

/// The tabs of the settings screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Appearance,
    Audio,
    Plugins,
}

const TABS: [(Tab, &str); 3] = [
    (Tab::Appearance, "Appearance"),
    (Tab::Audio, "Audio"),
    (Tab::Plugins, "Plugins"),
];

/// The screen: a heading, the tabs and the selected tab's `content`.
pub fn view(tab: Tab, content: Element<'_, Message>) -> Element<'_, Message> {
    let tabs = Row::with_children(
        TABS.iter()
            .map(|&(each, label)| chip(label, each == tab, Message::OpenSettings(each))),
    )
    .spacing(8);
    scrollable(
        column![heading("Settings", 26.0), tabs, content]
            .spacing(24)
            .padding([28, 32])
            .max_width(820),
    )
    .height(Fill)
    .into()
}

/// The appearance tab: theme cards, accent swatches and where custom themes go.
pub fn appearance<'a>(
    appearance: &'a Appearance,
    themes: &'a [Theme],
    errors: &'a [String],
    cover: Option<Color>,
) -> Element<'a, Message> {
    let cards = Row::with_children(
        themes
            .iter()
            .map(|each| theme_card(each, each.name == appearance.theme)),
    )
    .spacing(14)
    .wrap();

    let mut accents = vec![
        chip(
            "Theme",
            appearance.accent == Accent::Theme,
            Message::PickAccent(Accent::Theme),
        ),
        chip(
            "From cover",
            appearance.accent == Accent::Cover,
            Message::PickAccent(Accent::Cover),
        ),
    ];
    accents.extend(ACCENTS.iter().enumerate().map(|(index, color)| {
        swatch(
            *color,
            appearance.accent == Accent::Fixed(index),
            Message::PickAccent(Accent::Fixed(index)),
        )
    }));
    let accent_note = match (appearance.accent, cover) {
        (Accent::Cover, None) => Some("The accent changes with each song's cover."),
        _ => None,
    };

    let mut custom = column![
        note(format!(
            "Add your own themes as JSON files in {}. Colors are #rrggbb; the ones left out \
             come from Encore's dark theme, or from Daylight with \"light\": true.",
            themes_dir().display()
        )),
        container(
            text(
                r##"{ "name": "Rosé", "accent": "#eb6f92", "center": "#191724", "text": "#e0def4" }"##
            )
            .size(12)
            .font(Font::MONOSPACE)
            .color(theme::text_secondary())
        )
        .padding([8, 12])
        .style(|_| container::background(theme::tint(0.05)).border(border::rounded(6))),
        button(text("Reload themes").size(13))
            .on_press(Message::ReloadThemes)
            .padding([6, 14])
            .style(|_, status| {
                let mut style = theme::flat_button(status, false);
                style.border = border::rounded(16).color(theme::line()).width(1);
                style
            }),
    ]
    .spacing(10);
    for error in errors {
        custom = custom.push(text(error).size(13).color(theme::accent()));
    }

    column![
        heading("Theme", 16.0),
        cards,
        heading("Accent color", 16.0),
        Row::with_children(accents)
            .spacing(8)
            .align_y(Center)
            .wrap(),
    ]
    .push(accent_note.map(note))
    .push(heading("Custom themes", 16.0))
    .push(custom)
    .spacing(14)
    .into()
}

/// A small preview of a theme: its sidebar, center and accent, with the name below.
fn theme_card(each: &Theme, selected: bool) -> Element<'_, Message> {
    let palette = each.palette;
    let block = move |color: Color| {
        container(space())
            .width(Fill)
            .height(Fill)
            .style(move |_| container::background(color))
    };
    let accent_bar = container(space())
        .width(40)
        .height(6)
        .style(move |_| container::background(palette.accent).border(border::rounded(3)));
    let center = container(
        column![
            accent_bar,
            container(space())
                .width(70)
                .height(6)
                .style(move |_| container::background(palette.text_dim).border(border::rounded(3)))
        ]
        .spacing(6),
    )
    .padding(10)
    .width(Fill)
    .height(Fill)
    .style(move |_| container::background(palette.center));
    let preview = container(row![block(palette.sidebar).width(34), center])
        .width(150)
        .height(84)
        .clip(true)
        .style(move |_| {
            container::Style::default().border(
                border::rounded(8)
                    .color(palette.ink.scale_alpha(0.1))
                    .width(1),
            )
        });

    let border_color = if selected {
        theme::accent()
    } else {
        Color::TRANSPARENT
    };
    button(column![preview, text(&each.name).size(13)].spacing(8))
        .on_press(Message::PickTheme(each.name.clone()))
        .padding(6)
        .style(move |_, status| {
            let mut style = theme::flat_button(status, false);
            style.border = border::rounded(12).color(border_color).width(2);
            style
        })
        .into()
}

/// A round color button for the accent.
fn swatch(color: Color, selected: bool, message: Message) -> Element<'static, Message> {
    let ring = if selected {
        theme::text_primary()
    } else {
        Color::TRANSPARENT
    };
    button(space())
        .on_press(message)
        .width(28)
        .height(28)
        .style(move |_, _| button::Style {
            background: Some(color.into()),
            border: border::rounded(14).color(ring).width(2),
            ..button::Style::default()
        })
        .into()
}

fn chip(label: &str, selected: bool, message: Message) -> Element<'_, Message> {
    button(text(label).size(13))
        .on_press(message)
        .padding([6, 14])
        .style(move |_, status| {
            let mut style = theme::flat_button(status, false);
            if selected {
                style.background = Some(theme::accent().into());
                style.text_color = Color::WHITE;
            }
            style.border = border::rounded(16).color(theme::line()).width(1);
            style
        })
        .into()
}

fn heading(label: &str, size: f32) -> Element<'_, Message> {
    text(label)
        .size(size)
        .font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        })
        .into()
}

fn note<'a>(message: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(message).size(13).color(theme::text_secondary()).into()
}
