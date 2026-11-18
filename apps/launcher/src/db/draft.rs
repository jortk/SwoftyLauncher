use std::sync::Arc;
use std::time::Duration;

use crate::notices::Notification;
use gpui_kit::*;
use swofty_core::dbedit::draft::{Opened, Written};
use swofty_core::dbedit::package::{self, PackageInfo};
use swofty_i18n::{tr, trf};

use super::import::report_text;
use super::{Load, Nav, Slots, Then, guarded, lock, short, text};
use crate::shell::{Page, Shell};

const PERSIST_DELAY: Duration = Duration::from_millis(300);

impl Shell {
    pub(crate) fn db_count(&self) -> usize {
        self.db.draft.unsaved()
    }

    pub(crate) fn db_profile_changed(&mut self, cx: &mut Context<Self>) {
        self.db_refresh(cx);
    }

    pub(crate) fn db_addon(&self, id: &str) -> Option<&PackageInfo> {
        self.db.addons.iter().find(|p| p.id == id)
    }

    /// The installed addon the draft edits, if any.
    pub(crate) fn db_bound(&self) -> Option<&PackageInfo> {
        self.db.draft.addon.as_deref().and_then(|id| self.db_addon(id))
    }

    /// The draft's addon name, or "the new addon".
    pub(crate) fn db_draft_name(&self) -> String {
        match (&self.db.draft.addon, self.db_bound()) {
            (_, Some(info)) => info.name.clone(),
            (Some(id), None) => id.clone(),
            (None, None) => "the new addon".into(),
        }
    }

    pub(crate) fn db_locked(&self) -> bool {
        self.db.addon_error.is_some()
    }

    pub(crate) fn db_enabled(&self, id: &str) -> bool {
        self.library.profile.enabled().any(|e| e == id)
    }

    /// The DB Editor addons on in the active profile, in load order.
    pub(crate) fn db_stack(&self) -> Vec<String> {
        self.library
            .profile
            .enabled()
            .filter(|id| self.db_addon(id).is_some())
            .map(str::to_owned)
            .collect()
    }

    pub(crate) fn db_refresh(&mut self, cx: &mut Context<Self>) {
        let mut addons: Vec<PackageInfo> = self
            .library
            .addons
            .iter()
            .filter(|a| a.package)
            .map(|a| {
                let m = &a.manifest;
                PackageInfo {
                    id: m.id.clone(),
                    name: m.name.clone(),
                    version: m.version.to_string(),
                    author: m.author.clone(),
                    description: m.description.clone().unwrap_or_default(),
                    changes: 0,
                    game_build: String::new(),
                }
            })
            .collect();
        addons.sort_by_key(|p| p.name.to_lowercase());
        self.db.addons = addons;
        if !self.db.draft_read {
            self.db_read_draft();
        }
        self.db_sync_addon(false, cx);
        self.db_load_others(cx);
    }

