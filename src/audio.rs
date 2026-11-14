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
