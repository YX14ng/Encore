//! Playback time helpers.
/// Formats a number of seconds as `m:ss`, for example `93` as `"1:33"`.
pub fn format_time(secs: u32) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Seconds left in a track. Never goes below zero.
pub fn remaining(elapsed: u32, duration: u32) -> u32 {
    duration.saturating_sub(elapsed)
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
}
