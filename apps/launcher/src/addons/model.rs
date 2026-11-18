use super::drag::DropAt;
use crate::library::{Changes, Source, Type};
use crate::scale::px;
use crate::shell::Shell;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::Instant;
use swofty_core::conflicts::Report;
use swofty_core::folders::Key;
use swofty_core::manifest::Category;
use swofty_core::order;
use swofty_i18n::tr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum State {
    On,
    Off,
    Conflicts,
    Update,
}

impl State {
    pub(super) const ALL: [State; 4] = [Self::On, Self::Off, Self::Conflicts, Self::Update];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::On => "On",
            Self::Off => tr("Off"),
            Self::Conflicts => tr("Has conflicts"),
            Self::Update => tr("Update available"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Group {
    Category,
    Type,
    Source,
    State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Facet {
    Category(Option<Category>),
    Type(Type),
    Source(Source),
    State(State),
}

impl Facet {
    pub(super) fn group(self) -> Group {
        match self {
            Self::Category(_) => Group::Category,
            Self::Type(_) => Group::Type,
            Self::Source(_) => Group::Source,
            Self::State(_) => Group::State,
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Category(Some(c)) => c.label(),
            Self::Category(None) => tr("Uncategorised"),
            Self::Type(t) => t.label(),
            Self::Source(s) => s.label(),
            Self::State(s) => s.label(),
        }
    }
}

/// Addons that load first, in this order: SwoftyMod, then Simplified for Swofty.
/// They make up Built In, and nothing is placed above them.
pub(super) const PINNED: [&str; 2] = [swofty_core::builtin::CORE_ID, swofty_core::managed::ID];

#[derive(Clone, PartialEq)]
pub(super) enum AddonListItem {
    Heading,
    Folder {
        key: Key,
        at: usize,
        count: usize,
        on: usize,
        open: bool,
    },
    Addon {
        index: usize,
        id: String,
        position: Option<usize>,
        at: usize,
        nested: Option<(Key, usize)>,
    },
    Detail(String),
    End,
    BrokenHeading,
    Broken(usize),
}

impl AddonListItem {
    fn key(&self) -> String {
        match self {
            Self::Heading => "heading".to_owned(),
            Self::Folder { key, .. } => format!("folder:{key:?}"),
            Self::Addon { id, .. } => format!("addon:{id}"),
            Self::Detail(id) => format!("detail:{id}"),
            Self::End => "end".to_owned(),
            Self::BrokenHeading => "broken-heading".to_owned(),
            Self::Broken(i) => format!("broken:{i}"),
        }
    }
}

pub(super) fn anchor(old: &[AddonListItem], new: &[AddonListItem], top: usize) -> Option<(usize, bool)> {
    let at: HashMap<String, usize> = new.iter().enumerate().map(|(i, item)| (item.key(), i)).collect();
    old.get(top..)?
        .iter()
        .enumerate()
        .find_map(|(n, item)| at.get(&item.key()).map(|&ix| (ix, n == 0)))
}

pub(crate) struct AddonsPage {
    pub(super) search: Entity<InputState>,
    pub(super) window: AnyWindowHandle,
    pub(super) facets: BTreeSet<Facet>,
    pub(super) scroll: ListState,
    pub(super) smooth: crate::scroll::SmoothScroll,
    pub(super) list_items: Vec<AddonListItem>,
    pub(super) seen: Option<Instant>,
    pub(super) scroll_pending: bool,
    pub(super) animating: bool,
    pub(super) report: Option<Report>,
    pub(super) read_at: Option<Instant>,
    pub(crate) checking: bool,
    pub(super) open_pairs: HashSet<(String, String)>,
    pub(super) details: HashSet<(String, String)>,
    pub(super) all_items: HashSet<(String, String)>,
    pub(super) advanced: bool,
    pub(super) changes: HashMap<String, Changes>,
    pub(super) drop: Option<DropAt>,
    pub(super) selection: super::select::Selection,
    pub(super) ctrl: bool,
    /// From the core's reports of the last session: each addon's status lines, and the registry
    /// conflicts with the addons each is about.
    pub(super) status: std::collections::BTreeMap<String, Vec<String>>,
    pub(super) registry_conflicts: Vec<(Vec<String>, swofty_core::stdlib::reports::Conflict)>,
    pub(super) diagnosis: order::Diagnosis,
    pub(super) problems_key: Option<(Instant, u64, u64)>,
    pub(super) careers: Vec<swofty_core::stdlib::careers::Career>,
    pub(super) _search: Subscription,
}

impl AddonsPage {
    pub(crate) fn cancel_scroll(&self) {
        self.smooth.cancel();
    }

    pub(crate) fn new(window: &mut Window, cx: &mut Context<Shell>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(tr("Search addons")));
        let changed = cx.subscribe(&search, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            search,
            window: window.window_handle(),
            facets: BTreeSet::new(),
            scroll: ListState::new(0, ListAlignment::Top, px(200.)),
            smooth: Default::default(),
            list_items: Vec::new(),
            seen: None,
            scroll_pending: false,
            animating: false,
            report: None,
            read_at: None,
            checking: false,
            open_pairs: HashSet::new(),
            details: HashSet::new(),
            all_items: HashSet::new(),
            advanced: false,
            changes: HashMap::new(),
            drop: None,
            selection: Default::default(),
            ctrl: false,
            status: Default::default(),
            registry_conflicts: Vec::new(),
            diagnosis: order::Diagnosis::empty(),
            problems_key: None,
            careers: Vec::new(),
            _search: changed,
        }
    }

    pub(crate) fn clear_selection(&mut self) -> bool {
        let had = !self.selection.is_empty();
        self.selection.clear();
        had
    }

    pub(super) fn ticking(&self) -> bool {
        self.ctrl || !self.selection.is_empty()
    }

    pub(super) fn clear_search(&self, cx: &mut App) {
        let (input, window) = (self.search.clone(), self.window);
        cx.defer(move |cx| {
            window
                .update(cx, |_, window, cx| {
                    input.update(cx, |s, cx| s.set_value("", window, cx))
                })
                .ok();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{AddonListItem, anchor};

    fn addon(id: &str, index: usize) -> AddonListItem {
        AddonListItem::Addon {
            index,
            id: id.to_owned(),
            position: None,
            at: index,
            nested: None,
        }
    }

    #[test]
    fn anchor_follows_an_addon_whose_index_changed() {
        let old = [
            AddonListItem::Heading,
            addon("a", 0),
            addon("b", 1),
            addon("c", 2),
        ];
        let new = [AddonListItem::Heading, addon("a", 0), addon("c", 1)];
        assert_eq!(anchor(&old, &new, 3), Some((2, true)));
    }

    #[test]
    fn anchor_moves_to_the_next_row_when_the_top_row_is_gone() {
        let old = [
            AddonListItem::Heading,
            addon("a", 0),
            addon("b", 1),
            addon("c", 2),
        ];
        let new = [AddonListItem::Heading, addon("a", 0), addon("c", 1)];
        assert_eq!(anchor(&old, &new, 2), Some((2, false)));
    }

    #[test]
    fn anchor_is_none_without_old_rows() {
        assert_eq!(anchor(&[], &[AddonListItem::Heading], 0), None);
    }
}
