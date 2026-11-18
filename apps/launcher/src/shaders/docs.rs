//! The Documentation tab: every effect a config can use, from the core library and installed addons,
//! with its options, ranges and defaults.

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_i18n::{tr, trf, trn};
use swofty_shaders::{Effect, Origin, Param, ParamKind, Value};

use super::ShadersView;
use crate::pages::{label, tag};
use crate::theme::{Palette, ScrollArea};

/// The effect library's page in the launcher docs.
pub(super) const DOCS_URL: &str = "https://f1.swofty.net/reference/addons/shader-effects";

fn number(n: f64) -> String {
    format!("{n}")
}

fn value(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => number(*n),
        Value::Text(t) => format!("{t:?}"),
        Value::Vector(v) => format!(
            "[{}]",
            v.iter().map(|n| number(*n)).collect::<Vec<_>>().join(", ")
        ),
    }
}

fn bounds(min: &Option<f64>, max: &Option<f64>) -> String {
    match (min, max) {
        (Some(a), Some(b)) => format!("{} … {}", number(*a), number(*b)),
        (Some(a), None) => trf!("{min} or more", min = number(*a)),
        (None, Some(b)) => trf!("up to {max}", max = number(*b)),
        (None, None) => String::new(),
    }
}

/// The kind and range of a param, as one short line.
fn kind(p: &Param) -> String {
    let (name, range) = match &p.kind {
        ParamKind::Float { min, max, .. } => (tr("number"), bounds(min, max)),
        ParamKind::Int { min, max } => (
            tr("whole number"),
            bounds(&min.map(|v| v as f64), &max.map(|v| v as f64)),
        ),
        ParamKind::Bool => (tr("true or false"), String::new()),
        ParamKind::Enum { values } => (
            tr("one of"),
            values
                .iter()
                .map(|v| format!("{v:?}"))
                .collect::<Vec<_>>()
                .join(" · "),
        ),
        ParamKind::Float2 { min, max } => (tr("2 numbers"), bounds(min, max)),
        ParamKind::Color3 { min, max } => (tr("r, g, b"), bounds(min, max)),
        ParamKind::Color4 { min, max } => (tr("r, g, b, a"), bounds(min, max)),
    };
    if range.is_empty() {
        name.to_owned()
    } else {
        format!("{name}  {range}")
    }
}

/// The effect as a config table with every option at its default, ready to paste into a config.
pub(super) fn snippet(e: &Effect) -> String {
    let mut out = format!("[{}]\n", e.id);
    for p in &e.params {
        let label = if p.label.is_empty() || p.label == p.id {
            String::new()
        } else {
            format!("  # {}", p.label)
        };
        out.push_str(&format!("{} = {}{label}\n", p.id, value(&p.default)));
    }
    out
}

