use super::FADE;
use crate::library::{AddonItem, Changes, Source};
use crate::shell::Shell;
use gpui_kit::*;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, UNIX_EPOCH};
use swofty_core::conflicts::{self};
use swofty_core::folders::Entry;
use swofty_core::manifest::MANIFEST_FILE;
use swofty_i18n::trf;

impl Shell {
    pub(super) fn addon_on(&self, m: &AddonItem) -> bool {
        !self.vanilla() && (m.enabled || m.locked)
    }

    pub(super) fn update_ready(&self, m: &AddonItem) -> bool {
        (m.manifest.id == swofty_core::managed::ID && self.updates.plan().addon.is_some())
            || self.catalog_update(&m.manifest.id).is_some()
    }

    /// Why the play bar leaves an import out when an Addon Browser addon has its id, or the id the
    /// Simplified Importer gave it ([`swofty_core::imported::asked_id`]).
    pub(super) fn kept_separate(&self, m: &AddonItem) -> Option<String> {
        if m.source != Source::Importer {
            return None;
        }
        let id = m.manifest.id.as_str();
        if let Some(a) = self.browser.find(id) {
            return Some(trf!(
                "This import has the same id as {name} in the Addon Browser; it is kept separate.",
                name = a.name
            ));
        }
        let asked = swofty_core::imported::asked_id(&m.manifest)?;
        let a = self.browser.find(&asked)?;
        Some(trf!(
            "This import asked for the id {asked}, which {name} in the Addon Browser has; it is kept separate as {id}.",
            asked = asked,
            name = a.name,
            id = id
        ))
    }

    pub(super) fn changed_since(&self, id: &str, built_at: f64) -> bool {
        std::fs::metadata(self.store.addons_dir().join(id).join(MANIFEST_FILE))
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .is_some_and(|t| t.as_secs_f64() > built_at)
    }

    pub(super) fn addon_name(&self, id: &str) -> String {
        self.library
            .addons
            .iter()
            .find(|a| a.manifest.id == id)
            .map_or_else(|| id.to_owned(), |a| a.manifest.name.clone())
    }

    /// Where each listed addon that's on loads, counting from 1; and how many are on.
    pub(super) fn positions(&self) -> (HashMap<&str, usize>, usize) {
        if self.vanilla() {
            return (HashMap::new(), 0);
        }
        let on: HashSet<&str> = self
            .library
            .listed()
            .filter(|m| self.addon_on(m))
            .map(|m| m.manifest.id.as_str())
            .collect();
        let mut out = HashMap::new();
        for e in &self.library.entries {
            let ids = match e {
                Entry::Addon(id) => std::slice::from_ref(id),
                Entry::Folder(_, members) => members.as_slice(),
            };
            for id in ids.iter().filter(|id| on.contains(id.as_str())) {
                let n = out.len() + 1;
                out.insert(id.as_str(), n);
            }
        }
        let n = out.len();
        (out, n)
    }

    pub(super) fn sync_addons(&mut self) {
        let at = self.library.loaded;
        if self.addons.read_at == Some(at) {
            return;
        }
        self.addons.smooth.cancel();
        self.addons.read_at = Some(at);
        self.addons.changes.clear();
        let profile = self.library.profile.name.clone();
        let last = (!self.vanilla())
            .then(|| conflicts::last(&self.store, &profile))
            .flatten();
        let game = self.panel_install().map(|i| i.dir);
        let registry = swofty_core::stdlib::reports::Conflicts::read(&self.store.root, game.as_deref());
        let room = swofty_core::stdlib::reports::Room::read(&self.store.root, game.as_deref());
        self.addons.status = swofty_core::stdlib::reports::status_lines(registry.as_ref(), room.as_ref());
        self.addons.registry_conflicts = registry
            .map(|c| {
                c.registry
                    .into_iter()
                    .map(|e| (e.addons().into_iter().map(str::to_owned).collect(), e))
                    .collect()
            })
            .unwrap_or_default();
        let checked = self.addons.report.take().filter(|r| r.profile == profile);
        self.addons.report = match (last, checked) {
            (Some(l), Some(c)) => Some(if c.built_at > l.built_at { c } else { l }),
            (l, c) => l.or(c),
        };
    }

    pub(super) fn sync_highlight(
        &mut self,
        query: &mut String,
        cx: &mut Context<Self>,
    ) -> Option<(String, f32)> {
        let (id, at) = self.highlight.clone()?;
        let t = at.elapsed();
        if t >= FADE {
            self.highlight = None;
            return None;
        }
        if self.addons.seen != Some(at) {
            let item = self.library.listed().find(|a| a.manifest.id == id);
            if let Some(hidden) = item.map(|m| !self.passes(m, query, None)) {
                self.addons.seen = Some(at);
                self.addons.scroll_pending = true;
                self.open_addon = Some(id.clone());
                if let Some(key) = self
                    .library
                    .place(&id)
                    .filter(|k| self.library.folders.is_collapsed(k))
                {
                    let mut f = self.library.folders.clone();
                    f.set_collapsed(&key, false);
                    if self.store.save_folders(&f).is_ok() {
                        self.library.folders = f;
                    }
                }
                if hidden {
                    self.addons.facets.clear();
                    if !query.is_empty() {
                        query.clear();
                        self.addons.clear_search(cx);
                    }
                }
            }
            self.animate_highlight(cx);
        }
        Some((id, 1. - t.as_secs_f32() / FADE.as_secs_f32()))
    }

    pub(super) fn animate_highlight(&mut self, cx: &mut Context<Self>) {
        if self.addons.animating {
            return;
        }
        self.addons.animating = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(33)).await;
                let done = this
                    .update(cx, |s, cx| {
                        let done = s.highlight.as_ref().is_none_or(|(_, at)| at.elapsed() >= FADE);
                        if done {
                            s.highlight = None;
                            s.addons.animating = false;
                        }
                        cx.notify();
                        done
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        })
        .detach();
    }

    pub(super) fn launching(&self) -> bool {
        use crate::shell::PlayState::{Building, Launching, Preparing, Restoring, Running};
        matches!(
            self.play_state(),
            Preparing | Launching | Restoring | Running | Building
        )
    }

    pub(super) fn changes_of(&mut self, id: &str) -> Changes {
        if let Some(c) = self.addons.changes.get(id) {
            return c.clone();
        }
        let Some(item) = self.library.addons.iter().find(|a| a.manifest.id == id) else {
            return Changes::default();
        };
        let c = Changes::read(&self.store, &self.library.profile, &self.library.on(), item);
        self.addons.changes.insert(id.to_owned(), c.clone());
        c
    }
}
