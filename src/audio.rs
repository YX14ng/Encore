//! Plays audio files through libmpv.

use std::path::Path;

use libmpv2::Mpv;
use serde::Deserialize;

use crate::silence::Trim;

/// A sound output mpv can play through.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Device {
    /// mpv's name for it, like "pipewire/alsa_output.usb-…".
    pub name: String,
    /// What the system calls it, like "USB Audio Speakers".
    pub description: String,
}

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

    /// Replaces whatever is playing with `path` and starts it, from and to where `trim` says.
    pub fn play(&self, path: &Path, trim: &Trim) -> Result<(), String> {
        self.load(path, "replace", trim)?;
        self.set_paused(false);
        Ok(())
    }

    /// Queues `path` to start right when the current file ends, replacing anything queued.
    pub fn queue_next(&self, path: &Path, trim: &Trim) -> Result<(), String> {
        self.clear_queued();
        self.load(path, "append", trim)
    }

    /// mpv's `loadfile`, with `trim` as the file's own start and end.
    fn load(&self, path: &Path, flag: &str, trim: &Trim) -> Result<(), String> {
        let path = path.to_str().ok_or("the audio path is not valid UTF-8")?;
        let options = trim.mpv_options();
        let mut args = vec![path, flag];
        if let Some(options) = &options {
            // The third argument is where to insert; -1 is the usual place.
            args.extend(["-1", options]);
        }
        self.mpv
            .command("loadfile", &args)
            .map_err(|error| error.to_string())
    }

    /// Where the current file stops, in seconds: its trimmed end, or `None` to play it out.
    pub fn end(&self) -> Option<f64> {
        // mpv answers "none" when the file has no end of its own.
        let end: String = self.mpv.get_property("end").ok()?;
        end.parse().ok()
    }

    /// The sound outputs mpv can use. With PipeWire (or else PulseAudio) only its devices are
    /// listed, since the same devices show up once for each sound system.
    pub fn devices(&self) -> Vec<Device> {
        let list: String = self
            .mpv
            .get_property("audio-device-list")
            .unwrap_or_default();
        let devices: Vec<Device> = serde_json::from_str(&list).unwrap_or_default();
        pick_devices(devices)
    }

    /// Plays through `device` (mpv's name), or the system's default output with `None`.
    pub fn set_device(&self, device: Option<&str>) -> Result<(), String> {
        let device = device.unwrap_or("auto");
        // Setting it reopens the output, a gap in the sound; only a different one is worth it.
        if self
            .mpv
            .get_property::<String>("audio-device")
            .ok()
            .as_deref()
            == Some(device)
        {
            return Ok(());
        }
        self.mpv
            .set_property("audio-device", device)
            .map_err(|error| format!("mpv refused the sound output {device:?}: {error}"))
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

    /// The format of the current file, like "OPUS 152", once mpv opened it.
    pub fn format_label(&self) -> Option<String> {
        let codec: String = self.mpv.get_property("audio-codec-name").ok()?;
        let bytes: Option<i64> = self.mpv.get_property("file-size").ok();
        format_label(&codec, bytes, self.duration())
    }

    /// Whether mpv has nothing loaded, which after a file was playing means it ended.
    pub fn is_idle(&self) -> bool {
        self.mpv.get_property("idle-active").unwrap_or(false)
    }
}

/// The devices of the first sound system that has any (PipeWire, then PulseAudio), else every
/// device; never mpv's "auto", which is the system default offered on its own.
fn pick_devices(devices: Vec<Device>) -> Vec<Device> {
    let real = |device: &Device| device.name.contains('/');
    for system in ["pipewire/", "pulse/"] {
        let found: Vec<Device> = devices
            .iter()
            .filter(|device| device.name.starts_with(system))
            .cloned()
            .collect();
        if !found.is_empty() {
            return found;
        }
    }
    devices.into_iter().filter(real).collect()
}

/// Codecs whose bitrate says nothing about quality, so the label is only their name.
const LOSSLESS: [&str; 4] = ["flac", "alac", "wavpack", "ape"];

