//! An opened album, playlist or artist: a header with its image and a Play button, then its songs.

use iced::font::Weight;
use iced::widget::{button, column, row, scrollable, space, text};
use iced::{Center, Element, Fill, Font, border};

use crate::search::{self, Shared, Status};
use crate::youtube::{Collection, TileKind};
use crate::{Message, theme};

/// The collection screen, or its loading or error state.
pub fn view<'a>(
    collection: Option<&'a Collection>,
    status: &'a Status,
    shared: Shared<'a>,
) -> Element<'a, Message> {
    let content = match (status, collection) {
        (Status::Failed(error), _) => column![note(format!("Could not open it: {error}"))],
        (_, Some(collection)) => loaded(collection, shared),
        _ => column![note("Loading\u{2026}")],
    };

    scrollable(content.spacing(24).padding([28, 32]))
        .height(Fill)
        .into()
}

fn loaded<'a>(collection: &'a Collection, shared: Shared<'a>) -> iced::widget::Column<'a, Message> {
    let round = collection.kind == TileKind::Artist;
    let kind = match collection.kind {
        TileKind::Album => "ALBUM",
        TileKind::Artist => "ARTIST",
        TileKind::Playlist => "PLAYLIST",
    };

    let play = button(text("Play").size(14).color(theme::TEXT))
        .on_press_maybe(
            (!collection.tracks.is_empty())
                .then(|| Message::PlayQueue(collection.tracks.clone(), 0)),
        )
        .padding([8, 22])
        .style(|_, status| {
            let mut style = theme::flat_button(status, false);
            style.background = Some(theme::ACCENT.into());
            style.border = border::rounded(18);
            style
        });

    let details = column![
        text(kind).size(11).color(theme::TEXT_SECONDARY),
        text(&collection.title).size(34).font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        }),
        text(&collection.subtitle)
            .size(13)
            .color(theme::TEXT_SECONDARY),
        text(format!("{} songs", collection.tracks.len()))
            .size(13)
            .color(theme::TEXT_SECONDARY),
        space().height(8),
        play,
    ]
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
    text(message).color(theme::TEXT_SECONDARY).into()
}
