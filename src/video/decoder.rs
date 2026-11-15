//! Turns a downloaded video into frames for the screen: ffmpeg decodes it from a given time,
//! scaled to `WIDTH`×`HEIGHT` RGBA at `FPS`, and a thread reads the frames from its pipe. The
//! pipe holds a few frames, so a paused song simply leaves ffmpeg waiting.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, TryRecvError, sync_channel};

use rust_i18n::t;

pub const WIDTH: u32 = 960;
pub const HEIGHT: u32 = 540;
pub const FPS: f64 = 30.0;
/// Frames read ahead of the one on screen.
const AHEAD: usize = 6;
/// How far ahead of the newest frame the song may get before decoding starts over there:
/// further than ffmpeg catches up with quickly, so it was a seek.
const CATCH_UP: f64 = 2.0;

/// One picture and when it shows, in seconds into the video.
pub struct Frame {
    pub time: f64,
    pub pixels: Vec<u8>,
}

/// What the decoder has for a moment of the video.
pub enum Step {
    /// The frame on screen is still the right one.
    Same,
    /// A newer frame is due.
    New(Frame),
    /// The moment is outside what this decoder will give (a seek): start a new one there.
    Restart,
}

/// ffmpeg decoding one video from one moment on.
pub struct Decoder {
    child: Child,
    frames: Receiver<Frame>,
    /// Where decoding started, in seconds into the video.
    start: f64,
    /// The newest frame received, and whether ffmpeg is done.
    newest: f64,
    ended: bool,
    /// A frame received before its time.
    waiting: Option<Frame>,
}

impl Decoder {
    /// Starts decoding `path` from `at` seconds.
    pub fn start(path: &Path, at: f64) -> Result<Self, String> {
        let at = at.max(0.0);
        let filter = format!(
            "fps={FPS},scale={WIDTH}:{HEIGHT}:force_original_aspect_ratio=decrease,\
             pad={WIDTH}:{HEIGHT}:(ow-iw)/2:(oh-ih)/2:black"
        );
        let mut child = Command::new("ffmpeg")
            .args(["-hide_banner", "-nostats", "-nostdin", "-loglevel", "error"])
            .args(["-ss", &format!("{at:.3}"), "-i"])
            .arg(path)
            .args([
                "-an", "-vf", &filter, "-pix_fmt", "rgba", "-f", "rawvideo", "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| t!("video.ffmpeg_not_run", error = error).into_owned())?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| t!("video.ffmpeg_no_output").into_owned())?;
        let (sender, frames) = sync_channel(AHEAD);
        std::thread::spawn(move || {
            let size = (WIDTH * HEIGHT * 4) as usize;
            for index in 0_u32.. {
                let mut pixels = vec![0; size];
                if stdout.read_exact(&mut pixels).is_err() {
                    return;
                }
                let time = at + f64::from(index) / FPS;
                if sender.send(Frame { time, pixels }).is_err() {
                    return;
                }
            }
        });
        Ok(Self {
            child,
            frames,
            start: at,
            newest: at,
            ended: false,
            waiting: None,
        })
    }

    /// The frame for `time` seconds into the video.
    pub fn frame_at(&mut self, time: f64) -> Step {
        if time < self.start - 0.25 || (!self.ended && time > self.newest + CATCH_UP) {
            return Step::Restart;
        }
        let due = time + 0.5 / FPS;
        let mut latest = None;
        loop {
            let frame = match self.waiting.take() {
                Some(frame) => frame,
                None => match self.frames.try_recv() {
                    Ok(frame) => {
                        self.newest = frame.time;
                        frame
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.ended = true;
                        break;
                    }
                },
            };
            if frame.time > due {
                self.waiting = Some(frame);
                break;
            }
            latest = Some(frame);
        }
        latest.map_or(Step::Same, Step::New)
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Makes a 3 second test video with ffmpeg in a temporary folder.
    fn test_video(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("encore-video-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join(name);
        let made = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi"])
            .args(["-i", "testsrc=size=640x360:rate=25:duration=3"])
            .arg(&path)
            .status()
            .expect("ffmpeg");
        assert!(made.success());
        path
    }

    fn wait_for(decoder: &mut Decoder, time: f64) -> Option<Frame> {
        for _ in 0..200 {
            if let Step::New(frame) = decoder.frame_at(time) {
                return Some(frame);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        None
    }

    /// Decodes a generated video: `cargo test -- --ignored decodes_frames_in_time`.
    #[test]
    #[ignore = "needs ffmpeg"]
    fn decodes_frames_in_time() {
        let path = test_video("frames.mp4");
        let mut decoder = Decoder::start(&path, 1.0).expect("start");
        let first = wait_for(&mut decoder, 1.0).expect("a first frame");
        assert!((first.time - 1.0).abs() < 1e-9);
        assert_eq!(first.pixels.len(), (WIDTH * HEIGHT * 4) as usize);
        // The padding above the 16:9 picture is black, and the picture is not.
        assert!(first.pixels.iter().any(|byte| *byte > 0));

        // Until ffmpeg gets there, the newest frame it has is shown; then the due one.
        let mut later = wait_for(&mut decoder, 1.5).expect("a later frame");
        while later.time < 1.45 {
            later = wait_for(&mut decoder, 1.5).expect("the due frame");
        }
        assert!(later.time <= 1.52, "{}", later.time);
        assert!(
            matches!(decoder.frame_at(0.5), Step::Restart),
            "a seek back"
        );
        assert!(
            matches!(decoder.frame_at(9.0), Step::Restart),
            "a seek ahead"
        );
        let _ = std::fs::remove_dir_all(path.parent().expect("dir"));
    }
}
