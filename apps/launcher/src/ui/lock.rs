//! The content of a lock: a [`Overlay`] that blocks the page under it while
//! something long runs, with a title, a line of detail, a progress bar and an
//! optional action.

use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use super::{Overlay, ProgressBar};
use crate::scale::px;
use crate::theme::Palette;

pub(crate) struct LockPanel {
    title: SharedString,
    detail: Option<SharedString>,
    status: Option<SharedString>,
    fraction: Option<f32>,
    action: Option<AnyElement>,
}

impl LockPanel {
    pub(crate) fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            detail: None,
            status: None,
            fraction: None,
            action: None,
        }
    }

    pub(crate) fn detail(mut self, text: impl Into<SharedString>) -> Self {
        self.detail = Some(text.into());
        self
    }

    pub(crate) fn status(mut self, text: impl Into<SharedString>) -> Self {
        self.status = Some(text.into());
        self
    }

    pub(crate) fn progress(mut self, fraction: f32) -> Self {
        self.fraction = Some(fraction.clamp(0., 1.));
        self
    }

    pub(crate) fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }

    pub(crate) fn render(self, id: impl Into<ElementId>, p: Palette) -> Overlay {
        let bar = self.fraction.map(|done| {
            v_flex()
                .w_full()
                .gap(px(6.))
                .child(ProgressBar::new(done).height(6.))
                .child(
                    h_flex()
                        .w_full()
                        .gap(px(10.))
                        .text_size(px(12.))
                        .text_color(p.text_muted)
                        .child(div().flex_1().min_w_0().truncate().children(self.status.clone()))
                        .child(format!("{}%", (done * 100.).floor())),
                )
        });
        Overlay::new(id).opaque(p.background).below_title_bar().child(
            v_flex()
                .w(px(560.))
                .max_w_full()
                .px(px(24.))
                .gap(px(16.))
                .items_center()
                .child(Spinner::new().with_size(px(28.)))
                .child(
                    div()
                        .text_size(px(22.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(p.text)
                        .child(self.title),
                )
                .when_some(self.detail, |c, text| {
                    c.child(
                        div()
                            .w_full()
                            .text_center()
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .text_color(p.text_muted)
                            .child(text),
                    )
                })
                .children(bar)
                .children(self.action),
        )
    }
}
