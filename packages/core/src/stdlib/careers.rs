//! The career index (docs/std/dependencies.md "Careers remember what they
//! were made with"). The core gives each career an id and a dependency
//! record, saved in its save entry and mirrored to
//! `$DATA/careers/index.json`, the launcher's data folder, which the launcher
//! names to the core in `<game>/swofty/state.json`, with the launcher profile
//! that started it. Before 1.6.0 the core wrote `<game>/swofty/careers/index.json`;
//! the launcher merges that file into the data folder's by career id and
//! removes it, so the list survives a restore of the game folder, and checks
//! each career's addons against a profile.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::range::Range;
use crate::manifest::AddonManifest;
use crate::row_ids::IdOwners;

pub const GAME_PATH: &str = "swofty/careers/index.json";
pub const DATA_PATH: &str = "careers/index.json";

/// The `why` older launchers gave an addon whose database rows were on in a session that
/// saved the career. It says the addon was on, never that the save holds its rows.
pub const ROWS_WHY: &str = "database rows in the save";

/// The `why` older launchers gave an addon whose track any session loaded before the career
/// was saved. It says the track was driven in some mode, never that the career races it.
pub const TRACK_WHY: &str = "a track its sessions loaded";

/// The `why` of an addon that owns the team, car or driver row the career's save slot names.
pub const OWN_WHY: &str = "the career's team, car or driver";

/// The `why` of an addon that owns the vehicle row of the career's My Team car, which the core found
/// missing when it loaded the save.
pub const CAR_WHY: &str = "the career's My Team car";

/// The save slot's vehicle id of a My Team career is this flag and the id of its custom vehicle.
const CUSTOM_VEHICLE: i64 = 0x4000;

const VEHICLES: &str = "entrants.vehicles.vehicle";

fn is_guess(a: &CareerAddon) -> bool {
    matches!(a.why.as_deref(), Some(ROWS_WHY | TRACK_WHY))
}

fn is_proven(a: &CareerAddon) -> bool {
    matches!(a.why.as_deref(), Some(OWN_WHY | CAR_WHY))
}

fn launcher_wrote(a: &CareerAddon) -> bool {
    is_guess(a) || is_proven(a)
}

fn label_of(c: &Career) -> &str {
    if c.label.is_empty() { &c.career } else { &c.label }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CareerAddon {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub need: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Career {
    pub career: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default)]
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default)]
    pub addons: Vec<CareerAddon>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// The careers a career index holds: `{ "careers": [...] }`, a list, or an object of records by
/// save key.
pub fn parse(text: &str) -> Vec<Career> {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return Vec::new();
    };
    let one = |key: Option<String>, v: Value| -> Option<Career> {
        let mut c: Career = serde_json::from_value(v).ok()?;
        if c.key.is_none() {
            c.key = key;
        }
        (!c.career.is_empty()).then_some(c)
    };
    match v {
        Value::Array(list) => list.into_iter().filter_map(|c| one(None, c)).collect(),
        Value::Object(mut map) => match map.remove("careers") {
            Some(Value::Array(list)) => list.into_iter().filter_map(|c| one(None, c)).collect(),
            Some(Value::Object(by_key)) => by_key.into_iter().filter_map(|(k, c)| one(Some(k), c)).collect(),
            _ => map
                .into_iter()
                .filter(|(_, c)| c.is_object())
                .filter_map(|(k, c)| one(Some(k), c))
                .collect(),
        },
        _ => Vec::new(),
    }
}

pub fn read(path: &Path) -> Vec<Career> {
    std::fs::read_to_string(path)
        .map(|t| parse(&t))
        .unwrap_or_default()
}

pub fn data_path(data: &Path) -> PathBuf {
    data.join(DATA_PATH)
}

fn retarget(v: &mut Value, from: &str, to: &str) -> bool {
    match v {
        Value::Object(map) => {
            let mut changed = false;
            if map.get("career").is_some() && map.get("profile").and_then(Value::as_str) == Some(from) {
                map.insert("profile".into(), Value::String(to.into()));
                changed = true;
            }
            for child in map.values_mut() {
                changed |= retarget(child, from, to);
            }
            changed
        }
        Value::Array(list) => list.iter_mut().fold(false, |c, i| retarget(i, from, to) | c),
        _ => false,
    }
}

/// Point every career of profile `from` in the data folder's index at profile `to`.
pub fn rename_profile(data: &Path, from: &str, to: &str) -> crate::Result<()> {
    let path = data_path(data);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(());
    };
    let Ok(mut index) = serde_json::from_str::<Value>(&text) else {
        return Ok(());
    };
    if retarget(&mut index, from, to) {
        crate::write_json_shared(&path, &index)?;
    }
    Ok(())
}

/// Merge the game folder's career index, written before the core used the data folder, into the
/// launcher's copy, newest record per career id first, and return the merged list. A career the
/// game no longer lists stays in the copy. A career the core wrote without a profile gets `stamp`,
/// the profile of the session that just ran.
pub fn sync(data: &Path, game: Option<&Path>, stamp: Option<&str>) -> crate::Result<Vec<Career>> {
    let mut kept = read(&data_path(data));
    let mut changed = false;
    let fresh = game.map(|g| read(&g.join(GAME_PATH))).unwrap_or_default();
    for mut c in fresh {
        changed = true;
        match kept.iter_mut().find(|k| k.career == c.career) {
            Some(k) => {
                c.profile = c.profile.or_else(|| k.profile.clone());
                for a in k.addons.iter().filter(|a| launcher_wrote(a)) {
                    if !c.addons.iter().any(|o| o.id == a.id) {
                        c.addons.push(a.clone());
                    }
                }
                *k = c;
            }
            None => kept.push(c),
        }
    }
    if let Some(stamp) = stamp {
        for c in kept.iter_mut().filter(|c| c.profile.is_none()) {
            c.profile = Some(stamp.to_owned());
            changed = true;
        }
    }
    if changed {
        crate::write_json_shared(
            &data_path(data),
            &serde_json::json!({ "version": 1, "careers": kept }),
        )?;
    }
    Ok(kept)
}

/// A restore puts a snapshot's careers back as the profile's list: the careers of `profile` (and the
/// ones with no profile yet) in the index are replaced by those of `restored`, with their own details
/// and last-played times, until the core lists the profile's careers again at the next Play. Other
/// profiles' careers stay.
pub fn replace_profile(data: &Path, profile: &str, restored: &[Career]) -> crate::Result<()> {
    let mine = |c: &Career| c.profile.as_deref().is_none_or(|p| p == profile);
    let mut list: Vec<Career> = read(&data_path(data)).into_iter().filter(|c| !mine(c)).collect();
    list.extend(restored.iter().filter(|c| mine(c)).cloned());
    crate::write_json_shared(
        &data_path(data),
        &serde_json::json!({ "version": 1, "careers": list }),
    )
}

/// Where the launcher keeps the core's stat-id store: the one store of the install.
pub fn kept_ids(data: &Path) -> PathBuf {
    super::reports::path(data, super::reports::IDS)
}

/// Move what the core wrote in the game folder before it used the data folder: the career index
/// (merged), the stat-id store (the newer of the two wins) and the reports (when the data folder
/// has none). Returns one line for each thing moved.
pub fn migrate(data: &Path, game: &Path) -> crate::Result<Vec<String>> {
    use super::reports;
    let mut out = Vec::new();
    let old_index = game.join(GAME_PATH);
    if old_index.is_file() {
        sync(data, Some(game), None)?;
        std::fs::remove_file(&old_index).ok();
        out.push("career index".to_owned());
    }
    let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    for name in reports::SENT.iter().copied().chain([reports::IDS]) {
        let old = reports::legacy_path(game, name);
        if !old.is_file() {
            continue;
        }
        let new = reports::path(data, name);
        let take = !new.is_file() || (name == reports::IDS && modified(&old) > modified(&new));
        if take {
            let dir = new.parent().expect("has parent");
            std::fs::create_dir_all(dir).map_err(crate::io_err(dir))?;
            std::fs::copy(&old, &new).map_err(crate::io_err(&new))?;
            out.push(name.to_owned());
        }
        std::fs::remove_file(&old).ok();
    }
    Ok(out)
}

/// Before the game folder's `swofty/` goes (Restore game files): move anything the core left there
/// into the data folder, so nothing is lost. The data folder is not touched by the restore.
pub fn keep(data: &Path, game: &Path) -> crate::Result<()> {
    migrate(data, game).map(|_| ())
}

