use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::notices::Notification;
use gpui_kit::component::input::InputState;
use gpui_kit::*;
use swofty_core::dbedit::draft::DraftFile;
use swofty_core::dbedit::package::PackageInfo;
use swofty_core::dbedit::{
    Catalog, Draft, Edit, Edits, Hit, ImportReport, LinkSpec, Module, ModuleView, NodeId,
};

use super::fields::Slots;
use crate::shell::Shell;

pub(crate) enum Load {
    Idle,
    Loading,
    Ready(Arc<Catalog>),
    Failed(String),
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Nav {
    Module(String),
    Raw(NodeId),
    Unnamed,
    Changes,
}

pub(crate) struct Reveal {
    pub(super) module: String,
    pub(super) scope: Option<String>,
    pub(super) label: String,
}

pub(crate) struct ViewSlot {
    pub module: String,
    pub scope: Option<String>,
    pub view: Arc<ModuleView>,
    pub choices: Arc<HashMap<usize, Vec<(String, String)>>>,
    pub shown: Vec<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Which {
    Search,
    Filter,
    Unnamed,
    Raw,
}

pub(crate) struct Debounced {
    pub input: Entity<InputState>,
    pub query: String,
    pub(super) generation: u64,
    pub(super) _sub: Subscription,
}

pub(super) type GameId = (PathBuf, Option<u64>, Option<String>);

pub(super) fn game_id(install: &crate::game::Install) -> GameId {
    (install.dir.clone(), install.build_id, install.exe_version.clone())
}

pub(crate) struct Db {
    pub load: Load,
    pub game: Option<GameId>,
    pub modules: &'static [Module],
    /// The changes being edited; `draft.edits` is what every view shows.
    pub draft: Draft,
    pub(super) draft_read: bool,
    pub(super) file: Arc<Mutex<DraftFile>>,
    pub(super) persist_gen: u64,
    /// Changes not yet written to the draft file.
    pub(super) unwritten: bool,
    pub(super) writing: bool,
    /// Another launcher window wrote a newer draft; nothing is written until the player picks one.
    pub newer: bool,
    pub(super) others_gen: u64,
    pub(super) _quit: Subscription,
    pub(super) index: HashMap<u64, usize>,
    /// Why the draft's addon could not be read; editing stays off until it reads.
    pub addon_error: Option<String>,
    /// The DB Editor addons installed, from their manifests.
    pub addons: Vec<PackageInfo>,
    /// An addon to open once the player answers Save / Discard / Cancel.
    pub(super) pending_open: Option<String>,
    /// The addon file time last looked at, so a damaged file is read once per change.
    pub(super) seen: Option<Option<SystemTime>>,
    pub others: Arc<HashMap<u64, Vec<(String, String)>>>,
    pub toasts: Vec<Notification>,
    pub nav: Nav,
    pub scopes: HashMap<String, String>,
    pub view: Option<ViewSlot>,
    pub view_error: Option<String>,
    pub save_error: Option<String>,
    pub(super) view_want: Option<(String, Option<String>, u64)>,
    pub(super) stale_choices: bool,
    pub(super) rev: u64,
    pub row: Option<String>,
    pub max_cols: usize,
    pub notes_open: bool,
    pub reveal: Option<Reveal>,
    pub key: Option<NodeId>,
    pub errors: HashMap<u64, String>,
    pub search: Option<Debounced>,
    pub hits: Vec<Hit>,
    pub hits_open: bool,
    pub hit: usize,
    pub filter: Option<Debounced>,
    pub unnamed: Option<Debounced>,
    pub raw_filter: Option<Debounced>,
    pub raw_rows: Option<(NodeId, String, Arc<Vec<NodeId>>)>,
    pub(super) clear_raw: bool,
    pub unnamed_rows: Arc<Vec<NodeId>>,
    pub cell: Option<CellEdit>,
    pub slots: Slots,
    pub focus: Option<FocusHandle>,
    pub rows_scroll: UniformListScrollHandle,
    pub rows_motion: crate::scroll::SmoothScroll,
    pub list_scroll: UniformListScrollHandle,
    pub list_motion: crate::scroll::SmoothScroll,
    pub grid_scroll: ScrollHandle,
    pub(super) pending_tab: Option<(String, String, String)>,
    pub(super) pending_link: Option<LinkSpec>,
    pub importing: bool,
}

pub(crate) struct CellEdit {
    pub key: u64,
    pub input: Entity<InputState>,
    pub(super) _sub: Subscription,
}

impl Db {
    pub(crate) fn new(cx: &mut Context<Shell>) -> Self {
        Self {
            load: Load::Idle,
            game: None,
            modules: &[],
            draft: Draft::default(),
            draft_read: false,
            file: Arc::default(),
            persist_gen: 0,
            unwritten: false,
            writing: false,
            newer: false,
            others_gen: 0,
            _quit: cx.on_app_quit(|s: &mut Shell, _| {
                s.db_flush_now();
                async {}
            }),
            index: HashMap::new(),
            addon_error: None,
            addons: Vec::new(),
            pending_open: None,
            seen: None,
            others: Arc::new(HashMap::new()),
            toasts: Vec::new(),
            nav: Nav::Raw(0),
            scopes: HashMap::new(),
            view: None,
            view_error: None,
            save_error: None,
            view_want: None,
            stale_choices: false,
            rev: 0,
            row: None,
            max_cols: 8,
            notes_open: false,
            reveal: None,
            key: None,
            errors: HashMap::new(),
            search: None,
            hits: Vec::new(),
            hits_open: false,
            hit: 0,
            filter: None,
            unnamed: None,
            raw_filter: None,
            raw_rows: None,
            clear_raw: false,
            unnamed_rows: Arc::new(Vec::new()),
            cell: None,
            slots: Slots::default(),
            focus: None,
            rows_scroll: UniformListScrollHandle::new(),
            rows_motion: Default::default(),
            list_scroll: UniformListScrollHandle::new(),
            list_motion: Default::default(),
            grid_scroll: ScrollHandle::new(),
            pending_tab: None,
            pending_link: None,
            importing: false,
        }
    }

    pub(crate) fn catalog(&self) -> Option<&Arc<Catalog>> {
        match &self.load {
            Load::Ready(c) => Some(c),
            _ => None,
        }
    }

    pub(crate) fn edit(&self, key: u64) -> Option<&Edit> {
        self.index.get(&key).and_then(|&i| self.draft.edits.edits.get(i))
    }

    pub(crate) fn current(&self, key: u64) -> String {
        match self.edit(key) {
            Some(e) => e.value.clone().unwrap_or_default(),
            None => self
                .catalog()
                .and_then(|c| self.draft.edits.value(c, key))
                .map(str::to_owned)
                .unwrap_or_default(),
        }
    }

    pub(super) fn reindex(&mut self) {
        self.index = self
            .draft
            .edits
            .edits
            .iter()
            .enumerate()
            .map(|(i, e)| (e.key, i))
            .collect();
    }

    pub(crate) fn module(&self, id: &str) -> Option<&'static Module> {
        self.modules.iter().find(|m| m.id == id)
    }
}

/// What runs once the player answers Save / Discard / Cancel about unsaved changes.
#[derive(Clone)]
pub(crate) enum Then {
    Open(String),
    New,
    Load {
        file: String,
        name: String,
        edits: Edits,
        report: ImportReport,
    },
}
