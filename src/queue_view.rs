//! The queue panel at the right of the window, in the lyrics panel's place: the song that is
//! playing and the songs that come after it, which can be played, moved up or taken out.

use iced::font::Weight;
use iced::widget::{Column, button, column, container, row, scrollable, space, text, toggler};
use iced::{Center, Element, Fill, Font};

use crate::queue::Queue;
use crate::search::{Shared, picture, subtitle};
use crate::{Message, clock, lyrics, theme};

/// The panel for `queue`, with the autoplay switch.
pub fn view<'a>(queue: &'a Queue, autoplay: bool, shared: Shared<'a>) -> Element<'a, Message> {
    let upcoming = queue.upcoming();
    let clear = (!upcoming.is_empty()).then(|| {
        button(text("Clear").size(13).color(theme::text_secondary()))
            .on_press(Message::QueueClear)
            .padding([4, 10])
            .style(|_, status| theme::flat_button(status, false))
    });
    let header = row![heading("Queue", 20.0), space::horizontal()]
        .push(clear)
        .align_y(Center);

    let mut content = column![header].spacing(10);
    match queue.playing() {
        Some(playing) => {
            content = content.push(label("Now playing".to_string())).push(song(
                queue,
                playing,
                Place::Playing,
                shared,
            ));
            let count = label(match upcoming.len() {
                0 => "Nothing up next".to_string(),
                1 => "Up next \u{b7} 1 song".to_string(),
                count => format!("Up next \u{b7} {count} songs"),
            });
            content = content
                .push(row![count, space::horizontal(), autoplay_switch(autoplay)].align_y(Center));
            let first_suggested = upcoming.iter().position(|&index| queue.is_suggested(index));
            let mut rows = Column::new().spacing(2);
            for (place, &index) in upcoming.iter().enumerate() {
                if Some(place) == first_suggested {
                    rows = rows.push(label("Autoplay \u{b7} similar songs".to_string()));
                }
                let place = if place == 0 {
                    Place::Next
                } else {
                    Place::Later
                };
                rows = rows.push(song(queue, index, place, shared));
            }
            content = content.push(rows);
            if upcoming.is_empty() && autoplay {
                content = content.push(
                    text("Autoplay will go on with similar songs.")
                        .size(13)
                        .color(theme::text_secondary()),
                );
            }
        }
        None => {
            content = content.push(
                text(
                    "Play a song, an album or a playlist to fill the queue, and add songs to it \
                     with the + next to any song.",
                )
                .size(13)
                .color(theme::text_secondary()),
            );
        }
    }

    container(scrollable(content.padding([20, 16])).height(Fill))
        .width(lyrics::WIDTH)
        .height(Fill)
        .style(|_| theme::panel(theme::lyrics_bg()))
        .into()
}

/// Where a song is in the queue, which decides its buttons.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Playing,
    /// The first song up next: it already plays next, so it has no "play next" button.
    Next,
    Later,
}

/// One song: clicking it plays it; songs up next can be moved up or taken out.
fn song<'a>(
    queue: &'a Queue,
    index: usize,
    place: Place,
    shared: Shared<'a>,
) -> Element<'a, Message> {
    let Some(track) = queue.get(index) else {
        return space().into();
    };
    let details = column![
        text(&track.title).size(13).wrapping(text::Wrapping::None),
        text(subtitle(track))
            .size(12)
            .color(theme::text_secondary())
            .wrapping(text::Wrapping::None),
    ]
    .spacing(2)
    .width(Fill)
    .clip(true);

    let is_loading = shared.loading == Some(track.id.as_str());
    let right: Element<'a, Message> = if is_loading {
        text("Loading\u{2026}")
            .size(12)
            .color(theme::accent())
            .into()
    } else if place == Place::Playing {
        let duration = if track.duration == 0 {
            String::new()
        } else {
            clock::format_time(track.duration)
        };
        text(duration)
            .size(12)
            .color(theme::text_secondary())
            .into()
    } else {
        row![]
            .push((place == Place::Later).then(|| {
                small_button(
                    include_bytes!("../assets/icons/play-next.svg"),
                    Message::QueuePlayNext(index),
                )
            }))
            .push(small_button(
                include_bytes!("../assets/icons/close.svg"),
                Message::QueueRemove(index),
            ))
            .spacing(2)
            .into()
    };

    let cover = picture(track.cover_url.as_deref(), 38.0, false, shared);
    let playing = place == Place::Playing;
    button(row![cover, details, right].spacing(10).align_y(Center))
        .on_press_maybe((!playing).then_some(Message::QueueJump(index)))
        .width(Fill)
        .padding([5, 6])
        .style(move |_, status| theme::flat_button(status, playing))
        .into()
}

/// The switch that turns autoplay on and off.
fn autoplay_switch(on: bool) -> Element<'static, Message> {
    toggler(on)
        .label("Autoplay")
        .size(16)
        .text_size(12)
        .on_toggle(|_| Message::ToggleAutoplay)
        .style(theme::switch)
        .into()
}

/// A small icon button inside a song row.
fn small_button(bytes: &'static [u8], message: Message) -> Element<'static, Message> {
    button(theme::icon(bytes, theme::text_secondary(), 15.0))
        .on_press(message)
        .padding(5)
        .style(|_, status| theme::flat_button(status, false))
        .into()
}

fn label(label: String) -> Element<'static, Message> {
    container(
        text(label)
            .size(12)
            .font(Font {
                weight: Weight::Semibold,
                ..theme::INTER
            })
            .color(theme::text_dim()),
    )
    .padding([8, 0])
    .into()
}

fn heading(label: &str, size: f32) -> Element<'_, Message> {
    text(label)
        .size(size)
        .font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        })
        .into()
}
