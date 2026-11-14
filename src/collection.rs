//! An opened album, playlist or artist: a header with its image and a Play button, then its songs.

use iced::font::Weight;
use iced::widget::{button, column, row, scrollable, space, text};
use iced::{Center, Element, Fill, Font, border};

use crate::offline::{Offline, State};
use crate::search::{self, Shared, Status};
use crate::youtube::{Collection, TileKind};
use crate::{Message, theme};

/// The collection screen, or its loading or error state.
pub fn view<'a>(
    collection: Option<&'a Collection>,
    status: &'a Status,
    shared: Shared<'a>,
    offline: &'a Offline,
) -> Element<'a, Message> {
    let content = match (status, collection) {
        (Status::Failed(error), _) => column![note(format!("Could not open it: {error}"))],
        (_, Some(collection)) => loaded(collection, shared, offline),
        _ => column![note("Loading\u{2026}")],
    };

    scrollable(content.spacing(24).padding([28, 32]))
        .height(Fill)
        .into()
}

fn loaded<'a>(
    collection: &'a Collection,
    shared: Shared<'a>,
    offline: &'a Offline,
) -> iced::widget::Column<'a, Message> {
    let round = collection.kind == TileKind::Artist;
    let kind = match collection.kind {
        TileKind::Album => "ALBUM",
        TileKind::Artist => "ARTIST",
        TileKind::Playlist => "PLAYLIST",
        TileKind::Radio => "MIX",
    };

    let play = button(text("Play").size(14).color(theme::text_primary()))
        .on_press_maybe(
            (!collection.tracks.is_empty())
                .then(|| Message::PlayQueue(collection.tracks.clone(), 0)),
        )
        .padding([8, 22])
        .style(|_, status| {
            let mut style = theme::flat_button(status, false);
            style.background = Some(theme::accent().into());
            style.border = border::rounded(18);
            style
        });

    let details = column![
        text(kind).size(11).color(theme::text_secondary()),
        text(&collection.title).size(34).font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        }),
        text(&collection.subtitle)
            .size(13)
            .color(theme::text_secondary()),
        text(format!("{} songs", collection.tracks.len()))
            .size(13)
            .color(theme::text_secondary()),
        space().height(8),
        row![play, download_button(collection, offline)].spacing(10),
    ]
    .push(
        offline
            .error()
            .map(|error| text(error).size(12).color(theme::accent())),
    )
    .spacing(4);

    let header = row![
        search::picture(collection.image_url.as_deref(), 180.0, round, shared),
        details
    ]
    .spacing(24)
    .align_y(Center);

    let songs: iced::Element<'a, Message> = if collection.tracks.is_empty() {
        note("No songs here.")
    } else {
        search::track_list(&collection.tracks, None, shared)
    };

    column![header, songs].width(Fill)
}

fn note<'a>(message: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(message).color(theme::text_secondary()).into()
}

/// Download, the progress while downloading, or Remove download once it is on disk.
fn download_button<'a>(collection: &'a Collection, offline: &'a Offline) -> Element<'a, Message> {
    let (label, message) = match offline.state(&collection.id) {
        State::NotDownloaded => (
            "Download".to_string(),
            (!collection.tracks.is_empty())
                .then(|| Message::DownloadCollection(collection.clone())),
        ),
        State::Downloading { done, total } => (format!("Downloading {done}/{total}\u{2026}"), None),
        State::Downloaded => (
            "Downloaded \u{b7} Remove".to_string(),
            Some(Message::RemoveDownload(collection.id.clone())),
        ),
    };

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
    )
    .on_press_maybe(message)
    .padding([8, 18])
    .style(|_, status| {
        let mut style = theme::flat_button(status, false);
        style.border = border::rounded(18).color(theme::line()).width(1);
        style
    })
    .into()
}
