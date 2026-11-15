//! The user's playlists: creating one, adding songs to it and taking them out (on YouTube
//! Music), and a cover of the user's own kept on this computer. The dialogs for all of it float
//! over the window; a short note at the bottom says what happened.

pub mod covers;
pub mod edit;
mod view;

use std::path::PathBuf;
use std::time::Duration;

use iced::Task;
use iced::widget::image::Handle;
use rust_i18n::t;

use crate::library::Account;
use crate::sidebar::NavItem;
use crate::youtube::{SearchTrack, Tile, TileKind};
use crate::{App, Message};
pub use edit::Privacy;
pub use view::{dialog, toast};

/// How long the note at the bottom stays.
const TOAST_TIME: Duration = Duration::from_secs(3);

/// A dialog over the window.
#[derive(Debug, Clone)]
pub enum Dialog {
    /// Making a playlist, with `song` in it when it comes from "Add to playlist".
    New {
        name: String,
        privacy: Privacy,
        song: Option<SearchTrack>,
        busy: bool,
        error: Option<String>,
    },
    /// Picking the playlist to add `song` to.
    AddTo {
        song: SearchTrack,
        busy: bool,
        error: Option<String>,
    },
    /// Changing the cover of playlist `id`.
    Cover {
        id: String,
        title: String,
        /// The image's path, typed or picked.
        path: String,
        busy: bool,
        error: Option<String>,
    },
}

impl Dialog {
    fn new_playlist(song: Option<SearchTrack>) -> Self {
        Dialog::New {
            name: String::new(),
            privacy: Privacy::default(),
            song,
            busy: false,
            error: None,
        }
    }

    fn fail(&mut self, message: String) {
        match self {
            Dialog::New { busy, error, .. }
            | Dialog::AddTo { busy, error, .. }
            | Dialog::Cover { busy, error, .. } => {
                *busy = false;
                *error = Some(message);
            }
        }
    }
}

/// Something done in the playlist dialogs, or its answer.
#[derive(Debug, Clone)]
pub enum Event {
    OpenNew,
    OpenAddTo(SearchTrack),
    OpenCover {
        id: String,
        title: String,
    },
    Close,
    NameChanged(String),
    PrivacyPicked(Privacy),
    Create,
    /// "New playlist" inside "Add to playlist": a new playlist with that song.
    NewWithSong,
    Created {
        title: String,
        with_song: bool,
        result: Result<String, String>,
    },
    AddTo {
        id: String,
        title: String,
    },
    Added {
        id: String,
        title: String,
        song: SearchTrack,
        /// Where the song sits in the playlist, if YouTube said.
        result: Result<Option<String>, String>,
    },
    Remove {
        id: String,
        song_id: String,
        entry: Option<String>,
    },
    Removed {
        id: String,
        song_id: String,
        result: Result<(), String>,
    },
    CoverPathChanged(String),
    BrowseCover,
    CoverPicked(Option<PathBuf>),
    UseCover,
    CoverSaved {
        id: String,
        result: Result<Handle, String>,
    },
    ResetCover,
    /// The note shown `serial`-th goes away, unless a newer one replaced it.
    ToastDone(u64),
}

