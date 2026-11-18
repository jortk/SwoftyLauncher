//! `$DATA/registry/track-facts.json`: what each track's worlds and AI data hold after a build,
//! for the core's box track lists (docs/std/systems/tracks.md, contract C2). Every successful
//! build writes it, also with no track addon, next to the core's reports and never into the
//! game folder. The database scanned is the one the game reads: the build's file with the
//! runtime database edits the core applies at its early stage. `inputs` hashes every scanned file's path, size and modification time plus the
//! grid size; a build whose inputs match the file on disk leaves it as it is.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use swofty_formats::erp::ErpFile;
use swofty_formats::gvdb::{Gvdb, Record};
use swofty_formats::hash::fnv1a64;
use swofty_formats::pkg::{Node, Pkg};

use super::grid;
use super::place::WORLD_FALLBACKS;
use crate::Result;

pub const FILE: &str = "track-facts.json";
pub const VERSION: u32 = 1;
/// The stock tracks' folders, relative to the game folder.
pub const TRACKS_DIR: &str = "2025_asset_groups/environment_package/tracks";
const EAID: &str = "eaid://environment_package/tracks/";
const LAYERS: &str = "track_data.track_layers.TrackLayer";
const TRACKS: &str = "tracks.Track";
const PACKAGES: &str = "entrants.vehicles.vehicle_packages.vehicle_package";
const WORLD_MARK: &str = "[FILENAME]_";
const DIFFICULTY_FIELDS: [&str; 2] = ["time", "difficulty_ratio"];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Facts {
    pub version: u32,
    pub profile: String,
    pub inputs: String,
    pub grid_size: u32,
    pub layers: Vec<Layer>,
    pub tracks: BTreeMap<String, TrackFacts>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub label: String,
    pub setup_modes: Vec<String>,
    pub seasons: Vec<String>,
    pub weekend_formats: Vec<String>,
    pub world: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TrackFacts {
    pub worlds: BTreeMap<String, World>,
    pub ai: Vec<String>,
    pub ai_difficulties: Vec<String>,
    /// Whether Time Trial can run there: every track with a world counts as supported, tracks
    /// made for 1.5 included (decision D5); a track known not to run says why.
    #[serde(default = "supported")]
    pub time_trial: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_trial_why: Option<String>,
    /// For a track record: the menu UI lists the build made its own pictures for
    /// (`overrides.<list>.<code>`), which the core points the track's `TrackTextures` at.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ui_lists: Vec<String>,
}

fn supported() -> bool {
    true
}

/// Why a track whose files hold no world the game's layers load can't run Time Trial.
pub const NO_WORLD: &str = "its files hold none of the worlds the game's track layers load";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub grid: u32,
    pub garages: u32,
    pub pit_boxes: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stand_in: Option<String>,
}

/// A track the build adds with `track.add`: its package's own archive, for the worlds it
/// ships, and the archive the build places, with stand-in worlds and the extra grid slots.
#[derive(Debug, Clone, PartialEq)]
pub struct Added {
    pub code: String,
    pub shipped: PathBuf,
    pub placed: PathBuf,
    /// For a track record, whose rows the core adds at boot: the stock track its rows are
    /// cloned from, for its `ai_track_difficulties`.
    pub base: Option<String>,
    /// For a track record: the menu UI lists the build made its pictures for.
    pub ui_lists: Vec<String>,
}

/// What a build hands the scan.
pub struct Scan<'a> {
    pub profile: &'a str,
    /// The database the build made, or the game's own when it changed none.
    pub db: PathBuf,
    /// The game folder, for the stock tracks' AI folders.
    pub game: &'a Path,
    /// A game-folder path to the file the game reads after the build: the build's own, else the
    /// game's original.
    pub file: &'a dyn Fn(&str) -> PathBuf,
    /// The game-folder paths the build adds or replaces.
    pub placed: Vec<String>,
    pub added: Vec<Added>,
    /// The build's `swofty/assets.json`: on the runtime database route the core applies its
    /// `db_edits` at its early stage, so the database the game reads is `db` with them.
    pub edits: Option<PathBuf>,
}

#[derive(Deserialize)]
struct RuntimeEdit {
    hash: String,
    #[serde(rename = "type")]
    tag: u8,
    value: String,
}

#[derive(Deserialize)]
struct RuntimeAssets {
    #[serde(default)]
    db_edits: Vec<RuntimeEdit>,
}

