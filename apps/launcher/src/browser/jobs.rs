use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use crate::notices::Notification;
use gpui_kit::*;
use swofty_core::catalog::{self, Addon};
use swofty_core::manifest::AddonManifest;
use swofty_core::update::current_version;
use swofty_i18n::{tr, trf};

use super::*;
use crate::format;
use crate::shell::Shell;

pub(super) fn install_failure_toast(name: &str, verb: Verb, refused: bool, error: String) -> Notification {
    Notification::error(error).title(verb.failure_title(name, refused))
}

struct Finished<'a> {
    addon: &'a Addon,
    version: &'a catalog::Version,
    verb: Verb,
    took: Duration,
    in_batch: bool,
}

pub(super) fn requested_version_available(addon: Option<&Addon>, requested: &catalog::Version) -> bool {
    addon.is_some_and(|addon| {
        addon.listed().iter().any(|version| {
            version.version == requested.version
                && version.accepts(&current_version())
                && version.sha256 == requested.sha256
                && version.size == requested.size
                && version.url == requested.url
        })
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RetryAction {
    Retry,
    RequestCurrent,
    Unavailable,
}

pub(super) fn retry_action(addon: Option<&Addon>, requested: &catalog::Version) -> RetryAction {
    if requested_version_available(addon, requested) {
        RetryAction::Retry
    } else if addon.and_then(|a| a.offered(&current_version())).is_some() {
        RetryAction::RequestCurrent
    } else {
        RetryAction::Unavailable
    }
}

impl Shell {
    pub(crate) fn install_from_catalog(&mut self, id: &str, cx: &mut Context<Self>) {
        self.enqueue_catalog(id, cx);
    }

    /// True when a job was queued or an existing pending job will handle it.
    pub(super) fn enqueue_catalog(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        if self.data_move_locked() {
            return false;
        }
        let Some(addon) = self.browser.find(id).cloned() else {
            return false;
        };
        let Some(version) = addon.offered(&current_version()).cloned() else {
            return false;
        };
        if self.build.as_ref().is_some_and(|b| b.holds(id)) {
            self.browser
                .toasts
                .push(Notification::warning(crate::update::BUILDING));
            return false;
        }
        if let Some(existing) = self.browser.queue.jobs().iter().find(|job| {
            job.id == id
                && matches!(
                    job.state,
                    crate::download_queue::State::Queued | crate::download_queue::State::Active
                )
        }) {
            return existing.request.1 == version;
        }
        let claims = match self.requirements(&addon) {
            Some(Ok(plan)) => plan.also().iter().map(|s| s.addon.id.clone()).collect(),
            _ => BTreeSet::new(),
        };
        if self.browser.queue.enqueue(
            id.to_owned(),
            (addon, version, self.library.profile.name.clone()),
            claims,
        ) {
            self.start_queued_downloads(cx);
        }
        true
    }

    pub(super) fn start_queued_downloads(&mut self, cx: &mut Context<Self>) {
        let queued: Vec<_> = self
            .browser
            .queue
            .jobs()
            .iter()
            .filter(|job| job.state == crate::download_queue::State::Queued)
            .map(|job| (job.id.clone(), job.request.0.clone()))
            .collect();
        for (id, addon) in queued {
            if let Some(Ok(plan)) = self.requirements(&addon) {
                self.browser
                    .queue
                    .update_claims(&id, plan.also().iter().map(|s| s.addon.id.clone()).collect());
            }
        }
        while let Some(id) = self.browser.queue.start_next() {
            let (addon, version, profile) = self
                .browser
                .queue
                .jobs()
                .iter()
                .find(|job| job.id == id && job.state == crate::download_queue::State::Active)
                .expect("the scheduler just activated this job")
                .request
                .clone();
            if !requested_version_available(self.browser.find(&id), &version) {
                let error = trf!(
                    "{name} {version} is no longer available or compatible; request it again.",
                    name = addon.name,
                    version = version.version
                );
                self.browser.queue.finish(&id, Err(error.clone()));
                self.browser.failed.insert(id.clone(), error.clone());
                if let Some(batch) = &mut self.browser.batch
                    && batch.left.remove(&id)
                {
                    batch.failed.push((addon.name.clone(), error));
                } else {
                    let verb = Verb::of(self.installed_version(&id).is_some());
                    self.browser
                        .toasts
                        .push(install_failure_toast(&addon.name, verb, false, error));
                }
                if self
                    .browser
                    .batch
                    .as_ref()
                    .is_some_and(|batch| batch.left.is_empty())
                {
                    self.finish_addon_updates();
                }
                continue;
            }
            self.start_catalog_install(addon, version, profile, cx);
        }
        cx.notify();
    }

    pub(super) fn start_catalog_install(
        &mut self,
        addon: Addon,
        version: catalog::Version,
        profile: String,
        cx: &mut Context<Self>,
    ) {
        let id = addon.id.as_str();
        let updating = self.installed_version(id).is_some();
        self.browser.failed.remove(id);
        self.browser.refused.remove(id);
        let job = Job::new(version.size);
        let preview: Vec<String> = match self.requirements(&addon) {
            Some(Ok(plan)) => plan.also().iter().map(|s| s.addon.id.clone()).collect(),
            _ => Vec::new(),
        };
        let riders: Vec<String> = preview
            .into_iter()
            .filter(|d| {
                !self.browser.working.contains_key(d)
                    && !self.browser.batch.as_ref().is_some_and(|b| b.left.contains(d))
            })
            .collect();
        for d in &riders {
            self.browser.working.insert(d.clone(), job.clone());
        }
        self.browser.working.insert(addon.id.clone(), job.clone());
        catalog::log(
            &self.store.root,
            &format!(
                "{} {} {} ({}) from {}",
                if updating { "updating" } else { "installing" },
                addon.id,
                version.version,
                format::size(version.size),
                version.url
            ),
        );
        let store = self.store.clone();
        let catalog_now = self.browser.addons.clone();
        let started = Instant::now();
        let tracked = job.clone();
        cx.spawn(async move |this, cx| {
            let (target, v) = (addon.clone(), version.clone());
            let result = cx
                .background_executor()
                .spawn(async move {
                    catalog::requires::install_in_profile(&store, &catalog_now, &target, &v, &profile, &|e| {
                        job.track(&target.id, e)
                    })
                })
                .await;
            this.update(cx, |s, cx| {
                s.browser.working.remove(&addon.id);
                for d in &riders {
                    s.browser.working.remove(d);
                }
                let took = started.elapsed();
                s.browser.queue.finish(
                    &addon.id,
                    result
                        .as_ref()
                        .map(|_| ())
                        .map_err(|error| tracked.failure(&error.to_string())),
                );
                let in_batch = s.browser.batch.as_mut().is_some_and(|b| b.left.remove(&addon.id));
                let verb = Verb::of(updating);
                let finished = Finished {
                    addon: &addon,
                    version: &version,
                    verb,
                    took,
                    in_batch,
                };
                match result {
                    Ok(done) => s.install_succeeded(&finished, &done, cx),
                    Err(e) => s.install_failed(&finished, e, cx),
                }
                if in_batch {
                    let error = s.browser.failed.get(&addon.id).cloned().unwrap_or_default();
                    s.browser
                        .held
                        .insert(addon.id.clone(), (version.version.clone(), error));
                }
                if in_batch && s.browser.batch.as_ref().is_some_and(|b| b.left.is_empty()) {
                    s.finish_addon_updates();
                }
                s.start_queued_downloads(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        self.tick_browser(cx);
        cx.notify();
    }

    fn install_succeeded(&mut self, f: &Finished<'_>, done: &[AddonManifest], cx: &mut Context<Self>) {
        let (m, also) = done.split_last().expect("an install ends with its addon");
        let title = f.verb.done(&m.name, &m.version);
        let also: Vec<String> = also.iter().map(|d| format!("{} {}", d.name, d.version)).collect();
        let mut detail = format!("{} · {}", format::size(f.version.size), f.addon.id);
        if !also.is_empty() {
            detail.push_str(&format!(
                " · {}",
                trf!("also installed {names}", names = also.join(", "))
            ));
        }
        self.record(Op::Install, true, title.clone(), detail, f.took);
        self.reload(cx);
        if f.in_batch {
            if let Some(b) = self.browser.batch.as_mut() {
                b.updated.push(format!("{} {}", m.name, m.version));
            }
        } else {
            let (shell, id) = (cx.entity().downgrade(), m.id.clone());
            let body = match also.is_empty() {
                true => title.clone(),
                false => trf!("Also installed {names}", names = also.join(", ")),
            };
            let note = Notification::success(body).action(tr("Turn on"), move |window, cx| {
                shell
                    .update(cx, |s, cx| s.set_addon_enabled(&id, true, window, cx))
                    .ok();
            });
            self.browser
                .toasts
                .push(if also.is_empty() { note } else { note.title(title) });
        }
    }

    fn install_failed(&mut self, f: &Finished<'_>, e: swofty_core::Error, cx: &mut Context<Self>) {
        let refused = matches!(e, swofty_core::Error::Requires(_));
        let error = e.to_string();
        crate::app_log::action(f.verb.action())
            .addon(&f.addon.id)
            .failed(&error);
        self.record(
            Op::Install,
            false,
            f.verb.failed(&f.addon.name, &f.version.version),
            error.clone(),
            f.took,
        );
        self.browser.failed.insert(f.addon.id.clone(), error.clone());
        if refused {
            self.browser.refused.insert(f.addon.id.clone());
        }
        if f.in_batch {
            if let Some(b) = self.browser.batch.as_mut() {
                b.failed.push((f.addon.name.clone(), error));
            }
        } else {
            self.browser
                .toasts
                .push(install_failure_toast(&f.addon.name, f.verb, refused, error));
        }
        self.reload(cx);
    }
}
