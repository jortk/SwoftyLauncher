use crate::notices::{Notification, Notify};
use crate::theme::palette;
use gpui_kit::component::WindowExt;
use gpui_kit::component::button::ButtonVariant;
use gpui_kit::*;
use swofty_core::profile::VANILLA;
use swofty_core::saves::SavesWarning;
use swofty_i18n::{tr, trf};

use super::Shell;
use super::dialogs::saves_alert;

impl Shell {
    pub(crate) fn saves_warning(&self, id: &str, of: Option<SavesWarning>) -> Option<SavesWarning> {
        let careers =
            swofty_core::stdlib::careers::read(&swofty_core::stdlib::careers::data_path(&self.store.root));
        let name = self
            .library
            .addons
            .iter()
            .find(|a| a.manifest.id == id)
            .map_or_else(|| id.to_owned(), |a| a.manifest.name.clone());
        SavesWarning::with_careers(
            of,
            id,
            &name,
            &swofty_core::stdlib::careers::needing(&careers, id),
        )
    }

    pub(crate) fn turn_off_warning(&self, id: &str) -> Option<SavesWarning> {
        let of = self.library.registry.turn_off_warning(&self.library.profile, id);
        let on = self.library.profile.enabled().any(|e| e == id);
        match on {
            true => self.saves_warning(id, of),
            false => of,
        }
    }

    pub(crate) fn set_addon_enabled(
        &mut self,
        id: &str,
        on: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let registry = self.library.registry.clone();
        let warning = (!on).then(|| self.turn_off_warning(id)).flatten();
        let Some(warning) = warning else {
            self.update_profile(window, cx, |p| registry.set_enabled(p, id, on));
            return;
        };
        let shell = cx.entity().downgrade();
        let p = palette(cx);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shell, id) = (shell.clone(), warning.id.clone());
            saves_alert(alert, &warning, None, p)
                .ok_text(tr("Turn off anyway"))
                .on_ok(move |_, window, cx| {
                    shell
                        .update(cx, |s, cx| {
                            let registry = s.library.registry.clone();
                            s.update_profile(window, cx, |p| registry.set_enabled(p, &id, false));
                        })
                        .ok();
                    true
                })
        });
        cx.notify();
    }

    pub(crate) fn remove_addon(&mut self, id: &str, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let removes_plain = tr("Its files are deleted and it is taken out of every profile. \
                               You can install it again later.");
        let removes_db = tr(
            "Your DB Editor changes are deleted and it is taken out of every profile. \
                                  To keep them, use Share addon (.zip) in the DB Editor's Export menu first.",
        );
        let db_editor = self
            .library
            .addons
            .iter()
            .any(|a| a.manifest.id == id && a.source == crate::library::Source::DbEditor);
        let removes = if db_editor { removes_db } else { removes_plain };
        let shell = cx.entity().downgrade();
        let store = self.store.clone();
        let id = id.to_owned();
        let name = name.to_owned();
        let warning = self.saves_warning(
            &id,
            self.library
                .addons
                .iter()
                .find(|a| a.manifest.id == id)
                .and_then(|a| SavesWarning::of(&a.manifest)),
        );
        let p = palette(cx);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shell, store, id, name) = (shell.clone(), store.clone(), id.clone(), name.clone());
            let alert = match &warning {
                Some(w) => saves_alert(alert, w, Some(removes), p).ok_text(tr("Remove anyway")),
                None => alert
                    .title(trf!("Uninstall {name}?", name = name))
                    .description(removes)
                    .ok_text(tr("Uninstall")),
            };
            alert
                .ok_variant(ButtonVariant::Danger)
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    let (shell, store, id, name) = (shell.clone(), store.clone(), id.clone(), name.clone());
                    window
                        .spawn(cx, async move |cx| {
                            let removing = id.clone();
                            let removed = cx
                                .background_executor()
                                .spawn(async move { store.remove_addon(&removing) })
                                .await;
                            cx.update(|window, cx| match removed {
                                Ok(()) => {
                                    window.notify(
                                        Notification::success(trf!("Removed {name}", name = name)),
                                        cx,
                                    );
                                    shell
                                        .update(cx, |s, cx| {
                                            s.open_addon = None;
                                            s.reload_in_background(cx);
                                        })
                                        .ok();
                                }
                                Err(e) => {
                                    crate::app_log::action("remove addon").addon(&id).failed(&e);
                                    window.notify(Notification::error(e.to_string()), cx);
                                }
                            })
                            .ok();
                        })
                        .detach();
                    true
                })
        });
    }

    pub(crate) fn update_profile(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut swofty_core::profile::Profile) -> swofty_core::Result<()>,
    ) {
        if let Err(e) = self.store.update_profile(&self.library.profile.name, f) {
            crate::app_log::action("change the profile").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
        self.reload(cx);
    }

    pub(crate) fn switch_profile(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.use_profiles && name != self.library.profile.name && self.library.profile.name != VANILLA {
            return;
        }
        self.play_pending.cancel();
        if let Err(e) = self.store.set_active_profile(name) {
            crate::app_log::action("switch profile").failed(&e);
            window.notify(Notification::error(e.to_string()), cx);
        }
        self.reload_in_background(cx);
    }
}
