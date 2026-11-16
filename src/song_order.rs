//! Searching and sorting a long song list on screen (a playlist, an album, the library's songs):
//! a search box that keeps the songs whose title, artist or album has every word typed, and a
//! choice of order. The list itself is left as it is; what is shown is a list of its positions,
//! worked out again only when the list or the choices change, not on every redraw.

use std::sync::Arc;

use iced::widget::{button, container, pick_list, row, text, text_input, tooltip};
use iced::{Center, Element, border};
use rust_i18n::t;

use crate::youtube::SearchTrack;
use crate::{Message, theme};

/// What the songs are ordered by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortBy {
    /// The list's own order.
    #[default]
    List,
    Title,
    Artist,
    Album,
    Duration,
}

impl SortBy {
    pub const ALL: [SortBy; 5] = [
        SortBy::List,
        SortBy::Title,
        SortBy::Artist,
        SortBy::Album,
        SortBy::Duration,
    ];
}

impl std::fmt::Display for SortBy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            SortBy::List => t!("songs.sort_list"),
            SortBy::Title => t!("songs.sort_title"),
            SortBy::Artist => t!("songs.sort_artist"),
            SortBy::Album => t!("songs.sort_album"),
            SortBy::Duration => t!("songs.sort_duration"),
        };
        f.write_str(&label)
    }
}

/// What the user does with the search box and the order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Search(String),
    Sort(SortBy),
    /// Flips between ascending and descending.
    Reverse,
}

/// The list arranged for: its id, length, first and last song, and the choices.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Key {
    list: String,
    len: usize,
    ends: (Option<String>, Option<String>),
    query: String,
    sort: SortBy,
    descending: bool,
}

/// The search and order of the list on screen.
#[derive(Debug, Default)]
pub struct SongOrder {
    pub query: String,
    pub sort: SortBy,
    pub descending: bool,
    key: Option<Key>,
    /// Positions in the list, in the order shown; `None` shows the list as it is.
    order: Option<Arc<[usize]>>,
}

impl SongOrder {
    pub fn handle(&mut self, event: Event) {
        match event {
            Event::Search(query) => self.query = query,
            Event::Sort(sort) => self.sort = sort,
            Event::Reverse => self.descending = !self.descending,
        }
    }

    /// Arranges `tracks` (the list `list`) again if it or the choices changed. Another list
    /// starts with no search, in its own order.
    pub fn refresh(&mut self, list: &str, tracks: &[SearchTrack]) {
        if self.key.as_ref().is_some_and(|key| key.list != list) {
            self.query.clear();
            self.sort = SortBy::List;
            self.descending = false;
        }
        let key = Key {
            list: list.to_string(),
            len: tracks.len(),
            ends: (
                tracks.first().map(|track| track.id.clone()),
                tracks.last().map(|track| track.id.clone()),
            ),
            query: self.query.clone(),
            sort: self.sort,
            descending: self.descending,
        };
        if self.key.as_ref() == Some(&key) {
            return;
        }
        self.order = arrange(tracks, &self.query, self.sort, self.descending).map(Arc::from);
        self.key = Some(key);
    }

    /// The positions shown, or `None` when the list shows as it is.
    pub fn order(&self) -> Option<Arc<[usize]>> {
        self.order.clone()
    }

    /// How many songs show out of `total`.
    pub fn shown(&self, total: usize) -> usize {
        self.order.as_ref().map_or(total, |order| order.len())
    }
}

impl crate::App {
    /// Keeps the search and order in step with the long song list on screen, if one is.
    pub(crate) fn arrange_songs(&mut self) {
        use crate::Screen;
        use crate::library::{Account, Shelf};

        match self.screen {
            Screen::Collection => {
                if let (Some(collection), crate::search::Status::Done, _) = self.online.collection()
                {
                    self.song_order.refresh(&collection.id, &collection.tracks);
                }
            }
            Screen::Library(shelf @ (Shelf::History | Shelf::Songs | Shelf::Liked)) => {
                if let Account::SignedIn(data) = &self.account
                    && let Some(tracks) = data.shelf_tracks(shelf)
                {
                    self.song_order.refresh(&format!("{shelf:?}"), tracks);
                }
            }
            _ => {}
        }
    }
}

/// The positions of the songs of `tracks` to show, in order, or `None` for all of them as
/// they are.
pub fn arrange(
    tracks: &[SearchTrack],
    query: &str,
    sort: SortBy,
    descending: bool,
) -> Option<Vec<usize>> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() && sort == SortBy::List && !descending {
        return None;
    }
    let mut order: Vec<usize> = (0..tracks.len())
        .filter(|&index| {
            let track = &tracks[index];
            let haystack =
                format!("{} {} {}", track.title, track.artist, track.album).to_lowercase();
            words.iter().all(|word| haystack.contains(word.as_str()))
        })
        .collect();
    let text_key = |index: &usize| -> String {
        let track = &tracks[*index];
        match sort {
            SortBy::Title => track.title.to_lowercase(),
            SortBy::Artist => track.artist.to_lowercase(),
            SortBy::Album => track.album.to_lowercase(),
            SortBy::List | SortBy::Duration => String::new(),
        }
    };
    match sort {
        SortBy::List => {}
        // Songs without a known length go last.
        SortBy::Duration => order.sort_by_key(|&index| match tracks[index].duration {
            0 => u32::MAX,
            seconds => seconds,
        }),
        // Songs with an empty field go last; ties keep the list's order.
        _ => order.sort_by_cached_key(|index| {
            let key = text_key(index);
            (key.is_empty(), key)
        }),
    }
    if descending {
        order.reverse();
    }
    Some(order)
}

