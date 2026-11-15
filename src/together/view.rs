//! The together screen: host a session or join one, and see who is listening.

use iced::font::Weight;
use iced::widget::{Column, button, column, container, row, scrollable, text, text_input};
use iced::{Element, Fill, Font, border};
use rust_i18n::t;

use super::{DEFAULT_PORT, Event, Session, Together};
use crate::{Message, theme};

/// The screen for `together`.
pub fn view(together: &Together) -> Element<'_, Message> {
    let body: Element<'_, Message> = match &together.session {
        Session::Off => off(together),
        Session::FindingAddress { .. } => note(t!("together.finding_address")),
        Session::Hosting {
            invite,
            host,
            listeners,
            requests,
            ..
        } => {
            let status = if host.is_some() {
                t!("together.session_open", port = invite.port)
            } else {
                t!("together.opening_port")
            };
            let people: Element<'_, Message> = if listeners.is_empty() {
                note(t!("together.nobody_yet"))
            } else {
                Column::with_children(listeners.iter().map(|name| text(name).size(14).into()))
                    .spacing(6)
                    .into()
            };
            column![
                note(status),
                section(t!("together.invite")),
                container(
                    text(invite.encode())
                        .size(13)
                        .font(Font::MONOSPACE)
                        .color(theme::text_secondary())
                )
                .padding([8, 12])
                .style(|_| container::background(theme::tint(0.05)).border(border::rounded(6))),
                row![
                    accent_button(t!("together.copy_invite"), Event::CopyInvite),
                    plain_button(t!("together.end_session"), Event::Leave)
                ]
                .spacing(10),
                note(t!("together.invite_note")),
                section(match listeners.len() {
                    1 => t!("together.listener_one"),
                    count => t!("together.listeners", count = count),
                }),
                people,
            ]
            .push((!requests.is_empty()).then(|| {
                Column::with_children(requests.iter().map(|(user_id, name)| {
                    row![
                        text(t!("together.asks_to_join", name = name))
                            .size(14)
                            .width(Fill),
                        accent_button(
                            t!("together.accept"),
                            Event::Answer {
                                user_id: user_id.clone(),
                                accept: true,
                            }
                        ),
                        plain_button(
                            t!("together.decline"),
                            Event::Answer {
                                user_id: user_id.clone(),
                                accept: false,
                            }
                        ),
                    ]
                    .spacing(10)
                    .align_y(iced::Center)
                    .into()
                }))
                .spacing(8)
            }))
            .spacing(14)
            .into()
        }
        Session::Joining(config) => column![
            note(t!(
                "together.connecting",
                address = config.invite.socket_address()
            )),
            plain_button(t!("together.cancel"), Event::Leave),
        ]
        .spacing(14)
        .into(),
        Session::Listening { host, .. } => column![
            text(t!("together.listening_along", host = host)).size(16),
            note(t!("together.listening_note")),
            plain_button(t!("together.leave"), Event::Leave),
        ]
        .spacing(14)
        .into(),
    };

    let mut content = column![
        heading(t!("together.title"), 26.0),
        note(t!("together.intro")),
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
        section(t!("together.host_section")),
        row![
            field(
                &t!("together.address_placeholder"),
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
        note(t!("together.port_note")),
        accent_button(t!("together.host"), Event::StartHosting),
        section(t!("together.join_section")),
        field(
            "encore:\u{2026}",
            &together.invite_input,
            Event::InviteChanged
        )
        .on_submit(Message::Together(Event::Join)),
        accent_button(t!("together.join"), Event::Join),
    ]
    .spacing(12)
    .into()
}

fn accent_button<'a>(label: impl text::IntoFragment<'a>, event: Event) -> Element<'a, Message> {
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

fn plain_button<'a>(label: impl text::IntoFragment<'a>, event: Event) -> Element<'a, Message> {
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

fn heading<'a>(label: impl text::IntoFragment<'a>, size: f32) -> Element<'a, Message> {
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
