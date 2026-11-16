//! Skipping the silence at the start and end of songs. ffmpeg's `silencedetect` listens to the
//! first and last seconds of the downloaded file, and mpv is told where to start and stop.
//! Silences inside a song are part of it and stay.

use std::path::{Path, PathBuf};

use crate::download;

/// Quieter than this counts as silence.
const THRESHOLD: &str = "-50dB";
/// The seconds listened to at the start, for a silent intro.
const HEAD_SECONDS: f64 = 20.0;
/// The seconds listened to at the end, for a silent outro.
const TAIL_SECONDS: f64 = 30.0;
/// The shortest silence worth skipping at the start, in seconds.
const SHORTEST_HEAD: f64 = 0.5;
/// The shortest silence worth skipping at the end, in seconds.
const SHORTEST_TAIL: f64 = 1.0;
/// Seconds kept before the music starts and after it ends, so nothing is cut abruptly.
const MARGIN: f64 = 0.15;

/// Where a song should start and stop, in seconds; `None` plays from the start or to the end.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Trim {
    pub start: Option<f64>,
    pub end: Option<f64>,
}

impl Trim {
    /// mpv's per-file options for `loadfile` ("start=1.200,end=201.350"), or `None`.
    pub fn mpv_options(&self) -> Option<String> {
        let options: Vec<String> = [("start", self.start), ("end", self.end)]
            .into_iter()
            .filter_map(|(name, seconds)| seconds.map(|seconds| format!("{name}={seconds:.3}")))
            .collect();
        (!options.is_empty()).then(|| options.join(","))
    }
}

/// A downloaded song, with where to start and stop it.
#[derive(Debug, Clone, PartialEq)]
pub struct SongFile {
    pub path: PathBuf,
    pub trim: Trim,
}

/// Downloads song `id` from `source` (or takes it from the cache) and, with `skip_silence`,
/// finds the silence at its start and end. Without ffmpeg the song simply plays whole.
pub async fn song_file(
    id: String,
    source: download::Source,
    skip_silence: bool,
) -> Result<SongFile, String> {
    let path = download::audio(id, source).await?;
    let trim = if skip_silence {
        find(&path).await
    } else {
        Trim::default()
    };
    Ok(SongFile { path, trim })
}

/// The silence at the start and the end of the file at `path`.
async fn find(path: &Path) -> Trim {
    let head_window = HEAD_SECONDS.to_string();
    let tail_window = format!("-{TAIL_SECONDS}");
    let head_args = ["-t", head_window.as_str()];
    let tail_args = ["-sseof", tail_window.as_str()];
    let (head, tail) = tokio::join!(listen(path, &head_args), listen(path, &tail_args));
    let start = head.and_then(|output| leading(&output));
    let end = tail.and_then(|output| trailing(&output));
    let end = end.filter(|end| start.is_none_or(|start| *end > start + 5.0));
    Trim { start, end }
}

/// What `silencedetect` said about part of the file (its stderr), or `None` if ffmpeg failed.
async fn listen(path: &Path, window: &[&str]) -> Option<String> {
    let filter = format!("silencedetect=n={THRESHOLD}:d={SHORTEST_HEAD}");
    let output = crate::platform::command("ffmpeg")
        .args(["-hide_banner", "-nostats", "-nostdin"])
        .args(window)
        .arg("-i")
        .arg(path)
        .args(["-af", &filter, "-f", "null", "-"])
        .kill_on_drop(true)
        .output()
        .await
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stderr).into_owned())
}

/// The silences `silencedetect` reported, as (start, end) in seconds from the start of the
/// part it listened to. A silence still going when the part ended has no end.
fn silences(output: &str) -> Vec<(f64, Option<f64>)> {
    let mut found: Vec<(f64, Option<f64>)> = Vec::new();
    for line in output.lines() {
        if let Some(start) = number_after(line, "silence_start:") {
            found.push((start, None));
        } else if let Some(end) = number_after(line, "silence_end:")
            && let Some(last) = found.last_mut()
        {
            last.1 = Some(end);
        }
    }
    found
}

/// Where the music starts, if the file opens with silence.
fn leading(output: &str) -> Option<f64> {
    let (start, end) = *silences(output).first()?;
    let end = end?;
    // A silence that fills the whole part is not an intro; the song may be quiet throughout.
    (start <= 0.05 && end - start >= SHORTEST_HEAD && end < HEAD_SECONDS - 0.5)
        .then(|| (end - MARGIN).max(0.0))
}

