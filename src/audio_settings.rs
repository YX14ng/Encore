//! The player's settings that last between sessions: volume, shuffle and repeat, and the audio
//! screen's equalizer, volume normalization, speed and crossfade. Kept in
//! `$XDG_CONFIG_HOME/encore/audio.json` (or `~/.config/encore/audio.json`).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::queue::Repeat;

/// The equalizer's band centers in hertz.
pub const BANDS: [u32; 10] = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
/// The most a band can boost or cut, in decibels.
pub const MAX_GAIN: f32 = 12.0;
pub const MIN_SPEED: f32 = 0.5;
pub const MAX_SPEED: f32 = 2.0;
/// The longest crossfade, in seconds.
pub const MAX_CROSSFADE: u32 = 12;

/// The loudness every song is brought to with normalization on: -14 LUFS, like YouTube and
/// Spotify. The output is resampled back from the 192 kHz loudnorm works at.
const NORMALIZE_FILTER: &str = "loudnorm=I=-14:LRA=11:TP=-1.5,aresample=48000";

/// A named equalizer curve.
pub struct Preset {
    pub name: &'static str,
    pub gains: [f32; 10],
}

/// The curves offered above the equalizer.
pub const PRESETS: [Preset; 7] = [
    Preset {
        name: "Flat",
        gains: [0.0; 10],
    },
    Preset {
        name: "Bass boost",
        gains: [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    },
    Preset {
        name: "Treble boost",
        gains: [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 5.0, 6.0],
    },
    Preset {
        name: "Vocal",
        gains: [-2.0, -2.0, -1.0, 1.0, 3.0, 4.0, 3.0, 1.0, 0.0, -1.0],
    },
    Preset {
        name: "Rock",
        gains: [5.0, 4.0, 3.0, 1.0, -1.0, -1.0, 1.0, 3.0, 4.0, 5.0],
    },
    Preset {
        name: "Electronic",
        gains: [5.0, 4.0, 1.0, 0.0, -2.0, 1.0, 0.0, 1.0, 4.0, 5.0],
    },
    Preset {
        name: "Acoustic",
        gains: [4.0, 3.0, 2.0, 1.0, 1.0, 1.0, 2.0, 3.0, 3.0, 2.0],
    },
];

/// A change made on the audio screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioChange {
    Equalizer(bool),
    /// Band index and decibels.
    Gain(usize, f32),
    Preset(usize),
    Normalize(bool),
    Speed(f32),
    KeepPitch(bool),
    /// Seconds.
    Crossfade(f32),
}

/// The player's settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    /// From 0 to 100.
    pub volume: f32,
    pub muted: bool,
    pub shuffle: bool,
    pub repeat: Repeat,
    pub equalizer: bool,
    /// Decibels per band of `BANDS`.
    pub gains: [f32; 10],
    /// Plays every song at about the same loudness.
    pub normalize: bool,
    /// 1.0 is normal speed.
    pub speed: f32,
    /// Keeps voices at their pitch when the speed changes.
    pub keep_pitch: bool,
    /// Seconds the end of a song overlaps the start of the next; 0 is off.
    pub crossfade: u32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            volume: 80.0,
            muted: false,
            shuffle: false,
            repeat: Repeat::Off,
            equalizer: false,
            gains: [0.0; 10],
            normalize: false,
            speed: 1.0,
            keep_pitch: true,
            crossfade: 0,
        }
    }
}

