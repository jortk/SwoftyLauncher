use std::collections::HashSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::notices::{Notification, Notify};
use crate::scale::px;
use gpui_kit::assets::IconName;
use gpui_kit::*;
use swofty_core::baseline::Identity;
use swofty_core::install::Game;
use swofty_core::launcher::Progress;
use swofty_core::profile::Profile;
use swofty_core::registry::Host;
use swofty_core::store::Store;
use swofty_core::view::{self, Art, Effect, Event, Tone, View};
use swofty_i18n::trf;

use crate::game::Install;
use crate::library::SectionItem;
use crate::pages::{badge, monogram};
use crate::shell::Shell;
use crate::theme::Palette;

mod intake;
mod page;

pub(crate) type Shared = Arc<Mutex<Option<(f32, String)>>>;

pub(crate) struct Reporter(pub(crate) Shared);

impl Progress for Reporter {
    fn report(&self, done: f32, text: &str) {
        if let Ok(mut p) = self.0.lock() {
            *p = Some((done, text.to_owned()));
        }
    }
}

pub(crate) fn game(store: &Store, install: Option<&Install>) -> Option<Game> {
    let install = install?;
    let identity = Identity::detect_cached(&store.root, &install.dir, install.build_id).ok()?;
    Some(Game {
        dir: install.dir.clone(),
        user_reg: None,
        identity,
    })
}

fn host(store: &Store, addon: &str, profile: &Profile, game: Option<&Game>) -> Result<Host, String> {
    Host::new(store, addon, profile, game).map_err(|e| e.to_string())
}

pub(crate) fn icon(i: view::Icon) -> IconName {
    use view::Icon as I;
    match i {
        I::Activity => IconName::Activity,
        I::ArrowRightLeft => IconName::ArrowRightLeft,
        I::ChevronDown => IconName::ChevronDown,
        I::ChevronUp => IconName::ChevronUp,
        I::CircleAlert => IconName::CircleAlert,
        I::CircleCheck => IconName::CircleCheck,
        I::Copy => IconName::Copy,
        I::Eraser => IconName::Eraser,
        I::FolderOpen => IconName::FolderOpen,
        I::Gamepad => IconName::Gamepad2,
        I::Layers => IconName::Layers,
        I::Lock => IconName::Lock,
        I::Map => IconName::Map,
        I::Package => IconName::Package,
        I::Pencil => IconName::Pencil,
        I::Play => IconName::Play,
        I::Plus => IconName::Plus,
        I::Puzzle => IconName::Puzzle,
        I::Settings => IconName::Settings,
        I::ShieldCheck => IconName::ShieldCheck,
        I::Trash => IconName::Trash,
        I::Upload => IconName::Upload,
        I::Calendar => IconName::Calendar,
        I::Car => IconName::Car,
        I::Clock => IconName::Clock,
        I::CloudRain => IconName::CloudRain,
        I::Flag => IconName::Flag,
        I::Gauge => IconName::Gauge,
        I::History => IconName::RotateCcw,
        I::Info => IconName::Info,
        I::Refresh => IconName::RefreshCw,
        I::Save => IconName::Save,
        I::Search => IconName::Search,
        I::Timer => IconName::Timer,
        I::TrendingDown => IconName::TrendingDown,
        I::TrendingUp => IconName::TrendingUp,
        I::TriangleAlert => IconName::TriangleAlert,
        I::Trophy => IconName::Trophy,
        I::Undo => IconName::Undo2,
        I::User => IconName::User,
        I::Users => IconName::Users,
        I::Wallet => IconName::Wallet,
        I::Wrench => IconName::Wrench,
        I::Zap => IconName::Zap,
    }
}

pub(crate) fn tone(t: Tone, p: Palette) -> Hsla {
    match t {
        Tone::Neutral => p.text,
        Tone::Muted => p.text_muted,
        Tone::Accent => p.accent,
        Tone::Good => p.success,
        Tone::Warn => p.warning,
        Tone::Bad => p.error,
    }
}

pub(crate) fn art(a: &Art, size: f32, p: Palette) -> AnyElement {
    match a {
        Art::None => badge(IconName::Package, size, p).into_any_element(),
        Art::Monogram(text) => monogram(text, size, p).into_any_element(),
        Art::Image(path) => div()
            .size(px(size))
            .flex_shrink_0()
            .rounded(px(5.))
            .overflow_hidden()
            .border_1()
            .border_color(p.border_variant)
            .child(img(path.clone()).size_full().object_fit(ObjectFit::Cover))
            .into_any_element(),
    }
}

