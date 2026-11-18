use crate::scale::px;
use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::dbedit::{Hit, HitKind};
use swofty_i18n::{tr, trf};

use crate::pages::{GUTTER, label};
use crate::shell::Shell;
use crate::theme::{Palette, ScrollArea};

fn hit_group(kind: HitKind) -> &'static str {
    match kind {
        HitKind::ModuleRow => tr("Drivers, teams and tracks"),
        HitKind::Added => tr("Driver Manager drivers"),
        HitKind::Table => tr("Tables"),
        HitKind::Row => tr("Rows"),
        HitKind::Key => tr("Keys"),
        HitKind::Value => tr("Values"),
        HitKind::Text => tr("English text"),
    }
}

fn hit_icon(kind: HitKind) -> IconName {
    match kind {
        HitKind::ModuleRow => IconName::User,
        HitKind::Added => IconName::Users,
        HitKind::Table => IconName::Table2,
        HitKind::Row => IconName::Rows3,
        HitKind::Key => IconName::KeyRound,
        HitKind::Value => IconName::Hash,
        HitKind::Text => IconName::Languages,
    }
}

impl Shell {
    pub(super) fn db_search_bar(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(search) = &self.db.search else {
            return div().into_any_element();
        };
        let query = search.query.trim().to_owned();
        let open = self.db.hits_open && !query.is_empty();
        let results = open.then(|| self.db_hits_popup(&query, p, cx));
        h_flex()
            .px(px(GUTTER))
            .pb(px(14.))
            .flex_shrink_0()
            .child(
                div()
                    .relative()
                    .w(px(620.))
                    .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                        let n = this.db.hits.len();
                        match ev.keystroke.key.as_str() {
                            "down" if n > 0 => this.db.hit = (this.db.hit + 1) % n,
                            "up" if n > 0 => this.db.hit = (this.db.hit + n - 1) % n,
                            _ => return,
                        }
                        this.db.hits_open = true;
                        cx.stop_propagation();
                        cx.notify();
                    }))
                    .child(
                        Input::new(&search.input).cleanable(true).prefix(
                            Icon::new(IconName::Search)
                                .size(px(15.))
                                .text_color(p.text_placeholder),
                        ),
                    )
                    .children(results),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .child(tr("Ctrl+F or / to search")),
            )
            .into_any_element()
    }

    fn db_hits_popup(&self, query: &str, p: Palette, cx: &mut Context<Self>) -> Deferred {
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut last = None;
        for (i, h) in self.db.hits.iter().enumerate() {
            if last != Some(h.kind) {
                last = Some(h.kind);
                rows.push(
                    div()
                        .px(px(12.))
                        .pt(px(if i == 0 { 8. } else { 12. }))
                        .pb(px(4.))
                        .child(label(hit_group(h.kind).to_uppercase(), p))
                        .into_any_element(),
                );
            }
            rows.push(self.db_hit_row(i, h, p, cx));
        }
        let empty = self.db.hits.is_empty().then(|| {
            div()
                .px(px(14.))
                .py(px(14.))
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(trf!("Nothing matches “{query}”", query = query))
        });
        deferred(
            v_flex()
                .id("db-hits")
                .occlude()
                .absolute()
                .top(px(40.))
                .left_0()
                .w(px(620.))
                .max_h(px(440.))
                .rounded(px(8.))
                .border_1()
                .border_color(p.border)
                .bg(p.elevated)
                .shadow_lg()
                .overflow_hidden()
                .child(
                    v_flex()
                        .flex_1()
                        .scroll_y("db-hits-list")
                        .pb(px(6.))
                        .children(rows)
                        .children(empty),
                ),
        )
        .with_priority(1)
    }

    fn db_hit_row(&self, i: usize, h: &Hit, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let on = i == self.db.hit;
        h_flex()
            .id(SharedString::from(format!("db-hit-{i}")))
            .mx(px(4.))
            .h(px(40.))
            .px(px(8.))
            .gap(px(10.))
            .rounded(px(6.))
            .cursor_pointer()
            .when(on, |d| d.bg(p.selected))
            .hover(move |s| s.bg(p.hover))
            .child(
                Icon::new(hit_icon(h.kind))
                    .size(px(14.))
                    .text_color(p.text_placeholder),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_size(px(13.)).truncate().child(h.title.clone()))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.text_muted)
                            .truncate()
                            .child(h.detail.clone()),
                    ),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.db_open_hit(i, window, cx)))
            .into_any_element()
    }
}
