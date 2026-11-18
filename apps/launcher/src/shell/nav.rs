use gpui_kit::*;

use super::{Page, Shell};

impl Shell {
    pub(crate) fn cancel_scroll(&self) {
        self.news_scroll.cancel();
        self.addons.cancel_scroll();
        self.browser.smooth.cancel();
        self.db.rows_motion.cancel();
        self.db.list_motion.cancel();
    }

    pub(crate) fn open_page(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page != page {
            self.cancel_scroll();
        }
        self.db_page_change(page, cx);
        let was = self.page;
        self.page = page;
        if page == Page::Online {
            self.online_enter(cx);
        }
        if page == Page::Shaders {
            self.sync_shaders(true, cx);
        }
        if page == Page::Careers && was != Page::Careers {
            self.refresh_saves_in_background(cx);
        }
        if page == Page::Moderation {
            self.tt_open(cx);
        }
        if page == Page::Support {
            self.support_open(cx);
        }
        if let Some(i) = self.open_section().filter(|i| !self.section_loaded(*i)) {
            self.fetch_views(Some(i), cx);
        }
        cx.notify();
    }

    pub(super) fn section_loaded(&self, i: usize) -> bool {
        self.library
            .sections
            .get(i)
            .is_some_and(|s| self.views.contains_key(&format!("{}/{}", s.addon, s.spec.id)))
    }
}
