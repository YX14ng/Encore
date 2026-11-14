//! How the app reacts to messages: `App::update` and the helpers it calls.

use iced::{Task, window};

use crate::clock::Playback;
use crate::library::{Account, LibraryData, Shelf};
use crate::lyrics::Lyrics;
use crate::media_keys::MediaKey;
use crate::online::Poll;
use crate::queue::Repeat;
use crate::sidebar::NavItem;
use crate::song::Song;
use crate::youtube::SearchTrack;
use iced::Color;
use iced::widget::image;

use crate::{
    App, Message, Screen, account, audio_settings, clock, cover_looks, discord, library,
    load_cover, load_lyrics, lyrics, lyrics_source, mini_player, notifications, offline,
    sign_in_window, sponsorblock, theme, themes,
};

impl App {
    /// Applies one message to the state and returns the work it starts (downloads, requests),
    /// then tells the desktop's media controls what is playing.
    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);
        self.publish_media();
        task
    }

    /// Tells the desktop's media controls (MPRIS) and Discord about the song and the playback
    /// state. Discord only hears about real songs, not the samples.
    fn publish_media(&mut self) {
        let playing = self.online.playing_track().filter(|_| self.plugins.discord);
        let presence = playing.map(|track| discord::Presence {
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            cover_url: track.cover_url.clone(),
            duration: self.playback.duration,
            elapsed: self.playback.elapsed,
            playing: self.playback.is_playing,
            session: self.together.discord_session(),
        });
        self.discord.update(presence);
        self.share_with_listeners();

        let Some(media) = &mut self.media else {
            return;
        };
        let song = &self.now_playing;
        media.publish(
            &song.title,
            &song.artist,
            &song.album,
            song.art_url.as_deref(),
            self.playback.duration,
            self.playback.elapsed,
            self.playback.is_playing,
        );
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TogglePlay => {
                self.playback.toggle();
                if self.online.is_active() {
                    self.online.set_paused(!self.playback.is_playing);
                }
            }
            Message::Tick(now) => {
                self.now = now;
                let mut scrobble = Task::none();
                if self.playback.is_playing
                    && self.audio_settings.repeat != Repeat::One
                    && let Some((track, prefetch)) = self.online.crossfade(
                        self.audio_settings.crossfade,
                        self.audio_settings.speed,
                        self.audio_settings.repeat == Repeat::All,
                    )
                {
                    let started = self.song_started(track);
                    return Task::batch([prefetch, started]);
                }
                if let Some(started) = self.went_on() {
                    return started;
                }
                if self.online.is_active() {
                    match self.online.poll() {
                        Poll::Position(seconds) => {
                            self.playback.elapsed = seconds;
                            if self.audio_format.is_none() {
                                self.audio_format = self.online.format_label();
                            }
                            if self.playback.duration == 0 {
                                self.playback.duration = self.online.duration().unwrap_or(0);
                            }
                            if self.plugins.lastfm {
                                scrobble = self.lastfm.progress(seconds);
                            }
                            self.skip_offtopic();
                        }
                        Poll::Finished => return self.song_finished(),
                        Poll::Waiting => {}
                    }
                } else {
                    self.playback.tick();
                }
                self.follow_lyrics();
                return Task::batch([scrobble, self.follow_host()]);
            }
            Message::Frame(now) => {
                self.now = now;
                // Keep the lyrics on the beat between the one-second ticks.
                if self.online.is_active() && self.playback.is_playing {
                    if let Some(started) = self.went_on() {
                        return started;
                    }
                    self.online.step_fade();
                    self.follow_lyrics();
                }
            }
            Message::ToggleSidebar => self.show_sidebar = !self.show_sidebar,
            Message::TogglePanel(panel) => {
                self.panel = (self.panel != Some(panel)).then_some(panel);
            }
            Message::ToggleSection(section) => self.sidebar.toggle(section),
            Message::Select(item) => return self.select(item),
            Message::ToggleLike => self.liked = !self.liked,
            Message::Next => {
                if self.online.is_active() {
                    return self.play_next_online();
                }
                self.next_track();
            }
            Message::WindowResized(width) => self.window_width = width,
            Message::SearchChanged(query) => return self.online.query_changed(query),
            Message::SuggestionsLoaded(request, result) => {
                self.online.suggestions_loaded(request, result);
            }
            Message::PickSuggestion(term) => {
                self.online.query = term;
                self.screen = Screen::Search;
                return self.online.search();
            }
            Message::Search => {
                self.screen = Screen::Search;
                return self.online.search();
            }
            Message::SearchFinished(result) => return self.online.search_finished(result),
            Message::SetFilter(filter) => {
                self.screen = Screen::Search;
                self.online.set_filter(filter);
            }
            Message::ImageFetched(url, bytes) => {
                if self.online.image_fetched(url.clone(), bytes) {
                    self.show_cover(&url);
                }
                if self.offline.wants_image(&url)
                    && let Some(bytes) = self.online.image_bytes(&url)
                {
                    self.offline.save_image(&url, bytes);
                }
            }
            Message::OpenTile(tile) => {
                self.screen = Screen::Collection;
                return self.online.open(tile);
            }
            Message::CollectionLoaded(result) => return self.online.collection_loaded(result),
            Message::PlayQueue(tracks, index) => return self.online.play_queue(tracks, index),
            Message::AddToQueue(track) => return self.online.add_to_queue(track),
            Message::QueueJump(index) => return self.online.play(index),
            Message::QueuePlayNext(index) => return self.online.queue_play_next(index),
            Message::QueueRemove(index) => return self.online.queue_remove(index),
            Message::QueueClear => return self.online.queue_clear(),
            Message::OpenTogether => self.screen = Screen::Together,
            Message::Together(event) => return self.together_event(event),
            Message::Discord(event) => return self.discord_event(event),
            Message::ToggleMini => return self.toggle_mini(),
            Message::MiniEntered(id, size) => {
                self.mini = Some((id, size));
                return Task::batch([
                    window::resize(id, mini_player::SIZE),
                    window::set_level(id, window::Level::AlwaysOnTop),
                ]);
            }
            Message::PlayTop => {
                let (opens, task) = self.online.play_top();
                if opens {
                    self.screen = Screen::Collection;
                }
                return task;
            }
            Message::AudioReady(index, path) => return self.start_online(index, path),
            Message::Prefetched(id, path) => self.online.prefetched(id, path),
            Message::RadioLoaded(seed, songs) => {
                // A guest plays what the host plays; its own queue does not go on.
                if !self.together.is_guest() {
                    return self.online.radio_loaded(seed, songs);
                }
            }
            Message::Playlist(event) => return self.playlist_event(event),
            Message::ToggleAutoplay => {
                self.audio_settings.autoplay = !self.audio_settings.autoplay;
                self.audio_settings.save();
                return self.online.set_autoplay(self.audio_settings.autoplay);
            }
            Message::Back => {
                self.screen = match self.screen {
                    Screen::Collection => Screen::Search,
                    _ => Screen::Home,
                };
            }
            Message::OpenAccount => return self.select(NavItem::Account),
            Message::SignInWith(browser) => {
                let Some(client) = self.online.client() else {
                    return Task::none();
                };
                self.account = Account::SigningIn;
                return Task::perform(
                    library::sign_in_with_browser(client, browser),
                    Message::SignedIn,
                );
            }
            Message::SignInWindow => {
                self.account = Account::Waiting;
                return Task::perform(sign_in_window::sign_in(), Message::SignInWindowClosed);
            }
            Message::SignInWindowClosed(Err(error)) => self.account = Account::Failed(error),
            Message::SignInWindowClosed(Ok(cookie)) => {
                let Some(client) = self.online.client() else {
                    return Task::none();
                };
                // Loading the library takes a few seconds; say so as soon as the window is gone.
                self.account = Account::SigningIn;
                return Task::perform(
                    library::sign_in_with_cookie(client, cookie),
                    Message::SignedIn,
                );
            }
            Message::CookiePathChanged(path) => self.cookie_path = path,
            Message::ImportCookies => {
                let Some(client) = self.online.client() else {
                    return Task::none();
                };
                self.account = Account::SigningIn;
                let cookies = account::read_from_file(&self.cookie_path);
                return Task::perform(library::sign_in(client, cookies), Message::SignedIn);
            }
            Message::SignedIn(result) => return self.library_loaded(result),
            Message::Restored(Ok(data)) => return self.library_loaded(Ok(data)),
            Message::Restored(Err(library::RestoreError::NoSession)) => {
                self.account = Account::SignedOut;
                library::clear_cache();
            }
            Message::Restored(Err(library::RestoreError::Failed(_))) => {
                // Probably offline: keep showing the saved library if there is one.
                if !matches!(self.account, Account::SignedIn(_)) {
                    self.account = Account::SignedOut;
                }
            }
            Message::SignOut => {
                self.account = Account::SignedOut;
                library::clear_cache();
                if let Some(client) = self.online.client() {
                    return Task::perform(library::sign_out(client), |()| Message::SignedOut);
                }
            }
            Message::SignedOut => {}
            Message::DownloadCollection(collection) => {
                let task = self.offline.start(collection);
                return self.save_offline_covers(task);
            }
            Message::OfflineTrackDone(result) => {
                let task = self.offline.track_done(result);
                return self.save_offline_covers(task);
            }
            Message::RemoveDownload(id) => self.offline.remove(&id),
            Message::LyricsLoaded(id, lyrics) => {
                if self
                    .online
                    .playing_track()
                    .is_some_and(|track| track.id == id)
                {
                    self.lyrics = lyrics;
                    self.follow_lyrics();
                }
            }
            Message::MediaKey(key) => {
                let playing = self.playback.is_playing;
                return match key {
                    MediaKey::Toggle => self.handle(Message::TogglePlay),
                    MediaKey::Play if !playing => self.handle(Message::TogglePlay),
                    MediaKey::Pause if playing => self.handle(Message::TogglePlay),
                    MediaKey::Next => self.handle(Message::Next),
                    MediaKey::Previous => self.handle(Message::Previous),
                    MediaKey::Play | MediaKey::Pause => Task::none(),
                };
            }
            Message::ToggleImmersive => return self.set_immersive(!self.immersive),
            Message::ExitImmersive => return self.set_immersive(false),
            Message::LastFmConnect => return self.lastfm.connect(),
            Message::LastFmConnected(result) => return self.lastfm.connected(result),
            Message::LastFmDisconnect => self.lastfm.disconnect(),
            Message::LastFmAnswered(result) => self.lastfm.answered(result),
            Message::Scrobbled(result) => return self.lastfm.scrobbled(result),
            Message::Previous => return self.previous(),
            Message::Seeking(seconds) => self.seeking = Some(seconds),
            Message::SeekDone => {
                if let Some(seconds) = self.seeking.take() {
                    self.seek(seconds);
                }
            }
            Message::VolumeChanged(volume) => {
                self.audio_settings.volume = volume;
                self.audio_settings.muted = false;
                self.online.set_volume(volume);
            }
            Message::VolumeDone => self.audio_settings.save(),
            Message::ToggleMute => {
                self.audio_settings.muted = !self.audio_settings.muted;
                self.online
                    .set_volume(self.audio_settings.effective_volume());
                self.audio_settings.save();
            }
            Message::ToggleShuffle => {
                self.audio_settings.shuffle = !self.audio_settings.shuffle;
                self.audio_settings.save();
                return self.online.set_shuffle(self.audio_settings.shuffle);
            }
            Message::CycleRepeat => {
                self.audio_settings.repeat = self.audio_settings.repeat.next();
                self.audio_settings.save();
                return self.online.set_repeat(self.audio_settings.repeat);
            }
            Message::OpenSettings(tab) => self.screen = Screen::Settings(tab),
            Message::PickTheme(name) => {
                self.appearance.theme = name;
                self.appearance.save();
                self.apply_appearance();
            }
            Message::PickAccent(accent) => {
                self.appearance.accent = accent;
                self.appearance.save();
                self.apply_appearance();
            }
            Message::SetPlugin(plugin, on) => {
                self.plugins.set(plugin, on);
                self.plugins.save();
            }
            Message::SegmentsLoaded(id, Ok(segments)) => {
                self.skip_segments = Some((id, segments));
                self.skip_offtopic();
            }
            // Without SponsorBlock's answer the song simply plays whole.
            Message::SegmentsLoaded(_, Err(_)) | Message::NotificationShown => {}
            Message::ReloadThemes => {
                (self.themes, self.theme_errors) = themes::load_all();
                self.apply_appearance();
            }
            Message::AudioChanged(change) => {
                if self.audio_settings.change(change) {
                    self.apply_audio();
                } else if let audio_settings::AudioChange::Speed(_) = change {
                    // Speed is cheap to change, so it follows the slider while dragging.
                    self.apply_audio();
                }
            }
            Message::AudioChangeDone => self.apply_audio(),
            Message::OpenOffline(id) => {
                if let Some(collection) = self.offline.get(&id).cloned() {
                    self.screen = Screen::Collection;
                    return self.online.show_collection(collection);
                }
            }
        }
        Task::none()
    }

    /// Shows a YouTube track in the player once its audio has started.
    pub(crate) fn start_online(
        &mut self,
        index: usize,
        path: Result<std::path::PathBuf, String>,
    ) -> Task<Message> {
        let (started, prefetch) = self.online.audio_ready(index, path);
        let Some(track) = started else {
            return prefetch;
        };
        let started = self.song_started(track);
        Task::batch([prefetch, started])
    }

    /// Shows the song mpv went on into with no gap, once it does.
    fn went_on(&mut self) -> Option<Task<Message>> {
        let (track, prefetch) = self.online.went_on()?;
        let started = self.song_started(track);
        Some(Task::batch([prefetch, started]))
    }

    /// Applies the audio screen's settings to mpv and saves them.
    fn apply_audio(&mut self) {
        self.audio_error = self.online.apply_audio(&self.audio_settings).err();
        self.audio_settings.save();
    }

    /// Shows a song that just started (by itself or by crossfading into it): the player bar,
    /// the background and the lyrics, and tells Last.fm.
    fn song_started(&mut self, track: SearchTrack) -> Task<Message> {
        let cover = track
            .cover_url
            .as_deref()
            .and_then(|url| self.online.image(url));
        self.now_playing = Song::from_search(&track, cover);
        self.audio_format = None;
        let looks = cover_looks(
            track
                .cover_url
                .as_deref()
                .and_then(|url| self.online.image_bytes(url)),
        );
        self.set_cover_looks(looks);
        let duration = track.duration;
        self.playback = Playback {
            elapsed: 0,
            duration,
            is_playing: true,
        };
        self.lyrics = Lyrics::Loading;
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
        let id = track.id.clone();
        let lyrics = Task::perform(
            lyrics_source::fetch(track, self.online.client()),
            move |lyrics| Message::LyricsLoaded(id.clone(), lyrics),
        );
        Task::batch([lyrics, now_playing, notice, segments])
    }

    /// With SponsorBlock on, jumps over the part of the video that is playing if the community
    /// marked it as not music.
    fn skip_offtopic(&mut self) {
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

    /// Enters or leaves the immersive view, switching the window to full screen and back.
    pub(crate) fn set_immersive(&mut self, immersive: bool) -> Task<Message> {
        if self.immersive == immersive {
            return Task::none();
        }
        self.immersive = immersive;
        let mode = if immersive {
            window::Mode::Fullscreen
        } else {
            window::Mode::Windowed
        };
        window::latest().and_then(move |id| window::set_mode(id, mode))
    }

    /// Shrinks the window into the mini player (above other windows), or gives it back its size.
    fn toggle_mini(&mut self) -> Task<Message> {
        if let Some((id, size)) = self.mini.take() {
            return Task::batch([
                window::set_level(id, window::Level::Normal),
                window::resize(id, size),
            ]);
        }
        let leave_immersive = self.set_immersive(false);
        let shrink = window::latest()
            .and_then(|id| window::size(id).map(move |size| Message::MiniEntered(id, size)));
        Task::batch([leave_immersive, shrink])
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

    /// Once a download finishes, saves its covers so they show without a connection. Covers that
    /// are not in memory yet are downloaded again and saved when they arrive.
    pub(crate) fn save_offline_covers(&mut self, task: Task<Message>) -> Task<Message> {
        let Some(collection) = self.offline.take_finished() else {
            return task;
        };
        let mut missing = Vec::new();
        for url in offline::image_urls(&collection) {
            match self.online.image_bytes(url) {
                Some(bytes) => self.offline.save_image(url, bytes),
                None => missing.push(url.to_string()),
            }
        }
        Task::batch([task, self.online.refetch_images(missing)])
    }

    /// Highlights a sidebar entry and shows its screen.
    pub(crate) fn select(&mut self, item: NavItem) -> Task<Message> {
        if item == NavItem::NewPlaylist {
            return self.playlist_event(crate::playlists::Event::OpenNew);
        }
        self.sidebar.selected = item;
        self.screen = match item {
            NavItem::History => Screen::Library(Shelf::History),
            NavItem::Songs => Screen::Library(Shelf::Songs),
            NavItem::LikedMusic => Screen::Library(Shelf::Liked),
            NavItem::Albums => Screen::Library(Shelf::Albums),
            NavItem::Artists => Screen::Library(Shelf::Artists),
            NavItem::Downloads => Screen::Library(Shelf::Downloads),
            NavItem::Account => Screen::Account,
            NavItem::Playlist(index) => {
                if let Account::SignedIn(data) = &self.account
                    && let Some(playlist) = data.playlists.get(index).cloned()
                {
                    self.screen = Screen::Collection;
                    return self.online.open(playlist);
                }
                Screen::Home
            }
            _ => Screen::Home,
        };
        Task::none()
    }

    /// Shows the library after signing in or restoring a session, saves it for the next start
    /// and downloads its images. A failed sign-in shows the error.
    pub(crate) fn library_loaded(&mut self, result: Result<LibraryData, String>) -> Task<Message> {
        match result {
            Ok(data) => {
                library::save_cache(&data);
                let images = self.online.fetch_images(data.image_urls());
                self.account = Account::SignedIn(Box::new(data));
                images
            }
            Err(error) => {
                self.account = Account::Failed(error);
                Task::none()
            }
        }
    }

    /// Puts a freshly downloaded cover on the player and behind the home greeting.
    pub(crate) fn show_cover(&mut self, url: &str) {
        self.now_playing.cover = self.online.image(url);
        let looks = cover_looks(self.online.image_bytes(url));
        self.set_cover_looks(looks);
    }

    /// Puts a cover's blurred background behind the home greeting and its color on the accent.
    fn set_cover_looks(&mut self, (backdrop, color): (Option<image::Handle>, Option<Color>)) {
        self.backdrop = backdrop;
        self.cover_color = color;
        self.apply_appearance();
    }

    /// Draws with the chosen theme and accent from the next frame on.
    pub(crate) fn apply_appearance(&mut self) {
        theme::set_palette(self.appearance.palette(&self.themes, self.cover_color));
    }

    /// A song ended: plays it again with repeat one, else the next one.
    fn song_finished(&mut self) -> Task<Message> {
        if self.audio_settings.repeat == Repeat::One
            && let Some(index) = self.online.playing_index()
        {
            return self.online.play(index);
        }
        self.play_next_online()
    }

    /// The previous button: back to the start of the song, or to the previous song when the
    /// current one has just begun.
    fn previous(&mut self) -> Task<Message> {
        const RESTART_AFTER: u32 = 3;
        let wrap = self.audio_settings.repeat == Repeat::All;
        if self.position_ms() / 1000 < RESTART_AFTER {
            if self.online.is_active() {
                if let Some(index) = self.online.previous_index(wrap) {
                    return self.online.play(index);
                }
            } else {
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

    /// Moves to the next sample track: new cover, new background and the clock back at zero.
    pub(crate) fn next_track(&mut self) {
        self.show_sample(clock::next_index(self.current, self.tracks.len()));
    }

    /// Shows sample track `index` from the start.
    fn show_sample(&mut self, index: usize) {
        self.current = index;
        let track = &self.tracks[self.current];
        self.playback.restart(track.duration);
        let looks = load_cover(track);
        self.lyrics = load_lyrics(track);
        self.now_playing = Song::from_mock(track);
        self.set_cover_looks(looks);
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
}
