//! Listening together: one Encore hosts a session and friends' Encore follow what it plays. See
//! `docs/superpowers/specs/2026-11-14-encore-together-design.md`.

mod follow;
pub mod guest;
pub mod host;
pub mod invite;
pub mod protocol;
mod view;

use std::time::Instant;

use iced::Subscription;
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::themes::config_dir;
pub use view::view;

/// The port a session opens unless the user picks another.
pub const DEFAULT_PORT: u16 = 41730;

/// What the user set for hosting, kept in `~/.config/encore/together.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The address friends use to reach this computer; empty means "find it".
    pub address: String,
    pub port: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            address: String::new(),
            port: DEFAULT_PORT,
        }
    }
}

impl Settings {
    fn load() -> Self {
        std::fs::read_to_string(config_dir().join("together.json"))
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    /// Saves the settings. A failed write only means they are not remembered next time.
    fn save(&self) {
        let dir = config_dir();
        if std::fs::create_dir_all(&dir).is_ok()
            && let Ok(json) = serde_json::to_string_pretty(self)
        {
            let _ = std::fs::write(dir.join("together.json"), json);
        }
    }
}

/// Where the session is.
#[derive(Debug, Clone)]
pub enum Session {
    Off,
    /// Asking the internet for this computer's public address before hosting.
    FindingAddress {
        port: u16,
        token: String,
    },
    Hosting {
        config: host::Config,
        invite: invite::Invite,
        /// The server's handle, once the port is open.
        host: Option<host::Host>,
        listeners: Vec<String>,
        /// People who asked on Discord to join: their user id and name.
        requests: Vec<(String, String)>,
    },
    Joining(guest::Config),
    Listening {
        config: guest::Config,
        host: String,
    },
}

/// What can happen on the together screen and in a session.
#[derive(Debug, Clone)]
pub enum Event {
    AddressChanged(String),
    PortChanged(String),
    InviteChanged(String),
    StartHosting,
    AddressFound(Result<String, String>),
    Join,
    Leave,
    CopyInvite,
    Host(host::Event),
    Guest(guest::Event),
    /// Accepts or turns down a Discord request to join.
    Answer {
        user_id: String,
        accept: bool,
    },
}

/// The together state of the app.
#[derive(Debug)]
pub struct Together {
    pub settings: Settings,
    port_input: String,
    invite_input: String,
    session: Session,
    error: Option<String>,
    /// The host's latest state and when it came, for following it.
    target: Option<(protocol::State, Instant)>,
}

impl Together {
    pub fn load() -> Self {
        let settings = Settings::load();
        Self {
            port_input: settings.port.to_string(),
            settings,
            invite_input: String::new(),
            session: Session::Off,
            error: None,
            target: None,
        }
    }

    /// Whether a session runs (hosted or joined), to highlight the top bar button.
    pub fn is_active(&self) -> bool {
        !matches!(self.session, Session::Off)
    }

    /// Whether this Encore follows a host.
    pub fn is_guest(&self) -> bool {
        matches!(
            self.session,
            Session::Joining(_) | Session::Listening { .. }
        )
    }

    /// The server's handle while hosting.
    pub fn host(&self) -> Option<&host::Host> {
        match &self.session {
            Session::Hosting { host, .. } => host.as_ref(),
            _ => None,
        }
    }

    /// The invite while hosting, for the Discord join secret.
    pub fn hosted_invite(&self) -> Option<(&invite::Invite, usize)> {
        match &self.session {
            Session::Hosting {
                invite, listeners, ..
            } => Some((invite, listeners.len())),
            _ => None,
        }
    }

    /// The hosted session as Discord shows it: a party and the invite as the join secret.
    pub fn discord_session(&self) -> Option<crate::discord::Hosted> {
        let (invite, listeners) = self.hosted_invite()?;
        // Derived from the token so it names the session without giving the token away.
        let digest = sha2::Sha256::digest(invite.token.as_bytes());
        let party_id: String = digest
            .iter()
            .take(8)
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Some(crate::discord::Hosted {
            party_id: format!("encore-{party_id}"),
            listeners: listeners as u32,
            secret: invite.encode(),
        })
    }

