use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::view::{Align, Cell, Field, Search, Table, TableRow};
use swofty_i18n::tr;

use super::cell::*;
use super::state::*;
use super::widgets::*;
use super::*;
use crate::shell::Shell;
use crate::theme::Palette;

pub(super) fn column_drag_width(start_width: f32, start_x: f32, current_x: f32) -> f32 {
    (start_width + (current_x - start_x)).max(COLUMN_MIN_W)
}

pub(super) fn resolve_column_widths(
    available: f32,
    defaults: &[Option<f32>],
    custom: &[Option<f32>],
) -> Vec<Option<f32>> {
    if custom.iter().all(Option::is_none) {
        return defaults.to_vec();
    }
    let mut widths: Vec<Option<f32>> = custom
        .iter()
        .zip(defaults)
        .map(|(c, d)| c.or(*d).map(|w| w.max(COLUMN_MIN_W)))
        .collect();
    if available <= 0. {
        return widths;
    }
    let flex_count = widths.iter().filter(|w| w.is_none()).count();
    let budget = (available - flex_count as f32 * COLUMN_MIN_W).max(0.);
    let explicit_sum: f32 = widths.iter().flatten().sum();
    if explicit_sum > budget {
        let mut excess = explicit_sum - budget;
        // Shrink un-dragged fixed columns first so user-resized columns keep their target width.
        let slack: f32 = widths
            .iter()
            .enumerate()
            .filter(|(i, _)| custom.get(*i).copied().flatten().is_none())
            .filter_map(|(_, w)| w.map(|v| (v - COLUMN_MIN_W).max(0.)))
            .sum();
        if slack > 0. {
            let ratio = (excess / slack).min(1.);
            for (i, w) in widths.iter_mut().enumerate() {
                if custom.get(i).copied().flatten().is_none()
                    && let Some(v) = w
                {
                    let cut = (*v - COLUMN_MIN_W).max(0.) * ratio;
                    *v -= cut;
                    excess -= cut;
                }
            }
        }
        if excess > 0.5 {
            let rem_slack: f32 = widths
                .iter()
                .filter_map(|w| w.map(|v| (v - COLUMN_MIN_W).max(0.)))
                .sum();
            if rem_slack > 0. {
                let ratio = (excess / rem_slack).min(1.);
                for v in widths.iter_mut().flatten() {
                    *v -= (*v - COLUMN_MIN_W).max(0.) * ratio;
                }
            }
        }
    } else if flex_count == 0 && explicit_sum < available {
        let extra = available - explicit_sum;
        let idx = (0..widths.len())
            .rev()
            .find(|&i| custom.get(i).copied().flatten().is_none())
            .or_else(|| widths.len().checked_sub(1));
        if let Some(i) = idx
            && let Some(v) = &mut widths[i]
        {
            *v += extra;
        }
    }
    widths
}

