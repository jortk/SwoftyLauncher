use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use swofty_formats::gvdb::Gvdb;

use crate::activation::db::{Registry, TrackHost, activate_with};
use crate::activation::ers::{self, ErsMap, build_ers};
use crate::activation::grid::grid_size;
use crate::activation::handling::{self, HandlingMap};
use crate::activation::place::{
    PlaceInput, SLOTS, ai_teams, layer_worlds, missing_worlds, place, room_for, slot_asset_group,
    slot_asset_root, slot_file, track_group,
};
use crate::activation::ui::{Kept, MapUi, build_ui, plan_lists, stamp as ui_stamp};
use crate::asset::{
    AssetOp, FileKind, FileOp, GroupOp, ItemOp, LangOp, Op, OpFile, Origin, Root, Source, TrackOp,
};
use crate::baseline::{Baseline, Objects, hex, sha256_file, sha256_opt};
use crate::builtin::{
    CORE_DB, CORE_ID, CORE_INTRO, LOADER_ENV, core_db_ops, loader_dir, loader_files, stage_intro,
};
use crate::conflicts::Report;
use crate::dbedit::driver_manager;
use crate::erpbuild::{ErpBuild, Held, Resized, SliceMerge, reads, unsupported, writes};
use crate::install::{Game, LOADER_DIR, Plan};
use crate::launcher::BuildCx;
use crate::manifest::{
    AddonManifest, Binds, OptionType, PassValue, PostFx, Strength, placeholders, relative_inside,
};
use crate::map::{TrackPackage, Unusable};
use crate::multiplayer::fingerprint::{self, OutputBuilder};
use crate::owned;
use crate::placed::Placed;
use crate::planner::{
    Conflict, Features, Kind, Planner, Route, Target, kept_lists_line, load_features, stock_removals_line,
    track_removals_line, waiting_lines,
};
use crate::profile::Profile;
use crate::registry::Host;
use crate::resolver::resolve;
use crate::runtime::{
    PostFxLive, PostFxPass, PostFxTexture, PostFxWhen, Prebuilt, Runtime, db_edits, overlay_fits,
    runtime_loaded_estimate, runtime_record_room,
};
use crate::settings::{
    OnAddons, SessionMod, SessionSettings, SessionUi, render_ops, render_text, screen_file, screens,
};
use crate::store::Store;
use crate::ui::{ERPS as MENU_ERPS, READS as MENU_READS, ui_file};
use crate::{Error, Result, io_err};

pub const GVDB: &str = "game_package/gamedatabases/gamedatabases.2025.bin";
pub const PLAN_FILE: &str = "plan.json";
pub const SAVED_FILE: &str = "built.json";
const SAVED_VERSION: u32 = 3;
const PACKED_AGE: std::time::Duration = std::time::Duration::from_secs(30 * 24 * 3600);
const ARCHIVE_AGE: std::time::Duration = std::time::Duration::from_secs(3 * 24 * 3600);
const ARCHIVE_BYTES: u64 = 10 << 30;
pub const ADDONS_DIR: &str = "addons";
const LOCK_FILE: &str = "build.lock";

#[derive(Debug, Clone, Default)]
pub struct Built {
    pub buildhash: String,
    pub overlay: PathBuf,
    pub order: Vec<String>,
    pub plan: Plan,
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
    /// Every side-data finding of the add-row copies, tagged; see [`crate::dbedit::deps::Report`].
    pub side_data: Vec<String>,
    pub settings: Option<SessionSettings>,
    pub conflicts: Vec<Conflict>,
    pub waiting: Vec<Op>,
    pub report: Report,
    /// Each addon's added rows with a stat id, its `track.add` track rows among them, as `<table>.<row>/<label>` to
    /// the id the build gave.
    pub row_ids: BTreeMap<String, BTreeMap<String, i64>>,
    /// The track addons left out because the game's track lists are full.
    pub track_limit: Option<crate::tracks::TrackLimit>,
    /// The build restored a saved build with the same hash instead of building again.
    pub reused: bool,
}

/// The recorded originals of the last baseline read, by its path and when it
/// was written: a build looks originals up dozens of times.
type Originals = (
    PathBuf,
    (std::time::SystemTime, u64),
    std::sync::Arc<BTreeMap<String, String>>,
);
static ORIGINALS: std::sync::Mutex<Option<Originals>> = std::sync::Mutex::new(None);

fn originals(data: &Path, game: &Game) -> std::sync::Arc<BTreeMap<String, String>> {
    let path = Baseline::path(data, &game.identity);
    let stamp = std::fs::metadata(&path)
        .and_then(|m| Ok((m.modified()?, m.len())))
        .ok();
    let mut cached = ORIGINALS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((p, s, files)) = cached.as_ref()
        && *p == path
        && Some(*s) == stamp
    {
        return files.clone();
    }
    let files: std::sync::Arc<BTreeMap<String, String>> = std::sync::Arc::new(
        Baseline::load_or_new(data, &game.identity)
            .map(|b| b.files.into_iter().map(|(rel, rec)| (rel, rec.sha256)).collect())
            .unwrap_or_default(),
    );
    *cached = stamp.map(|stamp| (path, stamp, files.clone()));
    files
}

pub fn original(data: &Path, game: &Game, rel: &str) -> PathBuf {
    match originals(data, game).get(rel) {
        Some(sha256) => Objects {
            root: data.join("baseline/objects"),
        }
        .path(sha256),
        None => game.dir.join(rel),
    }
}

/// Stop a build whose record of original game files names backups that are
/// gone, and send the player through the setup again, which starts the record
/// over.
/// The highest stat id the stock game database gives each team, vehicle and driver table
/// (`row_ids::stock_max_ids`). Empty when the database can't be read.
pub fn stock_max_ids(data: &Path, game: &Game) -> BTreeMap<String, i64> {
    std::fs::read(original(data, game, GVDB))
        .ok()
        .and_then(|b| Gvdb::parse(&b).ok())
        .map(|db| crate::row_ids::stock_max_ids(&db))
        .unwrap_or_default()
}

fn check_originals(data: &Path, game: &Game) -> Result<()> {
    let objects = Objects {
        root: data.join("baseline/objects"),
    };
    let missing: Vec<String> = originals(data, game)
        .iter()
        .filter(|(_, sha256)| !objects.has(sha256))
        .map(|(rel, _)| rel.clone())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    crate::setup::redo(data)?;
    Err(Error::Other(crate::setup::backups_missing(&missing)))
}

pub type TrackPackages = (Vec<(PathBuf, TrackPackage, Origin)>, Vec<(Origin, Unusable)>);

/// The packages `track.add` ops add, and the ones a build leaves out.
pub fn track_packages(store: &Store, ops: Vec<Op>) -> Result<TrackPackages> {
    let mut out: Vec<(PathBuf, TrackPackage, Origin)> = Vec::new();
    let mut left_out = Vec::new();
    for op in ops {
        let AssetOp::Track(TrackOp::Add { source }) = op.op else {
            continue;
        };
        let dir = source.resolve(&source_dir(store, &op.origin.addon, source.root))?;
        let pkg = match crate::map::read_package(&dir) {
            Ok(pkg) => pkg,
            Err(why) => {
                left_out.push((op.origin, why));
                continue;
            }
        };
        if let Some((_, _, first)) = out.iter().find(|(_, p, _)| p.track_code == pkg.track_code) {
            return Err(Error::Map(match first.addon == op.origin.addon {
                true => format!("track {} is added twice by {}", pkg.track_code, first.addon),
                false => format!(
                    "track {} is added by both {} and {}; turn one of them off on the Addons page",
                    pkg.track_code, first.addon, op.origin.addon
                ),
            }));
        }
        out.push((dir, pkg, op.origin));
    }
    Ok((out, left_out))
}

// Page screens have no file: the ui build generates them.
pub fn screen_files(order: &[&AddonManifest], addons_dir: &Path) -> Result<BTreeMap<String, PathBuf>> {
    Ok(screens(order.iter().copied())?
        .into_iter()
        .filter(|(_, s)| s.page.is_none())
        .map(|(m, s)| {
            (
                format!("{LOADER_DIR}/{}", screen_file(&s.name)),
                addons_dir.join(&m.id).join(&s.file),
            )
        })
        .collect())
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plugins {
    pub files: BTreeMap<String, PathBuf>,
    pub dlls: Vec<String>,
}

pub fn plugin_files<'a>(
    store: &Store,
    order: impl IntoIterator<Item = &'a AddonManifest>,
    taken: impl IntoIterator<Item = String>,
) -> Result<Plugins> {
    let mut out = Plugins::default();
    let mut names: BTreeMap<String, String> = taken
        .into_iter()
        .map(|n| (n.to_ascii_lowercase(), CORE_ID.to_owned()))
        .collect();
    for m in order
        .into_iter()
        .filter(|m| !m.operations.native.is_empty() && !store.is_builtin(&m.id))
    {
        let dir = store.addons_dir().join(&m.id);
        for n in &m.operations.native {
            let name = n.dll.rsplit('/').next().unwrap_or(&n.dll);
            if let Some(other) = names.insert(name.to_ascii_lowercase(), m.id.clone()) {
                return Err(Error::Other(format!(
                    "{}: plugin {name} has the same file name as a plugin of {other}; rename one",
                    m.id
                )));
            }
            out.dlls.push(format!("{ADDONS_DIR}/{}/{}", m.id, n.dll));
            for rel in std::iter::once(&n.dll).chain(&n.files) {
                let rel = rel.trim_end_matches('/');
                let src = dir.join(rel);
                let found = if src.is_dir() {
                    loader_files(&src)?
                        .into_iter()
                        .filter(|(r, _)| !r.split('/').any(|p| p.starts_with('.')))
                        .map(|(r, p)| (format!("{rel}/{r}"), p))
                        .collect()
                } else if src.is_file() {
                    BTreeMap::from([(rel.to_owned(), src)])
                } else {
                    return Err(Error::Other(format!(
                        "{}: {rel} is missing; build the plugin and install the addon again",
                        m.id
                    )));
                };
                for (rel, src) in found {
                    out.files
                        .insert(format!("{LOADER_DIR}/{ADDONS_DIR}/{}/{rel}", m.id), src);
                }
            }
        }
    }
    Ok(out)
}

pub fn source_dir(store: &Store, addon: &str, root: Root) -> PathBuf {
    match root {
        Root::Addon => store.addons_dir().join(addon),
        Root::Data => store.addon_data_dir(addon),
        Root::Cache => store.addon_cache_dir(addon),
    }
}

/// The `.cube` a shader pack's settings pick, and its strength from 0 to 100.
fn shader_look(m: &AddonManifest, p: &Profile, on: &OnAddons) -> Result<(String, f64)> {
    let s = m.shader.as_ref().expect("a shader pack");
    let rel = render_text(&s.lut, m, p, on)?;
    if !relative_inside(&rel) {
        return Err(Error::Other(format!(
            "{}: shader lut {rel} must be a relative path inside the addon",
            m.name
        )));
    }
    let strength = match &s.strength {
        None => 100.0,
        Some(Strength::Percent(v)) => *v,
        Some(Strength::Option(t)) => render_text(t, m, p, on)?
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|v| (0.0..=100.0).contains(v))
            .ok_or_else(|| Error::Other(format!("{}: shader strength {t} is not from 0 to 100", m.name)))?,
    };
    Ok((rel, strength))
}

fn texture_name(m: &AddonManifest, name: &str) -> String {
    format!("{}/{name}", m.id)
}

fn option_id(t: &str) -> Option<&str> {
    match placeholders(t)[..] {
        [id] => Some(id),
        _ => None,
    }
}

/// The slots of a pass that bind to native options, which the core reads live.
fn pass_live(m: &AddonManifest, pass: &PostFx) -> Vec<PostFxLive> {
    pass.options
        .iter()
        .enumerate()
        .filter_map(|(slot, v)| {
            let PassValue::Option(t) = v else { return None };
            let id = option_id(t)?;
            m.options
                .iter()
                .any(|o| o.id == id && o.binds == Binds::Native)
                .then(|| PostFxLive {
                    slot,
                    option: id.into(),
                })
        })
        .collect()
}

/// `None` when the pass is left out; otherwise the live toggle that gates it, if any.
fn pass_enabled(
    m: &AddonManifest,
    pass: &PostFx,
    p: &Profile,
    on: &OnAddons,
) -> Result<Option<Option<String>>> {
    let Some(t) = &pass.enabled else {
        return Ok(Some(None));
    };
    let id = option_id(t)
        .ok_or_else(|| Error::Other(format!("{}: postfx enabled {t} is not one option", m.name)))?;
    if m.options.iter().any(|o| o.id == id && o.binds == Binds::Native) {
        return Ok(Some(Some(id.into())));
    }
    Ok(match render_text(t, m, p, on)?.trim() {
        "true" => Some(None),
        _ => None,
    })
}

/// The values a pass reads with `option(i)`, with `{{id}}` options filled from the profile's settings.
fn pass_options(m: &AddonManifest, pass: &PostFx, p: &Profile, on: &OnAddons) -> Result<Vec<f64>> {
    pass.options
        .iter()
        .map(|v| match v {
            PassValue::Number(n) => Ok(*n),
            PassValue::Option(t) => {
                let text = render_text(t, m, p, on)?;
                match text.trim() {
                    "true" => Ok(1.0),
                    "false" => Ok(0.0),
                    v => v.parse::<f64>().map_err(|_| {
                        Error::Other(format!("{}: postfx option {t} is {v:?}, not a number", m.name))
                    }),
                }
            }
        })
        .collect()
}

fn render_file(
    path: &Path,
    kind: FileKind,
    m: &AddonManifest,
    p: &Profile,
    on: &OnAddons,
    hasher: &mut Sha256,
) -> Result<OpFile> {
    let doc = render_doc(path, m, p, on)?;
    hasher.update(serde_json::to_vec(&doc).expect("serialisable"));
    OpFile::from_value(doc, kind).map_err(json_at(path))
}

/// The core's startup intro ops, empty while the `startup_intro` setting is off. The
/// video is staged in the core's cache folder, which the `file.put` reads.
fn core_intro(
    store: &Store,
    m: &AddonManifest,
    p: &Profile,
    on: &OnAddons,
    hasher: &mut Sha256,
) -> Result<OpFile> {
    let mut doc = serde_json::from_str(CORE_INTRO).expect("the built-in intro ops are JSON");
    render_ops(&mut doc, m, p, on)?;
    let file = OpFile::from_value(doc, FileKind::Assets).expect("the built-in intro ops are valid");
    hasher.update(serde_json::to_vec(&file).expect("serialisable"));
    if !file.ops.is_empty() {
        stage_intro(&store.addon_cache_dir(CORE_ID))?;
    }
    Ok(file)
}

/// One of an addon's op files as a build reads it, with its option
/// templating filled from the profile's settings and the `when`s checked against the addons on.
pub fn read_op_file(
    path: &Path,
    kind: FileKind,
    m: &AddonManifest,
    p: &Profile,
    on: &OnAddons,
) -> Result<OpFile> {
    OpFile::from_value(render_doc(path, m, p, on)?, kind).map_err(json_at(path))
}

fn render_doc(path: &Path, m: &AddonManifest, p: &Profile, on: &OnAddons) -> Result<serde_json::Value> {
    let raw = std::fs::read(path).map_err(io_err(path))?;
    let mut doc = serde_json::from_slice(&raw).map_err(json_at(path))?;
    render_ops(&mut doc, m, p, on)?;
    Ok(doc)
}

fn json_at(path: &Path) -> impl FnOnce(serde_json::Error) -> Error + '_ {
    move |source| Error::Json {
        path: path.to_owned(),
        source,
    }
}

/// A DB Editor package's ops, generated from its `dbedit.json` rather than
/// read from its `database.json`, and the Driver Manager drivers its values pick.
fn package_ops(store: &Store, id: &str) -> Result<(OpFile, Vec<String>)> {
    let edits = crate::dbedit::package::Package::open(store, id)?.edits;
    let ops = edits.to_ops();
    let file = OpFile {
        schema_version: ops.schema_version,
        fields: ops.fields,
        strict: ops.strict,
        side_data: ops.side_data,
        ops: ops.ops.into_iter().map(AssetOp::Db).collect(),
    };
    Ok((file, driver_manager::picks(&edits)))
}

fn unbuilt<'a>(ops: impl IntoIterator<Item = &'a AssetOp>) -> Option<&'static str> {
    ops.into_iter().find_map(|op| match op {
        AssetOp::Erp(op) => unsupported(op),
        _ => None,
    })
}

fn hash_sources(hasher: &mut Sha256, store: &Store, addon: &str, ops: &[AssetOp]) -> Result<()> {
    // Many resource edits read the same archive or directory. Inspect each source
    // once per batch, but feed every occurrence into the digest in its original order.
    // Keep this local: a helper can change cache/data sources before the next batch.
    let mut stamps = HashMap::<PathBuf, Vec<u8>>::new();
    for src in ops.iter().flat_map(AssetOp::sources) {
        let path = src.resolve(&source_dir(store, addon, src.root))?;
        if !stamps.contains_key(&path) {
            let meta = std::fs::metadata(&path).map_err(io_err(&path))?;
            let mut stamp = format!("{}|{:?}", meta.len(), meta.modified().ok()).into_bytes();
            if meta.is_dir() {
                for (rel, p) in loader_files(&path)? {
                    let meta = std::fs::metadata(&p).map_err(io_err(&p))?;
                    stamp.extend_from_slice(
                        format!("{rel}|{}|{:?}", meta.len(), meta.modified().ok()).as_bytes(),
                    );
                }
            }
            stamps.insert(path.clone(), stamp);
        }
        hasher.update(src.path.as_bytes());
        hasher.update(b"|");
        hasher.update(&stamps[&path]);
    }
    Ok(())
}

fn stamp(hasher: &mut Sha256, label: &str, path: &Path) {
    match std::fs::metadata(path) {
        Ok(meta) => hasher.update(format!("{label}|{}|{:?};", meta.len(), meta.modified().ok())),
        Err(_) => hasher.update(format!("{label}|missing;")),
    }
}

fn stamp_tree(hasher: &mut Sha256, label: &str, path: &Path) -> Result<()> {
    if path.is_dir() {
        for (rel, p) in loader_files(path)? {
            stamp(hasher, &format!("{label}/{rel}"), &p);
        }
    } else {
        stamp(hasher, label, path);
    }
    Ok(())
}

fn hash_unread(
    hasher: &mut Sha256,
    store: &Store,
    game: &Game,
    order: &[&AddonManifest],
    erp_ops: &[Op],
    resolve: &dyn Fn(&str) -> PathBuf,
) -> Result<()> {
    hasher.update(format!("game {};", game.identity.key()));
    hasher.update(format!("pipeline {};", env!("SWOFTY_PIPELINE")));
    if let Some(dir) = loader_dir() {
        stamp(
            hasher,
            "loader features",
            &dir.join(crate::planner::FEATURES_FILE),
        );
    }
    for m in order.iter().filter(|m| !store.is_builtin(&m.id)) {
        let dir = store.addons_dir().join(&m.id);
        for n in &m.operations.native {
            for rel in std::iter::once(&n.dll).chain(&n.files) {
                let rel = rel.trim_end_matches('/');
                stamp_tree(hasher, &format!("{}/{rel}", m.id), &dir.join(rel))?;
            }
        }
    }
    let read: std::collections::BTreeSet<&str> = erp_ops
        .iter()
        .filter_map(|op| match &op.op {
            AssetOp::Erp(e) => Some(reads(e)),
            _ => None,
        })
        .flatten()
        .collect();
    for rel in read {
        stamp(hasher, rel, &resolve(rel));
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Saved {
    version: u32,
    buildhash: String,
    data: BTreeMap<String, PathBuf>,
    added: BTreeMap<String, PathBuf>,
    loader: BTreeMap<String, PathBuf>,
    registry_override: bool,
    sizes: BTreeMap<PathBuf, u64>,
    #[serde(default)]
    stamps: BTreeMap<PathBuf, (u128, u64)>,
    settings: Option<SessionSettings>,
    lines: Vec<String>,
    warnings: Vec<String>,
    conflicts: Vec<Conflict>,
    waiting: Vec<Op>,
    #[serde(default)]
    report: Option<Report>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    row_ids: BTreeMap<String, BTreeMap<String, i64>>,
}

fn output_stamp(meta: &std::fs::Metadata) -> (u128, u64) {
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    #[cfg(unix)]
    let ino = std::os::unix::fs::MetadataExt::ino(meta);
    #[cfg(not(unix))]
    let ino = 0;
    (modified, ino)
}

impl Saved {
    fn of(built: &Built, lines: usize, warnings: usize) -> Result<Self> {
        let mut sizes = BTreeMap::new();
        let mut stamps = BTreeMap::new();
        for src in [&built.plan.data, &built.plan.added]
            .into_iter()
            .flat_map(|m| m.values())
        {
            let meta = std::fs::metadata(src).map_err(io_err(src))?;
            sizes.insert(src.clone(), meta.len());
            stamps.insert(src.clone(), output_stamp(&meta));
        }
        Ok(Self {
            version: SAVED_VERSION,
            buildhash: built.buildhash.clone(),
            data: built.plan.data.clone(),
            added: built.plan.added.clone(),
            loader: built_files(&built.plan.loader, loader_dir().as_deref()),
            registry_override: built.plan.registry_override,
            sizes,
            stamps,
            settings: built.settings.clone(),
            lines: built.lines[lines..].to_vec(),
            warnings: built.warnings[warnings..].to_vec(),
            conflicts: built.conflicts.clone(),
            waiting: built.waiting.clone(),
            report: Some(built.report.clone()),
            row_ids: built.row_ids.clone(),
        })
    }

    fn load(overlay: &Path, buildhash: &str) -> Option<Self> {
        let saved: Self = crate::read_json(&overlay.join(SAVED_FILE)).ok()?;
        (saved.version == SAVED_VERSION && saved.buildhash == buildhash).then_some(saved)
    }

    // The game folder decides whether a file is replaced or added, so a saved
    // plan only holds while each file it adds is still absent and each file it
    // replaces is still a game file.
    fn usable(&self, store: &Store, game: &Game) -> bool {
        let sources = self
            .sizes
            .iter()
            .all(|(p, len)| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() == *len));
        let untouched = self
            .stamps
            .iter()
            .all(|(p, stamp)| std::fs::metadata(p).is_ok_and(|m| output_stamp(&m) == *stamp));
        if !sources
            || !untouched
            || !self.loader.values().all(|p| p.is_file())
            || self
                .added
                .keys()
                .any(|rel| std::fs::symlink_metadata(game.dir.join(rel)).is_ok())
        {
            return false;
        }
        let Ok(baseline) = Baseline::load_or_new(&store.root, &game.identity) else {
            return false;
        };
        self.data
            .keys()
            .all(|rel| baseline.files.contains_key(rel) || game.dir.join(rel).is_file())
    }

    /// The loader's own files come from the launcher's loader folder as it is
    /// now: a launcher update changes them without changing what the build
    /// wrote. The rest of the placed files, such as screens, runtime assets and
    /// addon plugins, are the saved ones.
    fn restore(self, built: &mut Built) -> Result<()> {
        built.plan.data = self.data;
        built.plan.added = self.added;
        built.plan.loader = self.loader;
        if let Some(dir) = loader_dir() {
            built.plan.loader.extend(loader_files(&dir)?);
        }
        built.plan.registry_override = self.registry_override;
        built.settings = self.settings;
        built.lines.push(format!(
            "build: nothing changed since overlay {} was built; reusing it",
            built.buildhash
        ));
        built.lines.extend(self.lines);
        built.warnings.extend(self.warnings);
        built.report = match self.report {
            Some(report) => report,
            None => crate::conflicts::summarise(&built.plan.profile, &self.conflicts, &|_| None).0,
        };
        built.report.built_at = crate::conflicts::now();
        built.conflicts = self.conflicts;
        built.waiting = self.waiting;
        built.row_ids = self.row_ids;
        Ok(())
    }
}

/// The placed files that do not come from the loader folder `loader`.
fn built_files(placed: &BTreeMap<String, PathBuf>, loader: Option<&Path>) -> BTreeMap<String, PathBuf> {
    placed
        .iter()
        .filter(|(_, src)| loader.is_none_or(|dir| !src.starts_with(dir)))
        .map(|(rel, src)| (rel.clone(), src.clone()))
        .collect()
}

/// Held while a build writes a profile's overlay; another build of the
/// profile, in this process or another, waits until it is dropped.
pub struct BuildLock {
    _file: std::fs::File,
}

pub fn lock(store: &Store, profile: &str) -> Result<BuildLock> {
    let dir = store.overlays_dir(profile);
    std::fs::create_dir_all(&dir).map_err(io_err(&dir))?;
    let path = dir.join(LOCK_FILE);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(io_err(&path))?;
    file.lock().map_err(io_err(&path))?;
    Ok(BuildLock { _file: file })
}

pub fn build(store: &Store, profile: &str, game: &Game) -> Result<Built> {
    build_as(store, profile, game, false)
}

/// A build or install that stopped on a missing or cut-short file, as the
/// addon or game file it is and what to do about it. Any other error is
/// returned as it is.
pub fn named(store: &Store, game: &Path, e: Error) -> Error {
    let (Error::Io { path, source } | Error::Formats(swofty_formats::Error::IoAt { path, source })) = &e
    else {
        return e;
    };
    let gone = source.kind() == std::io::ErrorKind::NotFound;
    if !gone && source.kind() != std::io::ErrorKind::UnexpectedEof {
        return e;
    }
    let rel = |rest: &Path| {
        rest.components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    };
    if let Ok(rest) = path.strip_prefix(store.addons_dir())
        && let Some(id) = rest
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
        && let Ok(file) = rest.strip_prefix(&id)
        && !file.as_os_str().is_empty()
    {
        let name = store
            .addons()
            .ok()
            .and_then(|a| a.get(&id).map(|m| m.name.clone()))
            .unwrap_or_else(|| id.clone());
        let file = rel(file);
        let what = match gone {
            true => format!("{name} ({id}) is missing its file {file}"),
            false => format!("{name} ({id}) has an incomplete or damaged {file}"),
        };
        return Error::Other(format!(
            "{what}. Turn {name} off in this profile, or remove it on the Addons page and install or import it again."
        ));
    }
    if let Ok(rest) = path.strip_prefix(game) {
        let file = rel(rest);
        let version = crate::exe_version::read(&game.join(crate::GAME_EXE));
        return Error::Other(match gone {
            true => format!(
                "{file} is missing from the game folder; {}, then try again",
                crate::stock::missing_fix(1, version.as_deref())
            ),
            false => format!(
                "{file} in the game folder is incomplete or damaged; verify the game files in Steam (open {}) \
                 or repair the game in the EA app, then try again",
                crate::stock::STEAM_VERIFY_URL
            ),
        });
    }
    e
}

/// What to do when Windows refused Swofty a file in the game folder `game` or
/// in its data folder `data`; None for any other error.
pub fn refused_fix(data: &Path, game: &Path, e: &Error) -> Option<String> {
    let (Error::Io { path, source } | Error::Formats(swofty_formats::Error::IoAt { path, source })) = e
    else {
        return None;
    };
    let in_use = cfg!(windows) && source.raw_os_error() == Some(32);
    if !in_use && source.kind() != std::io::ErrorKind::PermissionDenied {
        return None;
    }
    if path.starts_with(game) {
        return Some(game_refused_text(data, game, in_use));
    }
    path.starts_with(data).then(|| {
        format!(
            "Windows refused to let Swofty change its own files in {data}, which an antivirus does while it scans \
             them; add {data} and the F1 25 folder {game} as exclusions in your antivirus, then try again",
            data = data.display(),
            game = game.display()
        )
    })
}

/// What to tell the player when Windows refused a write in the game folder; `in_use` is a sharing
/// violation (os error 32): another program has the file open, which no administrator right fixes.
fn game_refused_text(data: &Path, game: &Path, in_use: bool) -> String {
    if in_use {
        return format!(
            "another program has a file in {game} open, so Swofty can't change it: close F1 25 if it's still \
             running (look for F1_25.exe in Task Manager), let any Steam update or file check of F1 25 finish, \
             and add {game} and {data} as exclusions in your antivirus, then try again",
            game = game.display(),
            data = data.display()
        );
    }
    format!(
        "Windows refused to let Swofty change files in {}; run Swofty Launcher as administrator, or move F1 25 \
         to a folder outside Program Files",
        game.display()
    )
}

/// The names of the addons in `order` the core has registry records of to apply at boot (data
/// files or `SWOFTY_REGISTER` registrations), whatever route the database takes.
fn registering_addons<'a>(
    order: &[String],
    std16: &'a BTreeMap<String, crate::stdlib::deps::Std>,
) -> Vec<&'a str> {
    order
        .iter()
        .filter_map(|id| std16.get(id))
        .filter(|s| s.registers())
        .map(|s| s.name.as_str())
        .collect()
}

/// The build line for the addons whose registry records turn the early stage on.
fn early_stage_line(names: &[&str]) -> String {
    format!(
        "registry: the core loads at the early stage for the registry records of {}",
        names.join(", ")
    )
}

/// The `registry.json` of each addon whose `replaced_operations` the core in
/// `loader` applies instead: the core lists the rows feature and the addon has
/// data files, which reach the core through the registry index.
fn replaced_operations<'a>(
    std16: &'a BTreeMap<String, crate::stdlib::deps::Std>,
    loader: Option<&Path>,
) -> BTreeMap<&'a str, &'a crate::stdlib::manifest::RegistryManifest> {
    if !loader.is_some_and(crate::stdlib::manifest::core_applies_rows) {
        return BTreeMap::new();
    }
    std16
        .iter()
        .filter(|(_, s)| !s.data_files.is_empty())
        .filter_map(|(id, s)| Some((id.as_str(), s.registry.as_ref()?)))
        .filter(|(_, r)| !r.replaced_operations.is_empty())
        .collect()
}

/// The tables the `add-row` ops of the database operations file at `path` add rows to.
fn added_tables(path: &Path) -> BTreeSet<String> {
    let Ok(file) = crate::read_json::<serde_json::Value>(path) else {
        return BTreeSet::new();
    };
    file.get("ops")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|op| op.get("op").and_then(serde_json::Value::as_str) == Some("add-row"))
        .filter_map(|op| op.get("table")?.as_str().map(str::to_owned))
        .collect()
}

/// Turns off, in `on`, each toggle that is on whose rows add a round at a track the build doesn't hold, and
/// says so: the game won't start with a race weekend at a track it has no row for.
fn trackless_options(
    store: &Store,
    game: &Game,
    p: &Profile,
    order: &[String],
    addons: &BTreeMap<String, AddonManifest>,
    on: &mut OnAddons,
) -> Vec<String> {
    let gated: Vec<&AddonManifest> = order
        .iter()
        .filter_map(|id| addons.get(id))
        .filter(|m| {
            !m.operations.database.is_empty() && m.options.iter().any(|o| o.kind == OptionType::Toggle)
        })
        .collect();
    if gated.is_empty() {
        return Vec::new();
    }
    let Ok(shipped) = std::fs::read(original(&store.root, game, GVDB)).map(|d| Gvdb::parse(&d)) else {
        return Vec::new();
    };
    let Ok(shipped) = shipped else {
        return Vec::new();
    };
    let stock = crate::tracks::Schedule::of(&shipped);
    let held = stock.held(store, order, addons);
    let mut found = Vec::new();
    for m in gated {
        let dir = store.addons_dir().join(&m.id);
        let docs: Vec<serde_json::Value> = m
            .operations
            .database
            .iter()
            .filter_map(|rel| serde_json::from_slice(&std::fs::read(dir.join(rel)).ok()?).ok())
            .collect();
        for t in crate::tracks::option_tracks(m, &docs, &stock, &held) {
            let Some(o) = m.options.iter().find(|o| o.id == t.option) else {
                continue;
            };
            if on.value(p, m, o) == serde_json::Value::Bool(true) {
                found.push((m, o, t));
            }
        }
    }
    let codes = found.iter().map(|(_, _, t)| t.track.clone()).collect();
    let adders = crate::tracks::adders(store, &codes);
    let mut out = Vec::new();
    for (m, o, t) in found {
        on.force_off(&m.id, &o.id);
        let adds = adders
            .get(&t.track)
            .map(|id| addons.get(id).map_or(id.as_str(), |a| a.name.as_str()));
        out.push(t.line(&m.name, o.label.as_deref().unwrap_or(&o.id), adds));
    }
    out
}

/// What [`track_limit`] found and how its check went.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackLimitCheck {
    /// The tracks the on addons add with `track.add`.
    pub added: usize,
    /// The track addons left out because the game's track lists are full.
    pub limit: Option<crate::tracks::TrackLimit>,
    /// How long reading the profile and counting its tracks took.
    pub counting: std::time::Duration,
    /// The check build's steps, folded by [`crate::fold_steps`]; empty when too few tracks are on for it to run.
    pub steps: Vec<crate::Step>,
    /// The check build restored a saved build with the same hash.
    pub reused: bool,
    /// The check build is saved, so a build of the same addons and game files restores it.
    pub saved: bool,
}

impl TrackLimitCheck {
    /// Whether a check build ran.
    pub fn built(&self) -> bool {
        !self.steps.is_empty()
    }
}

/// A `checking` build gives addon build hooks a host that lists addons but
/// cannot install, remove, turn on or move them.
/// The track addons `profile` would lose to the game's full track lists, found by planning the build
/// without installing it; none, and no build, when too few tracks are on to fill them.
pub fn track_limit(store: &Store, profile: &str, game: &Game) -> Result<TrackLimitCheck> {
    let started = std::time::Instant::now();
    let addons = store.registry()?.manifests();
    let p = store.profile(profile)?;
    let order = resolve(&p, &addons)?.order;
    let mut check = TrackLimitCheck {
        added: crate::tracks::added_count(store, &order, &addons),
        counting: started.elapsed(),
        ..TrackLimitCheck::default()
    };
    if !crate::tracks::may_fill_track_lists(check.added) {
        return Ok(check);
    }
    let (built, steps) = crate::timed_steps("resolving the addons", || build_as(store, profile, game, true));
    let built = built?;
    check.steps = crate::fold_steps(steps, 3);
    check.reused = built.reused;
    check.saved = built.overlay.join(SAVED_FILE).is_file();
    check.limit = built.track_limit;
    Ok(check)
}

pub(crate) fn build_as(store: &Store, profile: &str, game: &Game, checking: bool) -> Result<Built> {
    let registry = store.registry()?;
    let addons = registry.manifests();
    let mut p = store.profile(profile)?;
    let mut skipped = Vec::new();
    for u in store.unreadable_addons() {
        if !addons.contains_key(&u.id) && p.enabled().any(|id| id == u.id) {
            p.set_enabled(&u.id, false)?;
            skipped.push(format!("{} is left out: it {}", u.name, u.reason));
        }
    }
    let lent_settings = crate::row_ids::lent().map(|o| (p.lend(&o), o));
    let resolved = resolve(&p, &addons)?;
    let mut on = OnAddons::new(resolved.order.iter().filter_map(|id| addons.get(id)));
    let mut std16 = match p.is_vanilla() {
        true => BTreeMap::new(),
        false => crate::stdlib::deps::read_all(store, &addons),
    };
    let deps = crate::stdlib::deps::for_order(profile, &resolved.order, &addons, &std16);
    if let Some(stop) = deps.stop_text() {
        return Err(Error::Dependencies(stop));
    }
    skipped.extend(trackless_options(
        store,
        game,
        &p,
        &resolved.order,
        &addons,
        &mut on,
    ));
    let mut built = Built {
        order: resolved.order.clone(),
        warnings: skipped
            .into_iter()
            .chain(resolved.warnings.iter().cloned())
            .collect(),
        plan: Plan {
            profile: profile.into(),
            ..Plan::default()
        },
        ..Built::default()
    };
    if let Some(((ordered, values), lent)) = &lent_settings
        && (*ordered > 0 || *values > 0)
    {
        built.lines.push(format!(
            "launch: using the host's load order and {values} option value(s) for {} addon(s)",
            lent.order
                .iter()
                .chain(lent.options.keys())
                .collect::<BTreeSet<_>>()
                .len()
        ));
    }
    built
        .warnings
        .extend(deps.of(crate::stdlib::deps::Level::Warn).map(|f| f.line()));
    built.lines.extend(
        deps.of(crate::stdlib::deps::Level::Hint)
            .map(|f| format!("registry: {}", f.line())),
    );
    built
        .lines
        .extend(deps.notes.iter().map(|n| format!("registry: {n}")));
    if !p.is_vanilla() {
        let index = crate::stdlib::careers::data_path(&store.root);
        let mut careers = crate::stdlib::careers::read(&index);
        let stock_max = crate::stdlib::careers::stock_max_for(&careers, || stock_max_ids(&store.root, game));
        let played: BTreeSet<String> = crate::stdlib::careers::in_save_of(&careers, &store.root, profile)
            .into_iter()
            .map(|c| c.career.clone())
            .collect();
        if crate::stdlib::careers::settle_rows(
            &mut careers,
            &crate::row_ids::id_owners(&store.root),
            &stock_max,
            &addons,
            Some((&played, &resolved.order)),
        ) && !checking
        {
            crate::write_json_shared(&index, &serde_json::json!({ "version": 1, "careers": careers })).ok();
        }
        let scoped: Vec<_> = careers
            .iter()
            .filter(|c| played.contains(&c.career))
            .cloned()
            .collect();
        if let Some(line) = crate::stdlib::careers::stop_line(&scoped, &resolved.order, &addons) {
            built.warnings.push(line);
        }
        if let Some(line) = crate::stdlib::careers::unowned_line(
            &scoped,
            &resolved.order,
            &addons,
            &crate::row_ids::save_owners(&store.root),
            &stock_max,
        ) {
            built.warnings.push(line);
        }
        if let Some(line) = crate::stdlib::careers::play_line(&careers, &resolved.order, &addons) {
            built.warnings.push(line);
        }
    }
    built.report = Report {
        profile: profile.into(),
        built_at: crate::conflicts::now(),
        pairs: Vec::new(),
        order: p.enabled().map(str::to_owned).collect(),
        stock_removals: BTreeMap::new(),
        kept_lists: BTreeMap::new(),
        waits: Vec::new(),
    };
    if p.is_vanilla() {
        built.plan = Plan::vanilla();
        return Ok(built);
    }
    check_originals(&store.root, game)?;
    let _lock = lock(store, profile)?;
    crate::now_doing("reading the addons");
    let mut hasher = Sha256::new();
    let packed = store.root.join("cache").join(crate::cache::PACKED_DIR);
    swofty_formats::erp::cache_compressed(Some(packed.clone()));
    let loader = loader_dir();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf));
    built
        .lines
        .push(loader_line(exe_dir.as_deref(), loader.as_deref()));
    let features = loader
        .as_deref()
        .map(load_features)
        .transpose()?
        .unwrap_or_default();
    let config = store.config();
    let packages: BTreeMap<String, String> = resolved
        .order
        .iter()
        .filter(|id| crate::dbedit::package::is_package(store, id))
        .map(|id| (id.clone(), addons[id].name.clone()))
        .collect();
    let overlay_names = |origin: &Origin, src: &Source| -> Vec<String> {
        src.resolve(&source_dir(store, &origin.addon, src.root))
            .ok()
            .and_then(|p| swofty_formats::erp::ErpFile::open(&p).ok())
            .map(|f| f.erp.resources.into_iter().map(|r| r.name).collect())
            .unwrap_or_default()
    };
    let mipmap_names = |origin: &Origin, src: &Source| -> Vec<String> {
        src.resolve(&source_dir(store, &origin.addon, src.root))
            .map(|p| crate::erpbuild::mipmap_names(&p))
            .unwrap_or_default()
    };
    let mut lenient = packages.clone();
    if let Some(core) = addons.get(CORE_ID) {
        lenient.insert(CORE_ID.to_owned(), core.name.clone());
    }
    let names = resolved
        .order
        .iter()
        .map(|id| (id.clone(), addons[id].name.clone()))
        .collect();
    let mut planner = Planner::new(&features, &config.runtime_assets)
        .build_only(&config.build_assets)
        .lenient(lenient)
        .names(names)
        .overlays(&overlay_names)
        .mipmaps(&mipmap_names);
    if features != Features::default() {
        hasher.update(format!("runtime {features:?} {:?};", config.runtime_assets));
        if !config.build_assets.is_empty() {
            hasher.update(format!("build {:?};", config.build_assets));
        }
    }
    let mut runtime = Runtime::new(planner.features());
    let mut driver_picks: Vec<(String, Vec<String>)> = Vec::new();
    let mut experimental: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for id in &resolved.order {
        for f in &addons[id].experimental_features {
            experimental.entry(f.name()).or_default().push(&addons[id].name);
        }
    }
    for (feature, by) in &experimental {
        runtime.enable(feature);
        hasher.update(format!("feature {feature};"));
        built.lines.push(format!(
            "features: the experimental {feature} feature is on for {}",
            by.join(", ")
        ));
    }
    let looks: Vec<&AddonManifest> = resolved
        .order
        .iter()
        .map(|id| &addons[id])
        .filter(|m| m.shader.is_some())
        .collect();
    if let Some((m, earlier)) = looks.split_last() {
        for e in earlier {
            built.warnings.push(format!(
                "{}: its look is not applied; one shader look applies at a time, and {} loads after it",
                e.name, m.name
            ));
        }
        let (rel, strength) = shader_look(m, &p, &on)?;
        let path = store.addons_dir().join(&m.id).join(&rel);
        let text =
            std::fs::read_to_string(&path).map_err(|e| Error::Other(format!("{}: {rel}: {e}", m.name)))?;
        let cube = swofty_formats::cube::Cube::parse(&text)
            .map_err(|e| Error::Other(format!("{}: {rel}: {e}", m.name)))?;
        hasher.update(format!("shader {} {rel} {strength};", m.id));
        hasher.update(text.as_bytes());
        runtime.shader(cube.pack(), strength / 100.0, &m.name);
        built.lines.push(format!(
            "shaders: {} grades the picture with {rel} ({n}x{n}x{n}) at {strength}% strength",
            m.name,
            n = cube.size
        ));
    }
    for id in &resolved.order {
        let m = &addons[id];
        if m.postfx.is_empty() {
            continue;
        }
        for t in &m.postfx_textures {
            let image = match &t.file {
                None => None,
                Some(file) => {
                    let path = store.addons_dir().join(&m.id).join(file);
                    let png =
                        std::fs::read(&path).map_err(|e| Error::Other(format!("{}: {file}: {e}", m.name)))?;
                    hasher.update(format!("postfx texture {} {file};", m.id));
                    hasher.update(&png);
                    Some(
                        crate::postfx::png_to_tex(&png)
                            .map_err(|e| Error::Other(format!("{}: {file}: {e}", m.name)))?,
                    )
                }
            };
            hasher.update(format!("postfx texture {} {t:?};", m.id));
            runtime.postfx_texture(
                PostFxTexture {
                    name: texture_name(m, &t.name),
                    scale: t.scale,
                    width: t.width,
                    height: t.height,
                    format: if image.is_some() {
                        "rgba8".into()
                    } else {
                        t.format.clone().unwrap_or_else(|| "rgba16f".into())
                    },
                    persistent: image.is_none().then_some(t.persistent),
                    file: None,
                },
                image,
            );
        }
        if let Some(t) = &m.postfx_toggle
            && let [option] = placeholders(t)[..]
        {
            hasher.update(format!("postfx toggle {} {option};", m.id));
            runtime.postfx_toggle(&m.id, option);
        }
    }
    let mut passes: Vec<(i32, usize, &AddonManifest, &PostFx)> = Vec::new();
    for id in &resolved.order {
        for pass in &addons[id].postfx {
            passes.push((pass.order, passes.len(), &addons[id], pass));
        }
    }
    passes.sort_by_key(|(order, at, ..)| (*order, *at));
    for (_, _, m, pass) in passes {
        let label = pass.label();
        let Some(enabled) = pass_enabled(m, pass, &p, &on)? else {
            hasher.update(format!("postfx {} {} off;", m.id, pass.shader));
            built
                .lines
                .push(format!("postfx: {}'s pass {label} is switched off", m.name));
            continue;
        };
        let options = pass_options(m, pass, &p, &on)?;
        let source = store.addons_dir().join(&m.id).join(&pass.shader);
        let budget = pass.budget_ms.unwrap_or(1.5);
        let spec = PostFxPass {
            shader: String::new(),
            name: label.clone(),
            addon: m.name.clone(),
            options,
            budget_ms: budget,
            addon_id: m.id.clone(),
            live: pass_live(m, pass),
            inputs: pass
                .inputs
                .iter()
                .map(|n| match n.as_str() {
                    "original" | "previous" => n.clone(),
                    _ => texture_name(m, n),
                })
                .collect(),
            output: pass
                .output
                .as_deref()
                .map(|n| texture_name(m, n))
                .unwrap_or_default(),
            threads: pass.threads.unwrap_or([8, 8]),
            raw: pass.raw,
            when: pass.when.as_ref().map(|w| PostFxWhen {
                views: w.views.clone(),
                skip: w.skip.clone(),
            }),
            enabled,
        };
        hasher.update(format!(
            "postfx {} {} {} {spec:?};",
            m.id, pass.shader, pass.order
        ));
        let entry = crate::postfx::Entry::of(pass);
        match crate::postfx::compile(&source, &store.addon_cache_dir(&m.id).join("postfx"), entry) {
            Ok(crate::postfx::Compiled { dxil, .. }) => {
                hasher.update(&dxil);
                built.lines.push(format!(
                    "postfx: {}'s pass {label} runs as pass {} before the HUD ({} bytes of DXIL)",
                    m.name,
                    runtime.assets.postfx.len() + 1,
                    dxil.len()
                ));
                runtime.postfx(dxil, spec);
            }
            Err(e) => {
                hasher.update(format!("skipped {e};"));
                built
                    .warnings
                    .push(format!("{}: pass {label} is skipped: {e}", m.name));
            }
        }
    }
    let shader_files = crate::shaders::build_files(store, &p);
    if !shader_files.files.is_empty() {
        runtime.enable("postfx");
    }
    hasher.update(format!("shaders {};", shader_files.digest));
    built.lines.extend(shader_files.line);
    built.warnings.extend(shader_files.warnings);
    let mut head_puts = crate::heads::Puts::default();
    let mut outputs = BTreeMap::new();
    let replaced = replaced_operations(&std16, loader_dir().as_deref());
    let mut left_to_core = false;
    let mut hud_inputs: Vec<crate::hud::Input> = Vec::new();
    for id in &resolved.order {
        let m = &addons[id];
        let mut out = OutputBuilder::default();
        hasher.update(format!("{id}@{};", m.version));
        let dir = store.addons_dir().join(id);
        let origin = Origin {
            addon: id.clone(),
            item: None,
        };
        let mut files = Vec::new();
        let package = packages.get(id);
        if let Some(name) = package {
            match package_ops(store, id) {
                Ok((file, picks)) => {
                    hasher.update(serde_json::to_vec(&file).expect("serialisable"));
                    out.made("picks", &picks);
                    files.push((FileKind::Database, file));
                    driver_picks.push((name.clone(), picks));
                }
                Err(e) => {
                    hasher.update(format!("unread {e};"));
                    built.warnings.push(format!("{name}: package skipped: {e}"));
                }
            }
        }
        if id == CORE_ID {
            hasher.update(CORE_DB);
            files.push((FileKind::Database, core_db_ops()));
            let intro = core_intro(store, m, &p, &on, &mut hasher)?;
            if !intro.ops.is_empty() {
                files.push((FileKind::Assets, intro));
            }
        }
        for (kind, rel) in m.operations.op_files() {
            if package.is_some() && kind == FileKind::Database {
                continue;
            }
            if replaced.get(id.as_str()).is_some_and(|r| r.replaces(rel)) {
                left_to_core = true;
                planner.add_core_rows(id, added_tables(&dir.join(rel)));
                hasher.update(format!("replaced {rel};"));
                built.lines.push(format!(
                    "registry: {}: {rel} is applied by the core from the addon's registry records",
                    m.name
                ));
                continue;
            }
            let file = render_file(&dir.join(rel), kind, m, &p, &on, &mut hasher)?;
            hash_sources(&mut hasher, store, id, &file.ops)?;
            files.push((kind, file));
        }
        let declared: Vec<AssetOp> = m.operations.file_ops().collect();
        hash_sources(&mut hasher, store, id, &declared)?;
        if let Some(op) = unbuilt(files.iter().flat_map(|(_, f)| &f.ops)) {
            built
                .warnings
                .push(format!("{id}: ERP operations skipped; {op} is not built yet"));
            for (_, f) in &mut files {
                f.ops
                    .retain(|op| !matches!(op, AssetOp::Erp(_) | AssetOp::Warn { .. }));
            }
        }
        let roots = |root: Root| source_dir(store, id, root);
        for (kind, file) in files {
            out.made(&format!("{kind:?}"), &file);
            for op in &file.ops {
                op.sources().into_iter().for_each(|src| out.reads(id, src));
                head_puts.op(id, op, &roots);
            }
            planner.push_file(&origin, kind, file);
        }
        for op in declared {
            out.made("declared", &op);
            op.sources().into_iter().for_each(|src| out.reads(id, src));
            head_puts.op(id, &op, &roots);
            planner.push(Op {
                op,
                origin: origin.clone(),
            });
        }
        if let Some(entry) = registry.get(id).filter(|e| !e.declared) {
            let host = Host::new(store, id, &p, Some(game))?.read_only(checking);
            let options = m
                .options
                .iter()
                .map(|o| (o.id.clone(), on.value(&p, m, o)))
                .collect();
            let batch = entry
                .addon
                .build(&BuildCx {
                    cx: host.cx(),
                    options: &options,
                    features: &features,
                })
                .map_err(|e| Error::Other(format!("building {id}: {e}")))?;
            hasher.update(serde_json::to_vec(&batch.ops).expect("serialisable"));
            if !batch.fields.is_empty() {
                hasher.update(serde_json::to_vec(&batch.fields).expect("serialisable"));
            }
            let ops: Vec<AssetOp> = batch.ops.iter().map(|op| op.op.clone()).collect();
            hash_sources(&mut hasher, store, id, &ops)?;
            for line in &batch.lines {
                out.line(line);
            }
            built
                .lines
                .extend(batch.lines.iter().map(|l| format!("{id}: {l}")));
            built
                .warnings
                .extend(batch.warnings.iter().map(|w| format!("{id}: {w}")));
            let mut ops = batch.ops;
            if let Some(op) = unbuilt(ops.iter().map(|o| &o.op)) {
                built
                    .warnings
                    .push(format!("{id}: ERP operations skipped; {op} is not built yet"));
                ops.retain(|o| !matches!(o.op, AssetOp::Erp(_)));
            }
            out.made("fields", &batch.fields);
            planner.add_fields(id, batch.fields);
            for op in ops {
                out.made("hook", &op);
                op.op
                    .sources()
                    .into_iter()
                    .for_each(|src| out.reads(&op.origin.addon, src));
                head_puts.op(&op.origin.addon, &op.op, &|root| {
                    source_dir(store, &op.origin.addon, root)
                });
                planner.push(op);
            }
        }
        for s in m.operations.screens.iter().filter(|s| s.page.is_none()) {
            out.reads(id, &Source::addon(s.file.clone()));
            let path = dir.join(&s.file);
            let data = std::fs::read(&path).map_err(io_err(&path))?;
            if swofty_formats::pkg::is_pkg(&data) {
                let (cat, engine) = (crate::ui::catalog::stock(), crate::ui::catalog::engine());
                let none = crate::ui::catalog::Scenes::default();
                let resource = format!("{id}/{}", s.file);
                let input = crate::ui::check::UiInput {
                    resource: &resource,
                    frag: "temp",
                    data: &data,
                    stock: None,
                };
                let report = crate::ui::check::ui_edit(&input, &none, cat, engine)?;
                if !report.hard.is_empty() {
                    let lines: Vec<String> = report.hard.iter().map(|h| h.to_string()).collect();
                    return Err(Error::Other(format!(
                        "{id}: screen {} breaks rules the game's UI loader enforces:\n  {}",
                        s.name,
                        lines.join("\n  ")
                    )));
                }
                built
                    .warnings
                    .extend(report.warnings.iter().map(|w| format!("{id}: ui: {w}")));
            }
            hasher.update(&data);
        }
        for f in &m.operations.hud {
            out.reads(id, &Source::addon(f.clone()));
            let path = dir.join(f);
            hasher.update(&std::fs::read(&path).map_err(io_err(&path))?);
            hud_inputs.extend(crate::hud::inputs_of(id, &dir, f)?);
        }
        let pending = m.operations.ui.len();
        if pending > 0 {
            built.warnings.push(format!(
                "{id}: {pending} UI operation file(s) skipped; that stage is not built yet"
            ));
        }
        if !store.is_builtin(id) {
            for n in &m.operations.native {
                out.made("native", n);
                for rel in std::iter::once(&n.dll).chain(&n.files) {
                    out.reads(id, &Source::addon(rel.trim_end_matches('/')));
                }
            }
        }
        outputs.insert(id.clone(), out.finish());
    }
    if !hud_inputs.is_empty() {
        for input in &hud_inputs {
            for sprite in input.record.sprites.values() {
                let path = input.dir.join(&sprite.png);
                hasher.update(&std::fs::read(&path).map_err(io_err(&path))?);
            }
        }
    }
    // The core applies registry records at boot only when it loads at the early stage, on the runtime and the
    // file database route alike.
    let registering = match loader_dir().is_some() {
        true => registering_addons(&resolved.order, &std16),
        false => Vec::new(),
    };
    if !registering.is_empty() {
        built.lines.push(early_stage_line(&registering));
    }
    runtime.assets.early_stage |= left_to_core || !registering.is_empty();
    if head_puts.pending() {
        let path = original(&store.root, game, GVDB);
        let stock = crate::heads::labels(&Gvdb::parse(&std::fs::read(&path).map_err(io_err(&path))?)?);
        let added = crate::heads::ops(&head_puts.new_heads(&stock), &stock, head_puts.registered());
        hasher.update(serde_json::to_vec(&added.ops).expect("serialisable"));
        let name = |id: &str| addons.get(id).map_or(id, |m| m.name.as_str()).to_owned();
        built
            .lines
            .extend(added.lines.iter().map(|(id, l)| format!("{}: {l}", name(id))));
        built
            .warnings
            .extend(added.warnings.iter().map(|(id, w)| format!("{}: {w}", name(id))));
        for op in added.ops {
            planner.push(op);
        }
    }
    let (mut tracks, mut left_out) = track_packages(store, planner.take_tracks())?;
    let recorded = crate::tracks::with_records(&resolved.order, &std16);
    let (more, unusable) = crate::tracks::record_packages(store, &resolved.order, &std16, &tracks);
    tracks.extend(more);
    left_out.extend(unusable);
    if crate::tracks::may_fill_track_lists(tracks.len()) {
        let path = original(&store.root, game, GVDB);
        let shipped = Gvdb::parse(&std::fs::read(&path).map_err(io_err(&path))?)?;
        let listed = crate::tracks::ListedTracks::of(&shipped);
        let (kept, full) = crate::tracks::fit_track_lists(&listed, tracks, &recorded);
        tracks = kept;
        let names: Vec<String> = full
            .iter()
            .map(|((_, pkg, origin), _)| {
                let addon = addons
                    .get(&origin.addon)
                    .map_or(origin.addon.as_str(), |m| m.name.as_str());
                match pkg.display_name() == addon {
                    true => addon.to_owned(),
                    false => format!("{} ({addon})", pkg.display_name()),
                }
            })
            .collect();
        for (((_, pkg, _), _), name) in full.iter().zip(&names) {
            if !tracks.iter().any(|(_, p, _)| p.track_code == pkg.track_code) {
                planner.leave_out_track(&pkg.track_code, name);
            }
        }
        built
            .warnings
            .extend(crate::tracks::track_lists_full_warning(&listed, &names));
        built.track_limit = crate::tracks::TrackLimit::new(
            &listed,
            names.clone(),
            full.iter()
                .map(|((_, _, origin), _)| origin.addon.clone())
                .collect(),
        );
        left_out.extend(full.into_iter().map(|((_, _, origin), why)| (origin, why)));
    }
    crate::tracks::keep_track_folders(store, &tracks, &mut std16);
    for (origin, why) in &left_out {
        let name = addons
            .get(&origin.addon)
            .map_or(origin.addon.as_str(), |m| m.name.as_str());
        built
            .warnings
            .push(crate::tracks::left_out_line(name, &origin.addon, why));
        planner.add_lenient(&origin.addon, name);
    }
    let left_codes: Vec<&str> = left_out
        .iter()
        .filter_map(|(_, why)| why.code.as_deref())
        .filter(|c| !tracks.iter().any(|(_, p, _)| p.track_code == *c))
        .collect();
    let maps: Vec<(PathBuf, TrackPackage)> = tracks.iter().map(|(d, p, _)| (d.clone(), p.clone())).collect();
    let mods: Vec<SessionMod> = planner
        .take_items()
        .into_iter()
        .filter_map(|op| match op.op {
            AssetOp::Item(ItemOp::Show {
                id,
                name,
                author,
                version,
                description,
            }) => Some(SessionMod {
                id,
                name,
                version,
                author,
                description,
            }),
            _ => None,
        })
        .collect();
    let natural = planner.route(Kind::Group) == Route::Runtime;
    if !natural && maps.len() > SLOTS.len() {
        if loader.is_none() {
            return Err(Error::Other(loader_missing(exe_dir.as_deref())));
        }
        return Err(Error::Map(format!(
            "at most {} tracks can be enabled at once",
            SLOTS.len()
        )));
    }
    let slots: Vec<Option<&str>> = (0..maps.len()).map(|i| (!natural).then(|| SLOTS[i])).collect();
    let reg = Registry::builtin();
    hasher.update(crate::activation::db::REGISTRY_JSON);
    for (dir, pkg) in &maps {
        hasher.update(serde_json::to_vec(pkg).expect("serialisable"));
        let erp = dir.join(&pkg.erp);
        let meta = std::fs::metadata(&erp).map_err(io_err(&erp))?;
        hasher.update(format!("{}|{:?}", meta.len(), meta.modified().ok()));
    }
    let resolve = |rel: &str| original(&store.root, game, rel);
    let addon_dir = |id: &str| store.addons_dir().join(id);
    let (handling_maps, handling_notes) = {
        let dirs: Vec<(String, PathBuf)> = resolved
            .order
            .iter()
            .map(|id| (id.clone(), addon_dir(id)))
            .collect();
        let options = |addon: &str, name: &str| {
            let m = addons.get(addon)?;
            m.options
                .iter()
                .find(|o| o.id == name)
                .map(|o| on.value(&p, m, o))
        };
        handling::read_maps(&dirs, &options)
    };
    built
        .warnings
        .extend(handling_notes.iter().map(|n| format!("handling: {n}")));
    if !handling_maps.is_empty() {
        let sources = handling::sources(&handling_maps, &resolve, &addon_dir);
        hasher.update(handling::stamp(&handling_maps, &sources)?);
    }
    let ui_maps: Vec<MapUi> = maps
        .iter()
        .zip(&slots)
        .filter(|((_, pkg), _)| pkg.ui.is_some())
        .map(|((dir, pkg), slot)| MapUi {
            pkg,
            dir,
            asset_group: match slot {
                Some(slot) => slot_asset_group(slot),
                None => track_group(&pkg.track_code, &pkg.track_code),
            },
        })
        .collect();
    if !ui_maps.is_empty() {
        hasher.update(ui_stamp(&ui_maps, &resolve)?);
    }
    let record_ui = record_ui_lists(&ui_maps, &tracks, &recorded, &resolve, &mut built.warnings);
    let mut caches = Vec::new();
    let mut session = match &lent_settings {
        Some(_) => store.session_settings_of(&p)?,
        None => store.session_settings(profile)?,
    };
    session.set_maps(tracks.iter().map(|(_, pkg, origin)| (pkg, origin.addon.as_str())));
    session.set_mods(mods.into_iter());
    hasher.update(serde_json::to_vec(&session).expect("serialisable"));
    let menus = loader_dir().is_some();
    if menus {
        for rel in MENU_ERPS.iter().chain(MENU_READS) {
            let p = resolve(&ui_file(rel));
            let meta = std::fs::metadata(&p).map_err(io_err(&p))?;
            hasher.update(format!("{rel}|{}|{:?}", meta.len(), meta.modified().ok()));
        }
    }
    let db_src = original(&store.root, game, GVDB);
    hasher.update(sha256_file(&db_src)?);
    let order: Vec<&AddonManifest> = resolved.order.iter().map(|id| &addons[id]).collect();
    hash_unread(&mut hasher, store, game, &order, planner.erp_ops(), &resolve)?;
    let std_index = loader_dir()
        .is_some()
        .then(|| {
            crate::stdlib::index::Index::plan(profile, &resolved.order, &addons, &std16, &store.addons_dir())
        })
        .flatten();
    if let Some(placed) = &std_index {
        hasher.update(crate::stdlib::index::Index::stamp(placed)?);
        for id in &resolved.order {
            for path in std16.get(id).into_iter().flat_map(|s| &s.files.missing) {
                built.warnings.push(format!(
                    "{}: its registry data names {path}, which isn't a file or folder inside the addon; the game \
                     reports it missing",
                    addons[id].name
                ));
            }
        }
    }
    let room_asked = std16
        .values()
        .any(|s| resolved.order.contains(&s.id) && s.is_std())
        .then(|| {
            crate::stdlib::room::asked(
                crate::stdlib::reports::Room::read(&store.root, Some(&game.dir)).as_ref(),
                profile,
                &resolved.order,
            )
        })
        .flatten();
    if let Some(asked) = room_asked {
        hasher.update(format!("room {asked};"));
    }
    let core_left_out = crate::stdlib::reports::Conflicts::read(&store.root, Some(&game.dir))
        .map(|c| c.left_out)
        .unwrap_or_default();
    let core_warning = crate::stdlib::reports::left_out_warning(
        &core_left_out,
        &BTreeMap::new(),
        &resolved.order,
        &|id: &str| addons.get(id).map_or(id, |m| m.name.as_str()).to_owned(),
    );
    let lent_ids = crate::row_ids::lent().filter(|o| o.stamp(&resolved.order).is_some());
    if let Some(stamp) = lent_ids.as_ref().and_then(|o| o.stamp(&resolved.order)) {
        hasher.update(stamp);
    }
    built.buildhash = hex(&hasher.finalize())[..16].to_owned();
    built.overlay = store.overlay_dir(profile, &built.buildhash);
    built.warnings.extend(core_warning);
    let (lines_at, warnings_at) = (built.lines.len(), built.warnings.len());
    if let Some(saved) = Saved::load(&built.overlay, &built.buildhash).filter(|s| s.usable(store, game)) {
        saved.restore(&mut built)?;
        built.reused = true;
        built.report.order = p.enabled().map(str::to_owned).collect();
        crate::write_json(&crate::conflicts::path(store, profile), &built.report)?;
        if !checking {
            fingerprint::record_outputs(store, profile, with_row_ids(outputs, &built.row_ids)).ok();
            track_files(store, game, &mut built, &tracks, &slots, &recorded, &record_ui);
        }
        return Ok(built);
    }
    let cancel = || match checking {
        true => Ok(()),
        false => crate::busy::check(&store.root),
    };
    cancel()?;
    std::fs::remove_dir_all(&built.overlay).ok();
    crate::now_doing("editing the game database");
    let ui_lists = if ui_maps.is_empty() {
        Default::default()
    } else {
        plan_lists(&ui_maps, &resolve)?.0
    };
    let mut stock_removals = BTreeMap::new();
    let mut kept_lists = BTreeMap::new();
    let mut waits = Vec::new();
    let mut udp_ids = crate::udp_ids::UdpIds::default();
    let mut id_aliases = crate::row_ids::IdAliases::default();
    let mut row_ids_unsaved = false;
    if !maps.is_empty() || planner.has_db() || room_asked.is_some() {
        let shipped = std::fs::read(&db_src).map_err(io_err(&db_src))?;
        let mut db = Gvdb::parse(&shipped)?;
        let stock: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
        let row_ids_lock = crate::row_ids::lock(&store.root)?;
        let last = crate::row_ids::last_build(&store.overlays_dir(profile), &built.buildhash, GVDB);
        let (mut row_ids, row_id_warnings) =
            crate::row_ids::Store::load(&store.root, Some(&game.dir), profile, last);
        built.warnings.extend(row_id_warnings);
        built.lines.extend(row_ids.recover(&store.sessions_dir()).lines);
        if let Some(lent) = lent_ids.as_ref().filter(|l| !l.rows.is_empty()) {
            let n = row_ids.lend(lent, &resolved.order).map_err(Error::Resolve)?;
            built.lines.push(format!(
                "database: {n} added-row id(s) are the room host's, for this session only; row-ids.json keeps yours"
            ));
        }
        let older = crate::tracks::read_older(&store.root, profile);
        // A full track id pool lends the id of a track whose addon this build leaves out (Store::track_stat_id).
        row_ids.building(resolved.order.iter().cloned());
        let mut track_udp: Vec<(String, String, Option<i64>)> = Vec::new();
        let mut track_ids: Vec<(String, String, i64)> = Vec::new();
        for (((_, pkg), slot), (_, _, origin)) in maps.iter().zip(&slots).zip(&tracks) {
            if recorded.contains(&origin.addon) {
                track_udp.push((origin.addon.clone(), pkg.track_code.clone(), None));
                built.lines.push(format!(
                    "{}: files only; its track record adds its rows in game",
                    pkg.track_code
                ));
                continue;
            }
            let names = slot.map(|s| (slot_asset_group(s), format!("{s}.assetgroup")));
            let root = slot_asset_root();
            let route = match &names {
                Some((asset_group, key)) => TrackHost::Slot {
                    asset_group,
                    asset_root: &root,
                    key,
                },
                None => TrackHost::Natural,
            };
            let lists = ui_lists.get(&pkg.track_code).cloned().unwrap_or_default();
            let seed = older
                .iter()
                .find(|o| o.addon == origin.addon && o.code == pkg.track_code)
                .and_then(|o| o.stat_id)
                .map(i64::from);
            let mut given = None;
            let mut give = |top: i64, held: &dyn Fn(i64) -> bool| {
                let id = row_ids.track_stat_id(&origin.addon, &pkg.track_code, seed, top, held);
                given = id.as_ref().ok().map(|(id, _)| *id);
                id
            };
            let report = activate_with(&mut db, pkg, &reg, Some(&route), &lists, Some(&mut give))?;
            track_udp.push((origin.addon.clone(), pkg.track_code.clone(), given));
            if let Some(id) = given {
                track_ids.push((origin.addon.clone(), pkg.track_code.clone(), id));
            }
            for l in report.lines {
                built.lines.push(format!("{}: {l}", pkg.track_code));
            }
            for w in report.warnings {
                built.warnings.push(format!("{}: {w}", pkg.track_code));
            }
        }
        let tracked: HashSet<u64> = db
            .records
            .iter()
            .map(|r| r.key)
            .filter(|k| !stock.contains(k))
            .collect();
        let shipped_db = Gvdb::parse(&shipped)?;
        let track_changes = match maps.is_empty() {
            true => Vec::new(),
            false => crate::tracks::changed_records(&db, &shipped_db),
        };
        let staged = (planner.route(Kind::Db) == Route::Runtime && planner.has_db()).then(|| db.clone());
        let mut db_report = crate::ops::DbReport::default();
        planner.keep_entrant_lists(crate::entrant_lists::StockLists::new(&shipped_db));
        planner.keep_row_ids(row_ids);
        let added = planner.apply_db_report(&mut db, &stock, &tracked, &mut db_report)?;
        built.lines.extend(crate::virtual_lists::settle(&mut db, &stock));
        built.lines.extend(crate::rnd_budget::report(&db));
        if let Some(ids) = planner.take_row_ids() {
            if let Some(missed) = ids.lent_missed() {
                return Err(Error::Resolve(missed));
            }
            id_aliases = ids.id_aliases();
            built.lines.extend(ids.summary());
            if checking {
                row_ids_unsaved = ids.unsaved();
            } else {
                ids.save(&store.root)?;
            }
        }
        drop(row_ids_lock);
        built.lines.extend(db_report.lines);
        built.warnings.extend(db_report.warnings);
        built.warnings.extend(planner.take_skipped());
        let id_left_out = planner.take_id_left_out();
        if !id_left_out.is_empty() {
            let name = |id: &str| addons.get(id).map_or(id, |m| m.name.as_str()).to_owned();
            built.warnings.extend(crate::stdlib::reports::left_out_warning(
                &BTreeMap::new(),
                &id_left_out,
                &resolved.order,
                &name,
            ));
        }
        let mut kept: BTreeMap<String, usize> = BTreeMap::new();
        for key in crate::tracks::keep_changes(&mut db, &shipped_db, &track_changes) {
            let by = planner
                .record_writer(key)
                .map_or("database", |id| addons.get(id).map_or(id, |m| m.name.as_str()));
            *kept.entry(by.to_owned()).or_default() += 1;
        }
        for (by, n) in kept {
            built.lines.push(format!(
                "{by}: kept the tracks' changes to {n} database value(s) it sets back to the game's own"
            ));
        }
        if !left_codes.is_empty() {
            let weekends = crate::tracks::restore_weekends(&mut db, &shipped_db, &left_codes)
                .map_err(|stuck| Error::Resolve(stuck.join("; ")))?;
            built.warnings.extend(weekends);
            built
                .warnings
                .extend(crate::tracks::drop_left_out_orders(&mut db, &left_codes));
        }
        let in_game: Vec<&str> = tracks
            .iter()
            .filter(|(_, _, o)| recorded.contains(&o.addon))
            .map(|(_, p, _)| p.track_code.as_str())
            .collect();
        let missing = crate::tracks::keep_track_refs(&mut db, &shipped_db, &in_game, &left_codes);
        if !missing.is_empty() {
            let codes = missing.iter().map(|m| m.track.clone()).collect();
            let adders = crate::tracks::adders(store, &codes);
            let name = |id: &str| addons.get(id).map_or(id, |m| m.name.as_str()).to_owned();
            for m in &missing {
                let adds = adders
                    .get(&m.track)
                    .filter(|id| !left_codes.contains(&m.track.as_str()) && !resolved.order.contains(*id));
                let by = planner.record_writer(m.key).map(name);
                built
                    .warnings
                    .push(m.line(by.as_deref(), adds.map(|id| name(id)).as_deref()));
            }
        }
        if let Ok(cat) = crate::dbedit::Catalog::open(store, game) {
            for t in crate::dbedit::rows::trim_row_counts(&cat, &mut db) {
                let by = planner
                    .record_writer(t.key)
                    .map_or("database", |id| addons.get(id).map_or(id, |m| m.name.as_str()));
                built.warnings.push(t.line(by));
            }
        }
        for c in crate::dbedit::calendar::cut_rounds(&db) {
            let by = planner
                .record_writer(c.key)
                .map_or("database", |id| addons.get(id).map_or(id, |m| m.name.as_str()));
            built.warnings.push(c.line(by));
        }
        for u in crate::heads::keep_wearable(&mut db, &shipped_db) {
            let by = planner
                .record_writer(u.key)
                .map_or("database", |id| addons.get(id).map_or(id, |m| m.name.as_str()));
            built.warnings.push(u.line(by));
        }
        let added_heads = crate::heads::registry_heads(&store.addons_dir(), &resolved.order, &std16);
        for h in crate::heads::wear_stand_ins(&mut db, &shipped_db, &added_heads) {
            built.lines.push(h.line());
        }
        for k in crate::livery_kits::keep_kits(&mut db) {
            let by = planner
                .record_writer(k.key)
                .map_or("database", |id| addons.get(id).map_or(id, |m| m.name.as_str()));
            built.warnings.push(k.line(by));
        }
        stock_removals = planner.take_stock_removals();
        for (id, n) in &stock_removals {
            let name = addons.get(id).map_or(id.as_str(), |m| m.name.as_str());
            built.warnings.push(stock_removals_line(name, *n));
        }
        for (id, n) in planner.take_track_removals() {
            let name = addons.get(&id).map_or(id.as_str(), |m| m.name.as_str());
            built.warnings.push(track_removals_line(name, n));
        }
        waits = planner
            .take_waits()
            .into_iter()
            .map(|(addon, on)| crate::conflicts::Wait { addon, on })
            .collect();
        kept_lists = planner.take_kept_lists();
        for (id, lists) in &kept_lists {
            let name = addons.get(id).map_or(id.as_str(), |m| m.name.as_str());
            built.warnings.push(kept_lists_line(name, lists));
        }
        let mut copies = planner.take_copies();
        for c in &mut copies {
            c.quiet = c.addon == CORE_ID;
        }
        let row_adders = crate::stdlib::room::row_adders(copies.iter().map(|c| c.addon.as_str()), CORE_ID);
        built.row_ids = added_row_ids(&db, &copies);
        let added_ai = ai_teams(&db, copies.iter().map(|c| (c.from.as_str(), c.row.as_str())));
        for (addon, code, id) in &track_ids {
            built
                .row_ids
                .entry(addon.clone())
                .or_default()
                .insert(crate::row_ids::track_row(code), *id);
        }
        udp_ids = crate::udp_ids::build(&db, &copies);
        for (addon, code, stat_id) in &track_udp {
            udp_ids.add_track(addon, code, *stat_id);
        }
        let addon_name = |id: &str| addons.get(id).map_or(id, |m| m.name.as_str()).to_owned();
        let side = crate::dbedit::deps::check(&db, &copies, &addon_name);
        built.warnings.extend(side.warnings);
        built.side_data = side.findings;
        if driver_picks.iter().any(|(_, p)| !p.is_empty()) {
            let why = match resolved.order.iter().any(|id| id == driver_manager::ID) {
                true => driver_manager::Why::NotAdded,
                false => driver_manager::Why::Off,
            };
            let names = driver_manager::names(store);
            built
                .warnings
                .extend(driver_manager::warnings(&db, &driver_picks, &names, why));
        }
        if !added.is_empty() {
            let found = owned::owned_items(&db, &Gvdb::parse(&shipped)?, &added);
            built.lines.extend(found.lines);
            built.warnings.extend(found.warnings);
            session.owned_items = found.items;
        }
        let grid = grid_size(&db);
        let worlds = layer_worlds(&db);
        let mut on_file = true;
        if let Some(staged) = staged {
            let loaded = runtime_loaded_estimate(staged.records.len());
            let room = runtime_record_room(loaded);
            let known: HashSet<u64> = staged.records.iter().map(|r| r.key).collect();
            let created = db.records.iter().filter(|r| !known.contains(&r.key)).count();
            let route = crate::stdlib::room::route(loaded, created, room_asked);
            match db_edits(&staged, &db) {
                Ok(_) if !row_adders.is_empty() => built.lines.push(format!(
                    "database: built into the game file; {} add rows to its tables, which the game indexes while it \
                     loads the file and cannot take as runtime edits",
                    row_adders.join(", ")
                )),
                Ok(_) if created > room => built.lines.push(format!(
                    "database: built into the game file; the addons add {created} record(s) and the game's \
                     database takes {room} more once it has loaded"
                )),
                Ok(edits) if route == crate::stdlib::room::Route::Runtime => {
                    runtime.db(edits);
                    db = staged;
                    on_file = false;
                }
                Ok(_) => {}
                Err(why) => built
                    .warnings
                    .push(format!("database: built into the game file; {why}")),
            }
        }
        if let Some(asked) = room_asked.filter(|_| on_file)
            && let crate::stdlib::room::Route::File { pad_to, short } =
                crate::stdlib::room::file_route(db.records.len(), asked)
        {
            let added = crate::stdlib::room::pad(&mut db, pad_to);
            if added > 0 || short > 0 {
                built
                    .lines
                    .push(crate::stdlib::room::line(pad_to, added, asked, short));
            }
        }
        let bytes = db.to_bytes();
        if bytes == shipped {
            built.lines.push("database: unchanged".into());
        } else {
            let out = built.overlay.join(GVDB);
            let dir = out.parent().expect("has parent");
            std::fs::create_dir_all(dir).map_err(io_err(dir))?;
            std::fs::write(&out, bytes).map_err(io_err(&out))?;
            built.plan.data.insert(GVDB.into(), out);
        }
        for ((dir, pkg, origin), slot) in tracks.iter().zip(&slots) {
            let slot_original = slot.map(|s| original(&store.root, game, &slot_file(s)));
            let track_worlds = missing_worlds(&worlds, &pkg.track_code, |group| {
                planner.last_writer(&[Target::Group(group.to_owned())]).is_some()
            });
            let cars = room_for(&track_worlds, grid);
            let track_ai: Vec<_> = added_ai
                .iter()
                .filter(|t| {
                    planner
                        .last_writer(&[Target::Group(t.group(&pkg.track_code))])
                        .is_none()
                })
                .cloned()
                .collect();
            let placement = place(
                &PlaceInput {
                    pkg,
                    dir,
                    game: &game.dir,
                    slot: slot.zip(slot_original.as_deref()),
                    cars,
                    worlds: &track_worlds,
                    ai_teams: &track_ai,
                },
                &store.root.join("cache/maps").join(&pkg.track_code),
            )?;
            caches.push(placement.dir.clone());
            let at = match slot {
                Some(slot) => format!("slot {slot}"),
                None => placement.archive.clone(),
            };
            built.lines.push(format!(
                "{}: track archive goes in {at}; {} texture(s) use its own mip files",
                pkg.track_code, placement.mips
            ));
            if !placement.groups.is_empty() {
                built.lines.push(format!(
                    "{}: {} asset group(s) registered in game",
                    pkg.track_code,
                    placement.groups.len()
                ));
            }
            for group in placement.groups {
                planner.push(Op {
                    op: AssetOp::Group(GroupOp::Register {
                        group,
                        erp: Some(placement.archive.clone()),
                        erps: Vec::new(),
                        deps: Vec::new(),
                        template: None,
                    }),
                    origin: origin.clone(),
                });
            }
            if let Some(g) = &placement.grid {
                built.lines.push(format!(
                    "{}: world extended from {} to {} grid slots, {} to {} garages and {} to {} pit boxes for {cars} cars",
                    pkg.track_code,
                    g.before.grid_slots,
                    g.after.grid_slots,
                    g.before.garages,
                    g.after.garages,
                    g.before.pit_boxes,
                    g.after.pit_boxes
                ));
            }
            for (alias, to) in &placement.aliases {
                built
                    .lines
                    .push(format!("{}: world {alias} stands in with {to}", pkg.track_code));
            }
            for (team, to) in &placement.ai_aliases {
                built.lines.push(format!(
                    "{}: AI data for {team} stands in with the track's {to}",
                    pkg.track_code
                ));
            }
            if !placement.repaired.is_empty() {
                built.lines.push(format!(
                    "{}: {}",
                    pkg.track_code,
                    crate::erpbuild::repaired_note(pkg.display_name(), &placement.repaired)
                ));
            }
            if !placement.spaced.is_empty() {
                built.lines.push(format!(
                    "{}: {}",
                    pkg.track_code,
                    crate::activation::ailine::fixed_note(pkg.display_name(), &placement.spaced)
                ));
            }
            built
                .warnings
                .extend(placement.notes.iter().map(|n| format!("{}: {n}", pkg.track_code)));
            let (left, replaced): (Vec<_>, Vec<_>) = placement
                .replaced
                .into_iter()
                .partition(|(rel, _)| left_over(store, game, rel));
            for (rel, src) in replaced {
                if built.plan.data.insert(rel.clone(), src).is_some() {
                    return Err(Error::Map(format!("{rel} is replaced twice")));
                }
            }
            for (rel, src) in placement.added.into_iter().chain(left) {
                if built.plan.added.insert(rel.clone(), src).is_some() {
                    return Err(Error::Map(format!("{rel} is placed by two tracks")));
                }
            }
        }
    }
    let mut menu_kept = Kept::new();
    if !ui_maps.is_empty() {
        let ui = build_ui(&ui_maps, &resolve, &store.root.join("cache/menus"))?;
        caches.push(ui.dir.clone());
        menu_kept = ui.kept;
        built
            .warnings
            .extend(ui.notes.iter().map(|n| format!("menus: {n}")));
        for (code, lists) in &ui.lists {
            built.lines.push(format!(
                "{code}: menu pictures in {} list(s), minimap and track-select scenes",
                lists.len()
            ));
        }
        for (rel, src) in ui.replaced {
            if built.plan.data.insert(rel.clone(), src).is_some() {
                return Err(Error::Map(format!("{rel} is replaced twice")));
            }
        }
    }
    let ers_maps: Vec<ErsMap> = maps
        .iter()
        .filter(|(_, pkg)| !pkg.ers.is_empty())
        .map(|(_, pkg)| ErsMap {
            code: &pkg.track_code,
            base: pkg.activation.base_track().unwrap_or(&pkg.track_code),
            values: &pkg.ers,
        })
        .collect();
    let mut handled: BTreeMap<String, PathBuf> = BTreeMap::new();
    if handling_maps
        .iter()
        .any(|m| matches!(m.target, handling::Target::Stock(_)))
    {
        let hb = handling::build_stock(&handling_maps, &resolve, &store.root.join("cache/handling"))?;
        caches.extend(hb.dir.clone());
        for line in hb.lines {
            match line.strip_prefix(crate::activation::wheel_check::WARNING_PREFIX) {
                Some(w) => built.warnings.push(w.to_owned()),
                None => built.lines.push(line),
            }
        }
        handled = hb.replaced;
    }
    let current = |rel: &str| handled.get(rel).cloned().unwrap_or_else(|| resolve(rel));
    let ers_cars = |rel: &str| !ers_maps.is_empty() && ers::is_car_archive(rel);
    for (rel, src) in &handled {
        if !ers_cars(rel) && built.plan.data.insert(rel.clone(), src.clone()).is_some() {
            return Err(Error::Map(format!("{rel} is replaced twice")));
        }
    }
    if !ers_maps.is_empty() {
        let ers = build_ers(&ers_maps, &game.dir, &current, &store.root.join("cache/ers"))?;
        for (rel, src) in &handled {
            if ers_cars(rel) && !ers.replaced.contains_key(rel) {
                built.plan.data.insert(rel.clone(), src.clone());
            }
        }
        caches.extend(ers.dir.clone());
        built.lines.push(format!(
            "ers: {} handling file(s) in {} car archive(s) get the tracks' settings",
            ers.handling_files,
            ers.replaced.len()
        ));
        built
            .warnings
            .extend(ers.notes.iter().map(|n| format!("ers: {n}")));
        for (rel, src) in ers.replaced {
            if built.plan.data.insert(rel.clone(), src).is_some() {
                return Err(Error::Map(format!("{rel} is replaced twice")));
            }
        }
    }
    let file_ops = planner.take_files();
    let mut erp_ops = planner.take_erp();
    let rank = |o: &Origin| {
        resolved
            .order
            .iter()
            .position(|id| *id == o.addon)
            .unwrap_or(usize::MAX)
    };
    let puts = archive_puts(store, &file_ops, &erp_ops, &rank)?;
    for (rel, put) in &puts {
        planner.put_archive(rel, &put.origin);
    }
    erp_ops.retain_mut(|op| {
        let rank = rank(&op.origin);
        after_puts(op, &puts, rank)
    });
    let mut mipmaps = BTreeMap::new();
    let mut built_over = BTreeSet::new();
    let name = |origin: &str| match origin.split_once(" (") {
        Some((id, item)) => match addons.get(id) {
            Some(m) => format!("{} ({item}", m.name),
            None => origin.to_owned(),
        },
        None => addons
            .get(origin)
            .map_or_else(|| origin.to_owned(), |m| m.name.clone()),
    };
    for (id, folder, n) in stray_asset_groups(&game.dir, &file_ops) {
        built
            .warnings
            .push(stray_asset_group_line(&name(&id), &folder, n));
    }
    if !hud_inputs.is_empty() {
        let stock = crate::hud::GameStock::with_baseline(&game.dir, &store.root)
            .unwrap_or_else(|_| crate::hud::GameStock::new(&game.dir));
        let staged = built.plan.data.clone();
        let base = |rel: &str| {
            staged
                .get(rel)
                .or_else(|| puts.get(rel).map(|p| &p.path))
                .cloned()
                .unwrap_or_else(|| resolve(rel))
        };
        let mfd_rows = match menus {
            true => crate::ui::mfd_rows(&session),
            false => Vec::new(),
        };
        let page = crate::ui::mfd_page_rows(&mfd_rows);
        let mfd_edit = |data: &[u8]| crate::ui::mfd::templates(data, &page);
        let put_names: Vec<String> = puts.keys().cloned().collect();
        let composed = crate::hud::integrate::compose_in_build(
            &crate::hud::integrate::Context {
                store,
                stock: &stock,
                base: &base,
                baseline: &resolve,
                puts: &put_names,
                name: &name,
            },
            &erp_ops,
            &hud_inputs,
            (!page.is_empty()).then_some(&mfd_edit as crate::hud::TemplateBase),
        )?;
        let owner = hud_inputs[0].addon.clone();
        composed
            .output
            .write(&store.addon_cache_dir(&owner).join("hud"))?;
        built
            .lines
            .extend(composed.lines.iter().map(|l| format!("hud: {l}")));
        built.warnings.extend(composed.warnings);
        erp_ops.extend(composed.ops);
    }
    if !erp_ops.is_empty() || !puts.is_empty() {
        let staged = built.plan.data.clone();
        let base = |rel: &str| {
            staged
                .get(rel)
                .or_else(|| puts.get(rel).map(|p| &p.path))
                .cloned()
                .unwrap_or_else(|| resolve(rel))
        };
        let mut absent: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        erp_ops.retain(|op| {
            let AssetOp::Erp(e) = &op.op else {
                return true;
            };
            let gone: Vec<&str> = reads(e)
                .into_iter()
                .filter(|rel| relative_inside(rel) && !base(rel).is_file())
                .collect();
            if gone.is_empty() {
                return true;
            }
            absent
                .entry(op.origin.addon.clone())
                .or_default()
                .extend(gone.into_iter().map(str::to_owned));
            false
        });
        for (id, rels) in &absent {
            built.warnings.push(missing_archives(&name(id), rels));
        }
        for (rel, put) in &puts {
            if erp_ops
                .iter()
                .any(|op| matches!(&op.op, AssetOp::Erp(e) if writes(e).contains(&rel.as_str())))
            {
                built.lines.push(format!(
                    "erp: {rel}: later changes apply to the whole archive {} puts",
                    put.origin
                ));
            }
        }
        let mut erps = ErpBuild::new(&base).with_baseline(&resolve);
        for rel in puts.keys() {
            erps.load(rel)?;
        }
        let contested =
            crate::erpbuild::contested_ui(&erp_ops, &|op, s| source_dir(store, &op.origin.addon, s.root));
        for (erp, resource) in menu_kept.keys() {
            erps.keep(erp, resource);
        }
        let runtime_route = planner.route(Kind::Erp) == Route::Runtime;
        // Every origin that writes, in load order: the later wins.
        let mut ranks: Vec<String> = Vec::new();
        let mut prebuilt: Vec<Prebuilt> = Vec::new();
        let slices = slice_merges(&erp_ops);
        for (at, op) in erp_ops.iter().enumerate() {
            cancel()?;
            crate::now_doing(format!("applying {}", op.origin));
            let id = &op.origin.addon;
            let resolve = |s: &Source| Ok(s.resolve(&source_dir(store, id, s.root))?);
            let origin = op.origin.to_string();
            if matches!(op.op, AssetOp::Erp(_)) && !ranks.contains(&origin) {
                ranks.push(origin.clone());
            }
            match &op.op {
                AssetOp::Erp(crate::ops::ErpOp::Overlay {
                    erps: targets,
                    source,
                }) if runtime_route => {
                    let path = resolve(source)?;
                    let file = swofty_formats::erp::ErpFile::open(&path)?;
                    let stale = crate::erpbuild::stale_stats_in(&file)?.len();
                    let menus = file
                        .erp
                        .resources
                        .iter()
                        .any(|r| targets.iter().any(|t| erps.keeps(t, &r.name)));
                    let fits = match merged_later(&slices, at, targets, &path)? {
                        Some(why) => Err(why),
                        None if menus => Err("it carries the track menu pictures".to_owned()),
                        None if targets.iter().any(|t| !crate::erpbuild::served_at_runtime(t)) => {
                            Err("the in-game loader patches only archives under 2025_asset_groups".to_owned())
                        }
                        None if stale > 0 => {
                            Err(format!("{stale} resource(s) in it need their size headers fixed"))
                        }
                        None if carries_handling(&path, targets)? => Err(
                            "it carries car handling files, which the game reads from the game file"
                                .to_owned(),
                        ),
                        None if let Some(uri) = contested_in(&contested, &path, targets)? => Err(format!(
                            "another addon replaces {uri} whole too, and the two are merged node by node"
                        )),
                        None => overlay_fits(&path, targets, &base)?,
                    };
                    match fits {
                        Ok(resources) => prebuilt.push(Prebuilt {
                            origin,
                            path,
                            erps: targets.clone(),
                            resources,
                        }),
                        Err(why) => {
                            built.lines.push(format!(
                                "erp: {origin}: its overlay archive goes in resource by resource; {why}"
                            ));
                            erps.apply_for(
                                &crate::ops::ErpOp::Overlay {
                                    erps: targets.clone(),
                                    source: source.clone(),
                                },
                                &origin,
                                &resolve,
                            )
                            .map_err(|e| Error::Other(format!("applying {}: {e}", op.origin)))?;
                            ui_merge_notes(&mut built, &mut planner, erps.take_ui_merges(), &name);
                        }
                    }
                }
                AssetOp::Erp(erp) => {
                    erps.set_merge_ui(op.origin.item.as_deref() != Some(crate::hud::integrate::ORIGIN));
                    match erps.apply_for(erp, &op.origin.to_string(), &resolve) {
                        Ok(()) => {}
                        Err(Error::ResourceMissing { erp, resource }) if puts.contains_key(&erp) => {
                            built.warnings.push(missing_resource(
                                &name(&op.origin.to_string()),
                                &erp,
                                &resource,
                                &name(&puts[&erp].origin.to_string()),
                            ));
                            continue;
                        }
                        Err(e) => {
                            return Err(Error::Other(format!("applying {}: {e}", op.origin)));
                        }
                    }
                    for m in erps.take_merges() {
                        record_merge(&mut built, &mut planner, &op.origin, m);
                    }
                    ui_merge_notes(&mut built, &mut planner, erps.take_ui_merges(), &name);
                }
                AssetOp::Warn { message } => built.warnings.push(format!("{}: {message}", op.origin)),
                _ => {}
            }
        }
        built.warnings.extend(erps.wheel_warnings());
        if !ers_maps.is_empty() {
            let edited = erps.edit_written(|rel, r| match ers::in_teams(rel) && r.kind() == "VTF" {
                true => ers::edit_vtf(r, &ers_maps),
                false => Ok(None),
            })?;
            if edited > 0 {
                built.lines.push(format!(
                    "ers: {edited} handling file(s) other addons change get the tracks' settings too"
                ));
            }
        }
        let keeper = |h: &Held| {
            let codes = menu_kept.get(&(h.erp.clone(), h.resource.clone()))?;
            tracks
                .iter()
                .filter(|(_, p, _)| codes.contains(&p.track_code))
                .map(|(_, _, o)| o)
                .max_by_key(|o| rank(o))
                .cloned()
        };
        let origin = |s: &str| {
            erp_ops
                .iter()
                .find(|op| op.origin.to_string() == s)
                .map(|op| op.origin.clone())
        };
        keep_menus(
            &mut built,
            &mut planner,
            erps.take_held(),
            &keeper,
            &origin,
            &name,
        );
        for n in erps.take_notes() {
            built.lines.push(format!("erp: {n}"));
        }
        for w in erps.check_ui(&name)? {
            built.warnings.push(format!("ui: {w}"));
        }
        for line in erps.cohere_arrays()? {
            built.lines.push(format!("erp: {line}"));
        }
        for (severity, w) in erps.check_textures(&name) {
            let kind = match severity {
                crate::texcheck::Severity::Invalid => "texture the game's renderer may reject",
                _ => "texture",
            };
            built.warnings.push(format!("{kind}: {w}"));
        }
        let stats = erps.repair_stats();
        let by_origin = |list: &[(String, String)]| {
            let mut out: Vec<(String, Vec<String>)> = Vec::new();
            for (origin, r) in list {
                match out.iter_mut().find(|(o, _)| o == origin) {
                    Some((_, rs)) => rs.push(r.clone()),
                    None => out.push((origin.clone(), vec![r.clone()])),
                }
            }
            out
        };
        if let Some((origin, files)) = by_origin(&stats.broken).first() {
            let who = name(origin);
            let fix = format!("Turn {who} off in this profile.");
            return Err(Error::Other(crate::erpbuild::unrepairable_message(
                &who, files, &fix,
            )));
        }
        for (origin, files) in by_origin(&stats.repaired) {
            built.lines.push(format!(
                "erp: {}",
                crate::erpbuild::repaired_note(&name(&origin), &files)
            ));
        }
        mipmaps = erps.take_mipmaps();
        if let Some(note) = planner.route_note(Kind::Erp) {
            built.lines.push(format!("erp: {note}"));
        }
        let resized = if planner.route(Kind::Erp) == Route::Runtime {
            let outside = erps.take_outside(&built.overlay)?;
            for rel in &outside {
                built.lines.push(format!(
                    "erp: {rel} built into the game files; the in-game loader patches only archives under 2025_asset_groups"
                ));
            }
            let resized = erps.take_resized(&built.overlay)?;
            for r in &resized {
                let (uri, why) = &r.resources[0];
                built.lines.push(format!(
                    "erp: {} built into the game files; {} resource(s) change their stock layout, which the in-game loader cannot load, such as {uri}: {why}",
                    r.erp,
                    r.resources.len()
                ));
            }
            let built_in: Vec<String> = resized.iter().map(|r| r.erp.clone()).chain(outside).collect();
            if !prebuilt.is_empty() {
                built.lines.push(format!(
                    "erp: {} ready overlay archive(s) load as they are",
                    prebuilt.len()
                ));
            }
            let notes = runtime.resources(erps.resources(), &ranks, &prebuilt, &built.overlay)?;
            built.warnings.extend(notes);
            stage_erps(&mut built, &staged, built_in)?;
            resized
        } else {
            let resized = erps.resized();
            let archives = store.root.join("cache").join(crate::cache::ARCHIVES_DIR);
            let (written, reused) = erps.write_cached(&built.overlay, Some(&archives), &cancel)?;
            if reused > 0 {
                built.lines.push(format!(
                    "erp: {reused} of {} archive(s) unchanged since an earlier build, linked from the cache",
                    written.len()
                ));
            }
            stage_erps(&mut built, &staged, written)?;
            resized
        };
        built.warnings.extend(stock_holders(&game.dir, &resized));
        let in_plan: BTreeSet<String> = built.plan.data.keys().map(|r| r.to_ascii_lowercase()).collect();
        let staged_l: BTreeSet<String> = staged.keys().map(|r| r.to_ascii_lowercase()).collect();
        built_over = puts
            .keys()
            .filter(|rel| {
                let l = rel.to_ascii_lowercase();
                in_plan.contains(&l) && !staged_l.contains(&l)
            })
            .cloned()
            .collect();
    }
    cancel()?;
    crate::now_doing("placing files");
    put_files(
        &mut built,
        store,
        game,
        mipmaps,
        file_ops,
        &built_over,
        (&ers_maps, &handling_maps),
    )?;
    match loader_dir() {
        Some(dir) => {
            built.plan.loader = loader_files(&dir)?;
            built.plan.registry_override = true;
            built.lines.push(format!(
                "loader: {} file(s) from {}",
                built.plan.loader.len(),
                dir.display()
            ));
            if menus {
                let lang = planner.route(Kind::Lang) == Route::Runtime;
                let writer = |erp: &str, res: &str| {
                    let targets = [
                        Target::Resource {
                            erp: erp.to_owned(),
                            uri: res.to_owned(),
                        },
                        Target::File(erp.to_owned()),
                    ];
                    planner.last_writer(&targets).map(|o| name(&o.to_string()))
                };
                for (key, value) in build_menus(&mut built, &mut session, &resolve, &writer, lang)? {
                    planner.push(Op {
                        op: AssetOp::Lang(LangOp::Set {
                            language: "*".into(),
                            key,
                            value,
                        }),
                        origin: Origin {
                            addon: CORE_ID.into(),
                            item: None,
                        },
                    });
                }
            }
            let files = screen_files(&order, &store.addons_dir())?;
            if !files.is_empty() {
                built.lines.push(format!("ui: {} screen(s) under {LOADER_DIR}/ui", files.len()));
            }
            for (rel, src) in files {
                if built.plan.loader.insert(rel.clone(), src).is_some() {
                    return Err(Error::Other(format!("{rel} is placed twice")));
                }
            }
            let core = format!("{LOADER_DIR}/{ADDONS_DIR}/core/");
            let taken: Vec<String> = built
                .plan
                .loader
                .keys()
                .filter_map(|rel| rel.strip_prefix(&core))
                .filter(|n| n.ends_with(".dll") && !n.contains('/'))
                .map(str::to_owned)
                .collect();
            let plugins = plugin_files(store, order.iter().copied(), taken)?;
            if !plugins.dlls.is_empty() {
                built.lines.push(format!(
                    "native: {} plugin(s), {} file(s) under {LOADER_DIR}/{ADDONS_DIR}",
                    plugins.dlls.len(),
                    plugins.files.len()
                ));
            }
            for (rel, src) in plugins.files {
                if built.plan.loader.insert(rel.clone(), src).is_some() {
                    return Err(Error::Other(format!("{rel} is placed twice")));
                }
            }
            session.plugins = plugins.dlls;
            if !udp_ids.is_empty() {
                let rel = format!("{LOADER_DIR}/{}", crate::udp_ids::FILE);
                let src = built.overlay.join(crate::udp_ids::FILE);
                std::fs::create_dir_all(&built.overlay).map_err(io_err(&built.overlay))?;
                crate::write_json(&src, &udp_ids)?;
                built.plan.loader.insert(rel, src);
                built.lines.push(format!(
                    "udp: telemetry reports {} added team id(s) and {} added driver id(s) under stock ids, {} track(s) by Swofty track id",
                    udp_ids.teams.len(),
                    udp_ids.drivers.len(),
                    udp_ids.track_ids.len()
                ));
            }
            if !id_aliases.is_empty() {
                let rel = format!("{LOADER_DIR}/{}", crate::row_ids::ALIASES_FILE);
                let src = built.overlay.join(crate::row_ids::ALIASES_FILE);
                std::fs::create_dir_all(&built.overlay).map_err(io_err(&built.overlay))?;
                id_aliases.write(&src)?;
                built.plan.loader.insert(rel, src);
                let n: usize = id_aliases.tables.values().map(|t| t.len()).sum();
                built.lines.push(format!(
                    "database: {LOADER_DIR}/{} maps {n} earlier row id(s) to the ids the rows have now, so saves made under the earlier ids load",
                    crate::row_ids::ALIASES_FILE
                ));
            }
            if let Some(ids) = lent_ids
                .as_ref()
                .and_then(|l| l.room_stat_ids(&resolved.order))
            {
                let rel = format!("{LOADER_DIR}/{}", crate::row_ids::ROOM_STAT_IDS_FILE);
                let src = built.overlay.join(crate::row_ids::ROOM_STAT_IDS_FILE);
                std::fs::create_dir_all(&built.overlay).map_err(io_err(&built.overlay))?;
                crate::write_json(&src, &ids)?;
                built.plan.loader.insert(rel, src);
                built.lines.push(format!(
                    "registry: {LOADER_DIR}/{} lends the host's stat ids for this session",
                    crate::row_ids::ROOM_STAT_IDS_FILE
                ));
            }
            if let Some(placed) = &std_index {
                let index = built.overlay.join("registry-index.json");
                std::fs::create_dir_all(&built.overlay).map_err(io_err(&built.overlay))?;
                std::fs::write(&index, placed.index.to_json()).map_err(io_err(&index))?;
                for (rel, src) in placed
                    .files
                    .iter()
                    .map(|(r, s)| (r.clone(), s.clone()))
                    .chain([(crate::stdlib::index::PATH.to_owned(), index)])
                {
                    match built.plan.loader.insert(rel.clone(), src.clone()) {
                        Some(before) if before != src => {
                            return Err(Error::Other(format!("{rel} is placed twice")));
                        }
                        _ => {}
                    }
                }
                let data: usize = placed.index.addons.iter().map(|a| a.data.len()).sum();
                let named: usize = placed
                    .index
                    .addons
                    .iter()
                    .filter_map(|a| std16.get(&a.id))
                    .map(|s| s.files.files.len())
                    .sum();
                built.lines.push(format!(
                    "registry: index for {} addon(s) with {data} data file(s) and {named} file(s) they name, under \
                     {LOADER_DIR}/registry and {LOADER_DIR}/{ADDONS_DIR}",
                    placed.index.addons.len()
                ));
            }
        }
        None => built.warnings.push(format!(
            "{CORE_ID}: the in-game loader was not found (native/dist, loader/ beside the swofty command, or {LOADER_ENV}); the game starts without it"
        )),
    }
    built.settings = Some(session);
    let mut changes = planner.changes();
    let mut planned = planner.finish();
    for op in planned.runtime {
        let resolve = |op: &Op, s: &Source| Ok(s.resolve(&source_dir(store, &op.origin.addon, s.root))?);
        runtime.push(op, &resolve)?;
    }
    if let Some(runtime) = runtime.finish(&built.overlay)? {
        built.lines.push(runtime.summary());
        for (rel, src) in runtime.files {
            if built.plan.loader.insert(rel.clone(), src).is_some() {
                return Err(Error::Other(format!("{rel} is placed twice")));
            }
        }
    }
    for (rel, bytes) in shader_files.files {
        let path = built.overlay.join(crate::runtime::SWOFTY).join(&rel);
        let dir = path.parent().expect("has parent");
        std::fs::create_dir_all(dir).map_err(io_err(dir))?;
        std::fs::write(&path, bytes).map_err(io_err(&path))?;
        built
            .plan
            .loader
            .insert(format!("{}/{rel}", crate::runtime::SWOFTY), path);
    }
    built.warnings.extend(waiting_lines(&planned.waiting, &features));
    for (_, _, origin) in &tracks {
        changes.entry(origin.addon.clone()).or_default().tracks += 1;
    }
    let name = |id: &str| addons.get(id).map_or_else(|| id.to_owned(), |m| m.name.clone());
    for id in resolved.order.iter().filter(|id| *id != CORE_ID) {
        if let Some(c) = changes.get(id).filter(|c| !c.is_empty()) {
            built.lines.push(format!("{}: {c}", name(id)));
        }
    }
    for c in &mut planned.conflicts {
        if matches!(&c.target, Target::File(f) if f == crate::builtin::CORE_INTRO_CACHED)
            || matches!(&c.target, Target::String { key, .. } if key == crate::builtin::CORE_INTRO_SUBTITLE)
        {
            c.overridden.retain(|o| o.addon != CORE_ID);
        }
    }
    planned.conflicts.retain(|c| !c.overridden.is_empty());
    let records = planned
        .conflicts
        .iter()
        .any(|c| matches!(c.target, Target::Record(_)));
    let catalog = records
        .then(|| crate::dbedit::Catalog::open(store, game).ok())
        .flatten();
    let record = |key: u64| {
        planned.record_names.get(&key).cloned().or_else(|| {
            catalog
                .as_deref()
                .and_then(|c| crate::conflicts::record_name(c, key))
        })
    };
    let (report, within) = crate::conflicts::summarise(profile, &planned.conflicts, &record);
    built.warnings.extend(report.pairs.iter().map(|p| p.line(&name)));
    built.warnings.extend(within);
    built.report = Report {
        order: built.report.order.clone(),
        stock_removals,
        kept_lists,
        waits,
        ..report
    };
    built.conflicts = planned.conflicts;
    built.waiting = planned.waiting;
    if !built.plan.is_vanilla() {
        let plan_tracks: Vec<crate::tracks::PlanTrack> = tracks
            .iter()
            .map(|(_, pkg, origin)| crate::tracks::PlanTrack {
                code: pkg.track_code.clone(),
                name: pkg.track_name.clone().filter(|n| !n.trim().is_empty()),
                addon: Some(origin.addon.clone()),
            })
            .collect();
        let listing = serde_json::json!({
            "profile": profile,
            "buildhash": built.buildhash,
            "tracks": plan_tracks,
            "replace": built.plan.data,
            "add": built.plan.added,
            "loader": built.plan.loader,
        });
        crate::write_json(&built.overlay.join(PLAN_FILE), &listing)?;
    }
    cancel()?;
    crate::now_doing("saving the build");
    let saved = (!built.plan.is_vanilla())
        .then(|| Saved::of(&built, lines_at, warnings_at))
        .transpose()?;
    caches.push(built.overlay.clone());
    caches.extend(
        [&built.plan.data, &built.plan.added, &built.plan.loader]
            .into_iter()
            .flat_map(|m| m.values().cloned()),
    );
    let removed = crate::cache::prune(&store.root.join("cache"), profile, &caches);
    if removed > 0 {
        built
            .lines
            .push(format!("cache: {removed} stale build folder(s) removed"));
    }
    crate::cache::prune_packed(&packed, PACKED_AGE);
    crate::cache::prune_archives(
        &store.root.join("cache").join(crate::cache::ARCHIVES_DIR),
        ARCHIVE_AGE,
        ARCHIVE_BYTES,
    );
    // A check keeps no row ids, so a build that reused this one would never keep them
    // either, and a later build could give those rows other ids than a career holds.
    if let Some(saved) = saved.filter(|_| !row_ids_unsaved) {
        crate::write_json(&built.overlay.join(SAVED_FILE), &saved)?;
    }
    crate::write_json(&crate::conflicts::path(store, profile), &built.report)?;
    if !checking {
        fingerprint::record_outputs(store, profile, with_row_ids(outputs, &built.row_ids)).ok();
        track_files(store, game, &mut built, &tracks, &slots, &recorded, &record_ui);
    }
    Ok(built)
}

// Each addon's added rows of top-level tables with a stat id, as `<table>.<row>/<label>` to its id.
fn added_row_ids(db: &Gvdb, copies: &[crate::dbedit::deps::Copy]) -> BTreeMap<String, BTreeMap<String, i64>> {
    let rows: HashSet<&str> = copies.iter().map(|c| c.row.as_str()).collect();
    let mut wanted: HashMap<u64, (&str, &str, &str)> = HashMap::new();
    for c in copies {
        let Some((table, last)) = c.row.rsplit_once('.') else {
            continue;
        };
        let Some((row, _)) = last.rsplit_once('_') else {
            continue;
        };
        if crate::ops::row_shape(table).contains("_N") {
            continue;
        }
        let field = crate::ops::stat_id_field(table, row);
        wanted.insert(
            swofty_formats::hash::fnv1a64(format!("{}.{field}", c.row)),
            (&c.addon, &c.row, "id"),
        );
        wanted.insert(
            swofty_formats::hash::fnv1a64(format!("{}._metadata_", c.row)),
            (&c.addon, &c.row, "label"),
        );
    }
    let mut found: BTreeMap<&str, (&str, Option<String>, Option<i64>)> = BTreeMap::new();
    for r in &db.records {
        let Some(&(addon, row, what)) = wanted.get(&r.key) else {
            continue;
        };
        let e = found.entry(row).or_insert((addon, None, None));
        match what {
            "label" => e.1 = Some(r.text().into_owned()),
            _ => e.2 = r.text().trim().parse().ok(),
        }
    }
    let mut out: BTreeMap<String, BTreeMap<String, i64>> = BTreeMap::new();
    for (row, (addon, label, id)) in found {
        let (Some(label), Some(id)) = (label, id) else {
            continue;
        };
        if !rows.contains(row) {
            continue;
        }
        let table = row.rsplit_once('_').map_or(row, |(t, _)| t);
        out.entry(addon.to_owned())
            .or_default()
            .insert(format!("{table}/{label}"), id);
    }
    out
}

fn with_row_ids(
    mut outputs: BTreeMap<String, fingerprint::Output>,
    row_ids: &BTreeMap<String, BTreeMap<String, i64>>,
) -> BTreeMap<String, fingerprint::Output> {
    for (addon, ids) in row_ids {
        if let Some(o) = outputs.get_mut(addon) {
            o.row_ids = ids.clone();
        }
    }
    outputs
}

/// The menu UI lists the build makes a track record's pictures for: its rows come from the core
/// at boot, so the core points their `TrackTextures` at those pictures from the facts file.
fn record_ui_lists(
    ui_maps: &[MapUi],
    tracks: &[(PathBuf, TrackPackage, Origin)],
    recorded: &BTreeSet<String>,
    resolve: &dyn Fn(&str) -> PathBuf,
    warnings: &mut Vec<String>,
) -> crate::activation::ui::Lists {
    let codes: BTreeSet<&str> = tracks
        .iter()
        .filter(|(_, _, o)| recorded.contains(&o.addon))
        .map(|(_, p, _)| p.track_code.as_str())
        .collect();
    let maps: Vec<MapUi> = ui_maps
        .iter()
        .filter(|m| codes.contains(m.pkg.track_code.as_str()))
        .map(|m| MapUi {
            pkg: m.pkg,
            dir: m.dir,
            asset_group: m.asset_group.clone(),
        })
        .collect();
    if maps.is_empty() {
        return Default::default();
    }
    match plan_lists(&maps, resolve) {
        Ok((lists, _)) => lists,
        Err(e) => {
            warnings.push(format!(
                "tracks: the record tracks' menu pictures can't be planned: {e}"
            ));
            Default::default()
        }
    }
}

/// Writes `$DATA/registry/track-facts.json` and `older-tracks.json` for a build of a profile with
/// addons. A file that can't be written is a warning; the build stands.
fn track_files(
    store: &Store,
    game: &Game,
    built: &mut Built,
    tracks: &[(PathBuf, TrackPackage, Origin)],
    slots: &[Option<&str>],
    recorded: &BTreeSet<String>,
    record_ui: &crate::activation::ui::Lists,
) {
    if built.plan.is_vanilla() {
        return;
    }
    let file = |rel: &str| {
        built
            .plan
            .data
            .get(rel)
            .or_else(|| built.plan.added.get(rel))
            .cloned()
            .unwrap_or_else(|| original(&store.root, game, rel))
    };
    let placed_archive = |code: &str, slot: Option<&str>| match slot {
        Some(s) => slot_file(s),
        None => crate::activation::place::track_file(code),
    };
    let added: Vec<crate::activation::facts::Added> = tracks
        .iter()
        .zip(slots)
        .map(|((dir, pkg, origin), slot)| crate::activation::facts::Added {
            code: pkg.track_code.clone(),
            shipped: dir.join(&pkg.erp),
            placed: file(&placed_archive(&pkg.track_code, *slot)),
            base: recorded
                .contains(&origin.addon)
                .then(|| pkg.activation.base_track().map(str::to_owned))
                .flatten(),
            ui_lists: match recorded.contains(&origin.addon) {
                true => record_ui
                    .get(&pkg.track_code)
                    .map(|l| l.iter().cloned().collect())
                    .unwrap_or_default(),
                false => Vec::new(),
            },
        })
        .collect();
    let scan = crate::activation::facts::Scan {
        profile: &built.plan.profile,
        db: file(GVDB),
        game: &game.dir,
        file: &file,
        placed: built
            .plan
            .data
            .keys()
            .chain(built.plan.added.keys())
            .cloned()
            .collect(),
        added,
        edits: built.plan.loader.get(crate::runtime::ASSETS_FILE).cloned(),
    };
    let mut warnings = Vec::new();
    if let Err(e) = crate::activation::facts::write(&store.root, &scan) {
        warnings.push(format!(
            "tracks: {} could not be written: {e}",
            crate::activation::facts::FILE
        ));
    }
    let older: Vec<(&str, &TrackPackage)> = tracks
        .iter()
        .filter(|(_, _, o)| !recorded.contains(&o.addon))
        .map(|(_, pkg, o)| (o.addon.as_str(), pkg))
        .collect();
    let written = crate::tracks::write_older(
        &store.root,
        &built.plan.profile,
        &older,
        &original(&store.root, game, GVDB),
        &file(GVDB),
    );
    if let Err(e) = written {
        warnings.push(format!(
            "tracks: {} could not be written: {e}",
            crate::tracks::OLDER_FILE
        ));
    }
    built.warnings.extend(warnings);
}

/// A file an earlier session created that still holds exactly what it wrote
/// is a leftover to clear, not a game file to replace.
fn left_over(store: &Store, game: &Game, rel: &str) -> bool {
    let Ok(placed) = Placed::load(&store.root) else {
        return false;
    };
    let Some(p) = placed.get(rel).filter(|p| !p.replaced) else {
        return false;
    };
    let stock = Baseline::load_or_new(&store.root, &game.identity).is_ok_and(|b| b.files.contains_key(rel));
    !stock && sha256_opt(&game.dir.join(rel)).ok().flatten().as_deref() == Some(p.sha256.as_str())
}

fn contested_in(
    contested: &BTreeSet<(String, String)>,
    path: &Path,
    targets: &[String],
) -> Result<Option<String>> {
    if contested.is_empty() {
        return Ok(None);
    }
    let names = crate::erpbuild::overlay_names(path)?;
    Ok(names.into_iter().find(|n| {
        targets
            .iter()
            .any(|t| contested.contains(&(t.clone(), n.clone())))
    }))
}

fn ui_merge_notes(
    built: &mut Built,
    planner: &mut Planner,
    merges: Vec<crate::erpbuild::UiMerge>,
    name: &dyn Fn(&str) -> String,
) {
    for m in merges {
        if m.conflicts.is_empty() {
            planner.merged(&Target::Resource {
                erp: m.erp.clone(),
                uri: m.resource.clone(),
            });
        }
        let who = |ids: &[String]| ids.iter().map(|i| name(i)).collect::<Vec<_>>().join(", ");
        built.lines.push(format!(
            "ui: {}: {} merged node by node against the game's own copy: {} then {} ({} added, {} changed, {} removed by {}; {} conflict(s))",
            m.erp,
            m.resource,
            who(&m.earlier),
            name(&m.later),
            m.stats.added,
            m.stats.changed,
            m.stats.removed,
            name(&m.later),
            m.conflicts.len()
        ));
        for c in m.conflicts {
            built.warnings.push(format!(
                "ui: {} and {} both change {} in {}; {}'s version is kept there because it loads later",
                who(&c.earlier),
                name(&m.later),
                c.path,
                m.resource,
                name(&m.later)
            ));
        }
    }
}

fn slice_merges(ops: &[Op]) -> Vec<(usize, &str, &str)> {
    ops.iter()
        .enumerate()
        .filter_map(|(i, op)| match &op.op {
            AssetOp::Erp(
                crate::ops::ErpOp::MergeSlices { erp, resource, .. }
                | crate::ops::ErpOp::AppendSlices { erp, resource, .. },
            ) => Some((i, erp.as_str(), resource.as_str())),
            _ => None,
        })
        .collect()
}

// A ready overlay loads apart from the build's archives, so a later merge-slices
// into one of its resources needs it to go in resource by resource.
fn merged_later(
    slices: &[(usize, &str, &str)],
    at: usize,
    targets: &[String],
    path: &Path,
) -> Result<Option<String>> {
    let later: Vec<_> = slices
        .iter()
        .filter(|(i, erp, _)| *i > at && targets.iter().any(|t| t == erp))
        .collect();
    if later.is_empty() {
        return Ok(None);
    }
    let names = crate::erpbuild::overlay_names(path)?;
    Ok(later
        .iter()
        .find(|(_, _, uri)| names.iter().any(|n| n == uri))
        .map(|(_, erp, uri)| format!("a later erp.merge-slices merges into {uri} in {erp}")))
}

fn kept_menus_line(name: &str) -> String {
    format!("{name} ships its own track menu pictures; the launcher's track menus are kept")
}

fn keep_menus(
    built: &mut Built,
    planner: &mut Planner,
    held: Vec<Held>,
    keeper: &dyn Fn(&Held) -> Option<Origin>,
    origin: &dyn Fn(&str) -> Option<Origin>,
    name: &dyn Fn(&str) -> String,
) {
    let mut warned = Vec::new();
    for h in held {
        let line = kept_menus_line(&name(&h.origin));
        if !warned.contains(&line) {
            built.warnings.push(line.clone());
            warned.push(line);
        }
        if let (Some(keeper), Some(lost)) = (keeper(&h), origin(&h.origin))
            && keeper.addon != lost.addon
        {
            let target = Target::Resource {
                erp: h.erp,
                uri: h.resource,
            };
            planner.kept(target, &keeper, &lost);
        }
    }
}

fn record_merge(built: &mut Built, planner: &mut Planner, origin: &Origin, m: SliceMerge) {
    built
        .warnings
        .extend(m.notes.iter().map(|n| format!("erp: {origin}: {n}")));
    if m.refused {
        return;
    }
    let slice = |slice: usize| Target::Slice {
        erp: m.erp.clone(),
        uri: m.resource.clone(),
        slice,
    };
    match (m.whole, m.slices) {
        (true, 0) => planner.replaced(
            Target::Resource {
                erp: m.erp.clone(),
                uri: m.resource.clone(),
            },
            origin,
        ),
        (true, n) => (0..n).for_each(|i| planner.replaced(slice(i), origin)),
        (false, n) => {
            built.lines.push(format!(
                "erp: {origin}: {} of {n} slice(s) of {} merged into {}",
                m.changed.len(),
                m.resource,
                m.erp
            ));
            m.changed.iter().for_each(|&i| planner.replaced(slice(i), origin));
        }
    }
}

/// Whether the overlay archive at `path` holds a car handling file for one of
/// a team archive in `targets`.
fn carries_handling(path: &Path, targets: &[String]) -> Result<bool> {
    if !targets.iter().any(|t| t.to_ascii_lowercase().contains("/teams/")) {
        return Ok(false);
    }
    let file = swofty_formats::erp::ErpFile::open(path)?;
    Ok(file.erp.resources.iter().any(|r| r.kind() == "VTF"))
}

fn stage_erps(built: &mut Built, staged: &BTreeMap<String, PathBuf>, rels: Vec<String>) -> Result<()> {
    for rel in rels {
        let src = built.overlay.join(&rel);
        if built.plan.data.insert(rel.clone(), src).is_some() && !staged.contains_key(&rel) {
            return Err(Error::Other(format!("{rel} is replaced twice")));
        }
    }
    Ok(())
}

fn loader_line(exe_dir: Option<&Path>, loader: Option<&Path>) -> String {
    let show = |p: Option<&Path>| p.map_or_else(|| "none".to_owned(), |p| p.display().to_string());
    format!("loader: {}; swofty.exe in {}", show(loader), show(exe_dir))
}

fn loader_missing(exe_dir: Option<&Path>) -> String {
    let at = exe_dir.map_or_else(|| "the launcher's folder".to_owned(), |d| d.display().to_string());
    format!(
        "Swofty Launcher's game files (version.dll, swofty_core.dll) are missing from {at}; your antivirus may have removed them. Restore them or run the installer again"
    )
}

/// The warning for an addon's archive changes a build skips because the
/// archives they change aren't in the game folder.
fn missing_archives(name: &str, rels: &BTreeSet<String>) -> String {
    let first = rels.first().map_or("", String::as_str);
    let what = match rels.len() {
        1 => format!("{first} is not a file in F1 25"),
        n => format!("{n} of the files it changes are not in F1 25, such as {first}"),
    };
    format!(
        "{name}: archive changes skipped; {what}. Turn {name} off if it doesn't work, and tell its author"
    )
}

/// The warning for an addon's change a build skips because the archive
/// another addon provides has no such resource.
fn missing_resource(name: &str, erp: &str, resource: &str, provider: &str) -> String {
    format!(
        "{name}: change skipped; {resource} is not in {erp}, which {provider} provides. Turn {name} off if it doesn't work, and tell its author"
    )
}

/// The game-root folders addons put files in that the game never reads from
/// there: not a folder at the game root, but one under `2025_asset_groups/`.
/// Each is the addon, the folder and how many files go in it.
fn stray_asset_groups(game: &Path, files: &[Op]) -> Vec<(String, String, usize)> {
    let mut counts: BTreeMap<(String, String), usize> = BTreeMap::new();
    for op in files {
        let AssetOp::File(FileOp::Put { target, .. }) = &op.op else {
            continue;
        };
        let Some((top, _)) = target.split_once('/') else {
            continue;
        };
        if top.is_empty()
            || top.eq_ignore_ascii_case("2025_asset_groups")
            || game.join(top).is_dir()
            || !game.join("2025_asset_groups").join(top).is_dir()
        {
            continue;
        }
        *counts
            .entry((op.origin.addon.clone(), top.to_owned()))
            .or_default() += 1;
    }
    counts.into_iter().map(|((a, f), n)| (a, f, n)).collect()
}

fn stray_asset_group_line(name: &str, folder: &str, n: usize) -> String {
    format!(
        "{name}: {n} file(s) under {folder}/ land at the game root, where the game never reads them; the folder belongs under 2025_asset_groups/ - re-import it with that path"
    )
}

/// An archive a `file.put` replaces whole while ERP ops change it too: its last
/// put in load order.
struct ArchivePut {
    rank: usize,
    origin: Origin,
    path: PathBuf,
}

fn archive_puts(
    store: &Store,
    files: &[Op],
    erps: &[Op],
    rank: &dyn Fn(&Origin) -> usize,
) -> Result<BTreeMap<String, ArchivePut>> {
    let changed: BTreeSet<&str> = erps
        .iter()
        .flat_map(|op| match &op.op {
            AssetOp::Erp(e) => writes(e),
            _ => Vec::new(),
        })
        .collect();
    let mut last = BTreeMap::new();
    for op in files {
        let AssetOp::File(FileOp::Put { source, target }) = &op.op else {
            continue;
        };
        let erp = target.to_ascii_lowercase().ends_with(".erp");
        if erp || changed.contains(target.as_str()) {
            last.insert(target.clone(), (op, source));
        }
    }
    let mut puts = BTreeMap::new();
    for (target, (op, source)) in last {
        let path = source.resolve(&source_dir(store, &op.origin.addon, source.root))?;
        if changed.contains(target.as_str()) || holds_texture_arrays(&path) {
            puts.insert(
                target,
                ArchivePut {
                    rank: rank(&op.origin),
                    origin: op.origin.clone(),
                    path,
                },
            );
        }
    }
    Ok(puts)
}

fn holds_texture_arrays(path: &Path) -> bool {
    swofty_formats::erp::ErpFile::open(path).is_ok_and(|f| {
        f.erp
            .resources
            .iter()
            .any(|r| r.name.ends_with(".texarray") || r.name.ends_with(".texarray.image"))
    })
}

/// Leaves out of `op` the archives a put replaces later in load order; false
/// when none is left.
fn after_puts(op: &mut Op, puts: &BTreeMap<String, ArchivePut>, rank: usize) -> bool {
    let replaced = |erp: &str| puts.get(erp).is_some_and(|p| p.rank > rank);
    match &mut op.op {
        AssetOp::Erp(crate::ops::ErpOp::Overlay { erps, .. }) => {
            erps.retain(|e| !replaced(e));
            !erps.is_empty()
        }
        AssetOp::Erp(e) => !writes(e).into_iter().any(replaced),
        _ => true,
    }
}

// The game loads a resource from whichever archive reads it first, so a
// resized resource that another stock archive also carries can still show
// the stock copy. Each name is matched as it sits in an archive index.
fn stock_holders(game_dir: &Path, resized: &[Resized]) -> Vec<String> {
    let mut by_root: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut written = std::collections::BTreeSet::new();
    for r in resized {
        let root: Vec<&str> = r.erp.splitn(3, '/').take(2).collect();
        for (uri, _) in &r.resources {
            written.insert((r.erp.to_ascii_lowercase(), uri.as_str()));
            by_root.entry(root.join("/")).or_default().push(uri);
        }
    }
    let mut out = Vec::new();
    for (root, mut uris) in by_root {
        uris.sort_unstable();
        uris.dedup();
        let mut files = Vec::new();
        crate::erpbuild::erp_files(&game_dir.join(&root), &root, &mut files);
        files.sort();
        for rel in files {
            let Some(index) = crate::erpbuild::index_of(&game_dir.join(&rel)) else {
                continue;
            };
            for uri in &uris {
                if crate::erpbuild::index_holds(&index, uri)
                    && !written.contains(&(rel.to_ascii_lowercase(), *uri))
                {
                    out.push(format!(
                        "erp: {uri} is resized, but {rel} also carries it and keeps the stock copy; replace it there too, or the game may show either one"
                    ));
                }
            }
        }
    }
    out
}

fn put_files(
    built: &mut Built,
    store: &Store,
    game: &Game,
    mipmaps: BTreeMap<String, PathBuf>,
    ops: Vec<Op>,
    built_over: &BTreeSet<String>,
    car_maps: (&[ErsMap], &[HandlingMap]),
) -> Result<()> {
    let (ers_maps, handling_maps) = car_maps;
    let baseline = Baseline::load_or_new(&store.root, &game.identity).ok();
    let mut spelling: BTreeMap<String, String> = BTreeMap::new();
    for rel in built.plan.data.keys().chain(built.plan.added.keys()) {
        spelling.insert(rel.to_ascii_lowercase(), rel.clone());
    }
    if let Some(b) = &baseline {
        for rel in b.files.keys() {
            spelling.insert(rel.to_ascii_lowercase(), rel.clone());
        }
    }
    let mut put: BTreeMap<String, (String, PathBuf, Option<Origin>)> = BTreeMap::new();
    for (rel, src) in mipmaps {
        let key = rel.to_ascii_lowercase();
        let real = spelling.entry(key.clone()).or_insert(rel).clone();
        put.insert(key, (real, src, None));
    }
    let mips = put.len();
    for op in ops {
        let AssetOp::File(FileOp::Put { source, target }) = op.op else {
            continue;
        };
        if !relative_inside(&target) {
            return Err(Error::Resolve(format!(
                "{target} must be a relative path inside the game folder"
            )));
        }
        let src = source.resolve(&source_dir(store, &op.origin.addon, source.root))?;
        let key = target.to_ascii_lowercase();
        let real = spelling.entry(key.clone()).or_insert(target).clone();
        if let Some((_, _, Some(first))) = put.get(&key)
            && *first != op.origin
        {
            built.lines.push(format!(
                "files: {real} is put by both {first} and {}; {} wins",
                op.origin, op.origin
            ));
        }
        put.insert(key, (real, src, Some(op.origin)));
    }
    if put.is_empty() {
        return Ok(());
    }
    let over: BTreeSet<String> = built_over.iter().map(|r| r.to_ascii_lowercase()).collect();
    let stock = |rel: &str| {
        baseline.as_ref().is_some_and(|b| b.files.contains_key(rel))
            || (game.dir.join(rel).is_file() && !left_over(store, game, rel))
    };
    let (mut replaced, mut added, mut handling) = (0, 0, 0);
    let addon_dir = |id: &str| store.addons_dir().join(id);
    for (_, (rel, mut src, origin)) in put {
        let lower = rel.to_ascii_lowercase();
        let group = handling::for_source(handling_maps, &addon_dir, &src);
        if !group.is_empty() && !over.contains(&lower) {
            let out = built.overlay.join("handling").join(&rel);
            let mut lines = Vec::new();
            if handling::edit_archive(&src, &group, &out, &mut lines)? > 0 {
                src = out;
            }
            built.lines.extend(lines);
        }
        let car = !ers_maps.is_empty() && ers::is_car_archive(&rel);
        if car && !over.contains(&lower) {
            let out = built.overlay.join("ers").join(&rel);
            let n = ers::edit_archive(&src, ers_maps, &out)?;
            if n > 0 {
                handling += n;
                src = out;
            }
        }
        let is_stock = stock(&rel);
        if is_stock {
            replaced += 1;
        } else {
            added += 1;
        }
        if over.contains(&lower) {
            if !is_stock && let Some(archive) = built.plan.data.remove(&rel) {
                built.plan.added.insert(rel, archive);
            }
            continue;
        }
        let into = match is_stock {
            true => &mut built.plan.data,
            false => &mut built.plan.added,
        };
        if into.insert(rel.clone(), src).is_some() && !car {
            return Err(Error::Other(match origin {
                Some(o) => format!(
                    "{rel} is replaced twice: {o} puts the whole file, and an added track changes it too"
                ),
                None => format!("{rel} is replaced twice"),
            }));
        }
    }
    built
        .lines
        .push(format!("files: {replaced} game file(s) replaced, {added} added"));
    if handling > 0 {
        built.lines.push(format!(
            "ers: {handling} handling file(s) in car archives addons put get the tracks' settings"
        ));
    }
    if mips > 0 {
        built
            .lines
            .push(format!("mipmaps: {mips} mip file(s) go in the game folder"));
    }
    Ok(())
}

pub fn build_menus(
    built: &mut Built,
    session: &mut SessionSettings,
    original: &dyn Fn(&str) -> PathBuf,
    writer: &crate::ui::Writer,
    lang: bool,
) -> Result<Vec<(String, String)>> {
    let input = |rel: &str| built.plan.data.get(rel).cloned().unwrap_or_else(|| original(rel));
    let writer = |erp: &str, res: &str| {
        built
            .plan
            .data
            .contains_key(erp)
            .then(|| writer(erp, res))
            .flatten()
    };
    let menus = crate::ui::build(session, &input, &writer, &built.overlay, lang)?;
    built.lines.push(format!(
        "ui: ADDON SETTINGS with {} screen(s) under {LOADER_DIR}/ui, {} game archive(s) patched",
        menus.session.len(),
        menus.erps.len()
    ));
    if !menus.strings.is_empty() {
        built.lines.push(format!(
            "ui: the online services popup reads its text from {} language string(s)",
            menus.strings.len()
        ));
    }
    built.lines.extend(menus.lines);
    built.warnings.extend(menus.warnings);
    built.plan.data.extend(menus.erps);
    for (rel, src) in menus.screens {
        if built.plan.loader.insert(rel.clone(), src).is_some() {
            return Err(Error::Other(format!("{rel} is placed twice")));
        }
    }
    let ui = session.ui.get_or_insert_with(|| SessionUi {
        register: true,
        strict: false,
        recover: true,
        screens: Vec::new(),
    });
    if let Some(s) = ui
        .screens
        .iter()
        .find(|s| menus.session.iter().any(|m| m.name == s.name))
    {
        return Err(Error::Other(format!(
            "screen {} is reserved for ADDON SETTINGS; rename it in its addon",
            s.name
        )));
    }
    ui.screens.retain(|s| s.page.is_none());
    ui.screens.splice(0..0, menus.session);
    Ok(menus.strings)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn a_file_another_program_holds_gets_its_own_hint() {
        let (data, game) = (Path::new("/data/Swofty"), Path::new("/games/F1 25"));
        let held = game_refused_text(data, game, true);
        assert!(
            held.starts_with("another program has a file in /games/F1 25 open"),
            "{held}"
        );
        assert!(
            held.contains("F1_25.exe") && held.contains("/data/Swofty"),
            "{held}"
        );
        assert!(!held.contains("administrator"), "{held}");
        assert!(game_refused_text(data, game, false).contains("run Swofty Launcher as administrator"));
    }

    #[cfg(windows)]
    #[test]
    fn a_sharing_violation_in_the_game_folder_is_not_an_administrator_problem() {
        let (data, game) = (
            Path::new("C:/data/Swofty"),
            Path::new("E:/SteamLibrary/steamapps/common/F1 25"),
        );
        let held = Error::Io {
            path: game.join("2025_asset_groups/animation_package/scenes/incardriver/cs16/incardriver_f1.erp"),
            source: std::io::Error::from_raw_os_error(32),
        };
        let text = refused_fix(data, game, &held).unwrap();
        assert!(text.starts_with("another program has a file in"), "{text}");
    }
    use crate::asset::{AssetBatch, Op, Origin};
    use crate::asset::{FileOp, Root, Source};
    use crate::baseline::Identity;
    use crate::launcher::{Addon, Builtin};

    fn uncached_source_hash(hasher: &mut Sha256, store: &Store, ops: &[AssetOp]) {
        for src in ops.iter().flat_map(AssetOp::sources) {
            let path = src.resolve(&source_dir(store, "test.pack", src.root)).unwrap();
            let meta = std::fs::metadata(&path).unwrap();
            hasher.update(format!("{}|{}|{:?}", src.path, meta.len(), meta.modified().ok()));
            if meta.is_dir() {
                for (rel, p) in loader_files(&path).unwrap() {
                    let meta = std::fs::metadata(&p).unwrap();
                    hasher.update(format!("{rel}|{}|{:?}", meta.len(), meta.modified().ok()));
                }
            }
        }
    }

    #[test]
    fn source_stamp_cache_preserves_digest_and_rechecks_each_batch() {
        let root = std::env::temp_dir().join(format!("swofty-source-stamps-{}", std::process::id()));
        let store = Store::new(&root);
        let dir = store.addon_data_dir("test.pack").join("assets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.bin"), b"a").unwrap();
        let ops: Vec<_> = ["assets", "assets/a.bin", "assets"]
            .into_iter()
            .map(|path| {
                AssetOp::File(FileOp::Put {
                    source: Source {
                        root: Root::Data,
                        path: path.into(),
                    },
                    target: "out.bin".into(),
                })
            })
            .collect();
        let digest = || {
            let mut cached = Sha256::new();
            hash_sources(&mut cached, &store, "test.pack", &ops).unwrap();
            let mut old = Sha256::new();
            uncached_source_hash(&mut old, &store, &ops);
            let cached = cached.finalize();
            assert_eq!(cached, old.finalize());
            cached
        };
        let before = digest();
        std::fs::write(dir.join("a.bin"), b"changed by helper").unwrap();
        assert_ne!(before, digest());
        let before = digest();
        std::fs::write(dir.join("new.bin"), b"new").unwrap();
        assert_ne!(before, digest());
        std::fs::remove_file(dir.join("a.bin")).unwrap();
        assert!(hash_sources(&mut Sha256::new(), &store, "test.pack", &ops).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "manual repeated-source build fingerprint benchmark"]
    fn benchmark_repeated_build_sources() {
        let root = std::env::temp_dir().join(format!("swofty-source-bench-{}", std::process::id()));
        let store = Store::new(&root);
        let dir = store.addons_dir().join("test.pack/assets");
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..512 {
            std::fs::write(dir.join(format!("{i}.bin")), b"asset").unwrap();
        }
        let ops: Vec<_> = (0..128)
            .map(|i| {
                AssetOp::File(FileOp::Put {
                    source: Source::addon("assets"),
                    target: format!("out/{i}"),
                })
            })
            .collect();
        let start = std::time::Instant::now();
        let mut old = Sha256::new();
        uncached_source_hash(&mut old, &store, &ops);
        println!("128 references / 512 files, uncached: {:?}", start.elapsed());
        let start = std::time::Instant::now();
        let mut cached = Sha256::new();
        hash_sources(&mut cached, &store, "test.pack", &ops).unwrap();
        println!("128 references / 512 files, cached: {:?}", start.elapsed());
        assert_eq!(cached.finalize(), old.finalize());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn only_a_core_with_the_rows_feature_takes_the_replaced_operations_of_addons_with_data_files() {
        let root = std::env::temp_dir().join(format!("swofty-replaced-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let features = root.join(crate::planner::FEATURES_FILE);
        std::fs::create_dir_all(features.parent().unwrap()).unwrap();
        let std = |id: &str, data: bool, registry: Option<&str>| crate::stdlib::deps::Std {
            id: id.into(),
            name: id.into(),
            version: semver::Version::new(1, 0, 0),
            registry: registry.map(|r| crate::stdlib::manifest::RegistryManifest::parse(r).unwrap()),
            lock: None,
            data_files: match data {
                true => vec!["registry/content.json".into()],
                false => Vec::new(),
            },
            records: Vec::new(),
            files: Default::default(),
            unreadable: Vec::new(),
            older_tracks: Vec::new(),
        };
        let replaces = r#"{ "replaced_operations": ["db/records.json"] }"#;
        let std16 = BTreeMap::from([
            ("a.season".to_owned(), std("a.season", true, Some(replaces))),
            ("b.nodata".to_owned(), std("b.nodata", false, Some(replaces))),
            (
                "c.plain".to_owned(),
                std("c.plain", true, Some(r#"{ "requires": [] }"#)),
            ),
            ("d.old".to_owned(), std("d.old", true, None)),
        ]);
        assert!(replaced_operations(&std16, None).is_empty(), "no loader");
        std::fs::write(&features, r#"{ "schema": 1, "features": { "db": "stable" } }"#).unwrap();
        assert!(
            replaced_operations(&std16, Some(&root)).is_empty(),
            "a core without the rows feature"
        );
        std::fs::write(
            &features,
            r#"{ "schema": 1, "features": { "db": "stable", "registry_rows": "stable" } }"#,
        )
        .unwrap();
        let r = replaced_operations(&std16, Some(&root));
        assert_eq!(r.keys().copied().collect::<Vec<_>>(), ["a.season"]);
        assert!(r["a.season"].replaces("db/records.json") && !r["a.season"].replaces("db/season.json"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn registry_records_turn_the_early_stage_on_whatever_the_database_route() {
        let std = |id: &str, data: bool, lock: Option<&str>, registry: bool| crate::stdlib::deps::Std {
            id: id.into(),
            name: format!("{id} name"),
            version: semver::Version::new(1, 0, 0),
            registry: registry.then(|| crate::stdlib::manifest::RegistryManifest::parse("{}").unwrap()),
            lock: lock.map(|l| crate::stdlib::lock::Lock::parse(l).unwrap()),
            data_files: match data {
                true => vec!["registry/teams.json".into()],
                false => Vec::new(),
            },
            records: Vec::new(),
            files: Default::default(),
            unreadable: Vec::new(),
            older_tracks: Vec::new(),
        };
        let registers = r#"{ "provides": { "team.identity": [ { "id": "dll.teams/hrt" } ] } }"#;
        let std16 = BTreeMap::from([
            ("a.data".to_owned(), std("a.data", true, None, true)),
            ("b.code".to_owned(), std("b.code", false, Some(registers), false)),
            ("c.needs".to_owned(), std("c.needs", false, Some("{}"), true)),
            ("d.off".to_owned(), std("d.off", true, None, false)),
        ]);
        let order: Vec<String> = ["swofty.core", "c.needs", "b.code", "a.data", "e.old"]
            .map(str::to_owned)
            .to_vec();
        let on = registering_addons(&order, &std16);
        assert_eq!(on, ["b.code name", "a.data name"]);
        assert_eq!(
            early_stage_line(&on),
            "registry: the core loads at the early stage for the registry records of b.code name, a.data name"
        );
        assert!(
            registering_addons(&order[..2], &std16).is_empty(),
            "a registry.json or a lock without registrations leaves it off"
        );
    }

    #[test]
    fn a_missing_or_cut_short_file_names_its_addon_or_the_game() {
        let root = std::env::temp_dir().join(format!("swofty-named-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let store = Store::new(root.join("data"));
        let game = root.join("game");
        let addon = store.addons_dir().join("mod.miami24");
        std::fs::create_dir_all(addon.join("track")).unwrap();
        std::fs::write(
            addon.join("swofty.json"),
            r#"{"schema_version":1,"id":"mod.miami24","name":"Igora Drive","version":"0.9.13","author":"t","compat":{"game":"f1_25"}}"#,
        )
        .unwrap();
        let io = |path: PathBuf, kind| Error::Io {
            path,
            source: std::io::Error::from(kind),
        };
        let gone = named(
            &store,
            &game,
            io(
                addon.join("track").join("miami24.erp"),
                std::io::ErrorKind::NotFound,
            ),
        );
        assert_eq!(
            gone.to_string(),
            "Igora Drive (mod.miami24) is missing its file track/miami24.erp. Turn Igora Drive off in this \
             profile, or remove it on the Addons page and install or import it again."
        );
        let short = named(
            &store,
            &game,
            Error::Formats(swofty_formats::Error::IoAt {
                path: addon.join("track/mipmaps/a.mipmaps"),
                source: std::io::Error::from(std::io::ErrorKind::UnexpectedEof),
            }),
        );
        assert!(
            short.to_string().starts_with(
                "Igora Drive (mod.miami24) has an incomplete or damaged track/mipmaps/a.mipmaps."
            ),
            "{short}"
        );
        let stock = named(
            &store,
            &game,
            io(
                game.join("2025_asset_groups/a.erp"),
                std::io::ErrorKind::UnexpectedEof,
            ),
        );
        assert!(
            stock
                .to_string()
                .starts_with("2025_asset_groups/a.erp in the game folder is incomplete or damaged; verify"),
            "{stock}"
        );
        let missing = named(
            &store,
            &game,
            io(
                game.join("character_package/a.mipmaps"),
                std::io::ErrorKind::NotFound,
            ),
        );
        assert!(
            missing
                .to_string()
                .starts_with("character_package/a.mipmaps is missing from the game folder; update F1 25"),
            "{missing}"
        );
        let denied = named(
            &store,
            &game,
            io(
                addon.join("track/miami24.erp"),
                std::io::ErrorKind::PermissionDenied,
            ),
        );
        assert!(matches!(denied, Error::Io { .. }), "{denied}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_missing_backup_stops_the_build_and_sends_the_player_through_the_setup_again() {
        let _pin = crate::builtin::LoaderPin::none();
        let root = std::env::temp_dir().join(format!("swofty-build-backup-gone-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join("a.erp"), "stock").unwrap();
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        let records = vec![swofty_formats::gvdb::Record::new(
            "entrants.teams._team_instancecount_",
            0,
            "0",
        )];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        crate::setup::reset(&store.root, &game, false).unwrap();
        crate::setup::finish(&store.root).unwrap();
        let objects = Objects {
            root: store.root.join("baseline/objects"),
        };
        let mut baseline = Baseline::new(&game.identity);
        let sha = baseline
            .record(&objects, &dir, "a.erp", &())
            .unwrap()
            .sha256
            .clone();
        baseline.save(&store.root).unwrap();

        build(&store, "default", &game).unwrap();
        assert!(crate::setup::done(&store.root));

        std::fs::remove_dir_all(objects.path(&sha).parent().unwrap()).unwrap();
        let e = build(&store, "default", &game).unwrap_err();
        assert_eq!(
            e.to_string(),
            "Swofty's backup of the original a.erp is missing from its data folder. Run the one-time setup \
             again: open Swofty Launcher and press Play, or run `swofty setup reset`, repair the game in the EA \
             app or verify the game files in Steam, then run `swofty setup done`"
        );
        assert!(!crate::setup::done(&store.root));
        assert!(
            build(&store, crate::profile::VANILLA, &game).is_ok(),
            "a build without addons reads no backups"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    struct Hook(AddonManifest);

    impl Addon for Hook {
        fn manifest(&self) -> &AddonManifest {
            &self.0
        }

        fn build(&self, cx: &BuildCx) -> swofty_addon::Result<AssetBatch> {
            let origin = Origin {
                addon: cx.cx.addon.into(),
                item: None,
            };
            Ok(AssetBatch {
                ops: vec![
                    Op {
                        op: AssetOp::Warn {
                            message: format!("{} cars", cx.options["cars"]),
                        },
                        origin: origin.clone(),
                    },
                    Op {
                        op: AssetOp::Erp(crate::ops::ErpOp::PatchPkg {
                            erp: "2025_asset_groups/missing.erp".into(),
                            resource: "eaid://missing".into(),
                            fragment: 0,
                            edits: vec![crate::ops::PkgEdit::SetText {
                                node: "n".into(),
                                value: "v".into(),
                                localise: false,
                            }],
                        }),
                        origin,
                    },
                ],
                lines: vec![
                    format!("profile {}", cx.cx.profile.name),
                    match cx
                        .cx
                        .addons()
                        .map(|a| (a.list(), a.set_enabled(cx.cx.addon, true)))
                    {
                        Some((Ok(_), Ok(()))) => "turned on".to_owned(),
                        Some((listed, turned)) => format!("{:?} {:?}", listed.err(), turned.err()),
                        None => "no addons".to_owned(),
                    },
                ],
                ..AssetBatch::default()
            })
        }
    }

    #[test]
    fn warns_about_stock_archives_that_still_carry_a_resized_resource() {
        use swofty_formats::erp::{Erp, Fragment, HEADER_LEN, MAGIC, Resource, STORED};
        let dir = std::env::temp_dir().join(format!("swofty-build-holders-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let ui = "2025_asset_groups/ui_package";
        let scene = "eaid://ui_package/art/screens/career/rnd_overview.idf";
        let archive = |names: &[&str]| {
            let mut header = [0u8; HEADER_LEN];
            header[..4].copy_from_slice(MAGIC);
            header[4..8].copy_from_slice(&3u32.to_le_bytes());
            Erp {
                header,
                version: 3,
                resources: names
                    .iter()
                    .map(|n| Resource {
                        name: (*n).into(),
                        kind: swofty_formats::erp::kind("UILayout").unwrap(),
                        unk: vec![0; 4],
                        fragments: vec![Fragment {
                            name: *b"temp",
                            offset: 0,
                            size: 1,
                            flags: 0x10,
                            compression: STORED,
                            packed: 1,
                            raw: vec![0],
                        }],
                        hash: [0; 16],
                    })
                    .collect(),
            }
            .to_bytes()
        };
        for (rel, names) in [
            ("flow_career_hub.erp", vec![scene]),
            ("flow_team_hq.erp", vec!["eaid://ui_package/x.idf", scene]),
            ("screens/laptop.erp", vec![scene]),
            (
                "screens/other.erp",
                vec!["eaid://ui_package/art/screens/career/rnd_overview.idf_b"],
            ),
        ] {
            let path = dir.join(ui).join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, archive(&names)).unwrap();
        }
        std::fs::write(dir.join(ui).join("notes.txt"), b"ERPK").unwrap();
        let resized = |erp: &str| Resized {
            erp: format!("{ui}/{erp}"),
            resources: vec![(scene.into(), "fragment 0 is 2 bytes, the stock one 1".into())],
        };
        assert_eq!(
            stock_holders(
                &dir,
                &[resized("flow_career_hub.erp"), resized("screens/laptop.erp")]
            ),
            [format!(
                "erp: {scene} is resized, but {ui}/flow_team_hq.erp also carries it and keeps the stock copy; replace it there too, or the game may show either one"
            )]
        );
        assert!(
            stock_holders(
                &dir,
                &[
                    resized("flow_career_hub.erp"),
                    resized("flow_team_hq.erp"),
                    resized("screens/laptop.erp")
                ]
            )
            .is_empty()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    fn logo_archive(name: &str, slices: &[u8]) -> Vec<u8> {
        use swofty_formats::erp::{Erp, Fragment, HEADER_LEN, MAGIC, Resource, STORED};
        let mut head = vec![0u8; 40];
        head[12..16].copy_from_slice(&2u32.to_le_bytes());
        head[28..32].copy_from_slice(&(slices.len() as u32).to_le_bytes());
        let pixels: Vec<u8> = slices.iter().flat_map(|&b| [b; 4]).collect();
        let fragment = |raw: Vec<u8>| Fragment {
            name: *b"temp",
            offset: 0,
            size: raw.len() as u64,
            flags: 0x10,
            compression: STORED,
            packed: raw.len() as u64,
            raw,
        };
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        header[4..8].copy_from_slice(&3u32.to_le_bytes());
        Erp {
            header,
            version: 3,
            resources: vec![Resource {
                name: name.into(),
                kind: swofty_formats::erp::kind("GfxSurfaceRes").unwrap(),
                unk: vec![0; 4],
                fragments: vec![fragment(head), fragment(pixels)],
                hash: [0; 16],
            }],
        }
        .to_bytes()
    }

    #[test]
    fn the_core_ships_the_startup_intro_and_a_later_addon_replaces_it() {
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-intro-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let records = vec![swofty_formats::gvdb::Record::new(
            "entrants.teams._team_instancecount_",
            0,
            "0",
        )];
        std::fs::write(dir.join(GVDB), swofty_formats::gvdb::Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let video = "videos/cm_f1_sting.bk2";
        let staged = store.addon_cache_dir(CORE_ID).join(video);

        let built = build(&store, "default", &game).unwrap();
        assert_eq!(std::fs::read(&staged).unwrap(), crate::builtin::CORE_INTRO_VIDEO);
        let put = |b: &Built| b.plan.data.get(video).or(b.plan.added.get(video)).cloned();
        assert_eq!(put(&built), Some(staged));
        assert!(built.conflicts.is_empty());

        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("swofty.json"),
            r#"{ "schema_version": 1, "id": "t.video", "name": "Video", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" }, "operations": { "files": [{ "source": "v.bk2", "target": "videos/cm_f1_sting.bk2" }], "assets": ["t.json"] } }"#,
        )
        .unwrap();
        std::fs::write(
            src.join("t.json"),
            r#"{ "schema_version": 1, "ops": [{ "op": "lang.set", "language": "*", "key": "lng_subtitle_ea_sports_intro", "value": "MINE" }] }"#,
        )
        .unwrap();
        std::fs::write(src.join("v.bk2"), "mine").unwrap();
        store.install_addon(&src).unwrap();
        store
            .update_profile("default", |p| p.set_enabled("t.video", true))
            .unwrap();
        let replaced = build(&store, "default", &game).unwrap();
        assert!(replaced.conflicts.is_empty(), "{:?}", replaced.conflicts);
        let mine = put(&replaced).unwrap();
        assert_eq!(std::fs::read(mine).unwrap(), b"mine");

        store
            .update_profile("default", |p| p.set_enabled("t.video", false))
            .unwrap();
        store
            .update_profile("default", |p| {
                p.set_option(
                    &crate::builtin::core_manifest(),
                    "startup_intro",
                    serde_json::json!(false),
                )
                .map(|_| ())
            })
            .unwrap();
        let off = build(&store, "default", &game).unwrap();
        assert_eq!(put(&off), None);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn merged_slices_conflict_one_slice_at_a_time() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-slices-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        let ui = "2025_asset_groups/ui_package/common_front_end.erp";
        let logos = "eaid://ui_package/art/texture_arrays/t.texarray.image";
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::create_dir_all(dir.join("2025_asset_groups/ui_package")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let records = vec![Record::new("entrants.teams._team_instancecount_", 0, "0")];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        std::fs::write(dir.join(ui), logo_archive(logos, &[0, 1, 2, 3])).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        for (id, slices) in [
            ("t.a", [0, 9, 2, 3]),
            ("t.b", [0, 1, 2, 8]),
            ("t.c", [0, 7, 2, 3]),
            ("t.same", [0, 1, 2, 3]),
        ] {
            let src = root.join("src").join(id);
            std::fs::create_dir_all(&src).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{id}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}, "operations": {{ "assets": ["ops.json"] }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(
                src.join("ops.json"),
                format!(
                    r#"{{ "schema_version": 1, "ops": [{{ "op": "erp.merge-slices", "erp": "{ui}", "resource": "{logos}", "source": "logos.erp" }}] }}"#
                ),
            )
            .unwrap();
            std::fs::write(src.join("logos.erp"), logo_archive(logos, &slices)).unwrap();
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled(id, true))
                .unwrap();
        }
        let built = build(&store, "default", &game).unwrap();
        let conflicts: Vec<String> = built.conflicts.iter().map(ToString::to_string).collect();
        assert_eq!(
            conflicts,
            [format!("slice 1 of {logos} in {ui}: t.c overrides t.a")]
        );
        for (id, n) in [("t.a", 1), ("t.b", 1), ("t.c", 1)] {
            let line = format!("erp: {id}: {n} of 4 slice(s) of {logos} merged into {ui}");
            assert!(built.lines.contains(&line), "{line}: {:?}", built.lines);
        }
        let file = swofty_formats::erp::ErpFile::open(&built.plan.data[ui]).unwrap();
        let merged = file.load(&file.erp.resources[0]).unwrap();
        let slices: Vec<u8> = merged.fragments[1]
            .data()
            .unwrap()
            .chunks(4)
            .map(|c| c[0])
            .collect();
        assert_eq!(slices, [0, 7, 2, 8]);
        store
            .update_profile("default", |p| {
                ["t.a", "t.b", "t.c"]
                    .iter()
                    .try_for_each(|id| p.set_enabled(id, false))
            })
            .unwrap();
        let same = build(&store, "default", &game).unwrap();
        assert!(!same.plan.data.contains_key(ui), "{:?}", same.plan.data);
        assert!(!same.overlay.join(ui).exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn overlays_go_in_resource_by_resource_before_a_later_merge_into_them() {
        let root = std::env::temp_dir().join(format!("swofty-build-merged-later-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("ready.erp");
        let logos = "eaid://ui_package/t.texarray.image";
        std::fs::write(&path, logo_archive(logos, &[1, 2])).unwrap();
        let op = |op: &str| Op {
            op: serde_json::from_str(op).unwrap(),
            origin: Origin {
                addon: "t".into(),
                item: None,
            },
        };
        let ops = [
            op(r#"{"op":"erp.overlay","erps":["a.erp","b.erp"],"source":"ready.erp"}"#),
            op(&format!(
                r#"{{"op":"erp.merge-slices","erp":"b.erp","resource":"{logos}","source":"x.erp"}}"#
            )),
            op(&format!(
                r#"{{"op":"erp.merge-slices","erp":"c.erp","resource":"{logos}","source":"x.erp"}}"#
            )),
        ];
        let slices = slice_merges(&ops);
        assert_eq!(slices, [(1, "b.erp", logos), (2, "c.erp", logos)]);
        let targets = ["a.erp".to_owned(), "b.erp".to_owned()];
        assert_eq!(
            merged_later(&slices, 0, &targets, &path).unwrap(),
            Some(format!("a later erp.merge-slices merges into {logos} in b.erp"))
        );
        assert_eq!(merged_later(&slices, 1, &targets, &path).unwrap(), None);
        assert_eq!(merged_later(&slices, 0, &targets[..1], &path).unwrap(), None);
        std::fs::remove_dir_all(&root).ok();
    }

    struct Grid(AddonManifest);

    impl Addon for Grid {
        fn manifest(&self) -> &AddonManifest {
            &self.0
        }

        fn build(&self, cx: &BuildCx) -> swofty_addon::Result<AssetBatch> {
            let op: AssetOp = serde_json::from_value(serde_json::json!({
                "op": "db.set", "select": "t.list[_metadata_=cs_f1_2016]", "field": "name", "value": "edited"
            }))
            .map_err(|e| swofty_addon::Error::Other(e.to_string()))?;
            Ok(AssetBatch {
                ops: vec![Op {
                    op,
                    origin: Origin {
                        addon: cx.cx.addon.into(),
                        item: Some("grid-editor".into()),
                    },
                }],
                ..AssetBatch::default()
            })
        }
    }

    #[test]
    fn an_editor_op_on_a_row_a_later_addon_adds_applies() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-late-row-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let records = vec![
            Record::new("t._list_instancecount_", 0, "1"),
            Record::new("t.list_0._metadata_", 4, "f1"),
            Record::new("t.list_0.name", 4, "stock"),
        ];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let manifest = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "t.grid", "name": "Grid", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" } }"#,
        )
        .unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![Builtin {
            addon: Arc::new(Grid(manifest)),
            locked: false,
            default_on: true,
        }]);
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("swofty.json"),
            r#"{ "schema_version": 1, "id": "t.seasons", "name": "Seasons", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" }, "operations": { "database": ["db.json"] } }"#,
        )
        .unwrap();
        std::fs::write(
            src.join("db.json"),
            r#"{ "schema_version": 1, "fields": { "t.list": ["_metadata_", "name"] }, "ops": [
                 { "op": "add-row", "table": "t", "from": "t.list[_metadata_=f1]", "set": { "_metadata_": "cs_f1_2016", "name": "seasons" } } ] }"#,
        )
        .unwrap();
        store.install_addon(&src).unwrap();
        store
            .update_profile("default", |p| p.set_enabled("t.seasons", true))
            .unwrap();

        let built = build(&store, "default", &game).unwrap();
        assert_eq!(built.order, [CORE_ID, "t.grid", "t.seasons"]);
        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(out.get_text("t.list_1._metadata_").as_deref(), Some("cs_f1_2016"));
        assert_eq!(out.get_text("t.list_1.name").as_deref(), Some("edited"));

        store
            .update_profile("default", |p| p.set_enabled("t.seasons", false))
            .unwrap();
        let e = build(&store, "default", &game).unwrap_err().to_string();
        assert!(
            e.contains("applying t.grid (grid-editor): t.list[_metadata_=cs_f1_2016]: no row matches, even after every addon's changes"),
            "{e}"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn ops_and_options_that_need_an_addon_apply_only_while_the_build_has_it_on() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-when-addon-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let fields = ["round", "sprint", "laps", "whole"];
        let records = std::iter::once(Record::new("t._list_instancecount_", 0, "1"))
            .chain(std::iter::once(Record::new("t.list_0._metadata_", 4, "f2")))
            .chain(
                fields
                    .iter()
                    .map(|f| Record::new(&format!("t.list_0.{f}"), 4, "off")),
            )
            .collect();
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let track = root.join("track");
        std::fs::create_dir_all(&track).unwrap();
        std::fs::write(
            track.join("swofty.json"),
            r#"{ "schema_version": 1, "id": "t.istanbul", "name": "Istanbul Park", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" } }"#,
        )
        .unwrap();
        store.install_addon(&track).unwrap();
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("swofty.json"),
            r#"{ "schema_version": 1, "id": "t.f2", "name": "F2", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" },
                 "options": [
                   { "id": "sprint", "type": "toggle", "default": true, "binds": "db" },
                   { "id": "istanbul_park", "type": "toggle", "default": true, "binds": "db", "needs": ["t.istanbul"] }
                 ],
                 "operations": { "database": ["rounds.json", "whole.json"] } }"#,
        )
        .unwrap();
        let set = |field: &str, when: &str| {
            format!(
                r#"{{ "op": "set", "select": "t.list[_metadata_=f2]", "field": "{field}", "value": "on", "when": {when} }}"#
            )
        };
        std::fs::write(
            src.join("rounds.json"),
            format!(
                r#"{{ "schema_version": 1, "ops": [{}, {}, {}] }}"#,
                set("round", r#"{ "addon": "t.istanbul" }"#),
                set("sprint", r#"{ "addon": "t.istanbul", "sprint": true }"#),
                set("laps", r#"{ "istanbul_park": true }"#)
            ),
        )
        .unwrap();
        std::fs::write(
            src.join("whole.json"),
            r#"{ "schema_version": 1, "when": { "addon": "t.istanbul" }, "ops": [
                 { "op": "set", "select": "t.list[_metadata_=f2]", "field": "whole", "value": "on" } ] }"#,
        )
        .unwrap();
        store.install_addon(&src).unwrap();
        store
            .update_profile("default", |p| p.set_enabled("t.f2", true))
            .unwrap();
        let values = || {
            let built = build(&store, "default", &game).unwrap();
            let path = built
                .plan
                .data
                .get(GVDB)
                .cloned()
                .unwrap_or_else(|| game.dir.join(GVDB));
            let out = Gvdb::parse(&std::fs::read(path).unwrap()).unwrap();
            fields.map(|f| out.get_text(&format!("t.list_0.{f}")).unwrap())
        };
        assert_eq!(values(), ["off", "off", "off", "off"]);
        store
            .update_profile("default", |p| p.set_enabled("t.istanbul", true))
            .unwrap();
        assert_eq!(values(), ["on", "on", "on", "on"]);
        let f2 = store.registry().unwrap().manifests()["t.f2"].clone();
        store
            .update_profile("default", |p| p.set_option(&f2, "sprint", false.into()).map(drop))
            .unwrap();
        assert_eq!(values(), ["on", "off", "on", "on"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_option_whose_rounds_need_a_track_the_build_lacks_builds_as_off_and_says_so() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-trackless-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let tracks = ["melbourne", "spa_francorchamps", "portimao"];
        let mut records = vec![
            Record::new("tracks._Track_instancecount_", 0, "3"),
            Record::new(
                "schedule.activities._activity_race_weekend_instancecount_",
                0,
                "2",
            ),
            Record::new("schedule.all_track_events._fixed_event_instancecount_", 2, "2"),
            Record::new("t._list_instancecount_", 0, "1"),
            Record::new("t.list_0._metadata_", 4, "f2"),
        ];
        for (i, code) in tracks.iter().enumerate() {
            records.push(Record::new(&format!("tracks.Track_{i}._metadata_"), 4, *code));
        }
        for (i, code) in tracks[1..].iter().enumerate() {
            let weekend = format!("schedule.activities.activity_race_weekend_{i}");
            records.push(Record::new(&format!("{weekend}._metadata_"), 4, *code));
            records.push(Record::new(&format!("{weekend}.track"), 4, *code));
            let event = format!("schedule.all_track_events.fixed_event_{i}");
            records.push(Record::new(
                &format!("{event}._metadata_"),
                4,
                format!("rw_{code}"),
            ));
            records.push(Record::new(&format!("{event}.activity"), 4, *code));
        }
        let fields = ["istanbul", "spa", "portimao"];
        records.extend(
            fields
                .iter()
                .map(|f| Record::new(&format!("t.list_0.{f}"), 4, "off")),
        );
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("swofty.json"),
            r#"{ "schema_version": 1, "id": "t.f2", "name": "F2", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" },
                 "options": [
                   { "id": "istanbul", "type": "toggle", "label": "Turkish GP", "default": true, "binds": "db" },
                   { "id": "spa", "type": "toggle", "default": true, "binds": "db" },
                   { "id": "portimao", "type": "toggle", "label": "Portuguese GP", "default": true, "binds": "db" }
                 ],
                 "operations": { "database": ["rounds.json"] } }"#,
        )
        .unwrap();
        let set = |field: &str| {
            format!(
                r#"{{ "op": "set", "select": "t.list[_metadata_=f2]", "field": "{field}", "value": "on", "when": {{ "{field}": true }} }}"#
            )
        };
        let row = |option: &str, from: &str| {
            format!(
                r#"{{ "op": "add-row", "table": "schedule.all_track_events",
                     "from": "schedule.all_track_events.fixed_event[_metadata_={from}]",
                     "set": {{ "_metadata_": "rw_{option}_27" }}, "when": {{ "{option}": true }} }}"#
            )
        };
        std::fs::write(
            src.join("rounds.json"),
            format!(
                r#"{{ "schema_version": 1, "ops": [
                     {{ "op": "add-row", "table": "schedule.activities",
                        "from": "schedule.activities.activity_race_weekend[_metadata_=spa_francorchamps]",
                        "set": {{ "_metadata_": "istanbul_park", "track": "istanbul_park" }}, "when": {{ "istanbul": true }} }},
                     {}, {}, {}, {}, {} ] }}"#,
                row("spa", "rw_spa_francorchamps"),
                row("portimao", "rw_portimao"),
                set("istanbul"),
                set("spa"),
                set("portimao")
            ),
        )
        .unwrap();
        store.install_addon(&src).unwrap();
        store
            .update_profile("default", |p| p.set_enabled("t.f2", true))
            .unwrap();
        let built = build(&store, "default", &game).unwrap();
        let path = built
            .plan
            .data
            .get(GVDB)
            .cloned()
            .unwrap_or_else(|| game.dir.join(GVDB));
        let out = Gvdb::parse(&std::fs::read(path).unwrap()).unwrap();
        let values = fields.map(|f| out.get_text(&format!("t.list_0.{f}")).unwrap());
        assert_eq!(values, ["off", "on", "off"]);
        assert_eq!(
            out.instance_count("schedule.activities", "activity_race_weekend"),
            Some(2)
        );
        let warned = |track: &str| {
            built
                .warnings
                .iter()
                .filter(|w| w.contains(&format!("adds rounds at {track},")))
                .count()
        };
        assert_eq!(
            (
                warned("istanbul_park"),
                warned("portimao"),
                warned("spa_francorchamps")
            ),
            (1, 1, 0),
            "{:#?}",
            built.warnings
        );
        assert!(
            built
                .warnings
                .iter()
                .any(|w| w.starts_with("F2: option Turkish GP adds rounds at istanbul_park"))
        );
        let f2 = store.registry().unwrap().manifests()["t.f2"].clone();
        assert_eq!(
            store.profile("default").unwrap().option(&f2, &f2.options[0]),
            serde_json::Value::Bool(true)
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_tables_a_records_file_adds_rows_to_are_the_add_row_tables() {
        let root = std::env::temp_dir().join(format!("swofty-added-tables-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("records.json");
        std::fs::write(
            &file,
            r#"{ "schema_version": 1, "ops": [
                 { "op": "set", "select": "a.b[_metadata_=x]", "field": "f", "value": "v" },
                 { "op": "add-row", "table": "entrants.entrant_lists", "from": "entrants.entrant_lists.entrant_list[_metadata_=f1]", "set": {} },
                 { "op": "add-row", "table": "entrants.drivers", "from": "entrants.drivers.driver[_metadata_=d]", "set": {} },
                 { "op": "add-row", "table": "entrants.drivers", "from": "entrants.drivers.driver[_metadata_=d]", "set": {} } ] }"#,
        )
        .unwrap();
        assert_eq!(
            added_tables(&file).into_iter().collect::<Vec<_>>(),
            ["entrants.drivers", "entrants.entrant_lists"]
        );
        assert!(added_tables(&root.join("gone.json")).is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn runs_the_build_hooks_of_builtin_addons() {
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-hook-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let manifest = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "t.hook", "name": "Hook", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" },
                 "options": [{ "id": "cars", "type": "slider", "label": "Cars", "min": 22, "max": 30, "step": 2, "default": 22, "binds": "db" }] }"#,
        )
        .unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![Builtin {
            addon: Arc::new(Hook(manifest)),
            locked: false,
            default_on: true,
        }]);
        let checked = build_as(&store, "default", &game, true).unwrap();
        assert!(
            checked.lines.contains(
                &"t.hook: None Some(Other(\"addons can't be changed while conflicts are checked\"))"
                    .to_owned()
            ),
            "{:?}",
            checked.lines
        );
        let built = build(&store, "default", &game).unwrap();
        assert_eq!(built.order, [CORE_ID, "t.hook"]);
        assert!(built.lines.contains(&"t.hook: profile default".to_owned()));
        assert!(built.lines.contains(&"t.hook: turned on".to_owned()));
        assert!(built.warnings.contains(&"t.hook: 22 cars".to_owned()));
        assert!(
            built
                .warnings
                .contains(&"t.hook: ERP operations skipped; set-text is not built yet".to_owned())
        );
        assert!(store.addon_data_dir("t.hook").is_dir());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn fingerprints_count_what_the_build_made_not_the_rest_of_the_folders() {
        if loader_dir().is_some() {
            return;
        }
        let hook = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "t.hook", "name": "Hook", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" },
                 "options": [{ "id": "cars", "type": "slider", "label": "Cars", "min": 22, "max": 30, "step": 2, "default": 22, "binds": "db" }] }"#,
        )
        .unwrap();
        let prints = |name: &str, zip: &str, texture: &str| {
            let root = std::env::temp_dir().join(format!("swofty-build-print-{name}-{}", std::process::id()));
            std::fs::remove_dir_all(&root).ok();
            let dir = root.join("game");
            std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
            std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
            std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
            let game = Game {
                identity: Identity::detect(&dir, Some(1)).unwrap(),
                dir,
                user_reg: None,
            };
            let store = Store::new(root.join("data")).with_builtins(vec![Builtin {
                addon: Arc::new(Hook(hook.clone())),
                locked: false,
                default_on: true,
            }]);
            let src = root.join("src");
            std::fs::create_dir_all(&src).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "mod.track", "name": "Track", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }},
                         "operations": {{ "assets": ["ops.json"], "files": [{{ "source": "a.dds", "target": "x/a.dds" }}] }},
                         "origin": {{ "tool": "teamsimplified.mods", "tool_version": "0.11.6", "file": "{zip}", "sha256": "ab" }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(src.join("ops.json"), r#"{ "schema_version": 1, "ops": [] }"#).unwrap();
            std::fs::write(src.join("a.dds"), texture).unwrap();
            std::fs::write(
                src.join("import-report.txt"),
                format!("{zip} -> mod.track (Track)"),
            )
            .unwrap();
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled("mod.track", true))
                .unwrap();
            let history = store.addon_data_dir("t.hook");
            std::fs::create_dir_all(&history).unwrap();
            std::fs::write(
                history.join("importer.json"),
                format!(r#"{{ "mods": {{ "{name}": {{ "made": {{ "id": "mod.{name}" }} }} }} }}"#),
            )
            .unwrap();
            let print = || {
                let p = store.profile("default").unwrap();
                fingerprint::fingerprint(&store, &p, &game.identity).unwrap()
            };
            let before = print();
            build(&store, "default", &game).unwrap();
            let after = print();
            std::fs::remove_dir_all(&root).ok();
            (before, after)
        };
        let (a, b) = (
            prints("a", "track.zip", "dds"),
            prints("b", "track(1).zip", "dds"),
        );
        assert_ne!(a.0.digest, b.0.digest);
        assert_eq!(a.1.digest, b.1.digest, "{:#?}\n{:#?}", a.1.addons, b.1.addons);
        let track = a.1.addons.iter().find(|x| x.id == "mod.track").unwrap();
        assert!(matches!(track.source, fingerprint::Source::Built(_)), "{track:?}");
        let detail = track.detail.as_ref().unwrap();
        assert!(
            detail.parts.contains_key("Assets") && detail.parts.contains_key("declared"),
            "{detail:?}"
        );
        assert_eq!(detail.paths, ["a.dds"]);
        let (_, other) = prints("c", "track.zip", "another texture");
        assert_ne!(other.digest, a.1.digest);
        let other_track = other.addons.iter().find(|x| x.id == "mod.track").unwrap();
        let other_detail = other_track.detail.as_ref().unwrap();
        assert_eq!(other_detail.paths, detail.paths);
        assert_ne!(other_detail.reads, detail.reads, "{other_detail:?}");
    }

    struct Rows(AddonManifest);

    impl Addon for Rows {
        fn manifest(&self) -> &AddonManifest {
            &self.0
        }

        fn build(&self, cx: &BuildCx) -> swofty_addon::Result<AssetBatch> {
            let reply: swofty_addon::helper::Built = serde_json::from_str(
                r#"{ "ops": [{ "op": "db.add-row", "table": "entrants.teams",
                               "from": "entrants.teams.team[_metadata_=mclaren_26]",
                               "set": { "_metadata_": "team12_26" } }],
                     "fields": { "entrants.teams.team": ["_metadata_", "stat_id"] } }"#,
            )
            .expect("valid reply");
            let origin = Origin {
                addon: cx.cx.addon.into(),
                item: None,
            };
            Ok(AssetBatch {
                ops: reply
                    .ops
                    .into_iter()
                    .map(|op| Op {
                        op,
                        origin: origin.clone(),
                    })
                    .collect(),
                fields: reply.fields,
                ..AssetBatch::default()
            })
        }
    }

    #[test]
    fn a_row_id_a_conflict_check_gave_is_kept_by_the_build_after_it() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-check-ids-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let mut records = vec![Record::new("entrants.teams._team_instancecount_", 0, "2")];
        for (i, label) in ["mclaren_26", "gm_cadillac_26"].iter().enumerate() {
            let row = format!("entrants.teams.team_{i}");
            records.push(Record::new(&format!("{row}._metadata_"), 4, *label));
            records.push(Record::new(&format!("{row}.stat_id"), 0, (100 + i).to_string()));
        }
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let manifest = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "t.rows", "name": "Rows", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" } }"#,
        )
        .unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![Builtin {
            addon: Arc::new(Rows(manifest)),
            locked: false,
            default_on: true,
        }]);
        let reused = |b: &Built| b.lines.iter().any(|l| l.starts_with("build: nothing changed"));
        let ids = crate::row_ids::path(&store.root);

        let checked = build_as(&store, "default", &game, true).unwrap();
        assert!(!ids.exists(), "a check keeps no ids");

        let built = build(&store, "default", &game).unwrap();
        assert_eq!(built.buildhash, checked.buildhash);
        assert!(!reused(&built), "{:?}", built.lines);
        let kept: serde_json::Value = serde_json::from_slice(&std::fs::read(&ids).unwrap()).unwrap();
        assert_eq!(kept["rows"]["t.rows/entrants.teams.team/team12_26"]["id"], 102);
        assert!(reused(&build(&store, "default", &game).unwrap()));

        // With every id already kept, a check's build is as good as any.
        std::fs::remove_file(built.overlay.join(SAVED_FILE)).unwrap();
        build_as(&store, "default", &game, true).unwrap();
        assert!(reused(&build(&store, "default", &game).unwrap()));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn build_reply_fields_name_the_columns_add_row_copies() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-fields-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let mut records = vec![Record::new("entrants.teams._team_instancecount_", 0, "2")];
        for (i, label) in ["mclaren_26", "gm_cadillac_26"].iter().enumerate() {
            let row = format!("entrants.teams.team_{i}");
            records.push(Record::new(&format!("{row}._metadata_"), 4, *label));
            records.push(Record::new(&format!("{row}.stat_id"), 0, (100 + i).to_string()));
        }
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let manifest = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "t.rows", "name": "Rows", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" } }"#,
        )
        .unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![Builtin {
            addon: Arc::new(Rows(manifest)),
            locked: false,
            default_on: true,
        }]);
        let built = build(&store, "default", &game).unwrap();
        let db = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(db.instance_count("entrants.teams", "team"), Some(3));
        assert_eq!(
            db.get_text("entrants.teams.team_2._metadata_").as_deref(),
            Some("team12_26")
        );
        let key = "t.rows/entrants.teams.team/team12_26";
        let next_id = 102;
        assert_eq!(
            db.get_text("entrants.teams.team_2.stat_id"),
            Some(next_id.to_string())
        );
        assert!(
            built
                .lines
                .iter()
                .any(|l| l.ends_with(&format!("stat_id {next_id} (the source's is 100)"))),
            "{:?}",
            built.lines
        );
        let outputs: serde_json::Value =
            serde_json::from_slice(&std::fs::read(store.root.join(fingerprint::OUTPUTS_FILE)).unwrap())
                .unwrap();
        assert_eq!(
            outputs["default"]["t.rows"]["row_ids"]["entrants.teams.team/team12_26"],
            next_id
        );
        let ids = crate::row_ids::path(&store.root);
        let kept: serde_json::Value = serde_json::from_slice(&std::fs::read(&ids).unwrap()).unwrap();
        assert_eq!(kept["rows"][key]["id"], next_id);
        std::fs::write(
            &ids,
            format!(r#"{{"version":1,"rows":{{"{key}":{{"id":140,"from":"next"}}}}}}"#),
        )
        .unwrap();
        std::fs::remove_file(built.overlay.join(SAVED_FILE)).unwrap();
        let again = build(&store, "default", &game).unwrap();
        let db = Gvdb::parse(&std::fs::read(&again.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(
            db.get_text("entrants.teams.team_2.stat_id").as_deref(),
            Some("140")
        );
        assert!(
            again
                .lines
                .iter()
                .any(|l| l.ends_with("stat_id 140 (the source's is 100; kept from row-ids.json)")),
            "{:?}",
            again.lines
        );
        std::fs::remove_file(&ids).unwrap();
        std::fs::rename(&again.overlay, store.overlays_dir("default").join("older")).unwrap();
        let seeded = build(&store, "default", &game).unwrap();
        assert!(
            seeded
                .lines
                .iter()
                .any(|l| l.ends_with("stat_id 140 (the source's is 100; as the last build gave it)")),
            "{:?}",
            seeded.lines
        );
        let kept: serde_json::Value = serde_json::from_slice(&std::fs::read(&ids).unwrap()).unwrap();
        assert_eq!(
            kept["rows"][key],
            serde_json::json!({ "id": 140, "from": "seeded" })
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[cfg(unix)]
    #[test]
    fn rebuilding_a_profile_never_rewrites_a_file_a_session_has_linked() {
        use std::os::unix::fs::MetadataExt;
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-linked-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let mut records = vec![Record::new("entrants.teams._team_instancecount_", 0, "2")];
        for (i, label) in ["mclaren_26", "gm_cadillac_26"].iter().enumerate() {
            let row = format!("entrants.teams.team_{i}");
            records.push(Record::new(&format!("{row}._metadata_"), 4, *label));
            records.push(Record::new(&format!("{row}.stat_id"), 0, (100 + i).to_string()));
        }
        let stock = Gvdb { records }.to_bytes();
        std::fs::write(dir.join(GVDB), &stock).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let manifest = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "t.rows", "name": "Rows", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" } }"#,
        )
        .unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![Builtin {
            addon: Arc::new(Rows(manifest)),
            locked: false,
            default_on: true,
        }]);
        let built = build(&store, "default", &game).unwrap();
        let out = built.plan.data[GVDB].clone();
        let made = std::fs::read(&out).unwrap();
        let lines = std::cell::RefCell::new(Vec::new());
        let say = |l: &str| lines.borrow_mut().push(l.to_owned());
        let inst = crate::install::Installer {
            out: &say,
            ..crate::install::Installer::new(&store.root, &game)
        };
        inst.apply(&built.plan).unwrap();
        let installed = dir.join(GVDB);
        assert_eq!(
            std::fs::metadata(&installed).unwrap().ino(),
            std::fs::metadata(&out).unwrap().ino()
        );
        assert_eq!(std::fs::read(&installed).unwrap(), made);

        std::fs::remove_file(built.overlay.join(SAVED_FILE)).unwrap();
        let again = build(&store, "default", &game).unwrap();
        assert_eq!(again.plan.data[GVDB], out);
        assert_eq!(std::fs::read(&installed).unwrap(), made);
        assert_ne!(
            std::fs::metadata(&installed).unwrap().ino(),
            std::fs::metadata(&out).unwrap().ino(),
            "the rebuild wrote a new file"
        );
        assert_eq!(std::fs::metadata(&installed).unwrap().nlink(), 1);

        inst.apply(&crate::install::Plan::vanilla()).unwrap();
        assert_eq!(std::fs::read(&installed).unwrap(), stock);
        assert_eq!(std::fs::read(&out).unwrap(), made);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_cached_output_changed_in_place_to_the_same_size_is_rebuilt() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-touched-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let mut records = vec![Record::new("entrants.teams._team_instancecount_", 0, "2")];
        for (i, label) in ["mclaren_26", "gm_cadillac_26"].iter().enumerate() {
            let row = format!("entrants.teams.team_{i}");
            records.push(Record::new(&format!("{row}._metadata_"), 4, *label));
            records.push(Record::new(&format!("{row}.stat_id"), 0, (100 + i).to_string()));
        }
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let manifest = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "t.rows", "name": "Rows", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" } }"#,
        )
        .unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![Builtin {
            addon: Arc::new(Rows(manifest)),
            locked: false,
            default_on: true,
        }]);
        let built = build(&store, "default", &game).unwrap();
        let out = built.plan.data[GVDB].clone();
        let made = std::fs::read(&out).unwrap();
        let again = build(&store, "default", &game).unwrap();
        assert!(again.lines.iter().any(|l| l.contains("reusing it")));

        let mut bad = made.clone();
        bad[0] ^= 0xff;
        let file = std::fs::OpenOptions::new().write(true).open(&out).unwrap();
        std::io::Write::write_all(&mut &file, &bad).unwrap();
        file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
            .unwrap();
        drop(file);
        assert_eq!(std::fs::read(&out).unwrap().len(), made.len());

        let rebuilt = build(&store, "default", &game).unwrap();
        assert!(!rebuilt.lines.iter().any(|l| l.contains("reusing it")));
        assert_eq!(std::fs::read(&rebuilt.plan.data[GVDB]).unwrap(), made);
        std::fs::remove_dir_all(&root).ok();
    }

    static LENDING: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn a_room_launch_build_uses_the_hosts_ids_under_its_own_cache_key_and_keeps_row_ids_json() {
        let _lending = LENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-lent-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let mut records = vec![Record::new("entrants.teams._team_instancecount_", 0, "2")];
        for (i, label) in ["mclaren_26", "gm_cadillac_26"].iter().enumerate() {
            let row = format!("entrants.teams.team_{i}");
            records.push(Record::new(&format!("{row}._metadata_"), 4, *label));
            records.push(Record::new(&format!("{row}.stat_id"), 0, (100 + i).to_string()));
        }
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let manifest = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "t.lent", "name": "Lent", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" } }"#,
        )
        .unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![Builtin {
            addon: Arc::new(Rows(manifest)),
            locked: false,
            default_on: true,
        }]);
        let own = build(&store, "default", &game).unwrap();
        let ids = crate::row_ids::path(&store.root);
        let kept = std::fs::read(&ids).unwrap();
        let stat_id = |b: &Built| {
            let db = Gvdb::parse(&std::fs::read(&b.plan.data[GVDB]).unwrap()).unwrap();
            db.get_text("entrants.teams.team_2.stat_id").unwrap()
        };
        assert_eq!(stat_id(&own), "102");
        let host = crate::row_ids::Overrides {
            rows: BTreeMap::from([(
                "t.lent".to_owned(),
                BTreeMap::from([("entrants.teams.team/team12_26".to_owned(), 140)]),
            )]),
            ..Default::default()
        };
        crate::row_ids::lend(Some(host));
        let lent = build(&store, "default", &game);
        crate::row_ids::lend(None);
        let lent = lent.unwrap();
        assert_ne!(
            lent.buildhash, own.buildhash,
            "the host's ids are part of the cache key"
        );
        assert_eq!(stat_id(&lent), "140");
        assert!(
            lent.lines
                .iter()
                .any(|l| l.contains("the room host's, for this session only")),
            "{:?}",
            lent.lines
        );
        assert_eq!(lent.row_ids["t.lent"]["entrants.teams.team/team12_26"], 140);
        assert_eq!(std::fs::read(&ids).unwrap(), kept, "row-ids.json is as it was");
        let back = build(&store, "default", &game).unwrap();
        assert_eq!(
            back.buildhash, own.buildhash,
            "the next normal build is the player's own"
        );
        assert_eq!(stat_id(&back), "102");
        let settings = crate::row_ids::Overrides {
            order: vec!["t.lent".to_owned()],
            options: BTreeMap::from([(
                "t.lent".to_owned(),
                BTreeMap::from([("laps".to_owned(), serde_json::json!(7))]),
            )]),
            ..Default::default()
        };
        crate::row_ids::lend(Some(settings));
        let ordered = build(&store, "default", &game);
        crate::row_ids::lend(None);
        let ordered = ordered.unwrap();
        assert!(
            ordered
                .lines
                .iter()
                .any(|l| l
                    .contains("launch: using the host's load order and 1 option value(s) for 1 addon(s)")),
            "{:?}",
            ordered.lines
        );
        assert_ne!(
            ordered.buildhash, own.buildhash,
            "the host's settings are part of the cache key"
        );
        assert_eq!(
            store.profile("default").unwrap().options.get("t.lent"),
            None,
            "the profile keeps its own"
        );
        let registry = crate::row_ids::Overrides {
            registry: BTreeMap::from([(
                "t.lent".to_owned(),
                BTreeMap::from([("content.package:t.lent/season".to_owned(), 116)]),
            )]),
            ..Default::default()
        };
        crate::row_ids::lend(Some(registry));
        let registered = build(&store, "default", &game);
        crate::row_ids::lend(None);
        assert_ne!(
            registered.unwrap().buildhash,
            own.buildhash,
            "the host's registry ids are part of the cache key"
        );
        let clash = crate::row_ids::Overrides {
            rows: BTreeMap::from([(
                "t.lent".to_owned(),
                BTreeMap::from([("entrants.teams.team/team12_26".to_owned(), 101)]),
            )]),
            ..Default::default()
        };
        crate::row_ids::lend(Some(clash));
        let refused = build(&store, "default", &game);
        crate::row_ids::lend(None);
        let err = refused
            .map(|_| ())
            .expect_err("a stock id is refused")
            .to_string();
        assert!(err.contains("stat id 101"), "{err}");
        assert_eq!(std::fs::read(&ids).unwrap(), kept);
        std::fs::remove_dir_all(&root).ok();
    }

    struct Drivers(AddonManifest);

    impl Addon for Drivers {
        fn manifest(&self) -> &AddonManifest {
            &self.0
        }

        fn build(&self, cx: &BuildCx) -> swofty_addon::Result<AssetBatch> {
            let reply: swofty_addon::helper::Built = serde_json::from_str(
                r#"{ "ops": [
                       { "op": "db.add-row", "table": "entrants.drivers",
                         "from": "entrants.drivers.driver[_metadata_=max_verstappen]",
                         "set": { "_metadata_": "dm_208", "stat_id": "208" } },
                       { "op": "db.add-row", "table": "entrants.entrant_lists.entrants",
                         "from": "entrants.entrant_lists.entrants.entrant[_metadata_=max_verstappen]",
                         "set": { "_metadata_": "dm_208", "driver": "dm_208" } }],
                     "fields": { "entrants.drivers.driver": ["_metadata_", "stat_id"],
                                 "entrants.entrant_lists.entrants.entrant": ["_metadata_", "driver"] } }"#,
            )
            .expect("valid reply");
            let origin = Origin {
                addon: cx.cx.addon.into(),
                item: None,
            };
            Ok(AssetBatch {
                ops: reply
                    .ops
                    .into_iter()
                    .map(|op| Op {
                        op,
                        origin: origin.clone(),
                    })
                    .collect(),
                fields: reply.fields,
                ..AssetBatch::default()
            })
        }
    }

    #[test]
    fn warns_when_a_db_editor_addon_picks_a_driver_driver_manager_does_not_add() {
        use crate::dbedit::driver_manager::tests::{database, fixture};
        use crate::dbedit::package::Package;
        use crate::dbedit::{Catalog, module};
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-dm-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let cat = Catalog::from_gvdb(database(), String::new());
        std::fs::write(dir.join(GVDB), database().to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let manifest = AddonManifest::parse(&format!(
            r#"{{ "schema_version": 1, "id": "{}", "name": "Driver Manager", "version": "1.0.0",
                 "author": "t", "compat": {{ "game": "f1_25" }} }}"#,
            driver_manager::ID
        ))
        .unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![Builtin {
            addon: Arc::new(Drivers(manifest)),
            locked: false,
            default_on: true,
        }]);
        fixture(&store.addon_data_dir(driver_manager::ID));
        let lineups = module("lineups").unwrap();
        let seat = lineups
            .fields
            .iter()
            .position(|f| f.id == "entrant_reference")
            .unwrap();
        let view = lineups.view(&cat, &Default::default(), None).unwrap();
        let mut seats = Package::create(&store, "Seats").unwrap();
        seats
            .edits
            .set(&cat, view.rows[0].cells[seat].key.unwrap(), "dm_208")
            .unwrap();
        let driver = swofty_formats::hash::fnv1a64("entrants.entrant_lists.entrants.entrant_0.driver");
        seats.edits.set(&cat, driver, "dm_209").unwrap();
        seats.save(&store, &cat).unwrap();
        store
            .update_profile("default", |p| p.set_enabled(&seats.id, true))
            .unwrap();

        let tail = "the game would read a driver and entrant its database doesn't have";
        let picks = |built: &Built| -> Vec<String> {
            built
                .warnings
                .iter()
                .filter(|w| w.contains(" picks "))
                .cloned()
                .collect()
        };
        let built = build(&store, "default", &game).unwrap();
        let db = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(
            db.get_text("entrants.entrant_lists.entrant_list_0.entrant_0.entrant_reference")
                .as_deref(),
            Some("dm_208")
        );
        assert_eq!(
            picks(&built),
            [format!(
                "Seats: picks Ret Ired (dm_209), which Driver Manager doesn't add (its page marks the drivers it leaves out); {tail}"
            )]
        );

        store
            .update_profile("default", |p| p.set_enabled(driver_manager::ID, false))
            .unwrap();
        assert_eq!(
            picks(&build(&store, "default", &game).unwrap()),
            [format!(
                "Seats: picks Jacob Swofty (dm_208), Ret Ired (dm_209), Driver Manager drivers, and Driver Manager is off in this profile; {tail}"
            )]
        );
        std::fs::remove_dir_all(&root).ok();
    }

    const PUT: &str = "environment_package/tracks/t/mipmaps/a.mipmaps";

    struct Putter(AddonManifest);

    impl Addon for Putter {
        fn manifest(&self) -> &AddonManifest {
            &self.0
        }

        fn build(&self, cx: &BuildCx) -> swofty_addon::Result<AssetBatch> {
            let mut batch = Rows(self.0.clone()).build(cx)?;
            batch.ops.push(Op {
                op: AssetOp::File(FileOp::Put {
                    source: Source {
                        root: Root::Data,
                        path: "mip".into(),
                    },
                    target: PUT.into(),
                }),
                origin: Origin {
                    addon: cx.cx.addon.into(),
                    item: None,
                },
            });
            Ok(batch)
        }
    }

    #[test]
    fn reuses_an_unchanged_build_until_an_input_or_the_game_folder_changes() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-reuse-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let db = |label: &str| {
            let mut records = vec![Record::new("entrants.teams._team_instancecount_", 0, "1")];
            records.push(Record::new("entrants.teams.team_0._metadata_", 4, "mclaren_26"));
            records.push(Record::new("entrants.teams.team_0.stat_id", 0, label));
            Gvdb { records }.to_bytes()
        };
        std::fs::write(dir.join(GVDB), db("100")).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let manifest = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "t.put", "name": "Put", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" } }"#,
        )
        .unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![Builtin {
            addon: Arc::new(Putter(manifest)),
            locked: false,
            default_on: true,
        }]);
        std::fs::create_dir_all(store.addon_data_dir("t.put")).unwrap();
        std::fs::write(store.addon_data_dir("t.put").join("mip"), "mip").unwrap();
        let reused = |b: &Built| b.lines.iter().any(|l| l.starts_with("build: nothing changed"));

        let first = build(&store, "default", &game).unwrap();
        assert!(!reused(&first));
        assert_eq!(first.plan.added[PUT], store.addon_data_dir("t.put").join("mip"));
        let again = build(&store, "default", &game).unwrap();
        assert!(reused(&again), "{:?}", again.lines);
        assert_eq!(again.buildhash, first.buildhash);
        assert_eq!(
            (&again.plan.data, &again.plan.added, &again.plan.loader),
            (&first.plan.data, &first.plan.added, &first.plan.loader)
        );
        assert_eq!(again.settings, first.settings);
        assert_eq!(again.warnings, first.warnings);

        // A file copied in by hand where the plan adds one makes the build
        // decide again, and it is now a game file to replace.
        std::fs::create_dir_all(dir.join(PUT).parent().unwrap()).unwrap();
        std::fs::write(dir.join(PUT), "by hand").unwrap();
        let by_hand = build(&store, "default", &game).unwrap();
        assert!(!reused(&by_hand));
        assert!(by_hand.plan.data.contains_key(PUT) && !by_hand.plan.added.contains_key(PUT));
        std::fs::remove_dir_all(dir.join("environment_package")).unwrap();

        let built = build(&store, "default", &game).unwrap();
        std::fs::write(&built.plan.data[GVDB], b"torn").unwrap();
        assert!(!reused(&build(&store, "default", &game).unwrap()));

        std::fs::write(dir.join(GVDB), db("101")).unwrap();
        let changed = build(&store, "default", &game).unwrap();
        assert!(!reused(&changed));
        assert_ne!(changed.buildhash, first.buildhash);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_cancelled_build_stops_and_the_next_one_starts_fresh() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-cancel-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let records = vec![
            Record::new("entrants.teams._team_instancecount_", 0, "1"),
            Record::new("entrants.teams.team_0._metadata_", 4, "mclaren_26"),
            Record::new("entrants.teams.team_0.stat_id", 0, "100"),
        ];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let manifest = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "t.put", "name": "Put", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" } }"#,
        )
        .unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![Builtin {
            addon: Arc::new(Putter(manifest)),
            locked: false,
            default_on: true,
        }]);
        std::fs::create_dir_all(store.addon_data_dir("t.put")).unwrap();
        std::fs::write(store.addon_data_dir("t.put").join("mip"), "mip").unwrap();
        let reused = |b: &Built| b.lines.iter().any(|l| l.starts_with("build: nothing changed"));

        crate::busy::request_cancel(&store.root).unwrap();
        assert!(matches!(build(&store, "default", &game), Err(Error::Cancelled)));
        crate::busy::clear_cancel(&store.root);
        let fresh = build(&store, "default", &game).unwrap();
        assert!(!reused(&fresh), "{:?}", fresh.lines);
        assert!(reused(&build(&store, "default", &game).unwrap()));
        crate::busy::request_cancel(&store.root).unwrap();
        build_as(&store, "default", &game, true).unwrap();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_reused_build_keeps_its_screens_assets_and_plugins() {
        let root = std::env::temp_dir().join(format!("swofty-build-saved-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let loader = root.join("loader");
        let placed: BTreeMap<String, PathBuf> = [
            (
                "swofty/ui/swofty_settings.ui",
                root.join("overlay/swofty/ui/swofty_settings.ui"),
            ),
            ("swofty/assets.json", root.join("overlay/swofty/assets.json")),
            (
                "swofty/addons/t.x/native/x.dll",
                root.join("addons/t.x/native/x.dll"),
            ),
            (
                "swofty/addons/core/swofty_core.dll",
                loader.join("swofty/addons/core/swofty_core.dll"),
            ),
            ("version.dll", loader.join("version.dll")),
        ]
        .into_iter()
        .map(|(rel, src)| (rel.to_owned(), src))
        .collect();
        for src in placed.values() {
            std::fs::create_dir_all(src.parent().unwrap()).unwrap();
            std::fs::write(src, "x").unwrap();
        }

        let kept = built_files(&placed, Some(&loader));
        assert_eq!(
            kept.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "swofty/addons/t.x/native/x.dll",
                "swofty/assets.json",
                "swofty/ui/swofty_settings.ui"
            ]
        );
        assert_eq!(built_files(&placed, None), placed);

        let mut built = Built::default();
        built.plan.loader = kept.clone();
        let saved = Saved::of(&built, 0, 0).unwrap();
        let store = Store::new(root.join("data"));
        let game = Game {
            identity: Identity {
                build_id: "1".into(),
                exe_sha256: "a".repeat(64),
            },
            dir: root.join("game"),
            user_reg: None,
        };
        assert!(saved.usable(&store, &game));
        let mut reused = Built::default();
        Saved::of(&built, 0, 0).unwrap().restore(&mut reused).unwrap();
        for rel in kept.keys() {
            assert_eq!(reused.plan.loader.get(rel), kept.get(rel), "{rel}");
        }

        std::fs::remove_file(&kept["swofty/ui/swofty_settings.ui"]).unwrap();
        assert!(!saved.usable(&store, &game));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn applies_db_editor_packages_leniently_in_load_order() {
        use crate::dbedit::Catalog;
        use crate::dbedit::package::Package;
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-dbedit-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let records = vec![
            Record::new("entrants.teams._team_instancecount_", 0, "1"),
            Record::new("entrants.teams.team_0._metadata_", 4, "mclaren_26"),
            Record::new("entrants.teams.team_0.stat_id", 0, "100"),
            Record::new("entrants.teams.team_0.prestige", 0, "5"),
        ];
        let stock = Gvdb { records }.to_bytes();
        std::fs::write(dir.join(GVDB), &stock).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let cat = Catalog::load(&stock).unwrap();
        let key = |p: &str| swofty_formats::hash::fnv1a64(p);
        let mut first = Package::create(&store, "First").unwrap();
        first
            .edits
            .set(&cat, key("entrants.teams.team_0.stat_id"), "7")
            .unwrap();
        first
            .edits
            .set(&cat, key("entrants.teams.team_0.prestige"), "6")
            .unwrap();
        first.save(&store, &cat).unwrap();
        let mut second = Package::create(&store, "Second").unwrap();
        second
            .edits
            .set(&cat, key("entrants.teams.team_0.prestige"), "9")
            .unwrap();
        second.save(&store, &cat).unwrap();
        store
            .update_profile("default", |p| {
                p.set_enabled(&first.id, true)?;
                p.set_enabled(&second.id, true)
            })
            .unwrap();

        let built = build(&store, "default", &game).unwrap();
        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(
            out.get_text("entrants.teams.team_0.stat_id").as_deref(),
            Some("7")
        );
        assert_eq!(
            out.get_text("entrants.teams.team_0.prestige").as_deref(),
            Some("9")
        );
        assert!(build(&store, "vanilla", &game).unwrap().lines.is_empty());

        first.edits.edits[0].select = Some("entrants.teams.team[_metadata_=gone_26]".into());
        first
            .edits
            .set(&cat, key("entrants.teams.team_0._metadata_"), "mclaren_27")
            .unwrap();
        first.save(&store, &cat).unwrap();
        let built = build(&store, "default", &game).unwrap();
        assert!(
            built
                .warnings
                .iter()
                .any(|w| w.starts_with("First: change skipped: ") && w.contains("gone_26")),
            "{:?}",
            built.warnings
        );
        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(
            out.get_text("entrants.teams.team_0.stat_id").as_deref(),
            Some("100")
        );
        assert_eq!(
            out.get_text("entrants.teams.team_0._metadata_").as_deref(),
            Some("mclaren_27")
        );

        let put = |path: &str, value: &str| serde_json::json!({"op": "put-record", "key": format!("0x{:016x}", key(path)), "tag": 0, "value": value});
        let hidden = serde_json::json!({"schema_version": 1, "ops": [
            put("entrants.teams.team_0.prestige", "77"), put("entrants.teams.team_0.stat_id", "55")]});
        let addon = store.addons_dir().join(&second.id);
        std::fs::write(addon.join("database.json"), hidden.to_string()).unwrap();
        std::fs::write(
            store.addons_dir().join(&first.id).join("dbedit.json"),
            "{\"format\": \"swofty-db-pa",
        )
        .unwrap();
        let built = build(&store, "default", &game).unwrap();
        assert!(
            built
                .warnings
                .iter()
                .any(|w| w.starts_with("First: package skipped: ")),
            "{:?}",
            built.warnings
        );
        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        for (path, want) in [
            ("entrants.teams.team_0.prestige", "9"),
            ("entrants.teams.team_0.stat_id", "100"),
            ("entrants.teams.team_0._metadata_", "mclaren_26"),
        ] {
            assert_eq!(out.get_text(path).as_deref(), Some(want), "{path}");
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_row_count_past_the_rows_there_stays_at_the_rows() {
        use crate::dbedit::Catalog;
        use crate::dbedit::package::Package;
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-short-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let list = "schedule.track_lists.track_event_list_0";
        let count = format!("{list}._fixed_event_reference_instancecount_");
        let mut records = vec![
            Record::new("schedule.track_lists._track_event_list_instancecount_", 0, "1"),
            Record::new(&format!("{list}._metadata_"), 2, "F1_2025_16"),
            Record::new(&count, 0, "2"),
        ];
        for (i, e) in ["rw_bahrain", "rw_suzuka"].iter().enumerate() {
            let row = format!("{list}.fixed_event_reference_{i}");
            records.push(Record::new(&format!("{row}._metadata_"), 2, format!("ref_{e}")));
            records.push(Record::new(&format!("{row}.event_name"), 4, *e));
        }
        let stock = Gvdb { records }.to_bytes();
        std::fs::write(dir.join(GVDB), &stock).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let cat = Catalog::load(&stock).unwrap();
        let mut p = Package::create(&store, "2017 v2").unwrap();
        let key = |p: &str| swofty_formats::hash::fnv1a64(p);
        let second = format!("{list}.fixed_event_reference_1.event_name");
        p.edits.set(&cat, key(&count), "4").unwrap();
        p.edits.set(&cat, key(&second), "rw_bahrain").unwrap();
        p.save(&store, &cat).unwrap();
        store
            .update_profile("default", |pr| pr.set_enabled(&p.id, true))
            .unwrap();

        let built = build(&store, "default", &game).unwrap();
        assert!(
            built.warnings.contains(&format!(
                "2017 v2: {list}.fixed_event_reference has 2 rows, not 4; the count stays at 2 (the game crashes on rows that don't exist)"
            )),
            "{:?}",
            built.warnings
        );
        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(out.get_text(&count).as_deref(), Some("2"));
        assert_eq!(out.get_text(&second).as_deref(), Some("rw_bahrain"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_special_livery_with_an_unknown_team_kit_builds_with_an_empty_one() {
        use crate::dbedit::Catalog;
        use crate::dbedit::package::Package;
        use crate::livery_kits::{KIT_YEARS, TABLE};
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-kits-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let year = format!("{KIT_YEARS}.team_customisation_for_year_0");
        let mut records = vec![
            Record::new(
                &format!("{KIT_YEARS}._team_customisation_for_year_instancecount_"),
                0,
                "1",
            ),
            Record::new(&format!("{year}._metadata_"), 2, "2026"),
            Record::new(&format!("{year}._team_customisation_instancecount_"), 0, "2"),
            Record::new(
                &format!("{year}.team_customisation_0._metadata_"),
                2,
                "2026-mclaren",
            ),
            Record::new(
                &format!("{year}.team_customisation_1._metadata_"),
                2,
                "2026-renault",
            ),
            Record::new(&format!("{TABLE}._se_livery_instancecount_"), 0, "2"),
        ];
        for (i, label) in ["mclaren_monaco_2026", "lotus_lx02"].iter().enumerate() {
            let row = format!("{TABLE}.se_livery_{i}");
            records.push(Record::new(&format!("{row}._metadata_"), 2, *label));
            records.push(Record::new(
                &format!("{row}.team_customisation"),
                4,
                "2026-mclaren",
            ));
        }
        let stock = Gvdb { records }.to_bytes();
        std::fs::write(dir.join(GVDB), &stock).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));

        let built = build(&store, "default", &game).unwrap();
        assert!(
            !built.warnings.iter().any(|w| w.contains("team_customisation")),
            "{:?}",
            built.warnings
        );

        let cat = Catalog::load(&stock).unwrap();
        let mut p = Package::create(&store, "More Liveries").unwrap();
        let kit = format!("{TABLE}.se_livery_1.team_customisation");
        p.edits
            .set(&cat, swofty_formats::hash::fnv1a64(&kit), "2026-lotus")
            .unwrap();
        p.save(&store, &cat).unwrap();
        store
            .update_profile("default", |pr| pr.set_enabled(&p.id, true))
            .unwrap();

        let built = build(&store, "default", &game).unwrap();
        assert!(
            built.warnings.contains(
                &"More Liveries: special livery lotus_lx02 has team_customisation 2026-lotus, and \
                  entrants.team_customisation has no row with that label; the value is left empty (the game \
                  crashes at start-up on a team kit it cannot find)"
                    .to_owned()
            ),
            "{:?}",
            built.warnings
        );
        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(out.get_text(&kit).as_deref(), Some(""));
        assert_eq!(
            out.get_text(&format!("{TABLE}.se_livery_0.team_customisation"))
                .as_deref(),
            Some("2026-mclaren")
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn adds_a_driver_head_row_and_its_groups_for_a_head_archive_no_row_names() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-heads-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let rows = "entrants.animation.driver_heads.driver_head";
        let stock_heads = [
            ("lando_norris", "79", "male_driver", "False"),
            ("kimi_raikkonen", "45", "male_driver", "False"),
            ("male_ls222", "198", "male_ea", "True"),
            ("female_ls112", "214", "female_ea", "True"),
        ];
        let mut records = vec![Record::new(
            "entrants.animation.driver_heads._driver_head_instancecount_",
            0,
            stock_heads.len().to_string(),
        )];
        for (i, (label, id, folder, custom)) in stock_heads.iter().enumerate() {
            let female = folder.starts_with("female");
            let model =
                format!("eaid://[CHARACTER_PACKAGE]/heads/{folder}/{label}/idf/{label}_rig_head_morph_v1");
            let (model, female_rig) = match female {
                true => (String::new(), model),
                false => (model, String::new()),
            };
            let body = if female { "generic_female" } else { "generic_male" };
            for (field, tag, value) in [
                ("_metadata_", 2, *label),
                ("asset_group_path_format", 4, "driver_head_format"),
                (
                    "asset_group_path_format_female_on_male_rig",
                    4,
                    "driver_head_format_female_male_rig",
                ),
                ("available_for_customisation", 5, *custom),
                ("body_type", 4, body),
                ("esports_name", 2, ""),
                ("face_morph_asset_group_path_format", 4, "face_morph_format"),
                (
                    "face_morph_asset_group_path_format_female_on_male_rig",
                    4,
                    "face_morph_format_female_male_rig",
                ),
                ("has_morph_data", 5, "True"),
                ("model_path", 2, model.as_str()),
                ("model_path_female_rig", 2, female_rig.as_str()),
                ("stat_id", 0, *id),
            ] {
                records.push(Record::new(&format!("{rows}_{i}.{field}"), tag, value));
            }
        }
        records.extend([
            Record::new("entrants.drivers._driver_instancecount_", 0, "1"),
            Record::new("entrants.drivers.driver_0._metadata_", 2, "lando_norris"),
            Record::new("entrants.drivers.driver_0.head", 4, "lando_norris"),
        ]);
        let stock = Gvdb { records };
        std::fs::write(dir.join(GVDB), stock.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let heads = "2025_asset_groups/animation_package/scenes/cinematics/drivers/heads";
        let morphs = "2025_asset_groups/animation_package/scenes/cinematics/drivers/morph_poses";
        let model_in = |folder: &str, label: &str, rig: &str| {
            archive_of(
                "GfxModelRes",
                &[&format!(
                    "eaid://character_package/heads/{folder}/{label}/idf/{label}_rig_head_morph_v1{rig}.idf?model"
                )],
                1,
            )
        };
        let model = |label: &str| model_in("male_driver", label, "");
        let install = |id: &str, name: &str, files: &[(&str, Vec<u8>)], extra: &str| {
            let src = root.join("src").join(id);
            std::fs::create_dir_all(src.join("files")).unwrap();
            let mut ops: Vec<String> = Vec::new();
            for (i, (target, bytes)) in files.iter().enumerate() {
                std::fs::write(src.join(format!("files/{i}.erp")), bytes).unwrap();
                ops.push(format!(
                    r#"{{"op":"file.put","source":"files/{i}.erp","target":"{target}"}}"#
                ));
            }
            if !extra.is_empty() {
                ops.push(extra.to_owned());
            }
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{name}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}, "operations": {{ "assets": ["ops.json"] }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(
                src.join("ops.json"),
                format!(r#"{{ "schema_version": 1, "ops": [{}] }}"#, ops.join(",")),
            )
            .unwrap();
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled(id, true))
                .unwrap();
        };
        install(
            "mod.rio",
            "Rio facemod",
            &[
                (&format!("{heads}/rio_haryanto.erp"), model("rio_haryanto")),
                (
                    &format!("{morphs}/rio_haryanto.erp"),
                    archive_of("AnimSet", &["x"], 2),
                ),
                (&format!("{heads}/nomodel.erp"), archive(&["x"], 3)),
                (&format!("{heads}/lando_norris.erp"), model("lando_norris")),
            ],
            "",
        );
        install(
            "mod.amy",
            "Amy facemod",
            &[
                (&format!("{heads}/amy.erp"), model_in("female_driver", "amy", "")),
                (
                    &format!("{heads}/amy_malerig.erp"),
                    model_in("female_driver", "amy", "_malerig"),
                ),
                (&format!("{morphs}/amy.erp"), archive_of("AnimSet", &["x"], 4)),
                (
                    &format!("{morphs}/amy_malerig.erp"),
                    archive_of("AnimSet", &["x"], 5),
                ),
            ],
            "",
        );
        install(
            "t.own",
            "Own rows",
            &[(&format!("{heads}/own_head.erp"), model("own_head"))],
            &format!(
                r#"{{"op":"db.add-row","table":"entrants.animation.driver_heads","from":"{rows}[_metadata_=kimi_raikkonen]","set":{{"_metadata_":"own_head"}}}}"#
            ),
        );
        install(
            "t.pick",
            "Pick",
            &[],
            r#"{"op":"db.set","select":"entrants.drivers.driver[_metadata_=lando_norris]","field":"head","value":"rio_haryanto"}"#,
        );

        let built = build(&store, "default", &game).unwrap();
        let archive_path = format!("{heads}/rio_haryanto.erp");
        for line in [
            format!(
                "Rio facemod: adds driver head rio_haryanto (male, copied from male_ls222) for {archive_path} and registers 2 asset group(s)"
            ),
            format!(
                "Amy facemod: adds driver head amy (female, copied from female_ls112) for {heads}/amy.erp and registers 4 asset group(s)"
            ),
        ] {
            assert!(built.lines.contains(&line), "{line}: {:?}", built.lines);
        }
        assert!(
            built.warnings.contains(&format!(
                "Rio facemod: {heads}/nomodel.erp gets no driver head row: it holds no head model"
            )),
            "{:?}",
            built.warnings
        );
        assert!(
            !built.warnings.iter().any(|w| w.contains("side-data")),
            "{:?}",
            built.warnings
        );
        assert_eq!(
            built
                .lines
                .iter()
                .filter(|l| l.contains("adds driver head"))
                .count(),
            2
        );

        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        let n: usize = out
            .get_text("entrants.animation.driver_heads._driver_head_instancecount_")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(n, 7);
        let field_of = |label: &str, f: &str| {
            let row = (0..n)
                .find(|i| out.get_text(&format!("{rows}_{i}._metadata_")).as_deref() == Some(label))
                .unwrap_or_else(|| panic!("a {label} row"));
            out.get_text(&format!("{rows}_{row}.{f}")).unwrap_or_default()
        };
        let field = |f: &str| field_of("rio_haryanto", f);
        assert_eq!(
            field("model_path"),
            "eaid://[CHARACTER_PACKAGE]/heads/male_driver/rio_haryanto/idf/rio_haryanto_rig_head_morph_v1"
        );
        assert_eq!(field("model_path_female_rig"), "");
        assert_eq!(field("has_morph_data"), "True");
        assert_eq!(field("body_type"), "generic_male");
        assert_eq!(field("available_for_customisation"), "False");
        assert_eq!(
            field_of("amy", "model_path"),
            "eaid://[CHARACTER_PACKAGE]/heads/female_driver/amy/idf/amy_rig_head_morph_v1_malerig"
        );
        assert_eq!(
            field_of("amy", "model_path_female_rig"),
            "eaid://[CHARACTER_PACKAGE]/heads/female_driver/amy/idf/amy_rig_head_morph_v1"
        );
        assert_eq!(field_of("amy", "body_type"), "generic_female");
        let ids: BTreeSet<String> = (0..n)
            .filter_map(|i| out.get_text(&format!("{rows}_{i}.stat_id")))
            .collect();
        assert_eq!(ids.len(), n, "every head keeps its own id: {ids:?}");

        let groups = |addon: &str| -> Vec<(String, Option<String>, Option<String>)> {
            built
                .waiting
                .iter()
                .filter_map(|op| match &op.op {
                    AssetOp::Group(GroupOp::Register {
                        group, erp, template, ..
                    }) if op.origin.addon == addon => Some((group.clone(), erp.clone(), template.clone())),
                    _ => None,
                })
                .collect()
        };
        let g = "asset_groups/animation_package/scenes/cinematics/drivers";
        assert_eq!(
            groups("mod.rio"),
            [
                (
                    format!("{g}/heads/rio_haryanto.assetgroup"),
                    Some(archive_path.clone()),
                    Some(format!("{g}/heads/lando_norris.assetgroup"))
                ),
                (
                    format!("{g}/morph_poses/rio_haryanto.assetgroup"),
                    Some(format!("{morphs}/rio_haryanto.erp")),
                    Some(format!("{g}/morph_poses/lando_norris.assetgroup"))
                ),
            ]
        );
        let amy: Vec<String> = groups("mod.amy")
            .into_iter()
            .map(|(group, _, template)| format!("{group} < {}", template.unwrap_or_default()))
            .collect();
        assert_eq!(
            amy,
            [
                format!("{g}/heads/amy.assetgroup < {g}/heads/female_ls493.assetgroup"),
                format!("{g}/heads/amy_malerig.assetgroup < {g}/heads/female_ls493_malerig.assetgroup"),
                format!("{g}/morph_poses/amy.assetgroup < {g}/morph_poses/female_ls493.assetgroup"),
                format!(
                    "{g}/morph_poses/amy_malerig.assetgroup < {g}/morph_poses/female_ls493_malerig.assetgroup"
                ),
            ]
        );

        let listed = crate::heads::for_profile(&store, &game, "default").unwrap().heads;
        let labels: Vec<&str> = listed.iter().map(|h| h.label.as_str()).collect();
        assert_eq!(labels, ["amy", "lando_norris", "own_head", "rio_haryanto"]);
        let rio = listed.iter().find(|h| h.label == "rio_haryanto").unwrap();
        assert_eq!(rio.addon.as_deref(), Some("mod.rio"));
        assert_eq!(rio.addon_name.as_deref(), Some("Rio facemod"));
        assert!(!rio.female);
        assert!(listed.iter().find(|h| h.label == "amy").unwrap().female);
        assert_eq!(crate::heads::installed(&store).len(), 4);
        store
            .update_profile("default", |p| p.set_enabled("mod.rio", false))
            .unwrap();
        let listed = crate::heads::for_profile(&store, &game, "default").unwrap().heads;
        assert_eq!(
            listed.iter().map(|h| h.label.as_str()).collect::<Vec<_>>(),
            ["amy", "own_head"]
        );

        let lando = "entrants.drivers.driver_0.head";
        assert_eq!(out.get_text(lando).as_deref(), Some("rio_haryanto"));
        assert!(!built.warnings.iter().any(|w| w.contains("wears head")));
        let built = build(&store, "default", &game).unwrap();
        assert!(
            built.warnings.contains(
                &"Pick: lando_norris wears head rio_haryanto, which no addon on in this profile adds, so it keeps the game's head lando_norris"
                    .to_owned()
            ),
            "{:?}",
            built.warnings
        );
        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(out.get_text(lando).as_deref(), Some("lando_norris"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn skips_removals_of_stock_records_so_runtime_edits_still_apply() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-stockremove-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let mut records = vec![
            Record::new("entrants.teams._team_instancecount_", 0, "1"),
            Record::new("entrants.teams.team_0._metadata_", 4, "mclaren_26"),
            Record::new("entrants.teams.team_0.stat_id", 0, "100"),
        ];
        records.extend((0..5).map(|i| Record::new(&format!("stock.key_{i}"), 0, i.to_string())));
        let stock = Gvdb { records };
        std::fs::write(dir.join(GVDB), stock.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let install = |id: &str, name: &str, ops: &str| {
            let src = root.join("src").join(id);
            std::fs::create_dir_all(&src).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{name}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}, "operations": {{ "assets": ["ops.json"] }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(
                src.join("ops.json"),
                format!(r#"{{ "schema_version": 1, "ops": [{ops}] }}"#),
            )
            .unwrap();
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled(id, true))
                .unwrap();
        };
        let put = |key: &str, value: &str| {
            format!(r#"{{"op":"db.put-record","key":"{key}","type":0,"value":"{value}"}},"#)
        };
        let remove = |key: &str| format!(r#"{{"op":"db.remove-record","key":"{key}"}}"#);
        let mut ops = put("entrants.teams.team_0.stat_id", "3135");
        for i in 0..3 {
            ops += &put(&format!("mod.key_{i}"), "3130");
        }
        for i in 0..5 {
            ops += &remove(&format!("stock.key_{i}"));
            ops += ",";
        }
        ops += &remove("entrants.teams.team_0.stat_id");
        ops += ",";
        ops += &remove("mod.key_0");
        install("t.peugeot", "Peugeot TotalEnergies F1 Team", &ops);

        let built = build(&store, "default", &game).unwrap();
        let line = "Peugeot TotalEnergies F1 Team: 6 removal(s) of stock database records skipped; \
                    the game crashes when a record it reads is missing";
        assert_eq!(
            built
                .warnings
                .iter()
                .filter(|w| w.contains("stock database records"))
                .collect::<Vec<_>>(),
            [line],
            "{:?}",
            built.warnings
        );
        assert_eq!(
            built.report.stock_removals,
            BTreeMap::from([("t.peugeot".to_owned(), 6)])
        );
        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        for r in &stock.records {
            assert!(out.get_key(r.key).is_some(), "{:#018x}", r.key);
        }
        assert_eq!(
            out.get_text("entrants.teams.team_0.stat_id").as_deref(),
            Some("15")
        );
        assert!(out.get("mod.key_0").is_none());
        assert_eq!(out.get_text("mod.key_1").as_deref(), Some("10"));
        assert_eq!(db_edits(&stock, &out).map(|e| e.len()), Ok(3));

        install(
            "t.author",
            "Author",
            &format!(
                r#"{}, {{"op":"db.remove-record","key":"stock.key_4","allow_stock_removal":true}}"#,
                remove("mod.key_1")
            ),
        );
        let built = build(&store, "default", &game).unwrap();
        assert_eq!(
            built.report.stock_removals,
            BTreeMap::from([("t.peugeot".to_owned(), 6)])
        );
        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        assert!(out.get("mod.key_1").is_none());
        assert!(out.get("stock.key_4").is_none());
        assert!(out.get("stock.key_3").is_some());
        assert!(db_edits(&stock, &out).is_err());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn warns_of_the_entrant_lists_an_addon_renames_or_removes() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-keeplists-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let lists = "entrants.entrant_lists";
        let whitelist = "content_sets.content_set_0.available_entrant_lists_0.entrant_lists";
        let mut records = vec![
            Record::new(&format!("{lists}._entrant_list_instancecount_"), 0, "3"),
            Record::new("content_sets._content_set_instancecount_", 0, "1"),
            Record::new(
                "content_sets.content_set_0._available_entrant_lists_instancecount_",
                0,
                "1",
            ),
            Record::new(whitelist, 13, "f1_2026,f2_2026,f2_2025"),
        ];
        for (i, (name, id)) in [("f1_2026", "67"), ("f2_2026", "104"), ("f2_2025", "95")]
            .into_iter()
            .enumerate()
        {
            records.push(Record::new(
                &format!("{lists}.entrant_list_{i}._metadata_"),
                4,
                name,
            ));
            records.push(Record::new(&format!("{lists}.entrant_list_{i}.stat_id"), 0, id));
            records.push(Record::new(
                &format!("{lists}.entrant_list_{i}.display_name"),
                4,
                format!("lng_{name}"),
            ));
        }
        let stock = Gvdb { records };
        std::fs::write(dir.join(GVDB), stock.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let src = root.join("src").join("t.old");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("swofty.json"),
            r#"{ "schema_version": 1, "id": "t.old", "name": "Old Team Pack", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" }, "operations": { "assets": ["ops.json"] } }"#,
        )
        .unwrap();
        let put = |key: &str, value: &str| {
            format!(
                r#"{{"op":"db.put-record","key":"{key}","type":0,"value":"{}"}}"#,
                hex(value.as_bytes())
            )
        };
        let ops = [
            put(&format!("{lists}._entrant_list_instancecount_"), "2"),
            put(&format!("{lists}.entrant_list_1._metadata_"), "f2_2025"),
            put(&format!("{lists}.entrant_list_1.stat_id"), "95"),
            put(whitelist, "f1_2026,f2_2025"),
        ]
        .join(",");
        std::fs::write(
            src.join("ops.json"),
            format!(r#"{{ "schema_version": 1, "ops": [{ops}] }}"#),
        )
        .unwrap();
        store.install_addon(&src).unwrap();
        store
            .update_profile("default", |p| p.set_enabled("t.old", true))
            .unwrap();

        let built = build(&store, "default", &game).unwrap();
        let line =
            "Old Team Pack: renames or removes the entrant list(s) f2_2026; F2 menus may crash with it";
        assert!(built.warnings.iter().any(|w| w == line), "{:?}", built.warnings);
        assert_eq!(
            built.report.kept_lists,
            BTreeMap::from([("t.old".to_owned(), vec!["f2_2026".to_owned()])])
        );
        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(
            out.get_text(&format!("{lists}._entrant_list_instancecount_"))
                .as_deref(),
            Some("2")
        );
        assert_eq!(
            out.get_text(&format!("{lists}.entrant_list_1._metadata_"))
                .as_deref(),
            Some("f2_2025")
        );
        assert_eq!(out.get_text(&format!("{lists}.entrant_list_3._metadata_")), None);
        assert_eq!(out.get_text(whitelist).as_deref(), Some("f1_2026,f2_2025"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_track_record_places_the_files_only_and_the_build_writes_the_track_files() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-trackrecord-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        for s in &SLOTS[..4] {
            let slot = dir.join(slot_file(s));
            std::fs::create_dir_all(slot.parent().unwrap()).unwrap();
            std::fs::write(&slot, archive(&["eaid://ui_package/slot.uil"], 0)).unwrap();
        }
        let res = "tracks.track_resource.TrackResource_0";
        let set = "track_data.game_setup.TrackDataSet_0";
        let layer = "track_data.track_layers.TrackLayer_0";
        let records = vec![
            Record::new("tracks._Track_instancecount_", 0, "2"),
            Record::new("tracks.Track_0._metadata_", 2, "melbourne"),
            Record::new("tracks.Track_0.statId", 0, "5"),
            Record::new("tracks.Track_0.track_resource", 2, "2019"),
            Record::new("tracks.Track_1._metadata_", 2, "sochi"),
            Record::new("tracks.Track_1.statId", 0, "9"),
            Record::new("tracks.Track_1.track_resource", 2, "2019"),
            Record::new("tracks.track_resource._TrackResource_instancecount_", 0, "1"),
            Record::new(&format!("{res}._metadata_"), 2, "2019"),
            Record::new(&format!("{res}.iblAssetGroupsRoot"), 2, "probes/"),
            Record::new(&format!("{res}.iblKey"), 2, "key_2019.assetgroup"),
            Record::new("track_data.game_setup._TrackDataSet_instancecount_", 0, "1"),
            Record::new(&format!("{set}._metadata_"), 2, "f1"),
            Record::new(&format!("{set}._TrackData_instancecount_"), 0, "1"),
            Record::new(&format!("{set}.TrackData_0._metadata_"), 2, "melbourne"),
            Record::new(&format!("{set}.TrackData_0.track"), 2, "melbourne"),
            Record::new("track_data.track_layers._TrackLayer_instancecount_", 0, "1"),
            Record::new(&format!("{layer}._metadata_"), 2, "f1_26"),
            Record::new(&format!("{layer}.game_setup_modes"), 2, "f1_26"),
            Record::new(&format!("{layer}.is_invalid"), 2, "False"),
            Record::new(&format!("{layer}.seasons"), 2, "season_2026"),
            Record::new(&format!("{layer}.weekend_formats"), 2, "default"),
            Record::new(
                &format!("{layer}.worldEri"),
                2,
                "eaid://environment_package/tracks/[FILENAME]/wep/[FILENAME]_f1_26.wep",
            ),
        ];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let install = |id: &str, code: &str, name: &str, ops: bool, record: Option<&str>| {
            let src = root.join("src").join(id);
            std::fs::create_dir_all(src.join("track")).unwrap();
            let operations = match ops {
                true => r#"{ "assets": ["ops.json"] }"#,
                false => "{}",
            };
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{name}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}, "operations": {operations} }}"#
                ),
            )
            .unwrap();
            if ops {
                std::fs::write(
                    src.join("ops.json"),
                    r#"{ "schema_version": 1, "ops": [{"op":"track.add","source":"track"}] }"#,
                )
                .unwrap();
                std::fs::write(
                    src.join("track/manifest.json"),
                    format!(
                        r#"{{ "track_code": "{code}", "track_name": "{name}", "erp": "{code}.erp",
                             "activation": {{ "copy_from_track": "melbourne", "copy_slot": "melbourne" }} }}"#
                    ),
                )
                .unwrap();
            }
            if let Some(record) = record {
                std::fs::create_dir_all(src.join("registry")).unwrap();
                std::fs::write(src.join("registry/track.json"), record).unwrap();
            }
            let world = format!("eaid://environment_package/tracks/{code}/wep/{code}_common.wep");
            std::fs::write(
                src.join(format!("track/{code}.erp")),
                archive_of("WEP", &[&world], 0),
            )
            .unwrap();
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled(id, true))
                .unwrap();
        };
        install("mod.okayama", "okayama", "Okayama", true, None);
        install("mod.sochi", "sochi", "Sochi Autodrom", true, None);
        install(
            "mod.testring",
            "testring",
            "Test Ring",
            true,
            Some(
                r#"{ "track": { "testring": { "code": "testring", "base": "game/melbourne", "files": "track/",
                                                "provides": ["venue/sepang"] } } }"#,
            ),
        );
        install(
            "mod.ring2",
            "ring2",
            "Ring Two",
            false,
            Some(
                r#"{ "track": { "ring2": { "code": "ring2", "name": "Ring Two", "base": "game/melbourne",
                                             "files": "track/" } } }"#,
            ),
        );

        let built = build(&store, "default", &game).unwrap();
        for code in ["testring", "ring2"] {
            let line = format!("{code}: files only; its track record adds its rows in game");
            assert!(built.lines.contains(&line), "{:?}", built.lines);
        }
        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(out.get_text("tracks._Track_instancecount_").as_deref(), Some("3"));
        let labels: Vec<String> = (0..3)
            .filter_map(|i| out.get_text(&format!("tracks.Track_{i}._metadata_")))
            .collect();
        assert_eq!(labels, ["melbourne", "sochi", "okayama"]);
        let placed: Vec<&String> = built
            .plan
            .data
            .keys()
            .filter(|k| k.contains("invitational_image"))
            .collect();
        assert_eq!(placed.len(), 4, "{placed:?}");

        let older: crate::tracks::OlderTracks = crate::read_json(&crate::stdlib::reports::path(
            &store.root,
            crate::tracks::OLDER_FILE,
        ))
        .unwrap();
        assert_eq!(older.profile, "default");
        let entry = |code: &str| {
            let t = older.tracks.iter().find(|t| t.code == code).unwrap();
            (t.addon.as_str(), t.slot.as_deref(), t.stat_id, t.venue.as_deref())
        };
        assert_eq!(older.tracks.len(), 2, "{:?}", older.tracks);
        assert_eq!(
            entry("okayama"),
            ("mod.okayama", None, Some(10), Some("venue/okayama"))
        );
        assert_eq!(
            entry("sochi"),
            ("mod.sochi", Some("game/sochi"), Some(9), Some("venue/sochi"))
        );
        let facts = crate::activation::facts::read(&store.root).unwrap();
        assert_eq!((facts.profile.as_str(), facts.layers.len()), ("default", 1));
        assert_eq!(facts.layers[0].world, "f1_26");
        let mut codes: Vec<&String> = facts.tracks.keys().collect();
        codes.sort();
        assert_eq!(codes, ["okayama", "ring2", "sochi", "testring"]);
        let written = std::fs::metadata(crate::activation::facts::path(&store.root))
            .unwrap()
            .modified()
            .unwrap();
        let again = build(&store, "default", &game).unwrap();
        assert_eq!(again.buildhash, built.buildhash);
        let kept = std::fs::metadata(crate::activation::facts::path(&store.root))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(
            kept, written,
            "a reused build with the same inputs keeps the facts file"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_track_rows_stat_id_is_an_added_row_id_and_a_lent_one_is_the_one_the_build_gives() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-tracklent-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let slot = dir.join(slot_file(SLOTS[0]));
        std::fs::create_dir_all(slot.parent().unwrap()).unwrap();
        std::fs::write(&slot, archive(&["eaid://ui_package/slot.uil"], 0)).unwrap();
        let res = "tracks.track_resource.TrackResource_0";
        let set = "track_data.game_setup.TrackDataSet_0";
        let records = vec![
            Record::new("tracks._Track_instancecount_", 0, "1"),
            Record::new("tracks.Track_0._metadata_", 2, "melbourne"),
            Record::new("tracks.Track_0.statId", 0, "5"),
            Record::new("tracks.Track_0.track_resource", 2, "2019"),
            Record::new("tracks.track_resource._TrackResource_instancecount_", 0, "1"),
            Record::new(&format!("{res}._metadata_"), 2, "2019"),
            Record::new(&format!("{res}.iblAssetGroupsRoot"), 2, "probes/"),
            Record::new(&format!("{res}.iblKey"), 2, "key_2019.assetgroup"),
            Record::new("track_data.game_setup._TrackDataSet_instancecount_", 0, "1"),
            Record::new(&format!("{set}._metadata_"), 2, "f1"),
            Record::new(&format!("{set}._TrackData_instancecount_"), 0, "1"),
            Record::new(&format!("{set}.TrackData_0._metadata_"), 2, "melbourne"),
            Record::new(&format!("{set}.TrackData_0.track"), 2, "melbourne"),
        ];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let src = root.join("src").join("mod.testring");
        std::fs::create_dir_all(src.join("track")).unwrap();
        std::fs::write(
            src.join("swofty.json"),
            r#"{ "schema_version": 1, "id": "mod.testring", "name": "Test Ring", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" }, "operations": { "assets": ["ops.json"] } }"#,
        )
        .unwrap();
        std::fs::write(
            src.join("ops.json"),
            r#"{ "schema_version": 1, "ops": [{"op":"track.add","source":"track"}] }"#,
        )
        .unwrap();
        std::fs::write(
            src.join("track/manifest.json"),
            r#"{ "track_code": "testring", "track_name": "Test Ring", "erp": "testring.erp",
                 "activation": { "copy_from_track": "melbourne", "copy_slot": "melbourne" } }"#,
        )
        .unwrap();
        let world = "eaid://environment_package/tracks/testring/wep/testring_common.wep";
        std::fs::write(src.join("track/testring.erp"), archive_of("WEP", &[world], 0)).unwrap();
        store.install_addon(&src).unwrap();
        store
            .update_profile("default", |p| p.set_enabled("mod.testring", true))
            .unwrap();
        let stat_id = |b: &Built| {
            let db = Gvdb::parse(&std::fs::read(&b.plan.data[GVDB]).unwrap()).unwrap();
            db.get_text("tracks.Track_1.statId").unwrap()
        };
        let row = crate::row_ids::track_row("testring");

        let own = build(&store, "default", &game).unwrap();
        assert_eq!(stat_id(&own), "6");
        assert_eq!(own.row_ids["mod.testring"][&row], 6, "{:?}", own.row_ids);
        let outputs: BTreeMap<String, BTreeMap<String, fingerprint::Output>> =
            crate::read_json(&store.root.join(fingerprint::OUTPUTS_FILE)).unwrap();
        assert_eq!(outputs["default"]["mod.testring"].row_ids[&row], 6);
        let kept = std::fs::read(crate::row_ids::path(&store.root)).unwrap();

        let host = crate::row_ids::Overrides {
            rows: BTreeMap::from([("mod.testring".to_owned(), BTreeMap::from([(row.clone(), 40)]))]),
            ..Default::default()
        };
        let lending = LENDING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        crate::row_ids::lend(Some(host));
        let lent = build(&store, "default", &game);
        crate::row_ids::lend(None);
        drop(lending);
        let lent = lent.unwrap();
        assert_eq!(stat_id(&lent), "40");
        assert_eq!(lent.row_ids["mod.testring"][&row], 40);
        assert_eq!(
            std::fs::read(crate::row_ids::path(&store.root)).unwrap(),
            kept,
            "row-ids.json keeps the player's own id"
        );
        let back = build(&store, "default", &game).unwrap();
        assert_eq!(stat_id(&back), "6");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn skips_removals_of_the_records_a_track_adds() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-trackremove-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let slot = dir.join(slot_file(SLOTS[0]));
        std::fs::create_dir_all(slot.parent().unwrap()).unwrap();
        std::fs::write(&slot, archive(&["eaid://ui_package/slot.uil"], 0)).unwrap();
        let res = "tracks.track_resource.TrackResource_0";
        let set = "track_data.game_setup.TrackDataSet_0";
        let records = vec![
            Record::new("tracks._Track_instancecount_", 0, "1"),
            Record::new("tracks.Track_0._metadata_", 2, "melbourne"),
            Record::new("tracks.Track_0.statId", 0, "5"),
            Record::new("tracks.Track_0.track_resource", 2, "2019"),
            Record::new("tracks.track_resource._TrackResource_instancecount_", 0, "1"),
            Record::new(&format!("{res}._metadata_"), 2, "2019"),
            Record::new(&format!("{res}.iblAssetGroupsRoot"), 2, "probes/"),
            Record::new(&format!("{res}.iblKey"), 2, "key_2019.assetgroup"),
            Record::new("track_data.game_setup._TrackDataSet_instancecount_", 0, "1"),
            Record::new(&format!("{set}._metadata_"), 2, "f1"),
            Record::new(&format!("{set}._TrackData_instancecount_"), 0, "1"),
            Record::new(&format!("{set}.TrackData_0._metadata_"), 2, "melbourne"),
            Record::new(&format!("{set}.TrackData_0.track"), 2, "melbourne"),
            Record::new("stock.key_0", 0, "0"),
        ];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let install = |id: &str, name: &str, ops: &str, track: bool| {
            let src = root.join("src").join(id);
            std::fs::create_dir_all(src.join("track")).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{name}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}, "operations": {{ "assets": ["ops.json"] }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(
                src.join("ops.json"),
                format!(r#"{{ "schema_version": 1, "ops": [{ops}] }}"#),
            )
            .unwrap();
            if track {
                std::fs::write(
                    src.join("track/manifest.json"),
                    r#"{ "track_code": "testring", "track_name": "Test Ring", "erp": "testring.erp",
                         "activation": { "copy_from_track": "melbourne", "copy_slot": "melbourne" } }"#,
                )
                .unwrap();
                let world = "eaid://environment_package/tracks/testring/wep/testring_common.wep";
                std::fs::write(src.join("track/testring.erp"), archive_of("WEP", &[world], 0)).unwrap();
            }
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled(id, true))
                .unwrap();
        };
        let track_keys = [
            "tracks.Track_1._metadata_",
            "tracks.Track_1.statId",
            "tracks.track_resource.TrackResource_1.iblKey",
            &format!("{set}.TrackData_1.track"),
        ];
        let remove =
            |key: &str| format!(r#"{{"op":"db.remove-record","key":"{key}","allow_stock_removal":true}},"#);
        let mut ops: String = track_keys.iter().map(|k| remove(k)).collect();
        ops += &remove("stock.key_0");
        ops += r#"{"op":"db.put-record","key":"mod.key_0","type":0,"value":"3130"},"#;
        ops += r#"{"op":"db.remove-record","key":"mod.key_0"}"#;
        install("t.orlen", "Scuderia AlphaTauri ORLEN", &ops, false);
        install(
            "mod.testring",
            "Test Ring",
            r#"{"op":"track.add","source":"track"}"#,
            true,
        );

        let built = build(&store, "default", &game).unwrap();
        let cloned = "testring: testring: new track row cloned from melbourne".to_owned();
        assert!(built.lines.contains(&cloned), "{:?}", built.lines);
        let line = "Scuderia AlphaTauri ORLEN: 4 removal(s) of database records the tracks add skipped; \
                    the game crashes when a record it reads is missing";
        assert_eq!(
            built
                .warnings
                .iter()
                .filter(|w| w.contains("removal(s)"))
                .collect::<Vec<_>>(),
            [line],
            "{:?}",
            built.warnings
        );
        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        for key in track_keys {
            assert!(out.get(key).is_some(), "{key}");
        }
        assert_eq!(out.get_text("tracks._Track_instancecount_").as_deref(), Some("2"));
        assert!(out.get("stock.key_0").is_none());
        assert!(out.get("mod.key_0").is_none());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn keeps_the_track_changes_a_whole_database_mod_carries_back() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-trackkeep-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let slot = dir.join(slot_file(SLOTS[0]));
        std::fs::create_dir_all(slot.parent().unwrap()).unwrap();
        std::fs::write(&slot, archive(&["eaid://ui_package/slot.uil"], 0)).unwrap();
        let res = "tracks.track_resource.TrackResource_0";
        let set = "track_data.game_setup.TrackDataSet_0";
        let list = "content_sets.content_set_0.available_tracks_0.tracks";
        let records = vec![
            Record::new("tracks._Track_instancecount_", 0, "1"),
            Record::new("tracks.Track_0._metadata_", 2, "melbourne"),
            Record::new("tracks.Track_0.statId", 0, "5"),
            Record::new("tracks.Track_0.track_resource", 2, "2019"),
            Record::new("tracks.track_resource._TrackResource_instancecount_", 0, "1"),
            Record::new(&format!("{res}._metadata_"), 2, "2019"),
            Record::new(&format!("{res}.iblAssetGroupsRoot"), 2, "probes/"),
            Record::new(&format!("{res}.iblKey"), 2, "key_2019.assetgroup"),
            Record::new("track_data.game_setup._TrackDataSet_instancecount_", 0, "1"),
            Record::new(&format!("{set}._metadata_"), 2, "f1"),
            Record::new(&format!("{set}._TrackData_instancecount_"), 0, "1"),
            Record::new(&format!("{set}.TrackData_0._metadata_"), 2, "melbourne"),
            Record::new(&format!("{set}.TrackData_0.track"), 2, "melbourne"),
            Record::new("content_sets._content_set_instancecount_", 0, "1"),
            Record::new("content_sets.content_set_0._metadata_", 2, "2025"),
            Record::new(
                "content_sets.content_set_0._available_tracks_instancecount_",
                0,
                "1",
            ),
            Record::new(list, 13, "melbourne"),
            Record::new("stock.key_0", 0, "0"),
        ];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let install = |id: &str, name: &str, ops: &str, track: bool| {
            let src = root.join("src").join(id);
            std::fs::create_dir_all(src.join("track")).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{name}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}, "operations": {{ "assets": ["ops.json"] }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(
                src.join("ops.json"),
                format!(r#"{{ "schema_version": 1, "ops": [{ops}] }}"#),
            )
            .unwrap();
            if track {
                std::fs::write(
                    src.join("track/manifest.json"),
                    r#"{ "track_code": "testring", "track_name": "Test Ring", "erp": "testring.erp",
                         "activation": { "copy_from_track": "melbourne", "copy_slot": "melbourne" } }"#,
                )
                .unwrap();
                let world = "eaid://environment_package/tracks/testring/wep/testring_common.wep";
                std::fs::write(src.join("track/testring.erp"), archive_of("WEP", &[world], 0)).unwrap();
            }
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled(id, true))
                .unwrap();
        };
        install(
            "mod.testring",
            "Test Ring",
            r#"{"op":"track.add","source":"track"}"#,
            true,
        );
        let put = |key: &str, ty: u8, value: &str| {
            let hex: String = value.bytes().map(|b| format!("{b:02x}")).collect();
            format!(r#"{{"op":"db.put-record","key":"{key}","type":{ty},"value":"{hex}"}}"#)
        };
        let ops = [
            put(list, 13, "melbourne"),
            put("tracks._Track_instancecount_", 0, "1"),
            put("stock.key_0", 0, "7"),
        ]
        .join(",");
        install("t.orlen", "Scuderia AlphaTauri ORLEN", &ops, false);

        let built = build(&store, "default", &game).unwrap();
        let line =
            "Scuderia AlphaTauri ORLEN: kept the tracks' changes to 2 database value(s) it sets back to the \
                    game's own"
                .to_owned();
        assert!(built.lines.contains(&line), "{:?}", built.lines);
        let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(out.get_text(list).as_deref(), Some("melbourne,testring"));
        assert_eq!(out.get_text("tracks._Track_instancecount_").as_deref(), Some("2"));
        assert_eq!(out.get_text("stock.key_0").as_deref(), Some("7"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn put_targets_take_the_games_spelling_and_the_ers_pass_builds_on_the_addons_file() {
        use swofty_formats::bxml::{Document, Element, Kind};
        use swofty_formats::erp::{Erp, ErpFile, Fragment, HEADER_LEN, MAGIC, Resource, STORED};
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-putcase-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let slot = dir.join(slot_file(SLOTS[0]));
        std::fs::create_dir_all(slot.parent().unwrap()).unwrap();
        std::fs::write(&slot, archive(&["eaid://ui_package/slot.uil"], 0)).unwrap();
        let res = "tracks.track_resource.TrackResource_0";
        let set = "track_data.game_setup.TrackDataSet_0";
        let records = vec![
            Record::new("tracks._Track_instancecount_", 0, "1"),
            Record::new("tracks.Track_0._metadata_", 2, "melbourne"),
            Record::new("tracks.Track_0.statId", 0, "5"),
            Record::new("tracks.Track_0.track_resource", 2, "2019"),
            Record::new("tracks.track_resource._TrackResource_instancecount_", 0, "1"),
            Record::new(&format!("{res}._metadata_"), 2, "2019"),
            Record::new(&format!("{res}.iblAssetGroupsRoot"), 2, "probes/"),
            Record::new(&format!("{res}.iblKey"), 2, "key_2019.assetgroup"),
            Record::new("track_data.game_setup._TrackDataSet_instancecount_", 0, "1"),
            Record::new(&format!("{set}._metadata_"), 2, "f1"),
            Record::new(&format!("{set}._TrackData_instancecount_"), 0, "1"),
            Record::new(&format!("{set}.TrackData_0._metadata_"), 2, "melbourne"),
            Record::new(&format!("{set}.TrackData_0.track"), 2, "melbourne"),
        ];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let el = |name: &str, attrs: &[(&str, &str)], children: Vec<Element>| Element {
            kind: Kind::Element,
            pad: 0,
            name: name.as_bytes().to_vec(),
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.as_bytes().to_vec(), v.as_bytes().to_vec()))
                .collect(),
            children,
            end_pad: 0,
        };
        let uri = "eaid://f1_2026_vehicle_package/teams/mclaren/handling/mclaren.vtf";
        let handling = |deploy: &str| {
            let doc = Document {
                root: el(
                    "VTF",
                    &[],
                    vec![el(
                        "EnergyRecoverySystem",
                        &[("numTrackSettingsTypes", "1")],
                        vec![el(
                            "TrackSettings",
                            &[("name", "0")],
                            vec![
                                el("Track", &[("value", "melbourne")], Vec::new()),
                                el("MediumDeployFactor", &[("value", deploy)], Vec::new()),
                                el("OverrideDetectionLapProgress", &[("value", "0.8")], Vec::new()),
                                el("OverrideActivationLapProgress", &[("value", "0.85")], Vec::new()),
                            ],
                        )],
                    )],
                ),
                tail: Vec::new(),
            };
            let data = doc.to_bytes();
            let mut header = [0u8; HEADER_LEN];
            header[..4].copy_from_slice(MAGIC);
            header[4..8].copy_from_slice(&3u32.to_le_bytes());
            Erp {
                header,
                version: 3,
                resources: vec![Resource {
                    name: uri.into(),
                    kind: swofty_formats::erp::kind("VTF").unwrap(),
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
                }],
            }
            .to_bytes()
        };
        let team = "2025_asset_groups/f1_2026_vehicle_package/teams/mclaren/wep/mclaren.erp";
        std::fs::create_dir_all(dir.join(team).parent().unwrap()).unwrap();
        std::fs::write(dir.join(team), handling("0.8")).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let install = |id: &str, ops: &str, files: &[(&str, Vec<u8>)]| {
            let src = root.join("src").join(id);
            std::fs::create_dir_all(src.join("track")).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{id}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}, "operations": {{ "assets": ["ops.json"] }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(
                src.join("ops.json"),
                format!(r#"{{ "schema_version": 1, "ops": [{ops}] }}"#),
            )
            .unwrap();
            for (rel, data) in files {
                std::fs::write(src.join(rel), data).unwrap();
            }
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled(id, true))
                .unwrap();
        };
        let world = "eaid://environment_package/tracks/testring/wep/testring_common.wep";
        install(
            "mod.testring",
            r#"{"op":"track.add","source":"track"}"#,
            &[
                (
                    "track/manifest.json",
                    br#"{ "track_code": "testring", "track_name": "Test Ring", "erp": "testring.erp",
                         "activation": { "copy_from_track": "melbourne", "copy_slot": "melbourne" },
                         "ers": { "OverrideDetectionLapProgress": 0.91, "OverrideActivationLapProgress": 0.95 } }"#
                        .to_vec(),
                ),
                ("track/testring.erp", archive_of("WEP", &[world], 0)),
            ],
        );
        let upper = "2025_asset_groups/F1_2026_vehicle_package/teams/mclaren/wep/mclaren.erp";
        install(
            "t.upper",
            &format!(r#"{{"op":"file.put","source":"u.erp","target":"{upper}"}}"#),
            &[("u.erp", handling("0.7"))],
        );
        let value = |archive: &Path, track: &str, key: &str| {
            let file = ErpFile::open(archive).unwrap();
            let r = file.erp.resources.iter().find(|r| r.name == uri).unwrap();
            let doc = Document::parse(&file.load(r).unwrap().fragments[0].data().unwrap()).unwrap();
            doc.root
                .child(b"EnergyRecoverySystem")
                .unwrap()
                .children
                .iter()
                .find(|c| c.child(b"Track").and_then(|t| t.attr(b"value")) == Some(track.as_bytes()))
                .and_then(|c| c.child(key.as_bytes()))
                .and_then(|f| f.attr(b"value"))
                .map(|v| String::from_utf8_lossy(v).into_owned())
        };
        let same = |plan: &Plan| {
            plan.data
                .keys()
                .chain(plan.added.keys())
                .filter(|k| k.eq_ignore_ascii_case(team))
                .cloned()
                .collect::<Vec<_>>()
        };

        let built = build(&store, "default", &game).unwrap();
        assert_eq!(same(&built.plan), [team.to_owned()]);
        let car = &built.plan.data[team];
        assert_eq!(
            value(car, "melbourne", "MediumDeployFactor").as_deref(),
            Some("0.7")
        );
        assert_eq!(
            value(car, "testring", "MediumDeployFactor").as_deref(),
            Some("0.7")
        );
        assert_eq!(
            value(car, "testring", "OverrideDetectionLapProgress").as_deref(),
            Some("0.91")
        );

        let other = "2025_asset_groups/f1_2026_vehicle_package/teams/mclaren/wep/MCLAREN.erp";
        install(
            "t.other",
            &format!(r#"{{"op":"file.put","source":"o.erp","target":"{other}"}}"#),
            &[("o.erp", handling("0.4"))],
        );
        install(
            "t.fresh",
            r#"{"op":"file.put","source":"f.bin","target":"Mixed_Case/New_File.bin"}"#,
            &[("f.bin", b"new".to_vec())],
        );
        let built = build(&store, "default", &game).unwrap();
        assert_eq!(same(&built.plan), [team.to_owned()]);
        assert!(
            built
                .lines
                .iter()
                .any(|l| l.starts_with(&format!("files: {team} is put by both"))),
            "{:?}",
            built.lines
        );
        assert!(built.plan.added.contains_key("Mixed_Case/New_File.bin"));
        assert!(!built.plan.added.contains_key("mixed_case/new_file.bin"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_overlay_with_a_handling_file_never_loads_as_it_is() {
        use swofty_formats::erp::{Erp, Fragment, HEADER_LEN, MAGIC, Resource, STORED};
        let archive = |name: &str, kind: &str| {
            let mut header = [0u8; HEADER_LEN];
            header[..4].copy_from_slice(MAGIC);
            header[4..8].copy_from_slice(&3u32.to_le_bytes());
            Erp {
                header,
                version: 3,
                resources: vec![Resource {
                    name: name.into(),
                    kind: swofty_formats::erp::kind(kind).unwrap(),
                    unk: vec![0; 4],
                    fragments: vec![Fragment {
                        name: *b"temp",
                        offset: 0,
                        size: 4,
                        flags: 0x10,
                        compression: STORED,
                        packed: 4,
                        raw: b"data".to_vec(),
                    }],
                    hash: [0; 16],
                }],
            }
            .to_bytes()
        };
        let dir = std::env::temp_dir().join(format!("swofty-carries-handling-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let (vtf, other) = (dir.join("vtf.erp"), dir.join("other.erp"));
        std::fs::write(
            &vtf,
            archive(
                "eaid://f1_2026_vehicle_package/teams/ferrari/handling/ferrari.vtf",
                "VTF",
            ),
        )
        .unwrap();
        std::fs::write(
            &other,
            archive(
                "eaid://f1_2026_vehicle_package/teams/ferrari/art/a.tex",
                "Texture",
            ),
        )
        .unwrap();
        let ferrari = ["2025_asset_groups/f1_2026_vehicle_package/teams/ferrari/wep/ferrari.erp".to_owned()];
        let monza =
            ["2025_asset_groups/f1_2025_vehicle_package/teams/ferrari/wep/ferrari_monza_25.erp".to_owned()];
        let track = ["2025_asset_groups/track_package/a.erp".to_owned()];
        assert!(carries_handling(&vtf, &ferrari).unwrap());
        assert!(carries_handling(&vtf, &monza).unwrap());
        assert!(!carries_handling(&vtf, &track).unwrap());
        assert!(!carries_handling(&other, &ferrari).unwrap());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn track_ers_settings_survive_an_addon_that_writes_the_handling_file() {
        use swofty_formats::bxml::{Document, Element, Kind};
        use swofty_formats::erp::{Erp, ErpFile, Fragment, HEADER_LEN, MAGIC, Resource, STORED};
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-ers-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let slot = dir.join(slot_file(SLOTS[0]));
        std::fs::create_dir_all(slot.parent().unwrap()).unwrap();
        std::fs::write(&slot, archive(&["eaid://ui_package/slot.uil"], 0)).unwrap();
        let res = "tracks.track_resource.TrackResource_0";
        let set = "track_data.game_setup.TrackDataSet_0";
        let records = vec![
            Record::new("tracks._Track_instancecount_", 0, "1"),
            Record::new("tracks.Track_0._metadata_", 2, "melbourne"),
            Record::new("tracks.Track_0.statId", 0, "5"),
            Record::new("tracks.Track_0.track_resource", 2, "2019"),
            Record::new("tracks.track_resource._TrackResource_instancecount_", 0, "1"),
            Record::new(&format!("{res}._metadata_"), 2, "2019"),
            Record::new(&format!("{res}.iblAssetGroupsRoot"), 2, "probes/"),
            Record::new(&format!("{res}.iblKey"), 2, "key_2019.assetgroup"),
            Record::new("track_data.game_setup._TrackDataSet_instancecount_", 0, "1"),
            Record::new(&format!("{set}._metadata_"), 2, "f1"),
            Record::new(&format!("{set}._TrackData_instancecount_"), 0, "1"),
            Record::new(&format!("{set}.TrackData_0._metadata_"), 2, "melbourne"),
            Record::new(&format!("{set}.TrackData_0.track"), 2, "melbourne"),
        ];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let el = |name: &str, attrs: &[(&str, &str)], children: Vec<Element>| Element {
            kind: Kind::Element,
            pad: 0,
            name: name.as_bytes().to_vec(),
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.as_bytes().to_vec(), v.as_bytes().to_vec()))
                .collect(),
            children,
            end_pad: 0,
        };
        let uri = "eaid://f1_2026_vehicle_package/teams/mclaren/handling/mclaren.vtf";
        let handling = |deploy: &str| {
            let doc = Document {
                root: el(
                    "VTF",
                    &[],
                    vec![el(
                        "EnergyRecoverySystem",
                        &[("numTrackSettingsTypes", "1")],
                        vec![el(
                            "TrackSettings",
                            &[("name", "0")],
                            vec![
                                el("Track", &[("value", "melbourne")], Vec::new()),
                                el("MediumDeployFactor", &[("value", deploy)], Vec::new()),
                                el("OverrideDetectionLapProgress", &[("value", "0.8")], Vec::new()),
                                el("OverrideActivationLapProgress", &[("value", "0.85")], Vec::new()),
                            ],
                        )],
                    )],
                ),
                tail: Vec::new(),
            };
            let data = doc.to_bytes();
            let mut header = [0u8; HEADER_LEN];
            header[..4].copy_from_slice(MAGIC);
            header[4..8].copy_from_slice(&3u32.to_le_bytes());
            Erp {
                header,
                version: 3,
                resources: vec![Resource {
                    name: uri.into(),
                    kind: swofty_formats::erp::kind("VTF").unwrap(),
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
                }],
            }
            .to_bytes()
        };
        let team = "2025_asset_groups/f1_2026_vehicle_package/teams/mclaren/wep/mclaren.erp";
        std::fs::create_dir_all(dir.join(team).parent().unwrap()).unwrap();
        std::fs::write(dir.join(team), handling("0.8")).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let install = |id: &str, ops: &str, files: &[(&str, Vec<u8>)]| {
            let src = root.join("src").join(id);
            std::fs::create_dir_all(src.join("track")).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{id}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}, "operations": {{ "assets": ["ops.json"] }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(
                src.join("ops.json"),
                format!(r#"{{ "schema_version": 1, "ops": [{ops}] }}"#),
            )
            .unwrap();
            for (rel, data) in files {
                std::fs::write(src.join(rel), data).unwrap();
            }
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled(id, true))
                .unwrap();
        };
        let world = "eaid://environment_package/tracks/testring/wep/testring_common.wep";
        install(
            "mod.testring",
            r#"{"op":"track.add","source":"track"}"#,
            &[
                (
                    "track/manifest.json",
                    br#"{ "track_code": "testring", "track_name": "Test Ring", "erp": "testring.erp",
                         "activation": { "copy_from_track": "melbourne", "copy_slot": "melbourne" },
                         "ers": { "OverrideDetectionLapProgress": 0.91, "OverrideActivationLapProgress": 0.95 } }"#
                        .to_vec(),
                ),
                ("track/testring.erp", archive_of("WEP", &[world], 0)),
            ],
        );
        install(
            "t.handling",
            &format!(r#"{{"op":"erp.put-resource","erp":"{team}","resource":"{uri}","source":"h.erp"}}"#),
            &[("h.erp", handling("0.5"))],
        );
        let season = "2025_asset_groups/cs_2019_vehicle_package/teams/mclaren/wep/mclaren.erp";
        install(
            "t.season",
            &format!(r#"{{"op":"file.put","source":"cs.erp","target":"{season}"}}"#),
            &[("cs.erp", handling("0.6"))],
        );

        let built = build(&store, "default", &game).unwrap();
        for line in [
            "ers: 1 handling file(s) other addons change get the tracks' settings too",
            "ers: 1 handling file(s) in car archives addons put get the tracks' settings",
        ] {
            assert!(built.lines.contains(&line.to_owned()), "{:?}", built.lines);
        }
        let value = |archive: &Path, track: &str, key: &str| {
            let file = ErpFile::open(archive).unwrap();
            let r = file.erp.resources.iter().find(|r| r.name == uri).unwrap();
            let doc = Document::parse(&file.load(r).unwrap().fragments[0].data().unwrap()).unwrap();
            doc.root
                .child(b"EnergyRecoverySystem")
                .unwrap()
                .children
                .iter()
                .find(|c| c.child(b"Track").and_then(|t| t.attr(b"value")) == Some(track.as_bytes()))
                .and_then(|c| c.child(key.as_bytes()))
                .and_then(|f| f.attr(b"value"))
                .map(|v| String::from_utf8_lossy(v).into_owned())
        };
        let car = &built.plan.data[team];
        assert_eq!(
            value(car, "melbourne", "MediumDeployFactor").as_deref(),
            Some("0.5")
        );
        assert_eq!(
            value(car, "testring", "MediumDeployFactor").as_deref(),
            Some("0.5")
        );
        assert_eq!(
            value(car, "testring", "OverrideDetectionLapProgress").as_deref(),
            Some("0.91")
        );
        assert_eq!(
            value(car, "testring", "OverrideActivationLapProgress").as_deref(),
            Some("0.95")
        );
        let car = &built.plan.added[season];
        assert_eq!(
            value(car, "melbourne", "MediumDeployFactor").as_deref(),
            Some("0.6")
        );
        assert_eq!(
            value(car, "testring", "OverrideActivationLapProgress").as_deref(),
            Some("0.95")
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_handling_record_changes_the_stock_team_archive_and_an_addons_own() {
        use swofty_formats::bxml::{Document, Element, Kind};
        use swofty_formats::erp::{Erp, ErpFile, Fragment, HEADER_LEN, MAGIC, Resource, STORED};
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-handling-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let slot = dir.join(slot_file(SLOTS[0]));
        std::fs::create_dir_all(slot.parent().unwrap()).unwrap();
        std::fs::write(&slot, archive(&["eaid://ui_package/slot.uil"], 0)).unwrap();
        let records = vec![Record::new("tracks._Track_instancecount_", 0, "0")];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let el = |name: &str, attrs: &[(&str, &str)], children: Vec<Element>| Element {
            kind: Kind::Element,
            pad: 0,
            name: name.as_bytes().to_vec(),
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.as_bytes().to_vec(), v.as_bytes().to_vec()))
                .collect(),
            children,
            end_pad: 0,
        };
        let leaf = |name: &str, v: &str| el(name, &[("value", v)], Vec::new());
        let uri = "eaid://f1_2026_vehicle_package/teams/mclaren/handling/mclaren.vtf";
        let handling = || {
            let doc = Document {
                root: el(
                    "VTF",
                    &[],
                    vec![
                        el(
                            "GearBox",
                            &[],
                            vec![
                                leaf("GearRatio1st", "3.5"),
                                leaf("GearRatio2nd", "2.6"),
                                leaf("GearRatio3rd", "2.0"),
                                leaf("GearUpSpeedMultiple1st", "0.8"),
                                leaf("GearUpSpeedMultiple2nd", "0.9"),
                                leaf("GearUpSpeedMultiple3rd", "1.0"),
                            ],
                        ),
                        el("Chassis", &[], vec![leaf("Mass", "771")]),
                        el(
                            "EnergyRecoverySystem",
                            &[],
                            vec![el("Common", &[], vec![leaf("MaxDeployPower", "350000")])],
                        ),
                        el(
                            "Upgrade",
                            &[("name", "radUpgradeERS")],
                            vec![el("EnergyRecoverySystem", &[], vec![leaf("X", "1")])],
                        ),
                    ],
                ),
                tail: Vec::new(),
            };
            let data = doc.to_bytes();
            let mut header = [0u8; HEADER_LEN];
            header[..4].copy_from_slice(MAGIC);
            header[4..8].copy_from_slice(&3u32.to_le_bytes());
            Erp {
                header,
                version: 3,
                resources: vec![Resource {
                    name: uri.into(),
                    kind: swofty_formats::erp::kind("VTF").unwrap(),
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
                }],
            }
            .to_bytes()
        };
        let team = "2025_asset_groups/f1_2026_vehicle_package/teams/mclaren/wep/mclaren.erp";
        std::fs::create_dir_all(dir.join(team).parent().unwrap()).unwrap();
        std::fs::write(dir.join(team), handling()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let season = "2025_asset_groups/f1_1990_vehicle_package/teams/mclaren/wep/mclaren_handling.erp";
        let src = root.join("src/t.cars");
        std::fs::create_dir_all(src.join("registry")).unwrap();
        std::fs::write(
            src.join("swofty.json"),
            r#"{ "schema_version": 1, "id": "t.cars", "name": "t.cars", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" }, "operations": { "assets": ["ops.json"] },
                 "options": [{ "id": "gears", "type": "choice", "values": ["six", "four"], "default": "six", "binds": "db" }] }"#,
        )
        .unwrap();
        std::fs::write(
            src.join("ops.json"),
            format!(r#"{{ "schema_version": 1, "ops": [{{"op":"file.put","source":"h.erp","target":"{season}"}}] }}"#),
        )
        .unwrap();
        std::fs::write(src.join("h.erp"), handling()).unwrap();
        std::fs::write(
            src.join("registry/handling.json"),
            format!(
                r#"{{ "vehicle.handling": {{
                    "stock_mclaren": {{ "vtf": {{ "stock": "{team}" }}, "ers": null,
                        "edits": [{{ "path": "chassis.mass", "op": "scale", "value": 0.8 }}],
                        "gearbox": {{ "ratios": [3.0, 2.2] }} }},
                    "own": {{ "vtf": "h.erp", "chassis": {{ "mass": 600 }} }},
                    "$select": [{{ "ids": "t.cars/own", "when": {{ "option": "gears", "is": "four" }},
                        "patch": {{ "gearbox": {{ "ratios": [3.0, 2.0, 1.5, 1.0] }} }} }}]
                }} }}"#
            ),
        )
        .unwrap();
        store.install_addon(&src).unwrap();
        store
            .update_profile("default", |p| p.set_enabled("t.cars", true))
            .unwrap();
        let read = |archive: &Path| {
            let file = ErpFile::open(archive).unwrap();
            let r = file.erp.resources.iter().find(|r| r.name == uri).unwrap();
            Document::parse(&file.load(r).unwrap().fragments[0].data().unwrap()).unwrap()
        };
        let value = |doc: &Document, section: &str, key: &str| {
            doc.root
                .child(section.as_bytes())
                .and_then(|s| s.child(key.as_bytes()))
                .and_then(|f| f.attr(b"value"))
                .map(|v| String::from_utf8_lossy(v).into_owned())
        };

        let built = build(&store, "default", &game).unwrap();
        let doc = read(&built.plan.data[team]);
        assert_eq!(value(&doc, "Chassis", "Mass").as_deref(), Some("616.8"));
        assert_eq!(value(&doc, "GearBox", "GearRatio1st").as_deref(), Some("3"));
        assert_eq!(value(&doc, "GearBox", "GearRatio3rd").as_deref(), Some("0"));
        assert!(
            doc.root
                .child(b"GearBox")
                .unwrap()
                .child(b"GearUpSpeedMultiple2nd")
                .is_none()
        );
        assert!(doc.root.child(b"EnergyRecoverySystem").is_none());
        let doc = read(&built.plan.added[season]);
        assert_eq!(value(&doc, "Chassis", "Mass").as_deref(), Some("600"));
        assert_eq!(value(&doc, "GearBox", "GearRatio3rd").as_deref(), Some("2.0"));
        assert!(
            built
                .lines
                .iter()
                .any(|l| l.contains("handling t.cars/stock_mclaren -> mclaren.vtf")),
            "{:?}",
            built.lines
        );
        assert!(
            built
                .lines
                .iter()
                .any(|l| l.contains("Chassis/Mass: 771 -> 616.8"))
        );

        store
            .update_profile("default", |p| {
                let m = store.addons().unwrap()["t.cars"].clone();
                p.set_option(&m, "gears", "four".into()).map(drop)
            })
            .unwrap();
        let built = build(&store, "default", &game).unwrap();
        let doc = read(&built.plan.added[season]);
        assert_eq!(value(&doc, "GearBox", "GearRatio3rd").as_deref(), Some("1.5"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_tracks_keep_their_menu_pictures_over_an_addon_that_ships_its_own() {
        use swofty_formats::erp::{Erp, ErpFile, Fragment, HEADER_LEN, MAGIC, Resource, STORED};
        use swofty_formats::gvdb::Record;
        use swofty_formats::pkg::{Arr, Node, Obj, Pkg, Val};
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-menus-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let write = |rel: &str, data: Vec<u8>| {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, data).unwrap();
        };
        for slot in [SLOTS[0], SLOTS[1], crate::activation::ui::PICTURE_SLOT] {
            write(&slot_file(slot), archive(&["eaid://ui_package/slot.uil"], 0));
        }
        let res = "tracks.track_resource.TrackResource_0";
        let set = "track_data.game_setup.TrackDataSet_0";
        let records = vec![
            Record::new("tracks._Track_instancecount_", 0, "1"),
            Record::new("tracks.Track_0._metadata_", 2, "melbourne"),
            Record::new("tracks.Track_0.statId", 0, "5"),
            Record::new("tracks.Track_0.track_resource", 2, "2019"),
            Record::new("tracks.track_resource._TrackResource_instancecount_", 0, "1"),
            Record::new(&format!("{res}._metadata_"), 2, "2019"),
            Record::new(&format!("{res}.iblAssetGroupsRoot"), 2, "probes/"),
            Record::new(&format!("{res}.iblKey"), 2, "key_2019.assetgroup"),
            Record::new("track_data.game_setup._TrackDataSet_instancecount_", 0, "1"),
            Record::new(&format!("{set}._metadata_"), 2, "f1"),
            Record::new(&format!("{set}._TrackData_instancecount_"), 0, "1"),
            Record::new(&format!("{set}.TrackData_0._metadata_"), 2, "melbourne"),
            Record::new(&format!("{set}.TrackData_0.track"), 2, "melbourne"),
        ];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();

        let resource = |name: &str, kind: &str, fragments: Vec<Vec<u8>>| {
            let mut r = Resource {
                name: name.into(),
                kind: swofty_formats::erp::kind(kind).unwrap(),
                unk: vec![0; 4],
                fragments: fragments
                    .into_iter()
                    .map(|data| Fragment {
                        name: *b"temp",
                        offset: 0,
                        size: data.len() as u64,
                        flags: 0x10,
                        compression: STORED,
                        packed: data.len() as u64,
                        raw: data,
                    })
                    .collect(),
                hash: [0; 16],
            };
            r.hash = r.content_hash().unwrap();
            r
        };
        let surface = |name: &str, slices: &[u8]| {
            let mut head = vec![0u8; 40];
            head[12..16].copy_from_slice(&2u32.to_le_bytes());
            head[16..20].copy_from_slice(&2u32.to_le_bytes());
            head[28..32].copy_from_slice(&(slices.len() as u32).to_le_bytes());
            let data = slices.iter().flat_map(|b| [*b; 4]).collect();
            resource(&format!("{name}.image"), "GfxSurfaceRes", vec![head, data])
        };
        let view = |name: &str, slices: u32| {
            let mut v = vec![0u8; 0x18];
            v[0x14..].copy_from_slice(&slices.to_le_bytes());
            resource(name, "GfxSurfaceRes", vec![v])
        };
        let erp_of = |resources: Vec<Resource>| {
            let mut header = [0u8; HEADER_LEN];
            header[..4].copy_from_slice(MAGIC);
            header[4..8].copy_from_slice(&3u32.to_le_bytes());
            Erp {
                header,
                version: 3,
                resources,
            }
            .to_bytes()
        };
        let obj = |pairs: Vec<(&str, Node)>| {
            Node::Obj(Obj::new(
                pairs.into_iter().map(|(k, v)| (Val::str(k), v)).collect(),
            ))
        };
        let list = |names: &[&str]| {
            let items = names
                .iter()
                .map(|n| {
                    obj(vec![
                        ("Name", Node::Val(Val::str(*n))),
                        ("Value", Node::Val(Val::str("ui_package/stock.tif"))),
                    ])
                })
                .collect();
            obj(vec![("Contents", Node::Arr(Arr::new(items)))])
        };
        let in_contents = |key: &str, inner: Node| obj(vec![(key, obj(vec![("Contents", inner)]))]);
        let pkg = |name: &str, root: Node| {
            let data = Pkg {
                name: *b"UIDB",
                root,
                vca: Vec::new(),
                layout: None,
            }
            .to_bytes()
            .unwrap();
            resource(name, "UIDB", vec![data])
        };
        const OVERRIDES: &str = "eaid://ui_package/databases/overrides.uidb";
        const MINIMAP: &str = "eaid://ui_package/databases/minimap.uidb";
        const UIDTA: &str = "eaid://ui_package/art/texture_arrays/ui_dynamic_texture_loading.uidta";
        const FLAGS: &str =
            "eaid://ui_package/art/texture_arrays/overrides.uidb/overrides.tracks.flags_circle.texarray";
        const TRACKS: &str =
            "eaid://ui_package/art/texture_arrays/overrides.uidb/overrides.fe_world.tracks.texarray";
        const LOGO: &str = "eaid://ui_package/art/textures/team_logo.tif";
        let stock = ["bahrain", "blank", "melbourne"];
        let overrides = obj(vec![(
            "overrides",
            obj(vec![(
                "Contents",
                obj(vec![
                    (
                        "tracks",
                        obj(vec![("Contents", obj(vec![("flags_circle", list(&stock))]))]),
                    ),
                    (
                        "fe_world",
                        obj(vec![("Contents", obj(vec![("tracks", list(&stock))]))]),
                    ),
                ]),
            )]),
        )]);
        let entry = obj(vec![("Name", Node::Val(Val::str("bahrain")))]);
        let minimap = in_contents(
            "hud",
            in_contents("minimap", in_contents("tracks", Node::Arr(Arr::new(vec![entry])))),
        );
        let ui = |rel: &str| format!("2025_asset_groups/ui_package/{rel}");
        let (front, hub) = (ui("common_front_end.erp"), ui("flow_career_hub.erp"));
        for rel in crate::activation::ui::UI_ERPS {
            let resources = match *rel {
                "databases.erp" => vec![pkg(OVERRIDES, overrides.clone()), pkg(MINIMAP, minimap.clone())],
                "flow_race.erp" => vec![pkg(MINIMAP, minimap.clone())],
                "persistent.erp" => vec![pkg(
                    UIDTA,
                    obj(vec![("TextureArrays", Node::Arr(Arr::new(Vec::new())))]),
                )],
                "common_front_end.erp" => vec![
                    view(FLAGS, 3),
                    surface(FLAGS, &[0, 1, 2]),
                    view(TRACKS, 3),
                    surface(TRACKS, &[0, 1, 2]),
                    surface(LOGO, &[0]),
                ],
                "flow_career_hub.erp" => vec![view(FLAGS, 3), surface(FLAGS, &[0, 1, 2])],
                _ => vec![view("eaid://ui_package/other.tif", 1)],
            };
            write(&ui(rel), erp_of(resources));
        }
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let install = |id: &str, ops: &str, files: Vec<(&str, Vec<u8>)>| {
            let src = root.join("src").join(id);
            std::fs::create_dir_all(src.join("track")).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{id}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}, "operations": {{ "assets": ["ops.json"] }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(
                src.join("ops.json"),
                format!(r#"{{ "schema_version": 1, "ops": [{ops}] }}"#),
            )
            .unwrap();
            for (rel, data) in files {
                std::fs::write(src.join(rel), data).unwrap();
            }
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled(id, true))
                .unwrap();
        };
        for (code, pixel) in [("ring_a", 0xa0u8), ("ring_b", 0xb0)] {
            let picture =
                |key: &str| format!("eaid://ui_package/art/textures/track_installer/{code}/{key}.tif");
            let images = serde_json::json!({ "resources": {
                "flag_circle": picture("flag_circle"),
                "preview_small": picture("preview_small"),
            } });
            let manifest = serde_json::json!({
                "track_code": code,
                "track_name": code,
                "erp": format!("{code}.erp"),
                "activation": { "copy_from_track": "melbourne", "copy_slot": "melbourne" },
                "ui": { "images_file": "images.json" },
            });
            let world = format!("eaid://environment_package/tracks/{code}/wep/{code}_common.wep");
            let archive = erp_of(vec![
                resource(&world, "WEP", vec![vec![0]]),
                view(&picture("flag_circle"), 1),
                surface(&picture("flag_circle"), &[pixel]),
                view(&picture("preview_small"), 1),
                surface(&picture("preview_small"), &[pixel + 1]),
            ]);
            install(
                &format!("mod.{code}"),
                r#"{"op":"track.add","source":"track"}"#,
                vec![
                    ("track/manifest.json", manifest.to_string().into_bytes()),
                    ("track/images.json", images.to_string().into_bytes()),
                    (&format!("track/{code}.erp"), archive),
                ],
            );
        }
        install(
            "t.pack",
            &format!(
                r#"{{"op":"erp.overlay","erps":["{front}"],"source":"front.erp"}},
                   {{"op":"erp.overlay","erps":["{hub}"],"source":"hub.erp"}}"#
            ),
            vec![
                (
                    "front.erp",
                    erp_of(vec![
                        view(FLAGS, 4),
                        surface(FLAGS, &[0, 1, 2, 9]),
                        view(TRACKS, 2),
                        surface(TRACKS, &[0, 9]),
                        surface(LOGO, &[7]),
                    ]),
                ),
                (
                    "hub.erp",
                    erp_of(vec![view(FLAGS, 4), surface(FLAGS, &[0, 1, 2, 9])]),
                ),
            ],
        );

        let built = build(&store, "default", &game).unwrap();
        assert!(
            built.warnings.contains(&kept_menus_line("t.pack")),
            "{:#?}",
            built.warnings
        );
        let slices = |rel: &str, name: &str| {
            let file = ErpFile::open(&built.plan.data[rel]).unwrap();
            let r = file.load(file.erp.find(name).unwrap()).unwrap();
            let data = r.fragments.last().unwrap().data().unwrap();
            match name.ends_with(".image") {
                true => data.chunks(4).map(|c| c[0]).collect::<Vec<u8>>(),
                false => data[0x14..0x18].to_vec(),
            }
        };
        let image = |name: &str| format!("{name}.image");
        assert_eq!(slices(&front, &image(FLAGS)), [0, 1, 2, 0xa0, 0xb0]);
        assert_eq!(slices(&front, FLAGS), 5u32.to_le_bytes());
        assert_eq!(slices(&front, &image(TRACKS)), [0, 1, 2, 0xa1, 0xb1]);
        assert_eq!(slices(&front, TRACKS), 5u32.to_le_bytes());
        assert_eq!(slices(&hub, &image(FLAGS)), [0, 1, 2, 0xa0, 0xb0]);
        assert_eq!(slices(&front, &image(LOGO)), [7]);
        let db = ErpFile::open(&built.plan.data[&ui("databases.erp")]).unwrap();
        let res = db.load(db.erp.find(OVERRIDES).unwrap()).unwrap();
        let pkg = Pkg::parse(&res.fragments[0].data().unwrap()).unwrap();
        let names = |a: &str, b: &str| -> Vec<String> {
            let mut cur = &pkg.root;
            for p in ["overrides", "Contents", a, "Contents", b, "Contents"] {
                cur = cur.as_obj().unwrap().get(p).unwrap();
            }
            cur.as_arr()
                .unwrap()
                .items
                .iter()
                .map(|n| {
                    let name = n.as_obj().unwrap().get("Name").unwrap();
                    name.as_val().unwrap().as_str().unwrap().to_owned()
                })
                .collect()
        };
        let indexed = ["bahrain", "blank", "melbourne", "ring_a", "ring_b"];
        assert_eq!(names("tracks", "flags_circle"), indexed);
        assert_eq!(names("fe_world", "tracks"), indexed);
        let pair = built
            .report
            .pairs
            .iter()
            .find(|p| p.loser == "t.pack")
            .unwrap_or_else(|| panic!("{:#?}", built.report.pairs));
        assert_eq!((pair.winner.as_str(), pair.resources), ("mod.ring_b", 6));
        assert!(pair.items.contains(&format!("{front}:{}", image(FLAGS))));
        assert_eq!(
            crate::conflicts::last(&store, "default").unwrap().pairs,
            built.report.pairs
        );
        std::fs::remove_dir_all(&root).ok();
    }

    fn archive(names: &[&str], byte: u8) -> Vec<u8> {
        archive_of("UILayout", names, byte)
    }

    fn archive_of(kind: &str, names: &[&str], byte: u8) -> Vec<u8> {
        use swofty_formats::erp::{Erp, Fragment, HEADER_LEN, MAGIC, Resource, STORED};
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        header[4..8].copy_from_slice(&3u32.to_le_bytes());
        Erp {
            header,
            version: 3,
            resources: names
                .iter()
                .map(|n| Resource {
                    name: (*n).into(),
                    kind: swofty_formats::erp::kind(kind).unwrap(),
                    unk: vec![0; 4],
                    fragments: vec![Fragment {
                        name: *b"temp",
                        offset: 0,
                        size: 1,
                        flags: 0x10,
                        compression: STORED,
                        packed: 1,
                        raw: vec![byte],
                    }],
                    hash: [0; 16],
                })
                .collect(),
        }
        .to_bytes()
    }

    #[test]
    fn skips_archive_changes_to_files_the_game_does_not_have() {
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-absent-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::create_dir_all(dir.join("2025_asset_groups")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb::default().to_bytes()).unwrap();
        let erp = "2025_asset_groups/t.erp";
        let uri = "eaid://t/a.idf";
        std::fs::write(dir.join(erp), archive(&[uri], 0)).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("swofty.json"),
            r#"{ "schema_version": 1, "id": "t.cams", "name": "Cams", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" }, "operations": { "assets": ["ops.json"] } }"#,
        )
        .unwrap();
        let put = |erp: &str| {
            format!(r#"{{"op":"erp.put-resource","erp":"{erp}","resource":"{uri}","source":"a.erp"}}"#)
        };
        std::fs::write(
            src.join("ops.json"),
            format!(
                r#"{{ "schema_version": 1, "ops": [{}, {}] }}"#,
                put(erp),
                put("2025_asset_groups/game_package/t.erp")
            ),
        )
        .unwrap();
        std::fs::write(src.join("a.erp"), archive(&[uri], 7)).unwrap();
        store.install_addon(&src).unwrap();
        store
            .update_profile("default", |p| p.set_enabled("t.cams", true))
            .unwrap();
        let built = build(&store, "default", &game).unwrap();
        assert!(
            built.warnings.contains(
                &"Cams: archive changes skipped; 2025_asset_groups/game_package/t.erp is not a file in F1 25. \
                  Turn Cams off if it doesn't work, and tell its author"
                    .to_owned()
            ),
            "{:?}",
            built.warnings
        );
        assert!(built.plan.data.contains_key(erp), "{:?}", built.plan.data);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn files_in_an_asset_group_folder_at_the_game_root_are_flagged() {
        let root = std::env::temp_dir().join(format!("swofty-stray-groups-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let groups = root.join("2025_asset_groups");
        std::fs::create_dir_all(groups.join("f1_2026_vehicle_package")).unwrap();
        std::fs::create_dir_all(groups.join("character_package")).unwrap();
        std::fs::create_dir_all(root.join("swofty")).unwrap();
        let put = |addon: &str, target: &str| Op {
            op: serde_json::from_str(&format!(
                r#"{{"op":"file.put","source":"f","target":"{target}"}}"#
            ))
            .unwrap(),
            origin: Origin {
                addon: addon.to_owned(),
                ..serde_json::from_str(r#"{"addon":"x"}"#).unwrap()
            },
        };
        let files = [
            put("m", "f1_2026_vehicle_package/teams/ferrari/wep/ferrari.erp"),
            put("m", "f1_2026_vehicle_package/teams/ferrari/wep/b.erp"),
            put("m", "character_package/a.erp"),
            put("m", "2025_asset_groups/ui_package/ok.erp"),
            put("m", "swofty/own.bin"),
            put("m", "unknown_folder/x.bin"),
            put("m", "top_level.bin"),
        ];
        assert_eq!(
            stray_asset_groups(&root, &files),
            [
                ("m".to_owned(), "character_package".to_owned(), 1),
                ("m".to_owned(), "f1_2026_vehicle_package".to_owned(), 2),
            ]
        );
        assert_eq!(
            stray_asset_group_line("Mod", "character_package", 1),
            "Mod: 1 file(s) under character_package/ land at the game root, where the game never reads them; the folder belongs under 2025_asset_groups/ - re-import it with that path"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    fn missing_resource_game(tag: &str) -> (PathBuf, Game, Store, String, String) {
        let root = std::env::temp_dir().join(format!("swofty-build-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::create_dir_all(dir.join("2025_asset_groups")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb::default().to_bytes()).unwrap();
        let provided = "2025_asset_groups/p.erp".to_owned();
        let stock = "2025_asset_groups/s.erp".to_owned();
        std::fs::write(dir.join(&stock), archive(&["eaid://s/a.idf"], 0)).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        (root, game, store, provided, stock)
    }

    fn install_ops(store: &Store, root: &Path, id: &str, name: &str, ops: &str, files: &[(&str, Vec<u8>)]) {
        let src = root.join("src").join(id);
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("swofty.json"),
            format!(
                r#"{{ "schema_version": 1, "id": "{id}", "name": "{name}", "version": "1.0.0", "author": "t",
                     "compat": {{ "game": "f1_25" }}, "operations": {{ "assets": ["ops.json"] }} }}"#
            ),
        )
        .unwrap();
        std::fs::write(
            src.join("ops.json"),
            format!(r#"{{ "schema_version": 1, "ops": [{ops}] }}"#),
        )
        .unwrap();
        for (file, bytes) in files {
            std::fs::write(src.join(file), bytes).unwrap();
        }
        store.install_addon(&src).unwrap();
        store
            .update_profile("default", |p| p.set_enabled(id, true))
            .unwrap();
    }

    #[test]
    fn an_edit_of_a_resource_another_addons_archive_lacks_is_skipped() {
        if loader_dir().is_some() {
            return;
        }
        let (root, game, store, provided, _) = missing_resource_game("missing-resource");
        let have = "eaid://p/have.idf";
        let lack = "eaid://p/lack.idf";
        install_ops(
            &store,
            &root,
            "t.base",
            "Base",
            &format!(r#"{{"op":"file.put","source":"p.erp","target":"{provided}"}}"#),
            &[("p.erp", archive_of("Material", &[have], 1))],
        );
        install_ops(
            &store,
            &root,
            "t.edit",
            "Edit",
            &format!(
                r#"{{"op":"erp.replace-resource","erp":"{provided}","resource":"{lack}","fragment":0,"source":"x.bin"}},
                   {{"op":"erp.replace-resource","erp":"{provided}","resource":"{have}","fragment":0,"source":"x.bin"}}"#
            ),
            &[("x.bin", vec![9; 4])],
        );
        let built = build(&store, "default", &game).unwrap();
        let warning = format!(
            "Edit: change skipped; {lack} is not in {provided}, which Base provides. Turn Edit off if it doesn't work, and tell its author"
        );
        assert!(built.warnings.contains(&warning), "{:?}", built.warnings);
        let file = swofty_formats::erp::ErpFile::open(&built.plan.added[&provided]).unwrap();
        let res = file.erp.find(have).unwrap();
        assert_eq!(file.load(res).unwrap().fragments[0].data().unwrap(), vec![9; 4]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_edit_of_a_resource_a_stock_archive_lacks_still_fails() {
        if loader_dir().is_some() {
            return;
        }
        let (root, game, store, provided, stock) = missing_resource_game("missing-resource-stock");
        install_ops(
            &store,
            &root,
            "t.base",
            "Base",
            &format!(r#"{{"op":"file.put","source":"p.erp","target":"{provided}"}}"#),
            &[("p.erp", archive(&["eaid://p/have.idf"], 1))],
        );
        install_ops(
            &store,
            &root,
            "t.edit",
            "Edit",
            &format!(
                r#"{{"op":"erp.replace-resource","erp":"{stock}","resource":"eaid://s/lack.idf","fragment":0,"source":"x.bin"}}"#
            ),
            &[("x.bin", vec![9; 4])],
        );
        let err = build(&store, "default", &game).unwrap_err().to_string();
        assert!(
            err.contains(&format!("applying t.edit: {stock}: eaid://s/lack.idf: not found")),
            "{err}"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_missing_loader_is_named_instead_of_a_track_limit() {
        assert_eq!(
            loader_missing(Some(Path::new("/apps/swofty"))),
            "Swofty Launcher's game files (version.dll, swofty_core.dll) are missing from /apps/swofty; your antivirus may have removed them. Restore them or run the installer again"
        );
        assert!(loader_missing(None).contains("missing from the launcher's folder;"));
    }

    #[test]
    fn the_build_log_names_the_exe_folder_and_the_loader() {
        assert_eq!(
            loader_line(Some(Path::new("/apps/swofty")), None),
            "loader: none; swofty.exe in /apps/swofty"
        );
        assert_eq!(
            loader_line(Some(Path::new("/apps/swofty")), Some(Path::new("/apps/loader"))),
            "loader: /apps/loader; swofty.exe in /apps/swofty"
        );
    }

    #[test]
    fn missing_archive_warnings_count_the_files() {
        let rels: BTreeSet<String> = ["b/x.erp", "a/x.erp"].map(str::to_owned).into();
        assert_eq!(
            missing_archives("Cams", &rels),
            "Cams: archive changes skipped; 2 of the files it changes are not in F1 25, such as a/x.erp. \
             Turn Cams off if it doesn't work, and tell its author"
        );
    }

    #[test]
    fn records_and_reports_the_conflicts_between_addons() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-conflicts-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::create_dir_all(dir.join("2025_asset_groups")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let records = vec![
            Record::new("entrants.teams._team_instancecount_", 0, "1"),
            Record::new("entrants.teams.team_0._metadata_", 4, "mclaren_26"),
            Record::new("entrants.teams.team_0.stat_id", 0, "100"),
        ];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let erp = "2025_asset_groups/t.erp";
        let uri = "eaid://t/a.idf";
        std::fs::write(dir.join(erp), archive(&[uri, "eaid://t/b.idf"], 0)).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let stat = format!(
            "0x{:016x}",
            swofty_formats::hash::fnv1a64("entrants.teams.team_0.stat_id")
        );
        for (id, name, erp_op) in [
            (
                "t.first",
                "First",
                format!(r#"{{"op":"erp.put-resource","erp":"{erp}","resource":"{uri}","source":"a.erp"}}"#),
            ),
            (
                "t.second",
                "Second",
                format!(r#"{{"op":"erp.overlay","erps":["{erp}"],"source":"a.erp"}}"#),
            ),
        ] {
            let src = root.join("src").join(id);
            std::fs::create_dir_all(&src).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{name}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }},
                         "operations": {{ "assets": ["ops.json"], "files": [{{ "source": "a.dds", "target": "x/a.dds" }}] }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(
                src.join("ops.json"),
                format!(
                    r#"{{ "schema_version": 1, "ops": [{{"op":"db.put-record","key":"{stat}","type":0,"value":"3135"}}, {erp_op}] }}"#
                ),
            )
            .unwrap();
            std::fs::write(src.join("a.erp"), archive(&[uri], 7)).unwrap();
            std::fs::write(src.join("a.dds"), name).unwrap();
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled(id, true))
                .unwrap();
        }
        let listing = |d: &Path| {
            let mut all: Vec<_> = loader_files(d).unwrap().into_iter().collect();
            all.sort();
            all
        };
        let before = listing(&dir);
        let held = lock(&store, "default").unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::scope(|s| {
            s.spawn(|| {
                tx.send(crate::conflicts::check(&store, "default", &game))
                    .unwrap()
            });
            let waited = rx.recv_timeout(std::time::Duration::from_millis(300));
            assert!(waited.is_err(), "the check ran while another build held the lock");
            drop(held);
        });
        let checked = rx.recv().unwrap().unwrap();
        assert_eq!(listing(&dir), before);
        let built = build(&store, "default", &game).unwrap();
        assert!(
            built
                .lines
                .contains(&"First: 1 database value, 1 archive resource, 1 file".to_owned()),
            "{:?}",
            built.lines
        );
        assert!(
            built.warnings.contains(
                &"Second overrides First on 1 database value, 1 archive resource and 1 file".to_owned()
            ),
            "{:?}",
            built.warnings
        );
        assert!(
            !built.warnings.iter().any(|w| w.starts_with("conflict:")),
            "{:?}",
            built.warnings
        );
        let report = crate::conflicts::last(&store, "default").unwrap();
        assert!(report.is_current(&store.profile("default").unwrap()));
        assert_eq!(report.pairs, checked.pairs);
        assert_eq!(report.pairs, built.report.pairs);
        let pair = &report.pairs[0];
        assert_eq!(
            (
                pair.winner.as_str(),
                pair.loser.as_str(),
                pair.database,
                pair.resources,
                pair.files
            ),
            ("t.second", "t.first", 1, 1, 1)
        );
        assert_eq!(
            pair.items,
            [
                format!("{erp}:{uri}"),
                "x/a.dds".to_owned(),
                "entrants.teams.team[_metadata_=mclaren_26].stat_id".to_owned()
            ]
        );
        std::fs::remove_file(crate::conflicts::path(&store, "default")).unwrap();
        let again = build(&store, "default", &game).unwrap();
        assert!(
            again
                .lines
                .iter()
                .any(|l| l.starts_with("build: nothing changed"))
        );
        assert_eq!(
            crate::conflicts::last(&store, "default").unwrap().pairs,
            report.pairs
        );
        assert!(crate::conflicts::last(&store, "vanilla").is_none());
        assert!(
            crate::conflicts::check(&store, "vanilla", &game)
                .unwrap()
                .pairs
                .is_empty()
        );
        std::fs::remove_dir_all(&root).ok();
    }

    // One addon puts `t.erp` whole and another replaces a resource in it with a
    // bigger one; `edit_first` loads the edit before the put.
    fn build_put_and_edit(name: &str, edit_first: bool) -> (Built, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("swofty-build-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::create_dir_all(dir.join("2025_asset_groups")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let erp = "2025_asset_groups/t.erp";
        std::fs::write(
            dir.join(erp),
            archive_of("Data", &["eaid://t/a.idf", "eaid://t/b.idf"], 0),
        )
        .unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let pack = (
            "t.pack",
            format!(r#""files": [{{ "source": "whole.erp", "target": "{erp}" }}]"#),
        );
        let edit = ("t.edit", r#""assets": ["ops.json"]"#.to_owned());
        let order = match edit_first {
            true => [edit, pack],
            false => [pack, edit],
        };
        for (id, operations) in order {
            let src = root.join("src").join(id);
            std::fs::create_dir_all(&src).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{id}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}, "operations": {{ {operations} }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(
                src.join("ops.json"),
                format!(
                    r#"{{ "schema_version": 1, "ops": [{{"op":"erp.replace-resource","erp":"{erp}","resource":"eaid://t/a.idf","source":"a.bin"}}] }}"#
                ),
            )
            .unwrap();
            std::fs::write(src.join("a.bin"), [9, 9]).unwrap();
            std::fs::write(
                src.join("whole.erp"),
                archive_of("Data", &["eaid://t/a.idf", "eaid://t/b.idf", "eaid://t/c.idf"], 5),
            )
            .unwrap();
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled(id, true))
                .unwrap();
        }
        let built = build(&store, "default", &game).unwrap();
        let whole = store.addons_dir().join("t.pack").join("whole.erp");
        (built, root, whole)
    }

    #[test]
    fn a_resource_edit_that_leaves_stale_tsmd_stats_gets_them_rebuilt() {
        use swofty_formats::bxml::{Document, Element, Kind};
        use swofty_formats::erp::{Erp, Fragment, HEADER_LEN, MAGIC, Resource, STORED};
        let _pin = crate::builtin::LoaderPin::none();
        let root = std::env::temp_dir().join(format!("swofty-build-tsmd-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        let erp = "2025_asset_groups/environment_package/tracks/bahrain/wep/bahrain_common.erp";
        std::fs::create_dir_all(dir.join(erp).parent().unwrap()).unwrap();
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let spline = |gate: &str| Document {
            root: Element {
                kind: Kind::Element,
                pad: 0,
                name: b"track".to_vec(),
                attrs: vec![(b"name".to_vec(), gate.as_bytes().to_vec())],
                children: Vec::new(),
                end_pad: 0,
            },
            tail: Vec::new(),
        };
        let fragment = |raw: Vec<u8>| Fragment {
            name: *b"temp",
            offset: 0,
            size: raw.len() as u64,
            flags: 0x10,
            compression: STORED,
            packed: raw.len() as u64,
            raw,
        };
        let res = "eaid://environment_package/tracks/bahrain/ai/bahrain_ai_track.aispline";
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        header[4..8].copy_from_slice(&3u32.to_le_bytes());
        let stock = spline("default");
        let bytes = Erp {
            header,
            version: 3,
            resources: vec![Resource {
                name: res.into(),
                kind: swofty_formats::erp::kind("AISplineData").unwrap(),
                unk: vec![0; 4],
                fragments: vec![fragment(stock.to_bytes()), fragment(stock.string_stats(1))],
                hash: [0; 16],
            }],
        }
        .to_bytes();
        std::fs::write(dir.join(erp), bytes).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("swofty.json"),
            r#"{ "schema_version": 1, "id": "t.ai", "name": "More Aggressive AI", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" }, "operations": { "erp": ["ops.json"] } }"#,
        )
        .unwrap();
        std::fs::write(
            src.join("ops.json"),
            format!(
                r#"{{ "schema_version": 1, "ops": [{{"op":"replace-resource","erp":"{erp}","resource":"{res}","fragment":0,"source":"line.aispline"}}] }}"#
            ),
        )
        .unwrap();
        let edited = spline("a racing line edited by hand");
        std::fs::write(src.join("line.aispline"), edited.to_bytes()).unwrap();
        store.install_addon(&src).unwrap();
        store
            .update_profile("default", |p| p.set_enabled("t.ai", true))
            .unwrap();
        let built = build(&store, "default", &game).unwrap();
        assert!(
            built.lines.iter().any(|l| l
                == "erp: fixed the size headers of 1 AI racing line(s) in More Aggressive AI so F1 25 reads them safely"),
            "{:?}",
            built.lines
        );
        assert!(
            !built
                .warnings
                .iter()
                .any(|w| w.contains("aispline") || w.contains("size header"))
        );
        if let Some(path) = built.plan.data.get(erp) {
            let f = swofty_formats::erp::ErpFile::open(path).unwrap();
            let r = f.load(f.erp.find(res).unwrap()).unwrap();
            assert_eq!(r.fragments[1].data().unwrap(), edited.string_stats(1));
        }

        let mut cut = edited.to_bytes();
        cut.truncate(cut.len() - 3);
        std::fs::write(src.join("line.aispline"), cut).unwrap();
        store.install_addon(&src).unwrap();
        let Err(e) = build(&store, "default", &game) else {
            panic!("the build should stop");
        };
        assert_eq!(
            e.to_string(),
            "More Aggressive AI would crash F1 25: its bahrain_ai_track.aispline doesn't match the size header the \
             game reads with it, and the launcher can't read the file to fix the header. Turn More Aggressive AI \
             off in this profile."
        );
        std::fs::remove_dir_all(&root).ok();
    }

    fn fragments(path: &Path) -> Vec<(String, Vec<u8>)> {
        let f = swofty_formats::erp::ErpFile::open(path).unwrap();
        f.erp
            .resources
            .iter()
            .map(|r| (r.name.clone(), f.load(r).unwrap().fragments[0].raw.clone()))
            .collect()
    }

    #[test]
    fn a_later_resource_edit_applies_to_an_archive_put_whole() {
        if loader_dir().is_some() {
            return;
        }
        let (built, root, _) = build_put_and_edit("put-then-edit", false);
        let erp = "2025_asset_groups/t.erp";
        assert_eq!(
            fragments(&built.plan.data[erp]),
            [
                ("eaid://t/a.idf".to_owned(), vec![9, 9]),
                ("eaid://t/b.idf".to_owned(), vec![5]),
                ("eaid://t/c.idf".to_owned(), vec![5]),
            ]
        );
        assert!(!built.plan.added.contains_key(erp));
        let pair = &built.report.pairs[0];
        assert_eq!(
            (pair.winner.as_str(), pair.loser.as_str(), pair.resources),
            ("t.edit", "t.pack", 1)
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_archive_put_whole_replaces_earlier_resource_edits() {
        if loader_dir().is_some() {
            return;
        }
        let (built, root, whole) = build_put_and_edit("edit-then-put", true);
        assert_eq!(built.plan.data["2025_asset_groups/t.erp"], whole);
        let pair = &built.report.pairs[0];
        assert_eq!(
            (pair.winner.as_str(), pair.loser.as_str(), pair.resources),
            ("t.pack", "t.edit", 1)
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_addon_turns_on_the_experimental_features_it_lists() {
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-features-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(
            src.join("swofty.json"),
            r#"{ "schema_version": 1, "id": "t.hud", "name": "Race Control", "version": "1.0.0", "author": "t",
                 "compat": { "game": "f1_25" }, "experimental_features": ["overlay"] }"#,
        )
        .unwrap();
        store.install_addon(&src).unwrap();
        let assets = |built: &Built| {
            built
                .plan
                .loader
                .get("swofty/assets.json")
                .map(|p| serde_json::from_slice::<serde_json::Value>(&std::fs::read(p).unwrap()).unwrap())
        };
        let off = build(&store, "default", &game).unwrap();
        assert_eq!(assets(&off), None);
        store
            .update_profile("default", |p| p.set_enabled("t.hud", true))
            .unwrap();
        let on = build(&store, "default", &game).unwrap();
        assert_eq!(assets(&on).unwrap()["features"], serde_json::json!(["overlay"]));
        assert!(
            on.lines
                .contains(&"features: the experimental overlay feature is on for Race Control".to_owned()),
            "{:?}",
            on.lines
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn telemetry_is_on_only_for_an_addon_that_lists_it() {
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-telemetry-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        for (id, extra) in [
            ("t.timing", r#", "experimental_features": ["telemetry"]"#),
            ("t.plain", ""),
        ] {
            let src = root.join(id);
            std::fs::create_dir_all(&src).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{id}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}{extra} }}"#
                ),
            )
            .unwrap();
            store.install_addon(&src).unwrap();
        }
        let assets = |built: &Built| {
            built
                .plan
                .loader
                .get("swofty/assets.json")
                .map(|p| serde_json::from_slice::<serde_json::Value>(&std::fs::read(p).unwrap()).unwrap())
        };
        store
            .update_profile("default", |p| p.set_enabled("t.plain", true))
            .unwrap();
        assert_eq!(assets(&build(&store, "default", &game).unwrap()), None);
        store
            .update_profile("default", |p| p.set_enabled("t.timing", true))
            .unwrap();
        let on = build(&store, "default", &game).unwrap();
        assert_eq!(assets(&on).unwrap()["features"], serde_json::json!(["telemetry"]));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_enabled_addon_for_a_newer_launcher_is_left_out_with_a_warning() {
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-future-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let future = store.addons_dir().join("w3.hud");
        std::fs::create_dir_all(&future).unwrap();
        std::fs::write(
            future.join(crate::manifest::MANIFEST_FILE),
            r#"{ "schema_version": 1, "id": "w3.hud", "name": "Race HUD", "version": "0.1.0", "author": "t",
                 "compat": { "game": "f1_25" }, "hud": { "widgets": [] } }"#,
        )
        .unwrap();
        store
            .update_profile("default", |p| p.set_enabled("w3.hud", true))
            .unwrap();
        let built = build(&store, "default", &game).unwrap();
        assert!(!built.order.iter().any(|id| id == "w3.hud"), "{:?}", built.order);
        assert!(
            built.warnings.contains(
                &"Race HUD is left out: it needs a newer Swofty Launcher (unknown field `widgets`)"
                    .to_owned()
            ),
            "{:?}",
            built.warnings
        );
        assert!(
            store
                .profile("default")
                .unwrap()
                .enabled()
                .any(|id| id == "w3.hud")
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_shader_pack_grades_with_the_look_its_settings_pick() {
        use swofty_formats::cube::Cube;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-shaders-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let pack = |id: &str, name: &str, warm: &str| {
            let src = root.join(id);
            std::fs::create_dir_all(src.join("looks")).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{name}", "version": "1.0.0", "author": "t",
                     "category": "shaders", "experimental_features": ["shaders"], "compat": {{ "game": "f1_25" }},
                     "options": [
                       {{ "id": "look", "type": "choice", "default": "mono", "values": ["mono", "warm"], "binds": "db" }},
                       {{ "id": "strength", "type": "slider", "min": 0, "max": 100, "step": 10, "default": 50,
                         "binds": "db" }}
                     ],
                     "shader": {{ "lut": "looks/{{{{look}}}}.cube", "strength": "{{{{strength}}}}" }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(src.join("looks/mono.cube"), Cube::identity(2).to_text()).unwrap();
            std::fs::write(src.join("looks/warm.cube"), warm).unwrap();
            store.install_addon(&src).unwrap()
        };
        let looks = pack("t.looks", "Looks", &Cube::identity(3).to_text());
        store
            .update_profile("default", |p| p.set_enabled("t.looks", true))
            .unwrap();
        let built = build(&store, "default", &game).unwrap();
        let assets: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&built.plan.loader["swofty/assets.json"]).unwrap())
                .unwrap();
        assert_eq!(assets["features"], serde_json::json!(["shaders"]));
        assert_eq!(
            assets["shader"],
            serde_json::json!({ "lut": "shaders/look.lut", "strength": 0.5, "addon": "Looks" })
        );
        let packed = std::fs::read(&built.plan.loader["swofty/shaders/look.lut"]).unwrap();
        assert_eq!(Cube::unpack(&packed).unwrap().table, Cube::identity(2).table);
        let line = |file: &str, size: usize, strength: u32| {
            format!(
                "shaders: Looks grades the picture with looks/{file} ({size}x{size}x{size}) at {strength}% strength"
            )
        };
        assert!(
            built.lines.contains(&line("mono.cube", 2, 50)),
            "{:?}",
            built.lines
        );

        store
            .update_profile("default", |p| {
                p.set_option(&looks, "look", "warm".into())?;
                p.set_option(&looks, "strength", 100.into()).map(drop)
            })
            .unwrap();
        let built = build(&store, "default", &game).unwrap();
        assert!(
            built.lines.contains(&line("warm.cube", 3, 100)),
            "{:?}",
            built.lines
        );

        let later = pack("t.later", "Later Looks", "LUT_3D_SIZE 2\n0 0 0\n");
        store
            .update_profile("default", |p| {
                p.set_enabled("t.later", true)?;
                p.set_option(&later, "look", "warm".into()).map(drop)
            })
            .unwrap();
        let err = build(&store, "default", &game).unwrap_err().to_string();
        assert!(
            err.contains("Later Looks: looks/warm.cube: 1 colour line(s), but LUT_3D_SIZE 2 needs 8"),
            "{err}"
        );
        store
            .update_profile("default", |p| {
                p.set_option(&later, "look", "mono".into()).map(drop)
            })
            .unwrap();
        let built = build(&store, "default", &game).unwrap();
        let skipped = "Looks: its look is not applied; one shader look applies at a time, and Later Looks loads after it";
        assert!(
            built.warnings.contains(&skipped.to_owned()),
            "{:?}",
            built.warnings
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_template_shader_pack_builds() {
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-template-look-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/shader-pack");
        let m = store.install_addon(&template).unwrap();
        store
            .update_profile("default", |p| p.set_enabled(&m.id, true))
            .unwrap();
        for look in ["monochrome", "overcast", "noir"] {
            store
                .update_profile("default", |p| p.set_option(&m, "look", look.into()).map(drop))
                .unwrap();
            let built = build(&store, "default", &game).unwrap();
            let line = format!(
                "shaders: Monochrome Looks grades the picture with looks/{look}.cube (17x17x17) at 100% strength"
            );
            assert!(built.lines.contains(&line), "{:?}", built.lines);
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_template_postfx_pass_compiles_into_the_build() {
        if loader_dir().is_some() || crate::postfx::dxc().is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-postfx-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/postfx-pass");
        let m = store.install_addon(&template).unwrap();
        store
            .update_profile("default", |p| p.set_enabled(&m.id, true))
            .unwrap();
        store
            .update_profile("default", |p| {
                p.set_option(&m, "saturation", serde_json::json!(40)).map(drop)
            })
            .unwrap();
        let built = build(&store, "default", &game).unwrap();
        let assets: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&built.plan.loader["swofty/assets.json"]).unwrap())
                .unwrap();
        assert_eq!(assets["features"], serde_json::json!(["postfx"]));
        let ns = "example.vignette-pass";
        let pass = |i: usize,
                    name: &str,
                    options: serde_json::Value,
                    inputs: serde_json::Value,
                    output: String| {
            serde_json::json!({ "shader": format!("postfx/{i:02}.dxil"), "name": name, "addon": "Vignette Pass",
                                "addon_id": ns, "options": options, "budget_ms": 1.5, "live": [],
                                "inputs": inputs, "output": output, "threads": [8, 8], "raw": false })
        };
        assert_eq!(
            assets["postfx"],
            serde_json::json!([
                pass(
                    0,
                    "vignette",
                    serde_json::json!([60.0, 40.0]),
                    serde_json::json!([]),
                    String::new()
                ),
                pass(
                    1,
                    "glow-down",
                    serde_json::json!([]),
                    serde_json::json!(["original"]),
                    format!("{ns}/half")
                ),
                pass(
                    2,
                    "glow-up",
                    serde_json::json!([30.0]),
                    serde_json::json!([format!("{ns}/half")]),
                    String::new()
                ),
            ])
        );
        assert_eq!(
            assets["postfx_textures"],
            serde_json::json!([{ "name": format!("{ns}/half"), "scale": 0.5, "format": "rgba16f", "persistent": false }])
        );
        assert!(assets.get("postfx_toggles").is_none());
        for i in 0..3 {
            assert!(
                std::fs::read(&built.plan.loader[&format!("swofty/postfx/{i:02}.dxil")])
                    .unwrap()
                    .starts_with(b"DXBC")
            );
        }
        let dxil = std::fs::read(&built.plan.loader["swofty/postfx/00.dxil"]).unwrap();
        assert!(dxil.starts_with(b"DXBC"));
        assert!(
            built
                .lines
                .iter()
                .any(|l| l
                    .starts_with("postfx: Vignette Pass's pass vignette runs as pass 1 before the HUD")),
            "{:?}",
            built.lines
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn shader_effects_and_the_active_config_build_into_the_loader_plan() {
        if loader_dir().is_some() || crate::postfx::dxc().is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-shaders-effects-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        crate::shaders::tests::addon(
            &store,
            "t.look",
            "",
            &[(
                "forced.swfx",
                "name = \"Forced look\"\n[spark]\nintensity = 3\n\n[vignette]\n",
            )],
            true,
        );
        let library = crate::shaders::catalog(&store).effects.len();
        assert!(library > 1);
        let built = build(&store, "default", &game).unwrap();
        let read = |rel: &str| std::fs::read(&built.plan.loader[&format!("swofty/{rel}")]).unwrap();
        let effects: serde_json::Value = serde_json::from_slice(&read("shaders/effects.json")).unwrap();
        let list = effects["effects"].as_array().unwrap();
        assert_eq!(list.len(), library, "{:?}", built.warnings);
        let assets: serde_json::Value = serde_json::from_slice(&read("assets.json")).unwrap();
        assert!(
            assets["features"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f == "postfx"),
            "a running config turns the core's postfx feature on"
        );
        let spark = list.iter().find(|e| e["id"] == "spark").unwrap();
        assert_eq!(
            (spark["addon"].as_str(), spark["slots"].as_u64()),
            (Some("t.look"), Some(1))
        );
        assert_eq!(spark["passes"][0]["dxil"], "shaders/fx/spark-00.dxil");
        assert!(read("shaders/fx/spark-00.dxil").starts_with(b"DXBC"));
        let active: serde_json::Value = serde_json::from_slice(&read("shaders/active.json")).unwrap();
        assert_eq!(active["name"], "Forced look");
        assert_eq!(active["forced_by"], "t.look name");
        assert_eq!(active["source"], "addon:t.look/forced");
        assert_eq!(active["toggle_key"], "B");
        assert_eq!(active["instances"][0]["effect"], "spark");
        assert_eq!(active["instances"][0]["params"], serde_json::json!([3.0]));
        assert_eq!(active["instances"][1]["effect"], "vignette");
        assert!(
            built
                .lines
                .iter()
                .any(|l| l.starts_with("shaders: config \"Forced look\" runs 2 effect(s) (")),
            "{:?}",
            built.lines
        );
        assert!(
            !built.warnings.iter().any(|w| w.starts_with("shaders")),
            "{:?}",
            built.warnings
        );

        let again = build(&store, "default", &game).unwrap();
        assert_eq!(
            again.buildhash, built.buildhash,
            "the live serial is not part of the hash"
        );
        crate::shaders::set_toggle_key(&store, "F9").unwrap();
        assert_ne!(
            build(&store, "default", &game).unwrap().buildhash,
            built.buildhash
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_config_with_errors_builds_with_a_warning_and_shaders_off() {
        if loader_dir().is_some() || crate::postfx::dxc().is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-shaders-bad-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let r = crate::shaders::create(&store, "Broken", Some("[no_such_effect]\n")).unwrap();
        let mut p = store.profile("default").unwrap();
        crate::shaders::set_active(&store, &mut p, Some(r)).unwrap();
        let built = build(&store, "default", &game).unwrap();
        assert!(
            !built.plan.loader.keys().any(|k| k.starts_with("swofty/shaders/")),
            "no shader files when nothing runs"
        );
        assert!(
            built
                .warnings
                .iter()
                .any(|w| w.starts_with("shaders: Broken line 2") && w.ends_with("shaders are off")),
            "{:?}",
            built.warnings
        );
        assert!(built.lines.iter().any(|l| l == "shaders: off"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_vanilla_profile_builds_no_shader_files() {
        let store = Store::new(
            std::env::temp_dir().join(format!("swofty-build-shaders-vanilla-{}", std::process::id())),
        );
        let files = crate::shaders::build_files(&store, &crate::profile::Profile::vanilla());
        assert!(files.files.is_empty() && files.line.is_none() && files.warnings.is_empty());
    }

    #[test]
    fn a_v2_postfx_addon_builds_live_slots_textures_and_toggles() {
        if loader_dir().is_some() || crate::postfx::dxc().is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-postfx2-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let src = root.join("src");
        std::fs::create_dir_all(src.join("fx")).unwrap();
        std::fs::create_dir_all(src.join("tex")).unwrap();
        let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/postfx-pass/fx");
        std::fs::copy(template.join("glow.hlsl"), src.join("fx/glow.hlsl")).unwrap();
        std::fs::write(
            src.join("fx/raw.hlsl"),
            "#include \"swofty/postfx.hlsli\"\n\
             [numthreads(16, 16, 1)] void fill(uint3 id : SV_DispatchThreadID)\n\
             { if (all(id.xy < swofty_size)) swofty_output[id.xy] = option(0).xxxx; }\n",
        )
        .unwrap();
        let mut png = Vec::new();
        {
            let mut e = png::Encoder::new(&mut png, 1, 2);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header()
                .unwrap()
                .write_image_data(&[1, 2, 3, 4, 5, 6, 7, 8])
                .unwrap();
        }
        std::fs::write(src.join("tex/dirt.png"), &png).unwrap();
        std::fs::write(
            src.join(crate::manifest::MANIFEST_FILE),
            r#"{ "schema_version": 1, "id": "x.fx2", "name": "Fx Two", "version": "1.0.0", "author": "a",
                "experimental_features": ["postfx"], "compat": { "game": "f1_25" },
                "options": [
                    { "id": "amount", "type": "slider", "min": 0, "max": 100, "step": 1, "default": 20, "binds": "native" },
                    { "id": "fixed", "type": "slider", "min": 0, "max": 100, "step": 1, "default": 7, "binds": "db" },
                    { "id": "live_on", "type": "toggle", "default": true, "binds": "native" },
                    { "id": "build_on", "type": "toggle", "default": false, "binds": "db" },
                    { "id": "key", "type": "choice", "default": "F9", "binds": "native", "values": ["F9", "None"] }
                ],
                "postfx_toggle": "{{key}}",
                "postfx_textures": [
                    { "name": "half", "scale": 0.5 },
                    { "name": "dirt", "file": "tex/dirt.png" }
                ],
                "postfx": [
                    { "shader": "fx/glow.hlsl", "entry": "down", "name": "d", "inputs": ["original", "dirt"], "output": "half" },
                    { "shader": "fx/glow.hlsl", "entry": "up", "name": "u", "options": ["{{amount}}", "{{fixed}}", 3],
                      "inputs": ["half", "previous"], "enabled": "{{live_on}}",
                      "when": { "views": ["cockpit"], "skip": ["paused"] } },
                    { "shader": "fx/glow.hlsl", "entry": "up", "name": "off", "enabled": "{{build_on}}" },
                    { "shader": "fx/raw.hlsl", "raw": true, "entry": "fill", "threads": [16, 16], "name": "r", "options": [1] }
                ] }"#,
        )
        .unwrap();
        let m = store.install_addon(&src).unwrap();
        store
            .update_profile("default", |p| p.set_enabled(&m.id, true))
            .unwrap();
        store
            .update_profile("default", |p| {
                p.set_option(&m, "amount", serde_json::json!(55)).map(drop)
            })
            .unwrap();
        let built = build(&store, "default", &game).unwrap();
        let assets: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&built.plan.loader["swofty/assets.json"]).unwrap())
                .unwrap();
        let ns = "x.fx2";
        let passes = assets["postfx"].as_array().unwrap();
        assert_eq!(
            passes.len(),
            3,
            "the pass enabled by a db toggle that is off is left out"
        );
        assert_eq!(
            passes[0]["inputs"],
            serde_json::json!(["original", format!("{ns}/dirt")])
        );
        assert_eq!(passes[0]["output"], format!("{ns}/half"));
        assert_eq!(passes[1]["options"], serde_json::json!([55.0, 7.0, 3.0]));
        assert_eq!(
            passes[1]["live"],
            serde_json::json!([{ "slot": 0, "option": "amount" }])
        );
        assert_eq!(passes[1]["enabled"], "live_on");
        assert_eq!(
            passes[1]["when"],
            serde_json::json!({ "views": ["cockpit"], "skip": ["paused"] })
        );
        assert_eq!(
            passes[1]["inputs"],
            serde_json::json!([format!("{ns}/half"), "previous"])
        );
        assert!(passes[0].get("enabled").is_none() && passes[0].get("when").is_none());
        assert_eq!(passes[2]["raw"], true);
        assert_eq!(passes[2]["threads"], serde_json::json!([16, 16]));
        assert_eq!(passes[2]["output"], "");
        assert_eq!(
            assets["postfx_textures"],
            serde_json::json!([
                { "name": format!("{ns}/half"), "scale": 0.5, "format": "rgba16f", "persistent": false },
                { "name": format!("{ns}/dirt"), "format": "rgba8", "file": "postfx/tex/00.tex" }
            ])
        );
        assert_eq!(
            assets["postfx_toggles"],
            serde_json::json!([{ "addon_id": ns, "option": "key" }])
        );
        let tex = std::fs::read(&built.plan.loader["swofty/postfx/tex/00.tex"]).unwrap();
        let mut want = b"SWTX".to_vec();
        want.extend_from_slice(&1u32.to_le_bytes());
        want.extend_from_slice(&2u32.to_le_bytes());
        want.extend_from_slice(&0u32.to_le_bytes());
        want.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(tex, want);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_built_addon_installs_and_builds() {
        use crate::builder::AddonBuilder;
        use crate::manifest::Category;
        use crate::ops::{DbOp, ErpOp};
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-builder-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::create_dir_all(dir.join("2025_asset_groups")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let records = vec![Record::new("rules.laps", 0, "5")];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let (erp, uri) = ("2025_asset_groups/t.erp", "eaid://t/a.idf");
        std::fs::write(dir.join(erp), archive(&[uri], 0)).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.erp"), archive(&[uri], 9)).unwrap();
        std::fs::write(src.join("a.dds"), "dds").unwrap();
        let mut b = AddonBuilder::new("t.generated", "Generated", "t");
        b.category = Some(Category::Helmets);
        b.db(DbOp::PutRecord {
            key: "rules.laps".into(),
            tag: 0,
            value: "3132".into(),
        })
        .erp(ErpOp::PutResource {
            erp: erp.into(),
            resource: uri.into(),
            source: Source::addon("erp/a.erp"),
            rename: None,
        })
        .source("erp/a.erp", src.join("a.erp"))
        .text("*", "lng_laps", "Laps")
        .file(src.join("a.dds"), "x/a.dds");
        b.write(&root.join("out")).unwrap();
        let store = Store::new(root.join("data"));
        let m = store.install_addon(&root.join("out")).unwrap();
        assert_eq!(m.category, Some(Category::Helmets));
        store
            .update_profile("default", |p| p.set_enabled("t.generated", true))
            .unwrap();
        let built = build(&store, "default", &game).unwrap();
        let db = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(db.get_text("rules.laps").as_deref(), Some("12"));
        assert!(built.plan.data.contains_key(erp));
        assert_eq!(
            built.plan.added["x/a.dds"],
            store.addons_dir().join("t.generated/files/x/a.dds")
        );
        assert!(
            built.lines.contains(
                &"Generated: 1 database value, 1 archive resource, 1 file, 1 text string".to_owned()
            ),
            "{:?}",
            built.lines
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn places_screens_under_the_loader_ui_dir() {
        let m = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "a.pages", "name": "S", "version": "1.0.0", "author": "a",
                 "compat": { "game": "f1_25" },
                 "operations": { "screens": [{ "name": "swofty_addon_0", "file": "ui/page0.ui" },
                   { "name": "generated", "page": "options", "flow": "flow_options", "anchor": "settings_container" }] } }"#,
        )
        .unwrap();
        let files = screen_files(&[&m], Path::new("/addons")).unwrap();
        assert_eq!(
            files,
            BTreeMap::from([(
                "swofty/ui/swofty_addon_0.ui".to_owned(),
                PathBuf::from("/addons/a.pages/ui/page0.ui")
            )])
        );
        assert!(screen_files(&[&m, &m], Path::new("/addons")).is_err());
    }

    #[test]
    fn leaves_out_a_track_package_without_a_base_track() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-left-out-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let weekend = "schedule.activities.activity_race_weekend_0.track";
        let records = vec![
            Record::new("tracks._Track_instancecount_", 0, "1"),
            Record::new("tracks.Track_0._metadata_", 2, "melbourne"),
            Record::new("tracks.Track_0.laps", 0, "58"),
            Record::new(
                "schedule.activities._activity_race_weekend_instancecount_",
                0,
                "1",
            ),
            Record::new(
                "schedule.activities.activity_race_weekend_0._metadata_",
                2,
                "rw_melbourne",
            ),
            Record::new(weekend, 2, "melbourne"),
            Record::new("entrants.teams._team_instancecount_", 0, "1"),
            Record::new("entrants.teams.team_0._metadata_", 2, "mclaren_26"),
            Record::new("entrants.teams.team_0.stat_id", 0, "100"),
        ];
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let addon = |id: &str, name: &str, ops: &str, track: Option<&str>| {
            let src = root.join("src").join(id);
            std::fs::create_dir_all(&src).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{name}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}, "operations": {{ "assets": ["ops.json"] }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(
                src.join("ops.json"),
                format!(r#"{{ "schema_version": 1, "ops": [{ops}] }}"#),
            )
            .unwrap();
            if let Some(manifest) = track {
                std::fs::create_dir_all(src.join("track")).unwrap();
                std::fs::write(src.join("track/manifest.json"), manifest).unwrap();
            }
            store.install_addon(&src).unwrap();
        };
        addon(
            "mod.portimao",
            "Portimao",
            r#"{"op":"track.add","source":"track"},
               {"op":"db.set","select":"tracks.Track[_metadata_=portimao]","field":"laps","value":"66"}"#,
            Some(r#"{ "track_code": "portimao", "track_name": "Portimao", "erp": "portimao.erp" }"#),
        );
        addon(
            "t.calendar",
            "Calendar",
            r#"{"op":"db.set","select":"schedule.activities.activity_race_weekend[_metadata_=rw_melbourne]","field":"track","value":"portimao"},
               {"op":"db.set","select":"entrants.teams.team[_metadata_=mclaren_26]","field":"stat_id","value":"7"}"#,
            None,
        );
        let left_out = "Portimao (mod.portimao): track Portimao is left out; its manifest.json doesn't say which track it replaces or copies. Import a fixed version, or ask its author for one";
        let restored =
            "race weekend rw_melbourne: races at melbourne again; track portimao is left out of this build";

        store
            .update_profile("default", |p| p.set_enabled("t.calendar", true))
            .unwrap();
        let without = build(&store, "default", &game).unwrap();
        assert!(
            !without.warnings.iter().any(|w| w.contains("left out")),
            "{:?}",
            without.warnings
        );
        let off = "Calendar sets race weekend rw_melbourne to portimao, which is off: turn Portimao on; until then \
                   this build puts back melbourne";
        assert!(
            without.warnings.iter().any(|w| w == off),
            "{:?}",
            without.warnings
        );
        let out = Gvdb::parse(&std::fs::read(&without.plan.data[GVDB]).unwrap()).unwrap();
        assert_eq!(out.get_text(weekend).as_deref(), Some("melbourne"));

        store
            .update_profile("default", |p| p.set_enabled("mod.portimao", true))
            .unwrap();
        for pass in ["built", "reused"] {
            let built = build(&store, "default", &game).unwrap();
            let reused = built.lines.iter().any(|l| l.ends_with("reusing it"));
            assert_eq!(reused, pass == "reused");
            let warnings = &built.warnings;
            assert!(warnings.iter().any(|w| w == left_out), "{pass}: {warnings:?}");
            assert!(warnings.iter().any(|w| w == restored), "{pass}: {warnings:?}");
            assert!(
                warnings
                    .iter()
                    .any(|w| w.starts_with("Portimao: change skipped: ") && w.contains("portimao")),
                "{pass}: {warnings:?}"
            );
            let out = Gvdb::parse(&std::fs::read(&built.plan.data[GVDB]).unwrap()).unwrap();
            assert_eq!(out.get_text(weekend).as_deref(), Some("melbourne"), "{pass}");
            assert_eq!(
                out.get_text("tracks.Track_0.laps").as_deref(),
                Some("58"),
                "{pass}"
            );
            assert_eq!(
                out.get_text("tracks._Track_instancecount_").as_deref(),
                Some("1"),
                "{pass}"
            );
            assert_eq!(
                out.get_text("entrants.teams.team_0.stat_id").as_deref(),
                Some("7"),
                "{pass}"
            );
            let plan: serde_json::Value = crate::read_json(&built.overlay.join(PLAN_FILE)).unwrap();
            assert_eq!(plan["tracks"], serde_json::json!([]), "{pass}");
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn leaves_out_the_tracks_the_game_s_track_lists_have_no_room_for() {
        use swofty_formats::gvdb::Record;
        if loader_dir().is_some() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-build-track-room-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        let stock: Vec<String> = (0..64)
            .map(|i| format!("stock_{i:02}"))
            .chain(["madrid".to_owned()])
            .collect();
        let mut records = vec![
            Record::new("tracks._Track_instancecount_", 0, stock.len().to_string()),
            Record::new("track_data.game_setup._TrackDataSet_instancecount_", 0, "1"),
            Record::new("track_data.game_setup.TrackDataSet_0._metadata_", 2, "f1"),
            Record::new(
                "track_data.game_setup.TrackDataSet_0._TrackData_instancecount_",
                0,
                stock.len().to_string(),
            ),
        ];
        for (i, code) in stock.iter().enumerate() {
            records.push(Record::new(
                &format!("tracks.Track_{i}._metadata_"),
                2,
                code.as_str(),
            ));
            records.push(Record::new(
                &format!("track_data.game_setup.TrackDataSet_0.TrackData_{i}.track"),
                2,
                code.as_str(),
            ));
        }
        std::fs::write(dir.join(GVDB), Gvdb { records }.to_bytes()).unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let codes: Vec<String> = (0..22).map(|i| format!("added_{i:02}")).collect();
        for code in &codes {
            let id = format!("mod.{code}");
            let src = root.join("src").join(&id);
            std::fs::create_dir_all(src.join("track")).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{code}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}, "operations": {{ "assets": ["ops.json"] }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(
                src.join("ops.json"),
                r#"{ "schema_version": 1, "ops": [{"op":"track.add","source":"track"}] }"#,
            )
            .unwrap();
            std::fs::write(
                src.join("track/manifest.json"),
                format!(
                    r#"{{ "track_code": "{code}", "track_name": "{code}", "erp": "{code}.erp",
                         "activation": {{ "copy_from_track": "stock_00" }} }}"#
                ),
            )
            .unwrap();
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled(&id, true))
                .unwrap();
        }
        let built = build(&store, "default", &game).unwrap();
        let notice = built
            .warnings
            .iter()
            .find(|w| w.starts_with("Too many tracks: "))
            .unwrap_or_else(|| panic!("{:?}", built.warnings));
        assert!(
            notice.starts_with(
                "Too many tracks: 22 track(s) are left out of this build because the game has room for 64 tracks, \
                 64 of its own and 0 added ones"
            ),
            "{notice}"
        );
        assert!(notice.contains(": added_00, added_01, "), "{notice}");
        for code in &codes {
            let line = format!(
                "{code} (mod.{code}): track {code} is left out; the game has room for 64 tracks, 64 of its own and \
                 0 added ones, and the track addons before it in the load order fill them. Turn some track addons \
                 off to make room for it"
            );
            assert!(built.warnings.contains(&line), "{line}: {:?}", built.warnings);
        }
        if let Some(db) = built.plan.data.get(GVDB) {
            let out = Gvdb::parse(&std::fs::read(db).unwrap()).unwrap();
            assert_eq!(
                out.get_text("tracks._Track_instancecount_").as_deref(),
                Some("65")
            );
        }

        let career = |id: &str, name: &str, track: &str| {
            let src = root.join("src").join(id);
            std::fs::create_dir_all(&src).unwrap();
            std::fs::write(
                src.join("swofty.json"),
                format!(
                    r#"{{ "schema_version": 1, "id": "{id}", "name": "{name}", "version": "1.0.0", "author": "t",
                         "compat": {{ "game": "f1_25" }}, "operations": {{ "assets": ["ops.json"] }} }}"#
                ),
            )
            .unwrap();
            std::fs::write(
                src.join("ops.json"),
                format!(
                    r#"{{ "schema_version": 1, "ops": [{{"op":"db.set",
                         "select":"track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData[track={track}]",
                         "field":"track","value":"{track}"}}] }}"#
                ),
            )
            .unwrap();
            store.install_addon(&src).unwrap();
            store
                .update_profile("default", |p| p.set_enabled(id, true))
                .unwrap();
        };
        career("t.career", "Career", "added_21");
        let built = build(&store, "default", &game).unwrap();
        let skipped = "Career: change skipped: track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData[track=added_21]: \
                       no row matches in track_data.game_setup.TrackDataSet_0.TrackData; it changes track added_21, \
                       which is left out of this build because the game has room for 64 tracks. Turn some track \
                       addons off to make room for it";
        assert!(
            built.warnings.iter().any(|w| w == skipped),
            "{:?}",
            built.warnings
        );

        career("t.other", "Other", "nowhere");
        let e = build(&store, "default", &game).unwrap_err().to_string();
        assert!(
            e.contains("applying t.other: track_data.game_setup.TrackDataSet[_metadata_=f1].TrackData[track=nowhere]: no row matches"),
            "{e}"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_op_file_is_read_with_its_option_templating_filled_from_the_profile() {
        let root = std::env::temp_dir().join(format!("swofty-read-op-file-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).unwrap();
        let m = AddonManifest::parse(
            r#"{
              "schema_version": 1, "id": "a.dnf", "name": "DNF", "version": "1.0.0", "author": "a",
              "compat": { "game": "f1_25" },
              "options": [
                { "id": "failures", "type": "toggle", "default": false, "binds": "db" },
                { "id": "chance", "type": "slider", "min": 0, "max": 10, "step": 1, "default": 2, "binds": "db" }
              ],
              "operations": { "database": ["db/dnf.json"] }
            }"#,
        )
        .unwrap();
        let path = root.join("dnf.json");
        std::fs::write(
            &path,
            r#"{"schema_version": 1, "ops": [
              {"op": "set", "select": "t.r", "field": "chance", "value": "{{chance}}"},
              {"op": "set", "select": "t.r", "field": "failures", "value": "1", "when": {"failures": true}}
            ]}"#,
        )
        .unwrap();
        let raw = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(OpFile::from_value(raw, FileKind::Database).is_err());
        let mut p = Profile::new("x");
        let file = read_op_file(&path, FileKind::Database, &m, &p, &OnAddons::default()).unwrap();
        assert_eq!(file.ops.len(), 1);
        p.set_option(&m, "failures", serde_json::json!(true)).unwrap();
        p.set_option(&m, "chance", serde_json::json!(7)).unwrap();
        let file = read_op_file(&path, FileKind::Database, &m, &p, &OnAddons::default()).unwrap();
        assert_eq!(file.ops.len(), 2);
        assert!(
            matches!(&file.ops[0], AssetOp::Db(crate::ops::DbOp::Set { value, .. }) if value.as_deref() == Some("7"))
        );
        std::fs::write(&path, r#"{"ops": [{"op": "set", "value": "{{missing}}"}]}"#).unwrap();
        assert!(read_op_file(&path, FileKind::Database, &m, &p, &OnAddons::default()).is_err());
        std::fs::remove_dir_all(&root).ok();
    }
}
