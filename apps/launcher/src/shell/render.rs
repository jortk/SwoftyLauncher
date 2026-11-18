use crate::notices::Notify;
use crate::scale::px;
use crate::theme::palette;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::time::Instant;

use super::{Page, PlayState, Shell};

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let started = Instant::now();
        self.painted = Instant::now();
        let p = palette(cx);
        if let Some(wizard) = self.onboarding_view(p, cx) {
            self.ads.begin(false);
            return wizard.children(self.ads.sweep());
        }
        let state = self.play_state();
        if self
            .ads
            .game(matches!(state, PlayState::Running | PlayState::Elsewhere(_)))
        {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(crate::ads::RELEASE_AFTER).await;
                this.update(cx, |this, cx| {
                    if this.ads.release_for_game() {
                        cx.notify();
                    }
                })
                .ok();
            })
            .detach();
        }
        let covered = self.ads_covered(window, cx);
        self.ads.begin(covered);
        let ad_video = self.ad_video(&state, p, cx);
        let wide = window.viewport_size().width > px(1500.);
        let short = window.viewport_size().height < px(760.);
        self.short = short;
        let layout_width = f32::from(window.viewport_size().width);
        self.content_width =
            crate::scale::content_width(f32::from(window.viewport_size().width), crate::scale::current());
        self.layout_width = layout_width;
        let side = self.profiles_sidebar(p, cx);
        if side.is_some() {
            self.content_width = (self.content_width - self.profiles_width()).max(320.);
        }
        self.sync_sliders(window, cx);
        self.sync_blocks(window, cx);
        self.db_pending_tab(window, cx);
        if self.page == Page::Database {
            self.sync_db(window, cx);
        }
        for note in std::mem::take(&mut self.db.toasts) {
            window.notify(note, cx);
        }
        for note in std::mem::take(&mut self.browser.toasts) {
            window.notify(note, cx);
        }
        for note in std::mem::take(&mut self.mp_toasts) {
            window.notify(note, cx);
        }
        let content = match self.page {
            Page::News => self.news_page(p, short, cx),
            Page::Importer => self.importer_page(p, cx),
            Page::Tools => match self.developer_section(p, cx) {
                Some(dev) => v_flex()
                    .size_full()
                    .child(div().flex_1().min_h_0().child(self.tools_page(p, window, cx)))
                    .child(div().px(px(crate::pages::GUTTER)).pb(px(16.)).child(dev))
                    .into_any_element(),
                None => self.tools_page(p, window, cx),
            },
            Page::Addons => self.addons_page(p, cx),
            Page::Browser => self.browser_page(p, cx),
            Page::Downloads => self.downloads_page(p, cx),
            Page::Shaders => self.shaders.clone().into_any_element(),
            Page::Database => self.db_page(p, cx),
            Page::Careers => self.careers_page(p, wide, cx),
            Page::Multiplayer => self.multiplayer_page(p, cx),
            Page::Online => self.online_page(p, window, cx),
            Page::Moderation => self.time_trial_page(p, window, cx),
            Page::Support => self.support_page(p, window, cx),
            Page::Settings => self.settings_page(p, cx),
            Page::Section(i) => self.section_page(i, p, cx),
        };
        if self.dropping && !cx.has_active_drag() {
            self.dropping = false;
        }
        let has_sub = self.sub_nav(layout_width).is_some();
        let page = self.page_name();
        let tree = v_flex()
            .id("shell")
            .relative()
            .track_focus(&self.root_focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| this.nav_key(ev, window, cx)))
            .size_full()
            .on_drag_move::<ExternalPaths>(cx.listener(|this, _, _, cx| {
                if !this.dropping {
                    this.dropping = true;
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.drop_paths(paths.paths().to_vec(), window, cx)
            }))
            .bg(p.background)
            .text_color(p.text)
            .text_size(px(14.))
            .child(self.top_bar(p, layout_width, cx))
            .child(
                h_flex().flex_1().min_h_0().items_start().bg(p.background).child(
                    h_flex()
                        .flex_1()
                        .h_full()
                        .min_w_0()
                        .overflow_hidden()
                        .bg(p.background)
                        .children(side)
                        .child(
                            v_flex()
                                .flex_1()
                                .h_full()
                                .min_w_0()
                                .pt(crate::scale::fixed(if has_sub {
                                    super::topbar::SUB_HEIGHT - 20.
                                } else {
                                    0.
                                }))
                                .child(content),
                        ),
                ),
            )
            .children(self.sub_row(p, layout_width, cx))
            .children(self.download_notch(p, layout_width, has_sub, window, cx))
            .children(self.popover_layer(p, cx))
            .children(self.popup_layer(p, cx))
            .children(self.update_bar(p))
            .children(self.scale_rescue(p, cx))
            .child(self.play_bar(p, wide, cx))
            .children(ad_video)
            .children(self.task_dialog(p, window, cx))
            .children(self.crash_overlay(p, cx))
            .children(self.ban_overlay(p, cx))
            .children(self.data_move_overlay(p, cx))
            .children(self.found_data_overlay(p, cx))
            .when(self.dropping, |d| d.child(self.drop_overlay(p)))
            .children(self.consent_probe(p))
            .children(self.ads.sweep());
        self.note_slow("render", page, started);
        tree
    }
}
