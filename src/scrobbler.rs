//! Scrobbling to Last.fm: when a song counts as listened to, the queue of listens waiting to be
//! sent (kept on disk, so nothing is lost offline) and the Last.fm part of the account screen.
//! The API itself lives in `lastfm.rs`.

use iced::font::Weight;
use iced::widget::{column, text};
use iced::{Element, Font, Task};

use crate::lastfm::{self, ApiError, Credentials, Scrobble, Session};
use crate::youtube::SearchTrack;
use crate::{Message, library, lyrics_source, theme};

const SESSION_FILE: &str = "session.json";
const PENDING_FILE: &str = "pending.json";
/// Last.fm's rule: a song counts once half of it, or four minutes, has been heard…
const MAX_REQUIRED: u32 = 240;
/// …and songs this short never count.
const MIN_DURATION: u32 = 30;
/// A jump in the position larger than this is a seek, not listening.
const MAX_STEP: u32 = 3;

/// The seconds of `duration` that must be heard before a song counts, or `None` if it never
/// does. An unknown length (0) needs the full four minutes.
pub fn required_seconds(duration: u32) -> Option<u32> {
    match duration {
        0 => Some(MAX_REQUIRED),
        short if short <= MIN_DURATION => None,
        duration => Some((duration / 2).min(MAX_REQUIRED)),
    }
}

/// How much of the current song has been heard.
#[derive(Debug, Clone)]
pub struct Listen {
    scrobble: Scrobble,
    heard: u32,
    position: u32,
    counted: bool,
}

impl Listen {
    /// A song that started playing at `timestamp` (seconds since 1970).
    pub fn new(track: &SearchTrack, timestamp: i64) -> Self {
        Self {
            scrobble: Scrobble {
                artist: lyrics_source::first_artist(&track.artist).to_string(),
                track: track.title.clone(),
                album: track.album.clone(),
                duration: track.duration,
                timestamp,
            },
            heard: 0,
            position: 0,
            counted: false,
        }
    }

    /// Feeds the playback position (seconds) and returns the scrobble the first time enough of
    /// the song has been heard. Seeking does not count as listening.
    pub fn advance(&mut self, position: u32) -> Option<Scrobble> {
        if position > self.position && position - self.position <= MAX_STEP {
            self.heard += position - self.position;
        }
        self.position = position;
        let required = required_seconds(self.scrobble.duration)?;
        if self.counted || self.heard < required {
            return None;
        }
        self.counted = true;
        Some(self.scrobble.clone())
    }
}

/// Whether Encore can scrobble.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// The API account is not in the environment; the text says what is missing.
    MissingKeys(String),
    SignedOut,
    /// Waiting for the user to approve Encore on the Last.fm page.
    Connecting,
    Connected(Session),
    Failed(String),
}

/// The Last.fm connection, the song being listened to and the listens not sent yet.
pub struct LastFm {
    credentials: Option<Credentials>,
    status: Status,
    listen: Option<Listen>,
    pending: Vec<Scrobble>,
    /// How many listens from the front of `pending` are on their way.
    sending: usize,
    last_error: Option<String>,
}

impl LastFm {
    /// Reads the API account from the environment and the saved session and queue from disk.
    /// Without the API account, the account screen says which variables are missing.
    pub fn load() -> Self {
        let (credentials, status) = match Credentials::from_env() {
            Ok(credentials) => {
                let status = match lastfm::load::<Session>(SESSION_FILE) {
                    Some(session) => Status::Connected(session),
                    None => Status::SignedOut,
                };
                (Some(credentials), status)
            }
            Err(error) => (None, Status::MissingKeys(error)),
        };
        Self {
            credentials,
            status,
            listen: None,
            pending: lastfm::load(PENDING_FILE).unwrap_or_default(),
            sending: 0,
            last_error: None,
        }
    }

