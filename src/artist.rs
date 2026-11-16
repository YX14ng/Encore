//! An artist's page: a banner with the artist's name, the most played songs, then albums,
//! singles, playlists that feature the artist, similar artists and what YouTube Music says
//! about them.

use iced::font::Weight;
use iced::gradient::Linear;
use iced::widget::{Column, button, column, container, image, row, scrollable, space, stack, text};
use iced::{Center, ContentFit, Degrees, Element, Fill, Font, border};
use rust_i18n::t;
use rustypipe::model::{AlbumType, MusicArtist};

use crate::offline::Offline;
use crate::search::{self, Shared};
use crate::youtube::{self, Collection, Tile, TileKind};
use crate::{Message, theme};

/// The widest banner worth downloading: the center column is rarely wider.
const BANNER_MAX_WIDTH: u32 = 1440;

const BANNER_HEIGHT: f32 = 300.0;

/// Songs shown under "Top songs"; Play plays them all, "See all" opens every song.
const TOP_SONGS: usize = 10;

/// What an artist's page shows besides the songs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtistPage {
    pub description: Option<String>,
    /// The wide picture at the top of the page.
    pub banner_url: Option<String>,
    pub albums: Vec<Tile>,
    /// Singles and EPs.
    pub singles: Vec<Tile>,
    /// Playlists that feature the artist.
    pub playlists: Vec<Tile>,
    pub similar: Vec<Tile>,
    /// The playlist with all of the artist's songs, behind "See all".
    pub songs_playlist: Option<String>,
    /// The id of the artist's radio, a playlist YouTube Music makes.
    pub radio: Option<String>,
}

impl ArtistPage {
    /// Every picture on the page, to download.
    pub fn image_urls(&self) -> impl Iterator<Item = String> + '_ {
        self.banner_url.iter().cloned().chain(
            [&self.albums, &self.singles, &self.playlists, &self.similar]
                .into_iter()
                .flatten()
                .filter_map(|tile| tile.image_url.clone()),
        )
    }
}

/// The page of `artist`, as YouTube Music answered it.
pub fn page(artist: &MusicArtist) -> ArtistPage {
    let (singles, albums): (Vec<_>, Vec<_>) = artist
        .albums
        .iter()
        .partition(|album| matches!(album.album_type, AlbumType::Single | AlbumType::Ep));
    ArtistPage {
        description: artist
            .description
            .as_ref()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty()),
        banner_url: youtube::image_up_to(&artist.header_image, BANNER_MAX_WIDTH),
        albums: albums.into_iter().map(youtube::album_tile).collect(),
        singles: singles.into_iter().map(youtube::album_tile).collect(),
        playlists: artist
            .playlists
            .iter()
            .map(youtube::playlist_tile)
            .collect(),
        similar: artist
            .similar_artists
            .iter()
            .map(youtube::artist_tile)
            .collect(),
        songs_playlist: artist.tracks_playlist_id.clone(),
        radio: artist.radio_id.clone(),
    }
}

/// The artist screen.
pub fn view<'a>(
    collection: &'a Collection,
    page: &'a ArtistPage,
    shared: Shared<'a>,
    offline: &'a Offline,
) -> Element<'a, Message> {
    let mut content = column![
        banner(collection, page, shared),
        buttons(collection, page, offline)
    ]
    .spacing(4)
    .width(Fill);

    if !collection.tracks.is_empty() {
        let see_all = page.songs_playlist.as_ref().map(|id| Tile {
            kind: TileKind::Playlist,
            id: id.clone(),
            title: t!("artist.all_songs", artist = collection.title).into_owned(),
            subtitle: t!("youtube.playlist_by", owner = collection.title).into_owned(),
            image_url: collection.image_url.clone(),
        });
        let songs = search::track_list(&collection.tracks, Some(TOP_SONGS), shared);
        content = content.push(section(t!("artist.top_songs"), see_all, songs));
    }
    for (key, tiles) in [
        ("library.albums", &page.albums),
        ("artist.singles", &page.singles),
        ("artist.featured_on", &page.playlists),
        ("artist.fans_also_like", &page.similar),
    ] {
        if !tiles.is_empty() {
            content = content.push(section(t!(key), None, search::shelf(tiles, shared)));
        }
    }
    if let Some(description) = &page.description {
        let about = text(description)
            .size(14)
            .color(theme::text_secondary())
            .width(Fill);
        content = content.push(section(t!("artist.about"), None, about.into()));
    }

    scrollable(content.padding(iced::Padding::default().bottom(28)))
        .height(Fill)
        .into()
}

