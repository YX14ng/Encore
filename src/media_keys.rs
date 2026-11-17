//! The desktop's media controls (MPRIS on Linux): media keys, the shell's player widget and
//! `playerctl` can see what Encore plays and control it.

use std::sync::Mutex;
use std::time::Duration;

use iced::futures::channel::mpsc::{UnboundedReceiver, unbounded};
use iced::futures::{Stream, StreamExt, stream};
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

use crate::Message;

/// What a media key asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKey {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
}

impl MediaKey {
    /// The keys Encore handles; the rest (seek, raise...) are ignored for now.
    pub fn from_event(event: &MediaControlEvent) -> Option<Self> {
        match event {
            MediaControlEvent::Play => Some(MediaKey::Play),
            MediaControlEvent::Pause | MediaControlEvent::Stop => Some(MediaKey::Pause),
            MediaControlEvent::Toggle => Some(MediaKey::Toggle),
            MediaControlEvent::Next => Some(MediaKey::Next),
            MediaControlEvent::Previous => Some(MediaKey::Previous),
            _ => None,
        }
    }
}

/// The events from the desktop, waiting for the app's subscription to pick them up.
static EVENTS: Mutex<Option<UnboundedReceiver<MediaKey>>> = Mutex::new(None);

/// What was last told to the desktop, so it is only told again when something changes.
#[derive(Default, PartialEq)]
struct Published {
    song: (String, String, String, Option<String>, u32),
    playing: Option<bool>,
}

/// Encore's entry in the desktop's media controls.
pub struct MediaKeys {
    controls: MediaControls,
    published: Published,
}

impl MediaKeys {
    /// Registers Encore as `org.mpris.MediaPlayer2.encore.instance<pid>` on Linux (`None` if
    /// there is no session bus), or in the media controls of `window` (its handle, `HWND`) on
    /// Windows.
    pub fn start(window: Option<*mut std::ffi::c_void>) -> Option<Self> {
        if cfg!(windows) && window.is_none() {
            return None;
        }
        // A name of this process's own: with a shared one, any other process asking for it
        // (another Encore, a test) takes it, and the bus never gives it back.
        let dbus_name = bus_name_suffix();
        let config = PlatformConfig {
            display_name: "Encore",
            dbus_name: &dbus_name,
            hwnd: window,
        };
        let mut controls = MediaControls::new(config).ok()?;
        let (sender, receiver) = unbounded();
        controls
            .attach(move |event| {
                if let Some(key) = MediaKey::from_event(&event) {
                    // The app is gone if this fails; nothing to do.
                    let _ = sender.unbounded_send(key);
                }
            })
            .ok()?;
        if let Ok(mut events) = EVENTS.lock() {
            *events = Some(receiver);
        }
        Some(Self {
            controls,
            published: Published::default(),
        })
    }

    /// Tells the desktop what is playing. Cheap to call often: it only sends changes, plus the
    /// position while playing.
    #[expect(clippy::too_many_arguments, reason = "one call site, plain data")]
    pub fn publish(
        &mut self,
        title: &str,
        artist: &str,
        album: &str,
        art_url: Option<&str>,
        duration: u32,
        elapsed: u32,
        playing: bool,
    ) {
        let song = (
            title.to_string(),
            artist.to_string(),
            album.to_string(),
            art_url.map(str::to_string),
            duration,
        );
        // Nothing has played yet: the desktop shows no song.
        let nothing = title.is_empty();
        if song != self.published.song {
            let _ = self.controls.set_metadata(MediaMetadata {
                title: (!nothing).then_some(title),
                artist: (!nothing).then_some(artist),
                album: (!nothing).then_some(album),
                cover_url: art_url,
                duration: (duration > 0).then(|| Duration::from_secs(u64::from(duration))),
            });
            self.published.song = song;
            self.published.playing = None;
        }
        if self.published.playing != Some(playing) || playing {
            let progress = Some(MediaPosition(Duration::from_secs(u64::from(elapsed))));
            let playback = if nothing {
                MediaPlayback::Stopped
            } else if playing {
                MediaPlayback::Playing { progress }
            } else {
                MediaPlayback::Paused { progress }
            };
            let _ = self.controls.set_playback(playback);
            self.published.playing = Some(playing);
        }
    }
}

/// The end of Encore's MPRIS name, `org.mpris.MediaPlayer2.encore.instance<pid>`: one per
/// process, as the MPRIS spec allows, so two can never take each other's.
fn bus_name_suffix() -> String {
    format!("encore.instance{}", std::process::id())
}

/// The media key presses, as app messages. Runs for the whole life of the app.
pub fn events() -> impl Stream<Item = Message> {
    let receiver = EVENTS.lock().ok().and_then(|mut events| events.take());
    match receiver {
        Some(receiver) => receiver.map(Message::MediaKey).left_stream(),
        None => stream::pending().right_stream(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_the_keys_encore_handles() {
        assert_eq!(
            MediaKey::from_event(&MediaControlEvent::Toggle),
            Some(MediaKey::Toggle)
        );
        assert_eq!(
            MediaKey::from_event(&MediaControlEvent::Stop),
            Some(MediaKey::Pause)
        );
        assert_eq!(MediaKey::from_event(&MediaControlEvent::Raise), None);
    }

    /// Registers on the session bus, publishes a song, reads it back with `busctl` and sends a
    /// PlayPause like a media key would: `cargo test -- --ignored talks_mpris`.
    #[test]
    #[ignore = "needs a D-Bus session and busctl"]
    fn talks_mpris() {
        let mut media = MediaKeys::start(None).expect("session bus");
        media.publish(
            "Test Song",
            "Test Artist",
            "Test Album",
            None,
            200,
            12,
            true,
        );
        std::thread::sleep(Duration::from_millis(500));

        let busctl = |args: &[&str]| {
            let output = std::process::Command::new("busctl")
                .arg("--user")
                .args(args)
                .output()
                .expect("busctl");
            String::from_utf8_lossy(&output.stdout).to_string()
        };
        let path = "/org/mpris/MediaPlayer2";
        let player = "org.mpris.MediaPlayer2.Player";
        let name = format!("org.mpris.MediaPlayer2.{}", bus_name_suffix());
        let name = name.as_str();
        let metadata = busctl(&["get-property", name, path, player, "Metadata"]);
        assert!(metadata.contains("Test Song"), "metadata: {metadata}");
        let status = busctl(&["get-property", name, path, player, "PlaybackStatus"]);
        assert!(status.contains("Playing"), "status: {status}");

        busctl(&["call", name, path, player, "PlayPause"]);
        std::thread::sleep(Duration::from_millis(300));
        let mut receiver = EVENTS.lock().expect("lock").take().expect("receiver");
        assert_eq!(receiver.try_recv().ok(), Some(MediaKey::Toggle));
    }
}
