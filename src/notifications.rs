//! Desktop notifications when a song starts, through the freedesktop notification service.

use std::sync::atomic::{AtomicU32, Ordering};

use notify_rust::{Notification, Timeout};

/// The last notification shown, so the next one replaces it instead of piling up.
static LAST_ID: AtomicU32 = AtomicU32::new(0);

/// What a song notification says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongNotice {
    pub title: String,
    pub artist: String,
    pub album: String,
    /// A cover image on disk, if it is downloaded.
    pub image_path: Option<String>,
}

impl SongNotice {
    /// The second line: "Artist — Album", or just the artist.
    pub fn body(&self) -> String {
        if self.album.is_empty() {
            self.artist.clone()
        } else {
            format!("{} \u{2014} {}", self.artist, self.album)
        }
    }
}

/// Shows (or replaces) the notification for a song. A desktop without a notification service
/// just shows nothing.
pub async fn show(notice: SongNotice) {
    let mut notification = Notification::new();
    notification
        .appname("Encore")
        .summary(&notice.title)
        .body(&notice.body())
        .icon("audio-x-generic")
        .timeout(Timeout::Milliseconds(5000));
    if let Some(path) = &notice.image_path {
        notification.image_path(path);
    }
    let last = LAST_ID.load(Ordering::Relaxed);
    if last != 0 {
        notification.id(last);
    }
    if let Ok(handle) = notification.show_async().await {
        LAST_ID.store(handle.id(), Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_names_the_album_when_known() {
        let notice = SongNotice {
            title: "Usseewa".to_string(),
            artist: "Ado".to_string(),
            album: "Kyougen".to_string(),
            image_path: None,
        };
        assert_eq!(notice.body(), "Ado \u{2014} Kyougen");
        let single = SongNotice {
            album: String::new(),
            ..notice
        };
        assert_eq!(single.body(), "Ado");
    }
}
