//! How the Explore screen and a mood's or genre's page look.

use iced::font::Weight;
use iced::widget::{Row, button, column, container, row, scrollable, space, text};
use iced::{Center, Color, Element, Fill, Font, border};
use rust_i18n::t;

use super::{Event, Explore, Genre};
use crate::artist::section;
use crate::search::{self, Shared, Status};
use crate::youtube::{Tile, TileKind};
use crate::{Message, theme};

/// Songs shown in each chart before "See all".
pub(super) const CHART_ROWS: usize = 10;

/// The Explore screen.
pub fn view<'a>(explore: &'a Explore, shared: Shared<'a>) -> Element<'a, Message> {
    let mut content = column![heading(t!("explore.title"))].spacing(4);
    match (&explore.status, &explore.data) {
        (Status::Failed(error), _) => {
            content = content.push(padded(note(t!("explore.load_failed", error = error))));
        }
        (_, Some(data)) => {
            if !data.moods.is_empty() {
                content = content.push(section(t!("explore.moods"), None, chips(&data.moods)));
            }
            if !data.new_albums.is_empty() {
                let shelf = search::shelf(&data.new_albums, shared);
                content = content.push(section(t!("explore.new_releases"), None, shelf));
            }
            if !data.charts.is_empty() {
                let shelf = search::shelf(&data.charts, shared);
                content = content.push(section(t!("explore.charts"), None, shelf));
            }
            if !data.top_songs.is_empty() {
                let see_all = data.top_playlist.as_ref().map(|id| Tile {
                    kind: TileKind::Playlist,
                    id: id.clone(),
                    title: t!("artist.top_songs").into_owned(),
                    subtitle: t!("youtube.playlist_by", owner = "YouTube Music").into_owned(),
                    image_url: None,
                });
                let list = search::track_list(&data.top_songs, Some(CHART_ROWS), shared);
                content = content.push(section(t!("artist.top_songs"), see_all, list));
            }
            if !data.trending.is_empty() {
                let list = search::track_list(&data.trending, Some(CHART_ROWS), shared);
                content = content.push(section(t!("explore.trending"), None, list));
            }
            if !data.top_artists.is_empty() {
                let shelf = search::shelf(&data.top_artists, shared);
                content = content.push(section(t!("explore.top_artists"), None, shelf));
            }
            if !data.new_videos.is_empty() {
                let list = search::track_list(&data.new_videos, Some(CHART_ROWS), shared);
                content = content.push(section(t!("explore.new_videos"), None, list));
            }
            if !data.genres.is_empty() {
                content = content.push(section(t!("explore.genres"), None, chips(&data.genres)));
            }
        }
        _ => content = content.push(padded(note(t!("search.loading")))),
    }
    scrollable(content.padding([14, 0])).height(Fill).into()
}

/// A mood's or genre's page.
pub fn genre_view<'a>(explore: &'a Explore, shared: Shared<'a>) -> Element<'a, Message> {
    let name = explore.genre.as_ref().map_or("", |page| page.name.as_str());
    let mut content = column![heading(name)].spacing(4);
    match (&explore.genre_status, &explore.genre) {
        (Status::Failed(error), _) => {
            content = content.push(padded(note(t!("collection.open_failed", error = error))));
        }
        (Status::Done, Some(page)) if page.sections.is_empty() => {
            content = content.push(padded(note(t!("explore.no_playlists"))));
        }
        (Status::Done, Some(page)) => {
            for (title, tiles) in &page.sections {
                content = content.push(section(title, None, search::shelf(tiles, shared)));
            }
        }
        _ => content = content.push(padded(note(t!("search.loading")))),
    }
    scrollable(content.padding([14, 0])).height(Fill).into()
}

/// A button for each mood or genre, with its color on the left, wrapping into rows.
fn chips(genres: &[Genre]) -> Element<'_, Message> {
    let chips = genres.iter().map(|genre| {
        let [_, red, green, blue] = genre.color.to_be_bytes();
        let color = Color::from_rgb8(red, green, blue);
        let stripe = container(space())
            .width(5)
            .height(22)
            .style(move |_| container::background(color).border(border::rounded(3)));
        button(
            row![stripe, text(&genre.name).size(14)]
                .spacing(10)
                .align_y(Center),
        )
        .on_press(Message::Explore(Event::OpenGenre(genre.clone())))
        .width(180)
        .padding([10, 12])
        .style(|_, status| {
            let hovered = status == button::Status::Hovered;
            let mut style = theme::flat_button(status, false);
            style.background = Some(theme::tint(if hovered { 0.12 } else { 0.06 }).into());
            style.border = border::rounded(8);
            style
        })
        .into()
    });
    Row::with_children(chips)
        .spacing(10)
        .wrap()
        .vertical_spacing(10)
        .into()
}

fn heading<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    padded(text(label).size(26).font(Font {
        weight: Weight::Bold,
        ..theme::INTER
    }))
}

fn padded<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content).padding([14, 32]).into()
}

fn note<'a>(message: impl text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    text(message).color(theme::text_secondary())
}
