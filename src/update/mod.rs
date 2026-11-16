//! How the app reacts to messages: `App::update` and the `match` over every message. The
//! helpers it calls are in `playback` (songs starting and ending), `display` (full screen, mini
//! player, looks) and `places` (sidebar entries and the library).

mod display;
pub(crate) mod lyric_scroll;
mod places;
mod playback;

use iced::{Task, window};

use crate::library::Account;
use crate::media_keys::MediaKey;
use crate::online::Poll;
use crate::queue::Repeat;
use crate::sidebar::NavItem;
use crate::volume;

use crate::{
    App, Message, Screen, account, audio_settings, discord, i18n, library, mini_player, session,
    sign_in_window, themes,
};

impl App {
    /// Applies one message to the state and returns the work it starts (downloads, requests),
    /// then tells the desktop's media controls what is playing.
    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);
        self.show_loading_song();
        self.publish_media();
        // The covers of the song in the player and the next ones, wherever they came from.
        let covers = self.online.fetch_queue_covers();
        Task::batch([task, covers])
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

    pub(crate) fn handle(&mut self, message: Message) -> Task<Message> {
        match message {
            // Nothing to play or pause before a song is picked.
            Message::TogglePlay if !self.has_song() => {}
            Message::TogglePlay => {
                // A song that never started (or nothing loaded) starts again instead.
                if let Some(restart) = self.online.restart_if_stuck(self.now) {
                    return restart;
                }
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
                } else if self.samples {
                    self.playback.tick();
                }
                self.follow_lyrics();
                return Task::batch([scrobble, self.follow_host()]);
            }
            Message::Frame(now) => {
                self.now = now;
                self.step_lyrics_browse();
                // Keep the lyrics on the beat between the one-second ticks.
                if self.online.is_active() && self.playback.is_playing {
                    if let Some(started) = self.went_on() {
                        return started;
                    }
                    self.online.step_fade();
                    self.follow_lyrics();
                    self.video_step();
                }
                if self.immersive && self.plugins.visualizer {
                    let playing = self.playback.is_playing && self.online.is_active();
                    let position = self.online.position_ms().filter(|_| playing);
                    self.visualizer
                        .step(position.map(|millis| f64::from(millis) / 1000.0));
                }
            }
            Message::ToggleSidebar => self.show_sidebar = !self.show_sidebar,
            Message::TogglePanel(panel) => {
                self.panel = (self.panel != Some(panel)).then_some(panel);
            }
            Message::ToggleSection(section) => self.sidebar.toggle(section),
            Message::Select(item) => return self.select(item),
            Message::Rating(event) => return self.rating_event(event),
            Message::Video(event) => return self.video_event(event),
            Message::Renewal(event) => return self.renewal_event(event),
            Message::ToggleVideo => return self.toggle_video(),
            Message::NowPlaying(event) => return self.now_playing_event(event),
            Message::Visualizer(event) => self.visualizer.event(event),
            Message::Next => {
                if self.online.is_active() {
                    return self.play_next_online();
                }
                if self.samples {
                    self.next_track();
                }
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
                    self.offline.save_image(&url, &bytes);
                }
            }
            Message::OpenTile(tile) => {
                self.go_to(Screen::Collection);
                return self.online.open(tile);
            }
            Message::CollectionLoaded(Ok(opened)) => {
                let shown = self.online.collection_loaded(Ok(opened.collection));
                let more = opened
                    .more
                    .map_or_else(Task::none, |more| self.online.load_more(more));
                return Task::batch([shown, more]);
            }
            Message::CollectionRefreshed(Ok(fresh)) => {
                let refreshed = self.online.collection_refreshed(fresh);
                return Task::batch([refreshed, self.fetch_covers_in_sight()]);
            }
            // The copy that is shown stays.
            Message::CollectionRefreshed(Err(_)) => {}
            Message::CollectionLoaded(Err(error)) => {
                return self.online.collection_loaded(Err(error));
            }
            Message::MoreTracks(id, Ok((tracks, more))) => {
                // Pages of a playlist that was left are dropped, and stop coming.
                if self.online.more_tracks_loaded(&id, tracks)
                    && let Some(more) = more
                {
                    let next = self.online.load_more(more);
                    return Task::batch([next, self.fetch_covers_in_sight()]);
                }
                return self.fetch_covers_in_sight();
            }
            // The songs loaded so far stay; the rest of the playlist is missing.
            Message::MoreTracks(_, Err(_)) => {}
            Message::ShelfScrolled(scroll) => {
                self.shelf_scroll = scroll;
                return self.fetch_shelf_images();
            }
            Message::CollectionScrolled(scroll) => {
                self.collection_scroll = scroll;
                return self.fetch_covers_in_sight();
            }
            Message::PlayQueue(tracks, index) => return self.online.play_queue(tracks, index),
            Message::PlayAll(tracks, shuffle) => {
                if shuffle && !self.audio_settings.shuffle {
                    self.audio_settings.shuffle = true;
                    self.audio_settings.save();
                }
                return self.online.play_all(tracks, shuffle);
            }
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
                    self.go_to(Screen::Collection);
                }
                return task;
            }
            Message::AudioReady(index, path) => return self.start_online(index, path),
            Message::Prefetched(id, path) => self.online.prefetched(id, path),
            Message::WarmedUp(id) => self.online.warmed_up(&id),
            Message::RadioLoaded(seed, songs) => {
                // A guest plays what the host plays; its own queue does not go on.
                if !self.together.is_guest() {
                    return self.online.radio_loaded(seed, songs);
                }
            }
            Message::Playlist(event) => return self.playlist_event(event),
            Message::Explore(event) => return self.explore_event(event),
            Message::ToggleAutoplay => {
                self.audio_settings.autoplay = !self.audio_settings.autoplay;
                self.audio_settings.save();
                return self.online.set_autoplay(self.audio_settings.autoplay);
            }
            Message::Back => match self.history.pop() {
                Some((screen, collection)) => {
                    self.screen = screen;
                    if let Some(collection) = collection {
                        return self.online.show_collection(collection);
                    }
                }
                None => self.screen = Screen::Home,
            },
            Message::OpenAccount => return self.select(NavItem::Account),
            Message::SignInWith(browser) => {
                let Some(client) = self.online.client() else {
                    return Task::none();
                };
                self.account = Account::SigningIn;
                self.signing_in_from_window(false);
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
            #[cfg(windows)]
            Message::MainWindowFound(handle) => {
                self.media =
                    crate::media_keys::MediaKeys::start(Some(handle as *mut std::ffi::c_void));
            }
            Message::SignInWindowClosed(Ok(cookie)) => {
                let Some(client) = self.online.client() else {
                    return Task::none();
                };
                // Loading the library takes a few seconds; say so as soon as the window is gone.
                self.account = Account::SigningIn;
                self.signing_in_from_window(true);
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
                self.signing_in_from_window(false);
                let cookies = account::read_from_file(&self.cookie_path);
                return Task::perform(library::sign_in(client, cookies), Message::SignedIn);
            }
            Message::SignedIn(result) => {
                if result.is_err() {
                    return self.library_loaded(result);
                }
                let from_window = self.signed_in_from_window();
                self.session_log.signed_in(session::now(), from_window);
                self.session_log.save();
                self.session_expired = false;
                let renew = self.start_renewal();
                return Task::batch([self.library_loaded(result), renew]);
            }
            Message::Restored(Ok(data)) => {
                self.session_log.working(session::now());
                self.session_log.save();
                self.session_expired = false;
                let renew = self.start_renewal();
                return Task::batch([self.library_loaded(Ok(data)), renew]);
            }
            Message::Restored(Err(library::RestoreError::Expired)) => self.session_rejected(),
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
                self.stop_renewal();
                self.session_log.signed_out();
                self.session_log.save();
                self.session_expired = false;
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
                    self.romanize_lyrics();
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
            Message::LyricsScrolled(lines) => self.lyrics_scrolled(lines),
            Message::LyricLine(index) => self.lyric_clicked(index),
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
            Message::VolumeScrolled(delta) => {
                let notches = volume::notches(&mut self.volume_scroll, delta);
                if notches != 0 {
                    let from = self.audio_settings.effective_volume();
                    let volume = volume::stepped(from, notches, self.audio_settings.volume_step);
                    self.audio_settings.volume = volume;
                    self.audio_settings.muted = false;
                    self.online.set_volume(volume);
                    self.audio_settings.save();
                }
            }
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
            Message::OpenSettings(tab) => {
                if tab == crate::settings_view::Tab::Audio {
                    // Headphones may have been plugged in since the screen was last open.
                    self.audio_devices = self.online.audio_devices();
                }
                self.screen = Screen::Settings(tab);
            }
            Message::PickTheme(name) => {
                self.appearance.theme = name;
                self.appearance.save();
                self.apply_appearance();
            }
            Message::PickLanguage(language) => {
                i18n::apply(language.as_deref());
                self.appearance.language = language;
                self.appearance.save();
                self.romanize_lyrics();
            }
            Message::PickAccent(accent) => {
                self.appearance.accent = accent;
                self.appearance.save();
                self.apply_appearance();
            }
            Message::SetPlugin(plugin, on) => {
                self.plugins.set(plugin, on);
                self.plugins.save();
                self.romanize_lyrics();
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
            Message::AudioOutput(device) => {
                self.audio_settings.output = device;
                self.audio_devices = self.online.audio_devices();
                self.apply_audio();
            }
            Message::OpenOffline(id) => {
                if let Some(collection) = self.offline.get(&id).cloned() {
                    self.go_to(Screen::Collection);
                    return self.online.show_collection(collection);
                }
            }
        }
        Task::none()
    }

    /// Applies the audio screen's settings to mpv and saves them.
    fn apply_audio(&mut self) {
        self.audio_error = self.online.apply_audio(&self.audio_settings).err();
        self.audio_settings.save();
    }

    /// YouTube no longer accepts the saved session: remember how long it lasted and ask to
    /// sign in again, keeping the saved library on screen meanwhile.
    pub(crate) fn session_rejected(&mut self) {
        self.stop_renewal();
        if !self.session_expired {
            self.session_expired = true;
            self.session_log.expired(session::now());
            self.session_log.save();
        }
        if !matches!(self.account, Account::SignedIn(_)) {
            // Nothing saved to show: offer the sign-in buttons.
            self.account = Account::SignedOut;
        }
    }
}
