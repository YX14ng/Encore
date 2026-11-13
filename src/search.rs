//! The search results screen, laid out like YouTube Music: filter chips, the top result, and a
//! section per category. Also the song rows and cards shared with the collection screen.

use std::collections::HashMap;

use iced::font::Weight;
use iced::widget::{Column, Row, button, column, container, image, row, scrollable, space, text};
use iced::{Center, Element, Fill, Font, border};

use crate::youtube::{Collection, SearchPage, SearchTrack, Tile, TileKind, TopResult};
use crate::{Message, clock, theme};

/// How many songs or videos the "All" view shows before "Show all".
const PREVIEW_ROWS: usize = 4;
/// Cards per row in a full album, artist or playlist list.
const GRID_COLUMNS: usize = 5;
/// The size of a card's image.
const CARD_SIZE: f32 = 160.0;

/// Where a search (or opening a collection) is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Idle,
    Loading,
    Done,
    Failed(String),
}

/// The chips at the top of the results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Songs,
    Videos,
    Albums,
    Artists,
    Playlists,
}

const FILTERS: [(Filter, &str); 6] = [
    (Filter::All, "All"),
    (Filter::Songs, "Songs"),
    (Filter::Videos, "Videos"),
    (Filter::Albums, "Albums"),
    (Filter::Artists, "Artists"),
    (Filter::Playlists, "Playlists"),
];

/// Images and playback state that rows and cards need.
#[derive(Clone, Copy)]
pub struct Shared<'a> {
    pub images: &'a HashMap<String, image::Handle>,
    /// The song being downloaded before it can play.
    pub loading: Option<&'a str>,
    /// The song that is playing.
    pub playing: Option<&'a str>,
}

/// What the results screen draws.
pub struct Results<'a> {
    pub query: &'a str,
    pub status: &'a Status,
    pub page: &'a SearchPage,
    pub filter: Filter,
    pub error: Option<&'a str>,
    pub shared: Shared<'a>,
}

/// The results screen.
pub fn view(results: Results<'_>) -> Element<'_, Message> {
    let shared = results.shared;
    let page = results.page;

    let mut content = column![
        heading(format!("Results for \u{201c}{}\u{201d}", results.query)),
        chips(results.filter),
    ]
    .spacing(18)
    .padding([28, 32]);

    if let Some(error) = results.error {
        content = content.push(note(format!("Could not play that song: {error}")));
    }

    match results.status {
        Status::Idle => {}
        Status::Loading => content = content.push(note("Searching YouTube Music\u{2026}")),
        Status::Failed(error) => {
            content = content.push(note(format!("The search failed: {error}")))
        }
        Status::Done => {
            content = match results.filter {
                Filter::All => all_sections(content, page, shared),
                Filter::Songs => content.push(track_list(&page.songs, None, shared)),
                Filter::Videos => content.push(track_list(&page.videos, None, shared)),
                Filter::Albums => content.push(card_grid(&page.albums, shared)),
                Filter::Artists => content.push(card_grid(&page.artists, shared)),
                Filter::Playlists => content.push(card_grid(&page.playlists, shared)),
            };
        }
    }

    scrollable(content).height(Fill).into()
}

fn all_sections<'a>(
    mut content: Column<'a, Message>,
    page: &'a SearchPage,
    shared: Shared<'a>,
) -> Column<'a, Message> {
    if page.top.is_none()
        && page.songs.is_empty()
        && page.albums.is_empty()
        && page.artists.is_empty()
    {
        return content.push(note("Nothing found."));
    }
    if let Some(top) = &page.top {
        content = content.push(section("Top result", None, top_card(top, shared)));
    }
    if !page.songs.is_empty() {
        let list = track_list(&page.songs, Some(PREVIEW_ROWS), shared);
        content = content.push(section("Songs", Some(Filter::Songs), list));
    }
    if !page.albums.is_empty() {
        content = content.push(section(
            "Albums",
            Some(Filter::Albums),
            shelf(&page.albums, shared),
        ));
    }
    if !page.artists.is_empty() {
        let artists = shelf(&page.artists, shared);
        content = content.push(section("Artists", Some(Filter::Artists), artists));
    }
    if !page.videos.is_empty() {
        let list = track_list(&page.videos, Some(PREVIEW_ROWS), shared);
        content = content.push(section("Videos", Some(Filter::Videos), list));
    }
    if !page.playlists.is_empty() {
        let playlists = shelf(&page.playlists, shared);
        content = content.push(section("Playlists", Some(Filter::Playlists), playlists));
    }
    content
}

