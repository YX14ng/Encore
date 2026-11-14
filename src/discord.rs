//! Discord Rich Presence: the song Encore is playing shows on the user's Discord profile as
//! "Listening to …", with the cover, the artist and a progress bar. While the user hosts a
//! listen-together session, the activity also carries the session as a party with a join
//! secret, so friends can ask to join from the profile or get an invite in a chat.
//!
//! Discord's local socket is blocking and may be missing (Discord closed), so a background
//! thread owns it, reconnects every few seconds, and only ever sees the latest state. Another
//! thread reads what Discord sends and passes join invites and requests to the app.

use std::os::unix::net::UnixStream;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{
    Activity, ActivityType, Assets, Party, Secrets, StatusDisplayType, Timestamps,
};
use iced::futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use iced::futures::{Stream, StreamExt, stream};
use serde_json::json;

use crate::Message;
use crate::discord_ipc::{self, Event};
use crate::together::host::MAX_GUESTS;

/// The "Encore" application on Discord's developer portal. Public: it names the activity.
const APPLICATION_ID: &str = "1570947687027839128";
/// How often to try again while Discord is closed.
const RETRY: Duration = Duration::from_secs(15);
/// A jump in position bigger than this (a seek) is sent again.
const SEEK_TOLERANCE: u32 = 3;

/// What Discord sends, waiting for the app's subscription to take it.
static EVENTS: Mutex<Option<UnboundedReceiver<Event>>> = Mutex::new(None);

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
    /// The listen-together session the user hosts, if any.
    pub session: Option<Hosted>,
}

/// A hosted session as Discord shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hosted {
    /// Names the party; not secret.
    pub party_id: String,
    pub listeners: u32,
    /// The invite, which Discord hands to friends who join.
    pub secret: String,
}

/// What the app asks the Discord thread to do.
enum Command {
    Show(Option<Presence>),
    /// Accepts or turns down someone's request to join.
    Answer {
        user_id: String,
        accept: bool,
    },
}

/// The connection to Discord, from the app's side.
pub struct Discord {
    sender: Sender<Command>,
    sent: Option<(Presence, Instant)>,
}

impl Discord {
    /// Starts the background threads. Nothing is shown until `update` gets a song.
    pub fn start() -> Self {
        let (sender, receiver) = mpsc::channel();
        let (events, events_receiver) = unbounded();
        if let Ok(mut slot) = EVENTS.lock() {
            *slot = Some(events_receiver);
        }
        thread::spawn(move || worker(&receiver, &events));
        Self { sender, sent: None }
    }

    /// Shows `presence` on Discord, or clears it with `None`. Cheap to call often: it only sends
    /// changes (another song, play or pause, a seek, or the session).
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
        let _ = self.sender.send(Command::Show(presence));
    }

    /// Accepts or turns down the request to join from `user_id`.
    pub fn answer(&self, user_id: String, accept: bool) {
        let _ = self.sender.send(Command::Answer { user_id, accept });
    }
}

