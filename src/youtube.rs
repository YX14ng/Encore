//! The YouTube Music catalog through rustypipe: search, suggestions, albums, playlists and
//! artists. Audio downloads live in `download.rs`.

use rust_i18n::t;
use rustypipe::client::RustyPipe;
use rustypipe::model::{
    AlbumItem, AlbumType, ArtistItem, MusicItem, MusicPlaylistItem, Thumbnail, TrackItem,
};
use serde::{Deserialize, Serialize};

/// How many songs of a playlist to load. The first page shows at once and the rest follow
/// a page at a time (`more_tracks`).
pub(crate) const PLAYLIST_LIMIT: usize = 5000;

/// The widest image worth downloading for covers and the blurred background.
const COVER_MAX_WIDTH: u32 = 544;

/// A song that can be played.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: u32,
    pub cover_url: Option<String>,
    /// Where the song sits in a playlist (YouTube's `setVideoId`), which taking it out needs:
    /// YouTube Music may hold a song under another id than the one it was added with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
}

/// What a card opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileKind {
    Album,
    Artist,
    Playlist,
    /// A radio built from a song (its id is the song's id).
    Radio,
}

/// An album, artist or playlist, shown as a card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tile {
    pub kind: TileKind,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub image_url: Option<String>,
}

/// The single best match of a search, shown big at the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopResult {
    Track(SearchTrack),
    Tile(Tile),
}

/// Everything a search found, by category.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchPage {
    pub top: Option<TopResult>,
    pub songs: Vec<SearchTrack>,
    pub videos: Vec<SearchTrack>,
    pub albums: Vec<Tile>,
    pub artists: Vec<Tile>,
    pub playlists: Vec<Tile>,
}

/// An opened album, playlist or artist with its songs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    /// The id of the album, playlist or artist on YouTube Music.
    pub id: String,
    pub kind: TileKind,
    pub title: String,
    pub subtitle: String,
    pub image_url: Option<String>,
    pub tracks: Vec<SearchTrack>,
    /// The rest of an artist's page; only artists have it, and it is not kept with downloads.
    #[serde(skip)]
    pub artist: Option<Box<crate::artist::ArtistPage>>,
}

/// A rustypipe client. Its storage holds the session cookies after signing in, so it lives in
/// Encore's private data folder.
pub fn client() -> Result<RustyPipe, String> {
    let storage = crate::account::data_dir().join("rustypipe");
    crate::account::private_dir(&crate::account::data_dir())?;
    crate::account::private_dir(&storage)?;
    RustyPipe::builder()
        .storage_dir(storage)
        // rustypipe's error reports keep whole answers (the user's library) and signed
        // request headers on disk; Encore does not keep them.
        .no_reporter()
        .lang(crate::i18n::youtube_language())
        .build()
        .map_err(|error| error.to_string())
}

/// Searches every category at once, the way YouTube Music's results page does.
pub async fn search_all(rp: RustyPipe, query: String) -> Result<SearchPage, String> {
    let q = rp.query();
    let (main, songs, videos, albums, artists, playlists) = tokio::join!(
        q.music_search_main(&query),
        q.music_search_tracks(&query),
        q.music_search_videos(&query),
        q.music_search_albums(&query),
        q.music_search_artists(&query),
        q.music_search_playlists(&query, false),
    );

    if let (Err(error), Err(_), Err(_), Err(_), Err(_), Err(_)) =
        (&main, &songs, &videos, &albums, &artists, &playlists)
    {
        return Err(error.to_string());
    }

    Ok(SearchPage {
        top: main
            .ok()
            .and_then(|result| result.items.items.first().and_then(top_result)),
        songs: songs
            .map(|result| result.items.items.iter().map(track).collect())
            .unwrap_or_default(),
        videos: videos
            .map(|result| result.items.items.iter().map(track).collect())
            .unwrap_or_default(),
        albums: albums
            .map(|result| result.items.items.iter().map(album_tile).collect())
            .unwrap_or_default(),
        artists: artists
            .map(|result| result.items.items.iter().map(artist_tile).collect())
            .unwrap_or_default(),
        playlists: playlists
            .map(|result| result.items.items.iter().map(playlist_tile).collect())
            .unwrap_or_default(),
    })
}

/// Search terms YouTube Music suggests for what has been typed so far.
pub async fn suggestions(rp: RustyPipe, query: String) -> Result<Vec<String>, String> {
    let result = rp
        .query()
        .music_search_suggestion(&query)
        .await
        .map_err(|error| error.to_string())?;
    Ok(result.terms.into_iter().take(6).collect())
}

