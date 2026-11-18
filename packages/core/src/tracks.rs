use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use swofty_addon::asset::{AssetOp, FileKind, LangOp, OpFile, Origin, TrackOp};
use swofty_addon::manifest::AddonManifest;
use swofty_addon::ops::DbOp;
use swofty_formats::gvdb::{Gvdb, Record, ValueType};
use swofty_formats::hash::fnv1a64;

use crate::launcher::{ProfileTrack, ProfileTracks};
use crate::map::{TrackPackage, Unusable};
use crate::stdlib::deps::Std;
use crate::store::Store;
use crate::{Result, io_err};

const TRACKS: &str = "tracks.Track";
const EVENTS: &str = "schedule.all_track_events.fixed_event";
const ACTIVITIES: &str = "schedule.activities.activity_race_weekend";
const CONTENT_SETS: &str = "content_sets.content_set";
const SEASON_SETS: [&str; 2] = ["2025", "2026"];
const RESOURCE_PREFIX: &str = "swofty_";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlanTrack {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub addon: Option<String>,
}

struct Rows<'a> {
    by_key: HashMap<u64, &'a Record>,
}

impl<'a> Rows<'a> {
    fn new(db: &'a Gvdb) -> Self {
        Self {
            by_key: db.records.iter().map(|r| (r.key, r)).collect(),
        }
    }

    fn text(&self, path: &str) -> Option<String> {
        self.by_key
            .get(&fnv1a64(path))
            .map(|r| r.text().trim().to_owned())
    }

    fn table(&self, path: &str, fields: &[&str]) -> Vec<Vec<Option<String>>> {
        let (container, row) = path.rsplit_once('.').unwrap_or(("", path));
        let count = self
            .text(&format!("{container}._{row}_instancecount_"))
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(0);
        (0..count)
            .map(|i| {
                fields
                    .iter()
                    .map(|f| self.text(&format!("{path}_{i}.{f}")))
                    .collect()
            })
            .collect()
    }
}

pub fn list(db: &Gvdb, added: &[PlanTrack]) -> Vec<ProfileTrack> {
    let rows = Rows::new(db);
    let activities: HashMap<String, String> = rows
        .table(ACTIVITIES, &["_metadata_", "track"])
        .into_iter()
        .filter_map(|r| Some((r[0].clone()?, r[1].clone()?)))
        .collect();
    let mut events: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in rows.table(EVENTS, &["_metadata_", "activity"]) {
        let (Some(name), Some(activity)) = (r[0].clone(), r[1].clone()) else {
            continue;
        };
        let track = activities.get(&activity).cloned().unwrap_or(activity);
        events.entry(track).or_default().push(name);
    }
    let mut playable: Vec<String> = Vec::new();
    for (i, set) in rows.table(CONTENT_SETS, &["_metadata_"]).into_iter().enumerate() {
        if !set[0].as_deref().is_some_and(|l| SEASON_SETS.contains(&l)) {
            continue;
        }
        let lists = rows.table(&format!("{CONTENT_SETS}_{i}.available_tracks"), &["tracks"]);
        for list in lists.into_iter().filter_map(|r| r[0].clone()) {
            playable.extend(
                list.split(',')
                    .map(|c| c.trim().to_owned())
                    .filter(|c| !c.is_empty()),
            );
        }
    }
    let mut out: Vec<ProfileTrack> = Vec::new();
    for r in rows.table(TRACKS, &["_metadata_", "statId", "track_resource"]) {
        let (Some(code), Some(stat_id)) = (r[0].clone(), r[1].as_deref().and_then(|s| s.parse::<u32>().ok()))
        else {
            continue;
        };
        if out.iter().any(|t| t.code == code) {
            continue;
        }
        let package = added.iter().find(|p| p.code == code);
        let modded = package.is_some() || r[2].as_deref() == Some(&format!("{RESOURCE_PREFIX}{code}"));
        out.push(ProfileTrack {
            events: events.remove(&code).unwrap_or_default(),
            stat_id,
            playable: playable.contains(&code),
            modded,
            name: package.and_then(|p| p.name.clone()),
            addon: package.and_then(|p| p.addon.clone()),
            code,
        });
    }
    out
}

#[derive(Deserialize)]
struct PlanListing {
    #[serde(default)]
    buildhash: Option<String>,
    #[serde(default)]
    tracks: Vec<PlanTrack>,
}

type Cached = (PathBuf, Option<SystemTime>, u64, ProfileTracks);

static CACHE: Mutex<Option<Cached>> = Mutex::new(None);

fn stamp(path: &Path) -> (Option<SystemTime>, u64) {
    std::fs::metadata(path).map_or((None, 0), |m| (m.modified().ok(), m.len()))
}

pub fn for_profile(data: &Path, game: &crate::install::Game, profile: &str) -> Result<ProfileTracks> {
    let plan = crate::cache::latest_plan(&data.join("cache/overlay").join(profile));
    let listing: Option<PlanListing> = plan.as_deref().and_then(|p| crate::read_json(p).ok());
    let built_db = plan
        .as_deref()
        .and_then(Path::parent)
        .map(|d| d.join(crate::build::GVDB))
        .filter(|p| p.is_file());
    let db_path = built_db
        .clone()
        .unwrap_or_else(|| crate::build::original(data, game, crate::build::GVDB));
    let (modified, len) = stamp(&db_path);
    let buildhash = listing.as_ref().and_then(|l| l.buildhash.clone());
    let added = listing.map(|l| l.tracks).unwrap_or_default();
    {
        let cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((p, m, l, tracks)) = cache.as_ref()
            && *p == db_path
            && *m == modified
            && *l == len
            && tracks.buildhash == buildhash
        {
            return Ok(tracks.clone());
        }
    }
    let bytes = std::fs::read(&db_path).map_err(io_err(&db_path))?;
    let db = Gvdb::parse(&bytes)?;
    let tracks = ProfileTracks {
        built: plan.is_some(),
        buildhash,
        tracks: list(&db, &added),
    };
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = Some((db_path, modified, len, tracks.clone()));
    Ok(tracks)
}

/// A data addon that is only tracks: its ops add one or more tracks and give
/// them names (`lang.set`, `lang.default`, `db.set` on a `TrackNameData` field), and it has no
/// code, screens, files, settings, launcher pages or other edits. The launcher
/// lists these on the Tracks page instead of the Addons page. An op file that
/// doesn't read makes the addon an ordinary one.
pub fn is_track_addon(dir: &Path, m: &AddonManifest) -> bool {
    let o = &m.operations;
    let other = !o.database.is_empty()
        || !o.erp.is_empty()
        || !o.ui.is_empty()
        || !o.files.is_empty()
        || !o.native.is_empty()
        || !o.screens.is_empty();
    if other || o.assets.is_empty() || m.launcher.is_some() || !m.options.is_empty() || m.saves.affects {
        return false;
    }
    let mut tracks = 0;
    for rel in &o.assets {
        let Some(doc) = std::fs::read(dir.join(rel))
            .ok()
            .and_then(|d| serde_json::from_slice(&d).ok())
        else {
            return false;
        };
        let Ok(file) = OpFile::from_value(doc, FileKind::Assets) else {
            return false;
        };
        for op in file.ops {
            match op {
                AssetOp::Track(TrackOp::Add { .. }) => tracks += 1,
                AssetOp::Lang(LangOp::Set { .. } | LangOp::Default { .. }) | AssetOp::Warn { .. } => {}
                AssetOp::Db(DbOp::Set { field, .. }) if field.starts_with("TrackNameData") => {}
                _ => return false,
            }
        }
    }
    tracks > 0
}

// The sources of the `track.add` ops in the op files of the addon in `dir`.
fn added_sources(dir: &Path, m: &AddonManifest) -> Vec<swofty_addon::asset::Source> {
    let mut out = Vec::new();
    for rel in &m.operations.assets {
        let Some(file) = std::fs::read(dir.join(rel))
            .ok()
            .and_then(|d| serde_json::from_slice(&d).ok())
            .and_then(|doc| OpFile::from_value(doc, FileKind::Assets).ok())
        else {
            continue;
        };
        for op in file.ops {
            if let AssetOp::Track(TrackOp::Add { source }) = op {
                out.push(source);
            }
        }
    }
    out
}

// The package folders an addon's `track.add` ops in its op files name.
fn added_dirs(store: &Store, id: &str, m: &AddonManifest) -> Vec<PathBuf> {
    added_sources(&store.addons_dir().join(id), m)
        .into_iter()
        .filter_map(|source| {
            source
                .resolve(&crate::build::source_dir(store, id, source.root))
                .ok()
        })
        .collect()
}

/// The `author_name` of the track packages inside the addon folder `dir` that its `track.add`
/// operations add, when they give one and the same one.
pub fn package_author(dir: &Path, m: &AddonManifest) -> Option<String> {
    let mut authors: Vec<String> = added_sources(dir, m)
        .into_iter()
        .filter(|source| source.root == swofty_addon::asset::Root::Addon)
        .filter_map(|source| source.resolve(dir).ok())
        .map(|d| {
            std::fs::read(d.join(crate::map::MANIFEST_FILE))
                .ok()
                .and_then(|data| serde_json::from_slice::<serde_json::Value>(&data).ok())
                .and_then(|doc| Some(doc.get("author_name")?.as_str()?.trim().to_owned()))
                .filter(|a| !a.is_empty())
        })
        .collect::<Option<Vec<String>>>()?;
    authors.dedup();
    match authors.as_slice() {
        [one] => Some(one.clone()),
        _ => None,
    }
}

/// The tracks an addon's `track.add` operations add, as the core lists them
/// (`<addon>/<code>`), with the venue each one's name matches.
pub fn older_ids(store: &Store, id: &str, m: &AddonManifest) -> Vec<(String, Option<String>)> {
    added_dirs(store, id, m)
        .iter()
        .filter_map(|d| crate::map::read_package(d).ok())
        .map(|pkg| {
            let venue = pkg
                .track_name
                .as_deref()
                .and_then(|n| Venues::shipped().by_name(n));
            (format!("{id}/{}", pkg.track_code), venue)
        })
        .collect()
}

/// The tracks installed addons add with `track.add`, named from their
/// packages, by addon id; the first addon to add a track keeps it. Ops or
/// packages that don't read are skipped.
pub fn installed(store: &Store) -> Vec<PlanTrack> {
    let mut out: Vec<PlanTrack> = Vec::new();
    for (id, m) in store.addons().unwrap_or_default() {
        for dir in added_dirs(store, &id, &m) {
            let Ok(pkg) = crate::map::read_package(&dir) else {
                continue;
            };
            if out.iter().any(|t| t.code == pkg.track_code) {
                continue;
            }
            out.push(PlanTrack {
                name: pkg.track_name.filter(|n| !n.trim().is_empty()),
                addon: Some(id.clone()),
                code: pkg.track_code,
            });
        }
    }
    out
}

/// The track packages an addon's op files add that builds leave out.
pub fn unusable(store: &Store, id: &str, m: &AddonManifest) -> Vec<Unusable> {
    added_dirs(store, id, m)
        .iter()
        .filter_map(|d| crate::map::read_package(d).err())
        .collect()
}

/// The registry of track records (`registry/track.json`, `tracks::tracks`).
pub const TRACK_REGISTRY: &str = "track";
/// `$DATA/registry/older-tracks.json`: the tracks `track.add` adds for addons with no track record.
pub const OLDER_FILE: &str = "older-tracks.json";
pub const OLDER_VERSION: u32 = 1;
const VENUES_JSON: &str = include_str!("../../sdk/std/data/venues.json");

/// A `track` record an addon's data file adds under its own id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordTrack {
    pub id: String,
    pub code: String,
    pub name: Option<String>,
    pub base: Option<String>,
    pub slot: Option<String>,
    pub files: Option<String>,
}

