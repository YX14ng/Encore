//! The Last.fm API: signed requests, the desktop sign-in (a token the user approves in the
//! browser, traded for a session key) and scrobbling. When to scrobble lives in `scrobbler.rs`.

use std::path::PathBuf;
use std::time::Duration;

use rust_i18n::t;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::account;

const API_URL: &str = "https://ws.audioscrobbler.com/2.0/";
const KEY_VAR: &str = "ENCORE_LASTFM_API_KEY";
const SECRET_VAR: &str = "ENCORE_LASTFM_API_SECRET";
/// How often to ask whether the user approved Encore yet, and for how long.
const APPROVAL_POLL: Duration = Duration::from_secs(3);
const APPROVAL_TRIES: u32 = 100;
/// Last.fm takes at most this many scrobbles in one request.
pub const BATCH_LIMIT: usize = 50;

/// Last.fm error codes Encore reacts to.
const INVALID_SESSION: u32 = 9;
const SERVICE_OFFLINE: u32 = 11;
const TOKEN_NOT_AUTHORIZED: u32 = 14;
const TEMPORARILY_UNAVAILABLE: u32 = 16;
const RATE_LIMITED: u32 = 29;

/// The API account Encore talks to Last.fm with, from the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub api_key: String,
    pub secret: String,
}

impl Credentials {
    /// Reads `ENCORE_LASTFM_API_KEY` and `ENCORE_LASTFM_API_SECRET`. The error names the
    /// variables that are missing.
    pub fn from_env() -> Result<Self, String> {
        Self::from_values(std::env::var(KEY_VAR).ok(), std::env::var(SECRET_VAR).ok())
    }

    fn from_values(api_key: Option<String>, secret: Option<String>) -> Result<Self, String> {
        let present = |value: Option<String>| value.filter(|value| !value.trim().is_empty());
        match (present(api_key), present(secret)) {
            (Some(api_key), Some(secret)) => Ok(Self {
                api_key: api_key.trim().to_string(),
                secret: secret.trim().to_string(),
            }),
            (None, None) => Err(t!(
                "lastfm.variables_missing",
                key = KEY_VAR,
                secret = SECRET_VAR
            )
            .into_owned()),
            (None, Some(_)) => Err(t!("lastfm.variable_missing", variable = KEY_VAR).into_owned()),
            (Some(_), None) => {
                Err(t!("lastfm.variable_missing", variable = SECRET_VAR).into_owned())
            }
        }
    }
}

/// A signed-in Last.fm user: the session key never expires unless the user revokes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub name: String,
    pub key: String,
}

/// One listen, as Last.fm wants it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scrobble {
    pub artist: String,
    pub track: String,
    pub album: String,
    /// Seconds; 0 when unknown.
    pub duration: u32,
    /// When the song started playing, in seconds since 1970 (UTC).
    pub timestamp: i64,
}

/// Why a request failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// The request did not get an answer (no connection, timeout, a broken reply).
    Network(String),
    /// Last.fm answered with an error.
    Api { code: u32, message: String },
}

impl ApiError {
    /// Whether trying the same request later may work.
    pub fn is_temporary(&self) -> bool {
        match self {
            ApiError::Network(_) => true,
            ApiError::Api { code, .. } => {
                matches!(
                    *code,
                    SERVICE_OFFLINE | TEMPORARILY_UNAVAILABLE | RATE_LIMITED
                )
            }
        }
    }

    /// Whether the user revoked Encore's access, so it has to connect again.
    pub fn is_invalid_session(&self) -> bool {
        matches!(self, ApiError::Api { code, .. } if *code == INVALID_SESSION)
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Network(error) => write!(f, "{}", t!("lastfm.unreachable", error = error)),
            ApiError::Api { code, message } => write!(
                f,
                "{}",
                t!("lastfm.api_error", code = code, message = message)
            ),
        }
    }
}

/// The `api_sig` Last.fm expects: the MD5 of every parameter (but `format` and `callback`)
/// sorted by name and written as name then value, followed by the shared secret.
pub fn signature(params: &[(String, String)], secret: &str) -> String {
    let mut sorted: Vec<&(String, String)> = params
        .iter()
        .filter(|(name, _)| name != "format" && name != "callback")
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut text = String::new();
    for (name, value) in sorted {
        text.push_str(name);
        text.push_str(value);
    }
    text.push_str(secret);
    format!("{:x}", md5::compute(text.as_bytes()))
}