/// Loads the songs of an album, a playlist or an artist (their top songs).
/// Songs like `seed`, from its YouTube Music radio, for autoplay. The first page is enough:
/// another one is asked for when those run out.
pub async fn radio(rp: RustyPipe, seed: String) -> Result<Vec<SearchTrack>, String> {
    let radio = rp
        .query()
        .music_radio_track(&seed)
        .await
        .map_err(|error| error.to_string())?;
    Ok(radio.items.iter().map(track).collect())
}

/// Which page of a song to go to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SongPlace {
    Artist,
    Album,
}

/// The artist (the first one with a page) or the album of song `id`, as a card to open.
pub async fn place_of_song(rp: RustyPipe, id: String, place: SongPlace) -> Result<Tile, String> {
    let details = rp
        .query()
        .music_details(&id)
        .await
        .map_err(|error| error.to_string())?;
    let track = details.track;
    match place {
        SongPlace::Artist => track
            .artists
            .iter()
            .find_map(|artist| {
                Some(Tile {
                    kind: TileKind::Artist,
                    id: artist.id.clone()?,
                    title: artist.name.clone(),
                    subtitle: String::new(),
                    image_url: None,
                })
            })
            .ok_or_else(|| t!("song_menu.no_artist").into_owned()),
        SongPlace::Album => track
            .album
            .map(|album| Tile {
                kind: TileKind::Album,
                id: album.id,
                title: album.name,
                subtitle: artist_names(&track.artists),
                image_url: best_image(&track.cover),
            })
            .ok_or_else(|| t!("song_menu.no_album").into_owned()),
    }
}

pub async fn open(rp: RustyPipe, tile: Tile) -> Result<Opened, String> {
    if tile.kind == TileKind::Playlist {
        return open_playlist(rp, tile).await;
    }
    open_collection(rp, tile).await.map(|collection| Opened {
        collection,
        more: None,
    })
}

/// An opened collection, and for a long playlist the songs still to load.
#[derive(Debug, Clone)]
pub struct Opened {
    pub collection: Collection,
    pub more: Option<MoreTracks>,
}

/// The rest of a playlist, loaded a page (about 100 songs) at a time after the first page is
/// on screen.
#[derive(Debug, Clone)]
pub struct MoreTracks {
    /// The playlist the songs belong to.
    pub collection_id: String,
    page: rustypipe::model::paginator::Paginator<rustypipe::model::TrackItem>,
    /// The playlist was read with the session, so the next pages are too.
    signed_in: bool,
    /// How many songs are loaded already.
    loaded: usize,
}

/// A playlist's first page. With a session, it is asked for with the session first: the
/// user's own playlists are private, and asking without it first only adds a failed request.
async fn open_playlist(rp: RustyPipe, tile: Tile) -> Result<Opened, String> {
    let q = rp.query();
    let has_session = rp.user_auth_cookie_header().is_some();
    let attempts = if has_session {
        [true, false]
    } else {
        [false, true]
    };
    let mut first_error = None;
    for signed_in in attempts {
        let q = if signed_in {
            q.clone().authenticated()
        } else {
            q.clone()
        };
        match q.music_playlist(&tile.id).await {
            Ok(playlist) => {
                let collection = Collection {
                    id: tile.id.clone(),
                    kind: TileKind::Playlist,
                    title: playlist.name.clone(),
                    subtitle: tile.subtitle,
                    image_url: best_image(&playlist.thumbnail),
                    tracks: playlist.tracks.items.iter().map(track).collect(),
                    artist: None,
                };
                let loaded = collection.tracks.len();
                let more =
                    (!playlist.tracks.is_exhausted() && loaded < PLAYLIST_LIMIT).then(|| {
                        MoreTracks {
                            collection_id: tile.id.clone(),
                            page: playlist.tracks,
                            signed_in,
                            loaded,
                        }
                    });
                return Ok(Opened { collection, more });
            }
            Err(error) => {
                first_error.get_or_insert(error.to_string());
            }
        }
    }
    Err(first_error.unwrap_or_default())
}

/// A collection with every song, all pages of a playlist loaded before it returns.
pub async fn open_whole(rp: RustyPipe, tile: Tile) -> Result<Collection, String> {
    let Opened {
        mut collection,
        mut more,
    } = open(rp.clone(), tile).await?;
    while let Some(next) = more {
        let (tracks, rest) = more_tracks(rp.clone(), next).await?;
        collection.tracks.extend(tracks);
        more = rest;
    }
    Ok(collection)
}

