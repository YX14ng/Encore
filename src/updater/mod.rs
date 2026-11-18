//! Updating Encore from inside it. At start (unless turned off in Settings → About) Encore asks
//! GitHub for its latest release; when there is a newer one whose package for this system is
//! already published, a banner offers to update. Updating downloads the package, checks its
//! SHA-256 and installs it with the system's package manager (asking for the password), or runs
//! the Windows installer; then Encore offers to restart. Installs it cannot update itself (the
//! `.tar.gz`, the Windows `.zip`) get the release page instead.

mod github;
mod install;

use iced::widget::{button, column, container, row, text, toggler};
use iced::{Center, Element, Fill, Task};
use rust_i18n::t;
use serde::{Deserialize, Serialize};

pub use github::{CURRENT, Release};
use install::{Kind, Outcome};

use crate::{App, Message, theme};

/// The update settings, in `updates.json` next to the others.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Look for a new version each time Encore starts.
    pub check_at_start: bool,
    /// A version the user chose to skip: it is not offered at start again.
    pub skipped: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            check_at_start: true,
            skipped: None,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        // Tests start from the defaults, whatever the user chose.
        if cfg!(test) {
            return Self::default();
        }
        std::fs::read_to_string(path())
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        // Tests must never touch the user's settings.
        if cfg!(test) {
            return;
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let path = path();
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, json);
        }
    }
}

fn path() -> std::path::PathBuf {
    crate::platform::config_dir().join("updates.json")
}

/// Where updating stands.
#[derive(Debug, Clone, Default)]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    UpToDate,
    /// A newer version, and how this Encore updates.
    Available(Release, Kind),
    Installing(Release),
    /// Installed: restart to use it.
    Installed(Release),
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum Event {
    /// Look for a new version; `asked` when the user pressed the button (a skipped version is
    /// offered then, and errors are shown).
    Check {
        asked: bool,
    },
    Checked {
        asked: bool,
        result: Result<(Release, Kind), String>,
    },
    Install,
    Installed(Result<Outcome, String>),
    Restart,
    /// Hides the banner until Encore starts again.
    Later,
    /// Never offers this version at start again.
    Skip,
    /// Opens the release's page with its notes.
    Notes,
    ToggleCheckAtStart(bool),
}

/// The updater's state.
#[derive(Debug, Default)]
pub struct Updater {
    pub settings: Settings,
    pub phase: Phase,
    /// The banner was put away for this run.
    hidden: bool,
}

impl Updater {
    pub fn new() -> Self {
        Self {
            settings: Settings::load(),
            ..Self::default()
        }
    }
}

/// Asks GitHub for the latest release and finds how this Encore updates.
async fn check() -> Result<(Release, Kind), String> {
    let release = github::latest().await?;
    Ok((release, install::detect().await))
}

impl App {
    /// The check at start, unless turned off.
    pub(crate) fn check_for_update_at_start(&mut self) -> Task<Message> {
        if !self.updater.settings.check_at_start || cfg!(test) {
            return Task::none();
        }
        self.update_event(Event::Check { asked: false })
    }