/// Sends a signed request and returns the answer, or the error Last.fm reports.
async fn call(
    credentials: &Credentials,
    method: &str,
    mut params: Vec<(String, String)>,
) -> Result<Value, ApiError> {
    params.push(("method".to_string(), method.to_string()));
    params.push(("api_key".to_string(), credentials.api_key.clone()));
    let api_sig = signature(&params, &credentials.secret);
    params.push(("api_sig".to_string(), api_sig));
    params.push(("format".to_string(), "json".to_string()));

    let response = reqwest::Client::new()
        .post(API_URL)
        .form(&params)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|error| ApiError::Network(error.to_string()))?;
    let body = response
        .text()
        .await
        .map_err(|error| ApiError::Network(error.to_string()))?;
    parse_answer(&body)
}

/// Reads an answer: Last.fm reports errors as `{"error": code, "message": "…"}`.
fn parse_answer(body: &str) -> Result<Value, ApiError> {
    let value: Value = serde_json::from_str(body).map_err(|_| {
        ApiError::Network(t!("lastfm.unexpected_answer", answer = short(body)).into_owned())
    })?;
    match value.get("error").and_then(Value::as_u64) {
        Some(code) => Err(ApiError::Api {
            code: u32::try_from(code).unwrap_or(u32::MAX),
            message: value
                .get("message")
                .and_then(Value::as_str)
                .map_or_else(|| t!("lastfm.no_message").into_owned(), str::to_string),
        }),
        None => Ok(value),
    }
}

fn short(text: &str) -> String {
    text.chars().take(120).collect()
}