/// The next page of a playlist, and what is left after it.
pub async fn more_tracks(
    rp: RustyPipe,
    more: MoreTracks,
) -> Result<(Vec<SearchTrack>, Option<MoreTracks>), String> {
    let q = if more.signed_in {
        rp.query().authenticated()
    } else {
        rp.query()
    };
    let Some(page) = more
        .page
        .next(&q)
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok((Vec::new(), None));
    };
    let tracks: Vec<SearchTrack> = page.items.iter().map(track).collect();
    let loaded = more.loaded + tracks.len();
    let go_on = !page.is_exhausted() && !tracks.is_empty() && loaded < PLAYLIST_LIMIT;
    let rest = go_on.then_some(MoreTracks {
        page,
        loaded,
        ..more
    });
    Ok((tracks, rest))
}

/// An album, artist or radio.
async fn open_collection(rp: RustyPipe, tile: Tile) -> Result<Collection, String> {
    let q = rp.query();
    match tile.kind {
        TileKind::Album => {
            let album = q
                .music_album(&tile.id)
                .await
                .map_err(|error| error.to_string())?;
            let cover = best_image(&album.cover);
            let tracks = album
                .tracks
                .iter()
                .map(|item| {
                    let mut song = track(item);
                    song.album.clone_from(&album.name);
                    if song.cover_url.is_none() {
                        song.cover_url.clone_from(&cover);
                    }
                    song
                })
                .collect();
            Ok(Collection {
                id: tile.id.clone(),
                kind: TileKind::Album,
                title: album.name.clone(),
                subtitle: album_subtitle(
                    album.album_type,
                    &artist_names(&album.artists),
                    album.year,
                ),
                image_url: cover,
                tracks,
                artist: None,
            })
        }
        TileKind::Radio => {
            // A song's radio, or a radio YouTube Music made (an artist's), by its playlist id.
            let radio = if is_song_id(&tile.id) {
                q.music_radio_track(&tile.id).await
            } else {
                q.music_radio(&tile.id).await
            };
            let mut radio = radio.map_err(|error| error.to_string())?;
            // A radio is endless; one more page is plenty for a session.
            let _ = radio.extend(&q).await;
            Ok(Collection {
                id: tile.id.clone(),
                kind: TileKind::Radio,
                title: tile.title.clone(),
                subtitle: tile.subtitle.clone(),
                image_url: tile.image_url.clone(),
                tracks: radio.items.iter().map(track).collect(),
                artist: None,
            })
        }
        TileKind::Playlist => Box::pin(open_playlist(rp, tile))
            .await
            .map(|opened| opened.collection),
        TileKind::Artist => {
            let artist = q
                .music_artist(&tile.id, false)
                .await
                .map_err(|error| error.to_string())?;
            Ok(Collection {
                id: tile.id.clone(),
                kind: TileKind::Artist,
                title: artist.name.clone(),
                subtitle: artist_subtitle(artist.subscriber_count),
                image_url: best_image(&artist.header_image).or(tile.image_url),
                tracks: artist.tracks.iter().map(track).collect(),
                artist: Some(Box::new(crate::artist::page(&artist))),
            })
        }
    }
}

fn top_result(item: &MusicItem) -> Option<TopResult> {
    match item {
        MusicItem::Track(item) => Some(TopResult::Track(track(item))),
        MusicItem::Album(item) => Some(TopResult::Tile(album_tile(item))),
        MusicItem::Artist(item) => Some(TopResult::Tile(artist_tile(item))),
        MusicItem::Playlist(item) => Some(TopResult::Tile(playlist_tile(item))),
        MusicItem::User(_) => None,
    }
}

pub(crate) fn track(item: &TrackItem) -> SearchTrack {
    SearchTrack {
        id: item.id.clone(),
        title: item.name.clone(),
        artist: artist_names(&item.artists),
        album: item
            .album
            .as_ref()
            .map(|album| album.name.clone())
            .unwrap_or_default(),
        duration: item.duration.unwrap_or(0),
        cover_url: best_image(&item.cover),
        entry: item.playlist_set_video_id.clone(),
    }
}

pub(crate) fn album_tile(item: &AlbumItem) -> Tile {
    Tile {
        kind: TileKind::Album,
        id: item.id.clone(),
        title: item.name.clone(),
        subtitle: album_subtitle(item.album_type, &artist_names(&item.artists), item.year),
        image_url: best_image(&item.cover),
    }
}

