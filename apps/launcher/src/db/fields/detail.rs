use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::dbedit::{Kind, NodeId};
use swofty_i18n::tr;

use super::FieldModel;
use super::value::{hex_rgb, swatch_color};
use super::{hex, type_name};
use crate::pages::fact;
use crate::shell::Shell;
use crate::theme::{Palette, ScrollArea};

impl Shell {
    pub(crate) fn db_key_detail(&self, node: NodeId, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(catalog) = self.db.catalog().cloned() else {
            return div().into_any_element();
        };
        let n = catalog.node(node);
        let Some(field) = self.db_key_field(node) else {
            return div().into_any_element();
        };
        let key = field.key.unwrap_or_default();
        let meta = catalog.meta(key).cloned().unwrap_or_default();
        let facts = [
            ("Key hash", hex(key)),
            ("Type", type_name(field.tag)),
            ("Meaning", meta.meaning.clone()),
            ("Unit", meta.unit.clone()),
            ("Range", meta.range.clone()),
            ("Example", meta.example.clone()),
            ("Evidence", meta.evidence.clone()),
        ];
        detail_shell(
            "db-key-detail",
            n.name.clone(),
            catalog.path(node),
            p,
            cx.listener(|this, _, _, cx| {
                this.db.key = None;
                cx.notify();
            }),
        )
        .child(
            v_flex()
                .flex_1()
                .min_h_0()
                .scroll_y("db-key-body")
                .px(px(16.))
                .py(px(14.))
                .gap(px(18.))
                .child(self.db_field(&field, p, cx))
                .child(
                    v_flex().children(
                        facts
                            .into_iter()
                            .filter(|(_, v)| !v.is_empty())
                            .map(|(l, v)| fact(l, v, p)),
                    ),
                ),
        )
        .into_any_element()
    }
}

pub(super) fn shown_value(f: &FieldModel, v: &str) -> String {
    match f.kind {
        Kind::Choice | Kind::Ref => f
            .options
            .iter()
            .find(|(o, _)| o == v)
            .map(|(_, n)| n.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| v.to_owned()),
        Kind::Color => hex_rgb(v).unwrap_or_else(|| v.to_owned()),
        _ if v.is_empty() => "empty".into(),
        _ => v.to_owned(),
    }
}

pub(super) fn plain(v: &str, p: Palette) -> AnyElement {
    div()
        .text_size(px(13.))
        .text_color(p.text)
        .truncate()
        .child(v.to_owned())
        .into_any_element()
}

pub(crate) fn swatch(v: &str, size: f32, p: Palette) -> Div {
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(4.))
        .border_1()
        .border_color(p.border)
        .when_some(swatch_color(v), |d, c| d.bg(c))
}

pub(crate) fn detail_shell(
    id: &'static str,
    title: String,
    subtitle: String,
    p: Palette,
    close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    v_flex()
        .id(id)
        .w(px(380.))
        .flex_shrink_0()
        .h_full()
        .rounded(px(8.))
        .border_1()
        .border_color(p.border_variant)
        .bg(p.surface)
        .overflow_hidden()
        .child(
            h_flex()
                .px(px(16.))
                .py(px(12.))
                .gap(px(10.))
                .flex_shrink_0()
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
                                .child(title),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.text_muted)
                                .truncate()
                                .child(subtitle),
                        ),
                )
                .child(
                    Button::new(SharedString::from(format!("{id}-close")))
                        .ghost()
                        .tap_sm()
                        .icon(IconName::X)
                        .tooltip(tr("Close"))
                        .on_click(close),
                ),
        )
}
