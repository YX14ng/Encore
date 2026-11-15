//! What the app draws and listens to: `App::view` and `App::subscription`.

use std::time::Duration;

use iced::widget::{column, container, image, row, stack};
use iced::{Element, Fill, Subscription, time, window};

use crate::layout::Panel;
use crate::library::Account;
use crate::sidebar::SidebarAccount;
use crate::{
    App, Message, Screen, audio_view, backdrop, collection, discord, explore, home, immersive,
    layout, library, lyrics, media_keys, mini_player, player, playlists, profile, queue_view,
    search, settings_view, sidebar, theme, together, top_bar,
};

/// How tall the visualizer's bars get in the immersive view.
const VISUALIZER_HEIGHT: f32 = 90.0;

impl App {
    /// Who is signed in, if the library knows.
    pub(crate) fn profile(&self) -> Option<&profile::Profile> {
        match &self.account {
            Account::SignedIn(data) => data.profile.as_ref(),
            _ => None,
        }
    }

    /// The profile photo, once downloaded.
    pub(crate) fn profile_photo(&self) -> Option<image::Handle> {
        let url = self.profile()?.photo_url.as_deref()?;
        self.online.image(url)
    }

    /// The state of the player bar's buttons and sliders.
    fn player_controls(&self) -> player::Controls {
        let settings = &self.audio_settings;
        player::Controls {
            rating: self.rating,
            can_rate: self.can_rate(),
            video: self.video.on,
            can_video: self.online.is_active(),
            seeking: self.seeking,
            volume: settings.volume,
            muted: settings.muted,
            shuffle: settings.shuffle,
            repeat: settings.repeat,
            compact: self.player_width() < player::COMPACT_WIDTH,
        }
    }

    /// How wide the player bar is: the center column, or the whole window in full screen.
    fn player_width(&self) -> f32 {
        if self.immersive || self.page.is_some() {
            return self.window_width;
        }
        let columns =
            layout::visible_columns(self.window_width, self.show_sidebar, self.panel.is_some());
        layout::center_width(self.window_width, columns)
    }

    /// The whole window: sidebar, center column and lyrics, or the immersive view.
    pub(crate) fn view(&self) -> Element<'_, Message> {
        use chrono::Timelike;

        if self.mini.is_some() {
            return mini_player::view(
                &self.now_playing,
                &self.playback,
                self.player_controls(),
                self.backdrop.as_ref(),
            );
        }

        if self.immersive {
            let seconds = self.now.duration_since(self.started).as_secs_f32();
            return immersive::view(
                &self.now_playing,
                self.backdrop.as_ref(),
                backdrop::zoom_at(seconds),
                self.lyrics_view(lyrics::IMMERSIVE),
                self.video.on.then(|| self.video.view()),
                (self.plugins.visualizer && self.visualizer.is_showing())
                    .then(|| self.visualizer.view(VISUALIZER_HEIGHT)),
                player::view(
                    &self.now_playing,
                    &self.playback,
                    self.player_controls(),
                    self.audio_format.as_deref(),
                ),
            );
        }

