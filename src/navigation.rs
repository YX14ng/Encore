//! Going from screen to screen: what Back goes back to, and what Forward goes forward to after
//! going back, as in a browser.

use iced::Task;

use crate::library::Shelf;
use crate::sidebar::NavItem;
use crate::{App, Message, Screen, search, youtube};

/// How many screens Back (and Forward) remember.
const HISTORY_LIMIT: usize = 30;

/// A screen, with the collection it showed if it was one.
pub(crate) type Place = (Screen, Option<youtube::Collection>);

impl App {
    /// Shows `screen`, remembering the one shown now (and its collection) for Back. Going
    /// somewhere new forgets what Forward led to.
    pub(crate) fn go_to(&mut self, screen: Screen) {
        // Another tab of the settings or another search is the same place; another collection
        // is a new one.
        let same_place = std::mem::discriminant(&screen) == std::mem::discriminant(&self.screen)
            && screen != Screen::Collection;
        if !same_place {
            let here = self.here();
            remember(&mut self.history, here);
            self.forward.clear();
        }
        self.screen = screen;
        self.follow_in_sidebar();
    }

    /// Back: the screen shown before, or Home; the screen left is kept for Forward.
    pub(crate) fn go_back(&mut self) -> Task<Message> {
        let Some(place) = self.history.pop() else {
            self.screen = Screen::Home;
            return Task::none();
        };
        let here = self.here();
        remember(&mut self.forward, here);
        self.show_place(place)
    }

    /// Forward: the screen Back left, if Back was used since the last new screen.
    pub(crate) fn go_forward(&mut self) -> Task<Message> {
        let Some(place) = self.forward.pop() else {
            return Task::none();
        };
        let here = self.here();
        remember(&mut self.history, here);
        self.show_place(place)
    }

    /// Whether Forward has somewhere to go.
    pub(crate) fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// Forgets both ways, when a sidebar entry starts over from its screen.
    pub(crate) fn forget_places(&mut self) {
        self.history.clear();
        self.forward.clear();
    }

    /// The screen shown now, with its collection once it has loaded.
    fn here(&self) -> Place {
        let (shown, status, _) = self.online.collection();
        let collection = (self.screen == Screen::Collection && *status == search::Status::Done)
            .then(|| shown.cloned())
            .flatten();
        (self.screen, collection)
    }

    fn show_place(&mut self, (screen, collection): Place) -> Task<Message> {
        self.screen = screen;
        self.follow_in_sidebar();
        match collection {
            Some(collection) => self.online.show_collection(collection),
            None => Task::none(),
        }
    }
}

impl App {
    /// Highlights the sidebar entry of the screen shown, when it has one; other screens keep
    /// the entry they were reached from, unhighlighted.
    fn follow_in_sidebar(&mut self) {
        let item = match self.screen {
            Screen::Home => NavItem::Home,
            Screen::Explore => NavItem::Explore,
            Screen::Account => NavItem::Account,
            Screen::Library(shelf) => match shelf {
                Shelf::History => NavItem::History,
                Shelf::Songs => NavItem::Songs,
                Shelf::Liked => NavItem::LikedMusic,
                Shelf::Albums => NavItem::Albums,
                Shelf::Artists => NavItem::Artists,
                Shelf::Subscriptions => NavItem::Subscriptions,
                Shelf::Podcasts => NavItem::Podcasts,
                Shelf::Mixes => NavItem::Radio,
                Shelf::Downloads => NavItem::Downloads,
            },
            _ => return,
        };
        self.sidebar.selected = item;
    }
}

fn remember(places: &mut Vec<Place>, place: Place) {
    if places.len() == HISTORY_LIMIT {
        places.remove(0);
    }
    places.push(place);
}

#[cfg(test)]
mod tests {
    use crate::settings_view::Tab;
    use crate::{App, Screen};

    #[test]
    fn forward_goes_back_to_where_back_left() {
        let mut app = App::new();
        app.go_to(Screen::Search);
        app.go_to(Screen::Settings(Tab::Audio));
        assert!(!app.can_go_forward());

        let _ = app.go_back();
        assert_eq!(app.screen, Screen::Search);
        assert!(app.can_go_forward());
        let _ = app.go_forward();
        assert_eq!(app.screen, Screen::Settings(Tab::Audio));
        assert!(!app.can_go_forward());

        let _ = app.go_back();
        app.go_to(Screen::Together);
        assert!(
            !app.can_go_forward(),
            "a new screen forgets the way forward"
        );
    }

    #[test]
    fn back_remembers_every_screen_and_the_sidebar_follows() {
        use crate::Message;
        use crate::library::Shelf;
        use crate::sidebar::NavItem;

        let mut app = App::new();
        app.go_to(Screen::Library(Shelf::Songs));
        assert_eq!(app.sidebar.selected, NavItem::Songs);
        let _ = app.handle(Message::OpenSettings(Tab::Audio));
        let _ = app.handle(Message::OpenSettings(Tab::Appearance));
        let _ = app.handle(Message::Back);
        assert_eq!(
            app.screen,
            Screen::Library(Shelf::Songs),
            "another settings tab is not another screen to go back through"
        );
        let _ = app.handle(Message::Back);
        assert_eq!(app.screen, Screen::Home);
        assert_eq!(app.sidebar.selected, NavItem::Home);
    }
}
