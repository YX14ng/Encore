mod clock;
mod theme;

use std::time::Duration;

use iced::widget::{button, column, container, row, text};
use iced::{Center, Element, Fill, Subscription, Theme, time};

use clock::Playback;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Encore")
        .subscription(App::subscription)
        .theme(Theme::Dark)
        .window_size((1280.0, 800.0))
        .run()
}

struct App {
    playback: Playback,
}

#[derive(Debug, Clone)] // step 10: derive
enum Message {
    TogglePlay,
    Tick,
}

impl App {
    fn new() -> Self {
        Self {
            playback: Playback {
                elapsed: 93,
                duration: 214,
                is_playing: false,
            },
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::TogglePlay => self.playback.toggle(),
            Message::Tick => self.playback.tick(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // step 11: the '_ lifetime
        let label = if self.playback.is_playing {
            "Pause"
        } else {
            "Play"
        };
        let elapsed = self.playback.elapsed;
        let left = clock::remaining(elapsed, self.playback.duration);

        let content = column![
            text("Encore").size(32),
            text(format!(
                "{} / -{}",
                clock::format_time(elapsed),
                clock::format_time(left)
            )),
            button(label).on_press(Message::TogglePlay),
        ]
        .spacing(16)
        .align_x(Center);

        let sidebar = container(text("Sidebar"))
            .width(236)
            .height(Fill)
            .padding(16)
            .style(|_| theme::panel(theme::SIDEBAR));

        let center = container(content)
            .center(Fill)
            .style(|_| theme::panel(theme::CENTER));

        let lyrics = container(text("Lyrics"))
            .width(300)
            .height(Fill)
            .padding(16)
            .style(|_| theme::panel(theme::LYRICS));

        row![sidebar, center, lyrics].into()
    }

    fn subscription(&self) -> Subscription<Message> {
        if self.playback.is_playing {
            time::every(Duration::from_secs(1)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        }
    }
}
