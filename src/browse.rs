//! YouTube Music pages that rustypipe skips: the artists of the user's library (each opens a
//! library page, which rustypipe does not know, so it drops them), the mixes YouTube Music makes
//! for the user, and the saved podcasts. They are read straight from YouTube's answer, as the
//! cards it shows, and turned into Encore's tiles.

use rustypipe::client::{ClientType, RustyPipe, RustyPipeQuery};
use serde_json::{Value, json};

use crate::youtube::{self, Tile, TileKind};

/// The widest picture worth asking for (the cards are 160 px).
const IMAGE_SIZE: u32 = 544;

/// The pages, by browse id.
const LIBRARY_ARTISTS: &str = "FEmusic_library_corpus_track_artists";
const MIXES: &str = "FEmusic_mixed_for_you";
const PODCASTS: &str = "FEmusic_library_non_music_audio_list";

/// One card of a page.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Card {
    /// What the card opens, like `VLRDTMAK5uy_…` (a playlist) or `MPLAUC…` (a library artist).
    browse_id: String,
    title: String,
    subtitle: String,
    image_url: Option<String>,
}

/// The parts of the library rustypipe has no call for.
#[derive(Debug, Default)]
pub struct Extras {
    /// The artists of the songs in the library.
    pub artists: Vec<Tile>,
    /// "Mixed for you": My Supermix, Discover Mix, the mood mixes…
    pub mixes: Vec<Tile>,
    /// Saved podcasts, and the "Episodes for later" playlist.
    pub podcasts: Vec<Tile>,
}

/// Loads the extras of the signed-in user's library, up to `limit` of each. A part that fails
/// comes back empty, like the rest of the library.
pub async fn extras(rp: &RustyPipe, limit: usize) -> Extras {
    let q = rp.query().authenticated();
    let (artists, mixes, podcasts) = tokio::join!(
        tiles(&q, LIBRARY_ARTISTS, limit),
        tiles(&q, MIXES, limit),
        tiles(&q, PODCASTS, limit),
    );
    Extras {
        artists: artists.unwrap_or_default(),
        mixes: mixes.unwrap_or_default(),
        podcasts: podcasts.unwrap_or_default(),
    }
}

/// The cards of page `browse_id` that Encore can open, following its next pages.
async fn tiles(q: &RustyPipeQuery, browse_id: &str, limit: usize) -> Result<Vec<Tile>, String> {
    let mut answer = request(q, json!({ "browseId": browse_id })).await?;
    let mut tiles = Vec::new();
    loop {
        let (cards, next) = read_cards(&answer);
        tiles.extend(cards.into_iter().filter_map(tile));
        let Some(token) = next.filter(|_| tiles.len() < limit) else {
            break;
        };
        // A later page that fails keeps what already came.
        match request(q, json!({ "continuation": token })).await {
            Ok(more) => answer = more,
            Err(_) => break,
        }
    }
    tiles.truncate(limit);
    Ok(tiles)
}

async fn request(q: &RustyPipeQuery, body: Value) -> Result<Value, String> {
    let answer = q
        .raw(ClientType::DesktopMusic, "browse", &body)
        .await
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&answer).map_err(|error| format!("YouTube's answer is not JSON: {error}"))
}

/// The cards in an answer, in order, and the token of the next page, if there is one.
fn read_cards(answer: &Value) -> (Vec<Card>, Option<String>) {
    let mut cards = Vec::new();
    let mut next = None;
    walk(answer, &mut cards, &mut next);
    (cards, next)
}

fn walk(value: &Value, cards: &mut Vec<Card>, next: &mut Option<String>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let card = match key.as_str() {
                    "musicTwoRowItemRenderer" => grid_card(value),
                    "musicResponsiveListItemRenderer" => list_card(value),
                    "nextContinuationData" => {
                        if let Some(token) = value["continuation"].as_str() {
                            next.get_or_insert_with(|| token.to_string());
                        }
                        None
                    }
                    _ => {
                        walk(value, cards, next);
                        None
                    }
                };
                cards.extend(card);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| walk(item, cards, next)),
        _ => {}
    }
}

