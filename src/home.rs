//! The home screen: greeting, recently played and mixes.

use iced::font::Weight;
use iced::gradient::Linear;
use iced::widget::text::Wrapping;
use iced::widget::{Column, Row, column, container, image, row, scrollable, space, stack, text};
use iced::{ContentFit, Degrees, Element, Fill, Font};

use crate::mock::{self, Card};
use crate::search::{self, Shared};
use crate::youtube::{SearchTrack, Tile};
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

/// How many recently played songs the home screen shows (three rows of three).
const RECENT_SONGS: usize = 9;

/// What the home screen shows below the greeting.
pub enum Sections<'a> {
    /// Signed out: the sample data.
    Sample {
        recents: &'a [Card],
        mixes: &'a [Card],
    },
    /// Signed in: the user's own music.
    Personal {
        history: &'a [SearchTrack],
        mixes: &'a [Tile],
        playlists: &'a [Tile],
        albums: &'a [Tile],
        shared: Shared<'a>,
    },
}

/// The greeting over the blurred cover, then recently played, mixes and, when signed in, the
/// user's playlists and albums.
pub fn view<'a>(
    hour: u32,
    backdrop: Option<&'a image::Handle>,
    zoom: f32,
    sections: Sections<'a>,
) -> Element<'a, Message> {
    let content = column![hero(hour, backdrop, zoom)];
    let content = match sections {
        Sections::Sample { recents, mixes } => content
            .push(section("Recently played", recents_grid(recents)))
            .push(section("Mixed for you", mixes_row(mixes))),
        Sections::Personal {
            history,
            mixes,
            playlists,
            albums,
            shared,
        } => content
            .push(
                (!history.is_empty())
                    .then(|| section("Recently played", personal_recents(history, shared))),
            )
            .push(
                (!mixes.is_empty()).then(|| section("Mixed for you", search::shelf(mixes, shared))),
            )
            .push(
                (!playlists.is_empty())
                    .then(|| section("Your playlists", search::shelf(playlists, shared))),
            )
            .push(
                (!albums.is_empty())
                    .then(|| section("Saved albums", search::shelf(albums, shared))),
            ),
    };

    scrollable(content).height(Fill).into()
}

/// The latest songs of the history in rows of three; clicking one plays the history from there.
fn personal_recents<'a>(history: &'a [SearchTrack], shared: Shared<'a>) -> Element<'a, Message> {
    let shown = RECENT_SONGS.min(history.len());
    let indexes: Vec<usize> = (0..shown).collect();
    let rows = indexes.chunks(3).map(|chunk| {
        Row::with_children(
            chunk
                .iter()
                .map(|&index| search::track_row(history, index, shared)),
        )
        .spacing(18)
        .into()
    });
    Column::with_children(rows).into()
}

fn hero(hour: u32, backdrop: Option<&image::Handle>, zoom: f32) -> Element<'_, Message> {
    let background: Element<'_, Message> = match backdrop {
        Some(handle) => image(handle.clone())
            .width(Fill)
            .height(HERO_HEIGHT)
            .content_fit(ContentFit::Cover)
            .scale(zoom)
            .into(),
        None => container(space())
            .width(Fill)
            .height(HERO_HEIGHT)
            .style(|_| theme::panel(theme::hero()))
            .into(),
    };

    let fade = container(space())
        .width(Fill)
        .height(HERO_HEIGHT)
        .style(|_| {
            let gradient = Linear::new(Degrees(180.0))
                .add_stop(0.0, theme::center_bg().scale_alpha(0.0))
                .add_stop(0.6, theme::center_bg().scale_alpha(0.5))
                .add_stop(1.0, theme::center_bg());
            container::background(gradient)
        });

    let greeting = container(text(greeting(hour)).size(40).font(bold()))
        .width(Fill)
        .height(HERO_HEIGHT)
        .align_bottom(HERO_HEIGHT)
        .padding([22, 32]);

    stack![background, fade, greeting].into()
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
        text(&card.subtitle).size(12).color(theme::text_secondary()),
    ]
    .spacing(2)
    .width(Fill);

    container(
        row![
            cover(card, 44.0),
            details,
            text("···").color(theme::accent())
        ]
        .spacing(12)
        .align_y(iced::Center),
    )
    .width(Fill)
    .padding([9, 6])
    .into()
}

fn mixes_row(mixes: &[Card]) -> Element<'_, Message> {
    let cards = row(mixes.iter().map(mix_card)).spacing(16);

    let scrollbar = scrollable::Scrollbar::new().spacing(10);

    scrollable(cards)
        .direction(scrollable::Direction::Horizontal(scrollbar))
        .into()
}

fn mix_card(card: &Card) -> Element<'_, Message> {
    column![
        cover(card, 150.0),
        space().height(8),
        text(&card.title).size(13),
        text(&card.subtitle)
            .size(12)
            .color(theme::text_secondary())
            .wrapping(Wrapping::None),
    ]
    .width(150)
    .clip(true)
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
        ..theme::INTER
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
