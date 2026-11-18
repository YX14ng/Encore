//! Short records of what Encore did, kept in the cache so a problem can be traced afterwards:
//! `playback.log` (songs asked for, downloaded, started, finished) and `session.log` (renewing
//! the YouTube session, signing in again). They hold times, song ids and errors, never cookies.
//! Past 256 KB a record starts over, keeping the previous one as `<name>.old`.

use std::io::Write;

const MAX_BYTES: u64 = 256 * 1024;

/// Adds `event` (its first line) to the record `name`, with the time. A record that cannot be
/// written is skipped: nothing must wait on it.
pub fn write(name: &str, event: impl AsRef<str>) {
    if cfg!(test) {
        return;
    }
    let path = crate::platform::cache_dir().join(name);
    if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() > MAX_BYTES) {
        let _ = std::fs::rename(&path, path.with_extension("log.old"));
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = event
        .as_ref()
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(file, "{time} {line}");
    }
}

/// Adds `event` to `session.log`.
pub fn session(event: impl AsRef<str>) {
    write("session.log", event);
}
