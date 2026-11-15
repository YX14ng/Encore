//! The now playing page, like Pear's (YouTube Music's player page): it slides up over the
//! window when the song in the player bar is clicked (or the video button is), with the music
//! video or the cover in the middle, a Song / Video switch above it, and the queue and the
//! lyrics in tabs beside it. The player bar stays below.

use std::time::{Duration, Instant};

use iced::font::Weight;
use iced::widget::{Space, button, column, container, image, row, space, stack, text};
use iced::{Center, Color, ContentFit, Element, Fill, Font, Task, border};

use crate::{App, Message, lyrics, queue_view, theme};

/// How long the page takes to slide up.
const SLIDE: Duration = Duration::from_millis(220);
/// How far below its place the page starts sliding from.
const SLIDE_FROM: f32 = 160.0;
/// The width of the tabs beside the video.
const TABS_WIDTH: f32 = 420.0;
/// The largest the cover is drawn when there is no video.
const COVER_MAX: f32 = 560.0;

/// The tabs beside the video.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    UpNext,
    Lyrics,
}

/// The open page.
#[derive(Debug, Clone, Copy)]
pub struct Page {
    pub tab: Tab,
    opened: Instant,
}

/// What happens on the page.
#[derive(Debug, Clone, Copy)]
pub enum Event {
    /// The song in the player bar was clicked: opens the page, or closes it.
    Toggle,
    Close,
    Tab(Tab),
    /// The Song / Video switch.
    ShowVideo(bool),
}

impl App {
    pub(crate) fn now_playing_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Toggle if self.page.is_some() => return self.close_page(),
            Event::Toggle => self.open_page(),
            Event::Close => return self.close_page(),
            Event::Tab(tab) => {
                if let Some(page) = &mut self.page {
                    page.tab = tab;
                }
            }
            Event::ShowVideo(on) => {
                self.open_page();
                if on != self.video.on {
                    self.video.on = on;
                    return self.find_video();
                }
            }
        }
        Task::none()
    }

    /// Opens the page (if it is not open) on the queue.
    pub(crate) fn open_page(&mut self) {
        if self.page.is_none() {
            self.page = Some(Page {
                tab: Tab::UpNext,
                opened: Instant::now(),
            });
        }
    }

    fn close_page(&mut self) -> Task<Message> {
        self.page = None;
        if !self.immersive {
            self.stop_video();
        }
        Task::none()
    }

    /// Whether the page is still sliding up, so frames keep coming.
    pub(crate) fn page_sliding(&self) -> bool {
        self.page
            .is_some_and(|page| self.now.saturating_duration_since(page.opened) < SLIDE)
    }

    /// The page, over the blurred cover; the player bar goes below it.
    pub(crate) fn page_view(&self, page: Page) -> Element<'_, Message> {
        let collapse = button(theme::icon(
            include_bytes!("../assets/icons/chevron-down.svg"),
            theme::text_primary(),
            22.0,
        ))
        .on_press(Message::NowPlaying(Event::Close))
        .padding(8)
        .style(|_, status| theme::flat_button(status, false));

        let stage = column![
            row![
                collapse,
                space::horizontal(),
                switch(self.video.on),
                space::horizontal()
            ]
            .push(Space::new().width(38))
            .align_y(Center),
            self.media(),
        ]
        .spacing(12)
        .width(Fill)
        .height(Fill);

        let tabs = column![tab_bar(page.tab), self.tab_content(page.tab)]
            .spacing(8)
            .width(TABS_WIDTH)
            .height(Fill);

        // Eased: fast at first, settling into place.
        let progress = (self
            .now
            .saturating_duration_since(page.opened)
            .as_secs_f32()
            / SLIDE.as_secs_f32())
        .min(1.0);
        let offset = SLIDE_FROM * (1.0 - progress).powi(3);
        let content = row![stage, tabs]
            .spacing(32)
            .padding(iced::Padding::from([20, 28]).top(16.0 + offset));

        stack![self.page_background(), content].into()
    }

    /// The music video when it plays, else the cover (with a line about the video if it was
    /// asked for).
    fn media(&self) -> Element<'_, Message> {
        if self.video.on
            && let Some(frame) = self.video.frame()
        {
            return image(frame.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Contain)
                .into();
        }
        let cover: Element<'_, Message> = match &self.now_playing.cover {
            Some(handle) => image(handle.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Contain)
                .border_radius(12)
                .into(),
            None => container(space())
                .width(COVER_MAX)
                .height(COVER_MAX)
                .style(|_| theme::panel(theme::hero()).border(border::rounded(12)))
                .into(),
        };
        let cover = container(cover).max_width(COVER_MAX).max_height(COVER_MAX);
        let note = self
            .video
            .on
            .then(|| self.video.note())
            .flatten()
            .map(|note| text(note).size(14).color(theme::text_secondary()));
        let title = text(&self.now_playing.title).size(24).font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        });
        let artist = text(format!(
            "{} \u{b7} {}",
            self.now_playing.artist, self.now_playing.album
        ))
        .size(15)
        .color(theme::text_secondary());
        container(
            column![cover, title, artist]
                .push(note)
                .spacing(10)
                .align_x(Center),
        )
        .center(Fill)
        .into()
    }

    fn tab_content(&self, tab: Tab) -> Element<'_, Message> {
        match tab {
            Tab::UpNext => queue_view::list(
                self.online.queue(),
                self.online.autoplay(),
                self.online.shared_view(),
            ),
            Tab::Lyrics => container(self.lyrics_view(lyrics::PAGE))
                .center_x(Fill)
                .height(Fill)
                .into(),
        }
    }

    /// The blurred cover, darkened, or black while a video plays.
    fn page_background(&self) -> Element<'_, Message> {
        let video = self.video.on && self.video.frame().is_some();
        let blurred: Element<'_, Message> = match (&self.backdrop, video) {
            (Some(handle), false) => image(handle.clone())
                .width(Fill)
                .height(Fill)
                .content_fit(ContentFit::Cover)
                .into(),
            _ => container(space())
                .width(Fill)
                .height(Fill)
                .style(|_| container::background(theme::center_bg()))
                .into(),
        };
        let shade = container(space()).width(Fill).height(Fill).style(move |_| {
            let alpha = if video { 0.85 } else { 0.55 };
            container::background(Color::from_rgba8(0, 0, 0, alpha))
        });
        stack![blurred, shade].into()
    }
}

