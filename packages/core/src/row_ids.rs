//! The install's ids for the rows `add-row` gives an id of its own, in `$DATA/registry/row-ids.json`
//! next to the core's `ids.json` (docs/std/registry.md "Stat ids"). A key names an addon's row,
//! `<addon>/<table>.<row>/<label>`, and keeps its id at every build of every profile, whatever else
//! the build adds, since careers in the shared Steam save hold the ids.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use swofty_formats::gvdb::{Gvdb, instance_count_path};
use swofty_formats::hash::fnv1a64;

use crate::ops::{
    Claimed, DbOp, DbOps, IdFrom, RowIds, UNLOCK_ROW, add_row_name, is_unlock_ids, row_shape, stat_id_field,
    unlock_list,
};
use crate::stdlib::reports;
use crate::{Loaded, Result, io_err};

mod recover;

pub use recover::Recovery;

pub const FILE: &str = "row-ids.json";
/// The table name of the track rows `track.add` adds; the game reads their `statId` in a byte, 0xff = none.
const TRACK_TABLE: &str = "tracks.Track";
pub const TRACK_STAT_MAX: i64 = 254;

/// The `<table>.<row>/<label>` name of the track row `track.add` adds for the track `code`, as a build's added-row
/// ids and [`Overrides::rows`] name it.
pub fn track_row(code: &str) -> String {
    format!("{TRACK_TABLE}/{code}")
}
const LOCK: &str = "row-ids.lock";
const VERSION: u32 = 2;
const ALIASES_VERSION: u32 = 1;

/// The core's registries whose `ids.json` ids are ids of these tables' rows.
const REGISTRY_TABLES: &[(&str, &str)] = &[
    ("vehicle", "entrants.vehicles.vehicle"),
    (
        "vehicle.package",
        "entrants.vehicles.vehicle_packages.vehicle_package",
    ),
    ("content.package", "entrants.entrant_lists.entrant_list"),
    ("content.package.scenario", "game_setup.scenarios.scenario"),
    ("team.identity", "entrants.teams.team"),
    ("driver.identity", "entrants.drivers.driver"),
    ("driver.head", "entrants.animation.driver_heads.driver_head"),
    // The core's livery unlock pool spans every unlock list ([`pool`]).
    ("livery.unlock", UNLOCK_POOL),
];

/// The name the core's livery unlock ids go under: the pool is one for all unlock lists.
const UNLOCK_POOL: &str = "unlockable_content.unlockable_items";

/// The name `ids.json`'s ids and careers of `table`'s rows go under: every unlock list's id space
/// (`<list>.unlockable_item`) shares the core's one livery unlock pool.
fn pool(table: &str) -> &str {
    if is_unlock_ids(table) { UNLOCK_POOL } else { table }
}

/// The core registries whose careers never hold ids: saves name vehicle packages by label, not by `stat_id`.
const UNHELD_REGISTRIES: &[&str] = &["vehicle.package"];

/// The tables saves hold ids of, and the name `swofty/id_aliases.json` gives each.
const ALIAS_TABLES: &[(&str, &str)] = &[
    ("entrants.teams.team", "teams"),
    ("entrants.vehicles.vehicle", "vehicles"),
    ("entrants.drivers.driver", "drivers"),
];

pub const ALIASES_FILE: &str = "id_aliases.json";

/// The environment variable the launcher sets for `swofty run` to the file of the host's ids for a room
/// launch ([`Overrides`]).
pub const ENV: &str = "SWOFTY_ROOM_ROW_IDS";

/// `swofty/id_aliases.json`: for each of `teams`, `vehicles` and `drivers`, the ids added rows had
/// before (1.6.10 and 1.6.11 hashed ids, ids before 1.6.10) mapped to the id they have now. The core
/// reads it at StateLoadSession to load a save made under the earlier ids.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdAliases {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub tables: BTreeMap<String, BTreeMap<i64, i64>>,
}

impl IdAliases {
    pub fn is_empty(&self) -> bool {
        self.tables.values().all(BTreeMap::is_empty)
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        let file = IdAliases {
            version: ALIASES_VERSION,
            tables: self.tables.clone(),
        };
        crate::write_json(path, &file)
    }
}

/// What a room's host lends a joiner for one session's builds, so both copies' fingerprints match: the
/// ids the host gave the addons' rows (`rows`, by addon and then `<table>.<row>/<label>`, in place of the
/// ids the joiner's own `row-ids.json` keeps), the host's load order of the addons both have (`order`) and
/// the host's values of their options (`options`). None of it is written to `row-ids.json` (see
/// [`Store::lend`]) or the profile (see [`crate::profile::Profile::lend`]), and all of it is part of the
/// build's cache key.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overrides {
    pub rows: BTreeMap<String, BTreeMap<String, i64>>,
    /// The shared addons that change game data, in the host's load order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    /// The host's value of each option that differs from the joiner's, by addon and option id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
    /// The host's stat ids for the core's registry records of each shared addon, as
    /// `<registry>:<record>` to id. The core reads them from `swofty/room_stat_ids.json` for the
    /// session and keeps its own `ids.json` as it is. An addon with no ids here means the host's
    /// registry holds none for it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub registry: BTreeMap<String, BTreeMap<String, i64>>,
}

/// The file in the game's `swofty` folder that carries a room's [`Overrides::registry`] to the core.
pub const ROOM_STAT_IDS_FILE: &str = "room_stat_ids.json";

impl Overrides {
    pub fn is_empty(&self) -> bool {
        self.rows.values().all(BTreeMap::is_empty)
            && self.order.is_empty()
            && self.options.is_empty()
            && self.registry.is_empty()
    }

    /// The registry ids of the addons in `order`, as `swofty/room_stat_ids.json` holds them: registry to
    /// record to id. `None` when there are none to lend.
    pub fn room_stat_ids(&self, order: &[String]) -> Option<serde_json::Value> {
        let mut ids: BTreeMap<&str, BTreeMap<&str, i64>> = BTreeMap::new();
        for (_, entries) in self.registry.iter().filter(|(a, _)| order.contains(a)) {
            for (key, id) in entries {
                if let Some((registry, record)) = key.split_once(':') {
                    ids.entry(registry).or_default().insert(record, *id);
                }
            }
        }
        (!ids.is_empty()).then(|| serde_json::json!({ "version": 1, "ids": ids }))
    }

    /// These overrides with `newer` laid over them: a row, option or order `newer` has wins.
    pub fn merged(mut self, newer: Overrides) -> Overrides {
        for (addon, rows) in newer.rows {
            self.rows.entry(addon).or_default().extend(rows);
        }
        for (addon, options) in newer.options {
            self.options.entry(addon).or_default().extend(options);
        }
        self.registry.extend(newer.registry);
        if !newer.order.is_empty() {
            self.order = newer.order;
        }
        self
    }

    /// How many option values `options` carries.
    pub fn option_values(&self) -> usize {
        self.options.values().map(BTreeMap::len).sum()
    }

    /// `current` with the addons of `order` that it holds put in the host's order, each in a place an
    /// addon of `order` held; the rest stay where they are.
    pub fn arranged(&self, current: &[String]) -> Vec<String> {
        let mut host = self.order.iter().filter(|id| current.contains(id));
        current
            .iter()
            .map(|id| match self.order.contains(id) {
                true => host.next().cloned().unwrap_or_else(|| id.clone()),
                false => id.clone(),
            })
            .collect()
    }

    /// The overrides for the addons in `order`, as `<addon>/<table>.<row>/<label>` keys.
    fn keys(&self, order: &[String]) -> BTreeMap<String, i64> {
        order
            .iter()
            .filter_map(|addon| Some((addon, self.rows.get(addon)?)))
            .flat_map(|(addon, rows)| rows.iter().map(move |(row, id)| (format!("{addon}/{row}"), *id)))
            .collect()
    }

    /// What goes into the build's cache key for the addons in `order`; empty when none apply.
    pub fn stamp(&self, order: &[String]) -> Option<String> {
        let keys = self.keys(order);
        let ranked: Vec<&String> = self.order.iter().filter(|id| order.contains(id)).collect();
        let registry: BTreeMap<&String, &BTreeMap<String, i64>> = self
            .registry
            .iter()
            .filter(|(id, _)| order.contains(id))
            .collect();
        let options: BTreeMap<&String, &BTreeMap<String, serde_json::Value>> =
            self.options.iter().filter(|(id, _)| order.contains(id)).collect();
        let mut out = String::new();
        if !keys.is_empty() {
            out += &format!(
                "room row ids {};",
                serde_json::to_string(&keys).expect("serialisable")
            );
        }
        if !ranked.is_empty() || !options.is_empty() {
            out += &format!(
                "room settings {} {};",
                serde_json::to_string(&ranked).expect("serialisable"),
                serde_json::to_string(&options).expect("serialisable")
            );
        }
        if !registry.is_empty() {
            out += &format!(
                "room registry ids {};",
                serde_json::to_string(&registry).expect("serialisable")
            );
        }
        (!out.is_empty()).then_some(out)
    }

    pub fn read(path: &Path) -> Result<Self> {
        match crate::load_json::<Overrides>(path)? {
            Loaded::Found(o) => Ok(o),
            Loaded::Missing | Loaded::Damaged(_) => Ok(Self::default()),
        }
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        crate::write_json(path, self)
    }
}

static LENT: std::sync::Mutex<Option<Overrides>> = std::sync::Mutex::new(None);

/// Makes this process's builds use `overrides` (a room launch's), or none. The `swofty run` that starts
/// the game for a room calls it once; no other process does.
pub fn lend(overrides: Option<Overrides>) {
    *LENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = overrides.filter(|o| !o.is_empty());
}

pub fn lent() -> Option<Overrides> {
    LENT.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    rows: BTreeMap<String, Entry>,
    /// The profiles whose last build before the store seeded it.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    seeded: BTreeSet<String>,
    /// The ids 1.6.10 and 1.6.11 gave from the rows' keys were put back once (see [`Store::recover`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    recovered: bool,
    /// Every id a key's row had before the one it keeps, so saves made under those ids still load.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    aliases: BTreeMap<String, BTreeSet<i64>>,
    /// Per profile, the ids and aliases that differ from the install-wide ones above: a row of the profile
    /// that a clash there moved. Another profile never reads them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    profiles: BTreeMap<String, ProfileIds>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct ProfileIds {
    #[serde(default)]
    rows: BTreeMap<String, Entry>,
    #[serde(default)]
    aliases: BTreeMap<String, BTreeSet<i64>>,
}

impl ProfileIds {
    fn is_empty(&self) -> bool {
        self.rows.is_empty() && self.aliases.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    id: i64,
    from: IdFrom,
}

/// One row the store keeps an id for: its table, label, id and how it got it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub table: String,
    pub label: String,
    pub id: i64,
    pub from: IdFrom,
}

/// The rows of `addon` the store in `data` keeps ids for, by table and label.
pub fn kept_rows(data: &Path, addon: &str) -> Result<Vec<Kept>> {
    let file: File = match crate::load_json(&path(data))? {
        Loaded::Found(f) => f,
        Loaded::Missing | Loaded::Damaged(_) => File::default(),
    };
    let prefix = format!("{addon}/");
    Ok(file
        .rows
        .into_iter()
        .filter_map(|(key, e)| {
            let (table, label) = key.strip_prefix(&prefix)?.split_once('/')?;
            Some(Kept {
                table: table.to_owned(),
                label: label.to_owned(),
                id: e.id,
                from: e.from,
            })
        })
        .collect())
}

/// The addons the store in `data` keeps vehicle, engine, team, driver or head rows for: the rows a
/// save names, so a career saved with one on can't load with it off.
pub fn save_owners(data: &Path) -> BTreeSet<String> {
    let file: File = match crate::load_json(&path(data)) {
        Ok(Loaded::Found(f)) => f,
        _ => File::default(),
    };
    let held = |table: &str| {
        (table.starts_with("entrants.") && !table.starts_with("entrants.entrant_lists"))
            || table.contains("engine")
    };
    file.rows
        .keys()
        .chain(file.profiles.values().flat_map(|p| p.rows.keys()))
        .filter_map(|key| {
            let mut parts = key.splitn(3, '/');
            let (addon, table) = (parts.next()?, parts.next()?);
            held(table).then(|| addon.to_owned())
        })
        .collect()
}

/// The addons that own the team, vehicle and driver ids a save can hold ([`id_owners`]).
#[derive(Debug, Default, Clone)]
pub struct IdOwners {
    now: BTreeMap<(String, i64), BTreeSet<String>>,
    careers: BTreeMap<String, BTreeMap<(String, i64), BTreeSet<String>>>,
}

impl IdOwners {
    /// The addons that own `id` of `table` for `career`: the record the core's `ids.json` keeps for
    /// that career when it keeps one, else the addons that hold the id now, else the record another
    /// career keeps. An id an addon held before under `pin` is never its id.
    pub fn of(&self, career: &str, table: &str, id: i64) -> Option<&BTreeSet<String>> {
        let key = (table.to_owned(), id);
        self.careers
            .get(career)
            .and_then(|kept| kept.get(&key))
            .or_else(|| self.now.get(&key))
            .or_else(|| self.careers.values().find_map(|kept| kept.get(&key)))
    }
}

impl From<BTreeMap<(String, i64), BTreeSet<String>>> for IdOwners {
    fn from(now: BTreeMap<(String, i64), BTreeSet<String>>) -> Self {
        Self {
            now,
            careers: BTreeMap::new(),
        }
    }
}

/// The addons that own each team, vehicle or driver id a save can hold: the ids the store keeps for
/// their rows, the ids those rows had before, the ids the core's `ids.json` gives their team, vehicle
/// and driver records now, and the ids it keeps for each career.
pub fn id_owners(data: &Path) -> IdOwners {
    let file: File = match crate::load_json(&path(data)) {
        Ok(Loaded::Found(f)) => f,
        _ => File::default(),
    };
    let mut out = IdOwners::default();
    let mut put = |key: &str, id: i64| {
        let mut parts = key.splitn(3, '/');
        if let (Some(addon), Some(table)) = (parts.next(), parts.next())
            && ALIAS_TABLES.iter().any(|(t, _)| *t == table)
        {
            out.now
                .entry((table.to_owned(), id))
                .or_default()
                .insert(addon.to_owned());
        }
    };
    let profiles = file.profiles.values();
    for (key, entry) in file
        .rows
        .iter()
        .chain(profiles.clone().flat_map(|p| p.rows.iter()))
    {
        put(key, entry.id);
    }
    for (key, ids) in file.aliases.iter().chain(profiles.flat_map(|p| p.aliases.iter())) {
        for id in ids {
            put(key, *id);
        }
    }
    let Some(doc) = std::fs::read(reports::path(data, reports::IDS))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
    else {
        return out;
    };
    let held = REGISTRY_TABLES
        .iter()
        .filter(|(_, table)| ALIAS_TABLES.iter().any(|(t, _)| t == table));
    for (registry, table) in held {
        let records = |of: &Value| -> Vec<(i64, String)> {
            of[*registry]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(record, e)| {
                    let id = e.as_i64().or_else(|| e["id"].as_i64())?;
                    Some((id, record.split_once('/')?.0.to_owned()))
                })
                .collect()
        };
        for (id, addon) in records(&doc["ids"]) {
            out.now
                .entry(((*table).to_owned(), id))
                .or_default()
                .insert(addon);
        }
        for (career, kept) in doc["careers"].as_object().into_iter().flatten() {
            for (id, addon) in records(kept) {
                out.careers
                    .entry(career.clone())
                    .or_default()
                    .entry(((*table).to_owned(), id))
                    .or_default()
                    .insert(addon);
            }
        }
    }
    out
}

/// The addons the store in `data` keeps a row of `table` for, by the row's label.
pub fn label_owners(data: &Path, table: &str) -> BTreeMap<String, BTreeSet<String>> {
    let file: File = match crate::load_json(&path(data)) {
        Ok(Loaded::Found(f)) => f,
        _ => File::default(),
    };
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for key in file
        .rows
        .keys()
        .chain(file.profiles.values().flat_map(|p| p.rows.keys()))
    {
        let mut parts = key.splitn(3, '/');
        if let (Some(addon), Some(t), Some(label)) = (parts.next(), parts.next(), parts.next())
            && t == table
        {
            out.entry(label.to_owned()).or_default().insert(addon.to_owned());
        }
    }
    out
}

