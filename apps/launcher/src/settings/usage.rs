//! What the data folder holds, measured off the UI thread, and the clear
//! actions that act on it.

use std::time::SystemTime;

use crate::notices::{Notification, Notify};
use crate::scale::px;
use gpui_kit::component::WindowExt;
use gpui_kit::component::button::ButtonVariant;
use gpui_kit::*;
use swofty_core::data_usage::{self, KEEP_FOR, KEEP_SESSIONS};
use swofty_i18n::{tr, trf};

use crate::format::size_coarse as size;
use crate::shell::Shell;

/// The link cache Play keeps in the game folder when the data folder is on
/// another drive.
#[derive(Default, Clone, Copy)]
pub struct GameCopy {
    pub bytes: u64,
    pub apart: bool,
}

/// Clears old session logs when the launcher starts, when the player turned
/// that on. Runs after an unfinished session was recovered; the newest sessions,
/// the last `KEEP_FOR` and anything written in the last hour always stay.
pub fn clear_logs_at_start(root: &std::path::Path) {
    let done = data_usage::prune_sessions(&root.join("sessions"), KEEP_SESSIONS, KEEP_FOR, SystemTime::now());
    if done.sessions > 0 {
        crate::app_log::note(&format!(
            "start: cleared {} old session logs ({})",
            done.sessions,
            size(done.bytes)
        ));
    }
}

/// Which drive is short of room and what that means, as Play and the Settings
/// page both say it, or `None` when both drives have enough.
pub fn low_space_warning(room: data_usage::Room) -> Option<String> {
    let need = size(data_usage::GAME_NEEDS);
    let game = room.game_low().map(|free| {
        if free < data_usage::GAME_NEEDS {
            trf!(
                "Only {free} free on {drive}. F1 25 won't start with less than {need} free there.",
                free = size(free),
                drive = data_usage::GAME_DRIVE,
                need = need
            )
        } else {
            trf!(
                "Only {free} free on {drive}, and F1 25 needs {need} to start.",
                free = size(free),
                drive = data_usage::GAME_DRIVE,
                need = need
            )
        }
    });
    let data = room.data_low().map(|free| {
        trf!(
            "Only {free} free on the drive with your addons and saves: installing or building addons may fail.",
            free = size(free)
        )
    });
    let lines: Vec<String> = game.into_iter().chain(data).collect();
    (!lines.is_empty()).then(|| lines.join(" "))
}

