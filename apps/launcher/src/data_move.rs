//! Moving the data folder from Settings: pick a folder, confirm, then a
//! locked screen with progress while the folder is copied; the launcher
//! restarts and finishes the switch before it opens anything.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::notices::{Notification, Notify};
use crate::scale::{Hit, px};
use gpui_kit::component::button::Button;
use gpui_kit::component::{WindowExt, h_flex, v_flex};
use gpui_kit::*;
use swofty_core::busy::Busy;
use swofty_core::data_move::{self, Location, Moved, Ops, Plan};
use swofty_core::launcher::Progress;
use swofty_i18n::{tr, trf};

use crate::format::size_coarse as size;
use crate::shell::{PlayState, Shell};
use crate::theme::Palette;
use crate::ui::LockPanel;

const POLL: Duration = Duration::from_millis(120);

type Feed = Arc<Mutex<(f32, String)>>;

struct Sink(Feed);

impl Progress for Sink {
    fn report(&self, done: f32, text: &str) {
        if let Ok(mut feed) = self.0.lock() {
            *feed = (done, text.to_owned());
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Copying,
    Cancelling,
    Restarting,
}

pub(crate) struct Moving {
    plan: Plan,
    feed: Feed,
    cancel: Arc<AtomicBool>,
    stage: Stage,
}

pub(crate) fn install_folder() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bin = exe
        .parent()
        .filter(|b| b.file_name().is_some_and(|n| n == "bin"))?;
    bin.parent().map(Path::to_path_buf)
}

/// A Play still under way: its checks, the launch, or the rollback after the game exits.
fn launch_refusal(state: &PlayState) -> Option<&'static str> {
    match state {
        PlayState::Preparing | PlayState::Launching => Some(tr(
            "Wait for F1 25 to finish launching before moving your addons and saves",
        )),
        PlayState::Restoring => Some(tr(
            "Wait for the game files to be restored before moving your addons and saves",
        )),
        _ => None,
    }
}

impl Shell {
    pub(crate) fn move_refusal(&self) -> Option<String> {
        if self.moving.is_some() {
            return Some(tr("Your addons and saves are already moving").to_owned());
        }
        let skip_game = cfg!(debug_assertions) && std::env::var_os("SWOFTY_DEBUG_MOVE_TO").is_some();
        if !skip_game && (!self.processes.is_empty() || swofty_core::process::game_running()) {
            return Some(tr("Close F1 25 before moving your addons and saves").to_owned());
        }
        if self.build.is_some() || Busy::live(&self.store.root).is_some() {
            return Some(tr("Wait for the build to finish before moving your addons and saves").to_owned());
        }
        if let Some(why) = launch_refusal(&self.play_state()) {
            return Some(why.to_owned());
        }
        if self.download_pending() > 0 {
            return Some(
                tr("Wait for the downloads to finish before moving your addons and saves").to_owned(),
            );
        }
        None
    }

    pub(crate) fn data_move_locked(&self) -> bool {
        self.moving.is_some()
    }

