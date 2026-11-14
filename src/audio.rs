//! Plays audio files through libmpv.

use std::path::Path;

use libmpv2::Mpv;

/// One mpv instance that plays one file at a time, with no video.
pub struct Audio {
    mpv: Mpv,
}

impl Audio {
    /// An mpv player that sends sound to the system's default output (mpv picks it, usually
    /// PipeWire). Note: `ao=auto` is not a valid mpv value; it silently disables sound.
    pub fn new() -> Result<Self, String> {
        Self::with_output(None)
    }

    /// An mpv player that decodes but makes no sound, for tests.
    #[cfg(test)]
    pub fn muted() -> Result<Self, String> {
        Self::with_output(Some("null"))
    }

    fn with_output(audio_output: Option<&str>) -> Result<Self, String> {
        let mpv = Mpv::with_initializer(|init| {
            init.set_property("vid", "no")?;
            // The next song is queued in mpv's playlist, and mpv goes into it without reopening
            // the sound output, so there is no silence between songs.
            init.set_property("gapless-audio", "yes")?;
            if let Some(output) = audio_output {
                init.set_property("ao", output)?;
            }
            Ok(())
        })
        .map_err(|error| error.to_string())?;
        Ok(Self { mpv })
    }

    /// Replaces whatever is playing with `path` and starts it.
    pub fn play(&self, path: &Path) -> Result<(), String> {
        let path = path.to_str().ok_or("the audio path is not valid UTF-8")?;
        self.mpv
            .command("loadfile", &[path])
            .map_err(|error| error.to_string())?;
        self.set_paused(false);
        Ok(())
    }

    /// Queues `path` to start right when the current file ends, replacing anything queued.
    pub fn queue_next(&self, path: &Path) -> Result<(), String> {
        let path = path.to_str().ok_or("the audio path is not valid UTF-8")?;
        self.clear_queued();
        self.mpv
            .command("loadfile", &[path, "append"])
            .map_err(|error| error.to_string())
    }

    /// Takes the queued file (and any finished one) out of mpv's playlist, keeping the
    /// current file.
    pub fn clear_queued(&self) {
        // Clearing an empty playlist fails; there is nothing to report.
        let _ = self.mpv.command("playlist-clear", &[]);
    }

    /// The position of the current file in mpv's playlist: 0 for the file that was started, 1
    /// once mpv went on into the queued one.
    pub fn playlist_pos(&self) -> Option<i64> {
        self.mpv.get_property("playlist-pos").ok()
    }

    /// Pauses or resumes.
    pub fn set_paused(&self, paused: bool) {
        // A failed pause only means nothing is loaded; there is nothing to report.
        let _ = self.mpv.set_property("pause", paused);
    }

    /// Seconds into the current file, or `None` while nothing is loaded.
    pub fn position(&self) -> Option<f64> {
        self.mpv.get_property("time-pos").ok()
    }

    /// The length of the current file in seconds, once mpv knows it.
    pub fn duration(&self) -> Option<f64> {
        self.mpv.get_property("duration").ok()
    }

    /// The audio output mpv opened (like "pipewire"), or `None` if it has no sound.
    #[cfg(test)]
    pub fn output(&self) -> Option<String> {
        self.mpv
            .get_property::<String>("current-ao")
            .ok()
            .filter(|name| !name.is_empty())
    }

    /// Sets the volume, from 0 to 100. mpv's scale is cubic, so it follows what the ear hears.
    pub fn set_volume(&self, volume: f64) {
        let _ = self.mpv.set_property("volume", volume);
    }

    /// Jumps to `seconds` into the current file.
    pub fn seek(&self, seconds: f64) {
        // Seeking with nothing loaded fails; there is nothing to report.
        let _ = self
            .mpv
            .command("seek", &[&format!("{seconds:.3}"), "absolute"]);
    }

    /// Stops and unloads the current file.
    pub fn stop(&self) {
        let _ = self.mpv.command("stop", &[]);
    }

    /// Sets mpv's audio filter chain (the `af` option); an empty text removes every filter.
    pub fn set_filters(&self, chain: &str) -> Result<(), String> {
        self.mpv
            .set_property("af", chain)
            .map_err(|error| format!("mpv refused the audio filters {chain:?}: {error}"))
    }

