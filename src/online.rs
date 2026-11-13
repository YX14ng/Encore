//! Everything that talks to YouTube Music: suggestions, searching, opening albums, artists and
//! playlists, downloading images and songs, and the audio player with its queue.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use iced::Task;
use iced::widget::image;
use rustypipe::client::RustyPipe;

use crate::Message;
use crate::audio::Audio;
use crate::download;
use crate::search::{Filter, Results, Shared, Status};
use crate::youtube::{self, Collection, SearchPage, SearchTrack, Tile, TopResult};

/// What the audio player reports on each tick.
pub enum Poll {
    /// Seconds into the song.
    Position(u32),
    /// The song ended.
    Finished,
    /// Nothing to report yet (still opening the file).
    Waiting,
}

/// The online half of the app.
pub struct Online {
    client: Option<RustyPipe>,
    audio: Option<Audio>,
    pub query: String,
    suggestions: Vec<String>,
    /// Bumped on every keystroke, so late suggestions for old text are dropped.
    suggestion_request: u64,
    status: Status,
    page: SearchPage,
    filter: Filter,
    collection: Option<Collection>,
    collection_status: Status,
    /// Play the opened collection as soon as it loads (the top result's Play button).
    play_when_loaded: bool,
    images: HashMap<String, image::Handle>,
    image_bytes: HashMap<String, Vec<u8>>,
    requested_images: HashSet<String>,
    queue: Vec<SearchTrack>,
    playing: Option<usize>,
    loading: Option<String>,
    /// Whether mpv reported a position for the current song; until then idle means "opening".
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
            suggestions: Vec::new(),
            suggestion_request: 0,
            status: Status::Idle,
            page: SearchPage::default(),
            filter: Filter::All,
            collection: None,
            collection_status: Status::Idle,
            play_when_loaded: false,
            images: HashMap::new(),
            image_bytes: HashMap::new(),
            requested_images: HashSet::new(),
            queue: Vec::new(),
            playing: None,
            loading: None,
            heard: false,
            error: None,
        }
    }

    /// The YouTube client, if it started.
    pub fn client(&self) -> Option<RustyPipe> {
        self.client.clone()
    }

    /// Images and playback state for the library screens.
    pub fn shared_view(&self) -> Shared<'_> {
        self.shared()
    }

    /// Updates the query and asks YouTube Music for suggestions.
    pub fn query_changed(&mut self, query: String) -> Task<Message> {
        self.query = query;
        self.suggestion_request += 1;
        let typed = self.query.trim().to_string();
        let Some(client) = self.client.clone().filter(|_| !typed.is_empty()) else {
            self.suggestions.clear();
            return Task::none();
        };
        let request = self.suggestion_request;
        Task::perform(youtube::suggestions(client, typed), move |result| {
            Message::SuggestionsLoaded(request, result)
        })
    }

    /// Shows suggestions, unless the text changed since they were asked for.
    pub fn suggestions_loaded(&mut self, request: u64, result: Result<Vec<String>, String>) {
        if request == self.suggestion_request {
            self.suggestions = result.unwrap_or_default();
        }
    }

    /// The suggestions under the search box.
    pub fn suggestions(&self) -> &[String] {
        &self.suggestions
    }

    /// Runs a search for the current query.
    pub fn search(&mut self) -> Task<Message> {
        self.suggestion_request += 1;
        self.suggestions.clear();
        let query = self.query.trim().to_string();
        if query.is_empty() {
            return Task::none();
        }
        let Some(client) = self.client.clone() else {
            self.status = Status::Failed("the YouTube client could not start".to_string());
            return Task::none();
        };
        self.status = Status::Loading;
        self.filter = Filter::All;
        self.page = SearchPage::default();
        Task::perform(youtube::search_all(client, query), Message::SearchFinished)
    }

    /// Shows the results and starts downloading their images.
    pub fn search_finished(&mut self, result: Result<SearchPage, String>) -> Task<Message> {
        match result {
            Ok(page) => {
                self.status = Status::Done;
                self.page = page;
                let page = &self.page;
                let urls: Vec<String> = page
                    .top
                    .iter()
                    .filter_map(|top| match top {
                        TopResult::Track(track) => track.cover_url.clone(),
                        TopResult::Tile(tile) => tile.image_url.clone(),
                    })
                    .chain(
                        page.songs
                            .iter()
                            .chain(&page.videos)
                            .filter_map(|t| t.cover_url.clone()),
                    )
                    .chain(
                        page.albums
                            .iter()
                            .chain(&page.artists)
                            .chain(&page.playlists)
                            .filter_map(|tile| tile.image_url.clone()),
                    )
                    .collect();
                self.fetch_images(urls)
            }
            Err(error) => {
                self.status = Status::Failed(error);
                Task::none()
            }
        }
    }

    /// Switches the chip filter.
    pub fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
    }

    /// Adds images that are already on disk (covers of downloads).
    pub fn seed_images(&mut self, images: Vec<(String, Vec<u8>)>) {
        for (url, bytes) in images {
            self.images
                .insert(url.clone(), image::Handle::from_bytes(bytes.clone()));
            self.image_bytes.insert(url.clone(), bytes);
            self.requested_images.insert(url);
        }
    }

    /// Downloads images again even if they were asked for before (their bytes went missing).
    pub fn refetch_images(&mut self, urls: Vec<String>) -> Task<Message> {
        for url in &urls {
            self.requested_images.remove(url);
        }
        self.fetch_images(urls)
    }

    /// Downloads images that were not asked for yet.
    pub fn fetch_images(&mut self, urls: Vec<String>) -> Task<Message> {
        let tasks: Vec<_> = urls
            .into_iter()
            .filter(|url| self.requested_images.insert(url.clone()))
            .map(|url| {
                Task::perform(download::bytes(url.clone()), move |bytes| {
                    Message::ImageFetched(url.clone(), bytes)
                })
            })
            .collect();
        Task::batch(tasks)
    }

    /// Keeps a downloaded image. Returns `true` if it is the cover of the song that is playing.
    pub fn image_fetched(&mut self, url: String, bytes: Result<Vec<u8>, String>) -> bool {
        let Ok(bytes) = bytes else {
            self.requested_images.remove(&url);
            return false;
        };
        self.images
            .insert(url.clone(), image::Handle::from_bytes(bytes.clone()));
        self.image_bytes.insert(url.clone(), bytes);
        self.playing_track()
            .is_some_and(|track| track.cover_url.as_deref() == Some(url.as_str()))
    }

    /// Opens an album, artist or playlist.
    pub fn open(&mut self, tile: Tile) -> Task<Message> {
        let Some(client) = self.client.clone() else {
            self.collection_status =
                Status::Failed("the YouTube client could not start".to_string());
            return Task::none();
        };
        self.collection = None;
        self.collection_status = Status::Loading;
        self.play_when_loaded = false;
        Task::perform(youtube::open(client, tile), Message::CollectionLoaded)
    }

    /// Shows a collection that is already known (a download), without asking YouTube.
    pub fn show_collection(&mut self, collection: Collection) -> Task<Message> {
        self.play_when_loaded = false;
        self.collection_loaded(Ok(collection))
    }

    /// Shows an opened collection and downloads its images.
    pub fn collection_loaded(&mut self, result: Result<Collection, String>) -> Task<Message> {
        match result {
            Ok(collection) => {
                self.collection_status = Status::Done;
                let urls = collection
                    .image_url
                    .iter()
                    .cloned()
                    .chain(collection.tracks.iter().filter_map(|t| t.cover_url.clone()))
                    .collect();
                let tracks = collection.tracks.clone();
                self.collection = Some(collection);
                let images = self.fetch_images(urls);
                if std::mem::take(&mut self.play_when_loaded) && !tracks.is_empty() {
                    return Task::batch([images, self.play_queue(tracks, 0)]);
                }
                images
            }
            Err(error) => {
                self.collection_status = Status::Failed(error);
                Task::none()
            }
        }
    }

    /// The top result's Play button: plays the song, or opens and plays the collection.
    /// Returns `true` if a collection is being opened, so the app can show it.
    pub fn play_top(&mut self) -> (bool, Task<Message>) {
        match self.page.top.clone() {
            Some(TopResult::Track(track)) => {
                let mut queue = vec![track.clone()];
                queue.extend(
                    self.page
                        .songs
                        .iter()
                        .filter(|song| song.id != track.id)
                        .cloned(),
                );
                (false, self.play_queue(queue, 0))
            }
            Some(TopResult::Tile(tile)) => {
                let task = self.open(tile);
                self.play_when_loaded = true;
                (true, task)
            }
            None => (false, Task::none()),
        }
    }

    /// Replaces the queue with `tracks` and plays song `index`.
    pub fn play_queue(&mut self, tracks: Vec<SearchTrack>, index: usize) -> Task<Message> {
        self.queue = tracks;
        self.play(index)
    }

    /// Downloads song `index` of the queue (or takes it from the cache) so it can play.
    pub fn play(&mut self, index: usize) -> Task<Message> {
        let Some(track) = self.queue.get(index) else {
            return Task::none();
        };
        self.error = None;
        self.loading = Some(track.id.clone());
        Task::perform(download::audio(track.id.clone()), move |path| {
            Message::AudioReady(index, path)
        })
    }

    /// Starts the downloaded file. Returns the song that started, and prefetches the next one.
    pub fn audio_ready(
        &mut self,
        index: usize,
        path: Result<PathBuf, String>,
    ) -> (Option<SearchTrack>, Task<Message>) {
        let Some(track) = self.queue.get(index) else {
            return (None, Task::none());
        };
        if self.loading.as_deref() != Some(track.id.as_str()) {
            // Another song was picked while this one was downloading.
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
        let prefetch = self.queue.get(index + 1).map_or_else(Task::none, |next| {
            Task::perform(download::audio(next.id.clone()), |_| Message::Prefetched)
        });
        (self.queue.get(index).cloned(), prefetch)
    }

    /// Whether a YouTube song is (or was last) playing.
    pub fn is_active(&self) -> bool {
        self.playing.is_some()
    }

    /// The YouTube song that is playing.
    pub fn playing_track(&self) -> Option<&SearchTrack> {
        self.queue.get(self.playing?)
    }

    /// An image as raw bytes, for the blurred background.
    pub fn image_bytes(&self, url: &str) -> Option<&[u8]> {
        self.image_bytes.get(url).map(Vec::as_slice)
    }

    /// An image, ready to draw.
    pub fn image(&self, url: &str) -> Option<image::Handle> {
        self.images.get(url).cloned()
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

    /// The length of the playing song according to mpv, for lists that come without durations.
    pub fn duration(&self) -> Option<u32> {
        let seconds = self.audio.as_ref()?.duration()?;
        (seconds > 0.0).then_some(seconds as u32)
    }

    /// The song after the one that is playing, if any.
    pub fn next_index(&self) -> Option<usize> {
        let next = self.playing? + 1;
        (next < self.queue.len()).then_some(next)
    }

    fn shared(&self) -> Shared<'_> {
        Shared {
            images: &self.images,
            loading: self.loading.as_deref(),
            playing: self.playing_track().map(|track| track.id.as_str()),
        }
    }

    /// What the results screen draws.
    pub fn results(&self) -> Results<'_> {
        Results {
            query: self.query.trim(),
            status: &self.status,
            page: &self.page,
            filter: self.filter,
            error: self.error.as_deref(),
            shared: self.shared(),
        }
    }

    /// What the collection screen draws.
    pub fn collection(&self) -> (Option<&Collection>, &Status, Shared<'_>) {
        (
            self.collection.as_ref(),
            &self.collection_status,
            self.shared(),
        )
    }
}
