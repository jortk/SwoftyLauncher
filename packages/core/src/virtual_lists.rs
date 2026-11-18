//! Virtual entrant lists: rows of `entrants.entrant_lists` the game does not load.
//!
//! The game loads one list object per row below `_entrant_list_instancecount_`, keeps its stat id in a byte (255)
//! and lets an entrant setup mode hold 32 lists. An addon marks a list virtual with the `virtual-list` database op.
//! After every addon's ops ran, [`settle`] moves each virtual list's row (its entrant rows included) to an index at
//! or above [`BASE`], fills the gap with the table's last added row and lowers the count, so the game never sees
//! the list. Its label is taken out of the content sets' and entrant setup modes' list fields. The core reads the
//! row when a box naming it is pressed (era_picker.cpp, `swofty_virtual_list`).

use std::collections::{BTreeMap, BTreeSet, HashSet};

use swofty_formats::gvdb::{Gvdb, Record, instance_count_path};
use swofty_formats::hash::fnv1a64;

use crate::ops::{VIRTUAL_LISTS, row_keys};
use crate::rowkeys::{Names, RowKeys};

const TABLE: &str = "entrants.entrant_lists";
const ROW: &str = "entrant_list";
/// Virtual rows start here, far above any count the game or the core's runtime rows reach.
pub const BASE: usize = 10000;

fn text(db: &Gvdb, path: &str) -> Option<String> {
    let k = fnv1a64(path);
    db.records
        .iter()
        .find(|r| r.key == k)
        .map(|r| r.text().into_owned())
}

fn set_text(db: &mut Gvdb, path: &str, tag: u8, value: &str) {
    let k = fnv1a64(path);
    match db.records.iter_mut().find(|r| r.key == k) {
        Some(r) => r.value = value.as_bytes().to_vec(),
        None => db.records.push(Record::new(path, tag, value)),
    }
}

fn items(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(String::from)
        .collect()
}

/// Moves every key of row `from` (as the row-key catalogue names them) to row `to`.
fn move_row(db: &mut Gvdb, names: &Names, from: &str, to: &str) -> usize {
    let keys = row_keys(db, names, &[from.to_owned()]).pop().unwrap_or_default();
    let renames: BTreeMap<u64, u64> = keys
        .iter()
        .map(|rel| (fnv1a64(format!("{from}.{rel}")), fnv1a64(format!("{to}.{rel}"))))
        .collect();
    let mut moved = 0;
    for r in db.records.iter_mut() {
        if let Some(&k) = renames.get(&r.key) {
            r.key = k;
            moved += 1;
        }
    }
    moved
}

/// The list the core fills on a press of a virtual box (era_picker.cpp, VIRTUAL_HOST).
pub const HOST: &str = "swofty_virtual_list";
/// The entrant setup mode every addon box runs under.
const HOST_MODE: &str = "swofty_f1";

/// Copies every key of row `from` to row `to`, leaving `from` as it was.
fn copy_row(db: &mut Gvdb, names: &Names, from: &str, to: &str) -> usize {
    let keys = row_keys(db, names, &[from.to_owned()]).pop().unwrap_or_default();
    let mut copies = Vec::new();
    for rel in &keys {
        let k = fnv1a64(format!("{from}.{rel}"));
        if let Some(r) = db.records.iter().find(|r| r.key == k) {
            copies.push(Record {
                key: fnv1a64(format!("{to}.{rel}")),
                tag: r.tag,
                value: r.value.clone(),
            });
        }
    }
    let n = copies.len();
    db.records.extend(copies);
    n
}

/// The first available_entrant_lists field of the content set labelled `set`.
fn content_set_lists(db: &Gvdb, set: &str) -> Option<String> {
    let n = text(db, &instance_count_path("content_sets", "content_set"))?
        .trim()
        .parse::<usize>()
        .ok()?;
    (0..n)
        .map(|i| format!("content_sets.content_set_{i}"))
        .find(|row| text(db, &format!("{row}._metadata_")).as_deref() == Some(set))
        .map(|row| format!("{row}.available_entrant_lists_0.entrant_lists"))
}

fn host_setup_mode(db: &Gvdb) -> Option<String> {
    let table = "entrants.entrant_setup_modes";
    let n = text(db, &instance_count_path(table, "entrant_setup_mode"))?
        .trim()
        .parse::<usize>()
        .ok()?;
    (0..n)
        .map(|i| format!("{table}.entrant_setup_mode_{i}"))
        .find(|row| text(db, &format!("{row}._metadata_")).as_deref() == Some(HOST_MODE))
}