/// The `track` records the addon's data files (`data_files`, relative to `dir`) add, in file
/// then key order. A record without a `code` is skipped.
pub fn record_tracks(dir: &Path, addon: &str, data_files: &[String]) -> Vec<RecordTrack> {
    let mut out = Vec::new();
    for rel in data_files {
        let Some(doc) = std::fs::read_to_string(dir.join(rel))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        else {
            continue;
        };
        let mut records: Vec<(&String, &serde_json::Value)> = doc
            .get(TRACK_REGISTRY)
            .and_then(|t| t.as_object())
            .into_iter()
            .flatten()
            .collect();
        records.sort_by_key(|(k, _)| *k);
        for (key, value) in records {
            if key.contains('/') || key.starts_with('$') {
                continue;
            }
            let s = |field: &str| value.get(field).and_then(|v| v.as_str()).map(str::to_owned);
            let Some(code) = s("code") else {
                continue;
            };
            out.push(RecordTrack {
                id: format!("{addon}/{key}"),
                code,
                name: s("name"),
                base: s("base"),
                slot: s("slot"),
                files: s("files"),
            });
        }
    }
    out
}

fn stock_code(id: &str) -> Option<&str> {
    id.strip_prefix("game/").filter(|c| !c.is_empty())
}

/// The package a track record's `files` folder holds, for the build to place: its
/// `manifest.json` when it has one, else the record's code, name, base and slot with
/// `<code>.erp` and the folder's `mipmaps/`.
pub fn record_package(
    addon_dir: &Path,
    t: &RecordTrack,
) -> std::result::Result<(PathBuf, TrackPackage), Unusable> {
    let unusable = |why: String| Unusable {
        code: Some(t.code.clone()),
        name: Some(t.name.clone().unwrap_or_else(|| t.code.clone())),
        why,
        fix: None,
    };
    let Some(rel) = t.files.as_deref() else {
        return Err(unusable(format!(
            "its track record {} names no files folder",
            t.id
        )));
    };
    let Some(inside) = crate::stdlib::paths::inside(rel) else {
        return Err(unusable(format!(
            "its track record {} names {rel}, which leaves the addon's folder",
            t.id
        )));
    };
    let dir = addon_dir.join(inside.trim_end_matches('/'));
    if dir.join(crate::map::MANIFEST_FILE).is_file() {
        let pkg = crate::map::read_package(&dir)?;
        if pkg.track_code != t.code {
            return Err(unusable(format!(
                "its track record {} has code {}, and {rel}{} is for {}",
                t.id,
                t.code,
                crate::map::MANIFEST_FILE,
                pkg.track_code
            )));
        }
        return Ok((dir, pkg));
    }
    let base = t.base.as_deref().and_then(stock_code).map(str::to_owned);
    let slot = t.slot.as_deref().and_then(stock_code).map(str::to_owned);
    if base.is_none() && slot.is_none() {
        return Err(unusable(format!(
            "its track record {} names no stock base track",
            t.id
        )));
    }
    let erp = format!("{}.erp", t.code);
    if !dir.join(&erp).is_file() {
        return Err(unusable(format!("its files folder {rel} has no {erp}")));
    }
    let mips = dir.join("mipmaps").is_dir();
    let pkg = TrackPackage {
        track_code: t.code.clone(),
        track_name: t.name.clone(),
        version: None,
        author_name: None,
        erp,
        activation: crate::map::Activation {
            copy_from_track: base,
            copy_slot: slot,
            ..Default::default()
        },
        ers: Default::default(),
        copy_directories: match mips {
            true => vec![crate::map::CopyDirectory {
                source: "mipmaps/".into(),
                destination: format!("asset_groups/environment_package/tracks/{}/mipmaps/", t.code),
            }],
            false => Vec::new(),
        },
        ui: None,
        extra: Default::default(),
    };
    pkg.validate().map_err(|e| unusable(e.to_string()))?;
    Ok((dir, pkg))
}

/// The addons in `order` with a `track` record, in a data file or their lock: `track.add` only
/// places their files, and the core adds the rows.
pub fn with_records(order: &[String], std16: &BTreeMap<String, Std>) -> BTreeSet<String> {
    order
        .iter()
        .filter(|id| std16.get(*id).is_some_and(|s| s.has_records(TRACK_REGISTRY)))
        .cloned()
        .collect()
}

pub type BuildTrack = (PathBuf, TrackPackage, Origin);

/// The packages of the track records with a `files` folder that the addons in `order` add and
/// no `track.add` of theirs places, in load order, and the ones a build leaves out.
pub fn record_packages(
    store: &Store,
    order: &[String],
    std16: &BTreeMap<String, Std>,
    added: &[BuildTrack],
) -> (Vec<BuildTrack>, Vec<(Origin, Unusable)>) {
    let (mut out, mut left_out): (Vec<BuildTrack>, Vec<(Origin, Unusable)>) = (Vec::new(), Vec::new());
    for id in order {
        let Some(s) = std16.get(id).filter(|s| !s.data_files.is_empty()) else {
            continue;
        };
        let dir = store.addons_dir().join(id);
        let origin = Origin {
            addon: id.clone(),
            item: None,
        };
        for t in record_tracks(&dir, id, &s.data_files) {
            let placed = |list: &[BuildTrack]| {
                list.iter()
                    .find(|(_, p, _)| p.track_code == t.code)
                    .map(|(_, _, o)| o.addon.clone())
            };
            match placed(added).or_else(|| placed(&out)) {
                Some(by) if by == *id => continue,
                Some(by) => {
                    left_out.push((
                        origin.clone(),
                        Unusable {
                            code: Some(t.code.clone()),
                            name: Some(t.name.clone().unwrap_or_else(|| t.code.clone())),
                            why: format!("{by} adds a track with the code {} too", t.code),
                            fix: None,
                        },
                    ));
                    continue;
                }
                None => {}
            }
            if t.files.is_none() {
                continue;
            }
            match record_package(&dir, &t) {
                Ok((d, pkg)) => out.push((d, pkg, origin.clone())),
                Err(why) => left_out.push((origin.clone(), why)),
            }
        }
    }
    (out, left_out)
}

/// Leaves the folders of the build's track packages out of their addons' files for the index:
/// the build places a track's archive, mipmaps and menu art itself.
pub fn keep_track_folders(store: &Store, tracks: &[BuildTrack], std16: &mut BTreeMap<String, Std>) {
    for (dir, _, origin) in tracks {
        let Some(s) = std16.get_mut(&origin.addon) else {
            continue;
        };
        let Ok(rel) = dir.strip_prefix(store.addons_dir().join(&origin.addon)) else {
            continue;
        };
        let prefix = format!(
            "{}/",
            rel.to_string_lossy().replace('\\', "/").trim_end_matches('/')
        );
        if prefix == "/" {
            continue;
        }
        s.files.files.retain(|f| !f.starts_with(&prefix));
    }
}

/// The most tracks the game's track lists hold. F1 25 gives each listed track one bit of a 64-bit set when it
/// loads its options at boot (`cmp eax, 0x40` then `bts` at 0x142ae87c7 on Steam) and throws
/// `Ne_Std_ThrowInvalidBitsetPosition` for the 65th, which leaves the screen black after the EA logo.
pub const TRACK_LIST_ROOM: usize = 64;
/// The track the game leaves out of those lists by name.
const UNLISTED_TRACK: &str = "madrid";
/// The fewest added tracks that can fill the lists: F1 25 ships 43 track rows.
pub const FILL_FROM: usize = TRACK_LIST_ROOM - 43 + 1;

/// The tracks a database puts in the game's track lists: its tracks.Track rows with a TrackData row in any
/// dataset, but madrid.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListedTracks {
    rows: BTreeSet<String>,
    with_data: BTreeSet<String>,
}

impl ListedTracks {
    pub fn of(db: &Gvdb) -> Self {
        let at: HashMap<u64, usize> = db.records.iter().enumerate().map(|(i, r)| (r.key, i)).collect();
        let text = |path: &str| {
            at.get(&fnv1a64(path))
                .map(|&i| db.records[i].text().trim().to_owned())
        };
        let count = |table: &str| {
            let (container, row) = table.rsplit_once('.').expect("a table path");
            text(&swofty_formats::gvdb::instance_count_path(container, row))
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(0)
        };
        let rows = (0..count(TRACKS))
            .filter_map(|i| text(&format!("{TRACKS}_{i}._metadata_")))
            .collect();
        let mut with_data = BTreeSet::new();
        for s in 0..count(TRACK_DATA) {
            let table = format!("{TRACK_DATA}_{s}.TrackData");
            with_data.extend((0..count(&table)).filter_map(|i| text(&format!("{table}_{i}.track"))));
        }
        Self { rows, with_data }
    }

    /// How many of the game's own tracks the lists hold.
    pub fn stock(&self) -> usize {
        self.rows.iter().filter(|c| self.lists(c)).count()
    }

    fn lists(&self, code: &str) -> bool {
        code != UNLISTED_TRACK && self.rows.contains(code) && self.with_data.contains(code)
    }

    /// Whether activating `pkg` adds a track to the lists: activation copies its base track's TrackData rows to
    /// it, so a track whose base has none stays out of them, and a code the lists hold already adds nothing.
    fn adds(&self, pkg: &TrackPackage) -> bool {
        let code = pkg.track_code.as_str();
        code != UNLISTED_TRACK
            && !self.lists(code)
            && pkg
                .activation
                .base_track()
                .is_some_and(|base| self.with_data.contains(base))
    }
}

/// Whether `added` tracks can fill the game's track lists, so a build has to read the database to know.
pub fn may_fill_track_lists(added: usize) -> bool {
    added >= FILL_FROM
}

/// The build's tracks the game's track lists have room for, and the ones left out: the tracks of `recorded`
/// addons first, since the core adds their rows in game whatever the build does, then the others in load
/// order until the lists are full. A track that adds nothing to the lists is always kept.
pub fn fit_track_lists(
    listed: &ListedTracks,
    tracks: Vec<BuildTrack>,
    recorded: &BTreeSet<String>,
) -> (Vec<BuildTrack>, Vec<(BuildTrack, Unusable)>) {
    let room = TRACK_LIST_ROOM.saturating_sub(listed.stock());
    let mut used = tracks
        .iter()
        .filter(|(_, p, o)| recorded.contains(&o.addon) && listed.adds(p))
        .count();
    let mut kept = Vec::with_capacity(tracks.len());
    let mut left_out = Vec::new();
    for t in tracks {
        let (_, pkg, origin) = &t;
        if recorded.contains(&origin.addon) || !listed.adds(pkg) {
            kept.push(t);
            continue;
        }
        if used < room {
            used += 1;
            kept.push(t);
            continue;
        }
        let why = Unusable {
            code: Some(pkg.track_code.clone()),
            name: Some(pkg.display_name().to_owned()),
            why: track_lists_full(listed),
            fix: Some("Turn some track addons off to make room for it".to_owned()),
        };
        left_out.push((t, why));
    }
    (kept, left_out)
}

fn track_lists_full(listed: &ListedTracks) -> String {
    let stock = listed.stock();
    format!(
        "the game has room for {TRACK_LIST_ROOM} tracks, {stock} of its own and {} added ones, and the track \
         addons before it in the load order fill them",
        TRACK_LIST_ROOM.saturating_sub(stock)
    )
}

/// The build's notice for the tracks [`fit_track_lists`] leaves out, given by name.
pub fn track_lists_full_warning(listed: &ListedTracks, left_out: &[String]) -> Option<String> {
    let shown = match left_out.len() {
        0 => return None,
        n if n <= 8 => left_out.join(", "),
        n => format!("{} and {} more", left_out[..8].join(", "), n - 8),
    };
    let stock = listed.stock();
    Some(format!(
        "Too many tracks: {} track(s) are left out of this build because the game has room for {TRACK_LIST_ROOM} \
         tracks, {stock} of its own and {} added ones, and more stop it at a black screen after the EA logo: \
         {shown}. Turn some track addons off to play them",
        left_out.len(),
        TRACK_LIST_ROOM.saturating_sub(stock)
    ))
}

/// The track addons a build leaves out because the game's track lists are full, for the launcher to show.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackLimit {
    pub room: usize,
    pub stock: usize,
    /// The display name of each track left out, in load order.
    pub names: Vec<String>,
    /// The addons that lose a track, each once.
    pub addons: Vec<String>,
}