/// A card of a grid (like the mixes).
fn grid_card(item: &Value) -> Option<Card> {
    Some(Card {
        browse_id: browse_id(item)?,
        title: runs(&item["title"]).filter(|title| !title.is_empty())?,
        subtitle: runs(&item["subtitle"]).unwrap_or_default(),
        image_url: image(&item["thumbnailRenderer"]),
    })
}

/// A row of a list (like the library's artists), shown as a card.
fn list_card(item: &Value) -> Option<Card> {
    let columns = item["flexColumns"].as_array()?;
    let column = |index: usize| {
        columns
            .get(index)
            .and_then(|column| runs(&column["musicResponsiveListItemFlexColumnRenderer"]["text"]))
    };
    let subtitle = (1..columns.len())
        .filter_map(column)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" \u{b7} ");
    Some(Card {
        browse_id: browse_id(item)?,
        title: column(0).filter(|title| !title.is_empty())?,
        subtitle,
        image_url: image(&item["thumbnail"]),
    })
}

fn browse_id(item: &Value) -> Option<String> {
    item.pointer("/navigationEndpoint/browseEndpoint/browseId")?
        .as_str()
        .map(str::to_string)
}

/// The text of a `{"runs": [{"text": …}, …]}`.
fn runs(text: &Value) -> Option<String> {
    let runs = text["runs"].as_array()?;
    Some(runs.iter().filter_map(|run| run["text"].as_str()).collect())
}

/// The best picture of a `musicThumbnailRenderer`, made larger when Google gives only small
/// ones (the library's artists come at 120 px).
fn image(thumbnail: &Value) -> Option<String> {
    let thumbnails = thumbnail
        .pointer("/musicThumbnailRenderer/thumbnail/thumbnails")?
        .as_array()?;
    let sizes = thumbnails.iter().filter_map(|thumbnail| {
        let width = thumbnail["width"].as_u64().unwrap_or(0) as u32;
        Some((width, thumbnail["url"].as_str()?))
    });
    let widest = sizes.clone().map(|(width, _)| width).max()?;
    let url = youtube::pick_cover(sizes, IMAGE_SIZE)?;
    Some(if widest < IMAGE_SIZE / 2 {
        resized(&url, IMAGE_SIZE)
    } else {
        url
    })
}

/// A googleusercontent picture at `size` px: their size is the `=w…-h…` ending of the address.
fn resized(url: &str, size: u32) -> String {
    let resizable = url.contains("googleusercontent.com/") || url.contains("ggpht.com/");
    match url.rfind("=w") {
        Some(end) if resizable => format!("{}=w{size}-h{size}-p-l90-rj", &url[..end]),
        _ => url.to_string(),
    }
}

