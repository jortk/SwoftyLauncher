use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use super::tokens::{radius, text};
use crate::scale::px;
use crate::theme::palette;

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A small clickable toggle button: outlined when off, tinted when on, accent-filled
/// when `filled`.
#[derive(IntoElement)]
pub(crate) struct Chip {
    id: SharedString,
    label: SharedString,
    on: bool,
    filled: bool,
    on_click: Option<Click>,
}

impl Chip {
    pub(crate) fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            on: false,
            filled: false,
            on_click: None,
        }
    }

    pub(crate) fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    pub(crate) fn filled(mut self, filled: bool) -> Self {
        self.filled = filled;
        self
    }

    pub(crate) fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Chip {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        h_flex()
            .id(self.id)
            .h(px(24.))
            .min_w(px(28.))
            .px(px(9.))
            .justify_center()
            .items_center()
            .rounded(radius::sm())
            .border_1()
            .cursor_pointer()
            .text_size(text::small())
            .font_weight(FontWeight::MEDIUM)
            .map(|d| match (self.on, self.filled) {
                (true, true) => d.bg(p.accent).border_color(p.accent).text_color(p.on_accent),
                (true, false) => d.bg(p.selected).border_color(p.border).text_color(p.text),
                _ => d
                    .border_color(p.border)
                    .text_color(p.text_muted)
                    .hover(move |s| s.text_color(p.text).bg(p.hover)),
            })
            .when_some(self.on_click, |d, f| d.on_click(f))
            .child(self.label)
    }
}
