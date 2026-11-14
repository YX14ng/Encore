//! The signed-in user's library: history, saved songs, liked songs, albums, artists and
//! playlists. Also the account screen where the user signs in or out.

use iced::font::Weight;
use iced::widget::{Row, button, column, scrollable, text, text_input};
use iced::{Element, Fill, Font, border};
use rustypipe::client::RustyPipe;
use serde::{Deserialize, Serialize};

use crate::account::{self, BROWSERS, Browser};
use crate::profile::{self, Profile};
use crate::search::{self, Shared};
use crate::youtube::{self, Collection, SearchTrack, Tile, TileKind};
use crate::{Message, theme};

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
    pub artists: Vec<Tile>,
    pub playlists: Vec<Tile>,
}

impl LibraryData {
    /// Every image the library screens can show.
    pub fn image_urls(&self) -> Vec<String> {
        let tracks = self.history.iter().chain(&self.songs).chain(&self.liked);
        let tiles = self
            .albums
            .iter()
            .chain(&self.artists)
            .chain(&self.playlists);
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
}

/// Where the account is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Account {
    /// Checking a saved session at startup.
    Checking,
    SignedOut,
    SigningIn,
    /// Waiting for the user to sign in in this browser.
    Waiting(Browser),
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
    Downloads,
}

/// How many songs to load from long lists (history, saved songs, liked music).
const SONG_LIMIT: usize = 500;
/// How many albums, artists or playlists to load.
const CARD_LIMIT: usize = 200;
/// How many mixes the home screen offers.
const MIX_COUNT: usize = 6;