impl Shell {
    pub(super) fn table(&self, at: At, t: &Table, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let table_key = at.key(&format!("table:{}", t.id()));
        let sort = self
            .blocks
            .sorts
            .get(&table_key)
            .cloned()
            .or_else(|| t.sort.as_ref().map(|s| (s.column.clone(), s.desc)));
        let search_key = t.search.as_ref().map(|s| at.key(&search_id(s, t.id())));
        let query = search_key
            .as_deref()
            .filter(|_| t.search.as_ref().is_some_and(|s| s.filter))
            .and_then(|k| self.query(k));
        let order: Vec<usize> = sorted_rows(t, sort.as_ref())
            .into_iter()
            .filter(|i| query.is_none_or(|q| Search::matches(q, t.rows[*i].cells().iter().map(Cell::text))))
            .collect();
        let (base_h, text) = if t.compact { (30., 12.) } else { (36., 13.) };
        let row_h = t.row_height.map_or(base_h, |h| h.max(base_h));
        // The row whose detail is open, while it is still shown.
        let selected = self.blocks.table_rows.get(&table_key).and_then(|id| {
            order
                .iter()
                .map(|&i| (i, &t.rows[i]))
                .find(|(i, r)| r.id(*i) == *id && !r.detail().is_empty())
        });
        let selected_id = selected.map(|(i, r)| r.id(i));
        let specs: Vec<_> = t.columns.iter().map(|c| c.spec()).collect();
        let defaults: Vec<Option<f32>> = specs.iter().map(|c| c.width).collect();
        let custom: Vec<Option<f32>> = t
            .columns
            .iter()
            .map(|c| {
                self.blocks
                    .column_widths
                    .get(&(table_key.clone(), c.key().to_owned()))
                    .copied()
            })
            .collect();
        // The open row's detail goes beside the table when every column still fits, else under
        // its row, where it was clicked.
        let needed = 24.
            + custom
                .iter()
                .zip(&defaults)
                .map(|(c, d)| c.or(*d).unwrap_or(DETAIL_COLUMN_MIN))
                .sum::<f32>()
            + 12. * specs.len().saturating_sub(1) as f32;
        let beside = selected.is_some() && at.width - DETAIL_W - GAP >= needed;
        let table_w = if beside {
            at.width - DETAIL_W - GAP
        } else {
            at.width
        };
        let avail = (table_w - 24. - 12. * specs.len().saturating_sub(1) as f32).max(0.);
        let widths = resolve_column_widths(avail, &defaults, &custom);
        let sized = |d: Div, i: usize| -> Div {
            let spec = &specs[i];
            let d = match widths[i] {
                Some(w) => d.w(px(w)).flex_shrink_0(),
                None => d.flex_1().flex_basis(px(0.)).min_w_0(),
            };
            match spec.align {
                Align::Left => d.justify_start(),
                Align::Right => d.justify_end(),
                Align::Center => d.justify_center(),
            }
        };
        let header = h_flex()
            .h(px(base_h - 2.))
            .px(px(12.))
            .gap(px(12.))
            .border_b_1()
            .border_color(p.border_variant)
            .children(t.columns.iter().enumerate().map(|(i, c)| {
                let spec = &specs[i];
                let column = c.key().to_owned();
                let active = sort.as_ref().filter(|(k, _)| *k == column).map(|(_, d)| *d);
                // A pencil for cells edited as text; a switch shows it can be flipped.
                let editable = t.rows.iter().any(|r| {
                    r.cells().get(i).is_some_and(|c| {
                        matches!(c, Cell::Spec(s) if s.edit.as_ref().is_some_and(|e| !matches!(e, Field::Toggle(_))))
                    })
                });
                let head = h_flex()
                    .id(SharedString::from(format!("{table_key}/h/{i}")))
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .pr(px(10.))
                    .gap(px(4.))
                    .map(|d| match spec.align {
                        Align::Left => d.justify_start(),
                        Align::Right => d.justify_end(),
                        Align::Center => d.justify_center()})
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if active.is_some() {
                        p.text
                    } else {
                        p.text_placeholder
                    })
                    .child(div().truncate().child(caps(&spec.title)))
                    .when(editable, |d| {
                        d.child(
                            Icon::new(IconName::Pencil)
                                .size(px(10.))
                                .text_color(p.text_placeholder),
                        )
                    })
                    .when_some(active, |d, desc| {
                        d.child(
                            Icon::new(if desc {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronUp
                            })
                            .size(px(12.))
                            .text_color(p.accent),
                        )
                    });
                let head = if spec.sort {
                    let key = table_key.clone();
                    let col = column.clone();
                    head.cursor_pointer()
                        .hover(move |s| s.text_color(p.text))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let desc = active.is_some_and(|d| !d);
                            this.blocks.sorts.insert(key.clone(), (col.clone(), desc));
                            cx.notify();
                        }))
                        .into_any_element()
                } else {
                    head.into_any_element()
                };
                let dragging = self
                    .blocks.column_drag
                    .as_ref()
                    .is_some_and(|d| d.table == table_key && d.column == column);
                let (r_table, r_col) = (table_key.clone(), column.clone());
                let fallback_w = widths[i].unwrap_or(DETAIL_COLUMN_MIN);
                let handle = div()
                    .id(SharedString::from(format!("{table_key}/resize/{i}")))
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(px(10.))
                    .flex()
                    .items_center()
                    .justify_end()
                    .cursor_col_resize()
                    .border_r_2()
                    .border_color(if dragging {
                        p.accent
                    } else {
                        p.border_variant.opacity(0.65)
                    })
                    .hover(move |s| s.border_color(p.accent).bg(p.accent.opacity(0.12)))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            let key = (r_table.clone(), r_col.clone());
                            if ev.click_count >= 2 {
                                this.blocks.column_drag = None;
                                this.blocks.column_widths.remove(&key);
                                cx.notify();
                                return;
                            }
                            let start_width = this
                                .blocks.column_widths
                                .get(&key)
                                .copied()
                                .or_else(|| this.blocks.column_bounds.borrow().get(&key).copied())
                                .unwrap_or(fallback_w)
                                .max(COLUMN_MIN_W);
                            this.blocks.column_drag = Some(ColumnDrag {
                                table: r_table.clone(),
                                column: r_col.clone(),
                                start_x: f32::from(ev.position.x),
                                start_width});
                            this.blocks.column_widths.insert(key, start_width);
                            cx.notify();
                        }),
                    );
                sized(h_flex(), i)
                    .relative()
                    .h_full()
                    .child(self.column_width_tracker(&table_key, &column))
                    .child(head)
                    .child(handle)
                    .into_any_element()
            }));
        // A long table draws the rows in and near the page's view and leaves room for the rest.
        let shown = self.visible_rows(at, &table_key, order.len(), row_h, selected.is_some() && !beside);
        let (above, below) = (shown.start, order.len() - shown.end);
        let rows: Vec<AnyElement> = order
            .iter()
            .enumerate()
            .skip(shown.start)
            .take(shown.len())
            .flat_map(|(n, &ri)| {
                let row = &t.rows[ri];
                let row_id = row.id(ri);
                let row_tone = row.tone();
                let opens = !row.detail().is_empty();
                let open = selected_id.as_deref() == Some(row_id.as_str());
                let (key, id) = (table_key.clone(), row_id.clone());
                let line = h_flex()
                    .id(SharedString::from(format!("{table_key}/r/{row_id}")))
                    .h(px(row_h))
                    .px(px(12.))
                    .gap(px(12.))
                    .when(n > 0, |d| d.border_t_1().border_color(p.border_variant))
                    .when(open, |d| d.bg(p.selected))
                    .when(opens && !open, |d| {
                        d.cursor_pointer().hover(move |s| s.bg(p.hover))
                    })
                    .when(opens, |d| {
                        // A click opens the row's detail, or closes it when it is open.
                        d.on_click(cx.listener(move |this, _, _, cx| {
                            if this.blocks.table_rows.get(&key) == Some(&id) {
                                this.blocks.table_rows.remove(&key);
                            } else {
                                this.blocks.table_rows.insert(key.clone(), id.clone());
                            }
                            cx.notify();
                        }))
                    })
                    .children(
                        row.cells()
                            .iter()
                            .take(specs.len())
                            .enumerate()
                            .map(|(ci, cell)| {
                                let ctx = CellCtx {
                                    at,
                                    table: t,
                                    table_key: &table_key,
                                    row_id: &row_id,
                                    row_tone,
                                    size: text,
                                    row_h,
                                };
                                let content = self.cell(ctx, &specs[ci], t.columns[ci].key(), cell, p, cx);
                                sized(h_flex(), ci).child(content).into_any_element()
                            }),
                    )
                    .into_any_element();
                let under = (open && !beside).then(|| self.row_detail(at, &table_key, row, None, p, cx));
                std::iter::once(line).chain(under)
            })
            .collect();
        let empty = order.is_empty().then(|| {
            div()
                .px(px(12.))
                .py(px(14.))
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(t.empty.clone().unwrap_or_else(|| tr("No matches").into()))
        });
        let body = v_flex()
            .w_full()
            .overflow_hidden()
            .when(!at.nested, |d| {
                d.rounded(px(6.)).border_1().border_color(p.border_variant)
            })
            .child(header)
            .child(
                v_flex()
                    .w_full()
                    .relative()
                    .child(self.rows_tracker(at, &table_key))
                    .when(above > 0, |d| {
                        d.child(div().flex_shrink_0().h(px(above as f32 * row_h)))
                    })
                    .children(rows)
                    .when(below > 0, |d| {
                        d.child(div().flex_shrink_0().h(px(below as f32 * row_h)))
                    }),
            )
            .children(empty);
        let detail = selected
            .filter(|_| beside)
            .map(|(_, r)| self.row_detail(at, &table_key, r, Some(DETAIL_W), p, cx));
        let table = match detail {
            Some(detail) => h_flex()
                .items_start()
                .gap(px(GAP))
                .child(div().flex_1().min_w_0().child(body))
                .child(detail)
                .into_any_element(),
            None => body.into_any_element(),
        };
        v_flex()
            .gap(px(10.))
            .children(search_key.and_then(|k| self.search_box(&k, p)))
            .child(table)
            .children(self.column_drag_listener(&table_key, cx))
            .into_any_element()
    }

    pub(super) fn column_width_tracker(&self, table_key: &str, column: &str) -> impl IntoElement {
        let bounds = self.blocks.column_bounds.clone();
        let key = (table_key.to_owned(), column.to_owned());
        canvas(
            move |b, _, _| {
                let w = f32::from(b.size.width);
                if w > 0. {
                    bounds.borrow_mut().insert(key.clone(), w);
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    }

    pub(super) fn column_drag_listener(
        &self,
        table_key: &str,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let drag = self
            .blocks
            .column_drag
            .as_ref()
            .filter(|d| d.table == table_key)?
            .clone();
        let this = cx.entity().downgrade();
        Some(
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let moved = this.clone();
                    let drag_move = drag.clone();
                    window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                        if phase != DispatchPhase::Bubble {
                            return;
                        }
                        let d = drag_move.clone();
                        moved
                            .update(cx, |this, cx| match ev.pressed_button {
                                Some(MouseButton::Left) => {
                                    let width =
                                        column_drag_width(d.start_width, d.start_x, f32::from(ev.position.x));
                                    this.blocks.column_widths.insert((d.table, d.column), width);
                                    cx.notify();
                                }
                                _ => {
                                    this.blocks.column_drag = None;
                                    cx.notify();
                                }
                            })
                            .ok();
                    });
                    let released = this.clone();
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble {
                            released
                                .update(cx, |this, cx| {
                                    this.blocks.column_drag = None;
                                    cx.notify();
                                })
                                .ok();
                        }
                    });
                },
            )
            .absolute()
            .size_0(),
        )
    }

    /// The rows of `n` worth drawing: those within the page's view, plus a few either side, for
    /// a long table on a scrolled page whose rows are all `row_h` tall (`varied` when an open
    /// row's detail sits under it). Every row on the first frame, before the rows' place is known.
    pub(super) fn visible_rows(
        &self,
        at: At,
        key: &str,
        n: usize,
        row_h: f32,
        varied: bool,
    ) -> std::ops::Range<usize> {
        const OVERSCAN: usize = 8;
        if varied || n <= LONG_TABLE {
            return 0..n;
        }
        let Some((scroll, _)) = self
            .blocks
            .section_scrolls
            .get(&format!("{}/{}", at.addon, at.section))
        else {
            return 0..n;
        };
        let Some(top) = self.blocks.table_tops.borrow().get(key).copied() else {
            return 0..n;
        };
        let view_h = f32::from(scroll.bounds().size.height);
        if view_h <= 0. {
            return 0..n;
        }
        // The view's top, from the rows' top.
        let from = -f32::from(scroll.offset().y) - top;
        let first = ((from / row_h).floor().max(0.) as usize)
            .saturating_sub(OVERSCAN)
            .min(n);
        let last = (((from + view_h) / row_h).ceil().max(0.) as usize + OVERSCAN).min(n);
        first..last.max(first)
    }

    /// An empty element over a table's rows that notes where they start in the page's scrolled
    /// content, for `visible_rows` on the next frame.
    pub(super) fn rows_tracker(&self, at: At, key: &str) -> impl IntoElement {
        let tops = self.blocks.table_tops.clone();
        let scroll = self
            .blocks
            .section_scrolls
            .get(&format!("{}/{}", at.addon, at.section))
            .map(|(s, _)| s.clone());
        let key = key.to_owned();
        canvas(
            move |bounds, _, _| {
                if let Some(scroll) = scroll {
                    let top = bounds.origin.y - scroll.bounds().origin.y - scroll.offset().y;
                    tops.borrow_mut().insert(key, f32::from(top));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    }

    /// A table row's detail panel: its title and subtitle, a close button and its blocks. A
    /// `width` puts it beside the table; without one it fills the table under its row.
    pub(super) fn row_detail(
        &self,
        at: At,
        table_key: &str,
        row: &TableRow,
        width: Option<f32>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = table_key.to_owned();
        let inner = At {
            width: width.unwrap_or(at.width) - 34.,
            nested: false,
            fill: false,
            ..at
        };
        v_flex()
            .map(|d| match width {
                Some(w) => d
                    .w(px(w))
                    .flex_shrink_0()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(p.border_variant),
                None => d.w_full().border_t_1().border_color(p.border_variant),
            })
            .bg(p.surface)
            .overflow_hidden()
            .child(
                h_flex()
                    .px(px(16.))
                    .py(px(12.))
                    .gap(px(10.))
                    .border_b_1()
                    .border_color(p.border_variant)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(15.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .truncate()
                                    .child(row.title()),
                            )
                            .when_some(row.subtitle().map(str::to_owned), |d, s| {
                                d.child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(p.text_muted)
                                        .truncate()
                                        .child(s),
                                )
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("{table_key}/close")))
                            .ghost()
                            .tap_sm()
                            .icon(IconName::X)
                            .tooltip(tr("Close"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.blocks.table_rows.remove(&key);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .px(px(16.))
                    .py(px(14.))
                    .gap(px(16.))
                    .children(row.detail().iter().map(|b| self.block_at(inner, b, p, cx))),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{COLUMN_MIN_W, column_drag_width, resolve_column_widths};

    #[test]
    fn dragging_a_column_clamps_at_the_minimum_width() {
        assert_eq!(column_drag_width(120., 300., 360.), 180.);
        assert_eq!(column_drag_width(120., 300., 100.), COLUMN_MIN_W);
    }

    #[test]
    fn untouched_columns_keep_their_declared_spec_widths() {
        let defaults = [Some(195.), None, Some(112.), None, Some(240.)];
        let custom = [None; 5];
        assert_eq!(resolve_column_widths(800., &defaults, &custom), defaults);
    }

    #[test]
    fn widening_a_flex_column_trims_unresized_fixed_columns_before_overflowing() {
        let defaults = [
            Some(195.),
            None,
            Some(112.),
            Some(84.),
            None,
            Some(180.),
            Some(240.),
        ];
        let custom = [None, Some(160.), None, None, None, None, None];
        let resolved = resolve_column_widths(850., &defaults, &custom);
        assert_eq!(resolved[1], Some(160.));
        assert_eq!(resolved[4], None);
        let explicit_sum: f32 = resolved.iter().flatten().sum();
        assert!(
            (explicit_sum - (850. - COLUMN_MIN_W)).abs() < 0.01,
            "{resolved:?}"
        );
        assert!(resolved.iter().flatten().all(|&w| w >= COLUMN_MIN_W));
    }

    #[test]
    fn narrowing_all_explicit_columns_fills_remaining_table_width() {
        let defaults = [Some(200.), Some(200.)];
        let custom = [Some(120.), None];
        let resolved = resolve_column_widths(400., &defaults, &custom);
        assert_eq!(resolved, vec![Some(120.), Some(280.)]);
    }
}
