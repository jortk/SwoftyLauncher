//! The ids UDP telemetry reports for rows an addon added, in `swofty/udp_ids.json`.
//!
//! An added team keeps its stock ancestor's stat id. An added driver keeps its stock ancestor's id
//! when its `first_name`, `last_name` and `full_name` equal the ancestor's, and is 255 otherwise;
//! so is any driver id above 254 with no stock ancestor. The 2024 and 2025 formats carry one byte
//! per id, so `teams_byte` and `drivers_byte` hold only the ids whose low byte no stock row and no
//! other mapped id shares; the 2026 format carries 16 bits and reads `teams` and `drivers`.
//!
//! Tracks: the Session packet's trackId is one signed byte, and an added track's statId differs from one install to
//! the next (row_ids.rs gives it per install). Every added track therefore has a Swofty track id, the same on every
//! PC: the number [`LISTED`] gives its `<addon>/<code>`, else `0x8000_0000 | (fnv1a32(key) & 0x7fff_ffff)`.
//! `track_ids` holds it for every track package in the build and `tracks` the statId -> key of the tracks the
//! launcher gave an id; the core adds the track records it gives ids at boot (apply/tracks/boot.cpp) and sends
//! the Swofty Track packet (udp_ids_logic.hpp) after each Session packet of such a track.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use swofty_formats::gvdb::Gvdb;

use crate::dbedit::deps::Copy;
use crate::ops::stat_id_field;

pub const FILE: &str = "udp_ids.json";
pub const NETWORK_DRIVER: i64 = 255;
const VERSION: u32 = 1;
const TEAMS: (&str, &str) = ("entrants.teams", "team");
const DRIVERS: (&str, &str) = ("entrants.drivers", "driver");
const NAME_FIELDS: [&str; 3] = ["first_name", "last_name", "full_name"];
const DEPTH: usize = 64;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UdpIds {
    pub version: u32,
    pub teams: BTreeMap<i64, i64>,
    pub drivers: BTreeMap<i64, i64>,
    pub teams_byte: BTreeMap<i64, i64>,
    pub drivers_byte: BTreeMap<i64, i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,
    /// statId -> `<addon>/<code>` of each added track the launcher gave its statId.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tracks: BTreeMap<i64, String>,
    /// `<addon>/<code>` -> Swofty track id of each track package in the build.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub track_ids: BTreeMap<String, u32>,
    /// `<addon>/<code>` -> the Session packet trackId byte [`LISTED`] gives it (64..=254), for the tracks in the build.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub track_udp: BTreeMap<String, i64>,
}

impl UdpIds {
    pub fn is_empty(&self) -> bool {
        self.teams.is_empty() && self.drivers.is_empty() && self.track_ids.is_empty()
    }

    /// Notes the track package `code` of `addon`, and its statId when the launcher gave it one.
    pub fn add_track(&mut self, addon: &str, code: &str, stat_id: Option<i64>) {
        let key = track_key(addon, code);
        if let Some(id) = stat_id {
            self.tracks.insert(id, key.clone());
        }
        let id = swofty_track_id(&key);
        if let Some(n) = session_track_id(&key) {
            self.track_udp.insert(key.clone(), n);
        }
        self.track_ids.insert(key, id);
    }
}

/// A track given ids by hand, never reused once listed.
pub struct Listed {
    /// `<addon>/<code>`.
    pub key: &'static str,
    /// Its Swofty track id, in 256..0x8000_0000 (0..=255 are left to the game's stock tracks), or 0 to keep the hashed one.
    pub id: u32,
    /// The trackId its Session packets carry, in [`SESSION_TRACK_IDS`], or -1 for none (the packet says -1).
    pub session: i64,
}

/// The Session packet's trackId bytes Swofty gives listed tracks: the stock tracks use 0..=41 in F1 25, 42..=63 stay
/// free for tracks the game may add and 255 is -1, unknown. The field is an `int8`, so 128..=254 reach an overlay as
/// -128..=-2: give out 64..=127 first.
pub const SESSION_TRACK_IDS: std::ops::RangeInclusive<i64> = 64..=254;

/// Tracks with ids given by hand, so every PC and every overlay knows them by the same numbers.
pub const LISTED: &[Listed] = &[];

/// The Session packet trackId [`LISTED`] gives the track keyed `key`.
pub fn session_track_id(key: &str) -> Option<i64> {
    LISTED
        .iter()
        .find(|l| l.key == key && SESSION_TRACK_IDS.contains(&l.session))
        .map(|l| l.session)
}

