//! Encore: a YouTube Music desktop player written with iced.

mod account;
mod audio;
mod backdrop;
mod clock;
mod collection;
mod download;
mod home;
mod immersive;
mod layout;
mod library;
mod lyrics;
mod lyrics_source;
mod mock;
mod offline;
mod online;
mod player;
mod profile;
mod search;
mod sidebar;
mod song;
mod theme;
mod top_bar;
mod youtube;

use std::fs;
use std::time::Duration;

use iced::animation::{Animation, Easing};
use iced::time::Instant;
use iced::widget::{column, container, image, row};
use iced::{Element, Fill, Subscription, Task, Theme, time, window};

use account::Browser;
use clock::Playback;
use library::{Account, LibraryData, Shelf};
use lyrics::Lyrics;
use mock::{Card, Playlist, Track};
use offline::Offline;
use online::{Online, Poll};
use search::Filter;
use sidebar::{NavItem, Section, SidebarAccount, SidebarState};
use song::Song;
use youtube::{Collection, SearchPage, SearchTrack, Tile};

fn main() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title("Encore")
        .subscription(App::subscription)
        .theme(Theme::Dark)
        .font(include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/Inter-SemiBold.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/Inter-Bold.ttf").as_slice())
        .default_font(theme::INTER)
        .window_size((1280.0, 800.0))
        .run()
}

/// Everything the app remembers between frames.
struct App {
    tracks: Vec<Track>,
    sidebar: SidebarState,
    playback: Playback,
    show_sidebar: bool,
    show_lyrics: bool,
    playlists: Vec<Playlist>,
    recents: Vec<Card>,
    mixes: Vec<Card>,
    backdrop: Option<image::Handle>,
    liked: bool,
    current: usize,
    lyrics: Lyrics,
    window_width: f32,
    /// When the app started; the background zoom is measured from here.
    started: Instant,
    /// The time of the latest tick or animation frame.
    now: Instant,
    /// The lyric line the panel is scrolled to, animated when it changes.
    lyrics_scroll: Animation<f32>,
    /// What the center column shows.
    screen: Screen,
    /// Search, downloads and audio.
    online: Online,
    /// The track shown in the player bar.
    now_playing: Song,
    /// Signed in or not, and the library once signed in.
    account: Account,
    /// The cookies.txt path typed on the account screen.
    cookie_path: String,
    /// The system's default browser, for "Sign in with your browser".
    default_browser: Option<Browser>,
    /// Downloaded collections and the download in progress.
    offline: Offline,
    /// Whether the immersive (full screen) view is showing.
    immersive: bool,
}

/// The screens of the center column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Search,
    Collection,
    Library(Shelf),
    Account,
}

/// Everything that can happen in the app.
#[derive(Debug, Clone)]
enum Message {
    TogglePlay,
    Tick(Instant),
    Frame(Instant),
    ToggleSidebar,
    ToggleLyrics,
    ToggleSection(Section),
    Select(NavItem),
    ToggleLike,
    Next,
    WindowResized(f32),
    SearchChanged(String),
    SuggestionsLoaded(u64, Result<Vec<String>, String>),
    PickSuggestion(String),
    Search,
    SearchFinished(Result<SearchPage, String>),
    SetFilter(Filter),
    ImageFetched(String, Result<Vec<u8>, String>),
    OpenTile(Tile),
    CollectionLoaded(Result<Collection, String>),
    PlayQueue(Vec<SearchTrack>, usize),
    PlayTop,
    AudioReady(usize, Result<std::path::PathBuf, String>),
    Prefetched,
    Back,
    OpenAccount,
    SignInDefault,
    SignInWith(Browser),
    CookiePathChanged(String),
    ImportCookies,
    SignedIn(Result<LibraryData, String>),
    Restored(Result<LibraryData, library::RestoreError>),
    SignOut,
    SignedOut,
    DownloadCollection(Collection),
    OfflineTrackDone(Result<std::path::PathBuf, String>),
    RemoveDownload(String),
    OpenOffline(String),
    LyricsLoaded(String, Lyrics),
    ToggleImmersive,
    ExitImmersive,
}

impl App {
    /// The initial state, plus restoring a saved session in the background.
    fn boot() -> (Self, Task<Message>) {
        let mut app = Self::new();
        app.online.seed_images(app.offline.saved_images());
        // Show the library saved by the last session at once; it reloads in the background.
        let cached_images = match library::load_cache() {
            Some(cached) => {
                let images = app.online.fetch_images(cached.image_urls());
                app.account = Account::SignedIn(Box::new(cached));
                images
            }
            None => Task::none(),
        };
        let restore = match app.online.client() {
            Some(client) => {
                if !matches!(app.account, Account::SignedIn(_)) {
                    app.account = Account::Checking;
                }
                Task::perform(library::restore(client), Message::Restored)
            }
            None => Task::none(),
        };
        (app, Task::batch([cached_images, restore]))
    }

