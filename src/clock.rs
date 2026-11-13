//! Playback time helpers.
/// Formats a number of seconds as `m:ss`, for example `93` as `"1:33"`.
pub fn format_time(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Seconds left in a track. Never goes below zero.
pub fn remaining(elapsed: u32, duration: u32) -> u32 {
    duration.saturating_sub(elapsed)
}

/// The fake playback clock: where we are in the track and whether it is running.
pub struct Playback {
    pub elapsed: u32,
    pub duration: u32,
    pub is_playing: bool,
}

impl Playback {
    /// Advances one second while playing, and starts over at the end of the track.
    pub fn tick(&mut self) {
        if !self.is_playing {
            return;
        }
        self.elapsed += 1;
        if self.elapsed >= self.duration {
            self.elapsed = 0;
        }
    }

    /// Switches between playing and paused
    pub fn toggle(&mut self) {
        self.is_playing = !self.is_playing;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_time_pads_seconds() {
        assert_eq!(format_time(0), "0:00");
        assert_eq!(format_time(59), "0:59");
        assert_eq!(format_time(93), "1:33");
        assert_eq!(format_time(214), "3:34");
        assert_eq!(format_time(600), "10:00");
    }

    #[test]
    fn remaining_counts_down() {
        assert_eq!(remaining(93, 214), 121);
        assert_eq!(remaining(214, 214), 0);
    }

    #[test]
    fn remaining_never_goes_negative() {
        assert_eq!(remaining(300, 214), 0);
    }

    fn playing_at(elapsed: u32) -> Playback {
        Playback {
            elapsed,
            duration: 214,
            is_playing: true,
        }
    }
    #[test]
    fn tick_advances_while_playing() {
        let mut playback = playing_at(93);
        playback.tick();
        assert_eq!(playback.elapsed, 94);
    }

    #[test]
    fn tick_starts_over_at_the_end() {
        let mut playback = playing_at(213);
        playback.tick();
        assert_eq!(playback.elapsed, 0);
    }

    #[test]
    fn tick_recovers_when_past_the_end() {
        let mut playback = playing_at(300);
        playback.tick();
        assert_eq!(playback.elapsed, 0);
    }

    #[test]
    fn tick_does_nothing_while_paused() {
        let mut playback = playing_at(93);
        playback.is_playing = false;
        playback.tick();
        assert_eq!(playback.elapsed, 93);
    }

    #[test]
    fn toggle_flips_between_playing_and_paused() {
        let mut playback = playing_at(93);
        playback.toggle();
        assert!(!playback.is_playing);
        playback.toggle();
        assert!(playback.is_playing);
    }
}
