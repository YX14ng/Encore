//! The playlist dialogs (new playlist, add to playlist, cover) and the note at the bottom.

use iced::font::Weight;
use iced::widget::{
    Column, button, center, column, container, mouse_area, opaque, row, scrollable, space, text,
    text_input,
};
use iced::{Center, Color, Element, Fill, Font, border};
use rust_i18n::t;

use super::{Dialog, Event, Privacy};
use crate::search::{Shared, cover_picture};
use crate::youtube::Tile;
use crate::{Message, theme};

const WIDTH: f32 = 400.0;

/// `dialog` in a card over a dimmed window; clicking outside the card closes it.
pub fn dialog<'a>(
    dialog: &'a Dialog,
    own: Vec<&'a Tile>,
    shared: Shared<'a>,
) -> Element<'a, Message> {
    let content = match dialog {
        Dialog::New {
            name,
            privacy,
            song,
            busy,
            error,
        } => new_playlist(
            name,
            *privacy,
            song.as_ref().map(|song| song.title.as_str()),
            *busy,
        )
        .push(error.as_deref().map(error_text)),
        Dialog::AddTo { song, busy, error } => {
            add_to(&song.title, own, *busy, shared).push(error.as_deref().map(error_text))
        }
        Dialog::Cover {
            id,
            title,
            path,
            busy,
            error,
        } => cover(title, path, shared.covers.contains_key(id), *busy)
            .push(error.as_deref().map(error_text)),
    };
    let card = container(content.spacing(14))
        .width(WIDTH)
        .padding(24)
        .style(|_| {
            theme::panel(theme::center_bg())
                .border(border::rounded(14).color(theme::line()).width(1))
        });
    let shade = container(center(opaque(card)))
        .width(Fill)
        .height(Fill)
        .style(|_| container::background(Color::from_rgba(0.0, 0.0, 0.0, 0.55)));
    opaque(mouse_area(shade).on_press(Message::Playlist(Event::Close)))
}

/// The note at the bottom of the window.
pub fn toast(message: &str) -> Element<'_, Message> {
    let note = container(text(message).size(13).color(Color::WHITE))
        .padding([10, 18])
        .style(|_| {
            container::background(Color::from_rgba(0.08, 0.08, 0.09, 0.94))
                .border(border::rounded(20))
        });
    container(note)
        .width(Fill)
        .height(Fill)
        .align_x(Center)
        .align_bottom(Fill)
        .padding(iced::Padding::default().bottom(112))
        .into()
}

fn new_playlist<'a>(
    name: &'a str,
    privacy: Privacy,
    song: Option<&'a str>,
    busy: bool,
) -> Column<'a, Message> {
    let privacy_chips = row(Privacy::ALL.iter().map(|&each| {
        chip(
            each.label(),
            each == privacy,
            Message::Playlist(Event::PrivacyPicked(each)),
        )
    }))
    .spacing(8);
    let create = if busy {
        t!("playlists.creating")
    } else {
        t!("playlists.create")
    };
    column![heading(t!("playlists.new_playlist"))]
        .push(song.map(|title| note(t!("playlists.with_song", title = title))))
        .push(
            text_input(&t!("playlists.name"), name)
                .on_input(|text| Message::Playlist(Event::NameChanged(text)))
                .on_submit(Message::Playlist(Event::Create))
                .padding([8, 12])
                .size(14),
        )
        .push(privacy_chips)
        .push(actions(vec![
            secondary(
                t!("playlists.cancel"),
                Some(Message::Playlist(Event::Close)),
            ),
            primary(create, (!busy).then_some(Message::Playlist(Event::Create))),
        ]))
}

