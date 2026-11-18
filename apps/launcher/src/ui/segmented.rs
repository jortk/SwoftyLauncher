use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::rc::Rc;

use crate::scale::px;
use crate::theme::Palette;

type Pick = Rc<dyn Fn(&mut Window, &mut App)>;

/// A row of mutually exclusive choices, the chosen one filled.
#[derive(IntoElement)]
pub(crate) struct Segmented {
    id: ElementId,
    items: Vec<(SharedString, bool, Pick)>,
    palette: Palette,
}

impl Segmented {
    pub(crate) fn new(id: impl Into<ElementId>, palette: Palette) -> Self {
        Self {
            id: id.into(),
            items: Vec::new(),
            palette,
        }
    }

    pub(crate) fn item(
        mut self,
        label: impl Into<SharedString>,
        active: bool,
        pick: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.items.push((label.into(), active, Rc::new(pick)));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let p = self.palette;
        let id = self.id;
        h_flex()
            .p(px(2.))
            .gap(px(2.))
            .rounded(px(8.))
            .border_1()
            .border_color(p.border_variant)
            .bg(p.surface)
            .children(
                self.items
                    .into_iter()
                    .enumerate()
                    .map(|(i, (label, active, pick))| {
                        h_flex()
                            .id(SharedString::from(format!("{id}-{i}")))
                            .h(px(28.))
                            .px(px(12.))
                            .items_center()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .map(|d| {
                                if active {
                                    d.bg(p.selected).text_color(p.text)
                                } else {
                                    d.text_color(p.text_muted)
                                        .hover(move |s| s.bg(p.hover).text_color(p.text))
                                }
                            })
                            .child(label)
                            .on_click(move |_, window, cx| pick(window, cx))
                    }),
            )
    }
}
