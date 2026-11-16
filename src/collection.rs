//! An opened album, playlist or artist: a header with its image and a Play button, then its songs.

use iced::font::Weight;
use iced::widget::{Row, button, column, row, scrollable, space, text};
use iced::{Center, Element, Fill, Font, border};
use rust_i18n::t;

use crate::offline::{Offline, State};
use crate::search::{self, Shared, Status};
use crate::song_order::{self, SongOrder};
use crate::virtual_list::Scroll;
use crate::youtube::{Collection, SearchTrack, TileKind};
use crate::{Message, playlists, theme};

/// The scrollable of the collection screen, scrolled back to the top when another opens.
pub const SCROLL_ID: &str = "collection";
/// Where the song list starts in the scrollable: the padding, the header, the search and order
/// bar and the space below each. Only a guide for which rows to build; a few pixels off are
/// covered by the overscan.
const LIST_TOP: f32 = 28.0 + 180.0 + 24.0 + 38.0 + 24.0;

/// The collection screen, or its loading or error state. In the user's `own` playlist, songs
/// can be taken out.
pub fn view<'a>(
    collection: Option<&'a Collection>,
    status: &'a Status,
    shared: Shared<'a>,
    offline: &'a Offline,
    own: bool,
    scroll: Scroll,
    songs: &'a SongOrder,
) -> Element<'a, Message> {
    if let (Status::Done, Some(collection)) = (status, collection)
        && let Some(page) = &collection.artist
    {
        return crate::artist::view(collection, page, shared, offline);
    }
    let content = match (status, collection) {
        (Status::Failed(error), _) => column![note(t!("collection.open_failed", error = error))],
        (_, Some(collection)) => loaded(collection, shared, offline, own, scroll, songs),
        _ => column![note(t!("search.loading"))],
    };

    scrollable(content.spacing(24).padding([28, 32]))
        .id(SCROLL_ID)
        .on_scroll(|viewport| {
            Message::CollectionScrolled(Scroll {
                offset: viewport.absolute_offset().y,
                height: viewport.bounds().height,
            })
        })
        .height(Fill)
        .into()
}

fn loaded<'a>(
    collection: &'a Collection,
    shared: Shared<'a>,
    offline: &'a Offline,
    own: bool,
    scroll: Scroll,
    songs: &'a SongOrder,
) -> iced::widget::Column<'a, Message> {
    let round = collection.kind == TileKind::Artist;
    let kind = match collection.kind {
        TileKind::Album => t!("collection.kind_album"),
        TileKind::Artist => t!("collection.kind_artist"),
        TileKind::Playlist => t!("collection.kind_playlist"),
        TileKind::Radio => t!("collection.kind_mix"),
    };

    let details = column![
        text(kind).size(11).color(theme::text_secondary()),
        text(&collection.title).size(34).font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        }),
        text(&collection.subtitle)
            .size(13)
            .color(theme::text_secondary()),
        text(t!("search.song_count", count = collection.tracks.len()))
            .size(13)
            .color(theme::text_secondary()),
        space().height(8),
        play_buttons(&collection.tracks, songs.order())
            .push(download_button(collection, offline))
            .push((collection.kind == TileKind::Playlist).then(|| cover_button(collection))),
    ]
    .push(
        offline
            .error()
            .map(|error| text(error).size(12).color(theme::accent())),
    )
    .spacing(4);

    let header = row![
        search::cover_picture(
            &collection.id,
            collection.image_url.as_deref(),
            180.0,
            round,
            shared
        ),
        details
    ]
    .spacing(24)
    .align_y(Center);

    if collection.tracks.is_empty() {
        return column![header, note(t!("collection.no_songs"))].width(Fill);
    }
    let total = collection.tracks.len();
    let list: iced::Element<'a, Message> = if songs.shown(total) == 0 {
        note(t!("songs.no_match"))
    } else {
        let rows = rows_in_sight(scroll, songs.shown(total));
        let own = own.then_some(collection.id.as_str());
        search::long_track_list(&collection.tracks, songs.order(), own, rows, shared)
    };

    column![header, song_order::toolbar(songs, total), list].width(Fill)
}

/// The songs of the collection in sight when scrolled to `scroll`.
pub fn rows_in_sight(scroll: Scroll, count: usize) -> std::ops::Range<usize> {
    search::song_rows_in_sight(scroll, LIST_TOP, count)
}

