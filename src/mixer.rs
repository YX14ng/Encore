//! The audio engine on top of mpv: two decks so one song can fade into the next, the volume,
//! seeking, and the audio screen's filters and speed applied to both decks.

use std::path::Path;
use std::time::{Duration, Instant};

use crate::audio::Audio;
use crate::audio_settings::AudioSettings;

/// A crossfade in progress: the other deck fades out while the active one fades in.
#[derive(Debug, Clone, Copy)]
struct Fade {
    started: Instant,
    length: Duration,
}

/// Plays songs through mpv at the chosen volume.
pub struct Mixer {
    decks: [Audio; 2],
    /// The deck playing the current song.
    active: usize,
    volume: f32,
    fade: Option<Fade>,
}

/// The volumes (0 to 1, on mpv's scale) of the song fading out and the one fading in, `progress`
/// of the way through a crossfade. mpv's volume is cubic, so the cube root of an equal-power
/// curve keeps the loudness steady in the middle.
pub fn fade_volumes(progress: f32) -> (f32, f32) {
    let angle = progress.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (angle.cos().max(0.0).cbrt(), angle.sin().max(0.0).cbrt())
}

impl Mixer {
    /// Starts mpv twice and applies `settings`. Fails if libmpv is missing.
    pub fn new(settings: &AudioSettings) -> Result<Self, String> {
        let mut mixer = Self {
            decks: [Audio::new()?, Audio::new()?],
            active: 0,
            volume: settings.effective_volume(),
            fade: None,
        };
        mixer.set_volume(mixer.volume);
        mixer.apply(settings)?;
        Ok(mixer)
    }

    fn deck(&self) -> &Audio {
        &self.decks[self.active]
    }

    fn other(&self) -> &Audio {
        &self.decks[1 - self.active]
    }

    /// Replaces whatever is playing with `path`, cutting any crossfade short.
    pub fn play(&mut self, path: &Path) -> Result<(), String> {
        self.fade = None;
        self.other().stop();
        self.deck().set_volume(f64::from(self.volume));
        self.deck().play(path)
    }

    /// Starts `path` on the other deck and fades over to it in `length`. The outgoing deck
    /// forgets the song it had queued, which the incoming deck now plays.
    pub fn crossfade_to(&mut self, path: &Path, length: Duration) -> Result<(), String> {
        self.finish_fade();
        self.deck().clear_queued();
        let incoming = &self.decks[1 - self.active];
        incoming.set_volume(0.0);
        incoming.play(path)?;
        self.active = 1 - self.active;
        self.fade = Some(Fade {
            started: Instant::now(),
            length,
        });
        Ok(())
    }

    /// Moves a crossfade along; call it often (every animation frame) while one runs.
    pub fn step(&mut self) {
        let Some(fade) = self.fade else {
            return;
        };
        let progress = fade.started.elapsed().as_secs_f32() / fade.length.as_secs_f32().max(0.1);
        if progress >= 1.0 {
            self.finish_fade();
            return;
        }
        let (outgoing, incoming) = fade_volumes(progress);
        self.other().set_volume(f64::from(self.volume * outgoing));
        self.deck().set_volume(f64::from(self.volume * incoming));
    }

    /// Ends a crossfade at once: the old song stops and the new one plays at full volume.
    fn finish_fade(&mut self) {
        if self.fade.take().is_some() {
            self.other().stop();
            self.deck().set_volume(f64::from(self.volume));
        }
    }

    /// Queues `path` to play with no gap after the current song.
    pub fn queue_next(&self, path: &Path) -> Result<(), String> {
        self.deck().queue_next(path)
    }

    /// Forgets the queued song.
    pub fn clear_queued(&self) {
        self.deck().clear_queued();
    }

    /// Whether mpv went on into the queued song.
    pub fn went_on(&self) -> bool {
        self.deck().playlist_pos() == Some(1)
    }

    /// Whether a crossfade is running.
    pub fn is_fading(&self) -> bool {
        self.fade.is_some()
    }

