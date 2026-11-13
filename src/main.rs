//! Encore: a YouTube Music desktop player written with iced.

mod backdrop;
mod clock;
mod home;
mod lyrics;
mod mock;
mod player;
mod sidebar;
mod theme;
mod top_bar;

use std::fs;
use std::time::Duration;

use iced::widget::{column, container, image, row};
use iced::{Element, Fill, Subscription, Theme, time};

use clock::Playback;
use mock::{Card, Playlist, Track};
use sidebar::{NavItem, Section, SidebarState};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Encore")
        .subscription(App::subscription)
        .theme(Theme::Dark)
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
}

/// Everything that can happen in the app.
#[derive(Debug, Clone)]
enum Message {
    TogglePlay,
    Tick,
    ToggleSidebar,
    ToggleLyrics,
    ToggleSection(Section),
    Select(NavItem),
    ToggleLike,
    Next,
}

impl App {
    fn new() -> Self {
        let tracks = mock::parse_tracks(include_str!("../assets/mock/tracks.tsv"));
        let first = tracks.first().expect("tracks.tsv lists at least one track");
        let duration = first.duration;
        let backdrop = load_backdrop(first);

        Self {
            tracks,
            playlists: mock::parse_playlists(include_str!("../assets/mock/playlists.tsv")),
            recents: mock::parse_cards(include_str!("../assets/mock/recents.tsv")),
            mixes: mock::parse_cards(include_str!("../assets/mock/mixes.tsv")),
            backdrop,
            sidebar: SidebarState::new(),
            playback: Playback {
                elapsed: 93,
                duration,
                is_playing: false,
            },
            show_sidebar: true,
            show_lyrics: true,
            liked: true,
            current: 0,
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::TogglePlay => self.playback.toggle(),
            Message::Tick => self.playback.tick(),
            Message::ToggleSidebar => self.show_sidebar = !self.show_sidebar,
            Message::ToggleLyrics => self.show_lyrics = !self.show_lyrics,
            Message::ToggleSection(section) => self.sidebar.toggle(section),
            Message::Select(item) => self.sidebar.selected = item,
            Message::ToggleLike => self.liked = !self.liked,
            Message::Next => self.next_track(),
        }
    }

    /// Moves to the next sample track: new cover, new background and the clock back at zero.
    fn next_track(&mut self) {
        self.current = clock::next_index(self.current, self.tracks.len());
        let track = &self.tracks[self.current];
        self.playback.restart(track.duration);
        self.backdrop = load_backdrop(track);
    }

    fn view(&self) -> Element<'_, Message> {
        use chrono::Timelike;

        let hour = chrono::Local::now().hour();
        let home = home::view(hour, self.backdrop.as_ref(), &self.recents, &self.mixes);

        let center = container(column![
            top_bar::view(),
            home,
            player::view(&self.tracks[self.current], &self.playback, self.liked)
        ])
        .width(Fill)
        .height(Fill)
        .style(|_| theme::panel(theme::CENTER));

        row![]
            .push(
                self.show_sidebar
                    .then(|| sidebar::view(&self.sidebar, &self.playlists)),
            )
            .push(center)
            .push(self.show_lyrics.then(lyrics::view))
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.playback.is_playing {
            time::every(Duration::from_secs(1)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        }
    }
}

/// Reads the track's cover from disk and blurs it for the home background.
fn load_backdrop(track: &Track) -> Option<image::Handle> {
    let bytes = fs::read(mock::cover_path(&track.cover)).ok()?;
    backdrop::blurred_backdrop(&bytes)
}
