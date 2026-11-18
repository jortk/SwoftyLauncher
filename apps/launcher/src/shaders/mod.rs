mod actions;
mod clips;
mod docs;
mod editor;
mod list;
mod preview;
mod triggers;

use std::path::PathBuf;
use std::sync::Arc;

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::shaders::{self, ConfigEntry, ConfigRef};
use swofty_core::store::Store;
use swofty_i18n::{tr, trf};
use swofty_shaders::Catalog;

use crate::pages::GUTTER;
use crate::theme::{Palette, palette};
use crate::ui::PageHeader;

pub(crate) const KEYS: [&str; 10] = [
    "B",
    "F9",
    "F10",
    "F11",
    "Insert",
    "Home",
    "End",
    "Page Up",
    "Page Down",
    "None",
];
const LIST_WIDTH: f32 = 320.;

/// What the shell tells the page about the profile and the game.
#[derive(Clone, Default)]
pub(crate) struct Info {
    pub profile: String,
    pub game_dir: Option<PathBuf>,
    pub running: bool,
}

pub(crate) enum Event {
    Browse,
    OpenAddon(String),
    ActiveChanged,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Editor,
    Preview,
    Documentation,
}

struct Snapshot {
    catalog: Arc<Catalog>,
    entries: Vec<ConfigEntry>,
    active: Option<ConfigRef>,
    forced: Option<(ConfigRef, String)>,
    key: String,
}

pub(crate) struct ShadersView {
    store: Store,
    info: Info,
    catalog: Arc<Catalog>,
    entries: Vec<ConfigEntry>,
    active: Option<ConfigRef>,
    forced: Option<(ConfigRef, String)>,
    key: String,
    selected: Option<ConfigRef>,
    mode: Mode,
    error: Option<String>,
    busy: bool,
    refreshed: bool,
    want_addon: Option<String>,
    editor: editor::EditorPane,
    list: list::ListState,
    preview: preview::PreviewState,
    /// The effect whose table the Documentation tab last copied.
    docs_copied: Option<String>,
}

impl EventEmitter<Event> for ShadersView {}

impl ShadersView {
    pub(crate) fn new(store: Store, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let p = palette(cx);
        let editor = editor::EditorPane::new(p, window, cx);
        let preview = preview::PreviewState::new(window, cx);
        let list = list::ListState::default();
        Self {
            store,
            info: Info::default(),
            catalog: Arc::new(Catalog::default()),
            entries: Vec::new(),
            active: None,
            forced: None,
            key: "B".into(),
            selected: None,
            mode: Mode::Editor,
            docs_copied: None,
            error: None,
            busy: false,
            refreshed: false,
            want_addon: None,
            editor,
            list,
            preview,
        }
    }

    pub(crate) fn sync(&mut self, info: Info, refresh: bool, cx: &mut Context<Self>) {
        let changed = self.info.profile != info.profile;
        let running = self.info.running != info.running;
        self.info = info;
        if refresh || changed || !self.refreshed {
            self.refresh(cx);
        }
        if running {
            cx.notify();
        }
    }

    pub(crate) fn select_addon(&mut self, addon: &str, cx: &mut Context<Self>) {
        self.want_addon = Some(addon.to_owned());
        self.mode = Mode::Editor;
        self.refresh(cx);
    }

