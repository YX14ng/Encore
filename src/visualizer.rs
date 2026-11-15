//! The visualizer: bars that move with the music in the immersive view. When a song starts,
//! ffmpeg decodes its downloaded file and an FFT measures how loud each band of frequencies is
//! every frame; the bars then read the frame at mpv's position, so they stay in step through
//! pauses, seeks and speed changes without listening to the sound card.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::widget::{Row, container, space};
use iced::{Bottom, Element, Fill, border};
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use tokio::process::Command;

use crate::{Message, theme};

/// Bars across the screen.
pub const BANDS: usize = 48;
/// Spectrum frames per second of music, the rate the screen redraws at.
const FPS: f64 = 30.0;
/// The rate the song is decoded at: enough for everything up to 11 kHz.
const SAMPLE_RATE: u32 = 22_050;
/// Samples per FFT: about a tenth of a second.
const WINDOW: usize = 2048;
const LOWEST: f32 = 40.0;
const HIGHEST: f32 = 10_000.0;
/// How far below the loudest band of the song a band reaches the floor, in decibels.
const RANGE_DB: f32 = 54.0;
/// How much of its height a bar keeps from one frame to the next as it falls.
const FALL: f32 = 0.86;

/// How loud each band is, frame by frame, from 0 to 255.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spectrum {
    levels: Vec<[u8; BANDS]>,
}

impl Spectrum {
    /// The bands at `seconds` into the song.
    pub fn at(&self, seconds: f64) -> Option<&[u8; BANDS]> {
        let frame = (seconds.max(0.0) * FPS) as usize;
        self.levels.get(frame)
    }
}

/// What happens with the visualizer.
#[derive(Debug, Clone)]
pub enum Event {
    /// The spectrum of song `id` was measured.
    Analyzed(String, Result<Arc<Spectrum>, String>),
}

/// The bars on screen, which rise at once and fall slowly.
#[derive(Debug, Clone, Default)]
pub struct Visualizer {
    /// The song the spectrum belongs to, and the spectrum once measured.
    pub song: Option<(String, Option<Arc<Spectrum>>)>,
    heights: Vec<f32>,
}

impl Visualizer {
    /// Starts measuring `path`, the file of song `id`.
    pub fn analyze(&mut self, id: String, path: PathBuf) -> iced::Task<Message> {
        self.song = Some((id.clone(), None));
        iced::Task::perform(measure(path), move |spectrum| {
            Message::Visualizer(Event::Analyzed(id.clone(), spectrum.map(Arc::new)))
        })
    }

    /// Keeps a spectrum if it is still the playing song's. A song that cannot be measured
    /// just shows no bars.
    pub fn event(&mut self, event: Event) {
        let Event::Analyzed(id, spectrum) = event;
        if let (Some((song, slot)), Ok(spectrum)) = (&mut self.song, spectrum)
            && *song == id
        {
            *slot = Some(spectrum);
        }
    }

    /// Forgets the song, so no bars show.
    pub fn clear(&mut self) {
        self.song = None;
        self.heights.clear();
    }

    /// Moves the bars to `seconds` into the song; `None` lets them fall (paused or stopped).
    pub fn step(&mut self, seconds: Option<f64>) {
        let spectrum = self
            .song
            .as_ref()
            .and_then(|(_, spectrum)| spectrum.as_deref());
        let target = seconds.and_then(|seconds| spectrum?.at(seconds));
        self.heights.resize(BANDS, 0.0);
        for (band, height) in self.heights.iter_mut().enumerate() {
            let level = target.map_or(0.0, |levels| f32::from(levels[band]) / 255.0);
            *height = level.max(*height * FALL);
        }
    }

    /// Whether there is anything to draw.
    pub fn is_showing(&self) -> bool {
        self.heights.iter().any(|height| *height > 0.01)
    }

    /// The bars, `height` tall at most.
    pub fn view(&self, height: f32) -> Element<'_, Message> {
        Row::with_children(self.heights.iter().map(|level| {
            let bar = (level * height).max(2.0);
            container(space())
                .width(Fill)
                .height(bar)
                .style(move |_| {
                    container::Style::default()
                        .background(theme::accent().scale_alpha(0.35 + 0.5 * level))
                        .border(border::rounded(border::top(3)))
                })
                .into()
        }))
        .spacing(4)
        .align_y(Bottom)
        .height(height)
        .width(Fill)
        .into()
    }
}

/// Decodes `path` to mono samples with ffmpeg and measures its spectrum.
async fn measure(path: PathBuf) -> Result<Spectrum, String> {
    let samples = decode(&path).await?;
    tokio::task::spawn_blocking(move || spectrum(&samples))
        .await
        .map_err(|error| error.to_string())
}

async fn decode(path: &Path) -> Result<Vec<f32>, String> {
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-nostats",
            "-nostdin",
            "-loglevel",
            "error",
            "-i",
        ])
        .arg(path)
        .args(["-vn", "-ac", "1", "-ar", &SAMPLE_RATE.to_string()])
        .args(["-f", "f32le", "pipe:1"])
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|error| format!("could not run ffmpeg: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(output
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| f32::from_le_bytes(*bytes))
        .collect())
}

