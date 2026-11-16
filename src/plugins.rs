//! Plugins, Pear-style: optional features the user switches on and off from the settings.
//! The choices are kept in `~/.config/encore/plugins.json`.

use std::borrow::Cow;

use iced::Element;
use iced::font::Weight;
use iced::widget::{column, text, toggler};
use rust_i18n::t;
use serde::{Deserialize, Serialize};

use crate::themes::config_dir;
use crate::{Message, theme};

/// The optional features.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plugin {
    Discord,
    LastFm,
    Notifications,
    SponsorBlock,
    SkipDisliked,
    Visualizer,
    Romanize,
}

impl Plugin {
    pub const ALL: [Plugin; 7] = [
        Plugin::Discord,
        Plugin::LastFm,
        Plugin::Notifications,
        Plugin::SponsorBlock,
        Plugin::SkipDisliked,
        Plugin::Visualizer,
        Plugin::Romanize,
    ];

    fn name(self) -> Cow<'static, str> {
        match self {
            Plugin::Discord => t!("plugins.discord_name"),
            Plugin::LastFm => t!("plugins.lastfm_name"),
            Plugin::Notifications => t!("plugins.notifications_name"),
            Plugin::SponsorBlock => t!("plugins.sponsorblock_name"),
            Plugin::SkipDisliked => t!("plugins.skip_disliked_name"),
            Plugin::Visualizer => t!("plugins.visualizer_name"),
            Plugin::Romanize => t!("plugins.romanize_name"),
        }
    }

    fn description(self) -> Cow<'static, str> {
        match self {
            Plugin::Discord => t!("plugins.discord_description"),
            Plugin::LastFm => t!("plugins.lastfm_description"),
            Plugin::Notifications => t!("plugins.notifications_description"),
            Plugin::SponsorBlock => t!("plugins.sponsorblock_description"),
            Plugin::SkipDisliked => t!("plugins.skip_disliked_description"),
            Plugin::Visualizer => t!("plugins.visualizer_description"),
            Plugin::Romanize => t!("plugins.romanize_description"),
        }
    }
}

/// Which plugins are on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Plugins {
    pub discord: bool,
    pub lastfm: bool,
    pub notifications: bool,
    pub sponsorblock: bool,
    pub skip_disliked: bool,
    pub visualizer: bool,
    /// Japanese, Korean and Chinese lyrics written out in the Latin alphabet too.
    pub romanize: bool,
}

impl Default for Plugins {
    fn default() -> Self {
        Self {
            discord: true,
            lastfm: true,
            notifications: false,
            sponsorblock: true,
            skip_disliked: true,
            visualizer: true,
            romanize: true,
        }
    }
}

impl Plugins {
    /// Reads the saved choices; a missing or broken file gives the defaults.
    pub fn load() -> Self {
        std::fs::read_to_string(config_dir().join("plugins.json"))
            .ok()
            .and_then(|json| serde_json::from_str::<Self>(&json).ok())
            .unwrap_or_default()
    }

    /// Saves the choices. A failed write only means they are not remembered next time.
    pub fn save(&self) {
        let dir = config_dir();
        if std::fs::create_dir_all(&dir).is_ok()
            && let Ok(json) = serde_json::to_string_pretty(self)
        {
            let _ = std::fs::write(dir.join("plugins.json"), json);
        }
    }

    pub fn is_on(&self, plugin: Plugin) -> bool {
        match plugin {
            Plugin::Discord => self.discord,
            Plugin::LastFm => self.lastfm,
            Plugin::Notifications => self.notifications,
            Plugin::SponsorBlock => self.sponsorblock,
            Plugin::SkipDisliked => self.skip_disliked,
            Plugin::Visualizer => self.visualizer,
            Plugin::Romanize => self.romanize,
        }
    }

    pub fn set(&mut self, plugin: Plugin, on: bool) {
        let slot = match plugin {
            Plugin::Discord => &mut self.discord,
            Plugin::LastFm => &mut self.lastfm,
            Plugin::Notifications => &mut self.notifications,
            Plugin::SponsorBlock => &mut self.sponsorblock,
            Plugin::SkipDisliked => &mut self.skip_disliked,
            Plugin::Visualizer => &mut self.visualizer,
            Plugin::Romanize => &mut self.romanize,
        };
        *slot = on;
    }

    /// The plugins tab of the settings: a switch and a line about each plugin.
    pub fn view(&self) -> Element<'_, Message> {
        column(Plugin::ALL.iter().map(|&plugin| {
            column![
                toggler(self.is_on(plugin))
                    .label(plugin.name())
                    .font(iced::Font {
                        weight: Weight::Bold,
                        ..theme::INTER
                    })
                    .on_toggle(move |on| Message::SetPlugin(plugin, on))
                    .style(theme::switch),
                text(plugin.description())
                    .size(13)
                    .color(theme::text_secondary()),
            ]
            .spacing(6)
            .into()
        }))
        .spacing(22)
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switches_each_plugin_on_its_own() {
        let mut plugins = Plugins::default();
        for plugin in Plugin::ALL {
            let before = plugins.is_on(plugin);
            plugins.set(plugin, !before);
            assert_eq!(plugins.is_on(plugin), !before);
            let others = Plugin::ALL.iter().filter(|other| **other != plugin);
            assert!(
                others
                    .clone()
                    .all(|other| plugins.is_on(*other) == Plugins::default().is_on(*other))
            );
            plugins.set(plugin, before);
        }
    }

    #[test]
    fn older_files_keep_the_defaults_for_new_plugins() {
        let plugins = serde_json::from_str::<Plugins>(r#"{"discord": false}"#).expect("json");
        assert!(!plugins.discord);
        assert!(plugins.sponsorblock);
        assert!(plugins.skip_disliked);
    }
}
