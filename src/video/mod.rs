//! The music video of the playing song, shown in the immersive view. The sound keeps coming from
//! mpv (with the equalizer and everything else); only the picture is added, kept at the song's
//! position.
//!
//! Songs on YouTube Music ("ATV", the album audio) have a music video under another id, which
//! YouTube Music's `next` answer names as the song's counterpart (signed in), with where the two
//! line up. A music video that plays as itself is its own picture. yt-dlp downloads the picture
//! alone (up to 720p) and `decoder` turns it into frames.

mod decoder;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::widget::{column, container, image, text};
use iced::{Center, ContentFit, Element, Fill, Task};
use rustypipe::client::{ClientType, RustyPipe};
use serde_json::{Value, json};

use crate::{App, Message};
use decoder::{Decoder, Step};

/// The least time between two restarts of the decoder, so a stuck one does not restart on
/// every frame.
const RESTART_GAP: Duration = Duration::from_millis(500);

/// A song's music video, and where it lines up with the song.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoSource {
    pub id: String,
    /// Seconds into the song where the video starts: the video's time is the song's minus this.
    pub offset: f64,
}

/// Where the video of the playing song is.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Finding,
    Downloading,
    Playing,
    /// YouTube has no video for the song.
    None,
    Failed(String),
}

/// What happens with the video.
#[derive(Debug, Clone)]
pub enum Event {
    /// The video of song `id` was looked up.
    Found(String, Result<Option<VideoSource>, String>),
    /// The video of song `id` was downloaded.
    Downloaded(String, Result<PathBuf, String>),
}

/// The video mode and the video of the playing song.
#[derive(Default)]
pub struct Video {
    /// Whether the video is shown (the video button is on).
    pub on: bool,
    /// The song the rest is about, and how far its video got.
    song: Option<(String, Status)>,
    source: Option<VideoSource>,
    file: Option<PathBuf>,
    decoder: Option<Decoder>,
    last_restart: Option<Instant>,
    /// The picture on screen.
    pub frame: Option<image::Handle>,
}

impl Video {
    /// How far the video of the playing song got.
    pub fn status(&self) -> Option<&Status> {
        self.song.as_ref().map(|(_, status)| status)
    }

