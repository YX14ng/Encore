//! How the app runs a session: starting and leaving it, sharing what plays with the guests,
//! and, as a guest, following the host.

use std::time::Instant;

use iced::Task;
use rust_i18n::t;

use super::invite::{self, Invite};
use super::protocol::{self, Track};
use super::{Event, Session, guest, host};
use crate::{App, Message, Screen, discord_ipc};

/// A guest seeks when it drifts this far from the host.
const DRIFT_MS: u64 = 2000;

impl App {
    /// Applies a together event.
    pub(crate) fn together_event(&mut self, event: Event) -> Task<Message> {
        let together = &mut self.together;
        match event {
            Event::AddressChanged(address) => together.settings.address = address,
            Event::PortChanged(port) => {
                together.port_input = port.chars().filter(char::is_ascii_digit).take(5).collect();
            }
            Event::InviteChanged(text) => together.invite_input = text,
            Event::StartHosting => return self.start_hosting(),
            Event::AddressFound(address) => {
                let Session::FindingAddress { port, token } = &together.session else {
                    return Task::none();
                };
                let (port, token) = (*port, token.clone());
                match address {
                    Ok(address) => self.host_at(address, port, token),
                    Err(error) => {
                        together.session = Session::Off;
                        together.error =
                            Some(t!("together.address_not_found", error = error).into_owned());
                    }
                }
            }
            Event::Join => match Invite::parse(&together.invite_input) {
                Ok(invite) => {
                    let name = self.listener_name();
                    let together = &mut self.together;
                    together.error = None;
                    together.session = Session::Joining(guest::Config { invite, name });
                }
                Err(error) => together.error = Some(error),
            },
            Event::Leave => {
                together.session = Session::Off;
                together.target = None;
            }
            Event::CopyInvite => {
                if let Some((invite, _)) = together.hosted_invite() {
                    return iced::clipboard::write(invite.encode());
                }
            }
            Event::Host(event) => self.host_event(event),
            Event::Guest(event) => return self.guest_event(event),
            Event::Answer { user_id, accept } => {
                if let Session::Hosting { requests, .. } = &mut together.session {
                    requests.retain(|(id, _)| *id != user_id);
                }
                self.discord.answer(user_id, accept);
            }
        }
        Task::none()
    }

    /// Applies a join invite or request from Discord (only with the Discord plugin on).
    pub(crate) fn discord_event(&mut self, event: discord_ipc::Event) -> Task<Message> {
        if !self.plugins.discord {
            return Task::none();
        }
        match event {
            discord_ipc::Event::Join { secret } => {
                if matches!(self.together.session, Session::Hosting { .. }) {
                    self.together.error = Some(t!("together.end_before_joining").into_owned());
                    return Task::none();
                }
                self.screen = Screen::Together;
                self.together.target = None;
                self.together.invite_input = secret;
                self.together_event(Event::Join)
            }
            discord_ipc::Event::JoinRequest { user_id, name } => {
                if let Session::Hosting { requests, .. } = &mut self.together.session
                    && !requests.iter().any(|(id, _)| *id == user_id)
                {
                    requests.push((user_id, name));
                }
                Task::none()
            }
        }
    }

    /// Checks the address and port, then hosts, first asking for the public address when the
    /// user left it empty.
    fn start_hosting(&mut self) -> Task<Message> {
        let together = &mut self.together;
        together.error = None;
        let Some(port) = together
            .port_input
            .parse::<u16>()
            .ok()
            .filter(|port| *port > 0)
        else {
            together.error = Some(t!("together.bad_port").into_owned());
            return Task::none();
        };
        let address = together.settings.address.trim().to_string();
        if !address.is_empty() && !invite::is_address(&address) {
            together.error = Some(t!("together.bad_address").into_owned());
            return Task::none();
        }
        let token = match invite::new_token() {
            Ok(token) => token,
            Err(error) => {
                together.error = Some(error);
                return Task::none();
            }
        };
        together.settings.address.clone_from(&address);
        together.settings.port = port;
        together.settings.save();
        if address.is_empty() {
            together.session = Session::FindingAddress { port, token };
            return Task::perform(public_address(), |address| {
                Message::Together(Event::AddressFound(address))
            });
        }
        self.host_at(address, port, token);
        Task::none()
    }

