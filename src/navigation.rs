//! Going from screen to screen: what Back goes back to.

use crate::{App, Screen, search};

/// How many screens Back remembers.
const HISTORY_LIMIT: usize = 30;

impl App {
    /// Shows `screen`, remembering the one shown now (and its collection) for Back.
    pub(crate) fn go_to(&mut self, screen: Screen) {
        let (shown, status, _) = self.online.collection();
        let collection = (self.screen == Screen::Collection && *status == search::Status::Done)
            .then(|| shown.cloned())
            .flatten();
        if self.history.len() == HISTORY_LIMIT {
            self.history.remove(0);
        }
        self.history.push((self.screen, collection));
        self.screen = screen;
    }
}