fn chips(selected: Filter) -> Element<'static, Message> {
    let chips = FILTERS.iter().map(|&(filter, label)| {
        let active = filter == selected;
        button(text(label).size(13))
            .on_press(Message::SetFilter(filter))
            .padding([6, 14])
            .style(move |_, status| {
                let mut style = theme::flat_button(status, false);
                style.border = border::rounded(16).color(theme::LINE).width(1);
                if active {
                    style.background = Some(theme::TEXT.into());
                    style.text_color = theme::CENTER;
                } else if style.background.is_none() {
                    style.background = Some(iced::Color::from_rgba8(255, 255, 255, 0.06).into());
                }
                style
            })
            .into()
    });
    Row::with_children(chips).spacing(8).into()
}

fn section<'a>(
    title: &'a str,
    show_all: Option<Filter>,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let header = row![text(title).size(20).font(bold()), space::horizontal()]
        .push(show_all.map(|filter| {
            button(text("Show all").size(13).color(theme::TEXT_SECONDARY))
                .on_press(Message::SetFilter(filter))
                .padding([4, 10])
                .style(|_, status| theme::flat_button(status, false))
        }))
        .align_y(Center);

    column![header, body].spacing(10).into()
}

fn top_card<'a>(top: &'a TopResult, shared: Shared<'a>) -> Element<'a, Message> {
    let (title, subtitle, image_url, round, open) = match top {
        TopResult::Track(track) => (
            track.title.as_str(),
            format!("Song \u{b7} {}", track.artist),
            track.cover_url.as_deref(),
            false,
            None,
        ),
        TopResult::Tile(tile) => (
            tile.title.as_str(),
            tile.subtitle.clone(),
            tile.image_url.as_deref(),
            tile.kind == TileKind::Artist,
            Some(Message::OpenTile(tile.clone())),
        ),
    };

    let play = button(text("Play").size(14).color(theme::TEXT))
        .on_press(Message::PlayTop)
        .padding([8, 22])
        .style(|_, status| {
            let mut style = theme::flat_button(status, false);
            style.background = Some(theme::ACCENT.into());
            style.border = border::rounded(18);
            style
        });

    let details = column![
        text(title).size(26).font(bold()),
        text(subtitle).size(13).color(theme::TEXT_SECONDARY),
        space().height(8),
        play,
    ]
    .spacing(4);

    let card = row![picture(image_url, 120.0, round, shared), details]
        .spacing(20)
        .align_y(Center);

    button(card)
        .on_press_maybe(open)
        .padding(16)
        .style(|_, status| {
            let mut style = theme::flat_button(status, false);
            style.background = Some(iced::Color::from_rgba8(255, 255, 255, 0.05).into());
            style.border = border::rounded(12).color(theme::LINE).width(1);
            style
        })
        .into()
}

/// A list of song rows. Clicking a row plays the list from that song.
pub fn track_list<'a>(
    tracks: &'a [SearchTrack],
    limit: Option<usize>,
    shared: Shared<'a>,
) -> Element<'a, Message> {
    let shown = limit.unwrap_or(tracks.len()).min(tracks.len());
    let rows = (0..shown).map(|index| track_row(tracks, index, shared));
    Column::with_children(rows).spacing(2).into()
}

/// One song row; clicking it plays `tracks` from `index`.
pub fn track_row<'a>(
    tracks: &'a [SearchTrack],
    index: usize,
    shared: Shared<'a>,
) -> Element<'a, Message> {
    let track = &tracks[index];
    let details = column![
        text(&track.title).size(14),
        text(subtitle(track)).size(12).color(theme::TEXT_SECONDARY),
    ]
    .spacing(2)
    .width(Fill);

    let is_loading = shared.loading == Some(track.id.as_str());
    let is_playing = shared.playing == Some(track.id.as_str());
    let right = if is_loading {
        text("Loading\u{2026}").size(12).color(theme::ACCENT)
    } else {
        // Some lists (an artist's top songs) come without durations.
        let duration = if track.duration == 0 {
            String::new()
        } else {
            clock::format_time(track.duration)
        };
        text(duration).size(12).color(theme::TEXT_SECONDARY)
    };

    let cover = picture(track.cover_url.as_deref(), 44.0, false, shared);
    button(row![cover, details, right].spacing(12).align_y(Center))
        .on_press(Message::PlayQueue(tracks.to_vec(), index))
        .width(Fill)
        .padding([6, 8])
        .style(move |_, status| theme::flat_button(status, is_playing))
        .into()
}

