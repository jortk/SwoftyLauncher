use crate::crash_modal::{self, CrashView};
use crate::game::{self, Copies, Install, Owner, Probe};
use crate::game_edited::Edited;
use crate::notices::{Notification, Notify};
use gpui_kit::*;
use std::time::{Duration, Instant};
use swofty_core::quarantine;
use swofty_core::recovery::{self, Blocked};
use swofty_core::session::SessionRecord;

use super::dialogs::{launch_headline, show_launch_failed, show_mods_blocked};
use super::{Page, Shell};
use swofty_i18n::{tr, trf, trn};

/// Times shown as "5 min ago" stay current without a repaint every probe.
const REPAINT: Duration = Duration::from_secs(30);

impl Shell {
    pub(super) fn apply(&mut self, probe: Probe, window: &mut Window, cx: &mut Context<Self>) {
        let started = Instant::now();
        self.apply_probe(probe, window, cx);
        self.note_slow("probe", self.page_name(), started);
    }

    pub(super) fn page_name(&self) -> &'static str {
        match self.page {
            Page::Section(_) => "panel",
            page => page.nav_label(),
        }
    }

    pub(super) fn note_slow(&mut self, what: &str, page: &str, started: Instant) {
        let took = started.elapsed();
        if took > Duration::from_millis(100) && !self.slow_logged {
            self.slow_logged = true;
            crate::app_log::note(&format!(
                "ui: {what} took {} ms on the UI thread while on the {page} page",
                took.as_millis()
            ));
        }
    }

    pub(super) fn apply_probe(&mut self, mut probe: Probe, window: &mut Window, cx: &mut Context<Self>) {
        let before = (self.requests, self.processes.clone(), self.session.clone());
        let was_live = self.launch.is_some();
        self.requests = probe.requests;
        let root = self.store.root.clone();
        if let Some(banner) = probe.banner.take()
            && probe.banner_profile == self.library.profile.name
        {
            self.saves.banner = banner;
            self.saves.banner_profile = probe.banner_profile.clone();
        }
        self.saves.game_running = !probe.processes.is_empty();
        if self
            .saves
            .refresh_due(self.page == Page::Careers, probe.saves_print)
        {
            self.refresh_saves_in_background(cx);
        }
        if probe.saves_marker && self.saves.take_notice(&root) {
            self.show_saves_notice(window, cx);
        }
        self.announce_reports(&probe, window, cx);
        self.apply_install(probe.install.take(), probe.merged, cx);
        if self.cancelling && probe.busy.is_none() && self.build.is_some() {
            self.cancelling = false;
            window.notify(
                Notification::info(tr(
                    "The game files are back as they were. Press Play to build again.",
                ))
                .title(tr("Build cancelled")),
                cx,
            );
        }
        self.build = probe.busy.take();
        self.processes = std::mem::take(&mut probe.processes);
        self.sync_shaders(false, cx);
        self.attribute();
        self.finish_launch(&probe);
        self.watch_session(probe.session.as_ref(), cx);
        self.session = probe.session;
        self.db_poll(cx);
        let live = was_live || self.launch.is_some();
        if live
            || before != (self.requests, self.processes.clone(), self.session.clone())
            || self.painted.elapsed() >= REPAINT
        {
            cx.notify();
        }
    }

    fn announce_reports(&mut self, probe: &Probe, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(n) = probe.recovered.as_ref().filter(|n| n.at > self.seen.recovered) {
            self.seen.recovered = n.at;
            let mut text: Vec<String> = n.lines.iter().take(4).cloned().collect();
            if n.lines.len() > text.len() {
                text.push(trf!(
                    "and {count} more; the launch log has them all",
                    count = n.lines.len() - text.len()
                ));
            }
            window.notify(Notification::info(text.join("\n")).title(recovery::HEADLINE), cx);
        }
        if let Some(r) = probe
            .mods_blocked
            .as_ref()
            .filter(|r| r.at > self.seen.mods_blocked)
        {
            self.seen.mods_blocked = r.at;
            show_mods_blocked(r, cx.entity().downgrade(), window, cx);
        }
        if let Some(r) = probe.not_stock.as_ref().filter(|r| r.at > self.seen.not_stock) {
            self.seen.not_stock = r.at;
            self.game_edited = Some(Edited::NotStock(r.clone()));
            self.show_game_edited(window, cx);
        }
        if let Some(b) = probe
            .blocked
            .as_ref()
            .filter(|_| probe.blocked_at.is_some_and(|at| at > self.seen.blocked))
        {
            self.seen.blocked = probe.blocked_at.expect("blocked file timestamp");
            self.blocked = Some(b.clone());
            self.game_edited = Some(Edited::Leftovers(b.clone()));
            self.show_game_edited(window, cx);
        }
        if let Some(f) = probe
            .launch_failed
            .as_ref()
            .filter(|f| f.at > self.seen.launch_failed)
        {
            self.seen.launch_failed = f.at;
            self.notice = Some(launch_headline(f));
            show_launch_failed(f, window, cx);
        }
    }

    fn apply_install(&mut self, found: Option<Copies>, merged: bool, cx: &mut Context<Self>) {
        if let Some(found) = found {
            let changed = !self.detected || self.install != found.active || self.copies != found.all;
            self.install = found.active;
            crate::app_log::set_game(self.install.as_ref().map(|i| i.dir.as_path()));
            self.copies = found.all;
            self.missing_copies = found.missing;
            self.detected = true;
            if self.page == Page::Careers && (changed || merged) {
                self.refresh_saves_in_background(cx);
            }
            if merged {
                self.reload(cx);
            } else if changed {
                self.refresh_views(cx);
                self.db_profile_changed(cx);
            }
        } else if merged {
            self.reload(cx);
        }
    }

    fn finish_launch(&mut self, probe: &Probe) {
        if let Some(launch) = self.launch.as_mut() {
            launch.saw_game |= self.owners.iter().any(|o| !matches!(o, Owner::Other(_)));
            launch.stage = probe.launch_stage.clone();
            if let Some(ok) = launch.finished() {
                if !ok && !launch.saw_game {
                    self.blocked = Blocked::read(&self.store.root);
                    let refused = probe
                        .mods_blocked
                        .as_ref()
                        .is_some_and(|r| r.stage == quarantine::Stage::BeforeLaunch && r.at >= launch.at);
                    let not_stock = probe
                        .not_stock
                        .as_ref()
                        .filter(|r| r.at >= launch.at)
                        .map(|r| crate::game_edited::notice(r, self.install.as_ref().map(|i| i.source)));
                    let failed = probe
                        .launch_failed
                        .as_ref()
                        .filter(|f| f.at >= launch.at)
                        .map(launch_headline);
                    self.notice = Some(match &self.blocked {
                        _ if refused => {
                            tr("an antivirus removed mod files, so F1 25 was not started").to_owned()
                        }
                        _ if not_stock.is_some() => not_stock.unwrap_or_default(),
                        Some(b) if b.ours() => trn(
                            b.files.len(),
                            "a file an earlier Swofty session left in the game folder is in the way; Restore game \
                             folder clears it",
                            "{count} files an earlier Swofty session left in the game folder are in the way; \
                             Restore game folder clears them",
                        ),
                        Some(b) => trn(
                            b.files.len(),
                            "a file Swofty did not put in the game folder is in the way",
                            "{count} files Swofty did not put in the game folder are in the way",
                        ),
                        None => failed.unwrap_or_else(|| launch.failure()),
                    })
                    .filter(|n| !n.contains(swofty_core::busy::CANCELLED));
                    let why = self.notice.clone().unwrap_or_else(|| launch.failure());
                    if self.notice.is_some() {
                        crate::app_log::action("start F1 25").failed(&why);
                    }
                    self.mp_launch_ended(Some(why));
                } else {
                    self.mp_launch_ended(None);
                }
                self.launch = None;
            }
        }
    }

    fn watch_session(&mut self, session: Option<&SessionRecord>, cx: &mut Context<Self>) {
        if let Some(s) = session.filter(|s| s.crashed() || s.end.is_some())
            && self.crash.as_ref().is_none_or(|(d, _)| *d != s.dir)
        {
            let session = s.clone();
            let crashed = s.crashed();
            let game_dir = self.install.as_ref().map(|i| i.dir.clone());
            let store = self.store.clone();
            let fresh = s
                .ended
                .is_some_and(|e| game::now_secs() - e < crash_modal::FRESH_SECS);
            self.crash = Some((s.dir.clone(), Ok(String::new())));
            cx.spawn(async move |this, cx| {
                let dir = session.dir.clone();
                let (summary, diagnosis) = cx
                    .background_executor()
                    .spawn(async move {
                        if !crashed {
                            let stall =
                                swofty_core::known_problems::stall(&session, game_dir.as_deref(), &store)
                                    .or_else(|| {
                                        swofty_core::known_problems::tyres(
                                            &session,
                                            game_dir.as_deref(),
                                            &store,
                                        )
                                    });
                            return (Ok(String::new()), stall);
                        }
                        let summary = game::crash_summary(&session, game_dir.as_deref());
                        let diagnosis =
                            swofty_core::known_problems::crash(&session, game_dir.as_deref(), &store);
                        (summary, diagnosis)
                    })
                    .await;
                this.update(cx, |s, cx| {
                    if let Ok(text) = &summary
                        && (crashed || diagnosis.is_some())
                    {
                        s.crash_view = Some(CrashView {
                            dir: dir.clone(),
                            diagnosis,
                            crashed,
                            summary: text.clone(),
                        });
                        s.crash_open |= fresh;
                    }
                    s.crash = Some((dir, summary));
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    /// The copy addon panels work with: the one a running game belongs to,
    /// else the one Play uses.
    pub(crate) fn panel_install(&self) -> Option<Install> {
        self.owners
            .iter()
            .find_map(|o| match o {
                Owner::Other(i) => Some(i.clone()),
                _ => None,
            })
            .or_else(|| self.install.clone())
    }

    pub(super) fn attribute(&mut self) {
        self.owners = self
            .processes
            .iter()
            .map(|g| game::owner(g, self.install.as_ref(), &self.copies))
            .collect();
    }
}
