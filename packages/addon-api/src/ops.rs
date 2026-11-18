use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use swofty_formats::gvdb::{Gvdb, Record, instance_count_path};
use swofty_formats::hash::{fnv1a64, fnv1a64_from};

use crate::asset::Source;
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DbOps {
    pub schema_version: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub strict: bool,
    #[serde(default, skip_serializing_if = "SideData::is_empty")]
    pub side_data: SideData,
    pub ops: Vec<DbOp>,
}

/// The file's settings for the build's side-data check of its `add-row` copies.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SideData {
    /// Tables and fields whose side-data findings were reviewed: dotted paths
    /// with or without row numbers, `*` matching any run of characters.
    #[serde(default, deserialize_with = "patterns", skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<String>,
}

impl SideData {
    pub fn is_empty(&self) -> bool {
        self.ignore.is_empty()
    }
}

fn patterns<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Vec<String>, D::Error> {
    let v = Vec::<String>::deserialize(d)?;
    for p in &v {
        let ok = !p.is_empty()
            && p.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.*{}[]=-".contains(&b));
        if !ok {
            return Err(serde::de::Error::custom(format!(
                "side_data.ignore: {p:?} is not a table or field path"
            )));
        }
    }
    Ok(v)
}

/// A file named by `operations.hud`: `hud.element` records keyed by record id, in the shape of a
/// registry data file. Other registries in the same file are left to the registry loader.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HudFile {
    #[serde(default, rename = "hud.element")]
    pub elements: BTreeMap<String, serde_json::Value>,
}