/// A horizontal row of cards.
pub fn shelf<'a>(tiles: &'a [Tile], shared: Shared<'a>) -> Element<'a, Message> {
    let cards = Row::with_children(tiles.iter().map(|tile| card(tile, shared))).spacing(16);
    let scrollbar = scrollable::Scrollbar::new().spacing(10);
    scrollable(cards)
        .direction(scrollable::Direction::Horizontal(scrollbar))
        .into()
}

/// Cards in rows of `GRID_COLUMNS`.
pub fn card_grid<'a>(tiles: &'a [Tile], shared: Shared<'a>) -> Element<'a, Message> {
    if tiles.is_empty() {
        return note("Nothing found.");
    }
    let rows = tiles.chunks(GRID_COLUMNS).map(|chunk| {
        Row::with_children(chunk.iter().map(|tile| card(tile, shared)))
            .spacing(16)
            .into()
    });
    Column::with_children(rows).spacing(20).into()
}

fn card<'a>(tile: &'a Tile, shared: Shared<'a>) -> Element<'a, Message> {
    let round = tile.kind == TileKind::Artist;
    card_parts(
        tile.image_url.as_deref(),
        &tile.title,
        tile.subtitle.clone(),
        round,
        Message::OpenTile(tile.clone()),
        shared,
    )
}

fn card_parts<'a>(
    image_url: Option<&str>,
    title: &'a str,
    subtitle: String,
    round: bool,
    on_press: Message,
    shared: Shared<'a>,
) -> Element<'a, Message> {
    let content = column![
        picture(image_url, CARD_SIZE, round, shared),
        space().height(6),
        text(title)
            .size(13)
            .wrapping(iced::widget::text::Wrapping::None),
        text(subtitle)
            .size(12)
            .color(theme::TEXT_SECONDARY)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .width(CARD_SIZE)
    .clip(true);

    button(content)
        .on_press(on_press)
        .padding(6)
        .style(|_, status| theme::flat_button(status, false))
        .into()
}

/// Downloaded collections as cards that open without a connection.
pub fn offline_grid<'a>(collections: &'a [Collection], shared: Shared<'a>) -> Element<'a, Message> {
    if collections.is_empty() {
        return note("Nothing downloaded yet. Open an album or playlist and press Download.");
    }
    let rows = collections.chunks(GRID_COLUMNS).map(|chunk| {
        let cards = chunk.iter().map(|collection| {
            card_parts(
                collection.image_url.as_deref(),
                &collection.title,
                format!("{} songs", collection.tracks.len()),
                collection.kind == TileKind::Artist,
                Message::OpenOffline(collection.id.clone()),
                shared,
            )
        });
        Row::with_children(cards).spacing(16).into()
    });
    Column::with_children(rows).spacing(20).into()
}

/// An image from the network, or a placeholder until it arrives. Round for artists.
pub fn picture<'a>(
    url: Option<&str>,
    size: f32,
    round: bool,
    shared: Shared<'a>,
) -> Element<'a, Message> {
    let radius = if round { size / 2.0 } else { 5.0 };
    match url.and_then(|url| shared.images.get(url)) {
        Some(handle) => image(handle.clone())
            .width(size)
            .height(size)
            .content_fit(iced::ContentFit::Cover)
            .border_radius(radius)
            .into(),
        None => container(space())
            .width(size)
            .height(size)
            .style(move |_| theme::panel(theme::HERO).border(border::rounded(radius)))
            .into(),
    }
}

fn heading<'a>(label: String) -> Element<'a, Message> {
    text(label).size(26).font(bold()).into()
}

fn note<'a>(message: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(message).color(theme::TEXT_SECONDARY).into()
}

fn bold() -> Font {
    Font {
        weight: Weight::Bold,
        ..theme::INTER
    }
}

/// "Artist · Album", or just the artist when the album is unknown.
pub fn subtitle(track: &SearchTrack) -> String {
    if track.album.is_empty() {
        track.artist.clone()
    } else {
        format!("{} \u{b7} {}", track.artist, track.album)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(album: &str) -> SearchTrack {
        SearchTrack {
            id: "id".to_string(),
            title: "Title".to_string(),
            artist: "Artist".to_string(),
            album: album.to_string(),
            duration: 200,
            cover_url: None,
        }
    }

    #[test]
    fn subtitle_includes_the_album_when_known() {
        assert_eq!(subtitle(&track("Album")), "Artist \u{b7} Album");
        assert_eq!(subtitle(&track("")), "Artist");
    }
}
