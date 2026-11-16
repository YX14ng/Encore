//! Liking and disliking songs on YouTube Music. rustypipe has no call for it, so these are the
//! requests music.youtube.com makes: `like/like`, `like/dislike` and `like/removelike`.
//!
//! YouTube does not say how a song was rated: `next` answers "INDIFFERENT" for liked songs (and
//! "LIKE" for any song when asked inside Liked Music). So a song counts as liked when it is in
//! the library's Liked Music, and as disliked when it was disliked in Encore, which keeps those
//! in `~/.local/share/encore/disliked.json`. With the "Skip disliked songs" plugin on (like
//! Pear's plugin of the same name), disliking the playing song skips it, and so does a disliked
//! song that comes up again.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use iced::Task;
use rust_i18n::t;
use rustypipe::client::{ClientType, RustyPipe};
use serde_json::{Value, json};

use crate::library::Account;
use crate::youtube::SearchTrack;
use crate::{App, Message};

/// How the user rated a song.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Rating {
    #[default]
    None,
    Like,
    Dislike,
}

impl Rating {
    fn endpoint(self) -> &'static str {
        match self {
            Self::None => "like/removelike",
            Self::Like => "like/like",
            Self::Dislike => "like/dislike",
        }
    }

    /// The rating after pressing the button for `pressed`: pressing the lit one takes it back.
    pub fn toggled(self, pressed: Self) -> Self {
        if self == pressed { Self::None } else { pressed }
    }
}

/// What happens with ratings.
#[derive(Debug, Clone)]
pub enum Event {
    /// The like or dislike button was pressed.
    Pressed(Rating),
    /// Like or Dislike was chosen in a song's menu.
    RateSong(SearchTrack, Rating),
    /// YouTube answered the change of `track` from `previous` to `rating`.
    Saved {
        track: SearchTrack,
        rating: Rating,
        previous: Rating,
        result: Result<(), String>,
    },
}

/// The songs disliked in Encore, by video id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Disliked(BTreeSet<String>);

impl Disliked {
    /// Reads the saved list; a missing or broken file is an empty list.
    pub fn load() -> Self {
        Self::read(&path())
    }

    fn read(path: &Path) -> Self {
        let ids = std::fs::read_to_string(path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Self(ids)
    }

    /// Saves the list. A failed write only means it is not remembered next time.
    fn save(&self) {
        self.write(&path());
    }

    fn write(&self, path: &Path) {
        if let Some(dir) = path.parent()
            && std::fs::create_dir_all(dir).is_ok()
            && let Ok(json) = serde_json::to_string_pretty(&self.0)
        {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        self.0.contains(id)
    }

    /// Records `rating` of song `id`; returns whether the list changed.
    fn set(&mut self, id: &str, rating: Rating) -> bool {
        if rating == Rating::Dislike {
            self.0.insert(id.to_string())
        } else {
            self.0.remove(id)
        }
    }
}

fn path() -> PathBuf {
    crate::account::data_dir().join("disliked.json")
}

/// Rates song `id` on YouTube Music; `Rating::None` takes a like or dislike back.
pub async fn set(rp: RustyPipe, id: String, rating: Rating) -> Result<(), String> {
    let body = json!({ "target": { "videoId": id } });
    let response = rp
        .query()
        .authenticated()
        .raw(ClientType::DesktopMusic, rating.endpoint(), &body)
        .await
        .map_err(|error| match error {
            rustypipe::error::Error::HttpStatus(401, _) => {
                crate::playlists::edit::session_expired()
            }
            error => error.to_string(),
        })?;
    serde_json::from_str::<Value>(&response)
        .map(|_| ())
        .map_err(|error| t!("rating.not_json", error = error).into_owned())
}

impl App {
    /// Lights the buttons for `track`, which just started. With the plugin on, a song
    /// disliked before is skipped instead: the returned task plays the next one.
    pub(crate) fn show_rating(&mut self, track: &SearchTrack) -> Option<Task<Message>> {
        self.rating = self.rating_of(&track.id);
        (self.rating == Rating::Dislike && self.plugins.skip_disliked).then(|| {
            Task::batch([
                self.handle(Message::Next),
                self.show_toast(t!("rating.skipped", title = track.title).into_owned()),
            ])
        })
    }

    pub(crate) fn rating_of(&self, id: &str) -> Rating {
        if self.disliked.contains(id) {
            return Rating::Dislike;
        }
        match &self.account {
            Account::SignedIn(data) if data.liked.iter().any(|liked| liked.id == id) => {
                Rating::Like
            }
            _ => Rating::None,
        }
    }

    /// Whether the playing song can be rated: a YouTube song, signed in.
    pub(crate) fn can_rate(&self) -> bool {
        self.rating_client().is_some() && self.online.playing_track().is_some()
    }

    pub(crate) fn rating_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Pressed(pressed) => match self.online.playing_track().cloned() {
                Some(track) => self.rate(track, pressed),
                None => Task::none(),
            },
            Event::RateSong(track, pressed) => self.rate(track, pressed),
            Event::Saved {
                track,
                rating,
                previous,
                result,
            } => match result {
                Ok(()) => {
                    self.keep_rating(&track, rating);
                    Task::none()
                }
                Err(error) => {
                    if self.is_playing(&track.id) {
                        self.rating = previous;
                    }
                    self.show_toast(
                        t!("rating.failed", title = track.title, error = error).into_owned(),
                    )
                }
            },
        }
    }