/// Join invites and requests from Discord, for the app's subscription.
pub fn events() -> impl Stream<Item = Message> {
    let receiver = EVENTS.lock().ok().and_then(|mut events| events.take());
    match receiver {
        Some(receiver) => receiver.map(Message::Discord).left_stream(),
        None => stream::pending().right_stream(),
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
        &sent.session,
    ) == (
        &new.title,
        &new.artist,
        &new.album,
        &new.cover_url,
        new.duration,
        &new.session,
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

fn worker(receiver: &Receiver<Command>, events: &UnboundedSender<Event>) {
    let mut socket: Option<UnixStream> = None;
    let mut current: Option<Presence> = None;
    loop {
        let command = match receiver.recv_timeout(RETRY) {
            Ok(command) => Some(command),
            Err(RecvTimeoutError::Timeout) if socket.is_some() => continue,
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => {
                if let Some(mut socket) = socket {
                    let _ = discord_ipc::write_frame(&mut socket, discord_ipc::CLOSE, &json!({}));
                }
                return;
            }
        };
        let mut show = socket.is_none();
        let answer = match command {
            Some(Command::Show(presence)) => {
                current = presence;
                show = true;
                None
            }
            Some(Command::Answer { user_id, accept }) => Some((user_id, accept)),
            None => None,
        };
        if socket.is_none() {
            socket = connect(events).ok();
        }
        let Some(stream) = &mut socket else {
            continue;
        };
        let mut result = Ok(());
        if show {
            let activity = current
                .as_ref()
                .map(|presence| activity(presence, now_ms()));
            let args = json!({ "pid": std::process::id(), "activity": activity });
            let payload = discord_ipc::command("SET_ACTIVITY", args, None);
            result = discord_ipc::write_frame(stream, discord_ipc::FRAME, &payload);
        }
        if let Some((user_id, accept)) = answer {
            let name = if accept {
                "SEND_ACTIVITY_JOIN_INVITE"
            } else {
                "CLOSE_ACTIVITY_REQUEST"
            };
            let payload = discord_ipc::command(name, json!({ "user_id": user_id }), None);
            result = result.and(discord_ipc::write_frame(
                stream,
                discord_ipc::FRAME,
                &payload,
            ));
        }
        if result.is_err() {
            // Discord closed: try again later.
            socket = None;
        }
    }
}

/// Connects, asks for join invites and requests, and starts the thread that reads them.
fn connect(events: &UnboundedSender<Event>) -> std::io::Result<UnixStream> {
    let mut stream = discord_ipc::connect(APPLICATION_ID)?;
    let mut reader = stream.try_clone()?;
    let events = events.clone();
    thread::spawn(move || {
        while let Ok((opcode, payload)) = discord_ipc::read_frame(&mut reader) {
            if opcode == discord_ipc::CLOSE {
                return;
            }
            if let Some(event) = discord_ipc::event(&payload) {
                let _ = events.unbounded_send(event);
            }
        }
    });
    for event in ["ACTIVITY_JOIN", "ACTIVITY_JOIN_REQUEST"] {
        let payload = discord_ipc::command("SUBSCRIBE", json!({}), Some(event));
        discord_ipc::write_frame(&mut stream, discord_ipc::FRAME, &payload)?;
    }
    Ok(stream)
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as i64)
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
    if let Some(session) = &presence.session {
        let size = [session.listeners as i32 + 1, MAX_GUESTS as i32 + 1];
        activity = activity
            .party(Party::new().id(session.party_id.as_str()).size(size))
            .secrets(Secrets::new().join(session.secret.as_str()));
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
            session: None,
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

    /// Connects to the running Discord with Encore's application id, asks for join events and
    /// clears the activity, without showing anything:
    /// `cargo test -- --ignored connects_to_discord`.
    #[test]
    #[ignore = "needs Discord running"]
    fn connects_to_discord() {
        let (events, _receiver) = unbounded();
        let mut stream = connect(&events).expect("handshake with Discord");
        let args = json!({ "pid": std::process::id(), "activity": null });
        let payload = discord_ipc::command("SET_ACTIVITY", args, None);
        discord_ipc::write_frame(&mut stream, discord_ipc::FRAME, &payload).expect("clear");
    }

    #[test]
    fn a_hosted_session_becomes_a_party_with_a_join_secret() {
        let mut hosting = presence(30, true);
        hosting.session = Some(Hosted {
            party_id: "party".to_string(),
            listeners: 2,
            secret: "encore:203.0.113.7:41730:token".to_string(),
        });
        let json = serde_json::to_value(activity(&hosting, 0)).expect("json");
        assert_eq!(json["party"]["size"], json!([3, MAX_GUESTS + 1]));
        assert_eq!(json["secrets"]["join"], "encore:203.0.113.7:41730:token");
        assert!(!same_activity(
            &presence(30, true),
            Duration::ZERO,
            &hosting
        ));
    }
}
