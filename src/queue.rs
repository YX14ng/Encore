//! The play queue: the songs in list order, the order they play in (shuffled or not) and which
//! one is playing.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::youtube::SearchTrack;

/// What happens at the end of a song.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Repeat {
    /// Stop after the last song.
    #[default]
    Off,
    /// Start the queue over after the last song.
    All,
    /// Play the same song again.
    One,
}

impl Repeat {
    /// The mode after pressing the repeat button: off, all, one, off…
    pub fn next(self) -> Self {
        match self {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        }
    }
}

/// The songs to play and where playback is.
#[derive(Debug, Default)]
pub struct Queue {
    /// Every song the queue has held since it was last replaced. Songs are only appended, so
    /// an index stays valid while its song downloads.
    tracks: Vec<SearchTrack>,
    /// Indexes into `tracks` in the order they play; a song taken out is missing here.
    order: Vec<usize>,
    /// The position in `order` of the song that is playing.
    cursor: Option<usize>,
    /// The songs autoplay added (similar songs after the ones picked).
    suggested: HashSet<usize>,
    /// The album or playlist the songs came from, when the queue is one: songs of it that load
    /// later join the queue.
    source: Option<String>,
}

impl Queue {
    /// Replaces the songs. With `shuffle`, song `first` plays first and the rest in random order.
    pub fn replace(
        &mut self,
        tracks: Vec<SearchTrack>,
        first: usize,
        shuffle: bool,
        rng: &mut fastrand::Rng,
    ) {
        self.order = play_order(tracks.len(), first, shuffle, rng);
        self.tracks = tracks;
        self.cursor = None;
        self.suggested.clear();
        self.source = None;
    }

    /// Notes that the songs are those of collection `id`.
    pub fn set_source(&mut self, id: &str) {
        self.source = Some(id.to_string());
    }

    /// Whether the songs are those of collection `id`.
    pub fn is_from(&self, id: &str) -> bool {
        self.source.as_deref() == Some(id)
    }

    /// Adds songs of the queue's collection that loaded after it started. With `shuffle`
    /// each lands at a random place among the songs still to play (which keeps every order
    /// of them equally likely); else they go at the end.
    pub fn extend(&mut self, tracks: Vec<SearchTrack>, shuffle: bool, rng: &mut fastrand::Rng) {
        for track in tracks {
            let index = self.tracks.len();
            self.tracks.push(track);
            let first_free = self.cursor.map_or(0, |cursor| cursor + 1);
            let at = if shuffle {
                rng.usize(first_free..=self.order.len())
            } else {
                self.order.len()
            };
            self.order.insert(at, index);
        }
    }

    /// Song `index` (in list order).
    pub fn get(&self, index: usize) -> Option<&SearchTrack> {
        self.tracks.get(index)
    }

    /// Marks song `index` (in list order) as the one playing. A song taken out of the queue
    /// while it downloaded goes back in, after the one that was playing.
    pub fn set_playing(&mut self, index: usize) {
        if index >= self.tracks.len() {
            self.cursor = None;
            return;
        }
        let at = match self.order.iter().position(|&song| song == index) {
            Some(at) => at,
            None => {
                let at = self.cursor.map_or(self.order.len(), |cursor| cursor + 1);
                self.order.insert(at, index);
                at
            }
        };
        self.cursor = Some(at);
    }

    /// The index (in list order) of the song that is playing.
    pub fn playing(&self) -> Option<usize> {
        self.order.get(self.cursor?).copied()
    }

    /// The song that is playing.
    pub fn playing_track(&self) -> Option<&SearchTrack> {
        self.tracks.get(self.playing()?)
    }

    /// The song after the one playing; with `wrap`, the first one after the last.
    pub fn next(&self, wrap: bool) -> Option<usize> {
        let cursor = self.cursor?;
        match self.order.get(cursor + 1) {
            Some(&song) => Some(song),
            None if wrap => self.order.first().copied(),
            None => None,
        }
    }

