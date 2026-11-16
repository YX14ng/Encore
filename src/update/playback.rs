//! Playing: what happens when a song starts or ends, previous and next, seeking, and keeping
//! the lyrics and the clock on the song.

use iced::Task;

use crate::clock::Playback;
use crate::lyrics::Lyrics;
use crate::queue::Repeat;
use crate::song::Song;
use crate::youtube::SearchTrack;

use crate::{
    App, Message, clock, cover_looks, load_cover, load_lyrics, lyrics, lyrics_source,
    notifications, sponsorblock,
};

impl App {
    /// Shows a YouTube track in the player once its audio has started.
    pub(crate) fn start_online(
        &mut self,
        index: usize,
        song: Result<crate::silence::SongFile, String>,
    ) -> Task<Message> {
        let (started, prefetch) = self.online.audio_ready(index, song);
        let Some(track) = started else {
            return prefetch;
        };
        let started = self.song_started(track);
        Task::batch([prefetch, started])
    }

    /// Shows the song mpv went on into with no gap, once it does.
    pub(super) fn went_on(&mut self) -> Option<Task<Message>> {
        let (track, prefetch) = self.online.went_on()?;
        let started = self.song_started(track);
        Some(Task::batch([prefetch, started]))
    }

    /// Shows a song that just started (by itself or by crossfading into it): the player bar,
    /// the background and the lyrics, and tells Last.fm.
    pub(super) fn song_started(&mut self, track: SearchTrack) -> Task<Message> {
        if let Some(skip) = self.show_rating(&track) {
            return skip;
        }
        let cover = track
            .cover_url
            .as_deref()
            .and_then(|url| self.online.playing_cover(url));
        // The list's cover is small: the player shows a large one once it arrives.
        let large_cover = track
            .cover_url
            .as_deref()
            .map_or_else(Task::none, |url| self.online.fetch_large_cover(url));
        self.now_playing = Song::from_search(&track, cover);
        self.audio_format = None;
        let looks = cover_looks(
            track
                .cover_url
                .as_deref()
                .and_then(|url| self.online.image_bytes(url))
                .as_deref(),
        );
        self.set_cover_looks(looks);
        let duration = track.duration;
        self.playback = Playback {
            elapsed: 0,
            duration,
            is_playing: true,
        };
        self.lyrics = Lyrics::Loading;
        self.romanized = None;
        self.follow_lyrics();
        let now_playing = if self.plugins.lastfm {
            self.lastfm.track_started(&track)
        } else {
            Task::none()
        };
        let notice = if self.plugins.notifications {
            let image_path = self
                .now_playing
                .art_url
                .as_deref()
                .and_then(|url| url.strip_prefix("file://"))
                .filter(|path| std::path::Path::new(path).exists())
                .map(str::to_string);
            Task::perform(
                notifications::show(notifications::SongNotice {
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    album: track.album.clone(),
                    image_path,
                }),
                |()| Message::NotificationShown,
            )
        } else {
            Task::none()
        };
        self.skip_segments = None;
        let segments = if self.plugins.sponsorblock {
            let id = track.id.clone();
            Task::perform(sponsorblock::fetch(id.clone()), move |segments| {
                Message::SegmentsLoaded(id.clone(), segments)
            })
        } else {
            Task::none()
        };
        let visualizer = match self.online.playing_file() {
            Some(path) if self.plugins.visualizer => self
                .visualizer
                .analyze(track.id.clone(), path.to_path_buf()),
            _ => {
                self.visualizer.clear();
                Task::none()
            }
        };
        let video = self.find_video();
        let id = track.id.clone();
        let lyrics = Task::perform(
            lyrics_source::fetch(track, self.online.client()),
            move |lyrics| Message::LyricsLoaded(id.clone(), lyrics),
        );
        Task::batch([
            lyrics,
            now_playing,
            notice,
            segments,
            visualizer,
            video,
            large_cover,
        ])
    }

    /// With SponsorBlock on, jumps over the part of the video that is playing if the community
    /// marked it as not music.
    pub(super) fn skip_offtopic(&mut self) {
        if !self.plugins.sponsorblock {
            return;
        }
        let Some((id, segments)) = &self.skip_segments else {
            return;
        };
        if self
            .online
            .playing_track()
            .is_none_or(|track| &track.id != id)
        {
            return;
        }
        let position = f64::from(self.position_ms()) / 1000.0;
        if let Some(end) = sponsorblock::skip_target(segments, position) {
            self.seek(end as f32);
        }
    }

    /// Milliseconds into the song: from mpv for real songs, from the clock for samples.
    pub(crate) fn position_ms(&self) -> u32 {
        let from_clock = self.playback.elapsed * 1000;
        if self.online.is_active() {
            self.online.position_ms().unwrap_or(from_clock)
        } else {
            from_clock
        }
    }