    // Reads every enabled DB Editor addon off the UI thread for "Also changed by".
    fn db_load_others(&mut self, cx: &mut Context<Self>) {
        self.db.others_gen += 1;
        let at = self.db.others_gen;
        let (store, profile) = (self.store.clone(), self.library.profile.name.clone());
        cx.spawn(async move |this, cx| {
            let others = cx
                .background_executor()
                .spawn(async move {
                    guarded(|| package::enabled_changes(&store, &profile).map_err(text)).unwrap_or_default()
                })
                .await;
            this.update(cx, |s, cx| {
                if s.db.others_gen == at {
                    s.db.others = Arc::new(others);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    // Reads the autosaved draft once; a file that does not read is set aside.
    fn db_read_draft(&mut self) {
        self.db.draft_read = true;
        let store = self.store.clone();
        let opened = guarded(|| Ok(lock(&self.db.file).open(&store)));
        let body = match opened {
            Ok(Opened::Read(d)) => {
                self.db.draft = d;
                None
            }
            Ok(Opened::SetAside { error, kept }) => Some(trf!(
                "{error}\nIt was kept as {path}",
                error = error,
                path = kept.display()
            )),
            Ok(Opened::Stuck { error }) => {
                self.db.save_error = Some(trf!(
                    "Your changes are not saved to the draft until the damaged draft file is moved: {error}",
                    error = error
                ));
                Some(error)
            }
            Err(e) => Some(e),
        };
        if let Some(body) = body {
            self.db.toasts.push(
                Notification::error(body)
                    .title(tr("The DB Editor draft could not be read"))
                    .autohide(false),
            );
        }
        self.db.reindex();
    }

    /// Writes the draft to disk off the UI thread once changes stop for a moment.
    pub(crate) fn db_persist(&mut self, cx: &mut Context<Self>) {
        self.db.unwritten = true;
        self.db.persist_gen += 1;
        let at = self.db.persist_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(PERSIST_DELAY).await;
            this.update(cx, |s, cx| {
                if s.db.persist_gen == at {
                    s.db_flush(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// Starts writing the draft's unwritten changes now.
    pub(crate) fn db_flush(&mut self, cx: &mut Context<Self>) {
        if !self.db.unwritten || self.db.writing || self.db.newer {
            return;
        }
        self.db.unwritten = false;
        self.db.writing = true;
        let (store, draft, file) = (self.store.clone(), self.db.draft.clone(), self.db.file.clone());
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { guarded(|| lock(&file).write(&store, &draft).map_err(text)) })
                .await;
            this.update(cx, |s, cx| {
                s.db.writing = false;
                s.db_written(result, cx);
            })
            .ok();
        })
        .detach();
    }

    fn db_written(&mut self, result: Result<Written, String>, cx: &mut Context<Self>) {
        match result {
            Ok(Written::Saved) => self.db.save_error = None,
            Ok(Written::Newer) => {
                self.db.unwritten = true;
                self.db.newer = true;
            }
            Err(e) => {
                self.db.unwritten = true;
                self.db.save_error = Some(trf!("Your changes could not be saved to the draft: {e}", e = e));
                cx.notify();
                return;
            }
        }
        if self.db.unwritten && !self.db.newer {
            self.db_persist(cx);
        }
        cx.notify();
    }

    /// Writes pending draft changes when the player leaves the DB Editor.
    pub(crate) fn db_page_change(&mut self, to: Page, cx: &mut Context<Self>) {
        if self.page == Page::Database && to != Page::Database {
            self.db_flush(cx);
        }
        // Other pages change what choices offer, such as Driver Manager's drivers.
        if self.page != Page::Database && to == Page::Database {
            self.db.stale_choices = true;
            self.db.view_want = None;
            self.db_refresh_view(cx);
        }
    }

    /// Writes the draft before the launcher quits.
    pub(crate) fn db_flush_now(&mut self) {
        if !self.db.unwritten || self.db.newer {
            return;
        }
        let (store, draft) = (self.store.clone(), &self.db.draft);
        if let Ok(Written::Saved) = guarded(|| lock(&self.db.file).write(&store, draft).map_err(text)) {
            self.db.unwritten = false;
        }
    }

    /// Replaces this window's draft with the newer one another window wrote.
    pub(crate) fn db_take_newer(&mut self, cx: &mut Context<Self>) {
        self.db.newer = false;
        self.db.unwritten = false;
        self.db.persist_gen += 1;
        self.db.draft_read = false;
        self.db_read_draft();
        self.db.seen = None;
        self.db.errors.clear();
        self.db.cell = None;
        self.db.slots = Slots::default();
        self.db_sync_addon(true, cx);
        self.db_changed(None, false, cx);
    }

    /// Keeps this window's draft, writing it over the one another window wrote.
    pub(crate) fn db_keep_mine(&mut self, cx: &mut Context<Self>) {
        let store = self.store.clone();
        guarded(|| {
            lock(&self.db.file).adopt(&store);
            Ok(())
        })
        .ok();
        self.db.newer = false;
        self.db_persist(cx);
        cx.notify();
    }

    // Another window wrote the draft while this one had nothing to write: take it.
    fn db_follow_draft(&mut self, cx: &mut Context<Self>) {
        if self.db.unwritten || self.db.writing || self.db.newer {
            return;
        }
        let store = self.store.clone();
        if guarded(|| Ok(lock(&self.db.file).is_newer(&store))).unwrap_or(false) {
            self.db_take_newer(cx);
            self.db.toasts.push(Notification::info(tr(
                "Another launcher window changed the draft, so it was reloaded.",
            )));
        }
    }

    // Follows the draft's addon on disk: removed, damaged, or saved by another writer.
    fn db_sync_addon(&mut self, quiet: bool, cx: &mut Context<Self>) {
        let Some(id) = self.db.draft.addon.clone() else {
            if self.db.addon_error.take().is_some() {
                self.db_changed(None, false, cx);
            }
            return;
        };
        let store = self.store.clone();
        if !guarded(|| Ok(package::is_package(&store, &id))).unwrap_or(false) {
            let name = self.db_draft_name();
            if self.db.draft.is_dirty() {
                let edits = self.db.draft.edits.clone();
                self.db.draft.clear();
                self.db.draft.edits = edits;
                self.db.draft.name = Some(name.clone());
                self.db.toasts.push(Notification::warning(trf!(
                    "{name} was removed. Your unsaved changes stay in the DB Editor as a new addon.",
                    name = name
                )));
            } else {
                self.db.draft.clear();
            }
            self.db.addon_error = None;
            self.db.seen = None;
            self.db_persist(cx);
            self.db_changed(None, false, cx);
            return;
        }
        let stamp = guarded(|| Ok(package::modified(&store, &id))).ok().flatten();
        if self.db.seen == Some(stamp) {
            return;
        }
        self.db.seen = Some(stamp);
        if self.db.addon_error.is_none() && stamp == self.db.draft.stamp {
            return;
        }
        match guarded(|| package::Package::open(&store, &id).map_err(text)) {
            Ok(pkg) => {
                let reload = self.db.addon_error.is_none() && self.db.draft.stamp.is_some();
                let dirty = self.db.draft.is_dirty();
                self.db.draft.rebase(pkg.edits, stamp);
                self.db.addon_error = None;
                self.db_persist(cx);
                if reload && !quiet {
                    let name = self.db_draft_name();
                    self.db.toasts.push(Notification::info(if dirty {
                        trf!(
                            "{name} changed on disk and was reloaded. Your unsaved changes are kept on top.",
                            name = name
                        )
                    } else {
                        trf!("{name} changed on disk and was reloaded.", name = name)
                    }));
                }
            }
            Err(e) => self.db.addon_error = Some(e),
        }
        self.db.errors.clear();
        self.db.cell = None;
        self.db.slots = Slots::default();
        self.db_changed(None, false, cx);
    }

    /// Re-reads the draft's addon after this window changed its files, without a notice.
    pub(crate) fn db_quiet_reload(&mut self, cx: &mut Context<Self>) {
        self.db.seen = None;
        self.db_sync_addon(true, cx);
    }

    /// Try again on a draft whose addon did not read.
    pub(crate) fn db_retry(&mut self, cx: &mut Context<Self>) {
        self.db.seen = None;
        self.db_sync_addon(false, cx);
        cx.notify();
    }

    /// The poll's check: another writer (the CLI, a second window) changed the draft or its addon.
    pub(crate) fn db_poll(&mut self, cx: &mut Context<Self>) {
        if self.page != crate::shell::Page::Database {
            return;
        }
        self.db_follow_draft(cx);
        if self.db.draft.addon.is_some() {
            self.db_sync_addon(false, cx);
        }
    }

    pub(super) fn db_ready_to_edit(&mut self, cx: &mut Context<Self>) -> bool {
        self.db_sync_addon(false, cx);
        self.db.catalog().is_some() && self.db.addon_error.is_none()
    }

    pub(crate) fn db_turn_on(&mut self, id: &str) -> Result<(), String> {
        let registry = self.library.registry.clone();
        self.store
            .update_profile(&self.library.profile.name, |p| registry.set_enabled(p, id, true))
            .map_err(text)
    }

    /// Opens the DB Editor addon `id` as the draft. Unsaved changes in the
    /// draft ask Save / Discard / Cancel first.
    pub(crate) fn db_edit_addon(&mut self, id: &str, cx: &mut Context<Self>) {
        self.open_page(Page::Database, cx);
        if self.db.draft.addon.as_deref() == Some(id) {
            self.db_retry(cx);
        } else if self.db.draft.is_dirty() {
            self.db.pending_open = Some(id.to_owned());
        } else {
            self.db_then(Then::Open(id.to_owned()), cx);
        }
        cx.notify();
    }

    pub(crate) fn db_then(&mut self, then: Then, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let mut toast = None;
        match then {
            Then::Open(id) => {
                let draft = &mut self.db.draft;
                self.db.addon_error = guarded(|| draft.open(&store, &id).map_err(text)).err();
                if self.db.addon_error.is_some() {
                    self.db.draft.clear();
                    self.db.draft.addon = Some(id);
                }
            }
            Then::New => self.db.draft.clear(),
            Then::Load {
                file,
                name,
                edits,
                report,
            } => {
                self.db.draft.clear();
                self.db.draft.edits = edits;
                self.db.draft.name = Some(name);
                self.db.addon_error = None;
                let note = if report.format == "bin" && report.skipped.len() > 1 {
                    Notification::warning(report_text(&report)).autohide(false)
                } else {
                    Notification::success(report_text(&report))
                };
                toast = Some(note.title(trf!(
                    "Opened {file} as a new addon. Review it, then Save as Addon.",
                    file = file
                )));
                self.db_go(Nav::Changes, cx);
            }
        }
        self.db.seen = Some(self.db.draft.stamp);
        self.db.errors.clear();
        self.db.cell = None;
        self.db.slots = Slots::default();
        self.db_persist(cx);
        self.db.toasts.extend(toast);
        self.db_load_others(cx);
        self.db_changed(None, false, cx);
    }

    /// "Also changed by …" for a key another enabled DB Editor addon changes.
    pub(crate) fn db_others(&self, key: u64) -> Option<String> {
        let editing = self.db.draft.addon.as_deref();
        let others: Vec<&(String, String)> = self
            .db
            .others
            .get(&key)?
            .iter()
            .filter(|(id, _)| Some(id.as_str()) != editing)
            .collect();
        let last = others.last()?;
        let name = |id: &str| {
            self.db_addon(id)
                .map_or_else(|| id.to_owned(), |p| p.name.clone())
        };
        let list: Vec<String> = others
            .iter()
            .map(|(id, v)| format!("{} ({})", name(id), short(v)))
            .collect();
        let order: Vec<&str> = self.library.profile.enabled().collect();
        let at = |id: &str| order.iter().position(|o| *o == id);
        let mine = self.db.edit(key).is_some();
        let later = match editing {
            Some(id) => at(id).is_some_and(|i| at(&last.0).is_none_or(|j| i > j)),
            None => true,
        };
        let wins = if mine && later {
            Some(tr("your change wins").to_owned())
        } else if mine || others.len() > 1 {
            Some(trf!("{name} wins", name = name(&last.0)))
        } else {
            None
        };
        let mut text = trf!("Also changed by {names}", names = list.join(", "));
        if let Some(w) = wins {
            text.push_str(&format!(" · {w}"));
        }
        Some(text)
    }

    pub(crate) fn db_shared(&self, key: u64) -> bool {
        let editing = self.db.draft.addon.as_deref();
        self.db
            .others
            .get(&key)
            .is_some_and(|l| l.iter().any(|(id, _)| Some(id.as_str()) != editing))
    }

    // A new game folder or build: drop the catalog and everything that points into it.
    pub(super) fn db_forget_game(&mut self) {
        let db = &mut self.db;
        db.load = Load::Idle;
        db.game = None;
        db.view = None;
        db.view_want = None;
        db.rev += 1;
        db.row = None;
        db.key = None;
        db.reveal = None;
        db.cell = None;
        db.hits.clear();
        db.raw_rows = None;
        db.unnamed_rows = Arc::new(Vec::new());
        db.scopes.clear();
        db.slots = Slots::default();
        if matches!(db.nav, Nav::Raw(_)) {
            db.nav = Nav::Raw(0);
        }
    }
}
