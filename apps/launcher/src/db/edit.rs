use crate::notices::Notification;
use gpui_kit::component::WindowExt;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::*;
use swofty_i18n::{tr, trf, trn};

use super::{CellEdit, Reveal, Slots, guarded, swap_field};
use crate::shell::Shell;

impl Shell {
    pub(crate) fn db_set(&mut self, key: u64, value: &str, cx: &mut Context<Self>) -> bool {
        let Some(catalog) = self.db.catalog().cloned() else {
            return false;
        };
        if !self.db_ready_to_edit(cx) {
            return false;
        }
        let result = guarded(|| self.db.draft.edits.set(&catalog, key, value));
        match result {
            Ok(()) => {
                self.db.errors.remove(&key);
                self.db_changed(Some(key), true, cx);
                true
            }
            Err(e) => {
                self.db.errors.insert(key, e);
                cx.notify();
                false
            }
        }
    }

    /// The table and field of row spec `spec` of the open page, and the selected row's label.
    fn db_row_target(&self, spec: usize) -> Option<(String, String, String)> {
        let slot = self.db.view.as_ref()?;
        let module = self.db.module(&slot.module)?;
        let spec = module.row_specs().into_iter().nth(spec)?;
        let table = module.row_table(&spec, slot.scope.as_deref());
        Some((table, spec.field, self.db.row.clone()?))
    }

    /// Gives the selected row its own row of spec `spec`'s table: a copy of `from`'s, or of the game default.
    pub(crate) fn db_copy_row(&mut self, spec: usize, from: Option<String>, cx: &mut Context<Self>) {
        let (Some(catalog), Some((table, field, to))) =
            (self.db.catalog().cloned(), self.db_row_target(spec))
        else {
            return;
        };
        if !self.db_ready_to_edit(cx) {
            return;
        }
        let edits = &mut self.db.draft.edits;
        match guarded(|| edits.copy_row(&catalog, &table, &field, from.as_deref(), &to)) {
            Ok(()) => self.db_changed(None, true, cx),
            Err(e) => {
                crate::app_log::action("db editor: copy a row").failed(&e);
                self.db.toasts.push(Notification::error(e));
                cx.notify();
            }
        }
    }

    pub(crate) fn db_remove_row(&mut self, spec: usize, cx: &mut Context<Self>) {
        let Some((table, field, to)) = self.db_row_target(spec) else {
            return;
        };
        self.db_remove_copy(&table, &field, &to, cx);
    }

    pub(crate) fn db_remove_copy(&mut self, table: &str, field: &str, to: &str, cx: &mut Context<Self>) {
        if !self.db_ready_to_edit(cx) {
            return;
        }
        if guarded(|| Ok(self.db.draft.edits.remove_row(table, field, to))).unwrap_or(false) {
            self.db.errors.clear();
            self.db.slots = Slots::default();
            self.db_changed(None, true, cx);
        }
    }

    pub(crate) fn db_move(&mut self, label: &str, down: bool, cx: &mut Context<Self>) {
        let Some(slot) = self.db.view.as_ref() else {
            return;
        };
        let Some(field) = self.db.module(&slot.module).and_then(swap_field) else {
            return;
        };
        let rows = &slot.view.rows;
        let Some(i) = rows.iter().position(|r| r.label == label) else {
            return;
        };
        let Some(j) = (if down { i.checked_add(1) } else { i.checked_sub(1) }).filter(|j| *j < rows.len())
        else {
            return;
        };
        let (a, b) = (&rows[i].cells[field], &rows[j].cells[field]);
        let (Some(ka), Some(kb)) = (a.key, b.key) else {
            return;
        };
        let (va, vb) = (a.value.clone(), b.value.clone());
        let reveal = Reveal {
            module: slot.module.clone(),
            scope: slot.scope.clone(),
            label: rows[j].label.clone(),
        };
        if self.db_set(ka, &vb, cx) && self.db_set(kb, &va, cx) {
            self.db.row = Some(reveal.label.clone());
            self.db.reveal = Some(reveal);
        }
    }

    /// How many values `field` of the open page reads in the rows the page lists.
    pub(crate) fn db_listed(&self, field: usize) -> usize {
        let Some(slot) = self.db.view.as_ref() else {
            return 0;
        };
        self.db
            .module(&slot.module)
            .map_or(0, |m| m.keys_of(&slot.view, &slot.shown, field).len())
    }

