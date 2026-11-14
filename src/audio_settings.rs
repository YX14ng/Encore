//! The player's settings that last between sessions: volume, shuffle and repeat. Kept in
//! `$XDG_CONFIG_HOME/encore/audio.json` (or `~/.config/encore/audio.json`).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::queue::Repeat;

/// The player's settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    /// From 0 to 100.
    pub volume: f32,
    pub muted: bool,
    pub shuffle: bool,
    pub repeat: Repeat,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            volume: 80.0,
            muted: false,
            shuffle: false,
            repeat: Repeat::Off,
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

    /// Brings values edited by hand back into range.
    fn clamped(mut self) -> Self {
        self.volume = self.volume.clamp(0.0, 100.0);
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
        let settings = serde_json::from_str::<AudioSettings>(r#"{"volume": 250}"#)
            .expect("json")
            .clamped();
        assert_eq!(settings.volume, 100.0);
        assert_eq!(settings.repeat, Repeat::Off);
    }
}