    /// Starts the sign-in: opens the approval page and waits for the user.
    pub fn connect(&mut self) -> Task<Message> {
        let Some(credentials) = self.credentials.clone() else {
            return Task::none();
        };
        self.status = Status::Connecting;
        Task::perform(lastfm::connect(credentials), Message::LastFmConnected)
    }

    /// Keeps the session once the user approved Encore and sends what was waiting.
    pub fn connected(&mut self, result: Result<Session, String>) -> Task<Message> {
        match result {
            Ok(session) => {
                if let Err(error) = lastfm::save(SESSION_FILE, &session) {
                    self.last_error = Some(format!("could not save the session: {error}"));
                }
                self.status = Status::Connected(session);
                self.flush()
            }
            Err(error) => {
                self.status = Status::Failed(error);
                Task::none()
            }
        }
    }

    /// Forgets the session. Listens not sent yet stay queued for the next connection.
    pub fn disconnect(&mut self) {
        lastfm::remove(SESSION_FILE);
        if self.credentials.is_some() {
            self.status = Status::SignedOut;
        }
    }

    /// A new song started: tells Last.fm it is playing now.
    pub fn track_started(&mut self, track: &SearchTrack) -> Task<Message> {
        let listen = Listen::new(track, chrono::Utc::now().timestamp());
        let playing = listen.scrobble.clone();
        self.listen = Some(listen);
        let Some((credentials, key)) = self.signed_in() else {
            return Task::none();
        };
        Task::perform(
            lastfm::update_now_playing(credentials, key, playing),
            Message::LastFmAnswered,
        )
    }

    /// The song reached `position`: once it has been heard enough, it is queued and sent.
    pub fn progress(&mut self, position: u32) -> Task<Message> {
        let Some(scrobble) = self
            .listen
            .as_mut()
            .and_then(|listen| listen.advance(position))
        else {
            return Task::none();
        };
        self.pending.push(scrobble);
        self.save_pending();
        self.flush()
    }

    /// Sends the oldest waiting listens, unless a batch is already on its way.
    fn flush(&mut self) -> Task<Message> {
        if self.sending > 0 || self.pending.is_empty() {
            return Task::none();
        }
        let Some((credentials, key)) = self.signed_in() else {
            return Task::none();
        };
        let batch: Vec<Scrobble> = self
            .pending
            .iter()
            .take(lastfm::BATCH_LIMIT)
            .cloned()
            .collect();
        self.sending = batch.len();
        Task::perform(
            lastfm::scrobble(credentials, key, batch),
            Message::Scrobbled,
        )
    }

    /// A batch was sent (or not). Sent and refused listens leave the queue; listens that failed
    /// for a passing reason stay for the next try.
    pub fn scrobbled(&mut self, result: Result<(), ApiError>) -> Task<Message> {
        let sent = std::mem::take(&mut self.sending);
        let keep = result
            .as_ref()
            .is_err_and(|error| error.is_temporary() || error.is_invalid_session());
        if !keep {
            self.pending.drain(..sent.min(self.pending.len()));
            self.save_pending();
        }
        match result {
            Ok(()) => {
                self.last_error = None;
                self.flush()
            }
            Err(error) => {
                self.answered(Err(error));
                Task::none()
            }
        }
    }

    /// Notes a failed request; a revoked session means connecting again.
    pub fn answered(&mut self, result: Result<(), ApiError>) {
        let Err(error) = result else {
            return;
        };
        if error.is_invalid_session() {
            lastfm::remove(SESSION_FILE);
            self.status = Status::Failed(
                "Last.fm no longer accepts Encore's session; connect again".to_string(),
            );
        }
        self.last_error = Some(error.to_string());
    }

    fn signed_in(&self) -> Option<(Credentials, String)> {
        match (&self.credentials, &self.status) {
            (Some(credentials), Status::Connected(session)) => {
                Some((credentials.clone(), session.key.clone()))
            }
            _ => None,
        }
    }

