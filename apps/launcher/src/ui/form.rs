//! Settings-style form pieces: titled sections of aligned rows, value pills,
//! selects, segmented choices and a side category list.

use std::rc::Rc;

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::theme::{Palette, palette};

const ROW_PAD_X: f32 = 20.;
const ROW_PAD_Y: f32 = 14.;
const SECTION_RADIUS: f32 = 8.;

#[derive(IntoElement)]
pub struct SettingsSection {
    title: SharedString,
    action: Option<AnyElement>,
    rows: Vec<AnyElement>,
}

impl SettingsSection {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            action: None,
            rows: Vec::new(),
        }
    }

    pub fn action(mut self, action: impl IntoElement) -> Self {
        self.action = Some(action.into_any_element());
        self
    }

    pub fn row(mut self, row: impl IntoElement) -> Self {
        self.rows.push(row.into_any_element());
        self
    }
}

impl RenderOnce for SettingsSection {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        v_flex()
            .w_full()
            .gap(px(10.))
            .child(
                h_flex()
                    .gap(px(12.))
                    .items_end()
                    .child(
                        v_flex().flex_1().min_w_0().gap(px(2.)).child(
                            div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(self.title),
                        ),
                    )
                    .children(self.action),
            )
            .child(
                v_flex()
                    .w_full()
                    .rounded(px(SECTION_RADIUS))
                    .border_1()
                    .border_color(p.border_variant)
                    .bg(p.surface)
                    .overflow_hidden()
                    .children(self.rows.into_iter().enumerate().map(|(i, row)| {
                        div()
                            .w_full()
                            .when(i > 0, |d| d.border_t_1().border_color(p.border_variant))
                            .child(row)
                    })),
            )
    }
}

#[derive(IntoElement)]
pub struct SettingRow {
    title: SharedString,
    description: Option<SharedString>,
    controls: Vec<AnyElement>,
    below: Option<AnyElement>,
}

impl SettingRow {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            controls: Vec::new(),
            below: None,
        }
    }

    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description = Some(text.into());
        self
    }

    pub fn control(mut self, control: impl IntoElement) -> Self {
        self.controls.push(control.into_any_element());
        self
    }

    pub fn below(mut self, below: impl IntoElement) -> Self {
        self.below = Some(below.into_any_element());
        self
    }
}

impl RenderOnce for SettingRow {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        v_flex()
            .w_full()
            .px(px(ROW_PAD_X))
            .py(px(ROW_PAD_Y))
            .gap(px(12.))
            .child(
                h_flex()
                    .w_full()
                    .gap(px(24.))
                    .items_center()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(3.))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(self.title),
                            )
                            .when_some(self.description, |d, text| {
                                d.child(
                                    div()
                                        .text_size(px(13.))
                                        .line_height(px(18.))
                                        .text_color(p.text_muted)
                                        .child(text),
                                )
                            }),
                    )
                    .when(!self.controls.is_empty(), |d| {
                        d.child(
                            h_flex()
                                .flex_shrink_0()
                                .gap(px(8.))
                                .items_center()
                                .children(self.controls),
                        )
                    }),
            )
            .children(self.below)
    }
}

#[derive(IntoElement)]
pub struct ValuePill {
    text: SharedString,
}

impl ValuePill {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for ValuePill {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        div()
            .px(px(10.))
            .py(px(3.))
            .rounded_full()
            .bg(p.elevated)
            .text_size(px(12.))
            .text_color(p.text_muted)
            .truncate()
            .child(self.text)
    }
}

#[derive(IntoElement)]
pub struct Notice {
    text: SharedString,
}

impl Notice {
    pub fn warning(text: impl Into<SharedString>) -> Self {
        Self { text: text.into() }
    }
}

impl RenderOnce for Notice {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = palette(cx);
        h_flex()
            .w_full()
            .px(px(ROW_PAD_X))
            .py(px(12.))
            .gap(px(10.))
            .items_start()
            .child(
                Icon::new(IconName::CircleAlert)
                    .size(px(16.))
                    .text_color(p.warning)
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .text_color(p.warning)
                    .child(self.text),
            )
    }
}

