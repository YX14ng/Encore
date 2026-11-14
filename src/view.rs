//! What the app draws and listens to: `App::view` and `App::subscription`.

use std::time::Duration;

use iced::widget::{column, container, image, row};
use iced::{Element, Fill, Subscription, time, window};

use crate::library::Account;
use crate::sidebar::SidebarAccount;
use crate::{
    App, Message, Screen, backdrop, collection, home, immersive, layout, library, lyrics,
    media_keys, player, profile, search, sidebar, theme, top_bar,
};

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

    /// The whole window: sidebar, center column and lyrics, or the immersive view.
    pub(crate) fn view(&self) -> Element<'_, Message> {
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
            Screen::Account => column![
                library::account_view(
                    &self.account,
                    &self.cookie_path,
                    self.default_browser,
                    self.profile_photo(),
                ),
                self.lastfm.view()
            ]
            .into(),
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
    pub(crate) fn lyrics_view(&self, style: lyrics::Style) -> Element<'_, Message> {
        let current = lyrics::current_line(self.lyrics.synced(), self.position_ms());
        let scroll = self.lyrics_scroll.interpolate_with(|line| line, self.now);
        lyrics::view(&self.lyrics, current, scroll, style)
    }

    /// The clock while playing, animation frames while something moves, window resizes and,
    /// in the immersive view, the Escape key.
    pub(crate) fn subscription(&self) -> Subscription<Message> {
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
        let resize = Subscription::batch([resize, escape, Subscription::run(media_keys::events)]);

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
