use super::select::{Bulk, Pick, Plan, plan};
use crate::library::Source;
use crate::notices::{Notification, Notify};
use crate::scale::{Hit, px};
use crate::shell::Shell;
use crate::theme::Palette;
use gpui_kit::assets::IconName;
use gpui_kit::component::WindowExt;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::h_flex;
use gpui_kit::*;
use swofty_i18n::{tr, trf, trn};

impl Shell {
    fn picks(&self) -> Vec<Pick> {
        self.addons
            .selection
            .ids()
            .iter()
            .filter_map(|id| self.library.listed().find(|m| &m.manifest.id == id))
            .map(|m| Pick {
                id: m.manifest.id.clone(),
                locked: m.locked,
                builtin: m.source == Source::Builtin,
                shader_only: m.manifest.is_shader_only(),
                warns_on_off: self.turn_off_warning(&m.manifest.id).is_some(),
            })
            .collect()
    }

    pub(super) fn bulk_bar(&self, p: Palette, cx: &mut Context<Self>) -> Option<Div> {
        let count = self.addons.selection.len();
        (count > 0 && !self.vanilla()).then(|| {
            h_flex()
                .flex_shrink_0()
                .h(px(40.))
                .px(px(12.))
                .gap(px(8.))
                .rounded(px(8.))
                .border_1()
                .border_color(p.border)
                .bg(p.selected)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(trn(count, "{count} addon selected", "{count} addons selected")),
                )
                .child(
                    Button::new("bulk-on")
                        .outline()
                        .tap_sm()
                        .icon(IconName::Power)
                        .label(tr("Turn on"))
                        .on_click(cx.listener(|this, _, window, cx| this.bulk_apply(Bulk::On, window, cx))),
                )
                .child(
                    Button::new("bulk-off")
                        .outline()
                        .tap_sm()
                        .icon(IconName::PowerOff)
                        .label(tr("Turn off"))
                        .on_click(cx.listener(|this, _, window, cx| this.bulk_apply(Bulk::Off, window, cx))),
                )
                .child(
                    Button::new("bulk-uninstall")
                        .outline()
                        .tap_sm()
                        .icon(IconName::Trash)
                        .label(tr("Uninstall"))
                        .text_color(p.error)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.bulk_apply(Bulk::Uninstall, window, cx)),
                        ),
                )
                .child(
                    Button::new("bulk-clear")
                        .ghost()
                        .tap_sm()
                        .icon(IconName::X)
                        .label(tr("Clear"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.addons.clear_selection();
                            cx.notify();
                        })),
                )
        })
    }

    pub(super) fn bulk_apply(&mut self, action: Bulk, window: &mut Window, cx: &mut Context<Self>) {
        let plan = plan(action, &self.picks());
        self.addons.selection.clear();
        cx.notify();
        match action {
            Bulk::Uninstall => self.bulk_uninstall(plan, window, cx),
            Bulk::On | Bulk::Off => self.bulk_switch(action == Bulk::On, plan, window, cx),
        }
    }

    fn bulk_switch(&mut self, on: bool, plan: Plan, window: &mut Window, cx: &mut Context<Self>) {
        if plan.skipped > 0 {
            window.notify(
                Notification::info(trn(
                    plan.skipped,
                    "Skipped {count} addon that can't be switched",
                    "Skipped {count} addons that can't be switched",
                )),
                cx,
            );
        }
        if !plan.warned.is_empty() {
            let kept: Vec<String> = plan.warned.iter().map(|id| self.addon_name(id)).collect();
            window.notify(
                Notification::info(trf!(
                    "Left on: {kept}. Turning it off warns about your saves, so use its own switch.",
                    kept = kept.join(", ")
                )),
                cx,
            );
        }
        if plan.apply.is_empty() {
            return;
        }
        let registry = self.library.registry.clone();
        let ids = plan.apply;
        let count = ids.len();
        self.update_profile(window, cx, |p| {
            ids.iter().try_for_each(|id| registry.set_enabled(p, id, on))
        });
        let text = match on {
            true => trn(count, "Turned on {count} addon", "Turned on {count} addons"),
            false => trn(count, "Turned off {count} addon", "Turned off {count} addons"),
        };
        window.notify(Notification::success(text), cx);
    }

    fn bulk_uninstall(&mut self, plan: Plan, window: &mut Window, cx: &mut Context<Self>) {
        if plan.apply.is_empty() {
            if plan.skipped > 0 {
                window.notify(Notification::info(skipped_text(plan.skipped)), cx);
            }
            return;
        }
        let ids = plan.apply;
        let skipped = plan.skipped;
        let db_editor = ids.iter().any(|id| {
            self.library
                .addons
                .iter()
                .any(|a| &a.manifest.id == id && a.source == Source::DbEditor)
        });
        let careers: Vec<String> = ids
            .iter()
            .filter(|id| self.saves_warning(id, None).is_some())
            .map(|id| self.addon_name(id))
            .collect();
        let mut description = tr(
            "Their files are deleted and they are taken out of every profile. \
                                  You can install them again later.",
        )
        .to_owned();
        if db_editor {
            description.push(' ');
            description.push_str(tr(
                "DB Editor addons lose their changes. To keep them, use Share addon (.zip) in the DB Editor's \
                 Export menu first.",
            ));
        }
        if !careers.is_empty() {
            description.push(' ');
            description.push_str(&trf!(
                "Your saved careers may not load without {names}.",
                names = careers.join(", ")
            ));
        }
        let title = trn(ids.len(), "Uninstall {count} addon?", "Uninstall {count} addons?");
        let shell = cx.entity().downgrade();
        let store = self.store.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shell, store, ids) = (shell.clone(), store.clone(), ids.clone());
            alert
                .title(title.clone())
                .description(description.clone())
                .ok_text(tr("Uninstall"))
                .ok_variant(ButtonVariant::Danger)
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    let (shell, store, ids) = (shell.clone(), store.clone(), ids.clone());
                    window
                        .spawn(cx, async move |cx| {
                            let results = cx
                                .background_executor()
                                .spawn(async move {
                                    ids.into_iter()
                                        .map(|id| {
                                            let r = store.remove_addon(&id);
                                            (id, r)
                                        })
                                        .collect::<Vec<_>>()
                                })
                                .await;
                            cx.update(|window, cx| {
                                let mut removed = 0;
                                for (id, r) in results {
                                    match r {
                                        Ok(()) => removed += 1,
                                        Err(e) => {
                                            crate::app_log::action("remove addon").addon(&id).failed(&e);
                                            window.notify(Notification::error(e.to_string()), cx);
                                        }
                                    }
                                }
                                if removed > 0 {
                                    let text =
                                        trn(removed, "Removed {count} addon", "Removed {count} addons");
                                    window.notify(Notification::success(text), cx);
                                }
                                if skipped > 0 {
                                    window.notify(Notification::info(skipped_text(skipped)), cx);
                                }
                                shell
                                    .update(cx, |s, cx| {
                                        s.open_addon = None;
                                        s.reload_in_background(cx);
                                    })
                                    .ok();
                            })
                            .ok();
                        })
                        .detach();
                    true
                })
        });
    }
}

fn skipped_text(n: usize) -> String {
    trn(
        n,
        "Skipped {count} built-in addon that can't be uninstalled",
        "Skipped {count} built-in addons that can't be uninstalled",
    )
}
