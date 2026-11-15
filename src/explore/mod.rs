//! The Explore screen, as YouTube Music's: new releases, the charts, new music videos, and
//! moods and genres, each of which opens a page of playlists.

mod view;

use iced::Task;
use rust_i18n::t;
use rustypipe::client::RustyPipe;
use rustypipe::model::{MusicGenre, MusicGenreItem};

use crate::search::Status;
use crate::youtube::{self, SearchTrack, Tile};
use crate::{App, Message, Screen};
pub use view::{genre_view, view};

/// Cards kept for each shelf; YouTube sends over a hundred new albums.
const SHELF_LIMIT: usize = 24;

/// What the Explore screen shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExploreData {
    pub new_albums: Vec<Tile>,
    pub top_songs: Vec<SearchTrack>,
    /// The playlist with all of the top songs, behind "See all".
    pub top_playlist: Option<String>,
    pub trending: Vec<SearchTrack>,
    pub top_artists: Vec<Tile>,
    /// Chart playlists, like the top songs of a genre.
    pub charts: Vec<Tile>,
    pub new_videos: Vec<SearchTrack>,
    pub moods: Vec<Genre>,
    pub genres: Vec<Genre>,
}

impl ExploreData {
    fn image_urls(&self) -> Vec<String> {
        [&self.new_albums, &self.top_artists, &self.charts]
            .into_iter()
            .flatten()
            .filter_map(|tile| tile.image_url.clone())
            .chain(
                [&self.top_songs, &self.trending, &self.new_videos]
                    .into_iter()
                    .flatten()
                    .filter_map(|song| song.cover_url.clone()),
            )
            .collect()
    }
}

/// A mood (like Chill) or a genre (like Rock).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Genre {
    pub id: String,
    pub name: String,
    /// The color YouTube Music gives its button, as `0xRRGGBB`.
    pub color: u32,
}

/// A mood's or genre's page: playlists, in sections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenrePage {
    pub name: String,
    pub sections: Vec<(String, Vec<Tile>)>,
}

/// The Explore screen and the mood or genre opened from it.
#[derive(Debug)]
pub struct Explore {
    status: Status,
    data: Option<ExploreData>,
    genre_status: Status,
    genre: Option<GenrePage>,
}

impl Default for Explore {
    fn default() -> Self {
        Self {
            status: Status::Idle,
            data: None,
            genre_status: Status::Idle,
            genre: None,
        }
    }
}

/// Something that happened on the Explore screen.
#[derive(Debug, Clone)]
pub enum Event {
    Loaded(Result<ExploreData, String>),
    OpenGenre(Genre),
    GenreLoaded(Result<GenrePage, String>),
}

impl App {
    /// Shows the Explore screen, asking YouTube Music for it unless it is loaded or loading.
    pub(crate) fn open_explore(&mut self) -> Task<Message> {
        self.screen = Screen::Explore;
        if matches!(self.explore.status, Status::Loading | Status::Done) {
            return Task::none();
        }
        let Some(client) = self.online.client() else {
            self.explore.status = Status::Failed(t!("explore.client_failed").into_owned());
            return Task::none();
        };
        self.explore.status = Status::Loading;
        Task::perform(load(client), |result| {
            Message::Explore(Event::Loaded(result))
        })
    }

    pub(crate) fn explore_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Loaded(Ok(data)) => {
                let images = self.online.fetch_images(data.image_urls());
                self.explore.status = Status::Done;
                self.explore.data = Some(data);
                return images;
            }
            Event::Loaded(Err(error)) => self.explore.status = Status::Failed(error),
            Event::OpenGenre(genre) => {
                let Some(client) = self.online.client() else {
                    return Task::none();
                };
                self.go_to(Screen::Genre);
                self.explore.genre = Some(GenrePage {
                    name: genre.name,
                    sections: Vec::new(),
                });
                self.explore.genre_status = Status::Loading;
                return Task::perform(load_genre(client, genre.id), |result| {
                    Message::Explore(Event::GenreLoaded(result))
                });
            }
            Event::GenreLoaded(Ok(page)) => {
                let urls = page
                    .sections
                    .iter()
                    .flat_map(|(_, tiles)| tiles)
                    .filter_map(|tile| tile.image_url.clone())
                    .collect();
                self.explore.genre_status = Status::Done;
                self.explore.genre = Some(page);
                return self.online.fetch_images(urls);
            }
            Event::GenreLoaded(Err(error)) => self.explore.genre_status = Status::Failed(error),
        }
        Task::none()
    }
}

