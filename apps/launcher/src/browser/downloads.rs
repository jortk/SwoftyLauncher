use gpui_kit::component::button::Button;
use gpui_kit::component::{Disableable, h_flex, v_flex};
use gpui_kit::*;
use swofty_i18n::{tr, trf};

use super::*;
use crate::download_queue::State;
use crate::scale::px;
use crate::shell::{Page, Shell};
use crate::theme::{Palette, ScrollArea};

impl Shell {
    pub(crate) fn download_pending(&self) -> usize {
        self.browser.queue.pending()
    }

    pub(crate) fn dock_input(&self) -> crate::download_dock::Input {
        crate::download_dock::Input {
            pending: self.browser.queue.pending(),
            failed: self
                .browser
                .queue
                .jobs()
                .iter()
                .filter(|job| matches!(job.state, crate::download_queue::State::Failed(_)))
                .count(),
            on_downloads: self.page == Page::Downloads,
        }
    }

    pub(crate) fn finished_downloads(&self) -> usize {
        self.browser
            .queue
            .jobs()
            .iter()
            .filter(|job| !matches!(job.state, State::Queued | State::Active))
            .count()
    }

    pub(crate) fn dock_line(&self) -> Option<crate::download_dock::Line> {
        let jobs = self.browser.queue.jobs();
        let pending = self.browser.queue.pending();
        if pending > 0 {
            let job = jobs
                .iter()
                .find(|job| job.state == State::Active)
                .or_else(|| jobs.iter().find(|job| job.state == State::Queued))?;
            let working = self.browser.working.get(&job.id);
            return Some(crate::download_dock::Line {
                name: job.request.0.name.clone(),
                fraction: working.map(|w| w.progress().fraction()),
                failed: false,
            });
        }
        let failed = jobs
            .iter()
            .rev()
            .find(|job| matches!(job.state, State::Failed(_)));
        let (Some(job), true) = (failed, self.dock.failure_shown(self.dock_input().failed)) else {
            let job = jobs.iter().rev().find(|job| job.state == State::Completed)?;
            return Some(crate::download_dock::Line {
                name: job.request.0.name.clone(),
                fraction: Some(1.),
                failed: false,
            });
        };
        let State::Failed(_) = &job.state else {
            return None;
        };
        Some(crate::download_dock::Line {
            name: job.request.0.name.clone(),
            fraction: None,
            failed: true,
        })
    }