pub(crate) fn section_key(s: &SectionItem) -> String {
    format!("{}/{}", s.addon, s.spec.id)
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

impl Shell {
    pub(crate) fn refresh_views(&mut self, cx: &mut Context<Self>) {
        self.fetch_views(None, cx);
    }

    pub(crate) fn fetch_views(&mut self, only: Option<usize>, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let library = &self.library;
        let registry = library.registry.clone();
        let profile = library.profile.clone();
        let install = self.panel_install();
        let open = self.open_section();
        let keys: HashSet<String> = library.sections.iter().map(section_key).collect();
        if self.views_profile != profile.name {
            self.views_profile = profile.name.clone();
            self.views.clear();
        }
        self.views.retain(|k, _| keys.contains(k));
        let prefetch = self.detected;
        let sections: Vec<(String, String, bool)> = library
            .sections
            .iter()
            .enumerate()
            .filter(|(i, _)| only.is_none_or(|o| o == *i))
            .filter(|(i, s)| {
                open == Some(*i) || prefetch || !registry.get(&s.addon).is_some_and(|e| e.helper)
            })
            .map(|(_, s)| {
                let key = section_key(s);
                let cached = self.views.contains_key(&key);
                (s.addon.clone(), s.spec.id.clone(), cached)
            })
            .collect();
        if sections.is_empty() {
            return;
        }
        self.view_gen += 1;
        let generation = self.view_gen;
        cx.spawn(async move |this, cx| {
            let views: Vec<(String, Result<View, String>)> = cx
                .background_executor()
                .spawn(async move {
                    let game = game(&store, install.as_ref());
                    sections
                        .into_iter()
                        .filter_map(|(addon, section, cached)| {
                            let view = registry
                                .get(&addon)
                                .ok_or_else(|| trf!("{addon} is gone", addon = addon))
                                .and_then(|e| {
                                    let host = host(&store, &addon, &profile, game.as_ref())?;
                                    let view = match (&e.plugin, cached) {
                                        (Some(plugin), true) => {
                                            let view = plugin.try_view(&host.cx(), &section);
                                            if view.is_none() {
                                                plugin.mark_stale(Some(&section));
                                            }
                                            view
                                        }
                                        _ => Some(e.addon.view(&host.cx(), &section)),
                                    };
                                    Ok(view.map(|v| v.map_err(|e| e.to_string())))
                                });
                            match view {
                                Ok(None) => None,
                                Ok(Some(view)) => Some((format!("{addon}/{section}"), view)),
                                Err(e) => Some((format!("{addon}/{section}"), Err(e))),
                            }
                        })
                        .collect()
                })
                .await;
            this.update(cx, |s, cx| {
                for (key, view) in views {
                    if s.view_gens.get(&key).is_some_and(|g| *g > generation) {
                        continue;
                    }
                    s.view_gens.insert(key.clone(), generation);
                    s.views.insert(key, view.map(Arc::new));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn live_poll(&mut self, cx: &mut Context<Self>) {
        self.live_n += 1;
        self.refresh_stale(cx);
        if self.live_n.is_multiple_of(4) {
            self.tick(cx);
        }
    }

    fn refresh_stale(&mut self, cx: &mut Context<Self>) {
        let open = self.open_section();
        let mut due = Vec::new();
        let mut seen = HashSet::new();
        for s in &self.library.sections {
            if !seen.insert(s.addon.clone()) {
                continue;
            }
            let Some(plugin) = self.library.registry.get(&s.addon).and_then(|e| e.plugin.clone()) else {
                continue;
            };
            if plugin.busy() {
                continue;
            }
            let Some(stale) = plugin.take_stale() else {
                continue;
            };
            for (i, t) in self.library.sections.iter().enumerate() {
                let shown = open == Some(i) || self.views.contains_key(&section_key(t));
                if t.addon == s.addon && shown && stale.covers(&t.spec.id) {
                    due.push(i);
                }
            }
        }
        for i in due {
            self.fetch_views(Some(i), cx);
        }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let Some(i) = self.open_section() else {
            return;
        };
        let Some(s) = self.library.sections.get(i) else {
            return;
        };
        let ticks = matches!(self.views.get(&section_key(s)), Some(Ok(v)) if v.tick);
        let Some(plugin) = self.library.registry.get(&s.addon).and_then(|e| e.plugin.clone()) else {
            return;
        };
        if !ticks || self.ticking || !plugin.running() {
            return;
        }
        self.ticking = true;
        let store = self.store.clone();
        let profile = self.library.profile.clone();
        let install = self.panel_install();
        let (addon, section) = (s.addon.clone(), s.spec.id.clone());
        cx.spawn(async move |this, cx| {
            let failed = cx
                .background_executor()
                .spawn(async move {
                    let game = game(&store, install.as_ref());
                    let Ok(host) = host(&store, &addon, &profile, game.as_ref()) else {
                        return false;
                    };
                    if let Some(Err(e)) = plugin.try_tick(&host.cx(), &section) {
                        eprintln!("swofty-launcher: {addon} tick: {e}");
                    }
                    plugin.failure().is_some()
                })
                .await;
            this.update(cx, |s, cx| {
                s.ticking = false;
                if failed {
                    s.fetch_views(Some(i), cx);
                }
                s.refresh_stale(cx);
            })
            .ok();
        })
        .detach();
    }

    /// Sends `startup` to the first section of each addon that asks for it.
    pub(crate) fn run_startup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut seen = std::collections::BTreeSet::new();
        let targets: Vec<(String, String)> = self
            .library
            .sections
            .iter()
            .filter(|s| seen.insert(s.addon.clone()))
            .filter(|s| {
                self.library.addons.iter().any(|a| {
                    a.manifest.id == s.addon && a.manifest.launcher.as_ref().is_some_and(|l| l.startup)
                })
            })
            .map(|s| (s.addon.clone(), s.spec.id.clone()))
            .collect();
        for (addon, section) in targets {
            let control = format!("{addon}/{section}/startup");
            self.send(&addon, &section, control, Event::Startup {}, window, cx);
        }
    }

    pub(crate) fn send(
        &mut self,
        addon: &str,
        section: &str,
        control: String,
        event: Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.library.registry.get(addon).cloned() else {
            return;
        };
        let store = self.store.clone();
        let profile = self.library.profile.clone();
        let install = self.panel_install();
        let (addon, section) = (addon.to_owned(), section.to_owned());
        let feed = crate::tasks::Feed::default();
        let sink = crate::tasks::Sink(feed.clone());
        self.busy.insert(control.clone());
        self.follow_tasks(control.clone(), feed.clone(), cx);
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let id = addon.clone();
            let effect = cx
                .background_executor()
                .spawn(async move {
                    let game = game(&store, install.as_ref());
                    let host = host(&store, &addon, &profile, game.as_ref())?;
                    entry
                        .addon
                        .event(&host.cx().with_tasks(&sink), &section, event)
                        .map_err(|e| e.to_string())
                })
                .await;
            this.update_in(cx, |s, window, cx| {
                s.busy.remove(&control);
                s.settled(&control, &effect);
                let effect = s.finish_tasks(&control, &feed, effect, cx);
                s.effect(&id, effect, window, cx);
            })
            .ok();
        })
        .detach();
    }

    fn effect(
        &mut self,
        addon: &str,
        effect: Result<Effect, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match effect {
            Ok(Effect::None) => cx.notify(),
            Ok(Effect::Reload) => self.reload(cx),
            Ok(Effect::Notice(text)) => {
                window.notify(Notification::info(text), cx);
                self.reload(cx);
            }
            Ok(Effect::SetAddonEnabled(on)) => self.set_addon_enabled(addon, on, window, cx),
            Ok(Effect::Open(path)) => {
                cx.open_with_system(&path);
                cx.notify();
            }
            Ok(Effect::ShowAddon(shown)) => {
                if let Some(text) = shown.notice() {
                    window.notify(Notification::info(text.to_owned()), cx);
                }
                self.reload(cx);
                self.show_addon(shown.id(), cx);
            }
            Err(e) => {
                crate::app_log::action("addon action").addon(addon).failed(&e);
                window.notify(Notification::error(e), cx);
                cx.notify();
            }
        }
    }
}
