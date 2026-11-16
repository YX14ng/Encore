//! The signed-in user's library: history, saved songs, liked songs, albums, artists and
//! playlists. Also the account screen where the user signs in or out.

use iced::font::Weight;
use iced::widget::{Row, button, column, scrollable, text, text_input};
use iced::{Element, Fill, Font, border};
use rust_i18n::t;
use rustypipe::client::RustyPipe;
use serde::{Deserialize, Serialize};

use crate::account::{self, BROWSERS, Browser};
use crate::playlists::edit;
use crate::profile::{self, Profile};
use crate::search::{self, Shared};
use crate::virtual_list::Scroll;
use crate::youtube::{self, Collection, SearchTrack, Tile, TileKind};
use crate::{Message, browse, theme};

/// Everything the library screens show. Saved to disk so the next start shows it at once.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryData {
    /// Who is signed in.
    pub profile: Option<Profile>,
    pub history: Vec<SearchTrack>,
    /// Radios built from the history, for the home screen.
    pub mixes: Vec<Tile>,
    pub songs: Vec<SearchTrack>,
    pub liked: Vec<SearchTrack>,
    pub albums: Vec<Tile>,
    /// The artists the user subscribed to.
    pub artists: Vec<Tile>,
    pub playlists: Vec<Tile>,
    /// The artists of the songs in the library.
    #[serde(default)]
    pub library_artists: Vec<Tile>,
    /// The mixes YouTube Music makes for the user ("Mixed for you").
    #[serde(default)]
    pub made_for_you: Vec<Tile>,
    /// Saved podcasts and "Episodes for later".
    #[serde(default)]
    pub podcasts: Vec<Tile>,
    /// The ids of the playlists the user made, which songs can be added to.
    #[serde(default)]
    pub own_playlists: Vec<String>,
}

impl LibraryData {
    /// Whether playlist `id` is one the user made.
    pub fn is_own(&self, id: &str) -> bool {
        let id = crate::playlists::edit::bare_id(id);
        self.own_playlists.iter().any(|own| own == id)
    }

    /// The images the home screen and the sidebar show: the latest songs, the shelves of
    /// cards and the profile photo. The library screens ask for theirs when opened
    /// (`shelf_image_urls`), so a large library does not download thousands at start.
    pub fn home_image_urls(&self) -> Vec<String> {
        let tracks = self.history.iter().take(crate::home::RECENT_SONGS);
        let tiles = self.mixes.iter().chain(&self.playlists).chain(&self.albums);
        tracks
            .filter_map(|track| track.cover_url.clone())
            .chain(tiles.filter_map(|tile| tile.image_url.clone()))
            .chain(
                self.profile
                    .iter()
                    .filter_map(|profile| profile.photo_url.clone()),
            )
            .collect()
    }

    /// The songs of a library screen that is a list of songs.
    pub fn shelf_tracks(&self, shelf: Shelf) -> Option<&[SearchTrack]> {
        match shelf {
            Shelf::History => Some(&self.history),
            Shelf::Songs => Some(&self.songs),
            Shelf::Liked => Some(&self.liked),
            _ => None,
        }
    }

    /// The images of library screen `shelf` when scrolled to `scroll`: the covers of the
    /// songs in sight, or every card's image.
    /// `order` is the songs' order on screen (a search or another order), if not as they are.
    pub fn shelf_image_urls(
        &self,
        shelf: Shelf,
        scroll: Scroll,
        order: Option<&[usize]>,
    ) -> Vec<String> {
        if let Some(tracks) = self.shelf_tracks(shelf) {
            let shown = order.map_or(tracks.len(), <[usize]>::len);
            let rows = shelf_rows_in_sight(scroll, shown);
            let songs: Vec<usize> = match order {
                Some(order) => order[rows].to_vec(),
                None => rows.collect(),
            };
            return songs
                .into_iter()
                .filter_map(|song| tracks.get(song)?.cover_url.clone())
                .collect();
        }
        let tiles: &[Tile] = match shelf {
            Shelf::Albums => &self.albums,
            Shelf::Artists if self.library_artists.is_empty() => &self.artists,
            Shelf::Artists => &self.library_artists,
            Shelf::Subscriptions => &self.artists,
            Shelf::Podcasts => &self.podcasts,
            Shelf::Mixes => &self.made_for_you,
            _ => &[],
        };
        tiles
            .iter()
            .chain(if shelf == Shelf::Mixes {
                &self.mixes[..]
            } else {
                &[]
            })
            .filter_map(|tile| tile.image_url.clone())
            .collect()
    }
}

