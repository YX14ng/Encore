//! The home screen: greeting, recently played and mixes.

use iced::font::Weight;
use iced::widget::{Column, column, container, image, row, scrollable, space, text};
use iced::{Element, Fill, Font};

use crate::mock::{self, Card};
use crate::{Message, theme};

const HERO_HEIGHT: f32 = 330.0;

/// The greeting for an hour of the day, from 0 to 23.
pub fn greeting(hour: u32) -> &'static str {
    match hour {
        5..=11 => "Good morning",
        12..=19 => "Good afternoon",
        _ => "Good evening",
    }
}

pub fn view<'a>(hour: u32, recents: &'a [Card], mixes: &'a [Card]) -> Element<'a, Message> {
    let hero = container(text(greeting(hour)).size(40).font(bold()))
        .width(Fill)
        .height(HERO_HEIGHT)
        .align_bottom(HERO_HEIGHT)
        .padding([22, 32])
        .style(|_| theme::panel(theme::HERO));

    let content = column![
        hero,
        section("Recently played", recents_grid(recents)),
        section("Mixed for you", mixes_row(mixes)),
    ];

    scrollable(content).height(Fill).into()
}

fn section<'a>(title: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
    column![text(title).size(18).font(bold()), body]
        .spacing(12)
        .padding([12, 32])
        .into()
}

fn recents_grid(recents: &[Card]) -> Element<'_, Message> {
    let rows = recents
        .chunks(3)
        .map(|chunk| row(chunk.iter().map(recent_row)).spacing(18).into());

    Column::with_children(rows).into()
}

fn recent_row(card: &Card) -> Element<'_, Message> {
    let details = column![
        text(&card.title).size(14),
        text(&card.subtitle).size(12).color(theme::TEXT_SECONDARY),
    ]
    .spacing(2)
    .width(Fill);

    container(
        row![cover(card, 44.0), details, text("···").color(theme::ACCENT)]
            .spacing(12)
            .align_y(iced::Center),
    )
    .width(Fill)
    .padding([9, 6])
    .into()
}

fn mixes_row(mixes: &[Card]) -> Element<'_, Message> {
    let cards = row(mixes.iter().map(mix_card)).spacing(16);

    scrollable(cards).horizontal().into()
}

fn mix_card(card: &Card) -> Element<'_, Message> {
    column![
        cover(card, 150.0),
        space().height(8),
        text(&card.title).size(13),
        text(&card.subtitle).size(12).color(theme::TEXT_SECONDARY),
    ]
    .width(150)
    .into()
}

fn cover(card: &Card, size: f32) -> Element<'_, Message> {
    image(mock::cover_path(&card.cover))
        .width(size)
        .height(size)
        .border_radius(5)
        .into()
}

fn bold() -> Font {
    Font {
        weight: Weight::Bold,
        ..Font::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greets_by_time_of_day() {
        assert_eq!(greeting(4), "Good evening");
        assert_eq!(greeting(5), "Good morning");
        assert_eq!(greeting(11), "Good morning");
        assert_eq!(greeting(12), "Good afternoon");
        assert_eq!(greeting(19), "Good afternoon");
        assert_eq!(greeting(20), "Good evening");
        assert_eq!(greeting(23), "Good evening");
    }
}