fn note<'a>(message: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(message).color(theme::text_secondary()).into()
}

/// Opens the dialog to give the playlist a cover of the user's own.
fn cover_button(collection: &Collection) -> Element<'_, Message> {
    button(text(t!("collection.cover")).size(14))
        .on_press(Message::Playlist(playlists::Event::OpenCover {
            id: crate::playlists::edit::bare_id(&collection.id).to_string(),
            title: collection.title.clone(),
        }))
        .padding([8, 18])
        .style(|_, status| {
            let mut style = theme::flat_button(status, false);
            style.border = border::rounded(18).color(theme::line()).width(1);
            style
        })
        .into()
}

/// Download, the progress while downloading, or Remove download once it is on disk.
/// `button` sending what `message` makes when pressed, or disabled without it. The message is
/// made only on a press: those of a collection copy all of its songs.
fn pressable<'a>(
    button: button::Button<'a, Message>,
    message: Option<impl Fn() -> Message + 'a>,
) -> button::Button<'a, Message> {
    match message {
        Some(message) => button.on_press_with(message),
        None => button,
    }
}

/// Play (from the first song, or a random one with shuffle on) and Shuffle (random order from
/// a random song, leaving shuffle on) for a list of songs. The messages copy the songs, so they
/// are made only when a button is pressed.
/// Play and Shuffle for `tracks`, as shown: in `order` (a search or another order) if given.
pub(crate) fn play_buttons(
    tracks: &[SearchTrack],
    order: Option<std::sync::Arc<[usize]>>,
) -> Row<'_, Message> {
    let has_songs = order
        .as_ref()
        .map_or(!tracks.is_empty(), |order| !order.is_empty());
    let shown = move |order: &Option<std::sync::Arc<[usize]>>| match order {
        Some(order) => search::in_order(tracks, order),
        None => tracks.to_vec(),
    };
    let shuffle_order = order.clone();
    let play = pressable(
        button(
            text(t!("search.play"))
                .size(14)
                .color(theme::text_primary()),
        ),
        has_songs.then_some(move || Message::PlayAll(shown(&order), false)),
    )
    .padding([8, 22])
    .style(|_, status| {
        let mut style = theme::flat_button(status, false);
        style.background = Some(theme::accent().into());
        style.border = border::rounded(18);
        style
    });
    let shuffle = pressable(
        button(
            row![
                theme::icon(
                    include_bytes!("../assets/icons/shuffle.svg"),
                    theme::text_primary(),
                    16.0
                ),
                text(t!("collection.shuffle")).size(14),
            ]
            .spacing(8)
            .align_y(Center),
        ),
        has_songs.then_some(move || Message::PlayAll(shown(&shuffle_order), true)),
    )
    .padding([8, 18])
    .style(|_, status| {
        let mut style = theme::flat_button(status, false);
        style.border = border::rounded(18).color(theme::line()).width(1);
        style
    });
    row![play, shuffle].spacing(10)
}

pub(crate) fn download_button<'a>(
    collection: &'a Collection,
    offline: &'a Offline,
) -> Element<'a, Message> {
    let state = offline.state(&collection.id);
    let label = match state {
        State::NotDownloaded => t!("collection.download"),
        State::Preparing => t!("collection.preparing"),
        State::Waiting => t!("collection.waiting"),
        State::Downloading { done, total } => {
            t!("collection.downloading", done = done, total = total)
        }
        State::Downloaded => t!("collection.downloaded"),
    };
    let message = match state {
        State::NotDownloaded => !collection.tracks.is_empty(),
        State::Preparing | State::Waiting | State::Downloading { .. } => false,
        State::Downloaded => true,
    }
    .then_some(move || match state {
        State::Downloaded => Message::RemoveDownload(collection.id.clone()),
        _ => Message::DownloadCollection(collection.clone()),
    });

    pressable(
        button(
            row![
                theme::icon(
                    include_bytes!("../assets/icons/download.svg"),
                    theme::text_primary(),
                    16.0
                ),
                text(label).size(14),
            ]
            .spacing(8)
            .align_y(Center),
        ),
        message,
    )
    .padding([8, 18])
    .style(|_, status| {
        let mut style = theme::flat_button(status, false);
        style.border = border::rounded(18).color(theme::line()).width(1);
        style
    })
    .into()
}