pub fn usage_bar(
    name: impl Into<SharedString>,
    size: impl Into<SharedString>,
    share: f32,
    p: Palette,
) -> Div {
    h_flex()
        .gap(px(12.))
        .w_full()
        .items_center()
        .child(
            div()
                .w(px(130.))
                .flex_shrink_0()
                .text_size(px(13.))
                .truncate()
                .child(name.into()),
        )
        .child(
            div()
                .flex_1()
                .h(px(6.))
                .rounded_full()
                .bg(p.border_variant)
                .child(div().h_full().w(relative(share)).rounded_full().bg(p.accent)),
        )
        .child(
            div()
                .w(px(76.))
                .flex_shrink_0()
                .text_size(px(13.))
                .text_color(p.text_muted)
                .text_right()
                .child(size.into()),
        )
}

pub type Pick = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
pub struct Choice {
    pub label: SharedString,
    pub selected: bool,
    pub pick: Pick,
}

impl Choice {
    pub fn new(
        label: impl Into<SharedString>,
        selected: bool,
        pick: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            selected,
            pick: Rc::new(pick),
        }
    }
}

pub fn select(id: impl Into<ElementId>, choices: Vec<Choice>) -> impl IntoElement {
    let current = choices
        .iter()
        .find(|c| c.selected)
        .map(|c| c.label.clone())
        .unwrap_or_default();
    Button::new(id)
        .outline()
        .tap_sm()
        .label(current)
        .dropdown_caret(true)
        .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
            menu = menu.min_w(px(180.));
            for choice in &choices {
                let pick = choice.pick.clone();
                menu = menu.item(
                    PopupMenuItem::new(choice.label.clone())
                        .checked(choice.selected)
                        .on_click(move |_, window, cx| pick(window, cx)),
                );
            }
            menu
        })
}

pub fn segmented(id: &'static str, choices: Vec<Choice>, p: Palette) -> Div {
    h_flex()
        .gap(px(2.))
        .p(px(2.))
        .rounded(px(8.))
        .bg(p.background)
        .border_1()
        .border_color(p.border_variant)
        .children(choices.into_iter().enumerate().map(|(i, choice)| {
            let pick = choice.pick;
            let button = Button::new(SharedString::from(format!("{id}-{i}")))
                .tap_sm()
                .label(choice.label)
                .on_click(move |_, window, cx| pick(window, cx));
            if choice.selected {
                button.primary()
            } else {
                button.ghost()
            }
        }))
}

pub fn side_nav_item(
    id: impl Into<ElementId>,
    icon: IconName,
    label: impl Into<SharedString>,
    active: bool,
    on_select: impl Fn(&mut Window, &mut App) + 'static,
    p: Palette,
) -> Stateful<Div> {
    let on_select = Rc::new(on_select);
    let (click, key) = (on_select.clone(), on_select);
    h_flex()
        .id(id)
        .h(px(36.))
        .px(px(12.))
        .gap(px(10.))
        .items_center()
        .rounded(px(6.))
        .border_1()
        .cursor_pointer()
        .text_size(px(14.))
        .tab_index(0)
        .focus_visible(move |s| s.border_color(p.accent_hover))
        .map(|d| {
            if active {
                d.bg(p.selected).border_color(p.accent).text_color(p.text)
            } else {
                d.border_color(transparent_black())
                    .text_color(p.text_muted)
                    .hover(move |s| s.bg(p.hover).text_color(p.text))
            }
        })
        .child(Icon::new(icon).size(px(16.)).text_color(if active {
            p.accent_hover
        } else {
            p.text_placeholder
        }))
        .child(div().flex_1().truncate().child(label.into()))
        .on_click(move |_, window, cx| click(window, cx))
        .on_key_down(move |event, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                key(window, cx);
            }
        })
}
