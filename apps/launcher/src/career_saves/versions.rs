use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Disableable, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::vault::restore::Running;
use swofty_core::vault::{self};
use swofty_i18n::{tr, trf};

use super::labels::{careers_held_line, short_block, steam_save_missing_text, version_label};
use super::list::{list_box, placeholder};
use super::state::{SavesView, Version};
use crate::format::{ago_since as ago, size};
use crate::pages::{GUTTER, label};
use crate::scale::{Hit, px};
use crate::shell::Shell;
use crate::theme::Palette;

pub(super) const WHEN_W: f32 = 104.;
pub(super) const SIZE_W: f32 = 64.;
pub(super) const ACTIONS_W: f32 = 164.;
pub(super) const PIN_W: f32 = 18.;

impl Shell {
    pub(super) fn versions_block(&self, view: &SavesView, p: Palette, cx: &mut Context<Self>) -> Div {
        let missing = self
            .saves
            .confirm
            .as_deref()
            .is_some_and(|c| !view.versions.iter().any(|v| v.snap.id == c));
        let mut rows: Vec<AnyElement> = match view.versions.is_empty() {
            true => vec![placeholder(
                match view.steam_missing {
                    true => steam_save_missing_text(),
                    false => tr("No versions yet."),
                },
                p,
            )],
            false => view
                .versions
                .iter()
                .map(|v| self.version_row(v, view, p, cx))
                .collect(),
        };
        if missing {
            rows.insert(
                0,
                placeholder(tr("The version to restore is not in this list."), p),
            );
        }
        v_flex()
            .gap(px(8.))
            .child(label(tr("VERSIONS"), p))
            .child(list_box(rows, p))
    }

    pub(super) fn version_row(
        &self,
        v: &Version,
        view: &SavesView,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let s = &v.snap;
        let id = s.id.clone();
        let pinned = !s.pinned.is_empty();
        let own_pin = s.pinned.iter().any(|x| x == "pinned");
        let confirming = self.saves.confirm.as_deref() == Some(s.id.as_str());
        let layout = view.layout();
        let running = Running {
            game: self.saves.game_running,
            steam: view.steam_running,
            ea_app: false,
        };
        let blocker = vault::restore::blocker(layout, running);
        let why = short_block(layout, running);
        let source = s.source.clone();
        let (restore_id, restore_source) = (id.clone(), source.clone());
        let (pin_id, pin_source) = (id.clone(), source);
        let ask_id = id.clone();
        let full = vault::human(s.at);
        let held = v
            .held
            .as_ref()
            .map(|h| careers_held_line(h.len(), v.newest.as_deref(), v.missing));
        v_flex()
            .w_full()
            .child(
                h_flex()
                    .h(px(44.))
                    .px(px(12.))
                    .gap(px(10.))
                    .items_center()
                    .child(div().w(px(PIN_W)).flex_shrink_0().when(pinned, |d| {
                        d.child(Icon::new(IconName::Star).size(px(13.)).text_color(p.accent))
                    }))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(version_label(s)),
                            )
                            .children(held.map(|h| {
                                div()
                                    .truncate()
                                    .text_size(px(12.))
                                    .text_color(p.text_placeholder)
                                    .child(h)
                            })),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("when-{id}")))
                            .w(px(WHEN_W))
                            .flex_shrink_0()
                            .truncate()
                            .text_size(px(13.))
                            .text_color(p.text_muted)
                            .child(ago(s.at as f64))
                            .tooltip(move |window, cx| Tooltip::new(full.clone()).build(window, cx)),
                    )
                    .child(
                        div()
                            .w(px(SIZE_W))
                            .flex_shrink_0()
                            .text_right()
                            .text_size(px(13.))
                            .text_color(p.text_muted)
                            .child(size(s.total())),
                    )
                    .child(
                        h_flex()
                            .w(px(ACTIONS_W))
                            .flex_shrink_0()
                            .gap(px(4.))
                            .justify_end()
                            .child(
                                Button::new(SharedString::from(format!("pin-{id}")))
                                    .ghost()
                                    .tap_sm()
                                    .icon(IconName::Star)
                                    .label(if own_pin { tr("Unpin") } else { tr("Pin") })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.pin_version(&pin_source, &pin_id, own_pin, cx)
                                    })),
                            )
                            .child(
                                Button::new(SharedString::from(format!("restore-{id}")))
                                    .outline()
                                    .tap_sm()
                                    .icon(IconName::Undo2)
                                    .label(tr("Restore"))
                                    .disabled(why.is_some())
                                    .when_some(why, |b, t| b.tooltip(t))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.saves.confirm =
                                            match this.saves.confirm.as_deref() == Some(ask_id.as_str()) {
                                                true => None,
                                                false => Some(ask_id.clone()),
                                            };
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .when(confirming, |d| {
                d.child(div().px(px(12.)).pb(px(12.)).child(self.restore_panel(
                    v,
                    layout,
                    blocker,
                    (restore_id, restore_source),
                    p,
                    cx,
                )))
            })
            .into_any_element()
    }

    pub(super) fn storage_line(&self, view: &SavesView, p: Palette, cx: &mut Context<Self>) -> Div {
        let dir = view.dir.clone();
        h_flex()
            .px(px(GUTTER))
            .py(px(8.))
            .gap(px(4.))
            .flex_shrink_0()
            .items_center()
            .border_t_1()
            .border_color(p.border_variant)
            .text_size(px(13.))
            .text_color(p.text_muted)
            .child(trf!(
                "Versions use {used} of {cap}",
                used = size(view.usage),
                cap = size(view.cap)
            ))
            .child("·")
            .child(
                Button::new("saves-storage-change")
                    .ghost()
                    .tap_sm()
                    .label(tr("Change"))
                    .on_click(cx.listener(|this, _, window, cx| this.cap_prompt(window, cx))),
            )
            .child("·")
            .child(
                Button::new("saves-storage-open")
                    .ghost()
                    .tap_sm()
                    .label(tr("Open folder"))
                    .on_click(cx.listener(move |_, _, _, cx| crate::reveal::show(&dir, cx))),
            )
    }
}