/// What a career needs that profile `order` doesn't have on: one line per addon, empty when it
/// loads in that profile. An addon recorded only because it was on ([`ROWS_WHY`], [`TRACK_WHY`]) is
/// not a need.
pub fn missing(c: &Career, order: &[String], installed: &BTreeMap<String, AddonManifest>) -> Vec<String> {
    let mut out = Vec::new();
    for a in c.addons.iter().filter(|a| !is_guess(a)) {
        let name = installed
            .get(&a.id)
            .map_or(a.id.clone(), |m| format!("{} ({})", m.name, a.id));
        let range = a
            .need
            .as_deref()
            .and_then(|n| Range::parse(n).ok())
            .unwrap_or_else(Range::any);
        match installed.get(&a.id) {
            None => out.push(format!("needs {name}, which isn't installed")),
            Some(_) if !order.contains(&a.id) => out.push(format!("needs {name}, which is off")),
            Some(m) if !range.matches(&m.version) => {
                out.push(format!("needs {name} {range}, and {} is installed", m.version))
            }
            Some(_) => {}
        }
    }
    out
}

/// The careers of `list` in the save `profile` boots: the ones made under a profile that boots the
/// same save (its own launcher save, or the shared Steam save), and the ones with no profile yet.
pub fn in_save_of<'a>(list: &'a [Career], data: &Path, profile: &str) -> Vec<&'a Career> {
    let mut sources: BTreeMap<&str, Option<String>> = BTreeMap::new();
    let mine = crate::vault::Location::source_of_profile(data, profile);
    list.iter()
        .filter(|c| match c.profile.as_deref() {
            None => true,
            Some(p) => {
                *sources
                    .entry(p)
                    .or_insert_with(|| crate::vault::Location::source_of_profile(data, p))
                    == mine
            }
        })
        .collect()
}

/// The My Team cars the core found without a vehicle row when it loaded the save, from the
/// `[myteam]` lines of its loader.log, as `(custom vehicle id, vehicle label)`.
pub fn lost_cars(loader_log: &str) -> BTreeSet<(i64, String)> {
    let between = |l: &str, from: &str, to: &str| -> Option<String> {
        let rest = l.split_once(from)?.1;
        Some(rest.split_once(to)?.0.to_owned())
    };
    loader_log
        .lines()
        .filter_map(|l| {
            let (id, label) = if l.contains("[myteam] keep: My Team car (custom vehicle ") {
                (between(l, "(custom vehicle ", ")")?, between(l, " on '", "'")?)
            } else if l.contains("[myteam] save: My Team vehicle base ")
                && l.contains("is not in the game data")
            {
                (between(l, "(stat id ", ")")?, between(l, "uses vehicle '", "'")?)
            } else {
                return None;
            };
            Some((id.trim().parse().ok()?, label))
        })
        .collect()
}

/// Records on each My Team career of `in_save` whose car is one of `lost` ([`lost_cars`]) the addons
/// that own the car's vehicle row (`owners`, by label: `row_ids::label_owners`). Returns whether any
/// career changed.
pub fn record_cars(
    list: &mut [Career],
    in_save: &dyn Fn(&Career) -> bool,
    lost: &BTreeSet<(i64, String)>,
    owners: &BTreeMap<String, BTreeSet<String>>,
    installed: &BTreeMap<String, AddonManifest>,
) -> bool {
    let mut changed = false;
    for c in list.iter_mut().filter(|c| !c.is_gone() && c.in_slots()) {
        if !in_save(c) {
            continue;
        }
        let Some(vehicle) = c.extra.get("vehicle_raw").and_then(Value::as_i64) else {
            continue;
        };
        for (id, label) in lost {
            if vehicle != CUSTOM_VEHICLE | *id {
                continue;
            }
            for addon in owners.get(label).into_iter().flatten() {
                changed |= prove(c, addon, CAR_WHY, installed);
            }
        }
    }
    changed
}

fn prove(c: &mut Career, addon: &str, why: &str, installed: &BTreeMap<String, AddonManifest>) -> bool {
    match c.addons.iter_mut().find(|a| a.id == addon) {
        Some(a) if is_guess(a) => {
            a.why = Some(why.to_owned());
            true
        }
        Some(_) => false,
        None => {
            c.addons.push(CareerAddon {
                id: addon.to_owned(),
                version: installed.get(addon).map(|m| m.version.to_string()),
                need: None,
                why: Some(why.to_owned()),
            });
            true
        }
    }
}

/// After a session of `profile`: records in the data folder's career index the addons that own the
/// team, car or driver row of each career ([`settle_rows`], with the stock maximum ids `stock_max`
/// gives) and the addons whose My Team cars the session's loader.log names as lost ([`record_cars`]).
pub fn record_session(
    data: &Path,
    profile: &str,
    loader_log: &str,
    stock_max: impl FnOnce() -> BTreeMap<String, i64>,
) -> crate::Result<()> {
    let mut list = read(&data_path(data));
    if list.is_empty() {
        return Ok(());
    }
    let installed = crate::store::Store::new(data).addons().unwrap_or_default();
    let ids: BTreeSet<String> = in_save_of(&list, data, profile)
        .into_iter()
        .map(|c| c.career.clone())
        .collect();
    let in_save = |c: &Career| ids.contains(&c.career);
    let stock_max = stock_max_for(&list, stock_max);
    let store = crate::store::Store::new(data);
    let on = store
        .profile(profile)
        .ok()
        .and_then(|p| crate::resolver::resolve(&p, &installed).ok())
        .map(|plan| plan.order);
    let mut changed = settle_rows(
        &mut list,
        &crate::row_ids::id_owners(data),
        &stock_max,
        &installed,
        on.as_deref().map(|on| (&ids, on)),
    );
    let lost = lost_cars(loader_log);
    if !lost.is_empty() {
        changed |= record_cars(
            &mut list,
            &in_save,
            &lost,
            &crate::row_ids::label_owners(data, VEHICLES),
            &installed,
        );
    }
    if changed {
        crate::write_json_shared(
            &data_path(data),
            &serde_json::json!({ "version": 1, "careers": list }),
        )?;
    }
    Ok(())
}

const SAVE_IDS: [(&str, &str); 3] = [
    ("team_raw", "entrants.teams.team"),
    ("vehicle_raw", "entrants.vehicles.vehicle"),
    ("driver_raw", "entrants.drivers.driver"),
];

/// The ids from this value up are the game's placeholders (no team, my team), never rows.
const PLACEHOLDER_IDS: i64 = 16383;

/// The ids a career's save entry holds above the stock database's highest (`stock_max`, by table,
/// see `row_ids::stock_max_ids`), as `(table, id)`: rows an addon added. A table with no stock
/// maximum yields none.
fn added_ids(c: &Career, stock_max: &BTreeMap<String, i64>) -> Vec<(&'static str, i64)> {
    SAVE_IDS
        .iter()
        .filter_map(|(field, table)| {
            let id = c.extra.get(*field).and_then(Value::as_i64)?;
            let top = stock_max.get(*table)?;
            (id > *top && id < PLACEHOLDER_IDS).then_some((*table, id))
        })
        .collect()
}

fn records_rows(c: &Career) -> bool {
    c.addons.iter().any(|a| a.why.as_deref() == Some(OWN_WHY))
}

/// The stock maximum ids `read` gives, read only when a career of `list` needs them: one that is not
/// deleted and holds a team, vehicle or driver id that could be an addon's. Empty otherwise.
pub fn stock_max_for(list: &[Career], read: impl FnOnce() -> BTreeMap<String, i64>) -> BTreeMap<String, i64> {
    let could_be_added = |c: &Career| {
        SAVE_IDS
            .iter()
            .filter_map(|(field, _)| c.extra.get(*field).and_then(Value::as_i64))
            .any(|id| id <= PLACEHOLDER_IDS)
    };
    if list.iter().any(|c| !c.is_gone() && could_be_added(c)) {
        read()
    } else {
        BTreeMap::new()
    }
}

