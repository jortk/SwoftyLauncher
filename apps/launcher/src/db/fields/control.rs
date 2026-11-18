use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::color_picker::ColorPicker;
use gpui_kit::component::input::{Input, NumberInput};
use gpui_kit::component::select::Select;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::dbedit::Kind;
use swofty_i18n::tr;

use super::FieldModel;
use super::bool_text;
use super::detail::{plain, swatch};
use super::value::{hex_rgb, is_on, list_items};
use crate::shell::Shell;
use crate::theme::Palette;

#[derive(Clone, Copy)]
struct ListEdit<'a> {
    f: &'a FieldModel,
    key: u64,
    locked: bool,
}

impl Shell {
    pub(super) fn db_control(
        &self,
        f: &FieldModel,
        key: u64,
        locked: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let slots = &self.db.slots;
        let unit = f
            .unit
            .clone()
            .map(|u| div().text_size(px(12.)).text_color(p.text_placeholder).child(u));
        match f.kind {
            Kind::Int | Kind::Float => match slots.inputs.get(&f.id) {
                Some(s) => NumberInput::new(&s.input)
                    .tap_sm()
                    .disabled(locked)
                    .when_some(unit, |n, u| n.suffix(u))
                    .into_any_element(),
                None => plain(&f.value, p),
            },
            Kind::List | Kind::Vector => self.db_list_control(f, locked, p),
            Kind::Text | Kind::Time => match slots.inputs.get(&f.id) {
                Some(s) => Input::new(&s.input)
                    .tap_sm()
                    .disabled(locked)
                    .when_some(unit, |n, u| n.suffix(u))
                    .into_any_element(),
                None => plain(&f.value, p),
            },
            Kind::Bool => Self::db_bool_control(f, key, locked, p, cx),
            Kind::Choice | Kind::Ref => self.db_select_control(f, locked, p),
            Kind::Color => self.db_color_control(f, locked, p),
            Kind::RefList => self.db_ref_list(f, key, locked, p, cx),
            Kind::Curve => self.db_curve(f, key, locked, p, cx),
        }
    }

    fn db_list_control(&self, f: &FieldModel, locked: bool, p: Palette) -> AnyElement {
        let slots = &self.db.slots;
        let items = list_items(&f.value);
        let wide = items.iter().any(|v| v.len() > 6);
        h_flex()
            .flex_wrap()
            .gap(px(6.))
            .children(items.iter().enumerate().map(|(i, v)| {
                let input = slots
                    .inputs
                    .get(&format!("{}/i{i}", f.id))
                    .map(|s| Input::new(&s.input).tap_xs().disabled(locked).into_any_element())
                    .unwrap_or_else(|| plain(v, p));
                div().w(px(if wide { 120. } else { 58. })).child(input)
            }))
            .when(items.is_empty(), |d| d.child(plain("empty", p)))
            .into_any_element()
    }

    fn db_bool_control(
        f: &FieldModel,
        key: u64,
        locked: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let on = is_on(&f.value);
        let (tag, stock) = (f.tag, f.stock.clone().unwrap_or_else(|| f.value.clone()));
        h_flex()
            .h(px(28.))
            .gap(px(10.))
            .child(
                Switch::new(SharedString::from(format!("switch-{}", f.id)))
                    .checked(on)
                    .tap_sm()
                    .disabled(locked)
                    .on_change(cx.listener(move |this, on: &bool, _, cx| {
                        this.db_set(key, &bool_text(tag, &stock, *on), cx);
                    })),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(if on { p.text } else { p.text_muted })
                    .child(if on { tr("On") } else { tr("Off") }),
            )
            .into_any_element()
    }

    fn db_select_control(&self, f: &FieldModel, locked: bool, p: Palette) -> AnyElement {
        let slots = &self.db.slots;
        match slots.selects.get(&f.id) {
            Some(s) => {
                let shown = f
                    .options
                    .iter()
                    .find(|(v, _)| *v == f.value)
                    .map(|(v, n)| if n.is_empty() { v.clone() } else { n.clone() })
                    .or_else(|| f.text.clone())
                    .unwrap_or_else(|| f.value.clone());
                Select::new(&s.state)
                    .id(SharedString::from(format!("select-{}", f.id)))
                    .tap_sm()
                    .w_full()
                    .menu_max_h(px(320.))
                    .search_placeholder(tr("Search"))
                    .placeholder(shown)
                    .disabled(locked)
                    .into_any_element()
            }
            None => plain(&f.value, p),
        }
    }

