use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use gpui_kit::assets::IconName;
use swofty_core::catalog::requires::Event;
use swofty_core::catalog::{self, Addon, Stage};
use swofty_i18n::{tr, trf};

use super::*;
use crate::format;

/// An addon, the version to install and the profile to add it to.
pub(super) type Request = (Addon, catalog::Version, String);

pub(super) type QueuedJob = crate::download_queue::Job<Request>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Progress {
    Downloading { done: u64, size: u64, rate: f64 },
    Verifying,
    Installing,
}

impl Progress {
    pub(super) fn fraction(self) -> f32 {
        match self {
            Self::Downloading { done, size, .. } if size > 0 => (done as f32 / size as f32).min(1.),
            Self::Downloading { .. } => 0.,
            Self::Verifying | Self::Installing => 1.,
        }
    }

    pub(super) fn short(self) -> String {
        match self {
            Self::Downloading { size: 0, .. } => tr("Downloading…").to_owned(),
            Self::Downloading { .. } => trf!(
                "Downloading {percent}",
                percent = format::percent(self.fraction())
            ),
            Self::Verifying => tr("Verifying…").to_owned(),
            Self::Installing => tr("Installing…").to_owned(),
        }
    }

    /// What it's doing, naming `item` when that's a requirement, not the addon itself.
    pub(super) fn long(self, item: Option<&str>) -> String {
        match self {
            Self::Downloading { done, size, rate } => {
                let amount = if size == 0 {
                    format::size(done)
                } else {
                    trf!(
                        "{done} of {size} · {percent}",
                        done = format::size(done),
                        size = format::size(size),
                        percent = format::percent(self.fraction())
                    )
                };
                let text = match item {
                    Some(item) => trf!("Downloading {item} · {amount}", item = item, amount = amount),
                    None => trf!("Downloading {amount}", amount = amount),
                };
                if rate > 0. {
                    trf!("{text} · {rate}/s", text = text, rate = format::size(rate as u64))
                } else {
                    text
                }
            }
            Self::Verifying => match item {
                Some(item) => trf!("Verifying {item}'s size and SHA-256…", item = item),
                None => tr("Verifying the download's size and SHA-256…").to_owned(),
            },
            Self::Installing => match item {
                Some(item) => trf!("Installing {item}…", item = item),
                None => tr("Installing…").to_owned(),
            },
        }
    }
}

/// An install's downloads so far: every addon it fetches, requirements included.
#[derive(Default)]
pub(super) struct Tally {
    pub(super) finished: u64,
    pub(super) current: u64,
    pub(super) size: u64,
    pub(super) stage: u8,
    pub(super) item: Option<String>,
    pub(super) also: Vec<String>,
    pub(super) installed: Vec<String>,
}

#[derive(Clone)]
pub(super) struct Job {
    pub(super) tally: Arc<Mutex<Tally>>,
    pub(super) started: Instant,
}

impl Job {
    pub(super) fn new(size: u64) -> Self {
        Self {
            tally: Arc::new(Mutex::new(Tally {
                size,
                ..Tally::default()
            })),
            started: Instant::now(),
        }
    }

    pub(super) fn tally(&self) -> std::sync::MutexGuard<'_, Tally> {
        self.tally.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn progress(&self) -> Progress {
        let t = self.tally();
        match t.stage {
            VERIFYING => Progress::Verifying,
            INSTALLING => Progress::Installing,
            _ => {
                let done = t.finished + t.current;
                let secs = self.started.elapsed().as_secs_f64();
                Progress::Downloading {
                    done,
                    size: t.size,
                    rate: if secs > 0.3 { done as f64 / secs } else { 0. },
                }
            }
        }
    }

    /// The requirement it's on, when it isn't on the addon itself.
    pub(super) fn item(&self) -> Option<String> {
        self.tally().item.clone()
    }

    /// The other addons it installs, once they're known.
    pub(super) fn also(&self) -> Vec<String> {
        self.tally().also.clone()
    }