    /// The song before the one playing; with `wrap`, the last one before the first.
    pub fn previous(&self, wrap: bool) -> Option<usize> {
        let cursor = self.cursor?;
        match cursor.checked_sub(1) {
            Some(before) => self.order.get(before).copied(),
            None if wrap => self.order.last().copied(),
            None => None,
        }
    }

    /// Shuffles the songs after the one playing, or puts them back in list order. Songs taken
    /// out of the queue stay out.
    pub fn set_shuffle(&mut self, shuffle: bool, rng: &mut fastrand::Rng) {
        let playing = self.playing();
        let mut others: Vec<usize> = self
            .order
            .iter()
            .copied()
            .filter(|&song| Some(song) != playing)
            .collect();
        if shuffle {
            rng.shuffle(&mut others);
            self.order = playing.into_iter().chain(others).collect();
        } else {
            others.extend(playing);
            others.sort_unstable();
            self.order = others;
        }
        if let Some(index) = playing {
            self.set_playing(index);
        }
    }

    /// The songs that play after the current one, in play order.
    pub fn upcoming(&self) -> &[usize] {
        match self.cursor {
            Some(cursor) => &self.order[cursor + 1..],
            None => &[],
        }
    }

    /// Adds `track` after the songs already picked, before any autoplay added. Returns its
    /// index.
    pub fn add(&mut self, track: SearchTrack) -> usize {
        self.tracks.push(track);
        let index = self.tracks.len() - 1;
        let start = self.cursor.map_or(0, |cursor| cursor + 1);
        let at = self.order[start.min(self.order.len())..]
            .iter()
            .position(|song| self.suggested.contains(song))
            .map_or(self.order.len(), |at| start + at);
        self.order.insert(at, index);
        index
    }

    /// Adds similar songs from autoplay at the end of the queue, leaving out any the queue
    /// already holds. Returns how many were added.
    pub fn add_suggested(&mut self, tracks: Vec<SearchTrack>) -> usize {
        let mut known: HashSet<String> = self.tracks.iter().map(|track| track.id.clone()).collect();
        let mut added = 0;
        for track in tracks {
            if known.insert(track.id.clone()) {
                self.tracks.push(track);
                let index = self.tracks.len() - 1;
                self.order.push(index);
                self.suggested.insert(index);
                added += 1;
            }
        }
        added
    }

    /// Whether autoplay added song `index`.
    pub fn is_suggested(&self, index: usize) -> bool {
        self.suggested.contains(&index)
    }

    /// Moves song `index` (already in the queue) to play right after the current one.
    pub fn play_next(&mut self, index: usize) {
        let Some(cursor) = self.cursor else {
            return;
        };
        if self.playing() == Some(index) {
            return;
        }
        let Some(from) = self.order.iter().position(|&song| song == index) else {
            return;
        };
        self.order.remove(from);
        // Moved up by hand, it is no longer just a suggestion.
        self.suggested.remove(&index);
        let cursor = if from < cursor { cursor - 1 } else { cursor };
        self.order.insert(cursor + 1, index);
        self.cursor = Some(cursor);
    }

    /// Takes song `index` out of the queue. The song that is playing stays.
    pub fn remove(&mut self, index: usize) {
        if self.playing() == Some(index) {
            return;
        }
        let Some(from) = self.order.iter().position(|&song| song == index) else {
            return;
        };
        self.order.remove(from);
        if let Some(cursor) = self.cursor
            && from < cursor
        {
            self.cursor = Some(cursor - 1);
        }
    }

    /// Takes every song after the current one out of the queue.
    pub fn clear_upcoming(&mut self) {
        if let Some(cursor) = self.cursor {
            self.order.truncate(cursor + 1);
        }
    }
}

/// The song a list of `len` songs starts with when played whole: the first, or with
/// `shuffle` any of them, each as likely.
pub fn first_song(len: usize, shuffle: bool, rng: &mut fastrand::Rng) -> usize {
    if shuffle && len > 0 {
        rng.usize(..len)
    } else {
        0
    }
}