/// Settles the lists the `virtual-list` op marked; returns the build's report lines.
pub fn settle(db: &mut Gvdb, stock: &HashSet<u64>) -> Vec<String> {
    let mut lines = Vec::new();
    let Some(marked) = text(db, VIRTUAL_LISTS).map(|t| items(&t)) else {
        return lines;
    };
    let fields = BTreeMap::new();
    let names = Names {
        fields: &fields,
        keys: RowKeys::builtin(),
        stock: Some(stock),
        tracks: None,
    };
    let count_path = instance_count_path(TABLE, ROW);
    let Some(mut count) = text(db, &count_path).and_then(|t| t.trim().parse::<usize>().ok()) else {
        return lines;
    };
    let mut next = BASE;
    while text(db, &format!("{TABLE}.{ROW}_{next}._metadata_")).is_some() {
        next += 1;
    }
    let mut settled = BTreeSet::new();
    for label in &marked {
        let Some(i) = (0..count)
            .find(|&i| text(db, &format!("{TABLE}.{ROW}_{i}._metadata_")).as_deref() == Some(label))
        else {
            lines.push(format!(
                "virtual list {label}: no row of {TABLE} has this label; nothing moved"
            ));
            continue;
        };
        let row = format!("{TABLE}.{ROW}_{i}");
        if stock.contains(&fnv1a64(format!("{row}._metadata_"))) {
            lines.push(format!("virtual list {label}: a stock list stays where it is"));
            continue;
        }
        let last = format!("{TABLE}.{ROW}_{}", count - 1);
        if i != count - 1 && stock.contains(&fnv1a64(format!("{last}._metadata_"))) {
            lines.push(format!(
                "virtual list {label}: the table's last row is a stock list; nothing moved"
            ));
            continue;
        }
        let to = format!("{TABLE}.{ROW}_{next}");
        let n = move_row(db, &names, &row, &to);
        if i != count - 1 {
            move_row(db, &names, &last, &row);
        }
        count -= 1;
        next += 1;
        settled.insert(label.clone());
        lines.push(format!(
            "virtual list {label}: {n} key(s) moved from {row} to {to}"
        ));
    }
    if settled.is_empty() {
        return lines;
    }
    // The host the core swaps a pressed virtual list into: one ordinary list, offered under swofty_f1.
    let label_at = |db: &Gvdb, i: usize| text(db, &format!("{TABLE}.{ROW}_{i}._metadata_"));
    if !(0..count).any(|i| label_at(db, i).as_deref() == Some(HOST)) {
        let used: HashSet<i64> = (0..count)
            .filter_map(|i| text(db, &format!("{TABLE}.{ROW}_{i}.stat_id")))
            .filter_map(|t| t.trim().parse().ok())
            .collect();
        match (0..=255).find(|id| !used.contains(id)) {
            Some(id) => {
                let host = format!("{TABLE}.{ROW}_{count}");
                copy_row(db, &names, &format!("{TABLE}.{ROW}_{BASE}"), &host);
                set_text(db, &format!("{host}._metadata_"), 4, HOST);
                set_text(db, &format!("{host}.stat_id"), 0, &id.to_string());
                count += 1;
                lines.push(format!(
                    "virtual lists: host {HOST} added as {host} (stat id {id})"
                ));
            }
            None => lines.push(format!(
                "virtual lists: no stat id up to 255 is free for the host {HOST}; virtual boxes stay hidden"
            )),
        }
    }
    for set in ["2025", "2026"] {
        if let Some(path) = content_set_lists(db, set) {
            let mut v = text(db, &path).map(|t| items(&t)).unwrap_or_default();
            if !v.iter().any(|l| l == HOST) {
                v.push(HOST.into());
                set_text(db, &path, 13, &v.join(","));
            }
        }
    }
    if let Some(esm) = host_setup_mode(db) {
        for field in ["entrant_lists", "selectable_entrant_lists"] {
            let path = format!("{esm}.{field}");
            let mut v = text(db, &path).map(|t| items(&t)).unwrap_or_default();
            if !v.iter().any(|l| l == HOST) {
                v.push(HOST.into());
                set_text(db, &path, 13, &v.join(","));
            }
        }
    }
    set_text(db, &count_path, 0, &count.to_string());
    // The game resolves these fields' labels against the loaded lists; a virtual one is never loaded.
    let mut refs = Vec::new();
    for (table, row, fields) in [
        ("content_sets", "content_set", &["available_entrant_lists"][..]),
        ("entrants.entrant_setup_modes", "entrant_setup_mode", &[][..]),
    ] {
        let n = text(db, &instance_count_path(table, row))
            .and_then(|t| t.trim().parse::<usize>().ok())
            .unwrap_or(0);
        for i in 0..n {
            let base = format!("{table}.{row}_{i}");
            if fields.is_empty() {
                refs.push(format!("{base}.entrant_lists"));
                refs.push(format!("{base}.selectable_entrant_lists"));
            }
            for sub in fields {
                let m = text(db, &instance_count_path(&base, sub))
                    .and_then(|t| t.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                for j in 0..m {
                    refs.push(format!("{base}.{sub}_{j}.entrant_lists"));
                }
            }
        }
    }
    let mut stripped = 0;
    for path in refs {
        let Some(v) = text(db, &path) else { continue };
        let kept: Vec<String> = items(&v).into_iter().filter(|l| !settled.contains(l)).collect();
        if kept.len() != items(&v).len() {
            set_text(db, &path, 13, &kept.join(","));
            stripped += 1;
        }
    }
    lines.push(format!(
        "virtual lists: {} moved above the count ({count} lists left for the game), {stripped} list field(s) no longer name them",
        settled.len()
    ));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(path: &str, tag: u8, value: &str) -> Record {
        Record::new(path, tag, value)
    }

    fn list(records: &mut Vec<Record>, i: usize, name: &str, entrants: &[&str]) {
        let row = format!("{TABLE}.{ROW}_{i}");
        records.push(rec(&format!("{row}._metadata_"), 4, name));
        records.push(rec(&format!("{row}.stat_id"), 0, &(60 + i).to_string()));
        records.push(rec(
            &format!("{row}._entrant_instancecount_"),
            0,
            &entrants.len().to_string(),
        ));
        for (j, e) in entrants.iter().enumerate() {
            records.push(rec(&format!("{row}.entrant_{j}._metadata_"), 4, e));
        }
    }

    fn get(db: &Gvdb, path: &str) -> Option<String> {
        text(db, path)
    }

    #[test]
    fn a_virtual_list_moves_above_the_count_and_its_name_leaves_the_list_fields() {
        let mut records = vec![
            rec(&instance_count_path(TABLE, ROW), 0, "4"),
            rec(
                "entrants.entrant_setup_modes._entrant_setup_mode_instancecount_",
                0,
                "1",
            ),
            rec(
                "entrants.entrant_setup_modes.entrant_setup_mode_0.entrant_lists",
                13,
                "f1_2026,season_a,season_b",
            ),
            rec(
                "entrants.entrant_setup_modes.entrant_setup_mode_0._metadata_",
                4,
                "swofty_f1",
            ),
            rec(VIRTUAL_LISTS, 4, "season_a"),
        ];
        list(&mut records, 0, "f1_2026", &["a1", "a2"]);
        list(&mut records, 1, "f2_2026", &["b1"]);
        list(&mut records, 2, "season_a", &["s1", "s2", "s3"]);
        list(&mut records, 3, "season_b", &["t1"]);
        let stock: HashSet<u64> = records
            .iter()
            .filter(|r| {
                [0, 1].iter().any(|i| {
                    r.key == fnv1a64(format!("{TABLE}.{ROW}_{i}._metadata_"))
                        || r.key == fnv1a64(format!("{TABLE}.{ROW}_{i}.stat_id"))
                })
            })
            .map(|r| r.key)
            .collect();
        let mut db = Gvdb { records };
        let lines = settle(&mut db, &stock);
        assert!(
            lines
                .iter()
                .any(|l| l.contains("1 moved above the count (4 lists left")),
            "{lines:?}"
        );
        assert_eq!(get(&db, &instance_count_path(TABLE, ROW)).as_deref(), Some("4"));
        let v = format!("{TABLE}.{ROW}_{BASE}");
        assert_eq!(get(&db, &format!("{v}._metadata_")).as_deref(), Some("season_a"));
        assert_eq!(
            get(&db, &format!("{v}.entrant_2._metadata_")).as_deref(),
            Some("s3")
        );
        assert_eq!(
            get(&db, &format!("{TABLE}.{ROW}_2._metadata_")).as_deref(),
            Some("season_b")
        );
        assert_eq!(
            get(&db, &format!("{TABLE}.{ROW}_2.entrant_0._metadata_")).as_deref(),
            Some("t1")
        );
        assert_eq!(
            get(&db, &format!("{TABLE}.{ROW}_3._metadata_")).as_deref(),
            Some(HOST)
        );
        assert_eq!(
            get(&db, &format!("{TABLE}.{ROW}_3.entrant_1._metadata_")).as_deref(),
            Some("s2")
        );
        assert_eq!(
            get(&db, &format!("{TABLE}.{ROW}_3.stat_id")).as_deref(),
            Some("0")
        );
        assert_eq!(get(&db, &format!("{TABLE}.{ROW}_4._metadata_")), None);
        assert_eq!(
            get(
                &db,
                "entrants.entrant_setup_modes.entrant_setup_mode_0.entrant_lists"
            )
            .as_deref(),
            Some("f1_2026,season_b,swofty_virtual_list")
        );
    }

    #[test]
    fn nothing_moves_without_the_marker_record() {
        let mut records = vec![rec(&instance_count_path(TABLE, ROW), 0, "1")];
        list(&mut records, 0, "f1_2026", &["a1"]);
        let mut db = Gvdb { records };
        assert!(settle(&mut db, &HashSet::new()).is_empty());
        assert_eq!(get(&db, &instance_count_path(TABLE, ROW)).as_deref(), Some("1"));
    }
}