/// The scrollable of the library screens, scrolled back to the top when another opens.
pub const SHELF_SCROLL_ID: &str = "shelf";
/// Where a library screen's song list starts in its scrollable: the padding, the heading, the
/// Play and Shuffle buttons, the search and order bar and the spaces between them (see
/// `virtual_list`).
const SHELF_LIST_TOP: f32 = 28.0 + 40.0 + 18.0 + 36.0 + 18.0 + 38.0 + 18.0;

/// The songs of a library list in sight when scrolled to `scroll`.
pub fn shelf_rows_in_sight(scroll: Scroll, count: usize) -> std::ops::Range<usize> {
    search::song_rows_in_sight(scroll, SHELF_LIST_TOP, count)
}

/// Where the account is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Account {
    /// Checking a saved session at startup.
    Checking,
    SignedOut,
    SigningIn,
    /// Waiting for the user to sign in in Encore's sign-in window.
    Waiting,
    SignedIn(Box<LibraryData>),
    Failed(String),
}

/// The library screens reachable from the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shelf {
    History,
    Songs,
    Liked,
    Albums,
    Artists,
    Subscriptions,
    Podcasts,
    Mixes,
    Downloads,
}

/// How many songs of the history to load.
const HISTORY_LIMIT: usize = 500;
/// How many saved and liked songs to load: all of them in practice, so playing them shuffled
/// does not leave the older ones out. It loads in the background, with the saved library on
/// screen meanwhile (about three seconds for a thousand songs).
const SONG_LIMIT: usize = 20_000;
/// How many albums, artists or playlists to load.
const CARD_LIMIT: usize = 200;
/// How many mixes the home screen offers.
const MIX_COUNT: usize = 6;

