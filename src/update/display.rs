//! The window and the looks: the full screen (immersive) view, the mini player, the cover's
//! colors and the theme.

use iced::widget::image;
use iced::{Color, Task, window};

use crate::{App, Message, cover_looks, offline, theme};

impl App {
    /// Enters or leaves the immersive view, switching the window to full screen and back.
    pub(crate) fn set_immersive(&mut self, immersive: bool) -> Task<Message> {
        if self.immersive == immersive {
            return Task::none();
        }
        self.immersive = immersive;
        if !immersive && self.page.is_none() {
            self.stop_video();
        }
        let mode = if immersive {
            window::Mode::Fullscreen
        } else {
            window::Mode::Windowed
        };
        window::latest().and_then(move |id| window::set_mode(id, mode))
    }

    /// Shrinks the window into the mini player (above other windows), or gives it back its size.
    pub(super) fn toggle_mini(&mut self) -> Task<Message> {
        if let Some((id, size)) = self.mini.take() {
            return Task::batch([
                window::set_level(id, window::Level::Normal),
                window::resize(id, size),
            ]);
        }
        let leave_immersive = self.set_immersive(false);
        let shrink = window::latest()
            .and_then(|id| window::size(id).map(move |size| Message::MiniEntered(id, size)));
        Task::batch([leave_immersive, shrink])
    }

    /// Once a download is asked for, saves its covers so they show without a connection.
    /// Covers that are not in memory yet are downloaded again and saved when they arrive.
    pub(crate) fn save_offline_covers(&mut self, task: Task<Message>) -> Task<Message> {
        let Some(collection) = self.offline.take_new() else {
            return task;
        };
        let mut missing = Vec::new();
        for url in offline::image_urls(&collection) {
            match self.online.image_bytes(url) {
                Some(bytes) => self.offline.save_image(url, &bytes),
                None => missing.push(url.to_string()),
            }
        }
        Task::batch([task, self.online.refetch_images(missing)])
    }

    /// Puts a freshly downloaded cover on the player and behind the home greeting.
    pub(crate) fn show_cover(&mut self, url: &str) {
        self.now_playing.cover = self.online.image(url);
        let looks = cover_looks(self.online.image_bytes(url).as_deref());
        self.set_cover_looks(looks);
    }

    /// Puts a cover's blurred background behind the home greeting and its color on the accent.
    pub(super) fn set_cover_looks(
        &mut self,
        (backdrop, color): (Option<image::Handle>, Option<Color>),
    ) {
        self.backdrop = backdrop;
        self.cover_color = color;
        self.apply_appearance();
    }

    /// Draws with the chosen theme and accent from the next frame on.
    pub(crate) fn apply_appearance(&mut self) {
        theme::set_palette(self.appearance.palette(&self.themes, self.cover_color));
    }
}
