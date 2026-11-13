//! How the app reacts to messages: `App::update` and the helpers it calls.

use iced::{Task, window};

use crate::clock::Playback;
use crate::library::{Account, LibraryData, Shelf};
use crate::lyrics::Lyrics;
use crate::media_keys::MediaKey;
use crate::online::Poll;
use crate::sidebar::NavItem;
use crate::song::Song;
use crate::{
    App, Message, Screen, account, backdrop, clock, library, load_backdrop, load_lyrics, lyrics,
    lyrics_source, offline,
};

impl App {
    /// Applies one message to the state and returns the work it starts (downloads, requests),
    /// then tells the desktop's media controls what is playing.
    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);
        self.publish_media();
        task
    }

    /// Tells the desktop's media controls (MPRIS) about the song and the playback state.
    fn publish_media(&mut self) {
        let Some(media) = &mut self.media else {
            return;
        };
        let song = &self.now_playing;
        media.publish(
            &song.title,
            &song.artist,
            &song.album,
            song.art_url.as_deref(),
            self.playback.duration,
            self.playback.elapsed,
            self.playback.is_playing,
        );
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TogglePlay => {
                self.playback.toggle();
                if self.online.is_active() {
                    self.online.set_paused(!self.playback.is_playing);
                }
            }
            Message::Tick(now) => {
                self.now = now;
                if self.online.is_active() {
                    match self.online.poll() {
                        Poll::Position(seconds) => {
                            self.playback.elapsed = seconds;
                            if self.playback.duration == 0 {
                                self.playback.duration = self.online.duration().unwrap_or(0);
                            }
                        }
                        Poll::Finished => return self.play_next_online(),
                        Poll::Waiting => {}
                    }
                } else {
                    self.playback.tick();
                }
                self.follow_lyrics();
            }
            Message::Frame(now) => {
                self.now = now;
                // Keep the lyrics on the beat between the one-second ticks.
                if self.online.is_active() && self.playback.is_playing {
                    self.follow_lyrics();
                }
            }
            Message::ToggleSidebar => self.show_sidebar = !self.show_sidebar,
            Message::ToggleLyrics => self.show_lyrics = !self.show_lyrics,
            Message::ToggleSection(section) => self.sidebar.toggle(section),
            Message::Select(item) => return self.select(item),
            Message::ToggleLike => self.liked = !self.liked,
            Message::Next => {
                if self.online.is_active() {
                    return self.play_next_online();
                }
                self.next_track();
            }
            Message::WindowResized(width) => self.window_width = width,
            Message::SearchChanged(query) => return self.online.query_changed(query),
            Message::SuggestionsLoaded(request, result) => {
                self.online.suggestions_loaded(request, result);
            }
            Message::PickSuggestion(term) => {
                self.online.query = term;
                self.screen = Screen::Search;
                return self.online.search();
            }
            Message::Search => {
                self.screen = Screen::Search;
                return self.online.search();
            }
            Message::SearchFinished(result) => return self.online.search_finished(result),
            Message::SetFilter(filter) => {
                self.screen = Screen::Search;
                self.online.set_filter(filter);
            }
            Message::ImageFetched(url, bytes) => {
                if self.online.image_fetched(url.clone(), bytes) {
                    self.show_cover(&url);
                }
                if self.offline.wants_image(&url)
                    && let Some(bytes) = self.online.image_bytes(&url)
                {
                    self.offline.save_image(&url, bytes);
                }
            }
            Message::OpenTile(tile) => {
                self.screen = Screen::Collection;
                return self.online.open(tile);
            }
            Message::CollectionLoaded(result) => return self.online.collection_loaded(result),
            Message::PlayQueue(tracks, index) => return self.online.play_queue(tracks, index),
            Message::PlayTop => {
                let (opens, task) = self.online.play_top();
                if opens {
                    self.screen = Screen::Collection;
                }
                return task;
            }
            Message::AudioReady(index, path) => return self.start_online(index, path),
            Message::Prefetched => {}
            Message::Back => {
                self.screen = match self.screen {
                    Screen::Collection => Screen::Search,
                    _ => Screen::Home,
                };
            }
            Message::OpenAccount => return self.select(NavItem::Account),
            Message::SignInWith(browser) => {
                let Some(client) = self.online.client() else {
                    return Task::none();
                };
                self.account = Account::SigningIn;
                return Task::perform(
                    library::sign_in_with_browser(client, browser),
                    Message::SignedIn,
                );
            }
            Message::SignInDefault => {
                let Some(client) = self.online.client() else {
                    return Task::none();
                };
                let Some(browser) = self.default_browser else {
                    self.account = Account::Failed(
                        "could not tell which browser is the default; pick it below".to_string(),
                    );
                    return Task::none();
                };
                if let Err(error) = account::open_sign_in_page() {
                    self.account = Account::Failed(error);
                    return Task::none();
                }
                self.account = Account::Waiting(browser);
                return Task::perform(
                    library::wait_for_sign_in(client, browser),
                    Message::SignedIn,
                );
            }
            Message::CookiePathChanged(path) => self.cookie_path = path,
            Message::ImportCookies => {
                let Some(client) = self.online.client() else {
                    return Task::none();
                };
                self.account = Account::SigningIn;
                let cookies = account::read_from_file(&self.cookie_path);
                return Task::perform(library::sign_in(client, cookies), Message::SignedIn);
            }
            Message::SignedIn(result) => return self.library_loaded(result),
            Message::Restored(Ok(data)) => return self.library_loaded(Ok(data)),
            Message::Restored(Err(library::RestoreError::NoSession)) => {
                self.account = Account::SignedOut;
                library::clear_cache();
            }
            Message::Restored(Err(library::RestoreError::Failed(_))) => {
                // Probably offline: keep showing the saved library if there is one.
                if !matches!(self.account, Account::SignedIn(_)) {
                    self.account = Account::SignedOut;
                }
            }
            Message::SignOut => {
                self.account = Account::SignedOut;
                library::clear_cache();
                if let Some(client) = self.online.client() {
                    return Task::perform(library::sign_out(client), |()| Message::SignedOut);
                }
            }
            Message::SignedOut => {}
            Message::DownloadCollection(collection) => {
                let task = self.offline.start(collection);
                return self.save_offline_covers(task);
            }
            Message::OfflineTrackDone(result) => {
                let task = self.offline.track_done(result);
                return self.save_offline_covers(task);
            }
            Message::RemoveDownload(id) => self.offline.remove(&id),
            Message::LyricsLoaded(id, lyrics) => {
                if self
                    .online
                    .playing_track()
                    .is_some_and(|track| track.id == id)
                {
                    self.lyrics = lyrics;
                    self.follow_lyrics();
                }
            }
            Message::MediaKey(key) => {
                let playing = self.playback.is_playing;
                return match key {
                    MediaKey::Toggle => self.handle(Message::TogglePlay),
                    MediaKey::Play if !playing => self.handle(Message::TogglePlay),
                    MediaKey::Pause if playing => self.handle(Message::TogglePlay),
                    MediaKey::Next => self.handle(Message::Next),
                    MediaKey::Play | MediaKey::Pause => Task::none(),
                };
            }
            Message::ToggleImmersive => return self.set_immersive(!self.immersive),
            Message::ExitImmersive => return self.set_immersive(false),
            Message::OpenOffline(id) => {
                if let Some(collection) = self.offline.get(&id).cloned() {
                    self.screen = Screen::Collection;
                    return self.online.show_collection(collection);
                }
            }
        }
        Task::none()
    }

    /// Shows a YouTube track in the player once its audio has started.
    pub(crate) fn start_online(
        &mut self,
        index: usize,
        path: Result<std::path::PathBuf, String>,
    ) -> Task<Message> {
        let (started, prefetch) = self.online.audio_ready(index, path);
        let Some(track) = started else {
            return prefetch;
        };
        let cover = track
            .cover_url
            .as_deref()
            .and_then(|url| self.online.image(url));
        self.now_playing = Song::from_search(&track, cover);
        self.backdrop = track
            .cover_url
            .as_deref()
            .and_then(|url| self.online.image_bytes(url))
            .and_then(backdrop::blurred_backdrop);
        let duration = track.duration;
        self.playback = Playback {
            elapsed: 0,
            duration,
            is_playing: true,
        };
        self.lyrics = Lyrics::Loading;
        self.follow_lyrics();
        let id = track.id.clone();
        let lyrics = Task::perform(
            lyrics_source::fetch(track, self.online.client()),
            move |lyrics| Message::LyricsLoaded(id.clone(), lyrics),
        );
        Task::batch([prefetch, lyrics])
    }

    /// Enters or leaves the immersive view, switching the window to full screen and back.
    pub(crate) fn set_immersive(&mut self, immersive: bool) -> Task<Message> {
        if self.immersive == immersive {
            return Task::none();
        }
        self.immersive = immersive;
        let mode = if immersive {
            window::Mode::Fullscreen
        } else {
            window::Mode::Windowed
        };
        window::latest().and_then(move |id| window::set_mode(id, mode))
    }

    /// Milliseconds into the song: from mpv for real songs, from the clock for samples.
    pub(crate) fn position_ms(&self) -> u32 {
        let from_clock = self.playback.elapsed * 1000;
        if self.online.is_active() {
            self.online.position_ms().unwrap_or(from_clock)
        } else {
            from_clock
        }
    }

    /// Once a download finishes, saves its covers so they show without a connection. Covers that
    /// are not in memory yet are downloaded again and saved when they arrive.
    pub(crate) fn save_offline_covers(&mut self, task: Task<Message>) -> Task<Message> {
        let Some(collection) = self.offline.take_finished() else {
            return task;
        };
        let mut missing = Vec::new();
        for url in offline::image_urls(&collection) {
            match self.online.image_bytes(url) {
                Some(bytes) => self.offline.save_image(url, bytes),
                None => missing.push(url.to_string()),
            }
        }
        Task::batch([task, self.online.refetch_images(missing)])
    }

    /// Highlights a sidebar entry and shows its screen.
    pub(crate) fn select(&mut self, item: NavItem) -> Task<Message> {
        self.sidebar.selected = item;
        self.screen = match item {
            NavItem::History => Screen::Library(Shelf::History),
            NavItem::Songs => Screen::Library(Shelf::Songs),
            NavItem::LikedMusic => Screen::Library(Shelf::Liked),
            NavItem::Albums => Screen::Library(Shelf::Albums),
            NavItem::Artists => Screen::Library(Shelf::Artists),
            NavItem::Downloads => Screen::Library(Shelf::Downloads),
            NavItem::Account => Screen::Account,
            NavItem::Playlist(index) => {
                if let Account::SignedIn(data) = &self.account
                    && let Some(playlist) = data.playlists.get(index).cloned()
                {
                    self.screen = Screen::Collection;
                    return self.online.open(playlist);
                }
                Screen::Home
            }
            _ => Screen::Home,
        };
        Task::none()
    }

    /// Shows the library after signing in or restoring a session, saves it for the next start
    /// and downloads its images. A failed sign-in shows the error.
    pub(crate) fn library_loaded(&mut self, result: Result<LibraryData, String>) -> Task<Message> {
        match result {
            Ok(data) => {
                library::save_cache(&data);
                let images = self.online.fetch_images(data.image_urls());
                self.account = Account::SignedIn(Box::new(data));
                images
            }
            Err(error) => {
                self.account = Account::Failed(error);
                Task::none()
            }
        }
    }

    /// Puts a freshly downloaded cover on the player and behind the home greeting.
    pub(crate) fn show_cover(&mut self, url: &str) {
        self.now_playing.cover = self.online.image(url);
        self.backdrop = self
            .online
            .image_bytes(url)
            .and_then(backdrop::blurred_backdrop);
    }

    /// Plays the next song of the queue, if there is one.
    pub(crate) fn play_next_online(&mut self) -> Task<Message> {
        match self.online.next_index() {
            Some(next) => self.online.play(next),
            None => {
                self.playback.is_playing = false;
                Task::none()
            }
        }
    }

    /// Moves to the next sample track: new cover, new background and the clock back at zero.
    pub(crate) fn next_track(&mut self) {
        self.current = clock::next_index(self.current, self.tracks.len());
        let track = &self.tracks[self.current];
        self.playback.restart(track.duration);
        self.backdrop = load_backdrop(track);
        self.lyrics = load_lyrics(track);
        self.now_playing = Song::from_mock(track);
        self.follow_lyrics();
    }

    /// Starts scrolling the lyrics panel when the current line changes.
    pub(crate) fn follow_lyrics(&mut self) {
        let line =
            lyrics::current_line(self.lyrics.synced(), self.position_ms()).unwrap_or(0) as f32;
        if line != self.lyrics_scroll.value() {
            self.lyrics_scroll.go_mut(line, self.now);
        }
    }
}