/// Loads every part of the library at once, following each list past its first page. Parts that
/// fail come back empty; a part that fails on a later page keeps what it already had.
pub async fn load(rp: RustyPipe) -> Result<LibraryData, String> {
    let q = rp.query();
    let (
        profile,
        extras,
        (mut history, mut songs, mut liked, mut albums, mut artists, mut playlists),
    ) = tokio::join!(
        profile::fetch(&rp),
        browse::extras(&rp, CARD_LIMIT),
        async {
            tokio::join!(
                q.music_history(),
                q.music_saved_tracks(),
                q.music_liked_tracks(),
                q.music_saved_albums(),
                q.music_saved_artists(),
                q.music_saved_playlists(),
            )
        }
    );
    if let (Err(error), Err(_), Err(_), Err(_), Err(_), Err(_)) =
        (&history, &songs, &liked, &albums, &artists, &playlists)
    {
        return Err(error.to_string());
    }

    // The history paginator only knows how to fetch one more page at a time.
    let more_history = async {
        if let Ok(page) = &mut history {
            while page.items.len() < HISTORY_LIMIT && matches!(page.extend(&q).await, Ok(true)) {}
        }
    };
    let more_songs = async {
        if let Ok(page) = &mut songs {
            let _ = page.extend_limit(&q, SONG_LIMIT).await;
        }
    };
    let more_liked = async {
        if let Ok(playlist) = &mut liked {
            let _ = playlist.tracks.extend_limit(&q, SONG_LIMIT).await;
        }
    };
    let more_albums = async {
        if let Ok(page) = &mut albums {
            let _ = page.extend_limit(&q, CARD_LIMIT).await;
        }
    };
    let more_artists = async {
        if let Ok(page) = &mut artists {
            let _ = page.extend_limit(&q, CARD_LIMIT).await;
        }
    };
    let more_playlists = async {
        if let Ok(page) = &mut playlists {
            let _ = page.extend_limit(&q, CARD_LIMIT).await;
        }
    };
    tokio::join!(
        more_history,
        more_songs,
        more_liked,
        more_albums,
        more_artists,
        more_playlists
    );

    let history: Vec<SearchTrack> = history
        .map(|page| {
            page.items
                .iter()
                .map(|entry| youtube::track(&entry.item))
                .collect()
        })
        .unwrap_or_default();

    let own_playlists: Vec<String> = playlists
        .as_ref()
        .map(|page| {
            page.items
                .iter()
                .filter(|playlist| edit::is_own(&playlist.id, playlist.channel.is_some()))
                .map(|playlist| playlist.id.clone())
                .collect()
        })
        .unwrap_or_default();
    // YouTube Music gives no owner for the user's own playlists; they are the user's.
    let owner = profile.as_ref().map(|profile| profile.name.clone());
    let playlists: Vec<Tile> = playlists
        .map(|page| {
            page.items
                .iter()
                // "LM" is Liked Music, which already has its own entry in the sidebar.
                .filter(|playlist| playlist.id != "LM")
                .map(|playlist| {
                    let mut tile = youtube::playlist_tile(playlist);
                    if let Some(owner) = &owner
                        && own_playlists.contains(&playlist.id)
                    {
                        tile.subtitle = t!("youtube.playlist_by", owner = owner).into_owned();
                    }
                    tile
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(LibraryData {
        profile,
        mixes: mixes_from(&history, MIX_COUNT),
        history,
        songs: songs
            .map(|page| page.items.iter().map(youtube::track).collect())
            .unwrap_or_default(),
        liked: liked
            .map(|playlist| playlist.tracks.items.iter().map(youtube::track).collect())
            .unwrap_or_default(),
        albums: albums
            .map(|page| page.items.iter().map(youtube::album_tile).collect())
            .unwrap_or_default(),
        artists: artists
            .map(|page| page.items.iter().map(youtube::artist_tile).collect())
            .unwrap_or_default(),
        playlists,
        own_playlists,
        library_artists: extras.artists,
        made_for_you: extras.mixes,
        podcasts: extras.podcasts,
    })
}

/// Mixes for the home screen: a radio for each recent song by a different artist, so the mixes
/// do not all sound alike. YouTube Music's own home feed is not available through rustypipe.
pub fn mixes_from(history: &[SearchTrack], count: usize) -> Vec<Tile> {
    let mut seen_artists = std::collections::HashSet::new();
    history
        .iter()
        .filter(|track| seen_artists.insert(track.artist.clone()))
        .take(count)
        .map(|track| Tile {
            kind: TileKind::Radio,
            id: track.id.clone(),
            title: t!("library.artist_mix", artist = track.artist).into_owned(),
            subtitle: t!("library.based_on", title = track.title).into_owned(),
            image_url: track.cover_url.clone(),
        })
        .collect()
}

/// Signs in with cookies (from a browser or a file) and loads the library.
pub async fn sign_in(
    rp: RustyPipe,
    cookies: Result<String, String>,
) -> Result<LibraryData, String> {
    let cookies = cookies?;
    rp.user_auth_set_cookie_txt(&cookies)
        .await
        .map_err(|error| error.to_string())?;
    load(rp).await
}

/// Reads the session from `browser` through yt-dlp, then signs in.
pub async fn sign_in_with_browser(rp: RustyPipe, browser: Browser) -> Result<LibraryData, String> {
    let cookies = account::read_from_browser(browser)
        .await
        .map_err(|error| error.to_string());
    sign_in(rp, cookies).await
}

/// Signs in with the session made in Encore's sign-in window (a `Cookie` header line).
pub async fn sign_in_with_cookie(rp: RustyPipe, cookie: String) -> Result<LibraryData, String> {
    rp.user_auth_set_cookie(cookie)
        .await
        .map_err(|error| error.to_string())?;
    load(rp).await
}

/// Why a saved session could not be restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreError {
    /// There is no saved session (never signed in, or signed out).
    NoSession,
    /// A session is saved but YouTube no longer accepts it.
    Expired,
    /// The session is saved but checking it failed, for example without a connection.
    Failed(String),
}

/// Restores the session saved by an earlier sign-in and reloads the library.
pub async fn restore(rp: RustyPipe) -> Result<LibraryData, RestoreError> {
    use rustypipe::error::{AuthError, Error};

    // rustypipe answers "not logged in" both without a saved session and when YouTube
    // refuses the saved one; only the second is an expired session.
    let saved = rp.user_auth_cookie_header().is_some();
    match rp.user_auth_check_cookie().await {
        Err(Error::Auth(AuthError::NoLogin)) if saved => return Err(RestoreError::Expired),
        Err(Error::Auth(AuthError::NoLogin)) => return Err(RestoreError::NoSession),
        Err(error) => return Err(RestoreError::Failed(error.to_string())),
        Ok(()) => {}
    }
    load(rp).await.map_err(RestoreError::Failed)
}

fn cache_path() -> std::path::PathBuf {
    account::data_dir().join("library.json")
}

/// The library saved by the last session, shown at startup while it reloads.
pub fn load_cache() -> Option<LibraryData> {
    let json = std::fs::read_to_string(cache_path()).ok()?;
    serde_json::from_str(&json).ok()
}

/// Saves the library for the next start. It holds the history, so it goes in the private
/// data folder.
pub fn save_cache(data: &LibraryData) {
    if account::private_dir(&account::data_dir()).is_err() {
        return;
    }
    if let Ok(json) = serde_json::to_string(data) {
        // Without the cache the next start just loads from the network.
        let _ = std::fs::write(cache_path(), json);
    }
}

/// Forgets the saved library (after signing out).
pub fn clear_cache() {
    let _ = std::fs::remove_file(cache_path());
}

/// Forgets the session.
pub async fn sign_out(rp: RustyPipe) {
    // If removing fails there is nothing left to do; the app shows "signed out" either way.
    let _ = rp.user_auth_remove_cookie().await;
}

/// A library screen: a list of songs or a grid of cards.
pub fn shelf_view<'a>(
    shelf: Shelf,
    account: &'a Account,
    downloads: &'a [Collection],
    shared: Shared<'a>,
    scroll: Scroll,
    songs: &'a crate::song_order::SongOrder,
) -> Element<'a, Message> {
    let title = match shelf {
        Shelf::History => t!("library.history"),
        Shelf::Songs => t!("library.songs"),
        Shelf::Liked => t!("library.liked"),
        Shelf::Albums => t!("library.albums"),
        Shelf::Artists => t!("library.artists"),
        Shelf::Subscriptions => t!("library.subscriptions"),
        Shelf::Podcasts => t!("library.podcasts"),
        Shelf::Mixes => t!("library.mixes"),
        Shelf::Downloads => t!("library.downloads"),
    };

    let body: Element<'a, Message> = match account {
        // Downloads work signed in or not, and without a connection.
        _ if shelf == Shelf::Downloads => search::offline_grid(downloads, shared),
        Account::SignedIn(data) => match shelf {
            Shelf::History | Shelf::Songs | Shelf::Liked => {
                let tracks = data.shelf_tracks(shelf).unwrap_or_default();
                let shown = songs.shown(tracks.len());
                let list: Element<'a, Message> = if !tracks.is_empty() && shown == 0 {
                    note(t!("songs.no_match"))
                } else {
                    let rows = shelf_rows_in_sight(scroll, shown);
                    search::long_track_list(tracks, songs.order(), None, rows, shared)
                };
                column![
                    crate::collection::play_buttons(tracks, songs.order()),
                    crate::song_order::toolbar(songs, tracks.len()),
                    list
                ]
                .spacing(18)
                .into()
            }
            Shelf::Albums => search::card_grid(&data.albums, shared),
            // A library saved before Encore read its artists has only the subscriptions.
            Shelf::Artists if data.library_artists.is_empty() => {
                search::card_grid(&data.artists, shared)
            }
            Shelf::Artists => search::card_grid(&data.library_artists, shared),
            Shelf::Subscriptions => search::card_grid(&data.artists, shared),
            Shelf::Podcasts => search::card_grid(&data.podcasts, shared),
            Shelf::Mixes => mixes(data, shared),
            Shelf::Downloads => search::offline_grid(downloads, shared),
        },
        Account::Checking | Account::SigningIn | Account::Waiting => note(t!("library.loading")),
        Account::SignedOut | Account::Failed(_) => column![
            note(t!("library.signed_out")),
            accent_button(t!("library.sign_in"), Message::OpenAccount),
        ]
        .spacing(12)
        .into(),
    };

    scrollable(column![heading(title), body].spacing(18).padding([28, 32]))
        .id(SHELF_SCROLL_ID)
        .on_scroll(|viewport| {
            Message::ShelfScrolled(Scroll {
                offset: viewport.absolute_offset().y,
                height: viewport.bounds().height,
            })
        })
        .height(Fill)
        .into()
}

