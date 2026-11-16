//! Discord's local IPC protocol: frames of a little-endian opcode, a length and JSON, over the
//! socket (Linux) or named pipe (Windows) Discord opens. Encore speaks it itself because it must
//! also read what Discord sends (join invites and requests), which `discord-rich-presence`
//! cannot.

use std::borrow::Cow;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};

use rust_i18n::t;
use serde_json::{Value, json};

use crate::discord_pipe::{self, Pipe};

/// The biggest frame read from Discord; anything bigger means the stream is broken.
const MAX_FRAME: u32 = 1 << 20;

/// Frame opcodes.
pub const HANDSHAKE: u32 = 0;
pub const FRAME: u32 = 1;
pub const CLOSE: u32 = 2;

/// Connects to the first Discord socket that answers and says hello as `client_id`.
pub fn connect(client_id: &str) -> std::io::Result<Pipe> {
    let mut stream = discord_pipe::open()?;
    write_frame(
        &mut stream,
        HANDSHAKE,
        &json!({ "v": 1, "client_id": client_id }),
    )?;
    let (opcode, ready) = read_frame(&mut stream)?;
    if opcode != FRAME || ready.get("evt").and_then(Value::as_str) != Some("READY") {
        return Err(std::io::Error::other(format!(
            "Discord refused the handshake: {ready}"
        )));
    }
    Ok(stream)
}

/// Sends one frame.
pub fn write_frame(stream: &mut impl Write, opcode: u32, payload: &Value) -> std::io::Result<()> {
    let body = payload.to_string();
    let mut frame = Vec::with_capacity(8 + body.len());
    frame.extend_from_slice(&opcode.to_le_bytes());
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(body.as_bytes());
    stream.write_all(&frame)
}

/// Reads one frame.
pub fn read_frame(stream: &mut impl Read) -> std::io::Result<(u32, Value)> {
    let mut header = [0u8; 8];
    stream.read_exact(&mut header)?;
    let opcode = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    if length > MAX_FRAME {
        return Err(std::io::Error::other(
            "Discord sent a frame that is too big",
        ));
    }
    let mut body = vec![0u8; length as usize];
    stream.read_exact(&mut body)?;
    let payload = serde_json::from_slice(&body).map_err(std::io::Error::other)?;
    Ok((opcode, payload))
}

/// A command frame's payload, with a fresh nonce.
pub fn command(cmd: &str, args: Value, event: Option<&str>) -> Value {
    static NONCE: AtomicU64 = AtomicU64::new(0);
    let nonce = format!(
        "encore-{}-{}",
        std::process::id(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    );
    let mut payload = json!({ "cmd": cmd, "args": args, "nonce": nonce });
    if let Some(event) = event {
        payload["evt"] = Value::from(event);
    }
    payload
}

/// What Discord tells Encore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The user clicked a join invite; the secret is the session's invite.
    Join { secret: String },
    /// Someone asked to join the user's session.
    JoinRequest { user_id: String, name: String },
}

/// The event in a frame Discord sent, if it is one Encore follows.
pub fn event(payload: &Value) -> Option<Event> {
    if payload.get("cmd").and_then(Value::as_str) != Some("DISPATCH") {
        return None;
    }
    let data = payload.get("data")?;
    match payload.get("evt").and_then(Value::as_str)? {
        "ACTIVITY_JOIN" => Some(Event::Join {
            secret: data.get("secret")?.as_str()?.to_string(),
        }),
        "ACTIVITY_JOIN_REQUEST" => {
            let user = data.get("user")?;
            let user_id = user.get("id")?.as_str()?.to_string();
            if user_id.is_empty() || !user_id.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            let name = ["global_name", "username"]
                .iter()
                .find_map(|key| user.get(*key)?.as_str())
                .map_or_else(|| t!("discord_ipc.someone"), Cow::Borrowed);
            Some(Event::JoinRequest {
                user_id,
                name: crate::together::protocol::cut(&name),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_go_through() {
        let mut sent = Vec::new();
        let payload = command("SUBSCRIBE", json!({}), Some("ACTIVITY_JOIN"));
        write_frame(&mut sent, FRAME, &payload).expect("write");
        assert_eq!(
            read_frame(&mut sent.as_slice()).expect("read"),
            (FRAME, payload)
        );
    }

    #[test]
    fn a_huge_frame_is_refused() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&FRAME.to_le_bytes());
        bytes.extend_from_slice(&(MAX_FRAME + 1).to_le_bytes());
        assert!(read_frame(&mut bytes.as_slice()).is_err());
    }

    #[test]
    fn reads_join_invites_and_requests() {
        let join = json!({"cmd": "DISPATCH", "evt": "ACTIVITY_JOIN", "data": {"secret": "s"}});
        assert_eq!(
            event(&join),
            Some(Event::Join {
                secret: "s".to_string()
            })
        );
        let request = json!({"cmd": "DISPATCH", "evt": "ACTIVITY_JOIN_REQUEST",
            "data": {"user": {"id": "1234", "username": "bea", "global_name": "Bea"}}});
        assert_eq!(
            event(&request),
            Some(Event::JoinRequest {
                user_id: "1234".to_string(),
                name: "Bea".to_string()
            })
        );
        let answer = json!({"cmd": "SUBSCRIBE", "evt": "ACTIVITY_JOIN", "data": {}});
        assert_eq!(event(&answer), None);
        let odd_user = json!({"cmd": "DISPATCH", "evt": "ACTIVITY_JOIN_REQUEST",
            "data": {"user": {"id": "1\"}"}}});
        assert_eq!(event(&odd_user), None);
    }
}
