//! Which side panels fit in the window.

/// Below this window width the lyrics panel is hidden.
pub const LYRICS_MIN_WIDTH: f32 = 1100.0;
/// Below this window width the sidebar is hidden too.
pub const SIDEBAR_MIN_WIDTH: f32 = 720.0;

/// The side panels that end up on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Columns {
    pub sidebar: bool,
    pub lyrics: bool,
}

/// A panel is shown only if the user wants it and the window is wide enough for it.
pub fn visible_columns(width: f32, show_sidebar: bool, show_lyrics: bool) -> Columns {
    Columns {
        sidebar: show_sidebar && width >= SIDEBAR_MIN_WIDTH,
        lyrics: show_lyrics && width >= LYRICS_MIN_WIDTH,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns(sidebar: bool, lyrics: bool) -> Columns {
        Columns { sidebar, lyrics }
    }

    #[test]
    fn a_wide_window_shows_both_panels() {
        assert_eq!(visible_columns(1280.0, true, true), columns(true, true));
    }

    #[test]
    fn a_medium_window_hides_the_lyrics() {
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
}
