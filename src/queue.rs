//! The play queue: the songs in list order, the order they play in (shuffled or not) and which
//! one is playing.

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
    tracks: Vec<SearchTrack>,
    /// Indexes into `tracks` in the order they play.
    order: Vec<usize>,
    /// The position in `order` of the song that is playing.
    cursor: Option<usize>,
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
    }

    /// Song `index` (in list order).
    pub fn get(&self, index: usize) -> Option<&SearchTrack> {
        self.tracks.get(index)
    }

    /// Marks song `index` (in list order) as the one playing.
    pub fn set_playing(&mut self, index: usize) {
        self.cursor = self.order.iter().position(|&song| song == index);
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

    /// Shuffles the songs after the one playing, or puts them back in list order.
    pub fn set_shuffle(&mut self, shuffle: bool, rng: &mut fastrand::Rng) {
        let playing = self.playing();
        self.order = play_order(self.tracks.len(), playing.unwrap_or(0), shuffle, rng);
        if let Some(index) = playing {
            self.set_playing(index);
        }
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
    fn repeat_cycles_through_its_modes() {
        assert_eq!(Repeat::Off.next(), Repeat::All);
        assert_eq!(Repeat::All.next(), Repeat::One);
        assert_eq!(Repeat::One.next(), Repeat::Off);
    }
}