    fn new() -> Self {
        let tracks = mock::parse_tracks(include_str!("../assets/mock/tracks.tsv"));
        let first = tracks.first().expect("tracks.tsv lists at least one track");
        let duration = first.duration;
        let backdrop = load_backdrop(first);
        let lyrics = load_lyrics(first);
        let elapsed = 93;
        let first_line = lyrics::current_line(lyrics.synced(), elapsed * 1000).unwrap_or(0);
        let now = Instant::now();
        let now_playing = Song::from_mock(first);

        Self {
            tracks,
            playlists: mock::parse_playlists(include_str!("../assets/mock/playlists.tsv")),
            recents: mock::parse_cards(include_str!("../assets/mock/recents.tsv")),
            mixes: mock::parse_cards(include_str!("../assets/mock/mixes.tsv")),
            backdrop,
            sidebar: SidebarState::new(),
            playback: Playback {
                elapsed,
                duration,
                is_playing: false,
            },
            show_sidebar: true,
            show_lyrics: true,
            liked: true,
            current: 0,
            lyrics,
            window_width: 1280.0,
            started: now,
            now,
            lyrics_scroll: Animation::new(first_line as f32)
                .duration(Duration::from_millis(500))
                .easing(Easing::EaseOutCubic),
            screen: Screen::Home,
            online: Online::new(),
            now_playing,
            account: Account::SignedOut,
            cookie_path: String::new(),
            default_browser: account::default_browser(),
            offline: Offline::load(),
            immersive: false,
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
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
    fn start_online(
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
    fn set_immersive(&mut self, immersive: bool) -> Task<Message> {
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
    fn position_ms(&self) -> u32 {
        let from_clock = self.playback.elapsed * 1000;
        if self.online.is_active() {
            self.online.position_ms().unwrap_or(from_clock)
        } else {
            from_clock
        }
    }

    /// Once a download finishes, saves its covers so they show without a connection. Covers that
    /// are not in memory yet are downloaded again and saved when they arrive.
    fn save_offline_covers(&mut self, task: Task<Message>) -> Task<Message> {
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
    fn select(&mut self, item: NavItem) -> Task<Message> {
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
    fn library_loaded(&mut self, result: Result<LibraryData, String>) -> Task<Message> {
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

    /// Who is signed in, if the library knows.
    fn profile(&self) -> Option<&profile::Profile> {
        match &self.account {
            Account::SignedIn(data) => data.profile.as_ref(),
            _ => None,
        }
    }

    /// The profile photo, once downloaded.
    fn profile_photo(&self) -> Option<image::Handle> {
        let url = self.profile()?.photo_url.as_deref()?;
        self.online.image(url)
    }

    /// Puts a freshly downloaded cover on the player and behind the home greeting.
    fn show_cover(&mut self, url: &str) {
        self.now_playing.cover = self.online.image(url);
        self.backdrop = self
            .online
            .image_bytes(url)
            .and_then(backdrop::blurred_backdrop);
    }

    /// Plays the next song of the queue, if there is one.
    fn play_next_online(&mut self) -> Task<Message> {
        match self.online.next_index() {
            Some(next) => self.online.play(next),
            None => {
                self.playback.is_playing = false;
                Task::none()
            }
        }
    }

    /// Moves to the next sample track: new cover, new background and the clock back at zero.
    fn next_track(&mut self) {
        self.current = clock::next_index(self.current, self.tracks.len());
        let track = &self.tracks[self.current];
        self.playback.restart(track.duration);
        self.backdrop = load_backdrop(track);
        self.lyrics = load_lyrics(track);
        self.now_playing = Song::from_mock(track);
        self.follow_lyrics();
    }

    /// Starts scrolling the lyrics panel when the current line changes.
    fn follow_lyrics(&mut self) {
        let line =
            lyrics::current_line(self.lyrics.synced(), self.position_ms()).unwrap_or(0) as f32;
        if line != self.lyrics_scroll.value() {
            self.lyrics_scroll.go_mut(line, self.now);
        }
    }

    fn view(&self) -> Element<'_, Message> {
        use chrono::Timelike;

        if self.immersive {
            let seconds = self.now.duration_since(self.started).as_secs_f32();
            return immersive::view(
                &self.now_playing,
                self.backdrop.as_ref(),
                backdrop::zoom_at(seconds),
                self.lyrics_view(lyrics::IMMERSIVE),
                player::view(&self.now_playing, &self.playback, self.liked),
            );
        }

        let hour = chrono::Local::now().hour();
        let columns =
            layout::visible_columns(self.window_width, self.show_sidebar, self.show_lyrics);
        let seconds = self.now.duration_since(self.started).as_secs_f32();
        let content = match self.screen {
            Screen::Home => {
                let sections = match &self.account {
                    Account::SignedIn(data) => home::Sections::Personal {
                        history: &data.history,
                        mixes: &data.mixes,
                        playlists: &data.playlists,
                        albums: &data.albums,
                        shared: self.online.shared_view(),
                    },
                    _ => home::Sections::Sample {
                        recents: &self.recents,
                        mixes: &self.mixes,
                    },
                };
                home::view(
                    hour,
                    self.backdrop.as_ref(),
                    backdrop::zoom_at(seconds),
                    sections,
                )
            }
            Screen::Search => search::view(self.online.results()),
            Screen::Collection => {
                let (collection, status, shared) = self.online.collection();
                collection::view(collection, status, shared, &self.offline)
            }
            Screen::Library(shelf) => library::shelf_view(
                shelf,
                &self.account,
                self.offline.collections(),
                self.online.shared_view(),
            ),
            Screen::Account => library::account_view(
                &self.account,
                &self.cookie_path,
                self.default_browser,
                self.profile_photo(),
            ),
        };

        let center = container(column![
            top_bar::view(columns.sidebar, columns.lyrics, self.screen != Screen::Home),
            content,
            player::view(&self.now_playing, &self.playback, self.liked)
        ])
        .width(Fill)
        .height(Fill)
        .style(|_| theme::panel(theme::CENTER));

        row![]
            .push(columns.sidebar.then(|| {
                sidebar::view(
                    &self.sidebar,
                    &self.playlists,
                    &self.online.query,
                    self.online.suggestions(),
                    SidebarAccount {
                        signed_in: matches!(self.account, Account::SignedIn(_)),
                        profile: self.profile(),
                        photo: self.profile_photo(),
                        playlists: match &self.account {
                            Account::SignedIn(data) => Some(&data.playlists),
                            _ => None,
                        },
                        highlight: matches!(
                            self.screen,
                            Screen::Home | Screen::Library(_) | Screen::Account
                        ),
                    },
                )
            }))
            .push(center)
            .push(columns.lyrics.then(|| self.lyrics_view(lyrics::PANEL)))
            .into()
    }

    /// The lyrics in `style`, scrolled to the current line.
    fn lyrics_view(&self, style: lyrics::Style) -> Element<'_, Message> {
        let current = lyrics::current_line(self.lyrics.synced(), self.position_ms());
        let scroll = self.lyrics_scroll.interpolate_with(|line| line, self.now);
        lyrics::view(&self.lyrics, current, scroll, style)
    }

    fn subscription(&self) -> Subscription<Message> {
        let resize = window::resize_events().map(|(_id, size)| Message::WindowResized(size.width));
        // Escape leaves the immersive view.
        let escape = if self.immersive {
            iced::keyboard::listen().filter_map(|event| match event {
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                } => Some(Message::ExitImmersive),
                _ => None,
            })
        } else {
            Subscription::none()
        };
        let resize = Subscription::batch([resize, escape]);

        // About 30 frames per second, only while something is moving.
        let frames = time::every(Duration::from_millis(33)).map(Message::Frame);

        if self.playback.is_playing {
            let tick = time::every(Duration::from_secs(1)).map(Message::Tick);
            Subscription::batch([tick, frames, resize])
        } else if self.lyrics_scroll.is_animating(self.now) {
            Subscription::batch([frames, resize])
        } else {
            resize
        }
    }
}

/// Reads the track's cover from disk and blurs it for the home background.
fn load_backdrop(track: &Track) -> Option<image::Handle> {
    let bytes = fs::read(mock::cover_path(&track.cover)).ok()?;
    backdrop::blurred_backdrop(&bytes)
}

/// Reads and parses the track's lyrics. A track without lyrics, or with a file that cannot be
/// read, gets no lines.
fn load_lyrics(track: &Track) -> Lyrics {
    let Some(file) = &track.lyrics else {
        return Lyrics::Missing;
    };
    fs::read_to_string(mock::lyrics_path(file))
        .map(|text| Lyrics::Synced(lyrics::parse_lrc(&text)))
        .unwrap_or(Lyrics::Missing)
}
