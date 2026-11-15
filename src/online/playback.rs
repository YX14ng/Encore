//! Playing songs: the queue, downloading the next song so it follows with no gap (or fades in),
//! autoplay's similar songs, and mpv through the mixer.

use iced::Task;
use rust_i18n::t;

use super::{Online, Poll};
use crate::Message;
use crate::audio_settings::AudioSettings;
use crate::download;
use crate::mixer::Mixer;
use crate::queue::{Queue, Repeat};
use crate::silence::{self, SongFile};
use crate::youtube::{self, SearchTrack, TopResult};

impl Online {
    /// The top result's Play button: plays the song, or opens and plays the collection.
    /// Returns `true` if a collection is being opened, so the app can show it.
    pub fn play_top(&mut self) -> (bool, Task<Message>) {
        match self.page.top.clone() {
            Some(TopResult::Track(track)) => {
                let mut queue = vec![track.clone()];
                queue.extend(
                    self.page
                        .songs
                        .iter()
                        .filter(|song| song.id != track.id)
                        .cloned(),
                );
                (false, self.play_queue(queue, 0))
            }
            Some(TopResult::Tile(tile)) => {
                let task = self.open(tile);
                self.play_when_loaded = true;
                (true, task)
            }
            None => (false, Task::none()),
        }
    }

    /// Replaces the queue with `tracks` and plays song `index`; with shuffle on, the rest play
    /// in random order.
    pub fn play_queue(&mut self, tracks: Vec<SearchTrack>, index: usize) -> Task<Message> {
        self.queue
            .replace(tracks, index, self.shuffle, &mut self.rng);
        self.play(index)
    }

    /// Adds `track` at the end of the queue, or plays it when nothing is queued.
    pub fn add_to_queue(&mut self, track: SearchTrack) -> Task<Message> {
        if !self.is_active() {
            return self.play_queue(vec![track], 0);
        }
        self.queue.add(track);
        self.prefetch_next()
    }

    /// Moves song `index` of the queue to play right after the current one.
    pub fn queue_play_next(&mut self, index: usize) -> Task<Message> {
        self.queue.play_next(index);
        self.prefetch_next()
    }

    /// Takes song `index` out of the queue.
    pub fn queue_remove(&mut self, index: usize) -> Task<Message> {
        self.queue.remove(index);
        // Taking out the last song lets autoplay ask again for similar songs.
        self.radio_for = None;
        self.prefetch_next()
    }

    /// Takes every song after the current one out of the queue.
    pub fn queue_clear(&mut self) -> Task<Message> {
        self.queue.clear_upcoming();
        self.next_file = None;
        // With autoplay on, similar songs take the cleared ones' place.
        self.radio_for = None;
        self.prefetch_next()
    }

    /// The queue, for the queue panel.
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// Turns shuffle on or off for the queue and the queues to come, and downloads the song
    /// that now plays next.
    pub fn set_shuffle(&mut self, shuffle: bool) -> Task<Message> {
        self.shuffle = shuffle;
        self.queue.set_shuffle(shuffle, &mut self.rng);
        self.prefetch_next()
    }

    /// Changes the repeat mode, and with it the song that comes next.
    pub fn set_repeat(&mut self, repeat: Repeat) -> Task<Message> {
        self.repeat = repeat;
        self.prefetch_next()
    }

    /// Turns autoplay on or off. On, it fills the queue if it is running out.
    pub fn set_autoplay(&mut self, autoplay: bool) -> Task<Message> {
        self.autoplay = autoplay;
        self.prefetch_next()
    }

    /// Whether autoplay is on, for the queue panel.
    pub fn autoplay(&self) -> bool {
        self.autoplay
    }

    /// Asks for songs like the one playing when nothing comes after it, once per song.
    fn fill_from_radio(&mut self) -> Task<Message> {
        let Some(seed) = self.queue.playing_track().map(|track| track.id.clone()) else {
            return Task::none();
        };
        if !self.autoplay || self.radio_for.as_deref() == Some(seed.as_str()) {
            return Task::none();
        }
        let Some(client) = self.client.clone() else {
            return Task::none();
        };
        self.radio_for = Some(seed.clone());
        Task::perform(youtube::radio(client, seed.clone()), move |songs| {
            Message::RadioLoaded(seed.clone(), songs)
        })
    }

