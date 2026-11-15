//! How long YouTube Music sessions last. Encore remembers when it first saw the current session
//! work and, when a session stops being accepted, how long it had lasted; the account screen
//! shows both. Kept in `~/.local/share/encore/session.json` (times only, no cookies); the
//! changes are made in memory and saved with `save`.
//!
//! The numbers say whether renewing the session (see
//! `docs/superpowers/specs/2026-11-15-encore-session-renewal-design.md`) is needed at all.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rust_i18n::t;
use serde::{Deserialize, Serialize};

const DAY: u64 = 24 * 60 * 60;

/// When the current session started working, and how long the last one lasted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionLog {
    /// When Encore signed in, or first saw the saved session work (seconds since 1970).
    pub since: Option<u64>,
    /// How long the last session that stopped working had worked, in seconds.
    pub last_lasted: Option<u64>,
}

impl SessionLog {
    /// Reads the log; a missing or broken file is an empty log.
    pub fn load() -> Self {
        Self::read(&path())
    }

    fn read(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    /// Saves the log. A failed write only means the times are not remembered next start.
    pub fn save(&self) {
        self.write(&path());
    }

    fn write(&self, path: &Path) {
        if let Some(dir) = path.parent()
            && std::fs::create_dir_all(dir).is_ok()
            && let Ok(json) = serde_json::to_string_pretty(self)
        {
            // Without the file the next start only forgets the times.
            let _ = std::fs::write(path, json);
        }
    }

    /// A new session started (the user signed in) at `now`.
    pub fn signed_in(&mut self, now: u64) {
        self.since = Some(now);
    }

    /// The saved session worked at `now`; the first time it is seen, that is when it starts.
    pub fn working(&mut self, now: u64) {
        self.since.get_or_insert(now);
    }

    /// The session stopped being accepted at `now`.
    pub fn expired(&mut self, now: u64) {
        if let Some(since) = self.since.take() {
            self.last_lasted = Some(now.saturating_sub(since));
        }
    }

    /// The user signed out: the session did not expire, so its length says nothing.
    pub fn signed_out(&mut self) {
        self.since = None;
    }

    /// "This session has worked for 3 days." for the account screen.
    pub fn age_text(&self, now: u64) -> Option<String> {
        let days = now.saturating_sub(self.since?) / DAY;
        Some(match days {
            0 => t!("session.working_today").into_owned(),
            1 => t!("session.working_one_day").into_owned(),
            days => t!("session.working_days", days = days).into_owned(),
        })
    }

    /// "The previous session lasted 12 days." for the account screen.
    pub fn last_text(&self) -> Option<String> {
        let days = self.last_lasted? / DAY;
        Some(match days {
            0 => t!("session.lasted_under_a_day").into_owned(),
            1 => t!("session.lasted_one_day").into_owned(),
            days => t!("session.lasted_days", days = days).into_owned(),
        })
    }
}

/// Seconds since 1970.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn path() -> PathBuf {
    crate::account::data_dir().join("session.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_temp(log: &SessionLog) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("encore-session-{}", std::process::id()));
        let path = dir.join("session.json");
        log.write(&path);
        path
    }

    #[test]
    fn measures_how_long_a_session_lasted() {
        let mut log = SessionLog::default();
        log.working(1_000);
        log.working(5_000);
        assert_eq!(log.since, Some(1_000), "the first time seen stays");
        log.expired(1_000 + 3 * DAY + 60);
        assert_eq!(log.since, None);
        assert_eq!(log.last_lasted, Some(3 * DAY + 60));
        assert_eq!(
            log.last_text().as_deref(),
            Some("The previous session lasted 3 days.")
        );
        log.signed_in(9_000);
        log.signed_out();
        assert_eq!(log.since, None);
        assert_eq!(
            log.last_lasted,
            Some(3 * DAY + 60),
            "signing out is not an expiry"
        );
    }

    #[test]
    fn says_how_old_the_session_is() {
        let log = SessionLog {
            since: Some(0),
            last_lasted: None,
        };
        assert_eq!(
            log.age_text(DAY / 2).as_deref(),
            Some("This session started today.")
        );
        assert_eq!(
            log.age_text(DAY + 5).as_deref(),
            Some("This session has worked for 1 day.")
        );
        assert_eq!(
            log.age_text(9 * DAY).as_deref(),
            Some("This session has worked for 9 days.")
        );
        assert_eq!(SessionLog::default().age_text(9).as_deref(), None);
    }

    #[test]
    fn the_log_survives_a_restart() {
        let log = SessionLog {
            since: Some(42),
            last_lasted: Some(7),
        };
        let path = in_temp(&log);
        assert_eq!(SessionLog::read(&path), log);
        let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
    }
}