/// The banner with the artist's name and subscribers over it, or the round picture beside
/// them when YouTube has no banner.
fn banner<'a>(
    collection: &'a Collection,
    page: &'a ArtistPage,
    shared: Shared<'a>,
) -> Element<'a, Message> {
    let name = column![
        text(t!("collection.kind_artist"))
            .size(11)
            .color(theme::text_secondary()),
        text(&collection.title).size(44).font(bold()),
    ]
    .push((!collection.subtitle.is_empty()).then(|| {
        text(&collection.subtitle)
            .size(13)
            .color(theme::text_secondary())
    }))
    .spacing(4);

    let picture = page
        .banner_url
        .as_deref()
        .and_then(|url| shared.images.get(url));
    let Some(handle) = picture else {
        let avatar = search::picture(collection.image_url.as_deref(), 180.0, true, shared);
        return row![avatar, name]
            .spacing(24)
            .align_y(Center)
            .padding([28, 32])
            .into();
    };

    let background = image(handle.clone())
        .width(Fill)
        .height(BANNER_HEIGHT)
        .content_fit(ContentFit::Cover);
    let fade = container(space())
        .width(Fill)
        .height(BANNER_HEIGHT)
        .style(|_| {
            let gradient = Linear::new(Degrees(180.0))
                .add_stop(0.0, theme::center_bg().scale_alpha(0.0))
                .add_stop(0.55, theme::center_bg().scale_alpha(0.45))
                .add_stop(1.0, theme::center_bg());
            container::background(gradient)
        });
    let over = container(name)
        .width(Fill)
        .height(BANNER_HEIGHT)
        .align_bottom(BANNER_HEIGHT)
        .padding([18, 32]);
    stack![background, fade, over].into()
}

/// Play (the top songs), the artist's radio and Download.
fn buttons<'a>(
    collection: &'a Collection,
    page: &'a ArtistPage,
    offline: &'a Offline,
) -> Element<'a, Message> {
    let play =
        (!collection.tracks.is_empty()).then(|| Message::PlayAll(collection.tracks.clone(), false));
    let radio = page.radio.as_ref().map(|id| {
        Message::OpenTile(Tile {
            kind: TileKind::Radio,
            id: id.clone(),
            title: t!("artist.radio_title", artist = collection.title).into_owned(),
            subtitle: t!("artist.radio_subtitle").into_owned(),
            image_url: collection.image_url.clone(),
        })
    });
    row![
        pill(t!("search.play"), play, true),
        pill(t!("artist.radio"), radio, false),
        crate::collection::download_button(collection, offline),
    ]
    .spacing(10)
    .padding([8, 32])
    .into()
}

/// A titled part of a page, with "See all" opening `see_all` when there is more.
pub(crate) fn section<'a>(
    title: impl text::IntoFragment<'a>,
    see_all: Option<Tile>,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let header = row![text(title).size(20).font(bold()), space::horizontal()]
        .push(see_all.map(|tile| {
            button(
                text(t!("artist.see_all"))
                    .size(13)
                    .color(theme::text_secondary()),
            )
            .on_press(Message::OpenTile(tile))
            .padding([4, 10])
            .style(|_, status| theme::flat_button(status, false))
        }))
        .align_y(Center);
    Column::new()
        .push(header)
        .push(body)
        .spacing(12)
        .padding([14, 32])
        .into()
}

