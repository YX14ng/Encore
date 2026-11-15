//! The library sidebar on the left.

use std::borrow::Cow;

use iced::widget::{Column, button, column, container, row, scrollable, space, text, text_input};
use iced::{Element, Fill, border};
use rust_i18n::t;

use crate::mock::Playlist;
use crate::profile::Profile;
use crate::youtube::Tile;
use crate::{Message, theme};

pub const WIDTH: f32 = 236.0;

/// The three foldable groups of the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Music,
    Library,
    Playlists,
}

/// Every entry that can be selected in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavItem {
    Home,
    Explore,
    Radio,
    History,
    Songs,
    Albums,
    Artists,
    Subscriptions,
    Podcasts,
    Downloads,
    NewPlaylist,
    LikedMusic,
    Playlist(usize),
    /// The account screen.
    Account,
    /// Nothing highlighted, while another screen is shown.
    None,
}

/// What the sidebar remembers: the selected entry and which sections are open.
pub struct SidebarState {
    pub selected: NavItem,
    music_open: bool,
    library_open: bool,
    playlists_open: bool,
}

impl SidebarState {
    /// Starts on Home with every section open.
    pub fn new() -> Self {
        Self {
            selected: NavItem::Home,
            music_open: true,
            library_open: true,
            playlists_open: true,
        }
    }

    /// Whether the entries of `section` are shown.
    pub fn is_open(&self, section: Section) -> bool {
        match section {
            Section::Music => self.music_open,
            Section::Library => self.library_open,
            Section::Playlists => self.playlists_open,
        }
    }

    /// Opens `section` if it is closed, and closes it if it is open.
    pub fn toggle(&mut self, section: Section) {
        match section {
            Section::Music => self.music_open = !self.music_open,
            Section::Library => self.library_open = !self.library_open,
            Section::Playlists => self.playlists_open = !self.playlists_open,
        }
    }
}

impl Default for SidebarState {
    fn default() -> Self {
        Self::new()
    }
}

/// One fixed entry of the sidebar: what it selects, the key of its label and its icon.
struct Entry {
    item: NavItem,
    label: &'static str,
    icon: &'static [u8],
}

const MUSIC: [Entry; 3] = [
    Entry {
        item: NavItem::Home,
        label: "sidebar.home",
        icon: include_bytes!("../assets/icons/home.svg"),
    },
    Entry {
        item: NavItem::Explore,
        label: "explore.title",
        icon: include_bytes!("../assets/icons/explore.svg"),
    },
    Entry {
        item: NavItem::Radio,
        label: "library.mixes",
        icon: include_bytes!("../assets/icons/radio.svg"),
    },
];

const LIBRARY: [Entry; 7] = [
    Entry {
        item: NavItem::History,
        label: "library.history",
        icon: include_bytes!("../assets/icons/history.svg"),
    },
    Entry {
        item: NavItem::Songs,
        label: "library.songs",
        icon: include_bytes!("../assets/icons/songs.svg"),
    },
    Entry {
        item: NavItem::Albums,
        label: "library.albums",
        icon: include_bytes!("../assets/icons/albums.svg"),
    },
    Entry {
        item: NavItem::Artists,
        label: "library.artists",
        icon: include_bytes!("../assets/icons/artists.svg"),
    },
    Entry {
        item: NavItem::Subscriptions,
        label: "library.subscriptions",
        icon: include_bytes!("../assets/icons/subscriptions.svg"),
    },
    Entry {
        item: NavItem::Podcasts,
        label: "library.podcasts",
        icon: include_bytes!("../assets/icons/podcasts.svg"),
    },
    Entry {
        item: NavItem::Downloads,
        label: "library.downloads",
        icon: include_bytes!("../assets/icons/download.svg"),
    },
];