pub(crate) fn artist_tile(item: &ArtistItem) -> Tile {
    Tile {
        kind: TileKind::Artist,
        id: item.id.clone(),
        title: item.name.clone(),
        subtitle: artist_subtitle(item.subscriber_count),
        image_url: best_image(&item.avatar),
    }
}

pub(crate) fn playlist_tile(item: &MusicPlaylistItem) -> Tile {
    let owner = item
        .channel
        .as_ref()
        .map_or("YouTube Music", |channel| channel.name.as_str());
    Tile {
        kind: TileKind::Playlist,
        id: item.id.clone(),
        title: item.name.clone(),
        subtitle: t!("youtube.playlist_by", owner = owner).into_owned(),
        image_url: best_image(&item.thumbnail),
    }
}

fn artist_names(artists: &[rustypipe::model::ArtistId]) -> String {
    artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// How big the cover of the song playing is asked for: sharp on the now playing page.
const LARGE_COVER: u32 = 1200;

/// The same cover as `url`, `LARGE_COVER` pixels wide. Song covers in lists come about 120
/// pixels wide, and Google's image servers make any size when the size options after the `=`
/// (`=w120-h120-l90-rj`) ask for it. `None` for other images, such as video thumbnails.
pub fn large_cover(url: &str) -> Option<String> {
    let host = url.split('/').nth(2)?;
    if !host.ends_with(".googleusercontent.com") && !host.ends_with(".ggpht.com") {
        return None;
    }
    let (base, options) = url.rsplit_once('=')?;
    let mut sized = false;
    let options: Vec<String> = options
        .split('-')
        .map(|option| {
            let is_size = |letter: char| {
                option.strip_prefix(letter).is_some_and(|number| {
                    !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
                })
            };
            match option.chars().next() {
                Some(letter @ ('w' | 'h' | 's')) if is_size(letter) => {
                    sized = true;
                    format!("{letter}{LARGE_COVER}")
                }
                _ => option.to_string(),
            }
        })
        .collect();
    sized.then(|| format!("{base}={}", options.join("-")))
}

fn best_image(thumbnails: &[Thumbnail]) -> Option<String> {
    image_up_to(thumbnails, COVER_MAX_WIDTH)
}

/// The largest of `thumbnails` that is at most `max_width` wide.
pub(crate) fn image_up_to(thumbnails: &[Thumbnail], max_width: u32) -> Option<String> {
    pick_cover(
        thumbnails
            .iter()
            .map(|thumbnail| (thumbnail.width, thumbnail.url.as_str())),
        max_width,
    )
}

/// Whether `id` is a song's (a video id is 11 characters), not a playlist's.
fn is_song_id(id: &str) -> bool {
    id.len() == 11
}

/// From `(width, url)` pairs, the largest image that is at most `max_width` wide, or the
/// smallest one if all are wider.
pub fn pick_cover<'a>(
    covers: impl Iterator<Item = (u32, &'a str)> + Clone,
    max_width: u32,
) -> Option<String> {
    let fitting = covers
        .clone()
        .filter(|(width, _)| *width <= max_width)
        .max_by_key(|(width, _)| *width);
    fitting
        .or_else(|| covers.min_by_key(|(width, _)| *width))
        .map(|(_, url)| url.to_string())
}

/// "Single · Ado · 2021", leaving out what is unknown.
pub fn album_subtitle(album_type: AlbumType, artists: &str, year: Option<u16>) -> String {
    let kind = match album_type {
        AlbumType::Single => t!("youtube.single"),
        AlbumType::Ep => t!("youtube.ep"),
        _ => t!("youtube.album"),
    };
    let mut parts = vec![kind.into_owned()];
    if !artists.is_empty() {
        parts.push(artists.to_string());
    }
    if let Some(year) = year {
        parts.push(year.to_string());
    }
    parts.join(" \u{b7} ")
}

/// "Artist · 38M subscribers", or just "Artist".
pub fn artist_subtitle(subscribers: Option<u64>) -> String {
    match subscribers {
        Some(count) => t!("youtube.artist_subscribers", count = compact_count(count)).into_owned(),
        None => t!("youtube.artist").into_owned(),
    }
}

/// A short count: 950, 1.2K, 38M.
pub fn compact_count(count: u64) -> String {
    let (value, suffix) = match count {
        0..=999 => return count.to_string(),
        1_000..=999_999 => (count as f64 / 1_000.0, "K"),
        1_000_000..=999_999_999 => (count as f64 / 1_000_000.0, "M"),
        _ => (count as f64 / 1_000_000_000.0, "B"),
    };
    if value < 10.0 && value.fract() >= 0.05 {
        format!("{value:.1}{suffix}")
    } else {
        format!("{value:.0}{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_largest_cover_that_fits() {
        let covers = [(60, "a"), (226, "b"), (544, "c"), (1200, "d")];
        assert_eq!(pick_cover(covers.into_iter(), 544).as_deref(), Some("c"));
    }

    #[test]
    fn falls_back_to_the_smallest_cover() {
        let covers = [(1200, "big"), (800, "smaller")];
        assert_eq!(
            pick_cover(covers.into_iter(), 544).as_deref(),
            Some("smaller")
        );
        assert_eq!(pick_cover(std::iter::empty(), 544), None);
    }

    #[test]
    fn counts_are_short() {
        assert_eq!(compact_count(950), "950");
        assert_eq!(compact_count(1_200), "1.2K");
        assert_eq!(compact_count(38_000_000), "38M");
        assert_eq!(compact_count(2_000_000_000), "2B");
    }

    #[test]
    fn album_subtitles_skip_unknown_parts() {
        assert_eq!(
            album_subtitle(AlbumType::Single, "Ado", Some(2021)),
            "Single \u{b7} Ado \u{b7} 2021"
        );
        assert_eq!(album_subtitle(AlbumType::Album, "", None), "Album");
    }

    #[test]
    fn artist_subtitle_mentions_subscribers() {
        assert_eq!(
            artist_subtitle(Some(38_000_000)),
            "Artist \u{b7} 38M subscribers"
        );
        assert_eq!(artist_subtitle(None), "Artist");
    }

    /// Hits the network and decodes audio silently: `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore = "needs the network, yt-dlp and libmpv"]
    async fn searches_downloads_and_plays() {
        let rp = client().expect("client");
        let page = search_all(rp.clone(), "Daft Punk Get Lucky".to_string())
            .await
            .expect("search");
        assert!(page.top.is_some(), "no top result");
        assert!(!page.albums.is_empty(), "no albums");
        let first = page.songs.first().expect("at least one song");
        let path = crate::download::audio(first.id.clone(), Default::default())
            .await
            .expect("download");
        let audio = crate::audio::Audio::muted().expect("mpv");
        audio
            .play(&path, &crate::silence::Trim::default())
            .expect("play");
        std::thread::sleep(std::time::Duration::from_secs(3));
        assert!(
            audio.position().unwrap_or(0.0) > 0.5,
            "audio did not advance"
        );

        let album = page.albums.first().cloned().expect("an album");
        let collection = open(rp, album).await.expect("open album").collection;
        assert!(!collection.tracks.is_empty(), "the album has no songs");
    }

    /// Opens a radio built from a song: `cargo test -- --ignored opens_a_radio`.
    #[tokio::test]
    #[ignore = "needs the network"]
    async fn opens_a_radio() {
        let tile = Tile {
            kind: TileKind::Radio,
            id: "Rgrt_8mXrK8".to_string(),
            title: "Daft Punk Mix".to_string(),
            subtitle: String::new(),
            image_url: None,
        };
        let radio = open(client().expect("client"), tile)
            .await
            .expect("radio")
            .collection;
        assert!(radio.tracks.len() > 10, "only {} songs", radio.tracks.len());
    }

    /// Asks for autoplay's similar songs: `cargo test -- --ignored finds_similar_songs`.
    #[tokio::test]
    #[ignore = "needs the network"]
    async fn finds_similar_songs() {
        let songs = radio(client().expect("client"), "Rgrt_8mXrK8".to_string())
            .await
            .expect("radio");
        assert!(songs.len() > 10, "only {} songs", songs.len());
        let others = songs.iter().filter(|song| song.id != "Rgrt_8mXrK8").count();
        assert!(others > 10, "only {others} other songs");
    }

    #[test]
    fn asks_google_for_a_large_cover() {
        assert_eq!(
            large_cover("https://lh3.googleusercontent.com/abc=w120-h120-l90-rj").as_deref(),
            Some("https://lh3.googleusercontent.com/abc=w1200-h1200-l90-rj")
        );
        assert_eq!(
            large_cover("https://yt3.ggpht.com/abc=s88-c-k").as_deref(),
            Some("https://yt3.ggpht.com/abc=s1200-c-k")
        );
        assert_eq!(
            large_cover("https://i.ytimg.com/vi/abc/hqdefault.jpg"),
            None,
            "video thumbnails stay"
        );
        assert_eq!(
            large_cover("https://lh3.googleusercontent.com/abc=rj"),
            None,
            "no size to change"
        );
    }
}
