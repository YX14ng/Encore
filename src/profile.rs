//! The signed-in user's YouTube Music profile: name, handle and photo, from the same account
//! menu request music.youtube.com makes for its avatar button.

use rustypipe::client::{ClientType, RustyPipe};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Who is signed in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    /// The channel handle, like "@someone".
    pub handle: Option<String>,
    pub photo_url: Option<String>,
}

/// Asks YouTube Music who is signed in. `None` if the request or the answer does not work out.
pub async fn fetch(rp: &RustyPipe) -> Option<Profile> {
    let body = serde_json::json!({});
    let response = rp
        .query()
        .authenticated()
        .raw(ClientType::DesktopMusic, "account/account_menu", &body)
        .await
        .ok()?;
    parse(&response)
}

/// Reads the profile out of an `account/account_menu` response.
pub fn parse(response: &str) -> Option<Profile> {
    let json: Value = serde_json::from_str(response).ok()?;
    let header = json.pointer(
        "/actions/0/openPopupAction/popup/multiPageMenuRenderer/header/activeAccountHeaderRenderer",
    )?;

    let name = header
        .pointer("/accountName/runs/0/text")?
        .as_str()?
        .to_string();
    let handle = header
        .pointer("/channelHandle/runs/0/text")
        .and_then(Value::as_str)
        .map(str::to_string);
    // The largest photo is the last one.
    let photo_url = header
        .pointer("/accountPhoto/thumbnails")
        .and_then(Value::as_array)
        .and_then(|thumbnails| thumbnails.last())
        .and_then(|thumbnail| thumbnail.get("url"))
        .and_then(Value::as_str)
        .map(str::to_string);

    Some(Profile {
        name,
        handle,
        photo_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_name_handle_and_largest_photo() {
        let response = r#"{"actions":[{"openPopupAction":{"popup":{"multiPageMenuRenderer":{
            "header":{"activeAccountHeaderRenderer":{
                "accountName":{"runs":[{"text":"Ada"}]},
                "channelHandle":{"runs":[{"text":"@ada"}]},
                "accountPhoto":{"thumbnails":[
                    {"url":"https://small","width":48},
                    {"url":"https://big","width":88}]}}}}}}}]}"#;
        assert_eq!(
            parse(response),
            Some(Profile {
                name: "Ada".to_string(),
                handle: Some("@ada".to_string()),
                photo_url: Some("https://big".to_string()),
            })
        );
    }

    #[test]
    fn a_signed_out_answer_has_no_profile() {
        assert_eq!(parse(r#"{"actions":[]}"#), None);
        assert_eq!(parse("not json"), None);
    }
}
