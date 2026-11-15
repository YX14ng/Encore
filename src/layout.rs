//! Which side panels fit in the window.

/// What the panel at the right shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Lyrics,
    Queue,
}

/// Below this window width the right panel (lyrics or queue) is hidden.
pub const LYRICS_MIN_WIDTH: f32 = 1100.0;
/// Below this window width the sidebar is hidden too.
pub const SIDEBAR_MIN_WIDTH: f32 = 720.0;

/// The side panels that end up on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Columns {
    pub sidebar: bool,
    /// The right panel, with the lyrics or the queue.
    pub panel: bool,
}

/// The width left for the center column once the side panels on screen take theirs.
pub fn center_width(window: f32, columns: Columns) -> f32 {
    let sidebar = if columns.sidebar {
        crate::sidebar::WIDTH
    } else {
        0.0
    };
    let panel = if columns.panel {
        crate::lyrics::WIDTH
    } else {
        0.0
    };
    (window - sidebar - panel).max(0.0)
}

/// A panel is shown only if the user wants it and the window is wide enough for it.
pub fn visible_columns(width: f32, show_sidebar: bool, show_panel: bool) -> Columns {
    Columns {
        sidebar: show_sidebar && width >= SIDEBAR_MIN_WIDTH,
        panel: show_panel && width >= LYRICS_MIN_WIDTH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns(sidebar: bool, panel: bool) -> Columns {
        Columns { sidebar, panel }
    }

    #[test]
    fn a_wide_window_shows_both_panels() {
        assert_eq!(visible_columns(1280.0, true, true), columns(true, true));
    }

    #[test]
    fn a_medium_window_hides_the_right_panel() {
        assert_eq!(visible_columns(1099.0, true, true), columns(true, false));
    }

    #[test]
    fn a_narrow_window_hides_both_panels() {
        assert_eq!(visible_columns(719.0, true, true), columns(false, false));
    }

    #[test]
    fn the_user_toggles_still_apply() {
        assert_eq!(visible_columns(1280.0, false, true), columns(false, true));
        assert_eq!(visible_columns(1099.0, true, false), columns(true, false));
    }

    #[test]
    fn the_center_gets_what_the_panels_leave() {
        let both = visible_columns(1300.0, true, true);
        assert_eq!(center_width(1300.0, both), 1300.0 - 236.0 - 300.0);
        let none = visible_columns(1300.0, false, false);
        assert_eq!(center_width(1300.0, none), 1300.0);
    }
}
