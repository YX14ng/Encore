//! What host and guests say to each other: one JSON object per line. Everything a guest gets
//! comes from another computer, so it is checked before it is used.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

use crate::youtube::SearchTrack;

/// The longest line either side accepts.
pub const MAX_LINE: usize = 16 * 1024;
/// The longest text kept from a message.
const MAX_TEXT: usize = 200;

/// Guest to host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToHost {
    Hello { token: String, name: String },
}

/// Host to guest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToGuest {
    Welcome { host: String },
    Refused { reason: String },
    State(State),
}

/// What the host is playing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub track: Option<Track>,
    pub position_ms: u64,
    pub playing: bool,
}

/// A song as the host sends it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: u32,
    pub cover_url: Option<String>,
}

impl Track {
    pub fn from_search(track: &SearchTrack) -> Self {
        Self {
            id: track.id.clone(),
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            duration: track.duration,
            cover_url: track.cover_url.clone(),
        }
    }

    /// The song to play, if the id is a real YouTube id. Long texts are cut and a cover that
    /// is not an https address is dropped.
    pub fn checked(&self) -> Option<SearchTrack> {
        is_video_id(&self.id).then(|| SearchTrack {
            id: self.id.clone(),
            title: cut(&self.title),
            artist: cut(&self.artist),
            album: cut(&self.album),
            duration: self.duration.min(24 * 60 * 60),
            cover_url: self
                .cover_url
                .as_ref()
                .filter(|url| url.starts_with("https://") && url.len() <= 1000)
                .cloned(),
        })
    }
}

/// Whether `id` looks like a YouTube video id: 11 characters of letters, digits, `_` and `-`.
/// It ends up in a yt-dlp address and a file name, so nothing else may pass.
pub fn is_video_id(id: &str) -> bool {
    id.len() == 11
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// A name or a text from the other side, cut to a sane length and without control characters.
pub fn cut(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .take(MAX_TEXT)
        .collect()
}

/// Writes one message as a line.
pub async fn send<W, M>(writer: &mut W, message: &M) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
    M: Serialize,
{
    let mut line = serde_json::to_vec(message).map_err(std::io::Error::other)?;
    line.push(b'\n');
    writer.write_all(&line).await?;
    writer.flush().await
}

/// Reads one message. `Ok(None)` means the other side closed the connection; a line that is
/// too long or is not a message is an error.
pub async fn receive<R, M>(reader: &mut R) -> std::io::Result<Option<M>>
where
    R: AsyncBufRead + Unpin,
    M: for<'de> Deserialize<'de>,
{
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(None);
        }
        let (taken, done) = match available.iter().position(|&byte| byte == b'\n') {
            Some(end) => (end + 1, true),
            None => (available.len(), false),
        };
        line.extend_from_slice(&available[..taken]);
        reader.consume(taken);
        if line.len() > MAX_LINE {
            return Err(std::io::Error::other("the message is too long"));
        }
        if done {
            return serde_json::from_slice(&line)
                .map(Some)
                .map_err(std::io::Error::other);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track {
            id: id.to_string(),
            title: "Usseewa".to_string(),
            artist: "Ado".to_string(),
            album: "Kyougen".to_string(),
            duration: 203,
            cover_url: Some("https://lh3.googleusercontent.com/x".to_string()),
        }
    }

    #[test]
    fn only_real_video_ids_are_played() {
        assert!(track("Qp3b-RXtz4w").checked().is_some());
        for id in [
            "../../etc/x",
            "Qp3b-RXtz4",
            "Qp3b RXtz4w",
            "--exec=rm-x",
            "",
        ] {
            assert!(track(id).checked().is_none(), "{id:?} was accepted");
        }
    }

    #[test]
    fn texts_and_covers_from_the_host_are_tamed() {
        let mut odd = track("Qp3b-RXtz4w");
        odd.title = format!("a\u{7}b{}", "x".repeat(500));
        odd.cover_url = Some("file:///etc/passwd".to_string());
        let checked = odd.checked().expect("valid id");
        assert!(checked.title.starts_with("ab"));
        assert_eq!(checked.title.chars().count(), MAX_TEXT);
        assert_eq!(checked.cover_url, None);
    }

    #[tokio::test]
    async fn messages_go_through_as_lines() {
        let state = ToGuest::State(State {
            track: Some(track("Qp3b-RXtz4w")),
            position_ms: 61_500,
            playing: true,
        });
        let mut wire = Vec::new();
        send(&mut wire, &state).await.expect("send");
        send(&mut wire, &ToGuest::Welcome { host: "Ana".into() })
            .await
            .expect("send");
        let mut reader = tokio::io::BufReader::new(wire.as_slice());
        assert_eq!(receive(&mut reader).await.expect("read"), Some(state));
        assert!(matches!(
            receive::<_, ToGuest>(&mut reader).await,
            Ok(Some(ToGuest::Welcome { .. }))
        ));
        assert_eq!(receive::<_, ToGuest>(&mut reader).await.expect("end"), None);
    }

    #[tokio::test]
    async fn a_line_without_end_cannot_grow_forever() {
        let flood = vec![b'x'; MAX_LINE + 10];
        let mut reader = tokio::io::BufReader::new(flood.as_slice());
        assert!(receive::<_, ToHost>(&mut reader).await.is_err());
    }
}
