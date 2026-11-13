//! Offline downloads: whole albums, playlists and artists' top songs kept on disk, playable
//! without a connection. The songs are plain audio files in `~/.local/share/encore/offline/`,
//! next to a small `index.json` that lists the downloaded collections. No database.

use std::collections::HashSet;
use std::path::PathBuf;

use iced::Task;

use crate::Message;
use crate::download;
use crate::youtube::Collection;

/// Where a collection stands, for its Download button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    NotDownloaded,
    Downloading { done: usize, total: usize },
    Downloaded,
}

/// A download in progress: one song at a time, so YouTube is not hammered.
struct Job {
    collection: Collection,
    next: usize,
    failed: usize,
}

/// The downloaded collections and the download in progress.
pub struct Offline {
    collections: Vec<Collection>,
    job: Option<Job>,
    error: Option<String>,
}

impl Offline {
    /// Reads the index of downloaded collections. A missing or broken index means none.
    pub fn load() -> Self {
        let collections = std::fs::read_to_string(index_path())
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        Self {
            collections,
            job: None,
            error: None,
        }
    }

    /// The downloaded collections, newest first.
    pub fn collections(&self) -> &[Collection] {
        &self.collections
    }

    /// A downloaded collection by id.
    pub fn get(&self, id: &str) -> Option<&Collection> {
        self.collections
            .iter()
            .find(|collection| collection.id == id)
    }

    /// The last download error, if any.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Where the collection `id` stands.
    pub fn state(&self, id: &str) -> State {
        if let Some(job) = self.job.as_ref().filter(|job| job.collection.id == id) {
            return State::Downloading {
                done: job.next,
                total: job.collection.tracks.len(),
            };
        }
        if self.get(id).is_some() {
            State::Downloaded
        } else {
            State::NotDownloaded
        }
    }

    /// Starts downloading every song of `collection`.
    pub fn start(&mut self, collection: Collection) -> Task<Message> {
        if self.job.is_some() {
            self.error = Some("another download is in progress; wait for it to finish".to_string());
            return Task::none();
        }
        self.error = None;
        self.job = Some(Job {
            collection,
            next: 0,
            failed: 0,
        });
        self.download_next()
    }

    /// Moves on after a song finished (or failed) downloading.
    pub fn track_done(&mut self, result: Result<PathBuf, String>) -> Task<Message> {
        let Some(job) = &mut self.job else {
            return Task::none();
        };
        if result.is_err() {
            job.failed += 1;
        }
        job.next += 1;
        self.download_next()
    }

    fn download_next(&mut self) -> Task<Message> {
        let Some(job) = &self.job else {
            return Task::none();
        };
        if let Some(track) = job.collection.tracks.get(job.next) {
            return Task::perform(
                download::keep_offline(track.id.clone()),
                Message::OfflineTrackDone,
            );
        }

        // Every song was tried: remember the collection.
        let Some(job) = self.job.take() else {
            return Task::none();
        };
        if job.failed > 0 {
            self.error = Some(format!(
                "{} of {} songs could not be downloaded",
                job.failed,
                job.collection.tracks.len()
            ));
        }
        self.collections
            .retain(|collection| collection.id != job.collection.id);
        self.collections.insert(0, job.collection);
        if let Err(error) = save(&self.collections) {
            self.error = Some(format!("could not save the downloads list: {error}"));
        }
        Task::none()
    }

    /// Deletes a downloaded collection and the songs no other download uses.
    pub fn remove(&mut self, id: &str) {
        let Some(position) = self
            .collections
            .iter()
            .position(|collection| collection.id == id)
        else {
            return;
        };
        let removed = self.collections.remove(position);
        let dir = download::offline_dir();
        for song in songs_to_delete(&removed, &self.collections) {
            if let Some(path) = download::find_cached(&dir, &song) {
                // A file that is already gone is fine.
                let _ = std::fs::remove_file(path);
            }
        }
        if let Err(error) = save(&self.collections) {
            self.error = Some(format!("could not save the downloads list: {error}"));
        }
    }
}

fn index_path() -> PathBuf {
    download::offline_dir().join("index.json")
}

fn save(collections: &[Collection]) -> Result<(), String> {
    std::fs::create_dir_all(download::offline_dir()).map_err(|error| error.to_string())?;
    let json = serde_json::to_string_pretty(collections).map_err(|error| error.to_string())?;
    std::fs::write(index_path(), json).map_err(|error| error.to_string())
}

/// The songs of `removed` that no remaining download needs.
pub fn songs_to_delete(removed: &Collection, remaining: &[Collection]) -> Vec<String> {
    let kept: HashSet<&str> = remaining
        .iter()
        .flat_map(|collection| &collection.tracks)
        .map(|track| track.id.as_str())
        .collect();
    removed
        .tracks
        .iter()
        .filter(|track| !kept.contains(track.id.as_str()))
        .map(|track| track.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::youtube::{SearchTrack, TileKind};

    fn collection(id: &str, songs: &[&str]) -> Collection {
        Collection {
            id: id.to_string(),
            kind: TileKind::Playlist,
            title: id.to_string(),
            subtitle: String::new(),
            image_url: None,
            tracks: songs
                .iter()
                .map(|song| SearchTrack {
                    id: (*song).to_string(),
                    title: (*song).to_string(),
                    artist: String::new(),
                    album: String::new(),
                    duration: 0,
                    cover_url: None,
                })
                .collect(),
        }
    }

    #[test]
    fn keeps_songs_that_other_downloads_use() {
        let removed = collection("a", &["x", "y", "z"]);
        let others = [collection("b", &["y"])];
        assert_eq!(songs_to_delete(&removed, &others), vec!["x", "z"]);
    }

    #[test]
    fn the_index_round_trips_through_json() {
        let list = vec![collection("a", &["x"]), collection("b", &["y", "z"])];
        let json = serde_json::to_string(&list).expect("serialize");
        let back: Vec<Collection> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, list);
    }

    #[test]
    fn reports_the_state_of_a_collection() {
        let mut offline = Offline {
            collections: vec![collection("done", &["x"])],
            job: None,
            error: None,
        };
        assert_eq!(offline.state("done"), State::Downloaded);
        assert_eq!(offline.state("other"), State::NotDownloaded);
        offline.job = Some(Job {
            collection: collection("busy", &["a", "b", "c"]),
            next: 1,
            failed: 0,
        });
        assert_eq!(
            offline.state("busy"),
            State::Downloading { done: 1, total: 3 }
        );
    }
}
