use gpui_kit::*;
use swofty_i18n::trf;

use super::value::{curve_text, list_items, parse_curve, sep_of, stored_y};
use crate::shell::Shell;

impl Shell {
    pub(super) fn db_slot_key(&self, id: &str) -> Option<u64> {
        let base = id.split_once('/').map_or(id, |(b, _)| b);
        self.db_detail_fields()
            .into_iter()
            .find(|f| f.id == base)
            .and_then(|f| f.key)
    }

    pub(super) fn db_commit_input(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(key) = self.db_slot_key(id) else {
            return;
        };
        let Some(slot) = self.db.slots.inputs.get(id) else {
            return;
        };
        let text = slot.input.read(cx).value().trim().to_owned();
        let part = id
            .rsplit_once('/')
            .filter(|(_, t)| t.starts_with(['x', 'y', 'i']));
        let Some((base, tail)) = part else {
            if text != slot.synced {
                self.db_set(key, &text, cx);
            }
            return;
        };
        let current = self.db.current(key);
        let edited = |slot: &str| {
            self.db
                .slots
                .inputs
                .get(slot)
                .map(|s| (s.input.read(cx).value().trim().to_owned(), s.synced.clone()))
                .filter(|(now, was)| now != was)
                .map(|(now, _)| now)
        };
        if tail.starts_with('i') {
            let mut items = list_items(&current);
            for (i, item) in items.iter_mut().enumerate() {
                if let Some(v) = edited(&format!("{base}/i{i}")) {
                    *item = v;
                }
            }
            let next = items.join(sep_of(&current));
            if next != current {
                self.db_set(key, &next, cx);
            }
            return;
        }
        let Some(stock) = parse_curve(&current) else {
            return;
        };
        let Some(mut curve) = parse_curve(&current) else {
            return;
        };
        let mut bad = None;
        for (i, (x, y)) in curve.points.iter_mut().enumerate() {
            if let Some(v) = edited(&format!("{base}/x{i}")) {
                *x = v;
            }
            if let Some(v) = edited(&format!("{base}/y{i}")) {
                match stored_y(&stock, &v) {
                    Some(stored) => *y = stored,
                    None => bad = Some(trf!("{v} is not a number", v = v)),
                }
            }
        }
        if let Some(e) = bad {
            self.db.errors.insert(key, e);
            cx.notify();
            return;
        }
        let next = curve_text(&curve);
        if next != current {
            self.db_set(key, &next, cx);
        }
    }

    pub(super) fn db_curve_edit(
        &mut self,
        key: u64,
        f: impl FnOnce(&mut Vec<(String, String)>),
        cx: &mut Context<Self>,
    ) {
        let current = self.db.current(key);
        if let Some(mut c) = parse_curve(&current) {
            f(&mut c.points);
            self.db.slots.forget(key);
            self.db_set(key, &curve_text(&c), cx);
        }
    }

    pub(super) fn db_list_edit(
        &mut self,
        key: u64,
        f: impl FnOnce(&mut Vec<String>),
        cx: &mut Context<Self>,
    ) {
        let current = self.db.current(key);
        let mut items = list_items(&current);
        f(&mut items);
        self.db_set(key, &items.join(sep_of(&current)), cx);
    }
}