/// The highest `stat_id` the stock database `db` gives a team, vehicle or driver row, by table
/// (`entrants.teams.team`): an id above it in a save names a row an addon added. A table `db`
/// holds no rows of is left out.
pub fn stock_max_ids(db: &Gvdb) -> BTreeMap<String, i64> {
    let index: HashMap<u64, usize> = db.records.iter().enumerate().map(|(i, r)| (r.key, i)).collect();
    let text = |path: &str| {
        index
            .get(&fnv1a64(path))
            .map(|&i| db.records[i].text().trim().to_owned())
    };
    let mut out = BTreeMap::new();
    for (full, _) in ALIAS_TABLES {
        let Some((table, row)) = full.rsplit_once('.') else {
            continue;
        };
        let count = text(&instance_count_path(table, row))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let field = stat_id_field(table, row);
        let max = (0..count)
            .filter_map(|i| text(&format!("{table}.{row}_{i}.{field}"))?.parse::<i64>().ok())
            .max();
        if let Some(max) = max {
            out.insert((*full).to_owned(), max);
        }
    }
    out
}

pub fn path(data: &Path) -> PathBuf {
    reports::path(data, FILE)
}

/// Held while a build reads and writes `row-ids.json`; another build waits until it is dropped.
pub fn lock(data: &Path) -> Result<std::fs::File> {
    let dir = reports::dir(data);
    std::fs::create_dir_all(&dir).map_err(io_err(&dir))?;
    let path = dir.join(LOCK);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(io_err(&path))?;
    file.lock().map_err(io_err(&path))?;
    Ok(file)
}

/// What a build left of its database: its overlay's database file `rel`, when the build wrote one, and
/// its `swofty/assets.json`, whose database edits the core applies in game when the build routes the
/// addons' rows there (the overlay's file then holds the tracks' rows at most, or is not written).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastBuild {
    pub db: Option<PathBuf>,
    pub edits: Option<PathBuf>,
}

/// The profile's last build in `overlays`, other than the build `current`; for a profile with none,
/// the newest of every profile's builds beside it.
pub fn last_build(overlays: &Path, current: &str, rel: &str) -> Option<LastBuild> {
    let of = |d: PathBuf| {
        let file = |p: PathBuf| {
            let at = std::fs::metadata(&p)
                .ok()
                .filter(|m| m.is_file())?
                .modified()
                .ok()?;
            Some((at, p))
        };
        let db = file(d.join(rel));
        let edits = file(d.join(crate::runtime::ASSETS_FILE));
        let at = db.iter().chain(&edits).map(|(at, _)| *at).max()?;
        Some((
            at,
            LastBuild {
                db: db.map(|(_, p)| p),
                edits: edits.map(|(_, p)| p),
            },
        ))
    };
    let newest = |dirs: Vec<PathBuf>| {
        dirs.into_iter()
            .filter_map(of)
            .max_by_key(|(at, _)| *at)
            .map(|(_, b)| b)
    };
    let subdirs = |dir: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
            .unwrap_or_default()
    };
    let own = subdirs(overlays)
        .into_iter()
        .filter(|d| d.file_name().is_some_and(|n| n != current))
        .collect();
    newest(own).or_else(|| {
        let profiles = subdirs(overlays.parent()?);
        newest(
            profiles
                .iter()
                .filter(|p| p.as_path() != overlays)
                .flat_map(|p| subdirs(p))
                .collect(),
        )
    })
}

#[derive(Deserialize)]
struct Assets {
    #[serde(default)]
    db_edits: Vec<Edit>,
}

#[derive(Deserialize)]
struct Edit {
    hash: String,
    value: String,
}

// The last build's database as the game loads it: the overlay's file, with the build's runtime edits over it.
struct LastDb {
    db: Option<(Gvdb, HashMap<u64, usize>)>,
    edits: HashMap<u64, String>,
}

impl LastDb {
    fn read(last: &LastBuild) -> Option<Self> {
        let db = last
            .db
            .as_ref()
            .and_then(|p| Gvdb::parse(&std::fs::read(p).ok()?).ok())
            .map(|db| {
                let index = db.records.iter().enumerate().map(|(i, r)| (r.key, i)).collect();
                (db, index)
            });
        let edits: HashMap<u64, String> = last
            .edits
            .as_ref()
            .and_then(|p| serde_json::from_slice::<Assets>(&std::fs::read(p).ok()?).ok())
            .map(|a| {
                a.db_edits
                    .into_iter()
                    .filter_map(|e| Some((u64::from_str_radix(&e.hash, 16).ok()?, e.value)))
                    .collect()
            })
            .unwrap_or_default();
        (db.is_some() || !edits.is_empty()).then_some(Self { db, edits })
    }

    fn text(&self, path: &str) -> Option<String> {
        let key = fnv1a64(path);
        if let Some(v) = self.edits.get(&key) {
            return Some(v.clone());
        }
        let (db, index) = self.db.as_ref()?;
        index.get(&key).map(|&i| db.records[i].text().into_owned())
    }
}

#[derive(Default)]
struct Seed {
    last: Option<LastBuild>,
    db: Option<Option<LastDb>>,
    stock_rows: HashMap<String, usize>,
    tables: HashMap<String, HashMap<String, Vec<i64>>>,
}

impl Seed {
    // The id of the `nth` row labelled `label` that the last build added to `table`'s rows named `row`.
    fn id(&mut self, table: &str, row: &str, label: &str, nth: usize) -> Option<i64> {
        let last = self.last.as_ref()?;
        let db = self.db.get_or_insert_with(|| LastDb::read(last)).as_ref()?;
        let name = format!("{table}.{row}");
        let skip = self.stock_rows.get(&name).copied().unwrap_or(0);
        let rows = self.tables.entry(name).or_insert_with(|| {
            let field = stat_id_field(table, row);
            let n = db
                .text(&instance_count_path(table, row))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            let mut out: HashMap<String, Vec<i64>> = HashMap::new();
            for i in skip..n {
                let label = db.text(&format!("{table}.{row}_{i}._metadata_"));
                let id = db
                    .text(&format!("{table}.{row}_{i}.{field}"))
                    .and_then(|v| v.trim().parse().ok());
                if let (Some(label), Some(id)) = (label, id) {
                    out.entry(label).or_default().push(id);
                }
            }
            out
        });
        rows.get(label)?.get(nth).copied()
    }
}

/// The kept ids, the ids the core's registry holds, and what this build claimed and changed.
#[derive(Default)]
pub struct Store {
    rows: BTreeMap<String, i64>,
    from: HashMap<String, IdFrom>,
    seeded: BTreeSet<String>,
    recovered: bool,
    aliases: BTreeMap<String, BTreeSet<i64>>,
    profile: String,
    base: BTreeMap<String, Entry>,
    base_ids: HashMap<String, HashMap<i64, String>>,
    base_aliases: BTreeMap<String, BTreeSet<i64>>,
    local: BTreeSet<String>,
    local_aliases: BTreeMap<String, BTreeSet<i64>>,
    others: BTreeMap<String, ProfileIds>,
    existed: bool,
    by_id: HashMap<String, BTreeMap<i64, String>>,
    pinned: HashMap<String, HashMap<i64, String>>,
    career_held: HashMap<String, HashSet<i64>>,
    seed: Seed,
    claimed: HashSet<String>,
    building: HashSet<String>,
    adds_to: HashSet<(String, String)>,
    added: usize,
    moved: usize,
    reused: usize,
    changed: bool,
    read_only: bool,
    lent: BTreeMap<String, i64>,
    lent_build: HashSet<String>,
}

fn addon_of(key: &str) -> &str {
    key.split('/').next().unwrap_or(key)
}

fn table_of(key: &str) -> &str {
    key.split('/').nth(1).unwrap_or_default()
}

fn row_name(key: &str) -> String {
    let (addon, rest) = key.split_once('/').unwrap_or(("", key));
    format!("{rest} of {addon}")
}

