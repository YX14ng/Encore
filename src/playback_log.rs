//! A short record of what playback did: the songs asked for, downloaded, started and finished,
//! kept in the cache (`playback.log`) so a song that never starts can be traced afterwards. It
//! holds times, song ids and errors only. Past 256 KB it starts over, keeping the previous one
//! as `playback.log.old`.

use std::io::Write;
use std::path::PathBuf;

const MAX_BYTES: u64 = 256 * 1024;

fn path() -> PathBuf {
    crate::platform::cache_dir().join("playback.log")
}

/// Adds `event` to the record, with the time. A record that cannot be written is skipped:
/// playback must never wait on it.
pub fn note(event: impl AsRef<str>) {
    if cfg!(test) {
        return;
    }
    let path = path();
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