impl AudioSettings {
    /// Reads the saved settings; missing or broken files give the defaults.
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|json| serde_json::from_str::<Self>(&json).ok())
            .unwrap_or_default()
            .clamped()
    }

    /// Saves the settings. A failed write only means they are not remembered next time.
    pub fn save(&self) {
        let path = path();
        if let Some(dir) = path.parent()
            && std::fs::create_dir_all(dir).is_ok()
            && let Ok(json) = serde_json::to_string_pretty(self)
        {
            let _ = std::fs::write(path, json);
        }
    }

    /// The volume mpv should play at: 0 while muted.
    pub fn effective_volume(&self) -> f32 {
        if self.muted { 0.0 } else { self.volume }
    }

    /// Applies a change from the audio screen. Returns `true` if mpv's filters should be
    /// rebuilt (and the settings saved) now; slider drags wait until the slider is released.
    pub fn change(&mut self, change: AudioChange) -> bool {
        match change {
            AudioChange::Equalizer(on) => self.equalizer = on,
            AudioChange::Gain(band, gain) => {
                if let Some(slot) = self.gains.get_mut(band) {
                    *slot = gain.clamp(-MAX_GAIN, MAX_GAIN);
                }
                return false;
            }
            AudioChange::Preset(index) => {
                if let Some(preset) = PRESETS.get(index) {
                    self.gains = preset.gains;
                    self.equalizer = true;
                }
            }
            AudioChange::Normalize(on) => self.normalize = on,
            AudioChange::Speed(speed) => {
                self.speed = speed.clamp(MIN_SPEED, MAX_SPEED);
                return false;
            }
            AudioChange::KeepPitch(on) => self.keep_pitch = on,
            AudioChange::Crossfade(seconds) => {
                self.crossfade = (seconds.round().max(0.0) as u32).min(MAX_CROSSFADE);
                return false;
            }
        }
        true
    }

    /// The preset the equalizer matches, if any.
    pub fn preset(&self) -> Option<usize> {
        PRESETS.iter().position(|preset| preset.gains == self.gains)
    }

    /// mpv's `af` value for these settings: a lavfi graph with the equalizer bands (after a
    /// preamp that keeps boosts from clipping) and the normalizer, or empty for none.
    pub fn filter_chain(&self) -> String {
        let mut filters = Vec::new();
        if self.equalizer && self.gains.iter().any(|gain| *gain != 0.0) {
            let boost = self.gains.iter().copied().fold(0.0_f32, f32::max);
            if boost > 0.0 {
                filters.push(format!("volume=-{boost}dB"));
            }
            for (frequency, gain) in BANDS.iter().zip(self.gains) {
                if gain != 0.0 {
                    filters.push(format!("equalizer=f={frequency}:t=o:w=1:g={gain}"));
                }
            }
        }
        if self.normalize {
            filters.push(NORMALIZE_FILTER.to_string());
        }
        if filters.is_empty() {
            String::new()
        } else {
            format!("lavfi=[{}]", filters.join(","))
        }
    }

    /// Brings values edited by hand back into range.
    fn clamped(mut self) -> Self {
        self.volume = self.volume.clamp(0.0, 100.0);
        for gain in &mut self.gains {
            *gain = gain.clamp(-MAX_GAIN, MAX_GAIN);
        }
        self.speed = self.speed.clamp(MIN_SPEED, MAX_SPEED);
        self.crossfade = self.crossfade.min(MAX_CROSSFADE);
        self
    }
}

fn path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(std::env::temp_dir)
        .join("encore")
        .join("audio.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muting_silences_without_forgetting_the_volume() {
        let settings = AudioSettings {
            volume: 40.0,
            muted: true,
            ..AudioSettings::default()
        };
        assert_eq!(settings.effective_volume(), 0.0);
        assert_eq!(settings.volume, 40.0);
    }

    #[test]
    fn missing_fields_take_defaults_and_values_are_clamped() {
        let settings = serde_json::from_str::<AudioSettings>(r#"{"volume": 250, "speed": 9}"#)
            .expect("json")
            .clamped();
        assert_eq!(settings.volume, 100.0);
        assert_eq!(settings.speed, MAX_SPEED);
        assert_eq!(settings.repeat, Repeat::Off);
        assert!(settings.keep_pitch);
    }

    #[test]
    fn no_filters_by_default() {
        assert_eq!(AudioSettings::default().filter_chain(), "");
    }

    #[test]
    fn the_equalizer_boosts_after_a_preamp() {
        let mut settings = AudioSettings::default();
        assert!(settings.change(AudioChange::Preset(1)));
        assert_eq!(settings.preset(), Some(1));
        let chain = settings.filter_chain();
        assert!(chain.starts_with("lavfi=[volume=-6dB,equalizer=f=31:t=o:w=1:g=6,"));
        assert!(!chain.contains("f=500"), "flat bands are skipped: {chain}");
        assert!(chain.ends_with(']'));
    }

    #[test]
    fn a_switched_off_equalizer_only_keeps_normalization() {
        let settings = AudioSettings {
            equalizer: false,
            gains: [3.0; 10],
            normalize: true,
            ..AudioSettings::default()
        };
        assert_eq!(
            settings.filter_chain(),
            format!("lavfi=[{NORMALIZE_FILTER}]")
        );
    }

    #[test]
    fn slider_changes_wait_for_the_release() {
        let mut settings = AudioSettings::default();
        assert!(!settings.change(AudioChange::Gain(0, 40.0)));
        assert_eq!(settings.gains[0], MAX_GAIN);
        assert!(!settings.change(AudioChange::Crossfade(5.6)));
        assert_eq!(settings.crossfade, 6);
        assert!(settings.change(AudioChange::Normalize(true)));
    }
}