const PLAYLIST_ACTIONS: [Entry; 2] = [
    Entry {
        item: NavItem::NewPlaylist,
        label: "sidebar.new_playlist",
        icon: include_bytes!("../assets/icons/plus.svg"),
    },
    Entry {
        item: NavItem::LikedMusic,
        label: "library.liked",
        icon: include_bytes!("../assets/icons/like.svg"),
    },
];

/// The search box and the three foldable sections, with the selected entry highlighted.
pub fn view<'a>(
    state: &'a SidebarState,
    playlists: &'a [Playlist],
    query: &'a str,
    suggestions: &'a [String],
    account: SidebarAccount<'a>,
) -> Element<'a, Message> {
    // Away from Home and the library (search results, an album) nothing is highlighted.
    let selected = if account.highlight {
        state.selected
    } else {
        NavItem::None
    };
    let mut content = column![search_box(query), suggestion_list(suggestions)].spacing(2);

    content = content.push(section_header("YouTube Music", Section::Music));
    if state.is_open(Section::Music) {
        content = content.push(entries(&MUSIC, selected));
    }

    content = content.push(section_header(&t!("sidebar.library"), Section::Library));
    if state.is_open(Section::Library) {
        content = content.push(entries(&LIBRARY, selected));
    }

    content = content.push(section_header(&t!("search.playlists"), Section::Playlists));
    if state.is_open(Section::Playlists) {
        content = content.push(entries(&PLAYLIST_ACTIONS, selected));
        match account.playlists {
            // Signed in: the user's own playlists.
            Some(user_playlists) => {
                for (index, playlist) in user_playlists.iter().enumerate() {
                    let glyph: Element<'a, Message> = match account
                        .covers
                        .get(crate::playlists::edit::bare_id(&playlist.id))
                    {
                        Some(cover) => iced::widget::image(cover.clone())
                            .width(18)
                            .height(18)
                            .content_fit(iced::ContentFit::Cover)
                            .border_radius(4)
                            .into(),
                        None => theme::icon(
                            include_bytes!("../assets/icons/queue.svg"),
                            theme::accent(),
                            18.0,
                        )
                        .into(),
                    };
                    let item = NavItem::Playlist(index);
                    content = content.push(nav_button(glyph, &playlist.title, item, selected));
                }
            }
            // Signed out: the sample playlists.
            None => {
                for (index, playlist) in playlists.iter().enumerate() {
                    let item = NavItem::Playlist(index);
                    let swatch = container(space()).width(18).height(18).style(move |_| {
                        container::background(playlist.color).border(border::rounded(4))
                    });
                    content =
                        content.push(nav_button(swatch.into(), &playlist.name, item, selected));
                }
            }
        }
    }

    let label = match (account.signed_in, account.profile) {
        (true, Some(profile)) => Cow::Borrowed(profile.name.as_str()),
        (true, None) => t!("sidebar.account"),
        (false, _) => t!("library.sign_in"),
    };
    let glyph: Element<'a, Message> = match account.photo {
        Some(photo) => iced::widget::image(photo)
            .width(22)
            .height(22)
            .border_radius(11)
            .into(),
        None => theme::icon(
            include_bytes!("../assets/icons/artists.svg"),
            theme::text_secondary(),
            18.0,
        )
        .into(),
    };
    let footer = nav_button(glyph, label, NavItem::Account, selected);

    container(column![scrollable(content).height(Fill), footer].spacing(6))
        .width(WIDTH)
        .height(Fill)
        .padding([12, 10])
        .style(|_| theme::panel(theme::sidebar_bg()))
        .into()
}

/// What the sidebar needs to know about the account.
pub struct SidebarAccount<'a> {
    pub signed_in: bool,
    /// Who is signed in, for the footer.
    pub profile: Option<&'a Profile>,
    /// The profile photo, once downloaded.
    pub photo: Option<iced::widget::image::Handle>,
    /// The user's playlists once signed in; the sample ones are shown otherwise.
    pub playlists: Option<&'a [Tile]>,
    /// The covers the user picked for playlists, by playlist id.
    pub covers: &'a std::collections::HashMap<String, iced::widget::image::Handle>,
    /// Whether the selected entry is on screen (Home or a library shelf).
    pub highlight: bool,
}