        let hour = chrono::Local::now().hour();
        let columns =
            layout::visible_columns(self.window_width, self.show_sidebar, self.panel.is_some());
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
                    layout::center_width(self.window_width, columns),
                    self.backdrop.as_ref(),
                    backdrop::zoom_at(seconds),
                    sections,
                )
            }
            Screen::Search => search::view(self.online.results()),
            Screen::Collection => {
                let (collection, status, shared) = self.online.collection();
                let own = collection.is_some_and(|collection| self.is_own_playlist(&collection.id));
                collection::view(collection, status, shared, &self.offline, own)
            }
            Screen::Library(shelf) => library::shelf_view(
                shelf,
                &self.account,
                self.offline.collections(),
                self.online.shared_view(),
            ),
            Screen::Settings(tab) => settings_view::view(
                tab,
                match tab {
                    settings_view::Tab::Appearance => settings_view::appearance(
                        &self.appearance,
                        self.appearance.language.as_deref(),
                        &self.themes,
                        &self.theme_errors,
                        self.cover_color,
                    ),
                    settings_view::Tab::Audio => audio_view::view(
                        &self.audio_settings,
                        &self.audio_devices,
                        self.audio_error.as_deref(),
                    ),
                    settings_view::Tab::Plugins => self.plugins.view(),
                },
            ),
            Screen::Together => together::view(&self.together),
            Screen::Explore => explore::view(&self.explore, self.online.shared_view()),
            Screen::Genre => explore::genre_view(&self.explore, self.online.shared_view()),
            Screen::Account => column![
                library::account_view(&self.account, &self.cookie_path, self.profile_photo(),),
                self.lastfm.view()
            ]
            .into(),
        };

        let center = container(column![
            top_bar::view(
                columns.sidebar,
                self.panel.filter(|_| columns.panel),
                self.screen != Screen::Home,
                self.together.is_active(),
            ),
            content,
            player::view(
                &self.now_playing,
                &self.playback,
                self.player_controls(),
                self.audio_format.as_deref(),
            )
        ])
        .width(Fill)
        .height(Fill)
        .style(|_| theme::panel(theme::center_bg()));

        let main: Element<'_, Message> = if let Some(page) = self.page {
            // The page takes the window above the player bar.
            column![
                self.page_view(page),
                player::view(
                    &self.now_playing,
                    &self.playback,
                    self.player_controls(),
                    self.audio_format.as_deref(),
                )
            ]
            .into()
        } else {
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
                            covers: self.online.shared_view().covers,
                            highlight: matches!(
                                self.screen,
                                Screen::Home | Screen::Library(_) | Screen::Account
                            ),
                        },
                    )
                }))
                .push(center)
                .push(
                    self.panel
                        .filter(|_| columns.panel)
                        .map(|panel| match panel {
                            Panel::Lyrics => self.lyrics_view(lyrics::PANEL),
                            Panel::Queue => queue_view::view(
                                self.online.queue(),
                                self.online.autoplay(),
                                self.online.shared_view(),
                            ),
                        }),
                )
                .into()
        };

        stack![main]
            .push(
                self.toast
                    .as_ref()
                    .map(|(_, message)| playlists::toast(message)),
            )
            .push(self.dialog.as_ref().map(|dialog| {
                playlists::dialog(dialog, self.own_playlists(), self.online.shared_view())
            }))
            .into()
    }

    /// The lyrics in `style`, scrolled to the current line.
    pub(crate) fn lyrics_view(&self, style: lyrics::Style) -> Element<'_, Message> {
        let current = lyrics::current_line(self.lyrics.synced(), self.position_ms());
        let scroll = match self.lyrics_browse {
            Some((at, _)) => at,
            None => self.lyrics_scroll.interpolate_with(|line| line, self.now),
        };
        lyrics::view(&self.lyrics, current, scroll, style)
    }

    /// The clock while playing, animation frames while something moves, window resizes and,
    /// in the immersive view, the Escape key.
    pub(crate) fn subscription(&self) -> Subscription<Message> {
        let resize = window::resize_events().map(|(_id, size)| Message::WindowResized(size.width));
        // Escape leaves the immersive view.
        // Escape leaves the immersive view, else closes the now playing page.
        let escape = if self.immersive {
            iced::keyboard::listen().filter_map(|event| match event {
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                } => Some(Message::ExitImmersive),
                _ => None,
            })
        } else if self.page.is_some() {
            iced::keyboard::listen().filter_map(|event| match event {
                iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                } => Some(Message::NowPlaying(crate::now_playing::Event::Close)),
                _ => None,
            })
        } else {
            Subscription::none()
        };
        let resize = Subscription::batch([
            resize,
            escape,
            Subscription::run(media_keys::events),
            Subscription::run(discord::events),
            self.together.subscription().map(Message::Together),
        ]);

        // About 30 frames per second, only while something is moving.
        let frames = time::every(Duration::from_millis(33)).map(Message::Frame);

        if self.playback.is_playing {
            let tick = time::every(Duration::from_secs(1)).map(Message::Tick);
            Subscription::batch([tick, frames, resize])
        } else if self.lyrics_scroll.is_animating(self.now)
            || (self.immersive && self.visualizer.is_showing())
            || self.page_sliding()
            || self.lyrics_browse.is_some()
        {
            Subscription::batch([frames, resize])
        } else {
            resize
        }
    }
}
