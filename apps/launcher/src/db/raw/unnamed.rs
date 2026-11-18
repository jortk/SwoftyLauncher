use std::ops::Range;

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::*;
use swofty_i18n::{tr, trn};

use crate::shell::Shell;
use crate::theme::{Palette, palette};
use crate::ui::SectionHead;

impl Shell {
    pub(crate) fn db_unnamed_view(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.db.unnamed_rows.clone();
        let total = self
            .db
            .catalog()
            .map_or(0, |c| c.records().saturating_sub(c.named()));
        let filtered = self
            .db
            .unnamed
            .as_ref()
            .is_some_and(|d| !d.query.trim().is_empty());
        let count = if filtered {
            format!("{} of {total}", rows.len())
        } else {
            trn(total, "{count} record", "{count} records")
        };
        let list = if rows.is_empty() {
            self.empty(
                IconName::Search,
                tr("No matching keys"),
                tr("No unnamed record has that key or value."),
                div(),
                p,
            )
        } else {
            let shown = rows.clone();
            let list = uniform_list(
                "db-unnamed",
                rows.len(),
                cx.processor(move |this, range: Range<usize>, _, cx| {
                    let p = palette(cx);
                    let Some(catalog) = this.db.catalog().cloned() else {
                        return Vec::new();
                    };
                    range
                        .filter_map(|i| shown.get(i).map(|&n| (i, n)))
                        .map(|(i, n)| this.db_entry(&catalog, i, n, false, p, cx))
                        .collect::<Vec<_>>()
                }),
            )
            .flex_1()
            .track_scroll(&self.db.list_scroll);
            v_flex()
                .size_full()
                .rounded(px(6.))
                .border_1()
                .border_color(p.border_variant)
                .overflow_hidden()
                .child(self.db.list_motion.wrap(list, &self.db.list_scroll, cx))
                .into_any_element()
        };
        let detail = self.db.key.map(|k| self.db_key_detail(k, p, cx));
        v_flex()
            .size_full()
            .gap(px(14.))
            .child(h_flex().gap(px(12.)).flex_shrink_0().child(SectionHead::new(
                IconName::Binary,
                tr("Unnamed keys"),
                tr("Records whose names aren't known yet. Their values still edit."),
            )))
            .child(
                h_flex()
                    .gap(px(16.))
                    .flex_shrink_0()
                    .children(self.db.unnamed.as_ref().map(|d| {
                        div().w(px(280.)).child(
                            Input::new(&d.input).tap_sm().cleanable(true).prefix(
                                Icon::new(IconName::ListFilter)
                                    .size(px(14.))
                                    .text_color(p.text_placeholder),
                            ),
                        )
                    }))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(count),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .gap(px(16.))
                    .items_start()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(self.db.list_motion.wrap(list, &self.db.list_scroll, cx)),
                    )
                    .children(detail),
            )
            .into_any_element()
    }
}