/// Search terms suggested while typing; clicking one searches for it.
fn suggestion_list(suggestions: &[String]) -> Element<'_, Message> {
    let rows = suggestions.iter().map(|term| {
        button(
            row![
                theme::icon(
                    include_bytes!("../assets/icons/search.svg"),
                    theme::text_dim(),
                    14.0,
                ),
                text(term).size(13),
            ]
            .spacing(10)
            .align_y(iced::Center),
        )
        .on_press(Message::PickSuggestion(term.clone()))
        .width(Fill)
        .padding([6, 10])
        .style(|_, status| theme::flat_button(status, false))
        .into()
    });
    Column::with_children(rows).padding([4, 0]).into()
}

/// The search field: typing updates the query and Enter runs the search.
fn search_box(query: &str) -> Element<'_, Message> {
    let input = text_input(&t!("sidebar.search"), query)
        .on_input(Message::SearchChanged)
        .on_submit(Message::Search)
        .padding(0)
        .size(14)
        .style(|_, _| text_input::Style {
            background: iced::Background::Color(iced::Color::TRANSPARENT),
            border: iced::Border::default(),
            icon: theme::text_dim(),
            placeholder: theme::text_dim(),
            value: theme::text_primary(),
            selection: theme::accent().scale_alpha(0.4),
        });

    container(
        row![
            input,
            theme::icon(
                include_bytes!("../assets/icons/search.svg"),
                theme::text_dim(),
                18.0,
            )
        ]
        .spacing(8)
        .align_y(iced::Center),
    )
    .padding([7, 10])
    .style(|_| {
        container::background(theme::tint(0.05))
            .border(border::rounded(8).color(theme::line()).width(1))
    })
    .into()
}

fn section_header(label: &str, section: Section) -> Element<'static, Message> {
    let title = text(label.to_uppercase())
        .size(11)
        .color(theme::text_secondary());
    let chevron = theme::icon(
        include_bytes!("../assets/icons/chevron-down.svg"),
        theme::text_secondary(),
        18.0,
    );

    button(row![title, space::horizontal(), chevron])
        .on_press(Message::ToggleSection(section))
        .width(Fill)
        .padding([10, 8])
        .style(|_, status| theme::flat_button(status, false))
        .into()
}

fn entries(list: &[Entry], selected: NavItem) -> Column<'static, Message> {
    let mut entries = column![].spacing(1);
    for entry in list {
        let glyph = theme::icon(entry.icon, theme::accent(), 18.0);
        entries = entries.push(nav_button(
            glyph.into(),
            t!(entry.label),
            entry.item,
            selected,
        ));
    }
    entries
}

fn nav_button<'a>(
    glyph: Element<'a, Message>,
    label: impl text::IntoFragment<'a>,
    item: NavItem,
    selected: NavItem,
) -> Element<'a, Message> {
    let is_selected = item == selected;

    button(row![glyph, text(label).size(14)].spacing(11))
        .on_press(Message::Select(item))
        .width(Fill)
        .padding([7, 10])
        .style(move |_, status| theme::flat_button(status, is_selected))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_on_home_with_every_section_open() {
        let state = SidebarState::new();
        assert_eq!(state.selected, NavItem::Home);
        assert!(state.is_open(Section::Music));
        assert!(state.is_open(Section::Library));
        assert!(state.is_open(Section::Playlists));
    }

    #[test]
    fn toggle_only_changes_that_section() {
        let mut state = SidebarState::new();
        state.toggle(Section::Library);
        assert!(!state.is_open(Section::Library));
        assert!(state.is_open(Section::Music));
        assert!(state.is_open(Section::Playlists));
        state.toggle(Section::Library);
        assert!(state.is_open(Section::Library));
    }
}