/// `gvdb` with the runtime database edits of `assets` (a `swofty/assets.json`) applied: each
/// edit sets its record's type and text, or adds the record.
fn with_edits(gvdb: &mut Gvdb, assets: &[u8]) -> usize {
    let Ok(parsed) = serde_json::from_slice::<RuntimeAssets>(assets) else {
        return 0;
    };
    let mut at: HashMap<u64, usize> = gvdb.records.iter().enumerate().map(|(i, r)| (r.key, i)).collect();
    let mut applied = 0;
    for e in parsed.db_edits {
        let Ok(key) = u64::from_str_radix(&e.hash, 16) else {
            continue;
        };
        let value = e.value.into_bytes();
        match at.get(&key) {
            Some(&i) => {
                gvdb.records[i].tag = e.tag;
                gvdb.records[i].value = value;
            }
            None => {
                at.insert(key, gvdb.records.len());
                gvdb.records.push(Record {
                    key,
                    tag: e.tag,
                    value,
                });
            }
        }
        applied += 1;
    }
    applied
}

struct Keyed<'a>(HashMap<u64, &'a Record>);

impl<'a> Keyed<'a> {
    fn new(db: &'a Gvdb) -> Self {
        Self(db.records.iter().map(|r| (r.key, r)).collect())
    }

    fn text(&self, path: &str) -> Option<String> {
        self.0.get(&fnv1a64(path)).map(|r| r.text().trim().to_owned())
    }

    fn has(&self, path: &str) -> bool {
        self.0.contains_key(&fnv1a64(path))
    }

    fn count(&self, table: &str) -> usize {
        let (container, row) = table.rsplit_once('.').unwrap_or(("", table));
        self.text(&format!("{container}._{row}_instancecount_"))
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    }

    fn list(&self, path: &str) -> Vec<String> {
        self.text(path)
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The valid track layers in row order, each with its `worldEri` template.
fn layer_rows(db: &Keyed) -> Vec<(Layer, String)> {
    (0..db.count(LAYERS))
        .filter_map(|i| {
            let row = format!("{LAYERS}_{i}");
            let invalid = db
                .text(&format!("{row}.is_invalid"))
                .is_some_and(|v| v.eq_ignore_ascii_case("true") || v == "1");
            if invalid {
                return None;
            }
            let eri = db.text(&format!("{row}.worldEri"))?;
            let world = eri
                .rsplit_once(WORLD_MARK)
                .map(|(_, w)| w.trim_end_matches(".wep").to_owned())
                .filter(|w| !w.is_empty())?;
            Some((
                Layer {
                    label: db.text(&format!("{row}._metadata_")).unwrap_or_default(),
                    setup_modes: db.list(&format!("{row}.game_setup_modes")),
                    seasons: db.list(&format!("{row}.seasons")),
                    weekend_formats: db.list(&format!("{row}.weekend_formats")),
                    world,
                },
                eri,
            ))
        })
        .collect()
}

/// The `tracks.Track` codes in row order, each once.
fn track_codes(db: &Keyed) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    for i in 0..db.count(TRACKS) {
        let Some(code) = db.text(&format!("{TRACKS}_{i}._metadata_")) else {
            continue;
        };
        if !code.is_empty() && !out.iter().any(|(_, c)| *c == code) {
            out.push((i, code));
        }
    }
    out
}

fn package_folders(db: &Keyed) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = super::db::REGISTRY_JSON
        .split('"')
        .filter_map(|s| s.strip_prefix("ai_track_difficulties."))
        .filter_map(|s| s.split('.').next())
        .map(str::to_owned)
        .collect();
    for i in 0..db.count(PACKAGES) {
        if let Some(f) = db.text(&format!("{PACKAGES}_{i}.ai_package_folder"))
            && !f.is_empty()
        {
            out.insert(f);
        }
    }
    out
}

fn difficulties(db: &Keyed, row: usize, folders: &BTreeSet<String>) -> Vec<String> {
    folders
        .iter()
        .filter(|f| {
            DIFFICULTY_FIELDS
                .iter()
                .any(|field| db.has(&format!("{TRACKS}_{row}.ai_track_difficulties.{f}.{field}")))
        })
        .cloned()
        .collect()
}

fn wep(code: &str, variant: &str) -> String {
    format!("{EAID}{code}/wep/{code}_{variant}.wep")
}

fn stock_world_file(code: &str, variant: &str) -> String {
    format!("{TRACKS_DIR}/{code}/wep/{code}_{variant}.erp")
}

/// A layer's world resource at one track: its `worldEri` template with the track's file name.
fn world_eri(template: &str, code: &str) -> String {
    template.replace("[FILENAME]", code)
}

/// The game-folder archive a world resource lives in, by the game's layout: `eaid://<path>.wep`
/// in `2025_asset_groups/<path>.erp`.
fn world_file(eri: &str) -> Option<String> {
    let path = eri.strip_prefix("eaid://")?.strip_suffix(".wep")?;
    Some(format!("2025_asset_groups/{path}.erp"))
}

/// Each world the layers load, with the `worldEri` template of the first layer naming it.
fn world_templates(rows: &[(Layer, String)]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for (l, eri) in rows {
        out.entry(l.world.clone()).or_insert_with(|| eri.clone());
    }
    out
}

