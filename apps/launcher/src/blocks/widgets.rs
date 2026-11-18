use crate::scale::px;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde_json::Value;
use swofty_core::view::{Cell, Field, Pill, Table, Tone};

use super::*;
use crate::colour::hsla_of;
use crate::pages::label;
use crate::sections::{icon, tone};
use crate::theme::Palette;

pub(super) fn swatch(rgb: Option<[u8; 3]>, size: f32, p: Palette) -> Div {
    let dot = div().size(px(size)).flex_shrink_0().rounded_full();
    match rgb {
        Some(c) => dot.bg(hsla_of(c)).border_1().border_color(hsla(0., 0., 1., 0.12)),
        None => dot.border_1().border_color(p.text_placeholder),
    }
}

pub(crate) fn pill(p: &Pill, pal: Palette) -> Div {
    let color = tone(p.tone, pal);
    let (bg, fg) = match p.tone {
        Tone::Neutral => (pal.elevated, pal.text),
        Tone::Muted => (pal.hover, pal.text_muted),
        _ => (color.opacity(0.14), color),
    };
    h_flex()
        .h(px(22.))
        .px(px(9.))
        .gap(px(6.))
        .flex_shrink_0()
        .rounded(px(11.))
        .bg(bg)
        .text_color(fg)
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .whitespace_nowrap()
        .when(p.dot, |d| d.child(div().size(px(6.)).rounded_full().bg(fg)))
        .when_some(p.icon.filter(|_| !p.dot), |d, i| {
            d.child(Icon::new(icon(i)).size(px(12.)))
        })
        .child(p.text.clone())
}

pub(super) fn pills(items: &[Pill], pal: Palette) -> Div {
    h_flex()
        .gap(px(6.))
        .flex_wrap()
        .children(items.iter().map(|p| pill(p, pal)))
}

pub(super) fn bar(fraction: f32, color: Hsla, height: f32, pal: Palette) -> Div {
    div()
        .h(px(height))
        .w_full()
        .rounded_full()
        .bg(pal.elevated)
        .child(
            div()
                .h_full()
                .w(relative(fraction.clamp(0., 1.)))
                .rounded_full()
                .bg(color),
        )
}

pub(super) fn fill_tone(t: Option<Tone>, pal: Palette) -> Hsla {
    match t {
        None | Some(Tone::Neutral) => pal.accent,
        Some(t) => tone(t, pal),
    }
}

pub(super) fn tile_columns(count: usize, width: f32) -> usize {
    let wanted = match count {
        0..=4 => count.max(1),
        5 | 6 => 3,
        _ => 4,
    };
    wanted.min(((width + GAP) / (TILE_COLUMN + GAP)).floor().max(1.) as usize)
}

pub(super) fn surface(pal: Palette) -> Div {
    v_flex()
        .rounded(px(8.))
        .border_1()
        .border_color(pal.border_variant)
        .bg(pal.surface)
}

pub(super) fn number_text(text: &str) -> Option<f64> {
    let cleaned: String = text.chars().filter(|c| !matches!(c, ',' | ' ' | '_')).collect();
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

pub(super) fn field_text(field: &Field, value: &Value) -> String {
    match (field, value) {
        (Field::Number(f) | Field::Slider(f), Value::Number(n)) => {
            n.as_f64().map(|v| f.text(v)).unwrap_or_default()
        }
        (_, Value::String(s)) => s.clone(),
        (_, Value::Null) => String::new(),
        (_, v) => v.to_string(),
    }
}

pub(super) fn sorted_rows(t: &Table, sort: Option<&(String, bool)>) -> Vec<usize> {
    let mut order: Vec<usize> = (0..t.rows.len()).collect();
    let Some((column, desc)) = sort else {
        return order;
    };
    let Some(ci) = t.columns.iter().position(|c| c.key() == column) else {
        return order;
    };
    let keys: Vec<_> = t
        .rows
        .iter()
        .map(|r| r.cells().get(ci).map(Cell::sort_key))
        .collect();
    order.sort_by(|a, b| match (&keys[*a], &keys[*b]) {
        (Some(x), Some(y)) => x.compare(y),
        (x, y) => x.is_some().cmp(&y.is_some()).reverse(),
    });
    if *desc {
        order.reverse();
    }
    order
}

pub(crate) fn caps(text: &str) -> String {
    text.to_uppercase()
}

pub(crate) fn eyebrow(text: &str, p: Palette) -> Div {
    label(caps(text), p)
}
