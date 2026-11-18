use std::path::Path;

use crate::notices::{Notification, Notify};
use gpui_kit::*;
use swofty_core::dbedit::ImportReport;
use swofty_core::dbedit::package::{self, Incoming};
use swofty_i18n::{tr, trf};

use super::{Then, debug_path, guarded, text};
use crate::shell::Shell;

impl Shell {
    pub(crate) fn db_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = debug_path("SWOFTY_DEBUG_IMPORT") {
            self.db_import_path(&path, window, cx);
            return;
        }
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = rx.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            this.update_in(cx, |s, window, cx| s.db_import_path(&path, window, cx))
                .ok();
        })
        .detach();
    }

    fn db_import_path(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let file = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let Some(catalog) = self.db.catalog().cloned() else {
            return;
        };
        if self.db.importing {
            return;
        }
        self.db.importing = true;
        window.notify(Notification::info(trf!("Importing {file}…", file = file)), cx);
        cx.notify();
        let (store, path) = (self.store.clone(), path.to_path_buf());
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { guarded(|| package::read(&store, &catalog, &path).map_err(text)) })
                .await;
            this.update_in(cx, |s, window, cx| {
                s.db.importing = false;
                s.db_imported(result, file, window, cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // An addon .zip installs as a new addon and shows on Addons; other files open as a new draft.
    fn db_imported(
        &mut self,
        result: Result<Incoming, String>,
        file: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(Incoming::Addon(done)) => {
                let mut body = report_text(&done.report);
                if let Some(from) = &done.renamed_from {
                    body.push_str(&trf!(
                        "\n{from} was already installed; this copy is {id}",
                        from = from,
                        id = done.package.id
                    ));
                }
                let id = done.package.id.clone();
                let profile = self.library.profile.name.clone();
                if !self.vanilla() && self.db_turn_on(&id).is_ok() {
                    body.push_str(&trf!("\nTurned on in {profile}", profile = profile));
                }
                let title = trf!("Imported {name}", name = done.package.name);
                window.notify(Notification::success(body).title(title), cx);
                self.reload(cx);
                self.show_addon(&id, cx);
            }
            Ok(Incoming::Changes { name, edits, report }) => {
                let then = Then::Load {
                    file,
                    name,
                    edits,
                    report,
                };
                self.db_leave(then, window, cx);
            }
            Err(e) => {
                crate::app_log::action("db editor: import")
                    .file(std::path::Path::new(&file))
                    .failed(&e);
                window.notify(
                    Notification::error(e).title(trf!("Could not import {file}", file = file)),
                    cx,
                )
            }
        }
    }

    pub(crate) fn db_add_key(
        &mut self,
        path: &str,
        tag: u8,
        value: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let catalog = self.db.catalog().cloned().ok_or("The database is not loaded")?;
        if !self.db_ready_to_edit(cx) {
            return Err(self
                .db
                .addon_error
                .clone()
                .unwrap_or_else(|| "The draft can't be edited now.".into()));
        }
        let key = guarded(|| self.db.draft.edits.add(&catalog, path, tag, value))?;
        self.db_changed(Some(key), true, cx);
        Ok(())
    }
}

pub(super) fn report_text(r: &ImportReport) -> String {
    let format = match r.format {
        "ego" => tr("EGO GameDatabases Editor export"),
        "package" | "zip" => tr("DB Editor addon"),
        "bin" => tr("Modded game database"),
        _ => tr("DB Editor export"),
    };
    if r.format == "bin" {
        let mut lines = vec![format.to_owned(), trf!("{count} imported", count = r.imported)];
        lines.extend(r.skipped.iter().cloned());
        return lines.join("\n");
    }
    let mut lines = vec![
        format.to_owned(),
        trf!(
            "{imported} imported · {unchanged} unchanged",
            imported = r.imported,
            unchanged = r.unchanged
        ),
    ];
    if !r.skipped.is_empty() {
        lines.push(trf!("{count} skipped:", count = r.skipped.len()));
        lines.extend(r.skipped.iter().take(4).cloned());
        if r.skipped.len() > 4 {
            lines.push(trf!("and {count} more", count = r.skipped.len() - 4));
        }
    }
    lines.join("\n")
}