impl ShadersView {
    pub(super) fn docs_pane(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let mono: SharedString = cx.theme().mono_font_family.clone();
        let mut library: Vec<&Effect> = Vec::new();
        let mut addons: Vec<(String, Vec<&Effect>)> = Vec::new();
        for e in &self.catalog.effects {
            match &e.origin {
                Origin::Library => library.push(e),
                Origin::Addon { name, .. } => match addons.iter_mut().find(|(n, _)| n == name) {
                    Some((_, list)) => list.push(e),
                    None => addons.push((name.clone(), vec![e])),
                },
            }
        }
        let mut body = v_flex().gap(px(14.)).pr(px(10.)).pb(px(16.));
        if !library.is_empty() {
            body = body.child(label(tr("CORE LIBRARY"), p));
            for e in library {
                body = body.child(self.effect_card(e, tr("Core"), &mono, p, cx));
            }
        }
        for (name, list) in addons {
            body = body.child(label(trf!("FROM {name}", name = name.to_uppercase()), p));
            for e in list {
                body = body.child(self.effect_card(e, &name, &mono, p, cx));
            }
        }
        v_flex()
            .size_full()
            .min_w_0()
            .gap(px(12.))
            .child(
                h_flex()
                    .w_full()
                    .gap(px(12.))
                    .items_start()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(3.))
                            .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(tr("Effect library")))
                            .child(div().text_size(px(13.)).text_color(p.text_muted).child(
                                tr("Every effect a config can use, with its options. Copy puts the effect's table in your clipboard with every option at its default, ready to paste into a config."),
                            )),
                    )
                    .child(
                        Button::new("shaders-docs-web")
                            .tap_sm()
                            .icon(IconName::ExternalLink)
                            .label(tr("Full documentation"))
                            .on_click(|_, _, cx| cx.open_url(DOCS_URL)),
                    ),
            )
            .child(div().flex_1().min_h_0().child(v_flex().size_full().scroll_y("shaders-docs").child(body)))
            .into_any_element()
    }

    fn effect_card(
        &self,
        e: &Effect,
        source: &str,
        mono: &SharedString,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let copied = self.docs_copied.as_deref() == Some(e.id.as_str());
        let text = snippet(e);
        let id = e.id.clone();
        let passes = e.passes.len();
        let mut params = v_flex().gap(px(1.));
        for (i, prm) in e.params.iter().enumerate() {
            params = params.child(
                h_flex()
                    .gap(px(14.))
                    .px(px(10.))
                    .py(px(7.))
                    .items_start()
                    .rounded(px(5.))
                    .when(i % 2 == 0, |d| d.bg(p.background))
                    .child(
                        v_flex()
                            .w(px(230.))
                            .flex_none()
                            .gap(px(2.))
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .text_size(px(12.5))
                                    .text_color(p.accent)
                                    .child(format!("{} = {}", prm.id, value(&prm.default))),
                            )
                            .child(
                                div()
                                    .font_family(mono.clone())
                                    .text_size(px(11.5))
                                    .text_color(p.text_placeholder)
                                    .child(kind(prm)),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(prm.label.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(p.text_muted)
                                    .child(prm.doc.clone()),
                            ),
                    ),
            );
        }
        v_flex()
            .gap(px(10.))
            .p(px(14.))
            .rounded(px(8.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .child(
                h_flex()
                    .gap(px(10.))
                    .items_center()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(e.name.clone()),
                    )
                    .child(
                        div()
                            .font_family(mono.clone())
                            .text_size(px(12.))
                            .text_color(p.text_muted)
                            .child(format!("[{}]", e.id)),
                    )
                    .child(tag(source.to_owned(), p))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(trf!(
                                "{options} · {passes}",
                                options = trn(e.params.len(), "{count} option", "{count} options"),
                                passes = trn(passes, "{count} pass", "{count} passes")
                            )),
                    )
                    .child(
                        Button::new(SharedString::from(format!("shaders-docs-copy-{}", e.id)))
                            .tap_xs()
                            .ghost()
                            .icon(if copied { IconName::Check } else { IconName::Copy })
                            .label(if copied { tr("Copied") } else { tr("Copy") })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                                this.docs_copied = Some(id.clone());
                                cx.notify();
                            })),
                    ),
            )
            .when(!e.description.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(px(13.))
                        .text_color(p.text_muted)
                        .child(e.description.clone()),
                )
            })
            .child(params)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{kind, snippet};
    use swofty_shaders::Origin;

    #[test]
    fn a_snippet_lists_every_option_at_its_default() {
        let json = r#"{ "id": "look", "name": "Look", "description": "",
            "params": [
                { "id": "strength", "type": "float", "default": 100, "min": 0, "max": 100, "label": "Look strength", "doc": "d" },
                { "id": "warm", "type": "bool", "default": false, "label": "", "doc": "d" },
                { "id": "mode", "type": "enum", "values": ["soft", "hard"], "default": "soft", "label": "Mode", "doc": "d" },
                { "id": "tint", "type": "color3", "default": [1, 0.5, 0], "label": "Tint", "doc": "d" }
            ],
            "passes": [{ "name": "look", "shader": "look.hlsl" }] }"#;
        let e = swofty_shaders::parse_effect(json, Origin::Library).unwrap();
        assert_eq!(
            snippet(&e),
            "[look]\nstrength = 100  # Look strength\nwarm = false\nmode = \"soft\"  # Mode\ntint = [1, 0.5, 0]  # Tint\n"
        );
        assert_eq!(kind(&e.params[0]), "number  0 … 100");
        assert_eq!(kind(&e.params[2]), "one of  \"soft\" · \"hard\"");
    }
}