    pub(crate) fn db_confirm_set_all(
        &mut self,
        field: usize,
        value: String,
        shown: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(slot) = self.db.view.as_ref() else {
            return;
        };
        let Some(f) = self.db.module(&slot.module).and_then(|m| m.fields.get(field)) else {
            return;
        };
        let n = self.db_listed(field);
        let title = trf!(
            "Set {label} to {shown} on {rows}?",
            label = f.label,
            shown = shown,
            rows = trn(n, "{count} row", "{count} rows")
        );
        let rows = if slot.shown.len() < slot.view.rows.len() {
            tr("Every row the search lists")
        } else {
            tr("Every row of this page")
        };
        let description = trf!(
            "{rows} gets this value, held inside the field's range. It saves to the addon like any other change.",
            rows = rows
        );
        let shell = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let (shell, value) = (shell.clone(), value.clone());
            alert
                .title(title.clone())
                .description(description.clone())
                .ok_text(tr("Set all"))
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    shell.update(cx, |s, cx| s.db_set_all(field, &value, cx)).ok();
                    true
                })
        });
    }

    /// Sets `field` of the open page to `value` in every listed row, as one change.
    pub(crate) fn db_set_all(&mut self, field: usize, value: &str, cx: &mut Context<Self>) {
        let Some(catalog) = self.db.catalog().cloned() else {
            return;
        };
        if !self.db_ready_to_edit(cx) {
            return;
        }
        let Some(slot) = self.db.view.as_ref() else {
            return;
        };
        let Some(module) = self.db.module(&slot.module) else {
            return;
        };
        let (view, rows) = (slot.view.clone(), slot.shown.clone());
        let label = module
            .fields
            .get(field)
            .map(|f| f.label.clone())
            .unwrap_or_default();
        let edits = &mut self.db.draft.edits;
        match guarded(|| module.set_all(&catalog, edits, &view, &rows, field, value)) {
            Ok(0) => {
                self.db.toasts.push(Notification::info(trf!(
                    "Every listed row already has that {label}",
                    label = label
                )));
                cx.notify();
            }
            Ok(n) => {
                self.db.slots = Slots::default();
                self.db.toasts.push(Notification::success(trf!(
                    "{label}: changed {rows}",
                    label = label,
                    rows = trn(n, "{count} row", "{count} rows")
                )));
                self.db_changed(None, true, cx);
            }
            Err(e) => {
                crate::app_log::action("db editor: change rows").failed(&e);
                self.db.toasts.push(Notification::error(e));
                cx.notify();
            }
        }
    }

    pub(crate) fn db_revert(&mut self, key: u64, cx: &mut Context<Self>) {
        self.db.errors.remove(&key);
        self.db.slots.forget(key);
        if !self.db_ready_to_edit(cx) {
            return;
        }
        if guarded(|| Ok(self.db.draft.edits.revert(key))).unwrap_or(false) {
            self.db_changed(Some(key), true, cx);
        }
        cx.notify();
    }

    pub(crate) fn db_revert_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.db.draft.edits.len();
        if n == 0 {
            return;
        }
        let shell = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let shell = shell.clone();
            alert
                .title(trf!("Revert all {n} changes?", n = n))
                .description(tr(
                    "Every value goes back to the game's stock value. Save afterwards to keep that in the addon.",
                ))
                .ok_text(tr("Revert all"))
                .ok_variant(gpui_kit::component::button::ButtonVariant::Danger)
                .show_cancel(true)
                .on_ok(move |_, _, cx| {
                    shell
                        .update(cx, |s, cx| {
                            if !s.db_ready_to_edit(cx) {
                                return;
                            }
                            s.db.draft.edits.clear();
                            s.db.errors.clear();
                            s.db.slots = Slots::default();
                            s.db_changed(None, true, cx);
                        })
                        .ok();
                    true
                })
        });
    }

    pub(crate) fn db_start_cell(
        &mut self,
        key: u64,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.db_locked() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).default_value(value));
        input.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        let sub = cx.subscribe_in(&input, window, move |this, state, ev: &InputEvent, _, cx| {
            if !matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                return;
            }
            if this.db.cell.as_ref().is_none_or(|c| c.key != key) {
                return;
            }
            let text = state.read(cx).value().trim().to_owned();
            this.db.cell = None;
            this.db_set(key, &text, cx);
            cx.notify();
        });
        self.db.cell = Some(CellEdit {
            key,
            input,
            _sub: sub,
        });
        cx.notify();
    }
}