/// The addons that own `id` of `table` for career `c` now (`owners`, see `row_ids::id_owners`). An
/// addon that is not installed is no owner when an installed one is, and with the load order `on` an
/// addon that is off is no owner when one that is on is: the id belongs to the rows in the game.
fn owners_now<'a>(
    c: &Career,
    (table, id): (&str, i64),
    owners: &'a IdOwners,
    installed: &BTreeMap<String, AddonManifest>,
    on: Option<&[String]>,
) -> Vec<&'a String> {
    let mut found: Vec<&String> = owners.of(&c.career, table, id).into_iter().flatten().collect();
    if found.iter().any(|a| installed.contains_key(*a)) {
        found.retain(|a| installed.contains_key(*a));
    }
    if let Some(on) = on
        && found.iter().any(|a| on.contains(a))
    {
        found.retain(|a| on.contains(a));
    }
    found
}

/// The careers of the save being played and its load order, for [`settle_rows`].
pub type Played<'a> = (&'a BTreeSet<String>, &'a [String]);

/// Gives each career the addons that own the team, vehicle or driver row its save slot names: an id
/// above the stock rows (`stock_max`, see `row_ids::stock_max_ids`) that the install's ids give an
/// addon ([`owners_now`], with the load order of `played` for its careers). An addon recorded that way
/// before and no longer the owner of one of the career's rows is taken off. With no `stock_max`
/// nothing can be told apart from a stock row, so nothing changes. Returns whether any career changed.
pub fn settle_rows(
    list: &mut [Career],
    owners: &IdOwners,
    stock_max: &BTreeMap<String, i64>,
    installed: &BTreeMap<String, AddonManifest>,
    played: Option<Played>,
) -> bool {
    if stock_max.is_empty() {
        return false;
    }
    let mut changed = false;
    for c in list.iter_mut().filter(|c| !c.is_gone()) {
        let on = played
            .filter(|(careers, _)| careers.contains(&c.career))
            .map(|(_, on)| on);
        let found: BTreeSet<String> = added_ids(c, stock_max)
            .into_iter()
            .flat_map(|at| owners_now(c, at, owners, installed, on))
            .cloned()
            .collect();
        let before = c.addons.len();
        c.addons
            .retain(|a| a.why.as_deref() != Some(OWN_WHY) || found.contains(&a.id));
        changed |= c.addons.len() != before;
        for addon in &found {
            match c.addons.iter_mut().find(|a| &a.id == addon) {
                Some(a) if a.why.as_deref() == Some(OWN_WHY) => {}
                Some(a) if is_guess(a) => {
                    a.why = Some(OWN_WHY.to_owned());
                    changed = true;
                }
                Some(_) => {}
                None => changed |= prove(c, addon, OWN_WHY, installed),
            }
        }
    }
    changed
}

/// Takes off each career of `list` the addons recorded as owners of its team, car or driver
/// ([`OWN_WHY`]) that own none of them now in load order `on` ([`owners_now`]): a record
/// [`settle_rows`] wrote from an id another addon holds since. A career none of whose ids has an
/// owner keeps its records. Returns whether any career changed.
pub fn drop_former_owners(
    list: &mut [Career],
    owners: &IdOwners,
    installed: &BTreeMap<String, AddonManifest>,
    on: &[String],
) -> bool {
    let mut changed = false;
    for c in list.iter_mut().filter(|c| !c.is_gone()) {
        let now: BTreeSet<&String> = SAVE_IDS
            .iter()
            .filter_map(|(field, table)| Some((*table, c.extra.get(*field).and_then(Value::as_i64)?)))
            .flat_map(|at| owners_now(c, at, owners, installed, Some(on)))
            .collect();
        if now.is_empty() {
            continue;
        }
        let before = c.addons.len();
        c.addons
            .retain(|a| a.why.as_deref() != Some(OWN_WHY) || now.contains(&a.id));
        changed |= c.addons.len() != before;
    }
    changed
}

/// The Play stop for careers with ids above the stock rows and no owner [`settle_rows`] could name:
/// no addon the install has ids for provides the row. Names the installed database addons that are
/// off as the ones to try. `None` when every career names its owners.
pub fn unowned_line(
    list: &[Career],
    order: &[String],
    installed: &BTreeMap<String, AddonManifest>,
    owners: &BTreeSet<String>,
    stock_max: &BTreeMap<String, i64>,
) -> Option<String> {
    let careers: Vec<&str> = list
        .iter()
        .filter(|c| !c.is_gone() && !records_rows(c) && !added_ids(c, stock_max).is_empty())
        .map(|c| {
            if c.label.is_empty() {
                c.career.as_str()
            } else {
                c.label.as_str()
            }
        })
        .collect();
    if careers.is_empty() {
        return None;
    }
    let off: Vec<String> = owners
        .iter()
        .filter(|id| !order.contains(id))
        .filter_map(|id| installed.get(id).map(|m| m.name.clone()))
        .collect();
    let shown = careers.iter().take(3).copied().collect::<Vec<_>>().join("; ");
    let more = careers.len().saturating_sub(3);
    let more = if more > 0 {
        format!("; {more} more")
    } else {
        String::new()
    };
    let try_on = if off.is_empty() {
        String::new()
    } else {
        format!(
            " Addons that are off: {}.",
            off.iter().take(6).cloned().collect::<Vec<_>>().join(", ")
        )
    };
    Some(format!(
        "{} in your save {} a team, car or driver that no installed addon provides ({shown}{more}). \
         Turn on the addons those careers were made with and press Play, or restore a save from before \
         in Profiles > Saves.{try_on}",
        match careers.len() {
            1 => "1 career".to_owned(),
            n => format!("{n} careers"),
        },
        if careers.len() == 1 { "uses" } else { "use" },
    ))
}

/// The Play stop for careers whose own rows come from an addon that is off or not installed in
/// profile `order`: the career's team, car or driver ([`OWN_WHY`]) or its My Team car ([`CAR_WHY`]).
/// An addon recorded only because it was on, a career gone from the save and a record no save slot
/// backs raise nothing. `None` when no such addon is missing.
pub fn stop_line(
    list: &[Career],
    order: &[String],
    installed: &BTreeMap<String, AddonManifest>,
) -> Option<String> {
    let mut addons: BTreeMap<&str, String> = BTreeMap::new();
    let mut careers: Vec<&str> = Vec::new();
    for c in list.iter().filter(|c| !c.is_gone() && c.in_slots()) {
        let gone: Vec<&CareerAddon> = c
            .addons
            .iter()
            .filter(|a| is_proven(a))
            .filter(|a| !order.contains(&a.id) || !installed.contains_key(&a.id))
            .collect();
        if gone.is_empty() {
            continue;
        }
        for a in gone {
            let name = installed.get(&a.id).map_or(a.id.clone(), |m| m.name.clone());
            let state = if installed.contains_key(&a.id) {
                "off"
            } else {
                "not installed"
            };
            addons.insert(&a.id, format!("{name} ({state})"));
        }
        careers.push(label_of(c));
    }
    if addons.is_empty() {
        return None;
    }
    let names = addons.values().cloned().collect::<Vec<_>>().join(", ");
    let shown = careers.iter().take(3).copied().collect::<Vec<_>>().join("; ");
    let more = careers.len().saturating_sub(3);
    let more = if more > 0 {
        format!("; {more} more")
    } else {
        String::new()
    };
    let it = if addons.len() == 1 { "it" } else { "them" };
    Some(format!(
        "{} in your save {} a team, car or driver from {names} ({shown}{more}). Without {it}, {} may not \
         load, or may load with a stock car in its place. Turn {it} back on and press Play, or restore a \
         save from before in Profiles > Saves.",
        match careers.len() {
            1 => "1 career".to_owned(),
            n => format!("{n} careers"),
        },
        if careers.len() == 1 { "uses" } else { "use" },
        if careers.len() == 1 {
            "that career"
        } else {
            "those careers"
        },
    ))
}

/// The careers in `list` that need `addon` and are still in the save: the careers turning `addon` off
/// would stop from loading as they were saved. An addon recorded only because it was on is not needed.
pub fn needing<'a>(list: &'a [Career], addon: &str) -> Vec<&'a Career> {
    list.iter()
        .filter(|c| !c.is_gone() && c.in_slots())
        .filter(|c| c.addons.iter().any(|a| a.id == addon && !is_guess(a)))
        .collect()
}

