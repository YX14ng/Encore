//! Encore: a YouTube Music desktop player written with iced.

mod account;
mod audio;
mod audio_settings;
mod audio_view;
mod backdrop;
mod clock;
mod collection;
mod discord;
mod download;
mod home;
mod immersive;
mod lastfm;
mod layout;
mod library;
mod lyrics;
mod lyrics_source;
mod media_keys;
mod mixer;
mod mock;
mod offline;
mod online;
mod player;
mod profile;
mod queue;
mod scrobbler;
mod search;
mod sidebar;
mod song;
mod theme;
mod top_bar;
mod update;
mod view;
mod youtube;

use std::fs;
use std::time::Duration;

use iced::animation::{Animation, Easing};
use iced::time::Instant;
use iced::widget::image;
use iced::{Task, Theme};

use account::Browser;
use clock::Playback;
use library::{Account, LibraryData, Shelf};
use lyrics::Lyrics;
use mock::{Card, Playlist, Track};
use offline::Offline;
use online::Online;
use search::Filter;
use sidebar::{NavItem, Section, SidebarState};
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
    /// Encore's entry in the desktop's media controls, if there is a session bus.
    media: Option<media_keys::MediaKeys>,
    /// The "Listening to …" status on Discord.
    discord: discord::Discord,
    /// Scrobbling to Last.fm.
    lastfm: scrobbler::LastFm,
    /// Volume, shuffle and repeat, remembered between sessions.
    audio_settings: audio_settings::AudioSettings,
    /// Where the progress bar is being dragged to, in seconds.
    seeking: Option<f32>,
    /// What mpv said when it refused the audio filters, shown on the audio screen.
    audio_error: Option<String>,
}

/// The screens of the center column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Search,
    Collection,
    Library(Shelf),
    Account,
    Audio,
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
    Prefetched(String, Result<std::path::PathBuf, String>),
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
    MediaKey(media_keys::MediaKey),
    LastFmConnect,
    LastFmConnected(Result<lastfm::Session, String>),
    LastFmDisconnect,
    LastFmAnswered(Result<(), lastfm::ApiError>),
    Scrobbled(Result<(), lastfm::ApiError>),
    Previous,
    Seeking(f32),
    SeekDone,
    VolumeChanged(f32),
    VolumeDone,
    ToggleMute,
    ToggleShuffle,
    CycleRepeat,
    OpenAudio,
    AudioChanged(audio_settings::AudioChange),
    AudioChangeDone,
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
        let audio_settings = audio_settings::AudioSettings::load();

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
            online: Online::new(&audio_settings),
            now_playing,
            account: Account::SignedOut,
            cookie_path: String::new(),
            default_browser: account::default_browser(),
            offline: Offline::load(),
            immersive: false,
            media: media_keys::MediaKeys::start(),
            discord: discord::Discord::start(),
            lastfm: scrobbler::LastFm::load(),
            audio_settings,
            seeking: None,
            audio_error: None,
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