    /// Rates `track` and tells YouTube; for the playing song the button lights at once, and
    /// disliking it skips it with the plugin on.
    fn rate(&mut self, track: SearchTrack, pressed: Rating) -> Task<Message> {
        let Some(client) = self.rating_client() else {
            return Task::none();
        };
        let playing = self.is_playing(&track.id);
        let previous = if playing {
            self.rating
        } else {
            self.rating_of(&track.id)
        };
        let rating = previous.toggled(pressed);
        if playing {
            self.rating = rating;
        }
        let saved = Task::perform(set(client, track.id.clone(), rating), move |result| {
            Message::Rating(Event::Saved {
                track: track.clone(),
                rating,
                previous,
                result,
            })
        });
        if playing && rating == Rating::Dislike && self.plugins.skip_disliked {
            return Task::batch([saved, self.handle(Message::Next)]);
        }
        saved
    }

    /// Remembers a rating YouTube accepted: in the Liked Music shelf and the disliked list.
    fn keep_rating(&mut self, track: &SearchTrack, rating: Rating) {
        if self.disliked.set(&track.id, rating) {
            self.disliked.save();
        }
        let Account::SignedIn(data) = &mut self.account else {
            return;
        };
        data.liked.retain(|liked| liked.id != track.id);
        if rating == Rating::Like {
            data.liked.insert(0, track.clone());
        }
    }

    fn is_playing(&self, id: &str) -> bool {
        self.online
            .playing_track()
            .is_some_and(|track| track.id == id)
    }

    fn rating_client(&self) -> Option<RustyPipe> {
        matches!(self.account, Account::SignedIn(_))
            .then(|| self.online.client())
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pressing_the_lit_button_takes_the_rating_back() {
        assert_eq!(Rating::None.toggled(Rating::Like), Rating::Like);
        assert_eq!(Rating::Like.toggled(Rating::Like), Rating::None);
        assert_eq!(Rating::Like.toggled(Rating::Dislike), Rating::Dislike);
        assert_eq!(Rating::Dislike.toggled(Rating::Dislike), Rating::None);
    }

    #[test]
    fn disliked_songs_are_remembered_until_taken_back() {
        let dir = std::env::temp_dir().join(format!("encore-disliked-{}", std::process::id()));
        let file = dir.join("disliked.json");
        let mut disliked = Disliked::default();
        assert!(disliked.set("abc", Rating::Dislike));
        assert!(!disliked.set("abc", Rating::Dislike), "already there");
        disliked.write(&file);
        let mut read = Disliked::read(&file);
        assert!(read.contains("abc"));
        assert!(read.set("abc", Rating::Like));
        assert!(!read.contains("abc"));
        assert_eq!(
            Disliked::read(&dir.join("missing.json")),
            Disliked::default()
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Dislikes a song on the signed-in account and takes the dislike back at once. It changes
    /// the account, so run it only with the user's go-ahead:
    /// `cargo test -- --ignored dislikes_and_takes_it_back`.
    #[tokio::test]
    #[ignore = "changes the signed-in account"]
    async fn dislikes_and_takes_it_back() {
        let rp = crate::youtube::client().expect("client");
        // Rick Astley's "Never Gonna Give You Up", the song.
        let id = "lYBUbBu4W08".to_string();
        let disliked = set(rp.clone(), id.clone(), Rating::Dislike).await;
        let taken_back = set(rp, id, Rating::None).await;
        assert_eq!(
            (disliked, taken_back),
            (Ok(()), Ok(())),
            "dislike, then remove"
        );
    }
}