    fn host_at(&mut self, address: String, port: u16, token: String) {
        let name = self.listener_name();
        self.together.session = Session::Hosting {
            config: host::Config {
                port,
                token: token.clone(),
                name,
            },
            invite: Invite {
                address,
                port,
                token,
            },
            host: None,
            listeners: Vec::new(),
            requests: Vec::new(),
        };
    }

    fn host_event(&mut self, event: host::Event) {
        let together = &mut self.together;
        let Session::Hosting {
            host, listeners, ..
        } = &mut together.session
        else {
            return;
        };
        match event {
            host::Event::Ready(ready) => *host = Some(ready),
            host::Event::Listeners(names) => *listeners = names,
            host::Event::Failed(error) => {
                together.session = Session::Off;
                together.error = Some(error);
            }
        }
    }

    fn guest_event(&mut self, event: guest::Event) -> Task<Message> {
        let together = &mut self.together;
        match event {
            guest::Event::Joined { host } => {
                if let Session::Joining(config) = &together.session {
                    together.session = Session::Listening {
                        config: config.clone(),
                        host,
                    };
                }
            }
            guest::Event::State(state) => {
                together.target = Some((state, Instant::now()));
                return self.follow_host();
            }
            guest::Event::Ended(reason) => {
                together.session = Session::Off;
                together.target = None;
                together.error = (!reason.is_empty()).then_some(reason);
            }
        }
        Task::none()
    }

    /// The name guests and hosts see: the YouTube profile name, if known.
    fn listener_name(&self) -> String {
        self.profile().map_or_else(
            || "Encore listener".to_string(),
            |profile| profile.name.clone(),
        )
    }

    /// Tells the guests what is playing (the server only sends what matters).
    pub(crate) fn share_with_listeners(&self) {
        let Some(host) = self.together.host() else {
            return;
        };
        host.publish(protocol::State {
            track: self.online.playing_track().map(Track::from_search),
            position_ms: u64::from(self.position_ms()),
            playing: self.playback.is_playing && self.online.is_active(),
        });
    }

    /// As a guest, plays what the host plays: the same song, near the same second, paused when
    /// the host is.
    pub(crate) fn follow_host(&mut self) -> Task<Message> {
        if !self.together.is_guest() {
            return Task::none();
        }
        let Some((state, received)) = self.together.target.clone() else {
            return Task::none();
        };
        let Some(track) = state.track.as_ref().and_then(Track::checked) else {
            self.set_playing(false);
            return Task::none();
        };
        let playing = self
            .online
            .playing_track()
            .map(|playing| playing.id.as_str());
        if playing != Some(track.id.as_str()) {
            if self.online.shared_view().loading == Some(track.id.as_str()) {
                return Task::none();
            }
            return self.online.play_queue(vec![track], 0);
        }
        let expected = if state.playing {
            state.position_ms + received.elapsed().as_millis() as u64
        } else {
            state.position_ms
        };
        if expected.abs_diff(u64::from(self.position_ms())) > DRIFT_MS {
            self.seek(expected as f32 / 1000.0);
        }
        self.set_playing(state.playing);
        Task::none()
    }
}

/// This computer's address on the internet, as a public echo service sees it. The request
/// carries nothing about the user.
async fn public_address() -> Result<String, String> {
    let text = reqwest::Client::new()
        .get("https://api.ipify.org")
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| error.to_string())?
        .text()
        .await
        .map_err(|error| error.to_string())?;
    let ip = text
        .trim()
        .parse::<std::net::IpAddr>()
        .map_err(|_| t!("together.not_an_address").into_owned())?;
    Ok(invite::address_of(ip))
}

#[cfg(test)]
mod tests {
    /// Asks the echo service for the public address: `cargo test -- --ignored finds_the_public`.
    #[tokio::test]
    #[ignore = "needs the network"]
    async fn finds_the_public_address() {
        let address = super::public_address().await.expect("address");
        assert!(super::invite::is_address(&address), "{address}");
    }
}
