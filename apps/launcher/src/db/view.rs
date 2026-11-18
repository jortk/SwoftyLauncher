use std::sync::Arc;
use std::time::Duration;

use crate::scale::px;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::v_flex;
use gpui_kit::*;
use swofty_core::dbedit::{Catalog, Kind, Module, NodeId, NodeKind, Target, driver_manager};
use swofty_i18n::{tr, trf};

use super::state::{Which, game_id};
use super::{Debounced, LinkState, Load, Nav, Reveal, Then, ViewSlot, guarded};
use crate::shell::Shell;

type Loaded = Result<(Arc<Catalog>, &'static [Module]), String>;

const DEBOUNCE: Duration = Duration::from_millis(120);

impl Shell {
    pub(crate) fn sync_db(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.install.as_ref().map(game_id);
        if self.db.game.is_some() && self.db.game != id {
            self.db_forget_game();
        }
        if matches!(self.db.load, Load::Idle) && id.is_some() {
            self.db_open(cx);
        }
        if self.db.focus.is_none() {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            self.db.focus = Some(focus);
        }
        if self.db.search.is_none() {
            let search = self.debounced(Which::Search, window, cx);
            self.db.search = Some(search);
        }
        if self.db.filter.is_none() {
            let filter = self.debounced(Which::Filter, window, cx);
            self.db.filter = Some(filter);
        }
        if self.db.unnamed.is_none() {
            let unnamed = self.debounced(Which::Unnamed, window, cx);
            self.db.unnamed = Some(unnamed);
        }
        if self.db.raw_filter.is_none() {
            let raw = self.debounced(Which::Raw, window, cx);
            self.db.raw_filter = Some(raw);
        }
        if std::mem::take(&mut self.db.clear_raw)
            && let Some(d) = self.db.raw_filter.as_mut()
        {
            d.query.clear();
            d.input.update(cx, |s, cx| s.set_value("", window, cx));
        }
        if let Some(id) = self.db.pending_open.take() {
            self.db_leave(Then::Open(id), window, cx);
        }
        self.db_apply_reveal(cx);
        self.sync_fields(window, cx);
    }

    // A search jump selects its row once the view shows the hit's module and scope.
    fn db_apply_reveal(&mut self, cx: &mut Context<Self>) {
        let (Some(r), Some(slot)) = (self.db.reveal.as_ref(), self.db.view.as_ref()) else {
            return;
        };
        let here = Nav::Module(slot.module.clone()) == self.db.nav
            && r.module == slot.module
            && r.scope.as_ref().is_none_or(|s| slot.scope.as_ref() == Some(s));
        if !here {
            return;
        }
        let Some(r) = self.db.reveal.take() else {
            return;
        };
        if let Some(i) = slot
            .shown
            .iter()
            .position(|&i| slot.view.rows[i].label == r.label)
        {
            self.db.rows_motion.cancel();
            self.db.rows_scroll.scroll_to_item(i, ScrollStrategy::Center);
        }
        if slot.view.rows.iter().any(|row| row.label == r.label) {
            self.db.row = Some(r.label);
        }
        cx.notify();
    }

    fn slot(&mut self, which: Which) -> Option<&mut Debounced> {
        match which {
            Which::Search => self.db.search.as_mut(),
            Which::Filter => self.db.filter.as_mut(),
            Which::Unnamed => self.db.unnamed.as_mut(),
            Which::Raw => self.db.raw_filter.as_mut(),
        }
    }

    fn debounced(&mut self, which: Which, window: &mut Window, cx: &mut Context<Self>) -> Debounced {
        let placeholder = match which {
            Which::Search => tr("Search drivers, teams, tracks, keys and values"),
            Which::Filter => tr("Filter rows"),
            Which::Unnamed => tr("Filter by key or value"),
            Which::Raw => tr("Filter this level"),
        };
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let sub = cx.subscribe_in(&input, window, move |this, state, ev: &InputEvent, window, cx| {
            match ev {
                InputEvent::Change => {}
                InputEvent::PressEnter { .. } if which == Which::Search => {
                    this.db_open_hit(this.db.hit, window, cx);
                    return;
                }
                InputEvent::Focus if which == Which::Search => {
                    this.db.hits_open = true;
                    cx.notify();
                    return;
                }
                _ => return,
            }
            let query = state.read(cx).value().to_string();
            let Some(slot) = this.slot(which) else {
                return;
            };
            slot.generation += 1;
            let generation = slot.generation;
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(DEBOUNCE).await;
                this.update(cx, |s, cx| {
                    if let Some(slot) = s.slot(which).filter(|d| d.generation == generation) {
                        slot.query = query;
                        match which {
                            Which::Search => s.db_search(cx),
                            Which::Filter => s.db_filter(cx),
                            Which::Unnamed => s.db_unnamed(cx),
                            Which::Raw => cx.notify(),
                        }
                    }
                })
                .ok();
            })
            .detach();
        });
        Debounced {
            input,
            query: String::new(),
            generation: 0,
            _sub: sub,
        }
    }

    fn db_open(&mut self, cx: &mut Context<Self>) {
        let Some(install) = self.install.clone() else {
            return;
        };
        let id = game_id(&install);
        self.db.load = Load::Loading;
        self.db.game = Some(id.clone());
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let loaded: Loaded = cx
                .background_executor()
                .spawn(async move {
                    let catalog = guarded(|| {
                        let game = crate::sections::game(&store, Some(&install))
                            .ok_or(tr("The game folder could not be read."))?;
                        Catalog::open(&store, &game)
                            .map_err(|e| trf!("The game database could not be read: {e}", e = e))
                    })?;
                    guarded(|| Ok(catalog.search("x", 1))).ok();
                    let modules = guarded(|| Ok(swofty_core::dbedit::modules())).unwrap_or(&[]);
                    Ok((catalog, modules))
                })
                .await;
            this.update(cx, |s, cx| {
                if s.db.game.as_ref() == Some(&id) {
                    s.db_loaded(loaded, cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn db_loaded(&mut self, loaded: Loaded, cx: &mut Context<Self>) {
        match loaded {
            Ok((catalog, modules)) => {
                self.db.modules = modules;
                if let Nav::Raw(0) = self.db.nav {
                    self.db.nav = modules
                        .first()
                        .map_or(Nav::Raw(catalog.root()), |m| Nav::Module(m.id.clone()));
                }
                self.db.load = Load::Ready(catalog);
                self.db_changed(None, false, cx);
                self.db_unnamed(cx);
            }
            Err(e) => self.db.load = Load::Failed(e),
        }
        cx.notify();
    }

    pub(crate) fn db_go(&mut self, nav: Nav, cx: &mut Context<Self>) {
        if self.db.nav != nav {
            self.db.rows_motion.cancel();
            self.db.list_motion.cancel();
            self.db.nav = nav;
            self.db.row = None;
            self.db.reveal = None;
            self.db.key = None;
            self.db.cell = None;
            self.db.rows_scroll = UniformListScrollHandle::new();
            self.db.list_scroll = UniformListScrollHandle::new();
            self.db.clear_raw = self.db.raw_filter.as_ref().is_some_and(|d| !d.query.is_empty());
        }
        self.db.hits_open = false;
        self.db_refresh_view(cx);
        cx.notify();
    }

    // After any edit: persist, patch the cached module view, then rebuild it off the UI thread.
    pub(crate) fn db_changed(&mut self, key: Option<u64>, save: bool, cx: &mut Context<Self>) {
        self.db.rev += 1;
        self.db.reindex();
        if save {
            self.db_persist(cx);
        }
        let edit = key.and_then(|k| self.db.edit(k).cloned());
        if let (Some(key), Some(catalog), Some(slot)) =
            (key, self.db.catalog().cloned(), self.db.view.as_mut())
        {
            let view = Arc::make_mut(&mut slot.view);
            for row in &mut view.rows {
                for cell in row.cells.iter_mut().filter(|c| c.key == Some(key)) {
                    cell.edited = edit.is_some();
                    cell.value = match &edit {
                        Some(e) => e.value.clone().unwrap_or_default(),
                        None => catalog
                            .record_by_key(key)
                            .map_or(cell.stock.clone(), |r| r.value.to_owned()),
                    };
                }
            }
        }
        self.db_refresh_view(cx);
        cx.notify();
    }

    pub(crate) fn db_refresh_view(&mut self, cx: &mut Context<Self>) {
        let Nav::Module(id) = self.db.nav.clone() else {
            return;
        };
        let (Some(catalog), Some(module)) = (self.db.catalog().cloned(), self.db.module(&id)) else {
            return;
        };
        let scope = self.db.scopes.get(&id).cloned();
        let want = (id.clone(), scope.clone(), self.db.rev);
        if self.db.view_want.as_ref() == Some(&want) {
            return;
        }
        self.db.view_want = Some(want.clone());
        let stale = std::mem::take(&mut self.db.stale_choices);
        let choices = self
            .db
            .view
            .as_ref()
            .filter(|v| !stale && v.module == id && v.scope == scope)
            .map(|v| v.choices.clone());
        let edits = self.db.draft.edits.clone();
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let (view, choices) = cx
                .background_executor()
                .spawn(async move {
                    let view = guarded(|| {
                        module
                            .view(&catalog, &edits, scope.as_deref())
                            .map_err(|e| e.to_string())
                    });
                    let choices = choices.unwrap_or_else(|| {
                        let tracks = swofty_core::tracks::installed(&store);
                        let drivers = driver_manager::drivers(&store, &catalog);
                        let heads = swofty_core::heads::installed(&store);
                        let refs = module
                            .fields
                            .iter()
                            .enumerate()
                            .filter(|(_, f)| matches!(f.kind, Some(Kind::Ref | Kind::RefList)))
                            .map(|(i, _)| {
                                let c = module.choices_with(
                                    &catalog,
                                    i,
                                    scope.as_deref(),
                                    &tracks,
                                    &drivers,
                                    &heads,
                                );
                                (i, c)
                            })
                            .collect();
                        Arc::new(refs)
                    });
                    (view, choices)
                })
                .await;
            this.update(cx, |s, cx| {
                if s.db.view_want.as_ref() != Some(&want) {
                    return;
                }
                match view {
                    Ok(view) => {
                        if let Some(scope) = view.scope.clone() {
                            s.db.scopes.entry(want.0.clone()).or_insert(scope);
                        }
                        if s.db
                            .view
                            .as_ref()
                            .is_some_and(|v| v.module == want.0 && v.scope != view.scope)
                        {
                            s.db.row = None;
                            s.db.cell = None;
                        }
                        s.db.view_error = None;
                        s.db.view = Some(ViewSlot {
                            module: want.0,
                            scope: view.scope.clone(),
                            view: Arc::new(view),
                            choices,
                            shown: Vec::new(),
                        });
                        s.db_filter(cx);
                    }
                    Err(e) => {
                        s.db.view = None;
                        s.db.view_error = Some(e);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn db_set_scope(&mut self, scope: String, cx: &mut Context<Self>) {
        if let Nav::Module(id) = &self.db.nav {
            self.db.scopes.insert(id.clone(), scope);
            self.db.row = None;
            self.db.cell = None;
            self.db.reveal = None;
            self.db_refresh_view(cx);
        }
    }

    fn db_filter(&mut self, cx: &mut Context<Self>) {
        let query = self
            .db
            .filter
            .as_ref()
            .map(|f| f.query.trim().to_lowercase())
            .unwrap_or_default();
        if let Some(slot) = self.db.view.as_mut() {
            let view = &slot.view;
            let columns: Vec<usize> = view.columns.iter().map(|c| c.field).collect();
            slot.shown = (0..view.rows.len())
                .filter(|&i| {
                    let r = &view.rows[i];
                    query.is_empty()
                        || r.name.to_lowercase().contains(&query)
                        || r.label.to_lowercase().contains(&query)
                        || columns.iter().any(|&c| {
                            r.cells.get(c).is_some_and(|cell| {
                                cell.value.to_lowercase().contains(&query)
                                    || cell
                                        .text
                                        .as_ref()
                                        .is_some_and(|t| t.to_lowercase().contains(&query))
                            })
                        })
                })
                .collect();
        }
        cx.notify();
    }

    fn db_search(&mut self, cx: &mut Context<Self>) {
        let query = self
            .db
            .search
            .as_ref()
            .map(|s| s.query.trim().to_owned())
            .unwrap_or_default();
        let generation = self.db.search.as_ref().map_or(0, |s| s.generation);
        let Some(catalog) = self.db.catalog().cloned() else {
            return;
        };
        if query.is_empty() {
            self.db.hits.clear();
            cx.notify();
            return;
        }
        let from = catalog.clone();
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let hits = cx
                .background_executor()
                .spawn(async move {
                    guarded(|| {
                        let drivers = driver_manager::drivers(&store, &catalog);
                        Ok(catalog.search_with(&query, 60, &drivers))
                    })
                    .unwrap_or_default()
                })
                .await;
            this.update(cx, |s, cx| {
                if s.db.search.as_ref().is_some_and(|d| d.generation == generation)
                    && s.db.catalog().is_some_and(|c| Arc::ptr_eq(c, &from))
                {
                    let mut hits = hits;
                    hits.sort_by_key(|h| h.kind);
                    s.db.hits = hits;
                    s.db.hit = 0;
                    s.db.hits_open = true;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn db_unnamed(&mut self, cx: &mut Context<Self>) {
        let Some(catalog) = self.db.catalog().cloned() else {
            return;
        };
        let query = self
            .db
            .unnamed
            .as_ref()
            .map(|d| d.query.trim().to_lowercase())
            .unwrap_or_default();
        let generation = self.db.unnamed.as_ref().map_or(0, |d| d.generation);
        let from = catalog.clone();
        cx.spawn(async move |this, cx| {
            let rows = cx
                .background_executor()
                .spawn(async move {
                    guarded(|| {
                        let Some(root) = catalog
                            .children(catalog.root())
                            .iter()
                            .copied()
                            .find(|&c| catalog.node(c).kind == NodeKind::Unnamed)
                        else {
                            return Ok(Vec::new());
                        };
                        Ok(catalog
                            .children(root)
                            .iter()
                            .copied()
                            .filter(|&n| {
                                if query.is_empty() {
                                    return true;
                                }
                                let node = catalog.node(n);
                                node.name.to_lowercase().contains(&query)
                                    || node.record.is_some_and(|r| {
                                        catalog.record(r).value.to_lowercase().contains(&query)
                                    })
                            })
                            .collect())
                    })
                    .unwrap_or_default()
                })
                .await;
            this.update(cx, |s, cx| {
                if s.db.unnamed.as_ref().is_none_or(|d| d.generation == generation)
                    && s.db.catalog().is_some_and(|c| Arc::ptr_eq(c, &from))
                {
                    s.db.unnamed_rows = Arc::new(rows);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn db_focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(s) = &self.db.search {
            s.input.update(cx, |s, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
            self.db.hits_open = true;
            cx.notify();
        }
    }

    pub(crate) fn db_open_hit(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hit) = self.db.hits.get(index).cloned() else {
            return;
        };
        self.db.hits_open = false;
        if let Some(f) = &self.db.focus {
            f.focus(window, cx);
        }
        match hit.target {
            Target::ModuleRow { module, scope, label } => {
                if let Some(scope) = scope.clone() {
                    self.db.scopes.insert(module.clone(), scope);
                }
                self.db_go(Nav::Module(module.clone()), cx);
                self.db.row = None;
                self.db.cell = None;
                self.db.reveal = Some(Reveal { module, scope, label });
                self.db_refresh_view(cx);
            }
            Target::Node(node) => self.db_show_node(node, cx),
            Target::Link(link) => match self.db_link_state(&link) {
                LinkState::Open => self.db_open_section(&link, cx),
                _ => {
                    if let Some(m) = self.db.modules.iter().find(|m| m.links.contains(&link)) {
                        self.db_go(Nav::Module(m.id.clone()), cx);
                    }
                }
            },
        }
        cx.notify();
    }

    pub(crate) fn db_show_node(&mut self, node: NodeId, cx: &mut Context<Self>) {
        let Some(catalog) = self.db.catalog().cloned() else {
            return;
        };
        let n = catalog.node(node);
        let parent = n.parent.unwrap_or(catalog.root());
        match n.kind {
            NodeKind::Key if catalog.node(parent).kind == NodeKind::Unnamed => {
                self.db_go(Nav::Unnamed, cx);
                self.db.key = Some(node);
            }
            NodeKind::Key => {
                self.db_go(Nav::Raw(parent), cx);
                self.db.key = Some(node);
            }
            NodeKind::Unnamed => self.db_go(Nav::Unnamed, cx),
            _ => self.db_go(Nav::Raw(node), cx),
        }
    }

    pub(crate) fn db_loading(&self, text: &str, p: crate::theme::Palette) -> AnyElement {
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap(px(10.))
            .pb(px(48.))
            .child(crate::pages::badge(IconName::Database, 44., p))
            .child(
                div()
                    .pt(px(8.))
                    .text_size(px(13.))
                    .text_color(p.text_muted)
                    .child(text.to_owned()),
            )
            .into_any_element()
    }
}
