//! The guest's side: connects to the host with the invite's token and reports what the host
//! plays.

use std::time::Duration;

use iced::futures::{SinkExt, Stream, channel::mpsc};
use rust_i18n::t;
use tokio::io::BufReader;
use tokio::net::TcpStream;

use super::invite::Invite;
use super::protocol::{self, State, ToGuest, ToHost};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// What joining needs; also what identifies the subscription.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Config {
    pub invite: Invite,
    pub name: String,
}

/// What the connection reports to the app.
#[derive(Debug, Clone)]
pub enum Event {
    /// The host let us in.
    Joined { host: String },
    /// What the host plays now.
    State(State),
    /// The session is over for us, and why.
    Ended(String),
}

/// Joins the session in `config` and follows it until it ends or the subscription stops.
pub fn run(config: &Config) -> impl Stream<Item = Event> + use<> {
    let config = config.clone();
    iced::stream::channel(16, async move |mut output: mpsc::Sender<Event>| {
        let reason = follow(&config, &mut output).await;
        let _ = output.send(Event::Ended(reason)).await;
    })
}

/// Connects and passes the host's messages on. Returns why it stopped.
async fn follow(config: &Config, output: &mut mpsc::Sender<Event>) -> String {
    let address = config.invite.socket_address();
    let stream = match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(&address)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(error)) => return t!("together.unreachable", error = error).into_owned(),
        Err(_) => return t!("together.no_answer").into_owned(),
    };
    let (read, mut write) = stream.into_split();
    let hello = ToHost::Hello {
        token: config.invite.token.clone(),
        name: config.name.clone(),
    };
    if let Err(error) = protocol::send(&mut write, &hello).await {
        return t!("together.connection_lost", error = error).into_owned();
    }
    let mut reader = BufReader::new(read);
    loop {
        let event = match protocol::receive::<_, ToGuest>(&mut reader).await {
            Ok(Some(ToGuest::Welcome { host })) => Event::Joined {
                host: protocol::cut(&host),
            },
            Ok(Some(ToGuest::State(state))) => Event::State(state),
            Ok(Some(ToGuest::Refused { reason })) => return protocol::cut(&reason),
            Ok(None) => return t!("together.host_ended").into_owned(),
            Err(error) => return t!("together.connection_lost", error = error).into_owned(),
        };
        if output.send(event).await.is_err() {
            return String::new();
        }
    }
}