    pub(crate) fn downloads_page(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let rows = self
            .browser
            .queue
            .jobs()
            .iter()
            .enumerate()
            .map(|(index, job)| self.download_row(index, job, p, cx))
            .collect::<Vec<_>>();
        v_flex()
            .size_full()
            .p(px(24.))
            .gap(px(12.))
            .child(
                h_flex().justify_between().child(tr("Downloads")).child(
                    Button::new("clear-downloads")
                        .label(tr("Clear finished"))
                        .on_click(cx.listener(|this, _, _, cx| this.clear_finished_downloads(cx))),
                ),
            )
            .child(self.download_limit_row(cx))
            .children(rows.is_empty().then(|| {
                v_flex().gap(px(8.)).child(tr("No downloads yet")).child(
                    Button::new("open-addon-browser")
                        .label(tr("Browse addons"))
                        .on_click(cx.listener(|this, _, _, cx| this.open_page(Page::Browser, cx))),
                )
            }))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .scroll_y("downloads-list")
                    .gap(px(8.))
                    .children(rows),
            )
            .into_any_element()
    }

    fn download_limit_row(&self, cx: &mut Context<Self>) -> Div {
        h_flex()
            .gap(px(8.))
            .child(tr("Concurrent jobs"))
            .child(
                Button::new("download-limit-minus")
                    .label("−")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.change_download_limit(this.browser.queue.limit().saturating_sub(1), cx);
                    })),
            )
            .child(self.browser.queue.limit().to_string())
            .child(
                Button::new("download-limit-plus")
                    .label("+")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.change_download_limit(this.browser.queue.limit() + 1, cx);
                    })),
            )
    }

    fn clear_finished_downloads(&mut self, cx: &mut Context<Self>) {
        let finished: Vec<_> = self
            .browser
            .queue
            .jobs()
            .iter()
            .filter(|job| matches!(job.state, State::Failed(_)))
            .map(|job| job.id.clone())
            .collect();
        self.browser.queue.clear_finished();
        for id in finished {
            if !self.browser.queue.jobs().iter().any(|job| job.id == id) {
                self.browser.failed.remove(&id);
                self.browser.refused.remove(&id);
            }
        }
        cx.notify();
    }

    fn download_row(&self, index: usize, job: &QueuedJob, p: Palette, cx: &mut Context<Self>) -> Div {
        v_flex()
            .gap(px(4.))
            .p(px(12.))
            .bg(p.surface)
            .child(
                h_flex()
                    .justify_between()
                    .child(format!("{} {}", job.request.0.name, job.request.1.version))
                    .children(self.download_action(index, job, cx)),
            )
            .child(div().text_color(p.text_muted).child(self.download_status(job)))
    }

    fn download_status(&self, job: &QueuedJob) -> String {
        let status = match &job.state {
            State::Queued => tr("Queued").to_owned(),
            State::Active => self.browser.working.get(&job.id).map_or_else(
                || tr("Starting…").to_owned(),
                |progress| progress.progress().long(progress.item().as_deref()),
            ),
            State::Completed => tr("Completed").to_owned(),
            State::Failed(error) => trf!("Failed: {error}", error = error),
        };
        let gone = matches!(job.state, State::Completed | State::Failed(_))
            && self
                .store
                .profile_names()
                .is_ok_and(|names| !names.contains(&job.request.2));
        if gone {
            trf!(
                "{status} · Profile {profile} no longer exists; nothing was added to another profile.",
                status = status,
                profile = job.request.2
            )
        } else {
            status
        }
    }

    fn download_action(&self, index: usize, job: &QueuedJob, cx: &mut Context<Self>) -> Option<Button> {
        let id = job.id.clone();
        match &job.state {
            State::Queued => Some(
                Button::new(SharedString::from(format!("remove-{id}-{index}")))
                    .label(tr("Remove"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.browser.queue.remove_queued(&id) {
                            if let Some(batch) = &mut this.browser.batch {
                                batch.left.remove(&id);
                            }
                            if this.browser.batch.as_ref().is_some_and(|b| b.left.is_empty()) {
                                this.finish_addon_updates();
                            }
                            this.start_queued_downloads(cx);
                        }
                    })),
            ),
            State::Failed(_) => {
                let action = retry_action(self.browser.find(&id), &job.request.1);
                let pending = self
                    .browser
                    .queue
                    .jobs()
                    .iter()
                    .any(|other| other.id == id && matches!(other.state, State::Queued | State::Active));
                let label = if pending {
                    tr("Already queued")
                } else {
                    match action {
                        RetryAction::Retry => tr("Retry"),
                        RetryAction::RequestCurrent => tr("Request current version"),
                        RetryAction::Unavailable => tr("Version unavailable"),
                    }
                };
                Some(
                    Button::new(SharedString::from(format!("retry-{id}-{index}")))
                        .label(label)
                        .disabled(pending || action == RetryAction::Unavailable)
                        .on_click(cx.listener(move |this, _, _, cx| match action {
                            RetryAction::Retry => {
                                if this.browser.queue.retry_at(index, &id) {
                                    this.start_queued_downloads(cx);
                                }
                            }
                            RetryAction::RequestCurrent => {
                                this.enqueue_catalog(&id, cx);
                            }
                            RetryAction::Unavailable => {}
                        })),
                )
            }
            _ => None,
        }
    }

    pub(super) fn retry_download(&mut self, id: &str, cx: &mut Context<Self>) {
        let index =
            self.browser.queue.jobs().iter().rposition(|job| {
                job.id == id && matches!(job.state, crate::download_queue::State::Failed(_))
            });
        if let Some(index) = index {
            let action = retry_action(self.browser.find(id), &self.browser.queue.jobs()[index].request.1);
            if action == RetryAction::RequestCurrent {
                self.enqueue_catalog(id, cx);
            } else if action == RetryAction::Retry && self.browser.queue.retry_at(index, id) {
                self.browser.failed.remove(id);
                self.browser.refused.remove(id);
                self.start_queued_downloads(cx);
            }
        } else {
            self.install_from_catalog(id, cx);
        }
    }

    pub(super) fn change_download_limit(&mut self, limit: usize, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.set_download_jobs(limit);
        if let Err(error) = self.store.save_config(&config) {
            crate::app_log::action("save download limit").failed(&error);
            return;
        }
        self.browser.queue.set_limit(config.download_jobs());
        self.start_queued_downloads(cx);
    }
}
