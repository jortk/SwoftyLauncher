//! Putting back the ids 1.6.10 and 1.6.11 gave added rows from their keys. Those builds hashed every
//! row the store had no id for, and their first build found no earlier id to keep (the build routed
//! the rows into `swofty/assets.json`, which the seed did not read), so every row moved once and the
//! Grand Prix and career saves from before 1.6.10 lost their teams and lists.
//!
//! Once per install the store reads the game session logs (`$DATA/sessions/<stamp>/loader.log`): the
//! core logs each record it creates in game, `gamedb: created 0x<key> (key 0x<key>, tag N type): value`,
//! a row's label as its fnv1a32 (`string, label fnv32 0x...`). A session whose rows hold other ids
//! than the store's is from before 1.6.10, and the newest such session gives each row its earlier id.
//! A row with no earlier id in any log is dropped, so the build gives it the next free id in load order
//! as builds before 1.6.10 did. The sessions after the newest one from before 1.6.10 ran with the
//! hashed ids: when one of them loaded a session with an addon's hashed team ids and none loaded its
//! earlier ones, a save made since then uses them, and that addon's rows keep the ids they have.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use swofty_formats::hash::{fnv1a32, fnv1a64};

use super::{IdFrom, Store, stat_id_field};

const SESSIONS_READ: usize = 400;
const ROW_SCAN: usize = 8192;
const ROW_GAP: usize = 64;
const STALE_SESSIONS: usize = 150;
const TEAMS: &str = "entrants.teams.team";
const TRACE: &str = "[era] session trace StateLoadSession Entering (";

/// What [`Store::recover`] changed, as build lines.
#[derive(Debug, Default)]
pub struct Recovery {
    pub lines: Vec<String>,
}

struct Row {
    key: String,
    addon: String,
    label: u32,
    nth: usize,
    kept: i64,
}

struct Session {
    created: HashMap<u64, (u8, String)>,
    teams: Vec<i64>,
}

fn rows_of(key: &str) -> Option<(String, String, String, usize)> {
    let (addon, rest) = key.split_once('/')?;
    let (path, label) = rest.split_once('/')?;
    let (table, row) = path.rsplit_once('.')?;
    let (label, nth) = match label.rsplit_once('#') {
        Some((l, n)) if n.parse::<usize>().is_ok_and(|n| n >= 2) => (l, n.parse::<usize>().ok()? - 1),
        _ => (label, 0),
    };
    Some((addon.to_owned(), format!("{table}.{row}"), label.to_owned(), nth))
}

fn hex(s: &str) -> Option<u64> {
    let s = s.strip_prefix("0x")?;
    let end = s.find(|c: char| !c.is_ascii_hexdigit()).unwrap_or(s.len());
    u64::from_str_radix(&s[..end], 16).ok()
}

fn read_session(dir: &Path, wanted: &HashSet<u64>) -> Option<Session> {
    let text = ["loader.log", "game-logs/loader.log"]
        .iter()
        .find_map(|f| std::fs::read(dir.join(f)).ok())?;
    let text = String::from_utf8_lossy(&text);
    let mut out = Session {
        created: HashMap::new(),
        teams: Vec::new(),
    };
    for line in text.lines() {
        if let Some(at) = line.find("gamedb: created 0x") {
            let rest = &line[at + "gamedb: created ".len()..];
            let Some(key) = hex(rest).filter(|k| wanted.contains(k)) else {
                continue;
            };
            let (Some(tag_at), Some(value_at)) = (rest.find("tag "), rest.find("): ")) else {
                continue;
            };
            let tag = rest[tag_at + 4..]
                .split(' ')
                .next()
                .and_then(|t| t.parse::<u8>().ok())
                .unwrap_or(u8::MAX);
            out.created
                .insert(key, (tag, rest[value_at + 3..].trim().to_owned()));
        } else if line.contains(TRACE)
            && let Some(at) = line.find("(teams ")
        {
            let list = &line[at + "(teams ".len()..];
            let list = &list[..list.find(')').unwrap_or(list.len())];
            out.teams
                .extend(list.split(',').filter_map(|t| t.trim().parse::<i64>().ok()));
        }
    }
    Some(out)
}

