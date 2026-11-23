//! The line under the song playing ("Artist, Artist · Album") with each artist's name a link:
//! clicking it opens the artist's page, like YouTube Music's player.

use iced::widget::text::{Rich, Span, Wrapping};
use iced::widget::{rich_text, span};
use iced::{Color, Task};

use crate::song::Song;
use crate::youtube::{self, SongPlace};
use crate::{App, Message};

/// How the artists' names are joined in a song's artist line (see `youtube::artist_names`).
const SEPARATOR: &str = ", ";

/// `song`'s artists and album, each artist a link to their page.
pub fn byline(song: &Song, size: f32, color: Color) -> Rich<'_, String, Message> {
    rich_text(spans(&song.artist, &song.album))
        .size(size)
        .color(color)
        .wrapping(Wrapping::None)
        .on_link_click(Message::GoToArtist)
}

fn spans<'a>(artist: &'a str, album: &'a str) -> Vec<Span<'a, String>> {
    let mut spans = Vec::new();
    for (index, name) in artist
        .split(SEPARATOR)
        .filter(|name| !name.is_empty())
        .enumerate()
    {
        if index > 0 {
            spans.push(span(SEPARATOR));
        }
        spans.push(span(name).link(name.to_string()));
    }
    if !album.is_empty() {
        if !spans.is_empty() {
            spans.push(span(" \u{b7} "));
        }
        spans.push(span(album));
    }
    spans
}

impl App {
    /// An artist's name was clicked under the song playing: their page opens in the main view,
    /// leaving the now playing page and the immersive view.
    pub(crate) fn go_to_artist(&mut self, name: String) -> Task<Message> {
        let Some(client) = self.online.client() else {
            return Task::none();
        };
        let Some(track) = self.online.playing_track() else {
            return Task::none();
        };
        let find = Task::perform(
            youtube::place_of_song(client, track.id.clone(), SongPlace::Artist(Some(name))),
            Message::SongPlaceFound,
        );
        let leave_page = if self.page.is_some() {
            self.close_page()
        } else {
            Task::none()
        };
        let leave_immersive = if self.immersive {
            self.set_immersive(false)
        } else {
            Task::none()
        };
        Task::batch([leave_page, leave_immersive, find])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(spans: &[Span<'_, String>]) -> Vec<(String, Option<String>)> {
        spans
            .iter()
            .map(|span| (span.text.to_string(), span.link.clone()))
            .collect()
    }

    #[test]
    fn each_artist_is_a_link_and_the_album_is_not() {
        let two = spans("Ado, Vaundy", "Kyougen");
        assert_eq!(
            shown(&two),
            [
                ("Ado".to_string(), Some("Ado".to_string())),
                (", ".to_string(), None),
                ("Vaundy".to_string(), Some("Vaundy".to_string())),
                (" \u{b7} ".to_string(), None),
                ("Kyougen".to_string(), None),
            ]
        );
        assert_eq!(shown(&spans("", "Album")), [("Album".to_string(), None)]);
        assert_eq!(
            shown(&spans("cattpur", "")),
            [("cattpur".to_string(), Some("cattpur".to_string()))]
        );
    }
}
