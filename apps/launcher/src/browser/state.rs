use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use crate::notices::Notification;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;
use swofty_core::catalog::publishers::{self, Creator, Profile};
use swofty_core::catalog::stars::Stars;
use swofty_core::catalog::{self, Addon, Trouble, Update};
use swofty_i18n::{tr, trf, trn};

use super::authors::View;
use super::*;
use crate::scale::px;
use crate::shell::Shell;

pub(crate) struct Browser {
    pub(crate) addons: Vec<Addon>,
    pub(crate) updates: Vec<Update>,
    pub(crate) toasts: Vec<Notification>,
    pub(super) icons: HashMap<String, Arc<Image>>,
    pub(super) checked: Option<Instant>,
    pub(super) fetched_at: Option<SystemTime>,
    pub(super) live: bool,
    pub(super) saved: bool,
    pub(super) fetching: bool,
    pub(super) trouble: Option<Trouble>,
    pub(super) search: Entity<InputState>,
    pub(super) window: AnyWindowHandle,
    pub(super) view: View,
    pub(super) trail: Vec<(View, Point<Pixels>)>,
    pub(super) scroll: ScrollHandle,
    pub(super) grid_scroll: ListState,
    pub(super) grid_cache: Option<GridCache>,
    pub(super) catalog_revision: u64,
    pub(super) facet_counts: RefCell<HashMap<Facet, usize>>,
    pub(super) needs_cache: RefCell<HashMap<String, AddonNeeds>>,
    pub(super) grid_layout: (Arc<[usize]>, usize, bool, u32),
    pub(crate) smooth: crate::scroll::SmoothScroll,
    pub(super) details: HashMap<String, Addon>,
    pub(super) loading: BTreeSet<String>,
    pub(super) asked: HashSet<String>,
    pub(super) history: HashSet<String>,
    pub(super) working: HashMap<String, Job>,
    pub(super) queue: crate::download_queue::Queue<Request>,
    pub(super) failed: HashMap<String, String>,
    pub(super) refused: BTreeSet<String>,
    pub(super) batch: Option<Batch>,
    pub(super) held: BTreeMap<String, (semver::Version, String)>,
    /// The version of each addon the play bar doesn't offer ([`catalog::source::skipped`]).
    pub(super) skipped: BTreeMap<String, semver::Version>,
    pub(super) ticking: bool,
    pub(super) stars: Stars,
    pub(super) syncing: bool,
    pub(super) sync_again: bool,
    pub(super) star_gen: u64,
    pub(super) sort: Sort,
    pub(super) search_sort: Option<Sort>,
    pub(super) landing: bool,
    pub(super) tab: Tab,
    pub(super) facets: BTreeSet<Facet>,
    pub(super) hide_code: bool,
    pub(super) activity: VecDeque<Activity>,
    pub(super) activity_open: bool,
    pub(super) creators: Vec<Creator>,
    pub(super) profiles: HashMap<String, Profile>,
    pub(super) profile_loading: BTreeSet<String>,
    pub(super) profile_failed: HashMap<String, String>,
    pub(super) author_sort: Sort,
    pub(super) avatars: HashMap<String, Arc<Image>>,
    pub(super) media: HashMap<String, Arc<Image>>,
    pub(super) media_pick: HashMap<String, usize>,
    pub(super) _search: Subscription,
}

impl Browser {
    pub(crate) fn new(root: &Path, window: &mut Window, cx: &mut Context<Shell>) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr("Search names, authors, author: or category:"))
        });
        let changed = cx.subscribe(&search, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        });
        let cached = catalog::cached(root);
        Self {
            saved: cached.is_some(),
            fetched_at: cached.as_ref().and_then(|_| catalog::cached_at(root)),
            addons: cached.unwrap_or_default(),
            updates: Vec::new(),
            toasts: Vec::new(),
            icons: HashMap::new(),
            checked: None,
            live: false,
            fetching: false,
            trouble: None,
            search,
            window: window.window_handle(),
            view: View::Grid,
            trail: Vec::new(),
            scroll: ScrollHandle::new(),
            grid_scroll: ListState::new(0, ListAlignment::Top, px(200.)),
            grid_layout: Default::default(),
            grid_cache: None,
            catalog_revision: 0,
            facet_counts: RefCell::default(),
            needs_cache: RefCell::default(),
            smooth: Default::default(),
            details: HashMap::new(),
            loading: BTreeSet::new(),
            asked: HashSet::new(),
            history: HashSet::new(),
            working: HashMap::new(),
            queue: {
                let mut queue = crate::download_queue::Queue::default();
                queue.set_limit(swofty_core::store::Store::new(root).config().download_jobs());
                queue
            },
            failed: HashMap::new(),
            refused: BTreeSet::new(),
            batch: None,
            held: BTreeMap::new(),
            skipped: catalog::source::skipped(root),
            ticking: false,
            stars: Stars::load(root),
            syncing: false,
            sync_again: false,
            star_gen: 0,
            sort: Sort::Stars,
            search_sort: None,
            landing: true,
            tab: None,
            facets: BTreeSet::new(),
            hide_code: false,
            activity: VecDeque::new(),
            activity_open: false,
            creators: publishers::cached(root).unwrap_or_default(),
            profiles: HashMap::new(),
            profile_loading: BTreeSet::new(),
            profile_failed: HashMap::new(),
            author_sort: Sort::Stars,
            avatars: HashMap::new(),
            media: HashMap::new(),
            media_pick: HashMap::new(),
            _search: changed,
        }
    }

    pub(super) fn on_landing(&self, searching: bool) -> bool {
        self.landing && self.tab.is_none() && !searching && self.facets.is_empty() && !self.hide_code
    }

    pub(super) fn stale(&self) -> bool {
        !self.fetching && self.checked.is_none_or(|t| t.elapsed() >= STALE)
    }

    pub(super) fn subtitle(&self) -> String {
        let base = catalog::base_url();
        let host = host(&base);
        let count = self.addons.len();
        let addons = trn(count, "{count} addon", "{count} addons");
        if self.fetching && self.fetched_at.is_none() {
            return trf!("Checking {host}…", host = host);
        }
        let mut text = match (&self.trouble, self.saved) {
            (None, _) => trf!("{addons} from {host}", addons = addons, host = host),
            (Some(_), true) if !self.live => trn(
                count,
                "Offline · showing the {count} addon saved last time",
                "Offline · showing the {count} addons saved last time",
            ),
            (Some(_), true) => trf!(
                "Offline · showing {addons} from {host}",
                addons = addons,
                host = host
            ),
            (Some(_), false) => tr("Offline · no addons saved yet").to_owned(),
        };
        if let Some(age) = self.fetched_at.map(since_time).filter(|_| self.saved) {
            text.push_str(&format!(" · {}", trf!("updated {when}", when = age)));
        }
        if self.fetching {
            text.push_str(&format!(" · {}", tr("checking…")));
        }
        text
    }

    pub(crate) fn has(&self, id: &str) -> bool {
        self.find(id).is_some()
    }

    pub(crate) fn find(&self, id: &str) -> Option<&Addon> {
        self.addons.iter().find(|a| a.id == id)
    }

    pub(super) fn stars_of(&self, a: &Addon) -> u64 {
        self.stars.count(&a.id, a.stars)
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