impl Store {
    /// The store in `data`, the ids the core's `ids.json` holds (in `data`, else in the game folder
    /// `game`), and, at `profile`'s first build with the store, its last build's database `last` to
    /// seed rows the store has no id for. The strings are warnings.
    pub fn load(
        data: &Path,
        game: Option<&Path>,
        profile: &str,
        last: Option<LastBuild>,
    ) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let mut store = Self::default();
        let file = path(data);
        match crate::load_json::<File>(&file) {
            Ok(Loaded::Found(f)) => {
                store.read_only = f.version > VERSION;
                store.seeded = f.seeded;
                store.recovered = f.recovered;
                store.aliases = f.aliases.clone();
                store.base_aliases = f.aliases;
                store.existed = true;
                for (key, e) in &f.rows {
                    store
                        .base_ids
                        .entry(table_of(key).to_owned())
                        .or_default()
                        .insert(e.id, key.clone());
                }
                store.base = f.rows.clone();
                for (key, e) in f.rows {
                    store.from.insert(key.clone(), e.from);
                    store.insert(key, e.id);
                }
                let mut profiles = f.profiles;
                if let Some(mine) = profiles.remove(profile) {
                    for (key, e) in mine.rows {
                        store.from.insert(key.clone(), e.from);
                        store.local.insert(key.clone());
                        store.insert(key, e.id);
                    }
                    for (key, ids) in mine.aliases {
                        store.aliases.entry(key.clone()).or_default().extend(&ids);
                        store.local_aliases.insert(key, ids);
                    }
                }
                store.others = profiles;
            }
            Ok(Loaded::Missing) => {}
            Ok(Loaded::Damaged(to)) => warnings.push(format!(
                "database: {FILE} was damaged and was moved to {}; added rows take the ids of the last build",
                to.display()
            )),
            Err(e) => {
                store.read_only = true;
                warnings.push(format!(
                    "database: {FILE} can't be read ({e}); it is left as it is"
                ));
            }
        }
        if let Some(ids) = reports::found(data, game, reports::IDS)
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        {
            store.pinned = registry_ids(&ids);
            store.career_held = career_ids(&ids);
        }
        store.profile = profile.to_owned();
        if store.seeded.insert(profile.to_owned()) {
            store.seed.last = last;
            store.changed = true;
        }
        (store, warnings)
    }

    fn insert(&mut self, key: String, id: i64) {
        let table = key.split('/').nth(1).unwrap_or_default().to_owned();
        if let Some(old) = self.rows.insert(key.clone(), id)
            && let Some(ids) = self.by_id.get_mut(&table)
            && ids.get(&old) == Some(&key)
        {
            ids.remove(&old);
        }
        if let Some(prev) = self.by_id.entry(table).or_default().insert(id, key.clone())
            && prev != key
        {
            self.rows.remove(&prev);
        }
    }

    fn put(&mut self, key: &str, id: i64, from: IdFrom) {
        let table = table_of(key).to_owned();
        let kept = self.rows.get(key).copied();
        if kept == Some(id) {
            return;
        }
        let in_base = self.base.get(key).map(|e| e.id);
        let clashes = self
            .base_ids
            .get(&table)
            .and_then(|ids| ids.get(&id))
            .is_some_and(|k| k != key);
        let local = kept.is_some() || in_base.is_some() || clashes;
        match kept {
            Some(old) => {
                self.moved += 1;
                self.alias(key, old, local);
            }
            None => self.added += 1,
        }
        self.insert(key.to_owned(), id);
        self.from.insert(key.to_owned(), from);
        if in_base == Some(id) {
            self.local.remove(key);
        } else if local {
            self.local.insert(key.to_owned());
        } else {
            self.base.insert(key.to_owned(), Entry { id, from });
            self.base_ids.entry(table).or_default().insert(id, key.to_owned());
        }
        self.changed = true;
    }

    fn base_put(&mut self, key: &str, id: i64, from: IdFrom) {
        self.base_drop(key);
        self.base.insert(key.to_owned(), Entry { id, from });
        self.base_ids
            .entry(table_of(key).to_owned())
            .or_default()
            .insert(id, key.to_owned());
    }

    fn base_drop(&mut self, key: &str) {
        if let Some(old) = self.base.remove(key)
            && let Some(ids) = self.base_ids.get_mut(table_of(key))
            && ids.get(&old.id).is_some_and(|k| k == key)
        {
            ids.remove(&old.id);
        }
    }

    fn alias(&mut self, key: &str, old: i64, local: bool) {
        if self.aliases.entry(key.to_owned()).or_default().insert(old) {
            self.changed = true;
        }
        let into = if local {
            &mut self.local_aliases
        } else {
            &mut self.base_aliases
        };
        into.entry(key.to_owned()).or_default().insert(old);
    }

    /// Gives the rows of `overrides` for the addons in `order` the host's ids for this build only: the store is
    /// not written afterwards, and the aliases leave these rows out. An id another row of the build keeps
    /// (one in `order`, not itself overridden) or a registry record of an addon in `order` holds is
    /// refused, naming both rows. A registry record of an addon outside the build doesn't register this
    /// session, so its id is lent.
    pub fn lend(&mut self, overrides: &Overrides, order: &[String]) -> std::result::Result<usize, String> {
        let wanted = overrides.keys(order);
        self.lent_build = order.iter().cloned().collect();
        for key in wanted.keys() {
            if let Some(old) = self.rows.remove(key)
                && let Some(ids) = self.by_id.get_mut(table_of(key))
                && ids.get(&old).is_some_and(|k| k == key)
            {
                ids.remove(&old);
            }
        }
        let mut given: HashMap<(&str, i64), &str> = HashMap::new();
        for (key, &id) in &wanted {
            let table = table_of(key);
            if let Some(&other) = given.get(&(table, id)) {
                return Err(format!(
                    "the host's stat id {id} is given to both {} and {}",
                    row_name(other),
                    row_name(key)
                ));
            }
            given.insert((table, id), key);
            if let Some(other) = self.by_id.get(table).and_then(|ids| ids.get(&id))
                && order.iter().any(|a| a == addon_of(other))
            {
                return Err(format!(
                    "the host's stat id {id} for {} is the one your {} uses",
                    row_name(key),
                    row_name(other)
                ));
            }
            if let Some(record) = self.pinned.get(pool(table)).and_then(|ids| ids.get(&id))
                && self.pin_holds(record)
            {
                return Err(format!(
                    "the host's stat id {id} for {} is held by the game's registry record {record} here",
                    row_name(key)
                ));
            }
        }
        for (key, &id) in &wanted {
            self.insert(key.clone(), id);
            self.from.insert(key.clone(), IdFrom::Seeded);
        }
        self.lent = wanted;
        self.read_only = true;
        Ok(self.lent.len())
    }

    /// Whether the row `key` holds `id` of `table` without this build giving it that id: its addon is
    /// left out of the build, or is in it but adds no rows to the table, and no career uses `id`.
    fn spare_row(&self, table: &str, id: i64, key: &str) -> bool {
        let addon = addon_of(key);
        let adds = self.adds_to.contains(&(addon.to_owned(), table.to_owned()));
        (!self.building.contains(addon) || (!adds && !self.claimed.contains(key)))
            && !self
                .career_held
                .get(pool(table))
                .is_some_and(|ids| ids.contains(&id))
    }

    /// Whether `record`, which `ids.json` holds `id` for, belongs to an addon this build leaves out and no career
    /// uses `id`: a full table may give the id to a row, and the core's registry moves the record when its addon
    /// is turned on.
    fn spare_pin(&self, table: &str, id: i64, record: &str) -> bool {
        let addon = addon_of(record);
        record.contains('/')
            && addon != "game"
            && !self.building.contains(addon)
            && !self.lent_build.contains(addon)
            && !self
                .career_held
                .get(pool(table))
                .is_some_and(|ids| ids.contains(&id))
    }

    /// Whether a registry record keeps its id from this build. While a host's ids are lent, a record of an
    /// addon outside the lent build doesn't register this session, so its id is free.
    fn pin_holds(&self, record: &str) -> bool {
        self.lent_build.is_empty() || !record.contains('/') || self.lent_build.contains(addon_of(record))
    }

    /// The first lent row the build gave another id than the host's, as the message that refuses the build.
    pub fn lent_missed(&self) -> Option<String> {
        self.lent.iter().find_map(|(key, &id)| {
            let got = self.rows.get(key).copied();
            (got != Some(id)).then(|| {
                format!(
                    "the host's stat id {id} for {} can't be used here: the build gave it {}",
                    row_name(key),
                    got.map_or("none".to_owned(), |g| g.to_string())
                )
            })
        })
    }

    /// The ids rows had before the ones they keep, by the tables saves hold them of; see [`IdAliases`].
    /// Only rows this build's database edits added give aliases: a kept row of an addon that now adds it
    /// through the core's registry has the id its registry record pins, not the one kept here. An old id
    /// a registry record pins is a live row's id, so it maps nowhere.
    pub fn id_aliases(&self) -> IdAliases {
        let mut out = IdAliases::default();
        let mut seen: HashMap<(&str, i64), Option<i64>> = HashMap::new();
        for (key, olds) in &self.aliases {
            if !self.building.is_empty() && !self.building.contains(addon_of(key)) {
                continue;
            }
            if !self.claimed.contains(key) {
                continue;
            }
            let Some(&now) = self.rows.get(key).filter(|_| !self.lent.contains_key(key)) else {
                continue;
            };
            let table = key.split('/').nth(1).unwrap_or_default();
            for &old in olds {
                if old == now
                    || self.by_id.get(table).is_some_and(|ids| ids.contains_key(&old))
                    || self
                        .pinned
                        .get(pool(table))
                        .is_some_and(|ids| ids.contains_key(&old))
                {
                    continue;
                }
                if self
                    .base_ids
                    .get(table)
                    .and_then(|ids| ids.get(&old))
                    .is_some_and(|holder| holder != key)
                {
                    continue;
                }
                match seen.entry((table, old)) {
                    std::collections::hash_map::Entry::Vacant(v) => {
                        v.insert(Some(now));
                    }
                    std::collections::hash_map::Entry::Occupied(mut o) => {
                        if o.get() != &Some(now) {
                            o.insert(None);
                        }
                    }
                }
            }
        }
        for ((table, old), now) in seen {
            let (Some(now), Some(name)) = (
                now,
                ALIAS_TABLES.iter().find(|(t, _)| *t == table).map(|(_, n)| *n),
            ) else {
                continue;
            };
            out.tables.entry(name.to_owned()).or_default().insert(old, now);
        }
        out
    }

    /// Reads, for each table the `add-row` ops in `files` add rows to, how many rows `db` holds from
    /// the stock database (`stock` holds its keys), so the seed matches only the rows the last build
    /// added. `tracks` holds the keys of the rows the tracks added.
    pub fn plan<'a>(
        &mut self,
        db: &Gvdb,
        stock: &HashSet<u64>,
        tracks: &HashSet<u64>,
        files: impl IntoIterator<Item = (&'a str, &'a DbOps)>,
    ) {
        let _ = tracks;
        self.adds_to.clear();
        let mut tables: BTreeSet<(String, String)> = BTreeSet::new();
        for (addon, ops) in files {
            for op in &ops.ops {
                let DbOp::AddRow { table, from, .. } = op else {
                    continue;
                };
                if let Some(row) = add_row_name(table, from) {
                    self.adds_to.insert((addon.to_owned(), format!("{table}.{row}")));
                }
                if let Some(list) = unlock_list(table) {
                    self.adds_to
                        .insert((addon.to_owned(), format!("{list}.{UNLOCK_ROW}")));
                }
                if table.contains(['[', '{']) || row_shape(table).contains("_N") {
                    continue;
                }
                if let Some(row) = add_row_name(table, from) {
                    tables.insert((table.clone(), row.to_owned()));
                }
            }
        }
        let index: HashMap<u64, usize> = db.records.iter().enumerate().map(|(i, r)| (r.key, i)).collect();
        for (table, row) in tables {
            let count = index
                .get(&fnv1a64(instance_count_path(&table, &row)))
                .and_then(|&i| db.records[i].text().trim().parse::<usize>().ok())
                .unwrap_or(0);
            let stock_rows = (0..count)
                .take_while(|i| stock.contains(&fnv1a64(format!("{table}.{row}_{i}._metadata_"))))
                .count();
            self.seed.stock_rows.insert(format!("{table}.{row}"), stock_rows);
        }
    }

    /// Starts a build of `addons`; ids kept for other addons' rows are reused when a table is full.
    pub fn building(&mut self, addons: impl IntoIterator<Item = String>) {
        self.building = addons.into_iter().collect();
        self.claimed.clear();
        (self.added, self.moved, self.reused) = (0, 0, 0);
    }

    /// The id kept for `key`.
    pub fn get(&self, key: &str) -> Option<i64> {
        self.rows.get(key).copied()
    }

    /// The stat id of the track row `addon` adds for the track `code` through `track.add`, kept in the store under
    /// `<addon>/tracks.Track/<code>`: the id kept for it, else `seed` (the id the last build gave it), else the first
    /// free id from `top`, the table's highest plus one, to [`TRACK_STAT_MAX`] and then from 0. `held` says whether a
    /// track row of this build holds an id. An id kept for any other track stays that track's whether or not its addon
    /// is on, so no id is ever given to two tracks; only when every id is kept does a track whose addon the build
    /// leaves out give its id up.
    pub fn track_stat_id(
        &mut self,
        addon: &str,
        code: &str,
        seed: Option<i64>,
        top: i64,
        held: &dyn Fn(i64) -> bool,
    ) -> std::result::Result<(i64, String), String> {
        let base = format!("{addon}/{}", track_row(code));
        let mut nth = 0;
        let key = loop {
            let key = match nth {
                0 => base.clone(),
                n => format!("{base}#{}", n + 1),
            };
            if self.claimed.insert(key.clone()) {
                break key;
            }
            nth += 1;
        };
        let blocked = |s: &Self, id: i64| {
            !(0..=TRACK_STAT_MAX).contains(&id)
                || held(id)
                || s.by_id
                    .get(TRACK_TABLE)
                    .and_then(|ids| ids.get(&id))
                    .is_some_and(|k| *k != key)
        };
        let kept = self.get(&key);
        if let Some(id) = kept.filter(|id| !blocked(self, *id)) {
            return Ok((id, format!("stat id {id} (kept from {FILE})")));
        }
        if kept.is_none()
            && let Some(id) = seed.filter(|id| !blocked(self, *id))
        {
            self.put(&key, id, IdFrom::Seeded);
            return Ok((id, format!("stat id {id} (as the last build gave it)")));
        }
        let from = match kept {
            Some(_) => IdFrom::Moved,
            None => IdFrom::Next,
        };
        let free = (top.max(0)..=TRACK_STAT_MAX)
            .chain(0..top.clamp(0, TRACK_STAT_MAX + 1))
            .find(|id| !blocked(self, *id));
        // Every id is kept for some track ever installed: the id of a track whose addon this build leaves out goes to
        // this one. A track of an addon in the build never lends its id, whether or not the build reached it yet.
        // Saves name a track by its id and no record says which ids careers use, so a calendar that named the left-out
        // track shows this one from then on. The left-out track takes a free or spare id again when it is turned on;
        // the ids UDP telemetry reports never change with it (udp_ids.rs, swofty_track_id).
        let spare = || {
            self.by_id
                .get(TRACK_TABLE)
                .into_iter()
                .flatten()
                .filter(|(id, k)| {
                    **k != key
                        && (0..=TRACK_STAT_MAX).contains(*id)
                        && !held(**id)
                        && !self.claimed.contains(*k)
                        && !self.building.contains(addon_of(k))
                })
                .map(|(id, k)| (*id, k.clone()))
                .min()
        };
        let (id, from, lent) = match free {
            Some(id) => (id, from, None),
            None => match spare() {
                Some((id, k)) => (id, IdFrom::Reused, Some(k)),
                None => {
                    return Err(format!(
                        "{code}: tracks have no free statId up to {TRACK_STAT_MAX}, the highest the game reads, and every track that holds \
                         one is in this build"
                    ));
                }
            },
        };
        self.put(&key, id, from);
        if let Some(k) = lent {
            self.reused += 1;
            return Ok((
                id,
                format!(
                    "stat id {id} (every id is kept; this one was {}, whose addon is off)",
                    row_name(&k)
                ),
            ));
        }
        let why = match kept {
            Some(old) => format!("stat id {id} (the {old} kept for it is held by another track)"),
            None => format!("stat id {id} (new, kept in {FILE})"),
        };
        Ok((id, why))
    }

    /// The build line saying what the last build changed, if anything.
    pub fn summary(&self) -> Option<String> {
        if self.added + self.moved + self.reused == 0 {
            return None;
        }
        let mut parts = vec![format!("{} new", self.added)];
        if self.moved > 0 {
            parts.push(format!("{} moved", self.moved));
        }
        if self.reused > 0 {
            parts.push(format!("{} reused", self.reused));
        }
        Some(format!(
            "database: {FILE} keeps {} added-row id(s) ({})",
            self.rows.len(),
            parts.join(", ")
        ))
    }

    /// Writes the store to `data` when this build changed it.
    /// Whether this build gave an id, or anything else, that [`Store::save`] would write.
    pub fn unsaved(&self) -> bool {
        self.changed && !self.read_only
    }

    pub fn save(&self, data: &Path) -> Result<()> {
        if !self.unsaved() {
            return Ok(());
        }
        let mut profiles = self.others.clone();
        let rows = self
            .local
            .iter()
            .filter_map(|key| {
                let id = *self.rows.get(key)?;
                let from = self.from.get(key).copied().unwrap_or(IdFrom::Hashed);
                Some((key.clone(), Entry { id, from }))
            })
            .collect();
        let mine = ProfileIds {
            rows,
            aliases: self.local_aliases.clone(),
        };
        if !mine.is_empty() {
            profiles.insert(self.profile.clone(), mine);
        }
        let file = File {
            version: VERSION,
            rows: self.base.clone(),
            seeded: self.seeded.clone(),
            recovered: self.recovered,
            aliases: self.base_aliases.clone(),
            profiles,
        };
        crate::write_json(&path(data), &file)
    }
}

// The ids the careers in `ids.json` use, by the table whose rows they are ids of.
fn career_ids(doc: &Value) -> HashMap<String, HashSet<i64>> {
    let mut out: HashMap<String, HashSet<i64>> = HashMap::new();
    let Some(careers) = doc["careers"].as_object() else {
        return out;
    };
    for (registry, table) in REGISTRY_TABLES {
        let ids = out.entry((*table).to_owned()).or_default();
        if UNHELD_REGISTRIES.contains(registry) {
            continue;
        }
        for career in careers.values() {
            if let Some(records) = career[*registry].as_object() {
                ids.extend(
                    records
                        .values()
                        .filter_map(|e| e.as_i64().or_else(|| e["id"].as_i64())),
                );
            }
        }
    }
    out.retain(|_, ids| !ids.is_empty());
    out
}

// The ids `ids.json` holds and its careers use, as `(table, id, record)`: the table whose rows they are
// ids of and the `<addon>/<label>` record each is the id of. A registry's own ids come before its careers'.
fn registry_records(doc: &Value) -> Vec<(&'static str, i64, &str)> {
    let mut out = Vec::new();
    for (registry, table) in REGISTRY_TABLES {
        if let Some(records) = doc["ids"][*registry].as_object() {
            out.extend(records.iter().filter_map(|(record, e)| {
                e.as_i64()
                    .or_else(|| e["id"].as_i64())
                    .map(|id| (*table, id, record.as_str()))
            }));
        }
        if let Some(careers) = doc["careers"]
            .as_object()
            .filter(|_| !UNHELD_REGISTRIES.contains(registry))
        {
            for career in careers.values() {
                if let Some(records) = career[*registry].as_object() {
                    out.extend(
                        records
                            .iter()
                            .filter_map(|(record, e)| e.as_i64().map(|id| (*table, id, record.as_str()))),
                    );
                }
            }
        }
    }
    out
}

// The ids `ids.json` holds and its careers use, by the table whose rows they are ids of.
fn registry_ids(doc: &Value) -> HashMap<String, HashMap<i64, String>> {
    let mut out: HashMap<String, HashMap<i64, String>> = HashMap::new();
    for (table, id, record) in registry_records(doc) {
        out.entry(table.to_owned())
            .or_default()
            .insert(id, record.to_owned());
    }
    out
}

/// The store as one addon's build sees it.
pub struct Scoped<'a> {
    pub addon: &'a str,
    pub store: &'a RefCell<Store>,
}

