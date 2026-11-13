//! The track shown in the player bar, whether it is sample data or a real search result.

use iced::widget::image;

use crate::mock::{self, Track};
use crate::youtube::SearchTrack;

/// What the player bar needs to show a track.
pub struct Song {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub cover: Option<image::Handle>,
    /// The cover as a `file://` URL, for the desktop's media controls (MPRIS).
    pub art_url: Option<String>,
}

impl Song {
    /// A sample track, with its cover from `assets/covers/`.
    pub fn from_mock(track: &Track) -> Self {
        Self {
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            cover: Some(image::Handle::from_path(mock::cover_path(&track.cover))),
            art_url: Some(format!(
                "file://{}",
                mock::cover_path(&track.cover).display()
            )),
        }
    }

    /// A track from YouTube Music, with its cover if it was already downloaded.
    pub fn from_search(track: &SearchTrack, cover: Option<image::Handle>) -> Self {
        Self {
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            cover,
            // Covers are kept on disk once downloaded (see `download::bytes`).
            art_url: track.cover_url.as_deref().map(|url| {
                let path = crate::download::cache_dir()
                    .join("images")
                    .join(crate::offline::image_file_name(url));
                format!("file://{}", path.display())
            }),
        }
    }
}