    pub(super) fn failure(&self, error: &str) -> String {
        let installed = self.tally().installed.clone();
        if installed.is_empty() {
            error.to_owned()
        } else {
            trf!(
                "{error} · Already installed: {names}",
                error = error,
                names = installed.join(", ")
            )
        }
    }

    pub(super) fn track(&self, root: &str, e: Event) {
        let mut t = self.tally();
        match e {
            Event::Planned(plan) => {
                t.size = plan.size();
                t.also = plan.also().iter().map(|s| s.label()).collect();
            }
            Event::Addon(a, _) => {
                t.finished += t.current;
                t.current = 0;
                t.stage = 0;
                t.item = (a.id != root).then(|| a.name.clone());
            }
            Event::Stage(Stage::Downloading(n)) => t.current = n,
            Event::Stage(Stage::Verifying) => t.stage = VERIFYING,
            Event::Stage(Stage::Installing) => t.stage = INSTALLING,
            Event::Installed(m) if m.id != root => t.installed.push(format!("{} {}", m.name, m.version)),
            Event::Installed(_) => {}
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Offer {
    Install,
    Installed(semver::Version),
    /// Installed, but not one of this profile's addons.
    Update(semver::Version, semver::Version),
    /// A newer version the play bar doesn't offer: the player skipped it.
    UpdateHere(semver::Version, semver::Version),
    /// An import has this id, so the Addon Browser doesn't install over it.
    Separate(semver::Version),
    TooOld,
    Queued,
    Working(Progress),
    Failed,
    /// Not installed, and its requirements can't all be met.
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Op {
    Catalog,
    Stars,
    Install,
    Details,
    Profile,
}

impl Op {
    pub(super) fn icon(self) -> IconName {
        match self {
            Self::Catalog => IconName::RefreshCw,
            Self::Stars => IconName::Star,
            Self::Install => IconName::Download,
            Self::Details => IconName::Info,
            Self::Profile => IconName::UserRound,
        }
    }
}

pub(super) struct Activity {
    pub(super) at: SystemTime,
    pub(super) op: Op,
    pub(super) ok: bool,
    pub(super) title: String,
    pub(super) detail: String,
    pub(super) took: Duration,
}

#[derive(Default)]
pub(super) struct Batch {
    pub(super) left: BTreeSet<String>,
    pub(super) updated: Vec<String>,
    pub(super) failed: Vec<(String, String)>,
}

impl Batch {
    pub(super) fn total(&self) -> usize {
        self.left.len() + self.updated.len() + self.failed.len()
    }

    pub(super) fn summary(&self) -> String {
        let mut lines = Vec::new();
        if !self.updated.is_empty() {
            lines.push(trf!("Updated {names}", names = self.updated.join(", ")));
        }
        lines.extend(
            self.failed
                .iter()
                .map(|(name, e)| trf!("Couldn't update {name}: {error}", name = name, error = e)),
        );
        lines.join("\n")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Verb {
    Install,
    Update,
}

impl Verb {
    pub(super) fn of(updating: bool) -> Self {
        if updating { Self::Update } else { Self::Install }
    }

    pub(super) fn action(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Update => "update",
        }
    }

    pub(super) fn done(self, name: &str, version: &semver::Version) -> String {
        match self {
            Self::Install => trf!("Installed {name} {version}", name = name, version = version),
            Self::Update => trf!("Updated {name} {version}", name = name, version = version),
        }
    }

    pub(super) fn failed(self, name: &str, version: &semver::Version) -> String {
        match self {
            Self::Install => trf!(
                "Couldn't install {name} {version}",
                name = name,
                version = version
            ),
            Self::Update => trf!("Couldn't update {name} {version}", name = name, version = version),
        }
    }

    pub(super) fn failure_title(self, name: &str, refused: bool) -> String {
        match (self, refused) {
            (Self::Install, true) => trf!("Can't install {name}: missing requirements", name = name),
            (Self::Update, true) => trf!("Can't update {name}: missing requirements", name = name),
            (Self::Install, false) => trf!("Could not install {name}", name = name),
            (Self::Update, false) => trf!("Could not update {name}", name = name),
        }
    }
}
