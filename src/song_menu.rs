//! The menu a right click on a song opens, in song lists and in the queue: play it next, add it
//! to the queue, start a radio from it, add it to a playlist or take it out of one, out of the
//! queue or out of the downloads, go to its artist or album, like or dislike it, copy its link.
//! Each choice shows only where it applies.

use iced::widget::{Column, button, column, container, mouse_area, opaque, rule, space, text};
use iced::{Element, Fill, Padding, Point, Size, Task, border};
use rust_i18n::t;

use crate::rating::{self, Rating};
use crate::youtube::{self, SearchTrack};
use crate::{App, Message, Screen, theme};

/// How wide the menu is.
const WIDTH: f32 = 250.0;
/// How tall each choice is, and each line between groups.
const ITEM_HEIGHT: f32 = 34.0;
const RULE_HEIGHT: f32 = 9.0;

/// Where the song was right-clicked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Place {
    /// A song list; `playlist` is the user's own playlist it is in, if it is one.
    List { playlist: Option<String> },
    /// Song `index` of the queue; `upcoming` unless it is the one playing.
    Queue { index: usize, upcoming: bool },
}

/// What the menu can do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    PlayNext,
    AddToQueue,
    StartRadio,
    AddToPlaylist,
    RemoveFromPlaylist(String),
    RemoveFromQueue(usize),
    /// Takes the song out of the downloaded collection with this id.
    RemoveFromDownloads(String),
    GoToArtist,
    GoToAlbum,
    Like,
    Dislike,
    CopyLink,
}

#[derive(Debug, Clone)]
pub enum Event {
    /// A song was right-clicked at `at` (window coordinates).
    Open {
        track: SearchTrack,
        place: Place,
        at: Point,
    },
    Close,
    Pick(Action),
}

/// An open menu.
#[derive(Debug, Clone)]
pub struct Menu {
    track: SearchTrack,
    place: Place,
    at: Point,
    /// The downloaded collection on screen holding the song, if any.
    downloaded_in: Option<String>,
    rating: Rating,
    signed_in: bool,
}

impl Menu {
    /// The choices, in groups.
    fn actions(&self) -> Vec<Vec<(Action, String)>> {
        let mut play = Vec::new();
        play.push((Action::PlayNext, t!("song_menu.play_next").into_owned()));
        if !matches!(self.place, Place::Queue { .. }) {
            play.push((
                Action::AddToQueue,
                t!("song_menu.add_to_queue").into_owned(),
            ));
        }
        play.push((Action::StartRadio, t!("song_menu.start_radio").into_owned()));

        let mut lists = Vec::new();
        if self.signed_in {
            lists.push((
                Action::AddToPlaylist,
                t!("song_menu.add_to_playlist").into_owned(),
            ));
        }
        match &self.place {
            Place::List {
                playlist: Some(playlist),
            } => lists.push((
                Action::RemoveFromPlaylist(playlist.clone()),
                t!("song_menu.remove_from_playlist").into_owned(),
            )),
            Place::Queue {
                index,
                upcoming: true,
            } => lists.push((
                Action::RemoveFromQueue(*index),
                t!("song_menu.remove_from_queue").into_owned(),
            )),
            _ => {}
        }
        if let Some(collection) = &self.downloaded_in {
            lists.push((
                Action::RemoveFromDownloads(collection.clone()),
                t!("song_menu.remove_from_downloads").into_owned(),
            ));
        }

        let mut go = vec![(
            Action::GoToArtist,
            t!("song_menu.go_to_artist").into_owned(),
        )];
        if !self.track.album.is_empty() {
            go.push((Action::GoToAlbum, t!("song_menu.go_to_album").into_owned()));
        }

        let mut more = Vec::new();
        if self.signed_in {
            let like = if self.rating == Rating::Like {
                t!("song_menu.unlike")
            } else {
                t!("song_menu.like")
            };
            let dislike = if self.rating == Rating::Dislike {
                t!("song_menu.undislike")
            } else {
                t!("song_menu.dislike")
            };
            more.push((Action::Like, like.into_owned()));
            more.push((Action::Dislike, dislike.into_owned()));
        }
        more.push((Action::CopyLink, t!("song_menu.copy_link").into_owned()));

        [play, lists, go, more]
            .into_iter()
            .filter(|group| !group.is_empty())
            .collect()
    }
}

