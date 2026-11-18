use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::scale::{Hit, px};
use crate::theme::Palette;

/// One row of a list card: a top divider unless it is `first`, tinted while `open`.
pub(crate) fn row(first: bool, open: bool, p: Palette) -> Div {
    v_flex()
        .flex_shrink_0()
        .when(!first, |d| d.border_t_1().border_color(p.border_variant))
        .when(open, |d| d.bg(p.surface))
}

/// The clickable head of a [`row`].
pub(crate) fn row_head(id: impl Into<SharedString>, open: bool, p: Palette) -> Stateful<Div> {
    h_flex()
        .id(id.into())
        .h(px(54.))
        .px(px(14.))
        .gap(px(12.))
        .cursor_pointer()
        .when(!open, |d| d.hover(move |s| s.bg(p.hover)))
}

/// A compact row in a selectable list: outlined and tinted while `selected`.
pub(crate) fn selectable_row(id: impl Into<ElementId>, selected: bool, p: Palette) -> Stateful<Div> {
    h_flex()
        .id(id)
        .min_h(px(44.))
        .px(px(8.))
        .py(px(6.))
        .gap(px(8.))
        .items_center()
        .rounded(px(5.))
        .border_1()
        .map(|d| {
            if selected {
                d.bg(p.selected).border_color(p.border)
            } else {
                d.border_color(gpui_kit::transparent_black())
            }
        })
}

type Change = Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

/// A switch that doesn't pass its clicks on to a clickable row around it.
#[derive(IntoElement)]
pub(crate) struct Toggle {
    id: SharedString,
    on: bool,
    disabled: bool,
    on_change: Option<Change>,
}

impl Toggle {
    pub(crate) fn new(id: impl Into<SharedString>, on: bool) -> Self {
        Self {
            id: id.into(),
            on,
            disabled: false,
            on_change: None,
        }
    }

    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub(crate) fn on_change(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Toggle {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let hold = SharedString::from(format!("{}-hold", self.id));
        let switch = Switch::new(self.id)
            .checked(self.on)
            .tap_sm()
            .disabled(self.disabled)
            .when_some(self.on_change, |s, f| s.on_change(f));
        div()
            .id(hold)
            .min_h(crate::scale::ctl(26.))
            .flex()
            .items_center()
            .px(crate::scale::ctl(4.))
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(switch)
    }
}
