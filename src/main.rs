//! Encore: a YouTube Music desktop player written with iced.

mod account;
mod audio;
mod backdrop;
mod clock;
mod collection;
mod download;
mod home;
mod layout;
mod library;
mod lyrics;
mod mock;
mod online;
mod player;
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
use lyrics::LyricLine;
use mock::{Card, Playlist, Track};
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
    lyrics: Vec<LyricLine>,
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
    Restored(Result<LibraryData, String>),
    SignOut,
    SignedOut,
}

impl App {
    /// The initial state, plus restoring a saved session in the background.
    fn boot() -> (Self, Task<Message>) {
        let mut app = Self::new();
        let restore = match app.online.client() {
            Some(client) => {
                app.account = Account::Checking;
                Task::perform(library::restore(client), Message::Restored)
            }
            None => Task::none(),
        };
        (app, restore)
    }

    fn new() -> Self {
        let tracks = mock::parse_tracks(include_str!("../assets/mock/tracks.tsv"));
        let first = tracks.first().expect("tracks.tsv lists at least one track");
        let duration = first.duration;
        let backdrop = load_backdrop(first);
        let lyrics = load_lyrics(first);
        let elapsed = 93;
        let first_line = lyrics::current_line(&lyrics, elapsed).unwrap_or(0);
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
            Message::Frame(now) => self.now = now,
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
            Message::SignedIn(result) => return self.library_loaded(result, true),
            Message::Restored(result) => return self.library_loaded(result, false),
            Message::SignOut => {
                self.account = Account::SignedOut;
                if let Some(client) = self.online.client() {
                    return Task::perform(library::sign_out(client), |()| Message::SignedOut);
                }
            }
            Message::SignedOut => {}
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
        self.lyrics = Vec::new();
        self.follow_lyrics();
        prefetch
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

    /// Shows the library after signing in or restoring a session, and downloads its images.
    /// A failed restore at startup just means "signed out"; a failed sign-in shows the error.
    fn library_loaded(
        &mut self,
        result: Result<LibraryData, String>,
        from_sign_in: bool,
    ) -> Task<Message> {
        match result {
            Ok(data) => {
                let images = self.online.fetch_images(data.image_urls());
                self.account = Account::SignedIn(data);
                images
            }
            Err(error) => {
                self.account = if from_sign_in {
                    Account::Failed(error)
                } else {
                    Account::SignedOut
                };
                Task::none()
            }
        }
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
        let line = lyrics::current_line(&self.lyrics, self.playback.elapsed).unwrap_or(0) as f32;
        if line != self.lyrics_scroll.value() {
            self.lyrics_scroll.go_mut(line, self.now);
        }
    }

    fn view(&self) -> Element<'_, Message> {
        use chrono::Timelike;

        let hour = chrono::Local::now().hour();
        let columns =
            layout::visible_columns(self.window_width, self.show_sidebar, self.show_lyrics);
        let seconds = self.now.duration_since(self.started).as_secs_f32();
        let content = match self.screen {
            Screen::Home => home::view(
                hour,
                self.backdrop.as_ref(),
                backdrop::zoom_at(seconds),
                &self.recents,
                &self.mixes,
            ),
            Screen::Search => search::view(self.online.results()),
            Screen::Collection => {
                let (collection, status, shared) = self.online.collection();
                collection::view(collection, status, shared)
            }
            Screen::Library(shelf) => {
                library::shelf_view(shelf, &self.account, self.online.shared_view())
            }
            Screen::Account => {
                library::account_view(&self.account, &self.cookie_path, self.default_browser)
            }
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
            .push(columns.lyrics.then(|| {
                let current = lyrics::current_line(&self.lyrics, self.playback.elapsed);
                let scroll = self.lyrics_scroll.interpolate_with(|line| line, self.now);
                lyrics::view(&self.lyrics, current, scroll)
            }))
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let resize = window::resize_events().map(|(_id, size)| Message::WindowResized(size.width));

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
fn load_lyrics(track: &Track) -> Vec<LyricLine> {
    let Some(file) = &track.lyrics else {
        return Vec::new();
    };
    fs::read_to_string(mock::lyrics_path(file))
        .map(|text| lyrics::parse_lrc(&text))
        .unwrap_or_default()
}
