//! A short record of what playback did: the songs asked for, downloaded, started and finished,
//! kept in `playback.log` in the cache (see `notes`) so a song that never starts can be traced
//! afterwards.

/// Adds `event` to the record, with the time. Playback never waits on it.
pub fn note(event: impl AsRef<str>) {
    crate::notes::write("playback.log", event);
}