/// The play order of `len` songs: in list order, or `first` and then the rest shuffled.
fn play_order(len: usize, first: usize, shuffle: bool, rng: &mut fastrand::Rng) -> Vec<usize> {
    if !shuffle || first >= len {
        return (0..len).collect();
    }
    let mut rest: Vec<usize> = (0..len).filter(|&song| song != first).collect();
    rng.shuffle(&mut rest);
    std::iter::once(first).chain(rest).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn songs(count: usize) -> Vec<SearchTrack> {
        (0..count)
            .map(|index| SearchTrack {
                id: index.to_string(),
                title: format!("Song {index}"),
                artist: String::new(),
                album: String::new(),
                duration: 200,
                cover_url: None,
                entry: None,
            })
            .collect()
    }

    fn queue(count: usize, first: usize, shuffle: bool) -> Queue {
        let mut queue = Queue::default();
        queue.replace(
            songs(count),
            first,
            shuffle,
            &mut fastrand::Rng::with_seed(7),
        );
        queue.set_playing(first);
        queue
    }

    #[test]
    fn walks_the_list_in_order() {
        let queue = queue(3, 1, false);
        assert_eq!(queue.playing(), Some(1));
        assert_eq!(queue.next(false), Some(2));
        assert_eq!(queue.previous(false), Some(0));
    }

    #[test]
    fn wraps_only_when_repeating() {
        let last = queue(3, 2, false);
        assert_eq!(last.next(false), None);
        assert_eq!(last.next(true), Some(0));
        let first = queue(3, 0, false);
        assert_eq!(first.previous(false), None);
        assert_eq!(first.previous(true), Some(2));
    }

    #[test]
    fn shuffling_plays_the_picked_song_first_and_every_song_once() {
        let mut queue = queue(20, 5, true);
        assert_eq!(queue.playing(), Some(5));
        let mut heard = vec![5];
        while let Some(next) = queue.next(false) {
            heard.push(next);
            queue.set_playing(next);
        }
        let in_order: Vec<usize> = (0..20).collect();
        assert_ne!(heard[1..], in_order[1..], "not shuffled");
        heard.sort_unstable();
        assert_eq!(heard, in_order);
    }

    #[test]
    fn turning_shuffle_off_returns_to_list_order() {
        let mut rng = fastrand::Rng::with_seed(3);
        let mut queue = queue(10, 4, false);
        queue.set_shuffle(true, &mut rng);
        assert_eq!(queue.playing(), Some(4));
        queue.set_shuffle(false, &mut rng);
        assert_eq!(queue.playing(), Some(4));
        assert_eq!(queue.next(false), Some(5));
    }

    #[test]
    fn added_songs_play_last_and_play_next_moves_them_up() {
        let mut queue = queue(3, 0, false);
        let added = queue.add(songs(1).remove(0));
        assert_eq!(queue.upcoming(), [1, 2, added]);
        queue.play_next(added);
        assert_eq!(queue.upcoming(), [added, 1, 2]);
        assert_eq!(queue.next(false), Some(added));
    }

    #[test]
    fn play_next_can_bring_back_an_earlier_song() {
        let mut queue = queue(4, 2, false);
        queue.play_next(0);
        assert_eq!(queue.playing(), Some(2));
        assert_eq!(queue.upcoming(), [0, 3]);
        assert_eq!(queue.previous(false), Some(1));
    }

    #[test]
    fn removed_songs_are_skipped_and_stay_out_when_shuffling() {
        let mut rng = fastrand::Rng::with_seed(1);
        let mut queue = queue(5, 1, false);
        queue.remove(0);
        queue.remove(3);
        queue.remove(1);
        assert_eq!(queue.playing(), Some(1), "the playing song stays");
        assert_eq!(queue.previous(false), None);
        assert_eq!(queue.upcoming(), [2, 4]);
        queue.set_shuffle(true, &mut rng);
        queue.set_shuffle(false, &mut rng);
        assert_eq!(queue.upcoming(), [2, 4]);
    }

    #[test]
    fn a_song_taken_out_while_downloading_comes_back_when_it_starts() {
        let mut queue = queue(4, 0, false);
        queue.remove(2);
        queue.set_playing(2);
        assert_eq!(queue.playing(), Some(2));
        assert_eq!(queue.previous(false), Some(0));
        assert_eq!(queue.upcoming(), [1, 3]);
    }

    #[test]
    fn clearing_keeps_the_playing_song_and_what_came_before() {
        let mut queue = queue(5, 2, false);
        queue.clear_upcoming();
        assert!(queue.upcoming().is_empty());
        assert_eq!(queue.next(false), None);
        assert_eq!(queue.previous(false), Some(1));
    }

    #[test]
    fn autoplay_adds_new_songs_after_the_picked_ones() {
        let mut queue = queue(2, 0, false);
        queue.set_playing(0);
        let mut similar = songs(4);
        similar[0].id = "x".to_string();
        // Song "1" is already in the queue.
        assert_eq!(queue.add_suggested(similar), 3);
        assert_eq!(queue.upcoming(), &[1, 2, 3, 4]);
        assert!(queue.is_suggested(2));
        assert!(!queue.is_suggested(1));
        let picked = queue.add(songs(1).remove(0));
        assert_eq!(
            queue.upcoming(),
            &[1, picked, 2, 3, 4],
            "picked songs go first"
        );
        queue.play_next(2);
        assert!(!queue.is_suggested(2), "moved up by hand");
        queue.replace(songs(1), 0, false, &mut fastrand::Rng::with_seed(1));
        assert!(!queue.is_suggested(2));
    }

    #[test]
    fn repeat_cycles_through_its_modes() {
        assert_eq!(Repeat::Off.next(), Repeat::All);
        assert_eq!(Repeat::All.next(), Repeat::One);
        assert_eq!(Repeat::One.next(), Repeat::Off);
    }

    #[test]
    fn songs_that_load_later_join_the_shuffle_anywhere_ahead() {
        let mut rng = fastrand::Rng::with_seed(7);
        // How often each late song lands among the next ten, over many queues.
        let mut early = 0;
        for _ in 0..200 {
            let mut queue = Queue::default();
            queue.replace(songs(100), 0, true, &mut rng);
            queue.set_playing(0);
            queue.extend(songs(100), true, &mut rng);
            assert_eq!(queue.playing(), Some(0), "the song playing stays");
            let ahead = queue.upcoming();
            assert_eq!(ahead.len(), 199);
            early += ahead[..10].iter().filter(|&&song| song >= 100).count();
        }
        // Half the songs ahead are late ones, so about 5 of each 10 should be.
        let share = early as f32 / 2000.0;
        assert!(
            (0.4..0.6).contains(&share),
            "late songs are not mixed in: {share}"
        );
    }

    #[test]
    fn songs_that_load_later_go_at_the_end_in_order() {
        let mut rng = fastrand::Rng::with_seed(1);
        let mut queue = Queue::default();
        queue.replace(songs(3), 0, false, &mut rng);
        queue.set_playing(0);
        queue.extend(songs(2), false, &mut rng);
        assert_eq!(queue.upcoming(), &[1, 2, 3, 4]);
    }

    #[test]
    fn shuffled_lists_start_anywhere() {
        let mut rng = fastrand::Rng::with_seed(5);
        let mut starts = [0usize; 10];
        for _ in 0..5000 {
            starts[first_song(10, true, &mut rng)] += 1;
        }
        // Each of the ten songs starts about 500 times in 5000.
        assert!(
            starts.iter().all(|&n| (400..600).contains(&n)),
            "{starts:?}"
        );
        assert_eq!(first_song(10, false, &mut rng), 0);
        assert_eq!(first_song(0, true, &mut rng), 0);
    }
}
