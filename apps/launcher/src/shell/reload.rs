use crate::library::Library;
use gpui_kit::*;
use std::time::Instant;

use super::{Page, Shell};

impl Shell {
    pub(crate) fn reload(&mut self, cx: &mut Context<Self>) {
        self.reload_generation += 1;
        let library = Library::load(&self.store);
        self.install_library(library, cx);
    }

    pub(crate) fn reload_in_background(&mut self, cx: &mut Context<Self>) {
        self.reload_generation += 1;
        let generation = self.reload_generation;
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let library = cx
                .background_executor()
                .spawn(async move { Library::read(&store) })
                .await;
            crate::app_log::note(&format!(
                "profile switch: library loaded in {} ms",
                started.elapsed().as_millis()
            ));
            this.update(cx, |s, cx| {
                if s.reload_generation == generation {
                    let mut library = library;
                    library.save_order(&s.store);
                    s.install_library(library, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn install_library(&mut self, library: Library, cx: &mut Context<Self>) {
        self.library = library;
        let config = self.store.config();
        self.developer_mode = config.developer;
        self.clear_old_session_logs = config.clear_old_session_logs;
        self.time_trial_boards = config.time_trial_boards;
        self.use_profiles = config.use_profiles;
        self.time_trial_ingame = config.time_trial_ingame;
        self.home_video = config.home_video;
        self.update_off_addons = config.update_off_addons;
        self.vr_profiles = config.vr_profiles;
        self.start_store = config.launch.start_store;
        self.update_check = config.updates.check;
        if matches!(self.page, Page::Section(i) if i >= self.library.sections.len()) {
            self.page = Page::Addons;
        }
        if self.page == Page::Careers {
            self.refresh_saves_in_background(cx);
        } else {
            self.refresh_banner_in_background(cx);
        }
        self.refresh_views(cx);
        self.db_profile_changed(cx);
        self.sync_catalog_updates();
        self.sync_shaders(true, cx);
        cx.notify();
    }

    pub(crate) fn sync_shaders(&mut self, refresh: bool, cx: &mut Context<Self>) {
        let dir = self.install.as_ref().map(|i| i.dir.clone());
        // Live edits need a session of this data dir whose game started with a shader config on: a game
        // that started with shaders off has no effects loaded.
        let running = dir.as_ref().is_some_and(|d| {
            self.processes.iter().any(|g| g.is_in(d)) && d.join("swofty/shaders/effects.json").exists()
        }) && swofty_core::recovery::SessionLock::held_elsewhere(&self.store.root);
        let info = crate::shaders::Info {
            profile: self.library.profile.name.clone(),
            game_dir: dir,
            running,
        };
        self.shaders.update(cx, |v, cx| v.sync(info, refresh, cx));
    }

    pub(super) fn shaders_event(
        &mut self,
        _: Entity<crate::shaders::ShadersView>,
        event: &crate::shaders::Event,
        cx: &mut Context<Self>,
    ) {
        match event {
            crate::shaders::Event::Browse => {
                self.open_page(Page::Browser, cx);
                self.show_shader_addons(cx);
            }
            crate::shaders::Event::OpenAddon(id) => {
                self.highlight = Some((id.clone(), Instant::now()));
                self.open_page(Page::Addons, cx);
            }
            crate::shaders::Event::ActiveChanged => self.reload_in_background(cx),
        }
    }

    pub(crate) fn open_shaders_for(&mut self, addon: &str, cx: &mut Context<Self>) {
        self.shaders.update(cx, |v, cx| v.select_addon(addon, cx));
        self.open_page(Page::Shaders, cx);
    }
}
