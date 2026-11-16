//! The mini player: the window shrinks to the cover, the song and the controls, and stays
//! above other windows until the user expands it again.

use iced::font::Weight;
use iced::widget::{column, container, image, row, space, stack, text};
use iced::{Center, ContentFit, Element, Fill, Font, Size, border};

use crate::clock::Playback;
use crate::player::{self, Controls};
use crate::song::Song;
use crate::{Message, theme};

/// The window size in mini mode.
pub const SIZE: Size = Size::new(360.0, 450.0);
const COVER: f32 = 240.0;

/// The mini player for `song`, over the blurred `backdrop`.
pub fn view<'a>(
    song: &'a Song,
    playback: &'a Playback,
    controls: Controls,
    backdrop: Option<&'a image::Handle>,
) -> Element<'a, Message> {
    let cover: Element<'a, Message> = match &song.cover {
        Some(handle) => image(handle.clone())
            .width(COVER)
            .height(COVER)
            .content_fit(ContentFit::Cover)
            .border_radius(10)
            .into(),
        None => container(space())
            .width(COVER)
            .height(COVER)
            .style(|_| theme::panel(theme::hero()).border(border::rounded(10)))
            .into(),
    };
    let details = column![
        text(&song.title)
            .size(16)
            .font(Font {
                weight: Weight::Bold,
                ..theme::INTER
            })
            .wrapping(text::Wrapping::None)
            .width(Fill)
            .align_x(Center),
        text(song.byline())
            .size(12)
            .color(theme::text_secondary())
            .wrapping(text::Wrapping::None)
            .width(Fill)
            .align_x(Center),
    ]
    .spacing(4)
    .clip(true);
    let expand = row![
        space::horizontal(),
        player::icon_button(
            include_bytes!("../assets/icons/miniplayer.svg"),
            theme::text_secondary(),
            16.0,
            Some(Message::ToggleMini),
        ),
    ];

    let content = column![
        expand,
        cover,
        details,
        player::progress(playback, controls.seeking),
        player::transport(playback, controls),
    ]
    .spacing(12)
    .padding([12, 18])
    .align_x(Center);

    let background: Element<'a, Message> = match backdrop {
        Some(handle) => image(handle.clone())
            .width(Fill)
            .height(Fill)
            .content_fit(ContentFit::Cover)
            .into(),
        None => space().into(),
    };
    let shade = container(space())
        .width(Fill)
        .height(Fill)
        .style(|_| container::background(theme::center_bg().scale_alpha(0.78)));

    container(stack![background, shade, content])
        .width(Fill)
        .height(Fill)
        .style(|_| theme::panel(theme::center_bg()))
        .into()
}