/// The world's room: the instances of the layers its `.wep` (`world`, the resource the layer's
/// `worldEri` names at this track) and the common `.wep` list, found in any of `files`. `None`
/// when no file holds the world.
fn room(files: &[&ErpFile], code: &str, world: &str) -> Option<grid::Room> {
    let find = |name: &str| {
        files
            .iter()
            .find_map(|f| f.erp.resources.iter().find(|r| r.name == name).map(|r| (*f, r)))
    };
    let names = |eaid: &str| -> Vec<String> {
        let Some((file, res)) = find(eaid) else {
            return Vec::new();
        };
        let Ok(pkg) = file
            .load(res)
            .and_then(|r| r.fragments[0].data())
            .and_then(|d| Pkg::parse(&d))
        else {
            return Vec::new();
        };
        pkg.root
            .as_obj()
            .and_then(|o| o.get("layers"))
            .and_then(Node::as_arr)
            .map(|a| {
                a.items
                    .iter()
                    .filter_map(|n| Some(n.as_val()?.as_str()?.to_owned()))
                    .collect()
            })
            .unwrap_or_default()
    };
    find(world)?;
    let mut wanted = names(&wep(code, "common"));
    for n in names(world) {
        if !wanted.contains(&n) {
            wanted.push(n);
        }
    }
    let mut roots = Vec::new();
    for name in &wanted {
        let Some((file, res)) = find(name) else {
            continue;
        };
        if let Ok(pkg) = file
            .load(res)
            .and_then(|r| r.fragments[0].data())
            .and_then(|d| Pkg::parse(&d))
        {
            roots.push(pkg.root);
        }
    }
    Some(grid::room_of(&roots))
}

fn ai_of_names<'a>(code: &str, names: impl Iterator<Item = &'a str>) -> BTreeSet<String> {
    let prefix = format!("{EAID}{code}/ai/");
    names
        .filter_map(|n| n.strip_prefix(&prefix))
        .filter_map(|rest| {
            let mut parts = rest.splitn(3, '/');
            let (package, asset) = (parts.next()?, parts.next()?);
            parts.next()?;
            Some(format!("{package}/{asset}"))
        })
        .collect()
}

fn ai_on_disk(dir: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let subdirs = |d: &Path| -> Vec<String> {
        std::fs::read_dir(d)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect()
    };
    for package in subdirs(dir) {
        for asset in subdirs(&dir.join(&package)) {
            out.insert(format!("{package}/{asset}"));
        }
    }
    out
}

fn ai_placed(code: &str, placed: &[String]) -> BTreeSet<String> {
    let prefix = format!("{TRACKS_DIR}/{code}/ai/");
    placed
        .iter()
        .filter_map(|p| p.strip_prefix(&prefix))
        .filter_map(|rest| {
            let mut parts = rest.splitn(3, '/');
            let (package, asset) = (parts.next()?, parts.next()?);
            parts.next()?;
            Some(format!("{package}/{asset}"))
        })
        .collect()
}

fn stamp(hasher: &mut Sha256, path: &Path) {
    let (len, mtime) = std::fs::metadata(path).map_or((0, None), |m| (m.len(), m.modified().ok()));
    hasher.update(format!("{}|{len}|{mtime:?};", path.display()));
}

/// The stand-in each world missing from a package's own archive gets, as `place` picks it.
fn stand_ins(code: &str, shipped: &Path) -> BTreeMap<String, String> {
    let Ok(file) = ErpFile::open(shipped) else {
        return BTreeMap::new();
    };
    let prefix = format!("{EAID}{code}/wep/{code}_");
    let present: BTreeSet<&str> = file
        .erp
        .resources
        .iter()
        .filter_map(|r| r.name.strip_prefix(&prefix)?.strip_suffix(".wep"))
        .collect();
    WORLD_FALLBACKS
        .iter()
        .filter(|(v, _)| !present.contains(v))
        .filter_map(|(v, fallbacks)| {
            let from = fallbacks.iter().find(|f| present.contains(**f))?;
            Some(((*v).to_owned(), (*from).to_owned()))
        })
        .collect()
}

/// The worlds of the tracks scanned before, by a hash of each track's files' paths, sizes and
/// modification times and the worlds asked for: a build scans only a track whose files changed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldsCache {
    pub version: u32,
    pub tracks: BTreeMap<String, BTreeMap<String, World>>,
}

/// The cache's path in the data folder.
pub const WORLDS_CACHE: &str = "cache/track-worlds.json";
const CACHE_LIMIT: usize = 512;

struct Job {
    code: String,
    files: Vec<PathBuf>,
    stand_ins: BTreeMap<String, String>,
    key: String,
}