impl TrackLimit {
    pub fn new(listed: &ListedTracks, names: Vec<String>, addons: Vec<String>) -> Option<Self> {
        (!names.is_empty()).then(|| Self {
            room: TRACK_LIST_ROOM,
            stock: listed.stock(),
            names,
            addons: addons.into_iter().collect::<BTreeSet<_>>().into_iter().collect(),
        })
    }
}

const LIMIT_FILE: &str = "state/track-limit.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct LimitRecord {
    profile: String,
    addons: Vec<String>,
}

/// Remembers which addons the last check of `profile` found left out for the full track lists.
pub fn record_track_limit(data: &Path, profile: &str, limit: Option<&TrackLimit>) -> Result<()> {
    let path = data.join(LIMIT_FILE);
    let addons = limit.map(|l| l.addons.clone()).unwrap_or_default();
    if addons.is_empty() && !path.is_file() {
        return Ok(());
    }
    crate::write_json(
        &path,
        &LimitRecord {
            profile: profile.to_owned(),
            addons,
        },
    )
}

/// The addons the last check of `profile` found left out for the full track lists.
pub fn recorded_track_limit(data: &Path, profile: &str) -> BTreeSet<String> {
    crate::read_json::<LimitRecord>(&data.join(LIMIT_FILE))
        .ok()
        .filter(|r| r.profile == profile)
        .map(|r| r.addons.into_iter().collect())
        .unwrap_or_default()
}

/// How many tracks the addons of `order` add with `track.add`, to tell whether a build can fill the lists.
pub fn added_count(store: &Store, order: &[String], addons: &BTreeMap<String, AddonManifest>) -> usize {
    order
        .iter()
        .filter_map(|id| addons.get(id).map(|m| added_dirs(store, id, m).len()))
        .sum()
}

/// Writes `$DATA/registry/older-tracks.json` for the tracks `track.add` added for addons with no
/// track record; `shipped` and `built` are the game's database and the build's.
pub fn write_older(
    data: &Path,
    profile: &str,
    added: &[(&str, &TrackPackage)],
    shipped: &Path,
    built: &Path,
) -> Result<()> {
    let path = crate::stdlib::reports::path(data, OLDER_FILE);
    let file = match added.is_empty() {
        true => OlderTracks {
            version: OLDER_VERSION,
            profile: profile.to_owned(),
            tracks: Vec::new(),
        },
        false => {
            let read = |p: &Path| -> Result<Gvdb> { Ok(Gvdb::parse(&std::fs::read(p).map_err(io_err(p))?)?) };
            older_tracks(profile, added, &read(shipped)?, &read(built)?, Venues::shipped())
        }
    };
    if crate::read_json::<OlderTracks>(&path).is_ok_and(|f| f == file) {
        return Ok(());
    }
    crate::write_json(&path, &file)
}

/// The tracks the last build recorded in `older-tracks.json`, for `profile`: their stat ids seed the ids the store keeps.
pub fn read_older(data: &Path, profile: &str) -> Vec<OlderTrack> {
    let path = crate::stdlib::reports::path(data, OLDER_FILE);
    crate::read_json::<OlderTracks>(&path)
        .ok()
        .filter(|f| f.profile == profile)
        .map(|f| f.tracks)
        .unwrap_or_default()
}

/// One venue of `packages/sdk/std/data/venues.json`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Venue {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub stock: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Venues {
    #[serde(default)]
    pub venues: BTreeMap<String, Venue>,
}

/// Lower case, every run of characters other than `a-z0-9` one space, trimmed.
pub fn normalise(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.to_lowercase().chars() {
        match c.is_ascii_lowercase() || c.is_ascii_digit() {
            true => out.push(c),
            false if !out.ends_with(' ') => out.push(' '),
            false => {}
        }
    }
    out.trim().to_owned()
}

impl Venues {
    pub fn parse(text: &str) -> Option<Self> {
        serde_json::from_str(text).ok()
    }

    /// The venues the launcher ships (`packages/sdk/std/data/venues.json`).
    pub fn shipped() -> &'static Self {
        static PARSED: std::sync::OnceLock<Venues> = std::sync::OnceLock::new();
        PARSED.get_or_init(|| Self::parse(VENUES_JSON).unwrap_or_default())
    }

    /// `venue/<key>` of the one venue whose name or an alias equals `name` once normalised;
    /// `None` when none does or more than one does.
    pub fn by_name(&self, name: &str) -> Option<String> {
        let want = normalise(name);
        if want.is_empty() {
            return None;
        }
        let mut found = self.venues.iter().filter(|(_, v)| {
            std::iter::once(&v.name)
                .chain(&v.aliases)
                .any(|n| normalise(n) == want)
        });
        let (key, _) = found.next()?;
        found.next().is_none().then(|| format!("venue/{key}"))
    }

    /// Whether a stock track with assets provides `id` (`venue/<key>`).
    pub fn stock_provides(&self, id: &str) -> bool {
        id.strip_prefix("venue/")
            .and_then(|k| self.venues.get(k))
            .is_some_and(|v| v.stock.as_deref().is_some_and(|s| !s.is_empty()))
    }
}

/// One entry of `older-tracks.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OlderTrack {
    pub addon: String,
    pub code: String,
    pub name: String,
    pub base: String,
    pub slot: Option<String>,
    pub stat_id: Option<u32>,
    pub venue: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OlderTracks {
    pub version: u32,
    pub profile: String,
    pub tracks: Vec<OlderTrack>,
}

fn label_rows(db: &Gvdb) -> Vec<(String, Option<u32>)> {
    Rows::new(db)
        .table(TRACKS, &["_metadata_", "statId"])
        .into_iter()
        .filter_map(|r| Some((r[0].clone()?, r[1].as_deref().and_then(|s| s.parse().ok()))))
        .collect()
}

/// `older-tracks.json` for the tracks `track.add` added in a build, by addon id in the build's
/// order: `shipped` is the game's own database, `built` the build's. `slot` is the stock row a
/// track fills (`game/<code>`: a row the game ships with the track's code), else null; `base` is
/// the track its rows were cloned from.
pub fn older_tracks(
    profile: &str,
    added: &[(&str, &TrackPackage)],
    shipped: &Gvdb,
    built: &Gvdb,
    venues: &Venues,
) -> OlderTracks {
    let stock: Vec<String> = label_rows(shipped).into_iter().map(|(c, _)| c).collect();
    let ids = label_rows(built);
    let tracks = added
        .iter()
        .map(|(addon, pkg)| {
            let code = pkg.track_code.clone();
            let name = pkg.display_name().trim().to_owned();
            OlderTrack {
                addon: (*addon).to_owned(),
                venue: pkg.track_name.as_deref().and_then(|n| venues.by_name(n)),
                base: format!("game/{}", pkg.activation.base_track().unwrap_or(&code)),
                slot: stock.contains(&code).then(|| format!("game/{code}")),
                stat_id: ids.iter().find(|(c, _)| *c == code).and_then(|(_, id)| *id),
                name: match name.is_empty() {
                    true => code.clone(),
                    false => name,
                },
                code,
            }
        })
        .collect();
    OlderTracks {
        version: OLDER_VERSION,
        profile: profile.to_owned(),
        tracks,
    }
}

pub fn left_out_line(addon: &str, id: &str, why: &Unusable) -> String {
    match addon == id {
        true => format!("{id}: {}", why.text()),
        false => format!("{addon} ({id}): {}", why.text()),
    }
}

/// A [`left_out_line`] for each track package `m`'s op files add that builds leave out.
pub fn left_out_lines(store: &Store, m: &AddonManifest) -> Vec<String> {
    unusable(store, &m.id, m)
        .iter()
        .map(|u| left_out_line(&m.name, &m.id, u))
        .collect()
}

/// The track a weekend with no stock track to go back to races at when its own track is left out, as the
/// game's Grand Prix does when its remembered track has no record: melbourne, else the first track the
/// build holds.
const FALLBACK_TRACK: &str = "melbourne";

/// Puts each race weekend at a track a build left out back at its stock
/// track, and says so; a weekend the game doesn't ship, an addon's own row,
/// races at [`FALLBACK_TRACK`] instead. Names the weekends the build holds no
/// track to put anywhere.
pub(crate) fn restore_weekends(
    db: &mut Gvdb,
    shipped: &Gvdb,
    left_out: &[&str],
) -> std::result::Result<Vec<String>, Vec<String>> {
    let text = |db: &Gvdb, path: &str| db.get_text(path).map(|t| t.trim().to_owned());
    let count = |db: &Gvdb, table: &str| {
        let (container, row) = table.rsplit_once('.').expect("a table path");
        db.instance_count(container, row).unwrap_or(0)
    };
    let mut held: Option<Vec<String>> = None;
    let mut out = Vec::new();
    let mut stuck = Vec::new();
    for i in 0..count(db, ACTIVITIES) {
        let path = format!("{ACTIVITIES}_{i}.track");
        let Some(code) = text(db, &path).filter(|c| left_out.contains(&c.as_str())) else {
            continue;
        };
        let held = held.get_or_insert_with(|| {
            (0..count(db, TRACKS))
                .filter_map(|t| text(db, &format!("{TRACKS}_{t}._metadata_")))
                .collect()
        });
        if held.contains(&code) {
            continue;
        }
        let weekend = text(db, &format!("{ACTIVITIES}_{i}._metadata_")).unwrap_or_else(|| i.to_string());
        let stock = shipped
            .get(&path)
            .filter(|r| !left_out.contains(&r.text().trim()) && held.iter().any(|h| h == r.text().trim()))
            .cloned();
        match stock {
            Some(r) => {
                let to = r.text().trim().to_owned();
                if db.set(&path, r.value, Some(r.tag)).is_ok() {
                    out.push(format!(
                        "race weekend {weekend}: races at {to} again; track {code} is left out of this build"
                    ));
                }
            }
            None => {
                let usable = |h: &&String| !left_out.contains(&h.as_str());
                let to = held
                    .iter()
                    .find(|h| *h == FALLBACK_TRACK)
                    .or_else(|| {
                        held.iter()
                            .filter(usable)
                            .find(|h| !DORMANT.contains(&h.as_str()))
                    })
                    .or_else(|| held.iter().find(usable))
                    .cloned();
                match to {
                    Some(to) if db.set(&path, to.clone().into_bytes(), None).is_ok() => out.push(format!(
                        "race weekend {weekend}: races at {to} instead; track {code} is left out of this build \
                         and the game won't start with a race weekend at it"
                    )),
                    _ => stuck.push(format!(
                        "race weekend {weekend} races at {code}, which is left out of this build, and the build \
                         holds no other track to move it to; the game won't start like this"
                    )),
                }
            }
        }
    }
    match stuck.is_empty() {
        true => Ok(out),
        false => Err(stuck),
    }
}

const SORTED_ORDER: &str = "track_data.sorted_track_order.sorted_track_order";