/// The key a track's Swofty track id is made from.
pub fn track_key(addon: &str, code: &str) -> String {
    format!("{addon}/{code}")
}

/// The Swofty track id of the track keyed `key` (see the module notes): the same on every PC.
pub fn swofty_track_id(key: &str) -> u32 {
    if let Some(l) = LISTED.iter().find(|l| l.key == key && l.id != 0) {
        return l.id;
    }
    let mut h: u32 = 0x811c_9dc5;
    for b in key.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    0x8000_0000 | (h & 0x7fff_ffff)
}

struct Table {
    stock: HashSet<i64>,
    ids: HashMap<String, i64>,
}

fn id_of(db: &Gvdb, table: (&str, &str), path: &str) -> Option<i64> {
    db.get_text(&format!("{path}.{}", stat_id_field(table.0, table.1)))?
        .trim()
        .parse()
        .ok()
}

fn read(db: &Gvdb, table: (&str, &str), copied: &HashMap<&str, &Copy>) -> Table {
    let n = db.instance_count(table.0, table.1).unwrap_or(0);
    let mut out = Table {
        stock: HashSet::new(),
        ids: HashMap::new(),
    };
    for i in 0..n {
        let path = format!("{}.{}_{i}", table.0, table.1);
        let Some(id) = id_of(db, table, &path) else {
            continue;
        };
        if !copied.contains_key(path.as_str()) {
            out.stock.insert(id);
        }
        out.ids.insert(path, id);
    }
    out
}

fn ancestor<'a>(copied: &HashMap<&'a str, &'a Copy>, row: &'a str) -> Option<&'a str> {
    let mut at = row;
    for _ in 0..DEPTH {
        at = copied.get(at)?.from.as_str();
        if !copied.contains_key(at) {
            return Some(at);
        }
    }
    None
}

fn same_person(db: &Gvdb, row: &str, other: &str) -> bool {
    NAME_FIELDS
        .iter()
        .all(|f| db.get_text(&format!("{row}.{f}")) == db.get_text(&format!("{other}.{f}")))
}

fn narrow(full: &BTreeMap<i64, i64>, stock: &HashSet<i64>) -> BTreeMap<i64, i64> {
    let mut by_byte: HashMap<i64, usize> = HashMap::new();
    for id in full.keys() {
        *by_byte.entry(id & 0xff).or_default() += 1;
    }
    full.iter()
        .filter(|(id, _)| **id < 256 || (!stock.contains(&(**id & 0xff)) && by_byte[&(**id & 0xff)] == 1))
        .map(|(id, to)| (id & 0xff, *to))
        .collect()
}