    pub(crate) fn update_event(&mut self, event: Event) -> Task<Message> {
        let updater = &mut self.updater;
        match event {
            Event::Check { asked } => {
                updater.phase = Phase::Checking;
                updater.hidden = false;
                Task::perform(check(), move |result| {
                    Message::Update(Event::Checked { asked, result })
                })
            }
            Event::Checked { asked, result } => {
                updater.phase = match result {
                    Ok((release, kind)) => offer(release, kind, asked, &updater.settings),
                    Err(error) if asked => Phase::Failed(error),
                    // A failed check at start (offline, GitHub busy) is not worth a word.
                    Err(_) => Phase::Idle,
                };
                Task::none()
            }
            Event::Install => match std::mem::take(&mut updater.phase) {
                Phase::Available(release, Kind::Manual) => {
                    let _ = crate::platform::open_url(&release.page);
                    updater.phase = Phase::Available(release, Kind::Manual);
                    Task::none()
                }
                Phase::Available(release, kind) => {
                    let Some(asset) = install::asset_for(&release, kind).cloned() else {
                        updater.phase = Phase::Available(release, kind);
                        return Task::none();
                    };
                    updater.phase = Phase::Installing(release);
                    Task::perform(install::install(asset, kind), |result| {
                        Message::Update(Event::Installed(result))
                    })
                }
                other => {
                    updater.phase = other;
                    Task::none()
                }
            },
            Event::Installed(result) => {
                let release = match std::mem::take(&mut updater.phase) {
                    Phase::Installing(release) => release,
                    other => {
                        updater.phase = other;
                        return Task::none();
                    }
                };
                match result {
                    Ok(Outcome::Installed) => {
                        updater.phase = Phase::Installed(release);
                        Task::none()
                    }
                    // The Windows installer replaces Encore and starts it again.
                    Ok(Outcome::InstallerStarted) => iced::exit(),
                    Err(error) => {
                        updater.phase = Phase::Failed(error);
                        Task::none()
                    }
                }
            }
            Event::Restart => match install::restart() {
                Ok(()) => iced::exit(),
                Err(error) => {
                    updater.phase = Phase::Failed(error.to_string());
                    Task::none()
                }
            },
            Event::Later => {
                updater.hidden = true;
                Task::none()
            }
            Event::Skip => {
                if let Phase::Available(release, _) = &updater.phase {
                    updater.settings.skipped = Some(release.version.to_string());
                    updater.settings.save();
                }
                updater.hidden = true;
                Task::none()
            }
            Event::Notes => {
                if let Phase::Available(release, _) | Phase::Installed(release) = &updater.phase {
                    let _ = crate::platform::open_url(&release.page);
                }
                Task::none()
            }
            Event::ToggleCheckAtStart(on) => {
                updater.settings.check_at_start = on;
                updater.settings.save();
                Task::none()
            }
        }
    }
}

/// What a check found: a version to offer, or nothing newer worth a word. A version whose
/// package for this system is not published yet waits for the next check; a skipped one is
/// only offered when the user asked.
fn offer(release: Release, kind: Kind, asked: bool, settings: &Settings) -> Phase {
    let ready = kind == Kind::Manual || install::asset_for(&release, kind).is_some();
    let skipped = settings.skipped.as_deref() == Some(release.version.to_string().as_str());
    if install::is_newer(&release) && ready && (asked || !skipped) {
        Phase::Available(release, kind)
    } else {
        Phase::UpToDate
    }
}

/// The banner over the center column while an update is offered, installing or installed.
pub fn banner(updater: &Updater) -> Option<Element<'_, Message>> {
    if updater.hidden {
        return None;
    }
    let (message, buttons): (String, Vec<Element<'_, Message>>) = match &updater.phase {
        Phase::Available(release, kind) => {
            let install = if *kind == Kind::Manual {
                t!("update.download")
            } else {
                t!("update.install")
            };
            (
                t!(
                    "update.available",
                    version = release.version.to_string(),
                    current = CURRENT
                )
                .into_owned(),
                vec![
                    pill(install.into_owned(), Event::Install, true),
                    pill(t!("update.notes").into_owned(), Event::Notes, false),
                    pill(t!("update.later").into_owned(), Event::Later, false),
                    pill(t!("update.skip").into_owned(), Event::Skip, false),
                ],
            )
        }
        Phase::Installing(release) => (
            t!("update.installing", version = release.version.to_string()).into_owned(),
            Vec::new(),
        ),
        Phase::Installed(release) => (
            t!("update.installed", version = release.version.to_string()).into_owned(),
            vec![
                pill(t!("update.restart").into_owned(), Event::Restart, true),
                pill(t!("update.later").into_owned(), Event::Later, false),
            ],
        ),
        _ => return None,
    };
    let row = row![text(message).size(13).width(Fill)]
        .extend(buttons)
        .spacing(10)
        .align_y(Center)
        .padding([10, 24]);
    Some(
        container(row)
            .width(Fill)
            .style(|_| container::background(theme::tint(0.08)))
            .into(),
    )
}

