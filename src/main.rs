use iced::widget::{container, text};
use iced::{Element, Fill};

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("Encore")
        .window_size((1280.0, 800.0))
        .run()
}

#[derive(Default)]
struct App;

#[derive(Debug, Clone)]
enum Message {}

impl App {
    fn update(&mut self, message: Message) {
        match message {}
    }

    fn view(&self) -> Element<'_, Message> {
        container(text("Encore")).center(Fill).into()
    }
}
