use gpui_kit::component::{TITLE_BAR_HEIGHT, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::scale::px;
use crate::theme::Palette;

type Dismiss = Box<dyn Fn(&mut Window, &mut App) + 'static>;

/// A full-window layer: a scrim with its content centred on it. It takes every mouse
/// and scroll event, so nothing underneath can be used while it is up.
#[derive(IntoElement)]
pub(crate) struct Overlay {
    id: ElementId,
    scrim: Hsla,
    below_title_bar: bool,
    scrolls: bool,
    on_dismiss: Option<Dismiss>,
    children: Vec<AnyElement>,
}

impl Overlay {
    pub(crate) fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            scrim: hsla(0., 0., 0., 0.42),
            below_title_bar: false,
            scrolls: false,
            on_dismiss: None,
            children: Vec::new(),
        }
    }

    /// A darker scrim for a lock that should read as "wait".
    pub(crate) fn heavy(mut self) -> Self {
        self.scrim = hsla(0., 0., 0., 0.62);
        self
    }

    /// A solid fill, for a takeover that hides the page behind it.
    pub(crate) fn opaque(mut self, color: Hsla) -> Self {
        self.scrim = color;
        self
    }

    /// Leave the window's title bar usable.
    pub(crate) fn below_title_bar(mut self) -> Self {
        self.below_title_bar = true;
        self
    }

    /// Scroll the content when it is taller than the window.
    pub(crate) fn scrolls(mut self) -> Self {
        self.scrolls = true;
        self
    }

    /// Called when the scrim itself (not the panel) is clicked.
    pub(crate) fn on_dismiss(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Box::new(f));
        self
    }
}

impl ParentElement for Overlay {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Overlay {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let dismiss = self.on_dismiss.map(std::rc::Rc::new);
        let panel = div()
            .id((self.id.clone(), "panel"))
            .on_click(|_, _, cx| cx.stop_propagation())
            .when(self.scrolls, |d| {
                d.max_h(window.viewport_size().height - TITLE_BAR_HEIGHT - px(32.))
                    .overflow_y_scroll()
            })
            .children(self.children);
        h_flex()
            .id(self.id)
            .absolute()
            .when_else(
                self.below_title_bar,
                |d| d.top(TITLE_BAR_HEIGHT).left_0().right_0().bottom_0(),
                |d| d.inset_0(),
            )
            .items_center()
            .justify_center()
            .occlude()
            .bg(self.scrim)
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .when_some(dismiss, |d, f| d.on_click(move |_, window, cx| f(window, cx)))
            .child(panel)
    }
}

/// The root of a page-replacing screen (the welcome wizard): opaque, full size,
/// with the app's text defaults. Its title bar comes from [`title_bar`].
pub(crate) fn takeover(id: impl Into<ElementId>, p: Palette) -> Stateful<Div> {
    v_flex()
        .id(id)
        .relative()
        .size_full()
        .overflow_hidden()
        .bg(p.background)
        .text_color(p.text)
        .text_size(px(14.))
}

pub(crate) fn title_bar(p: Palette) -> TitleBar {
    TitleBar::new().bg(p.chrome)
}
