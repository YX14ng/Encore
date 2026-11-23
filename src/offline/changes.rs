//! Downloaded playlists that changed on YouTube Music: when one is opened, its whole song list
//! is fetched again and compared with the downloaded one. If songs were added or taken out,
//! its Download button turns into Update, which deletes the songs taken out (unless another
//! download uses them) and downloads the new ones.

use std::collections::HashSet;

use iced::Task;

use super::{Job, Offline, State, songs_to_delete};
use crate::Message;
use crate::youtube::{Collection, SearchTrack, TileKind};

/// How a playlist changed since it was downloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Changes {
    /// Songs on YouTube Music that are not downloaded.
    pub added: usize,
    /// Downloaded songs that are not on YouTube Music any more.
    pub removed: usize,
}

impl Changes {
    pub fn any(self) -> bool {
        self.added > 0 || self.removed > 0
    }
}

/// What changed from the `kept` songs to the `fresh` ones; the order does not count.
pub fn changes(kept: &[SearchTrack], fresh: &[SearchTrack]) -> Changes {
    let kept: HashSet<&str> = kept.iter().map(|track| track.id.as_str()).collect();
    let fresh: HashSet<&str> = fresh.iter().map(|track| track.id.as_str()).collect();
    Changes {
        added: fresh.difference(&kept).count(),
        removed: kept.difference(&fresh).count(),
    }
}

impl Offline {
    /// Whether opening collection `id` should look for changes on YouTube Music: a downloaded
    /// playlist that is not downloading. Albums do not change, and an artist's top songs
    /// change all the time.
    pub fn wants_check(&self, id: &str, kind: TileKind) -> bool {
        kind == TileKind::Playlist && self.state(id) == State::Downloaded
    }

    /// The whole song list of playlist `id`, as YouTube Music has it now: remembered if it
    /// differs from the downloaded one, so the playlist can be updated.
    pub fn checked(&mut self, id: &str, fresh: Vec<SearchTrack>) {
        let Some(kept) = self.get(id) else {
            return;
        };
        // An empty answer for a playlist that had songs is more likely an error than a
        // playlist emptied on purpose (that is still done with Remove), and a list cut at the
        // limit would count the songs past it as taken out.
        let suspicious = (fresh.is_empty() && !kept.tracks.is_empty())
            || fresh.len() >= crate::youtube::PLAYLIST_LIMIT;
        if !suspicious && changes(&kept.tracks, &fresh).any() {
            self.outdated.insert(id.to_string(), fresh);
        } else {
            self.outdated.remove(id);
        }
    }

    /// How playlist `id` changed, if it did and it can be updated.
    pub(super) fn changes_of(&self, id: &str) -> Option<Changes> {
        let fresh = self.outdated.get(id)?;
        let kept = self.get(id)?;
        Some(changes(&kept.tracks, fresh)).filter(|changes| changes.any())
    }

    /// Update pressed: the playlist takes its new song list, the songs taken out are deleted
    /// (unless another download uses them) and the new ones download.
    pub fn update(&mut self, id: &str) -> Task<Message> {
        if self.state(id) == State::NotDownloaded || self.pending().iter().any(|busy| busy == id) {
            return Task::none();
        }
        let Some(fresh) = self.outdated.remove(id) else {
            return Task::none();
        };
        let Some(collection) = self
            .collections
            .iter_mut()
            .find(|collection| collection.id == id)
        else {
            return Task::none();
        };
        let old = Collection {
            tracks: std::mem::replace(&mut collection.tracks, fresh),
            ..collection.clone()
        };
        self.new = Some(collection.clone());
        self.forget_files(&old);
        self.error = None;
        if self.job.is_none() {
            self.job = Some(Job::new(id.to_string()));
        } else {
            self.waiting.push(id.to_string());
        }
        self.save_all();
        // The songs already on disk are skipped at once.
        self.download_next()
    }

    /// Deletes the songs and covers of `old` that no download uses any more.
    pub(super) fn forget_files(&self, old: &Collection) {
        // Tests must never touch the user's downloads.
        if cfg!(test) {
            return;
        }
        let still_used: HashSet<&str> = self
            .collections
            .iter()
            .flat_map(super::image_urls)
            .collect();
        for url in super::image_urls(old).filter(|url| !still_used.contains(url)) {
            // A file that is already gone is fine.
            let _ = std::fs::remove_file(super::images_dir().join(super::image_file_name(url)));
        }
        let dir = crate::download::offline_dir();
        for song in songs_to_delete(old, &self.collections) {
            if let Some(path) = crate::download::find_cached(&dir, &song) {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(id: &str) -> SearchTrack {
        SearchTrack {
            id: id.to_string(),
            title: id.to_string(),
            artist: String::new(),
            album: String::new(),
            duration: 0,
            cover_url: None,
            entry: None,
        }
    }

    fn songs(ids: &[&str]) -> Vec<SearchTrack> {
        ids.iter().map(|id| song(id)).collect()
    }

    fn playlist(id: &str, ids: &[&str]) -> Collection {
        Collection {
            id: id.to_string(),
            kind: TileKind::Playlist,
            title: id.to_string(),
            subtitle: String::new(),
            image_url: None,
            tracks: songs(ids),
            artist: None,
        }
    }

    fn downloaded(collections: Vec<Collection>) -> Offline {
        crate::offline::tests::offline_with(collections)
    }

    #[test]
    fn changes_count_songs_added_and_taken_out_but_not_the_order() {
        let kept = songs(&["a", "b", "c"]);
        assert_eq!(
            changes(&kept, &songs(&["c", "a", "d", "e"])),
            Changes {
                added: 2,
                removed: 1
            }
        );
        assert!(!changes(&kept, &songs(&["c", "b", "a"])).any());
    }

    #[test]
    fn a_changed_playlist_asks_to_be_updated() {
        let mut offline = downloaded(vec![playlist("p", &["a", "b"])]);
        assert!(offline.wants_check("p", TileKind::Playlist));
        assert!(!offline.wants_check("p", TileKind::Album));

        offline.checked("p", songs(&["b", "a"]));
        assert_eq!(offline.state("p"), State::Downloaded, "same songs");

        offline.checked("p", songs(&["a", "c"]));
        assert_eq!(
            offline.state("p"),
            State::Outdated(Changes {
                added: 1,
                removed: 1
            })
        );

        offline.checked("p", Vec::new());
        assert_eq!(
            offline.state("p"),
            State::Downloaded,
            "empty is not trusted"
        );
    }

    #[test]
    fn updating_takes_the_new_list_and_downloads_it() {
        let mut offline = downloaded(vec![playlist("p", &["a", "b"])]);
        offline.checked("p", songs(&["a", "c"]));
        let _ = offline.update("p");
        let ids: Vec<&str> = offline.collections[0]
            .tracks
            .iter()
            .map(|track| track.id.as_str())
            .collect();
        assert_eq!(ids, ["a", "c"]);
        assert!(matches!(
            offline.state("p"),
            State::Downloading { total: 2, .. }
        ));
        assert!(offline.take_new().is_some(), "its covers get saved");
    }

    #[test]
    fn a_playlist_downloading_is_not_updated_twice() {
        let mut offline = downloaded(vec![playlist("p", &["a"])]);
        offline.checked("p", songs(&["a", "b"]));
        let _ = offline.update("p");
        offline.outdated.insert("p".to_string(), songs(&["c"]));
        let _ = offline.update("p");
        assert_eq!(offline.collections[0].tracks.len(), 2);
    }
}
