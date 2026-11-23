//! Offline downloads: whole albums, playlists and artists' top songs kept on disk, playable
//! without a connection. The songs are plain audio files in the downloads folder
//! (`~/.local/share/encore/offline/` unless the user chose another), next to a small
//! `index.json` that lists the downloaded collections and `pending.json` with the ones still
//! downloading. No database.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use iced::Task;
use rust_i18n::t;

use crate::Message;
use crate::download;
use crate::youtube::Collection;

mod changes;
mod folder;

pub use changes::Changes;
pub use folder::move_downloads;

/// The downloads folder setting: whether the downloads are being moved, and the last error.
#[derive(Debug, Default)]
pub struct FolderStatus {
    pub moving: bool,
    pub error: Option<String>,
}

/// Where a collection stands, for its Download button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    NotDownloaded,
    /// Its whole song list is being fetched before the download starts.
    Preparing,
    /// Queued behind another download.
    Waiting,
    Downloading {
        done: usize,
        total: usize,
    },
    Downloaded,
    /// Downloaded, but the playlist changed on YouTube Music since.
    Outdated(Changes),
}

/// The download in progress: one song at a time, so YouTube is not hammered.
struct Job {
    /// The collection, which is in `collections` from the moment it is queued.
    id: String,
    next: usize,
    failed: usize,
}

impl Job {
    fn new(id: String) -> Self {
        Self {
            id,
            next: 0,
            failed: 0,
        }
    }
}

/// The downloaded collections and the downloads in progress.
///
/// A collection joins the list (and `index.json`) as soon as its download is asked for, and
/// `pending.json` lists the ones not finished yet. Encore picks those up when it starts again;
/// songs already on disk are skipped at once.
pub struct Offline {
    collections: Vec<Collection>,
    job: Option<Job>,
    /// Collections queued after the job, in order.
    waiting: Vec<String>,
    /// Collections whose whole song list is being fetched.
    preparing: HashSet<String>,
    error: Option<String>,
    /// A collection whose download was just asked for, until the app saves its covers.
    new: Option<Collection>,
    /// The song lists on YouTube Music of downloaded playlists that changed since.
    outdated: HashMap<String, Vec<crate::youtube::SearchTrack>>,
}

impl Offline {
    /// Reads the index of downloaded collections and the unfinished downloads. A missing or
    /// broken file means none.
    pub fn load() -> Self {
        let collections: Vec<Collection> = read_json(&index_path()).unwrap_or_default();
        let mut pending: Vec<String> = read_json(&pending_path()).unwrap_or_default();
        pending.retain(|id| collections.iter().any(|collection| &collection.id == id));
        let mut pending = pending.into_iter();
        Self {
            job: pending.next().map(Job::new),
            waiting: pending.collect(),
            collections,
            preparing: HashSet::new(),
            error: None,
            new: None,
            outdated: HashMap::new(),
        }
    }

    /// Goes on with the downloads that were not finished when Encore closed.
    pub fn resume(&mut self) -> Task<Message> {
        self.download_next()
    }

    /// The downloaded collections, newest first, with the ones still downloading.
    pub fn collections(&self) -> &[Collection] {
        &self.collections
    }

    /// A downloaded (or downloading) collection by id.
    pub fn get(&self, id: &str) -> Option<&Collection> {
        self.collections
            .iter()
            .find(|collection| collection.id == id)
    }

    /// The last download error, if any.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Whether anything is downloading, waiting or being prepared.
    pub fn is_busy(&self) -> bool {
        self.job.is_some() || !self.waiting.is_empty() || !self.preparing.is_empty()
    }

    /// Where the collection `id` stands.
    pub fn state(&self, id: &str) -> State {
        if self.preparing.contains(id) {
            return State::Preparing;
        }
        if let Some(job) = self.job.as_ref().filter(|job| job.id == id) {
            let total = self.get(id).map_or(0, |collection| collection.tracks.len());
            return State::Downloading {
                done: job.next.min(total),
                total,
            };
        }
        if self.waiting.iter().any(|waiting| waiting == id) {
            return State::Waiting;
        }
        if let Some(changes) = self.changes_of(id) {
            State::Outdated(changes)
        } else if self.get(id).is_some() {
            State::Downloaded
        } else {
            State::NotDownloaded
        }
    }

    /// Notes that the whole song list of `id` is being fetched for a download.
    pub fn prepare(&mut self, id: &str) {
        self.preparing.insert(id.to_string());
    }