/// The account screen: sign in through the default browser, pick another browser, import a
/// cookies file, or sign out.
pub fn account_view<'a>(
    account: &'a Account,
    session: Vec<String>,
    cookie_path: &'a str,
    photo: Option<iced::widget::image::Handle>,
) -> Element<'a, Message> {
    let mut content = column![heading(t!("library.account_title"))]
        .spacing(18)
        .padding([28, 32])
        .max_width(720);

    match account {
        Account::SignedIn(data) => {
            if let Some(profile) = &data.profile {
                let photo: Element<'a, Message> = match &photo {
                    Some(handle) => iced::widget::image(handle.clone())
                        .width(96)
                        .height(96)
                        .border_radius(48)
                        .into(),
                    None => iced::widget::container(iced::widget::space())
                        .width(96)
                        .height(96)
                        .style(|_| theme::panel(theme::hero()).border(border::rounded(48)))
                        .into(),
                };
                let names = column![text(&profile.name).size(22).font(Font {
                    weight: Weight::Bold,
                    ..theme::INTER
                })]
                .push(
                    profile
                        .handle
                        .as_deref()
                        .map(|handle| text(handle).color(theme::text_secondary())),
                )
                .spacing(4);
                content = content.push(
                    iced::widget::row![photo, names]
                        .spacing(20)
                        .align_y(iced::Center),
                );
            }
            content = content.push(note(t!("library.signed_in_note")));
            for line in session {
                content = content.push(note(line));
            }
            content = content.push(accent_button(t!("library.sign_out"), Message::SignOut));
        }
        Account::Checking | Account::SigningIn => {
            content = content.push(note(t!("library.signing_in")));
        }
        Account::Waiting => {
            content = content.push(note(t!("library.waiting")));
        }
        Account::SignedOut | Account::Failed(_) => {
            if let Account::Failed(error) = account {
                content = content
                    .push(text(t!("library.sign_in_failed", error = error)).color(theme::accent()));
            }
            let browsers = BROWSERS.iter().map(|&(browser, name)| {
                button(text(name).size(14))
                    .on_press(Message::SignInWith(browser))
                    .padding([8, 18])
                    .style(|_, status| {
                        let mut style = theme::flat_button(status, false);
                        style.border = border::rounded(18).color(theme::line()).width(1);
                        style
                    })
                    .into()
            });
            content = content
                .push(note(t!("library.sign_in_window_note")))
                .push(accent_button(t!("library.sign_in"), Message::SignInWindow))
                .push(note(t!("library.browser_note")))
                .push(Row::with_children(browsers).spacing(8))
                .push(note(t!("library.cookies_note")))
                .push(
                    iced::widget::row![
                        text_input("~/Downloads/cookies.txt", cookie_path)
                            .on_input(Message::CookiePathChanged)
                            .on_submit(Message::ImportCookies)
                            .padding([8, 12]),
                        accent_button(t!("library.import"), Message::ImportCookies),
                    ]
                    .spacing(8),
                );
        }
    }

    scrollable(content).height(Fill).into()
}