    /// A song ended: plays it again with repeat one, else the next one.
    pub(super) fn song_finished(&mut self) -> Task<Message> {
        if self.audio_settings.repeat == Repeat::One
            && let Some(index) = self.online.playing_index()
        {
            return self.online.play(index);
        }
        self.play_next_online()
    }

    /// The previous button: back to the start of the song, or to the previous song when the
    /// current one has just begun.
    pub(super) fn previous(&mut self) -> Task<Message> {
        const RESTART_AFTER: u32 = 3;
        let wrap = self.audio_settings.repeat == Repeat::All;
        if self.position_ms() / 1000 < RESTART_AFTER {
            if self.online.is_active() {
                if let Some(index) = self.online.previous_index(wrap) {
                    return self.online.play(index);
                }
            } else if self.samples {
                let count = self.tracks.len();
                self.show_sample((self.current + count - 1) % count);
                return Task::none();
            }
        }
        self.seek(0.0);
        Task::none()
    }

    /// Plays or pauses, if it is not doing so already.
    pub(crate) fn set_playing(&mut self, playing: bool) {
        if self.playback.is_playing != playing {
            self.playback.toggle();
            if self.online.is_active() {
                self.online.set_paused(!playing);
            }
        }
    }

    /// Jumps to `seconds` into the song.
    pub(crate) fn seek(&mut self, seconds: f32) {
        let seconds = seconds.max(0.0);
        if self.online.is_active() {
            self.online.seek(f64::from(seconds));
        }
        self.playback.elapsed = seconds as u32;
        self.follow_lyrics();
    }

    /// Plays the next song of the queue, if there is one (after the last one only with repeat).
    pub(crate) fn play_next_online(&mut self) -> Task<Message> {
        let wrap = self.audio_settings.repeat != Repeat::Off;
        match self.online.next_index(wrap) {
            Some(next) => self.online.play(next),
            None => {
                self.playback.is_playing = false;
                Task::none()
            }
        }
    }

    /// Downloads the covers of the open collection's songs in sight; the others wait until the
    /// list is scrolled to them.
    pub(crate) fn fetch_covers_in_sight(&mut self) -> Task<Message> {
        let count = self.online.collection().0.map_or(0, |c| c.tracks.len());
        let rows = crate::collection::rows_in_sight(self.collection_scroll, count);
        self.online.fetch_collection_covers(rows)
    }

    /// Whether a song is in the player: one from YouTube, or a sample.
    pub(crate) fn has_song(&self) -> bool {
        self.online.is_active() || self.samples
    }

    /// Moves to the next sample track: new cover, new background and the clock back at zero.
    pub(crate) fn next_track(&mut self) {
        self.show_sample(clock::next_index(self.current, self.tracks.len()));
    }

    /// Shows sample track `index` from the start.
    pub(super) fn show_sample(&mut self, index: usize) {
        self.current = index;
        let track = &self.tracks[self.current];
        self.playback.restart(track.duration);
        let looks = load_cover(track);
        self.lyrics = load_lyrics(track);
        self.now_playing = Song::from_mock(track);
        self.set_cover_looks(looks);
        self.romanize_lyrics();
        self.follow_lyrics();
    }

    /// Starts scrolling the lyrics panel when the current line changes.
    pub(crate) fn follow_lyrics(&mut self) {
        let line =
            lyrics::current_line(self.lyrics.synced(), self.position_ms()).unwrap_or(0) as f32;
        if line != self.lyrics_scroll.value() {
            self.lyrics_scroll.go_mut(line, self.now);
        }
    }

    /// Shows the song being downloaded in the player bar at once, with "Loading…", instead of
    /// keeping the previous one there until YouTube lets the download start.
    pub(super) fn show_loading_song(&mut self) {
        let Some(track) = self.online.loading_track().cloned() else {
            self.loading_shown = None;
            return;
        };
        if self.loading_shown.as_deref() == Some(track.id.as_str()) {
            return;
        }
        self.loading_shown = Some(track.id.clone());
        let cover = track
            .cover_url
            .as_deref()
            .and_then(|url| self.online.image(url));
        self.now_playing = Song::from_search(&track, cover);
        self.playback = Playback {
            elapsed: 0,
            duration: track.duration,
            is_playing: true,
        };
        self.audio_format = None;
        self.lyrics = Lyrics::Loading;
        self.romanized = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Message;

    #[test]
    fn the_app_starts_with_nothing_playing() {
        let mut app = App::starting(false);
        assert!(app.now_playing.is_nothing());
        assert_eq!(app.lyrics, Lyrics::Idle);
        let _ = app.handle(Message::TogglePlay);
        let _ = app.handle(Message::Next);
        assert!(!app.playback.is_playing, "there is nothing to play");
        assert!(app.now_playing.is_nothing());
    }
}