/// Takes each `left_out` track out of the game's sorted track orders, the lists its menus read tracks from,
/// and names the orders it was in.
pub(crate) fn drop_left_out_orders(db: &mut Gvdb, left_out: &[&str]) -> Vec<String> {
    let count = |db: &Gvdb, container: &str, row: &str| db.instance_count(container, row).unwrap_or(0);
    let mut out = Vec::new();
    for o in 0..count(db, "track_data.sorted_track_order", "sorted_track_order") {
        let order = format!("{SORTED_ORDER}_{o}");
        let rows: Vec<(Option<Record>, Option<Record>)> = (0..count(db, &order, "TrackRef"))
            .map(|r| {
                (
                    db.get(&format!("{order}.TrackRef_{r}._metadata_")).cloned(),
                    db.get(&format!("{order}.TrackRef_{r}.track")).cloned(),
                )
            })
            .collect();
        let held = |(_, t): &&(Option<Record>, Option<Record>)| {
            !t.as_ref().is_some_and(|t| left_out.contains(&t.text().trim()))
        };
        let kept: Vec<_> = rows.iter().filter(held).collect();
        if kept.len() == rows.len() {
            continue;
        }
        let gone: Vec<String> = rows
            .iter()
            .filter(|r| !held(r))
            .filter_map(|(_, t)| Some(t.as_ref()?.text().trim().to_owned()))
            .collect();
        for (r, (meta, track)) in kept.iter().enumerate() {
            for (field, rec) in [("_metadata_", meta), ("track", track)] {
                if let Some(rec) = rec {
                    let _ = db.set(
                        &format!("{order}.TrackRef_{r}.{field}"),
                        rec.value.clone(),
                        Some(rec.tag),
                    );
                }
            }
        }
        for r in kept.len()..rows.len() {
            db.remove(&format!("{order}.TrackRef_{r}._metadata_"));
            db.remove(&format!("{order}.TrackRef_{r}.track"));
        }
        let _ = db.set(
            &swofty_formats::gvdb::instance_count_path(&order, "TrackRef"),
            kept.len().to_string().into_bytes(),
            None,
        );
        let label = db
            .get_text(&format!("{order}._metadata_"))
            .map_or_else(|| o.to_string(), |l| l.trim().to_owned());
        out.push(format!(
            "sorted track order {label} listed {}, which is left out of this build; the build takes it out",
            gone.join(", ")
        ));
    }
    out
}

const TRACK_DATA: &str = "track_data.game_setup.TrackDataSet";
const GAME_MODES: &str = "game_setup.game_setup_modes.game_setup_mode";

/// A track a build's database names that its tracks table doesn't hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MissingTrack {
    /// The record that names it.
    pub key: u64,
    /// The row that names it, such as `race weekend bahrain`.
    pub at: String,
    pub track: String,
    pub fix: MissingFix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MissingFix {
    /// The record has its shipped track again.
    Stock(String),
    /// The track is out of the record's list.
    Dropped,
    /// The record is as it was: the game ships no track there the build holds.
    Kept,
}

impl MissingTrack {
    /// The build's warning, with the addon that set the record (`by`) and the addon
    /// that adds the track and is off (`adds`) when they are known.
    pub(crate) fn line(&self, by: Option<&str>, adds: Option<&str>) -> String {
        let (at, track) = (&self.at, &self.track);
        let head = match (by, &self.fix) {
            (Some(by), MissingFix::Dropped) => format!("{by} adds {track} to {at}"),
            (Some(by), _) => format!("{by} sets {at} to {track}"),
            (None, MissingFix::Dropped) => format!("{at} lists {track}"),
            (None, _) => format!("{at} is set to {track}"),
        };
        let fix = match adds {
            Some(addon) => format!("which is off: turn {addon} on"),
            None => "which isn't in this build".to_owned(),
        };
        match &self.fix {
            MissingFix::Stock(stock) => format!("{head}, {fix}; until then this build puts back {stock}"),
            MissingFix::Dropped => format!("{head}, {fix}; until then this build leaves it out"),
            MissingFix::Kept if adds.is_some() => {
                format!("{head}, {fix} or change it in DB Editor; the game won't start like this")
            }
            MissingFix::Kept => {
                format!("{head}, {fix}: change it in DB Editor; the game won't start like this")
            }
        }
    }
}

/// Points each record that names a track the build doesn't hold at its shipped track
/// again, or takes the track out of a record's list; a record the game ships with no
/// track the build holds is only named. The build holds its tracks table's tracks and
/// `in_game`, the tracks whose rows the game adds. A track the game ships the record
/// with is left as it is, and race weekends at a `left_out` track are
/// [`restore_weekends`]'s.
pub(crate) fn keep_track_refs(
    db: &mut Gvdb,
    shipped: &Gvdb,
    in_game: &[&str],
    left_out: &[&str],
) -> Vec<MissingTrack> {
    let at: HashMap<u64, usize> = db.records.iter().enumerate().map(|(i, r)| (r.key, i)).collect();
    let stock: HashMap<u64, &Record> = shipped.records.iter().map(|r| (r.key, r)).collect();
    let text = |db: &Gvdb, path: &str| {
        at.get(&fnv1a64(path))
            .map(|&i| db.records[i].text().trim().to_owned())
    };
    let count = |db: &Gvdb, table: &str| {
        let (container, row) = table.rsplit_once('.').expect("a table path");
        text(db, &swofty_formats::gvdb::instance_count_path(container, row))
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(0)
    };
    let label = |db: &Gvdb, table: &str, i: usize| {
        text(db, &format!("{table}_{i}._metadata_")).unwrap_or_else(|| i.to_string())
    };
    let mut have: HashSet<String> = (0..count(db, TRACKS))
        .filter_map(|i| text(db, &format!("{TRACKS}_{i}._metadata_")))
        .collect();
    if have.is_empty() {
        return Vec::new();
    }
    have.extend(in_game.iter().map(|c| (*c).to_owned()));
    let mut refs: Vec<(String, String, bool)> = Vec::new();
    for i in 0..count(db, ACTIVITIES) {
        let path = format!("{ACTIVITIES}_{i}.track");
        if text(db, &path).is_some_and(|c| left_out.contains(&c.as_str())) {
            continue;
        }
        refs.push((path, format!("race weekend {}", label(db, ACTIVITIES, i)), false));
    }
    for s in 0..count(db, TRACK_DATA) {
        let rows = format!("{TRACK_DATA}_{s}.TrackData");
        let set = label(db, TRACK_DATA, s);
        for i in 0..count(db, &rows) {
            let at = format!("track settings {set}/{}", label(db, &rows, i));
            refs.push((format!("{rows}_{i}.track"), at, false));
        }
    }
    for s in 0..count(db, CONTENT_SETS) {
        let rows = format!("{CONTENT_SETS}_{s}.available_tracks");
        let at = format!("content set {}", label(db, CONTENT_SETS, s));
        for i in 0..count(db, &rows) {
            refs.push((format!("{rows}_{i}.tracks"), at.clone(), true));
        }
    }
    for i in 0..count(db, GAME_MODES) {
        let at = format!("game mode {}", label(db, GAME_MODES, i));
        refs.push((format!("{GAME_MODES}_{i}.restricted_tracks"), at, true));
    }
    let mut out = Vec::new();
    for (path, row, list) in refs {
        let key = fnv1a64(&path);
        let Some(&i) = at.get(&key) else {
            continue;
        };
        let now = db.records[i].text().trim().to_owned();
        let shipped_text = stock
            .get(&key)
            .map(|r| r.text().trim().to_owned())
            .unwrap_or_default();
        if !list {
            if now.is_empty() || have.contains(&now) || now == shipped_text {
                continue;
            }
            let fix = match stock.get(&key).filter(|r| have.contains(r.text().trim())) {
                Some(r) => {
                    db.records[i].value = r.value.clone();
                    db.records[i].tag = r.tag;
                    MissingFix::Stock(r.text().trim().to_owned())
                }
                None => MissingFix::Kept,
            };
            out.push(MissingTrack {
                key,
                at: row,
                track: now,
                fix,
            });
            continue;
        }
        let codes: Vec<&str> = now.split(',').map(str::trim).filter(|c| !c.is_empty()).collect();
        let (kept, gone): (Vec<&str>, Vec<&str>) = codes
            .into_iter()
            .partition(|c| have.contains(*c) || shipped_text.split(',').any(|s| s.trim() == *c));
        if gone.is_empty() {
            continue;
        }
        db.records[i].value = kept.join(",").into_bytes();
        out.extend(gone.into_iter().map(|track| MissingTrack {
            key,
            at: row.clone(),
            track: track.to_owned(),
            fix: MissingFix::Dropped,
        }));
    }
    out
}

/// The installed addons, by track code, whose `track.add` or `track` record adds each of
/// `codes`, also where a build would leave the track out.
pub(crate) fn adders(store: &Store, codes: &BTreeSet<String>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (id, m) in store.addons().unwrap_or_default() {
        for (code, _) in added_codes(store, &id, &m) {
            if codes.contains(&code) {
                out.entry(code).or_insert_with(|| id.clone());
            }
        }
    }
    out
}

/// The track codes addon `id`'s `track.add` ops and `track` records add, each with whether its package reads.
fn added_codes(store: &Store, id: &str, m: &AddonManifest) -> Vec<(String, bool)> {
    let dir = store.addons_dir().join(id);
    let added = added_dirs(store, id, m)
        .iter()
        .filter_map(|d| match crate::map::read_package(d) {
            Ok(pkg) => Some((pkg.track_code, true)),
            Err(why) => why.code.map(|c| (c, false)),
        })
        .collect::<Vec<_>>();
    let recorded = record_tracks(&dir, id, &crate::stdlib::data::files(&dir));
    added
        .into_iter()
        .chain(recorded.into_iter().map(|t| (t.code, true)))
        .collect()
}

/// The stock rows of the tracks table with no track archive: a round there has no track until an addon fills
/// the row.
pub(crate) const DORMANT: [&str; 9] = [
    "paul_ricard",
    "portimao",
    "sochi",
    "hanoi",
    "texas_nat",
    "suzuka_east",
    "bahrain_pad",
    "silverstone_int",
    "bahrain24",
];

const ACTIVITY_TABLE: &str = "schedule.activities";
const EVENT_TABLE: &str = "schedule.all_track_events";

/// The shipped schedule an addon's added rounds are checked against: each race weekend's track and each
/// event's race weekend, by label, and the stock tracks with a track archive.
#[derive(Debug, Clone, Default)]
pub(crate) struct Schedule {
    weekends: HashMap<String, String>,
    events: HashMap<String, String>,
    tracks: HashSet<String>,
}

impl Schedule {
    pub(crate) fn of(db: &Gvdb) -> Self {
        let text = |path: &str| db.get_text(path).map(|t| t.trim().to_owned());
        let count = |table: &str| {
            let (container, row) = table.rsplit_once('.').expect("a table path");
            db.instance_count(container, row).unwrap_or(0)
        };
        let pairs = |table: &str, field: &str| {
            (0..count(table))
                .filter_map(|i| {
                    Some((
                        text(&format!("{table}_{i}._metadata_"))?,
                        text(&format!("{table}_{i}.{field}"))?,
                    ))
                })
                .collect()
        };
        Self {
            weekends: pairs(ACTIVITIES, "track"),
            events: pairs(EVENTS, "activity"),
            tracks: (0..count(TRACKS))
                .filter_map(|i| text(&format!("{TRACKS}_{i}._metadata_")))
                .filter(|c| !DORMANT.contains(&c.as_str()))
                .collect(),
        }
    }

    /// The tracks a build holds: the stock tracks with a track archive and the tracks the addons in `order`
    /// add, dormant rows they fill included.
    pub(crate) fn held(
        &self,
        store: &Store,
        order: &[String],
        addons: &BTreeMap<String, AddonManifest>,
    ) -> HashSet<String> {
        let mut out = self.tracks.clone();
        for id in order {
            if let Some(m) = addons.get(id) {
                out.extend(
                    added_codes(store, id, m)
                        .into_iter()
                        .filter(|(_, ok)| *ok)
                        .map(|(c, _)| c),
                );
            }
        }
        out
    }
}

/// A toggle of an addon whose rows add a round at a track the build doesn't hold.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct OptionTrack {
    pub option: String,
    pub track: String,
}

impl OptionTrack {
    /// The build's warning, with the addon's name, the option's label and the addon that adds the track when
    /// one is installed.
    pub(crate) fn line(&self, addon: &str, option: &str, adds: Option<&str>) -> String {
        let fix = match adds {
            Some(adds) => format!("turn {adds} on"),
            None => format!("install and turn on the addon that adds {}", self.track),
        };
        format!(
            "{addon}: option {option} adds rounds at {}, which isn't in this build, so the build leaves that \
             option's rows out and builds as if it were off; {fix}, or turn the option off",
            self.track
        )
    }
}