    /// Adds autoplay's similar songs, if the queue still runs out, and downloads the next one.
    /// A failed request only means the queue ends; it is asked again on the next song.
    pub fn radio_loaded(
        &mut self,
        seed: String,
        songs: Result<Vec<SearchTrack>, String>,
    ) -> Task<Message> {
        let Ok(songs) = songs else {
            if self.radio_for.as_deref() == Some(seed.as_str()) {
                self.radio_for = None;
            }
            return Task::none();
        };
        if !self.autoplay || !self.is_active() || self.following().is_some() {
            return Task::none();
        }
        self.queue.add_suggested(songs);
        // The song may have ended while the radio loaded; then the first similar one plays.
        let ended = self.loading.is_none() && self.mixer.as_ref().is_some_and(Mixer::is_idle);
        match self.following() {
            Some(next) if ended => self.play(next),
            _ => self.prefetch_next(),
        }
    }

    /// The song that comes after the current one: itself with repeat one, and the first one
    /// after the last with repeat all.
    fn following(&self) -> Option<usize> {
        match self.repeat {
            Repeat::One => self.queue.playing(),
            Repeat::All => self.queue.next(true),
            Repeat::Off => self.queue.next(false),
        }
    }

    /// Forgets the song queued in mpv and downloads the one that now comes next, so it can
    /// follow with no gap (or fade in).
    fn prefetch_next(&mut self) -> Task<Message> {
        self.forget_queued();
        if self.following().is_none() {
            return self.fill_from_radio();
        }
        self.following()
            .and_then(|next| self.queue.get(next))
            .map_or_else(Task::none, |next| {
                let id = next.id.clone();
                Task::perform(
                    silence::song_file(id.clone(), self.source(), self.skip_silence),
                    move |song| Message::Prefetched(id.clone(), song),
                )
            })
    }

    fn forget_queued(&mut self) {
        if self.queued.take().is_some()
            && let Some(mixer) = &self.mixer
        {
            mixer.clear_queued();
        }
    }

    /// Keeps a downloaded next song for crossfading, and queues it in mpv to follow the
    /// current one with no gap.
    pub fn prefetched(&mut self, id: String, song: Result<SongFile, String>) {
        let Ok(song) = song else {
            return;
        };
        let next = self
            .following()
            .filter(|&next| self.queue.get(next).is_some_and(|track| track.id == id));
        if let (Some(next), Some(mixer), None) = (next, &self.mixer, &self.loading)
            && mixer.queue_next(&song).is_ok()
        {
            self.queued = Some(next);
        }
        self.next_file = Some((id, song));
    }

    /// The song mpv went on into by itself, once it does, with the download of the one after
    /// it.
    pub fn went_on(&mut self) -> Option<(SearchTrack, Task<Message>)> {
        let next = self.queued?;
        if !self.mixer.as_ref()?.went_on() {
            return None;
        }
        self.queued = None;
        self.playing_file = self.next_file.take().map(|(_, song)| song.path);
        self.queue.set_playing(next);
        self.heard = false;
        let track = self.queue.get(next)?.clone();
        Some((track, self.prefetch_next()))
    }

    /// The file of the song that is playing.
    pub fn playing_file(&self) -> Option<&std::path::Path> {
        self.playing_file.as_deref()
    }

    /// Fades into the next song when the current one is `seconds` from its end (in real time,
    /// so at the playing `speed`) and the next song is downloaded. Returns the song that
    /// started and the download of the one after it.
    pub fn crossfade(
        &mut self,
        seconds: u32,
        speed: f32,
        wrap: bool,
    ) -> Option<(SearchTrack, Task<Message>)> {
        let mixer = self.mixer.as_mut()?;
        if seconds == 0 || mixer.is_fading() || self.loading.is_some() {
            return None;
        }
        let left = (mixer.end()? - mixer.position()?) / f64::from(speed.max(0.1));
        if left > f64::from(seconds) {
            return None;
        }
        let next = self.queue.next(wrap)?;
        let track = self.queue.get(next)?.clone();
        let (id, song) = self.next_file.as_ref()?;
        if *id != track.id {
            return None;
        }
        let length = std::time::Duration::from_secs_f64(left.max(1.0));
        if let Err(error) = mixer.crossfade_to(song, length) {
            self.error = Some(error);
            return None;
        }
        self.playing_file = Some(song.path.clone());
        self.next_file = None;
        self.queued = None;
        self.queue.set_playing(next);
        self.heard = false;
        Some((track, self.prefetch_next()))
    }

    /// Moves a running crossfade along.
    pub fn step_fade(&mut self) {
        if let Some(mixer) = &mut self.mixer {
            mixer.step();
        }
    }

    /// The sound outputs that can be picked, asked of mpv now.
    pub fn audio_devices(&self) -> Vec<crate::audio::Device> {
        self.mixer.as_ref().map(Mixer::devices).unwrap_or_default()
    }