impl Shell {
    /// Shows the launcher in `code` from now on, and remembers it.
    pub(crate) fn set_language(&mut self, code: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.language = (code != swofty_i18n::ENGLISH).then(|| code.to_owned());
        if let Err(e) = self.store.save_config(&config) {
            crate::app_log::action("save settings").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
        swofty_i18n::set_language(Some(code));
        window.refresh();
        cx.notify();
    }

    /// Measures the data folder again, off the UI thread.
    pub(crate) fn settings_measure(&mut self, cx: &mut Context<Self>) {
        if self.settings.measuring {
            return;
        }
        self.settings.measuring = true;
        let root = self.store.root.clone();
        let game = self.install.as_ref().map(|i| i.dir.clone());
        cx.spawn(async move |this, cx| {
            let (usage, prunable, cache) = cx
                .background_executor()
                .spawn(async move {
                    let usage = data_usage::measure(&root);
                    let old = data_usage::prunable(
                        &root.join("sessions"),
                        KEEP_SESSIONS,
                        KEEP_FOR,
                        SystemTime::now(),
                    );
                    let cache = game.map_or_else(GameCopy::default, |g| GameCopy {
                        bytes: swofty_core::link_cache::size(&g),
                        apart: swofty_core::link_cache::apart(
                            swofty_core::disk::volume(&swofty_core::store::overlay_root(&root)),
                            swofty_core::disk::volume(&g),
                        )
                        .unwrap_or(false),
                    });
                    (usage, old, cache)
                })
                .await;
            this.update(cx, |s, cx| {
                s.settings.usage = Some(usage);
                s.settings.prunable = prunable;
                s.settings.cache = cache;
                s.settings.measuring = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn confirm_clear_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.processes.is_empty() {
            window.notify(
                Notification::error(tr("Close F1 25 first: it may still be writing its session log")),
                cx,
            );
            return;
        }
        let old = self.settings.prunable;
        if old.sessions == 0 {
            return;
        }
        let shell = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let shell = shell.clone();
            alert
                .width(px(480.))
                .title(trf!("Delete {count} old sessions ({size})?", count = old.sessions, size = size(old.bytes)))
                .description(trf!(
                    "Their logs are deleted for good. The newest {sessions} sessions and everything from the last {days} days stay.",
                    sessions = KEEP_SESSIONS,
                    days = KEEP_FOR.as_secs() / 86_400
                ))
                .ok_text(tr("Delete sessions"))
                .ok_variant(ButtonVariant::Danger)
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    shell.update(cx, |s, cx| s.settings_clear_logs(window, cx)).ok();
                    true
                })
        });
    }

    pub(crate) fn settings_clear_cache(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.processes.is_empty() {
            window.notify(Notification::error(tr("Close F1 25 first")), cx);
            return;
        }
        let Some(game) = self.install.as_ref().map(|i| i.dir.clone()) else {
            return;
        };
        self.settings.clearing = true;
        cx.spawn_in(window, async move |this, cx| {
            let freed = cx
                .background_executor()
                .spawn(async move { swofty_core::link_cache::remove(&game) })
                .await;
            this.update_in(cx, |s, window, cx| {
                s.settings.clearing = false;
                crate::app_log::note(&format!(
                    "settings: cleared the game drive copy ({})",
                    size(freed)
                ));
                window.notify(
                    Notification::success(trf!(
                        "Cleared the game drive copy and freed {size}",
                        size = size(freed)
                    )),
                    cx,
                );
                s.settings_measure(cx);
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn settings_clear_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.processes.is_empty() {
            window.notify(
                Notification::error(tr("Close F1 25 first: it may still be writing its session log")),
                cx,
            );
            return;
        }
        self.settings.clearing = true;
        let sessions = self.store.root.join("sessions");
        cx.spawn_in(window, async move |this, cx| {
            let done = cx
                .background_executor()
                .spawn(async move {
                    data_usage::prune_sessions(&sessions, KEEP_SESSIONS, KEEP_FOR, SystemTime::now())
                })
                .await;
            this.update_in(cx, |s, window, cx| {
                s.settings.clearing = false;
                crate::app_log::note(&format!(
                    "settings: cleared {} old session logs ({})",
                    done.sessions,
                    size(done.bytes)
                ));
                window.notify(
                    Notification::success(trf!(
                        "Cleared {count} old session logs and freed {size}",
                        count = done.sessions,
                        size = size(done.bytes)
                    )),
                    cx,
                );
                s.settings_measure(cx);
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::low_space_warning;
    use swofty_core::data_usage::{GAME_DRIVE, Room};

    const GB: u64 = 1 << 30;

    #[test]
    fn play_warns_only_when_a_drive_is_short() {
        let room = |game, data| Room { game, data };
        assert_eq!(low_space_warning(room(Some(50 * GB), Some(50 * GB))), None);
        assert_eq!(low_space_warning(room(None, None)), None);
        let refused = low_space_warning(room(Some(GB), Some(GB))).unwrap();
        assert!(
            refused.starts_with(&format!(
                "Only 1.0 GB free on {GAME_DRIVE}. F1 25 won't start with less than 2.0 GB free there."
            )),
            "{refused}"
        );
        assert!(
            !refused.contains("addons and saves"),
            "one drive is named once: {refused}"
        );
        let tight = low_space_warning(room(Some(5 * GB / 2), Some(50 * GB))).unwrap();
        assert!(tight.contains("and F1 25 needs 2.0 GB to start"), "{tight}");
        let data = low_space_warning(room(Some(50 * GB), Some(GB))).unwrap();
        assert!(
            data.starts_with("Only 1.0 GB free on the drive with your addons and saves"),
            "{data}"
        );
    }
}