    /// Queues every song of `collection`, and starts if nothing else is downloading.
    pub fn start(&mut self, collection: Collection) -> Task<Message> {
        self.preparing.remove(&collection.id);
        let id = collection.id.clone();
        if self.job.as_ref().is_some_and(|job| job.id == id) || self.waiting.contains(&id) {
            return Task::none();
        }
        self.error = None;
        self.outdated.remove(&id);
        self.collections.retain(|kept| kept.id != id);
        self.new = Some(collection.clone());
        self.collections.insert(0, collection);
        if self.job.is_none() {
            self.job = Some(Job::new(id));
        } else {
            self.waiting.push(id);
        }
        self.save_all();
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
        loop {
            let Some(job) = &self.job else {
                return Task::none();
            };
            let track = self
                .get(&job.id)
                .and_then(|collection| collection.tracks.get(job.next));
            if let Some(track) = track {
                return Task::perform(
                    download::keep_offline(track.id.clone()),
                    Message::OfflineTrackDone,
                );
            }
            // Every song was tried: the next collection in line, if any.
            self.finish_job();
        }
    }

    fn finish_job(&mut self) {
        let Some(job) = self.job.take() else {
            return;
        };
        if job.failed > 0 {
            let total = self
                .get(&job.id)
                .map_or(0, |collection| collection.tracks.len());
            self.error =
                Some(t!("offline.some_failed", failed = job.failed, total = total).into_owned());
        }
        if !self.waiting.is_empty() {
            self.job = Some(Job::new(self.waiting.remove(0)));
        }
        self.save_all();
    }

    /// The unfinished downloads, the one in progress first.
    fn pending(&self) -> Vec<String> {
        self.job
            .iter()
            .map(|job| job.id.clone())
            .chain(self.waiting.iter().cloned())
            .collect()
    }

    fn save_all(&mut self) {
        // Tests must never touch the user's downloads.
        if cfg!(test) {
            return;
        }
        let saved = save(&self.collections).and_then(|()| save_pending(&self.pending()));
        if let Err(error) = saved {
            self.error = Some(t!("offline.save_failed", error = error).into_owned());
        }
    }

    /// Takes song `song` out of the downloaded collection `id`, deleting its file unless
    /// another download uses it.
    pub fn remove_song(&mut self, id: &str, song: &str) {
        let Some(collection) = self
            .collections
            .iter_mut()
            .find(|collection| collection.id == id)
        else {
            return;
        };
        let Some(position) = collection.tracks.iter().position(|track| track.id == song) else {
            return;
        };
        collection.tracks.remove(position);
        // A download in progress goes on from the same song.
        if let Some(job) = self.job.as_mut().filter(|job| job.id == id)
            && position < job.next
        {
            job.next -= 1;
        }
        let still_used = self
            .collections
            .iter()
            .any(|collection| collection.tracks.iter().any(|track| track.id == song));
        // Tests must never touch the user's downloads.
        if !still_used
            && !cfg!(test)
            && let Some(path) = download::find_cached(&download::offline_dir(), song)
        {
            // A file that is already gone is fine.
            let _ = std::fs::remove_file(path);
        }
        self.save_all();
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
        self.waiting.retain(|waiting| waiting != id);
        if self.job.as_ref().is_some_and(|job| job.id == id) {
            self.job = None;
            if !self.waiting.is_empty() {
                self.job = Some(Job::new(self.waiting.remove(0)));
            }
        }
        self.outdated.remove(id);
        self.forget_files(&removed);
        self.save_all();
    }
}

impl Offline {
    /// The collection whose download was just asked for, once, so its covers get saved.
    pub fn take_new(&mut self) -> Option<Collection> {
        self.new.take()
    }

    /// Every saved cover of the downloaded collections, to show them without a connection.
    pub fn saved_images(&self) -> Vec<(String, Vec<u8>)> {
        let urls: HashSet<&str> = self.collections.iter().flat_map(image_urls).collect();
        urls.into_iter()
            .filter_map(|url| {
                let bytes = std::fs::read(images_dir().join(image_file_name(url))).ok()?;
                Some((url.to_string(), bytes))
            })
            .collect()
    }

    /// Whether `url` is a cover of a downloaded collection that is not saved yet.
    pub fn wants_image(&self, url: &str) -> bool {
        let used = self
            .collections
            .iter()
            .any(|collection| image_urls(collection).any(|image| image == url));
        used && !images_dir().join(image_file_name(url)).exists()
    }

    /// Saves a cover next to the downloaded songs.
    pub fn save_image(&self, url: &str, bytes: &[u8]) {
        if std::fs::create_dir_all(images_dir()).is_ok() {
            // A cover that fails to save only means a gray square offline.
            let _ = std::fs::write(images_dir().join(image_file_name(url)), bytes);
        }
    }
}

