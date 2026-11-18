use crate::game::{self, Launch, Owner};
use crate::notices::{Notification, Notify};
use crate::play_checks;
use gpui_kit::*;
use std::path::PathBuf;
use swofty_core::install::Source;
use swofty_core::recovery::Blocked;
use swofty_core::session::{LaunchMode, SessionRecord};

use super::dialogs::note_notification;
use super::{PlayState, Shell};
use swofty_i18n::{tr, trf};

/// How long Play waits for a session module load that is already running.
const SESSION_MODULE_WAIT: std::time::Duration = std::time::Duration::from_millis(1500);

/// What a Play press came to once its checks passed or stopped it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Played {
    Started,
    Stopped,
    /// A prompt is open whose Play presses Play again; a room launch waits on it.
    Asking,
}

impl Shell {
    pub(crate) fn play_state(&self) -> PlayState {
        if self.owners.contains(&Owner::Active) {
            return PlayState::Running;
        }
        if self.build.is_some() {
            return PlayState::Building;
        }
        if self.play_pending.is_active() {
            return PlayState::Preparing;
        }
        if let Some(launch) = &self.launch {
            return if launch.saw_game {
                PlayState::Restoring
            } else {
                PlayState::Launching
            };
        }
        if self.restoring {
            return PlayState::Restoring;
        }
        if !self.detected {
            return PlayState::Detecting;
        }
        if let Some(owner) = self.owners.first() {
            return PlayState::Elsewhere(owner.clone());
        }
        if self.install.is_none() {
            return PlayState::NotInstalled;
        }
        if self.addons.checking {
            return PlayState::Checking;
        }
        match &self.session {
            Some(s)
                if s.crashed()
                    && self.dismissed.as_ref() != Some(&s.dir)
                    && s.ended.is_some_and(|e| game::now_secs() - e < 1800.0) =>
            {
                PlayState::Crashed(s.clone())
            }
            _ => PlayState::Ready,
        }
    }

    pub(crate) fn play(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let room = self.mp_take_held();
        let is_room = room.is_some();
        if !self.play_for(room, window, cx) && is_room {
            self.mp_room_launch_result(false);
        }
    }

