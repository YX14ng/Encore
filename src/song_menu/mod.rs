//! The menu a right click on a song opens, in song lists and in the queue: play it next, add it
//! to the queue, start a radio from it, add it to a playlist or take it out of one, out of the
//! queue or out of the downloads, go to its artist or album, like or dislike it, copy its link
//! or its title, open it on YouTube Music. Each choice shows only where it applies; a song in
//! the queue also offers what the playlist or download it came from allows.

use iced::widget::{Column, button, column, container, mouse_area, opaque, rule, space, text};
use iced::{Element, Fill, Padding, Point, Size, border};
use rust_i18n::t;

use crate::rating::Rating;
use crate::youtube::SearchTrack;
use crate::{Message, theme};

mod act;

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
    /// Plays song `index` of the queue now.
    PlayNow(usize),
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
    /// Copies "Title - Artist".
    CopyTitle,
    /// Opens the song on YouTube Music in the browser.
    OpenInBrowser,
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
    /// The user's own playlist holding the song (the one on screen, or the one the queue
    /// came from), if any.
    playlist: Option<String>,
    /// The downloaded collection holding the song (on screen, or the queue's), if any.
    downloaded_in: Option<String>,
    rating: Rating,
    signed_in: bool,
}

impl Menu {
    /// The choices, in groups.
    fn actions(&self) -> Vec<Vec<(Action, String)>> {
        let mut play = Vec::new();
        if let Place::Queue {
            index,
            upcoming: true,
        } = self.place
        {
            play.push((
                Action::PlayNow(index),
                t!("song_menu.play_now").into_owned(),
            ));
        }
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
        if let Some(playlist) = &self.playlist {
            lists.push((
                Action::RemoveFromPlaylist(playlist.clone()),
                t!("song_menu.remove_from_playlist").into_owned(),
            ));
        }
        if let Place::Queue {
            index,
            upcoming: true,
        } = self.place
        {
            lists.push((
                Action::RemoveFromQueue(index),
                t!("song_menu.remove_from_queue").into_owned(),
            ));
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
        more.push((Action::CopyTitle, t!("song_menu.copy_title").into_owned()));
        more.push((
            Action::OpenInBrowser,
            t!("song_menu.open_in_browser").into_owned(),
        ));

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::App;

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
        let playlist = match &place {
            Place::List { playlist } => playlist.clone(),
            Place::Queue { .. } => None,
        };
        Menu {
            track: song(album),
            place,
            at: Point::ORIGIN,
            playlist,
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
        assert!(queued.contains(&Action::PlayNow(4)));
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
        assert!(!playing.contains(&Action::PlayNow(0)), "it is playing");
        assert!(
            !playing.contains(&Action::Like) && !playing.contains(&Action::AddToPlaylist),
            "rating and playlists need the account"
        );
        assert!(playing.contains(&Action::CopyLink) && playing.contains(&Action::CopyTitle));
        assert!(playing.contains(&Action::OpenInBrowser));
    }

    #[test]
    fn the_title_is_copied_with_the_artist() {
        assert_eq!(act::title_and_artist(&song("")), "Usseewa - Ado");
        let mut alone = song("");
        alone.artist.clear();
        assert_eq!(act::title_and_artist(&alone), "Usseewa");
    }

    #[test]
    fn a_song_in_the_queue_can_leave_the_users_playlist_it_came_from() {
        let mut app = App::new();
        let mut data = crate::library::LibraryData::default();
        data.own_playlists.push("PLmine".to_string());
        app.account = crate::library::Account::SignedIn(Box::new(data));
        let playlist = crate::youtube::Collection {
            id: "PLmine".to_string(),
            kind: crate::youtube::TileKind::Playlist,
            title: "Mine".to_string(),
            subtitle: String::new(),
            image_url: None,
            tracks: vec![song("")],
            artist: None,
        };
        let _ = app.online.show_collection(playlist.clone());
        let _ = app.handle(Message::PlayAll(playlist.tracks.clone(), false));
        app.screen = crate::Screen::Home;
        let _ = app.handle(Message::SongMenu(Event::Open {
            track: song(""),
            place: Place::Queue {
                index: 0,
                upcoming: false,
            },
            at: Point::ORIGIN,
        }));
        assert_eq!(
            app.song_menu
                .as_ref()
                .and_then(|menu| menu.playlist.clone()),
            Some("PLmine".to_string())
        );

        let _ = app.handle(Message::SongMenu(Event::Open {
            track: SearchTrack {
                id: "other".to_string(),
                ..song("")
            },
            place: Place::Queue {
                index: 1,
                upcoming: true,
            },
            at: Point::ORIGIN,
        }));
        assert_eq!(
            app.song_menu
                .as_ref()
                .and_then(|menu| menu.playlist.clone()),
            None,
            "a song added by hand is not in the playlist"
        );
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
