//! Downloads for offline listening: fetching a collection's whole song list before it
//! downloads, and moving the downloads to the folder chosen in the settings.

use std::path::PathBuf;

use iced::Task;
use rust_i18n::t;

use crate::offline::{self, FolderStatus, Offline};
use crate::youtube::{self, Collection, Tile};
use crate::{App, Message, download};

/// Asks the user for a folder to keep the downloads in.
pub(super) fn pick_folder() -> Task<Message> {
    let title = t!("audio.download_folder").into_owned();
    Task::perform(
        async move {
            rfd::AsyncFileDialog::new()
                .set_title(title)
                .pick_folder()
                .await
                .map(|folder| folder.path().to_path_buf())
        },
        Message::DownloadFolderPicked,
    )
}

/// The card that opens `collection`.
pub(super) fn tile_of(collection: &Collection) -> Tile {
    Tile {
        kind: collection.kind,
        id: collection.id.clone(),
        title: collection.title.clone(),
        subtitle: collection.subtitle.clone(),
        image_url: collection.image_url.clone(),
    }
}

impl App {
    /// Download pressed: every page of a playlist is fetched first, so a long one downloads
    /// whole even when only its first songs had loaded on screen.
    pub(super) fn download_collection(&mut self, collection: Collection) -> Task<Message> {
        if self.offline.state(&collection.id) != offline::State::NotDownloaded {
            return Task::none();
        }
        let Some(client) = self.online.client() else {
            let task = self.offline.start(collection);
            return self.save_offline_covers(task);
        };
        self.offline.prepare(&collection.id);
        let tile = tile_of(&collection);
        Task::perform(youtube::open_whole(client, tile), move |whole| {
            Message::DownloadListReady(collection.clone(), whole)
        })
    }

    /// A downloaded playlist was opened: its song list is fetched again to see whether it
    /// changed on YouTube Music, which turns its Download button into Update.
    pub(super) fn check_download(&self, tile: &Tile) -> Task<Message> {
        if !self.offline.wants_check(&tile.id, tile.kind) {
            return Task::none();
        }
        let Some(client) = self.online.client() else {
            return Task::none();
        };
        let id = tile.id.clone();
        Task::perform(youtube::open_whole(client, tile.clone()), move |fresh| {
            Message::DownloadChecked(id.clone(), fresh)
        })
    }

    /// Moves the downloads to `folder` (`None` is Encore's own), then keeps them there.
    pub(super) fn move_downloads(&mut self, folder: Option<PathBuf>) -> Task<Message> {
        if self.download_folder.moving {
            return Task::none();
        }
        if self.offline.is_busy() {
            self.download_folder.error = Some(t!("audio.download_folder_busy").into_owned());
            return Task::none();
        }
        let from = download::offline_dir();
        let to = folder.clone().unwrap_or_else(download::default_offline_dir);
        if to == from {
            self.download_folder.error = None;
            self.keep_downloads_in(folder);
            return Task::none();
        }
        self.download_folder = FolderStatus {
            moving: true,
            error: None,
        };
        Task::perform(offline::move_downloads(from, to), move |result| {
            Message::DownloadFolderMoved(folder.clone(), result)
        })
    }

    /// The downloads finished moving to `folder`, or could not.
    pub(super) fn downloads_moved(&mut self, folder: Option<PathBuf>, result: Result<(), String>) {
        self.download_folder.moving = false;
        match result {
            Ok(()) => {
                self.keep_downloads_in(folder);
                self.offline = Offline::load();
            }
            Err(error) => self.download_folder.error = Some(error),
        }
    }

    fn keep_downloads_in(&mut self, folder: Option<PathBuf>) {
        download::set_offline_folder(folder.clone());
        self.audio_settings.download_folder = folder;
        self.audio_settings.save();
    }
}
