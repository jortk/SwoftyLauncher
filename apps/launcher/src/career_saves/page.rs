use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{Disableable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::profile::VANILLA;
use swofty_i18n::{tr, trf};

use super::labels::{close_game_first, empty_save, steam_save_missing_text};
use super::state::SavesView;
use crate::pages::GUTTER;
use crate::scale::{Hit, px};
use crate::shell::Shell;
use crate::theme::{Palette, ScrollArea};
use crate::ui::{Callout, EmptyState};

fn notice(callout: Callout) -> Div {
    div().mx(px(GUTTER)).mb(px(8.)).child(callout)
}

impl Shell {
    pub(crate) fn careers_page(&mut self, p: Palette, _wide: bool, cx: &mut Context<Self>) -> AnyElement {
        let banner = self.saves_banner(true, p, cx);
        let current = self.library.profile.name.as_str();
        let loading = self.saves.loading && self.saves.view.as_ref().is_none_or(|v| v.profile != current);
        let Some(view) = self.saves.view.as_ref().filter(|_| !loading) else {
            let empty = match loading {
                true => div()
                    .px(px(GUTTER))
                    .text_size(px(13.))
                    .text_color(p.text_muted)
                    .child(tr("Loading saves…"))
                    .into_any_element(),
                false => EmptyState::new(IconName::Save, tr("No F1 25 save found yet.")).into_any_element(),
            };
            return v_flex()
                .size_full()
                .child(self.header(tr("Saves"), String::new(), div(), p))
                .children(banner)
                .child(empty)
                .into_any_element();
        };
        let title = match (view.vanilla(), self.use_profiles) {
            (true, _) => tr("Vanilla saves").to_owned(),
            (false, true) => trf!("{profile} saves", profile = view.profile),
            (false, false) => tr("Saves").to_owned(),
        };
        let subtitle = match (view.vanilla(), self.use_profiles) {
            (true, _) => tr("Plays on your Steam save").to_owned(),
            (false, true) => tr("This profile's own save").to_owned(),
            (false, false) => tr("Launcher saves").to_owned(),
        };
        let actions = self.saves_actions(view, cx);
        let careers = self.careers_block(view, p, cx);
        let versions = self.versions_block(view, p, cx);
        let footer = self.storage_line(view, p, cx);
        let empty = self.empty_save_notice(view, p, cx);
        let missing = self.steam_missing_notice(view, p, cx);
        let warning = self.steam_warning_notice(view, p, cx);
        v_flex()
            .size_full()
            .child(self.header(title, subtitle, actions, p))
            .children(banner)
            .children(empty)
            .children(missing)
            .children(warning)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .scroll_y("career-saves")
                    .px(px(GUTTER))
                    .pt(px(4.))
                    .pb(px(16.))
                    .child(v_flex().gap(px(22.)).child(careers).child(versions)),
            )
            .child(footer)
            .into_any_element()
    }

    pub(super) fn empty_save_notice(
        &self,
        view: &SavesView,
        _: Palette,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        if !view.awaiting_import {
            return None;
        }
        let steam = view.steam.clone()?;
        let name = view.profile.clone();
        let blocked = self.saves.game_running;
        let import = Button::new("saves-empty-import")
            .tap_sm()
            .icon(IconName::Download)
            .label(tr("Import from Steam"))
            .disabled(blocked)
            .when(blocked, |b| b.tooltip(close_game_first()))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.ask_copy(steam.clone(), VANILLA.to_owned(), name.clone(), window, cx);
            }));
        Some(notice(Callout::info(empty_save()).actions(import)))
    }

    pub(super) fn steam_warning_notice(
        &self,
        view: &SavesView,
        _: Palette,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        if !view.steam_warning {
            return None;
        }
        let turn_on = !view.has_save;
        let actions = h_flex().gap(px(8.)).flex_wrap().when(turn_on, |row| {
            row.child(
                Button::new("saves-warning-launcher-saves")
                    .tap_sm()
                    .label(tr("Use launcher saves"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.set_launcher_saves(true, window, cx);
                        this.refresh_saves_in_background(cx);
                    })),
            )
        });
        let callout = Callout::warning(swofty_core::vault::guard::STEAM_SAVE_WARNING);
        Some(notice(match turn_on {
            true => callout.actions(actions),
            false => callout,
        }))
    }

    pub(super) fn steam_missing_notice(
        &self,
        view: &SavesView,
        _: Palette,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        if !view.steam_missing {
            return None;
        }
        let blocked = self.saves.game_running;
        let name = view.profile.clone();
        let on = self.library.profile.launcher_saves;
        let actions = h_flex()
            .gap(px(8.))
            .flex_wrap()
            .when(on, |row| {
                row.child(
                    Button::new("saves-missing-steam")
                        .tap_sm()
                        .label(tr("Play with your Steam saves"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.set_launcher_saves(false, window, cx);
                            this.refresh_saves_in_background(cx);
                        })),
                )
            })
            .child(
                Button::new("saves-missing-retry")
                    .tap_sm()
                    .icon(IconName::Download)
                    .label(tr("Import from Steam"))
                    .disabled(blocked)
                    .when(blocked, |b| b.tooltip(close_game_first()))
                    .on_click(cx.listener(|this, _, window, cx| this.retry_steam_import(window, cx))),
            )
            .child(
                Button::new("saves-missing-pick")
                    .tap_sm()
                    .outline()
                    .icon(IconName::Folder)
                    .label(tr("Choose the folder…"))
                    .disabled(blocked)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.pick_steam_save(name.clone(), window, cx);
                    })),
            );
        Some(notice(Callout::info(steam_save_missing_text()).actions(actions)))
    }

    pub(super) fn saves_actions(&self, view: &SavesView, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let game = self.saves.game_running;
        let steam = view.steam.clone();
        let name = view.profile.clone();
        let vanilla = view.vanilla();
        let why_import = match () {
            _ if vanilla => Some(tr("Vanilla uses the Steam save")),
            _ if steam.is_none() => Some(tr("No Steam save found")),
            _ if game => Some(close_game_first()),
            _ => None,
        };
        let why_export = match () {
            _ if vanilla => Some(tr("Vanilla uses the Steam save")),
            _ if steam.is_none() => Some(tr("No Steam save found")),
            _ if !view.has_save => Some(tr("Nothing to export yet")),
            _ if game => Some(close_game_first()),
            _ if view.steam_running => Some(tr("Close Steam first")),
            _ => None,
        };
        let why_copy = match () {
            _ if view.others.is_empty() => Some(tr("No other profiles")),
            _ if view.shared_by_every_profile() => Some(tr("Every profile plays on the EA copy's one save")),
            _ if !view.has_save => Some(tr("Nothing to copy yet")),
            _ if game => Some(close_game_first()),
            _ => None,
        };
        let (from, from_name) = (view.loc.clone(), name.clone());
        let others = view.others.clone();
        let shell = cx.entity().downgrade();
        let copy = Button::new("saves-copy")
            .outline()
            .tap_sm()
            .icon(IconName::Copy)
            .label(tr("Copy careers to…"))
            .dropdown_caret(true)
            .disabled(why_copy.is_some())
            .when_some(why_copy, |b, t| b.tooltip(t))
            .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                for target in &others {
                    let (shell, from, from_name, to) =
                        (shell.clone(), from.clone(), from_name.clone(), target.clone());
                    menu = menu.item(PopupMenuItem::new(target.clone()).on_click(move |_, window, cx| {
                        let Some(from) = from.clone() else { return };
                        let (from_name, to) = (from_name.clone(), to.clone());
                        shell
                            .update(cx, |s, cx| s.ask_copy(from, from_name, to, window, cx))
                            .ok();
                    }));
                }
                menu
            });
        let import_name = name.clone();
        let import = Button::new("saves-import")
            .outline()
            .tap_sm()
            .icon(IconName::Download)
            .label(tr("Import from Steam"))
            .disabled(why_import.is_some())
            .when_some(why_import, |b, t| b.tooltip(t))
            .on_click(cx.listener(move |this, _, window, cx| {
                if let Some(steam) = steam.clone() {
                    this.ask_copy(steam, VANILLA.to_owned(), import_name.clone(), window, cx);
                }
            }));
        let export = Button::new("saves-export")
            .outline()
            .tap_sm()
            .icon(IconName::Upload)
            .label(tr("Export to Steam"))
            .disabled(why_export.is_some())
            .when_some(why_export, |b, t| b.tooltip(t))
            .on_click(cx.listener(move |this, _, window, cx| this.ask_export(name.clone(), window, cx)));
        h_flex()
            .gap(px(8.))
            .flex_shrink_0()
            .when(self.use_profiles, |row| row.child(copy))
            .child(import)
            .child(export)
    }
}
