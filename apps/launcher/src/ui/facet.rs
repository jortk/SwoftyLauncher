use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::scale::{ctl, px};
use crate::theme::Palette;

pub(crate) const FILTERS_W: f32 = 226.;

pub(crate) fn checkbox(on: bool, p: Palette) -> Div {
    div()
        .size(ctl(14.))
        .flex_shrink_0()
        .rounded(px(3.))
        .border_1()
        .flex()
        .items_center()
        .justify_center()
        .map(|d| {
            if on {
                d.bg(p.accent)
                    .border_color(p.accent)
                    .child(Icon::new(IconName::Check).size(ctl(11.)).text_color(p.on_accent))
            } else {
                d.border_color(p.border)
            }
        })
}

/// A row of a filter column: a checkbox, its label and how many it would show.
pub(crate) fn facet_row(
    id: impl Into<SharedString>,
    on: bool,
    text: impl Into<SharedString>,
    count: usize,
    p: Palette,
) -> Stateful<Div> {
    facet_row_with(id, on, text, count.to_string(), count == 0, p)
}

pub(crate) fn facet_row_with(
    id: impl Into<SharedString>,
    on: bool,
    text: impl Into<SharedString>,
    count: String,
    empty: bool,
    p: Palette,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let dim = empty && !on;
    h_flex()
        .id(id)
        .h(ctl(26.))
        .px(px(6.))
        .gap(px(8.))
        .rounded(px(5.))
        .cursor_pointer()
        .hover(move |s| s.bg(p.hover))
        .child(checkbox(on, p))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(13.))
                .text_color(if dim { p.text_placeholder } else { p.text })
                .child(text.into()),
        )
        .child(
            div()
                .text_size(px(12.))
                .text_color(if on { p.text_muted } else { p.text_placeholder })
                .child(count),
        )
}
