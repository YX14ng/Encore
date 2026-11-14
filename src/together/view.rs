//! The together screen: host a session or join one, and see who is listening.

use iced::font::Weight;
use iced::widget::{Column, button, column, container, row, scrollable, text, text_input};
use iced::{Element, Fill, Font, border};

use super::{DEFAULT_PORT, Event, Session, Together};
use crate::{Message, theme};

/// The screen for `together`.
pub fn view(together: &Together) -> Element<'_, Message> {
    let body: Element<'_, Message> = match &together.session {
        Session::Off => off(together),
        Session::FindingAddress { .. } => note("Finding your public address\u{2026}"),
        Session::Hosting {
            invite,
            host,
            listeners,
            ..
        } => {
            let status = if host.is_some() {
                format!("Your session is open on port {}.", invite.port)
            } else {
                "Opening the port\u{2026}".to_string()
            };
            let people: Element<'_, Message> = if listeners.is_empty() {
                note("Nobody has joined yet.")
            } else {
                Column::with_children(listeners.iter().map(|name| text(name).size(14).into()))
                    .spacing(6)
                    .into()
            };
            column![
                note(status),
                section("Invite"),
                container(
                    text(invite.encode())
                        .size(13)
                        .font(Font::MONOSPACE)
                        .color(theme::text_secondary())
                )
                .padding([8, 12])
                .style(|_| container::background(theme::tint(0.05)).border(border::rounded(6))),
                row![
                    accent_button("Copy invite", Event::CopyInvite),
                    plain_button("End session", Event::Leave)
                ]
                .spacing(10),
                note(
                    "Send the invite to your friends, or let them join from your Discord profile. \
                     They connect straight to this computer, so they see its address, and you see \
                     theirs."
                ),
                section(match listeners.len() {
                    1 => "1 listener".to_string(),
                    count => format!("{count} listeners"),
                }),
                people,
            ]
            .spacing(14)
            .into()
        }
        Session::Joining(config) => column![
            note(format!(
                "Connecting to {}\u{2026}",
                config.invite.socket_address()
            )),
            plain_button("Cancel", Event::Leave),
        ]
        .spacing(14)
        .into(),
        Session::Listening { host, .. } => column![
            text(format!("Listening along with {host}")).size(16),
            note(
                "Encore plays what they play. Your own controls work, but the host's next \
                 change wins."
            ),
            plain_button("Leave", Event::Leave),
        ]
        .spacing(14)
        .into(),
    };

    let mut content = column![
        heading("Listen together", 26.0),
        note(
            "Friends hear what you play, in sync. Each Encore downloads the songs on its own; \
             only the song and the position travel."
        ),
        body,
    ]
    .spacing(18);
    if let Some(error) = &together.error {
        content = content.push(text(error).size(13).color(theme::accent()));
    }
    scrollable(content.padding([28, 32]).max_width(720))
        .height(Fill)
        .into()
}

/// No session: the host and join forms.
fn off(together: &Together) -> Element<'_, Message> {
    let field = |placeholder: &str, value: &str, on_input: fn(String) -> Event| {
        text_input(placeholder, value)
            .on_input(move |value| Message::Together(on_input(value)))
            .padding([8, 12])
            .size(14)
    };
    column![
        section("Host a session"),
        row![
            field(
                "Address friends use (found by itself if empty)",
                &together.settings.address,
                Event::AddressChanged
            )
            .width(Fill),
            field(
                &DEFAULT_PORT.to_string(),
                &together.port_input,
                Event::PortChanged
            )
            .width(90),
        ]
        .spacing(10),
        note(
            "Friends on the internet need your router to forward this port to this computer, as \
             for a game server."
        ),
        accent_button("Host", Event::StartHosting),
        section("Join a session"),
        field(
            "encore:\u{2026}",
            &together.invite_input,
            Event::InviteChanged
        )
        .on_submit(Message::Together(Event::Join)),
        accent_button("Join", Event::Join),
    ]
    .spacing(12)
    .into()
}

fn accent_button(label: &str, event: Event) -> Element<'_, Message> {
    button(text(label).size(14).color(iced::Color::WHITE))
        .on_press(Message::Together(event))
        .padding([8, 18])
        .style(|_, status| button::Style {
            background: Some(
                match status {
                    button::Status::Hovered => theme::accent().scale_alpha(0.85),
                    _ => theme::accent(),
                }
                .into(),
            ),
            border: border::rounded(18),
            ..button::Style::default()
        })
        .into()
}

fn plain_button(label: &str, event: Event) -> Element<'_, Message> {
    button(text(label).size(14))
        .on_press(Message::Together(event))
        .padding([8, 18])
        .style(|_, status| {
            let mut style = theme::flat_button(status, false);
            style.border = border::rounded(18).color(theme::line()).width(1);
            style
        })
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

fn section<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label)
        .size(16)
        .font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        })
        .into()
}

fn note<'a>(message: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(message).size(13).color(theme::text_secondary()).into()
}