pub(crate) fn accent_button<'a>(
    label: impl text::IntoFragment<'a>,
    message: Message,
) -> Element<'a, Message> {
    button(text(label).size(14).color(theme::text_primary()))
        .on_press(message)
        .padding([8, 22])
        .style(|_, status| {
            let mut style = theme::flat_button(status, false);
            style.background = Some(theme::accent().into());
            style.border = border::rounded(18);
            style
        })
        .into()
}

/// "Radio & mixes": the mixes YouTube Music makes for the user, then radios from the history.
fn mixes<'a>(data: &'a LibraryData, shared: Shared<'a>) -> Element<'a, Message> {
    let mut sections = column![].spacing(28);
    for (key, tiles) in [
        ("library.mixed_for_you", &data.made_for_you),
        ("library.from_history", &data.mixes),
    ] {
        if !tiles.is_empty() {
            let title = text(t!(key)).size(18).font(Font {
                weight: Weight::Bold,
                ..theme::INTER
            });
            sections = sections.push(column![title, search::card_grid(tiles, shared)].spacing(12));
        }
    }
    if data.made_for_you.is_empty() && data.mixes.is_empty() {
        return note(t!("library.no_mixes"));
    }
    sections.into()
}

fn heading<'a>(label: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(label)
        .size(26)
        .font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        })
        .into()
}

pub(crate) fn note<'a>(message: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(message).color(theme::text_secondary()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(id: &str, artist: &str) -> SearchTrack {
        SearchTrack {
            id: id.to_string(),
            title: format!("Song {id}"),
            artist: artist.to_string(),
            album: String::new(),
            duration: 0,
            cover_url: None,
            entry: None,
        }
    }

    #[test]
    fn mixes_use_one_song_per_artist() {
        let history = [
            song("a", "Ado"),
            song("b", "Ado"),
            song("c", "Daft Punk"),
            song("d", "Aimer"),
        ];
        let mixes = mixes_from(&history, 2);
        assert_eq!(mixes.len(), 2);
        assert_eq!(mixes[0].id, "a");
        assert_eq!(mixes[0].title, "Ado Mix");
        assert_eq!(mixes[1].id, "c");
        assert!(mixes.iter().all(|mix| mix.kind == TileKind::Radio));
    }
}