impl App {
    pub(crate) fn playlist_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::OpenNew => self.dialog = Some(Dialog::new_playlist(None)),
            Event::OpenAddTo(song) => {
                self.dialog = Some(Dialog::AddTo {
                    song,
                    busy: false,
                    error: None,
                });
            }
            Event::OpenCover { id, title } => {
                self.dialog = Some(Dialog::Cover {
                    id,
                    title,
                    path: String::new(),
                    busy: false,
                    error: None,
                });
            }
            Event::Close => self.dialog = None,
            Event::NameChanged(text) => {
                if let Some(Dialog::New { name, .. }) = &mut self.dialog {
                    *name = text;
                }
            }
            Event::PrivacyPicked(picked) => {
                if let Some(Dialog::New { privacy, .. }) = &mut self.dialog {
                    *privacy = picked;
                }
            }
            Event::NewWithSong => {
                if let Some(Dialog::AddTo { song, .. }) = self.dialog.take() {
                    self.dialog = Some(Dialog::new_playlist(Some(song)));
                }
            }
            Event::Create => return self.create_playlist(),
            Event::Created {
                title,
                with_song,
                result,
            } => return self.playlist_created(title, with_song, result),
            Event::AddTo { id, title } => return self.add_to_playlist(id, title),
            Event::Added {
                id,
                title,
                song,
                result,
            } => match result {
                Ok(entry) => {
                    self.dialog = None;
                    self.online.song_added(&id, SearchTrack { entry, ..song });
                    return self.show_toast(t!("playlists.added_to", title = title).into_owned());
                }
                Err(error) => match &mut self.dialog {
                    Some(dialog @ Dialog::AddTo { .. }) => dialog.fail(error),
                    _ => return self.show_toast(error),
                },
            },
            Event::Remove { id, song_id, entry } => {
                let Some(client) = self.account_client() else {
                    return self.show_toast(t!("playlists.sign_in_to_change").into_owned());
                };
                return Task::perform(
                    edit::remove(client, id.clone(), song_id.clone(), entry),
                    move |result| {
                        Message::Playlist(Event::Removed {
                            id: id.clone(),
                            song_id: song_id.clone(),
                            result,
                        })
                    },
                );
            }
            Event::Removed {
                id,
                song_id,
                result,
            } => {
                return match result {
                    Ok(()) => {
                        self.online.song_removed(&id, &song_id);
                        self.show_toast(t!("playlists.removed").into_owned())
                    }
                    Err(error) => self.show_toast(error),
                };
            }
            Event::CoverPathChanged(text) => {
                if let Some(Dialog::Cover { path, .. }) = &mut self.dialog {
                    *path = text;
                }
            }
            Event::BrowseCover => {
                return Task::perform(covers::pick(), |path| {
                    Message::Playlist(Event::CoverPicked(path))
                });
            }
            Event::CoverPicked(picked) => {
                if let (Some(picked), Some(Dialog::Cover { path, .. })) = (picked, &mut self.dialog)
                {
                    *path = picked.display().to_string();
                    return self.use_cover();
                }
            }
            Event::UseCover => return self.use_cover(),
            Event::CoverSaved { id, result } => match result {
                Ok(cover) => {
                    self.online.set_cover(id, Some(cover));
                    self.dialog = None;
                    return self.show_toast(t!("playlists.cover_changed").into_owned());
                }
                Err(error) => {
                    if let Some(dialog) = &mut self.dialog {
                        dialog.fail(error);
                    }
                }
            },
            Event::ResetCover => {
                if let Some(Dialog::Cover { id, .. }) = &self.dialog {
                    let id = id.clone();
                    match covers::remove(&id) {
                        Ok(()) => {
                            self.online.set_cover(id, None);
                            self.dialog = None;
                        }
                        Err(error) => {
                            if let Some(dialog) = &mut self.dialog {
                                dialog.fail(error);
                            }
                        }
                    }
                }
            }
            Event::ToastDone(serial) => {
                if self
                    .toast
                    .as_ref()
                    .is_some_and(|(shown, _)| *shown == serial)
                {
                    self.toast = None;
                }
            }
        }
        Task::none()
    }

    /// The YouTube client, when someone is signed in.
    fn account_client(&self) -> Option<rustypipe::client::RustyPipe> {
        matches!(self.account, Account::SignedIn(_))
            .then(|| self.online.client())
            .flatten()
    }

    fn create_playlist(&mut self) -> Task<Message> {
        let client = self.account_client();
        let Some(Dialog::New {
            name,
            privacy,
            song,
            busy,
            error,
        }) = &mut self.dialog
        else {
            return Task::none();
        };
        if *busy {
            return Task::none();
        }
        let title = name.trim().to_string();
        if title.is_empty() {
            *error = Some(t!("playlists.name_needed").into_owned());
            return Task::none();
        }
        let Some(client) = client else {
            *error = Some(t!("playlists.sign_in_to_make").into_owned());
            return Task::none();
        };
        *busy = true;
        *error = None;
        let songs: Vec<String> = song.iter().map(|song| song.id.clone()).collect();
        let with_song = !songs.is_empty();
        Task::perform(
            edit::create(client, title.clone(), *privacy, songs),
            move |result| {
                Message::Playlist(Event::Created {
                    title: title.clone(),
                    with_song,
                    result,
                })
            },
        )
    }

    /// Adds the new playlist to the library. Made from the sidebar, it opens; made for a song,
    /// the note says the song is in it.
    fn playlist_created(
        &mut self,
        title: String,
        with_song: bool,
        result: Result<String, String>,
    ) -> Task<Message> {
        let id = match result {
            Ok(id) => id,
            Err(error) => {
                return match &mut self.dialog {
                    Some(dialog @ Dialog::New { .. }) => {
                        dialog.fail(error);
                        Task::none()
                    }
                    _ => self.show_toast(error),
                };
            }
        };
        self.dialog = None;
        let Account::SignedIn(data) = &mut self.account else {
            return Task::none();
        };
        let subtitle = data.profile.as_ref().map_or_else(
            || t!("playlists.tile_subtitle"),
            |profile| t!("playlists.tile_subtitle_owner", owner = profile.name),
        );
        data.playlists.insert(
            0,
            Tile {
                kind: TileKind::Playlist,
                id: id.clone(),
                title: title.clone(),
                subtitle: subtitle.into_owned(),
                image_url: None,
            },
        );
        data.own_playlists.push(id);
        crate::library::save_cache(data);
        // The sidebar lists playlists by position; the new one pushed the others down.
        if let NavItem::Playlist(index) = self.sidebar.selected {
            self.sidebar.selected = NavItem::Playlist(index + 1);
        }
        if with_song {
            self.show_toast(t!("playlists.added_to", title = title).into_owned())
        } else {
            Task::batch([
                self.select(NavItem::Playlist(0)),
                self.show_toast(t!("playlists.created", title = title).into_owned()),
            ])
        }
    }

    fn add_to_playlist(&mut self, id: String, title: String) -> Task<Message> {
        let client = self.account_client();
        let Some(Dialog::AddTo { song, busy, error }) = &mut self.dialog else {
            return Task::none();
        };
        if *busy {
            return Task::none();
        }
        let Some(client) = client else {
            *error = Some(t!("playlists.sign_in_to_add").into_owned());
            return Task::none();
        };
        *busy = true;
        *error = None;
        let song = song.clone();
        Task::perform(
            edit::add(client, id.clone(), song.id.clone()),
            move |result| {
                Message::Playlist(Event::Added {
                    id: id.clone(),
                    title: title.clone(),
                    song: song.clone(),
                    result,
                })
            },
        )
    }

    /// Saves the image in the cover dialog as the playlist's cover, off the interface thread.
    fn use_cover(&mut self) -> Task<Message> {
        let Some(Dialog::Cover {
            id,
            path,
            busy,
            error,
            ..
        }) = &mut self.dialog
        else {
            return Task::none();
        };
        let typed = path.trim();
        if typed.is_empty() {
            *error = Some(t!("playlists.pick_image").into_owned());
            return Task::none();
        }
        let source = expand_home(typed);
        *busy = true;
        *error = None;
        let id = id.clone();
        Task::perform(
            {
                let id = id.clone();
                async move {
                    tokio::task::spawn_blocking(move || covers::save(&id, &source))
                        .await
                        .map_err(|error| error.to_string())?
                }
            },
            move |result| {
                Message::Playlist(Event::CoverSaved {
                    id: id.clone(),
                    result,
                })
            },
        )
    }

    /// Shows `message` at the bottom for a few seconds.
    pub(crate) fn show_toast(&mut self, message: String) -> Task<Message> {
        let serial = self.toast.as_ref().map_or(0, |(serial, _)| serial + 1);
        self.toast = Some((serial, message));
        Task::perform(tokio::time::sleep(TOAST_TIME), move |()| {
            Message::Playlist(Event::ToastDone(serial))
        })
    }

    /// The signed-in user's own playlists, which songs can be added to.
    pub(crate) fn own_playlists(&self) -> Vec<&Tile> {
        match &self.account {
            Account::SignedIn(data) => data
                .playlists
                .iter()
                .filter(|tile| data.is_own(&tile.id))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Whether playlist `id` is the signed-in user's, so its songs can be taken out.
    pub(crate) fn is_own_playlist(&self, id: &str) -> bool {
        match &self.account {
            Account::SignedIn(data) => data.is_own(id),
            _ => false,
        }
    }
}

/// `~/x` as a path in the home folder; anything else as it is.
fn expand_home(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_request_shows_in_its_dialog() {
        let mut dialog = Dialog::new_playlist(None);
        if let Dialog::New { busy, .. } = &mut dialog {
            *busy = true;
        }
        dialog.fail("No".to_string());
        assert!(matches!(
            dialog,
            Dialog::New { busy: false, error: Some(ref error), .. } if error == "No"
        ));
    }

    #[test]
    fn a_tilde_means_the_home_folder() {
        let home = PathBuf::from(std::env::var_os("HOME").expect("HOME"));
        assert_eq!(expand_home("~/a.png"), home.join("a.png"));
        assert_eq!(expand_home("/x/a.png"), PathBuf::from("/x/a.png"));
    }
}