impl HudFile {
    pub fn parse(text: &str) -> Result<Self> {
        serde_json::from_str(text).map_err(|e| Error::Other(format!("hud file: {e}")))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CopyMode {
    All,
    Listed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum StatIdMode {
    Fresh,
    Keep,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DbReport {
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
    /// `remove-record` ops left out because their key is a stock record.
    pub stock_removals: usize,
    /// `remove-record` ops left out because their key is a record a track added.
    pub track_removals: usize,
    /// Each `add-row` applied, in order.
    pub added_rows: Vec<AddedRow>,
    /// Each `add-row` whose `stat_id` a row added earlier held, or the game can't read, in order.
    pub moved_stat_ids: Vec<MovedStatId>,
    /// Each record a `set`, `put-record` or `remove-record` changed, in order.
    pub rewrites: Vec<Rewrite>,
}

/// A record an op changed: its value before and after, as text; `None` when it was missing or was removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewrite {
    pub key: u64,
    pub was: Option<String>,
    pub now: Option<String>,
}

fn rewrote(cx: &mut Cx, key: u64, was: Option<&[u8]>, now: Option<&[u8]>) {
    if was != now {
        let text = |v: &[u8]| String::from_utf8_lossy(v).into_owned();
        cx.report.rewrites.push(Rewrite {
            key,
            was: was.map(text),
            now: now.map(text),
        });
    }
}

/// An `add-row` that set a `stat_id` a row added earlier holds, or one above the table's
/// [`stat_id_ceiling`], and the id it got instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedStatId {
    pub row: String,
    /// The row's id key: `stat_id`, or `statId` in `game_setup.scenarios` and unlock lists.
    pub field: &'static str,
    pub label: Option<String>,
    pub old: i64,
    pub new: i64,
    /// The row added earlier that holds `old` (in an unlock list, also a stock row); `None` when
    /// `old` is above the ceiling or kept for `record`.
    pub holder: Option<String>,
    pub holder_label: Option<String>,
    /// The record of the core's registry `old` is kept for in `ids.json`.
    pub record: Option<String>,
}

/// The highest `stat_id` the game reads for a row of a table it keeps the id of in a byte, and
/// the lowest id a fresh row past it may take (`None`: above every stock row's id).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatIdCeiling {
    pub max: i64,
    pub from: Option<i64>,
}

/// The record that lists the virtual entrant lists (comma-separated labels) for the build's settle step.
pub const VIRTUAL_LISTS: &str = "swofty.virtual_entrant_lists";
const VIRTUAL_LISTS_TAG: u8 = 4;

fn virtual_lists(db: &Db) -> Vec<String> {
    db.text(fnv1a64(VIRTUAL_LISTS))
        .map(|t| {
            t.split(',')
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

const STAT_ID_CEILINGS: &[(&str, &str, StatIdCeiling)] = &[
    // Vehicle and team ids are 14 bits in the game's packed keys (0x3fff = none), like driver ids; the core's
    // registry pools end at the same 16382 (VEHICLE_STAT_MAX / TEAM_STAT_MAX in apply/content/lower.hpp).
    (
        "entrants.vehicles",
        "vehicle",
        StatIdCeiling {
            max: 16382,
            from: None,
        },
    ),
    (
        "entrants.teams",
        "team",
        StatIdCeiling {
            max: 16382,
            from: None,
        },
    ),
    // VehicleIdentity +0xcd8 and VehiclePackageRow +0x0 are u8, 0xff = none.
    (
        "entrants.vehicles.vehicle_packages",
        "vehicle_package",
        StatIdCeiling { max: 254, from: None },
    ),
    // Driver ids are u16 in memory and 14 bits in saves and net (0x3fff = none); 254 is default_driver, and UDP 2024/2025 read a low byte of 255 as a network player.
    (
        "entrants.drivers",
        "driver",
        StatIdCeiling {
            max: 16382,
            from: Some(208),
        },
    ),
    // The core's entrant list (content.package) id pool ends at 255.
    (
        "entrants.entrant_lists",
        "entrant_list",
        StatIdCeiling { max: 255, from: None },
    ),
    // VehicleIdentity +0xcd9 is u8; the R&D tree's row flag 0xff means a team row.
    (
        "entrants.engines",
        "Engine",
        StatIdCeiling { max: 254, from: None },
    ),
    // TeamRecord +0x581 team_boss is u8, 0xff = none.
    (
        "entrants.characters",
        "character",
        StatIdCeiling { max: 254, from: None },
    ),
    // TeamRecord +0x560 animation_set is u8, 0xff = none.
    (
        "entrants.animation.driver_animation_sets",
        "driver_animation_set",
        StatIdCeiling { max: 254, from: None },
    ),
    // TeamRecord +0x580 nationality is u8.
    (
        "entrants.nationality",
        "nationality",
        StatIdCeiling { max: 255, from: None },
    ),
    // The core's scenario id pool ends at 255.
    (
        "game_setup.scenarios",
        "scenario",
        StatIdCeiling { max: 254, from: None },
    ),
    // The mask parser 0x142a0ddb0 sets bit `stat_id` of a u32 for each mode a row lists and throws
    // 'invalid bitset position' at boot above 31.
    (
        "game_setup.game_setup_modes",
        "game_setup_mode",
        StatIdCeiling { max: 31, from: None },
    ),
    // The same parser's season mask, by `statId`.
    (
        "game_setup.seasons",
        "season",
        StatIdCeiling { max: 31, from: None },
    ),
    // The Time Trial leaderboard descriptor id is read as a byte (list +0xe0) and goes into the board name LBV<id><track><D|W>.
    (
        "leaderboards",
        "leaderboard_descriptor",
        StatIdCeiling { max: 255, from: None },
    ),
    // Each game setup mode names its entrant setup mode; kept to the mode mask's range with it.
    (
        "entrants.entrant_setup_modes",
        "entrant_setup_mode",
        StatIdCeiling { max: 31, from: None },
    ),
    // The F1 Life scene holds two car customisation slots (+0xd00, +0xe20) and writes car group `stat_id`
    // into slot `stat_id` with no bounds check (0x14252ccd0 on build 26566308).
    (
        "customisation.customisation_cars",
        "customisation_car_group",
        StatIdCeiling { max: 1, from: None },
    ),
];

/// The key that holds the id of `table`'s rows named `row`: `statId` for scenarios and seasons, else `stat_id`.
pub fn stat_id_field(table: &str, row: &str) -> &'static str {
    match (table, row) {
        ("game_setup.scenarios", "scenario") | ("game_setup.seasons", "season") => "statId",
        _ if unlock_list(table).is_some() => "statId",
        _ => "stat_id",
    }
}

const UNLOCK_LISTS: &str = "unlockable_content.unlockable_items.unlockable_item_list_";

/// The name the row ids of an unlock list go under: one id space for every folder and kind of the
/// list, kept as `<list>.unlockable_item`, as in
/// `unlockable_content.unlockable_items.unlockable_item_list_2.unlockable_item`.
pub const UNLOCK_ROW: &str = "unlockable_item";

/// The highest `statId` the launcher gives an unlock item, the top of the core's livery unlock pool.
pub const UNLOCK_STAT_MAX: i64 = 65535;

/// Where a fresh unlock `statId` starts when the list's stock rows stay below it. Game updates add their
/// unlock rows right above the highest stock id (`toro_rosso_barcelona_26` took 5280 in Steam build
/// 26566308), so an id just above it is the next update's.
pub const UNLOCK_FRESH_FROM: i64 = 10000;

/// The kinds of rows an unlock list's folders hold, each a table `<folder>.<kind>` with a `statId`.
pub const UNLOCK_KINDS: &[&str] = &[
    "consumable_xp_booster",
    "currency_reward",
    "unlockable_badge_bg",
    "unlockable_badge_emblem",
    "unlockable_casual_footwear",
    "unlockable_casual_headwear",
    "unlockable_decal_group",
    "unlockable_decal_set",
    "unlockable_driver",
    "unlockable_driver_number",
    "unlockable_eyewear",
    "unlockable_gloves",
    "unlockable_head",
    "unlockable_helmet",
    "unlockable_item_pack",
    "unlockable_livery",
    "unlockable_personal_sticker",
    "unlockable_podium_emote",
    "unlockable_pose",
    "unlockable_se_livery",
    "unlockable_suit",
    "unlockable_team",
    "unlockable_tops",
    "unlockable_trousers",
    "unlockable_vehicle",
    "unlockable_victory_vo",
];

/// The unlock list `unlockable_content.unlockable_items.unlockable_item_list_<n>` whose folder
/// `table` (`….unlockable_item_list_<n>.folder_<m>`) is.
pub fn unlock_list(table: &str) -> Option<&str> {
    let rest = table.strip_prefix(UNLOCK_LISTS)?;
    let (n, folder) = rest.split_once('.')?;
    let m = folder.strip_prefix("folder_")?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    (digits(n) && digits(m)).then(|| &table[..UNLOCK_LISTS.len() + n.len()])
}

/// Whether `table` (`<table>.<row>`, as [`RowIds`] keys name it) is an unlock list's id space,
/// `<list>.`[`UNLOCK_ROW`].
pub fn is_unlock_ids(table: &str) -> bool {
    table
        .strip_prefix(UNLOCK_LISTS)
        .and_then(|r| r.strip_suffix(UNLOCK_ROW))
        .and_then(|n| n.strip_suffix('.'))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The [`StatIdCeiling`] of `table`'s rows named `row`, for a table the game keeps the id of in a byte.
pub fn stat_id_ceiling(table: &str, row: &str) -> Option<StatIdCeiling> {
    STAT_ID_CEILINGS
        .iter()
        .find(|(t, r, _)| *t == table && *r == row)
        .map(|(_, _, c)| *c)
}

/// Why `table`'s ceiling is where it is, for tables whose limit an addon author can't work around by
/// picking another id: appended to the "no free stat_id" error so the message says what to change.
pub fn stat_id_ceiling_reason(table: &str, row: &str) -> Option<&'static str> {
    match (table, row) {
        ("customisation.customisation_cars", "customisation_car_group") => Some(concat!(
            "the F1 Life scene behind the main menu has two car slots and indexes them by the car ",
            "group's stat_id, so a third car group crashes the game at the main menu; leave the car ",
            "groups alone or edit one of the two"
        )),
        ("game_setup.game_setup_modes", "game_setup_mode")
        | ("entrants.entrant_setup_modes", "entrant_setup_mode") => Some(concat!(
            "the game keeps setup modes in a 32-bit mask (bit = stat_id) and stops at boot with ",
            "'invalid bitset position' above 31, so no install holds more than 32; share a mode ",
            "instead of adding one per box or season (list-append the entrant list to swofty_f1 ",
            "or to one mode the addons share)"
        )),
        ("game_setup.seasons", "season") => Some(concat!(
            "the game keeps seasons in a 32-bit mask (bit = statId) and stops at boot with ",
            "'invalid bitset position' above 31; reuse a stock season row instead of adding one"
        )),
        _ => None,
    }
}

/// Whether a fresh row of `table`'s rows named `row` may not take `id`: driver ids 254 and 255, and any with a low byte of 0xff.
pub fn stat_id_unusable(table: &str, row: &str, id: i64) -> bool {
    table == "entrants.drivers" && row == "driver" && (id == 254 || id & 0xff == 0xff)
}

/// An `add-row` applied: the source row's path and the new row's path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedRow {
    pub from: String,
    pub row: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DbOp {
    Set {
        select: String,
        field: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value_from: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        datasets: Vec<String>,
        #[serde(default)]
        create: bool,
        #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
        tag: Option<u8>,
    },
    AddRow {
        table: String,
        from: String,
        #[serde(default)]
        set: BTreeMap<String, String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        copy: Option<CopyMode>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stat_id: Option<StatIdMode>,
    },
    RemoveRow {
        select: String,
    },
    ListAppend {
        select: String,
        field: String,
        value: String,
    },
    ListRemove {
        select: String,
        field: String,
        value: String,
    },
    PutRecord {
        key: String,
        #[serde(rename = "type")]
        tag: u8,
        value: String,
    },
    /// Removing a key already absent is a no-op. A stock record stays unless
    /// `allow_stock_removal` is set, and a record a track added always stays;
    /// see [`FieldNames::stock`] and [`FieldNames::track`].
    RemoveRecord {
        key: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        allow_stock_removal: bool,
    },
    /// Marks the entrant list labelled `list` as virtual: the build moves its row above the table's count, so the
    /// game loads no list object for it and it takes no stat id and no entrant setup mode slot. The core reads it
    /// into `swofty_virtual_list` when a box naming it is pressed. Put it before the `add-row` that adds the list:
    /// that row then passes the 255 ceiling of `entrants.entrant_lists`.
    VirtualList {
        list: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ErpOp {
    ReplaceResource {
        erp: String,
        resource: String,
        #[serde(default)]
        fragment: usize,
        source: Source,
    },
    AddResource {
        erp: String,
        resource: String,
        #[serde(rename = "type")]
        kind: String,
        sources: Vec<Source>,
    },
    CopyResource {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from_erp: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<Source>,
        erp: String,
        resource: String,
        #[serde(default, rename = "as", skip_serializing_if = "Option::is_none")]
        rename: Option<String>,
    },
    PutResource {
        erp: String,
        resource: String,
        source: Source,
        #[serde(default, rename = "as", skip_serializing_if = "Option::is_none")]
        rename: Option<String>,
    },
    /// Removing a resource the archive does not have is a no-op.
    RemoveResource {
        erp: String,
        resource: String,
    },
    AddMipmaps {
        source: Source,
        target: String,
    },
    PatchPkg {
        erp: String,
        resource: String,
        #[serde(default)]
        fragment: usize,
        edits: Vec<PkgEdit>,
    },
    // A ready overlay archive: each resource in it goes into every one of `erps`, as a
    // put-resource from it would. On the runtime route the archive is loaded as it is.
    Overlay {
        erps: Vec<String>,
        source: Source,
    },
    // Each slice of the texture array `resource` in `source` that differs from the
    // game's stock slice goes over the array already in `erp`.
    MergeSlices {
        erp: String,
        resource: String,
        source: Source,
    },
    // Every slice of the texture array `resource` in `source` goes after the slices of
    // the array already in `erp` (the game's, or as earlier addons left it), in load
    // order, and the array's view (`resource` without `.image`) counts them.
    AppendSlices {
        erp: String,
        resource: String,
        source: Source,
    },
    /// Sets constants of a `.material` resource in place: a number for a scalar, a list for a
    /// vector, written as the material declares them.
    SetMaterial {
        erp: String,
        resource: String,
        params: BTreeMap<String, MaterialValue>,
    },
}

/// A material constant's value: a number, a list, or text from a `{{option}}` placeholder holding
/// one or more numbers separated by spaces or commas (`true` and `false` read as 1 and 0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum MaterialValue {
    Scalar(f64),
    Vector(Vec<MaterialNumber>),
    Text(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum MaterialNumber {
    Number(f64),
    Text(String),
}

fn material_number(text: &str) -> std::result::Result<f64, String> {
    match text.trim() {
        "true" => Ok(1.0),
        "false" => Ok(0.0),
        t => t
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| format!("{t:?} is not a number")),
    }
}

impl MaterialValue {
    pub fn values(&self) -> std::result::Result<Vec<f64>, String> {
        match self {
            Self::Scalar(v) => Ok(vec![*v]),
            Self::Vector(v) => v
                .iter()
                .map(|n| match n {
                    MaterialNumber::Number(v) => Ok(*v),
                    MaterialNumber::Text(t) => material_number(t),
                })
                .collect(),
            Self::Text(t) => t
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter(|s| !s.is_empty())
                .map(material_number)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PkgPatch {
    pub schema_version: u32,
    pub targets: Vec<PkgTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    pub edits: Vec<PkgEdit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PkgTarget {
    pub erp: String,
    pub resource: String,
    #[serde(default)]
    pub fragment: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Signal {
    pub to: String,
    pub pin: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attrs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EventReplacement {
    pub event: String,
    pub signals: Vec<Signal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PkgEdit {
    CloneNode {
        src: String,
        #[serde(rename = "as")]
        name: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        set: BTreeMap<String, serde_json::Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        replace_event: Option<EventReplacement>,
    },
    SetText {
        node: String,
        value: String,
        #[serde(default)]
        localise: bool,
    },
    SetProps {
        node: String,
        set: BTreeMap<String, serde_json::Value>,
    },
    AddSignal {
        node: String,
        event: String,
        signal: Signal,
    },
    RemoveSignal {
        node: String,
        event: String,
        to: String,
    },
    CloneEntry {
        at: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<PkgTarget>,
        src: String,
        #[serde(rename = "as")]
        name: String,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        set: BTreeMap<String, serde_json::Value>,
    },
    SetValue {
        path: String,
        value: serde_json::Value,
    },
}

pub trait FieldNames {
    fn fields(&self, table: &str, row: &str) -> Option<Vec<String>>;

    fn row_keys(&self, shape: &str) -> Option<&[String]> {
        let _ = shape;
        None
    }

    /// Whether `key` is a record of the game's stock database.
    fn stock(&self, key: u64) -> bool {
        let _ = key;
        false
    }

    /// Whether `key` is a record a track's activation added to the database.
    fn track(&self, key: u64) -> bool {
        let _ = key;
        false
    }

    /// Whether an addon later in the build sets `id` on a row it adds to `table`'s rows named `row`.
    fn reserved(&self, table: &str, row: &str, id: i64) -> bool {
        let _ = (table, row, id);
        false
    }

    /// The install's kept ids for the rows `add-row` gives an id of its own; see [`RowIds`].
    fn row_ids(&self) -> Option<&dyn RowIds> {
        None
    }
}

/// The ids the install keeps for the rows `add-row` gives an id of its own, so a row gets the same
/// id at every build whatever else the build adds. A key names an addon's row: `<addon>/<table>.<row>/<label>`.
pub trait RowIds {
    /// The key of this addon's row labelled `label` in `table`'s rows named `row`, claimed for this
    /// build, and the id kept for it.
    fn claim(&self, table: &str, row: &str, label: &str) -> Claimed;

    /// Whether `id` of `table`'s rows named `row` is kept for a row other than `key`, or held by a
    /// record of the core's registry.
    fn taken(&self, table: &str, row: &str, key: &str, id: i64) -> bool;

    /// Keeps `id` for `key`, which it got as `from` says.
    fn keep(&self, table: &str, row: &str, key: &str, id: i64, from: IdFrom);

    /// The lowest id kept for a row of an addon this build leaves out that no record of the core's registry
    /// holds and `free` accepts, moved to `key`, and the key that held it.
    fn reclaim(&self, table: &str, row: &str, key: &str, free: &dyn Fn(i64) -> bool)
    -> Option<(i64, String)>;

    /// The record of the core's registry that `id` of `table`'s rows named `row` is kept for, when saves
    /// hold the table's ids and the record is another addon's with a row label other than `label`.
    fn registry_record(&self, _table: &str, _row: &str, _label: Option<&str>, _id: i64) -> Option<String> {
        None
    }
}

/// A row's claimed key, its kept id, and the id the profile's last build gave a row with its label.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Claimed {
    pub key: String,
    pub kept: Option<i64>,
    pub seed: Option<i64>,
}

/// How a row got the id the install keeps for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IdFrom {
    /// The profile's last build before the store gave its row that id.
    Seeded,
    /// The hash of its key, or the next id after it that was free (1.6.10 and 1.6.11 builds).
    Hashed,
    /// The next free id above the table's rows in load order, as builds before 1.6.10 gave it.
    Next,
    /// The kept id was taken, and the row took a new one.
    Moved,
    /// The table was full, and the row took an id kept for an addon the build left out.
    Reused,
}

/// Stock record keys and no field names.
impl FieldNames for HashSet<u64> {
    fn fields(&self, _: &str, _: &str) -> Option<Vec<String>> {
        None
    }

    fn stock(&self, key: u64) -> bool {
        self.contains(&key)
    }
}

pub const ANY_SHAPE: &str = "*";

pub fn row_shape(path: &str) -> String {
    path.split('.')
        .map(|seg| match seg.rsplit_once('_') {
            Some((word, idx)) if !idx.is_empty() && idx.bytes().all(|b| b.is_ascii_digit()) => {
                format!("{word}_N")
            }
            _ => seg.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

impl FieldNames for BTreeMap<String, Vec<String>> {
    fn fields(&self, table: &str, row: &str) -> Option<Vec<String>> {
        self.get(&format!("{table}.{row}")).cloned()
    }
}

struct Schema<'a> {
    file: &'a BTreeMap<String, Vec<String>>,
    names: &'a dyn FieldNames,
}

impl FieldNames for Schema<'_> {
    fn fields(&self, table: &str, row: &str) -> Option<Vec<String>> {
        self.file
            .fields(table, row)
            .or_else(|| self.names.fields(table, row))
    }

    fn row_keys(&self, shape: &str) -> Option<&[String]> {
        self.names.row_keys(shape)
    }

    fn stock(&self, key: u64) -> bool {
        self.names.stock(key)
    }

    fn track(&self, key: u64) -> bool {
        self.names.track(key)
    }

    fn reserved(&self, table: &str, row: &str, id: i64) -> bool {
        self.names.reserved(table, row, id)
    }

    fn row_ids(&self) -> Option<&dyn RowIds> {
        self.names.row_ids()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Base {
    Path(String),
    Hash(u64),
}

impl Base {
    fn key(&self, suffix: &str) -> u64 {
        match self {
            Base::Path(p) => fnv1a64(format!("{p}{suffix}")),
            Base::Hash(h) => fnv1a64_from(*h, suffix),
        }
    }

    fn child(&self, name: &str) -> Base {
        match self {
            Base::Path(p) => Base::Path(format!("{p}.{name}")),
            Base::Hash(h) => Base::Hash(fnv1a64_from(*h, format!(".{name}"))),
        }
    }
}

impl std::fmt::Display for Base {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Base::Path(p) => f.write_str(p),
            Base::Hash(h) => write!(f, "{{{h:016x}}}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Row {
    base: Base,
}

impl Row {
    fn field(&self, field: &str) -> u64 {
        self.base.key(&format!(".{field}"))
    }
}

struct Db<'a> {
    gvdb: &'a mut Gvdb,
    index: HashMap<u64, usize>,
    removed: Vec<usize>,
    added_rows: HashSet<String>,
    moved: HashMap<(String, String), String>,
    ties: std::cell::RefCell<Vec<String>>,
    // Bumped by every write, so a cached read of the records knows when it is stale.
    generation: u64,
    stat_ids: std::cell::RefCell<HashMap<(String, String), StatIds>>,
}

// The stat_ids of a table's rows, read once and kept up to date by add-row until any other write.
struct StatIds {
    table: String,
    row: String,
    generation: u64,
    ids: Vec<Option<i64>>,
    used: HashSet<i64>,
    max: Option<i64>,
    // The highest id of a stock or track row.
    stock: i64,
    // The ids of the rows this apply didn't add, as a count and a set, to tell whether they are all distinct.
    kept: usize,
    kept_distinct: HashSet<i64>,
    // A start and the lowest id from it that no row holds and the game can read.
    lowest_free: Option<(i64, i64)>,
}

impl StatIds {
    fn read(db: &Db, names: &dyn FieldNames, table: &str, row: &str, rows: usize) -> Self {
        let field = stat_id_field(table, row);
        let mut ids = Self {
            table: table.to_owned(),
            row: row.to_owned(),
            generation: db.generation,
            ids: Vec::with_capacity(rows),
            used: HashSet::with_capacity(rows),
            max: None,
            stock: -1,
            kept: 0,
            kept_distinct: HashSet::new(),
            lowest_free: None,
        };
        for i in 0..rows {
            let name = format!("{table}.{row}_{i}");
            let key = fnv1a64(format!("{name}.{field}"));
            let id = db.text(key).and_then(|v| v.trim().parse::<i64>().ok());
            ids.push(names, key, id, db.added_rows.contains(&name));
        }
        ids
    }

    fn push(&mut self, names: &dyn FieldNames, key: u64, id: Option<i64>, added: bool) {
        self.ids.push(id);
        let Some(id) = id else {
            return;
        };
        if names.stock(key) || names.track(key) {
            self.stock = self.stock.max(id);
        }
        if !added {
            self.kept += 1;
            self.kept_distinct.insert(id);
        }
        self.used.insert(id);
        self.max = Some(self.max.map_or(id, |m| m.max(id)));
        if let Some((_, low)) = &mut self.lowest_free
            && *low == id
        {
            while self.used.contains(low) || stat_id_unusable(&self.table, &self.row, *low) {
                *low += 1;
            }
        }
    }

    // Whether the rows this apply didn't add have ids, all different.
    fn indexed(&self) -> bool {
        self.kept > 0 && self.kept_distinct.len() == self.kept
    }

    fn lowest_free(&mut self, from: i64) -> i64 {
        if let Some((start, low)) = self.lowest_free
            && start == from
        {
            return low;
        }
        let mut low = from;
        while self.used.contains(&low) || stat_id_unusable(&self.table, &self.row, low) {
            low += 1;
        }
        self.lowest_free = Some((from, low));
        low
    }
}

impl<'a> Db<'a> {
    fn new(gvdb: &'a mut Gvdb) -> Self {
        let mut index = HashMap::with_capacity(gvdb.records.len());
        for (i, r) in gvdb.records.iter().enumerate() {
            index.entry(r.key).or_insert(i);
        }
        Self {
            gvdb,
            index,
            removed: Vec::new(),
            added_rows: HashSet::new(),
            moved: HashMap::new(),
            ties: std::cell::RefCell::new(Vec::new()),
            generation: 0,
            stat_ids: std::cell::RefCell::new(HashMap::new()),
        }
    }

    fn stat_ids(
        &self,
        names: &dyn FieldNames,
        table: &str,
        row: &str,
        rows: usize,
    ) -> std::cell::RefMut<'_, StatIds> {
        std::cell::RefMut::map(self.stat_ids.borrow_mut(), |all| {
            let ids = all
                .entry((table.to_owned(), row.to_owned()))
                .or_insert_with(|| StatIds::read(self, names, table, row, rows));
            if ids.generation != self.generation || ids.ids.len() != rows {
                *ids = StatIds::read(self, names, table, row, rows);
            }
            ids
        })
    }

    // Adds the row `name`, the table's `rows`th, to its cached ids, when nothing but that row's writes
    // came since generation `before`.
    fn row_added(
        &self,
        names: &dyn FieldNames,
        table: &str,
        row: &str,
        rows: usize,
        name: &str,
        before: u64,
    ) {
        let mut all = self.stat_ids.borrow_mut();
        let Some(ids) = all.get_mut(&(table.to_owned(), row.to_owned())) else {
            return;
        };
        if ids.generation != before || ids.ids.len() != rows {
            return;
        }
        let key = fnv1a64(format!("{name}.{}", stat_id_field(table, row)));
        let id = self.text(key).and_then(|v| v.trim().parse::<i64>().ok());
        ids.push(names, key, id, true);
        ids.generation = self.generation;
    }

    fn drain_ties(&self, cx: &mut Cx) -> Result<()> {
        for tie in self.ties.take() {
            if cx.strict {
                return Err(Error::Resolve(format!("{tie} (strict)")));
            }
            if !cx.report.warnings.contains(&tie) {
                cx.report.warnings.push(tie);
            }
        }
        Ok(())
    }

    fn remove(&mut self, key: u64) -> bool {
        self.generation += 1;
        match self.index.remove(&key) {
            Some(i) => {
                self.removed.push(i);
                true
            }
            None => false,
        }
    }

    fn finish(self) {
        if self.removed.is_empty() {
            return;
        }
        let removed: HashSet<usize> = self.removed.into_iter().collect();
        let mut i = 0;
        self.gvdb.records.retain(|_| {
            i += 1;
            !removed.contains(&(i - 1))
        });
    }

    fn get(&self, key: u64) -> Option<&Record> {
        self.index.get(&key).map(|&i| &self.gvdb.records[i])
    }

    fn get_mut(&mut self, key: u64) -> Option<&mut Record> {
        self.generation += 1;
        self.index.get(&key).map(|&i| &mut self.gvdb.records[i])
    }

    fn text(&self, key: u64) -> Option<String> {
        self.get(key).map(|r| r.text().into_owned())
    }

    fn push(&mut self, record: Record) {
        self.generation += 1;
        self.index.insert(record.key, self.gvdb.records.len());
        self.gvdb.records.push(record);
    }

    fn instance_count(&self, table: &str, row: &str) -> Option<usize> {
        self.text(fnv1a64(instance_count_path(table, row)))?
            .trim()
            .parse()
            .ok()
    }
}

fn parse_base(s: &str) -> Result<Base> {
    if let Some(hex) = s.strip_prefix('{') {
        let (hex, rest) = hex
            .split_once('}')
            .ok_or_else(|| Error::Resolve(format!("bad placeholder {s}")))?;
        let h = u64::from_str_radix(hex, 16).map_err(|_| Error::Resolve(format!("bad placeholder {s}")))?;
        return Ok(if rest.is_empty() {
            Base::Hash(h)
        } else {
            Base::Hash(fnv1a64_from(h, rest))
        });
    }
    Ok(Base::Path(s.to_owned()))
}

fn row_name(base: &Base, idx: usize, row: &str) -> Base {
    match base {
        Base::Path(p) if row.is_empty() => Base::Path(format!("{p}{idx}")),
        Base::Path(p) => Base::Path(format!("{p}.{row}_{idx}")),
        Base::Hash(h) if row.is_empty() => Base::Hash(fnv1a64_from(*h, idx.to_string())),
        Base::Hash(h) => Base::Hash(fnv1a64_from(*h, format!(".{row}_{idx}"))),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment<'a> {
    pub name: &'a str,
    pub select: Option<(&'a str, &'a str)>,
}

pub fn first_segment(path: &str) -> Result<(Segment<'_>, Option<&str>)> {
    let end = path.find(['.', '[']).unwrap_or(path.len());
    let (name, rest) = path.split_at(end);
    let Some(inner) = rest.strip_prefix('[') else {
        return Ok((Segment { name, select: None }, rest.strip_prefix('.')));
    };
    let close = inner
        .match_indices(']')
        .map(|(i, _)| i)
        .find(|&i| matches!(inner.as_bytes().get(i + 1), None | Some(b'.')))
        .ok_or_else(|| Error::Resolve(format!("{path}: [ needs a ] at the end of its segment")))?;
    let sel = &inner[..close];
    let (field, want) = sel
        .split_once('=')
        .ok_or_else(|| Error::Resolve(format!("{path}: bad selector [{sel}]")))?;
    let segment = Segment {
        name,
        select: Some((field.trim(), want)),
    };
    Ok((segment, inner[close + 1..].strip_prefix('.')))
}

fn select_row(
    db: &Db,
    table: &Base,
    row: &str,
    field: &str,
    want: &str,
    first_wins: bool,
) -> std::result::Result<(Base, usize), usize> {
    let count = match table {
        Base::Path(t) if !row.is_empty() => db.instance_count(t, row),
        _ => None,
    };
    let want = match table {
        Base::Path(t) if field == stat_id_field(t, row) => db
            .moved
            .get(&(format!("{t}.{row}"), want.to_owned()))
            .map_or(want, String::as_str),
        _ => want,
    };
    let mut hits = Vec::new();
    for idx in 0..count.unwrap_or(4096) {
        let candidate = Row {
            base: row_name(table, idx, row),
        };
        match db.text(candidate.field("_metadata_")) {
            Some(_) if db.text(candidate.field(field)).as_deref() == Some(want) => hits.push(candidate.base),
            Some(_) => {}
            None if count.is_none() => break,
            None => {}
        }
    }
    match hits.len() {
        0 => Err(0),
        1 => Ok((hits.remove(0), 1)),
        n if first_wins => Ok((hits.remove(0), n)),
        n => Err(n),
    }
}

const NO_ROW: &str = "no row matches";

/// Whether `e` is a selector that matched no row, which a row added later can fix.
pub fn no_row(e: &Error) -> bool {
    matches!(e, Error::Resolve(m) if m.contains(&format!(": {NO_ROW}")))
}

/// A selector segment of a path: it picks the row `{table}.{row}_{i}` whose `field` is `want`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selector {
    pub table: String,
    pub row: String,
    pub field: String,
    pub want: String,
}

impl Selector {
    /// The key of row `i`'s `field`.
    pub fn field_key(&self, i: usize) -> u64 {
        row_name(&Base::Path(self.table.clone()), i, &self.row).key(&format!(".{}", self.field))
    }

    /// The key of the table's row count; `None` for a table of numbered rows, which has none.
    pub fn count_key(&self) -> Option<u64> {
        (!self.row.is_empty()).then(|| fnv1a64(instance_count_path(&self.table, &self.row)))
    }
}

/// The selector segments of a [`no_row`] error's path, in order, ending with the one that matched no row.
/// A segment after another selector has its table from the error, so only the failing one comes after one.
pub fn no_row_selectors(e: &Error) -> Vec<Selector> {
    let Error::Resolve(m) = e else {
        return Vec::new();
    };
    let Some((upto, scope)) = m.split_once(&format!(": {NO_ROW}")) else {
        return Vec::new();
    };
    let upto = upto.rsplit(": ").next().unwrap_or(upto);
    let resolved = scope.strip_prefix(" in ").and_then(|s| s.rsplit_once('.'));
    let mut out = Vec::new();
    let mut plain = Some(String::new());
    let mut rest = Some(upto);
    while let Some(p) = rest {
        let Ok((seg, tail)) = first_segment(p) else {
            return Vec::new();
        };
        rest = tail;
        let Some((field, want)) = seg.select else {
            if let Some(path) = plain.as_mut() {
                if !path.is_empty() {
                    path.push('.');
                }
                path.push_str(seg.name);
            }
            continue;
        };
        let (table, row) = match (rest, resolved, plain.take()) {
            (None, Some((table, row)), _) => (table.to_owned(), row.to_owned()),
            (_, _, Some(path)) if path.is_empty() => (seg.name.to_owned(), String::new()),
            (_, _, Some(path)) => (path, seg.name.to_owned()),
            (_, _, None) => continue,
        };
        if table.starts_with('{') {
            continue;
        }
        out.push(Selector {
            table,
            row,
            field: field.to_owned(),
            want: want.to_owned(),
        });
    }
    out
}

/// Whether `e` is an `add-row` that found no free `stat_id` in the vehicle or team table, up to the highest the game reads.
pub fn ids_exhausted(e: &Error) -> bool {
    matches!(e, Error::Resolve(m)
        if m.contains(": entrants.vehicles has no free stat_id") || m.contains(": entrants.teams has no free stat_id"))
}

fn resolve_path(db: &Db, path: &str) -> Result<Base> {
    resolve_path_with(db, path, true)
}

fn resolve_path_with(db: &Db, path: &str, first_wins: bool) -> Result<Base> {
    if !path.contains('[') {
        return parse_base(path);
    }
    let mut base: Option<Base> = None;
    let mut nested = false;
    let mut rest = Some(path);
    while let Some(p) = rest {
        let (seg, tail) = first_segment(p)?;
        if seg.name.is_empty() {
            return Err(Error::Resolve(format!("{path}: empty segment")));
        }
        base = Some(match (base, seg.select) {
            (None, None) => parse_base(seg.name)?,
            (Some(b), None) => b.child(seg.name),
            (prefix, Some((field, want))) => {
                let (table, row) = match prefix {
                    Some(b) => (b, seg.name),
                    None => (parse_base(seg.name)?, ""),
                };
                match select_row(db, &table, row, field, want, first_wins) {
                    Ok((b, matched)) => {
                        nested = true;
                        if matched > 1 {
                            let upto = &path[..path.len() - tail.map_or(0, |t| t.len() + 1)];
                            db.ties.borrow_mut().push(format!(
                                "{upto}: {matched} rows match; using the first, {}",
                                b.key("")
                            ));
                        }
                        b
                    }
                    Err(n) => {
                        let upto = &path[..path.len() - tail.map_or(0, |t| t.len() + 1)];
                        let what = match n {
                            0 => NO_ROW.to_owned(),
                            n => format!("{n} rows match"),
                        };
                        let scope = match (nested, row) {
                            (false, _) => String::new(),
                            (true, "") => format!(" in {table}"),
                            (true, row) => format!(" in {table}.{row}"),
                        };
                        return Err(Error::Resolve(format!("{upto}: {what}{scope}")));
                    }
                }
            }
        });
        rest = tail;
    }
    base.ok_or_else(|| Error::Resolve(format!("bad path {path}")))
}

fn resolve_row(db: &Db, select: &str) -> Result<Row> {
    Ok(Row {
        base: resolve_path(db, select)?,
    })
}

fn resolve_value(db: &Db, path: &str) -> Result<String> {
    if !path.contains(['.', ']']) {
        return Err(Error::Resolve(format!("bad path {path}")));
    }
    let key = resolve_path(db, path)?.key("");
    db.text(key)
        .ok_or_else(|| Error::Resolve(format!("{path}: no value")))
}

fn resolve_key(db: &Db, key: &str) -> Result<u64> {
    match key.starts_with("0x") || !key.contains('[') {
        true => record_key(key),
        false => Ok(resolve_path_with(db, key, false)?.key("")),
    }
}

pub fn apply_db_ops(db: &mut Gvdb, ops: &DbOps, names: &dyn FieldNames) -> Result<Vec<u64>> {
    apply_db_ops_report(db, ops, names, &mut DbReport::default())
}

pub fn apply_db_ops_report(
    db: &mut Gvdb,
    ops: &DbOps,
    names: &dyn FieldNames,
    report: &mut DbReport,
) -> Result<Vec<u64>> {
    let schema = Schema {
        file: &ops.fields,
        names,
    };
    let mut db = Db::new(db);
    let mut cx = Cx {
        strict: ops.strict,
        report,
    };
    let mut replaced = Vec::new();
    for op in &ops.ops {
        let applied = apply(&mut db, op, &schema, &mut cx);
        db.drain_ties(&mut cx)?;
        replaced.extend(applied?);
    }
    db.finish();
    Ok(replaced)
}

/// Like [`apply_db_ops`], but an op that fails is skipped and returned with
/// its error instead of stopping the rest. A failing op changes nothing.
pub fn apply_db_ops_lenient(
    db: &mut Gvdb,
    ops: &DbOps,
    names: &dyn FieldNames,
    report: &mut DbReport,
) -> (Vec<u64>, Vec<(usize, Error)>) {
    let schema = Schema {
        file: &ops.fields,
        names,
    };
    let mut db = Db::new(db);
    let mut cx = Cx {
        strict: ops.strict,
        report,
    };
    let mut replaced = Vec::new();
    let mut failed = Vec::new();
    for (i, op) in ops.ops.iter().enumerate() {
        let applied = apply(&mut db, op, &schema, &mut cx);
        let ties = db.drain_ties(&mut cx);
        match applied.and_then(|key| ties.map(|()| key)) {
            Ok(key) => replaced.extend(key),
            Err(e) => failed.push((i, e)),
        }
    }
    db.finish();
    (replaced, failed)
}

pub fn apply_db_op(db: &mut Gvdb, op: &DbOp, names: &dyn FieldNames) -> Result<Option<u64>> {
    let mut db = Db::new(db);
    let mut report = DbReport::default();
    let mut cx = Cx {
        strict: false,
        report: &mut report,
    };
    let replaced = apply(&mut db, op, names, &mut cx)?;
    db.drain_ties(&mut cx)?;
    db.finish();
    Ok(replaced)
}

/// The keys of each row in `rows` that `db` holds, relative to the row, as `add-row` copies them.
pub fn row_keys(db: &mut Gvdb, names: &dyn FieldNames, rows: &[String]) -> Vec<BTreeSet<String>> {
    let db = Db::new(db);
    rows.iter()
        .map(|row| {
            let mut out = Walk::default();
            walk(&db, names, row, "", &mut out);
            out.keys
        })
        .collect()
}

struct Cx<'r> {
    strict: bool,
    report: &'r mut DbReport,
}

impl Cx<'_> {
    fn warn(&mut self, what: String) -> Result<()> {
        if self.strict {
            return Err(Error::Resolve(format!("{what} (strict)")));
        }
        self.report.warnings.push(what);
        Ok(())
    }
}

pub fn record_key(key: &str) -> Result<u64> {
    match key.strip_prefix("0x") {
        Some(hex) if hex.len() == 16 => {
            u64::from_str_radix(hex, 16).map_err(|_| Error::Resolve(format!("bad record key {key}")))
        }
        Some(_) => Err(Error::Resolve(format!("bad record key {key}"))),
        None => Ok(fnv1a64(key)),
    }
}

fn hex_bytes(key: &str, value: &str) -> Result<Vec<u8>> {
    let bad = || Error::Resolve(format!("{key}: value must be hex bytes"));
    if !value.is_ascii() || !value.len().is_multiple_of(2) {
        return Err(bad());
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).map_err(|_| bad()))
        .collect()
}

/// The ids `add-row` ops in `ops` set on their rows, as `("<table>.<row>", id)`; a row of an unlock
/// list as `("<list>.unlockable_item", id)`, since the list's folders and kinds share one id space.
pub fn explicit_stat_ids(ops: &DbOps) -> impl Iterator<Item = (String, i64)> + '_ {
    ops.ops.iter().filter_map(|op| match op {
        DbOp::AddRow { table, from, set, .. } => {
            let row = add_row_name(table, from)?;
            let id = set.get(stat_id_field(table, row))?.trim().parse().ok()?;
            Some(match unlock_list(table) {
                Some(list) => (format!("{list}.{UNLOCK_ROW}"), id),
                None => (format!("{table}.{row}"), id),
            })
        }
        _ => None,
    })
}

pub fn add_row_name<'a>(table: &str, from: &'a str) -> Option<&'a str> {
    from.strip_prefix(table)
        .and_then(|r| r.strip_prefix('.'))
        .map(|r| r.split(['[', '.']).next().unwrap_or(r))
        .map(|r| {
            r.rsplit_once('_')
                .filter(|(_, n)| n.parse::<u32>().is_ok())
                .map_or(r, |(n, _)| n)
        })
}

fn list_items(value: &str) -> impl Iterator<Item = &str> {
    value.split(',').map(str::trim).filter(|v| !v.is_empty())
}

fn edit_list(db: &mut Db, select: &str, field: &str, edit: impl FnOnce(&mut Vec<String>)) -> Result<()> {
    let key = match field.contains('[') {
        true => resolve_path(db, &format!("{select}.{field}"))?.key(""),
        false => resolve_row(db, select)?.field(field),
    };
    let r = db
        .get_mut(key)
        .ok_or_else(|| Error::Resolve(format!("{select}.{field}: no such key")))?;
    let mut items: Vec<String> = list_items(&r.text()).map(String::from).collect();
    edit(&mut items);
    r.value = items.join(",").into_bytes();
    Ok(())
}

fn apply(db: &mut Db, op: &DbOp, names: &dyn FieldNames, cx: &mut Cx) -> Result<Option<u64>> {
    match op {
        DbOp::Set {
            select,
            field,
            value,
            value_from,
            datasets,
            create,
            tag,
        } => {
            if !datasets.is_empty() {
                return Err(Error::Resolve("dataset-scoped set is not implemented yet".into()));
            }
            let value = match (value, value_from) {
                (Some(v), None) => v.clone(),
                (None, Some(from)) => resolve_value(db, from)?,
                _ => {
                    return Err(Error::Resolve(format!(
                        "{select}.{field}: set needs value or value_from"
                    )));
                }
            };
            let key = match field.contains('[') {
                true => resolve_path(db, &format!("{select}.{field}"))?.key(""),
                false => resolve_row(db, select)?.field(field),
            };
            let value = value.into_bytes();
            let was = match db.get_mut(key) {
                Some(r) => Some(std::mem::replace(&mut r.value, value.clone())),
                None if *create => {
                    let tag =
                        tag.ok_or_else(|| Error::Resolve(format!("{select}.{field}: create needs type")))?;
                    db.push(Record {
                        key,
                        tag,
                        value: value.clone(),
                    });
                    None
                }
                None => return Err(Error::Resolve(format!("{select}.{field}: no such key"))),
            };
            rewrote(cx, key, was.as_deref(), Some(&value));
            Ok(Some(key))
        }
        DbOp::AddRow {
            table,
            from,
            set,
            copy,
            stat_id,
        } => {
            add_row(
                db,
                names,
                cx,
                AddRow {
                    table,
                    from,
                    set,
                    copy: *copy,
                    stat_id: *stat_id,
                },
            )?;
            Ok(None)
        }
        DbOp::RemoveRow { .. } => Err(Error::Resolve("remove-row is not implemented yet".into())),
        DbOp::ListAppend { select, field, value } => {
            edit_list(db, select, field, |items| {
                for v in list_items(value) {
                    if !items.iter().any(|i| i == v) {
                        items.push(v.to_string());
                    }
                }
            })?;
            Ok(None)
        }
        DbOp::ListRemove { select, field, value } => {
            let drop: Vec<&str> = list_items(value).collect();
            edit_list(db, select, field, |items| {
                items.retain(|i| !drop.contains(&i.as_str()))
            })?;
            Ok(None)
        }
        DbOp::PutRecord { key, tag, value } => {
            let value = hex_bytes(key, value)?;
            let k = resolve_key(db, key)?;
            let was = match db.get_mut(k) {
                Some(r) => {
                    r.tag = *tag;
                    Some(std::mem::replace(&mut r.value, value.clone()))
                }
                None => {
                    db.push(Record {
                        key: k,
                        tag: *tag,
                        value: value.clone(),
                    });
                    None
                }
            };
            rewrote(cx, k, was.as_deref(), Some(&value));
            Ok(Some(k))
        }
        DbOp::VirtualList { list } => {
            let key = fnv1a64(VIRTUAL_LISTS);
            let mut labels = virtual_lists(db);
            if !labels.iter().any(|l| l == list) {
                labels.push(list.clone());
            }
            let value = labels.join(",").into_bytes();
            match db.get_mut(key) {
                Some(r) => r.value = value,
                None => db.push(Record {
                    key,
                    tag: VIRTUAL_LISTS_TAG,
                    value,
                }),
            }
            Ok(Some(key))
        }
        DbOp::RemoveRecord {
            key,
            allow_stock_removal,
        } => {
            let k = resolve_key(db, key)?;
            if db.get(k).is_none() {
                cx.report.lines.push(format!("{key}: already removed"));
                return Ok(None);
            }
            // The game reads every stock key and crashes at the missing-key sentinel.
            if !allow_stock_removal && names.stock(k) {
                cx.report.stock_removals += 1;
                return Ok(None);
            }
            // A track's new rows count in their tables, and the game reads them like stock rows.
            if names.track(k) {
                cx.report.track_removals += 1;
                return Ok(None);
            }
            let was = db.get(k).map(|r| r.value.clone());
            db.remove(k);
            rewrote(cx, k, was.as_deref(), None);
            Ok(Some(k))
        }
    }
}

struct AddRow<'a> {
    table: &'a str,
    from: &'a str,
    set: &'a BTreeMap<String, String>,
    copy: Option<CopyMode>,
    stat_id: Option<StatIdMode>,
}

#[derive(Default)]
struct Walk {
    keys: BTreeSet<String>,
    uncatalogued: BTreeSet<String>,
}

fn walk(db: &Db, names: &dyn FieldNames, path: &str, rel: &str, out: &mut Walk) {
    let shape = row_shape(path);
    let entries = match names.row_keys(&shape) {
        Some(e) => e,
        None => {
            out.uncatalogued.insert(shape);
            match names.row_keys(ANY_SHAPE) {
                Some(e) => e,
                None => return,
            }
        }
    };
    for entry in entries {
        let Some(sub) = entry.strip_prefix('+') else {
            if db.get(fnv1a64(format!("{path}.{entry}"))).is_some() {
                out.keys.insert(format!("{rel}{entry}"));
            }
            continue;
        };
        let (head, word) = match sub.rsplit_once('.') {
            Some((h, w)) => (format!("{h}."), w),
            None => (String::new(), sub),
        };
        let container = format!("{path}.{}", head.trim_end_matches('.'));
        let container = container.trim_end_matches('.');
        let count_key = format!("{rel}{head}_{word}_instancecount_");
        let rows = match db.instance_count(container, word) {
            Some(n) => {
                out.keys.insert(count_key);
                n
            }
            None => (0..4096)
                .take_while(|i| {
                    db.get(fnv1a64(format!("{container}.{word}_{i}._metadata_")))
                        .is_some()
                })
                .count(),
        };
        for i in 0..rows {
            walk(
                db,
                names,
                &format!("{container}.{word}_{i}"),
                &format!("{rel}{head}{word}_{i}."),
                out,
            );
        }
    }
}

fn cut_by_count(key: &str, set: &BTreeMap<String, String>) -> bool {
    set.iter().any(|(k, v)| {
        let Some(stem) = k.strip_suffix("_instancecount_") else {
            return false;
        };
        let Ok(count) = v.trim().parse::<usize>() else {
            return false;
        };
        let (head, word) = match stem.rsplit_once('.') {
            Some((h, w)) => (format!("{h}."), w),
            None => (String::new(), stem),
        };
        let Some(word) = word.strip_prefix('_') else {
            return false;
        };
        key.strip_prefix(&format!("{head}{word}_"))
            .and_then(|r| r.split_once('.'))
            .and_then(|(i, _)| i.parse::<usize>().ok())
            .is_some_and(|i| i >= count)
    })
}

fn preview(keys: &[&String]) -> String {
    let shown: Vec<&str> = keys.iter().take(12).map(|k| k.as_str()).collect();
    match keys.len() > shown.len() {
        true => format!("{} and {} more", shown.join(", "), keys.len() - shown.len()),
        false => shown.join(", "),
    }
}

// The row added earlier that holds `id`, unless a stock row holds it too.
fn added_holder(
    db: &Db,
    names: &dyn FieldNames,
    table: &str,
    row: &str,
    rows: usize,
    id: i64,
) -> Option<String> {
    let field = stat_id_field(table, row);
    let mut holder = None;
    for i in 0..rows {
        let key = fnv1a64(format!("{table}.{row}_{i}.{field}"));
        if db.text(key).and_then(|v| v.trim().parse::<i64>().ok()) != Some(id) {
            continue;
        }
        if names.stock(key) || names.track(key) {
            return None;
        }
        holder.get_or_insert_with(|| format!("{table}.{row}_{i}"));
    }
    holder
}

// The lowest id from the ceiling's `from`, or above every stock row's id, up to its `max` that no row holds.
fn free_stat_id(
    db: &Db,
    names: &dyn FieldNames,
    table: &str,
    row: &str,
    rows: usize,
    ceiling: StatIdCeiling,
    skip: &dyn Fn(i64) -> bool,
) -> Option<i64> {
    let skip = |id: i64| skip(id) || stat_id_unusable(table, row, id);
    let mut ids = db.stat_ids(names, table, row, rows);
    let from = ceiling.from.unwrap_or(ids.stock + 1);
    // Every id below the lowest free one is held or unreadable, so the search starts there.
    let low = ids.lowest_free(from);
    (low..=ceiling.max).find(|id| !ids.used.contains(id) && !skip(*id))
}

fn add_row(db: &mut Db, names: &dyn FieldNames, cx: &mut Cx, op: AddRow) -> Result<()> {
    let AddRow {
        table,
        from,
        set,
        copy,
        stat_id,
    } = op;
    let src = resolve_row(db, from)?;
    let Base::Path(path) = resolve_path(db, table)? else {
        return Err(Error::Resolve(format!(
            "{table}: add-row needs the table's real path, not a {{hash}}"
        )));
    };
    let Base::Path(src_path) = &src.base else {
        return Err(Error::Resolve(format!("{from} is not a row of {table}")));
    };
    let row = add_row_name(&path, src_path)
        .ok_or_else(|| Error::Resolve(format!("{from} is not a row of {table}")))?;
    let count_path = instance_count_path(&path, row);
    let counted = db.instance_count(&path, row).ok_or_else(|| {
        Error::Resolve(format!(
            "{count_path}: missing; add-row needs the table's real path"
        ))
    })?;
    let mut n = counted;
    while db
        .get(fnv1a64(format!("{path}.{row}_{n}._metadata_")))
        .is_some_and(|r| !names.stock(r.key))
    {
        n += 1;
    }
    let listed = names.fields(table, row).or_else(|| names.fields(&path, row));
    let mode = copy.unwrap_or(CopyMode::All);
    let mut found = Walk::default();
    walk(db, names, src_path, "", &mut found);
    let dst_path = format!("{path}.{row}_{n}");
    let fields: Vec<String> = match (mode, &listed) {
        (CopyMode::Listed, None) => {
            return Err(Error::Resolve(format!(
                "{table}.{row}: copy listed needs a fields list"
            )));
        }
        (CopyMode::Listed, Some(l)) => l.clone(),
        (CopyMode::All, l) => {
            let mut all = found.keys.clone();
            all.extend(l.iter().flatten().cloned());
            all.into_iter().collect()
        }
    };
    if fields.is_empty() {
        return Err(Error::Resolve(format!("{table}.{row}: field names unknown")));
    }
    if let Some(f) = set.keys().find(|f| !fields.contains(f)) {
        return Err(Error::Resolve(format!("{table}.{row}: unknown field {f}")));
    }
    let what = format!("add-row {dst_path} (from {from})");
    if n > counted {
        cx.report.lines.push(format!(
            "{what}: {path}'s count is {counted} but rows {counted} to {} already hold another addon's records; adds after them and raises the count",
            n - 1
        ));
    }
    if !found.uncatalogued.is_empty() {
        let shapes: Vec<&String> = found.uncatalogued.iter().collect();
        cx.warn(format!(
            "{what}: the key catalogue has no entry for {}; keys only the exe builds at run time may be missing",
            preview(&shapes)
        ))?;
    }
    if let Some(l) = &listed {
        let gaps: Vec<&String> = found
            .keys
            .iter()
            .filter(|k| !l.contains(k) && !cut_by_count(k, set))
            .collect();
        match (mode, gaps.is_empty()) {
            (_, true) => {}
            (CopyMode::Listed, false) => cx.warn(format!(
                "{what}: the fields list for {table}.{row} misses {} key(s) of the source row, not copied: {}",
                gaps.len(),
                preview(&gaps)
            ))?,
            (CopyMode::All, false) => cx.report.lines.push(format!(
                "{what}: copied {} key(s) the fields list for {table}.{row} doesn't name: {}",
                gaps.len(),
                preview(&gaps)
            )),
        }
    }
    let dst = Row {
        base: Base::Path(dst_path.clone()),
    };
    let mut rows = Vec::with_capacity(fields.len());
    let mut reused = 0;
    for field in &fields {
        if let Some(r) = db.get(dst.field(field)) {
            // A stock row a mod left above the count it lowered is never read.
            if !names.stock(r.key) {
                return Err(Error::Resolve(format!("{dst_path}.{field} already exists")));
            }
            reused += 1;
        }
        let s = db
            .get(src.field(field))
            .ok_or_else(|| Error::Resolve(format!("{from}: has no {field}")))?;
        let value = set
            .get(field)
            .map(|v| v.clone().into_bytes())
            .unwrap_or_else(|| s.value.clone());
        rows.push(Record {
            key: dst.field(field),
            tag: s.tag,
            value,
        });
    }
    let stat_key = dst.field("stat_id");
    let indexed = || db.stat_ids(names, &path, row, n).indexed();
    let top_level = row_shape(&dst_path).matches("_N").count() == 1;
    let unlock = unlock_list(&path);
    let fresh = match (stat_id, set.contains_key("stat_id")) {
        _ if unlock.is_some() => false,
        (_, true) | (Some(StatIdMode::Keep), _) => false,
        (Some(StatIdMode::Fresh), false) => true,
        (None, false) => {
            let indexed = indexed();
            if indexed
                && !top_level
                && let Some(r) = rows.iter().find(|r| r.key == stat_key)
            {
                cx.report.lines.push(format!(
                    "{what}: keeps the source's stat_id {} in a nested table; set stat_id or \"stat_id\": \"fresh\" for its own",
                    String::from_utf8_lossy(&r.value)
                ));
            }
            indexed && top_level
        }
    };
    let virtual_row = path == "entrants.entrant_lists"
        && set
            .get("_metadata_")
            .is_some_and(|l| virtual_lists(db).contains(l));
    let ceiling = stat_id_ceiling(&path, row).filter(|_| top_level && !virtual_row);
    let field = stat_id_field(&path, row);
    let kept = names.row_ids().filter(|_| top_level);
    let label = rows
        .iter()
        .find(|r| r.key == dst.field("_metadata_"))
        .map(|r| r.text().into_owned());
    let later = |id: i64| names.reserved(&path, row, id) || stat_id_unusable(&path, row, id);
    let held = |id: i64| {
        (0..n).map(|i| format!("{path}.{row}_{i}")).find(|r| {
            db.text(fnv1a64(format!("{r}.{field}")))
                .and_then(|v| v.trim().parse::<i64>().ok())
                == Some(id)
        })
    };
    // The next id above the table's highest, or the lowest free one when that passes the ceiling.
    // A moved id, and any id under the ceiling, passes over the ids later addons set on their rows.
    let next = || -> Result<(i64, Option<String>)> {
        let mut top = db.stat_ids(names, &path, row, n).max.map_or(0, |m| m + 1);
        while later(top) || stat_id_unusable(&path, row, top) {
            top += 1;
        }
        match ceiling {
            Some(c) if top > c.max || c.from.is_some() => free_stat_id(db, names, &path, row, n, c, &later)
                .map(|id| {
                    (
                        id,
                        (top > c.max)
                            .then(|| format!("{top} is above {}, the highest the game reads", c.max)),
                    )
                })
                .ok_or_else(|| {
                    Error::Resolve(format!(
                        "{what}: {path} has no free {} up to {}, the highest the game reads{}",
                        stat_id_field(&path, row),
                        c.max,
                        stat_id_ceiling_reason(&path, row).map_or(String::new(), |r| format!(" ({r})"))
                    ))
                }),
            _ => Ok((top, None)),
        }
    };
    // The id kept for the row when it's still free, else the id the profile's last build gave a row with
    // its label, else the next free id above the table's rows in load order as builds before 1.6.10 gave
    // it (so a fresh install gives a save from then its ids back); a full table takes back an id kept for
    // an addon the build leaves out.
    let give = || -> Result<(i64, Option<String>)> {
        let Some((k, label)) = kept.zip(label.as_deref()) else {
            return next();
        };
        let claimed = k.claim(&path, row, label);
        let key = claimed.key.as_str();
        let unfit = |id: i64| -> Option<String> {
            if let Some(c) = ceiling.filter(|c| id > c.max || id < 0) {
                return Some(format!("is above {}, the highest the game reads", c.max));
            }
            if stat_id_unusable(&path, row, id) {
                return Some("is an id the game reads as another driver or none".to_owned());
            }
            let r = held(id)?;
            let label = db.text(fnv1a64(format!("{r}._metadata_")));
            Some(format!(
                "is held by {r} ({})",
                label.as_deref().unwrap_or("no label")
            ))
        };
        let blocked = |id: i64| -> Option<String> {
            unfit(id)
                .or_else(|| later(id).then(|| "is set by a later addon".to_owned()))
                .or_else(|| {
                    k.taken(&path, row, key, id).then(|| {
                        "is kept for another row or held by a record of the core's registry".to_owned()
                    })
                })
        };
        let mut notes = Vec::new();
        let mut from = IdFrom::Next;
        match claimed.kept {
            Some(id) => match blocked(id) {
                None => return Ok((id, Some("kept from row-ids.json".to_owned()))),
                Some(why) => {
                    notes.push(format!("{id}, kept for it, {why}"));
                    from = IdFrom::Moved;
                }
            },
            None => {
                if let Some(id) = claimed.seed.filter(|id| blocked(*id).is_none()) {
                    k.keep(&path, row, key, id, IdFrom::Seeded);
                    return Ok((id, Some("as the last build gave it".to_owned())));
                }
            }
        }
        let top = db.stat_ids(names, &path, row, n).max.map_or(0, |m| m + 1);
        let picked = (top..=ceiling.map_or(i64::MAX, |c| c.max))
            .find(|id| blocked(*id).is_none())
            .or_else(|| {
                ceiling.and_then(|c| {
                    free_stat_id(db, names, &path, row, n, c, &|id| {
                        later(id) || blocked(id).is_some()
                    })
                })
            })
            .map(|id| (id, IdFrom::Next));
        let id = match picked {
            Some((id, how)) => {
                if from != IdFrom::Moved {
                    from = how;
                }
                id
            }
            None => {
                let free = |id: i64| unfit(id).is_none() && !later(id);
                let Some((id, held_by)) = k.reclaim(&path, row, key, &free) else {
                    return Err(Error::Resolve(format!(
                        "{what}: {path} has no free {field} up to {}, the highest the game reads{}",
                        ceiling.map_or(i64::MAX, |c| c.max),
                        stat_id_ceiling_reason(&path, row).map_or(String::new(), |r| format!(" ({r})"))
                    )));
                };
                let max = ceiling.map_or(String::new(), |c| format!(" up to {}", c.max));
                notes.push(format!(
                    "no other id{max} is free; reused from {held_by}, which this build leaves out"
                ));
                from = IdFrom::Reused;
                id
            }
        };
        k.keep(&path, row, key, id, from);
        Ok((id, (!notes.is_empty()).then(|| notes.join("; "))))
    };
    if fresh && let Some(i) = rows.iter().position(|r| r.key == stat_key) {
        let old = String::from_utf8_lossy(&rows[i].value).into_owned();
        let (next, passed) = give()?;
        rows[i].value = next.to_string().into_bytes();
        cx.report.lines.push(match passed {
            Some(why) => format!("{what}: stat_id {next} (the source's is {old}; {why})"),
            None => format!("{what}: stat_id {next} (the source's is {old})"),
        });
    }
    let id_key = dst.field(field);
    let asked = set.get(field).and_then(|v| v.trim().parse::<i64>().ok());
    let moved = match (asked, ceiling) {
        (Some(old), Some(c)) if old > c.max => Some((
            old,
            None,
            None,
            None,
            format!("is above {}, the highest the game reads", c.max),
        )),
        // Stock scenarios share statId 14, but the core keeps an id pool for them.
        (Some(old), _) if top_level && unlock.is_none() && (field != "stat_id" || indexed()) => {
            added_holder(db, names, &path, row, n, old)
                .map(|holder| {
                    let label = db.text(fnv1a64(format!("{holder}._metadata_")));
                    let why = format!(
                        "is taken by {holder} ({})",
                        label.as_deref().unwrap_or("no label")
                    );
                    (old, Some(holder), label, None, why)
                })
                // A career holds the id the core gave a registry record, so the copy takes another.
                .or_else(|| {
                    let record = kept?.registry_record(&path, row, label.as_deref(), old)?;
                    let why = format!("is kept for {record}, a record of the core's registry");
                    Some((old, None, None, Some(record), why))
                })
        }
        _ => None,
    };
    if let Some((old, holder, holder_label, record, why)) = moved
        && let Some(r) = rows.iter_mut().find(|r| r.key == id_key)
    {
        let (new, note) = give()?;
        r.value = new.to_string().into_bytes();
        let label = set.get("_metadata_").cloned();
        cx.report.lines.push(match note {
            Some(note) => format!("{what}: {field} {old} {why}; {field} {new} ({note})"),
            None => format!("{what}: {field} {old} {why}; {field} {new}"),
        });
        db.moved
            .insert((format!("{path}.{row}"), old.to_string()), new.to_string());
        cx.report.moved_stat_ids.push(MovedStatId {
            row: dst_path.clone(),
            field,
            label,
            old,
            new,
            holder,
            holder_label,
            record,
        });
    }
    if let Some(list) = unlock {
        let unlock_row = UnlockRow {
            list,
            folder: &path,
            kind: row,
            dst: &dst,
            dst_path: &dst_path,
            what: &what,
            set,
            stat_id,
        };
        unlock_stat_id(db, names, cx, &unlock_row, &mut rows)?;
    }
    cx.report
        .lines
        .push(format!("{what}: copied {} key(s)", rows.len()));
    if reused > 0 {
        cx.report.lines.push(format!(
            "{what}: reuses {reused} stock key(s) of a row above the table's count"
        ));
    }
    let before = db.generation;
    for r in rows {
        match db.get_mut(r.key) {
            Some(old) => *old = r,
            None => db.push(r),
        }
    }
    cx.report.added_rows.push(AddedRow {
        from: src_path.clone(),
        row: dst_path.clone(),
    });
    let count = db
        .get_mut(fnv1a64(&count_path))
        .expect("instance_count read it above");
    count.value = (n + 1).to_string().into_bytes();
    db.row_added(names, &path, row, n, &dst_path, before);
    db.added_rows.insert(dst_path);
    Ok(())
}

// A row `add-row` adds to a folder of an unlock list.
struct UnlockRow<'a> {
    list: &'a str,
    folder: &'a str,
    kind: &'a str,
    dst: &'a Row,
    dst_path: &'a str,
    what: &'a str,
    set: &'a BTreeMap<String, String>,
    stat_id: Option<StatIdMode>,
}

// The statIds the rows of an unlock list hold, across all its folders and kinds.
struct UnlockIds {
    // Each id and the row holding it, a stock row before an added one, and whether that row is stock.
    held: HashMap<i64, (String, bool)>,
    // The highest id a stock or track row holds, -1 for none.
    stock_top: i64,
}

impl UnlockIds {
    fn read(db: &Db, names: &dyn FieldNames, list: &str, kind: &str) -> Self {
        let mut ids = Self {
            held: HashMap::new(),
            stock_top: -1,
        };
        let extra = (!UNLOCK_KINDS.contains(&kind)).then_some(kind);
        for f in 0..db.instance_count(list, "folder").unwrap_or(0) {
            let folder = format!("{list}.folder_{f}");
            for kind in UNLOCK_KINDS.iter().copied().chain(extra) {
                let Some(rows) = db.instance_count(&folder, kind) else {
                    continue;
                };
                for i in 0..rows {
                    let row = format!("{folder}.{kind}_{i}");
                    let key = fnv1a64(format!("{row}.statId"));
                    let Some(id) = db.text(key).and_then(|v| v.trim().parse::<i64>().ok()) else {
                        continue;
                    };
                    let stock = names.stock(key) || names.track(key);
                    if stock {
                        ids.stock_top = ids.stock_top.max(id);
                    }
                    match ids.held.entry(id) {
                        std::collections::hash_map::Entry::Vacant(v) => {
                            v.insert((row, stock));
                        }
                        std::collections::hash_map::Entry::Occupied(mut o) => {
                            if stock && !o.get().1 {
                                o.insert((row, stock));
                            }
                        }
                    }
                }
            }
        }
        ids
    }

    // Why the row `key` can't take `id`, if it can't.
    fn blocked(
        &self,
        db: &Db,
        names: &dyn FieldNames,
        list: &str,
        key: Option<&str>,
        id: i64,
    ) -> Option<String> {
        if !(0..=UNLOCK_STAT_MAX).contains(&id) {
            return Some(format!("is outside 0 to {UNLOCK_STAT_MAX}"));
        }
        if let Some((row, stock)) = self.held.get(&id) {
            let label = db.text(fnv1a64(format!("{row}._metadata_")));
            let whose = if *stock { "the game's row" } else { "row" };
            return Some(format!(
                "is held by {whose} {row} ({})",
                label.as_deref().unwrap_or("no label")
            ));
        }
        if names.reserved(list, UNLOCK_ROW, id) {
            return Some("is set by a later addon".to_owned());
        }
        let kept = names.row_ids().zip(key);
        if kept.is_some_and(|(k, key)| k.taken(list, UNLOCK_ROW, key, id)) {
            return Some("is kept for another row or held by a record of the core's registry".to_owned());
        }
        None
    }
}

// Gives the unlock row `u` a `statId` of its own when its `set` leaves the id out (unless
// `"stat_id": "keep"`) or sets one another row of the list holds: the id the install keeps for it, else
// the lowest id from UNLOCK_FRESH_FROM (or above the list's stock rows) to UNLOCK_STAT_MAX that no row of
// the list holds, no later addon sets and row-ids.json keeps for no other row, else one kept for an addon
// the build leaves out.
fn unlock_stat_id(
    db: &mut Db,
    names: &dyn FieldNames,
    cx: &mut Cx,
    u: &UnlockRow,
    rows: &mut [Record],
) -> Result<()> {
    let Some(i) = rows.iter().position(|r| r.key == u.dst.field("statId")) else {
        return Ok(());
    };
    let source = String::from_utf8_lossy(&rows[i].value).trim().to_owned();
    let asked = u.set.get("statId").map(|v| v.trim().parse::<i64>());
    let ids = UnlockIds::read(db, names, u.list, u.kind);
    let clash = match asked {
        None if u.stat_id == Some(StatIdMode::Keep) => return Ok(()),
        None => None,
        // Not a number, or below 0 (some stock rows hold -1): left as the addon set it.
        Some(Err(_)) => return Ok(()),
        Some(Ok(old)) if old < 0 => return Ok(()),
        Some(Ok(old)) => match ids.held.get(&old) {
            Some(held) => Some((old, held.clone())),
            None => return Ok(()),
        },
    };
    let label = rows
        .iter()
        .find(|r| r.key == u.dst.field("_metadata_"))
        .map(|r| r.text().into_owned());
    let kept = names.row_ids().zip(label.as_deref());
    let claimed = kept.map(|(k, label)| k.claim(u.list, UNLOCK_ROW, &format!("{}/{label}", u.kind)));
    let key = claimed.as_ref().map(|c| c.key.as_str());
    let blocked = |id: i64| ids.blocked(db, names, u.list, key, id);
    let keep = |id: i64, from: IdFrom| {
        if let (Some((k, _)), Some(key)) = (kept, key) {
            k.keep(u.list, UNLOCK_ROW, key, id, from);
        }
    };
    let mut notes = Vec::new();
    let mut from = IdFrom::Next;
    let mut given = None;
    match claimed.as_ref().and_then(|c| c.kept) {
        Some(id) => match blocked(id) {
            None => {
                given = Some(id);
                notes.push("kept from row-ids.json".to_owned());
            }
            Some(why) => {
                notes.push(format!("{id}, kept for it, {why}"));
                from = IdFrom::Moved;
            }
        },
        None => {
            if let Some(id) = claimed
                .as_ref()
                .and_then(|c| c.seed)
                .filter(|id| blocked(*id).is_none())
            {
                keep(id, IdFrom::Seeded);
                given = Some(id);
                notes.push("as the last build gave it".to_owned());
            }
        }
    }
    let new = match given {
        Some(id) => id,
        None => {
            let low = (ids.stock_top + 1).max(0);
            let start = low.max(UNLOCK_FRESH_FROM);
            let picked = (start..=UNLOCK_STAT_MAX)
                .chain(low..start)
                .find(|id| blocked(*id).is_none());
            let id = match picked {
                Some(id) => id,
                None => {
                    let free = |id: i64| {
                        (0..=UNLOCK_STAT_MAX).contains(&id)
                            && !ids.held.contains_key(&id)
                            && !names.reserved(u.list, UNLOCK_ROW, id)
                    };
                    let reclaimed = match (kept, key) {
                        (Some((k, _)), Some(key)) => k.reclaim(u.list, UNLOCK_ROW, key, &free),
                        _ => None,
                    };
                    let Some((id, held_by)) = reclaimed else {
                        return Err(Error::Resolve(format!(
                            "{}: {} has no free statId up to {UNLOCK_STAT_MAX} in any of its folders",
                            u.what, u.list
                        )));
                    };
                    notes.push(format!(
                        "no other id up to {UNLOCK_STAT_MAX} is free; reused from {held_by}, which this build leaves out"
                    ));
                    from = IdFrom::Reused;
                    id
                }
            };
            if from != IdFrom::Reused {
                keep(id, from);
            }
            id
        }
    };
    rows[i].value = new.to_string().into_bytes();
    let note = (!notes.is_empty()).then(|| notes.join("; "));
    let what = u.what;
    let Some((old, (holder, _))) = clash else {
        cx.report.lines.push(match note {
            Some(note) => format!("{what}: statId {new} (the source's is {source}; {note})"),
            None => format!("{what}: statId {new} (the source's is {source})"),
        });
        return Ok(());
    };
    let holder_label = db.text(fnv1a64(format!("{holder}._metadata_")));
    let why = format!(
        "is taken by {holder} ({})",
        holder_label.as_deref().unwrap_or("no label")
    );
    cx.report.lines.push(match note {
        Some(note) => format!("{what}: statId {old} {why}; statId {new} ({note})"),
        None => format!("{what}: statId {old} {why}; statId {new}"),
    });
    db.moved.insert(
        (format!("{}.{}", u.folder, u.kind), old.to_string()),
        new.to_string(),
    );
    cx.report.moved_stat_ids.push(MovedStatId {
        row: u.dst_path.to_owned(),
        field: "statId",
        label: u.set.get("_metadata_").cloned(),
        old,
        new,
        holder: Some(holder),
        holder_label,
        record: None,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{FileKind, OpFile};

    #[test]
    fn mask_tables_say_why_their_ceiling_is_31() {
        for (t, r) in [
            ("game_setup.game_setup_modes", "game_setup_mode"),
            ("entrants.entrant_setup_modes", "entrant_setup_mode"),
            ("game_setup.seasons", "season"),
        ] {
            assert_eq!(stat_id_ceiling(t, r).map(|c| c.max), Some(31), "{t}");
            assert!(
                stat_id_ceiling_reason(t, r).is_some_and(|w| w.contains("32-bit mask")),
                "{t}"
            );
        }
        assert_eq!(stat_id_ceiling_reason("entrants.vehicles", "vehicle"), None);
    }

    #[test]
    fn a_third_car_group_fails_the_build_instead_of_the_main_menu() {
        const CARS: &str = "customisation.customisation_cars";
        let mut db = table(CARS, "customisation_car_group", &[0, 1]);
        let stock: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let err = apply_db_ops_report(
            &mut db,
            &add_rows(CARS, "customisation_car_group", &[("sw12_cars", None)]),
            &stock,
            &mut DbReport::default(),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("has no free stat_id up to 1"), "{err}");
        assert!(err.contains("crashes the game at the main menu"), "{err}");
    }

    #[test]
    fn unlock_folders_name_their_list_and_share_its_stat_id_space() {
        let list = "unlockable_content.unlockable_items.unlockable_item_list_2";
        assert_eq!(unlock_list(&format!("{list}.folder_23")), Some(list));
        assert_eq!(unlock_list(list), None);
        assert_eq!(unlock_list(&format!("{list}.folder_x")), None);
        assert_eq!(unlock_list(&format!("{list}.folder_2.more")), None);
        assert_eq!(
            stat_id_field(&format!("{list}.folder_0"), "unlockable_suit"),
            "statId"
        );
        assert!(is_unlock_ids(&format!("{list}.{UNLOCK_ROW}")));
        assert!(!is_unlock_ids("entrants.teams.team"));
        let ops: DbOps = serde_json::from_str(&format!(
            r#"{{"schema_version":1,"ops":[
            {{"op":"add-row","table":"{list}.folder_23","from":"{list}.folder_23.unlockable_se_livery_0","set":{{"_metadata_":"a","statId":"7"}}}},
            {{"op":"add-row","table":"{list}.folder_1","from":"{list}.folder_1.unlockable_suit[_metadata_=x]","set":{{"_metadata_":"b"}}}}]}}"#
        ))
        .unwrap();
        assert_eq!(
            explicit_stat_ids(&ops).collect::<Vec<_>>(),
            [(format!("{list}.unlockable_item"), 7)]
        );
    }

    fn db() -> Gvdb {
        let mut records = vec![Record::new("entrants.teams._team_instancecount_", 0, "2")];
        for (i, label) in ["mclaren_26", "gm_cadillac_26"].iter().enumerate() {
            records.push(Record::new(
                &format!("entrants.teams.team_{i}._metadata_"),
                4,
                *label,
            ));
            records.push(Record::new(
                &format!("entrants.teams.team_{i}.stat_id"),
                0,
                (100 + i).to_string(),
            ));
            records.push(Record::new(
                &format!("entrants.teams.team_{i}.display_name_code"),
                2,
                "XXX",
            ));
        }
        let world = fnv1a64("hidden.world_table_");
        for (i, (label, eri)) in [("f1_26", "_f1_26.wep"), ("myteam_26", "_f1_26_myteam.wep")]
            .iter()
            .enumerate()
        {
            for (f, v) in [("_metadata_", *label), ("worldEri", *eri)] {
                let key = fnv1a64_from(world, format!("{i}.{f}"));
                records.push(Record {
                    key,
                    tag: 2,
                    value: v.as_bytes().to_vec(),
                });
            }
        }
        Gvdb { records }
    }

    fn names() -> BTreeMap<String, Vec<String>> {
        let fields = ["_metadata_", "stat_id", "display_name_code"]
            .map(String::from)
            .to_vec();
        BTreeMap::from([("entrants.teams.team".to_owned(), fields)])
    }

    fn ops(json: &str) -> DbOps {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn side_data_lists_ignore_patterns_and_rejects_the_rest() {
        let file = ops(
            r#"{"schema_version":1,"side_data":{"ignore":["bank.*","accolades.AccoladeDataGroup_N.driver_data.AccoladeDriver_N"]},"ops":[]}"#,
        );
        assert_eq!(file.side_data.ignore.len(), 2);
        let parse = |json: &str| serde_json::from_str::<DbOps>(json).unwrap_err().to_string();
        assert!(
            parse(r#"{"schema_version":1,"side_data":{"ignored":[]},"ops":[]}"#)
                .contains("unknown field `ignored`")
        );
        assert!(
            parse(r#"{"schema_version":1,"side_data":{"ignore":["bank initial"]},"ops":[]}"#)
                .contains("is not a table or field path")
        );
        assert!(
            parse(r#"{"schema_version":1,"side_data":{"ignore":[""]},"ops":[]}"#)
                .contains("side_data.ignore")
        );
        let roundtrip = serde_json::to_string(&ops(r#"{"schema_version":1,"ops":[]}"#)).unwrap();
        assert!(!roundtrip.contains("side_data"), "{roundtrip}");
    }

    #[test]
    fn add_row_appends_and_bumps_count() {
        let mut db = db();
        let ops = ops(
            r#"{"schema_version":1,"ops":[{"op":"add-row","table":"entrants.teams",
            "from":"entrants.teams.team[_metadata_=gm_cadillac_26]",
            "set":{"_metadata_":"team12_26","display_name_code":"T12"}}]}"#,
        );
        apply_db_ops(&mut db, &ops, &names()).unwrap();
        assert_eq!(db.instance_count("entrants.teams", "team"), Some(3));
        assert_eq!(
            db.get_text("entrants.teams.team_2._metadata_").as_deref(),
            Some("team12_26")
        );
        assert_eq!(
            db.get_text("entrants.teams.team_2.stat_id").as_deref(),
            Some("102")
        );
        assert_eq!(
            db.get_text("entrants.teams.team_2.display_name_code").as_deref(),
            Some("T12")
        );
        let bytes = db.to_bytes();
        assert_eq!(Gvdb::parse(&bytes).unwrap().to_bytes(), bytes);
    }

    #[test]
    fn add_row_reads_field_names_from_the_file() {
        let mut db = db();
        let ops = ops(
            r#"{"schema_version":1,"fields":{"entrants.teams.team":["_metadata_","stat_id"]},"ops":[
            {"op":"add-row","table":"entrants.teams","from":"entrants.teams.team[_metadata_=mclaren_26]",
            "stat_id":"keep","set":{"_metadata_":"team12_26"}}]}"#,
        );
        apply_db_ops(&mut db, &ops, &BTreeMap::new()).unwrap();
        assert_eq!(db.instance_count("entrants.teams", "team"), Some(3));
        assert_eq!(
            db.get_text("entrants.teams.team_2.stat_id").as_deref(),
            Some("100")
        );
        assert!(db.get("entrants.teams.team_2.display_name_code").is_none());
    }

    #[test]
    fn add_row_needs_every_field_on_the_source_and_a_free_row() {
        let mut db = db();
        let missing = ops(
            r#"{"schema_version":1,"fields":{"entrants.teams.team":["_metadata_","nope"]},"ops":[
            {"op":"add-row","table":"entrants.teams","from":"entrants.teams.team_0"}]}"#,
        );
        let err = apply_db_ops(&mut db, &missing, &BTreeMap::new()).unwrap_err();
        assert!(err.to_string().contains("has no nope"), "{err}");
        assert_eq!(db.instance_count("entrants.teams", "team"), Some(2));
        db.records
            .push(Record::new("entrants.teams.team_2._metadata_", 4, "stale"));
        let taken = ops(
            r#"{"schema_version":1,"ops":[{"op":"add-row","table":"entrants.teams","from":"entrants.teams.team_0"}]}"#,
        );
        let mut report = DbReport::default();
        apply_db_ops_report(&mut db, &taken, &names(), &mut report).unwrap();
        assert_eq!(db.instance_count("entrants.teams", "team"), Some(4));
        assert_eq!(
            db.get_text("entrants.teams.team_2._metadata_").as_deref(),
            Some("stale")
        );
        assert!(db.get("entrants.teams.team_3._metadata_").is_some());
    }

    #[test]
    fn add_row_adds_after_rows_another_addon_left_above_the_count() {
        let mut db = db();
        for i in [2, 3] {
            db.records.push(Record::new(
                &format!("entrants.teams.team_{i}._metadata_"),
                4,
                format!("other_{i}"),
            ));
        }
        let stock: HashSet<u64> = db
            .records
            .iter()
            .filter(|r| !r.text().starts_with("other_"))
            .map(|r| r.key)
            .collect();
        let add = ops(
            r#"{"schema_version":1,"fields":{"entrants.teams.team":["_metadata_","stat_id","display_name_code"]},
            "ops":[{"op":"add-row","table":"entrants.teams","from":"entrants.teams.team_0",
            "set":{"_metadata_":"mine"}}]}"#,
        );
        let mut report = DbReport::default();
        apply_db_ops_report(&mut db, &add, &stock, &mut report).unwrap();
        assert_eq!(
            db.get_text("entrants.teams.team_2._metadata_").as_deref(),
            Some("other_2")
        );
        assert_eq!(
            db.get_text("entrants.teams.team_3._metadata_").as_deref(),
            Some("other_3")
        );
        assert_eq!(
            db.get_text("entrants.teams.team_4._metadata_").as_deref(),
            Some("mine")
        );
        assert_eq!(db.instance_count("entrants.teams", "team"), Some(5));
        assert!(
            report
                .lines
                .iter()
                .any(|l| l.contains("rows 2 to 3 already hold")),
            "{:?}",
            report.lines
        );
    }

    #[test]
    fn add_row_reuses_a_stock_row_above_a_lowered_count() {
        let mut db = db();
        for (f, v) in [
            ("_metadata_", "stale"),
            ("stat_id", "7"),
            ("display_name_code", "OLD"),
        ] {
            db.records
                .push(Record::new(&format!("entrants.teams.team_2.{f}"), 2, v));
        }
        let stock: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let add = ops(
            r#"{"schema_version":1,"fields":{"entrants.teams.team":["_metadata_","stat_id","display_name_code"]},
            "ops":[{"op":"add-row","table":"entrants.teams","from":"entrants.teams.team_0",
            "set":{"_metadata_":"new_26"}}]}"#,
        );
        let mut report = DbReport::default();
        apply_db_ops_report(&mut db, &add, &stock, &mut report).unwrap();
        let key = fnv1a64("entrants.teams.team_2._metadata_");
        let found: Vec<&Record> = db.records.iter().filter(|r| r.key == key).collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text(), "new_26");
        assert_eq!(found[0].tag, 4);
        let code = fnv1a64("entrants.teams.team_2.display_name_code");
        assert_eq!(db.records.iter().find(|r| r.key == code).unwrap().text(), "XXX");
        assert_eq!(db.instance_count("entrants.teams", "team"), Some(3));
        assert!(
            report.lines.iter().any(|l| l.contains("reuses 3 stock key(s)")),
            "{:?}",
            report.lines
        );
    }

    #[test]
    fn erp_ops_parse_patch_pkg_and_warn() {
        let file = |json: &str| OpFile::from_value(serde_json::from_str(json).unwrap(), FileKind::Erp);
        let ops = file(
            r#"{"schema_version":1,"ops":[
            {"op":"patch-pkg","erp":"a.erp","resource":"eaid://x.layer","edits":[
              {"op":"clone-entry","at":"instances","from":{"erp":"b.erp","resource":"eaid://y.layer"},
               "src":"[name=y$A]","as":"x$B","set":{"overrides.VehPlacement.Id":25}}]},
            {"op":"warn","message":"Imola has room for 26 cars"}]}"#,
        )
        .unwrap();
        assert_eq!(ops.ops.len(), 2);
        assert!(file(r#"{"schema_version":1,"ops":[{"op":"warn"}]}"#).is_err());
    }

    #[test]
    fn set_with_selector_and_value_from_hash_placeholder() {
        let mut db = db();
        let h = format!("{{{:016x}}}", fnv1a64("hidden.world_table_"));
        let json = format!(
            r#"{{"schema_version":1,"ops":[{{"op":"set","select":"{h}[_metadata_=f1_26]","field":"worldEri",
            "value_from":"{h}[_metadata_=myteam_26].worldEri"}}]}}"#
        );
        apply_db_ops(&mut db, &ops(&json), &names()).unwrap();
        let key = fnv1a64_from(fnv1a64("hidden.world_table_"), "0.worldEri");
        assert_eq!(db.get_key(key).unwrap().text(), "_f1_26_myteam.wep");
    }

    #[test]
    fn put_and_remove_records_by_path_and_hash() {
        let mut db = db();
        let count = db.records.len();
        let code = fnv1a64("entrants.teams.team_0.display_name_code");
        let ops = ops(&format!(
            r#"{{"schema_version":1,"ops":[
            {{"op":"put-record","key":"{code:#018x}","type":4,"value":"4d434c"}},
            {{"op":"put-record","key":"entrants.teams.team_0.new_field","type":0,"value":"3132"}},
            {{"op":"remove-record","key":"entrants.teams.team_1.stat_id","allow_stock_removal":true}}]}}"#
        ));
        let stock: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let replaced = apply_db_ops(&mut db, &ops, &stock).unwrap();
        assert_eq!(replaced[0], code);
        assert_eq!(replaced.len(), 3);
        let r = db.get("entrants.teams.team_0.display_name_code").unwrap();
        assert_eq!((r.tag, r.text().as_ref()), (4, "MCL"));
        assert_eq!(
            db.get_text("entrants.teams.team_0.new_field").as_deref(),
            Some("12")
        );
        assert!(db.get("entrants.teams.team_1.stat_id").is_none());
        assert_eq!(db.records.len(), count);
        for bad in [
            r#"{"op":"put-record","key":"0x12","type":0,"value":"00"}"#,
            r#"{"op":"put-record","key":"a.b","type":0,"value":"0"}"#,
        ] {
            let op: DbOp = serde_json::from_str(bad).unwrap();
            assert!(apply_db_op(&mut db, &op, &names()).is_err(), "{bad}");
        }
    }

    #[test]
    fn remove_record_twice_is_idempotent() {
        let key = "entrants.teams.team_1.stat_id";
        let json = format!(
            r#"{{"schema_version":1,"ops":[
            {{"op":"remove-record","key":"{key}"}},
            {{"op":"remove-record","key":"{key}"}}]}}"#
        );
        let mut lenient = db();
        let mut report = DbReport::default();
        let replaced = apply_db_ops_report(&mut lenient, &ops(&json), &names(), &mut report).unwrap();
        assert_eq!(replaced, [fnv1a64(key)]);
        assert!(lenient.get(key).is_none());
        assert_eq!(report.lines, [format!("{key}: already removed")]);

        let strict = format!(
            r#"{{"schema_version":1,"strict":true,"ops":[
            {{"op":"remove-record","key":"{key}"}},
            {{"op":"remove-record","key":"{key}"}}]}}"#
        );
        apply_db_ops(&mut db(), &ops(&strict), &names()).unwrap();
    }

    fn set_track_op(select: &str, value: &str) -> DbOp {
        DbOp::Set {
            select: select.to_owned(),
            field: "track".into(),
            value: Some(value.into()),
            value_from: None,
            datasets: Vec::new(),
            create: false,
            tag: None,
        }
    }

    #[test]
    fn duplicate_labels_resolve_to_the_earliest_row_with_a_warning() {
        let select = "track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData[_metadata_=okayama]";
        let mut ops = ops(r#"{"schema_version": 1, "ops": []}"#);
        ops.ops = vec![set_track_op(select, "first"), set_track_op(select, "again")];
        let mut db = track_db();
        let mut report = DbReport::default();
        apply_db_ops_report(&mut db, &ops, &names(), &mut report).unwrap();
        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
        assert!(
            report.warnings[0].starts_with(&format!("{select}: 2 rows match; using the first")),
            "{:?}",
            report.warnings
        );
        let value = |row: usize| {
            let key = fnv1a64(format!("{SETS}.TrackDataSet_0.TrackData_{row}.track"));
            db.records
                .iter()
                .find(|r| r.key == key)
                .map(|r| String::from_utf8_lossy(&r.value).into_owned())
        };
        assert_eq!(value(1).as_deref(), Some("again"));
        assert_eq!(value(2).as_deref(), Some("okayama"));
    }

    #[test]
    fn duplicate_labels_still_fail_where_the_choice_changes_the_result() {
        let key =
            "track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData[_metadata_=okayama]._metadata_";
        let op = DbOp::RemoveRecord {
            key: key.into(),
            allow_stock_removal: true,
        };
        let err = apply_db_op(&mut track_db(), &op, &names())
            .unwrap_err()
            .to_string();
        assert!(
            err.ends_with("2 rows match in track_data.game_setup.TrackDataSet_0.TrackData"),
            "{err}"
        );
    }

    #[test]
    fn remove_record_keeps_stock_records_unless_the_op_allows_it() {
        let mut db = db();
        let stock: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let json = r#"{"schema_version":1,"ops":[
            {"op":"put-record","key":"entrants.teams.team_0.extra","type":0,"value":"31"},
            {"op":"put-record","key":"entrants.teams.team_1.extra","type":0,"value":"32"},
            {"op":"remove-record","key":"entrants.teams.team_0.stat_id"},
            {"op":"remove-record","key":"entrants.teams.team[_metadata_=gm_cadillac_26].stat_id"},
            {"op":"remove-record","key":"entrants.teams.team_0.extra"},
            {"op":"remove-record","key":"entrants.teams.team_1._metadata_","allow_stock_removal":true}]}"#;
        let mut report = DbReport::default();
        let replaced = apply_db_ops_report(&mut db, &ops(json), &stock, &mut report).unwrap();
        assert_eq!(report.stock_removals, 2);
        assert!(report.warnings.is_empty(), "{report:?}");
        let extra = fnv1a64("entrants.teams.team_0.extra");
        assert_eq!(
            replaced,
            [
                extra,
                fnv1a64("entrants.teams.team_1.extra"),
                extra,
                fnv1a64("entrants.teams.team_1._metadata_")
            ]
        );
        for kept in ["entrants.teams.team_0.stat_id", "entrants.teams.team_1.stat_id"] {
            assert!(db.get(kept).is_some(), "{kept}");
        }
        assert!(db.get("entrants.teams.team_0.extra").is_none());
        assert_eq!(db.get_text("entrants.teams.team_1.extra").as_deref(), Some("2"));
        assert!(db.get("entrants.teams.team_1._metadata_").is_none());

        let mut unguarded = self::db();
        apply_db_ops(&mut unguarded, &ops(json), &names()).unwrap();
        assert!(unguarded.get("entrants.teams.team_0.stat_id").is_none());
        let op: DbOp = serde_json::from_str(r#"{"op":"remove-record","key":"a.b"}"#).unwrap();
        assert_eq!(
            serde_json::to_string(&op).unwrap(),
            r#"{"op":"remove-record","key":"a.b"}"#
        );
    }

    const SETS: &str = "track_data.game_setup";

    fn track_db() -> Gvdb {
        let mut records = vec![Record::new(&instance_count_path(SETS, "TrackDataSet"), 0, "2")];
        for (i, (label, tracks)) in [
            ("f1", &["melbourne", "okayama", "okayama"][..]),
            ("f1_2026", &["melbourne", "suzuka"][..]),
        ]
        .into_iter()
        .enumerate()
        {
            let set = format!("{SETS}.TrackDataSet_{i}");
            records.push(Record::new(&format!("{set}._metadata_"), 4, label));
            records.push(Record::new(
                &instance_count_path(&set, "TrackData"),
                0,
                tracks.len().to_string(),
            ));
            for (j, track) in tracks.iter().enumerate() {
                let row = format!("{set}.TrackData_{j}");
                records.push(Record::new(&format!("{row}._metadata_"), 4, *track));
                records.push(Record::new(&format!("{row}.track"), 2, *track));
                records.push(Record::new(
                    &format!("{row}._TrackNameData_instancecount_"),
                    0,
                    "1",
                ));
                records.push(Record::new(
                    &format!("{row}.TrackNameData_0._metadata_"),
                    4,
                    "names",
                ));
                records.push(Record::new(
                    &format!("{row}.TrackNameData_0.title"),
                    4,
                    format!("lng_{label}_{track}"),
                ));
            }
        }
        Gvdb { records }
    }

    #[test]
    fn parses_segments_like_pkg_paths() {
        let (seg, rest) = first_segment("TrackData[name=a.b$c].TrackNameData_0.title").unwrap();
        assert_eq!(
            (seg.name, seg.select, rest),
            (
                "TrackData",
                Some(("name", "a.b$c")),
                Some("TrackNameData_0.title")
            )
        );
        let (seg, rest) = first_segment("t[ k =x[1]").unwrap();
        assert_eq!((seg.select, rest), (Some(("k", "x[1")), None));
        let (seg, _) = first_segment("t[k=x]y]").unwrap();
        assert_eq!(seg.select, Some(("k", "x]y")));
        let (seg, rest) = first_segment("{446668bc25ba571c}[k=v].a").unwrap();
        assert_eq!((seg.name, rest), ("{446668bc25ba571c}", Some("a")));
        for bad in ["t[k=v", "t[kv].a", "t[k=v]a"] {
            assert!(first_segment(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn selects_rows_on_every_segment() {
        let mut db = track_db();
        let ops = ops(r#"{"schema_version":1,"ops":[
            {"op":"set","select":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=suzuka]",
             "field":"TrackNameData_0.title","value":"lng_a"},
            {"op":"set","select":"track_data.game_setup.TrackDataSet_1.TrackData[_metadata_=melbourne]",
             "field":"TrackNameData_0.title","value":"lng_b"},
            {"op":"set","select":"track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData_0",
             "field":"TrackNameData[_metadata_=names].title","value":"lng_c"}]}"#);
        let replaced = apply_db_ops(&mut db, &ops, &names()).unwrap();
        let key = |p: &str| fnv1a64(format!("{SETS}.{p}.TrackNameData_0.title"));
        assert_eq!(
            replaced,
            [
                key("TrackDataSet_1.TrackData_1"),
                key("TrackDataSet_1.TrackData_0"),
                key("TrackDataSet_0.TrackData_0")
            ]
        );
        for (row, want) in [
            ("TrackDataSet_1.TrackData_1", "lng_a"),
            ("TrackDataSet_1.TrackData_0", "lng_b"),
            ("TrackDataSet_0.TrackData_0", "lng_c"),
        ] {
            let got = db.get_text(&format!("{SETS}.{row}.TrackNameData_0.title"));
            assert_eq!(got.as_deref(), Some(want), "{row}");
        }
    }

    #[test]
    fn nested_selector_errors_name_the_segment() {
        let set = |select: &str| {
            let op = DbOp::Set {
                select: select.to_owned(),
                field: "track".into(),
                value: Some("x".into()),
                value_from: None,
                datasets: Vec::new(),
                create: false,
                tag: None,
            };
            apply_db_op(&mut track_db(), &op, &names())
                .unwrap_err()
                .to_string()
        };
        assert_eq!(
            set("track_data.game_setup.TrackDataSet[_metadata_=f9].TrackData[_metadata_=suzuka]"),
            "track_data.game_setup.TrackDataSet[_metadata_=f9]: no row matches"
        );
        assert_eq!(
            set("track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=okayama]"),
            "track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=okayama]: \
             no row matches in track_data.game_setup.TrackDataSet_1.TrackData"
        );
        assert!(
            set("track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData[track]")
                .contains("bad selector")
        );
        assert!(set("track_data.game_setup..TrackDataSet[_metadata_=f1]").contains("empty segment"));
    }

    #[test]
    fn no_row_errors_give_their_selectors() {
        let fail = |select: &str| {
            let op = DbOp::Set {
                select: select.to_owned(),
                field: "track".into(),
                value: Some("x".into()),
                value_from: None,
                datasets: Vec::new(),
                create: false,
                tag: None,
            };
            no_row_selectors(&apply_db_op(&mut track_db(), &op, &names()).unwrap_err())
        };
        let sel = |table: &str, row: &str, want: &str| Selector {
            table: table.into(),
            row: row.into(),
            field: "_metadata_".into(),
            want: want.into(),
        };
        assert_eq!(
            fail("track_data.game_setup.TrackDataSet[_metadata_=f9].TrackData[_metadata_=suzuka]"),
            [sel(SETS, "TrackDataSet", "f9")]
        );
        assert_eq!(
            fail("track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=okayama]"),
            [
                sel(SETS, "TrackDataSet", "f1_2026"),
                sel(&format!("{SETS}.TrackDataSet_1"), "TrackData", "okayama")
            ]
        );
        let s = sel(SETS, "TrackDataSet", "f1_2026");
        assert_eq!(
            s.field_key(1),
            fnv1a64(format!("{SETS}.TrackDataSet_1._metadata_"))
        );
        assert_eq!(
            s.count_key(),
            Some(fnv1a64(instance_count_path(SETS, "TrackDataSet")))
        );
        assert!(no_row_selectors(&Error::Resolve("t.x: no such key".into())).is_empty());
    }

    #[test]
    fn reports_the_records_ops_change() {
        let mut db = track_db();
        let ops = ops(&format!(
            r#"{{"schema_version":1,"ops":[
            {{"op":"put-record","key":"{SETS}.TrackDataSet_1._metadata_","type":4,"value":"6631"}},
            {{"op":"put-record","key":"{SETS}.TrackDataSet_0._metadata_","type":4,"value":"6631"}},
            {{"op":"set","select":"{SETS}.TrackDataSet_0.TrackData_0","field":"track","value":"x"}},
            {{"op":"remove-record","key":"{SETS}.TrackDataSet_0.TrackData_1.track"}}]}}"#
        ));
        let mut report = DbReport::default();
        apply_db_ops_lenient(&mut db, &ops, &names(), &mut report);
        let rw = |path: &str, was: Option<&str>, now: Option<&str>| Rewrite {
            key: fnv1a64(format!("{SETS}.{path}")),
            was: was.map(String::from),
            now: now.map(String::from),
        };
        assert_eq!(
            report.rewrites,
            [
                rw("TrackDataSet_1._metadata_", Some("f1_2026"), Some("f1")),
                rw("TrackDataSet_0.TrackData_0.track", Some("melbourne"), Some("x")),
                rw("TrackDataSet_0.TrackData_1.track", Some("okayama"), None),
            ]
        );
    }

    #[test]
    fn add_row_into_a_nested_table() {
        let mut db = track_db();
        let first = ops(r#"{"schema_version":1,
            "fields":{"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData":
              ["_metadata_","track","_TrackNameData_instancecount_","TrackNameData_0._metadata_","TrackNameData_0.title"]},
            "ops":[
            {"op":"add-row","table":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026]",
             "from":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=melbourne]",
             "set":{"_metadata_":"okayama","track":"okayama"}},
            {"op":"set","select":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=okayama]",
             "field":"TrackNameData_0.title","value":"lng_okayama"}]}"#);
        apply_db_ops(&mut db, &first, &BTreeMap::new()).unwrap();
        let set = format!("{SETS}.TrackDataSet_1");
        assert_eq!(db.instance_count(&set, "TrackData"), Some(3));
        assert_eq!(
            db.instance_count(&format!("{SETS}.TrackDataSet_0"), "TrackData"),
            Some(3)
        );
        for (field, want) in [
            ("_metadata_", "okayama"),
            ("track", "okayama"),
            ("TrackNameData_0.title", "lng_okayama"),
        ] {
            let got = db.get_text(&format!("{set}.TrackData_2.{field}"));
            assert_eq!(got.as_deref(), Some(want), "{field}");
        }
        let resolved_fields = first.fields.values().next().unwrap().clone();
        let again: DbOps = serde_json::from_value(serde_json::json!({
            "schema_version": 1,
            "fields": { format!("{set}.TrackData"): resolved_fields },
            "ops": [{ "op": "add-row", "table": set,
                "from": "track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=okayama]",
                "set": { "_metadata_": "sepang", "track": "sepang" } }]
        }))
        .unwrap();
        apply_db_ops(&mut db, &again, &BTreeMap::new()).unwrap();
        assert_eq!(db.instance_count(&set, "TrackData"), Some(4));
        assert_eq!(
            db.get_text(&format!("{set}.TrackData_3.TrackNameData_0.title"))
                .as_deref(),
            Some("lng_okayama")
        );
        let wrong_table = ops(
            r#"{"schema_version":1,"ops":[{"op":"add-row","table":"track_data.game_setup.TrackDataSet[_metadata_=f1]",
            "from":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=melbourne]"}]}"#,
        );
        let err = apply_db_ops(&mut db, &wrong_table, &BTreeMap::new()).unwrap_err();
        assert!(err.to_string().contains("is not a row of"), "{err}");
    }

    #[test]
    fn value_from_and_record_keys_take_nested_selectors() {
        let mut db = track_db();
        let ops = ops(r#"{"schema_version":1,"ops":[
            {"op":"set","select":"track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData[_metadata_=melbourne]",
             "field":"TrackNameData_0.title",
             "value_from":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=suzuka].TrackNameData_0.title"},
            {"op":"put-record","key":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=suzuka].laps","type":0,"value":"3533"},
            {"op":"remove-record","key":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=melbourne].track",
             "allow_stock_removal":true}]}"#);
        let stock: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        apply_db_ops(&mut db, &ops, &stock).unwrap();
        let get = |p: &str| db.get_text(&format!("{SETS}.{p}"));
        assert_eq!(
            get("TrackDataSet_0.TrackData_0.TrackNameData_0.title").as_deref(),
            Some("lng_f1_2026_suzuka")
        );
        assert_eq!(get("TrackDataSet_1.TrackData_1.laps").as_deref(), Some("53"));
        assert_eq!(get("TrackDataSet_1.TrackData_0.track"), None);
        assert_eq!(
            get("TrackDataSet_0.TrackData_0.track").as_deref(),
            Some("melbourne")
        );
    }

    #[test]
    fn hash_placeholder_takes_nested_selectors() {
        let mut db = db();
        let world = fnv1a64("hidden.world_table_");
        for (i, (label, n)) in [("a", "6"), ("b", "7")].iter().enumerate() {
            for (f, v) in [("_metadata_", *label), ("n", *n)] {
                db.records.push(Record {
                    key: fnv1a64_from(world, format!("1.slot_{i}.{f}")),
                    tag: 0,
                    value: v.as_bytes().to_vec(),
                });
            }
        }
        let h = format!("{{{world:016x}}}");
        let json = format!(
            r#"{{"schema_version":1,"ops":[{{"op":"set","select":"{h}[_metadata_=f1_26]","field":"worldEri",
            "value_from":"{h}[_metadata_=myteam_26].slot[_metadata_=b].n"}}]}}"#
        );
        apply_db_ops(&mut db, &ops(&json), &names()).unwrap();
        assert_eq!(db.get_key(fnv1a64_from(world, "0.worldEri")).unwrap().text(), "7");
        let json = format!(
            r#"{{"schema_version":1,"ops":[{{"op":"set","select":"{h}[_metadata_=myteam_26].slot[_metadata_=c]","field":"n","value":"1"}}]}}"#
        );
        let err = apply_db_ops(&mut db, &ops(&json), &names())
            .unwrap_err()
            .to_string();
        let row = format!("{{{:016x}}}", fnv1a64_from(world, "1"));
        assert!(
            err.ends_with(&format!("[_metadata_=c]: no row matches in {row}.slot")),
            "{err}"
        );
    }

    #[test]
    fn set_missing_key_fails_without_create() {
        let mut db = db();
        let ops = ops(
            r#"{"schema_version":1,"ops":[{"op":"set","select":"entrants.teams.team_0","field":"nope","value":"1"}]}"#,
        );
        assert!(apply_db_ops(&mut db, &ops, &names()).is_err());
    }

    #[test]
    fn list_append_adds_missing_values_once_and_list_remove_takes_them_out() {
        let mut db = db();
        let append = |v: &str| {
            ops(&format!(
                r#"{{"schema_version":1,"ops":[{{"op":"list-append","select":"entrants.teams.team[_metadata_=mclaren_26]","field":"display_name_code","value":"{v}"}}]}}"#
            ))
        };
        let key = fnv1a64("entrants.teams.team_0.display_name_code");
        assert!(
            apply_db_ops(&mut db, &append("YYY"), &names())
                .unwrap()
                .is_empty()
        );
        apply_db_ops(&mut db, &append("ZZZ, XXX"), &names()).unwrap();
        apply_db_ops(&mut db, &append("YYY"), &names()).unwrap();
        assert_eq!(db.get_key(key).unwrap().text(), "XXX,YYY,ZZZ");
        let remove = ops(
            r#"{"schema_version":1,"ops":[{"op":"list-remove","select":"entrants.teams.team_0","field":"display_name_code","value":"XXX,ZZZ"}]}"#,
        );
        assert!(apply_db_ops(&mut db, &remove, &names()).unwrap().is_empty());
        assert_eq!(db.get_key(key).unwrap().text(), "YYY");
    }

    #[test]
    fn list_ops_on_a_missing_key_fail() {
        let mut db = db();
        let ops = ops(
            r#"{"schema_version":1,"ops":[{"op":"list-append","select":"entrants.teams.team_0","field":"nope","value":"a"}]}"#,
        );
        assert!(apply_db_ops(&mut db, &ops, &names()).is_err());
    }

    #[test]
    fn pkg_patch_parses_design_example() {
        let p: PkgPatch = serde_json::from_str(
            r#"{"schema_version":1,"targets":[{"erp":"2025_asset_groups/ui_package/flow_frontend.erp",
            "resource":"eaid://ui_package/screens/fe_home.ui"}],"root":"Screens[0]:fe_home","edits":[
            {"op":"clone-node","src":"settings","as":"mod_menu_listener","set":{"action_name":"Menu Right Stick Click"},
             "replace_event":{"event":"pressed","signals":[{"to":"fe_home.Action","pin":"route","attrs":{"key":"mod_menu"}}]}},
            {"op":"set-text","node":"contextual_panels.offline_contextual_panel.subtitle","value":"x","localise":false}]}"#,
        )
        .unwrap();
        assert_eq!(p.edits.len(), 2);
    }

    struct Catalogue {
        fields: BTreeMap<String, Vec<String>>,
        shapes: HashMap<String, Vec<String>>,
    }

    impl FieldNames for Catalogue {
        fn fields(&self, table: &str, row: &str) -> Option<Vec<String>> {
            self.fields.fields(table, row)
        }

        fn row_keys(&self, shape: &str) -> Option<&[String]> {
            self.shapes.get(shape).map(Vec::as_slice)
        }
    }

    fn catalogue(fields: &[&str]) -> Catalogue {
        let shape = |keys: &[&str]| keys.iter().map(|k| k.to_string()).collect::<Vec<_>>();
        Catalogue {
            fields: match fields.is_empty() {
                true => BTreeMap::new(),
                false => BTreeMap::from([("modes.mode".to_owned(), shape(fields))]),
            },
            shapes: HashMap::from([
                (
                    "modes.mode_N".to_owned(),
                    shape(&[
                        "_metadata_",
                        "stat_id",
                        "player.driver.preloaded",
                        "_Com_instancecount_",
                        "+Com",
                        "+Loose",
                        "+gone",
                    ]),
                ),
                (
                    "modes.mode_N.Com_N".to_owned(),
                    shape(&["_metadata_", "save_data.group", "_Inner_instancecount_", "+Inner"]),
                ),
                ("modes.mode_N.Com_N.Inner_N".to_owned(), shape(&["value"])),
                ("modes.mode_N.Loose_N".to_owned(), shape(&["_metadata_"])),
            ]),
        }
    }

    fn modes(ids: [&str; 2]) -> Gvdb {
        let mut records = vec![Record::new("modes._mode_instancecount_", 0, "2")];
        for (i, (label, id)) in ["f1", "classic"].iter().zip(ids).enumerate() {
            let r = format!("modes.mode_{i}");
            records.push(Record::new(&format!("{r}._metadata_"), 4, *label));
            records.push(Record::new(&format!("{r}.stat_id"), 0, id));
            records.push(Record::new(&format!("{r}.player.driver.preloaded"), 5, "True"));
        }
        for (k, tag, v) in [
            ("modes.mode_0._Com_instancecount_", 0, "1"),
            ("modes.mode_0.Com_0._metadata_", 4, "ComCareer"),
            ("modes.mode_0.Com_0.save_data.group", 2, "career"),
            ("modes.mode_0.Com_0._Inner_instancecount_", 0, "2"),
            ("modes.mode_0.Com_0.Inner_0.value", 1, "0.5"),
            ("modes.mode_0.Com_0.Inner_1.value", 1, "0.75"),
            ("modes.mode_0.Loose_0._metadata_", 4, "a"),
            ("modes.mode_0.Loose_1._metadata_", 4, "b"),
        ] {
            records.push(Record::new(k, tag, v));
        }
        Gvdb { records }
    }

    fn add_mode(extra: &str) -> DbOps {
        ops(&format!(
            r#"{{"schema_version":1{extra},"ops":[{{"op":"add-row","table":"modes",
            "from":"modes.mode[_metadata_=f1]","set":{{"_metadata_":"f1_1990"}}}}]}}"#
        ))
    }

    #[test]
    fn add_row_copies_every_key_with_nested_rows_and_counts() {
        let mut db = modes(["3", "7"]);
        let mut report = DbReport::default();
        apply_db_ops_report(&mut db, &add_mode(""), &catalogue(&[]), &mut report).unwrap();
        let get = |k: &str| db.get_text(&format!("modes.mode_2.{k}"));
        assert_eq!(db.instance_count("modes", "mode"), Some(3));
        assert_eq!(get("_metadata_").as_deref(), Some("f1_1990"));
        assert_eq!(get("player.driver.preloaded").as_deref(), Some("True"));
        assert_eq!(get("_Com_instancecount_").as_deref(), Some("1"));
        assert_eq!(get("Com_0.save_data.group").as_deref(), Some("career"));
        assert_eq!(get("Com_0._Inner_instancecount_").as_deref(), Some("2"));
        assert_eq!(get("Com_0.Inner_1.value").as_deref(), Some("0.75"));
        assert_eq!(get("Loose_1._metadata_").as_deref(), Some("b"));
        assert!(get("Loose_2._metadata_").is_none());
        assert_eq!(db.get("modes.mode_2.Com_0.Inner_0.value").unwrap().tag, 1);
        assert!(report.warnings.is_empty(), "{:?}", report.warnings);
        assert!(
            report.lines.iter().any(|l| l.ends_with("copied 11 key(s)")),
            "{:?}",
            report.lines
        );
        let bytes = db.to_bytes();
        assert_eq!(Gvdb::parse(&bytes).unwrap().to_bytes(), bytes);
    }

    #[test]
    fn add_row_gives_a_clone_the_next_free_stat_id_and_reports_it() {
        let mut db = modes(["3", "7"]);
        let mut report = DbReport::default();
        apply_db_ops_report(&mut db, &add_mode(""), &catalogue(&[]), &mut report).unwrap();
        apply_db_ops_report(&mut db, &add_mode(""), &catalogue(&[]), &mut report).unwrap();
        assert_eq!(db.get_text("modes.mode_2.stat_id").as_deref(), Some("8"));
        assert_eq!(db.get_text("modes.mode_3.stat_id").as_deref(), Some("9"));
        assert!(
            report.lines.iter().any(|l| l
                == "add-row modes.mode_2 (from modes.mode[_metadata_=f1]): stat_id 8 (the source's is 3)"),
            "{:?}",
            report.lines
        );
        let rows: Vec<(&str, &str)> = report
            .added_rows
            .iter()
            .map(|r| (r.from.as_str(), r.row.as_str()))
            .collect();
        assert_eq!(
            rows,
            [("modes.mode_0", "modes.mode_2"), ("modes.mode_0", "modes.mode_3")]
        );
    }

    #[test]
    fn add_row_keeps_shared_stat_ids_and_explicit_ones() {
        let mut db = modes(["3", "3"]);
        apply_db_ops_report(&mut db, &add_mode(""), &catalogue(&[]), &mut DbReport::default()).unwrap();
        assert_eq!(db.get_text("modes.mode_2.stat_id").as_deref(), Some("3"));

        let mut db = modes(["3", "7"]);
        let keep = ops(
            r#"{"schema_version":1,"ops":[{"op":"add-row","table":"modes","from":"modes.mode_0",
            "stat_id":"keep","set":{"_metadata_":"x"}},{"op":"add-row","table":"modes","from":"modes.mode_0",
            "set":{"_metadata_":"y","stat_id":"40"}}]}"#,
        );
        apply_db_ops_report(&mut db, &keep, &catalogue(&[]), &mut DbReport::default()).unwrap();
        assert_eq!(db.get_text("modes.mode_2.stat_id").as_deref(), Some("3"));
        assert_eq!(db.get_text("modes.mode_3.stat_id").as_deref(), Some("40"));

        let mut db = modes(["3", "3"]);
        let fresh = ops(
            r#"{"schema_version":1,"ops":[{"op":"add-row","table":"modes","from":"modes.mode_0",
            "stat_id":"fresh","set":{"_metadata_":"x"}}]}"#,
        );
        apply_db_ops_report(&mut db, &fresh, &catalogue(&[]), &mut DbReport::default()).unwrap();
        assert_eq!(db.get_text("modes.mode_2.stat_id").as_deref(), Some("4"));
    }

    struct Stocked {
        names: Catalogue,
        stock: HashSet<u64>,
    }

    impl FieldNames for Stocked {
        fn fields(&self, table: &str, row: &str) -> Option<Vec<String>> {
            self.names.fields(table, row)
        }

        fn row_keys(&self, shape: &str) -> Option<&[String]> {
            self.names.row_keys(shape)
        }

        fn stock(&self, key: u64) -> bool {
            self.stock.contains(&key)
        }
    }

    fn stocked(db: &Gvdb) -> Stocked {
        Stocked {
            names: catalogue(&[]),
            stock: db.records.iter().map(|r| r.key).collect(),
        }
    }

    struct Reserved {
        inner: Stocked,
        ids: Vec<(String, i64)>,
    }

    impl FieldNames for Reserved {
        fn fields(&self, table: &str, row: &str) -> Option<Vec<String>> {
            self.inner.fields(table, row)
        }

        fn row_keys(&self, shape: &str) -> Option<&[String]> {
            self.inner.row_keys(shape)
        }

        fn stock(&self, key: u64) -> bool {
            self.inner.stock(key)
        }

        fn reserved(&self, table: &str, row: &str, id: i64) -> bool {
            self.ids.contains(&(format!("{table}.{row}"), id))
        }
    }

    #[test]
    fn a_free_stat_id_skips_the_id_a_later_addon_pins() {
        let mut db = modes(["3", "7"]);
        let names = Reserved {
            inner: stocked(&db),
            ids: vec![("modes.mode".to_owned(), 8)],
        };
        let mut report = DbReport::default();
        apply_db_ops_report(&mut db, &add_labelled("early", "", ""), &names, &mut report).unwrap();
        assert_eq!(db.get_text("modes.mode_2.stat_id").as_deref(), Some("9"));
        let mut report = DbReport::default();
        apply_db_ops_report(
            &mut db,
            &add_labelled("late", r#","stat_id":"8""#, ""),
            &names,
            &mut report,
        )
        .unwrap();
        assert_eq!(db.get_text("modes.mode_3.stat_id").as_deref(), Some("8"));
        assert!(report.moved_stat_ids.is_empty(), "{:?}", report.moved_stat_ids);
    }

    #[test]
    fn two_pins_on_the_same_stat_id_still_conflict() {
        let mut db = modes(["3", "7"]);
        let names = Reserved {
            inner: stocked(&db),
            ids: vec![("modes.mode".to_owned(), 40)],
        };
        let mut report = DbReport::default();
        apply_db_ops_report(
            &mut db,
            &add_labelled("a", r#","stat_id":"40""#, ""),
            &names,
            &mut report,
        )
        .unwrap();
        let mut report = DbReport::default();
        apply_db_ops_report(
            &mut db,
            &add_labelled("b", r#","stat_id":"40""#, ""),
            &names,
            &mut report,
        )
        .unwrap();
        assert_eq!(db.get_text("modes.mode_2.stat_id").as_deref(), Some("40"));
        assert_ne!(db.get_text("modes.mode_3.stat_id").as_deref(), Some("40"));
        assert_eq!(report.moved_stat_ids.len(), 1);
        assert_eq!(report.moved_stat_ids[0].old, 40);
    }

    fn add_labelled(label: &str, stat_id: &str, then: &str) -> DbOps {
        ops(&format!(
            r#"{{"schema_version":1,"ops":[{{"op":"add-row","table":"modes","from":"modes.mode[_metadata_=f1]",
            "set":{{"_metadata_":"{label}"{stat_id}}}}}{then}]}}"#
        ))
    }

    #[test]
    fn an_explicit_stat_id_an_added_row_holds_gets_the_next_free_one() {
        let mut db = modes(["3", "7"]);
        let names = stocked(&db);
        let mut report = DbReport::default();
        apply_db_ops_report(&mut db, &add_labelled("first", "", ""), &names, &mut report).unwrap();
        assert_eq!(db.get_text("modes.mode_2.stat_id").as_deref(), Some("8"));
        let mut report = DbReport::default();
        let ours = add_labelled(
            "second",
            r#","stat_id":"8""#,
            r#",{"op":"set","select":"modes.mode[stat_id=8]","field":"player.driver.preloaded","value":"False"},
            {"op":"set","select":"modes.mode_0","field":"player.driver.preloaded","value_from":"modes.mode[_metadata_=second].stat_id"}"#,
        );
        apply_db_ops_report(&mut db, &ours, &names, &mut report).unwrap();
        assert_eq!(db.get_text("modes.mode_3.stat_id").as_deref(), Some("9"));
        assert_eq!(
            report.moved_stat_ids,
            [MovedStatId {
                row: "modes.mode_3".into(),
                field: "stat_id",
                label: Some("second".into()),
                old: 8,
                new: 9,
                holder: Some("modes.mode_2".into()),
                holder_label: Some("first".into()),
                record: None,
            }]
        );
        assert!(
            report.lines.iter().any(|l| l
                == "add-row modes.mode_3 (from modes.mode[_metadata_=f1]): stat_id 8 is taken by modes.mode_2 (first); stat_id 9"),
            "{:?}",
            report.lines
        );
        assert_eq!(
            db.get_text("modes.mode_3.player.driver.preloaded").as_deref(),
            Some("False")
        );
        assert_eq!(
            db.get_text("modes.mode_2.player.driver.preloaded").as_deref(),
            Some("True")
        );
        assert_eq!(
            db.get_text("modes.mode_0.player.driver.preloaded").as_deref(),
            Some("9")
        );
    }

    #[test]
    fn explicit_stat_ids_without_a_clash_or_shared_with_a_stock_row_stay() {
        for (id, want) in [("40", "40"), ("7", "7")] {
            let mut db = modes(["3", "7"]);
            let names = stocked(&db);
            apply_db_ops(&mut db, &add_labelled("first", "", ""), &names).unwrap();
            let mut report = DbReport::default();
            let ours = add_labelled("second", &format!(r#","stat_id":"{id}""#), "");
            apply_db_ops_report(&mut db, &ours, &names, &mut report).unwrap();
            assert_eq!(db.get_text("modes.mode_3.stat_id").as_deref(), Some(want));
            assert!(report.moved_stat_ids.is_empty(), "{:?}", report.moved_stat_ids);
            assert!(
                !report.lines.iter().any(|l| l.contains("is taken by")),
                "{:?}",
                report.lines
            );
        }
        let mut db = modes(["3", "7"]);
        let names = stocked(&db);
        let keep = ops(
            r#"{"schema_version":1,"ops":[{"op":"add-row","table":"modes","from":"modes.mode_0",
            "stat_id":"keep","set":{"_metadata_":"x"}},{"op":"add-row","table":"modes","from":"modes.mode_0",
            "set":{"_metadata_":"y","stat_id":"3"}}]}"#,
        );
        let mut report = DbReport::default();
        apply_db_ops_report(&mut db, &keep, &names, &mut report).unwrap();
        assert_eq!(db.get_text("modes.mode_3.stat_id").as_deref(), Some("3"));
        assert!(report.moved_stat_ids.is_empty());
    }

    const PACKAGES: &str = "entrants.vehicles.vehicle_packages";

    fn table(path: &str, row: &str, ids: &[i64]) -> Gvdb {
        let mut records = vec![Record::new(
            &format!("{path}._{row}_instancecount_"),
            0,
            ids.len().to_string(),
        )];
        for (i, id) in ids.iter().enumerate() {
            let r = format!("{path}.{row}_{i}");
            records.push(Record::new(&format!("{r}._metadata_"), 4, format!("stock_{i}")));
            records.push(Record::new(&format!("{r}.stat_id"), 0, id.to_string()));
        }
        Gvdb { records }
    }

    fn add_rows(path: &str, row: &str, rows: &[(&str, Option<i64>)]) -> DbOps {
        let ops: Vec<String> = rows
            .iter()
            .map(|(label, id)| {
                let id = id.map_or_else(String::new, |id| format!(r#","stat_id":"{id}""#));
                format!(
                    r#"{{"op":"add-row","table":"{path}","from":"{path}.{row}_0","set":{{"_metadata_":"{label}"{id}}}}}"#
                )
            })
            .collect();
        self::ops(&format!(
            r#"{{"schema_version":1,"fields":{{"{path}.{row}":["_metadata_","stat_id"]}},"ops":[{}]}}"#,
            ops.join(",")
        ))
    }

    fn ids(db: &Gvdb, path: &str, row: &str) -> Vec<i64> {
        let n = db.instance_count(path, row).unwrap();
        (0..n)
            .map(|i| {
                db.get_text(&format!("{path}.{row}_{i}.stat_id"))
                    .unwrap()
                    .parse()
                    .unwrap()
            })
            .collect()
    }

    #[test]
    fn fresh_package_ids_past_the_byte_ceiling_take_the_lowest_free_ones_above_stock() {
        let mut db = table(PACKAGES, "vehicle_package", &[2, 5, 58]);
        let stock: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let labels: Vec<String> = (0..90).map(|i| format!("pkg_{i}")).collect();
        let mut era: Vec<(&str, Option<i64>)> = labels[..24].iter().map(|l| (l.as_str(), None)).collect();
        era.push(("era_200", Some(200)));
        let mut report = DbReport::default();
        apply_db_ops_report(
            &mut db,
            &add_rows(PACKAGES, "vehicle_package", &era),
            &stock,
            &mut report,
        )
        .unwrap();
        assert_eq!(
            ids(&db, PACKAGES, "vehicle_package")[3..],
            [(59..=82).collect::<Vec<_>>(), vec![200]].concat()
        );
        assert!(report.moved_stat_ids.is_empty());
        let more: Vec<(&str, Option<i64>)> = labels.iter().map(|l| (l.as_str(), None)).collect();
        let mut report = DbReport::default();
        apply_db_ops_report(
            &mut db,
            &add_rows(PACKAGES, "vehicle_package", &more),
            &stock,
            &mut report,
        )
        .unwrap();
        let all = ids(&db, PACKAGES, "vehicle_package");
        let added = &all[3 + 25..];
        assert_eq!(added[..54], (201..=254).collect::<Vec<_>>());
        assert_eq!(added[54..], (83..=118).collect::<Vec<_>>());
        assert!(all.iter().all(|id| *id <= 254));
        assert_eq!(all.iter().collect::<HashSet<_>>().len(), all.len());
        assert!(
            report.lines.iter().any(|l| l
                == "add-row entrants.vehicles.vehicle_packages.vehicle_package_82 (from entrants.vehicles.vehicle_packages.vehicle_package_0): stat_id 83 (the source's is 2; 255 is above 254, the highest the game reads)"),
            "{:?}",
            report.lines
        );
    }

    #[test]
    fn a_virtual_entrant_list_passes_the_byte_ceiling_its_row_never_reaches() {
        const LISTS: &str = "entrants.entrant_lists";
        let mut db = table(LISTS, "entrant_list", &[0, 255]);
        let stock: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let full = apply_db_ops_report(
            &mut db.clone(),
            &add_rows(LISTS, "entrant_list", &[("season_1960", None)]),
            &stock,
            &mut DbReport::default(),
        )
        .unwrap_err();
        assert!(
            full.to_string().contains("has no free stat_id up to 255"),
            "{full}"
        );
        let mut ops = add_rows(LISTS, "entrant_list", &[("season_1960", None)]);
        ops.ops.insert(
            0,
            DbOp::VirtualList {
                list: "season_1960".into(),
            },
        );
        apply_db_ops_report(&mut db, &ops, &stock, &mut DbReport::default()).unwrap();
        assert_eq!(db.get_text(VIRTUAL_LISTS).as_deref(), Some("season_1960"));
        assert_eq!(
            db.get_text("entrants.entrant_lists.entrant_list_2._metadata_")
                .as_deref(),
            Some("season_1960")
        );
        assert_eq!(ids(&db, LISTS, "entrant_list"), [0, 255, 256]);
    }

    #[test]
    fn leaderboard_descriptor_ids_stay_in_the_byte_the_game_reads() {
        const BOARDS: &str = "leaderboards";
        assert_eq!(
            stat_id_ceiling(BOARDS, "leaderboard_descriptor"),
            Some(StatIdCeiling { max: 255, from: None })
        );
        let mut db = table(BOARDS, "leaderboard_descriptor", &[0, 1, 3, 2, 5]);
        let stock: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let rows = [("swofty_a", None), ("swofty_b", None), ("swofty_c", Some(300))];
        let mut report = DbReport::default();
        apply_db_ops_report(
            &mut db,
            &add_rows(BOARDS, "leaderboard_descriptor", &rows),
            &stock,
            &mut report,
        )
        .unwrap();
        assert_eq!(
            ids(&db, BOARDS, "leaderboard_descriptor"),
            [0, 1, 3, 2, 5, 6, 7, 8]
        );
        assert_eq!(
            db.get_text("leaderboards.leaderboard_descriptor_5._metadata_")
                .as_deref(),
            Some("swofty_a")
        );
        let labels: Vec<String> = (0..260).map(|i| format!("box_{i}")).collect();
        let many: Vec<(&str, Option<i64>)> = labels.iter().map(|l| (l.as_str(), None)).collect();
        let mut full = table(BOARDS, "leaderboard_descriptor", &[0, 1, 3, 2, 5]);
        let stock: HashSet<u64> = full.records.iter().map(|r| r.key).collect();
        let err = apply_db_ops_report(
            &mut full,
            &add_rows(BOARDS, "leaderboard_descriptor", &many),
            &stock,
            &mut DbReport::default(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("has no free stat_id up to 255"), "{err}");
    }

    #[test]
    fn package_ids_under_the_ceiling_stay_as_they_were() {
        let mut db = table(PACKAGES, "vehicle_package", &[2, 5, 58]);
        let stock: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let rows = [("a", None), ("b", Some(120)), ("c", None), ("d", Some(254))];
        let mut report = DbReport::default();
        apply_db_ops_report(
            &mut db,
            &add_rows(PACKAGES, "vehicle_package", &rows),
            &stock,
            &mut report,
        )
        .unwrap();
        assert_eq!(
            ids(&db, PACKAGES, "vehicle_package"),
            [2, 5, 58, 59, 120, 121, 254]
        );
        assert!(report.moved_stat_ids.is_empty());
        assert!(
            !report
                .lines
                .iter()
                .any(|l| l.contains("the highest the game reads")),
            "{:?}",
            report.lines
        );
        let mut teams = table("entrants.teams", "team", &[0, 499]);
        let stock: HashSet<u64> = teams.records.iter().map(|r| r.key).collect();
        apply_db_ops(
            &mut teams,
            &add_rows("entrants.teams", "team", &[("t", None), ("u", Some(900))]),
            &stock,
        )
        .unwrap();
        assert_eq!(ids(&teams, "entrants.teams", "team"), [0, 499, 500, 900]);
    }

    #[test]
    fn an_explicit_package_id_above_the_ceiling_moves_and_selectors_follow() {
        let mut db = table(PACKAGES, "vehicle_package", &[2, 5, 58]);
        let stock: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let mut file = add_rows(PACKAGES, "vehicle_package", &[("high", Some(300))]);
        file.ops.extend(
            self::ops(&format!(
                r#"{{"schema_version":1,"ops":[{{"op":"set","select":"{PACKAGES}.vehicle_package[stat_id=300]","field":"_metadata_","value":"found"}}]}}"#
            ))
            .ops,
        );
        let mut report = DbReport::default();
        apply_db_ops_report(&mut db, &file, &stock, &mut report).unwrap();
        assert_eq!(ids(&db, PACKAGES, "vehicle_package"), [2, 5, 58, 59]);
        assert_eq!(
            db.get_text(&format!("{PACKAGES}.vehicle_package_3._metadata_"))
                .as_deref(),
            Some("found")
        );
        assert_eq!(
            report.moved_stat_ids,
            [MovedStatId {
                row: format!("{PACKAGES}.vehicle_package_3"),
                field: "stat_id",
                label: Some("high".into()),
                old: 300,
                new: 59,
                holder: None,
                holder_label: None,
                record: None,
            }]
        );
    }

    #[test]
    fn driver_ids_fill_208_to_253_then_256_up_and_skip_254_255_and_low_byte_0xff() {
        let mut db = table("entrants.drivers", "driver", &[254, 0, 207, 210]);
        let stock: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let labels: Vec<String> = (0..300).map(|i| format!("d_{i}")).collect();
        let rows: Vec<(&str, Option<i64>)> = labels.iter().map(|l| (l.as_str(), None)).collect();
        apply_db_ops(&mut db, &add_rows("entrants.drivers", "driver", &rows), &stock).unwrap();
        let all = ids(&db, "entrants.drivers", "driver");
        assert_eq!(all[4..7], [208, 209, 211]);
        let added = &all[4..];
        assert_eq!(
            added[..45],
            (208..=253).filter(|id| *id != 210).collect::<Vec<_>>()[..45]
        );
        assert_eq!(added[45], 256);
        assert_eq!(added[300 - 1], 510);
        assert!(
            added
                .iter()
                .all(|id| *id != 254 && *id != 255 && id & 0xff != 0xff && *id <= 16382)
        );
        assert_eq!(added.iter().filter(|id| **id > 255).count(), 300 - 45);
    }

    #[test]
    fn driver_ids_stop_at_the_14_bit_ceiling() {
        let mut db = table("entrants.drivers", "driver", &[254]);
        let stock: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let usable = (208..=16382)
            .filter(|id| !stat_id_unusable("entrants.drivers", "driver", *id))
            .count();
        let labels: Vec<String> = (0..usable).map(|i| format!("d_{i}")).collect();
        let rows: Vec<(&str, Option<i64>)> = labels.iter().map(|l| (l.as_str(), None)).collect();
        apply_db_ops(&mut db, &add_rows("entrants.drivers", "driver", &rows), &stock).unwrap();
        let all = ids(&db, "entrants.drivers", "driver");
        assert_eq!(*all.last().unwrap(), 16382);
        let e = apply_db_ops(
            &mut db,
            &add_rows("entrants.drivers", "driver", &[("full", None)]),
            &stock,
        )
        .unwrap_err()
        .to_string();
        assert!(
            e.contains("entrants.drivers has no free stat_id up to 16382"),
            "{e}"
        );
        let (_, failed) = apply_db_ops_lenient(
            &mut db,
            &add_rows("entrants.drivers", "driver", &[("full", None)]),
            &stock,
            &mut DbReport::default(),
        );
        assert_eq!(failed.len(), 1);
        assert_eq!(db.instance_count("entrants.drivers", "driver"), Some(1 + usable));
    }

    // The ids fresh rows took before 1.7.1, when each add-row read every row's id again: the next above
    // the table's highest, or for a table with a ceiling the lowest free one from its `from` (or above
    // the stock rows) that the game reads. None once nothing is free.
    fn ids_before_the_cache(stock: &[i64], path: &str, row: &str, adds: usize) -> Vec<Option<i64>> {
        let ceiling = stat_id_ceiling(path, row);
        let stock_max = stock.iter().copied().max().unwrap_or(-1);
        let mut all = stock.to_vec();
        (0..adds)
            .map(|_| {
                let mut top = all.iter().copied().max().map_or(0, |m| m + 1);
                while stat_id_unusable(path, row, top) {
                    top += 1;
                }
                let id = match ceiling {
                    Some(c) if top > c.max || c.from.is_some() => {
                        let used: HashSet<i64> = all.iter().copied().collect();
                        (c.from.unwrap_or(stock_max + 1)..=c.max)
                            .find(|id| !used.contains(id) && !stat_id_unusable(path, row, *id))
                    }
                    _ => Some(top),
                };
                all.extend(id);
                id
            })
            .collect()
    }

    fn added_ids(stock: &[i64], path: &str, row: &str, adds: usize) -> Vec<Option<i64>> {
        let mut db = table(path, row, stock);
        let names: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let labels: Vec<String> = (0..adds).map(|i| format!("added_{i}")).collect();
        let rows: Vec<(&str, Option<i64>)> = labels.iter().map(|l| (l.as_str(), None)).collect();
        let (_, failed) = apply_db_ops_lenient(
            &mut db,
            &add_rows(path, row, &rows),
            &names,
            &mut DbReport::default(),
        );
        let mut got: Vec<Option<i64>> = ids(&db, path, row)[stock.len()..]
            .iter()
            .copied()
            .map(Some)
            .collect();
        got.extend(failed.iter().map(|_| None));
        got
    }

    #[test]
    fn fresh_driver_ids_match_the_search_before_the_cache() {
        let stock = [1, 7, 30, 207, 254];
        assert_eq!(
            added_ids(&stock, "entrants.drivers", "driver", 400),
            ids_before_the_cache(&stock, "entrants.drivers", "driver", 400)
        );
    }

    #[test]
    fn fresh_driver_ids_up_to_the_14_bit_ceiling_match_the_search_before_the_cache() {
        let mut stock: Vec<i64> = (208..=16100)
            .filter(|id| !stat_id_unusable("entrants.drivers", "driver", *id) && id % 97 != 0)
            .collect();
        stock.push(254);
        let before = ids_before_the_cache(&stock, "entrants.drivers", "driver", 450);
        assert_eq!(before.iter().flatten().max(), Some(&16382));
        assert!(before.last().unwrap().is_none());
        assert_eq!(added_ids(&stock, "entrants.drivers", "driver", 450), before);
    }

    #[test]
    fn fresh_package_ids_past_the_byte_ceiling_match_the_search_before_the_cache() {
        let stock = [2, 5, 58];
        let before = ids_before_the_cache(&stock, PACKAGES, "vehicle_package", 300);
        assert!(before.last().unwrap().is_none());
        assert_eq!(added_ids(&stock, PACKAGES, "vehicle_package", 300), before);
    }

    #[test]
    fn an_id_set_between_add_rows_frees_the_one_it_replaced() {
        let mut db = table("entrants.drivers", "driver", &[1, 7]);
        let names: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let ops = self::ops(
            r#"{"schema_version":1,"fields":{"entrants.drivers.driver":["_metadata_","stat_id"]},"ops":[
            {"op":"add-row","table":"entrants.drivers","from":"entrants.drivers.driver_0","set":{"_metadata_":"a"}},
            {"op":"add-row","table":"entrants.drivers","from":"entrants.drivers.driver_0","set":{"_metadata_":"b"}},
            {"op":"set","select":"entrants.drivers.driver_2","field":"stat_id","value":"300"},
            {"op":"add-row","table":"entrants.drivers","from":"entrants.drivers.driver_0","set":{"_metadata_":"c"}},
            {"op":"add-row","table":"entrants.drivers","from":"entrants.drivers.driver_0","set":{"_metadata_":"d"}}]}"#,
        );
        apply_db_ops(&mut db, &ops, &names).unwrap();
        assert_eq!(ids(&db, "entrants.drivers", "driver"), [1, 7, 300, 209, 208, 210]);
    }

    #[test]
    fn a_fields_list_names_extra_keys_and_the_build_reports_what_it_misses() {
        let listed = ["_metadata_", "stat_id"];
        let mut db = modes(["3", "7"]);
        let mut report = DbReport::default();
        apply_db_ops_report(&mut db, &add_mode(""), &catalogue(&listed), &mut report).unwrap();
        assert_eq!(
            db.get_text("modes.mode_2.Com_0.Inner_1.value").as_deref(),
            Some("0.75")
        );
        assert!(
            report
                .lines
                .iter()
                .any(|l| l.contains("copied 9 key(s) the fields list for modes.mode doesn't name")),
            "{:?}",
            report.lines
        );

        let mut db = modes(["3", "7"]);
        let mut report = DbReport::default();
        let legacy = ops(
            r#"{"schema_version":1,"ops":[{"op":"add-row","table":"modes","from":"modes.mode_0",
            "copy":"listed","set":{"_metadata_":"x"}}]}"#,
        );
        apply_db_ops_report(&mut db, &legacy, &catalogue(&listed), &mut report).unwrap();
        assert!(db.get("modes.mode_2.player.driver.preloaded").is_none());
        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
        assert!(
            report.warnings[0].contains(
                "misses 9 key(s) of the source row, not copied: Com_0.Inner_0.value, Com_0.Inner_1.value"
            ),
            "{}",
            report.warnings[0]
        );

        let strict = ops(
            r#"{"schema_version":1,"strict":true,"ops":[{"op":"add-row","table":"modes","from":"modes.mode_0",
            "copy":"listed","set":{"_metadata_":"x"}}]}"#,
        );
        let err = apply_db_ops(&mut modes(["3", "7"]), &strict, &catalogue(&listed)).unwrap_err();
        assert!(err.to_string().ends_with("(strict)"), "{err}");
    }

    #[test]
    fn a_lowered_row_count_in_set_drops_those_rows_from_the_gap_check() {
        let listed = [
            "_metadata_",
            "stat_id",
            "player.driver.preloaded",
            "_Com_instancecount_",
        ];
        let lowered = ops(
            r#"{"schema_version":1,"ops":[{"op":"add-row","table":"modes","from":"modes.mode_0",
            "copy":"listed","set":{"_metadata_":"x","_Com_instancecount_":"0"}}]}"#,
        );
        let mut report = DbReport::default();
        let mut db = modes(["3", "7"]);
        apply_db_ops_report(&mut db, &lowered, &catalogue(&listed), &mut report).unwrap();
        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
        assert!(
            report.warnings[0].contains("misses 2 key(s)")
                && report.warnings[0].ends_with("Loose_0._metadata_, Loose_1._metadata_"),
            "{}",
            report.warnings[0]
        );
    }

    #[test]
    fn a_row_shape_missing_from_the_catalogue_warns_or_fails_when_strict() {
        let mut names = catalogue(&["_metadata_", "stat_id"]);
        names.shapes.remove("modes.mode_N.Com_N");
        let mut report = DbReport::default();
        apply_db_ops_report(&mut modes(["3", "7"]), &add_mode(""), &names, &mut report).unwrap();
        assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
        assert!(
            report.warnings[0].contains("the key catalogue has no entry for modes.mode_N.Com_N"),
            "{}",
            report.warnings[0]
        );
        let err = apply_db_ops(&mut modes(["3", "7"]), &add_mode(r#","strict":true"#), &names).unwrap_err();
        assert!(
            err.to_string().contains("no entry for modes.mode_N.Com_N"),
            "{err}"
        );
    }

    #[test]
    fn row_shapes_number_every_row_segment() {
        assert_eq!(row_shape("a.b_3.c.d_12"), "a.b_N.c.d_N");
        assert_eq!(
            row_shape("canned_events.Per-race DNFs.Per-race DNF_5"),
            "canned_events.Per-race DNFs.Per-race DNF_N"
        );
        assert_eq!(row_shape("x_.y"), "x_.y");
    }
}
