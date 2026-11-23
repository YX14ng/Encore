//! What the song menu does: opening it (finding the playlist and the download that hold the
//! song, also for a song in the queue) and carrying out the choice.

use iced::Task;
use rust_i18n::t;

use super::{Action, Event, Menu, Place};
use crate::rating::{self, Rating};
use crate::youtube::{self, SearchTrack};
use crate::{App, Message, Screen};

impl App {
    pub(crate) fn song_menu_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Open { track, place, at } => {
                self.song_menu = Some(Menu {
                    rating: self.rating_of(&track.id),
                    signed_in: matches!(self.account, crate::library::Account::SignedIn(_)),
                    playlist: self.own_playlist_with(&track.id, &place),
                    downloaded_in: self.downloaded_collection_with(&track.id, &place),
                    track,
                    place,
                    at,
                });
                Task::none()
            }
            Event::Close => {
                self.song_menu = None;
                Task::none()
            }
            Event::Pick(action) => match self.song_menu.take() {
                Some(menu) => self.do_menu_action(menu, action),
                None => Task::none(),
            },
        }
    }

    /// The collection song `id` was right-clicked in: the one on screen for a list, the one
    /// the queue came from for a song in the queue.
    fn collection_of(&self, place: &Place) -> Option<String> {
        match place {
            Place::List { .. } if self.screen == Screen::Collection => self
                .online
                .collection()
                .0
                .map(|collection| collection.id.clone()),
            Place::List { .. } => None,
            Place::Queue { .. } => self.online.queue().source().map(str::to_string),
        }
    }

    /// The user's own playlist holding song `id`, so it can be taken out of it.
    fn own_playlist_with(&self, id: &str, place: &Place) -> Option<String> {
        if let Place::List { playlist } = place {
            return playlist.clone();
        }
        let source = self.collection_of(place)?;
        if !self.is_own_playlist(&source) {
            return None;
        }
        // Only a song known to be in it: the queue also holds songs added by hand and
        // autoplay's.
        let holds = |tracks: &[SearchTrack]| tracks.iter().any(|track| track.id == id);
        let known = self.online.known_tracks(&source).is_some_and(holds)
            || self
                .offline
                .get(&source)
                .is_some_and(|downloaded| holds(&downloaded.tracks));
        known.then_some(source)
    }

    /// The downloaded collection holding song `id`, where the menu opened on it.
    fn downloaded_collection_with(&self, id: &str, place: &Place) -> Option<String> {
        let source = self.collection_of(place)?;
        self.offline
            .get(&source)
            .filter(|downloaded| downloaded.tracks.iter().any(|track| track.id == id))
            .map(|downloaded| downloaded.id.clone())
    }

    fn do_menu_action(&mut self, menu: Menu, action: Action) -> Task<Message> {
        let track = menu.track;
        match action {
            Action::PlayNow(index) => self.handle(Message::QueueJump(index)),
            Action::PlayNext => match menu.place {
                Place::Queue { index, .. } => self.handle(Message::QueuePlayNext(index)),
                Place::List { .. } => self.online.play_next(track),
            },
            Action::AddToQueue => self.handle(Message::AddToQueue(track)),
            Action::StartRadio => match self.online.client() {
                Some(client) => {
                    let seed = track.id.clone();
                    Task::perform(youtube::radio(client, seed), move |songs| {
                        Message::RadioReady(track.clone(), songs)
                    })
                }
                None => Task::none(),
            },
            Action::AddToPlaylist => {
                self.handle(Message::Playlist(crate::playlists::Event::OpenAddTo(track)))
            }
            Action::RemoveFromPlaylist(id) => {
                self.handle(Message::Playlist(crate::playlists::Event::Remove {
                    id,
                    song_id: track.id,
                    entry: track.entry,
                }))
            }
            Action::RemoveFromQueue(index) => self.handle(Message::QueueRemove(index)),
            Action::RemoveFromDownloads(collection) => {
                self.offline.remove_song(&collection, &track.id);
                // The list on screen follows, if it is that download.
                let on_screen = self.screen == Screen::Collection
                    && self
                        .online
                        .collection()
                        .0
                        .is_some_and(|shown| shown.id == collection);
                match self.offline.get(&collection).cloned() {
                    Some(updated) if on_screen => self.online.show_collection(updated),
                    _ => Task::none(),
                }
            }
            Action::GoToArtist | Action::GoToAlbum => {
                let Some(client) = self.online.client() else {
                    return Task::none();
                };
                let place = if action == Action::GoToArtist {
                    youtube::SongPlace::Artist(None)
                } else {
                    youtube::SongPlace::Album
                };
                Task::perform(
                    youtube::place_of_song(client, track.id, place),
                    Message::SongPlaceFound,
                )
            }
            Action::Like => self.handle(Message::Rating(rating::Event::RateSong(
                track,
                Rating::Like,
            ))),
            Action::Dislike => self.handle(Message::Rating(rating::Event::RateSong(
                track,
                Rating::Dislike,
            ))),
            Action::CopyLink => Task::batch([
                iced::clipboard::write(link(&track)),
                self.show_toast(t!("song_menu.link_copied").into_owned()),
            ]),
            Action::CopyTitle => Task::batch([
                iced::clipboard::write(title_and_artist(&track)),
                self.show_toast(t!("song_menu.title_copied").into_owned()),
            ]),
            Action::OpenInBrowser => match crate::platform::open_url(&link(&track)) {
                Ok(()) => Task::none(),
                Err(error) => self.show_toast(error.to_string()),
            },
        }
    }
}

/// The song on YouTube Music.
fn link(track: &SearchTrack) -> String {
    format!("https://music.youtube.com/watch?v={}", track.id)
}

/// "Title - Artist", or the title alone.
pub(super) fn title_and_artist(track: &SearchTrack) -> String {
    if track.artist.is_empty() {
        track.title.clone()
    } else {
        format!("{} - {}", track.title, track.artist)
    }
}
