use gpui_kit::*;
use swofty_core::catalog::Addon;
use swofty_core::catalog::search::Query;
use swofty_core::manifest::{Category, Platform};
use swofty_core::update::current_version;
use swofty_i18n::tr;

use crate::library::Type;
use crate::shell::Shell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Sort {
    Match,
    Stars,
    Downloads,
    Trending,
    Updated,
    Newest,
    Name,
}

impl Sort {
    pub(super) const ALL: [Sort; 7] = [
        Self::Match,
        Self::Stars,
        Self::Downloads,
        Self::Trending,
        Self::Updated,
        Self::Newest,
        Self::Name,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Match => tr("Best match"),
            Self::Stars => tr("Most stars"),
            Self::Downloads => tr("Most downloads"),
            Self::Trending => tr("Trending"),
            Self::Updated => tr("Recently updated"),
            Self::Newest => tr("Newest"),
            Self::Name => tr("Name A–Z"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Want {
    Installed,
    NotInstalled,
    Update,
    Starred,
    Works,
}

impl Want {
    pub(super) const ALL: [Want; 5] = [
        Self::Installed,
        Self::NotInstalled,
        Self::Update,
        Self::Starred,
        Self::Works,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Installed => tr("Installed"),
            Self::NotInstalled => tr("Not installed"),
            Self::Update => tr("Update available"),
            Self::Starred => tr("Starred by you"),
            Self::Works => tr("Works with this launcher"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Where {
    Steam,
    Ea,
    NotTested,
}

impl Where {
    pub(super) const ALL: [Where; 3] = [Self::Steam, Self::Ea, Self::NotTested];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Steam => tr("Steam"),
            Self::Ea => tr("EA"),
            Self::NotTested => tr("Not tested"),
        }
    }

    pub(super) fn of(platform: Platform) -> Self {
        match platform {
            Platform::Steam => Self::Steam,
            Platform::Ea => Self::Ea,
        }
    }

    pub(super) fn tags(platforms: &[Platform]) -> Vec<Where> {
        if platforms.is_empty() {
            vec![Self::NotTested]
        } else {
            platforms.iter().copied().map(Self::of).collect()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Group {
    Category,
    Type,
    State,
    Platform,
    Author,
    Code,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum Facet {
    Type(Type),
    State(Want),
    Platform(Where),
    Author { id: String, name: String },
}

impl Facet {
    pub(super) fn group(&self) -> Group {
        match self {
            Self::Type(_) => Group::Type,
            Self::State(_) => Group::State,
            Self::Platform(_) => Group::Platform,
            Self::Author { .. } => Group::Author,
        }
    }

    pub(super) fn label(&self) -> String {
        match self {
            Self::Type(t) => t.label().to_owned(),
            Self::State(w) => w.label().to_owned(),
            Self::Platform(w) => w.label().to_owned(),
            Self::Author { name, .. } => name.clone(),
        }
    }
}

/// A category bar tab: all addons, one category, or none of them.
pub(super) type Tab = Option<Option<Category>>;

pub(super) fn tab_label(tab: Tab) -> &'static str {
    match tab {
        None => tr("All"),
        Some(Some(c)) => c.label(),
        Some(None) => tr("Other"),
    }
}

/// The categories, Other last.
pub(super) fn shelves() -> impl Iterator<Item = Tab> {
    Category::ALL
        .into_iter()
        .map(|c| Some(Some(c)))
        .chain([Some(None)])
}

impl Shell {
    pub(super) fn catalog_facet(&self, a: &Addon, f: &Facet) -> bool {
        match f {
            Facet::Type(t) => a.kind_labels().contains(&t.label()),
            Facet::State(Want::Installed) => self.installed_version(&a.id).is_some(),
            Facet::State(Want::NotInstalled) => self.installed_version(&a.id).is_none(),
            Facet::State(Want::Update) => self.catalog_update(&a.id).is_some(),
            Facet::State(Want::Starred) => self.browser.stars.has(&a.id),
            Facet::State(Want::Works) => a.offered(&current_version()).is_some(),
            Facet::Platform(w) => Where::tags(a.platforms()).contains(w),
            Facet::Author { id, .. } => a.owner().is_some_and(|o| o.id == *id),
        }
    }

    pub(super) fn catalog_passes(&self, a: &Addon, matched: bool, except: Option<Group>) -> bool {
        let b = &self.browser;
        matched
            && (except == Some(Group::Category) || b.tab.is_none_or(|t| a.category() == t))
            && (except == Some(Group::Code) || !(b.hide_code && a.native(&current_version())))
            && [Group::Type, Group::State, Group::Platform, Group::Author]
                .into_iter()
                .filter(|g| Some(*g) != except)
                .all(|g| {
                    let mut chosen = b.facets.iter().filter(|f| f.group() == g).peekable();
                    chosen.peek().is_none() || chosen.any(|f| self.catalog_facet(a, f))
                })
    }

    pub(super) fn sort_now(&self, query: &Query) -> Sort {
        if query.is_empty() {
            self.browser.sort
        } else {
            self.browser.search_sort.unwrap_or(Sort::Match)
        }
    }

    pub(super) fn set_sort(&mut self, sort: Sort, cx: &mut Context<Self>) {
        let searching = !self.browser.search.read(cx).value().trim().is_empty();
        if searching {
            self.browser.search_sort = Some(sort);
        } else {
            self.browser.sort = sort;
        }
        cx.notify();
    }

    pub(super) fn clear_filters(&mut self, cx: &mut Context<Self>) {
        let b = &mut self.browser;
        b.facets.clear();
        b.hide_code = false;
        b.search_sort = None;
        b.clear_search(cx);
        cx.notify();
    }

    pub(super) fn shelve(&mut self, tab: Tab, cx: &mut Context<Self>) {
        let b = &mut self.browser;
        if b.landing || b.tab != tab {
            b.smooth.cancel();
            b.scroll.set_offset(Point::default());
            b.grid_scroll.scroll_to(ListOffset::default());
        }
        b.landing = false;
        b.tab = tab;
        cx.notify();
    }

    pub(super) fn show_categories(&mut self, cx: &mut Context<Self>) {
        let b = &mut self.browser;
        b.landing = true;
        b.tab = None;
        b.smooth.cancel();
        b.scroll.set_offset(Point::default());
        b.grid_scroll.scroll_to(ListOffset::default());
        self.clear_filters(cx);
    }

    pub(crate) fn show_shader_addons(&mut self, cx: &mut Context<Self>) {
        self.shelve(Some(Some(Category::Shaders)), cx);
    }
}
