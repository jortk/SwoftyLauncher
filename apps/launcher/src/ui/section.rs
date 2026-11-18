use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;

use super::tokens::{space, text};
use crate::pages::badge;
use crate::scale::px;
use crate::theme::palette;

/// An icon tile with a title and a one-line subtitle, heading a section of a page.
#[derive(IntoElement)]
pub(crate) struct SectionHead {
    icon: IconName,
    title: SharedString,
    subtitle: SharedString,
}

impl SectionHead {
    pub(crate) fn new(
        icon: IconName,
        title: impl Into<SharedString>,
        subtitle: impl Into<SharedString>,
    ) -> Self {
        Self {
            icon,
            title: title.into(),
            subtitle: subtitle.into(),
        }
    }
}

impl RenderOnce for SectionHead {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        h_flex()
            .flex_1()
            .min_w_0()
            .gap(space::md())
            .child(badge(self.icon, 36., p))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(text::title())
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.title),
                    )
                    .child(
                        div()
                            .text_size(text::body())
                            .text_color(p.text_muted)
                            .truncate()
                            .child(self.subtitle),
                    ),
            )
    }
}

/// A small muted caption above a control.
#[derive(IntoElement)]
pub(crate) struct Field {
    caption: SharedString,
    control: AnyElement,
}

impl Field {
    pub(crate) fn new(caption: impl Into<SharedString>, control: impl IntoElement) -> Self {
        Self {
            caption: caption.into(),
            control: control.into_any_element(),
        }
    }
}

impl RenderOnce for Field {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        v_flex()
            .gap(px(6.))
            .child(
                div()
                    .text_size(text::small())
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.text_muted)
                    .child(self.caption),
            )
            .child(self.control)
    }
}
