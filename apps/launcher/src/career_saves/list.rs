use gpui_kit::component::v_flex;
use gpui_kit::*;

use crate::scale::px;
use crate::theme::Palette;
use crate::ui::row;

pub(super) fn list_box(rows: Vec<AnyElement>, p: Palette) -> Div {
    v_flex()
        .rounded(px(8.))
        .border_1()
        .border_color(p.border)
        .overflow_hidden()
        .children(
            rows.into_iter()
                .enumerate()
                .map(|(i, r)| row(i == 0, false, p).child(r)),
        )
}

pub(super) fn placeholder(text: &'static str, p: Palette) -> AnyElement {
    div()
        .px(px(12.))
        .py(px(14.))
        .text_size(px(13.))
        .text_color(p.text_muted)
        .child(text)
        .into_any_element()
}
