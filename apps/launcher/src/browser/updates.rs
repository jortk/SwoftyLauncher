use std::collections::{BTreeMap, HashSet};

use crate::notices::{Notification, Notify};
use gpui_kit::assets::IconName;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, Sizable, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::catalog::{self, Addon, Update};
use swofty_i18n::{fill, tr, trf, trn};

use super::*;
use crate::library::AddonItem;
use crate::play_bar::link;
use crate::scale::px;
use crate::shell::{PlayState, Shell};
use crate::theme::Palette;

/// The updates for addons in `on` that haven't failed at the version offered in `held`, and whose
/// version the player didn't skip.
pub(super) fn due_updates<'a>(
    updates: &'a [Update],
    on: &HashSet<&str>,
    held: &BTreeMap<String, (semver::Version, String)>,
    skipped: &BTreeMap<String, semver::Version>,
) -> Vec<&'a Update> {
    updates
        .iter()
        .filter(|u| on.contains(u.id.as_str()))
        .filter(|u| held.get(&u.id).is_none_or(|(v, _)| *v != u.to.version))
        .filter(|u| skipped.get(&u.id) != Some(&u.to.version))
        .collect()
}

/// Whether the play bar updates `a`: it's on, or it's shader-only and so is in use without a switch.
pub(super) fn takes_updates(a: &AddonItem) -> bool {
    a.enabled || a.locked || a.manifest.is_shader_only()
}

/// The installed addons the play bar updates: the ones that take updates in a modded profile, or every
/// one when `off_too` is set, as the Update turned-off addons setting does.
pub(super) fn updated_addons(addons: &[AddonItem], vanilla: bool, off_too: bool) -> HashSet<&str> {
    addons
        .iter()
        .filter(|a| off_too || (!vanilla && takes_updates(a)))
        .map(|a| a.manifest.id.as_str())
        .collect()
}

/// Minimal identity for an update that disappeared from the current catalog.
pub(super) fn update_identity(update: &Update) -> Addon {
    Addon {
        id: update.id.clone(),
        name: update.name.clone(),
        author: String::new(),
        summary: String::new(),
        description: String::new(),
        category: None,
        kind: Vec::new(),
        homepage: None,
        repository: None,
        icon: None,
        publisher: None,
        publishers: Vec::new(),
        downloads: 0,
        downloads_30d: 0,
        stars: 0,
        trend: None,
        created_at: String::new(),
        updated_at: String::new(),
        latest: None,
        versions: Vec::new(),
        media: Vec::new(),
    }
}

/// Whether a held update represents a real failure rather than a successful
/// install waiting for the asynchronous catalog refresh.
pub(super) fn held_failure(error: &str) -> bool {
    !error.is_empty()
}

impl Shell {
    /// The addon updates the play bar offers: every outdated registry addon that's on in the active
    /// profile or shader-only, or every installed one with Update turned-off addons on, except the
    /// ones that failed at this version and the versions skipped.
    pub(crate) fn addon_updates(&self) -> Vec<&Update> {
        let on = updated_addons(&self.library.addons, self.vanilla(), self.update_off_addons);
        due_updates(
            &self.browser.updates,
            &on,
            &self.browser.held,
            &self.browser.skipped,
        )
    }

    /// Whether the play bar leaves the update of `id` out because its version was skipped.
    pub(crate) fn update_skipped(&self, id: &str) -> bool {
        self.catalog_update(id)
            .is_some_and(|u| self.browser.skipped.get(id) == Some(&u.to.version))
    }