/// A codec and its average bitrate in kbit/s ("OPUS 152"), from the file's size and length.
/// Without them, or for lossless codecs, just the codec.
pub fn format_label(codec: &str, bytes: Option<i64>, seconds: Option<f64>) -> Option<String> {
    let codec = codec.trim();
    if codec.is_empty() {
        return None;
    }
    let name = if codec.starts_with("pcm") {
        "PCM".to_string()
    } else {
        codec.to_uppercase()
    };
    let lossless = LOSSLESS.contains(&codec) || codec.starts_with("pcm");
    let kbps = match (bytes, seconds) {
        (Some(bytes), Some(seconds)) if bytes > 0 && seconds >= 1.0 && !lossless => {
            Some((bytes as f64 * 8.0 / seconds / 1000.0).round() as u32)
        }
        _ => None,
    };
    Some(match kbps {
        Some(kbps) => format!("{name} {kbps}"),
        None => name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(name: &str) -> Device {
        Device {
            name: name.to_string(),
            description: name.to_string(),
        }
    }

    #[test]
    fn devices_come_from_one_sound_system() {
        let all = [
            "auto",
            "pipewire",
            "pipewire/hdmi",
            "pipewire/usb",
            "pulse/hdmi",
            "alsa",
        ];
        let names = |devices: Vec<Device>| {
            devices
                .into_iter()
                .map(|device| device.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(pick_devices(all.map(device).to_vec())),
            ["pipewire/hdmi", "pipewire/usb"]
        );
        assert_eq!(
            names(pick_devices(
                ["auto", "alsa/default", "alsa/hw"].map(device).to_vec()
            )),
            ["alsa/default", "alsa/hw"]
        );
    }

    #[test]
    fn labels_say_the_codec_and_the_average_bitrate() {
        // 3:20 of Opus in 4 MB is 160 kbit/s.
        assert_eq!(
            format_label("opus", Some(4_000_000), Some(200.0)),
            Some("OPUS 160".to_string())
        );
        assert_eq!(
            format_label("aac", None, Some(200.0)),
            Some("AAC".to_string())
        );
        assert_eq!(
            format_label("flac", Some(30_000_000), Some(200.0)),
            Some("FLAC".to_string())
        );
        assert_eq!(
            format_label("pcm_s16le", Some(1), Some(1.0)),
            Some("PCM".to_string())
        );
        assert_eq!(format_label("", Some(1), Some(1.0)), None);
    }

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
        audio.play(&cached_song(), &Trim::default()).expect("play");
        let opened = (0..50).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            audio.duration().is_some()
        });
        assert!(opened, "the song did not open");
        let label = audio.format_label().expect("a format");
        assert!(label.starts_with("OPUS "), "{label}");
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
        audio.play(&songs[0], &Trim::default()).expect("play");
        let duration = (0..50)
            .find_map(|_| {
                std::thread::sleep(std::time::Duration::from_millis(100));
                audio.duration()
            })
            .expect("the song did not open");
        audio
            .queue_next(&songs[1], &Trim::default())
            .expect("queue");
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
        audio
            .queue_next(&songs[0], &Trim::default())
            .expect("queue");
        audio.play(&songs[0], &Trim::default()).expect("play");
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(
            audio.playlist_pos(),
            Some(0),
            "playing replaces the playlist"
        );
        let count: i64 = audio.mpv.get_property("playlist-count").expect("count");
        assert_eq!(count, 1, "nothing stays queued after playing a song");
    }

    /// Plays two seconds of a cached song with its own start and end, then a queued one that
    /// starts late too: `cargo test -- --ignored plays_only_the_trimmed_part`.
    #[test]
    #[ignore = "needs libmpv and a song in the cache"]
    fn plays_only_the_trimmed_part() {
        let song = cached_song();
        let audio = Audio::muted().expect("mpv");
        let trim = Trim {
            start: Some(10.0),
            end: Some(12.0),
        };
        audio.play(&song, &trim).expect("play");
        let position = (0..50)
            .find_map(|_| {
                std::thread::sleep(std::time::Duration::from_millis(100));
                audio.position()
            })
            .expect("the song did not open");
        assert!((10.0..11.0).contains(&position), "starts at {position}");
        assert_eq!(audio.end(), Some(12.0), "the file's own end");
        let later = Trim {
            start: Some(30.0),
            end: None,
        };
        audio.queue_next(&song, &later).expect("queue");
        let started = std::time::Instant::now();
        while audio.playlist_pos() != Some(1) {
            assert!(started.elapsed().as_secs() < 6, "did not stop at the end");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
        let position = audio.position().unwrap_or(0.0);
        assert!(
            (30.0..31.5).contains(&position),
            "the queued one is at {position}"
        );
        assert_eq!(audio.end(), None, "the queued one plays to its end");
    }

    /// Lists the sound outputs without playing anything:
    /// `cargo test -- --ignored lists_the_sound_outputs`.
    #[test]
    #[ignore = "needs libmpv and a sound server"]
    fn lists_the_sound_outputs() {
        let audio = Audio::new().expect("mpv");
        let devices = audio.devices();
        assert!(!devices.is_empty(), "no sound outputs");
        assert!(
            devices.iter().all(|device| !device.description.is_empty()),
            "{devices:?}"
        );
        audio.set_device(Some(&devices[0].name)).expect("pick one");
        audio.set_device(None).expect("back to the default");
    }

    /// Opens the real sound output at volume 0 and plays a cached song for a moment:
    /// `cargo test -- --ignored opens_the_sound_output`.
    #[test]
    #[ignore = "needs libmpv, a sound server and a song in the cache"]
    fn opens_the_sound_output() {
        let song = cached_song();
        let audio = Audio::new().expect("mpv");
        audio.set_volume(0.0);
        audio.play(&song, &Trim::default()).expect("play");
        std::thread::sleep(std::time::Duration::from_millis(2000));
        assert!(audio.output().is_some(), "mpv opened no sound output");
        assert!(
            audio.position().unwrap_or(0.0) > 0.5,
            "audio did not advance"
        );
        assert!(audio.duration().unwrap_or(0.0) > 1.0, "no duration");
    }
}
