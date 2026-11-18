use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::*;

use super::tokens::{Tone, radius, space, text};
use crate::scale::px;
use crate::theme::palette;

/// A tinted box with an icon and a message: info, warning or error.
#[derive(IntoElement)]
pub(crate) struct Callout {
    tone: Tone,
    icon: Option<IconName>,
    title: Option<SharedString>,
    body: SharedString,
    actions: Option<AnyElement>,
}

impl Callout {
    pub(crate) fn new(tone: Tone, body: impl Into<SharedString>) -> Self {
        Self {
            tone,
            icon: None,
            title: None,
            body: body.into(),
            actions: None,
        }
    }

    pub(crate) fn info(body: impl Into<SharedString>) -> Self {
        Self::new(Tone::Info, body)
    }

    pub(crate) fn warning(body: impl Into<SharedString>) -> Self {
        Self::new(Tone::Warning, body)
    }

    pub(crate) fn error(body: impl Into<SharedString>) -> Self {
        Self::new(Tone::Error, body)
    }

    pub(crate) fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub(crate) fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub(crate) fn actions(mut self, actions: impl IntoElement) -> Self {
        self.actions = Some(actions.into_any_element());
        self
    }
}

impl RenderOnce for Callout {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let color = self.tone.color(&p);
        h_flex()
            .gap(px(10.))
            .items_start()
            .p(space::md())
            .rounded(radius::md())
            .bg(color.opacity(0.08))
            .border_1()
            .border_color(color.opacity(0.4))
            .child(
                Icon::new(self.icon.unwrap_or_else(|| self.tone.icon()))
                    .size(px(15.))
                    .flex_shrink_0()
                    .text_color(color),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(3.))
                    .children(self.title.map(|t| {
                        div()
                            .text_size(text::body())
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.text)
                            .child(t)
                    }))
                    .child(
                        div()
                            .text_size(text::body())
                            .line_height(px(19.))
                            .text_color(p.text)
                            .child(self.body),
                    )
                    .children(self.actions.map(|a| div().pt(space::xs()).child(a))),
            )
    }
}
