//! Scrolling synced lyrics by hand and clicking a line to jump to it. The wheel moves a target
//! and the lyrics glide after it, frame by frame, the way smooth scrolling does; four seconds
//! after the last turn of the wheel they slide back to the line being sung, and clicking a line
//! sends them there at once.

use std::time::{Duration, Instant};

use crate::{App, lyrics};

/// How long lyrics scrolled by hand stay put before following the song again.
const BROWSE_TIME: Duration = Duration::from_secs(4);
/// How quickly the lyrics catch up with the wheel: after this long they have covered about
/// two thirds of the way (an exponential ease, like smooth scrolling).
const GLIDE: Duration = Duration::from_millis(90);

/// Lyrics scrolled by hand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Browse {
    /// The line the wheel asked for (a line index, fractional with a touchpad).
    target: f32,
    /// The line shown now, gliding toward `target`.
    pub shown: f32,
    /// When the wheel last turned.
    since: Instant,
    /// When `shown` last moved.
    stepped: Instant,
}

impl Browse {
    fn new(at: f32, now: Instant) -> Self {
        Self {
            target: at,
            shown: at,
            since: now,
            stepped: now,
        }
    }

    /// Moves `shown` toward `target` for the time since the last step.
    fn step(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.stepped).as_secs_f32();
        self.stepped = now;
        let kept = (-elapsed / GLIDE.as_secs_f32()).exp();
        self.shown = self.target + (self.shown - self.target) * kept;
        if (self.shown - self.target).abs() < 0.001 {
            self.shown = self.target;
        }
    }
}

impl App {
    /// Moves the lyrics `lines` lines down (up when negative), away from the song.
    pub(super) fn lyrics_scrolled(&mut self, lines: f32) {
        let count = self.lyrics.synced().len();
        if count == 0 {
            return;
        }
        let now = self.now;
        let mut browse = self.lyrics_browse.unwrap_or_else(|| {
            Browse::new(self.lyrics_scroll.interpolate_with(|line| line, now), now)
        });
        browse.target = (browse.target + lines).clamp(0.0, (count - 1) as f32);
        browse.since = now;
        self.lyrics_browse = Some(browse);
    }

    /// Jumps to where line `index` of the synced lyrics starts; the lyrics slide there from
    /// where they were scrolled to.
    pub(super) fn lyric_clicked(&mut self, index: usize) {
        let Some(start) = self.lyrics.synced().get(index).map(|line| line.start_ms) else {
            return;
        };
        self.seek(start as f32 / 1000.0);
        self.end_browsing();
    }

    /// Glides lyrics scrolled by hand toward where the wheel left them, and lets them follow
    /// the song again once the user stopped scrolling.
    pub(crate) fn step_lyrics_browse(&mut self) {
        let Some(browse) = &mut self.lyrics_browse else {
            return;
        };
        browse.step(self.now);
        if self.now.saturating_duration_since(browse.since) >= BROWSE_TIME {
            self.end_browsing();
        }
    }

    /// The line the lyrics show while scrolled by hand.
    pub(crate) fn browsed_line(&self) -> Option<f32> {
        self.lyrics_browse.map(|browse| browse.shown)
    }

    /// Slides from where the user left the lyrics to the line being sung.
    fn end_browsing(&mut self) {
        if let Some(browse) = self.lyrics_browse.take() {
            self.lyrics_scroll = lyrics::scroll_animation(browse.shown);
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

    fn target(app: &App) -> Option<f32> {
        app.lyrics_browse.map(|browse| browse.target)
    }

    #[test]
    fn the_wheel_moves_within_the_lyrics() {
        let mut app = app_with_lines(5);
        app.lyrics_browse = Some(Browse::new(1.0, app.now));
        app.lyrics_scrolled(2.0);
        assert_eq!(target(&app), Some(3.0));
        app.lyrics_scrolled(10.0);
        assert_eq!(target(&app), Some(4.0), "stops at the last line");
        app.lyrics_scrolled(-10.0);
        assert_eq!(target(&app), Some(0.0), "stops at the first line");
    }

    #[test]
    fn the_lyrics_glide_after_the_wheel() {
        let mut app = app_with_lines(10);
        app.lyrics_browse = Some(Browse::new(0.0, app.now));
        app.lyrics_scrolled(4.0);
        let mut shown = Vec::new();
        for _ in 0..20 {
            app.now += Duration::from_millis(16);
            app.step_lyrics_browse();
            shown.push(app.browsed_line().expect("browsing"));
        }
        assert!(
            shown[0] > 0.0 && shown[0] < 2.0,
            "starts moving gently: {shown:?}"
        );
        assert!(
            shown.windows(2).all(|pair| pair[1] >= pair[0]),
            "never goes back"
        );
        assert!(
            shown[19] > 3.8,
            "arrives in about a third of a second: {shown:?}"
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
        // The lyrics slide to the clicked line.
        assert_eq!(app.lyrics_scroll.value(), 3.0);
    }
}
