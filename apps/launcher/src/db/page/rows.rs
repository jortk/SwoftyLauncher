use std::ops::Range;

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::dbedit::{Cell, FieldSpec, Kind, Module, ModuleRow};
use swofty_i18n::{tr, trf, trn};

use super::{MOVE_W, NAME_W, ROW_H};
use crate::db::fields::{curve_rating, detail_shell, hex_rgb, is_on, list_items, swatch};
use crate::db::{Nav, ViewSlot, bool_text, clean, kind_of, swap_field};
use crate::shell::Shell;
use crate::theme::{Palette, ScrollArea, palette};

impl Shell {
    pub(super) fn db_rows(
        &mut self,
        module: &'static Module,
        range: Range<usize>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let p = palette(cx);
        let Some(slot) = self.db.view.as_ref() else {
            return Vec::new();
        };
        let view = slot.view.clone();
        let rows: Vec<usize> = range.filter_map(|i| slot.shown.get(i).copied()).collect();
        let columns: Vec<usize> = view
            .columns
            .iter()
            .take(self.db.max_cols)
            .map(|c| c.field)
            .collect();
        let moves = swap_field(module).is_some() && !self.db_locked();
        let total = view.rows.len();
        rows.into_iter()
            .enumerate()
            .map(|(n, ri)| {
                let r = &view.rows[ri];
                let selected = self.db.row.as_deref() == Some(r.label.as_str());
                let label = r.label.clone();
                let edited = r.cells.iter().any(|c| c.edited);
                let shared = r.cells.iter().any(|c| c.key.is_some_and(|k| self.db_shared(k)));
                h_flex()
                    .id(SharedString::from(format!("db-row-{}", r.label)))
                    .w_full()
                    .h(px(ROW_H))
                    .px(px(12.))
                    .gap(px(12.))
                    .when(n > 0 || ri > 0, |d| d.border_t_1().border_color(p.border_variant))
                    .cursor_pointer()
                    .map(|d| {
                        if selected {
                            d.bg(p.selected)
                        } else {
                            d.hover(move |s| s.bg(p.hover))
                        }
                    })
                    .child(row_name(r, ri, shared, edited, p))
                    .children(columns.iter().map(|&f| {
                        div()
                            .flex_1()
                            .flex_basis(px(0.))
                            .min_w_0()
                            .child(self.db_cell(module, ri, f, p, cx))
                    }))
                    .when(moves, |d| d.child(Self::db_row_moves(&label, ri, total, cx)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.db.row = (this.db.row.as_ref() != Some(&label)).then(|| label.clone());
                        this.db.cell = None;
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect()
    }

    fn db_row_moves(label: &str, ri: usize, total: usize, cx: &mut Context<Self>) -> Div {
        let (up, down) = (label.to_owned(), label.to_owned());

        h_flex()
            .w(px(MOVE_W))
            .flex_shrink_0()
            .justify_end()
            .child(
                Button::new(SharedString::from(format!("db-up-{ri}")))
                    .ghost()
                    .tap_xs()
                    .icon(IconName::ArrowUp)
                    .tooltip(tr("Move up"))
                    .disabled(ri == 0)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.db_move(&up, false, cx);
                    })),
            )
            .child(
                Button::new(SharedString::from(format!("db-down-{ri}")))
                    .ghost()
                    .tap_xs()
                    .icon(IconName::ArrowDown)
                    .tooltip(tr("Move down"))
                    .disabled(ri + 1 == total)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.db_move(&down, true, cx);
                    })),
            )
    }

    fn db_cell(
        &self,
        module: &'static Module,
        ri: usize,
        f: usize,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(slot) = self.db.view.as_ref() else {
            return div().into_any_element();
        };
        let (Some(cell), Some(spec)) = (slot.view.rows[ri].cells.get(f), module.fields.get(f)) else {
            return div().into_any_element();
        };
        let Some(key) = cell.key else {
            return div()
                .text_size(px(13.))
                .text_color(p.text_placeholder)
                .whitespace_nowrap()
                .truncate()
                .child(if spec.optional { tr("Default") } else { "—" })
                .into_any_element();
        };
        let kind = kind_of(spec.kind, cell.tag);
        let locked = spec.readonly || self.db_locked();
        let error = self.db.errors.contains_key(&key);
        let color = if error {
            p.error
        } else if cell.edited {
            p.accent
        } else {
            p.text
        };
        if let Some(edit) = self.db.cell.as_ref().filter(|c| c.key == key) {
            return Self::db_cell_input(key, edit.input.clone(), cx);
        }
        let choices = cell_choices(kind, spec, slot, f);
        let shown = cell_shown(kind, cell, &choices);
        let revert = (cell.edited && !self.db_locked()).then(|| Self::db_cell_revert(key, p, cx));
        let text = div()
            .min_w_0()
            .truncate()
            .whitespace_nowrap()
            .text_size(px(13.))
            .text_color(color)
            .child(shown.clone());
        let body: AnyElement = match kind {
            Kind::Bool => {
                let on = is_on(&cell.value);
                let (tag, stock) = (cell.tag, cell.stock.clone());
                div()
                    .id(SharedString::from(format!("db-cell-bool-{key:x}")))
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(
                        Switch::new(SharedString::from(format!("db-cell-switch-{key:x}")))
                            .checked(on)
                            .tap_sm()
                            .disabled(locked)
                            .on_change(cx.listener(move |this, on: &bool, _, cx| {
                                this.db_set(key, &bool_text(tag, &stock, *on), cx);
                            })),
                    )
                    .into_any_element()
            }
            Kind::Color => h_flex()
                .gap(px(8.))
                .min_w_0()
                .child(swatch(&cell.value, 14., p))
                .child(text)
                .into_any_element(),
            Kind::Choice | Kind::Ref if !locked && !choices.is_empty() => {
                Self::db_cell_choice(key, cell.value.clone(), choices, text, cx)
            }
            Kind::Int | Kind::Float | Kind::Text | Kind::Time | Kind::List | Kind::Vector if !locked => {
                let value = cell.value.clone();
                h_flex()
                    .id(SharedString::from(format!("db-cell-{key:x}")))
                    .min_w_0()
                    .h(px(26.))
                    .px(px(6.))
                    .mx(px(-6.))
                    .rounded(px(4.))
                    .cursor_text()
                    .hover(move |s| s.bg(p.hover))
                    .child(text)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.db_start_cell(key, value.clone(), window, cx);
                    }))
                    .into_any_element()
            }
            _ => text.into_any_element(),
        };
        h_flex()
            .gap(px(2.))
            .min_w_0()
            .child(div().min_w_0().child(body))
            .children(revert)
            .into_any_element()
    }

    fn db_cell_input(key: u64, input: Entity<InputState>, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id(SharedString::from(format!("db-cell-edit-{key:x}")))
            .w_full()
            .on_click(|_, _, cx| cx.stop_propagation())
            .capture_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "escape" {
                    this.db.cell = None;
                    cx.notify();
                }
            }))
            .child(Input::new(&input).tap_xs())
            .into_any_element()
    }

    fn db_cell_revert(key: u64, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id(SharedString::from(format!("db-cell-revert-{key:x}")))
            .flex_shrink_0()
            .p(px(3.))
            .rounded(px(4.))
            .cursor_pointer()
            .text_color(p.text_placeholder)
            .hover(move |s| s.bg(p.hover).text_color(p.text))
            .child(Icon::new(IconName::Undo2).size(px(12.)))
            .tooltip(move |window, cx| {
                gpui_kit::component::tooltip::Tooltip::new(tr("Revert to stock")).build(window, cx)
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.db_revert(key, cx);
            }))
    }

    fn db_cell_choice(
        key: u64,
        current: String,
        choices: Choices,
        text: Div,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let shell = cx.entity().downgrade();
        let pick = Button::new(SharedString::from(format!("db-cell-pick-{key:x}")))
            .ghost()
            .tap_xs()
            .icon(IconName::ChevronDown)
            .dropdown_menu_with_anchor(Anchor::TopLeft, move |mut menu, _, _| {
                menu = menu.max_h(px(320.)).scrollable(true).min_w(px(220.));
                for (v, n) in &choices {
                    let (shell, v2) = (shell.clone(), v.clone());
                    let t = if n.is_empty() { v.clone() } else { n.clone() };
                    menu = menu.item(PopupMenuItem::new(t).checked(*v == current).on_click(
                        move |_, _, cx| {
                            shell.update(cx, |s, cx| s.db_set(key, &v2, cx)).ok();
                        },
                    ));
                }
                menu
            });
        h_flex()
            .gap(px(2.))
            .min_w_0()
            .child(text)
            .child(
                div()
                    .flex_shrink_0()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(pick),
            )
            .into_any_element()
    }

    pub(super) fn db_row_detail(
        &self,
        module: &'static Module,
        row: &str,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(r) = self
            .db
            .view
            .as_ref()
            .and_then(|s| s.view.rows.iter().find(|r| r.label == row))
        else {
            return div().into_any_element();
        };
        let title = if r.name.is_empty() {
            r.label.clone()
        } else {
            r.name.clone()
        };
        let edited = r.cells.iter().filter(|c| c.edited).count();
        let subtitle = if edited > 0 {
            trf!("{label} · {edited} edited", label = r.label, edited = edited)
        } else {
            r.label.clone()
        };
        let fields = self.db_row_fields();
        let raw = r.node;
        detail_shell(
            "db-row-detail",
            title,
            subtitle,
            p,
            cx.listener(|this, _, _, cx| {
                this.db.row = None;
                cx.notify();
            }),
        )
        .child(
            v_flex()
                .flex_1()
                .min_h_0()
                .scroll_y(SharedString::from(format!("db-row-body-{}-{row}", module.id)))
                .px(px(16.))
                .py(px(14.))
                .gap(px(22.))
                .children(self.db_fields_panel(&fields, p, cx))
                .child(
                    h_flex()
                        .pt(px(4.))
                        .gap(px(6.))
                        .when(swap_field(module).is_some() && !self.db_locked(), |d| {
                            let (up, down) = (row.to_owned(), row.to_owned());
                            d.child(
                                Button::new("db-row-up")
                                    .outline()
                                    .tap_sm()
                                    .icon(IconName::ArrowUp)
                                    .label(tr("Move up"))
                                    .on_click(
                                        cx.listener(move |this, _, _, cx| this.db_move(&up, false, cx)),
                                    ),
                            )
                            .child(
                                Button::new("db-row-down")
                                    .outline()
                                    .tap_sm()
                                    .icon(IconName::ArrowDown)
                                    .label(tr("Move down"))
                                    .on_click(
                                        cx.listener(move |this, _, _, cx| this.db_move(&down, true, cx)),
                                    ),
                            )
                            .child(div().flex_1())
                        })
                        .child(
                            Button::new("db-row-raw")
                                .ghost()
                                .tap_sm()
                                .icon(IconName::FolderTree)
                                .label(tr("Show in All tables"))
                                .on_click(cx.listener(move |this, _, _, cx| this.db_go(Nav::Raw(raw), cx))),
                        ),
                ),
        )
        .into_any_element()
    }
}

