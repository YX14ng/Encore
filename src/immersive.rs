//! The immersive view: the whole window (full screen) for the song that is playing, with a big
//! cover over the blurred background and large lyrics beside it, like Cider's.

use iced::font::Weight;
use iced::widget::{button, column, container, image, row, space, stack, text};
use iced::{Center, Color, ContentFit, Element, Fill, Font, border};

use rust_i18n::t;

use crate::song::Song;
use crate::{Message, theme};

/// The size of the cover.
const COVER_SIZE: f32 = 380.0;

/// The immersive view around the large `lyrics` and the `player` bar. With a `video`, the music
/// video takes the place of the cover and the lyrics; `bars` (the visualizer) go above the
/// player.
pub fn view<'a>(
    song: &'a Song,
    backdrop: Option<&'a image::Handle>,
    zoom: f32,
    lyrics: Element<'a, Message>,
    video: Option<Element<'a, Message>>,
    bars: Option<Element<'a, Message>>,
    player: Element<'a, Message>,
) -> Element<'a, Message> {
    let background: Element<'a, Message> = match backdrop {
        Some(handle) => image(handle.clone())
            .width(Fill)
            .height(Fill)
            .content_fit(ContentFit::Cover)
            .scale(zoom)
            .into(),
        None => container(space())
            .width(Fill)
            .height(Fill)
            .style(|_| theme::panel(theme::hero()))
            .into(),
    };
    // Darkens the blurred cover so the text stays readable over bright artwork.
    let shade = container(space())
        .width(Fill)
        .height(Fill)
        .style(|_| container::background(Color::from_rgba8(0, 0, 0, 0.45)));

    let cover: Element<'a, Message> = match &song.cover {
        Some(handle) => image(handle.clone())
            .width(COVER_SIZE)
            .height(COVER_SIZE)
            .content_fit(ContentFit::Cover)
            .border_radius(14)
            .into(),
        None => container(space())
            .width(COVER_SIZE)
            .height(COVER_SIZE)
            .style(|_| theme::panel(theme::hero()).border(border::rounded(14)))
            .into(),
    };

    let exit = button(
        text(t!("immersive.exit"))
            .size(13)
            .color(theme::text_secondary()),
    )
    .on_press(Message::ToggleImmersive)
    .padding([6, 14])
    .style(|_, status| {
        let mut style = theme::flat_button(status, false);
        style.border = border::rounded(16).color(theme::line()).width(1);
        style
    });

    if let Some(video) = video {
        let caption = row![
            column![
                text(&song.title).size(22).font(Font {
                    weight: Weight::Bold,
                    ..theme::INTER
                }),
                text(format!("{} \u{b7} {}", song.artist, song.album))
                    .size(15)
                    .color(theme::text_secondary()),
            ]
            .spacing(4),
            space::horizontal(),
            exit,
        ]
        .align_y(Center);
        let stage = column![container(video).height(Fill).width(Fill), caption]
            .spacing(16)
            .padding([24, 48]);
        let backdrop = container(space())
            .width(Fill)
            .height(Fill)
            .style(|_| container::background(Color::BLACK));
        return stack![backdrop, column![stage, player]].into();
    }

    let details = column![
        cover,
        text(&song.title).size(30).font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        }),
        text(format!("{} \u{b7} {}", song.artist, song.album))
            .size(17)
            .color(theme::text_secondary()),
        exit,
    ]
    .spacing(14)
    .width(COVER_SIZE);

    let stage = row![space::horizontal(), details, lyrics, space::horizontal()]
        .spacing(64)
        .align_y(Center)
        .height(Fill)
        .padding([40, 48]);

    let bars = bars.map(|bars| container(bars).padding([0, 48]));
    stack![background, shade, column![stage].push(bars).push(player)].into()
}
