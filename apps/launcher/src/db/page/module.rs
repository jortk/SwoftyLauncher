use std::ops::Range;

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::Input;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::dbedit::{Module, Scope};
use swofty_i18n::{tr, trf, trn};

use super::{MOVE_W, NAME_W, notice};
use crate::db::{ViewSlot, icon_named, swap_field};
use crate::shell::Shell;
use crate::theme::Palette;
use crate::ui::{SectionHead, chevron};

impl Shell {
    pub(super) fn db_module(&mut self, id: &str, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(module) = self.db.module(id) else {
            return div().into_any_element();
        };
        let detail_w = if self.db.row.is_some() { 396. } else { 0. };
        let moves_w = if swap_field(module).is_some() { 68. } else { 0. };
        let room = self.content_width - 240. - detail_w - moves_w - NAME_W - 26.;
        self.db.max_cols = ((room / 124.).floor() as usize).max(1);
        let slot = self.db.view.as_ref().filter(|v| v.module == id);
        let links: Vec<Button> = module
            .links
            .iter()
            .enumerate()
            .map(|(i, l)| self.db_link_button(i, l, cx))
            .collect();
        let head = h_flex()
            .gap(px(12.))
            .flex_shrink_0()
            .child(SectionHead::new(
                icon_named(&module.icon),
                module.title.clone(),
                module.summary.clone(),
            ))
            .children(links);
        let notes_open = self.db.notes_open || !self.short;
        let about = (self.short && !module.notes.is_empty()).then(|| {
            h_flex()
                .id("db-notes-toggle")
                .gap(px(6.))
                .flex_shrink_0()
                .cursor_pointer()
                .text_size(px(12.))
                .text_color(p.text_placeholder)
                .hover(move |s| s.text_color(p.text_muted))
                .child(chevron(notes_open, p))
                .child(tr("About this page"))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.db.notes_open = !this.db.notes_open;
                    cx.notify();
                }))
        });
        let notes = module
            .notes
            .iter()
            .enumerate()
            .filter(|_| notes_open)
            .map(|(i, n)| {
                notice("db-note", IconName::Info, p.text_placeholder, n.clone(), p)
                    .id(SharedString::from(format!("db-note-{i}")))
                    .into_any_element()
            });
        let warnings: Vec<AnyElement> = slot
            .map(|s| s.view.warnings.clone())
            .unwrap_or_default()
            .into_iter()
            .enumerate()
            .map(|(i, w)| {
                notice("db-warning", IconName::TriangleAlert, p.warning, w, p)
                    .id(SharedString::from(format!("db-warning-{i}")))
                    .into_any_element()
            })
            .collect();
        let toolbar = slot.map(|slot| self.db_module_toolbar(module, slot, p, cx));
        let has_slot = slot.is_some();
        let content = match (has_slot, self.db.view_error.clone()) {
            (_, Some(e)) => self.empty(
                IconName::CircleAlert,
                tr("This module could not load"),
                e,
                div(),
                p,
            ),
            (false, None) => self.db_loading(tr("Loading rows…"), p),
            (true, None) => self.db_table(module, p, cx),
        };
        let detail = self
            .db
            .row
            .clone()
            .filter(|_| has_slot)
            .map(|row| self.db_row_detail(module, &row, p, cx));
        v_flex()
            .size_full()
            .gap(px(14.))
            .child(head)
            .children(about)
            .children(notes)
            .children(warnings)
            .children(toolbar)
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .gap(px(16.))
                    .items_start()
                    .child(div().flex_1().min_w_0().h_full().flex().flex_col().child(content))
                    .children(detail),
            )
            .into_any_element()
    }

    fn db_module_toolbar(
        &self,
        module: &'static Module,
        slot: &ViewSlot,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let total = slot.view.rows.len();
        let count = if slot.shown.len() == total {
            trn(total, "{count} row", "{count} rows")
        } else {
            trf!("{shown} of {total} rows", shown = slot.shown.len(), total = total)
        };
        h_flex()
            .gap(px(16.))
            .flex_shrink_0()
            .children(
                module
                    .scope
                    .as_ref()
                    .map(|sc| Self::db_scope_picker(sc, slot, p, cx)),
            )
            .children(self.db.filter.as_ref().map(|f| {
                div().w(px(280.)).child(
                    Input::new(&f.input).tap_sm().cleanable(true).prefix(
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
            )
    }

    fn db_scope_picker(sc: &Scope, slot: &ViewSlot, p: Palette, cx: &mut Context<Self>) -> Div {
        let current = slot.scope.clone().unwrap_or_default();
        let scopes = slot.view.scopes.clone();
        let shown = scopes
            .iter()
            .find(|(l, _)| *l == current)
            .map(|(l, n)| if n.is_empty() { l.clone() } else { n.clone() })
            .unwrap_or_else(|| current.clone());
        let shell = cx.entity().downgrade();
        h_flex()
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(p.text_muted)
                    .child(sc.title.clone()),
            )
            .child(
                Button::new("db-scope")
                    .outline()
                    .tap_sm()
                    .min_w(px(200.))
                    .label(shown)
                    .dropdown_caret(true)
                    .dropdown_menu_with_anchor(Anchor::TopLeft, move |mut menu, _, _| {
                        menu = menu.max_h(px(360.)).scrollable(true).min_w(px(260.));
                        for (l, n) in &scopes {
                            let (shell, value) = (shell.clone(), l.clone());
                            let text = if n.is_empty() { l.clone() } else { n.clone() };
                            menu = menu.item(PopupMenuItem::new(text).checked(*l == current).on_click(
                                move |_, _, cx| {
                                    shell.update(cx, |s, cx| s.db_set_scope(value.clone(), cx)).ok();
                                },
                            ));
                        }
                        menu
                    }),
            )
    }

    fn db_table(&mut self, module: &'static Module, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(slot) = self.db.view.as_ref() else {
            return div().into_any_element();
        };
        if slot.shown.is_empty() {
            let filtered = !slot.view.rows.is_empty();
            return self.empty(
                IconName::Search,
                if filtered {
                    tr("No matching rows")
                } else {
                    tr("No rows")
                },
                if filtered {
                    tr("Nothing in this list matches the filter.")
                } else {
                    tr("This table has no rows in the stock database.")
                },
                div(),
                p,
            );
        }
        let columns: Vec<(usize, String)> = slot
            .view
            .columns
            .iter()
            .take(self.db.max_cols)
            .map(|c| (c.field, c.label.clone()))
            .collect();
        let moves = swap_field(module).is_some() && !self.db_locked();
        let header = h_flex()
            .h(px(32.))
            .px(px(12.))
            .gap(px(12.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(p.border_variant)
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.text_placeholder)
            .child(div().w(px(NAME_W)).flex_shrink_0().child(tr("NAME")))
            .children(columns.iter().map(|(f, l)| {
                let editable = module.fields.get(*f).is_some_and(|s| !s.readonly);
                h_flex()
                    .flex_1()
                    .flex_basis(px(0.))
                    .min_w_0()
                    .gap(px(4.))
                    .child(div().truncate().child(l.to_uppercase()))
                    .when(editable && !self.db_locked(), |d| {
                        d.child(
                            Icon::new(IconName::Pencil)
                                .size(px(10.))
                                .text_color(p.text_placeholder),
                        )
                    })
            }))
            .when(moves, |d| d.child(div().w(px(MOVE_W)).flex_shrink_0()));
        let count = slot.shown.len();
        let list = uniform_list(
            "db-rows",
            count,
            cx.processor(move |this, range: Range<usize>, _, cx| this.db_rows(module, range, cx)),
        )
        .flex_1()
        .track_scroll(&self.db.rows_scroll);
        v_flex()
            .size_full()
            .rounded(px(6.))
            .border_1()
            .border_color(p.border_variant)
            .overflow_hidden()
            .child(header)
            .child(self.db.rows_motion.wrap(list, &self.db.rows_scroll, cx))
            .into_any_element()
    }
}