/// The toggles of `m` whose `when`-gated `add-row`s, in its database op files `docs`, add a race weekend or a
/// calendar event at a track `held` doesn't name. A weekend's track is the row's `track`, else its source
/// row's; an event's is its weekend's, from the row's `activity`, else its source row's.
pub(crate) fn option_tracks(
    m: &AddonManifest,
    docs: &[serde_json::Value],
    stock: &Schedule,
    held: &HashSet<String>,
) -> Vec<OptionTrack> {
    use serde_json::Value;
    use swofty_addon::manifest::OptionType;
    let toggles = |when: Option<&Value>| -> Vec<String> {
        when.and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter(|(id, want)| {
                want.as_bool() == Some(true)
                    && m.options
                        .iter()
                        .any(|o| &o.id == *id && o.kind == OptionType::Toggle)
            })
            .map(|(id, _)| id.clone())
            .collect()
    };
    let label = |from: &str| {
        let (_, rest) = from.split_once("[_metadata_=")?;
        rest.split_once(']').map(|(l, _)| l.trim().to_owned())
    };
    let mut rows = Vec::new();
    for doc in docs {
        let file = toggles(doc.get("when"));
        for op in doc.get("ops").and_then(Value::as_array).into_iter().flatten() {
            if op.get("op").and_then(Value::as_str) != Some("add-row") {
                continue;
            }
            let field = |f: &str| {
                op.get("set")
                    .and_then(|s| s.get(f))
                    .and_then(Value::as_str)
                    .map(|v| v.trim().to_owned())
            };
            let from = op.get("from").and_then(Value::as_str).unwrap_or_default();
            let mut gates = file.clone();
            gates.extend(toggles(op.get("when")));
            match op.get("table").and_then(Value::as_str) {
                Some(ACTIVITY_TABLE) if from.starts_with(ACTIVITIES) => {
                    let track = field("track").or_else(|| stock.weekends.get(&label(from)?).cloned());
                    rows.push((true, field("_metadata_"), track, gates));
                }
                Some(EVENT_TABLE) if from.starts_with(EVENTS) => {
                    let weekend = field("activity").or_else(|| stock.events.get(&label(from)?).cloned());
                    rows.push((false, weekend, None, gates));
                }
                _ => {}
            }
        }
    }
    let mut weekends = stock.weekends.clone();
    for (weekend, label, track) in rows.iter().map(|(w, l, t, _)| (w, l, t)) {
        if let (true, Some(label), Some(track)) = (weekend, label, track) {
            weekends.insert(label.clone(), track.clone());
        }
    }
    let mut out = BTreeSet::new();
    for (weekend, label, track, gates) in rows {
        let track = match weekend {
            true => track,
            false => label.and_then(|w| weekends.get(&w).cloned()),
        };
        let Some(track) = track.filter(|t| !t.is_empty() && !held.contains(t)) else {
            continue;
        };
        out.extend(gates.into_iter().map(|option| OptionTrack {
            option,
            track: track.clone(),
        }));
    }
    out.into_iter().collect()
}

/// The shipped records `db` holds with other values: what the tracks changed.
pub(crate) fn changed_records(db: &Gvdb, shipped: &Gvdb) -> Vec<Record> {
    let stock: HashMap<u64, &[u8]> = shipped
        .records
        .iter()
        .map(|r| (r.key, r.value.as_slice()))
        .collect();
    db.records
        .iter()
        .filter(|r| stock.get(&r.key).is_some_and(|v| *v != r.value.as_slice()))
        .cloned()
        .collect()
}

/// Puts back the track changes in `changed` that database ops undid. A mod
/// made from a whole game database carries the shipped value of every record
/// it holds, so a record an op set back to its shipped value gets the tracks'
/// value again, and a list an op rewrote otherwise gets back the entries the
/// tracks added to it. Returns the keys put back.
pub(crate) fn keep_changes(db: &mut Gvdb, shipped: &Gvdb, changed: &[Record]) -> Vec<u64> {
    let at: HashMap<u64, usize> = db.records.iter().enumerate().map(|(i, r)| (r.key, i)).collect();
    let stock: HashMap<u64, &Record> = shipped.records.iter().map(|r| (r.key, r)).collect();
    let mut kept = Vec::new();
    for ours in changed {
        let (Some(&i), Some(stock)) = (at.get(&ours.key), stock.get(&ours.key)) else {
            continue;
        };
        let now = &db.records[i];
        let value = if now.value == ours.value {
            None
        } else if now.value == stock.value {
            Some(ours.value.clone())
        } else if now.value_type() == ValueType::List && ours.value_type() == ValueType::List {
            merged_list(&now.value, &ours.value, &stock.value)
        } else {
            None
        };
        if let Some(value) = value {
            db.records[i].value = value;
            kept.push(ours.key);
        }
    }
    kept
}

