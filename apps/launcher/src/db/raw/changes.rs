use std::ops::Range;
use std::sync::Arc;

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Disableable, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::dbedit::{Edit, NodeKind, Origin, RowCopy};
use swofty_i18n::{tr, trf, trn};

use super::hex;
use crate::pages::label;
use crate::shell::Shell;
use crate::theme::{Palette, palette};
use crate::ui::SectionHead;

use super::*;

impl Shell {
    pub(super) fn db_change_items(&self) -> Vec<ChangeItem> {
        let Some(catalog) = self.db.catalog().cloned() else {
            return Vec::new();
        };
        let mut groups: Vec<(String, Vec<ChangeItem>)> = Vec::new();
        let rows: Vec<ChangeItem> = self
            .db
            .draft
            .edits
            .rows
            .iter()
            .map(|r| ChangeItem {
                group: None,
                edit: None,
                row: Some(r.clone()),
                name: trf!("{name} › Own row", name = self.db_row_name(&r.to)),
                path: Some(r.select()),
                node: None,
            })
            .collect();
        if !rows.is_empty() {
            groups.push((tr("Copied rows").to_owned(), rows));
        }
        for e in &self.db.draft.edits.edits {
            let node = catalog.node_of_key(e.key);
            let mut path = Some(e.path.clone().unwrap_or_else(|| hex(e.key)));
            let (name, group) = match node {
                Some(n) if e.stock.is_some() => {
                    let (mut name, row) = friendly(&catalog, n);
                    let group = match table_of(&catalog, n) {
                        Some(t) => {
                            let table = catalog.path(t);
                            let shaped = shape(&table);
                            let module = self.db.modules.iter().find(|m| shape(&m.rows) == shaped);
                            if let (Some(m), Some(row)) = (module, row) {
                                let r = catalog.node(row);
                                let rel = name.split_once(" › ").map_or("", |(_, rel)| rel);
                                let field = m.fields.iter().find(|f| f.path == rel);
                                let label = r.label.clone().unwrap_or_else(|| r.name.clone());
                                let shown = self
                                    .db
                                    .view
                                    .as_ref()
                                    .filter(|v| v.module == m.id)
                                    .and_then(|v| v.view.rows.iter().find(|x| x.label == label))
                                    .map(|x| x.name.clone())
                                    .filter(|n| !n.is_empty())
                                    .or_else(|| r.display.clone())
                                    .unwrap_or_else(|| title_case(&label));
                                if let Some(f) = field {
                                    name = format!("{shown} › {}", f.label);
                                    path = None;
                                }
                            }
                            module.map_or(table, |m| m.title.clone())
                        }
                        None if catalog.node(catalog.node(n).parent.unwrap_or(0)).kind
                            == NodeKind::Unnamed =>
                        {
                            tr("Unnamed keys").to_owned()
                        }
                        None => catalog.path(catalog.node(n).parent.unwrap_or(0)),
                    };
                    (name, group)
                }
                _ if e.is_copied() => (
                    self.db_cell_name(e.key)
                        .unwrap_or_else(|| e.path.clone().unwrap_or_else(|| hex(e.key))),
                    tr("Copied rows").to_owned(),
                ),
                _ => (
                    e.path.clone().unwrap_or_else(|| hex(e.key)),
                    tr("Added keys").to_owned(),
                ),
            };
            let item = ChangeItem {
                group: None,
                edit: Some(e.clone()),
                row: None,
                name,
                path,
                node,
            };
            match groups.iter_mut().find(|(g, _)| *g == group) {
                Some((_, list)) => list.push(item),
                None => groups.push((group, vec![item])),
            }
        }
        let mut out = Vec::new();
        for (g, items) in groups {
            out.push(ChangeItem {
                group: Some(format!("{g} · {}", items.len())),
                edit: None,
                row: None,
                name: String::new(),
                path: None,
                node: None,
            });
            out.extend(items);
        }
        out
    }

