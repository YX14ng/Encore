//! Plays audio files through libmpv.

use std::path::Path;

use libmpv2::Mpv;

/// One mpv instance that plays one file at a time, with no video.
pub struct Audio {
    mpv: Mpv,
}

impl Audio {
    /// An mpv player that sends sound to the system's default output.
    pub fn new() -> Result<Self, String> {
        Self::with_output("auto")
    }

    /// An mpv player that decodes but makes no sound, for tests.
    #[cfg(test)]
    pub fn muted() -> Result<Self, String> {
        Self::with_output("null")
    }

    fn with_output(audio_output: &str) -> Result<Self, String> {
        let mpv = Mpv::with_initializer(|init| {
            init.set_property("vid", "no")?;
            init.set_property("ao", audio_output)?;
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

    /// Whether mpv has nothing loaded, which after a file was playing means it ended.
    pub fn is_idle(&self) -> bool {
        self.mpv.get_property("idle-active").unwrap_or(false)
    }
}