    pub(crate) fn open_data_move(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(why) = self.move_refusal() {
            window.notify(Notification::error(why), cx);
            return;
        }
        #[cfg(debug_assertions)]
        if let Some(picked) = std::env::var_os("SWOFTY_DEBUG_MOVE_TO") {
            self.plan_data_move(PathBuf::from(picked), window, cx);
            return;
        }
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr("Move here").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(picked) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |s, window, cx| s.plan_data_move(picked, window, cx))
                .ok();
        })
        .detach();
    }

    fn plan_data_move(&mut self, picked: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let to = data_move::choose(&picked);
        let games: Vec<PathBuf> = self
            .copies
            .iter()
            .map(|c| c.dir.clone())
            .chain(self.install.as_ref().map(|i| i.dir.clone()))
            .collect();
        let location = Location::find(install_folder(), games);
        cx.spawn_in(window, async move |this, cx| {
            let planned = cx
                .background_executor()
                .spawn(async move { data_move::plan(&location, &to, &Ops::system()) })
                .await;
            this.update_in(cx, |s, window, cx| match planned {
                Ok(plan) => s.confirm_data_move(plan, window, cx),
                Err(e) => s.data_move_failed(&e.to_string(), window, cx),
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn data_move_failed(&mut self, why: &str, window: &mut Window, cx: &mut Context<Self>) {
        crate::app_log::note(&format!("data move: {why}"));
        let why = why.to_owned();
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .width(px(500.))
                .title(tr("Your addons and saves were not moved"))
                .description(why.clone())
                .ok_text(tr("OK"))
        });
    }

    fn confirm_data_move(&mut self, plan: Plan, window: &mut Window, cx: &mut Context<Self>) {
        let shell = cx.entity().downgrade();
        let p = crate::theme::palette(cx);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let shell = shell.clone();
            let plan = plan.clone();
            let row = |label: String, value: String| {
                h_flex()
                    .w_full()
                    .gap(px(12.))
                    .items_start()
                    .child(div().w(px(96.)).flex_shrink_0().text_color(p.text_muted).child(label))
                    .child(div().flex_1().min_w_0().text_color(p.text).child(value))
            };
            let free = plan
                .free
                .map_or_else(|| tr("Unknown").to_owned(), size);
            let note = if plan.same_volume {
                tr("It is on the same drive, so it moves in a moment. Swofty Launcher restarts when it is done.")
            } else {
                tr("Swofty Launcher locks while it copies and restarts when it is done. Your addons and saves stay where they are until the copy checks out.")
            };
            alert
                .width(px(560.))
                .title(tr("Move your addons and saves?"))
                .child(
                    v_flex()
                        .gap(px(8.))
                        .text_size(px(13.))
                        .child(row(tr("From").to_owned(), plan.from.display().to_string()))
                        .child(row(tr("To").to_owned(), plan.to.display().to_string()))
                        .child(row(tr("Size").to_owned(), size(plan.bytes)))
                        .child(row(tr("Free there").to_owned(), free))
                        .child(div().pt(px(6.)).text_color(p.text_muted).child(note)),
                )
                .ok_text(tr("Move"))
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    let plan = plan.clone();
                    shell.update(cx, |s, cx| s.start_data_move(plan, window, cx)).ok();
                    true
                })
        });
    }

    fn start_data_move(&mut self, plan: Plan, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(why) = self.move_refusal() {
            window.notify(Notification::error(why), cx);
            return;
        }
        let feed: Feed = Arc::default();
        let cancel = Arc::new(AtomicBool::new(false));
        crate::app_log::note(&format!(
            "data move: {} to {} ({})",
            plan.from.display(),
            plan.to.display(),
            size(plan.bytes)
        ));
        self.moving = Some(Moving {
            plan: plan.clone(),
            feed: feed.clone(),
            cancel: cancel.clone(),
            stage: Stage::Copying,
        });
        let work = cx
            .background_executor()
            .spawn(async move { data_move::move_data(&plan, &Ops::system(), &Sink(feed), &cancel) });
        cx.spawn_in(window, async move |this, cx| {
            let result = work.await;
            this.update_in(cx, |s, window, cx| s.data_move_done(result, window, cx))
                .ok();
        })
        .detach();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                let live = this
                    .update(cx, |s, cx| {
                        cx.notify();
                        s.moving.as_ref().is_some_and(|m| m.stage != Stage::Restarting)
                    })
                    .unwrap_or(false);
                if !live {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    fn data_move_done(
        &mut self,
        result: Result<Moved, data_move::Error>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(moved) => {
                crate::app_log::note(&format!("data move: copied, restarting ({:?})", moved.method));
                if let Some(m) = &mut self.moving {
                    m.stage = Stage::Restarting;
                }
                cx.notify();
                self.relaunch_for_move(window, cx);
            }
            Err(e) => {
                self.moving = None;
                cx.notify();
                self.data_move_failed(&e.to_string(), window, cx);
            }
        }
    }

    pub(crate) fn relaunch_for_move(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let started = std::env::current_exe().and_then(|exe| std::process::Command::new(exe).spawn());
        match started {
            Ok(_) => cx.quit(),
            Err(e) => {
                self.moving = None;
                cx.notify();
                self.data_move_failed(
                    &trf!(
                        "Your addons and saves are copied. Close Swofty Launcher and open it again to finish the move. ({error})",
                        error = e
                    ),
                    window,
                    cx,
                );
            }
        }
    }

    fn cancel_data_move(&mut self, cx: &mut Context<Self>) {
        if let Some(m) = self.moving.as_mut().filter(|m| m.stage == Stage::Copying) {
            m.cancel.store(true, Ordering::Relaxed);
            m.stage = Stage::Cancelling;
            cx.notify();
        }
    }

    pub(crate) fn data_move_overlay(&self, p: Palette, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let m = self.moving.as_ref()?;
        let (done, file) = m.feed.lock().map(|f| f.clone()).unwrap_or_default();
        let overlay = LockPanel::new(tr("Moving your addons and saves")).detail(trf!(
            "{from} to {to}",
            from = m.plan.from.display(),
            to = m.plan.to.display()
        ));
        let overlay = match m.stage {
            Stage::Copying => overlay
                .progress(done)
                .status(if file.is_empty() {
                    tr("Counting files").to_owned()
                } else {
                    trf!("Copying {file}", file = file)
                })
                .action(
                    Button::new("data-move-cancel")
                        .outline()
                        .tap_sm()
                        .label(tr("Cancel"))
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_data_move(cx))),
                ),
            Stage::Cancelling => overlay.progress(done).status(tr("Cancelling")),
            Stage::Restarting => overlay.progress(1.).status(tr("Restarting to finish the move")),
        };
        Some(overlay.render("data-move-lock", p))
    }

    pub(crate) fn show_data_move_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Some(result)) = data_move::startup().get() else {
            return;
        };
        match result {
            Ok(done) => {
                crate::app_log::note(&format!("data move: now in {}", done.to.display()));
                let text = match &done.leftover {
                    None if done.adopted => {
                        trf!(
                            "Your addons and saves are back. They are in {to}",
                            to = done.to.display()
                        )
                    }
                    Some(old) => trf!(
                        "Your addons and saves moved to {to}, but the old copy in {old} could not be deleted. You can delete it yourself.",
                        to = done.to.display(),
                        old = old.display()
                    ),
                    None => trf!("Your addons and saves moved to {to}", to = done.to.display()),
                };
                let note = if done.leftover.is_some() {
                    Notification::warning(text)
                } else {
                    Notification::success(text)
                };
                window.notify(note, cx);
            }
            Err(e) => {
                crate::app_log::note(&format!("data move: not finished: {e}"));
                window.notify(
                    Notification::error(trf!(
                        "Your addons and saves were not moved. They are where they were. {error}",
                        error = e
                    )),
                    cx,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::launch_refusal;
    use crate::shell::PlayState;

    #[test]
    fn a_play_under_way_refuses_the_move() {
        for state in [PlayState::Preparing, PlayState::Launching, PlayState::Restoring] {
            assert!(launch_refusal(&state).is_some());
        }
        for state in [
            PlayState::Ready,
            PlayState::Detecting,
            PlayState::NotInstalled,
            PlayState::Checking,
        ] {
            assert!(launch_refusal(&state).is_none());
        }
    }
}
