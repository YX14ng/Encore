//! Encore: a YouTube Music desktop player written with iced.

mod backdrop;
mod clock;
mod home;
mod layout;
mod lyrics;
mod mock;
mod player;
mod sidebar;
mod theme;
mod top_bar;

use std::fs;
use std::time::Duration;

use iced::animation::{Animation, Easing};
use iced::time::Instant;
use iced::widget::{column, container, image, row};
use iced::{Element, Fill, Subscription, Theme, time, window};

use clock::Playback;
use lyrics::LyricLine;
use mock::{Card, Playlist, Track};
use sidebar::{NavItem, Section, SidebarState};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
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
}

impl App {
    fn new() -> Self {
        let tracks = mock::parse_tracks(include_str!("../assets/mock/tracks.tsv"));
        let first = tracks.first().expect("tracks.tsv lists at least one track");
        let duration = first.duration;
        let backdrop = load_backdrop(first);
        let lyrics = load_lyrics(first);
        let elapsed = 93;
        let first_line = lyrics::current_line(&lyrics, elapsed).unwrap_or(0);
        let now = Instant::now();

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
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::TogglePlay => self.playback.toggle(),
            Message::Tick(now) => {
                self.now = now;
                self.playback.tick();
                self.follow_lyrics();
            }
            Message::Frame(now) => self.now = now,
            Message::ToggleSidebar => self.show_sidebar = !self.show_sidebar,
            Message::ToggleLyrics => self.show_lyrics = !self.show_lyrics,
            Message::ToggleSection(section) => self.sidebar.toggle(section),
            Message::Select(item) => self.sidebar.selected = item,
            Message::ToggleLike => self.liked = !self.liked,
            Message::Next => self.next_track(),
            Message::WindowResized(width) => self.window_width = width,
        }
    }

    /// Moves to the next sample track: new cover, new background and the clock back at zero.
    fn next_track(&mut self) {
        self.current = clock::next_index(self.current, self.tracks.len());
        let track = &self.tracks[self.current];
        self.playback.restart(track.duration);
        self.backdrop = load_backdrop(track);
        self.lyrics = load_lyrics(track);
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
        let home = home::view(
            hour,
            self.backdrop.as_ref(),
            backdrop::zoom_at(seconds),
            &self.recents,
            &self.mixes,
        );

        let center = container(column![
            top_bar::view(columns.sidebar, columns.lyrics),
            home,
            player::view(&self.tracks[self.current], &self.playback, self.liked)
        ])
        .width(Fill)
        .height(Fill)
        .style(|_| theme::panel(theme::CENTER));

        row![]
            .push(
                columns
                    .sidebar
                    .then(|| sidebar::view(&self.sidebar, &self.playlists)),
            )
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