    fn db_color_control(&self, f: &FieldModel, locked: bool, p: Palette) -> AnyElement {
        let slots = &self.db.slots;
        let picker = slots
            .colors
            .get(&f.id)
            .map(|s| ColorPicker::new(&s.state).tap_sm());
        let raw = slots
            .inputs
            .get(&f.id)
            .map(|s| Input::new(&s.input).tap_sm().disabled(locked));
        h_flex()
            .gap(px(8.))
            .children(picker.filter(|_| !locked))
            .when(locked, |d| d.child(swatch(&f.value, 22., p)))
            .child(div().flex_1().min_w_0().children(raw))
            .child(
                div()
                    .w(px(64.))
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(p.text_muted)
                    .child(hex_rgb(&f.value).unwrap_or_default()),
            )
            .into_any_element()
    }

    fn db_ref_row(
        list: &ListEdit,
        i: usize,
        n: usize,
        shown: String,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Div {
        let ListEdit { f, key, locked } = *list;
        let small = |id: &str, icon: IconName| {
            Button::new(SharedString::from(format!("{}-{id}-{i}", f.id)))
                .ghost()
                .tap_xs()
                .icon(icon)
                .disabled(locked)
        };
        h_flex()
            .h(px(32.))
            .px(px(8.))
            .gap(px(8.))
            .when(i > 0, |d| d.border_t_1().border_color(p.border_variant))
            .child(
                div()
                    .w(px(20.))
                    .flex_shrink_0()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .child((i + 1).to_string()),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(div().text_size(px(13.)).truncate().child(shown)),
            )
            .child(
                small("up", IconName::ArrowUp)
                    .disabled(locked || i == 0)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.db_list_edit(key, |l| l.swap(i, i - 1), cx)),
                    ),
            )
            .child(
                small("down", IconName::ArrowDown)
                    .disabled(locked || i + 1 == n)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.db_list_edit(key, |l| l.swap(i, i + 1), cx)),
                    ),
            )
            .child(
                small("remove", IconName::X).on_click(cx.listener(move |this, _, _, cx| {
                    this.db_list_edit(
                        key,
                        |l| {
                            l.remove(i);
                        },
                        cx,
                    )
                })),
            )
    }

    pub(super) fn db_ref_list(
        &self,
        f: &FieldModel,
        key: u64,
        locked: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let items = list_items(&f.value);
        let n = items.len();
        let name = |v: &str| {
            f.options
                .iter()
                .find(|(o, _)| o == v)
                .map(|(_, n)| n.clone())
                .filter(|n| !n.is_empty())
        };
        let rows = items.iter().enumerate().map(|(i, v)| {
            let shown = name(v).unwrap_or_else(|| v.clone());
            Self::db_ref_row(&ListEdit { f, key, locked }, i, n, shown, p, cx)
        });
        let add = self.db.slots.selects.get(&format!("{}/add", f.id)).map(|s| {
            Select::new(&s.state)
                .id(SharedString::from(format!("add-{}", f.id)))
                .tap_sm()
                .w_full()
                .menu_max_h(px(320.))
                .search_placeholder(tr("Search"))
                .placeholder(tr("Add…"))
                .disabled(locked)
        });
        v_flex()
            .gap(px(8.))
            .child(
                v_flex()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(p.border_variant)
                    .bg(p.background)
                    .children(rows)
                    .when(n == 0, |d| {
                        d.child(
                            div()
                                .px(px(10.))
                                .py(px(8.))
                                .text_size(px(13.))
                                .text_color(p.text_placeholder)
                                .child(tr("Empty")),
                        )
                    }),
            )
            .children(add)
            .into_any_element()
    }
}
