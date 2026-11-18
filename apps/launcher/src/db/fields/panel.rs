use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::select::Select;
use gpui_kit::component::{Disableable, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::dbedit::{Backing, Kind};
use swofty_i18n::{tr, trf};

use super::FieldModel;
use super::detail::shown_value;
use crate::pages::label;
use crate::shell::Shell;
use crate::theme::Palette;
use crate::ui::Callout;

impl Shell {
    pub(crate) fn db_fields_panel(
        &self,
        fields: &[FieldModel],
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut sections: Vec<(Option<String>, Vec<&FieldModel>)> = Vec::new();
        for f in fields {
            match sections.iter_mut().find(|(s, _)| *s == f.section) {
                Some((_, list)) => list.push(f),
                None => sections.push((f.section.clone(), vec![f])),
            }
        }
        let mut shown: Vec<usize> = Vec::new();
        sections
            .into_iter()
            .map(|(title, list)| {
                let strip = list
                    .iter()
                    .filter_map(|f| f.row.as_ref())
                    .find(|(spec, b)| *b != Backing::Stock && !shown.contains(spec))
                    .cloned();
                if let Some((spec, _)) = &strip {
                    shown.push(*spec);
                }
                v_flex()
                    .flex_shrink_0()
                    .gap(px(14.))
                    .when_some(title, |d, t| d.child(label(t.to_uppercase(), p)))
                    .when_some(strip, |d, (spec, b)| d.child(self.db_row_strip(spec, &b, p, cx)))
                    .children(list.into_iter().map(|f| self.db_field(f, p, cx)))
                    .into_any_element()
            })
            .collect()
    }

    /// Above the fields of another table's row: a picker to copy a row when this
    /// entity has none, or where its own row came from with a way to remove it.
    pub(super) fn db_row_strip(
        &self,
        spec: usize,
        backing: &Backing,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let locked = self.db_locked();
        match backing {
            Backing::Copied(from) => self.db_copied_strip(spec, from.as_deref(), locked, p, cx),
            _ => self.db_default_strip(spec, locked, p),
        }
    }

    fn db_copied_strip(
        &self,
        spec: usize,
        from: Option<&str>,
        locked: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let from = match from {
            Some(l) => self
                .db
                .view
                .as_ref()
                .and_then(|v| v.view.rows.iter().find(|r| r.label == l))
                .map(|r| r.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| l.to_owned()),
            None => "the game default".into(),
        };
        h_flex()
            .gap(px(8.))
            .min_h(px(22.))
            .child(Icon::new(IconName::Copy).size(px(13.)).text_color(p.accent))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(12.))
                    .text_color(p.text_muted)
                    .truncate()
                    .child(trf!("Copied from {from}", from = from)),
            )
            .child(
                Button::new(SharedString::from(format!("db-row-remove-{spec}")))
                    .ghost()
                    .tap_xs()
                    .icon(IconName::Trash)
                    .label(tr("Remove own row"))
                    .disabled(locked)
                    .on_click(cx.listener(move |this, _, _, cx| this.db_remove_row(spec, cx))),
            )
            .into_any_element()
    }

    fn db_default_strip(&self, spec: usize, locked: bool, p: Palette) -> AnyElement {
        let picker = self
            .copy_id(spec)
            .and_then(|id| self.db.slots.copies.get(&id))
            .map(|s| {
                Select::new(&s.state)
                    .id(SharedString::from(format!("db-row-copy-{spec}")))
                    .tap_sm()
                    .w_full()
                    .menu_max_h(px(320.))
                    .search_placeholder(tr("Search"))
                    .placeholder(tr("Copy from…"))
                    .disabled(locked)
            });
        v_flex()
            .gap(px(8.))
            .px(px(10.))
            .py(px(9.))
            .rounded(px(6.))
            .border_1()
            .border_color(p.border_variant)
            .bg(p.surface)
            .child(
                h_flex()
                    .gap(px(8.))
                    .items_start()
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .text_color(p.text)
                    .child(
                        Icon::new(IconName::Info)
                            .size(px(13.))
                            .mt(px(2.))
                            .text_color(p.text_muted),
                    )
                    .child(div().flex_1().min_w_0().child(tr(
                        "These values come from the game default. Copy someone's row to edit them.",
                    ))),
            )
            .children(picker)
            .into_any_element()
    }

    pub(super) fn db_set_all_menu(
        &self,
        f: &FieldModel,
        locked: bool,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let field = f.all.filter(|_| !locked)?;
        let n = self.db_listed(field);
        if n < 2 {
            return None;
        }
        let mut picks: Vec<(String, String, String)> = Vec::new();
        if !f.value.is_empty() {
            let shown = shown_value(f, &f.value);
            picks.push((
                trf!("Set all {n} to {shown}", n = n, shown = shown),
                f.value.clone(),
                shown,
            ));
        }
        for (end, limit) in [(tr("max"), f.max), (tr("min"), f.min)] {
            if let Some(v) = limit {
                picks.push((
                    trf!("Set all {n} to the {end}, {v}", n = n, end = end, v = v),
                    format!("{v}"),
                    format!("{v}"),
                ));
            }
        }
        if picks.is_empty() {
            return None;
        }
        let shell = cx.entity().downgrade();
        Some(
            Button::new(SharedString::from(format!("all-{}", f.id)))
                .ghost()
                .tap_xs()
                .icon(IconName::ListChecks)
                .label(tr("All rows"))
                .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                    for (text, value, shown) in &picks {
                        let (shell, value, shown) = (shell.clone(), value.clone(), shown.clone());
                        menu = menu.item(PopupMenuItem::new(text.clone()).on_click(move |_, window, cx| {
                            shell
                                .update(cx, |s, cx| {
                                    s.db_confirm_set_all(field, value.clone(), shown.clone(), window, cx)
                                })
                                .ok();
                        }));
                    }
                    menu
                })
                .into_any_element(),
        )
    }

    pub(crate) fn db_field(&self, f: &FieldModel, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(key) = f.key else {
            return missing_field(f, p);
        };
        let locked = f.readonly || self.db_locked();
        let error = self.db.errors.get(&key).cloned();
        let revert = f.edited.then(|| {
            Button::new(SharedString::from(format!("revert-{}", f.id)))
                .ghost()
                .tap_xs()
                .icon(IconName::Undo2)
                .label(tr("Revert"))
                .disabled(self.db_locked())
                .on_click(cx.listener(move |this, _, _, cx| this.db_revert(key, cx)))
        });
        let all = self.db_set_all_menu(f, locked, cx);
        let control = self.db_control(f, key, locked, p, cx);
        let stock = f.stock.as_ref().filter(|_| f.edited).map(|s| shown_value(f, s));
        v_flex()
            .flex_shrink_0()
            .gap(px(6.))
            .child(field_head(f, all, revert, p))
            .child(control)
            .when_some(
                f.text
                    .clone()
                    .filter(|_| !matches!(f.kind, Kind::Ref | Kind::RefList | Kind::Choice)),
                |d, t| {
                    d.child(
                        h_flex()
                            .gap(px(6.))
                            .text_size(px(12.))
                            .text_color(p.text_muted)
                            .child(
                                Icon::new(IconName::Languages)
                                    .size(px(12.))
                                    .text_color(p.text_placeholder),
                            )
                            .child(div().min_w_0().truncate().child(t)),
                    )
                },
            )
            .when_some(error, |d, e| {
                d.child(
                    h_flex()
                        .gap(px(6.))
                        .items_start()
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .text_color(p.error)
                        .child(Icon::new(IconName::CircleAlert).size(px(12.)).mt(px(2.)))
                        .child(div().flex_1().min_w_0().child(e)),
                )
            })
            .when_some(stock, |d, s| {
                d.child(
                    div()
                        .text_size(px(12.))
                        .text_color(p.text_placeholder)
                        .truncate()
                        .flex_shrink_0()
                        .child(match &f.row {
                            Some((_, Backing::Copied(_))) => trf!("Copied: {s}", s = s),
                            _ => trf!("Stock: {s}", s = s),
                        }),
                )
            })
            .when_some(f.others.clone(), |d, t| {
                d.child(
                    h_flex()
                        .gap(px(6.))
                        .items_start()
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .text_color(p.text_muted)
                        .child(
                            Icon::new(IconName::Layers)
                                .size(px(12.))
                                .mt(px(2.))
                                .text_color(p.warning),
                        )
                        .child(div().flex_1().min_w_0().child(t)),
                )
            })
            .when_some(f.help.clone(), |d, h| {
                if f.shared {
                    d.child(Callout::warning(h))
                } else {
                    d.child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(17.))
                            .text_color(p.text_placeholder)
                            .child(h),
                    )
                }
            })
            .into_any_element()
    }
}

fn missing_field(f: &FieldModel, p: Palette) -> AnyElement {
    v_flex()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.text_muted)
                .child(f.label.clone()),
        )
        .child(
            div()
                .text_size(px(13.))
                .text_color(p.text_placeholder)
                .child(match (&f.row, f.optional) {
                    (Some((_, Backing::Copied(_))), _) => tr("Not in the copied row"),
                    (_, true) => tr("Inherited"),
                    (_, false) => tr("Not in this row"),
                }),
        )
        .into_any_element()
}

fn field_head(f: &FieldModel, all: Option<AnyElement>, revert: Option<Button>, p: Palette) -> Div {
    h_flex()
        .gap(px(6.))
        .min_h(px(22.))
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if f.edited { p.accent } else { p.text_muted })
                .truncate()
                .child(f.label.clone()),
        )
        .when(f.edited, |d| {
            d.child(div().size(px(6.)).flex_shrink_0().rounded_full().bg(p.accent))
        })
        .when(f.readonly, |d| {
            d.child(
                Icon::new(IconName::Lock)
                    .size(px(11.))
                    .text_color(p.text_placeholder),
            )
        })
        .child(div().flex_1())
        .children(all)
        .children(revert)
}
