mod clock;
mod lyrics;
mod mock;
mod player;
mod sidebar;
mod theme;
mod top_bar;

use std::time::Duration;

use iced::widget::{column, container, row, text};
use iced::{Element, Fill, Subscription, Theme, time};

use clock::Playback;
use mock::{Playlist, Track};
use sidebar::{NavItem, Section, SidebarState};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Encore")
        .subscription(App::subscription)
        .theme(Theme::Dark)
        .window_size((1280.0, 800.0))
        .run()
}

struct App {
    tracks: Vec<Track>,
    sidebar: SidebarState,
    playback: Playback,
    show_sidebar: bool,
    show_lyrics: bool,
    playlists: Vec<Playlist>,
}

#[derive(Debug, Clone)] // step 10: derive
enum Message {
    TogglePlay,
    Tick,
    ToggleSidebar,
    ToggleLyrics,
    ToggleSection(Section),
    Select(NavItem),
}

impl App {
    fn new() -> Self {
        let tracks = mock::parse_tracks(include_str!("../assets/mock/tracks.tsv"));
        let duration = tracks[0].duration;

        Self {
            tracks,
            playlists: mock::parse_playlists(include_str!("../assets/mock/playlists.tsv")),
            sidebar: SidebarState::new(),
            playback: Playback {
                elapsed: 93,
                duration,
                is_playing: false,
            },
            show_sidebar: true,
            show_lyrics: true,
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
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // step 11: the '_ lifetime
        let home = container(text("Encore").size(32)).center(Fill);

        let center = container(column![
            top_bar::view(),
            home,
            player::view(&self.tracks[0], &self.playback)
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