    /// Where songs are downloaded from: the chosen quality, with the session for High.
    fn source(&self) -> download::Source {
        let cookie = (self.quality == download::Quality::High)
            .then(|| self.client.as_ref()?.user_auth_cookie_header())
            .flatten();
        download::Source {
            quality: self.quality,
            cookie,
        }
    }

    /// Applies the audio screen's settings to mpv. Skipping silence and the quality start with
    /// the next song that downloads.
    pub fn apply_audio(&mut self, settings: &AudioSettings) -> Result<(), String> {
        self.skip_silence = settings.skip_silence;
        self.quality = settings.quality;
        match &mut self.mixer {
            Some(mixer) => mixer.apply(settings),
            None => Ok(()),
        }
    }

    /// Downloads song `index` of the queue (or takes it from the cache) so it can play.
    pub fn play(&mut self, index: usize) -> Task<Message> {
        let Some(id) = self.queue.get(index).map(|track| track.id.clone()) else {
            return Task::none();
        };
        self.error = None;
        self.loading = Some(id.clone());
        // The song playing now must not go on into the queued one while this one downloads.
        self.forget_queued();
        Task::perform(
            silence::song_file(id, self.source(), self.skip_silence),
            move |song| Message::AudioReady(index, song),
        )
    }

    /// Starts the downloaded file. Returns the song that started, and prefetches the next one.
    pub fn audio_ready(
        &mut self,
        index: usize,
        song: Result<SongFile, String>,
    ) -> (Option<SearchTrack>, Task<Message>) {
        let Some(track) = self.queue.get(index) else {
            return (None, Task::none());
        };
        if self.loading.as_deref() != Some(track.id.as_str()) {
            // Another song was picked while this one was downloading.
            return (None, Task::none());
        }
        self.loading = None;

        let started = song.and_then(|song| {
            let mixer = self
                .mixer
                .as_mut()
                .ok_or_else(|| t!("online.mpv_not_started").into_owned())?;
            mixer.play(&song)?;
            Ok(song.path)
        });
        match started {
            Ok(path) => self.playing_file = Some(path),
            Err(error) => {
                self.error = Some(error);
                return (None, Task::none());
            }
        }

        self.queue.set_playing(index);
        self.heard = false;
        (self.queue.get(index).cloned(), self.prefetch_next())
    }

    /// Whether a YouTube song is (or was last) playing.
    pub fn is_active(&self) -> bool {
        self.queue.playing().is_some()
    }

    /// The YouTube song that is playing.
    pub fn playing_track(&self) -> Option<&SearchTrack> {
        self.queue.playing_track()
    }

    /// The index of the song that is playing, to play it again.
    pub fn playing_index(&self) -> Option<usize> {
        self.queue.playing()
    }

    /// Pauses or resumes the audio.
    pub fn set_paused(&mut self, paused: bool) {
        if let Some(mixer) = &mut self.mixer {
            mixer.set_paused(paused);
        }
    }

    /// Sets the volume, from 0 to 100.
    pub fn set_volume(&mut self, volume: f32) {
        if let Some(mixer) = &mut self.mixer {
            mixer.set_volume(volume);
        }
    }

    /// Jumps to `seconds` into the song.
    pub fn seek(&mut self, seconds: f64) {
        if let Some(mixer) = &mut self.mixer {
            mixer.seek(seconds);
        }
    }

    /// Asks mpv where it is.
    pub fn poll(&mut self) -> Poll {
        let Some(mixer) = &self.mixer else {
            return Poll::Waiting;
        };
        if let Some(position) = mixer.position() {
            self.heard = true;
            return Poll::Position(position as u32);
        }
        if self.heard && mixer.is_idle() {
            self.heard = false;
            return Poll::Finished;
        }
        Poll::Waiting
    }

    /// The format of the playing song, like "OPUS 152".
    pub fn format_label(&self) -> Option<String> {
        self.mixer.as_ref()?.format_label()
    }

    /// Milliseconds into the playing song, for keeping lyrics in sync between ticks.
    pub fn position_ms(&self) -> Option<u32> {
        let seconds = self.mixer.as_ref()?.position()?;
        Some((seconds * 1000.0) as u32)
    }

    /// The length of the playing song according to mpv, for lists that come without durations.
    pub fn duration(&self) -> Option<u32> {
        let seconds = self.mixer.as_ref()?.duration()?;
        (seconds > 0.0).then_some(seconds as u32)
    }

    /// The song after the one that is playing; with `wrap`, the first after the last.
    pub fn next_index(&self, wrap: bool) -> Option<usize> {
        self.queue.next(wrap)
    }

    /// The song before the one that is playing; with `wrap`, the last before the first.
    pub fn previous_index(&self, wrap: bool) -> Option<usize> {
        self.queue.previous(wrap)
    }
}
