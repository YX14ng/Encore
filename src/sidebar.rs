//! The library sidebar on the left.

use iced::widget::{Column, button, column, container, row, space, text};
use iced::{Element, Fill, border};

use crate::mock::Playlist;
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
    NewPlaylist,
    LikedMusic,
    Playlist(usize),
}

/// What the sidebar remembers: the selected entry and which sections are open.
pub struct SidebarState {
    pub selected: NavItem,
    music_open: bool,
    library_open: bool,
    playlists_open: bool,
}

impl SidebarState {
    pub fn new() -> Self {
        Self {
            selected: NavItem::Home,
            music_open: true,
            library_open: true,
            playlists_open: true,
        }
    }

    pub fn is_open(&self, section: Section) -> bool {
        match section {
            Section::Music => self.music_open,
            Section::Library => self.library_open,
            Section::Playlists => self.playlists_open,
        }
    }

    pub fn toggle(&mut self, section: Section) {
        match section {
            Section::Music => self.music_open = !self.music_open,
            Section::Library => self.library_open = !self.library_open,
            Section::Playlists => self.playlists_open = !self.playlists_open,
        }
    }
}

type Entry = (NavItem, &'static str, &'static [u8]);

const MUSIC: [Entry; 3] = [
    (
        NavItem::Home,
        "Home",
        include_bytes!("../assets/icons/home.svg"),
    ),
    (
        NavItem::Explore,
        "Explore",
        include_bytes!("../assets/icons/explore.svg"),
    ),
    (
        NavItem::Radio,
        "Radio & mixes",
        include_bytes!("../assets/icons/radio.svg"),
    ),
];

const LIBRARY: [Entry; 6] = [
    (
        NavItem::History,
        "History",
        include_bytes!("../assets/icons/history.svg"),
    ),
    (
        NavItem::Songs,
        "Songs",
        include_bytes!("../assets/icons/songs.svg"),
    ),
    (
        NavItem::Albums,
        "Albums",
        include_bytes!("../assets/icons/albums.svg"),
    ),
    (
        NavItem::Artists,
        "Artists",
        include_bytes!("../assets/icons/artists.svg"),
    ),
    (
        NavItem::Subscriptions,
        "Subscriptions",
        include_bytes!("../assets/icons/subscriptions.svg"),
    ),
    (
        NavItem::Podcasts,
        "Podcasts",
        include_bytes!("../assets/icons/podcasts.svg"),
    ),
];

const PLAYLIST_ACTIONS: [Entry; 2] = [
    (
        NavItem::NewPlaylist,
        "New playlist",
        include_bytes!("../assets/icons/plus.svg"),
    ),
    (
        NavItem::LikedMusic,
        "Liked music",
        include_bytes!("../assets/icons/like.svg"),
    ),
];

pub fn view<'a>(state: &'a SidebarState, playlists: &'a [Playlist]) -> Element<'a, Message> {
    let mut content = column![search_box()].spacing(2);

    content = content.push(section_header("YouTube Music", Section::Music));
    if state.is_open(Section::Music) {
        content = content.push(entries(&MUSIC, state.selected));
    }

    content = content.push(section_header("Library", Section::Library));
    if state.is_open(Section::Library) {
        content = content.push(entries(&LIBRARY, state.selected));
    }

    content = content.push(section_header("Playlists", Section::Playlists));
    if state.is_open(Section::Playlists) {
        content = content.push(entries(&PLAYLIST_ACTIONS, state.selected));
        for (index, playlist) in playlists.iter().enumerate() {
            let item = NavItem::Playlist(index);
            let swatch = container(space())
                .width(18)
                .height(18)
                .style(move |_| container::background(playlist.color).border(border::rounded(4)));
            content = content.push(nav_button(
                swatch.into(),
                &playlist.name,
                item,
                state.selected,
            ));
        }
    }

    container(content)
        .width(WIDTH)
        .height(Fill)
        .padding([12, 10])
        .style(|_| theme::panel(theme::SIDEBAR))
        .into()
}

fn search_box() -> Element<'static, Message> {
    container(row![
        text("Search").color(theme::TEXT_DIM),
        space::horizontal(),
        theme::icon(
            include_bytes!("../assets/icons/search.svg"),
            theme::TEXT_DIM,
            18.0,
        )
    ])
    .padding([7, 10])
    .style(|_| {
        container::background(iced::Color::from_rgba8(255, 255, 255, 0.05))
            .border(border::rounded(8).color(theme::LINE).width(1))
    })
    .into()
}

fn section_header(label: &'static str, section: Section) -> Element<'static, Message> {
    let title = text(label.to_uppercase())
        .size(11)
        .color(theme::TEXT_SECONDARY);
    let chevron = theme::icon(
        include_bytes!("../assets/icons/chevron-down.svg"),
        theme::TEXT_SECONDARY,
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
    for (item, label, icon_bytes) in list {
        let glyph = theme::icon(icon_bytes, theme::ACCENT, 18.0);
        entries = entries.push(nav_button(glyph.into(), label, *item, selected));
    }
    entries
}

fn nav_button<'a>(
    glyph: Element<'a, Message>,
    label: &'a str,
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