/// The tile for a card, or `None` for what Encore cannot open (or already shows elsewhere).
fn tile(card: Card) -> Option<Tile> {
    let id = &card.browse_id;
    let (kind, id) = if let Some(channel) = id.strip_prefix("MPLA") {
        // A library artist's page: the artist is the channel after the prefix.
        (TileKind::Artist, channel)
    } else if id.starts_with("UC") {
        (TileKind::Artist, id.as_str())
    } else if id.starts_with("MPRE") {
        (TileKind::Album, id.as_str())
    } else if let Some(podcast) = id.strip_prefix("MPSP") {
        (TileKind::Playlist, podcast)
    } else {
        // Liked Music has its own entry in the sidebar.
        let playlist = id.strip_prefix("VL").filter(|playlist| *playlist != "LM")?;
        (TileKind::Playlist, playlist)
    };
    Some(Tile {
        kind,
        id: id.to_string(),
        title: card.title,
        subtitle: card.subtitle,
        image_url: card.image_url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thumbnails(width: u32, url: &str) -> Value {
        json!({ "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
            { "url": url, "width": width, "height": width }
        ] } } })
    }

    #[test]
    fn reads_grid_cards_list_rows_and_the_next_page() {
        let answer = json!({ "contents": [
            { "gridRenderer": { "items": [
                { "musicTwoRowItemRenderer": {
                    "title": { "runs": [{ "text": "My Supermix" }] },
                    "subtitle": { "runs": [{ "text": "Ado" }, { "text": ", " }, { "text": "Eve" }] },
                    "navigationEndpoint": { "browseEndpoint": { "browseId": "VLRDTMAK5uy_x" } },
                    "thumbnailRenderer": thumbnails(600, "https://music.youtube.com/image/mixart?r=a"),
                } },
                { "musicTwoRowItemRenderer": {
                    "title": { "runs": [{ "text": "Liked Music" }] },
                    "navigationEndpoint": { "browseEndpoint": { "browseId": "VLLM" } },
                } },
            ] } },
            { "musicShelfRenderer": {
                "contents": [{ "musicResponsiveListItemRenderer": {
                    "flexColumns": [
                        { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "Ado" }] } } },
                        { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": "23 songs" }] } } },
                    ],
                    "navigationEndpoint": { "browseEndpoint": { "browseId": "MPLAUC5DHwv9N4OQ9AWtKr8YSefw" } },
                    "thumbnail": thumbnails(120, "https://lh3.googleusercontent.com/abc=w120-h120-p-l90-rj"),
                } }],
                "continuations": [{ "nextContinuationData": { "continuation": "next-page" } }],
            } },
        ] });
        let (cards, next) = read_cards(&answer);
        assert_eq!(next.as_deref(), Some("next-page"));
        let tiles: Vec<Tile> = cards.into_iter().filter_map(tile).collect();
        assert_eq!(tiles.len(), 2, "Liked Music is left out");
        assert_eq!(tiles[0].kind, TileKind::Playlist);
        assert_eq!(tiles[0].id, "RDTMAK5uy_x");
        assert_eq!(tiles[0].subtitle, "Ado, Eve");
        assert_eq!(tiles[1].kind, TileKind::Artist);
        assert_eq!(tiles[1].id, "UC5DHwv9N4OQ9AWtKr8YSefw");
        assert_eq!(tiles[1].subtitle, "23 songs");
        assert_eq!(
            tiles[1].image_url.as_deref(),
            Some("https://lh3.googleusercontent.com/abc=w544-h544-p-l90-rj"),
            "small artist pictures are asked for larger"
        );
    }

    #[test]
    fn only_google_pictures_are_resized() {
        let mix = "https://music.youtube.com/image/mixart?r=a=w";
        assert_eq!(resized(mix, 544), mix);
    }

    /// Reads the signed-in user's library artists, mixes and podcasts, and opens the first mix
    /// and "Episodes for later" (nothing changes on the account):
    /// `cargo test -- --ignored reads_the_library_extras`.
    #[tokio::test]
    #[ignore = "reads the signed-in user's YouTube Music library"]
    async fn reads_the_library_extras() {
        let rp = youtube::client().expect("client");
        let extras = extras(&rp, 200).await;
        let names = |tiles: &[Tile]| {
            tiles
                .iter()
                .take(3)
                .map(|tile| format!("{} ({})", tile.title, tile.id))
                .collect::<Vec<_>>()
        };
        let found = format!(
            "artists {}: {:?}; mixes {}: {:?}; podcasts {}: {:?}",
            extras.artists.len(),
            names(&extras.artists),
            extras.mixes.len(),
            names(&extras.mixes),
            extras.podcasts.len(),
            names(&extras.podcasts),
        );
        assert!(
            !extras.mixes.is_empty() && !extras.artists.is_empty(),
            "{found}"
        );
        let mix = youtube::open(rp.clone(), extras.mixes[0].clone())
            .await
            .unwrap_or_else(|error| panic!("{found}; the first mix: {error}"))
            .collection;
        assert!(!mix.tracks.is_empty(), "{found}; {} is empty", mix.title);
        if let Some(later) = extras.podcasts.iter().find(|tile| tile.id == "SE") {
            youtube::open(rp, later.clone())
                .await
                .unwrap_or_else(|error| panic!("{found}; Episodes for later: {error}"));
        }
    }
}
