//! The signed-in user's library: history, saved songs, liked songs, albums, artists and
//! playlists. Also the account screen where the user signs in or out.

use iced::font::Weight;
use iced::widget::{Row, button, column, scrollable, text, text_input};
use iced::{Element, Fill, Font, border};
use rustypipe::client::RustyPipe;

use crate::account::{self, BROWSERS, Browser};
use crate::search::{self, Shared};
use crate::youtube::{self, SearchTrack, Tile};
use crate::{Message, theme};

/// Everything the library screens show.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibraryData {
    pub history: Vec<SearchTrack>,
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
    SignedIn(LibraryData),
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
}

/// Loads every part of the library at once. Parts that fail come back empty.
pub async fn load(rp: RustyPipe) -> Result<LibraryData, String> {
    let q = rp.query();
    let (history, songs, liked, albums, artists, playlists) = tokio::join!(
        q.music_history(),
        q.music_saved_tracks(),
        q.music_liked_tracks(),
        q.music_saved_albums(),
        q.music_saved_artists(),
        q.music_saved_playlists(),
    );
    if let (Err(error), Err(_), Err(_), Err(_), Err(_), Err(_)) =
        (&history, &songs, &liked, &albums, &artists, &playlists)
    {
        return Err(error.to_string());
    }

    Ok(LibraryData {
        history: history
            .map(|page| {
                page.items
                    .iter()
                    .map(|entry| youtube::track(&entry.item))
                    .collect()
            })
            .unwrap_or_default(),
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
        playlists: playlists
            .map(|page| page.items.iter().map(youtube::playlist_tile).collect())
            .unwrap_or_default(),
    })
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

/// Restores the session saved by an earlier sign-in, if it is still valid.
pub async fn restore(rp: RustyPipe) -> Result<LibraryData, String> {
    rp.user_auth_check_cookie()
        .await
        .map_err(|error| error.to_string())?;
    load(rp).await
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
    shared: Shared<'a>,
) -> Element<'a, Message> {
    let title = match shelf {
        Shelf::History => "History",
        Shelf::Songs => "Songs",
        Shelf::Liked => "Liked music",
        Shelf::Albums => "Albums",
        Shelf::Artists => "Artists",
    };

    let body: Element<'a, Message> = match account {
        Account::SignedIn(data) => match shelf {
            Shelf::History => search::track_list(&data.history, None, shared),
            Shelf::Songs => search::track_list(&data.songs, None, shared),
            Shelf::Liked => search::track_list(&data.liked, None, shared),
            Shelf::Albums => search::card_grid(&data.albums, shared),
            Shelf::Artists => search::card_grid(&data.artists, shared),
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
) -> Element<'a, Message> {
    let mut content = column![heading("Your YouTube Music account")]
        .spacing(18)
        .padding([28, 32])
        .max_width(720);

    match account {
        Account::SignedIn(_) => {
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

fn accent_button(label: &str, message: Message) -> Element<'_, Message> {
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

fn note<'a>(message: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    text(message).color(theme::TEXT_SECONDARY).into()
}