/// The covers a collection shows: its own image and each song's cover.
pub fn image_urls(collection: &Collection) -> impl Iterator<Item = &str> {
    collection.image_url.as_deref().into_iter().chain(
        collection
            .tracks
            .iter()
            .filter_map(|track| track.cover_url.as_deref()),
    )
}

fn images_dir() -> PathBuf {
    download::offline_dir().join("images")
}

/// A file name for an image URL that is the same on every run (64-bit FNV-1a in hex).
pub fn image_file_name(url: &str) -> String {
    let hash = url.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("{hash:016x}")
}

/// The downloaded collections, in `dir` (the downloads folder).
const INDEX: &str = "index.json";
/// The collections not fully downloaded yet.
const PENDING: &str = "pending.json";

fn index_path() -> PathBuf {
    download::offline_dir().join(INDEX)
}

fn pending_path() -> PathBuf {
    download::offline_dir().join(PENDING)
}

fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn write_json<T: serde::Serialize>(path: &std::path::Path, value: &T) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    // Written next to the old file and renamed over it, so a crash never leaves half a file.
    let partial = path.with_extension("json.part");
    std::fs::write(&partial, json).map_err(|error| error.to_string())?;
    std::fs::rename(&partial, path).map_err(|error| error.to_string())
}

fn save(collections: &[Collection]) -> Result<(), String> {
    write_json(&index_path(), &collections)
}

fn save_pending(pending: &[String]) -> Result<(), String> {
    if pending.is_empty() {
        // A file that is already gone is fine.
        let _ = std::fs::remove_file(pending_path());
        return Ok(());
    }
    write_json(&pending_path(), &pending)
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
                    entry: None,
                })
                .collect(),
            artist: None,
        }
    }

    #[test]
    fn image_names_are_stable_and_distinct() {
        assert_eq!(image_file_name(""), "cbf29ce484222325");
        assert_eq!(image_file_name("a"), "af63dc4c8601ec8c");
        assert_ne!(
            image_file_name("https://x/1"),
            image_file_name("https://x/2")
        );
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

    pub(super) fn offline_with(collections: Vec<Collection>) -> Offline {
        Offline {
            collections,
            job: None,
            waiting: Vec::new(),
            preparing: HashSet::new(),
            error: None,
            new: None,
            outdated: HashMap::new(),
        }
    }

    #[test]
    fn reports_the_state_of_a_collection() {
        let mut offline = offline_with(vec![
            collection("done", &["x"]),
            collection("busy", &["a", "b", "c"]),
            collection("later", &["d"]),
        ]);
        offline.job = Some(Job {
            id: "busy".to_string(),
            next: 1,
            failed: 0,
        });
        offline.waiting.push("later".to_string());
        offline.prepare("asked");
        assert_eq!(offline.state("done"), State::Downloaded);
        assert_eq!(offline.state("other"), State::NotDownloaded);
        assert_eq!(
            offline.state("busy"),
            State::Downloading { done: 1, total: 3 }
        );
        assert_eq!(offline.state("later"), State::Waiting);
        assert_eq!(offline.state("asked"), State::Preparing);
        assert!(offline.is_busy());
    }

    #[test]
    fn the_unfinished_downloads_come_first_in_line() {
        let mut offline = offline_with(vec![collection("a", &[]), collection("b", &[])]);
        offline.job = Some(Job::new("a".to_string()));
        offline.waiting.push("b".to_string());
        assert_eq!(offline.pending(), vec!["a", "b"]);
        // "a" has no songs left: "b" takes its place.
        offline.finish_job();
        assert_eq!(offline.pending(), vec!["b"]);
        offline.finish_job();
        assert!(offline.pending().is_empty());
        assert!(!offline.is_busy());
    }

    #[test]
    fn a_song_leaves_a_download_and_the_download_goes_on_from_the_same_song() {
        let mut offline = offline_with(vec![collection("a", &["x", "y", "z"])]);
        offline.job = Some(Job {
            id: "a".to_string(),
            next: 2,
            failed: 0,
        });
        offline.remove_song("a", "x");
        let ids: Vec<&str> = offline
            .get("a")
            .expect("a")
            .tracks
            .iter()
            .map(|t| t.id.as_str())
            .collect();
        assert_eq!(ids, ["y", "z"]);
        assert_eq!(offline.state("a"), State::Downloading { done: 1, total: 2 });
    }
}