fn pill<'a>(
    label: impl text::IntoFragment<'a>,
    message: Option<Message>,
    primary: bool,
) -> Element<'a, Message> {
    button(text(label).size(14).color(theme::text_primary()))
        .on_press_maybe(message)
        .padding([8, 22])
        .style(move |_, status| {
            let mut style = theme::flat_button(status, false);
            if primary {
                style.background = Some(theme::accent().into());
                style.border = border::rounded(18);
            } else {
                style.border = border::rounded(18).color(theme::line()).width(1);
            }
            style
        })
        .into()
}

fn bold() -> Font {
    Font {
        weight: Weight::Bold,
        ..theme::INTER
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album(id: &str, album_type: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "name": id,
            "cover": [],
            "artists": [{ "id": "UC1", "name": "Ado" }],
            "artist_id": "UC1",
            "album_type": album_type,
            "year": 2022,
            "by_va": false,
        })
    }

    #[test]
    fn singles_and_eps_go_apart_from_albums() {
        let artist: MusicArtist = serde_json::from_value(serde_json::json!({
            "id": "UC1",
            "name": "Ado",
            "header_image": [],
            "description": "  A singer.  ",
            "wikipedia_url": null,
            "subscriber_count": 7_000_000,
            "tracks": [],
            "albums": [
                album("full", "album"),
                album("single", "single"),
                album("ep", "ep"),
            ],
            "playlists": [],
            "similar_artists": [],
            "tracks_playlist_id": "OLAK5uy_songs",
            "videos_playlist_id": null,
            "radio_id": "RDEMradio",
        }))
        .expect("artist");
        let page = page(&artist);
        let ids = |tiles: &[Tile]| tiles.iter().map(|t| t.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&page.albums), ["full"]);
        assert_eq!(ids(&page.singles), ["single", "ep"]);
        assert_eq!(page.description.as_deref(), Some("A singer."));
        assert_eq!(page.radio.as_deref(), Some("RDEMradio"));
        assert_eq!(page.songs_playlist.as_deref(), Some("OLAK5uy_songs"));
    }

    /// Opens an artist, signed out, then the artist's radio and all of their songs; a failure
    /// says what came back: `cargo test -- --ignored opens_an_artist`.
    #[tokio::test]
    #[ignore = "asks YouTube Music"]
    async fn opens_an_artist() {
        let rp = rustypipe::client::RustyPipe::builder()
            .no_storage()
            .build()
            .expect("client");
        let tile = |kind, id: &str| Tile {
            kind,
            id: id.to_string(),
            title: String::new(),
            subtitle: String::new(),
            image_url: None,
        };
        // Ado.
        let artist = youtube::open(
            rp.clone(),
            tile(TileKind::Artist, "UCln9P4Qm3-EAY4aiEPmRwEA"),
        )
        .await
        .expect("artist")
        .collection;
        let page = artist.artist.as_deref().expect("an artist page");
        let found = format!(
            "{}: {} songs, banner {:?}, albums {}, singles {}, playlists {}, similar {}, \
             songs {:?}, radio {:?}, about {} chars",
            artist.title,
            artist.tracks.len(),
            page.banner_url,
            page.albums.len(),
            page.singles.len(),
            page.playlists.len(),
            page.similar.len(),
            page.songs_playlist,
            page.radio,
            page.description.as_ref().map_or(0, String::len),
        );
        assert!(
            !artist.tracks.is_empty() && !page.albums.is_empty(),
            "{found}"
        );
        let radio = page
            .radio
            .clone()
            .unwrap_or_else(|| panic!("no radio: {found}"));
        let radio = youtube::open(rp.clone(), tile(TileKind::Radio, &radio))
            .await
            .expect("radio")
            .collection;
        assert!(!radio.tracks.is_empty(), "the radio is empty: {found}");
        let songs =
            (page.songs_playlist.clone()).unwrap_or_else(|| panic!("no songs playlist: {found}"));
        let songs = youtube::open(rp, tile(TileKind::Playlist, &songs))
            .await
            .expect("songs")
            .collection;
        assert!(
            songs.tracks.len() > artist.tracks.len(),
            "all songs: {}; {found}",
            songs.tracks.len()
        );
    }
}