/// Asks for a token for the user to approve.
pub async fn get_token(credentials: &Credentials) -> Result<String, ApiError> {
    let answer = call(credentials, "auth.getToken", Vec::new()).await?;
    answer
        .get("token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ApiError::Network(t!("lastfm.no_token").into_owned()))
}

/// The page where the user approves `token` for Encore.
pub fn approval_url(credentials: &Credentials, token: &str) -> String {
    format!(
        "https://www.last.fm/api/auth/?api_key={}&token={token}",
        credentials.api_key
    )
}

/// Signs in: gets a token, opens the approval page in the browser and waits (up to five
/// minutes) until the user approves it.
pub async fn connect(credentials: Credentials) -> Result<Session, String> {
    let token = get_token(&credentials)
        .await
        .map_err(|error| error.to_string())?;
    account::open_in_browser(&approval_url(&credentials, &token))?;

    for _ in 0..APPROVAL_TRIES {
        tokio::time::sleep(APPROVAL_POLL).await;
        let params = vec![("token".to_string(), token.clone())];
        match call(&credentials, "auth.getSession", params).await {
            Ok(answer) => return session_from(&answer),
            Err(ApiError::Api { code, .. }) if code == TOKEN_NOT_AUTHORIZED => {}
            Err(error) if error.is_temporary() => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Err(t!("lastfm.not_approved").into_owned())
}

fn session_from(answer: &Value) -> Result<Session, String> {
    let session = answer
        .get("session")
        .ok_or_else(|| t!("lastfm.no_session").into_owned())?;
    let text = |key: &str| {
        session
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| t!("lastfm.session_lacks", field = key).into_owned())
    };
    Ok(Session {
        name: text("name")?,
        key: text("key")?,
    })
}

/// Tells Last.fm what is playing right now.
pub async fn update_now_playing(
    credentials: Credentials,
    session_key: String,
    playing: Scrobble,
) -> Result<(), ApiError> {
    let mut params = vec![
        ("artist".to_string(), playing.artist),
        ("track".to_string(), playing.track),
        ("sk".to_string(), session_key),
    ];
    if !playing.album.is_empty() {
        params.push(("album".to_string(), playing.album));
    }
    if playing.duration > 0 {
        params.push(("duration".to_string(), playing.duration.to_string()));
    }
    call(&credentials, "track.updateNowPlaying", params)
        .await
        .map(|_| ())
}

/// Scrobbles up to `BATCH_LIMIT` listens in one request.
pub async fn scrobble(
    credentials: Credentials,
    session_key: String,
    listens: Vec<Scrobble>,
) -> Result<(), ApiError> {
    let mut params = scrobble_params(&listens);
    params.push(("sk".to_string(), session_key));
    call(&credentials, "track.scrobble", params)
        .await
        .map(|_| ())
}

/// The numbered parameters of a batch: `artist[0]`, `track[0]`, `timestamp[0]`, …
pub fn scrobble_params(listens: &[Scrobble]) -> Vec<(String, String)> {
    let mut params = Vec::new();
    for (index, listen) in listens.iter().take(BATCH_LIMIT).enumerate() {
        params.push((format!("artist[{index}]"), listen.artist.clone()));
        params.push((format!("track[{index}]"), listen.track.clone()));
        params.push((format!("timestamp[{index}]"), listen.timestamp.to_string()));
        if !listen.album.is_empty() {
            params.push((format!("album[{index}]"), listen.album.clone()));
        }
        if listen.duration > 0 {
            params.push((format!("duration[{index}]"), listen.duration.to_string()));
        }
    }
    params
}

/// Where the session key and the scrobbles waiting to be sent are kept.
fn dir() -> PathBuf {
    account::data_dir().join("lastfm")
}

/// Reads a JSON file from the Last.fm folder.
pub fn load<T: for<'de> Deserialize<'de>>(name: &str) -> Option<T> {
    let json = std::fs::read_to_string(dir().join(name)).ok()?;
    serde_json::from_str::<T>(&json).ok()
}

/// Writes a JSON file to the Last.fm folder, readable only by the user.
pub fn save<T: Serialize>(name: &str, value: &T) -> Result<(), String> {
    let dir = dir();
    account::private_dir(&dir)?;
    let json = serde_json::to_string(value).map_err(|error| error.to_string())?;
    crate::platform::write_private(&dir.join(name), json.as_bytes())
        .map_err(|error| error.to_string())
}

/// Deletes a file from the Last.fm folder; a missing file is fine.
pub fn remove(name: &str) {
    let _ = std::fs::remove_file(dir().join(name));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(name: &str, value: &str) -> (String, String) {
        (name.to_string(), value.to_string())
    }

    #[test]
    fn signs_sorted_parameters_without_format() {
        let params = [
            pair("method", "auth.getToken"),
            pair("api_key", "key"),
            pair("format", "json"),
        ];
        // `printf %s api_keykeymethodauth.getTokensecret | md5sum`
        assert_eq!(
            signature(&params, "secret"),
            "b4705499705a550b07ca058a15bde9b0"
        );
    }

    #[test]
    fn names_the_missing_variables() {
        let set = |value: &str| Some(value.to_string());
        assert!(Credentials::from_values(set("k"), set("s")).is_ok());
        assert_eq!(
            Credentials::from_values(None, set("s")),
            Err(format!("{KEY_VAR} is not set"))
        );
        assert_eq!(
            Credentials::from_values(set("k"), set("  ")),
            Err(format!("{SECRET_VAR} is not set"))
        );
        assert!(
            Credentials::from_values(None, None)
                .is_err_and(|error| error.contains(KEY_VAR) && error.contains(SECRET_VAR))
        );
    }

    #[test]
    fn reads_errors_and_answers() {
        assert_eq!(
            parse_answer(r#"{"error":14,"message":"Unauthorized Token"}"#),
            Err(ApiError::Api {
                code: 14,
                message: "Unauthorized Token".to_string()
            })
        );
        assert!(parse_answer(r#"{"token":"abc"}"#).is_ok());
        assert!(matches!(parse_answer("<html>"), Err(ApiError::Network(_))));
    }

    #[test]
    fn tells_temporary_errors_from_revoked_access() {
        let api = |code| ApiError::Api {
            code,
            message: String::new(),
        };
        assert!(api(SERVICE_OFFLINE).is_temporary());
        assert!(api(RATE_LIMITED).is_temporary());
        assert!(ApiError::Network("timeout".to_string()).is_temporary());
        assert!(!api(INVALID_SESSION).is_temporary());
        assert!(api(INVALID_SESSION).is_invalid_session());
    }

    #[test]
    fn reads_a_session() {
        let answer: Value =
            serde_json::from_str(r#"{"session":{"name":"YX14ng","key":"sk","subscriber":0}}"#)
                .expect("json");
        assert_eq!(
            session_from(&answer),
            Ok(Session {
                name: "YX14ng".to_string(),
                key: "sk".to_string()
            })
        );
    }

    #[test]
    fn numbers_a_batch_and_skips_unknown_fields() {
        let listen = |track: &str, album: &str, duration| Scrobble {
            artist: "Ado".to_string(),
            track: track.to_string(),
            album: album.to_string(),
            duration,
            timestamp: 1_700_000_000,
        };
        let params = scrobble_params(&[listen("Usseewa", "Kyougen", 204), listen("Odo", "", 0)]);
        assert!(params.contains(&pair("track[0]", "Usseewa")));
        assert!(params.contains(&pair("album[0]", "Kyougen")));
        assert!(params.contains(&pair("duration[0]", "204")));
        assert!(params.contains(&pair("timestamp[1]", "1700000000")));
        assert!(!params.iter().any(|(name, _)| name == "album[1]"));
        assert!(!params.iter().any(|(name, _)| name == "duration[1]"));
    }

    /// Gets a real token with the API account from the environment (nothing is approved or
    /// scrobbled): `cargo test -- --ignored gets_a_token`.
    #[tokio::test]
    #[ignore = "needs the network and the Last.fm API account"]
    async fn gets_a_token() {
        let credentials = Credentials::from_env().expect("credentials");
        let token = get_token(&credentials).await.expect("token");
        assert!(token.len() >= 16, "got {token:?}");
    }
}