/// The open menu over everything, at the pointer and kept inside the window. Clicking
/// anywhere else closes it.
pub fn view(menu: &Menu, window: Size) -> Element<'_, Message> {
    let groups = menu.actions();
    let items: usize = groups.iter().map(Vec::len).sum();
    let height = items as f32 * ITEM_HEIGHT + (groups.len() - 1) as f32 * RULE_HEIGHT + 16.0;
    let x = if menu.at.x + WIDTH > window.width {
        (menu.at.x - WIDTH).max(0.0)
    } else {
        menu.at.x
    };
    let y = if menu.at.y + height > window.height {
        (menu.at.y - height).max(0.0)
    } else {
        menu.at.y
    };

    let mut list = Column::new().push(
        text(&menu.track.title)
            .size(12)
            .color(theme::text_secondary())
            .wrapping(text::Wrapping::None),
    );
    for (number, group) in groups.into_iter().enumerate() {
        if number > 0 {
            list = list.push(container(rule::horizontal(1)).padding([4, 0]));
        }
        for (action, label) in group {
            list = list.push(
                button(text(label).size(14))
                    .on_press(Message::SongMenu(Event::Pick(action)))
                    .width(Fill)
                    .height(ITEM_HEIGHT)
                    .padding([8, 10])
                    .style(|_, status| theme::flat_button(status, false)),
            );
        }
    }
    let card = container(list.width(WIDTH).clip(true))
        .padding(8)
        .style(|_| {
            theme::panel(theme::hero()).border(border::rounded(10).color(theme::line()).width(1))
        });

    let placed = container(opaque(card)).padding(Padding {
        top: y,
        left: x,
        ..Padding::ZERO
    });
    let close = Message::SongMenu(Event::Close);
    opaque(
        mouse_area(column![placed, space::vertical()].width(Fill).height(Fill))
            .on_press(close.clone())
            .on_right_press(close),
    )
}

impl App {
    pub(crate) fn song_menu_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Open { track, place, at } => {
                let downloaded_in = self.downloaded_collection_with(&track.id, &place);
                self.song_menu = Some(Menu {
                    rating: self.rating_of(&track.id),
                    signed_in: matches!(self.account, crate::library::Account::SignedIn(_)),
                    track,
                    place,
                    at,
                    downloaded_in,
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

    /// The downloaded collection on screen that holds song `id`, when the menu opened on it.
    fn downloaded_collection_with(&self, id: &str, place: &Place) -> Option<String> {
        if self.screen != Screen::Collection || !matches!(place, Place::List { .. }) {
            return None;
        }
        let shown = self.online.collection().0?;
        self.offline
            .get(&shown.id)
            .filter(|downloaded| downloaded.tracks.iter().any(|track| track.id == id))
            .map(|downloaded| downloaded.id.clone())
    }

    fn do_menu_action(&mut self, menu: Menu, action: Action) -> Task<Message> {
        let track = menu.track;
        match action {
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
                if let Some(updated) = self.offline.get(&collection).cloned() {
                    return self.online.show_collection(updated);
                }
                Task::none()
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
            Action::CopyLink => {
                let link = format!("https://music.youtube.com/watch?v={}", track.id);
                Task::batch([
                    iced::clipboard::write(link),
                    self.show_toast(t!("song_menu.link_copied").into_owned()),
                ])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(album: &str) -> SearchTrack {
        SearchTrack {
            id: "abcdefghijk".to_string(),
            title: "Usseewa".to_string(),
            artist: "Ado".to_string(),
            album: album.to_string(),
            duration: 203,
            cover_url: None,
            entry: None,
        }
    }

    fn menu(place: Place, signed_in: bool, album: &str) -> Vec<Action> {
        Menu {
            track: song(album),
            place,
            at: Point::ORIGIN,
            downloaded_in: None,
            rating: Rating::None,
            signed_in,
        }
        .actions()
        .into_iter()
        .flatten()
        .map(|(action, _)| action)
        .collect()
    }

    #[test]
    fn each_place_offers_what_applies_there() {
        let own = menu(
            Place::List {
                playlist: Some("PL1".to_string()),
            },
            true,
            "Kyougen",
        );
        assert!(own.contains(&Action::RemoveFromPlaylist("PL1".to_string())));
        assert!(own.contains(&Action::AddToQueue) && own.contains(&Action::GoToAlbum));
        assert!(own.contains(&Action::Like) && own.contains(&Action::AddToPlaylist));

        let queued = menu(
            Place::Queue {
                index: 4,
                upcoming: true,
            },
            true,
            "",
        );
        assert!(queued.contains(&Action::RemoveFromQueue(4)));
        assert!(
            !queued.contains(&Action::AddToQueue),
            "it is already queued"
        );
        assert!(!queued.contains(&Action::GoToAlbum), "no album to go to");

        let playing = menu(
            Place::Queue {
                index: 0,
                upcoming: false,
            },
            false,
            "",
        );
        assert!(!playing.contains(&Action::RemoveFromQueue(0)));
        assert!(
            !playing.contains(&Action::Like) && !playing.contains(&Action::AddToPlaylist),
            "rating and playlists need the account"
        );
        assert!(playing.contains(&Action::CopyLink));
    }

    #[test]
    fn a_right_click_opens_the_menu_and_a_choice_closes_it() {
        let mut app = App::new();
        let _ = app.handle(Message::SongMenu(Event::Open {
            track: song("Kyougen"),
            place: Place::List { playlist: None },
            at: Point::new(10.0, 20.0),
        }));
        assert!(app.song_menu.is_some());
        let _ = app.handle(Message::SongMenu(Event::Pick(Action::AddToQueue)));
        assert!(app.song_menu.is_none());
        // Nothing was queued, so the song is the queue now.
        assert_eq!(
            app.online.queue().get(0).map(|track| track.id.as_str()),
            Some("abcdefghijk")
        );
    }
}
