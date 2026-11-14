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
mod notifications;
mod offline;
mod online;
mod player;
mod plugins;
mod profile;
mod queue;
mod queue_view;
mod scrobbler;
mod search;
mod settings_view;
mod sidebar;
mod song;
mod sponsorblock;
mod theme;
mod themes;
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
        .theme(App::theme)
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
    /// The right panel, if it is open.
    panel: Option<layout::Panel>,
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
    /// The chosen theme and accent.
    appearance: themes::Appearance,
    /// The built-in and custom themes.
    themes: Vec<themes::Theme>,
    /// Custom theme files that could not be read.
    theme_errors: Vec<String>,
    /// The most colorful color of the playing song's cover, for the "from cover" accent.
    cover_color: Option<iced::Color>,
    /// Which optional features are on.
    plugins: plugins::Plugins,
    /// The playing video's SponsorBlock segments, by video id.
    skip_segments: Option<(String, Vec<sponsorblock::Segment>)>,
}

/// The screens of the center column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Search,
    Collection,
    Library(Shelf),
    Account,
    Settings(settings_view::Tab),
}

/// Everything that can happen in the app.
#[derive(Debug, Clone)]
enum Message {
    TogglePlay,
    Tick(Instant),
    Frame(Instant),
    ToggleSidebar,
    /// Opens the right panel with this content, or closes it if it already shows it.
    TogglePanel(layout::Panel),
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
    AddToQueue(SearchTrack),
    QueueJump(usize),
    QueuePlayNext(usize),
    QueueRemove(usize),
    QueueClear,
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
    OpenSettings(settings_view::Tab),
    PickTheme(String),
    PickAccent(themes::Accent),
    ReloadThemes,
    SetPlugin(plugins::Plugin, bool),
    SegmentsLoaded(String, Result<Vec<sponsorblock::Segment>, String>),
    NotificationShown,
    AudioChanged(audio_settings::AudioChange),
    AudioChangeDone,
}

impl App {
    /// iced's own widgets (scrollbars, text inputs, switches) follow the palette's lightness.
    fn theme(&self) -> Theme {
        if theme::palette().light {
            Theme::Light
        } else {
            Theme::Dark
        }
    }

    /// The initial state, plus restoring a saved session in the background.
    fn boot() -> (Self, Task<Message>) {
        let mut app = Self::new();
        app.apply_appearance();
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
        let (backdrop, cover_color) = load_cover(first);
        let (themes, theme_errors) = themes::load_all();
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
            panel: Some(layout::Panel::Lyrics),
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
            appearance: themes::Appearance::load(),
            themes,
            theme_errors,
            cover_color,
            plugins: plugins::Plugins::load(),
            skip_segments: None,
        }
    }
}

/// Reads the sample track's cover from disk: blurred for the home background, and its most
/// colorful color for the accent.
fn load_cover(track: &Track) -> (Option<image::Handle>, Option<iced::Color>) {
    cover_looks(fs::read(mock::cover_path(&track.cover)).ok().as_deref())
}

/// What a cover gives the interface: the blurred background and the accent color.
fn cover_looks(bytes: Option<&[u8]>) -> (Option<image::Handle>, Option<iced::Color>) {
    match bytes {
        Some(bytes) => (
            backdrop::blurred_backdrop(bytes),
            themes::cover_color(bytes),
        ),
        None => (None, None),
    }
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