/// The levels of `samples` (mono, at `SAMPLE_RATE`), `FPS` frames per second, each band
/// measured in decibels and scaled so the loudest moment of the song reaches the top.
fn spectrum(samples: &[f32]) -> Spectrum {
    let fft = FftPlanner::<f32>::new().plan_fft_forward(WINDOW);
    let hann: Vec<f32> = (0..WINDOW)
        .map(|i| {
            let phase = std::f32::consts::TAU * i as f32 / (WINDOW - 1) as f32;
            0.5 - 0.5 * phase.cos()
        })
        .collect();
    let edges = band_edges();
    let hop = f64::from(SAMPLE_RATE) / FPS;
    let frames = (samples.len() as f64 / hop) as usize;

    let mut decibels: Vec<[f32; BANDS]> = Vec::with_capacity(frames);
    let mut buffer = vec![Complex::new(0.0, 0.0); WINDOW];
    for frame in 0..frames {
        // The window is centered on the frame's time.
        let center = (frame as f64 * hop) as isize;
        for (i, slot) in buffer.iter_mut().enumerate() {
            let at = center + i as isize - (WINDOW / 2) as isize;
            let sample = usize::try_from(at)
                .ok()
                .and_then(|at| samples.get(at))
                .copied()
                .unwrap_or(0.0);
            *slot = Complex::new(sample * hann[i], 0.0);
        }
        fft.process(&mut buffer);
        let mut bands = [f32::NEG_INFINITY; BANDS];
        for (band, level) in bands.iter_mut().enumerate() {
            let (low, high) = edges[band];
            let power: f32 =
                buffer[low..high].iter().map(Complex::norm_sqr).sum::<f32>() / (high - low) as f32;
            *level = 10.0 * power.max(1e-12).log10();
        }
        decibels.push(bands);
    }

    let loudest = decibels
        .iter()
        .flatten()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let levels = decibels
        .iter()
        .map(|bands| {
            bands.map(|db| {
                let level = (db - (loudest - RANGE_DB)) / RANGE_DB;
                (level.clamp(0.0, 1.0) * 255.0).round() as u8
            })
        })
        .collect();
    Spectrum { levels }
}

/// The FFT bins of each band, spaced evenly in pitch from `LOWEST` to `HIGHEST`; every band
/// gets at least one bin.
fn band_edges() -> [(usize, usize); BANDS] {
    let bin = |hertz: f32| (hertz * WINDOW as f32 / SAMPLE_RATE as f32).round() as usize;
    let ratio = (HIGHEST / LOWEST).powf(1.0 / BANDS as f32);
    let mut edges = [(0, 0); BANDS];
    let mut low = bin(LOWEST).max(1);
    for (band, edge) in edges.iter_mut().enumerate() {
        let high = bin(LOWEST * ratio.powi(band as i32 + 1)).max(low + 1);
        *edge = (low, high);
        low = high;
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One second of a sine at `hertz`.
    fn tone(hertz: f32) -> Vec<f32> {
        (0..SAMPLE_RATE)
            .map(|i| (std::f32::consts::TAU * hertz * i as f32 / SAMPLE_RATE as f32).sin())
            .collect()
    }

    fn loudest_band(spectrum: &Spectrum) -> usize {
        let levels = spectrum.at(0.5).expect("a frame in the middle");
        (0..BANDS).max_by_key(|band| levels[*band]).expect("bands")
    }

    #[test]
    fn a_low_tone_lights_a_low_bar_and_a_high_one_a_high_bar() {
        let low = loudest_band(&spectrum(&tone(100.0)));
        let high = loudest_band(&spectrum(&tone(5_000.0)));
        assert!(low < BANDS / 4, "100 Hz is bar {low}");
        assert!(high > BANDS * 3 / 4, "5 kHz is bar {high}");
    }

    #[test]
    fn frames_follow_the_music_thirty_times_a_second() {
        let spectrum = spectrum(&tone(440.0));
        assert_eq!(spectrum.levels.len(), 30);
        assert!(spectrum.at(0.99).is_some());
        assert!(spectrum.at(1.5).is_none());
    }

    #[test]
    fn bands_cover_the_range_without_gaps() {
        let edges = band_edges();
        assert!(edges.windows(2).all(|pair| pair[0].1 == pair[1].0));
        assert!(edges.iter().all(|(low, high)| high > low));
        assert!(edges[BANDS - 1].1 < WINDOW / 2);
    }

    #[test]
    fn bars_rise_at_once_and_fall_slowly() {
        let mut visualizer = Visualizer {
            song: Some(("abc".to_string(), Some(Arc::new(spectrum(&tone(440.0)))))),
            heights: Vec::new(),
        };
        visualizer.step(Some(0.5));
        let peak = visualizer.heights.iter().copied().fold(0.0, f32::max);
        assert!(peak > 0.9, "the tone reaches the top: {peak}");
        visualizer.step(None);
        let fallen = visualizer.heights.iter().copied().fold(0.0, f32::max);
        assert!((fallen - peak * FALL).abs() < 1e-4);
        assert!(visualizer.is_showing());
    }

    /// Measures a cached song: `cargo test -- --ignored measures_a_song`.
    #[tokio::test]
    #[ignore = "needs ffmpeg and a song in the cache"]
    async fn measures_a_song() {
        let song = std::fs::read_dir(crate::download::cache_dir().join("audio"))
            .expect("cache dir")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|ext| ext == "webm"))
            .expect("a cached song");
        let started = std::time::Instant::now();
        let spectrum = measure(song).await.expect("spectrum");
        let took = started.elapsed();
        let seconds = spectrum.levels.len() as f64 / FPS;
        let moving = spectrum
            .levels
            .iter()
            .filter(|levels| levels.iter().any(|level| *level > 128))
            .count();
        assert!(
            seconds > 60.0 && moving > spectrum.levels.len() / 2 && took.as_secs() < 5,
            "{seconds:.0} s of music, {moving} lively frames, measured in {took:?}"
        );
    }
}
