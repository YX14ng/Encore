//! Everything that talks to YouTube Music: the search, the downloads and the audio player.

use std::collections::HashMap;
use std::path::PathBuf;

use iced::Task;
use iced::widget::image;
use rustypipe::client::RustyPipe;

use crate::Message;
use crate::audio::Audio;
use crate::search::{Results, SearchStatus};
use crate::youtube::{self, SearchTrack};

/// What the audio player reports on each tick.
pub enum Poll {
    /// Seconds into the track.
    Position(u32),
    /// The track ended.
    Finished,
    /// Nothing to report yet (still opening the file).
    Waiting,
}

/// The online half of the app.
pub struct Online {
    client: Option<RustyPipe>,
    audio: Option<Audio>,
    pub query: String,
    status: SearchStatus,
    results: Vec<SearchTrack>,
    covers: HashMap<String, image::Handle>,
    cover_bytes: HashMap<String, Vec<u8>>,
    loading: Option<String>,
    playing: Option<usize>,
    /// Whether mpv reported a position for the current track; until then idle means "opening".
    heard: bool,
    error: Option<String>,
}

impl Online {
    /// Starts the YouTube client and mpv. If either fails, searching or playing shows an error.
    pub fn new() -> Self {
        Self {
            client: youtube::client().ok(),
            audio: Audio::new().ok(),
            query: String::new(),
            status: SearchStatus::Idle,
            results: Vec::new(),
            covers: HashMap::new(),
            cover_bytes: HashMap::new(),
            loading: None,
            playing: None,
            heard: false,
            error: None,
        }
    }

    /// Runs a search for the current query.
    pub fn search(&mut self) -> Task<Message> {
        let query = self.query.trim().to_string();
        if query.is_empty() {
            return Task::none();
        }
        let Some(client) = self.client.clone() else {
            self.status = SearchStatus::Failed("the YouTube client could not start".to_string());
            return Task::none();
        };
        self.status = SearchStatus::Searching;
        self.results.clear();
        Task::perform(youtube::search(client, query), Message::SearchFinished)
    }

    /// Shows the results and starts downloading their covers.
    pub fn search_finished(&mut self, result: Result<Vec<SearchTrack>, String>) -> Task<Message> {
        match result {
            Ok(tracks) => {
                self.status = SearchStatus::Done;
                self.results = tracks;
                let downloads = self
                    .results
                    .iter()
                    .filter(|track| !self.covers.contains_key(&track.id))
                    .filter_map(|track| {
                        let id = track.id.clone();
                        let url = track.cover_url.clone()?;
                        Some(Task::perform(youtube::fetch_bytes(url), move |bytes| {
                            Message::CoverFetched(id.clone(), bytes)
                        }))
                    });
                Task::batch(downloads)
            }
            Err(error) => {
                self.status = SearchStatus::Failed(error);
                Task::none()
            }
        }
    }

    /// Keeps a downloaded cover. Returns `true` if it belongs to the track that is playing.
    pub fn cover_fetched(&mut self, id: String, bytes: Result<Vec<u8>, String>) -> bool {
        let Ok(bytes) = bytes else {
            return false;
        };
        self.covers
            .insert(id.clone(), image::Handle::from_bytes(bytes.clone()));
        self.cover_bytes.insert(id.clone(), bytes);
        self.playing_track().is_some_and(|track| track.id == id)
    }

    /// Downloads result `index` (or takes it from the cache) so it can play.
    pub fn play(&mut self, index: usize) -> Task<Message> {
        let Some(track) = self.results.get(index) else {
            return Task::none();
        };
        self.error = None;
        self.loading = Some(track.id.clone());
        Task::perform(youtube::download_audio(track.id.clone()), move |path| {
            Message::AudioReady(index, path)
        })
    }

    /// Starts the downloaded file. Returns the track that started, and prefetches the next one.
    pub fn audio_ready(
        &mut self,
        index: usize,
        path: Result<PathBuf, String>,
    ) -> (Option<SearchTrack>, Task<Message>) {
        let Some(track) = self.results.get(index) else {
            return (None, Task::none());
        };
        if self.loading.as_deref() != Some(track.id.as_str()) {
            // The user picked another song while this one was downloading.
            return (None, Task::none());
        }
        self.loading = None;

        let started = path.and_then(|path| {
            let audio = self
                .audio
                .as_ref()
                .ok_or("mpv could not start; is libmpv installed?")?;
            audio.play(&path)
        });
        if let Err(error) = started {
            self.error = Some(error);
            return (None, Task::none());
        }

        self.playing = Some(index);
        self.heard = false;
        let prefetch = self
            .results
            .get(index + 1)
            .map(|next| {
                Task::perform(youtube::download_audio(next.id.clone()), |_| {
                    Message::Prefetched
                })
            })
            .unwrap_or_else(Task::none);
        (self.results.get(index).cloned(), prefetch)
    }

    /// Whether a YouTube track is (or was last) playing.
    pub fn is_active(&self) -> bool {
        self.playing.is_some()
    }

    /// The YouTube track that is playing.
    pub fn playing_track(&self) -> Option<&SearchTrack> {
        self.results.get(self.playing?)
    }

    /// The cover of a track as raw bytes, for the blurred background.
    pub fn cover_bytes(&self, id: &str) -> Option<&[u8]> {
        self.cover_bytes.get(id).map(Vec::as_slice)
    }

    /// The cover of a track, ready to draw.
    pub fn cover(&self, id: &str) -> Option<image::Handle> {
        self.covers.get(id).cloned()
    }

    /// Pauses or resumes the audio.
    pub fn set_paused(&self, paused: bool) {
        if let Some(audio) = &self.audio {
            audio.set_paused(paused);
        }
    }

    /// Asks mpv where it is.
    pub fn poll(&mut self) -> Poll {
        let Some(audio) = &self.audio else {
            return Poll::Waiting;
        };
        if let Some(position) = audio.position() {
            self.heard = true;
            return Poll::Position(position as u32);
        }
        if self.heard && audio.is_idle() {
            self.heard = false;
            return Poll::Finished;
        }
        Poll::Waiting
    }

    /// The result after the one that is playing, if any.
    pub fn next_index(&self) -> Option<usize> {
        let next = self.playing? + 1;
        (next < self.results.len()).then_some(next)
    }

    /// The data the results screen draws.
    pub fn results(&self) -> Results<'_> {
        Results {
            query: self.query.trim(),
            status: &self.status,
            tracks: &self.results,
            covers: &self.covers,
            loading: self.loading.as_deref(),
            playing: self.playing_track().map(|track| track.id.as_str()),
            error: self.error.as_deref(),
        }
    }
}