impl RowIds for Scoped<'_> {
    fn claim(&self, table: &str, row: &str, label: &str) -> Claimed {
        let mut s = self.store.borrow_mut();
        let base = format!("{}/{table}.{row}/{label}", self.addon);
        let mut nth = 0;
        let key = loop {
            let key = match nth {
                0 => base.clone(),
                n => format!("{base}#{}", n + 1),
            };
            if !s.claimed.contains(&key) {
                break key;
            }
            nth += 1;
        };
        s.claimed.insert(key.clone());
        let kept = s.get(&key);
        let seed = match kept {
            Some(_) => None,
            None => s.seed.id(table, row, label, nth),
        };
        Claimed { key, kept, seed }
    }

    fn taken(&self, table: &str, row: &str, key: &str, id: i64) -> bool {
        let s = self.store.borrow();
        let table = format!("{table}.{row}");
        s.by_id
            .get(&table)
            .and_then(|ids| ids.get(&id))
            .is_some_and(|k| k != key)
            || s.pinned
                .get(pool(&table))
                .and_then(|ids| ids.get(&id))
                .is_some_and(|record| {
                    s.pin_holds(record)
                        && !(s.spare_pin(&table, id, record)
                            && s.by_id
                                .get(&table)
                                .and_then(|ids| ids.get(&id))
                                .is_some_and(|k| k == key))
                })
    }

    fn keep(&self, _: &str, _: &str, key: &str, id: i64, from: IdFrom) {
        self.store.borrow_mut().put(key, id, from);
    }

    fn registry_record(&self, table: &str, row: &str, label: Option<&str>, id: i64) -> Option<String> {
        let s = self.store.borrow();
        let table = format!("{table}.{row}");
        let unheld = REGISTRY_TABLES
            .iter()
            .any(|(registry, t)| *t == table && UNHELD_REGISTRIES.contains(registry));
        let record = s.pinned.get(&table).filter(|_| !unheld)?.get(&id)?;
        let own = addon_of(record) == self.addon || record.rsplit('/').next() == label;
        (s.pin_holds(record) && !own).then(|| record.clone())
    }

    fn reclaim(
        &self,
        table: &str,
        row: &str,
        key: &str,
        free: &dyn Fn(i64) -> bool,
    ) -> Option<(i64, String)> {
        let table = format!("{table}.{row}");
        let spare: Vec<(i64, String)> = {
            let s = self.store.borrow();
            let pinned = s.pinned.get(pool(&table));
            let mut spare: Vec<(i64, String)> = s
                .by_id
                .get(&table)
                .into_iter()
                .flatten()
                .filter(|(id, k)| s.spare_row(&table, **id, k) && pinned.is_none_or(|p| !p.contains_key(id)))
                .map(|(id, k)| (*id, k.clone()))
                .collect();
            let mut released: Vec<(i64, String)> = pinned
                .into_iter()
                .flatten()
                .filter(|(id, record)| {
                    s.spare_pin(&table, **id, record)
                        && !s.by_id.get(&table).is_some_and(|ids| ids.contains_key(*id))
                })
                .map(|(id, record)| (*id, record.clone()))
                .collect();
            released.sort();
            spare.extend(released);
            spare
        };
        let (id, from) = spare.into_iter().find(|(id, _)| free(*id))?;
        let mut s = self.store.borrow_mut();
        s.put(key, id, IdFrom::Reused);
        s.reused += 1;
        Some((id, from))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_hosts_order_fills_the_places_its_addons_held_and_is_part_of_the_cache_key() {
        let host = Overrides {
            order: vec!["c".into(), "a".into()],
            options: BTreeMap::from([(
                "a".to_owned(),
                BTreeMap::from([("laps".to_owned(), serde_json::json!(5))]),
            )]),
            ..Default::default()
        };
        let own: Vec<String> = ["a", "x", "c"].map(String::from).to_vec();
        assert_eq!(host.arranged(&own), ["c", "x", "a"]);
        assert!(!host.is_empty());
        let stamp = host.stamp(&own).unwrap();
        assert!(
            stamp.contains("room settings") && stamp.contains("laps"),
            "{stamp}"
        );
        assert_eq!(
            host.stamp(&["x".to_owned()]),
            None,
            "an addon outside the build adds nothing"
        );
        let other = Overrides {
            options: BTreeMap::from([(
                "a".to_owned(),
                BTreeMap::from([("laps".to_owned(), serde_json::json!(6))]),
            )]),
            ..host.clone()
        };
        assert_ne!(other.stamp(&own), host.stamp(&own));
    }

    use super::*;
    use crate::asset::{Features, Op, Origin};
    use crate::ops::{DbReport, stat_id_ceiling};
    use crate::planner::Planner;
    use std::collections::BTreeSet;
    use swofty_formats::gvdb::Record;

    const VEHICLES: (&str, &str) = ("entrants.vehicles", "vehicle");
    const PACKAGES: (&str, &str) = ("entrants.vehicles.vehicle_packages", "vehicle_package");

    fn db((table, row): (&str, &str), stock: i64) -> Gvdb {
        Gvdb {
            records: vec![
                Record::new(&format!("{table}._{row}_instancecount_"), 0, "1"),
                Record::new(&format!("{table}.{row}_0._metadata_"), 4, "base"),
                Record::new(&format!("{table}.{row}_0.stat_id"), 0, stock.to_string()),
            ],
        }
    }

    pub(super) struct Built {
        pub(super) ids: BTreeMap<String, i64>,
        pub(super) store: Store,
        pub(super) lines: Vec<String>,
        pub(super) left_out: BTreeMap<String, usize>,
    }

    fn try_build(
        table: (&str, &str),
        stock: i64,
        addons: &[(&str, &[&str])],
        store: Store,
    ) -> crate::Result<Built> {
        try_build_on(db(table, stock), table, addons, store)
    }

    pub(super) fn try_build_on(
        mut gvdb: Gvdb,
        (table, row): (&str, &str),
        addons: &[(&str, &[&str])],
        store: Store,
    ) -> crate::Result<Built> {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in);
        for (addon, labels) in addons {
            p.add_fields(
                addon,
                BTreeMap::from([(
                    format!("{table}.{row}"),
                    vec!["_metadata_".to_owned(), "stat_id".to_owned()],
                )]),
            );
            for label in *labels {
                let (label, set_id) = match label.split_once('@') {
                    Some((l, id)) => (l, format!(r#","stat_id":"{id}""#)),
                    None => (*label, String::new()),
                };
                let json = format!(
                    r#"{{"op":"db.add-row","table":"{table}","from":"{table}.{row}_0","set":{{"_metadata_":"{label}"{set_id}}}}}"#
                );
                p.push(Op {
                    op: serde_json::from_str(&json).unwrap(),
                    origin: Origin {
                        addon: (*addon).to_owned(),
                        item: None,
                    },
                });
            }
        }
        p.keep_row_ids(store);
        let first = gvdb.instance_count(table, row).unwrap();
        let stock_keys = gvdb.records.iter().map(|r| r.key).collect();
        let mut report = DbReport::default();
        p.apply_db_report(&mut gvdb, &stock_keys, &HashSet::new(), &mut report)?;
        let n = gvdb.instance_count(table, row).unwrap();
        let field = stat_id_field(table, row);
        let ids = (first..n)
            .map(|i| {
                let label = gvdb.get_text(&format!("{table}.{row}_{i}._metadata_")).unwrap();
                let id = gvdb.get_text(&format!("{table}.{row}_{i}.{field}")).unwrap();
                (label, id.parse().unwrap())
            })
            .collect();
        let left_out = p.take_id_left_out();
        Ok(Built {
            ids,
            store: p.take_row_ids().unwrap(),
            lines: report.lines,
            left_out,
        })
    }

    fn build(table: (&str, &str), stock: i64, addons: &[(&str, &[&str])], store: Store) -> Built {
        try_build(table, stock, addons, store).unwrap()
    }

    fn data_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-row-ids-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn put(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn key(addon: &str, (table, row): (&str, &str), label: &str) -> String {
        format!("{addon}/{table}.{row}/{label}")
    }

    pub(super) const TEAMS: (&str, &str) = ("entrants.teams", "team");

    const STOCK_TRACKS: i64 = 46;

    fn track_build(store: &mut Store, on: &[(&str, &str)], seeds: &[(&str, i64)]) -> BTreeMap<String, i64> {
        store.building(on.iter().map(|(a, _)| (*a).to_owned()));
        let mut rows: BTreeSet<i64> = (0..STOCK_TRACKS).collect();
        let mut out = BTreeMap::new();
        for (addon, code) in on {
            let seed = seeds.iter().find(|(c, _)| c == code).map(|(_, id)| *id);
            let top = rows.iter().max().map_or(0, |m| m + 1);
            let held = rows.clone();
            let (id, _) = store
                .track_stat_id(addon, code, seed, top, &|id| held.contains(&id))
                .unwrap();
            rows.insert(id);
            out.insert((*code).to_owned(), id);
        }
        out
    }

    fn sixteen() -> Vec<(String, String)> {
        let mut v: Vec<(String, String)> = (0..15).map(|i| (format!("mod.t{i}"), format!("t{i}"))).collect();
        v.insert(9, ("mod.sokol".to_owned(), "sokol".to_owned()));
        v
    }

    fn refs(v: &[(String, String)]) -> Vec<(&str, &str)> {
        v.iter().map(|(a, c)| (a.as_str(), c.as_str())).collect()
    }

    #[test]
    fn a_track_keeps_its_stat_id_when_other_tracks_are_turned_off() {
        let all = sixteen();
        let mut store = Store::default();
        let first = track_build(&mut store, &refs(&all), &[]);
        assert_eq!(first["sokol"], STOCK_TRACKS + 9);
        let few: Vec<_> = all
            .iter()
            .filter(|(_, c)| c == "sokol" || c == "t2" || c == "t13")
            .cloned()
            .collect();
        let second = track_build(&mut store, &refs(&few), &[]);
        assert_eq!(second["sokol"], first["sokol"]);
        assert_eq!(second["t13"], first["t13"]);
        let only: Vec<_> = all.iter().filter(|(_, c)| c == "sokol").cloned().collect();
        assert_eq!(
            track_build(&mut store, &refs(&only), &[])["sokol"],
            first["sokol"]
        );
    }

    #[test]
    fn a_turned_off_tracks_stat_id_is_never_given_to_another_track() {
        let all = sixteen();
        let mut store = Store::default();
        let first = track_build(&mut store, &refs(&all), &[]);
        let off: Vec<_> = all.iter().filter(|(_, c)| c == "t14").cloned().collect();
        let mut with_new = refs(&off);
        with_new.insert(0, ("mod.fresh", "fresh"));
        let second = track_build(&mut store, &with_new, &[]);
        assert!(!first.values().any(|id| *id == second["fresh"]));
        assert_ne!(second["fresh"], first["t14"]);
        assert_ne!(second["fresh"], first["t13"]);
        let again = track_build(&mut store, &refs(&all), &[]);
        assert_eq!(again["t13"], first["t13"]);
        assert_eq!(again["t14"], first["t14"]);
        let ids: BTreeSet<i64> = again.values().copied().collect();
        assert_eq!(ids.len(), again.len());
    }

    #[test]
    fn a_track_without_a_kept_id_takes_the_id_the_last_build_gave_it() {
        let mut store = Store::default();
        let on = [("mod.sokol", "sokol"), ("mod.other", "other")];
        let got = track_build(&mut store, &on, &[("sokol", 48), ("other", 48)]);
        assert_eq!(got["sokol"], 48);
        assert_ne!(got["other"], 48);
        assert!(got["other"] >= STOCK_TRACKS);
    }

    #[test]
    fn a_lent_track_id_is_the_one_the_build_gives_even_where_a_turned_off_track_keeps_it() {
        let mut store = Store::default();
        let own = track_build(
            &mut store,
            &[
                ("mod.lemans", "le_mans_2017"),
                ("track.bahrain_outer", "bahrain_outer"),
                ("mod.brhgp", "brhgp"),
            ],
            &[],
        );
        assert_eq!(own["le_mans_2017"], STOCK_TRACKS);
        assert_eq!(own["brhgp"], STOCK_TRACKS + 2);
        let host = Overrides {
            rows: BTreeMap::from([
                (
                    "track.bahrain_outer".to_owned(),
                    BTreeMap::from([(track_row("bahrain_outer"), STOCK_TRACKS)]),
                ),
                (
                    "mod.brhgp".to_owned(),
                    BTreeMap::from([(track_row("brhgp"), STOCK_TRACKS + 6)]),
                ),
            ]),
            ..Overrides::default()
        };
        let on = [("track.bahrain_outer", "bahrain_outer"), ("mod.brhgp", "brhgp")];
        let order: Vec<String> = on.iter().map(|(a, _)| (*a).to_owned()).collect();
        assert_eq!(store.lend(&host, &order), Ok(2));
        let lent = track_build(&mut store, &on, &[]);
        assert_eq!(lent["bahrain_outer"], STOCK_TRACKS);
        assert_eq!(lent["brhgp"], STOCK_TRACKS + 6);
        assert_eq!(store.lent_missed(), None);
        assert!(!store.unsaved(), "row-ids.json keeps the player's own ids");
    }

    #[test]
    fn a_lent_track_id_a_track_of_the_build_keeps_is_refused_by_name() {
        let mut store = Store::default();
        let on = [
            ("mod.lemans", "le_mans_2017"),
            ("track.bahrain_outer", "bahrain_outer"),
        ];
        track_build(&mut store, &on, &[]);
        let host = Overrides {
            rows: BTreeMap::from([(
                "track.bahrain_outer".to_owned(),
                BTreeMap::from([(track_row("bahrain_outer"), STOCK_TRACKS)]),
            )]),
            ..Overrides::default()
        };
        let order: Vec<String> = on.iter().map(|(a, _)| (*a).to_owned()).collect();
        let err = store.lend(&host, &order).unwrap_err();
        assert!(err.contains("le_mans_2017"), "{err}");
    }

    #[test]
    fn track_stat_ids_stay_within_what_the_game_reads() {
        let mut store = Store::default();
        let (id, _) = store
            .track_stat_id("mod.a", "a", Some(300), 250, &|_| false)
            .unwrap();
        assert_eq!(id, 250);
        let err = store.track_stat_id("mod.b", "b", None, 0, &|_| true).unwrap_err();
        assert!(err.contains("254"), "{err}");
    }

    #[test]
    fn a_full_track_pool_lends_the_id_of_a_track_whose_addon_is_off() {
        let mut store = Store::default();
        for i in 0..=TRACK_STAT_MAX {
            store
                .track_stat_id(&format!("mod.t{i}"), "t", None, 0, &|_| false)
                .unwrap();
        }
        let off = store.get("mod.t7/tracks.Track/t").unwrap();
        let on: Vec<String> = (0..=TRACK_STAT_MAX)
            .filter(|i| *i != 7)
            .map(|i| format!("mod.t{i}"))
            .chain(["mod.new".to_owned()])
            .collect();
        store.building(on.clone());
        let before: Vec<i64> = (0..=TRACK_STAT_MAX)
            .filter(|i| *i != 7)
            .map(|i| store.get(&format!("mod.t{i}/tracks.Track/t")).unwrap())
            .collect();
        // The new track asks first: no track of the build has asked yet, and none of them lends its id.
        let (id, why) = store.track_stat_id("mod.new", "t", None, 0, &|_| false).unwrap();
        assert_eq!(id, off);
        assert!(why.contains("mod.t7"), "{why}");
        assert_eq!(store.get("mod.t7/tracks.Track/t"), None);
        let after: Vec<i64> = on
            .iter()
            .filter(|a| *a != "mod.new")
            .map(|a| store.track_stat_id(a, "t", None, 0, &|_| false).unwrap().0)
            .collect();
        assert_eq!(after, before);

        let mut full = Store::default();
        for i in 0..=TRACK_STAT_MAX {
            full.track_stat_id(&format!("mod.t{i}"), "t", None, 0, &|_| false)
                .unwrap();
        }
        full.building(
            (0..=TRACK_STAT_MAX)
                .map(|i| format!("mod.t{i}"))
                .chain(["mod.x".to_owned()]),
        );
        for i in 0..=TRACK_STAT_MAX {
            full.track_stat_id(&format!("mod.t{i}"), "t", None, 0, &|_| false)
                .unwrap();
        }
        let err = full.track_stat_id("mod.x", "t", None, 0, &|_| false).unwrap_err();
        assert!(err.contains("254"), "{err}");
    }

    #[test]
    fn a_kept_track_id_that_stock_now_holds_moves_and_is_aliased() {
        let mut store = Store::default();
        store.track_stat_id("mod.a", "a", None, 46, &|_| false).unwrap();
        store.building(["mod.a".to_owned()]);
        let (id, why) = store
            .track_stat_id("mod.a", "a", None, 47, &|id| id == 46)
            .unwrap();
        assert_eq!(id, 47);
        assert!(why.contains("46"), "{why}");
        assert_eq!(store.get("mod.a/tracks.Track/a"), Some(47));
    }

    #[test]
    fn new_ids_are_the_next_free_ids_in_load_order_as_builds_before_1_6_10_gave_them() {
        let first = build(
            VEHICLES,
            10,
            &[("a", &["a1", "a2"]), ("b", &["b1"])],
            Store::default(),
        );
        assert_eq!(
            first.ids,
            BTreeMap::from([("a1".into(), 11), ("a2".into(), 12), ("b1".into(), 13)])
        );
        assert!(
            first
                .lines
                .iter()
                .any(|l| l.ends_with(": stat_id 13 (the source's is 10)")),
            "{:?}",
            first.lines
        );
        assert_eq!(first.store.from[&key("b", VEHICLES, "b1")], IdFrom::Next);
        let again = build(
            VEHICLES,
            10,
            &[("a", &["a1", "a2"]), ("b", &["b1"])],
            Store::default(),
        );
        assert_eq!(
            again.ids, first.ids,
            "identical setups built fresh get identical ids"
        );
    }

    // minik8824 (1565755736410161313): a fresh data folder at 1.6.11 hashed every id, and his save from before
    // 1.6.10 held Classic Seasons 2019's teams at 550-559, the ids those builds gave them in load order.
    #[test]
    fn a_fresh_install_gives_a_save_from_before_1_6_10_its_sequential_ids_back() {
        let years: [(&str, &str); 5] = [
            ("swofty.classic-seasons-2017", "cs17"),
            ("swofty.classic-seasons-2018", "cs18"),
            ("swofty.classic-seasons-2020", "cs20"),
            ("swofty.classic-seasons-2021", "cs21"),
            ("swofty.classic-seasons-2019", "cs19"),
        ];
        let teams = [
            "mercedes",
            "ferrari",
            "redbull",
            "renault",
            "haas",
            "mclaren",
            "racingpoint",
            "alfaromeo",
            "tororosso",
            "williams",
        ];
        let labels: Vec<Vec<String>> = years
            .iter()
            .map(|(_, y)| teams.iter().map(|t| format!("{y}_{t}")).collect())
            .collect();
        let refs: Vec<Vec<&str>> = labels
            .iter()
            .map(|l| l.iter().map(String::as_str).collect())
            .collect();
        let set: Vec<(&str, &[&str])> = years
            .iter()
            .zip(&refs)
            .map(|((a, _), l)| (*a, l.as_slice()))
            .collect();
        let fresh = try_build_on(table_to(TEAMS, 509), TEAMS, &set, Store::default()).unwrap();
        let cs19: Vec<i64> = labels[4].iter().map(|l| fresh.ids[l]).collect();
        assert_eq!(cs19, (550..560).collect::<Vec<_>>());
        assert_eq!(fresh.ids["cs17_mercedes"], 510);
        let cs16: Vec<String> = teams.iter().map(|t| format!("cs16_{t}")).collect();
        let cs16: Vec<&str> = cs16.iter().map(String::as_str).collect();
        let mut more: Vec<(&str, &[&str])> = vec![("swofty.classic-seasons-2016", cs16.as_slice())];
        more.extend(set.iter().copied());
        let later = try_build_on(table_to(TEAMS, 509), TEAMS, &more, fresh.store).unwrap();
        let cs19_later: Vec<i64> = labels[4].iter().map(|l| later.ids[l]).collect();
        assert_eq!(
            cs19_later, cs19,
            "a year added in front later leaves the kept ids where they are"
        );
        assert_eq!(later.ids["cs16_mercedes"], 560);
    }

    #[test]
    fn an_order_swap_keeps_each_rows_id() {
        let first = build(
            VEHICLES,
            10,
            &[("a", &["a1", "a2"]), ("b", &["b1"])],
            Store::default(),
        );
        let swapped = build(VEHICLES, 10, &[("b", &["b1"]), ("a", &["a1", "a2"])], first.store);
        assert_eq!(swapped.ids, first.ids);
        assert!(
            swapped.lines.iter().any(|l| l.ends_with(&format!(
                ": stat_id {} (the source's is 10; kept from row-ids.json)",
                first.ids["b1"]
            ))),
            "{:?}",
            swapped.lines
        );
        assert_eq!(swapped.store.summary(), None);
    }

    #[test]
    fn an_update_that_adds_a_row_keeps_the_old_ids_and_gives_the_new_row_a_fresh_one() {
        let first = build(
            VEHICLES,
            10,
            &[("a", &["a1", "a2"]), ("b", &["b1"])],
            Store::default(),
        );
        let updated = build(
            VEHICLES,
            10,
            &[("a", &["a0", "a1", "a2"]), ("b", &["b1"])],
            first.store,
        );
        for label in ["a1", "a2", "b1"] {
            assert_eq!(updated.ids[label], first.ids[label]);
        }
        assert_eq!(updated.ids["a0"], 14);
        assert_eq!(
            updated.store.summary().as_deref(),
            Some("database: row-ids.json keeps 4 added-row id(s) (1 new)")
        );
    }

    #[test]
    fn a_removed_addon_keeps_its_ids_and_gets_them_back() {
        let first = build(VEHICLES, 10, &[("a", &["a1"]), ("b", &["b1"])], Store::default());
        let without = build(VEHICLES, 10, &[("b", &["b1"]), ("c", &["c1"])], first.store);
        assert_eq!(without.ids["b1"], first.ids["b1"]);
        assert_ne!(without.ids["c1"], first.ids["a1"]);
        let back = build(
            VEHICLES,
            10,
            &[("a", &["a1"]), ("b", &["b1"]), ("c", &["c1"])],
            without.store,
        );
        assert_eq!(back.ids["a1"], first.ids["a1"]);
        assert_eq!(back.ids["b1"], first.ids["b1"]);
        assert_eq!(back.ids["c1"], without.ids["c1"]);
    }

    #[test]
    fn two_profiles_share_a_rows_id_through_the_install_store() {
        let data = data_dir("profiles");
        let (store, warnings) = Store::load(&data, None, "one", None);
        assert!(warnings.is_empty(), "{warnings:?}");
        let one = build(VEHICLES, 10, &[("a", &["a1"]), ("b", &["b1"])], store);
        one.store.save(&data).unwrap();
        let saved: Value = serde_json::from_slice(&std::fs::read(path(&data)).unwrap()).unwrap();
        assert_eq!(
            saved["rows"]["b/entrants.vehicles.vehicle/b1"]["id"],
            one.ids["b1"]
        );
        assert_eq!(saved["rows"]["b/entrants.vehicles.vehicle/b1"]["from"], "next");
        let (store, _) = Store::load(&data, None, "two", None);
        let two = build(VEHICLES, 10, &[("b", &["b1"])], store);
        assert_eq!(two.ids["b1"], one.ids["b1"]);
        std::fs::remove_dir_all(&data).ok();
    }

    fn saved(data: &Path) -> Value {
        serde_json::from_slice(&std::fs::read(path(data)).unwrap()).unwrap()
    }

    #[test]
    fn a_clash_in_one_profile_leaves_another_profiles_ids_alone() {
        let data = data_dir("clash-profile");
        let (store, _) = Store::load(&data, None, "a", None);
        let a = build(VEHICLES, 10, &[("x", &["x1", "x2"]), ("y", &["y1"])], store);
        a.store.save(&data).unwrap();
        let x1 = key("x", VEHICLES, "x1");

        let (store, _) = Store::load(&data, None, "b", None);
        let b = build(
            VEHICLES,
            a.ids["x1"],
            &[("x", &["x1", "x2"]), ("y", &["y1"])],
            store,
        );
        assert_ne!(b.ids["x1"], a.ids["x1"]);
        b.store.save(&data).unwrap();
        let file = saved(&data);
        assert_eq!(file["rows"][&x1]["id"], a.ids["x1"]);
        assert_eq!(file["profiles"]["b"]["rows"][&x1]["id"], b.ids["x1"]);
        assert!(file["profiles"].get("a").is_none());

        let (store, _) = Store::load(&data, None, "a", None);
        let again_a = build(VEHICLES, 10, &[("x", &["x1", "x2"]), ("y", &["y1"])], store);
        assert_eq!(again_a.ids, a.ids);
        assert_eq!(again_a.store.summary(), None);
        again_a.store.save(&data).unwrap();
        assert_eq!(saved(&data), file);

        let (store, _) = Store::load(&data, None, "b", None);
        let again_b = build(
            VEHICLES,
            a.ids["x1"],
            &[("x", &["x1", "x2"]), ("y", &["y1"])],
            store,
        );
        assert_eq!(again_b.ids, b.ids);
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_file_from_before_per_profile_ids_loads_and_is_written_back_whole() {
        let data = data_dir("old-format");
        let x1 = key("x", VEHICLES, "x1");
        let doc = serde_json::json!({
            "version": 1,
            "rows": { x1.clone(): { "id": 11, "from": "next" } },
            "seeded": ["default"],
            "recovered": true,
            "aliases": { x1.clone(): [7] },
        });
        put(&path(&data), &doc.to_string());
        let (store, warnings) = Store::load(&data, None, "default", None);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(store.get(&x1), Some(11));
        let built = build(VEHICLES, 10, &[("x", &["x1", "x2"])], store);
        assert_eq!(built.ids["x1"], 11);
        built.store.save(&data).unwrap();
        let file = saved(&data);
        assert_eq!(file["version"], VERSION);
        assert_eq!(file["rows"][&x1], doc["rows"][&x1]);
        assert_eq!(file["aliases"], doc["aliases"]);
        assert_eq!(file["seeded"], doc["seeded"]);
        assert!(file.get("profiles").is_none());
        assert_eq!(file["rows"].as_object().unwrap().len(), 2);
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_profiles_rebuilds_keep_its_ids_and_a_move_there_keeps_the_old_id_as_an_alias() {
        let data = data_dir("rebuilds");
        let (store, _) = Store::load(&data, None, "a", None);
        let first = build(VEHICLES, 10, &[("x", &["x1"])], store);
        first.store.save(&data).unwrap();
        let (store, _) = Store::load(&data, None, "b", None);
        let moved = build(VEHICLES, first.ids["x1"], &[("x", &["x1"])], store);
        moved.store.save(&data).unwrap();
        for _ in 0..2 {
            let (store, _) = Store::load(&data, None, "b", None);
            let again = build(VEHICLES, first.ids["x1"], &[("x", &["x1"])], store);
            assert_eq!(again.ids, moved.ids);
            assert_eq!(again.store.summary(), None);
            assert_eq!(
                again.store.id_aliases().tables["vehicles"][&first.ids["x1"]],
                moved.ids["x1"]
            );
            again.store.save(&data).unwrap();
        }
        let (store, _) = Store::load(&data, None, "a", None);
        assert!(store.id_aliases().is_empty());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn aliases_of_addons_outside_the_build_do_not_hide_the_builds_own() {
        let mut store = Store::default();
        let ours = key("x", VEHICLES, "x1");
        let theirs = key("y", VEHICLES, "y1");
        store.insert(ours.clone(), 20);
        store.insert(theirs.clone(), 21);
        store.claimed.extend([ours.clone(), theirs.clone()]);
        store.aliases.entry(ours.clone()).or_default().insert(12);
        store.aliases.entry(theirs.clone()).or_default().insert(12);
        assert!(store.id_aliases().is_empty());
        store.building(["x".to_owned()]);
        store.claimed.insert(ours);
        assert_eq!(store.id_aliases().tables["vehicles"][&12], 20);
    }

    #[test]
    fn an_old_id_another_row_holds_in_the_shared_ids_is_that_rows_alias() {
        let mut store = Store::default();
        let owner = key("x", VEHICLES, "x1");
        let stale = key("x", VEHICLES, "x2");
        store.insert(owner.clone(), 30);
        store.insert(stale.clone(), 31);
        store.claimed.extend([owner.clone(), stale.clone()]);
        store
            .base_ids
            .entry("entrants.vehicles.vehicle".to_owned())
            .or_default()
            .insert(12, owner.clone());
        store.aliases.entry(owner).or_default().insert(12);
        store.aliases.entry(stale).or_default().insert(12);
        assert_eq!(store.id_aliases().tables["vehicles"][&12], 30);
    }

    #[test]
    fn a_kept_row_the_build_did_not_add_gives_no_alias() {
        let mut store = Store::default();
        let added = key("x", VEHICLES, "x1");
        let kept = key("x", VEHICLES, "x2");
        store.insert(added.clone(), 20);
        store.insert(kept.clone(), 21);
        store.claimed.insert(added.clone());
        store.aliases.entry(added).or_default().insert(12);
        store.aliases.entry(kept).or_default().insert(13);
        let aliases = store.id_aliases();
        assert_eq!(aliases.tables["vehicles"].get(&12), Some(&20));
        assert_eq!(aliases.tables["vehicles"].get(&13), None);
    }

    #[test]
    fn an_old_id_a_registry_record_pins_gives_no_alias() {
        let mut store = Store::default();
        let row = key("x", VEHICLES, "x1");
        store.insert(row.clone(), 725);
        store.claimed.insert(row.clone());
        store.aliases.entry(row).or_default().extend([509, 600]);
        store
            .pinned
            .entry("entrants.vehicles.vehicle".to_owned())
            .or_default()
            .insert(509, "y/mercedes".to_owned());
        let aliases = store.id_aliases();
        assert_eq!(aliases.tables["vehicles"].get(&509), None);
        assert_eq!(aliases.tables["vehicles"].get(&600), Some(&725));
    }

    const DRIVERS: (&str, &str) = ("entrants.drivers", "driver");

    fn career_holds_247(data: &Path) {
        put(
            &reports::path(data, reports::IDS),
            r#"{"version":1,"ids":{"driver.identity":{"sw.2022/sw22_vettel":{"id":247,"since":"2026-11-12"}},
            "vehicle.package":{"sw.2022/sw_2022":{"id":247,"since":"2026-11-12"}}},
            "careers":{"dc-1":{"driver.identity":{"sw.2022/sw22_vettel":247}}}}"#,
        );
    }

    #[test]
    fn a_stat_id_an_addon_sets_yields_to_the_registry_record_a_career_holds_it_for() {
        let data = data_dir("explicit-registry");
        career_holds_247(&data);
        let (store, _) = Store::load(&data, None, "p", None);
        let rows: &[&str] = &["aarav_amin@247", "alex_gillon@246"];
        let first = build(DRIVERS, 100, &[("mdm", rows)], store);
        let got = first.ids["aarav_amin"];
        assert_ne!(got, 247, "{:?}", first.lines);
        assert_ne!(got, 246);
        assert_eq!(first.ids["alex_gillon"], 246);
        assert!(
            first.lines.iter().any(|l| l.contains(
                "stat_id 247 is kept for sw.2022/sw22_vettel, a record of the core's registry; stat_id"
            )),
            "{:?}",
            first.lines
        );
        assert_eq!(first.store.get(&key("mdm", DRIVERS, "aarav_amin")), Some(got));
        let mut kept = first.store;
        let loaded = Store::load(&data, None, "p", None).0;
        kept.pinned = loaded.pinned;
        kept.career_held = loaded.career_held;
        let again = build(DRIVERS, 100, &[("mdm", rows)], kept);
        assert_eq!(again.ids["aarav_amin"], got);
        assert_eq!(again.ids["alex_gillon"], 246);
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_stat_id_an_addon_sets_stays_on_its_own_record_and_where_saves_hold_no_ids() {
        let data = data_dir("explicit-own");
        career_holds_247(&data);
        let (store, _) = Store::load(&data, None, "p", None);
        let own = build(DRIVERS, 100, &[("sw.2022", &["other@247"])], store);
        assert_eq!(own.ids["other"], 247);
        let (store, _) = Store::load(&data, None, "p", None);
        let same_row = build(DRIVERS, 100, &[("grid", &["sw22_vettel@247"])], store);
        assert_eq!(same_row.ids["sw22_vettel"], 247);
        let (store, _) = Store::load(&data, None, "p", None);
        let package = build(PACKAGES, 100, &[("mdm", &["pkg@247"])], store);
        assert_eq!(package.ids["pkg"], 247);
        let free = build(DRIVERS, 100, &[("mdm", &["aarav_amin@247"])], Store::default());
        assert_eq!(free.ids["aarav_amin"], 247);
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn ids_the_core_registry_holds_are_passed_over() {
        let data = data_dir("registry");
        let slot = 11;
        put(
            &reports::path(&data, reports::IDS),
            &format!(
                r#"{{"version":1,"ids":{{"vehicle":{{"mod.x/car":{{"id":{slot},"since":"2026-10-29"}},"mod.x/old":12}}}},
                "careers":{{"dc-1":{{"vehicle":{{"mod.y/car":13}}}}}}}}"#
            ),
        );
        let (store, _) = Store::load(&data, None, "p", None);
        let first = build(VEHICLES, 10, &[("a", &["a1"])], store);
        let want = 14;
        assert_eq!(first.ids["a1"], want);
        let mut kept = Store::default();
        kept.put(&key("a", VEHICLES, "a1"), 13, IdFrom::Next);
        let loaded = Store::load(&data, None, "p", None).0;
        kept.pinned = loaded.pinned;
        kept.career_held = loaded.career_held;
        let moved = build(VEHICLES, 10, &[("a", &["a1"])], kept);
        assert_eq!(moved.ids["a1"], want);
        assert!(
            moved.lines.iter().any(|l| l.ends_with(&format!(
                "stat_id {want} (the source's is 10; 13, kept for it, is kept for another row or held by a record of the core's registry)"
            ))),
            "{:?}",
            moved.lines
        );
        assert_eq!(moved.store.get(&key("a", VEHICLES, "a1")), Some(want));
        assert_eq!(moved.store.from[&key("a", VEHICLES, "a1")], IdFrom::Moved);
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_full_table_takes_an_id_ids_json_holds_for_an_addon_the_build_leaves_out_even_one_a_career_listed() {
        let data = data_dir("registry-full");
        let records: Vec<String> = (1..=254).map(|id| format!(r#""gone.x/pkg_{id}":{id}"#)).collect();
        put(
            &reports::path(&data, reports::IDS),
            &format!(
                r#"{{"version":1,"ids":{{"vehicle.package":{{{}}}}},
                "careers":{{"dc-1":{{"vehicle.package":{{"gone.x/pkg_1":1}}}}}}}}"#,
                records.join(",")
            ),
        );
        let (store, _) = Store::load(&data, None, "p", None);
        let first = build(PACKAGES, 0, &[("a", &["a1", "a2"])], store);
        assert_eq!((first.ids["a1"], first.ids["a2"]), (1, 2));
        assert_eq!(first.store.from[&key("a", PACKAGES, "a1")], IdFrom::Reused);
        assert!(
            first.lines.iter().any(|l| l.contains("stat_id 1 (the source's is 0; no other id up to 254 is free; reused from gone.x/pkg_1, which this build leaves out)")),
            "{:?}",
            first.lines
        );
        first.store.save(&data).unwrap();
        let (store, _) = Store::load(&data, None, "p", None);
        let again = build(PACKAGES, 0, &[("a", &["a1", "a2"])], store);
        assert_eq!(again.ids, first.ids);
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn careers_hold_ids_of_teams_vehicles_and_drivers_but_not_vehicle_packages() {
        let doc = serde_json::json!({
            "careers": {"c": {
                "vehicle": {"x/v": 7},
                "team.identity": {"x/t": 8},
                "driver.identity": {"x/d": {"id": 9}},
                "vehicle.package": {"x/p": 10}
            }}
        });
        let held = career_ids(&doc);
        assert_eq!(held["entrants.vehicles.vehicle"], HashSet::from([7]));
        assert_eq!(held["entrants.teams.team"], HashSet::from([8]));
        assert_eq!(held["entrants.drivers.driver"], HashSet::from([9]));
        assert!(!held.contains_key("entrants.vehicles.vehicle_packages.vehicle_package"));
        let pinned = registry_ids(&doc);
        assert_eq!(pinned["entrants.vehicles.vehicle"][&7], "x/v");
        assert!(!pinned.contains_key("entrants.vehicles.vehicle_packages.vehicle_package"));
    }

    #[test]
    fn package_ids_a_career_held_return_when_an_addon_provides_fewer_records() {
        let data = data_dir("career-packages-shrink");
        let held: Vec<String> = (1..=89).map(|id| format!(r#""cs/pkg_{id}":{id}"#)).collect();
        put(
            &reports::path(&data, reports::IDS),
            &format!(
                r#"{{"version":1,"ids":{{}},
                "careers":{{"f1_career_progressive.driver_career:new1":{{"vehicle.package":{{{}}}}}}}}}"#,
                held.join(",")
            ),
        );
        let (store, _) = Store::load(&data, None, "p", None);
        assert!(store.career_held.is_empty());
        let labels: Vec<String> = (1..=10).map(|i| format!("p{i}")).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let built = build(PACKAGES, 0, &[("cs", &refs)], store);
        assert_eq!(
            built.ids.values().copied().collect::<BTreeSet<_>>(),
            (1..=10).collect()
        );
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_full_table_leaves_the_ids_ids_json_holds_for_addons_the_build_has() {
        let data = data_dir("registry-full-building");
        let records: Vec<String> = (1..=254).map(|id| format!(r#""a/pkg_{id}":{id}"#)).collect();
        put(
            &reports::path(&data, reports::IDS),
            &format!(
                r#"{{"version":1,"ids":{{"vehicle.package":{{{}}}}}}}"#,
                records.join(",")
            ),
        );
        let (store, _) = Store::load(&data, None, "p", None);
        let e = try_build(PACKAGES, 0, &[("a", &["a1"])], store).err().unwrap();
        assert!(e.to_string().contains("has no free stat_id up to 254"), "{e}");
        std::fs::remove_dir_all(&data).ok();
    }

    fn pinned_vehicles(upto: i64) -> String {
        let records: Vec<String> = (1..=upto).map(|id| format!(r#""a/car_{id}":{id}"#)).collect();
        format!(r#"{{"version":1,"ids":{{"vehicle":{{{}}}}}}}"#, records.join(","))
    }

    #[test]
    fn vehicle_and_team_rows_never_take_an_id_past_16382() {
        let data = data_dir("vehicle-ceiling");
        put(&reports::path(&data, reports::IDS), &pinned_vehicles(16381));
        let (store, _) = Store::load(&data, None, "p", None);
        let built = build(VEHICLES, 0, &[("b", &["v1"])], store);
        assert_eq!(built.ids["v1"], 16382);
        assert!(built.left_out.is_empty());
        let seasons: Vec<String> = (0..20).map(|i| format!("s{i}")).collect();
        let refs: Vec<&str> = seasons.iter().map(String::as_str).collect();
        for table in [VEHICLES, TEAMS] {
            let built = build(table, 16379, &[("b", &refs)], Store::default());
            assert!(
                built.ids.values().all(|id| (16380..=16382).contains(id)),
                "{:?}",
                built.ids
            );
            assert_eq!(built.ids.len(), 3);
            assert_eq!(built.left_out["b"], 17);
        }
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_vehicle_row_that_finds_no_id_is_left_out_without_failing_the_build() {
        let data = data_dir("vehicle-left-out");
        put(&reports::path(&data, reports::IDS), &pinned_vehicles(16382));
        let (store, _) = Store::load(&data, None, "p", None);
        let built = build(VEHICLES, 0, &[("a", &["v1", "v2"]), ("b", &["w1"])], store);
        assert!(built.ids.is_empty());
        assert_eq!(built.left_out["a"], 2);
        assert_eq!(built.left_out["b"], 1);
        assert!(
            !built
                .store
                .rows
                .keys()
                .any(|k| k.contains("entrants.vehicles.vehicle/"))
        );
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn two_addons_never_share_a_vehicle_id() {
        let labels: Vec<String> = (0..10).map(|i| format!("c{i}")).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let mut store = Store::default();
        let mut all = BTreeSet::new();
        for addon in ["s1972", "s1986", "s2000"] {
            let built = build(VEHICLES, 509, &[(addon, &refs)], store);
            for id in built.ids.values() {
                assert!(all.insert(*id), "{id} given twice");
            }
            store = built.store;
        }
        assert_eq!(all.len(), 30);
    }

    #[test]
    fn driver_ids_above_255_skip_254_255_and_low_byte_0xff() {
        const DRIVERS: (&str, &str) = ("entrants.drivers", "driver");
        let labels: Vec<String> = (0..120).map(|i| format!("d{i}")).collect();
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let built = build(DRIVERS, 254, &[("a", &refs)], Store::default());
        let ids: BTreeSet<i64> = built.ids.values().copied().collect();
        assert_eq!(ids.len(), 120);
        assert!(ids.iter().any(|id| *id > 255));
        assert!(
            ids.iter()
                .all(|id| (208..=16382).contains(id) && *id != 254 && id & 0xff != 0xff)
        );
        let again = build(DRIVERS, 254, &[("a", &refs)], built.store);
        assert_eq!(again.ids, built.ids);
    }

    #[test]
    fn a_full_table_reuses_an_id_kept_for_an_addon_the_build_leaves_out_and_logs_it() {
        let first = build(
            PACKAGES,
            250,
            &[("a", &["a1", "a2", "a3", "a4"])],
            Store::default(),
        );
        let mut ids: Vec<i64> = first.ids.values().copied().collect();
        ids.sort();
        assert_eq!(ids, [251, 252, 253, 254]);
        let e = try_build(
            PACKAGES,
            250,
            &[("a", &["a1", "a2", "a3", "a4"]), ("b", &["b1"])],
            Store::default(),
        )
        .err()
        .unwrap();
        assert!(e.to_string().contains("has no free stat_id up to 254"), "{e}");
        let holder = |id: i64| first.ids.iter().find(|(_, v)| **v == id).unwrap().0.clone();
        let b = build(PACKAGES, 250, &[("b", &["b1", "b2"])], first.store);
        assert_eq!((b.ids["b1"], b.ids["b2"]), (251, 252));
        let line = format!(
            "b: add-row entrants.vehicles.vehicle_packages.vehicle_package_1 (from entrants.vehicles.vehicle_packages.vehicle_package_0): stat_id 251 (the source's is 250; no other id up to 254 is free; reused from {}, which this build leaves out)",
            key("a", PACKAGES, &holder(251))
        );
        assert!(b.lines.contains(&line), "{:?}", b.lines);
        assert_eq!(b.store.get(&key("a", PACKAGES, &holder(251))), None);
        assert_eq!(b.store.get(&key("a", PACKAGES, &holder(253))), Some(253));
        assert_eq!(b.store.from[&key("b", PACKAGES, "b1")], IdFrom::Reused);
        assert_eq!(
            b.store.summary().as_deref(),
            Some("database: row-ids.json keeps 4 added-row id(s) (2 new, 2 reused)")
        );
    }

    fn leaves_out_rows(
        labels: &[&str],
        career: Option<i64>,
    ) -> crate::Result<(BTreeMap<String, i64>, Store)> {
        let mut store = Store::default();
        for (i, id) in (251..=254).enumerate() {
            store.put(&key("b", PACKAGES, &format!("old{i}")), id, IdFrom::Seeded);
        }
        if let Some(id) = career {
            store
                .career_held
                .entry("entrants.vehicles.vehicle_packages.vehicle_package".to_owned())
                .or_default()
                .insert(id);
        }
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in);
        let (table, row) = PACKAGES;
        p.add_fields(
            "a",
            BTreeMap::from([(
                format!("{table}.{row}"),
                vec!["_metadata_".to_owned(), "stat_id".to_owned()],
            )]),
        );
        for label in labels {
            let json = format!(
                r#"{{"op":"db.add-row","table":"{table}","from":"{table}.{row}_0","set":{{"_metadata_":"{label}"}}}}"#
            );
            p.push(Op {
                op: serde_json::from_str(&json).unwrap(),
                origin: Origin {
                    addon: "a".to_owned(),
                    item: None,
                },
            });
        }
        p.push(Op {
            op: serde_json::from_str(&format!(
                r#"{{"op":"db.set","select":"{table}.{row}_0","field":"_metadata_","value":"base"}}"#
            ))
            .unwrap(),
            origin: Origin {
                addon: "b".to_owned(),
                item: None,
            },
        });
        p.keep_row_ids(store);
        let mut gvdb = db(PACKAGES, 250);
        let stock_keys = gvdb.records.iter().map(|r| r.key).collect();
        p.apply_db_report(&mut gvdb, &stock_keys, &HashSet::new(), &mut DbReport::default())?;
        let n = gvdb.instance_count(table, row).unwrap();
        let ids = (1..n)
            .map(|i| {
                let label = gvdb.get_text(&format!("{table}.{row}_{i}._metadata_")).unwrap();
                let id = gvdb.get_text(&format!("{table}.{row}_{i}.stat_id")).unwrap();
                (label, id.parse().unwrap())
            })
            .collect();
        Ok((ids, p.take_row_ids().unwrap()))
    }

    #[test]
    fn a_full_table_takes_ids_kept_for_rows_an_addon_in_the_build_no_longer_adds() {
        let (ids, store) = leaves_out_rows(&["a1", "a2", "a3", "a4"], None).unwrap();
        assert_eq!(
            ids.values().copied().collect::<BTreeSet<_>>(),
            BTreeSet::from([251, 252, 253, 254])
        );
        assert_eq!(store.reused, 4);
        assert_eq!(store.get(&key("b", PACKAGES, "old0")), None);
    }

    #[test]
    fn a_full_table_never_takes_an_id_a_career_uses_from_an_addon_in_the_build() {
        let (ids, _) = leaves_out_rows(&["a1", "a2", "a3"], Some(254)).unwrap();
        assert!(ids.values().all(|id| *id != 254), "{ids:?}");
        let e = leaves_out_rows(&["a1", "a2", "a3", "a4"], Some(254))
            .err()
            .unwrap();
        assert!(e.to_string().contains("has no free stat_id up to 254"), "{e}");
    }

    #[test]
    fn rows_the_store_has_no_id_for_take_the_profiles_last_build_ids() {
        let data = data_dir("seed");
        let overlays = data.join("cache/overlay/p");
        let (table, row) = VEHICLES;
        let mut last = db(VEHICLES, 10);
        last.set(&format!("{table}._{row}_instancecount_"), "3", None)
            .unwrap();
        for (i, (label, id)) in [("a1", "40"), ("b1", "41")].iter().enumerate() {
            last.set(&format!("{table}.{row}_{}._metadata_", i + 1), *label, Some(4))
                .unwrap();
            last.set(&format!("{table}.{row}_{}.stat_id", i + 1), *id, Some(0))
                .unwrap();
        }
        let rel = "game_package/db.bin";
        put(&overlays.join("old").join(rel), "");
        std::fs::write(overlays.join("old").join(rel), last.to_bytes()).unwrap();
        assert_eq!(
            last_build(&overlays, "new", rel),
            Some(LastBuild {
                db: Some(overlays.join("old").join(rel)),
                edits: None
            })
        );
        assert_eq!(last_build(&overlays, "old", rel), None);
        let (store, _) = Store::load(&data, None, "p", last_build(&overlays, "new", rel));
        let first = build(
            VEHICLES,
            10,
            &[("b", &["b1"]), ("a", &["a1"]), ("c", &["c1"])],
            store,
        );
        assert_eq!(
            first.ids,
            BTreeMap::from([("a1".into(), 40), ("b1".into(), 41), ("c1".into(), 42)])
        );
        assert!(
            first
                .lines
                .iter()
                .any(|l| l.ends_with("stat_id 41 (the source's is 10; as the last build gave it)")),
            "{:?}",
            first.lines
        );
        assert_eq!(first.store.from[&key("a", VEHICLES, "a1")], IdFrom::Seeded);
        first.store.save(&data).unwrap();
        let seed = |profile: &str| Store::load(&data, None, profile, last_build(&overlays, "new", rel)).0;
        assert_eq!(seed("p").seed.last, None);
        assert!(seed("q").seed.last.is_some());
        let kept = kept_rows(&data, "a").unwrap();
        assert_eq!(
            kept,
            [Kept {
                table: "entrants.vehicles.vehicle".into(),
                label: "a1".into(),
                id: 40,
                from: IdFrom::Seeded
            }]
        );
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn two_rows_of_one_addon_with_one_label_keep_their_own_ids() {
        let first = build(VEHICLES, 10, &[("a", &["x", "x"])], Store::default());
        let x = first.store.get(&key("a", VEHICLES, "x")).unwrap();
        let x2 = first
            .store
            .get(&format!("{}#2", key("a", VEHICLES, "x")))
            .unwrap();
        assert_ne!(x, x2);
        let again = build(VEHICLES, 10, &[("b", &["b1"]), ("a", &["x", "x"])], first.store);
        assert_eq!(again.store.get(&key("a", VEHICLES, "x")), Some(x));
        assert_eq!(
            again.store.get(&format!("{}#2", key("a", VEHICLES, "x"))),
            Some(x2)
        );
    }

    const MODES: (&str, &str) = ("game_setup.game_setup_modes", "game_setup_mode");
    const ENTRANT_MODES: (&str, &str) = ("entrants.entrant_setup_modes", "entrant_setup_mode");

    // A table of stock rows holding the ids 0 to `top`.
    pub(super) fn table_to((table, row): (&str, &str), top: i64) -> Gvdb {
        let field = stat_id_field(table, row);
        let mut records = vec![Record::new(
            &format!("{table}._{row}_instancecount_"),
            0,
            (top + 1).to_string(),
        )];
        for i in 0..=top {
            records.push(Record::new(
                &format!("{table}.{row}_{i}._metadata_"),
                4,
                format!("stock_{i}"),
            ));
            records.push(Record::new(
                &format!("{table}.{row}_{i}.{field}"),
                0,
                i.to_string(),
            ));
        }
        Gvdb { records }
    }

    #[test]
    fn mode_ids_stay_under_the_mask_and_take_the_next_free_id_in_load_order() {
        let labels: Vec<String> = (0..10).map(|i| format!("mode_{i}")).collect();
        let many: Vec<&str> = labels.iter().map(String::as_str).collect();
        let addons: &[(&str, &[&str])] = &[("bydanix", &["bdx_2012_mode"]), ("many", &many)];
        let built = try_build_on(table_to(MODES, 20), MODES, addons, Store::default()).unwrap();
        assert_eq!(built.ids["bdx_2012_mode"], 21);
        assert_eq!(built.ids["mode_9"], 31);
        assert!(built.ids.values().all(|id| *id <= 31), "{:?}", built.ids);
        assert_eq!(
            built.store.from[&key("bydanix", MODES, "bdx_2012_mode")],
            IdFrom::Next
        );
        let full: &[(&str, &[&str])] = &[
            ("bydanix", &["bdx_2012_mode"]),
            ("many", &many),
            ("more", &["one"]),
        ];
        let e = try_build_on(table_to(MODES, 20), MODES, full, Store::default())
            .err()
            .unwrap();
        assert!(e.to_string().contains("has no free stat_id up to 31"), "{e}");
        let mut kept = Store::default();
        kept.put(&key("bydanix", MODES, "bdx_2012_mode"), 44, IdFrom::Hashed);
        let moved = try_build_on(
            table_to(MODES, 20),
            MODES,
            &[("bydanix", &["bdx_2012_mode"])],
            kept,
        )
        .unwrap();
        assert_eq!(moved.ids["bdx_2012_mode"], 21);
        assert!(
            moved.lines.iter().any(|l| l.ends_with(
                "stat_id 21 (the source's is 0; 44, kept for it, is above 31, the highest the game reads)"
            )),
            "{:?}",
            moved.lines
        );
        let core_first: &[(&str, &[&str])] =
            &[("swofty.core", &["swofty_f1"]), ("bydanix", &["bdx_2012_mode"])];
        let entrants = try_build_on(
            table_to(ENTRANT_MODES, 12),
            ENTRANT_MODES,
            core_first,
            Store::default(),
        )
        .unwrap();
        assert_eq!(
            (entrants.ids["swofty_f1"], entrants.ids["bdx_2012_mode"]),
            (13, 14)
        );
        let seasons = ("game_setup.seasons", "season");
        assert_eq!(stat_id_field(seasons.0, seasons.1), "statId");
        assert_eq!(stat_id_ceiling(seasons.0, seasons.1).map(|c| c.max), Some(31));
    }

    #[test]
    fn a_new_profile_seeds_from_another_profiles_last_build() {
        let data = data_dir("other-profile");
        let overlay = data.join("cache/overlay");
        let mut old = table_to(MODES, 20);
        let (table, row) = MODES;
        old.set(&format!("{table}._{row}_instancecount_"), "22", None)
            .unwrap();
        old.set(&format!("{table}.{row}_21._metadata_"), "bdx_2012_mode", Some(4))
            .unwrap();
        old.set(&format!("{table}.{row}_21.stat_id"), "25", Some(0))
            .unwrap();
        let rel = "game_package/db.bin";
        put(&overlay.join("ig10/older").join(rel), "");
        std::fs::write(overlay.join("ig10/older").join(rel), old.to_bytes()).unwrap();
        std::fs::create_dir_all(overlay.join("fresh")).unwrap();
        let last = last_build(&overlay.join("fresh"), "new", rel);
        assert_eq!(
            last.as_ref().and_then(|l| l.db.clone()),
            Some(overlay.join("ig10/older").join(rel))
        );
        let (store, _) = Store::load(&data, None, "fresh", last);
        let built = try_build_on(
            table_to(MODES, 20),
            MODES,
            &[("bydanix", &["bdx_2012_mode"])],
            store,
        )
        .unwrap();
        assert_eq!(built.ids["bdx_2012_mode"], 25);
        assert!(
            built
                .lines
                .iter()
                .any(|l| l.ends_with("stat_id 25 (the source's is 0; as the last build gave it)")),
            "{:?}",
            built.lines
        );
        assert_eq!(
            built.store.from[&key("bydanix", MODES, "bdx_2012_mode")],
            IdFrom::Seeded
        );
        std::fs::remove_dir_all(&data).ok();
    }

    fn host_ids(rows: &[(&str, &str, i64)]) -> Overrides {
        let mut o = Overrides::default();
        for (addon, label, id) in rows {
            o.rows
                .entry((*addon).to_owned())
                .or_default()
                .insert(format!("{}.{}/{label}", VEHICLES.0, VEHICLES.1), *id);
        }
        o
    }

    fn order(addons: &[&str]) -> Vec<String> {
        addons.iter().map(|a| (*a).to_owned()).collect()
    }

    fn players_store(name: &str) -> (PathBuf, Built) {
        let data = data_dir(name);
        let first = build(
            VEHICLES,
            10,
            &[("a", &["a1", "a2"]), ("b", &["b1"])],
            Store::default(),
        );
        first.store.save(&data).unwrap();
        (data, first)
    }

    #[test]
    fn the_hosts_ids_are_used_for_one_build_and_never_written() {
        let (data, own) = players_store("lend-apply");
        assert_eq!(
            own.ids,
            BTreeMap::from([("a1".into(), 11), ("a2".into(), 12), ("b1".into(), 13)])
        );
        let before = std::fs::read(path(&data)).unwrap();
        let (mut store, _) = Store::load(&data, None, "default", None);
        let host = host_ids(&[("a", "a1", 21), ("a", "a2", 22), ("b", "b1", 23)]);
        assert_eq!(store.lend(&host, &order(&["a", "b"])), Ok(3));
        let lent = build(VEHICLES, 10, &[("a", &["a1", "a2"]), ("b", &["b1"])], store);
        assert_eq!(
            lent.ids,
            BTreeMap::from([("a1".into(), 21), ("a2".into(), 22), ("b1".into(), 23)])
        );
        assert_eq!(lent.store.lent_missed(), None);
        lent.store.save(&data).unwrap();
        assert_eq!(
            std::fs::read(path(&data)).unwrap(),
            before,
            "row-ids.json is as the player's own build left it"
        );
        let (own_again, _) = Store::load(&data, None, "default", None);
        let again = build(VEHICLES, 10, &[("a", &["a1", "a2"]), ("b", &["b1"])], own_again);
        assert_eq!(
            again.ids, own.ids,
            "the next normal build has the player's own ids"
        );
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn two_addons_that_swap_ids_with_the_host_are_not_a_collision() {
        let (data, _) = players_store("lend-swap");
        let (mut store, _) = Store::load(&data, None, "default", None);
        let host = host_ids(&[("a", "a1", 13), ("b", "b1", 11)]);
        assert_eq!(store.lend(&host, &order(&["a", "b"])), Ok(2));
        let lent = build(VEHICLES, 10, &[("a", &["a1", "a2"]), ("b", &["b1"])], store);
        assert_eq!(lent.ids["a1"], 13);
        assert_eq!(lent.ids["b1"], 11);
        assert_eq!(lent.store.lent_missed(), None);
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn an_id_another_row_of_the_build_uses_is_refused_and_one_of_an_addon_left_out_is_not() {
        let (data, _) = players_store("lend-collide");
        let (mut store, _) = Store::load(&data, None, "default", None);
        let host = host_ids(&[("a", "a1", 13)]);
        let err = store.lend(&host, &order(&["a", "b"])).unwrap_err();
        assert!(
            err.contains("stat id 13") && err.contains("a1") && err.contains("b1 of b"),
            "{err}"
        );
        let (mut store, _) = Store::load(&data, None, "default", None);
        assert_eq!(store.lend(&host, &order(&["a"])), Ok(1));
        let lent = build(VEHICLES, 10, &[("a", &["a1", "a2"])], store);
        assert_eq!(lent.ids["a1"], 13);
        assert_eq!(lent.store.lent_missed(), None);
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_host_id_the_game_registry_holds_is_refused() {
        let data = data_dir("lend-pinned");
        let (mut store, _) = Store::load(&data, None, "default", None);
        store.pinned.insert(
            "entrants.vehicles.vehicle".into(),
            HashMap::from([(40, "a/a_record".to_owned())]),
        );
        let err = store
            .lend(&host_ids(&[("a", "a1", 40)]), &order(&["a"]))
            .unwrap_err();
        assert!(err.contains("registry record a/a_record"), "{err}");
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_lent_id_an_outside_addon_pins_is_the_one_the_build_gives() {
        let data = data_dir("lend-pinned-build");
        let (mut store, _) = Store::load(&data, None, "default", None);
        store.pinned.insert(
            "entrants.vehicles.vehicle".into(),
            HashMap::from([(13, "z.indycar/z_record".to_owned())]),
        );
        assert_eq!(store.lend(&host_ids(&[("a", "a1", 13)]), &order(&["a"])), Ok(1));
        let lent = build(VEHICLES, 10, &[("a", &["a1", "a2"])], store);
        assert_eq!(lent.ids["a1"], 13);
        assert_eq!(lent.store.lent_missed(), None);
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_host_id_held_by_a_registry_record_of_an_addon_outside_the_build_is_lent() {
        let data = data_dir("lend-pinned-off");
        let (mut store, _) = Store::load(&data, None, "default", None);
        store.pinned.insert(
            "entrants.vehicles.vehicle".into(),
            HashMap::from([(40, "z.indycar/z_record".to_owned())]),
        );
        assert_eq!(store.lend(&host_ids(&[("a", "a1", 40)]), &order(&["a"])), Ok(1));
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn the_cache_key_names_only_the_overrides_of_addons_in_the_build() {
        let host = host_ids(&[("a", "a1", 21), ("z", "z1", 5)]);
        let with = host.stamp(&order(&["a", "b"])).unwrap();
        assert!(
            with.contains("a/entrants.vehicles.vehicle/a1") && !with.contains("z1"),
            "{with}"
        );
        assert_ne!(
            Some(with),
            host_ids(&[("a", "a1", 22)]).stamp(&order(&["a", "b"]))
        );
        assert_eq!(host.stamp(&order(&["b"])), None);
        assert_eq!(Overrides::default().stamp(&order(&["a"])), None);
    }

    #[test]
    fn a_lent_row_leaves_the_id_aliases_alone() {
        let (data, _) = players_store("lend-aliases");
        let (mut store, _) = Store::load(&data, None, "default", None);
        let key = key("a", VEHICLES, "a1");
        store.claimed.insert(key.clone());
        store.aliases.insert(key.clone(), BTreeSet::from([5]));
        assert!(
            store
                .id_aliases()
                .tables
                .get("vehicles")
                .is_some_and(|t| t.get(&5) == Some(&11))
        );
        store
            .lend(&host_ids(&[("a", "a1", 21)]), &order(&["a", "b"]))
            .unwrap();
        assert!(store.id_aliases().is_empty(), "{:?}", store.id_aliases());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn the_hosts_registry_ids_become_the_cores_room_file_for_the_addons_in_the_build() {
        let lent = Overrides {
            registry: BTreeMap::from([
                (
                    "a".to_owned(),
                    BTreeMap::from([
                        ("content.package:a/season".to_owned(), 116),
                        ("content.package.scenario:a/season".to_owned(), 26),
                    ]),
                ),
                (
                    "gone".to_owned(),
                    BTreeMap::from([("vehicle:gone/car".to_owned(), 9)]),
                ),
            ]),
            ..Default::default()
        };
        assert!(!lent.is_empty());
        assert_eq!(
            lent.room_stat_ids(&order(&["a", "b"])).unwrap(),
            serde_json::json!({
                "version": 1,
                "ids": {
                    "content.package": { "a/season": 116 },
                    "content.package.scenario": { "a/season": 26 }
                }
            })
        );
        assert!(lent.room_stat_ids(&order(&["b"])).is_none());
        let stamp = lent.stamp(&order(&["a"])).unwrap();
        assert!(stamp.contains("116"), "{stamp}");
        assert_ne!(lent.stamp(&order(&["a"])), lent.stamp(&order(&["gone"])));
        let newer = Overrides {
            registry: BTreeMap::from([("a".to_owned(), BTreeMap::new())]),
            ..Default::default()
        };
        assert!(lent.merged(newer).registry["a"].is_empty());
    }

    #[test]
    fn overrides_round_trip_through_their_file() {
        let data = data_dir("lend-file");
        let file = data.join("room.json");
        let host = host_ids(&[("a", "a1", 21)]);
        host.write(&file).unwrap();
        assert_eq!(Overrides::read(&file).unwrap(), host);
        assert_eq!(
            Overrides::read(&data.join("none.json")).unwrap(),
            Overrides::default()
        );
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn save_owners_are_the_addons_with_vehicle_team_engine_or_head_rows() {
        let data = data_dir("owners");
        put(
            &path(&data),
            r#"{"version":2,"rows":{
              "a.engines/entrants.vehicles.vehicle/v1":{"id":1,"from":"next"},
              "b.heads/entrants.animation.driver_heads.driver_head/h":{"id":2,"from":"next"},
              "c.tracks/tracks.Track/t":{"id":3,"from":"next"},
              "d.lists/entrants.entrant_lists.entrant_list/l":{"id":4,"from":"next"}},
              "profiles":{"p":{"rows":{"e.teams/entrants.teams.team/t":{"id":5,"from":"next"}}}}}"#,
        );
        let owners: Vec<String> = save_owners(&data).into_iter().collect();
        assert_eq!(owners, ["a.engines", "b.heads", "e.teams"]);
        assert!(save_owners(&data_dir("none")).is_empty());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn id_owners_name_the_addon_of_a_kept_id_and_of_an_older_one() {
        let data = data_dir("id-owners");
        put(
            &path(&data),
            r#"{"version":2,"rows":{
              "a.season/entrants.teams.team/t":{"id":501,"from":"next"},
              "a.season/tracks.Track/x":{"id":9,"from":"next"}},
              "aliases":{"a.season/entrants.teams.team/t":[480]},
              "profiles":{"p":{"rows":{"b.cars/entrants.vehicles.vehicle/v":{"id":510,"from":"next"}}}}}"#,
        );
        let owners = id_owners(&data);
        let of = |table: &str, id: i64| owners.of("c", table, id).cloned().unwrap_or_default();
        assert_eq!(
            of("entrants.teams.team", 501),
            BTreeSet::from(["a.season".to_owned()])
        );
        assert_eq!(
            of("entrants.teams.team", 480),
            BTreeSet::from(["a.season".to_owned()])
        );
        assert_eq!(
            of("entrants.vehicles.vehicle", 510),
            BTreeSet::from(["b.cars".to_owned()])
        );
        assert!(of("tracks.Track", 9).is_empty());
        assert_eq!(
            label_owners(&data, "entrants.vehicles.vehicle").get("v"),
            Some(&BTreeSet::from(["b.cars".to_owned()]))
        );
        assert!(!label_owners(&data, "entrants.vehicles.vehicle").contains_key("t"));
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn id_owners_name_the_addon_of_a_registry_id_and_of_a_careers_pin() {
        let data = data_dir("id-owners-registry");
        put(
            &path(&data),
            r#"{"version":2,"rows":{"b.cars/entrants.vehicles.vehicle/v":{"id":545,"from":"next"}}}"#,
        );
        put(
            &reports::path(&data, reports::IDS),
            r#"{"version":1,"ids":{
              "team.identity":{"x.season/t":{"id":536,"since":"2026-11-13"}},
              "vehicle":{"x.season/v":{"id":545},"z.season/w":528,"old.season/o":{"id":825,"pin":529}},
              "driver.identity":{"x.season/d":{"id":255}},
              "driver.head":{"x.season/h":{"id":300}},
              "vehicle.package":{"x.season/p":{"id":600}},
              "helmet":{"x.season/k":{"id":537}}},
              "careers":{"c":{"vehicle":{"y.season/w":528},"team.identity":{"y.season/u":700}}}}"#,
        );
        let owners = id_owners(&data);
        let of = |career: &str, table: &str, id: i64| -> Vec<String> {
            owners
                .of(career, table, id)
                .into_iter()
                .flatten()
                .cloned()
                .collect()
        };
        assert_eq!(of("c", "entrants.teams.team", 536), ["x.season"]);
        assert_eq!(of("c", "entrants.teams.team", 700), ["y.season"]);
        assert_eq!(of("other", "entrants.teams.team", 700), ["y.season"]);
        assert_eq!(of("c", "entrants.vehicles.vehicle", 545), ["b.cars", "x.season"]);
        assert_eq!(of("c", "entrants.vehicles.vehicle", 528), ["y.season"]);
        assert_eq!(of("other", "entrants.vehicles.vehicle", 528), ["z.season"]);
        assert_eq!(of("c", "entrants.vehicles.vehicle", 825), ["old.season"]);
        assert!(of("c", "entrants.vehicles.vehicle", 529).is_empty());
        assert_eq!(of("c", "entrants.drivers.driver", 255), ["x.season"]);
        assert!(of("c", "entrants.drivers.driver", 300).is_empty());
        assert!(of("c", "entrants.vehicles.vehicle", 600).is_empty());
        assert!(of("c", "entrants.teams.team", 537).is_empty());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn stock_max_ids_are_the_highest_stat_id_of_each_stock_table() {
        use swofty_formats::gvdb::Record;
        let mut records = vec![Record::new(
            &instance_count_path("entrants.teams", "team"),
            0,
            "3",
        )];
        for (i, id) in [7, 159, 12].into_iter().enumerate() {
            records.push(Record::new(
                &format!("entrants.teams.team_{i}.stat_id"),
                0,
                id.to_string(),
            ));
        }
        let max = stock_max_ids(&Gvdb { records });
        assert_eq!(max.get("entrants.teams.team"), Some(&159));
        assert_eq!(max.get("entrants.vehicles.vehicle"), None);
    }

    const LIST: &str = "unlockable_content.unlockable_items.unlockable_item_list_2";
    const LIST_1: &str = "unlockable_content.unlockable_items.unlockable_item_list_1";
    const LIVERIES: &str = "unlockable_content.unlockable_items.unlockable_item_list_2.folder_0";
    const DECALS: &str = "unlockable_content.unlockable_items.unlockable_item_list_2.folder_1";
    const LIST_1_DECALS: &str = "unlockable_content.unlockable_items.unlockable_item_list_1.folder_0";
    const SE: &str = "unlockable_se_livery";
    const DECAL: &str = "unlockable_decal_set";

    type UnlockRows = &'static [(&'static str, i64)];

    // A stock database with unlock list 2 (special liveries 5279 and 5280 in folder 0, decal sets 40 and
    // 41 in folder 1) and unlock list 1 (decal set 3552 in folder 0), as Steam build 26566308 has them.
    fn unlock_db() -> Gvdb {
        let mut records = vec![
            Record::new(&format!("{LIST}._folder_instancecount_"), 0, "2"),
            Record::new(&format!("{LIST_1}._folder_instancecount_"), 0, "1"),
        ];
        let tables: [(&str, &str, UnlockRows); 3] = [
            (
                LIVERIES,
                SE,
                &[
                    ("force_india_monaco_2026", 5279),
                    ("toro_rosso_barcelona_26", 5280),
                ],
            ),
            (DECALS, DECAL, &[("avolo_s1", 40), ("avolo_s2", 41)]),
            (LIST_1_DECALS, DECAL, &[("avolo_s1", 3552)]),
        ];
        for (folder, kind, rows) in tables {
            records.push(Record::new(
                &format!("{folder}._{kind}_instancecount_"),
                0,
                rows.len().to_string(),
            ));
            for (i, (label, id)) in rows.iter().enumerate() {
                records.push(Record::new(&format!("{folder}.{kind}_{i}._metadata_"), 4, *label));
                records.push(Record::new(
                    &format!("{folder}.{kind}_{i}.customisation_item"),
                    4,
                    *label,
                ));
                records.push(Record::new(
                    &format!("{folder}.{kind}_{i}.statId"),
                    0,
                    id.to_string(),
                ));
            }
        }
        Gvdb { records }
    }

    struct Unlock<'a> {
        folder: &'a str,
        kind: &'a str,
        label: &'a str,
        id: Option<i64>,
    }

    fn row<'a>(folder: &'a str, kind: &'a str, label: &'a str, id: Option<i64>) -> Unlock<'a> {
        Unlock {
            folder,
            kind,
            label,
            id,
        }
    }

    struct UnlockBuilt {
        // `<folder>.<kind>/<label>` to the added row's statId.
        ids: BTreeMap<String, i64>,
        store: Store,
        lines: Vec<String>,
        warnings: Vec<String>,
    }

    const UNLOCK_FIELDS: [&str; 3] = ["_metadata_", "customisation_item", "statId"];

    fn unlock_build(addons: &[(&str, &[Unlock])], store: Store) -> UnlockBuilt {
        let mut gvdb = unlock_db();
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in);
        for (addon, rows) in addons {
            let mut fields = BTreeMap::new();
            for r in *rows {
                fields.insert(
                    format!("{}.{}", r.folder, r.kind),
                    UNLOCK_FIELDS.map(String::from).to_vec(),
                );
                let id = r.id.map_or(String::new(), |id| format!(r#","statId":"{id}""#));
                let json = format!(
                    r#"{{"op":"db.add-row","table":"{f}","from":"{f}.{k}_0","set":{{"_metadata_":"{l}","customisation_item":"{l}"{id}}}}}"#,
                    f = r.folder,
                    k = r.kind,
                    l = r.label
                );
                p.push(Op {
                    op: serde_json::from_str(&json).unwrap(),
                    origin: Origin {
                        addon: (*addon).to_owned(),
                        item: None,
                    },
                });
            }
            p.add_fields(addon, fields);
        }
        p.keep_row_ids(store);
        let stock_keys: HashSet<u64> = gvdb.records.iter().map(|r| r.key).collect();
        let mut report = DbReport::default();
        p.apply_db_report(&mut gvdb, &stock_keys, &HashSet::new(), &mut report)
            .unwrap();
        let mut ids = BTreeMap::new();
        for folder in [LIVERIES, DECALS, LIST_1_DECALS] {
            for kind in [SE, DECAL] {
                for i in 0..gvdb.instance_count(folder, kind).unwrap_or(0) {
                    let name = format!("{folder}.{kind}_{i}");
                    if stock_keys.contains(&fnv1a64(format!("{name}._metadata_"))) {
                        continue;
                    }
                    let label = gvdb.get_text(&format!("{name}._metadata_")).unwrap();
                    let id = gvdb.get_text(&format!("{name}.statId")).unwrap();
                    ids.insert(format!("{folder}.{kind}/{label}"), id.parse().unwrap());
                }
            }
        }
        UnlockBuilt {
            ids,
            store: p.take_row_ids().unwrap(),
            lines: report.lines,
            // The builtin key catalogue doesn't list unlockable_se_livery's keys: not an id warning.
            warnings: report
                .warnings
                .into_iter()
                .filter(|w| !w.contains("the key catalogue has no entry"))
                .collect(),
        }
    }

    fn at(folder: &str, kind: &str, label: &str) -> String {
        format!("{folder}.{kind}/{label}")
    }

    fn unlock_key(addon: &str, list: &str, kind: &str, label: &str) -> String {
        format!("{addon}/{list}.unlockable_item/{kind}/{label}")
    }

    #[test]
    fn an_unlock_row_without_a_stat_id_gets_its_own_from_10000_across_folders_and_kinds() {
        let rows = [
            row(LIVERIES, SE, "my_livery", None),
            row(DECALS, DECAL, "acme_s1", None),
            row(DECALS, DECAL, "acme_s2", None),
        ];
        let built = unlock_build(&[("a", &rows)], Store::default());
        assert_eq!(built.ids[&at(LIVERIES, SE, "my_livery")], 10000);
        assert_eq!(built.ids[&at(DECALS, DECAL, "acme_s1")], 10001);
        assert_eq!(built.ids[&at(DECALS, DECAL, "acme_s2")], 10002);
        let line = format!(
            "add-row {LIVERIES}.{SE}_2 (from {LIVERIES}.{SE}_0): statId 10000 (the source's is 5279)"
        );
        assert!(
            built.lines.iter().any(|l| l.ends_with(&line)),
            "{:#?}",
            built.lines
        );
        assert!(built.warnings.is_empty(), "{:?}", built.warnings);
        assert_eq!(
            built.store.get(&unlock_key("a", LIST, SE, "my_livery")),
            Some(10000)
        );
    }

    #[test]
    fn an_unlock_rows_kept_stat_id_holds_whatever_else_the_build_adds() {
        let a = [row(LIVERIES, SE, "a_livery", None)];
        let b = [
            row(DECALS, DECAL, "b_decal", None),
            row(LIVERIES, SE, "b_livery", None),
        ];
        let first = unlock_build(&[("a", &a), ("b", &b)], Store::default());
        assert_eq!(first.ids[&at(LIVERIES, SE, "a_livery")], 10000);
        assert_eq!(first.ids[&at(DECALS, DECAL, "b_decal")], 10001);
        assert_eq!(first.ids[&at(LIVERIES, SE, "b_livery")], 10002);
        let alone = unlock_build(&[("b", &b)], first.store);
        assert_eq!(alone.ids[&at(DECALS, DECAL, "b_decal")], 10001);
        assert_eq!(alone.ids[&at(LIVERIES, SE, "b_livery")], 10002);
        assert!(
            alone
                .lines
                .iter()
                .any(|l| l.ends_with("statId 10001 (the source's is 40; kept from row-ids.json)")),
            "{:#?}",
            alone.lines
        );
        let c = [row(LIVERIES, SE, "c_livery", None)];
        let swapped = unlock_build(&[("c", &c), ("b", &b), ("a", &a)], alone.store);
        assert_eq!(swapped.ids[&at(LIVERIES, SE, "a_livery")], 10000);
        assert_eq!(swapped.ids[&at(DECALS, DECAL, "b_decal")], 10001);
        assert_eq!(swapped.ids[&at(LIVERIES, SE, "b_livery")], 10002);
        // c comes first now, but the ids kept for a and b stay theirs.
        assert_eq!(swapped.ids[&at(LIVERIES, SE, "c_livery")], 10003);
    }

    #[test]
    fn each_unlock_list_has_its_own_ids_and_keep_copies_the_source_id() {
        let rows = [
            row(DECALS, DECAL, "acme_s1", None),
            row(LIST_1_DECALS, DECAL, "acme_s1", None),
        ];
        let built = unlock_build(&[("a", &rows)], Store::default());
        assert_eq!(built.ids[&at(DECALS, DECAL, "acme_s1")], 10000);
        assert_eq!(built.ids[&at(LIST_1_DECALS, DECAL, "acme_s1")], 10000);
        let mut gvdb = unlock_db();
        let stock: HashSet<u64> = gvdb.records.iter().map(|r| r.key).collect();
        let keep: DbOps = serde_json::from_str(&format!(
            r#"{{"schema_version":1,"fields":{{"{DECALS}.{DECAL}":["_metadata_","customisation_item","statId"]}},
            "ops":[{{"op":"add-row","table":"{DECALS}","from":"{DECALS}.{DECAL}_0","stat_id":"keep","set":{{"_metadata_":"x"}}}}]}}"#
        ))
        .unwrap();
        crate::ops::apply_db_ops(&mut gvdb, &keep, &stock).unwrap();
        assert_eq!(
            gvdb.get_text(&format!("{DECALS}.{DECAL}_2.statId")).as_deref(),
            Some("40")
        );
    }

    #[test]
    fn an_explicit_unlock_stat_id_the_game_now_holds_moves_and_stays_moved() {
        // An addon that took 5280, the highest stock id + 1, before Steam build 26566308 gave 5280 to
        // toro_rosso_barcelona_26.
        let rows = [row(LIVERIES, SE, "generic_livery", Some(5280))];
        let built = unlock_build(&[("lm", &rows)], Store::default());
        assert_eq!(built.ids[&at(LIVERIES, SE, "generic_livery")], 10000);
        assert_eq!(
            built.warnings,
            [
                "lm: statId 5280 of generic_livery is taken by toro_rosso_barcelona_26 from the game; the build gave it statId 10000. The addon's author should leave statId out of add-row"
            ]
        );
        let line =
            format!("statId 5280 is taken by {LIVERIES}.{SE}_1 (toro_rosso_barcelona_26); statId 10000");
        assert!(
            built.lines.iter().any(|l| l.ends_with(&line)),
            "{:#?}",
            built.lines
        );
        let again = unlock_build(&[("lm", &rows)], built.store);
        assert_eq!(again.ids[&at(LIVERIES, SE, "generic_livery")], 10000);
        assert!(
            again
                .lines
                .iter()
                .any(|l| l.ends_with("statId 10000 (kept from row-ids.json)")),
            "{:#?}",
            again.lines
        );
    }

    #[test]
    fn an_explicit_unlock_stat_id_an_earlier_addon_holds_in_another_folder_moves() {
        let first = [row(DECALS, DECAL, "f1_sticker", Some(5281))];
        let second = [
            row(LIVERIES, SE, "generic_livery", Some(5281)),
            row(LIVERIES, SE, "free", Some(6000)),
        ];
        let built = unlock_build(&[("stickers", &first), ("lm", &second)], Store::default());
        assert_eq!(built.ids[&at(DECALS, DECAL, "f1_sticker")], 5281);
        assert_eq!(built.ids[&at(LIVERIES, SE, "generic_livery")], 10000);
        assert_eq!(built.ids[&at(LIVERIES, SE, "free")], 6000);
        assert_eq!(
            built.warnings,
            [
                "lm: statId 5281 of generic_livery is taken by f1_sticker from stickers; the build gave it statId 10000. The addon's author should leave statId out of add-row"
            ]
        );
    }

    #[test]
    fn a_fresh_unlock_stat_id_skips_ids_later_addons_set_and_the_cores_livery_unlock_ids() {
        let data = data_dir("unlock-pins");
        put(
            &reports::path(&data, reports::IDS),
            r#"{"version":1,"ids":{"livery.unlock":{"x/livery":10001}}}"#,
        );
        let (store, _) = Store::load(&data, None, "p", None);
        let early = [
            row(DECALS, DECAL, "early", None),
            row(DECALS, DECAL, "early_2", None),
        ];
        let late = [row(LIVERIES, SE, "late", Some(10000))];
        let built = unlock_build(&[("a", &early), ("b", &late)], store);
        assert_eq!(built.ids[&at(DECALS, DECAL, "early")], 10002);
        assert_eq!(built.ids[&at(DECALS, DECAL, "early_2")], 10003);
        assert_eq!(built.ids[&at(LIVERIES, SE, "late")], 10000);
        assert!(built.warnings.is_empty(), "{:?}", built.warnings);
        built.store.save(&data).unwrap();
        let kept: Vec<(String, String, i64)> = kept_rows(&data, "a")
            .unwrap()
            .into_iter()
            .map(|r| (r.table, r.label, r.id))
            .collect();
        let table = format!("{LIST}.unlockable_item");
        assert_eq!(
            kept,
            [
                (table.clone(), "unlockable_decal_set/early".to_owned(), 10002),
                (table, "unlockable_decal_set/early_2".to_owned(), 10003),
            ]
        );
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_selector_on_a_moved_unlock_stat_id_finds_the_new_row_in_the_same_file() {
        let mut gvdb = unlock_db();
        let stock: HashSet<u64> = gvdb.records.iter().map(|r| r.key).collect();
        let ops: DbOps = serde_json::from_str(&format!(
            r#"{{"schema_version":1,"fields":{{"{LIVERIES}.{SE}":["_metadata_","customisation_item","statId"]}},"ops":[
            {{"op":"add-row","table":"{LIVERIES}","from":"{LIVERIES}.{SE}_0","set":{{"_metadata_":"mine","statId":"5280"}}}},
            {{"op":"set","select":"{LIVERIES}.{SE}[statId=5280]","field":"customisation_item","value":"moved"}}]}}"#
        ))
        .unwrap();
        let mut report = DbReport::default();
        crate::ops::apply_db_ops_report(&mut gvdb, &ops, &stock, &mut report).unwrap();
        let text = |f: &str| gvdb.get_text(&format!("{LIVERIES}.{f}"));
        assert_eq!(text(&format!("{SE}_2.statId")).as_deref(), Some("10000"));
        assert_eq!(
            text(&format!("{SE}_2.customisation_item")).as_deref(),
            Some("moved")
        );
        assert_eq!(
            text(&format!("{SE}_1.customisation_item")).as_deref(),
            Some("toro_rosso_barcelona_26")
        );
        assert_eq!(report.moved_stat_ids.len(), 1);
        assert_eq!(report.moved_stat_ids[0].field, "statId");
    }
}
