//! Going to sidebar entries and showing the library once it loads.

use iced::Task;

use crate::library::{Account, LibraryData, Shelf};
use crate::sidebar::NavItem;

use crate::{App, Message, Screen, library};

impl App {
    /// Highlights a sidebar entry and shows its screen.
    pub(crate) fn select(&mut self, item: NavItem) -> Task<Message> {
        if item == NavItem::NewPlaylist {
            return self.playlist_event(crate::playlists::Event::OpenNew);
        }
        self.sidebar.selected = item;
        self.history.clear();
        self.screen = match item {
            NavItem::History => Screen::Library(Shelf::History),
            NavItem::Songs => Screen::Library(Shelf::Songs),
            NavItem::LikedMusic => Screen::Library(Shelf::Liked),
            NavItem::Albums => Screen::Library(Shelf::Albums),
            NavItem::Artists => Screen::Library(Shelf::Artists),
            NavItem::Subscriptions => Screen::Library(Shelf::Subscriptions),
            NavItem::Podcasts => Screen::Library(Shelf::Podcasts),
            NavItem::Radio => Screen::Library(Shelf::Mixes),
            NavItem::Downloads => Screen::Library(Shelf::Downloads),
            NavItem::Account => Screen::Account,
            NavItem::Explore => return self.open_explore(),
            NavItem::Playlist(index) => {
                if let Account::SignedIn(data) = &self.account
                    && let Some(playlist) = data.playlists.get(index).cloned()
                {
                    self.screen = Screen::Collection;
                    return self.online.open(playlist);
                }
                Screen::Home
            }
            _ => Screen::Home,
        };
        Task::none()
    }

    /// Shows the library after signing in or restoring a session, saves it for the next start
    /// and downloads its images. A failed sign-in shows the error.
    pub(crate) fn library_loaded(&mut self, result: Result<LibraryData, String>) -> Task<Message> {
        match result {
            Ok(data) => {
                library::save_cache(&data);
                let images = self.online.fetch_images(data.image_urls());
                self.account = Account::SignedIn(Box::new(data));
                images
            }
            Err(error) => {
                self.account = Account::Failed(error);
                Task::none()
            }
        }
    }
}
