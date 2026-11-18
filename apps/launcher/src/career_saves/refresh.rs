use gpui_kit::*;

use super::build::build_saves;
use super::state::SavesPage;
use crate::shell::{Page, Shell};

impl Shell {
    pub(crate) fn refresh_saves_in_background(&mut self, cx: &mut Context<Self>) {
        self.saves.generation += 1;
        self.saves.loading = true;
        let generation = self.saves.generation;
        let store = self.store.clone();
        let install = self.install.clone();
        let name = self.library.profile.name.clone();
        let game_running = self.saves.game_running;
        cx.spawn(async move |this, cx| {
            let built = cx
                .background_executor()
                .spawn(async move { build_saves(&store, install.as_ref(), &name, game_running) })
                .await;
            this.update(cx, |s, cx| {
                if s.saves.generation == generation {
                    s.saves.apply(built);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn refresh_banner_in_background(&mut self, cx: &mut Context<Self>) {
        self.saves.generation += 1;
        let generation = self.saves.generation;
        let root = self.store.root.clone();
        let name = self.library.profile.name.clone();
        let name_done = name.clone();
        cx.spawn(async move |this, cx| {
            let banner = cx
                .background_executor()
                .spawn(async move { SavesPage::banner_for(&root, &name) })
                .await;
            this.update(cx, |s, cx| {
                if s.saves.generation == generation {
                    s.saves.banner_profile = name_done;
                    s.saves.banner = banner;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn open_saves(&mut self, confirm: Option<String>, cx: &mut Context<Self>) {
        self.saves.confirm = confirm;
        self.open_page(Page::Careers, cx);
    }
}