    /// Pauses or resumes. Pausing in the middle of a crossfade finishes it.
    pub fn set_paused(&mut self, paused: bool) {
        if paused {
            self.finish_fade();
        }
        self.deck().set_paused(paused);
    }

    /// Sets the volume, from 0 to 100.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume;
        if self.fade.is_none() {
            self.deck().set_volume(f64::from(volume));
        }
    }

    /// Jumps to `seconds` into the song, finishing any crossfade.
    pub fn seek(&mut self, seconds: f64) {
        self.finish_fade();
        self.deck().seek(seconds);
    }

    /// Applies the equalizer, normalization and speed to both decks.
    pub fn apply(&mut self, settings: &AudioSettings) -> Result<(), String> {
        let chain = settings.filter_chain();
        for deck in &self.decks {
            deck.set_filters(&chain)?;
            deck.set_speed(f64::from(settings.speed), settings.keep_pitch);
        }
        Ok(())
    }

    /// Seconds into the song, once it is open.
    pub fn position(&self) -> Option<f64> {
        self.deck().position()
    }

    /// The length of the song in seconds, once mpv knows it.
    pub fn duration(&self) -> Option<f64> {
        self.deck().duration()
    }

    /// Whether nothing is loaded, which after a song was playing means it ended.
    pub fn is_idle(&self) -> bool {
        self.deck().is_idle()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_settings::AudioChange;

    fn muted_mixer() -> Mixer {
        Mixer {
            decks: [Audio::muted().expect("mpv"), Audio::muted().expect("mpv")],
            active: 0,
            volume: 80.0,
            fade: None,
        }
    }

    fn cached_songs() -> Vec<std::path::PathBuf> {
        std::fs::read_dir(crate::download::cache_dir().join("audio"))
            .expect("cache dir")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "webm"))
            .take(2)
            .collect()
    }

    fn wait(mixer: &mut Mixer, millis: u64) {
        for _ in 0..millis / 33 {
            mixer.step();
            std::thread::sleep(Duration::from_millis(33));
        }
    }

    /// Plays a cached song through the equalizer, normalization and double speed, then fades
    /// into a second one, all without sound: `cargo test -- --ignored mixes_real_songs`.
    #[test]
    #[ignore = "needs libmpv and two songs in the cache"]
    fn mixes_real_songs() {
        let songs = cached_songs();
        assert_eq!(songs.len(), 2, "needs two cached songs");
        let mut settings = AudioSettings::default();
        settings.change(AudioChange::Preset(1));
        settings.change(AudioChange::Normalize(true));
        settings.change(AudioChange::Speed(2.0));
        let mut mixer = muted_mixer();
        mixer.apply(&settings).expect("mpv takes the filters");

        mixer.play(&songs[0]).expect("play");
        wait(&mut mixer, 1500);
        let first = mixer.position().expect("first song plays");
        assert!(first > 1.5, "double speed: at {first} after 1.5 s");

        mixer
            .crossfade_to(&songs[1], Duration::from_secs(1))
            .expect("crossfade");
        assert!(mixer.is_fading());
        wait(&mut mixer, 1500);
        assert!(!mixer.is_fading(), "the fade did not finish");
        assert!(mixer.other().is_idle(), "the old song still plays");
        assert!(
            mixer.position().unwrap_or(0.0) > 1.0,
            "the new song does not play"
        );
    }

    #[test]
    fn a_crossfade_keeps_the_loudness_steady() {
        assert_eq!(fade_volumes(0.0), (1.0, 0.0));
        let (out, inc) = fade_volumes(1.0);
        assert!(out.abs() < 1e-2 && (inc - 1.0).abs() < 1e-6);
        // Back on a linear scale (cubed), the two powers always add up to one.
        for step in 0..=10 {
            let (out, inc) = fade_volumes(step as f32 / 10.0);
            let power = out.powi(6) + inc.powi(6);
            assert!((power - 1.0).abs() < 1e-3, "power {power} at step {step}");
        }
    }
}