/// The search box, the order and the count of songs shown.
pub fn toolbar(order: &SongOrder, total: usize) -> Element<'_, Message> {
    let search = text_input(&t!("songs.search"), &order.query)
        .on_input(|query| Message::SongOrder(Event::Search(query)))
        .padding([8, 12])
        .size(14)
        .width(280)
        .style(|_, _| text_input::Style {
            background: theme::tint(0.05).into(),
            border: border::rounded(18).color(theme::line()).width(1),
            icon: theme::text_dim(),
            placeholder: theme::text_dim(),
            value: theme::text_primary(),
            selection: theme::accent().scale_alpha(0.4),
        });
    let sort = pick_list(SortBy::ALL, Some(order.sort), |sort| {
        Message::SongOrder(Event::Sort(sort))
    })
    .text_size(14)
    .padding([8, 12]);
    let (arrow, label) = if order.descending {
        ("\u{2193}", t!("songs.descending"))
    } else {
        ("\u{2191}", t!("songs.ascending"))
    };
    let reverse = tooltip(
        button(text(arrow).size(16))
            .on_press(Message::SongOrder(Event::Reverse))
            .padding([6, 12])
            .style(|_, status| {
                let mut style = theme::flat_button(status, false);
                style.border = border::rounded(18).color(theme::line()).width(1);
                style
            }),
        container(text(label).size(12))
            .padding([3, 8])
            .style(|_| theme::panel(theme::hero()).border(border::rounded(6))),
        tooltip::Position::Bottom,
    );
    let shown = order.shown(total);
    let count = (shown != total).then(|| {
        text(t!("songs.shown", shown = shown, total = total))
            .size(13)
            .color(theme::text_secondary())
    });
    row![search, sort, reverse]
        .push(count)
        .spacing(10)
        .align_y(Center)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(title: &str, artist: &str, album: &str, duration: u32) -> SearchTrack {
        SearchTrack {
            id: title.to_string(),
            title: title.to_string(),
            artist: artist.to_string(),
            album: album.to_string(),
            duration,
            cover_url: None,
            entry: None,
        }
    }

    fn songs() -> Vec<SearchTrack> {
        vec![
            track("Usseewa", "Ado", "Kyougen", 203),
            track("Idol", "YOASOBI", "", 213),
            track("Kaerimichi", "MONOGATARI Series", "Utamonogatari", 247),
            track("Odo", "Ado", "Kyougen", 0),
        ]
    }

    #[test]
    fn the_list_as_it_is_needs_no_arranging() {
        assert_eq!(arrange(&songs(), "  ", SortBy::List, false), None);
    }

    #[test]
    fn every_word_must_match_title_artist_or_album() {
        let songs = songs();
        assert_eq!(
            arrange(&songs, "ado", SortBy::List, false),
            Some(vec![0, 3])
        );
        assert_eq!(
            arrange(&songs, "ADO kyou odo", SortBy::List, false),
            Some(vec![3])
        );
        assert_eq!(
            arrange(&songs, "utamono", SortBy::List, false),
            Some(vec![2])
        );
        assert_eq!(
            arrange(&songs, "nothing", SortBy::List, false),
            Some(vec![])
        );
    }

    #[test]
    fn sorts_by_each_field_with_blanks_last() {
        let songs = songs();
        assert_eq!(
            arrange(&songs, "", SortBy::Title, false),
            Some(vec![1, 2, 3, 0])
        );
        assert_eq!(
            arrange(&songs, "", SortBy::Artist, false),
            Some(vec![0, 3, 2, 1])
        );
        assert_eq!(
            arrange(&songs, "", SortBy::Album, false),
            Some(vec![0, 3, 2, 1])
        );
        assert_eq!(
            arrange(&songs, "", SortBy::Duration, false),
            Some(vec![0, 1, 2, 3])
        );
        assert_eq!(
            arrange(&songs, "", SortBy::List, true),
            Some(vec![3, 2, 1, 0])
        );
    }

    #[test]
    fn another_list_starts_over() {
        let songs = songs();
        let mut order = SongOrder::default();
        order.refresh("a", &songs);
        order.handle(Event::Search("ado".to_string()));
        order.handle(Event::Sort(SortBy::Title));
        order.refresh("a", &songs);
        assert_eq!(order.order().as_deref(), Some(&[3, 0][..]));
        assert_eq!(order.shown(songs.len()), 2);
        order.refresh("b", &songs);
        assert!(order.query.is_empty() && order.sort == SortBy::List);
        assert_eq!(order.order(), None);
    }
}