/// The one Play line for the careers that won't load in profile `order`: how many, and the first
/// few with the addon each needs. `None` when every career loads.
pub fn play_line(
    list: &[Career],
    order: &[String],
    installed: &BTreeMap<String, AddonManifest>,
) -> Option<String> {
    let short: Vec<(&Career, Vec<String>)> = list
        .iter()
        .filter(|c| !c.is_gone() && c.in_slots())
        .map(|c| (c, missing(c, order, installed)))
        .filter(|(_, m)| !m.is_empty())
        .collect();
    if short.is_empty() {
        return None;
    }
    let shown = short
        .iter()
        .take(3)
        .map(|(c, m)| {
            let name = if c.label.is_empty() {
                c.career.as_str()
            } else {
                c.label.as_str()
            };
            format!("{name} {}", m[0])
        })
        .collect::<Vec<_>>()
        .join("; ");
    let more = short.len().saturating_sub(3);
    let more = if more > 0 {
        format!("; {more} more")
    } else {
        String::new()
    };
    Some(format!(
        "{} won't load in this profile: {shown}{more}",
        match short.len() {
            1 => "1 career".to_owned(),
            n => format!("{n} careers"),
        }
    ))
}

impl Career {
    /// Whether the career is gone from the save: missing from the last profile load or deleted in game.
    pub fn is_gone(&self) -> bool {
        matches!(self.status(), "missing" | "deleted")
    }

    /// Whether a save slot backs the record: the core found the career in the game's save slots.
    /// A record with no slot is one an older core kept under another id, or a career never saved.
    pub fn in_slots(&self) -> bool {
        self.field("slot").is_some()
    }

    /// Whether the career is in the save and needs an addon: one recorded only because it was on
    /// doesn't count.
    pub fn uses_addons(&self) -> bool {
        !self.is_gone() && self.in_slots() && self.addons.iter().any(|a| !is_guess(a))
    }

    /// `present` unless the core marked the career missing from the last profile load.
    pub fn status(&self) -> &str {
        self.extra
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("present")
    }

    /// The text field the core wrote beside the dependency record, such as `mode` or `driver`.
    pub fn field(&self, name: &str) -> Option<&str> {
        self.extra
            .get(name)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECORD: &str = r#"{
      "career": "6f3e9a",
      "profile": "progressive",
      "label": "Progressive Career: Max Verstappen, 2016, season 1",
      "match": { "driver": 4103, "team": 2, "season_index": 0 },
      "made_with": { "launcher": "1.6.0", "core": "0.3.0", "game_build": 25251167 },
      "addons": [
        { "id": "swofty.progressive", "version": "1.3.0", "need": "^1.3.0", "why": "career content f1_career_progressive" },
        { "id": "mod.sochi", "version": "1.1.0", "need": "^1.1.0", "why": "calendar round 4: track mod.sochi/sochi" }
      ],
      "records": [ { "ref": "track:venue/sochi", "resolved": "mod.sochi/sochi", "stat_id": 18 } ]
    }"#;