/// The About tab of the settings: the version, the check at start and a button to check now.
pub fn about(updater: &Updater) -> Element<'_, Message> {
    let status: Option<String> = match &updater.phase {
        Phase::Idle => None,
        Phase::Checking => Some(t!("update.checking").into_owned()),
        Phase::UpToDate => Some(t!("update.up_to_date").into_owned()),
        Phase::Available(release, _) => Some(
            t!(
                "update.available",
                version = release.version.to_string(),
                current = CURRENT
            )
            .into_owned(),
        ),
        Phase::Installing(release) => {
            Some(t!("update.installing", version = release.version.to_string()).into_owned())
        }
        Phase::Installed(release) => {
            Some(t!("update.installed", version = release.version.to_string()).into_owned())
        }
        Phase::Failed(error) => Some(t!("update.failed", error = error).into_owned()),
    };
    let available = matches!(updater.phase, Phase::Available(..));
    column![
        text("Encore").size(22),
        text(t!("update.version", version = CURRENT))
            .size(13)
            .color(theme::text_secondary()),
        toggler(updater.settings.check_at_start)
            .label(t!("update.check_at_start"))
            .size(18)
            .text_size(14)
            .on_toggle(|on| Message::Update(Event::ToggleCheckAtStart(on)))
            .style(theme::switch),
        row![pill(
            t!("update.check_now").into_owned(),
            Event::Check { asked: true },
            false
        )]
        .push(available.then(|| pill(t!("update.install").into_owned(), Event::Install, true)))
        .spacing(10),
    ]
    .push(status.map(|status| text(status).size(13).color(theme::text_secondary())))
    .spacing(14)
    .into()
}

/// A round button of the banner; `main` is the accented one.
fn pill<'a>(label: String, event: Event, main: bool) -> Element<'a, Message> {
    button(text(label).size(13))
        .on_press(Message::Update(event))
        .padding([6, 14])
        .style(move |_, status| {
            let mut style = theme::flat_button(status, false);
            if main {
                style.background = Some(theme::accent().into());
            }
            style.text_color = theme::text_primary();
            style.border = iced::border::rounded(16).color(theme::line()).width(1);
            style
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::github::{Asset, Version};
    use super::*;

    fn release(version: Version) -> Release {
        Release {
            version,
            page: "https://x".to_string(),
            assets: vec![Asset {
                name: format!("encore-ytm-{version}-1-x86_64.pkg.tar.zst"),
                url: "https://x/a".to_string(),
                sha256: Some("00".to_string()),
            }],
        }
    }

    #[test]
    fn offers_only_newer_ready_unskipped_versions() {
        let settings = Settings::default();
        let newer = release(Version(99, 0, 0));
        assert!(matches!(
            offer(newer.clone(), Kind::Arch, false, &settings),
            Phase::Available(..)
        ));
        assert!(
            matches!(
                offer(newer.clone(), Kind::Rpm, false, &settings),
                Phase::UpToDate
            ),
            "its package is not out yet"
        );
        assert!(matches!(
            offer(release(Version(0, 0, 1)), Kind::Arch, false, &settings),
            Phase::UpToDate
        ));
        let skipped = Settings {
            skipped: Some("99.0.0".to_string()),
            ..Settings::default()
        };
        assert!(matches!(
            offer(newer.clone(), Kind::Arch, false, &skipped),
            Phase::UpToDate
        ));
        assert!(
            matches!(
                offer(newer, Kind::Arch, true, &skipped),
                Phase::Available(..)
            ),
            "asking shows it anyway"
        );
    }

    #[test]
    fn later_and_skip_put_the_banner_away() {
        let mut app = App::new();
        app.updater.phase = Phase::Available(release(Version(99, 0, 0)), Kind::Arch);
        assert!(banner(&app.updater).is_some());
        let _ = app.update_event(Event::Skip);
        assert!(banner(&app.updater).is_none());
        assert_eq!(app.updater.settings.skipped.as_deref(), Some("99.0.0"));
    }
}
