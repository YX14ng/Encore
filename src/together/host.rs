//! The host's side: a TCP server that lets in guests with the right token and sends them what
//! is playing. It runs while the session's subscription runs; dropping the `Host` handle ends
//! every connection.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use iced::futures::{SinkExt, Stream, channel::mpsc};
use rust_i18n::t;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

use super::protocol::{self, State, ToGuest, ToHost};

/// The most guests a session takes.
pub const MAX_GUESTS: usize = 8;
/// How long a new connection has to show its token.
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
/// Guests get the position again this often, so they stay in sync.
const RESEND: Duration = Duration::from_secs(5);
/// A position this far from the expected one is a seek, sent at once.
const SEEK_TOLERANCE_MS: u64 = 2000;

/// What a hosted session needs; also what identifies its subscription.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Config {
    pub port: u16,
    pub token: String,
    pub name: String,
}

/// What the server reports to the app.
#[derive(Debug, Clone)]
pub enum Event {
    /// The port is open; the handle shares the playing state with the guests.
    Ready(Host),
    /// The names of the guests listening now.
    Listeners(Vec<String>),
    /// The port could not be opened.
    Failed(String),
}

/// The app's handle on the server.
#[derive(Debug, Clone)]
pub struct Host {
    state: Arc<watch::Sender<State>>,
    sent_at: Arc<Mutex<Instant>>,
}

impl Host {
    /// Shares `state` with the guests when it matters: another song, play or pause, a seek, or
    /// every few seconds.
    pub fn publish(&self, state: State) {
        let Ok(mut sent_at) = self.sent_at.lock() else {
            return;
        };
        let since = sent_at.elapsed();
        let sent = self
            .state
            .send_if_modified(|last| match worth_sending(last, since, &state) {
                true => {
                    *last = state;
                    true
                }
                false => false,
            });
        if sent {
            *sent_at = Instant::now();
        }
    }
}

/// Whether guests need `new`, `since` after they got `last`.
pub fn worth_sending(last: &State, since: Duration, new: &State) -> bool {
    let same_song =
        last.track.as_ref().map(|track| &track.id) == new.track.as_ref().map(|track| &track.id);
    if !same_song || last.playing != new.playing {
        return true;
    }
    if since >= RESEND {
        return true;
    }
    let expected = if new.playing {
        last.position_ms + since.as_millis() as u64
    } else {
        last.position_ms
    };
    expected.abs_diff(new.position_ms) > SEEK_TOLERANCE_MS
}

/// Runs the server for `config` until the subscription stops.
pub fn run(config: &Config) -> impl Stream<Item = Event> + use<> {
    let config = config.clone();
    iced::stream::channel(16, async move |mut output: mpsc::Sender<Event>| {
        let listener = match bind(config.port).await {
            Ok(listener) => listener,
            Err(error) => {
                let _ = output.send(Event::Failed(error)).await;
                return;
            }
        };
        let (state, receiver) = watch::channel(State::default());
        let host = Host {
            state: Arc::new(state),
            sent_at: Arc::new(Mutex::new(Instant::now())),
        };
        let _ = output.send(Event::Ready(host)).await;

        let guests = Arc::new(Mutex::new(Guests::default()));
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            tokio::spawn(serve(
                stream,
                config.clone(),
                receiver.clone(),
                guests.clone(),
                output.clone(),
            ));
        }
    })
}

/// Opens the port on every address, IPv6 and IPv4 when the system allows both.
async fn bind(port: u16) -> Result<TcpListener, String> {
    match TcpListener::bind(("::", port)).await {
        Ok(listener) => Ok(listener),
        Err(_) => TcpListener::bind(("0.0.0.0", port))
            .await
            .map_err(|error| t!("together.port_failed", port = port, error = error).into_owned()),
    }
}

/// The guests connected now, by connection number.
#[derive(Default)]
struct Guests {
    next: u64,
    names: Vec<(u64, String)>,
}

impl Guests {
    fn names(&self) -> Vec<String> {
        self.names.iter().map(|(_, name)| name.clone()).collect()
    }
}