    fn manifest(id: &str, version: &str) -> AddonManifest {
        serde_json::from_str(&format!(
            r#"{{ "schema_version": 1, "id": "{id}", "name": "N {id}", "version": "{version}", "author": "a",
                 "compat": {{ "game": "f1_25" }} }}"#
        ))
        .unwrap()
    }

    #[test]
    fn reads_every_shape_of_the_index() {
        let by_key = format!(r#"{{ "f1_career_progressive/pg-1849223-max_verstappen": {RECORD} }}"#);
        let c = parse(&by_key);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].career, "6f3e9a");
        assert_eq!(c[0].profile.as_deref(), Some("progressive"));
        assert_eq!(
            c[0].key.as_deref(),
            Some("f1_career_progressive/pg-1849223-max_verstappen")
        );
        assert_eq!(c[0].addons[1].need.as_deref(), Some("^1.1.0"));
        assert!(c[0].extra.contains_key("made_with"));
        assert_eq!(parse(&format!(r#"{{ "careers": [{RECORD}] }}"#)).len(), 1);
        assert_eq!(parse(&format!("[{RECORD}]")).len(), 1);
        assert!(parse("garbage").is_empty());
        assert!(parse(r#"{ "version": 1 }"#).is_empty());
    }

    #[test]
    fn names_what_a_career_needs_in_a_profile() {
        let c = &parse(&format!("[{RECORD}]"))[0];
        let installed: BTreeMap<String, AddonManifest> = [
            ("swofty.progressive", manifest("swofty.progressive", "1.3.2")),
            ("mod.sochi", manifest("mod.sochi", "1.0.0")),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        let both: Vec<String> = vec!["swofty.progressive".into(), "mod.sochi".into()];
        assert_eq!(
            missing(c, &both, &installed),
            ["needs N mod.sochi (mod.sochi) ^1.1.0, and 1.0.0 is installed"]
        );
        assert_eq!(
            missing(c, &both[..1], &installed),
            ["needs N mod.sochi (mod.sochi), which is off"]
        );
        assert_eq!(
            missing(c, &both, &BTreeMap::new()),
            [
                "needs swofty.progressive, which isn't installed",
                "needs mod.sochi, which isn't installed"
            ]
        );
    }

    #[test]
    fn says_which_careers_an_addon_would_drop_and_one_play_line() {
        let record = |id: &str, status: &str, addon: &str| {
            format!(
                r#"{{"career":"{id}","label":"L {id}","status":"{status}","slot":"{id}","mode":"driver_career","addons":[{{"id":"{addon}"}}]}}"#
            )
        };
        let list = parse(&format!(
            "[{}, {}, {}]",
            record("a", "present", "x.one"),
            record("b", "missing", "x.one"),
            record("c", "present", "x.two")
        ));
        let names = |v: Vec<&Career>| v.iter().map(|c| c.career.clone()).collect::<Vec<_>>();
        assert_eq!(names(needing(&list, "x.one")), ["a"]);
        assert_eq!(names(needing(&list, "x.two")), ["c"]);
        assert!(needing(&list, "x.none").is_empty());
        assert_eq!(list[0].field("mode"), Some("driver_career"));
        assert_eq!(list[0].field("driver"), None);
        assert_eq!(list[1].status(), "missing");
        let installed: BTreeMap<String, AddonManifest> = [
            ("x.one", manifest("x.one", "1.0.0")),
            ("x.two", manifest("x.two", "1.0.0")),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        let both: Vec<String> = vec!["x.one".into(), "x.two".into()];
        assert_eq!(play_line(&list, &both, &installed), None);
        let one: Vec<String> = vec!["x.one".into()];
        assert_eq!(
            play_line(&list, &one, &installed).as_deref(),
            Some("1 career won't load in this profile: L c needs N x.two (x.two), which is off")
        );
        assert_eq!(
            play_line(&list, &[], &installed).as_deref(),
            Some(
                "2 careers won't load in this profile: L a needs N x.one (x.one), which is off; L c needs N x.two (x.two), which is off"
            )
        );
    }

    #[test]
    fn careers_deleted_in_game_are_left_out_of_the_play_line_and_stop_line() {
        let record = |id: &str, status: &str| {
            format!(
                r#"{{"career":"{id}","label":"L {id}","status":"{status}","slot":"{id}","addons":[{{"id":"x.one","why":"{OWN_WHY}"}}]}}"#
            )
        };
        let installed = installed_of(&["x.one"]);
        let list_of = |status: &str| parse(&format!("[{}]", record("a", status)));

        assert!(play_line(&list_of("present"), &[], &installed).is_some());
        assert!(stop_line(&list_of("present"), &[], &installed).is_some());
        assert!(needing(&list_of("present"), "x.one").len() == 1);

        let deleted = list_of("deleted");
        assert_eq!(play_line(&deleted, &[], &installed), None);
        assert_eq!(stop_line(&deleted, &[], &installed), None);
        assert!(needing(&deleted, "x.one").is_empty());

        let missing = list_of("missing");
        assert_eq!(play_line(&missing, &[], &installed), None);
        assert_eq!(stop_line(&missing, &[], &installed), None);
        assert!(needing(&missing, "x.one").is_empty());
    }

    #[test]
    fn a_record_no_save_slot_backs_raises_nothing() {
        let installed = installed_of(&["x.one"]);
        let list = parse(&format!(
            r#"[{{"career":"f1_career_2026","label":"f1_career_2026","status":"present","mode":"driver_career",
                 "addons":[{{"id":"x.one","why":"{OWN_WHY}"}},{{"id":"x.two","why":"career content x"}}]}}]"#
        ));
        assert!(!list[0].in_slots());
        assert!(!list[0].uses_addons());
        assert_eq!(stop_line(&list, &[], &installed), None);
        assert_eq!(play_line(&list, &[], &installed), None);
        assert!(needing(&list, "x.one").is_empty());
        assert!(needing(&list, "x.two").is_empty());
    }

    fn installed_of(ids: &[&str]) -> BTreeMap<String, AddonManifest> {
        ids.iter()
            .map(|id| ((*id).to_owned(), manifest(id, "1.0.0")))
            .collect()
    }

    fn my_team(saved: i64) -> Vec<Career> {
        parse(&format!(
            r#"[{{"career":"mt-1","profile":"p","label":"My Team","mode":"my_team","slot":"1","vehicle_raw":16387,"last_saved":{saved},"addons":[]}},
               {{"career":"mt-2","profile":"other","label":"Elsewhere","mode":"my_team","slot":"2","vehicle_raw":16387,"last_saved":{saved},"addons":[]}},
               {{"career":"mt-3","profile":"p","label":"Old","mode":"my_team","slot":"3","vehicle_raw":16387,"status":"missing","last_saved":{saved},"addons":[]}},
               {{"career":"mt-4","profile":"p","label":"Stock engine","mode":"my_team","slot":"4","vehicle_raw":16388,"last_saved":{saved},"addons":[]}}]"#
        ))
    }

    const LOST: &str = "[myteam] keep: My Team car (custom vehicle 3) on 'myteam_x_porsche' (engine 12, base car 3fff0000) has no vehicle row (its addon is off or gone); it loads as the stock 'myteam_mercedes'\n\
                        [myteam] save: My Team vehicle base 0 (stat id 9) uses vehicle 'myteam_x_audi', which is not in the game data (no entrants.vehicles row)\n\
                        [myteam] save: My Team vehicle base 1 (stat id 4) uses vehicle 'myteam_mercedes', whose row index reads as missing; it is not a known addon's vehicle\n\
                        [myteam] vehicle keep armed\n";

    fn car_owners() -> BTreeMap<String, BTreeSet<String>> {
        [
            (
                "myteam_x_porsche".to_owned(),
                BTreeSet::from(["x.engines".to_owned()]),
            ),
            (
                "myteam_x_audi".to_owned(),
                BTreeSet::from(["x.engines".to_owned()]),
            ),
        ]
        .into()
    }

    #[test]
    fn records_the_addon_of_a_my_team_car_the_core_found_without_its_row() {
        let lost = lost_cars(LOST);
        assert_eq!(
            lost.iter().cloned().collect::<Vec<_>>(),
            [
                (3, "myteam_x_porsche".to_owned()),
                (9, "myteam_x_audi".to_owned())
            ]
        );
        let installed = installed_of(&["x.engines"]);
        let mut list = my_team(2_000);
        let mine = |c: &Career| c.profile.as_deref() == Some("p");
        assert!(record_cars(&mut list, &mine, &lost, &car_owners(), &installed));
        assert_eq!(list[0].addons.len(), 1);
        assert_eq!(list[0].addons[0].id, "x.engines");
        assert_eq!(list[0].addons[0].version.as_deref(), Some("1.0.0"));
        assert_eq!(list[0].addons[0].why.as_deref(), Some(CAR_WHY));
        assert!(list[1].addons.is_empty(), "another profile's career");
        assert!(list[2].addons.is_empty(), "a career gone from the save");
        assert!(list[3].addons.is_empty(), "a My Team career on another car");
        assert!(!record_cars(&mut list, &mine, &lost, &car_owners(), &installed));
        let mut unowned = my_team(2_000);
        assert!(!record_cars(
            &mut unowned,
            &mine,
            &lost,
            &BTreeMap::new(),
            &installed
        ));
    }

    #[test]
    fn a_profile_boots_only_the_careers_of_its_own_save() {
        let root = std::env::temp_dir().join(format!("swofty-careers-scope-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        for (name, launcher_saves) in [("a", true), ("b", true), ("s1", false), ("s2", false)] {
            let dir = root.join("profiles");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(format!("{name}.json")),
                format!(r#"{{"name":"{name}","launcher_saves":{launcher_saves}}}"#),
            )
            .ok();
        }
        let list = parse(
            r#"[{"career":"1","profile":"a"},{"career":"2","profile":"b"},{"career":"3"},
                {"career":"4","profile":"s1"},{"career":"5","profile":"s2"}]"#,
        );
        let ids = |p: &str| -> Vec<String> {
            in_save_of(&list, &root, p)
                .iter()
                .map(|c| c.career.clone())
                .collect()
        };
        assert_eq!(ids("a"), ["1", "3"]);
        assert_eq!(ids("b"), ["2", "3"]);
        std::fs::remove_dir_all(&root).ok();
    }

    fn copy_switch(name: &str, ea: bool) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("swofty-careers-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let game = root.join("game");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(game.join(crate::GAME_EXE), b"exe").unwrap();
        match ea {
            true => {
                std::fs::create_dir_all(game.join("__Installer")).unwrap();
                std::fs::write(game.join("__Installer/installerdata.xml"), b"<game/>").unwrap();
            }
            false => std::fs::write(game.join("EAJavelinInstaller_installscript.vdf"), b"").unwrap(),
        }
        let pfx = root.join("pfx");
        std::fs::create_dir_all(pfx.join("drive_c/users/steamuser/AppData/Local/F1 25/savegame/ea/profile"))
            .unwrap();
        std::fs::write(pfx.join("user.reg"), b"").unwrap();
        let store = crate::store::Store::new(&root);
        let mut config = crate::session::Config::default();
        config.games.active = Some(game.clone());
        config.games.added = vec![game];
        config.launch.ea_prefix = Some(pfx);
        store.save_config(&config).unwrap();
        let dir = root.join("profiles");
        std::fs::create_dir_all(&dir).unwrap();
        for p in ["hi", "default"] {
            std::fs::write(
                dir.join(format!("{p}.json")),
                format!(r#"{{"name":"{p}","launcher_saves":true}}"#),
            )
            .unwrap();
        }
        root
    }

    fn classic_careers() -> Vec<Career> {
        parse(&format!(
            r#"[{{"career":"dc-2019","profile":"default","label":"Driver Career 2019","addons":[
                   {{"id":"swofty.classic-seasons","version":"1.0.0","why":"{OWN_WHY}"}}],"slot":"19"}},
                {{"career":"dc-hi","profile":"hi","label":"Driver Career","addons":[]}}]"#
        ))
    }

    #[test]
    fn an_ea_copy_profile_switch_keeps_each_profiles_own_save() {
        let root = copy_switch("ea-switch", true);
        let list = classic_careers();
        let installed = installed_of(&["swofty.classic-seasons"]);
        let scoped: Vec<Career> = in_save_of(&list, &root, "hi").into_iter().cloned().collect();
        let ids: Vec<&str> = scoped.iter().map(|c| c.career.as_str()).collect();
        assert_eq!(ids, ["dc-hi"]);
        assert_eq!(stop_line(&scoped, &[], &installed), None);
        if !cfg!(windows) {
            for (p, source, layout) in [
                ("hi", "launcher-ea.hi", crate::vault::Layout::LauncherEa),
                ("default", "launcher-ea.default", crate::vault::Layout::LauncherEa),
                (crate::profile::VANILLA, "ea-steamuser", crate::vault::Layout::Ea),
            ] {
                let at = crate::vault::Location::of_profile(&root, p).expect("the EA save");
                assert_eq!((at.source.as_str(), at.layout), (source, layout), "{p}");
            }
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_steam_copy_profile_switch_keeps_each_profiles_own_save() {
        let root = copy_switch("steam-switch", false);
        let list = classic_careers();
        let installed = installed_of(&["swofty.classic-seasons"]);
        let scoped: Vec<Career> = in_save_of(&list, &root, "hi").into_iter().cloned().collect();
        let ids: Vec<&str> = scoped.iter().map(|c| c.career.as_str()).collect();
        assert_eq!(ids, ["dc-hi"]);
        assert_eq!(stop_line(&scoped, &[], &installed), None);
        let at = crate::vault::Location::of_profile(&root, "hi").expect("the launcher save");
        assert_eq!(
            (at.source.as_str(), at.layout),
            ("launcher-hi", crate::vault::Layout::Launcher)
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn stops_play_when_the_addon_of_a_careers_my_team_car_is_off_or_gone() {
        let installed = installed_of(&["x.engines"]);
        let mut list = my_team(2_000);
        let on: Vec<String> = vec!["x.engines".into()];
        record_cars(
            &mut list,
            &|c| c.profile.as_deref() == Some("p"),
            &lost_cars(LOST),
            &car_owners(),
            &installed,
        );
        assert_eq!(stop_line(&list, &on, &installed), None);
        let stop = stop_line(&list, &[], &installed).unwrap();
        assert_eq!(
            stop,
            "1 career in your save uses a team, car or driver from N x.engines (off) (My Team). Without it, \
             that career may not load, or may load with a stock car in its place. Turn it back on and press \
             Play, or restore a save from before in Profiles > Saves."
        );
        assert!(!stop.contains("won't start"), "{stop}");
        assert_eq!(needing(&list, "x.engines").len(), 1);
        let gone = stop_line(&list, &on, &BTreeMap::new()).unwrap();
        assert!(gone.contains("x.engines (not installed)"), "{gone}");
        list[0]
            .extra
            .insert("status".into(), Value::String("missing".into()));
        assert_eq!(stop_line(&list, &[], &installed), None);
    }

    #[test]
    fn an_addon_that_was_only_on_when_a_career_was_saved_raises_nothing() {
        let installed = installed_of(&["x.engines", "track.nyc"]);
        let list = parse(&format!(
            r#"[{{"career":"driver_career:a","label":"Driver Career","slot":"a","status":"present",
                 "driver_raw":3,"team_raw":7,"vehicle_raw":9,"addons":[
                   {{"id":"x.engines","why":"{ROWS_WHY}"}},
                   {{"id":"track.nyc","why":"{TRACK_WHY}"}},
                   {{"id":"track.gone","why":"{TRACK_WHY}"}}]}}]"#
        ));
        assert_eq!(stop_line(&list, &[], &installed), None);
        assert_eq!(play_line(&list, &[], &installed), None);
        assert!(missing(&list[0], &[], &installed).is_empty());
        assert!(!list[0].uses_addons());
        for addon in ["x.engines", "track.nyc", "track.gone"] {
            assert!(needing(&list, addon).is_empty(), "{addon}");
        }
    }

    #[test]
    fn a_track_in_a_careers_calendar_is_needed_and_one_it_never_races_is_not() {
        let installed = installed_of(&["mod.sochi", "track.nyc"]);
        let list = parse(&format!(
            r#"[{{"career":"driver_career:a","label":"Progressive","slot":"a","status":"present","addons":[
                   {{"id":"mod.sochi","why":"calendar round 4: track mod.sochi/sochi"}},
                   {{"id":"track.nyc","why":"{TRACK_WHY}"}}]}}]"#
        ));
        let both: Vec<String> = vec!["mod.sochi".into(), "track.nyc".into()];
        assert_eq!(play_line(&list, &both, &installed), None);
        assert_eq!(play_line(&list, &both[..1], &installed), None);
        assert!(needing(&list, "track.nyc").is_empty());
        assert_eq!(
            play_line(&list, &both[1..], &installed).as_deref(),
            Some(
                "1 career won't load in this profile: Progressive needs N mod.sochi (mod.sochi), which is off"
            )
        );
        assert_eq!(needing(&list, "mod.sochi").len(), 1);
    }

    fn old_careers() -> Vec<Career> {
        parse(
            r#"{"version":1,"careers":[
              {"career":"driver_career:a","label":"","addons":[],"driver_raw":3,"team_raw":480,"vehicle_raw":217,"source":"slots","slot":"s","status":"present"},
              {"career":"driver_career:b","label":"","addons":[],"driver_raw":16388,"team_raw":501,"vehicle_raw":510,"source":"slots","slot":"s","status":"present"},
              {"career":"driver_career:c","label":"","addons":[],"driver_raw":102,"team_raw":155,"vehicle_raw":161,"source":"slots","slot":"s","status":"present"},
              {"career":"my_team:d","label":"","addons":[],"driver_raw":3,"team_raw":16387,"vehicle_raw":16390,"source":"slots","slot":"s","status":"present"},
              {"career":"driver_career:e","label":"","addons":[],"driver_raw":3,"team_raw":501,"vehicle_raw":510,"source":"slots","status":"deleted"}]}"#,
        )
    }

    fn stock_max() -> BTreeMap<String, i64> {
        [
            ("entrants.teams.team".to_owned(), 475),
            ("entrants.vehicles.vehicle".to_owned(), 211),
            ("entrants.drivers.driver".to_owned(), 300),
        ]
        .into()
    }

    #[test]
    fn a_career_on_the_highest_stock_ids_raises_nothing() {
        let mut list = parse(
            r#"[{"career":"driver_career:s","addons":[],"driver_raw":300,"team_raw":475,"vehicle_raw":211,"status":"present"}]"#,
        );
        let owners: BTreeMap<(String, i64), BTreeSet<String>> = [(
            ("entrants.teams.team".to_owned(), 475),
            BTreeSet::from(["x.season".to_owned()]),
        )]
        .into();
        let installed = installed_of(&["x.season"]);
        assert!(!settle_rows(
            &mut list,
            &owners.into(),
            &stock_max(),
            &installed,
            None
        ));
        assert!(list[0].addons.is_empty());
        let off: BTreeSet<String> = ["x.season".to_owned()].into();
        assert_eq!(unowned_line(&list, &[], &installed, &off, &stock_max()), None);
        list[0].extra.insert("team_raw".into(), 476.into());
        assert!(unowned_line(&list, &[], &installed, &off, &stock_max()).is_some());
        assert_eq!(unowned_line(&list, &[], &installed, &off, &BTreeMap::new()), None);
    }

    fn id_owners() -> IdOwners {
        let one =
            |table: &str, id: i64, addon: &str| ((table.to_owned(), id), BTreeSet::from([addon.to_owned()]));
        [
            one("entrants.teams.team", 501, "x.season"),
            one("entrants.vehicles.vehicle", 510, "x.season"),
            one("entrants.vehicles.vehicle", 600, "x.other"),
        ]
        .into_iter()
        .collect::<BTreeMap<_, _>>()
        .into()
    }

    #[test]
    fn records_the_owners_of_the_added_ids_a_career_holds() {
        let installed = installed_of(&["x.season"]);
        let mut list = old_careers();
        assert!(settle_rows(
            &mut list,
            &id_owners(),
            &stock_max(),
            &installed,
            None
        ));
        assert_eq!(list[1].addons.len(), 1);
        assert_eq!(list[1].addons[0].id, "x.season");
        assert_eq!(list[1].addons[0].why.as_deref(), Some(OWN_WHY));
        assert!(list[0].addons.is_empty() && list[2].addons.is_empty() && list[3].addons.is_empty());
        assert!(list[4].addons.is_empty());
        assert!(!settle_rows(
            &mut list,
            &id_owners(),
            &stock_max(),
            &installed,
            None
        ));
        let stop = stop_line(&list, &[], &installed).unwrap();
        assert!(stop.contains("x.season"), "{stop}");
        assert_eq!(stop_line(&list, &["x.season".to_owned()], &installed), None);
    }

    #[test]
    fn the_stock_database_is_read_only_for_careers_that_may_hold_added_ids() {
        let reads = std::cell::Cell::new(0);
        let read = || {
            reads.set(reads.get() + 1);
            stock_max()
        };
        assert!(stock_max_for(&[], read).is_empty());
        let mut gone = old_careers();
        gone.retain(Career::is_gone);
        assert!(stock_max_for(&gone, read).is_empty());
        let mut sentinel = parse(
            r#"[{"career":"my_team:s","addons":[],"driver_raw":16384,"team_raw":16387,"vehicle_raw":16390,"status":"present"}]"#,
        );
        assert!(stock_max_for(&sentinel, read).is_empty());
        assert_eq!(reads.get(), 0);
        sentinel.extend(old_careers());
        assert_eq!(stock_max_for(&sentinel, read), stock_max());
        assert_eq!(reads.get(), 1);
    }

    #[test]
    fn an_addon_that_was_on_is_needed_only_when_it_owns_the_careers_rows() {
        let installed = installed_of(&["x.season", "x.recorded"]);
        let mut list = old_careers();
        for c in list.iter_mut() {
            for id in ["x.season", "x.recorded"] {
                c.addons.push(CareerAddon {
                    id: id.into(),
                    why: Some(ROWS_WHY.into()),
                    ..CareerAddon::default()
                });
            }
        }
        assert_eq!(stop_line(&list, &[], &installed), None);
        assert!(!settle_rows(
            &mut list,
            &id_owners(),
            &BTreeMap::new(),
            &installed,
            None
        ));
        assert!(settle_rows(
            &mut list,
            &id_owners(),
            &stock_max(),
            &installed,
            None
        ));
        let why = |c: &Career, id: &str| c.addons.iter().find(|a| a.id == id).and_then(|a| a.why.clone());
        assert_eq!(why(&list[1], "x.season").as_deref(), Some(OWN_WHY));
        assert_eq!(why(&list[1], "x.recorded").as_deref(), Some(ROWS_WHY));
        assert_eq!(why(&list[0], "x.season").as_deref(), Some(ROWS_WHY));
        let stop = stop_line(&list, &[], &installed).unwrap();
        assert!(
            stop.contains("N x.season (off)") && !stop.contains("x.recorded"),
            "{stop}"
        );
        assert!(stop.contains("(driver_career:b)"), "{stop}");
        assert_eq!(needing(&list, "x.season").len(), 1);
        assert!(needing(&list, "x.recorded").is_empty());
        list[1].extra.insert("team_raw".into(), 7.into());
        list[1].extra.insert("vehicle_raw".into(), 9.into());
        assert!(settle_rows(
            &mut list,
            &id_owners(),
            &stock_max(),
            &installed,
            None
        ));
        assert_eq!(why(&list[1], "x.season"), None);
        assert_eq!(stop_line(&list, &[], &installed), None);
    }

    #[test]
    fn warns_about_added_ids_no_installed_addon_owns() {
        let installed = installed_of(&["x.season"]);
        let owners: BTreeSet<String> = ["x.season".to_owned()].into();
        let mut list = old_careers();
        settle_rows(&mut list, &id_owners(), &stock_max(), &installed, None);
        let line = unowned_line(&list, &[], &installed, &owners, &stock_max()).unwrap();
        assert!(
            line.contains(
                "1 career in your save uses a team, car or driver that no installed addon provides \
                 (driver_career:a). Turn on"
            ),
            "{line}"
        );
        assert!(line.contains("Addons that are off: N x.season"), "{line}");
        assert_eq!(
            unowned_line(&list[1..], &[], &installed, &owners, &stock_max()),
            None
        );
        assert_eq!(unowned_line(&[], &[], &installed, &owners, &stock_max()), None);
    }

    #[test]
    fn a_career_on_a_registry_team_or_car_names_its_season_and_is_never_unowned() {
        let root = std::env::temp_dir().join(format!("swofty-careers-registry-ids-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let ids = crate::stdlib::reports::path(&root, crate::stdlib::reports::IDS);
        std::fs::create_dir_all(ids.parent().unwrap()).unwrap();
        std::fs::write(
            &ids,
            r#"{"version":1,"ids":{
              "team.identity":{"x.season/sw15_redbull":{"id":536,"since":"2026-11-13"}},
              "vehicle":{"x.season/redbull":{"id":545,"since":"2026-11-13"},
                         "y.season/williams":{"id":528,"since":"2026-11-13"}}},"careers":{}}"#,
        )
        .unwrap();
        let owners = crate::row_ids::id_owners(&root);
        std::fs::remove_dir_all(&root).ok();
        let stock_max: BTreeMap<String, i64> = [
            ("entrants.teams.team".to_owned(), 499),
            ("entrants.vehicles.vehicle".to_owned(), 508),
            ("entrants.drivers.driver".to_owned(), 254),
        ]
        .into();
        let installed = installed_of(&["x.season", "y.season"]);
        let none = BTreeSet::new();
        let on = ["x.season".to_owned(), "y.season".to_owned()];
        let mut list = parse(
            r#"[{"career":"driver_career:a","addons":[],"driver_raw":2,"team_raw":536,"vehicle_raw":545,"source":"slots","slot":"s","status":"present"},
                {"career":"driver_career:b","addons":[],"driver_raw":62,"team_raw":479,"vehicle_raw":528,"source":"slots","slot":"s","status":"present"},
                {"career":"driver_career:c","addons":[],"driver_raw":2,"team_raw":760,"vehicle_raw":700,"source":"slots","slot":"s","status":"present"}]"#,
        );
        assert!(settle_rows(&mut list, &owners, &stock_max, &installed, None));
        assert_eq!(unowned_line(&list[..2], &on, &installed, &none, &stock_max), None);
        assert_eq!(stop_line(&list, &on, &installed), None);
        let stop = stop_line(&list, &on[1..], &installed).unwrap();
        assert!(
            stop.contains("N x.season (off)") && stop.contains("(driver_career:a)"),
            "{stop}"
        );
        assert!(!stop.contains("y.season"), "{stop}");
        assert_eq!(
            unowned_line(&list[..2], &on[1..], &installed, &none, &stock_max),
            None
        );
        let gone = stop_line(&list, &on, &installed_of(&["x.season"])).unwrap();
        assert!(
            gone.contains("y.season (not installed)") && gone.contains("(driver_career:b)"),
            "{gone}"
        );
        let line = unowned_line(&list, &on, &installed, &none, &stock_max).unwrap();
        assert!(
            line.starts_with(
                "1 career in your save uses a team, car or driver that no installed addon provides \
                 (driver_career:c). Turn on"
            ),
            "{line}"
        );
        assert!(!line.contains("crash"), "{line}");
    }

    #[test]
    fn an_id_a_removed_addon_held_before_is_not_a_careers_need() {
        let root = std::env::temp_dir().join(format!("swofty-careers-former-ids-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let ids = crate::stdlib::reports::path(&root, crate::stdlib::reports::IDS);
        std::fs::create_dir_all(ids.parent().unwrap()).unwrap();
        std::fs::write(
            crate::row_ids::path(&root),
            r#"{"version":2,"rows":{
              "old.season/entrants.teams.team/cs17_mclaren":{"id":528,"from":"next"},
              "off.season/entrants.teams.team/t":{"id":530,"from":"next"},
              "old.season/entrants.teams.team/alone":{"id":560,"from":"next"}}}"#,
        )
        .unwrap();
        std::fs::write(
            &ids,
            r#"{"version":1,"ids":{
              "team.identity":{"new.season/sw17_sauber":{"id":528,"since":"2026-11-09"},
                               "new.season/sw17_haas":{"id":530,"since":"2026-11-09"}},
              "driver.identity":{"new.season/sw17_wehrlein":{"id":300,"since":"2026-11-09"}},
              "vehicle":{"new.season/sauber":{"id":607,"since":"2026-11-09"},
                         "old.season/mclaren":{"id":829,"since":"2026-11-04","pin":607},
                         "old.season/ferrari":{"id":825,"since":"2026-11-04","pin":522},
                         "old.season/haas":{"id":610,"since":"2026-11-04"}}},
              "careers":{
                "progressive:new1":{"vehicle":{"old.season/ferrari":825,"old.season/mclaren":829}},
                "driver_career:pinned":{"vehicle":{"new.season/sauber":610}}}}"#,
        )
        .unwrap();
        let owners = crate::row_ids::id_owners(&root);
        std::fs::remove_dir_all(&root).ok();
        let stock_max: BTreeMap<String, i64> = [
            ("entrants.teams.team".to_owned(), 499),
            ("entrants.vehicles.vehicle".to_owned(), 508),
            ("entrants.drivers.driver".to_owned(), 254),
        ]
        .into();
        let installed = installed_of(&["new.season", "off.season"]);
        let on = ["new.season".to_owned()];
        let none = BTreeSet::new();
        let former = r#"{"id":"old.season","why":"the career's team, car or driver"}"#;
        let mut list = parse(&format!(
            r#"[{{"career":"driver_career:a","addons":[{former}],"driver_raw":300,"team_raw":528,"vehicle_raw":607,"source":"slots","slot":"a","status":"present"}},
                {{"career":"progressive:new1","addons":[],"driver_raw":300,"team_raw":528,"vehicle_raw":607,"source":"slots","slot":"b","status":"present"}},
                {{"career":"driver_career:c","addons":[],"driver_raw":2,"team_raw":530,"vehicle_raw":9,"source":"slots","slot":"c","status":"present"}},
                {{"career":"driver_career:pinned","addons":[],"driver_raw":2,"team_raw":7,"vehicle_raw":610,"source":"slots","slot":"d","status":"present"}},
                {{"career":"driver_career:e","addons":[],"driver_raw":2,"team_raw":560,"vehicle_raw":9,"source":"slots","slot":"e","status":"present"}}]"#,
        ));
        let stale = stop_line(&list, &on, &installed).unwrap();
        assert!(stale.contains("old.season (not installed)"), "{stale}");
        let mut checked = list.clone();
        assert!(drop_former_owners(&mut checked, &owners, &installed, &on));
        assert_eq!(stop_line(&checked[..4], &on, &installed), None);
        assert!(checked[0].addons.is_empty());

        let played: BTreeSet<String> = list.iter().map(|c| c.career.clone()).collect();
        assert!(settle_rows(
            &mut list,
            &owners,
            &stock_max,
            &installed,
            Some((&played, &on))
        ));
        let ids = |c: &Career| c.addons.iter().map(|a| a.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&list[0]), ["new.season"]);
        assert_eq!(ids(&list[1]), ["new.season"]);
        assert_eq!(ids(&list[2]), ["new.season"]);
        assert_eq!(ids(&list[3]), ["new.season"]);
        assert_eq!(ids(&list[4]), ["old.season"]);
        assert_eq!(stop_line(&list[..4], &on, &installed), None);
        assert_eq!(unowned_line(&list, &on, &installed, &none, &stock_max), None);
        assert!(
            needing(&list, "old.season")
                .iter()
                .all(|c| c.career == "driver_career:e")
        );
        assert!(needing(&list, "off.season").is_empty());
        assert!(list[..4].iter().all(Career::uses_addons));
        let alone = stop_line(&list, &on, &installed).unwrap();
        assert!(
            alone.contains("old.season (not installed)") && alone.contains("(driver_career:e)"),
            "{alone}"
        );
        assert!(!drop_former_owners(&mut list, &owners, &installed, &on));

        let mut elsewhere = parse(
            r#"[{"career":"driver_career:c","addons":[],"driver_raw":2,"team_raw":530,"vehicle_raw":9,"source":"slots","slot":"c","status":"present"}]"#,
        );
        settle_rows(&mut elsewhere, &owners, &stock_max, &installed, None);
        assert_eq!(ids(&elsewhere[0]), ["new.season", "off.season"]);
        let off = stop_line(&elsewhere, &on, &installed).unwrap();
        assert!(off.contains("off.season (off)"), "{off}");
        assert!(drop_former_owners(&mut elsewhere, &owners, &installed, &on));
        assert_eq!(stop_line(&elsewhere, &on, &installed), None);
        let other = ["off.season".to_owned()];
        settle_rows(
            &mut elsewhere,
            &owners,
            &stock_max,
            &installed,
            Some((&played, &other)),
        );
        assert_eq!(ids(&elsewhere[0]), ["off.season"]);
    }

    #[test]
    fn a_content_addon_is_not_a_play_stop_and_sync_keeps_the_recorded_rows() {
        let list = parse(&format!("[{RECORD}]"));
        assert_eq!(stop_line(&list, &[], &BTreeMap::new()), None);
        let root = std::env::temp_dir().join(format!("swofty-careers-rows-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let (data, game) = (root.join("data"), root.join("game"));
        let mut mine = my_team(2_000);
        mine[0].addons.push(CareerAddon {
            id: "x.engines".into(),
            why: Some(ROWS_WHY.into()),
            ..CareerAddon::default()
        });
        std::fs::create_dir_all(data.join("careers")).unwrap();
        std::fs::write(
            data_path(&data),
            serde_json::json!({ "version": 1, "careers": mine }).to_string(),
        )
        .unwrap();
        std::fs::create_dir_all(game.join("swofty/careers")).unwrap();
        std::fs::write(
            game.join(GAME_PATH),
            r#"[{"career":"mt-1","profile":"p","label":"My Team","addons":[]}]"#,
        )
        .unwrap();
        let merged = sync(&data, Some(&game), None).unwrap();
        let kept = merged.iter().find(|c| c.career == "mt-1").unwrap();
        assert_eq!(kept.addons.len(), 1);
        assert_eq!(kept.addons[0].id, "x.engines");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_launchers_copy_keeps_careers_and_their_profile() {
        let root = std::env::temp_dir().join(format!("swofty-careers-{}", std::process::id()));
        let (data, game) = (root.join("data"), root.join("game"));
        std::fs::create_dir_all(game.join("swofty/careers")).unwrap();
        std::fs::write(game.join(GAME_PATH), format!("[{RECORD}]")).unwrap();
        let first = sync(&data, Some(&game), Some("progressive")).unwrap();
        assert_eq!(first.len(), 1);
        let later = RECORD
            .replace("\"profile\": \"progressive\",", "")
            .replace("season 1", "season 2");
        let other = RECORD
            .replace("6f3e9a", "77aa01")
            .replace("progressive\",", "vanilla-ish\",");
        std::fs::write(game.join(GAME_PATH), format!("[{later}, {other}]")).unwrap();
        let merged = sync(&data, Some(&game), Some("other")).unwrap();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].profile.as_deref(), Some("progressive"));
        assert!(merged[0].label.ends_with("season 2"));
        std::fs::remove_file(game.join(GAME_PATH)).unwrap();
        assert_eq!(sync(&data, Some(&game), None).unwrap().len(), 2);
        assert_eq!(sync(&data, None, None).unwrap().len(), 2);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_career_without_a_profile_gets_the_sessions_and_the_ids_survive_a_restore() {
        let root = std::env::temp_dir().join(format!("swofty-careers-keep-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let (data, game) = (root.join("data"), root.join("game"));
        let unstamped = RECORD.replace("\"profile\": \"progressive\",", "");
        std::fs::create_dir_all(game.join("swofty/careers")).unwrap();
        std::fs::write(game.join(GAME_PATH), format!("[{unstamped}]")).unwrap();
        let synced = sync(&data, Some(&game), Some("tails")).unwrap();
        assert_eq!(synced[0].profile.as_deref(), Some("tails"));
        let ids = crate::stdlib::reports::legacy_path(&game, crate::stdlib::reports::IDS);
        std::fs::create_dir_all(ids.parent().unwrap()).unwrap();
        std::fs::write(&ids, "{\"track\":{\"mod.sochi/sochi\":18}}").unwrap();
        keep(&data, &game).unwrap();
        std::fs::remove_dir_all(game.join("swofty")).unwrap();
        assert_eq!(read(&data_path(&data)).len(), 1);
        assert_eq!(
            std::fs::read_to_string(kept_ids(&data)).unwrap(),
            "{\"track\":{\"mod.sochi/sochi\":18}}"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn migrating_moves_the_old_files_once_and_the_newer_ids_win() {
        let root = std::env::temp_dir().join(format!("swofty-careers-migrate-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let (data, game) = (root.join("d\u{e4}ta \u{30b9}"), root.join("game"));
        let put = |p: &Path, t: &str| {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, t).unwrap();
        };
        use crate::stdlib::reports::{IDS, ROOM, legacy_path, path};
        put(&legacy_path(&game, IDS), "{\"old\":1}");
        put(&legacy_path(&game, ROOM), "{\"presize\":\"applied\"}");
        put(&game.join(GAME_PATH), &format!("[{RECORD}]"));
        put(&path(&data, ROOM), "{\"presize\":\"kept\"}");
        let moved = migrate(&data, &game).unwrap();
        assert_eq!(moved, ["career index", "ids.json"]);
        assert_eq!(std::fs::read_to_string(path(&data, IDS)).unwrap(), "{\"old\":1}");
        assert_eq!(
            std::fs::read_to_string(path(&data, ROOM)).unwrap(),
            "{\"presize\":\"kept\"}"
        );
        assert_eq!(read(&data_path(&data)).len(), 1);
        for gone in [
            legacy_path(&game, IDS),
            legacy_path(&game, ROOM),
            game.join(GAME_PATH),
        ] {
            assert!(!gone.exists(), "{}", gone.display());
        }
        assert!(migrate(&data, &game).unwrap().is_empty());
        put(&legacy_path(&game, IDS), "{\"stale\":1}");
        std::fs::File::options()
            .write(true)
            .open(legacy_path(&game, IDS))
            .unwrap()
            .set_modified(std::time::SystemTime::UNIX_EPOCH)
            .unwrap();
        assert!(migrate(&data, &game).unwrap().is_empty());
        assert_eq!(std::fs::read_to_string(path(&data, IDS)).unwrap(), "{\"old\":1}");
        assert!(!legacy_path(&game, IDS).exists());
        std::fs::remove_dir_all(&root).ok();
    }
}
