use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{Disableable, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::shaders::{ConfigEntry, ConfigRef};
use swofty_i18n::tr;

use super::{ShadersView, list_width};
use crate::pages::{label, tag};
use crate::theme::{Palette, ScrollArea};
use crate::ui::selectable_row;

#[derive(Default)]
pub(super) struct ListState {
    pub rename: Option<Rename>,
    pub pending_rename: Option<ConfigRef>,
    pub rename_new: bool,
}

pub(super) struct Rename {
    pub target: ConfigRef,
    pub input: Entity<InputState>,
    pub _events: Subscription,
}

impl ShadersView {
    pub(super) fn list_pane(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let locked = self.forced.is_some();
        let yours: Vec<ConfigEntry> = self.entries.iter().filter(|e| e.editable).cloned().collect();
        let addons: Vec<ConfigEntry> = self.entries.iter().filter(|e| !e.editable).cloned().collect();
        let off_on = self.active.is_none() && self.forced.is_none();
        let off = selectable_row("shaders-off", off_on, p).child(
            h_flex()
                .flex_1()
                .gap(px(10.))
                .items_center()
                .child(self.state_dot(off_on, p))
                .child(
                    v_flex()
                        .flex_1()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(tr("Off")),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.text_placeholder)
                                .child(tr("Play without shaders")),
                        ),
                ),
        );
        let off = if locked {
            off.opacity(0.5)
        } else {
            off.cursor_pointer()
                .hover(move |s| s.bg(p.hover))
                .on_click(cx.listener(|this, _, _, cx| this.use_config(None, cx)))
        };
        let mut body = v_flex().gap(px(2.)).p(px(6.)).child(off);
        body = body.child(self.group_label(tr("YOUR CONFIGS"), yours.len(), p));
        if yours.is_empty() {
            body = body.child(self.hint(tr("Nothing here yet. Start from the template, or import one."), p));
        }
        for entry in &yours {
            body = body.child(self.config_row(entry, locked, p, cx));
        }
        body = body.child(self.group_label(tr("FROM ADDONS"), addons.len(), p));
        if addons.is_empty() {
            body = body.child(self.hint(tr("Addons that bundle shader configs show up here."), p));
        }
        for entry in &addons {
            body = body.child(self.config_row(entry, locked, p, cx));
        }
        let shaders = cx.entity().downgrade();
        let import = Button::new("shaders-import")
            .outline()
            .tap_sm()
            .icon(IconName::FileUp)
            .label(tr("Import"))
            .dropdown_caret(true)
            .disabled(self.busy)
            .dropdown_menu_with_anchor(Anchor::BottomRight, move |menu, _, _| {
                let (a, b) = (shaders.clone(), shaders.clone());
                menu.item(
                    PopupMenuItem::new(tr("From a file…")).on_click(move |_, window, cx| {
                        a.update(cx, |v, cx| v.import_file(window, cx)).ok();
                    }),
                )
                .item(
                    PopupMenuItem::new(tr("From the clipboard")).on_click(move |_, _, cx| {
                        b.update(cx, |v, cx| v.import_clipboard(cx)).ok();
                    }),
                )
            });
        v_flex()
            .w(px(list_width()))
            .flex_shrink_0()
            .h_full()
            .rounded(px(6.))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(v_flex().size_full().scroll_y("shaders-list").child(body)),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .p(px(10.))
                    .gap(px(8.))
                    .border_t_1()
                    .border_color(p.border_variant)
                    .child(
                        Button::new("shaders-new")
                            .tap_sm()
                            .icon(IconName::Plus)
                            .label(tr("New config"))
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| this.new_config(window, cx))),
                    )
                    .child(import),
            )
            .into_any_element()
    }

    fn group_label(&self, title: &'static str, count: usize, p: Palette) -> Div {
        h_flex()
            .px(px(8.))
            .pt(px(14.))
            .pb(px(4.))
            .gap(px(6.))
            .child(label(title, p))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(p.text_placeholder)
                    .child(count.to_string()),
            )
    }

    fn hint(&self, text: &'static str, p: Palette) -> Div {
        div()
            .px(px(8.))
            .py(px(6.))
            .text_size(px(12.))
            .text_color(p.text_placeholder)
            .child(text)
    }

    fn state_dot(&self, on: bool, p: Palette) -> Div {
        div()
            .size(px(18.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .child(if on {
                Icon::new(IconName::CircleCheck)
                    .size(px(16.))
                    .text_color(p.success)
            } else {
                Icon::new(IconName::Circle).size(px(14.)).text_color(p.border)
            })
    }

    fn config_row(
        &self,
        entry: &ConfigEntry,
        locked: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let r = entry.r#ref.clone();
        let is_active = self.running_ref() == Some(&r) && (self.forced.is_some() || self.active.is_some());
        let selected = self.selected.as_ref() == Some(&r);
        let id = SharedString::from(format!("shaders-row-{r}"));
        let pick = r.clone();
        let dot_ref = r.clone();
        let dot = div()
            .id(SharedString::from(format!("shaders-use-{r}")))
            .child(self.state_dot(is_active, p))
            .when(!locked, |d| {
                d.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.use_config(Some(dot_ref.clone()), cx)
                }))
            });
        selectable_row(id, selected, p)
            .cursor_pointer()
            .hover(move |s| if selected { s } else { s.bg(p.hover) })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.selected = Some(pick.clone());
                cx.notify();
            }))
            .child(dot)
            .child(self.config_name(entry, p))
            .child(self.config_menu(entry, locked, cx))
            .into_any_element()
    }

    fn config_name(&self, entry: &ConfigEntry, p: Palette) -> AnyElement {
        let renaming = self.list.rename.as_ref().filter(|n| n.target == entry.r#ref);
        match renaming {
            Some(n) => div()
                .flex_1()
                .min_w_0()
                .child(Input::new(&n.input).tap_sm())
                .into_any_element(),
            None => v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .truncate()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(entry.name.clone()),
                )
                .child(div().flex().child(tag(
                    if entry.editable {
                        tr("Yours").to_owned()
                    } else {
                        entry.source.clone()
                    },
                    p,
                )))
                .into_any_element(),
        }
    }

    fn config_menu(&self, entry: &ConfigEntry, locked: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let r = entry.r#ref.clone();
        let editable = entry.editable;
        let shaders = cx.entity().downgrade();
        Button::new(SharedString::from(format!("shaders-more-{r}")))
            .ghost()
            .tap_sm()
            .icon(IconName::Ellipsis)
            .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                let act = |f: fn(&mut ShadersView, ConfigRef, &mut Window, &mut Context<ShadersView>)| {
                    let (shaders, r) = (shaders.clone(), r.clone());
                    move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                        let r = r.clone();
                        shaders.update(cx, |v, cx| f(v, r, window, cx)).ok();
                    }
                };
                menu = menu.item(
                    PopupMenuItem::new(tr("Use"))
                        .disabled(locked)
                        .on_click(act(|v, r, _, cx| v.use_config(Some(r), cx))),
                );
                if editable {
                    menu.item(PopupMenuItem::new(tr("Rename")).on_click(act(ShadersView::begin_rename)))
                        .item(
                            PopupMenuItem::new(tr("Duplicate")).on_click(act(ShadersView::copy_to_personal)),
                        )
                        .item(PopupMenuItem::new(tr("Export…")).on_click(act(ShadersView::export)))
                        .item(PopupMenuItem::new(tr("Delete…")).on_click(act(ShadersView::ask_delete)))
                } else {
                    menu.item(
                        PopupMenuItem::new(tr("Copy to personal"))
                            .on_click(act(ShadersView::copy_to_personal)),
                    )
                    .item(PopupMenuItem::new(tr("Export…")).on_click(act(ShadersView::export)))
                }
            })
    }
}
