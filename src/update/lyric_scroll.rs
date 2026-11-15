//! Scrolling synced lyrics by hand and clicking a line to jump to it. While the user scrolls,
//! the lyrics stay where they put them; a few seconds after the last turn of the wheel they
//! slide back to the line being sung.

use std::time::Duration;

use crate::{App, lyrics};

/// How long lyrics scrolled by hand stay put before following the song again.
const BROWSE_TIME: Duration = Duration::from_secs(4);

impl App {
    /// Moves the lyrics `lines` lines down (up when negative), away from the song.
    pub(super) fn lyrics_scrolled(&mut self, lines: f32) {
        let count = self.lyrics.synced().len();
        if count == 0 {
            return;
        }
        let from = match self.lyrics_browse {
            Some((at, _)) => at,
            None => self.lyrics_scroll.interpolate_with(|line| line, self.now),
        };
        let at = (from + lines).clamp(0.0, (count - 1) as f32);
        self.lyrics_browse = Some((at, self.now));
    }

    /// Jumps to where line `index` of the synced lyrics starts, and follows the song again.
    pub(super) fn lyric_clicked(&mut self, index: usize) {
        let Some(start) = self.lyrics.synced().get(index).map(|line| line.start_ms) else {
            return;
        };
        self.end_browsing();
        self.seek(start as f32 / 1000.0);
    }

    /// Lets lyrics scrolled by hand follow the song again once the user stopped scrolling.
    pub(crate) fn step_lyrics_browse(&mut self) {
        if let Some((_, since)) = self.lyrics_browse
            && self.now.saturating_duration_since(since) >= BROWSE_TIME
        {
            self.end_browsing();
        }
    }

    /// Slides from where the user left the lyrics back to the line being sung.
    fn end_browsing(&mut self) {
        if let Some((at, _)) = self.lyrics_browse.take() {
            self.lyrics_scroll = lyrics::scroll_animation(at);
            self.follow_lyrics();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::{LyricLine, Lyrics};

    fn app_with_lines(count: u32) -> App {
        let mut app = App::new();
        app.lyrics = Lyrics::Synced(
            (0..count)
                .map(|index| LyricLine {
                    start_ms: index * 5_000,
                    text: format!("line {index}"),
                })
                .collect(),
        );
        app
    }

    #[test]
    fn the_wheel_moves_within_the_lyrics() {
        let mut app = app_with_lines(5);
        app.lyrics_browse = Some((1.0, app.now));
        app.lyrics_scrolled(2.0);
        assert_eq!(app.lyrics_browse.map(|(at, _)| at), Some(3.0));
        app.lyrics_scrolled(10.0);
        assert_eq!(
            app.lyrics_browse.map(|(at, _)| at),
            Some(4.0),
            "stops at the last line"
        );
        app.lyrics_scrolled(-10.0);
        assert_eq!(
            app.lyrics_browse.map(|(at, _)| at),
            Some(0.0),
            "stops at the first line"
        );
    }

    #[test]
    fn scrolled_lyrics_follow_the_song_again_after_a_while() {
        let mut app = app_with_lines(5);
        app.lyrics_scrolled(2.0);
        app.step_lyrics_browse();
        assert!(
            app.lyrics_browse.is_some(),
            "still browsing right after scrolling"
        );
        app.now += BROWSE_TIME;
        app.step_lyrics_browse();
        assert!(app.lyrics_browse.is_none());
    }

    #[test]
    fn clicking_a_line_jumps_to_where_it_starts() {
        let mut app = app_with_lines(5);
        app.lyrics_scrolled(3.0);
        app.lyric_clicked(3);
        assert_eq!(app.playback.elapsed, 15);
        assert!(app.lyrics_browse.is_none());
    }
}