fn merged_list(now: &[u8], ours: &[u8], stock: &[u8]) -> Option<Vec<u8>> {
    let items = |v: &[u8]| -> Vec<String> {
        String::from_utf8_lossy(v)
            .split(',')
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    };
    let (mut out, shipped) = (items(now), items(stock));
    let added: Vec<String> = items(ours)
        .into_iter()
        .filter(|c| !shipped.contains(c) && !out.contains(c))
        .collect();
    if added.is_empty() {
        return None;
    }
    out.extend(added);
    Some(out.join(",").into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_changes_survive_ops_that_carry_shipped_values() {
        let list = |path: &str, v: &str| Record::new(path, 13, v);
        let int = |path: &str, v: &str| Record::new(path, 0, v);
        let shipped = Gvdb {
            records: vec![
                list("sets.a.tracks", "melbourne,suzuka"),
                list("sets.b.tracks", "melbourne"),
                list("sets.c.tracks", "melbourne"),
                int("tracks._Track_instancecount_", "2"),
                int("tracks.Track_0.statId", "5"),
            ],
        };
        let mut db = Gvdb {
            records: vec![
                list("sets.a.tracks", "melbourne,suzuka,testring"),
                list("sets.b.tracks", "melbourne,testring"),
                list("sets.c.tracks", "melbourne,testring"),
                int("tracks._Track_instancecount_", "3"),
                int("tracks.Track_0.statId", "6"),
                int("tracks.Track_2.statId", "50"),
            ],
        };
        let changed = changed_records(&db, &shipped);
        assert_eq!(changed.len(), 5);
        db.set("sets.a.tracks", "melbourne,suzuka", None).unwrap();
        db.set("sets.b.tracks", "suzuka,imola", None).unwrap();
        db.set("tracks._Track_instancecount_", "2", None).unwrap();
        db.set("tracks.Track_0.statId", "9", None).unwrap();
        let kept = keep_changes(&mut db, &shipped, &changed);
        assert_eq!(kept.len(), 3);
        assert_eq!(
            db.get_text("sets.a.tracks").as_deref(),
            Some("melbourne,suzuka,testring")
        );
        assert_eq!(
            db.get_text("sets.b.tracks").as_deref(),
            Some("suzuka,imola,testring")
        );
        assert_eq!(
            db.get_text("sets.c.tracks").as_deref(),
            Some("melbourne,testring")
        );
        assert_eq!(db.get_text("tracks._Track_instancecount_").as_deref(), Some("3"));
        assert_eq!(db.get_text("tracks.Track_0.statId").as_deref(), Some("9"));
    }

    const STOCK_WITH_DATA: [&str; 36] = [
        "melbourne",
        "suzuka",
        "shanghai",
        "bahrain",
        "jeddah",
        "miami",
        "imola",
        "monaco",
        "catalunya",
        "montreal",
        "austria",
        "silverstone",
        "spa_francorchamps",
        "hungaroring",
        "zandvoort",
        "monza",
        "baku",
        "singapore",
        "texas",
        "mexico",
        "brazil",
        "las_vegas",
        "losail",
        "abu_dhabi",
        "portimao",
        "texas_nat",
        "suzuka_east",
        "bahrain_pad",
        "silverstone_int",
        "sochi",
        "styria",
        "hanoi",
        "silverstone_reverse",
        "austria_reverse",
        "zandvoort_reverse",
        "madrid",
    ];
    const STOCK_WITHOUT_DATA: [&str; 7] = [
        "paul_ricard",
        "imola_prototype",
        "melbourne24",
        "bahrain24",
        "suzuka24",
        "imola24",
        "miami24",
    ];

    fn stock_lists() -> Gvdb {
        let codes: Vec<&str> = STOCK_WITH_DATA
            .iter()
            .chain(&STOCK_WITHOUT_DATA)
            .copied()
            .collect();
        let mut records = vec![
            rec("tracks._Track_instancecount_", &codes.len().to_string()),
            rec("track_data.game_setup._TrackDataSet_instancecount_", "2"),
            rec(&format!("{TRACK_DATA}_0._TrackData_instancecount_"), "30"),
            rec(&format!("{TRACK_DATA}_1._TrackData_instancecount_"), "6"),
        ];
        for (i, code) in codes.iter().enumerate() {
            records.push(rec(&format!("{TRACKS}_{i}._metadata_"), code));
        }
        for (i, code) in STOCK_WITH_DATA.iter().enumerate() {
            let (set, row) = match i < 30 {
                true => (0, i),
                false => (1, i - 30),
            };
            records.push(rec(&format!("{TRACK_DATA}_{set}.TrackData_{row}.track"), code));
        }
        Gvdb { records }
    }

    fn build_track(addon: &str, code: &str, base: &str) -> BuildTrack {
        let pkg = TrackPackage {
            track_code: code.into(),
            track_name: Some(format!("{code} circuit")),
            version: None,
            author_name: None,
            erp: format!("{code}.erp"),
            activation: crate::map::Activation {
                copy_from_track: Some(base.into()),
                ..Default::default()
            },
            ers: Default::default(),
            copy_directories: Vec::new(),
            ui: None,
            extra: Default::default(),
        };
        let origin = Origin {
            addon: addon.into(),
            item: None,
        };
        (PathBuf::from(format!("/addons/{addon}/track")), pkg, origin)
    }

    fn codes(tracks: &[BuildTrack]) -> Vec<&str> {
        tracks.iter().map(|(_, p, _)| p.track_code.as_str()).collect()
    }

    #[test]
    fn the_track_lists_hold_the_stock_tracks_with_track_data_but_madrid() {
        let listed = ListedTracks::of(&stock_lists());
        assert_eq!(listed.stock(), 35);
        assert!(!listed.lists("madrid"));
        assert!(!listed.lists("bahrain24"));
        assert!(listed.lists("texas_nat"));
        assert_eq!(ListedTracks::of(&Gvdb::default()).stock(), 0);
    }

    #[test]
    fn only_enough_added_tracks_to_fill_the_lists_read_the_database() {
        assert!(!may_fill_track_lists(21));
        assert!(may_fill_track_lists(22));
    }

    #[test]
    fn the_tracks_past_the_track_lists_room_are_left_out_in_load_order() {
        let listed = ListedTracks::of(&stock_lists());
        let mut tracks = Vec::new();
        for slot in ["bahrain24", "miami24", "suzuka24", "paul_ricard"] {
            tracks.push(build_track(&format!("mod.{slot}"), slot, slot));
        }
        tracks.push(build_track("mod.hanoi", "hanoi", "hanoi"));
        tracks.push(build_track("mod.sochi", "sochi", "melbourne"));
        let added: Vec<String> = (0..36).map(|i| format!("added_{i:02}")).collect();
        for code in &added {
            tracks.push(build_track(&format!("mod.{code}"), code, "melbourne"));
        }
        assert_eq!(tracks.len(), 42);
        let (kept, left_out) = fit_track_lists(&listed, tracks.clone(), &BTreeSet::new());
        let mut want: Vec<&str> = vec![
            "bahrain24",
            "miami24",
            "suzuka24",
            "paul_ricard",
            "hanoi",
            "sochi",
        ];
        want.extend(added[..29].iter().map(String::as_str));
        assert_eq!(codes(&kept), want);
        let gone: Vec<&str> = left_out
            .iter()
            .map(|((_, p, _), _)| p.track_code.as_str())
            .collect();
        assert_eq!(gone, added[29..].iter().map(String::as_str).collect::<Vec<_>>());
        for ((_, pkg, origin), why) in &left_out {
            assert_eq!(why.code.as_deref(), Some(pkg.track_code.as_str()));
            assert_eq!(origin.addon, format!("mod.{}", pkg.track_code));
        }
        assert_eq!(
            left_out[0].1.text(),
            "track added_29 circuit is left out; the game has room for 64 tracks, 35 of its own and 29 added ones, \
             and the track addons before it in the load order fill them. Turn some track addons off to make room \
             for it"
        );
        assert_eq!(
            fit_track_lists(&listed, tracks, &BTreeSet::new()),
            (kept, left_out)
        );
    }

    #[test]
    fn a_track_with_a_base_that_has_no_track_data_takes_no_room() {
        let listed = ListedTracks::of(&stock_lists());
        let mut tracks: Vec<BuildTrack> = (0..29)
            .map(|i| build_track(&format!("mod.r{i}"), &format!("r{i}"), "melbourne"))
            .collect();
        tracks.push(build_track("mod.ricard", "magny_cours", "paul_ricard"));
        tracks.push(build_track("mod.sepang", "bahrain24", "bahrain"));
        tracks.push(build_track("mod.late", "late", "suzuka"));
        let (kept, left_out) = fit_track_lists(&listed, tracks, &BTreeSet::new());
        assert_eq!(kept.len(), 30);
        assert!(codes(&kept).contains(&"magny_cours"));
        let gone: Vec<&str> = left_out
            .iter()
            .map(|((_, p, _), _)| p.track_code.as_str())
            .collect();
        assert_eq!(gone, ["bahrain24", "late"]);
    }

    #[test]
    fn the_tracks_the_core_adds_take_their_room_first() {
        let listed = ListedTracks::of(&stock_lists());
        let mut tracks: Vec<BuildTrack> = (0..29)
            .map(|i| build_track(&format!("mod.r{i}"), &format!("r{i}"), "melbourne"))
            .collect();
        tracks.push(build_track("std.recorded", "recorded_a", "suzuka"));
        tracks.push(build_track("std.recorded", "recorded_b", "suzuka"));
        let recorded: BTreeSet<String> = ["std.recorded".to_owned()].into();
        let (kept, left_out) = fit_track_lists(&listed, tracks, &recorded);
        let gone: Vec<&str> = left_out
            .iter()
            .map(|((_, p, _), _)| p.track_code.as_str())
            .collect();
        assert_eq!(gone, ["r27", "r28"]);
        assert_eq!(&codes(&kept)[27..], ["recorded_a", "recorded_b"]);
    }

    #[test]
    fn the_track_limit_lists_each_addon_once_and_is_remembered_per_profile() {
        let listed = ListedTracks::of(&stock_lists());
        assert_eq!(TrackLimit::new(&listed, Vec::new(), Vec::new()), None);
        let limit = TrackLimit::new(
            &listed,
            vec!["A".into(), "B".into(), "C".into()],
            vec!["x.two".into(), "x.one".into(), "x.two".into()],
        )
        .unwrap();
        assert_eq!((limit.room, limit.stock), (64, 35));
        assert_eq!(limit.addons, ["x.one", "x.two"]);
        let root = std::env::temp_dir().join(format!("swofty-track-limit-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        assert!(recorded_track_limit(&root, "p").is_empty());
        record_track_limit(&root, "p", Some(&limit)).unwrap();
        assert_eq!(recorded_track_limit(&root, "p").len(), 2);
        assert!(recorded_track_limit(&root, "other").is_empty());
        record_track_limit(&root, "p", None).unwrap();
        assert!(recorded_track_limit(&root, "p").is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_full_lists_notice_names_the_tracks_left_out() {
        let listed = ListedTracks::of(&stock_lists());
        assert_eq!(track_lists_full_warning(&listed, &[]), None);
        assert_eq!(
            track_lists_full_warning(&listed, &["Mugello".into(), "Kymi Ring (Kymiring)".into()]).unwrap(),
            "Too many tracks: 2 track(s) are left out of this build because the game has room for 64 tracks, 35 \
             of its own and 29 added ones, and more stop it at a black screen after the EA logo: Mugello, Kymi \
             Ring (Kymiring). Turn some track addons off to play them"
        );
        let many: Vec<String> = (0..10).map(|i| format!("T{i}")).collect();
        assert!(
            track_lists_full_warning(&listed, &many)
                .unwrap()
                .contains(": T0, T1, T2, T3, T4, T5, T6, T7 and 2 more. Turn")
        );
    }

    fn rec(path: &str, value: &str) -> Record {
        Record::new(path, 2, value.as_bytes().to_vec())
    }

    fn db() -> Gvdb {
        let mut records = vec![
            rec("tracks._Track_instancecount_", "3"),
            rec("schedule.all_track_events._fixed_event_instancecount_", "3"),
            rec("schedule.activities._activity_race_weekend_instancecount_", "1"),
            rec("schedule.activities.activity_race_weekend_0._metadata_", "suzuka"),
            rec("schedule.activities.activity_race_weekend_0.track", "suzuka"),
            rec("content_sets._content_set_instancecount_", "2"),
            rec("content_sets.content_set_0._metadata_", "2024"),
            rec("content_sets.content_set_0._available_tracks_instancecount_", "1"),
            rec(
                "content_sets.content_set_0.available_tracks_0.tracks",
                "bahrain24",
            ),
            rec("content_sets.content_set_1._metadata_", "2026"),
            rec("content_sets.content_set_1._available_tracks_instancecount_", "1"),
            rec(
                "content_sets.content_set_1.available_tracks_0.tracks",
                "suzuka,okayama",
            ),
        ];
        for (i, (code, id, res)) in [
            ("suzuka", "13", "2019"),
            ("bahrain24", "35", "swofty_bahrain24"),
            ("okayama", "43", "swofty_okayama"),
        ]
        .into_iter()
        .enumerate()
        {
            records.push(rec(&format!("tracks.Track_{i}._metadata_"), code));
            records.push(rec(&format!("tracks.Track_{i}.statId"), id));
            records.push(rec(&format!("tracks.Track_{i}.track_resource"), res));
        }
        for (i, (name, activity)) in [
            ("rw_suzuka", "suzuka"),
            ("rw_suzuka_26", "suzuka"),
            ("rw_gone", "gone"),
        ]
        .into_iter()
        .enumerate()
        {
            records.push(rec(
                &format!("schedule.all_track_events.fixed_event_{i}._metadata_"),
                name,
            ));
            records.push(rec(
                &format!("schedule.all_track_events.fixed_event_{i}.activity"),
                activity,
            ));
        }
        Gvdb { records }
    }

    #[test]
    fn events_resolve_through_activities_and_packages_name_modded_tracks() {
        let added = [PlanTrack {
            code: "okayama".into(),
            name: Some("Okayama".into()),
            addon: Some("teamsimplified.mods".into()),
        }];
        let tracks = list(&db(), &added);
        let by = |c: &str| tracks.iter().find(|t| t.code == c).unwrap();
        assert_eq!(tracks.len(), 3);
        assert_eq!(by("suzuka").stat_id, 13);
        assert_eq!(by("suzuka").events, ["rw_suzuka", "rw_suzuka_26"]);
        assert!(!by("suzuka").modded);
        assert!(by("suzuka").playable && by("okayama").playable && !by("bahrain24").playable);
        assert!(
            by("bahrain24").modded && by("bahrain24").name.is_none() && by("bahrain24").events.is_empty()
        );
        let o = by("okayama");
        assert_eq!(
            (o.stat_id, o.modded, o.name.as_deref()),
            (43, true, Some("Okayama"))
        );
        assert_eq!(o.addon.as_deref(), Some("teamsimplified.mods"));
    }

    #[test]
    fn lists_the_tracks_installed_addons_add() {
        let root = std::env::temp_dir().join(format!("swofty-installed-tracks-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let store = Store::new(&root);
        let addon = |id: &str, ops: &str, package: Option<String>| {
            let dir = store.addons_dir().join(id);
            std::fs::create_dir_all(dir.join("ops")).unwrap();
            std::fs::write(
                dir.join("swofty.json"),
                format!(
                    r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t",
                    "compat":{{"game":"f1_25"}},"operations":{{"assets":["ops/track.json"]}}}}"#
                ),
            )
            .unwrap();
            std::fs::write(dir.join("ops/track.json"), ops).unwrap();
            if let Some(package) = package {
                std::fs::create_dir_all(dir.join("track")).unwrap();
                std::fs::write(dir.join("track/manifest.json"), package).unwrap();
            }
        };
        let add = r#"{"schema_version":1,"ops":[{"op":"track.add","source":"track"}]}"#;
        let like = |code: &str, name: &str| {
            crate::map::tests::OKAYAMA_LIKE
                .replace(
                    r#""track_code": "testring""#,
                    &format!(r#""track_code": "{code}""#),
                )
                .replace(
                    r#""track_name": "Test Ring""#,
                    &format!(r#""track_name": "{name}""#),
                )
        };
        addon("mod.bahrain24", add, Some(like("bahrain24", "Sepang")));
        addon("mod.okayama", add, Some(like("okayama", "")));
        addon("mod.twice", add, Some(like("okayama", "Again")));
        addon("mod.unread", add, None);
        addon("mod.other", r#"{"schema_version":1,"ops":[]}"#, None);
        addon(
            "mod.portimao",
            add,
            Some(r#"{ "track_code": "portimao", "track_name": "Portimao", "erp": "portimao.erp" }"#.into()),
        );
        let found = installed(&store);
        assert_eq!(
            found,
            [
                PlanTrack {
                    code: "bahrain24".into(),
                    name: Some("Sepang".into()),
                    addon: Some("mod.bahrain24".into()),
                },
                PlanTrack {
                    code: "okayama".into(),
                    name: None,
                    addon: Some("mod.okayama".into()),
                },
            ]
        );
        let addons = store.addons().unwrap();
        let bad = |id: &str| unusable(&store, id, &addons[id]);
        assert!(bad("mod.okayama").is_empty() && bad("mod.other").is_empty());
        assert_eq!(bad("mod.unread")[0].why, "it has no manifest.json");
        let portimao = bad("mod.portimao");
        assert_eq!(portimao.len(), 1);
        assert_eq!(portimao[0].why, crate::map::NO_BASE_TRACK);
        assert_eq!(
            left_out_line("Portimao", "mod.portimao", &portimao[0]),
            "Portimao (mod.portimao): track Portimao is left out; its manifest.json doesn't say which track \
             it replaces or copies. Import a fixed version, or ask its author for one"
        );
        assert!(
            left_out_line("mod.portimao", "mod.portimao", &portimao[0]).starts_with("mod.portimao: track ")
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn venues_match_a_track_name_never_its_code() {
        let v = Venues::shipped();
        assert!(v.venues.len() >= 33, "{}", v.venues.len());
        assert_eq!(
            normalise("  Autódromo José-Carlos  Pace "),
            "aut dromo jos carlos pace"
        );
        assert_eq!(normalise("Spa-Francorchamps"), "spa francorchamps");
        assert_eq!(v.by_name("Sepang").as_deref(), Some("venue/sepang"));
        assert_eq!(
            v.by_name("SEPANG international circuit").as_deref(),
            Some("venue/sepang")
        );
        assert_eq!(v.by_name("Okayama").as_deref(), Some("venue/okayama"));
        assert_eq!(v.by_name("Delhi").as_deref(), Some("venue/buddh"));
        assert_eq!(v.by_name("Igora Drive").as_deref(), Some("venue/igora_drive"));
        assert_eq!(v.by_name("bahrain24"), None);
        assert_eq!(v.by_name(""), None);
        assert_eq!(v.by_name("Test Ring"), None);
        let twice = Venues::parse(
            r#"{ "version": 1, "venues": {
                "a": { "name": "Ring", "country": "x", "aliases": [], "stock": null },
                "b": { "name": "Other", "country": "x", "aliases": ["ring"], "stock": "b" } } }"#,
        )
        .unwrap();
        assert_eq!(twice.by_name("Ring"), None, "two venues match: none");
        assert_eq!(twice.by_name("other").as_deref(), Some("venue/b"));
        assert!(twice.stock_provides("venue/b") && !twice.stock_provides("venue/a"));
        assert!(!twice.stock_provides("venue/zzz") && !twice.stock_provides("b"));
        assert!(v.stock_provides("venue/albert_park") && !v.stock_provides("venue/sochi"));
    }

    #[test]
    fn older_tracks_name_their_base_the_row_they_fill_their_stat_id_and_venue() {
        let pkg = |code: &str, name: Option<&str>, from: &str, slot: Option<&str>| TrackPackage {
            track_code: code.into(),
            track_name: name.map(str::to_owned),
            version: None,
            author_name: None,
            erp: format!("{code}.erp"),
            activation: crate::map::Activation {
                copy_from_track: Some(from.into()),
                copy_slot: slot.map(str::to_owned),
                ..Default::default()
            },
            ers: Default::default(),
            copy_directories: Vec::new(),
            ui: None,
            extra: Default::default(),
        };
        let shipped = {
            let mut d = db();
            d.set("tracks._Track_instancecount_", "4", None).unwrap();
            d.records.push(rec("tracks.Track_3._metadata_", "sochi"));
            d.records.push(rec("tracks.Track_3.statId", "30"));
            d
        };
        let mut built = shipped.clone();
        built.set("tracks._Track_instancecount_", "5", None).unwrap();
        built.records.push(rec("tracks.Track_4._metadata_", "okayama2"));
        built.records.push(rec("tracks.Track_4.statId", "44"));
        let okayama = pkg("okayama2", Some("Okayama"), "melbourne", Some("melbourne"));
        let sochi = pkg("sochi", Some("Sochi Autodrom"), "baku", None);
        let unnamed = pkg("ring", None, "zandvoort", Some("zandvoort"));
        let out = older_tracks(
            "Classic Seasons",
            &[
                ("mod.okayama", &okayama),
                ("mod.sochi", &sochi),
                ("mod.ring", &unnamed),
            ],
            &shipped,
            &built,
            Venues::shipped(),
        );
        assert_eq!((out.version, out.profile.as_str()), (1, "Classic Seasons"));
        let entry = |addon: &str,
                     code: &str,
                     name: &str,
                     base: &str,
                     slot: Option<&str>,
                     id: Option<u32>,
                     venue: Option<&str>| OlderTrack {
            addon: addon.into(),
            code: code.into(),
            name: name.into(),
            base: base.into(),
            slot: slot.map(str::to_owned),
            stat_id: id,
            venue: venue.map(str::to_owned),
        };
        assert_eq!(
            out.tracks,
            [
                entry(
                    "mod.okayama",
                    "okayama2",
                    "Okayama",
                    "game/melbourne",
                    None,
                    Some(44),
                    Some("venue/okayama")
                ),
                entry(
                    "mod.sochi",
                    "sochi",
                    "Sochi Autodrom",
                    "game/baku",
                    Some("game/sochi"),
                    Some(30),
                    Some("venue/sochi")
                ),
                entry("mod.ring", "ring", "ring", "game/zandvoort", None, None, None),
            ]
        );
        let json = serde_json::to_value(&out).unwrap();
        assert_eq!(json["tracks"][2]["slot"], serde_json::Value::Null);
        assert_eq!(json["tracks"][2]["venue"], serde_json::Value::Null);
    }

    #[test]
    fn track_records_become_packages_from_their_files_folder() {
        let root = std::env::temp_dir().join(format!("swofty-record-tracks-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(root.join("registry")).unwrap();
        std::fs::create_dir_all(root.join("track/mipmaps")).unwrap();
        std::fs::create_dir_all(root.join("copy")).unwrap();
        std::fs::write(
            root.join("registry/track.json"),
            r#"{ "track": {
                "ring": { "code": "ring", "name": "Ring", "base": "game/austria", "files": "track/" },
                "copy": { "code": "testring", "base": "game/melbourne", "files": "copy/" },
                "nofiles": { "code": "nofiles", "base": "game/melbourne" },
                "outside": { "code": "outside", "base": "game/melbourne", "files": "../x/" },
                "$select": [],
                "game/melbourne": { "name": "a layer" },
                "nocode": { "base": "game/melbourne" } },
              "track.data": { "mod.x/ring/f1_2025": { "laps": 3 } } }"#,
        )
        .unwrap();
        std::fs::write(root.join("track/ring.erp"), b"erp").unwrap();
        std::fs::write(root.join("copy/manifest.json"), crate::map::tests::OKAYAMA_LIKE).unwrap();
        let found = record_tracks(&root, "mod.x", &["registry/track.json".into()]);
        let ids: Vec<&str> = found.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            ["mod.x/copy", "mod.x/nofiles", "mod.x/outside", "mod.x/ring"]
        );
        let by = |id: &str| found.iter().find(|t| t.id == id).unwrap();
        let (dir, ring) = record_package(&root, by("mod.x/ring")).unwrap();
        assert_eq!(dir, root.join("track"));
        assert_eq!(
            (
                ring.track_code.as_str(),
                ring.track_name.as_deref(),
                ring.erp.as_str()
            ),
            ("ring", Some("Ring"), "ring.erp")
        );
        assert_eq!(ring.activation.base_track(), Some("austria"));
        assert_eq!(ring.copy_directories.len(), 1);
        let (_, copy) = record_package(&root, by("mod.x/copy")).unwrap();
        assert_eq!(copy.track_code, "testring");
        assert!(
            record_package(&root, by("mod.x/nofiles"))
                .unwrap_err()
                .why
                .contains("names no files")
        );
        assert!(
            record_package(&root, by("mod.x/outside"))
                .unwrap_err()
                .why
                .contains("leaves")
        );
        std::fs::remove_file(root.join("track/ring.erp")).unwrap();
        assert!(
            record_package(&root, by("mod.x/ring"))
                .unwrap_err()
                .why
                .contains("has no ring.erp")
        );
        let wrong = RecordTrack {
            code: "other".into(),
            ..by("mod.x/copy").clone()
        };
        assert!(
            record_package(&root, &wrong)
                .unwrap_err()
                .why
                .contains("is for testring")
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn weekends_at_a_left_out_track_go_back_to_their_stock_track() {
        let weekend = |i: usize, label: &str, track: &str| {
            [
                rec(&format!("{ACTIVITIES}_{i}._metadata_"), label),
                rec(&format!("{ACTIVITIES}_{i}.track"), track),
            ]
        };
        let mut shipped = db();
        shipped
            .set(
                "schedule.activities._activity_race_weekend_instancecount_",
                "2",
                None,
            )
            .unwrap();
        shipped.records.extend(weekend(1, "rw_okayama", "okayama"));
        let mut built = shipped.clone();
        built
            .set(
                "schedule.activities._activity_race_weekend_instancecount_",
                "4",
                None,
            )
            .unwrap();
        built
            .set(&format!("{ACTIVITIES}_0.track"), "portimao", None)
            .unwrap();
        built.records.extend(weekend(2, "rw_added", "portimao"));
        built.records.extend(weekend(3, "rw_other", "monza"));
        let warnings = restore_weekends(&mut built, &shipped, &["portimao", "okayama"]).unwrap();
        assert_eq!(
            warnings,
            [
                "race weekend suzuka: races at suzuka again; track portimao is left out of this build",
                "race weekend rw_added: races at suzuka instead; track portimao is left out of this build and the game won't start with a race weekend at it",
            ]
        );
        let track = |i: usize| built.get_text(&format!("{ACTIVITIES}_{i}.track")).unwrap();
        assert_eq!(
            [track(0), track(1), track(2), track(3)],
            ["suzuka", "okayama", "suzuka", "monza"]
        );
        let mut untouched = shipped.clone();
        assert!(
            restore_weekends(&mut untouched, &shipped, &["monza"])
                .unwrap()
                .is_empty()
        );
    }

    fn weekends(db: &mut Gvdb, rows: &[(&str, &str)]) {
        let n = rows.len();
        db.set(
            "schedule.activities._activity_race_weekend_instancecount_",
            n.to_string(),
            Some(2),
        )
        .unwrap();
        for (i, (label, track)) in rows.iter().enumerate() {
            db.set(&format!("{ACTIVITIES}_{i}._metadata_"), *label, Some(2))
                .unwrap();
            db.set(&format!("{ACTIVITIES}_{i}.track"), *track, Some(2))
                .unwrap();
        }
    }

    #[test]
    fn a_calendar_addons_own_weekend_at_a_left_out_track_races_at_a_held_track() {
        let shipped = db();
        let mut built = shipped.clone();
        weekends(
            &mut built,
            &[("suzuka", "suzuka"), ("dejw_career_nurburgring", "nurburgring")],
        );
        let said = restore_weekends(&mut built, &shipped, &["nurburgring", "sepang"]).unwrap();
        assert_eq!(
            said,
            [
                "race weekend dejw_career_nurburgring: races at suzuka instead; track nurburgring is left out of \
              this build and the game won't start with a race weekend at it"
            ]
        );
        assert_eq!(
            built.get_text(&format!("{ACTIVITIES}_1.track")).unwrap(),
            "suzuka"
        );
    }

    #[test]
    fn season_addon_weekends_at_left_out_tracks_race_at_melbourne_when_the_build_holds_it() {
        let shipped = db();
        let mut built = shipped.clone();
        built.set("tracks._Track_instancecount_", "4", None).unwrap();
        built
            .set("tracks.Track_3._metadata_", "melbourne", Some(2))
            .unwrap();
        weekends(
            &mut built,
            &[
                ("suzuka", "suzuka"),
                ("cs_brhgp", "brh"),
                ("cs_daytona", "daytona"),
                ("cs_hanoi_ac", "hanoi_ac"),
            ],
        );
        let said = restore_weekends(&mut built, &shipped, &["brh", "daytona", "hanoi_ac"]).unwrap();
        assert_eq!(said.len(), 3);
        for i in 1..4 {
            assert_eq!(
                built.get_text(&format!("{ACTIVITIES}_{i}.track")).unwrap(),
                "melbourne"
            );
        }
        assert_eq!(
            built.get_text(&format!("{ACTIVITIES}_0.track")).unwrap(),
            "suzuka"
        );
    }

    #[test]
    fn a_weekend_with_no_track_to_move_to_stops_the_build() {
        let shipped = db();
        let mut built = shipped.clone();
        built.set("tracks._Track_instancecount_", "0", None).unwrap();
        weekends(&mut built, &[("dejw_career_nurburgring", "nurburgring")]);
        let stuck = restore_weekends(&mut built, &shipped, &["nurburgring"]).unwrap_err();
        assert_eq!(stuck.len(), 1);
        assert!(stuck[0].contains("the game won't start like this"));
    }

    #[test]
    fn a_sorted_track_order_loses_a_left_out_track() {
        let mut built = Gvdb::default();
        let order = format!("{SORTED_ORDER}_0");
        built
            .set(
                "track_data.sorted_track_order._sorted_track_order_instancecount_",
                "1",
                Some(0),
            )
            .unwrap();
        built
            .set(
                &format!("{order}._metadata_"),
                "default_sorted_track_order",
                Some(2),
            )
            .unwrap();
        built
            .set(&format!("{order}._TrackRef_instancecount_"), "3", Some(0))
            .unwrap();
        for (i, t) in ["melbourne", "suzuka_reverse", "monza"].iter().enumerate() {
            built
                .set(&format!("{order}.TrackRef_{i}._metadata_"), *t, Some(2))
                .unwrap();
            built
                .set(&format!("{order}.TrackRef_{i}.track"), *t, Some(2))
                .unwrap();
        }
        let said = drop_left_out_orders(&mut built, &["suzuka_reverse"]);
        assert_eq!(
            said,
            [
                "sorted track order default_sorted_track_order listed suzuka_reverse, which is left out of this build; the build takes it out"
            ]
        );
        assert_eq!(
            built
                .get_text(&format!("{order}._TrackRef_instancecount_"))
                .unwrap(),
            "2"
        );
        assert_eq!(
            built.get_text(&format!("{order}.TrackRef_1.track")).unwrap(),
            "monza"
        );
        assert!(built.get(&format!("{order}.TrackRef_2.track")).is_none());
        assert!(drop_left_out_orders(&mut built, &["suzuka_reverse"]).is_empty());
    }

    #[test]
    fn records_naming_a_track_not_in_the_build_get_their_shipped_track_back() {
        let mut shipped = db();
        shipped
            .set(
                "schedule.activities._activity_race_weekend_instancecount_",
                "2",
                None,
            )
            .unwrap();
        shipped.records.extend([
            rec(&format!("{ACTIVITIES}_1._metadata_"), "okayama"),
            rec(&format!("{ACTIVITIES}_1.track"), "okayama"),
            rec("track_data.game_setup._TrackDataSet_instancecount_", "1"),
            rec(&format!("{TRACK_DATA}_0._metadata_"), "f1"),
            rec(&format!("{TRACK_DATA}_0._TrackData_instancecount_"), "1"),
            rec(&format!("{TRACK_DATA}_0.TrackData_0._metadata_"), "suzuka"),
            rec(&format!("{TRACK_DATA}_0.TrackData_0.track"), "suzuka"),
            rec("game_setup.game_setup_modes._game_setup_mode_instancecount_", "1"),
            rec(&format!("{GAME_MODES}_0._metadata_"), "f1"),
            rec(
                &format!("{GAME_MODES}_0.restricted_tracks"),
                "okayama,shipped_only",
            ),
        ]);
        let mut untouched = shipped.clone();
        assert!(keep_track_refs(&mut untouched, &shipped, &[], &[]).is_empty());
        let mut built = shipped.clone();
        let mut set = |path: &str, value: &str| built.set(path, value, None).unwrap();
        set("schedule.activities._activity_race_weekend_instancecount_", "4");
        set(&format!("{ACTIVITIES}_0.track"), "istanbul_park");
        set(&format!("{TRACK_DATA}_0.TrackData_0.track"), "paul_ricard");
        set(
            "content_sets.content_set_1.available_tracks_0.tracks",
            "suzuka,istanbul_park,okayama",
        );
        set(
            &format!("{GAME_MODES}_0.restricted_tracks"),
            "okayama,shipped_only,hockenheim,ring",
        );
        built.records.extend([
            rec(&format!("{ACTIVITIES}_2._metadata_"), "rw_added"),
            rec(&format!("{ACTIVITIES}_2.track"), "hockenheim"),
            rec(&format!("{ACTIVITIES}_3._metadata_"), "rw_left"),
            rec(&format!("{ACTIVITIES}_3.track"), "portimao"),
        ]);
        let missing = keep_track_refs(&mut built, &shipped, &["ring"], &["portimao"]);
        let got: Vec<(&str, &str, &MissingFix)> = missing
            .iter()
            .map(|m| (m.at.as_str(), m.track.as_str(), &m.fix))
            .collect();
        assert_eq!(
            got,
            [
                (
                    "race weekend suzuka",
                    "istanbul_park",
                    &MissingFix::Stock("suzuka".into())
                ),
                ("race weekend rw_added", "hockenheim", &MissingFix::Kept),
                (
                    "track settings f1/suzuka",
                    "paul_ricard",
                    &MissingFix::Stock("suzuka".into())
                ),
                ("content set 2026", "istanbul_park", &MissingFix::Dropped),
                ("game mode f1", "hockenheim", &MissingFix::Dropped),
            ]
        );
        assert_eq!(missing[0].key, fnv1a64(format!("{ACTIVITIES}_0.track")));
        let text = |path: &str| built.get_text(path).unwrap();
        assert_eq!(text(&format!("{ACTIVITIES}_0.track")), "suzuka");
        assert_eq!(text(&format!("{ACTIVITIES}_2.track")), "hockenheim");
        assert_eq!(text(&format!("{ACTIVITIES}_3.track")), "portimao");
        assert_eq!(text(&format!("{TRACK_DATA}_0.TrackData_0.track")), "suzuka");
        assert_eq!(
            text("content_sets.content_set_1.available_tracks_0.tracks"),
            "suzuka,okayama"
        );
        assert_eq!(
            text(&format!("{GAME_MODES}_0.restricted_tracks")),
            "okayama,shipped_only,ring"
        );
        let again = keep_track_refs(&mut built, &shipped, &["ring"], &["portimao"]);
        assert_eq!(again, [missing[1].clone()]);
        assert!(keep_track_refs(&mut Gvdb::default(), &shipped, &[], &[]).is_empty());
    }

    #[test]
    fn missing_track_lines_name_the_addon_that_set_it_and_the_one_to_turn_on() {
        let missing = |at: &str, fix: MissingFix| MissingTrack {
            key: 0,
            at: at.into(),
            track: "istanbul_park".into(),
            fix,
        };
        let weekend = missing("race weekend bahrain", MissingFix::Stock("bahrain".into()));
        assert_eq!(
            weekend.line(Some("My Calendar"), Some("Istanbul Park")),
            "My Calendar sets race weekend bahrain to istanbul_park, which is off: turn Istanbul Park on; \
             until then this build puts back bahrain"
        );
        assert_eq!(
            weekend.line(None, None),
            "race weekend bahrain is set to istanbul_park, which isn't in this build; until then this build puts \
             back bahrain"
        );
        assert_eq!(
            missing("content set 2026", MissingFix::Dropped).line(Some("My Calendar"), Some("Istanbul Park")),
            "My Calendar adds istanbul_park to content set 2026, which is off: turn Istanbul Park on; until then \
             this build leaves it out"
        );
        let added = missing("race weekend rw_added", MissingFix::Kept);
        assert_eq!(
            added.line(Some("My Calendar"), Some("Istanbul Park")),
            "My Calendar sets race weekend rw_added to istanbul_park, which is off: turn Istanbul Park on or \
             change it in DB Editor; the game won't start like this"
        );
        assert_eq!(
            added.line(Some("My Calendar"), None),
            "My Calendar sets race weekend rw_added to istanbul_park, which isn't in this build: change it in DB \
             Editor; the game won't start like this"
        );
    }

    #[test]
    fn adders_name_the_addon_whose_track_record_adds_a_track() {
        let root = std::env::temp_dir().join(format!("swofty-track-adders-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let store = Store::new(&root);
        let dir = store.addons_dir().join("track.testring");
        std::fs::create_dir_all(dir.join("registry")).unwrap();
        std::fs::write(
            dir.join("swofty.json"),
            r#"{"schema_version":1,"id":"track.testring","name":"Testring","version":"1.0.0","author":"t",
            "compat":{"game":"f1_25"}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("registry/track.json"),
            r#"{"track":{"testring":{"code":"testring","name":"Testring","files":"track/"}}}"#,
        )
        .unwrap();
        let codes: BTreeSet<String> = ["testring".to_owned(), "nowhere".to_owned()].into();
        let got = adders(&store, &codes);
        std::fs::remove_dir_all(&root).ok();
        assert_eq!(
            got,
            BTreeMap::from([("testring".to_owned(), "track.testring".to_owned())])
        );
    }

    fn schedule() -> Schedule {
        let mut records = vec![
            rec("tracks._Track_instancecount_", "4"),
            rec("schedule.activities._activity_race_weekend_instancecount_", "3"),
            rec("schedule.all_track_events._fixed_event_instancecount_", "3"),
        ];
        for (i, code) in ["melbourne", "spa_francorchamps", "portimao", "sochi"]
            .iter()
            .enumerate()
        {
            records.push(rec(&format!("{TRACKS}_{i}._metadata_"), code));
        }
        for (i, code) in ["spa_francorchamps", "portimao", "sochi"].iter().enumerate() {
            records.push(rec(&format!("{ACTIVITIES}_{i}._metadata_"), code));
            records.push(rec(&format!("{ACTIVITIES}_{i}.track"), code));
            records.push(rec(&format!("{EVENTS}_{i}._metadata_"), &format!("rw_{code}")));
            records.push(rec(&format!("{EVENTS}_{i}.activity"), code));
        }
        Schedule::of(&Gvdb { records })
    }

    #[test]
    fn an_option_adding_rounds_at_a_track_the_build_lacks_is_named_and_present_or_filled_tracks_are_not() {
        let m = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "t.f2", "name": "F2", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" },
                 "options": [
                   { "id": "istanbul", "type": "toggle", "default": false, "binds": "db" },
                   { "id": "spa", "type": "toggle", "default": false, "binds": "db" },
                   { "id": "portimao", "type": "toggle", "default": false, "binds": "db" },
                   { "id": "sochi", "type": "toggle", "default": false, "binds": "db" },
                   { "id": "laps", "type": "choice", "default": "a", "binds": "db",
                     "values": [{ "value": "a", "label": "A" }, { "value": "b", "label": "B" }] }
                 ] }"#,
        )
        .unwrap();
        let docs = vec![serde_json::json!({ "schema_version": 1, "ops": [
                { "op": "add-row", "table": "schedule.activities",
                  "from": "schedule.activities.activity_race_weekend[_metadata_=spa_francorchamps]",
                  "set": { "_metadata_": "istanbul_park", "track": "istanbul_park" }, "when": { "istanbul": true } },
                { "op": "add-row", "table": "schedule.all_track_events",
                  "from": "schedule.all_track_events.fixed_event[_metadata_=rw_spa_francorchamps]",
                  "set": { "_metadata_": "rw_istanbul_27", "activity": "istanbul_park" }, "when": { "istanbul": true } },
                { "op": "add-row", "table": "schedule.all_track_events",
                  "from": "schedule.all_track_events.fixed_event[_metadata_=rw_spa_francorchamps]",
                  "set": { "_metadata_": "rw_spa_27" }, "when": { "spa": true } },
                { "op": "add-row", "table": "schedule.all_track_events",
                  "from": "schedule.all_track_events.fixed_event[_metadata_=rw_portimao]",
                  "set": { "_metadata_": "rw_portimao_27" }, "when": { "portimao": true } },
                { "op": "add-row", "table": "schedule.all_track_events",
                  "from": "schedule.all_track_events.fixed_event[_metadata_=rw_sochi]",
                  "set": { "_metadata_": "rw_sochi_27" }, "when": { "sochi": true, "laps": "b" } },
                { "op": "add-row", "table": "schedule.all_track_events",
                  "from": "schedule.all_track_events.fixed_event[_metadata_=rw_spa_francorchamps]",
                  "set": { "_metadata_": "rw_mugello", "activity": "istanbul_park" } },
                { "op": "set", "select": "schedule.track_lists.track_event_list[_metadata_=F1_2027]",
                  "field": "_fixed_event_reference_instancecount_", "value": "24",
                  "when": { "istanbul": true, "portimao": true } }
            ] })];
        let stock = schedule();
        let found = |held: &[&str]| -> Vec<(String, String)> {
            let held: HashSet<String> = held.iter().map(|c| (*c).to_owned()).collect();
            option_tracks(&m, &docs, &stock, &stock.tracks.union(&held).cloned().collect())
                .into_iter()
                .map(|t| (t.option, t.track))
                .collect()
        };
        let pair = |o: &str, t: &str| (o.to_owned(), t.to_owned());
        assert!(!stock.tracks.contains("portimao") && stock.tracks.contains("spa_francorchamps"));
        assert_eq!(
            found(&[]),
            [
                pair("istanbul", "istanbul_park"),
                pair("portimao", "portimao"),
                pair("sochi", "sochi")
            ]
        );
        assert_eq!(found(&["istanbul_park", "portimao", "sochi"]), []);
        assert_eq!(
            found(&["sochi"]),
            [pair("istanbul", "istanbul_park"), pair("portimao", "portimao")]
        );
        let line = OptionTrack {
            option: "istanbul".into(),
            track: "istanbul_park".into(),
        }
        .line("F2 Revamp", "Turkish GP (Istanbul Park)", Some("Istanbul Park"));
        assert_eq!(
            line,
            "F2 Revamp: option Turkish GP (Istanbul Park) adds rounds at istanbul_park, which isn't in this build, \
             so the build leaves that option's rows out and builds as if it were off; turn Istanbul Park on, or \
             turn the option off"
        );
    }
}