    /// The server or the connection to the host, while a session runs.
    pub fn subscription(&self) -> Subscription<Event> {
        match &self.session {
            Session::Hosting { config, .. } => {
                Subscription::run_with(config.clone(), host::run).map(Event::Host)
            }
            Session::Joining(config) | Session::Listening { config, .. } => {
                Subscription::run_with(config.clone(), guest::run).map(Event::Guest)
            }
            Session::Off | Session::FindingAddress { .. } => Subscription::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use iced::futures::{Stream, StreamExt};

    use super::protocol::{State, Track};
    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .map(|address| address.port())
            .expect("a free port")
    }

    async fn next<S: Stream + Unpin>(stream: &mut S) -> S::Item {
        tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .expect("an event in time")
            .expect("the stream goes on")
    }

    /// Runs the server on its own task, as iced would, and passes its events on.
    fn serve(config: &host::Config) -> tokio::sync::mpsc::UnboundedReceiver<host::Event> {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut server = Box::pin(host::run(config));
        tokio::spawn(async move {
            while let Some(event) = server.next().await {
                if sender.send(event).is_err() {
                    return;
                }
            }
        });
        receiver
    }

    async fn heard(server: &mut tokio::sync::mpsc::UnboundedReceiver<host::Event>) -> host::Event {
        tokio::time::timeout(Duration::from_secs(5), server.recv())
            .await
            .expect("an event in time")
            .expect("the server goes on")
    }

    fn guest(port: u16, token: &str) -> guest::Config {
        guest::Config {
            invite: invite::Invite {
                address: "127.0.0.1".to_string(),
                port,
                token: token.to_string(),
            },
            name: "Bea".to_string(),
        }
    }

    #[tokio::test]
    async fn a_guest_joins_and_hears_what_the_host_plays() {
        let port = free_port();
        let config = host::Config {
            port,
            token: TOKEN.to_string(),
            name: "Ana".to_string(),
        };
        let mut server = serve(&config);
        let host::Event::Ready(host) = heard(&mut server).await else {
            panic!("the port did not open");
        };
        let playing = State {
            track: Some(Track {
                id: "Qp3b-RXtz4w".to_string(),
                title: "Usseewa".to_string(),
                artist: "Ado".to_string(),
                album: String::new(),
                duration: 203,
                cover_url: None,
            }),
            position_ms: 42_000,
            playing: true,
        };
        host.publish(playing.clone());

        let mut client = Box::pin(guest::run(&guest(port, TOKEN)));
        assert!(matches!(
            next(&mut client).await,
            guest::Event::Joined { host } if host == "Ana"
        ));
        assert!(matches!(next(&mut client).await, guest::Event::State(state) if state == playing));
        assert!(matches!(
            heard(&mut server).await,
            host::Event::Listeners(names) if names == ["Bea"]
        ));

        let paused = State {
            playing: false,
            ..playing
        };
        host.publish(paused.clone());
        assert!(matches!(next(&mut client).await, guest::Event::State(state) if state == paused));

        drop(client);
        assert!(matches!(
            heard(&mut server).await,
            host::Event::Listeners(names) if names.is_empty()
        ));
    }

    #[tokio::test]
    async fn a_wrong_token_is_turned_away() {
        let port = free_port();
        let config = host::Config {
            port,
            token: TOKEN.to_string(),
            name: "Ana".to_string(),
        };
        let mut server = serve(&config);
        let host::Event::Ready(_host) = heard(&mut server).await else {
            panic!("the port did not open");
        };
        let wrong = "ffffffffffffffffffffffffffffffff";
        let mut client = Box::pin(guest::run(&guest(port, wrong)));
        assert!(matches!(
            next(&mut client).await,
            guest::Event::Ended(reason) if reason.contains("not valid")
        ));
    }

    #[tokio::test]
    async fn guests_hear_when_the_host_ends_the_session() {
        let port = free_port();
        let config = host::Config {
            port,
            token: TOKEN.to_string(),
            name: "Ana".to_string(),
        };
        let mut server = serve(&config);
        let host::Event::Ready(host) = heard(&mut server).await else {
            panic!("the port did not open");
        };
        let mut client = Box::pin(guest::run(&guest(port, TOKEN)));
        assert!(matches!(
            next(&mut client).await,
            guest::Event::Joined { .. }
        ));
        assert!(matches!(next(&mut client).await, guest::Event::State(_)));
        // Ending the session drops the app's handle and stops the subscription.
        drop(host);
        drop(server);
        assert!(matches!(
            next(&mut client).await,
            guest::Event::Ended(reason) if reason.contains("ended")
        ));
    }
}
