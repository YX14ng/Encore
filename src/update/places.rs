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
        if let Screen::Library(_) = self.screen {
            // A library screen opens at its top, with the images it shows first.
            self.shelf_scroll = crate::virtual_list::Scroll::default();
            return Task::batch([
                iced::widget::operation::snap_to(
                    library::SHELF_SCROLL_ID,
                    iced::widget::operation::RelativeOffset::START,
                ),
                self.fetch_shelf_images(),
            ]);
        }
        Task::none()
    }

    /// Downloads the images the library screen that is open shows now.
    pub(crate) fn fetch_shelf_images(&mut self) -> Task<Message> {
        match (&self.screen, &self.account) {
            (Screen::Library(shelf), Account::SignedIn(data)) => {
                let urls = data.shelf_image_urls(*shelf, self.shelf_scroll);
                self.online.fetch_images(urls)
            }
            _ => Task::none(),
        }
    }

    /// Shows the library after signing in or restoring a session, saves it for the next start
    /// and downloads its images. A failed sign-in shows the error.
    pub(crate) fn library_loaded(&mut self, result: Result<LibraryData, String>) -> Task<Message> {
        match result {
            Ok(data) => {
                library::save_cache(&data);
                let images = self.online.fetch_images(data.home_image_urls());
                self.account = Account::SignedIn(Box::new(data));
                Task::batch([images, self.fetch_shelf_images()])
            }
            Err(error) => {
                self.account = Account::Failed(error);
                Task::none()
            }
        }
    }
}
