//! The search results screen.

use std::collections::HashMap;

use iced::font::Weight;
use iced::widget::{Column, button, column, container, image, row, scrollable, space, text};
use iced::{Center, Element, Fill, Font};

use crate::clock;
use crate::youtube::SearchTrack;
use crate::{Message, theme};

/// Where a search is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchStatus {
    Idle,
    Searching,
    Done,
    Failed(String),
}

/// What the results screen needs from the app.
pub struct Results<'a> {
    pub query: &'a str,
    pub status: &'a SearchStatus,
    pub tracks: &'a [SearchTrack],
    pub covers: &'a HashMap<String, image::Handle>,
    /// The track being downloaded before it can play.
    pub loading: Option<&'a str>,
    /// The track that is playing.
    pub playing: Option<&'a str>,
    /// The last playback error, if any.
    pub error: Option<&'a str>,
}

/// The heading, the search state and one clickable row per track.
pub fn view<'a>(results: Results<'a>) -> Element<'a, Message> {
    let heading = text(format!("Results for \u{201c}{}\u{201d}", results.query))
        .size(26)
        .font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        });

    let status: Option<Element<'a, Message>> = match results.status {
        SearchStatus::Idle => None,
        SearchStatus::Searching => Some(note("Searching YouTube Music…")),
        SearchStatus::Done if results.tracks.is_empty() => Some(note("No songs found.")),
        SearchStatus::Done => None,
        SearchStatus::Failed(error) => Some(note(format!("The search failed: {error}"))),
    };
    let error = results
        .error
        .map(|error| note(format!("Could not play that song: {error}")));

    let rows = results
        .tracks
        .iter()
        .enumerate()
        .map(|(index, track)| track_row(index, track, &results));

    let content = column![heading]
        .push(status)
        .push(error)
        .push(Column::with_children(rows).spacing(2))
        .spacing(16)
        .padding([28, 32]);

    scrollable(content).height(Fill).into()
}

fn note<'a>(message: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(message).color(theme::TEXT_SECONDARY).into()
}

fn track_row<'a>(
    index: usize,
    track: &'a SearchTrack,
    results: &Results<'a>,
) -> Element<'a, Message> {
    let cover: Element<'a, Message> = match results.covers.get(&track.id) {
        Some(handle) => image(handle.clone())
            .width(44)
            .height(44)
            .border_radius(5)
            .into(),
        None => container(space())
            .width(44)
            .height(44)
            .style(|_| theme::panel(theme::HERO))
            .into(),
    };

    let details = column![
        text(&track.title).size(14),
        text(subtitle(track)).size(12).color(theme::TEXT_SECONDARY),
    ]
    .spacing(2)
    .width(Fill);

    let is_loading = results.loading == Some(track.id.as_str());
    let is_playing = results.playing == Some(track.id.as_str());
    let right = if is_loading {
        text("Loading…").size(12).color(theme::ACCENT)
    } else {
        text(clock::format_time(track.duration))
            .size(12)
            .color(theme::TEXT_SECONDARY)
    };

    button(row![cover, details, right].spacing(12).align_y(Center))
        .on_press(Message::Play(index))
        .width(Fill)
        .padding([6, 8])
        .style(move |_, status| theme::flat_button(status, is_playing))
        .into()
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
