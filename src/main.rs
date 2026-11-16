//! Encore: a YouTube Music desktop player written with iced.

// The interface's texts in every language, from `locales/`; English fills any gap.
rust_i18n::i18n!("locales", fallback = "en");

mod account;
mod artist;
mod audio;
mod audio_settings;
mod audio_view;
mod backdrop;
mod browse;
mod clock;
mod collection;
mod discord;
mod discord_ipc;
mod download;
mod explore;
mod home;
mod i18n;
mod immersive;
mod lastfm;
mod layout;
mod library;
mod lyrics;
mod lyrics_source;
mod media_keys;
mod mini_player;
mod mixer;
mod mock;
mod navigation;
mod notifications;
mod now_playing;
mod offline;
mod online;
mod player;
mod playlists;
mod plugins;
mod profile;
mod queue;
mod queue_view;
mod rating;
mod renewal;
mod romanize;
mod scrobbler;
mod search;
mod session;
mod settings_view;
mod sidebar;
mod sign_in_window;
mod silence;
mod song;
mod sponsorblock;
mod theme;
mod themes;
mod together;
mod top_bar;
mod update;
mod video;
mod view;
mod visualizer;
mod volume;
mod youtube;

use std::fs;

use iced::animation::Animation;
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
    if std::env::args().nth(1).as_deref() == Some(sign_in_window::FLAG) {
        std::process::exit(sign_in_window::run());
    }
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
    /// The playing file's format for the player bar, like "OPUS 152".
    audio_format: Option<String>,
    /// In the mini player: the window and the size it had before.
    mini: Option<(iced::window::Id, iced::Size)>,
    /// The listen-together session, if any.
    together: together::Together,
    playlists: Vec<Playlist>,
    recents: Vec<Card>,
    mixes: Vec<Card>,
    backdrop: Option<image::Handle>,
    /// How the user rated the playing song on YouTube Music.
    rating: rating::Rating,
    /// The songs disliked in Encore, skipped when they come up again.
    disliked: rating::Disliked,
    /// The music video of the playing song, in the immersive view.
    video: video::Video,
    /// The now playing page, when it is open.
    page: Option<now_playing::Page>,
    /// The bars that move with the music in the immersive view.
    visualizer: visualizer::Visualizer,
    current: usize,
    lyrics: Lyrics,
    window_width: f32,
    /// When the app started; the background zoom is measured from here.
    started: Instant,
    /// The time of the latest tick or animation frame.
    now: Instant,
    /// The lyric line the panel is scrolled to, animated when it changes.
    lyrics_scroll: Animation<f32>,
    /// Where the user scrolled the lyrics by hand (a line index) and when; `None` follows the
    /// song.
    lyrics_browse: Option<(f32, Instant)>,
    /// The song shown in the player bar while it downloads, so the bar changes at once.
    loading_shown: Option<String>,
    /// How long YouTube Music sessions last, measured.
    session_log: session::SessionLog,
    /// YouTube stopped accepting the saved session; a banner asks to sign in again.
    session_expired: bool,
    /// The lyrics in the Latin alphabet, a line per line of `lyrics`, when they are in Japanese,
    /// Korean or Chinese and the plugin is on.
    romanized: Option<Vec<String>>,
    /// The part of a touchpad scroll over the volume bar not yet a whole step.
    volume_scroll: f32,
    /// Renewing the session while Encore is open.
    renewal: renewal::Renewal,
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
    /// The sound outputs offered on the audio screen, asked of mpv when it opens.
    audio_devices: Vec<audio::Device>,
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
    /// The playlist dialog over the window, if one is open.
    dialog: Option<playlists::Dialog>,
    /// The note at the bottom of the window and its number, to know when it goes.
    toast: Option<(u64, String)>,
    /// The screens Back goes to, the latest last, with the collection each one showed.
    history: Vec<(Screen, Option<youtube::Collection>)>,
    explore: explore::Explore,
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
    Together,
    Explore,
    /// A mood's or genre's page, opened from Explore.
    Genre,
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
    Rating(rating::Event),
    Video(video::Event),
    ToggleVideo,
    NowPlaying(now_playing::Event),
    Visualizer(visualizer::Event),
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
    OpenTogether,
    Together(together::Event),
    Discord(discord_ipc::Event),
    ToggleMini,
    /// The window to shrink into the mini player, and its size to go back to.
    MiniEntered(iced::window::Id, iced::Size),
    PlayTop,
    AudioReady(usize, Result<silence::SongFile, String>),
    Prefetched(String, Result<silence::SongFile, String>),
    /// A song after the next one finished downloading ahead.
    WarmedUp(String),
    /// Similar songs for autoplay, for the song with this id.
    RadioLoaded(String, Result<Vec<youtube::SearchTrack>, String>),
    ToggleAutoplay,
    Playlist(playlists::Event),
    Explore(explore::Event),
    Back,
    OpenAccount,
    SignInWindow,
    /// The sign-in window closed: the session's cookies, or why there are none.
    SignInWindowClosed(Result<String, String>),
    SignInWith(Browser),
    CookiePathChanged(String),
    ImportCookies,
    SignedIn(Result<LibraryData, String>),
    Restored(Result<LibraryData, library::RestoreError>),
    SignOut,
    SignedOut,
    Renewal(renewal::Event),
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
    /// The lyrics were scrolled by hand, this many lines down (up when negative).
    LyricsScrolled(f32),
    /// A line of the synced lyrics was clicked.
    LyricLine(usize),
    Seeking(f32),
    SeekDone,
    VolumeChanged(f32),
    VolumeDone,
    /// The mouse wheel turned over the volume bar.
    VolumeScrolled(iced::mouse::ScrollDelta),
    ToggleMute,
    ToggleShuffle,
    CycleRepeat,
    OpenSettings(settings_view::Tab),
    PickTheme(String),
    PickAccent(themes::Accent),
    /// A language picked on the appearance screen; `None` follows the system.
    PickLanguage(Option<String>),
    ReloadThemes,
    SetPlugin(plugins::Plugin, bool),
    SegmentsLoaded(String, Result<Vec<sponsorblock::Segment>, String>),
    NotificationShown,
    AudioChanged(audio_settings::AudioChange),
    AudioChangeDone,
    /// A sound output picked on the audio screen (mpv's name), or `None` for the default.
    AudioOutput(Option<String>),
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
        // The language comes first: the YouTube client below asks in it.
        let appearance = themes::Appearance::load();
        i18n::apply(appearance.language.as_deref());
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
            together: together::Together::load(),
            audio_format: None,
            mini: None,
            panel: Some(layout::Panel::Lyrics),
            rating: rating::Rating::None,
            disliked: rating::Disliked::load(),
            video: video::Video::default(),
            page: None,
            visualizer: visualizer::Visualizer::default(),
            current: 0,
            lyrics,
            window_width: 1280.0,
            started: now,
            now,
            lyrics_scroll: lyrics::scroll_animation(first_line as f32),
            lyrics_browse: None,
            loading_shown: None,
            session_log: session::SessionLog::load(),
            session_expired: false,
            romanized: None,
            volume_scroll: 0.0,
            renewal: renewal::Renewal::default(),
            screen: Screen::Home,
            online: Online::new(&audio_settings),
            now_playing,
            account: Account::SignedOut,
            cookie_path: String::new(),
            offline: Offline::load(),
            immersive: false,
            media: media_keys::MediaKeys::start(),
            discord: discord::Discord::start(),
            lastfm: scrobbler::LastFm::load(),
            audio_settings,
            seeking: None,
            audio_error: None,
            audio_devices: Vec::new(),
            appearance,
            themes,
            theme_errors,
            cover_color,
            plugins: plugins::Plugins::load(),
            skip_segments: None,
            dialog: None,
            toast: None,
            history: Vec::new(),
            explore: explore::Explore::default(),
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