    /// Play, or Play for a room: armed to go straight to the room's lobby, with the host's row ids for the
    /// build when `room` has them. Whether Play was taken up: the pre-launch checks run off the UI thread
    /// and the launch follows when they pass.
    pub(crate) fn play_for(
        &mut self,
        room: Option<crate::multiplayer::RoomLaunch>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.data_move_locked() || self.play_pending.is_active() {
            return false;
        }
        let Some(install) = self.install.clone() else {
            return false;
        };
        if install.source == Source::EaApp && self.ea_app_warn_pending() {
            self.ea_app_warn(window, cx);
            return false;
        }
        if self.addons.checking {
            window.notify(
                Notification::info(tr("Play when the conflict check finishes.")),
                cx,
            );
            return false;
        }
        if let Some(moved) = install.moved.clone() {
            self.show_ea_moved(moved, window, cx);
            return false;
        }
        let Some(generation) = self.play_pending.begin() else {
            return false;
        };
        let inputs = play_checks::Inputs {
            store: self.store.clone(),
            profile: self.library.profile.name.clone(),
            game: install.dir.clone(),
            build_id: install.build_id,
            anyway: std::mem::take(&mut self.play_anyway),
        };
        // This press's ask only: a dialog, a cancel or a stale result must not hand it to the next Play.
        let steam_saves_once = std::mem::take(&mut self.steam_saves_once);
        cx.notify();
        let is_room = room.is_some();
        cx.spawn_in(window, async move |this, cx| {
            let checked = cx
                .background_executor()
                .spawn(async move { play_checks::run(inputs) })
                .await;
            this.update_in(cx, |s, window, cx| {
                if !s.play_pending.finish(generation) {
                    return;
                }
                let played = s.play_checked(checked, room, steam_saves_once, window, cx);
                if is_room && played != Played::Asking {
                    s.mp_room_launch_result(played == Played::Started);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        true
    }

    pub(super) fn play_checked(
        &mut self,
        checked: play_checks::Checked,
        room: Option<crate::multiplayer::RoomLaunch>,
        steam_saves_once: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Played {
        crate::app_log::note(&checked.timings.line());
        for line in &checked.lines {
            crate::app_log::note(line);
        }
        let stale = self.library.profile.name != checked.profile
            || self.install.as_ref().map(|i| &i.dir) != Some(&checked.game);
        let Some(install) = self.install.clone().filter(|_| !stale) else {
            crate::app_log::note("[play] dropped: the profile or game copy changed during the checks");
            return Played::Stopped;
        };
        let gate = checked.gate;
        if gate.write_blocked {
            self.show_admin_needed(&install, window, cx);
            return Played::Stopped;
        }
        if !gate.setup_done {
            self.open_setup(window, cx);
            return Played::Stopped;
        }
        if !gate.offline_accepted {
            self.confirm_offline_play(window, cx);
            return self.asking(room);
        }
        if gate.first_launch {
            self.first_launch_notice(window, cx);
            return self.asking(room);
        }
        self.notice = None;
        self.blocked = None;
        self.game_edited = None;
        Blocked::clear(&self.store.root).ok();
        self.seen.blocked = game::blocked_at(&self.store.root).unwrap_or(0.0);
        self.dismissed = self.session.as_ref().map(|s| s.dir.clone());
        for note in &checked.notes {
            window.notify(note_notification(note), cx);
        }
        match checked.verdict {
            play_checks::Verdict::Go => {}
            play_checks::Verdict::Stop(stop) => {
                self.notice = Some(stop);
                return Played::Stopped;
            }
            play_checks::Verdict::Confirm(text) => {
                self.confirm_missing_rows(text, window, cx);
                return self.asking(room);
            }
        }
        if let Some(d) = swofty_core::known_problems::outdated_exe(install.version().as_deref()) {
            window.notify(Notification::warning(d.text()).title(d.title).autohide(false), cx);
        }
        if let Some(short) = crate::settings::low_space_warning(checked.room) {
            let text = trf!(
                "{short} Settings shows what takes the room and clears old logs.",
                short = short
            );
            crate::app_log::note(&format!("play: {text}"));
            window.notify(
                Notification::warning(text)
                    .title(tr("Low on disk space"))
                    .autohide(false),
                cx,
            );
        }
        let armed = (room.is_some() || self.mp_allowed()).then(|| {
            swofty_core::multiplayer::client::launch_env_value(&self.store.root, SESSION_MODULE_WAIT)
        });
        match Launch::start(
            &self.store,
            &self.library.profile.name,
            &install,
            self.launch_mode(),
            armed.as_deref(),
            room.as_ref().and_then(|r| r.row_ids.as_deref()),
            room.as_ref().map(|r| r.direct_start.as_str()),
            steam_saves_once,
        ) {
            Ok(l) => {
                self.launch = Some(l);
                Played::Started
            }
            Err(e) => {
                self.seen.launch_failed = game::now_secs();
                window.notify(
                    Notification::error(e)
                        .title(tr("Could not launch F1 25"))
                        .autohide(false),
                    cx,
                );
                Played::Stopped
            }
        }
    }

    fn asking(&mut self, room: Option<crate::multiplayer::RoomLaunch>) -> Played {
        if let Some(room) = room {
            self.mp_hold_room(room);
        }
        Played::Asking
    }

    pub(crate) fn saves_refused(&self) -> Option<&SessionRecord> {
        self.session.as_ref().filter(|s| {
            s.exit_code == Some(swofty_core::launcher_saves::EXIT)
                && self.dismissed.as_ref() != Some(&s.dir)
                && s.ended.is_some_and(|e| game::now_secs() - e < 1800.0)
        })
    }

    pub(crate) fn play_with_steam_saves(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.steam_saves_once = true;
        if !self.play_for(None, window, cx) {
            self.steam_saves_once = false;
        }
    }

    pub(crate) fn launch_mode(&self) -> LaunchMode {
        if self.vr_profiles.contains(&self.library.profile.name) {
            LaunchMode::Vr
        } else {
            LaunchMode::Normal
        }
    }

    /// The game switcher: play another copy of F1 25.
    pub(crate) fn use_copy(&mut self, dir: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.play_pending.cancel();
        let mut config = self.store.config();
        swofty_core::copies::set_active(&mut config, &dir);
        if let Err(e) = self.store.save_config(&config) {
            crate::app_log::action("save settings").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
            return;
        }
        if let Some(copy) = self.copies.iter().find(|c| c.dir == dir) {
            self.install = Some(copy.clone());
            crate::app_log::set_game(Some(&copy.dir));
        }
        self.attribute();
        self.notice = None;
        self.rediscover = true;
        self.reload(cx);
    }
}