    /// The picture, as large as fits, or a line about where it is.
    pub fn view(&self) -> Element<'_, Message> {
        if let Some(frame) = &self.frame {
            return image(frame.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Contain)
                .into();
        }
        let message = match self.status() {
            Some(Status::Finding) | None => "Looking for the video\u{2026}".to_string(),
            Some(Status::Downloading) => "Downloading the video\u{2026}".to_string(),
            Some(Status::Playing) => String::new(),
            Some(Status::None) => "This song has no music video.".to_string(),
            Some(Status::Failed(error)) => format!("The video could not play: {error}"),
        };
        container(column![
            text(message).size(16).color(crate::theme::text_secondary())
        ])
        .center(Fill)
        .align_x(Center)
        .into()
    }

    fn reset(&mut self) {
        self.song = None;
        self.source = None;
        self.file = None;
        self.decoder = None;
        self.frame = None;
    }

    fn set_status(&mut self, id: &str, status: Status) {
        if let Some((song, slot)) = &mut self.song
            && song == id
        {
            *slot = status;
        }
    }

    fn is_for(&self, id: &str) -> bool {
        self.song.as_ref().is_some_and(|(song, _)| song == id)
    }

    /// Shows the frame due at `song_time` seconds into the song.
    fn step(&mut self, song_time: f64) {
        let (Some(source), Some(file)) = (&self.source, &self.file) else {
            return;
        };
        let time = song_time - source.offset;
        if time < 0.0 {
            // The song starts before the video does.
            self.frame = None;
            return;
        }
        let step = match &mut self.decoder {
            Some(decoder) => decoder.frame_at(time),
            None => Step::Restart,
        };
        match step {
            Step::Same => {}
            Step::New(frame) => {
                self.frame = Some(image::Handle::from_rgba(
                    decoder::WIDTH,
                    decoder::HEIGHT,
                    frame.pixels,
                ));
            }
            Step::Restart => {
                let recently = self
                    .last_restart
                    .is_some_and(|last| last.elapsed() < RESTART_GAP);
                if !recently {
                    self.last_restart = Some(Instant::now());
                    self.decoder = None;
                    match Decoder::start(file, time) {
                        Ok(decoder) => self.decoder = Some(decoder),
                        Err(error) => {
                            if let Some((_, status)) = &mut self.song {
                                *status = Status::Failed(error);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Looks up the music video of song `id`: its counterpart when signed in, or itself when it
/// is a music video. `None` when YouTube has no video for it.
pub async fn find(
    rp: RustyPipe,
    id: String,
    signed_in: bool,
) -> Result<Option<VideoSource>, String> {
    let body = json!({ "videoId": id });
    let q = rp.query();
    let q = if signed_in { q.authenticated() } else { q };
    let answer = q
        .raw(ClientType::DesktopMusic, "next", &body)
        .await
        .map_err(|error| error.to_string())?;
    let answer: Value = serde_json::from_str(&answer).map_err(|error| error.to_string())?;
    Ok(source_in(&answer, &id))
}

/// The video of song `id` in a `next` answer.
fn source_in(answer: &Value, id: &str) -> Option<VideoSource> {
    let mut wrappers = Vec::new();
    collect(answer, "playlistPanelVideoWrapperRenderer", &mut wrappers);
    for wrapper in wrappers {
        let primary = wrapper.pointer("/primaryRenderer/playlistPanelVideoRenderer");
        if primary.and_then(|video| video.get("videoId")?.as_str()) != Some(id) {
            continue;
        }
        let counterpart = wrapper.pointer("/counterpart/0")?;
        let video = counterpart
            .pointer("/counterpartRenderer/playlistPanelVideoRenderer/videoId")?
            .as_str()?;
        let millis = |key: &str| {
            counterpart
                .pointer(&format!("/segmentMap/segment/0/{key}"))
                .and_then(|value| value.as_str()?.parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        let offset = (millis("primaryVideoStartTimeMilliseconds")
            - millis("counterpartVideoStartTimeMilliseconds"))
            / 1000.0;
        return Some(VideoSource {
            id: video.to_string(),
            offset,
        });
    }
    // Without a counterpart, the song is its own video unless it is album audio.
    let mut videos = Vec::new();
    collect(answer, "playlistPanelVideoRenderer", &mut videos);
    let own = videos
        .into_iter()
        .find(|video| video.get("videoId").and_then(Value::as_str) == Some(id))?;
    let mut kinds = Vec::new();
    collect(own, "musicVideoType", &mut kinds);
    let kind = kinds.first().and_then(|kind| kind.as_str())?;
    (kind != "MUSIC_VIDEO_TYPE_ATV").then(|| VideoSource {
        id: id.to_string(),
        offset: 0.0,
    })
}

/// Every value under `key`, anywhere in `value`.
fn collect<'a>(value: &'a Value, key: &str, found: &mut Vec<&'a Value>) {
    match value {
        Value::Object(map) => {
            if let Some(inner) = map.get(key) {
                found.push(inner);
            }
            for inner in map.values() {
                collect(inner, key, found);
            }
        }
        Value::Array(items) => items.iter().for_each(|inner| collect(inner, key, found)),
        _ => {}
    }
}

impl App {
    /// Turns the video on (in the immersive view) or off.
    pub(crate) fn toggle_video(&mut self) -> Task<Message> {
        if self.video.on {
            self.video.on = false;
            self.video.reset();
            return Task::none();
        }
        self.video.on = true;
        let find = self.find_video();
        Task::batch([self.set_immersive(true), find])
    }

    /// Stops the video, when the immersive view closes.
    pub(crate) fn stop_video(&mut self) {
        self.video.on = false;
        self.video.reset();
    }

    /// Looks up the video of the playing song, if the video is on.
    pub(crate) fn find_video(&mut self) -> Task<Message> {
        self.video.reset();
        let Some(track) = self.online.playing_track().filter(|_| self.video.on) else {
            return Task::none();
        };
        let id = track.id.clone();
        let Some(client) = self.online.client() else {
            return Task::none();
        };
        let signed_in = matches!(self.account, crate::library::Account::SignedIn(_));
        self.video.song = Some((id.clone(), Status::Finding));
        Task::perform(find(client, id.clone(), signed_in), move |source| {
            Message::Video(Event::Found(id.clone(), source))
        })
    }

    pub(crate) fn video_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Found(id, _) | Event::Downloaded(id, _) if !self.video.is_for(&id) => {
                Task::none()
            }
            Event::Found(id, Ok(Some(source))) => {
                self.video.set_status(&id, Status::Downloading);
                let video_id = source.id.clone();
                self.video.source = Some(source);
                Task::perform(crate::download::video(video_id), move |file| {
                    Message::Video(Event::Downloaded(id.clone(), file))
                })
            }
            Event::Found(id, Ok(None)) => {
                self.video.set_status(&id, Status::None);
                Task::none()
            }
            Event::Found(id, Err(error)) | Event::Downloaded(id, Err(error)) => {
                self.video.set_status(&id, Status::Failed(error));
                Task::none()
            }
            Event::Downloaded(id, Ok(file)) => {
                self.video.set_status(&id, Status::Playing);
                self.video.file = Some(file);
                self.video_step();
                Task::none()
            }
        }
    }

    /// Keeps the picture at the song's position; called on every frame.
    pub(crate) fn video_step(&mut self) {
        if !self.video.on {
            return;
        }
        if let Some(millis) = self.online.position_ms() {
            self.video.step(f64::from(millis) / 1000.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(id: &str, kind: &str) -> Value {
        json!({ "playlistPanelVideoRenderer": {
            "videoId": id,
            "navigationEndpoint": { "watchEndpoint": { "watchEndpointMusicSupportedConfigs": {
                "watchEndpointMusicConfig": { "musicVideoType": kind } } } },
        } })
    }

    #[test]
    fn a_song_shows_its_counterpart_lined_up() {
        let answer = json!({ "contents": [ { "playlistPanelVideoWrapperRenderer": {
            "primaryRenderer": video("song", "MUSIC_VIDEO_TYPE_ATV"),
            "counterpart": [ {
                "counterpartRenderer": video("clip", "MUSIC_VIDEO_TYPE_OMV"),
                "segmentMap": { "segment": [ {
                    "primaryVideoStartTimeMilliseconds": "927",
                    "counterpartVideoStartTimeMilliseconds": "0",
                    "durationMilliseconds": "212500",
                } ] },
            } ],
        } } ] });
        let source = source_in(&answer, "song").expect("a video");
        assert_eq!(source.id, "clip");
        assert!((source.offset - 0.927).abs() < 1e-9);
    }

    #[test]
    fn a_music_video_is_its_own_picture_and_album_audio_has_none() {
        let clip = json!({ "contents": [ video("clip", "MUSIC_VIDEO_TYPE_OMV") ] });
        assert_eq!(
            source_in(&clip, "clip"),
            Some(VideoSource {
                id: "clip".to_string(),
                offset: 0.0
            })
        );
        let song = json!({ "contents": [ video("song", "MUSIC_VIDEO_TYPE_ATV") ] });
        assert_eq!(source_in(&song, "song"), None);
    }

    /// Downloads a music video's picture, signed out, and shows it 30 s in:
    /// `cargo test -- --ignored plays_a_real_video`.
    #[tokio::test]
    #[ignore = "downloads from YouTube"]
    async fn plays_a_real_video() {
        let file = crate::download::video("dQw4w9WgXcQ".to_string())
            .await
            .expect("download");
        let mut video = Video {
            on: true,
            song: Some(("song".to_string(), Status::Playing)),
            source: Some(VideoSource {
                id: "dQw4w9WgXcQ".to_string(),
                offset: 0.927,
            }),
            file: Some(file.clone()),
            ..Video::default()
        };
        let started = Instant::now();
        while video.frame.is_none() {
            video.step(30.0);
            assert!(started.elapsed().as_secs() < 10, "no frame from {file:?}");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(video.status(), Some(&Status::Playing));
    }

    /// Finds the music video of a song with the saved session, reading only:
    /// `cargo test -- --ignored finds_a_songs_video`.
    #[tokio::test]
    #[ignore = "asks YouTube Music with the saved session"]
    async fn finds_a_songs_video() {
        let rp = crate::youtube::client().expect("client");
        // Rick Astley's "Never Gonna Give You Up", the song, and its music video.
        let source = find(rp, "lYBUbBu4W08".to_string(), true).await;
        assert!(
            matches!(&source, Ok(Some(source)) if source.id == "dQw4w9WgXcQ"),
            "{source:?}"
        );
    }
}
