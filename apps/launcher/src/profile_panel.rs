use crate::library::ProfileItem;
use crate::pages::avatar;
use crate::scale::{Hit, px};
use crate::shell::{Page, PlayState, Shell};
use crate::theme::{Palette, ScrollArea};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_i18n::{tr, trf};

pub(crate) const WIDTH: f32 = 252.;
pub(crate) const RAIL: f32 = 44.;
const SIDEBAR_MIN_LAYOUT: f32 = 1000.;

impl Shell {
    pub(crate) fn profiles_sidebar(&mut self, p: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.use_profiles || !matches!(self.page, Page::Addons | Page::Careers) {
            return None;
        }
        Some(if self.profiles_shown() {
            self.profiles_open_panel(p, cx)
        } else {
            self.profiles_rail(p, cx)
        })
    }

    pub(crate) fn profiles_shown(&self) -> bool {
        self.profiles_open && self.layout_width / crate::scale::current() >= SIDEBAR_MIN_LAYOUT
    }

    pub(crate) fn profiles_width(&self) -> f32 {
        if !self.use_profiles {
            return 0.;
        }
        match (self.page, self.profiles_shown()) {
            (Page::Addons | Page::Careers, true) => WIDTH,
            (Page::Addons | Page::Careers, false) => RAIL,
            _ => 0.,
        }
    }

    fn profiles_rail(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("profiles-rail")
            .w(px(RAIL))
            .h_full()
            .flex_shrink_0()
            .items_center()
            .py(px(10.))
            .gap(px(10.))
            .border_r_1()
            .border_color(p.border_variant)
            .child(
                Button::new("profiles-expand")
                    .ghost()
                    .tap_sm()
                    .icon(IconName::ChevronRight)
                    .tooltip(tr("Show profiles"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.profiles_open = true;
                        cx.notify();
                    })),
            )
            .child(avatar(&self.library.profile.name, 24., p))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(p.text_placeholder)
                    .child(self.library.profiles.len().to_string()),
            )
            .into_any_element()
    }

    fn profiles_open_panel(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let deletable = self.library.profiles.iter().filter(|i| !i.vanilla()).count() > 1;
        let locked = matches!(
            self.play_state(),
            PlayState::Running
                | PlayState::Preparing
                | PlayState::Launching
                | PlayState::Restoring
                | PlayState::Building
        );
        let rows: Vec<AnyElement> = self
            .library
            .profiles
            .iter()
            .map(|item| self.profile_row(item, deletable, locked, p, cx))
            .collect();
        v_flex()
            .id("profiles-panel")
            .w(px(WIDTH))
            .h_full()
            .flex_shrink_0()
            .border_r_1()
            .border_color(p.border_variant)
            .child(
                h_flex()
                    .h(px(44.))
                    .px(px(12.))
                    .gap(px(4.))
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.text_muted)
                            .child(tr("PROFILES")),
                    )
                    .child(
                        Button::new("profile-new")
                            .ghost()
                            .tap_sm()
                            .icon(IconName::Plus)
                            .tooltip(tr("New profile"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.profile_prompt(None, false, window, cx)
                            })),
                    )
                    .child(
                        Button::new("profiles-collapse")
                            .ghost()
                            .tap_sm()
                            .icon(IconName::ChevronLeft)
                            .tooltip(tr("Hide profiles"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.profiles_open = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .scroll_y("profiles-list")
                    .pl(px(8.))
                    .pr(px(18.))
                    .pb(px(16.))
                    .gap(px(2.))
                    .children(rows),
            )
            .into_any_element()
    }

    fn profile_row(
        &self,
        item: &ProfileItem,
        deletable: bool,
        locked: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = item.name.as_str();
        let vanilla = item.vanilla();
        let active = name == self.library.profile.name;
        let on = if active {
            self.library.listed().filter(|a| a.enabled).count()
        } else {
            self.library
                .profiles
                .iter()
                .find(|i| i.name == name)
                .map_or(0, |i| i.addons.len())
        };
        let total = self.library.listed().count();
        let shell = cx.entity().downgrade();
        let menu_name = name.to_owned();
        let pick = name.to_owned();
        let head = h_flex()
            .id(SharedString::from(format!("profile-row-{name}")))
            .h(px(40.))
            .pl(px(8.))
            .pr(px(2.))
            .gap(px(8.))
            .items_center()
            .rounded(px(8.))
            .border_1()
            .map(|d| {
                if active {
                    d.bg(p.selected).border_color(p.accent)
                } else {
                    d.border_color(transparent_black())
                        .cursor_pointer()
                        .hover(move |s| s.bg(p.hover))
                }
            })
            .child(avatar(name, 24., p))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(name.to_owned()),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.text_muted)
                            .child(if vanilla {
                                tr("No addons").to_owned()
                            } else {
                                trf!("{on} of {total} on", on = on, total = total)
                            }),
                    ),
            )
            .when(active, |d| {
                d.child(div().size(px(7.)).rounded_full().bg(p.accent).mr(px(2.)))
            })
            .child(
                Button::new(SharedString::from(format!("profile-menu-{name}")))
                    .ghost()
                    .tap_xs()
                    .icon(IconName::Ellipsis)
                    .dropdown_menu_with_anchor(Anchor::TopRight, move |menu, _, _| {
                        let name = menu_name.clone();
                        let mut menu = menu.min_w(px(180.));
                        let (s, n) = (shell.clone(), name.clone());
                        menu = menu.item(
                            PopupMenuItem::new(tr("Use this profile"))
                                .icon(IconName::ArrowRightLeft)
                                .disabled(active || locked)
                                .on_click(move |_, window, cx| {
                                    s.update(cx, |s, cx| s.switch_profile(&n, window, cx)).ok();
                                }),
                        );
                        if !vanilla {
                            let (s, n) = (shell.clone(), name.clone());
                            menu =
                                menu.item(PopupMenuItem::new(tr("Rename")).icon(IconName::Pencil).on_click(
                                    move |_, window, cx| {
                                        s.update(cx, |s, cx| {
                                            s.profile_prompt(Some(n.clone()), true, window, cx)
                                        })
                                        .ok();
                                    },
                                ));
                            let (s, n) = (shell.clone(), name.clone());
                            menu =
                                menu.item(PopupMenuItem::new(tr("Duplicate")).icon(IconName::Copy).on_click(
                                    move |_, window, cx| {
                                        s.update(cx, |s, cx| {
                                            s.profile_prompt(Some(n.clone()), false, window, cx)
                                        })
                                        .ok();
                                    },
                                ));
                            let (s, n) = (shell.clone(), name.clone());
                            menu = menu.item(
                                PopupMenuItem::new(tr("Delete"))
                                    .icon(IconName::Trash)
                                    .disabled(!deletable)
                                    .on_click(move |_, window, cx| {
                                        s.update(cx, |s, cx| s.confirm_delete_profile(&n, window, cx))
                                            .ok();
                                    }),
                            );
                        }
                        menu
                    }),
            )
            .when(!active && !locked, |d| {
                d.on_click(cx.listener(move |this, _, window, cx| this.switch_profile(&pick, window, cx)))
            });
        v_flex()
            .flex_shrink_0()
            .gap(px(2.))
            .child(head)
            .into_any_element()
    }
}