    /// Skips the versions the play bar offers, so Play comes back; the next version of each
    /// is offered again.
    pub(super) fn skip_addon_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let versions: Vec<(String, semver::Version)> = self
            .addon_updates()
            .into_iter()
            .map(|u| (u.id.clone(), u.to.version.clone()))
            .collect();
        if versions.is_empty() {
            return;
        }
        let names: Vec<String> = versions.iter().map(|(id, v)| format!("{id} {v}")).collect();
        match catalog::source::skip(&self.store.root, versions) {
            Ok(all) => {
                let names = names.join(", ");
                catalog::log(&self.store.root, &format!("skipped updates: {names}"));
                self.browser.skipped = all;
            }
            Err(e) => window.notify(
                Notification::error(e.to_string()).title(tr("Could not skip the addon updates")),
                cx,
            ),
        }
        cx.notify();
    }

    pub(super) fn updating_addons(&self) -> bool {
        self.browser.batch.as_ref().is_some_and(|b| !b.left.is_empty())
    }

    pub(crate) fn addon_update_takes_play(&self, state: &PlayState) -> bool {
        self.updating_addons()
            || (matches!(state, PlayState::Ready | PlayState::Crashed(_)) && !self.addon_updates().is_empty())
    }

    pub(super) fn update_addons(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.play_state(),
            PlayState::Running | PlayState::Elsewhere(_) | PlayState::Launching | PlayState::Preparing
        ) {
            window.notify(Notification::error(tr("Close F1 25 before updating addons.")), cx);
            return;
        }
        let building = self.build.clone();
        let (held, updates): (Vec<Update>, Vec<Update>) = self
            .addon_updates()
            .into_iter()
            .cloned()
            .partition(|u| building.as_ref().is_some_and(|b| b.holds(&u.id)));
        if !held.is_empty() {
            window.notify(Notification::warning(crate::update::BUILDING), cx);
        }
        if updates.is_empty() || self.updating_addons() {
            return;
        }
        let ids: Vec<_> = updates.iter().map(|u| u.id.clone()).collect();
        catalog::log(&self.store.root, &format!("updating addons: {}", ids.join(", ")));
        self.browser.batch = Some(Batch {
            left: ids.into_iter().collect(),
            ..Batch::default()
        });
        for update in updates {
            if !self.enqueue_catalog(&update.id, cx) {
                let error =
                    tr("This update is no longer available or another version is already queued.").to_owned();
                let addon = self
                    .browser
                    .find(&update.id)
                    .cloned()
                    .unwrap_or_else(|| update_identity(&update));
                self.browser.queue.record_failure(
                    update.id.clone(),
                    (addon, update.to.clone(), self.library.profile.name.clone()),
                    error.clone(),
                );
                if let Some(batch) = &mut self.browser.batch {
                    batch.left.remove(&update.id);
                    batch.failed.push((update.name, error));
                }
            }
        }
        if self
            .browser
            .batch
            .as_ref()
            .is_some_and(|batch| batch.left.is_empty())
        {
            self.finish_addon_updates();
        }
    }

    pub(super) fn retry_addon_updates(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.browser.held.clear();
        self.update_addons(window, cx);
    }

    pub(super) fn finish_addon_updates(&mut self) {
        let Some(b) = self.browser.batch.take() else {
            return;
        };
        let summary = b.summary();
        catalog::log(&self.store.root, &summary.replace('\n', "; "));
        let title = fill(
            &trn(
                b.total(),
                "Updated {done} of {count} addon",
                "Updated {done} of {count} addons",
            ),
            &[("done", b.updated.len().to_string())],
        );
        self.browser.toasts.push(if b.failed.is_empty() {
            Notification::success(summary).title(title)
        } else {
            Notification::warning(summary).title(title)
        });
    }

    pub(super) fn batch_progress(&self) -> Option<(f32, String)> {
        let b = self.browser.batch.as_ref()?;
        let total = b.total().max(1) as f32;
        let mut done = (b.updated.len() + b.failed.len()) as f32;
        let mut detail = None;
        for id in &b.left {
            if let Some(job) = self.browser.working.get(id) {
                let pr = job.progress();
                done += pr.fraction();
                if detail.is_none() {
                    let name = self.browser.find(id).map_or(id.as_str(), |a| a.name.as_str());
                    detail = Some(format!("{name} · {}", pr.short()));
                }
            }
        }
        Some((done / total, detail.unwrap_or_else(|| tr("Starting…").to_owned())))
    }

    pub(crate) fn addon_update_button(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let running = self.updating_addons();
        let due = self.addon_updates();
        let (label, sub, progress) = match self.batch_progress().filter(|_| running) {
            Some((done, detail)) => (
                trf!("Updating {percent}", percent = format::percent(done)),
                detail,
                Some(done),
            ),
            None => (
                trf!("Update addons ({count})", count = due.len()),
                match due.as_slice() {
                    [one] => format!("{} {} → {}", one.name, one.installed, one.to.version),
                    many => trn(many.len(), "{count} addon update", "{count} addon updates"),
                },
                None,
            ),
        };
        div()
            .id("play")
            .relative()
            .overflow_hidden()
            .h(px(52.))
            .w(px(crate::update::BUTTON_WIDTH))
            .flex_shrink_0()
            .rounded(px(9.))
            .bg(if running { p.elevated } else { p.accent })
            .text_color(if running { p.text } else { p.on_accent })
            .flex()
            .items_center()
            .gap(px(10.))
            .px(px(14.))
            .when(running, |d| d.border_1().border_color(p.border))
            .when(!running, |d| {
                d.cursor_pointer()
                    .hover(move |s| s.bg(p.accent_hover))
                    .on_click(cx.listener(|this, _, window, cx| this.update_addons(window, cx)))
            })
            .map(|d| {
                if running {
                    d.child(Spinner::new().icon(IconName::LoaderCircle).with_size(px(18.)))
                } else {
                    d.child(Icon::new(IconName::CircleArrowUp).size(px(18.)))
                }
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .truncate()
                            .text_color(if running { p.text_muted } else { p.on_accent })
                            .when(!running, |d| d.opacity(0.85))
                            .child(sub),
                    ),
            )
            .when_some(progress, |d, done| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .bottom_0()
                        .h(px(4.))
                        .w(relative(done.clamp(0., 1.)))
                        .bg(p.accent),
                )
            })
            .into_any_element()
    }

    pub(crate) fn addon_update_status(
        &self,
        state: &PlayState,
        row: Div,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let dot = |c: Hsla| div().size(px(7.)).rounded_full().bg(c).flex_shrink_0();
        let text = |t: String| div().min_w_0().truncate().child(t);
        if let Some(b) = self.browser.batch.as_ref().filter(|_| self.updating_addons()) {
            let names: Vec<String> = b
                .left
                .iter()
                .map(|id| {
                    self.browser
                        .find(id)
                        .map_or_else(|| id.clone(), |a| a.name.clone())
                })
                .collect();
            return Some(
                row.child(dot(p.warning))
                    .child(text(trf!("Updating {names}.", names = names.join(", ")))),
            );
        }
        if !matches!(state, PlayState::Ready | PlayState::Crashed(_)) {
            return None;
        }
        let due = self.addon_updates();
        if !due.is_empty() {
            let changes: Vec<String> = due
                .iter()
                .map(|u| format!("{} {} → {}", u.name, u.installed, u.to.version))
                .collect();
            let skip = match due.len() {
                1 => tr("Skip this version"),
                _ => tr("Skip these versions"),
            };
            return Some(
                row.child(dot(p.accent))
                    .child(text(trf!(
                        "Addon updates: {changes}.",
                        changes = changes.join(", ")
                    )))
                    .child(
                        link("addon-updates-skip", skip, p)
                            .flex_shrink_0()
                            .on_click(cx.listener(|this, _, window, cx| this.skip_addon_updates(window, cx))),
                    ),
            );
        }
        let failed: Vec<String> = self
            .browser
            .held
            .iter()
            .filter(|(id, (v, error))| {
                held_failure(error) && self.catalog_update(id).is_some_and(|u| u.to.version == *v)
            })
            .map(|(id, (_, e))| {
                let name = self.browser.find(id).map_or(id.as_str(), |a| a.name.as_str());
                format!("{name}: {e}")
            })
            .collect();
        if failed.is_empty() {
            return None;
        }
        let tip = SharedString::from(failed.join("\n"));
        Some(
            row.text_color(p.error)
                .child(Icon::new(IconName::CircleAlert).size(px(16.)))
                .child(
                    div()
                        .id("addon-updates-failed")
                        .min_w_0()
                        .truncate()
                        .child(trf!("Couldn't update {names}", names = failed.join("; ")))
                        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx)),
                )
                .child(
                    link("addon-updates-again", tr("Try again"), p)
                        .flex_shrink_0()
                        .on_click(cx.listener(|this, _, window, cx| this.retry_addon_updates(window, cx))),
                ),
        )
    }
}