/// Where the music ends, if the file closes with silence. The part listened to is the last
/// `TAIL_SECONDS` (or the whole file if it is shorter), so its times are moved by where it began.
fn trailing(output: &str) -> Option<f64> {
    let length = duration(output)?;
    let window = length.min(TAIL_SECONDS);
    let offset = length - window;
    let (start, end) = *silences(output).last()?;
    let reaches_the_end = end.is_none_or(|end| end >= window - 0.1);
    let long = end.unwrap_or(window) - start >= SHORTEST_TAIL;
    (reaches_the_end && long && start > 0.0).then_some(offset + start + MARGIN)
}

/// The file's length from ffmpeg's "Duration: 00:03:30.36" line.
fn duration(output: &str) -> Option<f64> {
    let line = output.lines().find(|line| line.contains("Duration:"))?;
    let time = line.split("Duration:").nth(1)?.split(',').next()?.trim();
    let mut seconds = 0.0;
    for part in time.split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(seconds)
}

/// The number that follows `label` on `line`.
fn number_after(line: &str, label: &str) -> Option<f64> {
    line.split(label)
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "\
Input #0, matroska,webm, from 'song.webm':
  Duration: 00:03:30.36, start: 0.000000, bitrate: 132 kb/s
[Parsed_silencedetect_0 @ 0x1] silence_start: 0
[Parsed_silencedetect_0 @ 0x1] silence_end: 2.5 | silence_duration: 2.5
";

    const TAIL: &str = "\
  Duration: 00:03:30.36, start: 0.000000, bitrate: 132 kb/s
[Parsed_silencedetect_0 @ 0x1] silence_start: 3.1
[Parsed_silencedetect_0 @ 0x1] silence_end: 3.9 | silence_duration: 0.8
[Parsed_silencedetect_0 @ 0x1] silence_start: 26.821
[Parsed_silencedetect_0 @ 0x1] silence_end: 29.986667 | silence_duration: 3.165667
";

    #[test]
    fn a_silent_intro_is_skipped() {
        assert_eq!(leading(HEAD), Some(2.5 - MARGIN));
    }

    #[test]
    fn a_pause_in_the_intro_is_not_the_start() {
        let late = HEAD.replace("silence_start: 0\n", "silence_start: 4.2\n");
        assert_eq!(leading(&late), None);
    }

    #[test]
    fn a_silent_outro_ends_the_song_early() {
        // The last 30 s of 210.36 s start at 180.36.
        let end = trailing(TAIL).expect("an end");
        assert!((end - (180.36 + 26.821 + MARGIN)).abs() < 1e-6, "{end}");
    }

    #[test]
    fn a_silence_before_the_last_note_is_kept() {
        let middle = TAIL.replace("silence_end: 29.986667", "silence_end: 28.0");
        assert_eq!(trailing(&middle), None);
    }

    #[test]
    fn a_silence_still_going_at_the_end_counts() {
        let open = TAIL.lines().take(4).collect::<Vec<_>>().join("\n");
        assert!(trailing(&open).is_some());
    }

    #[test]
    fn short_files_are_listened_to_whole() {
        let short = TAIL.replace("00:03:30.36", "00:00:20.00");
        let short = short
            .replace("26.821", "16.0")
            .replace("29.986667", "19.99");
        assert_eq!(trailing(&short), Some(16.0 + MARGIN));
    }

    #[test]
    fn options_name_only_what_is_trimmed() {
        assert_eq!(Trim::default().mpv_options(), None);
        let trim = Trim {
            start: Some(1.2),
            end: Some(201.35),
        };
        assert_eq!(
            trim.mpv_options().as_deref(),
            Some("start=1.200,end=201.350")
        );
        let end_only = Trim {
            start: None,
            end: Some(9.0),
        };
        assert_eq!(end_only.mpv_options().as_deref(), Some("end=9.000"));
    }

    /// Finds the silence in the songs of the cache: `cargo test -- --ignored finds_silence`.
    #[tokio::test]
    #[ignore = "needs ffmpeg and songs in the cache"]
    async fn finds_silence() {
        let songs: Vec<_> = std::fs::read_dir(download::cache_dir().join("audio"))
            .expect("cache dir")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "webm"))
            .collect();
        assert!(!songs.is_empty(), "no songs in the cache");
        let mut found = Vec::new();
        for song in &songs {
            found.push((
                song.file_name().map(|name| name.to_owned()),
                find(song).await,
            ));
        }
        assert!(
            found.iter().any(|(_, trim)| trim.end.is_some()),
            "no song ends in silence: {found:?}"
        );
    }
}
