use crate::notices::{Notification, Notify};
use gpui_kit::*;
use swofty_core::session::LaunchMode;

use super::Shell;
use swofty_i18n::trf;

impl Shell {
    pub(crate) fn set_launcher_saves(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.library.profile.name.clone();
        match self.store.update_profile(&name, |p| {
            p.launcher_saves = on;
            Ok(())
        }) {
            Ok(()) => {
                crate::app_log::note(&format!(
                    "saves: profile {name} now plays on {}",
                    if on { "its launcher saves" } else { "Steam's saves" }
                ));
                self.library.profile.launcher_saves = on;
                window.notify(
                    Notification::info(match on {
                        true => trf!("{name} plays on its own launcher saves.", name = name),
                        false => trf!("{name} plays on your Steam saves.", name = name),
                    }),
                    cx,
                );
            }
            Err(e) => {
                crate::app_log::action("save profile").failed(&e);
                window.notify(Notification::error(e.to_string()), cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn set_start_store(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.launch.start_store = on;
        if let Err(e) = self.store.save_config(&config) {
            crate::app_log::action("save settings").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
        self.start_store = config.launch.start_store;
        cx.notify();
    }

    pub(crate) fn set_clear_old_session_logs(
        &mut self,
        on: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut config = self.store.config();
        config.clear_old_session_logs = on;
        if let Err(e) = self.store.save_config(&config) {
            crate::app_log::action("save settings").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
        self.clear_old_session_logs = config.clear_old_session_logs;
        cx.notify();
    }

    pub(crate) fn set_use_profiles(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.use_profiles = on;
        if let Err(e) = self.store.save_config(&config) {
            crate::app_log::action("save settings").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
        self.use_profiles = config.use_profiles;
        if !on {
            let active = self.library.profile.name.clone();
            if let Some(name) = crate::profiles::implicit_profile(&self.library.profiles, &active) {
                self.switch_profile(&name, window, cx);
            }
        }
        cx.notify();
    }

    pub(crate) fn set_time_trial_boards(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.time_trial_boards = on;
        if let Err(e) = self.store.save_config(&config) {
            crate::app_log::action("save settings").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
        self.time_trial_boards = config.time_trial_boards;
        cx.notify();
    }

    pub(crate) fn set_time_trial_ingame(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.time_trial_ingame = on;
        if let Err(e) = self.store.save_config(&config) {
            crate::app_log::action("save settings").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
        self.time_trial_ingame = config.time_trial_ingame;
        cx.notify();
    }

    pub(crate) fn set_home_video(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.home_video = on;
        if let Err(e) = self.store.save_config(&config) {
            crate::app_log::action("save settings").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
        self.home_video = config.home_video;
        cx.notify();
    }

    pub(crate) fn set_update_off_addons(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.update_off_addons = on;
        if let Err(e) = self.store.save_config(&config) {
            crate::app_log::action("save settings").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
        self.update_off_addons = config.update_off_addons;
        cx.notify();
    }

    pub(crate) fn set_launch_mode(&mut self, mode: LaunchMode, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.store.config();
        config.set_launch_mode(&self.library.profile.name, mode);
        if let Err(e) = self.store.save_config(&config) {
            crate::app_log::action("save settings").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
        self.vr_profiles = config.vr_profiles;
        cx.notify();
    }
}
