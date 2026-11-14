//! Changes to the signed-in user's playlists on YouTube Music: creating one, adding a song and
//! taking one out. rustypipe only reads playlists, so these are the same InnerTube requests
//! music.youtube.com makes, sent through rustypipe's authenticated `raw` call.

use rustypipe::client::{ClientType, RustyPipe};
use serde_json::{Value, json};

/// Who can see a new playlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Privacy {
    #[default]
    Private,
    Unlisted,
    Public,
}

impl Privacy {
    pub const ALL: [Privacy; 3] = [Privacy::Private, Privacy::Unlisted, Privacy::Public];

    pub fn label(self) -> &'static str {
        match self {
            Privacy::Private => "Private",
            Privacy::Unlisted => "Unlisted",
            Privacy::Public => "Public",
        }
    }

    fn status(self) -> &'static str {
        match self {
            Privacy::Private => "PRIVATE",
            Privacy::Unlisted => "UNLISTED",
            Privacy::Public => "PUBLIC",
        }
    }
}

/// Creates a playlist called `title` holding `songs` (video ids). Returns its id.
pub async fn create(
    rp: RustyPipe,
    title: String,
    privacy: Privacy,
    songs: Vec<String>,
) -> Result<String, String> {
    let body = json!({
        "title": title,
        "description": "",
        "privacyStatus": privacy.status(),
        "videoIds": songs,
    });
    let response = send(&rp, "playlist/create", &body).await?;
    created_id(&response)
}

/// Adds song `video_id` at the end of playlist `playlist_id`. A song already in it is skipped.
pub async fn add(rp: RustyPipe, playlist_id: String, video_id: String) -> Result<(), String> {
    let action = json!({
        "action": "ACTION_ADD_VIDEO",
        "addedVideoId": video_id,
        "dedupeOption": "DEDUPE_OPTION_SKIP",
    });
    edit(&rp, &playlist_id, action).await
}

/// Takes song `video_id` out of playlist `playlist_id`.
pub async fn remove(rp: RustyPipe, playlist_id: String, video_id: String) -> Result<(), String> {
    let action = json!({
        "action": "ACTION_REMOVE_VIDEO_BY_VIDEO_ID",
        "removedVideoId": video_id,
    });
    edit(&rp, &playlist_id, action).await
}

/// Deletes playlist `playlist_id`. Only the live test uses it, to clean up after itself.
#[cfg(test)]
pub async fn delete(rp: RustyPipe, playlist_id: String) -> Result<(), String> {
    let body = json!({ "playlistId": bare_id(&playlist_id) });
    send(&rp, "playlist/delete", &body).await.map(|_| ())
}

async fn edit(rp: &RustyPipe, playlist_id: &str, action: Value) -> Result<(), String> {
    let body = json!({ "playlistId": bare_id(playlist_id), "actions": [action] });
    let response = send(rp, "browse/edit_playlist", &body).await?;
    succeeded(&response)
}

async fn send(rp: &RustyPipe, endpoint: &str, body: &Value) -> Result<Value, String> {
    let response = rp
        .query()
        .authenticated()
        .raw(ClientType::DesktopMusic, endpoint, body)
        .await
        .map_err(|error| request_error(&error))?;
    serde_json::from_str(&response)
        .map_err(|error| format!("YouTube's answer is not JSON: {error}"))
}

/// What went wrong with a request, in words. YouTube answers 401 when the saved session no
/// longer works (it expired, or the browser it came from signed out).
fn request_error(error: &rustypipe::error::Error) -> String {
    match error {
        rustypipe::error::Error::HttpStatus(401, _) => SESSION_EXPIRED.to_string(),
        error => error.to_string(),
    }
}

const SESSION_EXPIRED: &str =
    "Your YouTube Music session expired. Sign out and sign in again in Account.";

/// A playlist's id without the "VL" its browse page adds.
pub fn bare_id(playlist_id: &str) -> &str {
    playlist_id.strip_prefix("VL").unwrap_or(playlist_id)
}

/// Whether a playlist is the user's own, so songs can be added to it and taken out. YouTube
/// Music gives no owner for the user's own playlists, and their ids start with "PL"; Liked
/// Music ("LM") and the playlists YouTube makes have other ids.
pub fn is_own(playlist_id: &str, has_owner: bool) -> bool {
    !has_owner && bare_id(playlist_id).starts_with("PL")
}

