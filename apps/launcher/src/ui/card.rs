use gpui_kit::component::v_flex;
use gpui_kit::*;

use super::tokens::radius;
use crate::theme::palette;

/// A bordered panel on the surface colour. Style and fill it like a `div`.
#[derive(IntoElement)]
pub(crate) struct Card {
    style: StyleRefinement,
    children: Vec<AnyElement>,
}

impl Card {
    pub(crate) fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: Vec::new(),
        }
    }
}

impl Styled for Card {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Card {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Card {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        let mut card = v_flex()
            .rounded(radius::md())
            .border_1()
            .overflow_hidden()
            .border_color(p.border)
            .bg(p.surface)
            .children(self.children);
        card.style().refine(&self.style);
        card
    }
}
