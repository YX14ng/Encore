use iced::widget::{button, column, container, text};
use iced::{Center, Element, Fill};

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("Encore")
        .window_size((1280.0, 800.0))
        .run()
}

#[derive(Default)]
struct App {
    is_playing: bool,
}

#[derive(Debug, Clone)]
enum Message {
    TogglePlay,
}

impl App {
    fn update(&mut self, message: Message) {
        match message {
            Message::TogglePlay => self.is_playing = !self.is_playing,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let label = if self.is_playing { "Pause" } else { "Play" };

        let content = column![
            text("Encore").size(32),
            button(label).on_press(Message::TogglePlay),
        ]
        .spacing(16)
        .align_x(Center);

        container(content).center(Fill).into()
    }
}
