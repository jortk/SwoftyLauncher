use std::path::{Path, PathBuf};

use swofty_core::launcher_saves;
use swofty_core::profile::VANILLA;
use swofty_core::vault::careers::{Diff, Listed};
use swofty_core::vault::{self, Location, Snapshot};

use super::banner_data::{Banner, banner_from};
use super::steam::source_of_profile;

#[derive(Default)]
pub(crate) struct SavesPage {
    pub view: Option<SavesView>,
    pub confirm: Option<String>,
    pub banner: Option<Banner>,
    pub game_running: bool,
    pub(super) notice_done: bool,
    pub(super) generation: u64,
    pub loading: bool,
    pub banner_profile: String,
    pub(super) print: Option<u64>,
}

pub(crate) struct Built {
    pub(super) print: u64,
    pub(super) banner: Option<Banner>,
    pub(super) running: bool,
    pub(super) view: SavesView,
}

pub(super) struct Version {
    pub(super) snap: Snapshot,
    pub(super) held: Option<Vec<String>>,
    pub(super) diff: Option<Diff>,
    pub(super) newest: Option<String>,
    pub(super) missing: usize,
}

pub(super) struct Entry {
    pub(super) l: Listed,
    pub(super) place: Option<String>,
}

impl Entry {
    pub(super) fn mode(&self) -> String {
        self.l.mode_label()
    }
}

pub(super) struct Imported {
    pub(super) title: String,
    pub(super) at: Option<i64>,
}

pub(crate) struct SavesView {
    pub(super) profile: String,
    pub(super) loc: Option<Location>,
    pub(super) entries: Vec<Entry>,
    pub(super) imported: Option<Imported>,
    pub(super) has_save: bool,
    pub(super) others: Vec<String>,
    pub(super) versions: Vec<Version>,
    pub(super) dir: PathBuf,
    pub(super) usage: u64,
    pub(super) cap: u64,
    pub(super) steam: Option<Location>,
    pub(super) steam_running: bool,
    pub(super) awaiting_import: bool,
    pub(super) steam_missing: bool,
    pub(super) steam_warning: bool,
}

impl SavesView {
    pub(crate) fn awaiting_import_for(&self, profile: &str) -> bool {
        self.awaiting_import && self.profile == profile
    }

    pub(super) fn vanilla(&self) -> bool {
        self.profile == VANILLA
    }

    pub(super) fn shared_by_every_profile(&self) -> bool {
        self.loc
            .as_ref()
            .is_some_and(|l| !launcher_saves::supported(l.layout))
    }

    pub(super) fn layout(&self) -> vault::Layout {
        self.loc
            .as_ref()
            .map(|l| l.layout)
            .or_else(|| self.versions.first().map(|v| v.snap.layout))
            .unwrap_or(vault::Layout::Launcher)
    }
}

impl SavesPage {
    pub(crate) fn banner_for(root: &Path, profile: &str) -> Option<Banner> {
        banner_from(root, source_of_profile(root, profile)?, profile.to_owned())
    }

    pub(crate) fn refresh_due(&self, shown: bool, print: Option<u64>) -> bool {
        shown && !self.loading && print.is_some_and(|p| self.print != Some(p))
    }

    pub(super) fn apply(&mut self, built: Built) {
        self.loading = false;
        self.print = Some(built.print);
        self.banner_profile = built.view.profile.clone();
        self.banner = built.banner;
        self.game_running = built.running;
        self.view = Some(built.view);
    }

    pub(crate) fn take_notice(&mut self, root: &Path) -> bool {
        if self.notice_done || launcher_saves::marker(root).is_none() {
            return false;
        }
        self.notice_done = true;
        let due = launcher_saves::notice_due(root);
        if due && let Err(e) = launcher_saves::mark_seen(root) {
            crate::app_log::action("record the saves notice").failed(&e);
        }
        due
    }
}