    pub(crate) fn has_configs(&self, addon: &str) -> bool {
        self.entries
            .iter()
            .any(|e| matches!(&e.r#ref, ConfigRef::Addon { addon: a, .. } if a == addon))
    }

    pub(super) fn selected_entry(&self) -> Option<&ConfigEntry> {
        let r = self.selected.as_ref()?;
        self.entries.iter().find(|e| &e.r#ref == r)
    }

    pub(super) fn entry_of(&self, r: &ConfigRef) -> Option<&ConfigEntry> {
        self.entries.iter().find(|e| &e.r#ref == r)
    }

    /// The config the game runs: the forced one, else the profile's.
    pub(super) fn running_ref(&self) -> Option<&ConfigRef> {
        self.forced.as_ref().map(|(r, _)| r).or(self.active.as_ref())
    }

    pub(super) fn is_live(&self, r: &ConfigRef) -> bool {
        self.info.running && self.running_ref() == Some(r)
    }

    pub(super) fn live_target(&self, r: &ConfigRef) -> Option<(PathBuf, String)> {
        if !self.is_live(r) {
            return None;
        }
        Some((self.info.game_dir.clone()?, self.info.profile.clone()))
    }

    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.info.profile.is_empty() {
            return;
        }
        self.refreshed = true;
        let store = self.store.clone();
        let name = self.info.profile.clone();
        cx.spawn(async move |this, cx| {
            let snap = cx
                .background_executor()
                .spawn(async move {
                    let profile = store.profile(&name).ok();
                    Snapshot {
                        catalog: Arc::new(shaders::catalog(&store)),
                        entries: shaders::list(&store),
                        active: profile.as_ref().and_then(|p| shaders::active(&store, p)),
                        forced: profile.as_ref().and_then(|p| shaders::forced(&store, p)),
                        key: shaders::toggle_key(&store),
                    }
                })
                .await;
            this.update(cx, |v, cx| v.apply(snap, cx)).ok();
        })
        .detach();
    }

    fn apply(&mut self, snap: Snapshot, cx: &mut Context<Self>) {
        let effects_changed = {
            let ids = |c: &Catalog| c.effects.iter().map(|e| e.id.clone()).collect::<Vec<_>>();
            ids(&snap.catalog) != ids(&self.catalog)
        };
        self.catalog = snap.catalog;
        *self.editor.catalog.borrow_mut() = self.catalog.clone();
        self.entries = snap.entries;
        self.active = snap.active;
        self.forced = snap.forced;
        self.key = snap.key;
        if let Some(addon) = self.want_addon.take() {
            let found = self
                .entries
                .iter()
                .find(|e| matches!(&e.r#ref, ConfigRef::Addon { addon: a, .. } if *a == addon));
            if let Some(entry) = found {
                self.selected = Some(entry.r#ref.clone());
            }
        }
        let valid = self.selected.as_ref().is_some_and(|s| self.entry_of(s).is_some());
        if !valid {
            self.selected = self.default_selection();
        }
        if effects_changed {
            self.preview_catalog_changed(cx);
            if self.editor.loaded.is_some() {
                let text = self.editor.last_text.clone();
                self.schedule_lint(text, std::time::Duration::ZERO, cx);
            }
        }
        cx.notify();
    }

    fn default_selection(&self) -> Option<ConfigRef> {
        self.forced
            .as_ref()
            .map(|(r, _)| r.clone())
            .or_else(|| self.active.clone())
            .filter(|r| self.entry_of(r).is_some())
            .or_else(|| self.entries.first().map(|e| e.r#ref.clone()))
    }

    pub(super) fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        match mode {
            Mode::Preview => self.preview_open(cx),
            Mode::Editor | Mode::Documentation => self.preview_pause(cx),
        }
        cx.notify();
    }

    fn ensure_loaded(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.loading || self.selected == self.editor.loaded {
            return;
        }
        let Some(r) = self.selected.clone() else {
            self.editor.loaded = None;
            self.editor.saved.clear();
            self.editor.last_text.clear();
            self.editor.lint = Default::default();
            self.editor.problems.borrow_mut().clear();
            self.editor.state.update(cx, |s, cx| s.set_value("", window, cx));
            return;
        };
        self.editor.loading = true;
        let store = self.store.clone();
        let target = r.clone();
        cx.spawn_in(window, async move |this, cx| {
            let text = cx
                .background_executor()
                .spawn(async move { shaders::read(&store, &target).map_err(|e| e.to_string()) })
                .await;
            this.update_in(cx, |v, window, cx| v.loaded(r, text, window, cx))
                .ok();
        })
        .detach();
    }

    fn loaded(
        &mut self,
        r: ConfigRef,
        text: Result<String, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor.loading = false;
        if self.selected.as_ref() != Some(&r) {
            cx.notify();
            return;
        }
        let editable = self.entry_of(&r).is_some_and(|e| e.editable);
        let (text, error) = match text {
            Ok(t) => (t, None),
            Err(e) => (String::new(), Some(trf!("Couldn't open the config: {e}", e = e))),
        };
        self.error = error;
        self.editor.loaded = Some(r);
        self.editor.saved = text.clone();
        self.editor.last_text = text.clone();
        self.editor.lint = Default::default();
        self.editor.problems.borrow_mut().clear();
        self.editor.save_task = None;
        self.editor.state.update(cx, |s, cx| {
            s.set_value(text.clone(), window, cx);
            s.set_readonly(!editable, cx);
        });
        self.schedule_lint(text, std::time::Duration::ZERO, cx);
        cx.notify();
    }

    fn subtitle(&self) -> String {
        let name_of = |r: &ConfigRef| self.entry_of(r).map(|e| (e.name.clone(), e.source.clone()));
        if let Some((r, addon)) = &self.forced {
            let name = name_of(r).map_or_else(|| tr("A config").to_owned(), |n| n.0);
            return trf!("{name} · forced by {addon}", name = name, addon = addon);
        }
        match self.active.as_ref().and_then(name_of) {
            Some((name, source)) if source == "Personal" => trf!("{name} · yours", name = name),
            Some((name, source)) => trf!("{name} · from {source}", name = name, source = source),
            None => tr("Off").to_owned(),
        }
    }

    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        let shaders = cx.entity().downgrade();
        let current = self.key.clone();
        let keys = Button::new("shaders-key")
            .outline()
            .tap_sm()
            .icon(IconName::Power)
            .label(trf!("In-game key: {current}", current = current))
            .dropdown_caret(true)
            .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                for key in KEYS {
                    let shaders = shaders.clone();
                    menu = menu.item(PopupMenuItem::new(key).checked(key == current).on_click(
                        move |_, _, cx| {
                            shaders.update(cx, |v, cx| v.set_key(key, cx)).ok();
                        },
                    ));
                }
                menu
            });
        let browse = Button::new("shaders-browse")
            .outline()
            .tap_sm()
            .icon(IconName::PackageSearch)
            .label(tr("Browse shaders"))
            .on_click(cx.listener(|_, _, _, cx| cx.emit(Event::Browse)));
        PageHeader::new(tr("Shaders"))
            .subtitle(self.subtitle())
            .error(self.error.clone())
            .actions(h_flex().gap(px(8.)).child(keys).child(browse))
            .into_any_element()
    }

    fn set_key(&mut self, key: &'static str, cx: &mut Context<Self>) {
        if self.key == key {
            return;
        }
        self.key = key.to_owned();
        let store = self.store.clone();
        let live = self.info.running.then(|| self.info.game_dir.clone()).flatten();
        let name = self.info.profile.clone();
        cx.spawn(async move |this, cx| {
            let done = cx
                .background_executor()
                .spawn(async move {
                    shaders::set_toggle_key(&store, key).map_err(|e| e.to_string())?;
                    if let Some(dir) = live {
                        let profile = store.profile(&name).map_err(|e| e.to_string())?;
                        shaders::push_live(&dir, &shaders::active_file(&store, &profile))
                            .map_err(|e| e.to_string())?;
                    }
                    Ok::<(), String>(())
                })
                .await;
            this.update(cx, |v, cx| {
                v.error = done.err().map(|e| trf!("Couldn't change the key: {e}", e = e));
                v.refresh(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn forced_banner(&self, p: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (r, addon) = self.forced.clone()?;
        let id = match &r {
            ConfigRef::Addon { addon, .. } => Some(addon.clone()),
            ConfigRef::Personal(_) => None,
        };
        Some(
            h_flex()
                .mx(px(GUTTER))
                .mb(px(14.))
                .px(px(14.))
                .py(px(11.))
                .gap(px(12.))
                .items_center()
                .flex_shrink_0()
                .rounded(px(6.))
                .border_1()
                .border_color(p.accent)
                .bg(p.selected)
                .child(Icon::new(IconName::Lock).size(px(16.)).text_color(p.accent))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(tr("You have an addon which forces this shader configuration")),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.text_muted)
                                .child(trf!("{addon} chooses the shaders while it is on.", addon = addon)),
                        ),
                )
                .when_some(id, |d, id| {
                    d.child(
                        Button::new("shaders-forced-addon")
                            .outline()
                            .tap_sm()
                            .label(tr("View addon"))
                            .on_click(cx.listener(move |_, _, _, cx| cx.emit(Event::OpenAddon(id.clone())))),
                    )
                })
                .into_any_element(),
        )
    }
}

impl Render for ShadersView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        self.ensure_loaded(window, cx);
        self.start_pending_rename(window, cx);
        self.sync_slider(window, cx);
        let left = match self.mode {
            Mode::Editor => self.editor_pane(p, cx),
            Mode::Preview => self.preview_pane(p, cx),
            Mode::Documentation => self.docs_pane(p, cx),
        };
        v_flex()
            .size_full()
            .child(self.header(cx))
            .children(self.forced_banner(p, cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .px(px(GUTTER))
                    .pb(px(20.))
                    .gap(px(18.))
                    .items_stretch()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(10.))
                            .child(h_flex().flex_shrink_0().child(self.mode_switch(p, cx)))
                            .child(div().flex_1().min_h_0().child(left)),
                    )
                    .child(self.list_pane(p, cx)),
            )
    }
}

pub(super) const fn list_width() -> f32 {
    LIST_WIDTH
}