    /// Sets the playback speed (1.0 is normal) and whether the pitch stays the same.
    pub fn set_speed(&self, speed: f64, keep_pitch: bool) {
        let _ = self.mpv.set_property("audio-pitch-correction", keep_pitch);
        let _ = self.mpv.set_property("speed", speed);
    }

    /// Whether mpv has nothing loaded, which after a file was playing means it ended.
    pub fn is_idle(&self) -> bool {
        self.mpv.get_property("idle-active").unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached_song() -> std::path::PathBuf {
        std::fs::read_dir(crate::download::cache_dir().join("audio"))
            .expect("cache dir")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|ext| ext == "webm"))
            .expect("a cached song")
    }

    /// Jumps into a cached song without making a sound:
    /// `cargo test -- --ignored seeks_in_a_song`.
    #[test]
    #[ignore = "needs libmpv and a song in the cache"]
    fn seeks_in_a_song() {
        let audio = Audio::muted().expect("mpv");
        audio.play(&cached_song()).expect("play");
        let opened = (0..50).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            audio.duration().is_some()
        });
        assert!(opened, "the song did not open");
        audio.seek(30.0);
        std::thread::sleep(std::time::Duration::from_millis(500));
        let position = audio.position().unwrap_or(0.0);
        assert!((30.0..32.0).contains(&position), "at {position}");
    }

    /// Plays the last seconds of a cached song with another one queued and checks mpv goes
    /// into it without stopping in between: `cargo test -- --ignored goes_on_without_a_gap`.
    #[test]
    #[ignore = "needs libmpv and two songs in the cache"]
    fn goes_on_without_a_gap() {
        let songs: Vec<_> = std::fs::read_dir(crate::download::cache_dir().join("audio"))
            .expect("cache dir")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "webm"))
            .take(2)
            .collect();
        assert_eq!(songs.len(), 2, "needs two cached songs");
        let audio = Audio::muted().expect("mpv");
        audio.play(&songs[0]).expect("play");
        let duration = (0..50)
            .find_map(|_| {
                std::thread::sleep(std::time::Duration::from_millis(100));
                audio.duration()
            })
            .expect("the song did not open");
        audio.queue_next(&songs[1]).expect("queue");
        audio.seek(duration - 1.5);
        let mut went_idle = false;
        let started = std::time::Instant::now();
        while audio.playlist_pos() != Some(1) {
            went_idle |= audio.is_idle();
            assert!(started.elapsed().as_secs() < 10, "never went on");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!went_idle, "mpv stopped between the songs");
        std::thread::sleep(std::time::Duration::from_millis(800));
        let position = audio.position().unwrap_or(0.0);
        assert!(
            (0.3..2.0).contains(&position),
            "the next song is at {position}"
        );
        audio.clear_queued();
        assert_eq!(
            audio.playlist_pos(),
            Some(0),
            "the finished song is cleared"
        );
        audio.queue_next(&songs[0]).expect("queue");
        audio.play(&songs[0]).expect("play");
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(
            audio.playlist_pos(),
            Some(0),
            "playing replaces the playlist"
        );
        let count: i64 = audio.mpv.get_property("playlist-count").expect("count");
        assert_eq!(count, 1, "nothing stays queued after playing a song");
    }

    /// Opens the real sound output at volume 0 and plays a cached song for a moment:
    /// `cargo test -- --ignored opens_the_sound_output`.
    #[test]
    #[ignore = "needs libmpv, a sound server and a song in the cache"]
    fn opens_the_sound_output() {
        let song = cached_song();
        let audio = Audio::new().expect("mpv");
        audio.set_volume(0.0);
        audio.play(&song).expect("play");
        std::thread::sleep(std::time::Duration::from_millis(2000));
        assert!(audio.output().is_some(), "mpv opened no sound output");
        assert!(
            audio.position().unwrap_or(0.0) > 0.5,
            "audio did not advance"
        );
        assert!(audio.duration().unwrap_or(0.0) > 1.0, "no duration");
    }
}