fn add_to<'a>(
    song: &'a str,
    own: Vec<&'a Tile>,
    busy: bool,
    shared: Shared<'a>,
) -> Column<'a, Message> {
    let new = button(
        row![
            container(theme::icon(
                include_bytes!("../../assets/icons/plus.svg"),
                theme::text_primary(),
                18.0,
            ))
            .center(40)
            .style(|_| container::background(theme::tint(0.08)).border(border::rounded(5))),
            text(t!("playlists.new_playlist")).size(14),
        ]
        .spacing(12)
        .align_y(Center),
    )
    .on_press_maybe((!busy).then_some(Message::Playlist(Event::NewWithSong)))
    .width(Fill)
    .padding([5, 6])
    .style(|_, status| theme::flat_button(status, false));

    let list: Element<'a, Message> = if own.is_empty() {
        note(t!("playlists.no_own")).into()
    } else {
        let rows = own.into_iter().map(|tile| {
            button(
                row![
                    cover_picture(&tile.id, tile.image_url.as_deref(), 40.0, false, shared),
                    text(&tile.title).size(14),
                ]
                .spacing(12)
                .align_y(Center),
            )
            .on_press_maybe((!busy).then(|| {
                Message::Playlist(Event::AddTo {
                    id: tile.id.clone(),
                    title: tile.title.clone(),
                })
            }))
            .width(Fill)
            .padding([5, 6])
            .style(|_, status| theme::flat_button(status, false))
            .into()
        });
        scrollable(Column::with_children(rows).spacing(2))
            .height(iced::Length::Shrink)
            .into()
    };
    let list = container(list).max_height(320);

    column![
        heading(t!("playlists.add_to_playlist")),
        note(t!("playlists.quoted", title = song)),
        new,
        list
    ]
    .push(busy.then(|| note(t!("playlists.adding"))))
    .push(actions(vec![secondary(
        t!("playlists.cancel"),
        Some(Message::Playlist(Event::Close)),
    )]))
}

fn cover<'a>(title: &'a str, path: &'a str, custom: bool, busy: bool) -> Column<'a, Message> {
    let idle = |message: Event| (!busy).then_some(Message::Playlist(message));
    let mut buttons = Vec::new();
    if custom {
        buttons.push(secondary(
            t!("playlists.use_youtube"),
            idle(Event::ResetCover),
        ));
    }
    buttons.push(secondary(
        t!("playlists.cancel"),
        Some(Message::Playlist(Event::Close)),
    ));
    buttons.push(primary(
        if busy {
            t!("playlists.saving")
        } else {
            t!("playlists.use_image")
        },
        idle(Event::UseCover),
    ));
    column![
        heading(t!("playlists.cover_title")),
        note(t!("playlists.cover_note", title = title)),
        secondary(t!("playlists.choose_image"), idle(Event::BrowseCover)),
        text_input(&t!("playlists.paste_path"), path)
            .on_input(|text| Message::Playlist(Event::CoverPathChanged(text)))
            .on_submit(Message::Playlist(Event::UseCover))
            .padding([8, 12])
            .size(13),
        actions(buttons),
    ]
}

fn actions(buttons: Vec<Element<'_, Message>>) -> Element<'_, Message> {
    row![space::horizontal()]
        .extend(buttons)
        .spacing(8)
        .align_y(Center)
        .into()
}

fn primary<'a>(
    label: impl text::IntoFragment<'a>,
    message: Option<Message>,
) -> Element<'a, Message> {
    button(text(label).size(14).color(Color::WHITE))
        .on_press_maybe(message)
        .padding([8, 18])
        .style(|_, status| {
            let mut style = theme::flat_button(status, false);
            style.background = Some(theme::accent().into());
            style.border = border::rounded(18);
            style
        })
        .into()
}

fn secondary<'a>(
    label: impl text::IntoFragment<'a>,
    message: Option<Message>,
) -> Element<'a, Message> {
    button(text(label).size(14))
        .on_press_maybe(message)
        .padding([8, 18])
        .style(|_, status| {
            let mut style = theme::flat_button(status, false);
            style.border = border::rounded(18).color(theme::line()).width(1);
            style
        })
        .into()
}

fn chip<'a>(
    label: impl text::IntoFragment<'a>,
    selected: bool,
    message: Message,
) -> Element<'a, Message> {
    button(text(label).size(13))
        .on_press(message)
        .padding([6, 14])
        .style(move |_, status| {
            let mut style = theme::flat_button(status, false);
            if selected {
                style.background = Some(theme::accent().into());
                style.text_color = Color::WHITE;
            }
            style.border = border::rounded(16).color(theme::line()).width(1);
            style
        })
        .into()
}

fn heading<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label)
        .size(20)
        .font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        })
        .into()
}

fn note<'a>(message: impl text::IntoFragment<'a>) -> iced::widget::Text<'a> {
    text(message).size(13).color(theme::text_secondary())
}

fn error_text(message: &str) -> iced::widget::Text<'_> {
    text(message).size(13).color(theme::accent())
}