/// The new playlist's id in a `playlist/create` answer.
fn created_id(response: &Value) -> Result<String, String> {
    response
        .get("playlistId")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .ok_or_else(|| error_message(response, "YouTube did not create the playlist"))
}

/// Whether an `edit_playlist` answer says the change was made.
fn succeeded(response: &Value) -> Result<(), String> {
    match response.get("status").and_then(Value::as_str) {
        Some("STATUS_SUCCEEDED") => Ok(()),
        _ => Err(error_message(
            response,
            "YouTube did not change the playlist",
        )),
    }
}

/// YouTube's own error text, if the answer has one, or `fallback`.
fn error_message(response: &Value, fallback: &str) -> String {
    response
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map_or_else(
            || fallback.to_string(),
            |message| format!("{fallback}: {message}"),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_answers() {
        assert_eq!(
            created_id(&json!({"playlistId": "PLabc"})),
            Ok("PLabc".to_string())
        );
        assert!(created_id(&json!({})).is_err());
        assert_eq!(succeeded(&json!({"status": "STATUS_SUCCEEDED"})), Ok(()));
        let failed = succeeded(&json!({"status": "STATUS_FAILED", "error": {"message": "No"}}));
        assert_eq!(
            failed,
            Err("YouTube did not change the playlist: No".to_string())
        );
    }

    #[test]
    fn an_expired_session_says_so() {
        use rustypipe::error::Error;
        let expired = Error::HttpStatus(401, "You must be signed in".into());
        assert_eq!(request_error(&expired), SESSION_EXPIRED);
        let other = Error::HttpStatus(500, "oops".into());
        assert_eq!(request_error(&other), "http status code: 500 message: oops");
    }

    #[test]
    fn only_the_users_playlists_are_their_own() {
        assert!(is_own("PLEphN123", false));
        assert!(is_own("VLPLEphN123", false));
        assert!(!is_own("PLEphN123", true), "someone else's");
        assert!(!is_own("LM", false), "Liked Music");
        assert!(!is_own("SE", false), "Episodes for later");
        assert!(!is_own("RDCLAK5uy", false), "made by YouTube");
    }

    /// Creates a private playlist on the signed-in account, adds a music video, takes it out by
    /// the id the playlist shows (as the ✕ does) and deletes the playlist:
    /// `cargo test -- --ignored edits_a_real_playlist`.
    #[tokio::test]
    #[ignore = "changes the signed-in YouTube Music account"]
    async fn edits_a_real_playlist() {
        let rp = crate::youtube::client().expect("client");
        let id = create(
            rp.clone(),
            "Encore test (delete me)".to_string(),
            Privacy::Private,
            vec!["Rgrt_8mXrK8".to_string()],
        )
        .await
        .expect("create");
        let result = async {
            // A music video: YouTube Music keeps the song version of it instead
            // (dQw4w9WgXcQ shows as lYBUbBu4W08).
            add(rp.clone(), id.clone(), "dQw4w9WgXcQ".to_string()).await?;
            let after_add = settled_songs(&rp, &id, 2).await?;
            let shown = after_add.get(1).cloned().unwrap_or_default();
            remove(rp.clone(), id.clone(), shown).await?;
            let after_remove = settled_songs(&rp, &id, 1).await?;
            Ok::<_, String>((after_add, after_remove))
        }
        .await;
        delete(rp, id).await.expect("delete");
        let (after_add, after_remove) = result.expect("edit");
        assert_eq!(after_add.len(), 2, "after adding: {after_add:?}");
        assert_eq!(after_add[0], "Rgrt_8mXrK8", "after adding: {after_add:?}");
        assert_eq!(after_remove, ["Rgrt_8mXrK8"], "after removing");
    }

    /// The songs in playlist `id`, read again for a few seconds until there are `count` of them:
    /// YouTube can answer with the playlist as it was a moment before.
    async fn settled_songs(rp: &RustyPipe, id: &str, count: usize) -> Result<Vec<String>, String> {
        let mut songs = Vec::new();
        for _ in 0..5 {
            let playlist = rp
                .query()
                .authenticated()
                .music_playlist(id)
                .await
                .map_err(|error| error.to_string())?;
            songs = playlist
                .tracks
                .items
                .into_iter()
                .map(|track| track.id)
                .collect();
            if songs.len() == count {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        Ok(songs)
    }
}