// The record keys of a table's rows 0 to ROW_SCAN: each row's label and id.
fn row_keys(table: &str) -> Vec<(u64, u64)> {
    let Some((path, row)) = table.rsplit_once('.') else {
        return Vec::new();
    };
    let field = stat_id_field(path, row);
    (0..ROW_SCAN)
        .map(|i| {
            (
                fnv1a64(format!("{table}_{i}._metadata_")),
                fnv1a64(format!("{table}_{i}.{field}")),
            )
        })
        .collect()
}

fn label_hash(value: &str) -> Option<u32> {
    let at = value.find("label fnv32 ")?;
    u32::from_str_radix(
        value[at + "label fnv32 ".len()..].trim().trim_start_matches("0x"),
        16,
    )
    .ok()
}

// The ids the session's log gives `rows` of a table whose row keys are `keys`, by row key.
fn evidence(session: &Session, keys: &[(u64, u64)], rows: &[Row]) -> BTreeMap<String, i64> {
    let mut found: Vec<(u32, Option<i64>)> = Vec::new();
    let mut gap = 0;
    for (meta, stat) in keys {
        let Some((_, label)) = session.created.get(meta) else {
            gap += 1;
            if !found.is_empty() && gap > ROW_GAP {
                break;
            }
            continue;
        };
        gap = 0;
        let id = session
            .created
            .get(stat)
            .filter(|(tag, _)| *tag == 0)
            .and_then(|(_, v)| v.trim().parse::<i64>().ok());
        if let Some(h) = label_hash(label) {
            found.push((h, id));
        }
    }
    let mut out = BTreeMap::new();
    for r in rows {
        if let Some(Some(id)) = found
            .iter()
            .filter(|(h, _)| *h == r.label)
            .nth(r.nth)
            .map(|(_, id)| *id)
        {
            out.insert(r.key.clone(), id);
        }
    }
    out
}

fn sessions(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_dir()
                        && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                            n.len() == 15
                                && n.as_bytes()[8] == b'-'
                                && n.bytes().enumerate().all(|(i, b)| i == 8 || b.is_ascii_digit())
                        })
                })
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    dirs.reverse();
    dirs.truncate(SESSIONS_READ);
    dirs
}