/// Asks YouTube Music for everything on the Explore screen at once. A part that fails is left
/// out; only if every part fails is it an error.
pub async fn load(rp: RustyPipe) -> Result<ExploreData, String> {
    let q = rp.query();
    let (albums, charts, videos, genres) = tokio::join!(
        q.music_new_albums(),
        q.music_charts(None),
        q.music_new_videos(),
        q.music_genres(),
    );
    if let (Err(error), Err(_), Err(_), Err(_)) = (&albums, &charts, &videos, &genres) {
        return Err(error.to_string());
    }
    let mut data = ExploreData::default();
    if let Ok(albums) = albums {
        data.new_albums = albums
            .iter()
            .take(SHELF_LIMIT)
            .map(youtube::album_tile)
            .collect();
    }
    if let Ok(charts) = charts {
        data.top_songs = charts.top_tracks.iter().map(youtube::track).collect();
        data.top_playlist = charts.top_playlist_id;
        data.trending = charts.trending_tracks.iter().map(youtube::track).collect();
        data.top_artists = (charts.artists.iter().take(SHELF_LIMIT))
            .map(youtube::artist_tile)
            .collect();
        data.charts = charts
            .playlists
            .iter()
            .map(youtube::playlist_tile)
            .collect();
    }
    if let Ok(videos) = videos {
        data.new_videos = videos
            .iter()
            .take(view::CHART_ROWS)
            .map(youtube::track)
            .collect();
    }
    if let Ok(genres) = genres {
        (data.moods, data.genres) = split_genres(&genres);
    }
    Ok(data)
}

/// The playlists of a mood or genre.
pub async fn load_genre(rp: RustyPipe, id: String) -> Result<GenrePage, String> {
    let genre = rp
        .query()
        .music_genre(&id)
        .await
        .map_err(|error| error.to_string())?;
    Ok(genre_page(&genre))
}

/// Moods and genres, apart.
fn split_genres(items: &[MusicGenreItem]) -> (Vec<Genre>, Vec<Genre>) {
    let (moods, genres): (Vec<_>, Vec<_>) = items.iter().partition(|item| item.is_mood);
    let convert = |items: Vec<&MusicGenreItem>| {
        items
            .into_iter()
            .map(|item| Genre {
                id: item.id.clone(),
                name: item.name.clone(),
                color: item.color & 0x00ff_ffff,
            })
            .collect()
    };
    (convert(moods), convert(genres))
}

fn genre_page(genre: &MusicGenre) -> GenrePage {
    GenrePage {
        name: genre.name.clone(),
        sections: genre
            .sections
            .iter()
            .filter(|section| !section.playlists.is_empty())
            .map(|section| {
                let tiles = section
                    .playlists
                    .iter()
                    .map(youtube::playlist_tile)
                    .collect();
                (section.name.clone(), tiles)
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moods_go_apart_from_genres() {
        let items: Vec<MusicGenreItem> = serde_json::from_value(serde_json::json!([
            { "id": "a", "name": "Chill", "is_mood": true, "color": 0xff11_2233_u32 },
            { "id": "b", "name": "Rock", "is_mood": false, "color": 0xff44_5566_u32 },
        ]))
        .expect("genres");
        let (moods, genres) = split_genres(&items);
        assert_eq!(moods.len(), 1);
        assert_eq!(moods[0].name, "Chill");
        assert_eq!(moods[0].color, 0x11_2233, "the opacity is dropped");
        assert_eq!(genres[0].id, "b");
    }

    /// Asks YouTube Music, signed out, for the Explore screen and a genre; a failure says what
    /// came back: `cargo test -- --ignored loads_explore`.
    #[tokio::test]
    #[ignore = "asks YouTube Music"]
    async fn loads_explore() {
        let rp = RustyPipe::builder().no_storage().build().expect("client");
        let data = load(rp.clone()).await.expect("explore");
        let found = format!(
            "albums {} top {} (playlist {:?}) trending {} artists {} charts {} videos {} \
             moods {} genres {}",
            data.new_albums.len(),
            data.top_songs.len(),
            data.top_playlist,
            data.trending.len(),
            data.top_artists.len(),
            data.charts.len(),
            data.new_videos.len(),
            data.moods.len(),
            data.genres.len(),
        );
        // YouTube Music now gives the charts as playlists, without the songs on the page.
        assert!(!data.new_albums.is_empty(), "{found}");
        assert!(
            !data.charts.is_empty() || !data.top_songs.is_empty(),
            "{found}"
        );
        let genre = data.genres.first().or(data.moods.first()).expect("a genre");
        let page = load_genre(rp, genre.id.clone()).await.expect("genre");
        assert!(!page.sections.is_empty(), "{} has no playlists", page.name);
    }
}