fn job_key(code: &str, files: &[PathBuf], worlds: &BTreeMap<String, String>) -> String {
    let mut h = Sha256::new();
    h.update(format!("{VERSION}|{code}|{worlds:?};"));
    for f in files {
        stamp(&mut h, f);
    }
    crate::baseline::hex(&h.finalize())[..16].to_owned()
}

fn scan_worlds(job: &Job, worlds: &BTreeMap<String, String>) -> BTreeMap<String, World> {
    let opened: Vec<ErpFile> = job.files.iter().filter_map(|p| ErpFile::open(p).ok()).collect();
    let refs: Vec<&ErpFile> = opened.iter().collect();
    worlds
        .iter()
        .filter_map(|(w, template)| {
            let r = room(&refs, &job.code, &world_eri(template, &job.code))?;
            Some((
                w.clone(),
                World {
                    grid: r.grid_slots,
                    garages: r.garages,
                    pit_boxes: r.pit_boxes,
                    stand_in: job.stand_ins.get(w).cloned(),
                },
            ))
        })
        .collect()
}

/// The worlds of every job, from `cache` when its key is there, else scanned, several tracks at
/// once. `cache` gets the scanned ones.
fn all_worlds(
    jobs: &[Job],
    worlds: &BTreeMap<String, String>,
    cache: &mut WorldsCache,
) -> BTreeMap<String, BTreeMap<String, World>> {
    let todo: Vec<&Job> = jobs
        .iter()
        .filter(|j| !cache.tracks.contains_key(&j.key))
        .collect();
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(1, 8);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let found = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads.min(todo.len()) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(job) = todo.get(i) else {
                        break;
                    };
                    let w = scan_worlds(job, worlds);
                    found
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push((job.key.clone(), w));
                }
            });
        }
    });
    let found = found.into_inner().unwrap_or_else(|e| e.into_inner());
    if cache.tracks.len() + found.len() > CACHE_LIMIT {
        let keep: BTreeSet<&str> = jobs.iter().map(|j| j.key.as_str()).collect();
        cache.tracks.retain(|k, _| keep.contains(k.as_str()));
    }
    cache.version = VERSION;
    cache.tracks.extend(found);
    jobs.iter()
        .map(|j| {
            (
                j.code.clone(),
                cache.tracks.get(&j.key).cloned().unwrap_or_default(),
            )
        })
        .collect()
}

/// The facts for the build `scan` describes, or `None` when `previous` holds the same inputs.
pub fn facts(scan: &Scan, previous: Option<&Facts>, cache: &mut WorldsCache) -> Result<Option<Facts>> {
    let bytes = std::fs::read(&scan.db).map_err(crate::io_err(&scan.db))?;
    let mut gvdb = Gvdb::parse(&bytes)?;
    let mut hasher = Sha256::new();
    if let Some(edits) = &scan.edits
        && let Ok(assets) = std::fs::read(edits)
    {
        let n = with_edits(&mut gvdb, &assets);
        hasher.update(format!("edits {n};"));
        stamp(&mut hasher, edits);
    }
    let db = Keyed::new(&gvdb);
    let grid_size = grid::grid_size(&gvdb);
    let rows = layer_rows(&db);
    let worlds = world_templates(&rows);
    let layers: Vec<Layer> = rows.into_iter().map(|(l, _)| l).collect();
    let codes = track_codes(&db);
    hasher.update(format!("grid {grid_size};"));
    stamp(&mut hasher, &scan.db);
    let mut jobs: Vec<Job> = Vec::new();
    let mut rows: BTreeMap<String, usize> = BTreeMap::new();
    for (row, code) in &codes {
        if scan.added.iter().any(|a| a.code == *code) {
            continue;
        }
        let common = (scan.file)(&stock_world_file(code, "common"));
        if !common.is_file() {
            continue;
        }
        let mut files = vec![common];
        for template in worlds.values() {
            let Some(at) = world_file(&world_eri(template, code)) else {
                continue;
            };
            let p = (scan.file)(&at);
            if p.is_file() && !files.contains(&p) {
                files.push(p);
            }
        }
        for p in &files {
            stamp(&mut hasher, p);
        }
        rows.insert(code.clone(), *row);
        jobs.push(Job {
            key: job_key(code, &files, &worlds),
            code: code.clone(),
            files,
            stand_ins: BTreeMap::new(),
        });
    }
    let mut ai: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for job in &jobs {
        let mut found = ai_on_disk(&scan.game.join(TRACKS_DIR).join(&job.code).join("ai"));
        found.extend(ai_placed(&job.code, &scan.placed));
        hasher.update(format!("{} ai {found:?};", job.code));
        ai.insert(job.code.clone(), found);
    }
    for a in &scan.added {
        stamp(&mut hasher, &a.shipped);
        stamp(&mut hasher, &a.placed);
        hasher.update(format!("{} ui {:?};", a.code, a.ui_lists));
        let placed_ai = ai_placed(&a.code, &scan.placed);
        hasher.update(format!("{} ai {placed_ai:?};", a.code));
        let row = codes
            .iter()
            .find(|(_, c)| *c == a.code)
            .or_else(|| codes.iter().find(|(_, c)| Some(c) == a.base.as_ref()));
        let Some((row, _)) = row else {
            continue;
        };
        let Ok(placed) = ErpFile::open(&a.placed) else {
            continue;
        };
        let mut found = ai_of_names(&a.code, placed.erp.resources.iter().map(|r| r.name.as_str()));
        found.extend(placed_ai);
        ai.insert(a.code.clone(), found);
        rows.insert(a.code.clone(), *row);
        let files = vec![a.placed.clone(), a.shipped.clone()];
        jobs.push(Job {
            key: job_key(&a.code, &files, &worlds),
            code: a.code.clone(),
            files: vec![a.placed.clone()],
            stand_ins: stand_ins(&a.code, &a.shipped),
        });
    }
    let inputs = crate::baseline::hex(&hasher.finalize())[..16].to_owned();
    if previous.is_some_and(|p| p.version == VERSION && p.profile == scan.profile && p.inputs == inputs) {
        return Ok(None);
    }
    let folders = package_folders(&db);
    let scanned = all_worlds(&jobs, &worlds, cache);
    let tracks = scanned
        .into_iter()
        .map(|(code, worlds)| {
            let no_world = worlds.is_empty();
            let t = TrackFacts {
                worlds,
                ai: ai.remove(&code).unwrap_or_default().into_iter().collect(),
                ai_difficulties: rows
                    .get(&code)
                    .map(|row| difficulties(&db, *row, &folders))
                    .unwrap_or_default(),
                time_trial: !no_world,
                time_trial_why: no_world.then(|| NO_WORLD.to_owned()),
                ui_lists: scan
                    .added
                    .iter()
                    .find(|a| a.code == code)
                    .map(|a| a.ui_lists.clone())
                    .unwrap_or_default(),
            };
            (code, t)
        })
        .collect();
    Ok(Some(Facts {
        version: VERSION,
        profile: scan.profile.to_owned(),
        inputs,
        grid_size,
        layers,
        tracks,
    }))
}

