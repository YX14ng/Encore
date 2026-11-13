//! Discord Rich Presence: the song Encore is playing shows on the user's Discord profile as
//! "Listening to …", with the cover, the artist and a progress bar.
//!
//! Discord's local socket is blocking and may be missing (Discord closed), so a background
//! thread owns it, reconnects every few seconds, and only ever sees the latest state.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{
    Activity, ActivityType, Assets, StatusDisplayType, Timestamps,
};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

/// The "Encore" application on Discord's developer portal. Public: it names the activity.
const APPLICATION_ID: &str = "1570947687027839128";
/// How often to try again while Discord is closed.
const RETRY: Duration = Duration::from_secs(15);
/// A jump in position bigger than this (a seek) is sent again.
const SEEK_TOLERANCE: u32 = 3;

/// What Discord shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presence {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// The cover's web address (Discord loads it itself).
    pub cover_url: Option<String>,
    pub duration: u32,
    pub elapsed: u32,
    pub playing: bool,
}

/// The connection to Discord, from the app's side.
pub struct Discord {
    sender: Sender<Option<Presence>>,
    sent: Option<(Presence, Instant)>,
}

impl Discord {
    /// Starts the background thread. Nothing is shown until `update` gets a song.
    pub fn start() -> Self {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || worker(&receiver));
        Self { sender, sent: None }
    }

    /// Shows `presence` on Discord, or clears it with `None`. Cheap to call often: it only sends
    /// changes (another song, play or pause, or a seek).
    pub fn update(&mut self, presence: Option<Presence>) {
        let unchanged = match (&self.sent, &presence) {
            (None, None) => true,
            (Some((sent, at)), Some(new)) => same_activity(sent, at.elapsed(), new),
            _ => false,
        };
        if unchanged {
            return;
        }
        self.sent = presence.clone().map(|presence| (presence, Instant::now()));
        // The thread only stops with the app.
        let _ = self.sender.send(presence);
    }
}

/// Whether `new` is what Discord already shows, given `since` passed since `sent` was sent.
fn same_activity(sent: &Presence, since: Duration, new: &Presence) -> bool {
    let same_song = (
        &sent.title,
        &sent.artist,
        &sent.album,
        &sent.cover_url,
        sent.duration,
    ) == (
        &new.title,
        &new.artist,
        &new.album,
        &new.cover_url,
        new.duration,
    );
    if !same_song || sent.playing != new.playing {
        return false;
    }
    if !new.playing {
        return true;
    }
    let expected = sent.elapsed + since.as_secs() as u32;
    expected.abs_diff(new.elapsed) <= SEEK_TOLERANCE
}

fn worker(receiver: &Receiver<Option<Presence>>) {
    let mut client = DiscordIpcClient::new(APPLICATION_ID);
    let mut connected = false;
    let mut current: Option<Presence> = None;
    loop {
        match receiver.recv_timeout(RETRY) {
            Ok(presence) => current = presence,
            Err(RecvTimeoutError::Timeout) if connected => continue,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                let _ = client.close();
                return;
            }
        }
        if !connected {
            connected = client.connect().is_ok();
            if !connected {
                continue;
            }
        }
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_millis() as i64);
        let result = match &current {
            Some(presence) => client.set_activity(activity(presence, now_ms)),
            None => client.clear_activity(),
        };
        if result.is_err() {
            // Discord closed: try again later.
            let _ = client.close();
            connected = false;
        }
    }
}

fn activity(presence: &Presence, now_ms: i64) -> Activity<'_> {
    let text = activity_text(presence, now_ms);
    let mut assets = Assets::new().large_text(text.album);
    if let Some(cover) = &presence.cover_url {
        assets = assets.large_image(cover.as_str());
    }
    let mut activity = Activity::new()
        .activity_type(ActivityType::Listening)
        .status_display_type(StatusDisplayType::Details)
        .details(text.details)
        .state(text.state)
        .assets(assets);
    if let Some((start, end)) = text.timestamps {
        activity = activity.timestamps(Timestamps::new().start(start).end(end));
    }
    activity
}

/// The words and times of an activity.
#[derive(Debug, PartialEq, Eq)]
struct ActivityText {
    details: String,
    state: String,
    album: String,
    /// Start and end in Unix milliseconds, so Discord draws the progress bar; none while paused.
    timestamps: Option<(i64, i64)>,
}

fn activity_text(presence: &Presence, now_ms: i64) -> ActivityText {
    let state = if presence.playing {
        presence.artist.clone()
    } else {
        format!("{} \u{b7} paused", presence.artist)
    };
    let album = if presence.album.is_empty() {
        presence.title.clone()
    } else {
        presence.album.clone()
    };
    let timestamps = (presence.playing && presence.duration > 0).then(|| {
        let start = now_ms - i64::from(presence.elapsed) * 1000;
        (start, start + i64::from(presence.duration) * 1000)
    });
    ActivityText {
        details: fit(&presence.title),
        state: fit(&state),
        album: fit(&album),
        timestamps,
    }
}

/// Discord wants between 2 and 128 characters.
fn fit(text: &str) -> String {
    let mut fitted: String = text.chars().take(128).collect();
    while fitted.chars().count() < 2 {
        fitted.push(' ');
    }
    fitted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn presence(elapsed: u32, playing: bool) -> Presence {
        Presence {
            title: "Usseewa".to_string(),
            artist: "Ado".to_string(),
            album: "Kyougen".to_string(),
            cover_url: None,
            duration: 200,
            elapsed,
            playing,
        }
    }

    #[test]
    fn playing_songs_get_a_progress_bar() {
        let text = activity_text(&presence(30, true), 1_000_000);
        assert_eq!(text.details, "Usseewa");
        assert_eq!(text.state, "Ado");
        assert_eq!(text.timestamps, Some((970_000, 1_170_000)));
    }

    #[test]
    fn paused_songs_say_so_without_a_bar() {
        let text = activity_text(&presence(30, false), 1_000_000);
        assert_eq!(text.state, "Ado \u{b7} paused");
        assert_eq!(text.timestamps, None);
    }

    #[test]
    fn short_and_long_texts_fit_discord_limits() {
        assert_eq!(fit("A"), "A ");
        assert_eq!(fit(&"x".repeat(200)).chars().count(), 128);
    }

    #[test]
    fn only_changes_and_seeks_are_sent_again() {
        let sent = presence(30, true);
        let ten_seconds = Duration::from_secs(10);
        assert!(same_activity(&sent, ten_seconds, &presence(40, true)));
        assert!(
            !same_activity(&sent, ten_seconds, &presence(90, true)),
            "a seek"
        );
        assert!(
            !same_activity(&sent, ten_seconds, &presence(40, false)),
            "a pause"
        );
        let mut other = presence(40, true);
        other.title = "Odo".to_string();
        assert!(!same_activity(&sent, ten_seconds, &other), "another song");
    }

    /// Connects to the running Discord with Encore's application id and clears the activity,
    /// without showing anything: `cargo test -- --ignored connects_to_discord`.
    #[test]
    #[ignore = "needs Discord running"]
    fn connects_to_discord() {
        let mut client = DiscordIpcClient::new(APPLICATION_ID);
        client.connect().expect("handshake with Discord");
        client.clear_activity().expect("clear activity");
        client.close().expect("close");
    }
}