/// The Song / Video switch, like YouTube Music's.
fn switch(video: bool) -> Element<'static, Message> {
    let side = |label: &'static str, selected: bool, on: bool| {
        button(text(label).size(13))
            .on_press(Message::NowPlaying(Event::ShowVideo(on)))
            .padding([6, 18])
            .style(move |_, status| {
                let mut style = theme::flat_button(status, false);
                style.border = border::rounded(16);
                if selected {
                    style.background = Some(theme::tint(0.16).into());
                    style.text_color = theme::text_primary();
                }
                style
            })
    };
    container(row![side("Song", !video, false), side("Video", video, true)].spacing(2))
        .padding(3)
        .style(|_| {
            container::background(theme::tint(0.06))
                .border(border::rounded(19).color(theme::line()).width(1))
        })
        .into()
}

/// "Up next" and "Lyrics", the open one underlined.
fn tab_bar(open: Tab) -> Element<'static, Message> {
    let tab = |label: &'static str, tab: Tab| {
        let selected = open == tab;
        let underline = container(space()).width(Fill).height(2).style(move |_| {
            container::background(if selected {
                theme::text_primary()
            } else {
                theme::line()
            })
        });
        button(
            column![
                text(label)
                    .size(15)
                    .color(if selected {
                        theme::text_primary()
                    } else {
                        theme::text_secondary()
                    })
                    .width(Fill)
                    .align_x(Center),
                underline
            ]
            .spacing(10),
        )
        .on_press(Message::NowPlaying(Event::Tab(tab)))
        .padding(0)
        .width(Fill)
        .style(|_, status| {
            let mut style = theme::flat_button(status, false);
            style.background = None;
            style
        })
    };
    row![tab("Up next", Tab::UpNext), tab("Lyrics", Tab::Lyrics)]
        .padding(iced::Padding::default().top(10))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_song_in_the_bar_opens_and_closes_the_page() {
        let mut app = App::new();
        let _ = app.now_playing_event(Event::Toggle);
        assert_eq!(app.page.map(|page| page.tab), Some(Tab::UpNext));
        let _ = app.now_playing_event(Event::Tab(Tab::Lyrics));
        assert_eq!(app.page.map(|page| page.tab), Some(Tab::Lyrics));
        let _ = app.now_playing_event(Event::Toggle);
        assert!(app.page.is_none());
    }

    #[test]
    fn the_video_switch_opens_the_page_and_closing_it_stops_the_video() {
        let mut app = App::new();
        let _ = app.now_playing_event(Event::ShowVideo(true));
        assert!(app.page.is_some() && app.video.on);
        let _ = app.now_playing_event(Event::Close);
        assert!(!app.video.on);
    }
}