pub fn path(data: &Path) -> PathBuf {
    crate::stdlib::reports::path(data, FILE)
}

pub fn read(data: &Path) -> Option<Facts> {
    crate::read_json(&path(data)).ok()
}

/// Writes `$DATA/registry/track-facts.json` for the build `scan` describes, unless the file there
/// already holds facts for the same inputs. Returns whether it wrote.
pub fn write(data: &Path, scan: &Scan) -> Result<bool> {
    let previous = read(data);
    let cache_path = data.join(WORLDS_CACHE);
    let mut cache: WorldsCache = crate::read_json(&cache_path).unwrap_or_default();
    let before = cache.clone();
    let made = facts(scan, previous.as_ref(), &mut cache)?;
    if cache != before {
        crate::write_json(&cache_path, &cache)?;
    }
    match made {
        Some(f) => {
            crate::write_json(&path(data), &f)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swofty_formats::erp::{Erp, Fragment, HEADER_LEN, MAGIC, Resource, STORED};
    use swofty_formats::pkg::{Arr, Obj, Val};

    fn pkg_bytes(name: &[u8; 4], root: Node) -> Vec<u8> {
        Pkg {
            name: *name,
            root,
            vca: Vec::new(),
            layout: None,
        }
        .to_bytes()
        .unwrap()
    }

    fn wep(layers: &[&str]) -> Vec<u8> {
        let list = layers.iter().map(|l| Node::Val(Val::str(*l))).collect();
        pkg_bytes(
            b"WEP ",
            Node::Obj(Obj::new(vec![(Val::str("layers"), Node::Arr(Arr::new(list)))])),
        )
    }

    fn archive(resources: Vec<(String, Vec<u8>)>) -> Vec<u8> {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        header[4..8].copy_from_slice(&3u32.to_le_bytes());
        Erp {
            header,
            version: 3,
            resources: resources
                .into_iter()
                .map(|(name, data)| Resource {
                    name,
                    kind: swofty_formats::erp::kind("WEP").unwrap(),
                    unk: vec![0; 4],
                    fragments: vec![Fragment {
                        name: *b"temp",
                        offset: 0,
                        size: data.len() as u64,
                        flags: 0x10,
                        compression: STORED,
                        packed: data.len() as u64,
                        raw: data,
                    }],
                    hash: [0; 16],
                })
                .collect(),
        }
        .to_bytes()
    }

    fn worlds(code: &str, variants: &[&str]) -> Vec<(String, Vec<u8>)> {
        let [common, myteam]: [Node; 2] = super::grid::tests::world().try_into().unwrap();
        let layer = |n: &str| format!("{EAID}{code}/layers/{n}.layer");
        let mut out = vec![
            (wep_name(code, "common"), wep(&[&layer("common")])),
            (layer("common"), pkg_bytes(b"INST", common)),
            (layer("myteam"), pkg_bytes(b"INST", myteam)),
        ];
        for v in variants {
            let layers: Vec<String> = match v.ends_with("myteam") {
                true => vec![layer("myteam")],
                false => Vec::new(),
            };
            let refs: Vec<&str> = layers.iter().map(String::as_str).collect();
            out.push((wep_name(code, v), wep(&refs)));
        }
        out
    }

    fn wep_name(code: &str, v: &str) -> String {
        super::wep(code, v)
    }

    fn database() -> Gvdb {
        use swofty_formats::gvdb::Record;
        let mut r = vec![
            Record::new("track_data.track_layers._TrackLayer_instancecount_", 0, "3"),
            Record::new("tracks._Track_instancecount_", 0, "3"),
            Record::new(
                "entrants.entrant_lists.entrant_list_34._entrant_instancecount_",
                0,
                "22",
            ),
            Record::new(
                "entrants.vehicles.vehicle_packages._vehicle_package_instancecount_",
                0,
                "1",
            ),
            Record::new(
                "entrants.vehicles.vehicle_packages.vehicle_package_0.ai_package_folder",
                2,
                "f1_1990_vehicle_package",
            ),
        ];
        for (i, (label, modes, invalid, world)) in [
            ("f1_26", "f1_26,f1a_mp", "False", "f1_26"),
            ("myteam_26", "", "False", "f1_26_myteam"),
            ("gone", "f1", "True", "f1"),
        ]
        .into_iter()
        .enumerate()
        {
            let row = format!("{LAYERS}_{i}");
            r.push(Record::new(&format!("{row}._metadata_"), 2, label));
            r.push(Record::new(&format!("{row}.game_setup_modes"), 2, modes));
            r.push(Record::new(&format!("{row}.is_invalid"), 2, invalid));
            r.push(Record::new(&format!("{row}.seasons"), 2, "season_2026"));
            r.push(Record::new(
                &format!("{row}.weekend_formats"),
                2,
                "default,sprint",
            ));
            r.push(Record::new(
                &format!("{row}.worldEri"),
                2,
                format!("eaid://environment_package/tracks/[FILENAME]/wep/[FILENAME]_{world}.wep"),
            ));
        }
        for (i, code) in ["melbourne", "added", "sochi"].into_iter().enumerate() {
            r.push(Record::new(&format!("{TRACKS}_{i}._metadata_"), 2, code));
        }
        r.push(Record::new(
            &format!("{TRACKS}_0.ai_track_difficulties.f1_2025_vehicle_package.time"),
            2,
            "80,79",
        ));
        r.push(Record::new(
            &format!("{TRACKS}_1.ai_track_difficulties.f1_1990_vehicle_package.difficulty_ratio"),
            2,
            "1,1",
        ));
        Gvdb { records: r }
    }

    #[test]
    fn scans_stock_worlds_added_archives_ai_folders_and_difficulties() {
        let root = std::env::temp_dir().join(format!("swofty-track-facts-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let game = root.join("game");
        let melbourne = game.join(TRACKS_DIR).join("melbourne");
        std::fs::create_dir_all(melbourne.join("wep")).unwrap();
        std::fs::create_dir_all(melbourne.join("ai/f1_2025_vehicle_package/ferrari")).unwrap();
        std::fs::create_dir_all(melbourne.join("ai/f1_2025_vehicle_package/mclaren")).unwrap();
        let all = worlds("melbourne", &["f1_26", "f1_26_myteam"]);
        let (common, variants) = all.split_at(3);
        std::fs::write(
            melbourne.join("wep/melbourne_common.erp"),
            archive(common.to_vec()),
        )
        .unwrap();
        for (name, data) in variants {
            let v = name.rsplit_once("melbourne_").unwrap().1.trim_end_matches(".wep");
            std::fs::write(
                melbourne.join(format!("wep/melbourne_{v}.erp")),
                archive(vec![(name.clone(), data.clone())]),
            )
            .unwrap();
        }
        let db_path = root.join("built.bin");
        std::fs::write(&db_path, database().to_bytes()).unwrap();
        let shipped = root.join("added/added.erp");
        std::fs::create_dir_all(shipped.parent().unwrap()).unwrap();
        std::fs::write(&shipped, archive(worlds("added", &["f1"]))).unwrap();
        let placed = root.join("placed/added.erp");
        std::fs::create_dir_all(placed.parent().unwrap()).unwrap();
        let mut res = worlds("added", &["f1", "f1_26", "f1_26_myteam"]);
        res.push((
            format!("{EAID}added/ai/f1_2025_vehicle_package/ferrari/line.aispline"),
            vec![1],
        ));
        std::fs::write(&placed, archive(res)).unwrap();
        let file = |rel: &str| game.join(rel);
        let scan = Scan {
            profile: "Classic Seasons",
            db: db_path.clone(),
            game: &game,
            file: &file,
            placed: vec![
                format!("{TRACKS_DIR}/melbourne/ai/f1_1990_vehicle_package/f1_1990_car/line.aispline"),
                format!("{TRACKS_DIR}/added/ai/f1_1990_vehicle_package/ferrari/ferrari.erp"),
            ],
            added: vec![
                Added {
                    code: "added".into(),
                    shipped: shipped.clone(),
                    placed: placed.clone(),
                    base: None,
                    ui_lists: Vec::new(),
                },
                Added {
                    code: "record".into(),
                    shipped: shipped.clone(),
                    placed: placed.clone(),
                    base: Some("melbourne".into()),
                    ui_lists: vec!["track_select".into()],
                },
            ],
            edits: None,
        };
        let f = facts(&scan, None, &mut WorldsCache::default()).unwrap().unwrap();
        assert_eq!(
            (f.version, f.profile.as_str(), f.grid_size),
            (1, "Classic Seasons", 22)
        );
        assert_eq!(f.inputs.len(), 16);
        assert_eq!(
            f.layers,
            [
                Layer {
                    label: "f1_26".into(),
                    setup_modes: vec!["f1_26".into(), "f1a_mp".into()],
                    seasons: vec!["season_2026".into()],
                    weekend_formats: vec!["default".into(), "sprint".into()],
                    world: "f1_26".into(),
                },
                Layer {
                    label: "myteam_26".into(),
                    setup_modes: Vec::new(),
                    seasons: vec!["season_2026".into()],
                    weekend_formats: vec!["default".into(), "sprint".into()],
                    world: "f1_26_myteam".into(),
                },
            ]
        );
        assert_eq!(
            f.tracks.keys().collect::<Vec<_>>(),
            ["added", "melbourne", "record"]
        );
        assert_eq!(f.tracks["record"].ai_difficulties, ["f1_2025_vehicle_package"]);
        assert_eq!(f.tracks["record"].ui_lists, ["track_select"]);
        assert!(f.tracks["added"].ui_lists.is_empty());
        assert!(
            f.tracks["added"].time_trial,
            "a track made for 1.5 runs Time Trial"
        );
        for code in ["added", "melbourne"] {
            assert!(f.tracks[code].time_trial && f.tracks[code].time_trial_why.is_none());
        }
        let bare = &f.tracks["record"];
        assert!(
            bare.worlds.is_empty() && bare.ai.is_empty(),
            "its archive holds another code's files"
        );
        assert_eq!(
            (bare.time_trial, bare.time_trial_why.as_deref()),
            (false, Some(NO_WORLD))
        );
        let json = serde_json::to_value(&f).unwrap();
        assert_eq!(json["tracks"]["added"]["time_trial"], true);
        assert!(json["tracks"]["added"].get("time_trial_why").is_none());
        let old: TrackFacts =
            serde_json::from_str(r#"{ "worlds": {}, "ai": [], "ai_difficulties": [] }"#).unwrap();
        assert!(old.time_trial);
        let room = |grid, garages, pit_boxes, stand_in: Option<&str>| World {
            grid,
            garages,
            pit_boxes,
            stand_in: stand_in.map(str::to_owned),
        };
        let m = &f.tracks["melbourne"];
        assert_eq!(m.worlds["f1_26"], room(22, 11, 11, None));
        assert_eq!(m.worlds["f1_26_myteam"], room(24, 12, 12, None));
        assert_eq!(
            m.ai,
            [
                "f1_1990_vehicle_package/f1_1990_car",
                "f1_2025_vehicle_package/ferrari",
                "f1_2025_vehicle_package/mclaren"
            ]
        );
        assert_eq!(m.ai_difficulties, ["f1_2025_vehicle_package"]);
        let a = &f.tracks["added"];
        assert_eq!(a.worlds["f1_26"], room(22, 11, 11, Some("f1")));
        assert_eq!(a.worlds["f1_26_myteam"], room(24, 12, 12, Some("f1")));
        assert_eq!(
            a.ai,
            [
                "f1_1990_vehicle_package/ferrari",
                "f1_2025_vehicle_package/ferrari"
            ],
            "an added track lists the ai of its archive and the ai addons place for it"
        );
        assert_eq!(a.ai_difficulties, ["f1_1990_vehicle_package"]);
        let text = serde_json::to_value(&f).unwrap();
        assert!(
            text["tracks"]["melbourne"]["worlds"]["f1_26"]
                .get("stand_in")
                .is_none()
        );

        let data = root.join("data");
        assert!(write(&data, &scan).unwrap());
        assert!(!write(&data, &scan).unwrap(), "the same inputs keep the file");
        assert_eq!(read(&data).unwrap(), f);
        let other = Scan {
            profile: "default",
            ..scan
        };
        assert!(write(&data, &other).unwrap(), "another profile writes again");
        std::fs::write(&db_path, database().to_bytes()).unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(&db_path)
            .unwrap()
            .set_modified(later)
            .unwrap();
        assert!(write(&data, &other).unwrap(), "a changed file writes again");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_runtime_database_edits_are_scanned() {
        let mut gvdb = database();
        let key = |path: &str| format!("{:016x}", fnv1a64(path));
        let row = format!("{LAYERS}_3");
        let edit = |path: &str, tag: u8, value: &str| serde_json::json!({ "hash": key(path), "type": tag, "value": value });
        let assets = serde_json::json!({ "early_stage": true, "db_edits": [
            edit("track_data.track_layers._TrackLayer_instancecount_", 0, "4"),
            edit(&format!("{row}._metadata_"), 2, "swofty_f1"),
            edit(&format!("{row}.game_setup_modes"), 2, "swofty_f1"),
            edit(&format!("{row}.is_invalid"), 2, "False"),
            edit(&format!("{row}.seasons"), 2, "season_2025"),
            edit(&format!("{row}.worldEri"), 2, "eaid://environment_package/tracks/[FILENAME]/wep/[FILENAME]_f1_26.wep"),
            edit(&format!("{LAYERS}_0.game_setup_modes"), 2, "f1_26"),
        ] });
        let applied = with_edits(&mut gvdb, &serde_json::to_vec(&assets).unwrap());
        assert_eq!(applied, 7);
        let db = Keyed::new(&gvdb);
        let found: Vec<Layer> = layer_rows(&db).into_iter().map(|(l, _)| l).collect();
        assert_eq!(
            found
                .iter()
                .map(|l| (l.label.as_str(), l.setup_modes.join(","), l.world.as_str()))
                .collect::<Vec<_>>(),
            [
                ("f1_26", "f1_26".to_owned(), "f1_26"),
                ("myteam_26", String::new(), "f1_26_myteam"),
                ("swofty_f1", "swofty_f1".to_owned(), "f1_26"),
            ],
            "an edited row and a row the edits add are read as the game reads them"
        );
        assert_eq!(with_edits(&mut gvdb, b"not json"), 0);
        assert_eq!(with_edits(&mut gvdb, b"{}"), 0);
    }

    #[test]
    fn a_layer_world_in_an_addon_package_is_read_at_its_own_path() {
        let template = "eaid://bdx_s18_environment_package/tracks/[FILENAME]/wep/[FILENAME]_2012.wep";
        let eri = world_eri(template, "monza");
        assert_eq!(
            eri,
            "eaid://bdx_s18_environment_package/tracks/monza/wep/monza_2012.wep"
        );
        assert_eq!(
            world_file(&eri).as_deref(),
            Some("2025_asset_groups/bdx_s18_environment_package/tracks/monza/wep/monza_2012.erp")
        );
        assert_eq!(
            world_file(&wep_name("monza", "f1_26")),
            Some(stock_world_file("monza", "f1_26"))
        );
        let mut resources = worlds("monza", &[]);
        resources.push((eri.clone(), wep(&[&format!("{EAID}monza/layers/myteam.layer")])));
        let dir = std::env::temp_dir().join(format!("swofty-addon-world-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let at = dir.join("monza_2012.erp");
        std::fs::write(&at, archive(resources)).unwrap();
        let file = ErpFile::open(&at).unwrap();
        let found = room(&[&file], "monza", &eri).unwrap();
        assert_eq!((found.grid_slots, found.garages, found.pit_boxes), (24, 12, 12));
        assert_eq!(room(&[&file], "monza", &wep_name("monza", "2012")), None);
        let rows = vec![
            (
                Layer {
                    world: "2012".into(),
                    ..Layer::default()
                },
                template.to_owned(),
            ),
            (
                Layer {
                    world: "2012".into(),
                    ..Layer::default()
                },
                "eaid://other/tracks/[FILENAME]/wep/[FILENAME]_2012.wep".to_owned(),
            ),
        ];
        assert_eq!(world_templates(&rows)["2012"], template);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_world_missing_from_every_file_has_no_facts() {
        let file = ErpFile {
            path: PathBuf::new(),
            erp: Erp {
                header: [0; HEADER_LEN],
                version: 3,
                resources: Vec::new(),
            },
            data_off: 0,
        };
        assert_eq!(room(&[&file], "t", &wep_name("t", "f1")), None);
    }
}
