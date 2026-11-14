//! The audio engine on top of mpv: the volume, seeking and the deck that is playing.

use std::path::Path;

use crate::audio::Audio;

/// Plays songs through mpv at the chosen volume.
pub struct Mixer {
    deck: Audio,
    volume: f32,
}

impl Mixer {
    /// Starts mpv. Fails if libmpv is missing.
    pub fn new(volume: f32) -> Result<Self, String> {
        let deck = Audio::new()?;
        deck.set_volume(f64::from(volume));
        Ok(Self { deck, volume })
    }

    /// Replaces whatever is playing with `path`.
    pub fn play(&mut self, path: &Path) -> Result<(), String> {
        self.deck.set_volume(f64::from(self.volume));
        self.deck.play(path)
    }

    /// Pauses or resumes.
    pub fn set_paused(&self, paused: bool) {
        self.deck.set_paused(paused);
    }

    /// Sets the volume, from 0 to 100.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
        self.deck.set_volume(f64::from(volume));
    }

    /// Jumps to `seconds` into the song.
    pub fn seek(&self, seconds: f64) {
        self.deck.seek(seconds);
    }

    /// Seconds into the song, once it is open.
    pub fn position(&self) -> Option<f64> {
        self.deck.position()
    }

    /// The length of the song in seconds, once mpv knows it.
    pub fn duration(&self) -> Option<f64> {
        self.deck.duration()
    }

    /// Whether nothing is loaded, which after a song was playing means it ended.
    pub fn is_idle(&self) -> bool {
        self.deck.is_idle()
    }
}
