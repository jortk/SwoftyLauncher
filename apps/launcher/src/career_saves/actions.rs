use crate::notices::{Notification, Notify};
use gpui_kit::component::WindowExt;
use gpui_kit::*;
use swofty_core::launcher_saves;
use swofty_core::profile::VANILLA;
use swofty_core::vault::restore::Running;
use swofty_core::vault::{self, Location, Vault};
use swofty_i18n::{tr, trf};

use super::labels::{missing_warning, notice_body, notice_title};
use super::steam::steam_save_in;
use crate::format::ago_since as ago;
use crate::scale::px;
use crate::shell::{Prompt, Shell};

impl Shell {
    pub(crate) fn show_saves_notice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shell = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let shell = shell.clone();
            alert
                .width(px(480.))
                .title(notice_title())
                .description(notice_body())
                .ok_text(tr("Show me"))
                .cancel_text(tr("OK"))
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    shell.update(cx, |s, cx| s.open_saves(None, cx)).ok();
                    true
                })
        });
    }

    pub(super) fn ask_copy(
        &mut self,
        from: Location,
        from_name: String,
        to: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let shell = cx.entity().downgrade();
        let text = trf!(
            "{from} replaces the save of {to}. Its current save is kept as a version.",
            from = match from_name == VANILLA {
                true => tr("Your Steam save").to_owned(),
                false => from_name.clone(),
            },
            to = to
        );
        let steam = from_name == VANILLA;
        let title = match steam {
            true => trf!("Import from Steam into {profile}?", profile = to),
            false => trf!("Copy careers to {profile}?", profile = to),
        };
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shell, from, to, from_name) = (shell.clone(), from.clone(), to.clone(), from_name.clone());
            alert
                .width(px(480.))
                .title(title.clone())
                .description(text.clone())
                .ok_text(if steam { tr("Import") } else { tr("Copy") })
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    shell
                        .update(cx, |s, cx| s.copy_save(&from, &from_name, &to, window, cx))
                        .ok();
                    true
                })
        });
    }

    pub(super) fn copy_save(
        &mut self,
        from: &Location,
        from_name: &str,
        to: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let root = self.store.root.clone();
        let done = launcher_saves::copy_to(&root, from, to, Running::now(), vault::now());
        let note = match done {
            Ok((files, bytes, _)) => {
                crate::app_log::note(&format!(
                    "saves: copied {files} file(s), {bytes} bytes, from {from_name} into the launcher saves of profile {to}"
                ));
                Notification::success(trf!(
                    "Copied the save of {from} to {to}.",
                    from = from_name,
                    to = to
                ))
            }
            Err(e) => {
                crate::app_log::action("copy a save").failed(&e);
                Notification::warning(trf!("The save was not copied: {error}", error = e))
            }
        };
        window.notify(note, cx);
        self.refresh_saves_in_background(cx);
        cx.notify();
    }

    pub(super) fn ask_export(&mut self, profile: String, window: &mut Window, cx: &mut Context<Self>) {
        let shell = cx.entity().downgrade();
        let text = trf!(
            "The save of {profile} replaces your Steam save. Your Steam save is kept as a version.",
            profile = profile
        );
        let title = trf!("Export {profile} to Steam?", profile = profile);
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shell, profile) = (shell.clone(), profile.clone());
            alert
                .width(px(480.))
                .title(title.clone())
                .description(text.clone())
                .ok_text(tr("Export"))
                .show_cancel(true)
                .on_ok(move |_, window, cx| {
                    shell.update(cx, |s, cx| s.export_save(&profile, window, cx)).ok();
                    true
                })
        });
    }

    pub(super) fn export_save(&mut self, profile: &str, window: &mut Window, cx: &mut Context<Self>) {
        let root = self.store.root.clone();
        let done = match Location::active(&root).filter(|l| l.layout == vault::Layout::Steam) {
            Some(steam) => launcher_saves::export(&root, profile, &steam, Running::now(), vault::now())
                .map_err(|e| e.to_string()),
            None => Err(tr("no Steam save folder of F1 25 found").to_owned()),
        };
        let note = match done {
            Ok(r) => {
                crate::app_log::note(&r.log_line());
                Notification::success(trf!(
                    "Exported the save of {profile} to Steam.",
                    profile = profile
                ))
            }
            Err(e) => {
                crate::app_log::action("export a save").failed(&e);
                Notification::warning(trf!("The save was not exported: {error}", error = e))
            }
        };
        window.notify(note, cx);
        self.refresh_saves_in_background(cx);
        cx.notify();
    }

    pub(crate) fn restore_version(
        &mut self,
        source: &str,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let root = self.store.root.clone();
        let when = Vault::open(&root, source)
            .and_then(|v| v.find(name))
            .ok()
            .map(|s| s.at);
        let flagged = Vault::open(&root, source)
            .and_then(|v| Ok(v.index_of(&v.find(name)?)))
            .ok()
            .flatten()
            .map(|i| vault::careers::missing_briefs(&i))
            .unwrap_or_default();
        let restored = Vault::open(&root, source).and_then(|v| {
            let snap = v.find(name)?;
            let loc = Location::of_source(&root, source)
                .or_else(|| Location::active(&root).filter(|l| l.source == source))
                .unwrap_or(Location {
                    source: snap.source.clone(),
                    layout: snap.layout,
                    dir: snap.save_dir.clone(),
                });
            vault::restore::restore(&v, name, &loc, vault::restore::Running::now(), vault::now())
        });
        let note = match restored {
            Ok(r) => {
                crate::app_log::note(&r.log_line());
                self.saves.confirm = None;
                let ok = match (when, &r.before) {
                    (Some(at), Some(_)) => trf!(
                        "Restored your save from {when}. The save it replaced is kept as a version.",
                        when = ago(at as f64)
                    ),
                    (Some(at), None) => trf!("Restored your save from {when}.", when = ago(at as f64)),
                    (None, _) => tr("Restored your save.").to_owned(),
                };
                match flagged.is_empty() {
                    true => Notification::success(ok),
                    false => Notification::warning(format!("{ok} {}", missing_warning(&flagged))),
                }
            }
            Err(e) => {
                crate::app_log::action("restore a save").failed(&e);
                Notification::warning(trf!("The save was not restored: {error}", error = e))
            }
        };
        window.notify(note, cx);
        self.refresh_saves_in_background(cx);
        cx.notify();
    }

    pub(super) fn pin_version(&mut self, source: &str, id: &str, pinned: bool, cx: &mut Context<Self>) {
        let root = self.store.root.clone();
        let done = Vault::open(&root, source).and_then(|v| match pinned {
            true => v.unpin_why(id, "pinned", vault::now()),
            false => v.pin(id, "pinned", vault::now()),
        });
        if let Err(e) = done {
            crate::app_log::action("pin a version").failed(&e);
        }
        self.refresh_saves_in_background(cx);
        cx.notify();
    }

    pub(super) fn cap_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let initial = self
            .store
            .config()
            .vault_cap_mb
            .unwrap_or(vault::DEFAULT_CAP_MB)
            .to_string();
        self.prompt(
            Prompt {
                title: tr("Career saves storage"),
                description: tr("The most space, in MB, the saved versions may take. The oldest unpinned versions go first.")
                    .into(),
                initial,
                placeholder: "500",
                ok: tr("Save"),
                browse: None,
                action: std::rc::Rc::new(|store, value| {
                    let mb: u64 = value.parse().map_err(|_| tr("Enter a number of MB.").to_owned())?;
                    if mb == 0 {
                        return Err(tr("The limit must be at least 1 MB.").to_owned());
                    }
                    let mut c = store.config();
                    c.vault_cap_mb = Some(mb);
                    store.save_config(&c).map_err(|e| e.to_string())?;
                    Ok(trf!("The career saves keep at most {mb} MB", mb = mb))
                }),
            },
            window,
            cx,
        );
    }
    pub(super) fn retry_steam_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::game::import_now(&self.store);
        self.refresh_saves_in_background(cx);
        let found = !self.saves.view.as_ref().is_some_and(|v| v.steam_missing);
        window.notify(
            match found {
                true => Notification::success(tr("Imported your Steam save.")),
                false => Notification::warning(tr(
                    "Still couldn't find your Steam save. The log lists where it looked.",
                )),
            },
            cx,
        );
        cx.notify();
    }

    pub(super) fn pick_steam_save(&mut self, profile: String, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr("Choose").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else { return };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |s, window, cx| match steam_save_in(&folder) {
                Some(from) => s.ask_copy(from, VANILLA.to_owned(), profile, window, cx),
                None => {
                    crate::app_log::note(&format!(
                        "saves: {} is not a Steam save folder (userdata/<id>/{}/remote)",
                        folder.display(),
                        swofty_core::STEAM_APP_ID
                    ));
                    window.notify(
                        Notification::warning(tr(
                            "That isn't a Steam save folder. Pick the one named remote under userdata.",
                        )),
                        cx,
                    );
                }
            })
            .ok();
        })
        .detach();
    }
}