pub fn build(db: &Gvdb, copies: &[Copy]) -> UdpIds {
    let name = |path: &str| {
        db.get_text(&format!("{path}._metadata_"))
            .unwrap_or_else(|| path.to_owned())
    };
    let copied: HashMap<&str, &Copy> = copies.iter().map(|c| (c.row.as_str(), c)).collect();
    let mut out = UdpIds {
        version: VERSION,
        ..UdpIds::default()
    };
    let teams = read(db, TEAMS, &copied);
    let drivers = read(db, DRIVERS, &copied);
    for (path, &id) in &teams.ids {
        if !copied.contains_key(path.as_str()) {
            continue;
        }
        let Some(from) = ancestor(&copied, path) else {
            continue;
        };
        let Some(&stock) = teams.ids.get(from) else {
            continue;
        };
        if stock != id && teams.stock.contains(&stock) {
            out.teams.insert(id, stock);
            out.lines.push(format!("team {id} -> {stock} ({})", name(path)));
        }
    }
    for (path, &id) in &drivers.ids {
        let target = match copied.contains_key(path.as_str()) {
            true => match ancestor(&copied, path).and_then(|from| Some((from, *drivers.ids.get(from)?))) {
                Some((from, stock)) if drivers.stock.contains(&stock) && same_person(db, path, from) => stock,
                _ => NETWORK_DRIVER,
            },
            false => NETWORK_DRIVER,
        };
        let needs = match copied.contains_key(path.as_str()) {
            true => target != id,
            false => id > 254,
        };
        if needs {
            let why = match target {
                NETWORK_DRIVER => "no stock driver".to_owned(),
                _ => name(path),
            };
            out.drivers.insert(id, target);
            out.lines.push(format!("driver {id} -> {target} ({why})"));
        }
    }
    out.teams_byte = narrow(&out.teams, &teams.stock);
    out.drivers_byte = narrow(&out.drivers, &drivers.stock);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_track_keeps_one_swofty_id_and_listed_ones_win() {
        let a = swofty_track_id("mod.sokol/sokol_gp");
        assert_eq!(a, swofty_track_id("mod.sokol/sokol_gp"));
        assert!(a >= 0x8000_0000);
        assert_ne!(a, swofty_track_id("mod.sokol/sokol_short"));
        // FNV-1a 32 of "a" is 0xe40c292c; the core computes the same (udp_ids_logic.hpp).
        assert_eq!(swofty_track_id("a"), 0x8000_0000 | (0xe40c_292c & 0x7fff_ffff));
        let mut keys = HashSet::new();
        let mut ids = HashSet::new();
        let mut sessions = HashSet::new();
        for l in LISTED {
            assert!(keys.insert(l.key), "{} listed twice", l.key);
            assert!(
                l.id == 0 || ((256..0x8000_0000).contains(&l.id) && ids.insert(l.id)),
                "{}",
                l.key
            );
            assert!(
                l.session == -1 || (SESSION_TRACK_IDS.contains(&l.session) && sessions.insert(l.session)),
                "{}",
                l.key
            );
        }
        let mut u = UdpIds::default();
        u.add_track("mod.sokol", "sokol_gp", Some(250));
        u.add_track("mod.rec", "rec", None);
        assert_eq!(u.tracks[&250], "mod.sokol/sokol_gp");
        assert_eq!(u.track_ids.len(), 2);
        assert!(!u.is_empty());
    }
    use swofty_formats::gvdb::Record;

    fn row(records: &mut Vec<Record>, table: (&str, &str), i: usize, id: i64, extra: &[(&str, &str)]) {
        let p = format!("{}.{}_{i}", table.0, table.1);
        records.push(Record::new(
            &format!("{p}._metadata_"),
            4,
            format!("{}_{i}", table.1),
        ));
        records.push(Record::new(&format!("{p}.stat_id"), 0, id.to_string()));
        for (f, v) in extra {
            records.push(Record::new(&format!("{p}.{f}"), 4, *v));
        }
    }

    fn count(records: &mut Vec<Record>, table: (&str, &str), n: usize) {
        records.push(Record::new(
            &format!("{}._{}_instancecount_", table.0, table.1),
            0,
            n.to_string(),
        ));
    }

    fn copy(from: &str, row: &str) -> Copy {
        Copy {
            addon: "cs".into(),
            from: from.into(),
            row: row.into(),
            ..Copy::default()
        }
    }

    const NORRIS: [(&str, &str); 3] = [
        ("first_name", "lng_lando"),
        ("last_name", "lng_norris"),
        ("full_name", "lng_lando_norris"),
    ];
    const KIMI: [(&str, &str); 3] = [
        ("first_name", "lng_kimi"),
        ("last_name", "lng_raikkonen"),
        ("full_name", "lng_kimi_raikkonen"),
    ];

    fn fixture() -> (Gvdb, Vec<Copy>) {
        let mut r = Vec::new();
        count(&mut r, TEAMS, 5);
        row(&mut r, TEAMS, 0, 4, &[]);
        row(&mut r, TEAMS, 1, 1, &[]);
        row(&mut r, TEAMS, 2, 507, &[]);
        row(&mut r, TEAMS, 3, 518, &[]);
        row(&mut r, TEAMS, 4, 509, &[]);
        count(&mut r, DRIVERS, 6);
        row(&mut r, DRIVERS, 0, 8, &NORRIS);
        row(&mut r, DRIVERS, 1, 197, &KIMI);
        row(&mut r, DRIVERS, 2, 198, &NORRIS);
        row(&mut r, DRIVERS, 3, 300, &KIMI);
        row(&mut r, DRIVERS, 4, 301, &KIMI);
        row(&mut r, DRIVERS, 5, 44, &KIMI);
        let copies = vec![
            copy("entrants.teams.team_0", "entrants.teams.team_2"),
            copy("entrants.teams.team_1", "entrants.teams.team_3"),
            copy("entrants.teams.team_2", "entrants.teams.team_4"),
            copy("entrants.drivers.driver_0", "entrants.drivers.driver_1"),
            copy("entrants.drivers.driver_0", "entrants.drivers.driver_2"),
            copy("entrants.drivers.driver_0", "entrants.drivers.driver_3"),
            copy("entrants.drivers.driver_3", "entrants.drivers.driver_4"),
        ];
        (Gvdb { records: r }, copies)
    }

    fn built() -> UdpIds {
        let (db, copies) = fixture();
        build(&db, &copies)
    }

    #[test]
    fn added_teams_take_their_stock_ancestor() {
        let ids = built();
        assert_eq!(ids.teams, BTreeMap::from([(507, 4), (509, 4), (518, 1)]));
    }

    #[test]
    fn a_chain_of_copies_ends_at_the_stock_row() {
        assert_eq!(built().teams.get(&509), Some(&4));
    }

    #[test]
    fn drivers_keep_stock_only_for_the_same_person() {
        let ids = built();
        assert_eq!(ids.drivers.get(&197), Some(&255));
        assert_eq!(ids.drivers.get(&198), Some(&8));
        assert_eq!(ids.drivers.get(&300), Some(&255));
        assert_eq!(ids.drivers.get(&301), Some(&255));
        assert_eq!(ids.drivers.get(&8), None);
        assert_eq!(ids.drivers.get(&44), None);
    }

    #[test]
    fn a_driver_above_254_that_no_copy_made_is_255() {
        let mut r = Vec::new();
        count(&mut r, DRIVERS, 2);
        row(&mut r, DRIVERS, 0, 8, &NORRIS);
        row(&mut r, DRIVERS, 1, 400, &KIMI);
        let ids = build(&Gvdb { records: r }, &[]);
        assert_eq!(ids.drivers, BTreeMap::from([(400, 255)]));
    }

    #[test]
    fn byte_tables_drop_ids_whose_low_byte_is_shared() {
        let ids = built();
        assert_eq!(ids.drivers_byte.get(&197), Some(&255));
        assert_eq!(ids.drivers_byte.get(&198), Some(&8));
        assert_eq!(ids.drivers_byte.get(&(300 & 0xff)), None);
        assert_eq!(ids.drivers_byte.get(&(301 & 0xff)), Some(&255));
        assert_eq!(ids.teams_byte.get(&(507 & 0xff)), Some(&4));
        assert_eq!(ids.teams_byte.get(&(518 & 0xff)), Some(&1));
    }

    #[test]
    fn a_chain_through_rows_of_other_addons_ends_at_the_stock_row() {
        let mut r = Vec::new();
        count(&mut r, TEAMS, 4);
        row(&mut r, TEAMS, 0, 6, &[]);
        row(&mut r, TEAMS, 1, 700, &[]);
        row(&mut r, TEAMS, 2, 701, &[]);
        row(&mut r, TEAMS, 3, 702, &[]);
        let mut a = copy("entrants.teams.team_0", "entrants.teams.team_1");
        a.addon = "bdx-base".into();
        let mut b = copy("entrants.teams.team_1", "entrants.teams.team_2");
        b.addon = "bdx-teams".into();
        let mut c = copy("entrants.teams.team_2", "entrants.teams.team_3");
        c.addon = "season-2012".into();
        let ids = build(&Gvdb { records: r }, &[c, a, b]);
        assert_eq!(ids.teams, BTreeMap::from([(700, 6), (701, 6), (702, 6)]));
    }

    #[test]
    fn a_copy_that_keeps_its_source_stat_id_is_not_rewritten() {
        let mut r = Vec::new();
        count(&mut r, TEAMS, 2);
        row(&mut r, TEAMS, 0, 6, &[]);
        row(&mut r, TEAMS, 1, 6, &[]);
        let ids = build(
            &Gvdb { records: r },
            &[copy("entrants.teams.team_0", "entrants.teams.team_1")],
        );
        assert!(ids.is_empty());
    }

    #[test]
    fn nothing_added_gives_nothing() {
        let mut r = Vec::new();
        count(&mut r, TEAMS, 1);
        row(&mut r, TEAMS, 0, 4, &[]);
        count(&mut r, DRIVERS, 1);
        row(&mut r, DRIVERS, 0, 8, &NORRIS);
        assert!(build(&Gvdb { records: r }, &[]).is_empty());
    }

    #[test]
    fn the_file_round_trips() {
        let ids = built();
        let text = serde_json::to_string(&ids).unwrap();
        assert_eq!(serde_json::from_str::<UdpIds>(&text).unwrap(), ids);
    }
}
