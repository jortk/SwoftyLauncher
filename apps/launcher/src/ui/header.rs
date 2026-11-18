use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use super::tokens::{space, text};
use crate::theme::palette;

/// A page's title row: title and subtitle on the left, actions on the right,
/// and optionally something centred between them.
#[derive(IntoElement)]
pub(crate) struct PageHeader {
    title: SharedString,
    subtitle: Option<SharedString>,
    error: Option<SharedString>,
    actions: Option<AnyElement>,
    middle: Option<AnyElement>,
}

impl PageHeader {
    pub(crate) fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            subtitle: None,
            error: None,
            actions: None,
            middle: None,
        }
    }

    pub(crate) fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub(crate) fn error(mut self, error: Option<impl Into<SharedString>>) -> Self {
        self.error = error.map(Into::into);
        self
    }

    pub(crate) fn actions(mut self, actions: impl IntoElement) -> Self {
        self.actions = Some(actions.into_any_element());
        self
    }

    /// The two sides then share the rest of the row equally, which keeps it centred.
    pub(crate) fn middle(mut self, middle: Option<impl IntoElement>) -> Self {
        self.middle = middle.map(IntoElement::into_any_element);
        self
    }
}

impl RenderOnce for PageHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let centred = self.middle.is_some();
        h_flex()
            .px(space::gutter())
            .gap(space::lg())
            .map(|row| {
                if centred {
                    row.py(crate::scale::px(6.)).items_center()
                } else {
                    row.pt(crate::scale::px(22.)).pb(space::lg()).items_end()
                }
            })
            .flex_shrink_0()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(crate::scale::px(3.))
                    .child(
                        div()
                            .text_size(text::heading())
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.title),
                    )
                    .children(
                        self.subtitle
                            .filter(|s| !s.is_empty())
                            .map(|s| div().text_size(text::body()).text_color(p.text_muted).child(s)),
                    )
                    .children(
                        self.error
                            .map(|e| div().text_size(text::body()).text_color(p.error).child(e)),
                    ),
            )
            .children(self.middle)
            .child(
                h_flex()
                    .when(centred, |side| side.flex_1().justify_end())
                    .gap(space::sm())
                    .pb(crate::scale::px(2.))
                    .children(self.actions),
            )
    }
}
