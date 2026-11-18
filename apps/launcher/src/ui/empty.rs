use gpui_kit::assets::IconName;
use gpui_kit::component::v_flex;
use gpui_kit::*;

use super::tokens::{space, text};
use crate::pages::badge;
use crate::scale::px;
use crate::theme::palette;

/// The centred icon, title, text and optional action shown when a page has nothing.
#[derive(IntoElement)]
pub(crate) struct EmptyState {
    icon: IconName,
    title: SharedString,
    body: Option<SharedString>,
    action: Option<AnyElement>,
}

impl EmptyState {
    pub(crate) fn new(icon: IconName, title: impl Into<SharedString>) -> Self {
        Self {
            icon,
            title: title.into(),
            body: None,
            action: None,
        }
    }

    pub(crate) fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub(crate) fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }
}

impl RenderOnce for EmptyState {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap(space::sm())
            .pb(px(48.))
            .child(badge(self.icon, 44., p))
            .child(
                div()
                    .pt(space::sm())
                    .text_size(text::title())
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.title),
            )
            .children(self.body.map(|body| {
                div()
                    .max_w(px(400.))
                    .text_center()
                    .text_size(text::body())
                    .line_height(px(19.))
                    .text_color(p.text_muted)
                    .child(body)
            }))
            .children(self.action.map(|a| div().pt(space::sm()).child(a)))
    }
}
