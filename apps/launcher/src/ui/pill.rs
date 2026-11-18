use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, h_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use super::tokens::{Tone, radius, text};
use crate::scale::px;
use crate::theme::palette;

/// A small label: outlined and muted, or tinted by a [`Tone`].
#[derive(IntoElement)]
pub(crate) struct Pill {
    label: SharedString,
    tone: Option<Tone>,
    icon: Option<IconName>,
    style: StyleRefinement,
}

impl Pill {
    pub(crate) fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            tone: None,
            icon: None,
            style: StyleRefinement::default(),
        }
    }

    pub(crate) fn tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    pub(crate) fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }
}

impl Styled for Pill {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Pill {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let mut pill = h_flex()
            .px(px(6.))
            .h(px(20.))
            .gap(px(4.))
            .items_center()
            .flex_shrink_0()
            .rounded(radius::xs())
            .text_size(text::caption())
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .map(|d| match self.tone {
                Some(tone) => {
                    let color = tone.color(&p);
                    d.bg(color.opacity(0.14)).text_color(color)
                }
                None => d.border_1().border_color(p.border).text_color(p.text_muted),
            })
            .children(self.icon.map(|i| Icon::new(i).size(text::caption())))
            .child(self.label);
        pill.style().refine(&self.style);
        pill
    }
}