    fn save_pending(&mut self) {
        let saved = if self.pending.is_empty() {
            lastfm::remove(PENDING_FILE);
            Ok(())
        } else {
            lastfm::save(PENDING_FILE, &self.pending)
        };
        if let Err(error) = saved {
            self.last_error = Some(format!("could not save the scrobble queue: {error}"));
        }
    }

    /// The Last.fm part of the account screen.
    pub fn view(&self) -> Element<'_, Message> {
        let title = text("Last.fm").size(20).font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        });
        let mut content = column![title].spacing(14).padding([8, 32]).max_width(720);

        content = match &self.status {
            Status::MissingKeys(error) => content.push(
                text(format!(
                    "Scrobbling is off: {error}. Set both variables to your Last.fm API account \
                     and restart Encore."
                ))
                .color(theme::accent()),
            ),
            Status::SignedOut => content
                .push(library::note(
                    "Connect Last.fm to scrobble the songs you play in Encore.",
                ))
                .push(library::accent_button(
                    "Connect Last.fm",
                    Message::LastFmConnect,
                )),
            Status::Connecting => content.push(library::note(
                "Approve Encore on the Last.fm page that opened in your browser; Encore notices \
                 on its own within a few seconds.",
            )),
            Status::Connected(session) => content
                .push(library::note(format!("Scrobbling as {}.", session.name)))
                .push(library::accent_button(
                    "Disconnect",
                    Message::LastFmDisconnect,
                )),
            Status::Failed(error) => content
                .push(text(format!("Could not connect: {error}")).color(theme::accent()))
                .push(library::accent_button(
                    "Connect Last.fm",
                    Message::LastFmConnect,
                )),
        };

        if !self.pending.is_empty() {
            content = content.push(library::note(format!(
                "{} {} waiting to be sent.",
                self.pending.len(),
                if self.pending.len() == 1 {
                    "scrobble is"
                } else {
                    "scrobbles are"
                }
            )));
        }
        if let Some(error) = &self.last_error {
            content = content.push(text(error).size(13).color(theme::text_secondary()));
        }
        content.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(duration: u32) -> SearchTrack {
        SearchTrack {
            id: "id".to_string(),
            title: "Usseewa".to_string(),
            artist: "Ado, Someone".to_string(),
            album: "Kyougen".to_string(),
            duration,
            cover_url: None,
        }
    }

    /// Plays `listen` second by second from its current position up to `to`.
    fn play(listen: &mut Listen, to: u32) -> Option<Scrobble> {
        let mut scrobble = None;
        for position in listen.position + 1..=to {
            scrobble = scrobble.or(listen.advance(position));
        }
        scrobble
    }

    #[test]
    fn needs_half_the_song_or_four_minutes() {
        assert_eq!(required_seconds(200), Some(100));
        assert_eq!(required_seconds(600), Some(240));
        assert_eq!(required_seconds(0), Some(240));
        assert_eq!(required_seconds(30), None);
        assert_eq!(required_seconds(31), Some(15));
    }

    #[test]
    fn scrobbles_once_after_half_the_song() {
        let mut listen = Listen::new(&track(200), 1_700_000_000);
        assert_eq!(play(&mut listen, 99), None);
        let scrobble = play(&mut listen, 100).expect("counts at half");
        assert_eq!(scrobble.artist, "Ado");
        assert_eq!(scrobble.timestamp, 1_700_000_000);
        assert_eq!(play(&mut listen, 200), None, "counted twice");
    }

    #[test]
    fn seeking_ahead_is_not_listening() {
        let mut listen = Listen::new(&track(200), 0);
        assert_eq!(listen.advance(150), None);
        assert_eq!(play(&mut listen, 200), None);
        assert_eq!(listen.heard, 50);
    }

    #[test]
    fn short_songs_never_count() {
        let mut listen = Listen::new(&track(25), 0);
        assert_eq!(play(&mut listen, 25), None);
    }
}
