//! Everything that talks to YouTube Music: suggestions, searching, opening albums, artists and
//! playlists and downloading images. The audio player and its queue are in `playback`.

mod playback;

use std::collections::{HashMap, HashSet};

use iced::Task;
use iced::widget::image;
use rust_i18n::t;
use rustypipe::client::RustyPipe;

use crate::Message;
use crate::audio_settings::AudioSettings;
use crate::download;
use crate::mixer::Mixer;
use crate::queue::{Queue, Repeat};
use crate::search::{Filter, Results, Shared, Status};
use crate::silence::SongFile;
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
    mixer: Option<Mixer>,
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
    queue: Queue,
    /// Whether new queues start shuffled; mirrors the shuffle button.
    shuffle: bool,
    rng: fastrand::Rng,
    loading: Option<String>,
    /// The next song's audio once it is downloaded, ready to crossfade into.
    next_file: Option<(String, SongFile)>,
    /// Whether mpv reported a position for the current song; until then idle means "opening".
    heard: bool,
    error: Option<String>,
    /// The repeat mode, which decides the song that comes next.
    repeat: Repeat,
    /// The song queued in mpv to follow the current one without a gap.
    queued: Option<usize>,
    /// Whether similar songs are added when the queue runs out.
    autoplay: bool,
    /// Whether songs start and end where their music does.
    skip_silence: bool,
    /// How good the downloaded audio is.
    quality: download::Quality,
    /// Songs being downloaded ahead of time, so each is asked for once.
    warming: HashSet<String>,
    /// The file of the song that is playing, for the visualizer.
    playing_file: Option<std::path::PathBuf>,
    /// The song whose radio autoplay last asked for, so it is asked only once.
    radio_for: Option<String>,
    /// The covers the user picked for playlists, by playlist id.
    covers: HashMap<String, image::Handle>,
}

impl Online {
    /// Starts the YouTube client and mpv with `settings`. If either fails, searching or
    /// playing shows an error.
    pub fn new(settings: &AudioSettings) -> Self {
        Self {
            client: youtube::client().ok(),
            mixer: Mixer::new(settings).ok(),
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
            queue: Queue::default(),
            shuffle: settings.shuffle,
            rng: fastrand::Rng::new(),
            loading: None,
            next_file: None,
            heard: false,
            error: None,
            repeat: settings.repeat,
            queued: None,
            autoplay: settings.autoplay,
            skip_silence: settings.skip_silence,
            quality: settings.quality,
            playing_file: None,
            warming: HashSet::new(),
            radio_for: None,
            covers: crate::playlists::covers::load(),
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
            self.status = Status::Failed(t!("online.client_not_started").into_owned());
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
            self.collection_status = Status::Failed(t!("online.client_not_started").into_owned());
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
                    .chain(collection.artist.iter().flat_map(|page| page.image_urls()))
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

    /// An image as raw bytes, for the blurred background.
    pub fn image_bytes(&self, url: &str) -> Option<&[u8]> {
        self.image_bytes.get(url).map(Vec::as_slice)
    }

    /// An image, ready to draw.
    pub fn image(&self, url: &str) -> Option<image::Handle> {
        self.images.get(url).cloned()
    }

    /// Sets or (with `None`) forgets the cover the user picked for playlist `id`.
    pub fn set_cover(&mut self, id: String, cover: Option<image::Handle>) {
        match cover {
            Some(cover) => self.covers.insert(id, cover),
            None => self.covers.remove(&id),
        };
    }

    /// Shows `song` at the end of playlist `id`, if it is the open collection.
    pub fn song_added(&mut self, id: &str, song: SearchTrack) {
        if let Some(collection) = self.open_playlist(id)
            && !collection.tracks.iter().any(|track| track.id == song.id)
        {
            collection.tracks.push(song);
        }
    }

    /// Takes song `song_id` out of playlist `id`, if it is the open collection.
    pub fn song_removed(&mut self, id: &str, song_id: &str) {
        if let Some(collection) = self.open_playlist(id) {
            collection.tracks.retain(|track| track.id != song_id);
        }
    }

    fn open_playlist(&mut self, id: &str) -> Option<&mut Collection> {
        let bare = crate::playlists::edit::bare_id;
        self.collection
            .as_mut()
            .filter(|collection| bare(&collection.id) == bare(id))
    }

    fn shared(&self) -> Shared<'_> {
        Shared {
            images: &self.images,
            covers: &self.covers,
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
