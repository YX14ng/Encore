//! Albums, playlists and artists opened lately, kept as they were left (pages loaded, songs
//! added or taken out) so opening one again shows it at once while a fresh copy loads.

use std::collections::VecDeque;

use crate::youtube::Collection;

/// How many collections are kept.
const KEPT: usize = 12;

/// The collections opened lately, the latest first.
#[derive(Debug, Default)]
pub struct Recent {
    collections: VecDeque<Collection>,
}

impl Recent {
    /// The kept copy of collection `id`.
    pub fn get(&self, id: &str) -> Option<&Collection> {
        self.collections
            .iter()
            .find(|collection| collection.id == id)
    }

    /// Keeps `collection`, replacing an older copy; the oldest goes once there are too many.
    pub fn keep(&mut self, collection: Collection) {
        self.collections.retain(|kept| kept.id != collection.id);
        self.collections.push_front(collection);
        self.collections.truncate(KEPT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::youtube::TileKind;

    fn collection(id: &str, songs: usize) -> Collection {
        Collection {
            id: id.to_string(),
            kind: TileKind::Playlist,
            title: id.to_string(),
            subtitle: String::new(),
            image_url: None,
            tracks: (0..songs)
                .map(|index| crate::youtube::SearchTrack {
                    id: format!("{id}-{index}"),
                    title: String::new(),
                    artist: String::new(),
                    album: String::new(),
                    duration: 0,
                    cover_url: None,
                    entry: None,
                })
                .collect(),
            artist: None,
        }
    }

    #[test]
    fn keeps_the_latest_copy_of_each() {
        let mut recent = Recent::default();
        recent.keep(collection("a", 1));
        recent.keep(collection("b", 1));
        recent.keep(collection("a", 3));
        assert_eq!(recent.get("a").map(|c| c.tracks.len()), Some(3));
        assert_eq!(recent.collections.len(), 2);
        for index in 0..KEPT {
            recent.keep(collection(&format!("x{index}"), 0));
        }
        assert!(recent.get("a").is_none(), "the oldest are dropped");
        assert_eq!(recent.collections.len(), KEPT);
    }

    #[test]
    fn a_collection_opened_again_shows_at_once_as_it_was_left() {
        let mut online =
            super::super::Online::new(&crate::audio_settings::AudioSettings::default());
        let _ = online.show_collection(collection("a", 250));
        online.song_removed("a", "a-0");
        let _ = online.show_collection(collection("b", 3));
        let tile = crate::youtube::Tile {
            kind: TileKind::Playlist,
            id: "a".to_string(),
            title: String::new(),
            subtitle: String::new(),
            image_url: None,
        };
        // The tasks are only built, so nothing is asked of YouTube.
        let _ = online.open(tile);
        let shown = online.collection().0.expect("shown at once");
        assert_eq!(shown.tracks.len(), 249, "with the song taken out");
        assert_eq!(online.collection_status, crate::search::Status::Done);
        assert!(online.recent.get("b").is_some(), "the one left is kept");
    }
}