/// One guest's connection: the token check, then the state each time it changes.
async fn serve(
    stream: TcpStream,
    config: Config,
    mut state: watch::Receiver<State>,
    guests: Arc<Mutex<Guests>>,
    mut output: mpsc::Sender<Event>,
) {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let hello = tokio::time::timeout(HELLO_TIMEOUT, protocol::receive(&mut reader)).await;
    let name = match hello {
        Ok(Ok(Some(ToHost::Hello { token, name }))) if same_token(&token, &config.token) => {
            protocol::cut(&name)
        }
        _ => {
            let reason = "That invite is not valid any more.".to_string();
            let _ = protocol::send(&mut write, &ToGuest::Refused { reason }).await;
            return;
        }
    };

    let joined = {
        let Ok(mut guests) = guests.lock() else {
            return;
        };
        if guests.names.len() >= MAX_GUESTS {
            None
        } else {
            guests.next += 1;
            let id = guests.next;
            guests.names.push((id, name));
            Some((id, guests.names()))
        }
    };
    let Some((id, names)) = joined else {
        let reason = "The session is full.".to_string();
        let _ = protocol::send(&mut write, &ToGuest::Refused { reason }).await;
        return;
    };
    let _ = output.send(Event::Listeners(names)).await;

    let welcome = ToGuest::Welcome {
        host: config.name.clone(),
    };
    let mut open = protocol::send(&mut write, &welcome).await.is_ok();
    let first = ToGuest::State(state.borrow_and_update().clone());
    open = open && protocol::send(&mut write, &first).await.is_ok();
    while open {
        tokio::select! {
            changed = state.changed() => {
                // An error means the app dropped the session.
                let message = changed.map(|()| ToGuest::State(state.borrow_and_update().clone()));
                open = match message {
                    Ok(message) => protocol::send(&mut write, &message).await.is_ok(),
                    Err(_) => false,
                };
            }
            // Guests say nothing after the hello; reading only notices when they leave.
            read = reader.fill_buf() => {
                open = match read {
                    Ok(bytes) if !bytes.is_empty() => {
                        let length = bytes.len();
                        reader.consume(length);
                        true
                    }
                    _ => false,
                };
            }
        }
    }

    let names = match guests.lock() {
        Ok(mut guests) => {
            guests.names.retain(|(guest, _)| *guest != id);
            guests.names()
        }
        Err(_) => return,
    };
    let _ = output.send(Event::Listeners(names)).await;
}

/// Compares tokens without stopping at the first difference, so timing says nothing.
fn same_token(given: &str, expected: &str) -> bool {
    given.len() == expected.len()
        && given
            .bytes()
            .zip(expected.bytes())
            .fold(0, |difference, (a, b)| difference | (a ^ b))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::together::protocol::Track;

    fn state(id: &str, position_ms: u64, playing: bool) -> State {
        State {
            track: Some(Track {
                id: id.to_string(),
                title: String::new(),
                artist: String::new(),
                album: String::new(),
                duration: 200,
                cover_url: None,
            }),
            position_ms,
            playing,
        }
    }

    #[test]
    fn sends_changes_and_skips_the_steady_ticks() {
        let last = state("aaaaaaaaaaa", 10_000, true);
        let second = Duration::from_secs(1);
        assert!(!worth_sending(
            &last,
            second,
            &state("aaaaaaaaaaa", 11_000, true)
        ));
        assert!(worth_sending(&last, second, &state("bbbbbbbbbbb", 0, true)));
        assert!(worth_sending(
            &last,
            second,
            &state("aaaaaaaaaaa", 11_000, false)
        ));
        assert!(worth_sending(
            &last,
            second,
            &state("aaaaaaaaaaa", 60_000, true)
        ));
        assert!(worth_sending(
            &last,
            RESEND,
            &state("aaaaaaaaaaa", 15_000, true)
        ));
    }

    #[test]
    fn tokens_must_match_exactly() {
        assert!(same_token("abc", "abc"));
        assert!(!same_token("abd", "abc"));
        assert!(!same_token("ab", "abc"));
    }
}