impl Store {
    /// Once per install, puts back the ids builds before 1.6.10 gave the rows 1.6.10 and 1.6.11 gave
    /// an id from their keys, from the game session logs in `sessions`; see the module docs.
    pub fn recover(&mut self, sessions_dir: &Path) -> Recovery {
        let mut out = Recovery::default();
        if self.read_only || self.recovered {
            return out;
        }
        self.recovered = true;
        self.changed = true;
        if !self.existed {
            return out;
        }
        let mut tables: BTreeMap<String, Vec<Row>> = BTreeMap::new();
        for (key, &id) in &self.rows {
            if self.from.get(key) != Some(&IdFrom::Hashed) {
                continue;
            }
            let Some((addon, table, label, nth)) = rows_of(key) else {
                continue;
            };
            tables.entry(table).or_default().push(Row {
                key: key.clone(),
                addon,
                label: fnv1a32(&label),
                nth,
                kept: id,
            });
        }
        let hashed: usize = tables.values().map(Vec::len).sum();
        if hashed == 0 {
            return out;
        }
        let keys: BTreeMap<&String, Vec<(u64, u64)>> = tables.keys().map(|t| (t, row_keys(t))).collect();
        let wanted: HashSet<u64> = keys.values().flatten().flat_map(|(m, s)| [*m, *s]).collect();
        let mut earlier: BTreeMap<String, (usize, i64)> = BTreeMap::new();
        let mut since: HashSet<i64> = HashSet::new();
        let (mut before, mut after, mut stale) = (0, 0, 0);
        for (age, dir) in sessions(sessions_dir).into_iter().enumerate() {
            if earlier.len() == hashed || stale >= STALE_SESSIONS {
                break;
            }
            let Some(session) = read_session(&dir, &wanted) else {
                continue;
            };
            let mut seen = BTreeMap::new();
            for (table, rows) in &tables {
                seen.extend(evidence(&session, &keys[table], rows));
            }
            let kept = |key: &String| self.rows.get(key).copied();
            let same = seen.iter().filter(|(k, id)| kept(k) == Some(**id)).count();
            let other = seen.len() - same;
            if other > same {
                before += 1;
                let known = earlier.len();
                for (key, id) in seen {
                    if kept(&key) != Some(id) {
                        earlier.entry(key).or_insert((age, id));
                    }
                }
                stale = if earlier.len() == known { stale + 1 } else { 0 };
            } else if before == 0 {
                after += 1;
                since.extend(session.teams);
            }
        }
        let mut keep: BTreeSet<String> = BTreeSet::new();
        if let Some(rows) = tables.get(TEAMS) {
            let mut by_addon: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
            for r in rows {
                by_addon.entry(r.addon.as_str()).or_default().push(r);
            }
            for (addon, rows) in by_addon {
                let new = rows.iter().any(|r| since.contains(&r.kept));
                let old = rows
                    .iter()
                    .any(|r| earlier.get(&r.key).is_some_and(|(_, id)| since.contains(id)));
                if new && !old {
                    keep.insert(addon.to_owned());
                }
            }
        }
        let (mut back, mut dropped, mut kept_new) = (0, 0, 0);
        let mut addons: BTreeSet<String> = BTreeSet::new();
        let mut order: Vec<&Row> = tables.values().flatten().collect();
        order.sort_by_key(|r| {
            (
                earlier.get(&r.key).map_or(usize::MAX, |(age, _)| *age),
                r.key.clone(),
            )
        });
        let mut taken: HashMap<String, HashSet<i64>> = HashMap::new();
        let candidates: HashSet<&str> = order.iter().map(|r| r.key.as_str()).collect();
        for row in order {
            if keep.contains(&row.addon) {
                if let Some((_, id)) = earlier.get(&row.key) {
                    self.alias(&row.key, *id, false);
                }
                kept_new += 1;
                continue;
            }
            addons.insert(row.addon.clone());
            let table = row.key.split('/').nth(1).unwrap_or_default().to_owned();
            let held = |id: i64| {
                self.by_id
                    .get(&table)
                    .and_then(|ids| ids.get(&id))
                    .is_some_and(|k| !candidates.contains(k.as_str()))
            };
            match earlier.get(&row.key).map(|(_, id)| *id) {
                Some(id) if !held(id) && taken.entry(table.clone()).or_default().insert(id) => {
                    self.alias(&row.key, row.kept, false);
                    self.insert(row.key.clone(), id);
                    self.from.insert(row.key.clone(), IdFrom::Seeded);
                    self.base_put(&row.key, id, IdFrom::Seeded);
                    back += 1;
                }
                _ => {
                    self.alias(&row.key, row.kept, false);
                    self.base_drop(&row.key);
                    if self.rows.get(&row.key) == Some(&row.kept) {
                        self.rows.remove(&row.key);
                        if let Some(ids) = self.by_id.get_mut(&table)
                            && ids.get(&row.kept) == Some(&row.key)
                        {
                            ids.remove(&row.kept);
                        }
                    }
                    self.from.remove(&row.key);
                    dropped += 1;
                }
            }
        }
        if back + dropped > 0 {
            let list: Vec<&str> = addons.iter().map(String::as_str).collect();
            out.lines.push(format!(
                "database: row-ids.json: {} added-row id(s) that 1.6.10 or 1.6.11 gave from the row's key go back to \
                 the ids builds before 1.6.10 gave them, so Grand Prix and career saves from before 1.6.10 find their \
                 teams and grids again ({}): {back} read from {before} game session log(s) from before 1.6.10, \
                 {dropped} given again in load order as those builds did ({after} later session log(s) read)",
                back + dropped,
                list.join(", ")
            ));
        }
        if kept_new > 0 {
            let list: Vec<&str> = keep.iter().map(String::as_str).collect();
            out.lines.push(format!(
                "database: row-ids.json: {kept_new} added-row id(s) of {} stay as 1.6.10 or 1.6.11 gave them: a \
                 session loaded since then used their team ids and none used the earlier ones",
                list.join(", ")
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{TEAMS as TEAM_TABLE, table_to, try_build_on};
    use super::*;

    fn line(key: &str, tag: u8, value: &str) -> String {
        let k = fnv1a64(key);
        format!(
            "2026-10-29 21:35:38.232 [ 8652] [assets] gamedb: created 0x{k:016x} (key 0x{k:016x}, tag {tag} x): {value}\n"
        )
    }

    fn row_lines(table: &str, i: usize, label: &str, id: i64) -> String {
        line(
            &format!("{table}_{i}._metadata_"),
            4,
            &format!("string, label fnv32 0x{:08x}", fnv1a32(label)),
        ) + &line(&format!("{table}_{i}.stat_id"), 0, &id.to_string())
    }

    fn session(root: &Path, stamp: &str, text: &str) {
        let dir = root.join(stamp);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("loader.log"), text).unwrap();
    }

    fn data_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-row-ids-recover-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // A store as 1.6.10 left it: every row hashed, the profile seeded.
    fn hit(data: &Path, rows: &[(&str, i64)]) {
        let rows: serde_json::Map<String, serde_json::Value> = rows
            .iter()
            .map(|(k, id)| ((*k).to_owned(), serde_json::json!({ "id": id, "from": "hashed" })))
            .collect();
        let doc = serde_json::json!({ "version": 1, "rows": rows, "seeded": ["default"] });
        let path = super::super::path(data);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(&doc).unwrap()).unwrap();
    }

    const T: &str = "entrants.teams.team";

    #[test]
    fn the_ids_from_before_1_6_10_come_back_from_the_session_logs_once() {
        let data = data_dir("back");
        let sessions = data.join("sessions");
        let a = "swofty.classic-seasons-2016/entrants.teams.team/cs16_mercedes";
        let b = "swofty.classic-seasons-2016/entrants.teams.team/cs16_redbull";
        hit(&data, &[(a, 646), (b, 652)]);
        session(
            &sessions,
            "20261029-203531",
            &(row_lines(T, 510, "cs16_mercedes", 518) + &row_lines(T, 511, "cs16_redbull", 524)),
        );
        session(
            &sessions,
            "20261028-195559",
            &(row_lines(T, 510, "cs16_mercedes", 600) + &row_lines(T, 511, "cs16_redbull", 601)),
        );
        session(
            &sessions,
            "20261030-182214",
            &(row_lines(T, 510, "cs16_mercedes", 646)
                + &row_lines(T, 511, "cs16_redbull", 652)
                + "2026-10-30 18:25:08.832 [ 5884] [era] session trace StateLoadSession Entering (grand_prix_23): entrants 2 (teams 518,524), list ? (- cars) of setup mode swofty_f1; participants: game manager 0\n"),
        );
        let (mut store, _) = Store::load(&data, None, "default", None);
        let r = store.recover(&sessions);
        assert_eq!((store.get(a), store.get(b)), (Some(518), Some(524)));
        assert_eq!(store.from[a], IdFrom::Seeded);
        assert!(
            r.lines
                .iter()
                .any(|l| l.contains("2 read from 1 game session log(s) from before 1.6.10")),
            "{:?}",
            r.lines
        );
        let teams = BTreeMap::from([(646, 518), (652, 524)]);
        store.claimed.extend([a.to_owned(), b.to_owned()]);
        assert_eq!(store.id_aliases().tables["teams"], teams);
        store.save(&data).unwrap();
        let (mut again, _) = Store::load(&data, None, "default", None);
        assert!(
            again.recover(&sessions).lines.is_empty(),
            "the recovery runs once"
        );
        assert_eq!(again.get(a), Some(518));
        again.claimed.extend([a.to_owned(), b.to_owned()]);
        let aliases = again.id_aliases();
        assert_eq!(
            aliases.tables["teams"], teams,
            "the earlier ids outlive the recovery"
        );
        let out = data.join("id_aliases.json");
        aliases.write(&out).unwrap();
        let doc: serde_json::Value = serde_json::from_slice(&std::fs::read(&out).unwrap()).unwrap();
        assert_eq!(doc["version"], 1);
        assert_eq!(doc["tables"]["teams"]["646"], 518);
        assert_eq!(doc["tables"]["teams"]["652"], 524);
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_row_no_log_holds_gets_its_load_order_id_again_and_a_save_since_keeps_the_hashed_ids() {
        let data = data_dir("drop");
        let sessions = data.join("sessions");
        let cs19: Vec<String> = (0..3)
            .map(|i| format!("swofty.classic-seasons-2019/entrants.teams.team/cs19_t{i}"))
            .collect();
        let rows: Vec<(&str, i64)> = cs19
            .iter()
            .zip([700, 731, 712])
            .map(|(k, id)| (k.as_str(), id))
            .collect();
        hit(&data, &rows);
        let (mut store, _) = Store::load(&data, None, "default", None);
        let r = store.recover(&sessions);
        assert!(
            cs19.iter().all(|k| store.get(k).is_none()),
            "rows with no earlier id are dropped"
        );
        assert!(
            r.lines.iter().any(|l| l.contains("3 given again in load order")),
            "{:?}",
            r.lines
        );
        let labels: Vec<&str> = vec!["cs19_t0", "cs19_t1", "cs19_t2"];
        let built = try_build_on(
            table_to(TEAM_TABLE, 549),
            TEAM_TABLE,
            &[("swofty.classic-seasons-2019", labels.as_slice())],
            store,
        )
        .unwrap();
        assert_eq!(
            labels.iter().map(|l| built.ids[*l]).collect::<Vec<_>>(),
            [550, 551, 552]
        );
        assert_eq!(
            built.store.id_aliases().tables["teams"],
            BTreeMap::from([(700, 550), (731, 551), (712, 552)]),
            "a row given again keeps its hashed id as an alias of the new one"
        );
        std::fs::remove_dir_all(&data).ok();

        let data = data_dir("since");
        let sessions = data.join("sessions");
        hit(&data, &rows);
        session(
            &sessions,
            "20261031-101010",
            "2026-10-31 10:10:10.000 [ 1] [era] session trace StateLoadSession Entering (grand_prix_23): entrants 3 (teams 700,731,712), list cs_f1_2019 (3 cars) of setup mode swofty_f1\n",
        );
        let (mut store, _) = Store::load(&data, None, "default", None);
        let r = store.recover(&sessions);
        assert_eq!(store.get(&cs19[1]), Some(731));
        assert!(
            store.id_aliases().is_empty(),
            "rows that keep their ids have no aliases"
        );
        assert!(
            r.lines
                .iter()
                .any(|l| l.contains("stay as 1.6.10 or 1.6.11 gave them")),
            "{:?}",
            r.lines
        );
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_store_made_since_the_hash_was_dropped_has_nothing_to_put_back() {
        let data = data_dir("fresh");
        let (mut store, _) = Store::load(&data, None, "default", None);
        assert!(store.recover(&data.join("sessions")).lines.is_empty());
        assert!(store.recovered);
        std::fs::remove_dir_all(&data).ok();
    }
}