/// Loads every part of the library at once, following each list past its first page. Parts that
/// fail come back empty; a part that fails on a later page keeps what it already had.
pub async fn load(rp: RustyPipe) -> Result<LibraryData, String> {
    let q = rp.query();
    let (profile, (mut history, mut songs, mut liked, mut albums, mut artists, mut playlists)) =
        tokio::join!(profile::fetch(&rp), async {
            tokio::join!(
                q.music_history(),
                q.music_saved_tracks(),
                q.music_liked_tracks(),
                q.music_saved_albums(),
                q.music_saved_artists(),
                q.music_saved_playlists(),
            )
        });
    if let (Err(error), Err(_), Err(_), Err(_), Err(_), Err(_)) =
        (&history, &songs, &liked, &albums, &artists, &playlists)
    {
        return Err(error.to_string());
    }

    // The history paginator only knows how to fetch one more page at a time.
    let more_history = async {
        if let Ok(page) = &mut history {
            while page.items.len() < SONG_LIMIT && matches!(page.extend(&q).await, Ok(true)) {}
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
        // "LM" is Liked Music, which already has its own entry in the sidebar.
        playlists: playlists
            .map(|page| {
                page.items
                    .iter()
                    .filter(|playlist| playlist.id != "LM")
                    .map(youtube::playlist_tile)
                    .collect()
            })
            .unwrap_or_default(),
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
            title: format!("{} Mix", track.artist),
            subtitle: format!("Based on {}", track.title),
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

/// How many times to look for the session while the user signs in, five seconds apart.
const SIGN_IN_ATTEMPTS: u32 = 36;

/// Waits for the user to sign in to YouTube Music in `browser` (opened by Encore), checking
/// every five seconds for up to three minutes, then signs in.
pub async fn wait_for_sign_in(rp: RustyPipe, browser: Browser) -> Result<LibraryData, String> {
    for _ in 0..SIGN_IN_ATTEMPTS {
        match account::read_from_browser(browser).await {
            Ok(cookies) => return sign_in(rp, Ok(cookies)).await,
            Err(account::ReadError::NoSession) => {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Err(format!(
        "no sign-in was detected in {} after three minutes",
        browser.name()
    ))
}

/// Why a saved session could not be restored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreError {
    /// There is no saved session (never signed in, or signed out).
    NoSession,
    /// The session is saved but checking it failed, for example without a connection.
    Failed(String),
}

/// Restores the session saved by an earlier sign-in and reloads the library.
pub async fn restore(rp: RustyPipe) -> Result<LibraryData, RestoreError> {
    use rustypipe::error::{AuthError, Error};

    match rp.user_auth_check_cookie().await {
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
) -> Element<'a, Message> {
    let title = match shelf {
        Shelf::History => "History",
        Shelf::Songs => "Songs",
        Shelf::Liked => "Liked music",
        Shelf::Albums => "Albums",
        Shelf::Artists => "Artists",
        Shelf::Downloads => "Downloads",
    };

    let body: Element<'a, Message> = match account {
        // Downloads work signed in or not, and without a connection.
        _ if shelf == Shelf::Downloads => search::offline_grid(downloads, shared),
        Account::SignedIn(data) => match shelf {
            Shelf::History => search::track_list(&data.history, None, shared),
            Shelf::Songs => search::track_list(&data.songs, None, shared),
            Shelf::Liked => search::track_list(&data.liked, None, shared),
            Shelf::Albums => search::card_grid(&data.albums, shared),
            Shelf::Artists => search::card_grid(&data.artists, shared),
            Shelf::Downloads => search::offline_grid(downloads, shared),
        },
        Account::Checking | Account::SigningIn | Account::Waiting(_) => {
            note("Loading your library\u{2026}")
        }
        Account::SignedOut | Account::Failed(_) => column![
            note("Sign in to see your library."),
            accent_button("Sign in", Message::OpenAccount),
        ]
        .spacing(12)
        .into(),
    };

    scrollable(column![heading(title), body].spacing(18).padding([28, 32]))
        .height(Fill)
        .into()
}

/// The account screen: sign in through the default browser, pick another browser, import a
/// cookies file, or sign out.
pub fn account_view<'a>(
    account: &'a Account,
    cookie_path: &'a str,
    default_browser: Option<Browser>,
    photo: Option<iced::widget::image::Handle>,
) -> Element<'a, Message> {
    let mut content = column![heading("Your YouTube Music account")]
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
                        .style(|_| theme::panel(theme::HERO).border(border::rounded(48)))
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
                        .map(|handle| text(handle).color(theme::TEXT_SECONDARY)),
                )
                .spacing(4);
                content = content.push(
                    iced::widget::row![photo, names]
                        .spacing(20)
                        .align_y(iced::Center),
                );
            }
            content = content
                .push(note(
                    "You are signed in. Your library and playlists are in the sidebar.",
                ))
                .push(accent_button("Sign out", Message::SignOut));
        }
        Account::Checking | Account::SigningIn => {
            content = content.push(note("Signing in\u{2026}"));
        }
        Account::Waiting(browser) => {
            content = content.push(note(format!(
                "YouTube Music is open in {}. Sign in there with your Google account; Encore \
                 notices on its own within a few seconds.",
                browser.name()
            )));
        }
        Account::SignedOut | Account::Failed(_) => {
            if let Account::Failed(error) = account {
                content =
                    content.push(text(format!("Could not sign in: {error}")).color(theme::ACCENT));
            }
            let browsers = BROWSERS.iter().map(|&(browser, name)| {
                button(text(name).size(14))
                    .on_press(Message::SignInWith(browser))
                    .padding([8, 18])
                    .style(|_, status| {
                        let mut style = theme::flat_button(status, false);
                        style.border = border::rounded(18).color(theme::LINE).width(1);
                        style
                    })
                    .into()
            });
            let main_label = match default_browser {
                Some(browser) => format!("Sign in with {}", browser.name()),
                None => "Sign in with your browser".to_string(),
            };
            content = content
                .push(note(
                    "Encore opens YouTube Music in your browser. Sign in there with your Google \
                     account and Encore picks up the session: only the YouTube cookies are kept, \
                     on this computer.",
                ))
                .push(accent_button_owned(main_label, Message::SignInDefault))
                .push(note(
                    "Already signed in somewhere else? Read the session from:",
                ))
                .push(Row::with_children(browsers).spacing(8))
                .push(note("Or import a cookies.txt exported from your browser:"))
                .push(
                    iced::widget::row![
                        text_input("~/Downloads/cookies.txt", cookie_path)
                            .on_input(Message::CookiePathChanged)
                            .on_submit(Message::ImportCookies)
                            .padding([8, 12]),
                        accent_button("Import", Message::ImportCookies),
                    ]
                    .spacing(8),
                );
        }
    }

    scrollable(content).height(Fill).into()
}

fn accent_button_owned(label: String, message: Message) -> Element<'static, Message> {
    button(text(label).size(14).color(theme::TEXT))
        .on_press(message)
        .padding([8, 22])
        .style(|_, status| {
            let mut style = theme::flat_button(status, false);
            style.background = Some(theme::ACCENT.into());
            style.border = border::rounded(18);
            style
        })
        .into()
}

pub(crate) fn accent_button(label: &str, message: Message) -> Element<'_, Message> {
    button(text(label).size(14).color(theme::TEXT))
        .on_press(message)
        .padding([8, 22])
        .style(|_, status| {
            let mut style = theme::flat_button(status, false);
            style.background = Some(theme::ACCENT.into());
            style.border = border::rounded(18);
            style
        })
        .into()
}

fn heading(label: &str) -> Element<'_, Message> {
    text(label)
        .size(26)
        .font(Font {
            weight: Weight::Bold,
            ..theme::INTER
        })
        .into()
}

pub(crate) fn note<'a>(message: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(message).color(theme::TEXT_SECONDARY).into()
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
