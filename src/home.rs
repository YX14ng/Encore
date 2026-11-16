//! The home screen: greeting, recently played and mixes.

use std::borrow::Cow;

use iced::font::Weight;
use iced::gradient::Linear;
use iced::widget::text::Wrapping;
use iced::widget::{Column, Row, column, container, image, row, scrollable, space, stack, text};
use iced::{ContentFit, Degrees, Element, Fill, Font};
use rust_i18n::t;

use crate::mock::{self, Card};
use crate::search::{self, Shared};
use crate::youtube::{SearchTrack, Tile};
use crate::{Message, theme};

const HERO_HEIGHT: f32 = 330.0;

/// The greeting for an hour of the day, from 0 to 23.
pub fn greeting(hour: u32) -> Cow<'static, str> {
    match hour {
        5..=11 => t!("home.good_morning"),
        12..=19 => t!("home.good_afternoon"),
        _ => t!("home.good_evening"),
    }
}

/// How many recently played songs the home screen shows (three rows of three).
const RECENT_SONGS: usize = 9;
/// The narrowest a recently played song gets before the rows hold fewer songs.
const RECENT_MIN_WIDTH: f32 = 300.0;

/// What the home screen shows below the greeting.
pub enum Sections<'a> {
    /// Not signed in: a button to sign in, or (`checking`) a note while the saved session is
    /// checked.
    SignedOut { checking: bool },
    /// The sample data (only in tests).
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
    width: f32,
    backdrop: Option<&'a image::Handle>,
    zoom: f32,
    sections: Sections<'a>,
) -> Element<'a, Message> {
    let content = column![hero(hour, backdrop, zoom)];
    let columns = recent_columns(width);
    let content = match sections {
        Sections::SignedOut { checking: true } => {
            content.push(container(crate::library::note(t!("library.loading"))).padding([12, 32]))
        }
        Sections::SignedOut { checking: false } => content.push(
            column![
                crate::library::note(t!("library.signed_out")),
                crate::library::accent_button(t!("library.sign_in"), Message::OpenAccount),
            ]
            .spacing(12)
            .padding([12, 32]),
        ),
        Sections::Sample { recents, mixes } => content
            .push(section(
                t!("home.recently_played"),
                recents_grid(recents, columns),
            ))
            .push(section(t!("library.mixed_for_you"), mixes_row(mixes))),
        Sections::Personal {
            history,
            mixes,
            playlists,
            albums,
            shared,
        } => content
            .push((!history.is_empty()).then(|| {
                section(
                    t!("home.recently_played"),
                    personal_recents(history, columns, shared),
                )
            }))
            .push(
                (!mixes.is_empty())
                    .then(|| section(t!("library.mixed_for_you"), search::shelf(mixes, shared))),
            )
            .push(
                (!playlists.is_empty())
                    .then(|| section(t!("home.your_playlists"), search::shelf(playlists, shared))),
            )
            .push(
                (!albums.is_empty())
                    .then(|| section(t!("home.saved_albums"), search::shelf(albums, shared))),
            ),
    };

    scrollable(content).height(Fill).into()
}

/// How many recently played songs fit side by side in `width`: up to three.
fn recent_columns(width: f32) -> usize {
    // The sections' side padding, and the room a row needs for a readable title.
    ((width - 64.0) / RECENT_MIN_WIDTH).floor().clamp(1.0, 3.0) as usize
}

/// The latest songs of the history in rows of `columns`; clicking one plays the history from
/// there.
fn personal_recents<'a>(
    history: &'a [SearchTrack],
    columns: usize,
    shared: Shared<'a>,
) -> Element<'a, Message> {
    let shown = RECENT_SONGS.min(history.len());
    let indexes: Vec<usize> = (0..shown).collect();
    let rows = indexes.chunks(columns).map(|chunk| {
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

fn section<'a>(
    title: impl text::IntoFragment<'a>,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    column![text(title).size(18).font(bold()), body]
        .spacing(12)
        .padding([12, 32])
        .into()
}

fn recents_grid(recents: &[Card], columns: usize) -> Element<'_, Message> {
    let rows = recents
        .chunks(columns)
        .map(|chunk| row(chunk.iter().map(recent_row)).spacing(18).into());

    Column::with_children(rows).into()
}

fn recent_row(card: &Card) -> Element<'_, Message> {
    let details = column![
        text(&card.title).size(14).wrapping(Wrapping::None),
        text(&card.subtitle)
            .size(12)
            .color(theme::text_secondary())
            .wrapping(Wrapping::None),
    ]
    .spacing(2)
    .width(Fill)
    .clip(true);

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
    fn recent_songs_take_as_many_columns_as_fit() {
        assert_eq!(recent_columns(1300.0 - 236.0 - 300.0), 2);
        assert_eq!(recent_columns(1600.0), 3);
        assert_eq!(recent_columns(300.0), 1);
    }

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
