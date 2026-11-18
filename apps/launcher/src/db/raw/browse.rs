use std::ops::Range;
use std::sync::Arc;

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::Input;
use gpui_kit::component::{Disableable, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::dbedit::{Catalog, Node, NodeId, NodeKind};
use swofty_i18n::{tr, trf, trn};

use super::{Nav, hex, type_name};
use crate::shell::Shell;
use crate::theme::{Palette, palette, scrollbar_x};

use super::*;

impl Shell {
    pub(super) fn db_crumbs(&self, node: NodeId, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(catalog) = self.db.catalog().cloned() else {
            return h_flex();
        };
        let mut chain = vec![node];
        let mut at = catalog.node(node).parent;
        while let Some(id) = at {
            chain.push(id);
            at = catalog.node(id).parent;
        }
        chain.reverse();
        let last = chain.len() - 1;
        h_flex()
            .flex_1()
            .min_w_0()
            .gap(px(4.))
            .flex_wrap()
            .children(chain.into_iter().enumerate().map(|(i, id)| {
                let name = if id == catalog.root() {
                    tr("All tables").to_owned()
                } else {
                    catalog.node(id).name.clone()
                };
                h_flex()
                    .gap(px(4.))
                    .when(i > 0, |d| {
                        d.child(
                            Icon::new(IconName::ChevronRight)
                                .size(px(12.))
                                .text_color(p.text_placeholder),
                        )
                    })
                    .child(
                        div()
                            .id(SharedString::from(format!("db-crumb-{i}")))
                            .px(px(6.))
                            .py(px(2.))
                            .rounded(px(4.))
                            .text_size(px(13.))
                            .map(|d| {
                                if i == last {
                                    d.font_weight(FontWeight::SEMIBOLD).text_color(p.text)
                                } else {
                                    d.cursor_pointer()
                                        .text_color(p.text_muted)
                                        .hover(move |s| s.bg(p.hover).text_color(p.text))
                                }
                            })
                            .child(name)
                            .on_click(cx.listener(move |this, _, _, cx| this.db_go(Nav::Raw(id), cx))),
                    )
            }))
    }

    pub(crate) fn db_raw(&mut self, node: NodeId, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(catalog) = self.db.catalog().cloned() else {
            return div().into_any_element();
        };
        let n = catalog.node(node).clone();
        let kids = catalog.children(node).len();
        let rows = self.db_raw_rows(node, &catalog);
        let filtered = rows.len() != kids;
        let summary = node_summary(&catalog, node, &n, kids);
        let add = Button::new("db-add-key")
            .outline()
            .tap_sm()
            .icon(IconName::Plus)
            .label(tr("Add key…"))
            .disabled(self.db_locked())
            .on_click(cx.listener(move |this, _, window, cx| this.db_add_dialog(node, window, cx)));
        let content = match n.kind {
            _ if rows.is_empty() && filtered => self.empty(
                IconName::Search,
                tr("No matches"),
                tr("Nothing at this level matches the filter."),
                div(),
                p,
            ),
            NodeKind::Table { .. } => self.db_grid(node, rows.clone(), &catalog, p, cx),
            _ if kids == 0 => self.empty(
                IconName::Folder,
                tr("Empty"),
                tr("Nothing is stored under this part."),
                div(),
                p,
            ),
            _ => self.db_children_list(rows.clone(), p, cx),
        };
        let detail = self.db.key.map(|k| self.db_key_detail(k, p, cx));
        v_flex()
            .size_full()
            .gap(px(12.))
            .child(
                h_flex()
                    .gap(px(12.))
                    .flex_shrink_0()
                    .child(self.db_crumbs(node, p, cx))
                    .children(self.db.raw_filter.as_ref().map(|d| {
                        div().w(px(240.)).flex_shrink_0().child(
                            Input::new(&d.input).tap_sm().cleanable(true).prefix(
                                Icon::new(IconName::ListFilter)
                                    .size(px(14.))
                                    .text_color(p.text_placeholder),
                            ),
                        )
                    }))
                    .child(add),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(p.text_muted)
                    .truncate()
                    .child(summary),
            )
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

    fn db_children_list(&mut self, rows: Arc<Vec<NodeId>>, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let shown = rows.clone();
        let list = uniform_list(
            "db-children",
            rows.len(),
            cx.processor(move |this, range: Range<usize>, _, cx| this.db_children(&shown, range, cx)),
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
    }

    pub(super) fn db_raw_rows(&mut self, node: NodeId, catalog: &Arc<Catalog>) -> Arc<Vec<NodeId>> {
        let query = self
            .db
            .raw_filter
            .as_ref()
            .map(|d| d.query.trim().to_lowercase())
            .unwrap_or_default();
        if let Some((n, q, rows)) = &self.db.raw_rows
            && *n == node
            && *q == query
        {
            return rows.clone();
        }
        let rows: Vec<NodeId> = catalog
            .children(node)
            .iter()
            .copied()
            .filter(|&c| {
                let n = catalog.node(c);
                query.is_empty()
                    || n.name.to_lowercase().contains(&query)
                    || n.label
                        .as_ref()
                        .is_some_and(|l| l.to_lowercase().contains(&query))
                    || n.display
                        .as_ref()
                        .is_some_and(|d| d.to_lowercase().contains(&query))
            })
            .collect();
        let rows = Arc::new(rows);
        self.db.raw_rows = Some((node, query, rows.clone()));
        rows
    }

    pub(super) fn db_children(
        &mut self,
        kids: &[NodeId],
        range: Range<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let p = palette(cx);
        let Some(catalog) = self.db.catalog().cloned() else {
            return Vec::new();
        };
        range
            .filter_map(|i| kids.get(i).map(|&c| (i, c)))
            .map(|(i, child)| self.db_entry(&catalog, i, child, true, p, cx))
            .collect()
    }

    pub(super) fn db_entry(
        &self,
        catalog: &Arc<Catalog>,
        i: usize,
        child: NodeId,
        named: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let n = catalog.node(child);
        let selected = self.db.key == Some(child);
        let mut value = None;
        let mut edited = false;
        let detail = match &n.kind {
            NodeKind::Table { rows, .. } => {
                let mut s = trf!(
                    "Table · {rows}",
                    rows = trn(*rows as usize, "{count} row", "{count} rows")
                );
                if let Some(m) = catalog.table_meaning(child).filter(|m| !m.is_empty()) {
                    s.push_str(&format!(" · {m}"));
                }
                s
            }
            NodeKind::Row { .. } => match (&n.display, &n.label) {
                (Some(d), Some(l)) => format!("{d} · {l}"),
                (Some(d), None) => d.clone(),
                (None, Some(l)) => l.clone(),
                (None, None) => trn(catalog.children(child).len(), "{count} entry", "{count} entries"),
            },
            NodeKind::Key => {
                let r = n.record.map(|r| catalog.record(r));
                let key = r.map(|r| r.key).unwrap_or_default();
                edited = self.db.edit(key).is_some();
                value = Some(self.db.current(key));
                let meaning = catalog.meta(key).map(|m| m.meaning.clone()).unwrap_or_default();
                let tag = r.map_or(String::new(), |r| type_name(r.tag));
                if meaning.is_empty() {
                    tag
                } else {
                    format!("{tag} · {meaning}")
                }
            }
            NodeKind::Unnamed => tr("Records no known name reaches").into(),
            _ => trn(catalog.children(child).len(), "{count} entry", "{count} entries"),
        };
        let title = if named {
            n.name.clone()
        } else {
            n.record
                .map_or_else(|| n.name.clone(), |r| hex(catalog.record(r).key))
        };
        let is_key = n.kind == NodeKind::Key;
        h_flex()
            .id(SharedString::from(format!("db-entry-{child}")))
            .w_full()
            .h(px(LIST_H))
            .px(px(12.))
            .gap(px(12.))
            .when(i > 0, |d| d.border_t_1().border_color(p.border_variant))
            .cursor_pointer()
            .map(|d| {
                if selected {
                    d.bg(p.selected)
                } else {
                    d.hover(move |s| s.bg(p.hover))
                }
            })
            .child(icon_tile(kind_icon(&n.kind), p))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.text_muted)
                            .truncate()
                            .child(detail),
                    ),
            )
            .children(value.map(|v| {
                div()
                    .w(px(240.))
                    .flex_shrink_0()
                    .flex()
                    .justify_end()
                    .text_size(px(13.))
                    .text_color(if edited { p.accent } else { p.text })
                    .child(
                        div()
                            .truncate()
                            .child(if v.is_empty() { "empty".to_owned() } else { v }),
                    )
            }))
            .when(!is_key, |d| {
                d.child(
                    Icon::new(IconName::ChevronRight)
                        .size(px(14.))
                        .text_color(p.text_placeholder),
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                if is_key {
                    this.db.key = (this.db.key != Some(child)).then_some(child);
                    cx.notify();
                } else {
                    this.db_show_node(child, cx);
                }
            }))
            .into_any_element()
    }

    pub(super) fn db_grid(
        &self,
        table: NodeId,
        shown: Arc<Vec<NodeId>>,
        catalog: &Arc<Catalog>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fields = Arc::new(catalog.fields(table));
        let rows = shown.len();
        let width = GRID_LABEL + GRID_COL * fields.len() as f32 + 24.;
        let header = h_flex()
            .h(px(GRID_HEADER))
            .px(px(12.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(p.border_variant)
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.text_placeholder)
            .child(div().w(px(GRID_LABEL)).flex_shrink_0().child(tr("ROW")))
            .children(fields.iter().map(|f| {
                div()
                    .w(px(GRID_COL))
                    .flex_shrink_0()
                    .pr(px(10.))
                    .truncate()
                    .child(f.clone())
            }));
        let cols = fields.clone();
        let list = uniform_list(
            "db-grid",
            rows,
            cx.processor(move |this, range: Range<usize>, _, cx| this.db_grid_rows(&shown, &cols, range, cx)),
        )
        .flex_1()
        .track_scroll(&self.db.list_scroll);
        div()
            .relative()
            .size_full()
            .rounded(px(6.))
            .border_1()
            .border_color(p.border_variant)
            .overflow_hidden()
            .child(
                div()
                    .id("db-grid-scroll")
                    .size_full()
                    .flex()
                    .overflow_x_scroll()
                    .restrict_scroll_to_axis()
                    .track_scroll(&self.db.grid_scroll)
                    .child(
                        v_flex()
                            .flex_none()
                            .w(px(width))
                            .h_full()
                            .child(header)
                            .child(list),
                    ),
            )
            .child(
                div().absolute().inset_0().child(
                    scrollbar_x(&self.db.grid_scroll)
                        .id("db-grid-scrollbar")
                        .viewport_from_layout(),
                ),
            )
            .child(
                div()
                    .absolute()
                    .top(px(GRID_HEADER))
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .child(self.db.list_motion.overlay(&self.db.list_scroll, cx)),
            )
            .into_any_element()
    }

    pub(super) fn db_grid_rows(
        &mut self,
        rows: &[NodeId],
        fields: &[String],
        range: Range<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let p = palette(cx);
        let Some(catalog) = self.db.catalog().cloned() else {
            return Vec::new();
        };
        range
            .filter_map(|i| rows.get(i).map(|&r| (i, r)))
            .map(|(i, row)| {
                let n = catalog.node(row);
                let title = n
                    .display
                    .clone()
                    .or_else(|| n.label.clone())
                    .unwrap_or_else(|| n.name.clone());
                h_flex()
                    .id(SharedString::from(format!("db-grid-row-{row}")))
                    .w_full()
                    .h(px(36.))
                    .px(px(12.))
                    .when(i > 0, |d| d.border_t_1().border_color(p.border_variant))
                    .hover(move |s| s.bg(p.hover))
                    .child(
                        div()
                            .id(SharedString::from(format!("db-grid-open-{row}")))
                            .w(px(GRID_LABEL))
                            .flex_shrink_0()
                            .pr(px(10.))
                            .cursor_pointer()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(title)
                            .on_click(cx.listener(move |this, _, _, cx| this.db_go(Nav::Raw(row), cx))),
                    )
                    .children(fields.iter().map(|f| {
                        let cell = catalog.find_in(row, f);
                        let key = cell
                            .and_then(|c| catalog.node(c).record)
                            .map(|r| catalog.record(r).key);
                        let (text, color) = match key {
                            Some(k) => (
                                self.db.current(k),
                                if self.db.errors.contains_key(&k) {
                                    p.error
                                } else if self.db.edit(k).is_some() {
                                    p.accent
                                } else {
                                    p.text
                                },
                            ),
                            None => ("—".to_owned(), p.text_placeholder),
                        };
                        let selected = cell.is_some() && self.db.key == cell;
                        div()
                            .id(SharedString::from(format!("db-grid-{row}-{f}")))
                            .w(px(GRID_COL))
                            .flex_shrink_0()
                            .h_full()
                            .flex()
                            .items_center()
                            .pr(px(10.))
                            .text_size(px(13.))
                            .text_color(color)
                            .when(selected, |d| d.bg(p.selected))
                            .when(cell.is_some(), |d| d.cursor_pointer())
                            .child(div().truncate().child(text))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(c) = cell {
                                    this.db.key = (this.db.key != Some(c)).then_some(c);
                                    cx.notify();
                                }
                            }))
                    }))
                    .into_any_element()
            })
            .collect()
    }
}

fn node_summary(catalog: &Catalog, node: NodeId, n: &Node, kids: usize) -> String {
    match &n.kind {
        NodeKind::Root => trf!(
            "{records} records · {named} named · {parts}",
            records = catalog.records(),
            named = catalog.named(),
            parts = trn(kids, "{count} top-level part", "{count} top-level parts")
        ),
        NodeKind::Table { rows, .. } => {
            let fields = catalog.fields(node).len();
            let meaning = catalog.table_meaning(node).unwrap_or_default();
            let mut s = trf!(
                "Table · {rows} × {fields}",
                rows = trn(*rows as usize, "{count} row", "{count} rows"),
                fields = trn(fields, "{count} field", "{count} fields")
            );
            if !meaning.is_empty() {
                s.push_str(&format!(" · {meaning}"));
            }
            s
        }
        NodeKind::Row { index } => {
            let mut s = trf!("Row {index}", index = index);
            if let Some(l) = &n.label {
                s.push_str(&format!(" · {l}"));
            }
            if let Some(d) = &n.display {
                s.push_str(&format!(" · {d}"));
            }
            format!("{s} · {}", trn(kids, "{count} entry", "{count} entries"))
        }
        _ => trn(kids, "{count} entry", "{count} entries"),
    }
}