type Choices = Vec<(String, String)>;

fn cell_choices(kind: Kind, spec: &FieldSpec, slot: &ViewSlot, field: usize) -> Choices {
    match kind {
        Kind::Choice => spec
            .options
            .iter()
            .map(|o| (o.value.clone(), o.label.clone()))
            .collect(),
        Kind::Ref | Kind::RefList => slot.choices.get(&field).cloned().unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn cell_shown(kind: Kind, cell: &Cell, choices: &Choices) -> String {
    match kind {
        Kind::Choice | Kind::Ref => choices
            .iter()
            .find(|(v, _)| *v == cell.value)
            .map(|(_, n)| n.clone())
            .filter(|n| !n.is_empty())
            .or_else(|| cell.text.as_deref().map(clean))
            .unwrap_or_else(|| cell.value.clone()),
        Kind::RefList => trn(list_items(&cell.value).len(), "{count} item", "{count} items"),
        Kind::Curve => curve_rating(&cell.value).unwrap_or_else(|| "curve".into()),
        Kind::Color => hex_rgb(&cell.value).unwrap_or_else(|| cell.value.clone()),
        _ => cell
            .text
            .as_deref()
            .map(clean)
            .unwrap_or_else(|| cell.value.clone()),
    }
}

fn row_name(r: &ModuleRow, ri: usize, shared: bool, edited: bool, p: Palette) -> Div {
    h_flex()
        .w(px(NAME_W))
        .flex_shrink_0()
        .gap(px(8.))
        .child(
            v_flex().flex_1().min_w_0().child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .child(if r.name.is_empty() {
                        r.label.clone()
                    } else {
                        r.name.clone()
                    }),
            ),
        )
        .when(shared, |d| {
            d.child(
                div()
                    .id(SharedString::from(format!("db-row-shared-{ri}")))
                    .size(px(7.))
                    .flex_shrink_0()
                    .rounded_full()
                    .border_1()
                    .border_color(p.warning)
                    .tooltip(|window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(tr(
                            "Another DB Editor addon on in this profile changes this row",
                        ))
                        .build(window, cx)
                    }),
            )
        })
        .when(edited, |d| {
            d.child(div().size(px(6.)).flex_shrink_0().rounded_full().bg(p.accent))
        })
}