    pub(crate) fn db_changes(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let n = self.db.draft.edits.len();
        let actions = h_flex().gap(px(8.)).child(
            Button::new("db-revert-all")
                .outline()
                .tap_sm()
                .icon(IconName::Undo2)
                .label(tr("Revert all changes"))
                .disabled(n == 0 || self.db_locked())
                .on_click(cx.listener(|this, _, window, cx| this.db_revert_all(window, cx))),
        );
        let head = h_flex()
            .gap(px(12.))
            .flex_shrink_0()
            .child(SectionHead::new(
                IconName::ListChecks,
                tr("Changes"),
                match self.db.draft.unsaved() {
                    0 => trf!(
                        "{changes} in {name}",
                        changes = trn(n, "{count} change", "{count} changes"),
                        name = self.db_draft_name()
                    ),
                    k => trf!(
                        "{changes} in {name} · {k} not saved yet",
                        changes = trn(n, "{count} change", "{count} changes"),
                        name = self.db_draft_name(),
                        k = k
                    ),
                },
            ))
            .child(actions);
        if n == 0 {
            return v_flex()
                .size_full()
                .gap(px(14.))
                .child(head)
                .child(self.empty(
                    IconName::ListChecks,
                    tr("No changes yet"),
                    tr("Values you change in the modules or the table browser show up here. Save them as an addon to use them in the game."),
                    div(),
                    p,
                ))
                .into_any_element();
        }
        let items = Arc::new(self.db_change_items());
        let shown = items.clone();
        v_flex()
            .size_full()
            .gap(px(14.))
            .child(head)
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(p.border_variant)
                    .overflow_hidden()
                    .child(
                        self.db.list_motion.wrap(
                            uniform_list(
                                "db-changes",
                                items.len(),
                                cx.processor(move |this, range: Range<usize>, _, cx| {
                                    let p = palette(cx);
                                    range
                                        .filter_map(|i| shown.get(i).map(|it| (i, it)))
                                        .map(|(i, it)| this.db_change_row(i, it, p, cx))
                                        .collect::<Vec<_>>()
                                }),
                            )
                            .flex_1()
                            .track_scroll(&self.db.list_scroll),
                            &self.db.list_scroll,
                            cx,
                        ),
                    ),
            )
            .into_any_element()
    }

    fn change_values(&self, e: &Edit) -> (String, String, Option<String>) {
        let copied = self.db.catalog().filter(|_| e.is_copied()).and_then(|c| {
            let source = self.db.draft.edits.copied(c, e.key)?.source;
            c.record_by_key(source).map(|r| r.value.to_owned())
        });
        let stock = match e.stock.clone().or(copied) {
            Some(s) if s.is_empty() => "empty".to_owned(),
            Some(s) => s,
            None => "added".to_owned(),
        };
        let new = match &e.value {
            Some(v) if v.is_empty() => "empty".to_owned(),
            Some(v) => v.clone(),
            None => tr("removed").to_owned(),
        };
        let origin = match &e.origin {
            Origin::Typed => None,
            Origin::Imported(f) => Some(trf!("Imported from {f}", f = f)),
        };
        (stock, new, origin)
    }

    pub(super) fn db_change_row(
        &self,
        i: usize,
        it: &ChangeItem,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(g) = &it.group {
            return change_group_head(g, i, p);
        }
        if let Some(r) = &it.row {
            return self.db_copy_change_row(r, it, p, cx);
        }
        let Some(e) = &it.edit else {
            return div().into_any_element();
        };
        let key = e.key;
        let node = it.node;
        let (stock, new, origin) = self.change_values(e);
        h_flex()
            .id(SharedString::from(format!("db-change-{key:x}")))
            .w_full()
            .h(px(LIST_H))
            .px(px(12.))
            .gap(px(14.))
            .border_t_1()
            .border_color(p.border_variant)
            .hover(move |s| s.bg(p.hover))
            .when(node.is_some(), |d| d.cursor_pointer())
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(160.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(it.name.clone()),
                    )
                    .children(it.path.clone().map(|path| {
                        div()
                            .text_size(px(11.))
                            .text_color(p.text_placeholder)
                            .truncate()
                            .child(path)
                    })),
            )
            .child(
                h_flex()
                    .w(px(220.))
                    .flex_shrink_0()
                    .gap(px(8.))
                    .text_size(px(13.))
                    .child(
                        div()
                            .max_w(px(100.))
                            .truncate()
                            .text_color(p.text_muted)
                            .child(stock),
                    )
                    .child(
                        Icon::new(IconName::ArrowRight)
                            .size(px(12.))
                            .text_color(p.text_placeholder),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(p.accent)
                            .child(new),
                    ),
            )
            .when_some(origin.filter(|_| self.content_width >= 1100.), |d, origin| {
                d.child(
                    div()
                        .w(px(170.))
                        .flex_shrink_0()
                        .text_size(px(12.))
                        .text_color(p.text_muted)
                        .truncate()
                        .child(origin),
                )
            })
            .child(
                Button::new(SharedString::from(format!("db-change-revert-{key:x}")))
                    .ghost()
                    .tap_xs()
                    .icon(IconName::Undo2)
                    .tooltip(tr("Revert"))
                    .disabled(self.db_locked())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.db_revert(key, cx);
                    })),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(n) = node {
                    this.db_show_node(n, cx);
                }
            }))
            .into_any_element()
    }

    /// `Row › Field` of the open page's cell holding `key`.
    pub(super) fn db_cell_name(&self, key: u64) -> Option<String> {
        let slot = self.db.view.as_ref()?;
        let module = self.db.module(&slot.module)?;
        slot.view.rows.iter().find_map(|r| {
            let i = r.cells.iter().position(|c| c.key == Some(key))?;
            let name = if r.name.is_empty() { &r.label } else { &r.name };
            Some(format!("{name} › {}", module.fields.get(i)?.label))
        })
    }

    /// The name a page gives the row labelled `label`, else the label in title case.
    pub(super) fn db_row_name(&self, label: &str) -> String {
        self.db
            .view
            .as_ref()
            .and_then(|v| v.view.rows.iter().find(|r| r.label == label))
            .map(|r| r.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| title_case(label))
    }

    pub(super) fn db_copy_change_row(
        &self,
        r: &RowCopy,
        it: &ChangeItem,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let from = match &r.from {
            Some(l) => trf!("copy of {name}", name = self.db_row_name(l)),
            None => tr("copy of the game default").to_owned(),
        };
        let (table, field, to) = (r.table.clone(), r.field.clone(), r.to.clone());
        h_flex()
            .id(SharedString::from(format!("db-change-row-{}", r.select())))
            .w_full()
            .h(px(LIST_H))
            .px(px(12.))
            .gap(px(14.))
            .border_t_1()
            .border_color(p.border_variant)
            .hover(move |s| s.bg(p.hover))
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(160.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(it.name.clone()),
                    )
                    .children(it.path.clone().map(|path| {
                        div()
                            .text_size(px(11.))
                            .text_color(p.text_placeholder)
                            .truncate()
                            .child(path)
                    })),
            )
            .child(
                h_flex()
                    .w(px(220.))
                    .flex_shrink_0()
                    .gap(px(8.))
                    .text_size(px(13.))
                    .child(div().text_color(p.text_muted).child(tr("none")))
                    .child(
                        Icon::new(IconName::ArrowRight)
                            .size(px(12.))
                            .text_color(p.text_placeholder),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(p.accent)
                            .child(from),
                    ),
            )
            .child(
                Button::new(SharedString::from(format!("db-change-unrow-{}", r.select())))
                    .ghost()
                    .tap_xs()
                    .icon(IconName::Undo2)
                    .tooltip(tr("Remove own row"))
                    .disabled(self.db_locked())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.db_remove_copy(&table, &field, &to, cx);
                    })),
            )
            .into_any_element()
    }
}

fn change_group_head(group: &str, i: usize, p: Palette) -> AnyElement {
    h_flex()
        .w_full()
        .h(px(LIST_H))
        .px(px(12.))
        .items_end()
        .pb(px(8.))
        .when(i > 0, |d| d.border_t_1().border_color(p.border_variant))
        .bg(p.surface)
        .child(label(group.to_uppercase(), p))
        .into_any_element()
}
