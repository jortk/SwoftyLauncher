//! DB Editor addons: named sets of edits installed as data addons
//! (`<data>/addons/<id>/` with `swofty.json`, `database.json` and `dbedit.json`).
//! The module keeps its old name, `package`.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use swofty_formats::gvdb::{Gvdb, Record};

use super::{Catalog, Edit, Edits, ImportReport, Origin, RowCopy};
use crate::install::Game;
use crate::manifest::{AddonManifest, Category, MANIFEST_FILE, valid_id};
use crate::store::Store;
use crate::{Error, Result, io_err};

/// The DB Editor's source file; its presence marks a DB Editor addon.
pub const MARKER: &str = "dbedit.json";
/// The database operation file generated from the edits.
pub const OPS_FILE: &str = "database.json";
pub const FORMAT: &str = "swofty-db-package";
pub const ID_PREFIX: &str = "dbedit.";
pub const DEFAULT_AUTHOR: &str = "Unknown";
/// `origin.tool` of every DB Editor addon.
pub const ORIGIN_TOOL: &str = "db-editor";
const GAME_BUILDS: &str = ">=25251167";
/// The first launcher that reads copied rows (`rows` in `dbedit.json`, format version 2).
const ROWS_LAUNCHER: semver::Version = semver::Version::new(1, 3, 4);
/// The game database's file name, which a modded-database import looks for.
pub const GVDB_NAME: &str = "gamedatabases.2025.bin";
pub const README: &str = "README.txt";
const MAX_GVDB: u64 = 1 << 30;
const MAX_TRIES: u32 = 1000;

#[derive(Debug, Clone, PartialEq)]
pub struct PackageInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub changes: usize,
    /// The catalog build the edits were made against.
    pub game_build: String,
}

#[derive(Debug, Clone)]
pub struct Package {
    pub id: String,
    pub manifest: AddonManifest,
    pub edits: Edits,
    pub game_build: String,
}

#[derive(Debug, Clone)]
pub struct Imported {
    pub package: PackageInfo,
    pub report: ImportReport,
    /// The id the file asked for when it was taken and a new one was given.
    pub renamed_from: Option<String>,
}

/// What [`export_game_files`] wrote.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GameExport {
    pub changes: usize,
    /// Changes that no longer apply, each naming its addon.
    pub skipped: Vec<String>,
    /// Values the database has no rows for, such as Driver Manager drivers, each naming its addon.
    pub warnings: Vec<String>,
    /// Size of the written zip.
    pub bytes: u64,
}

/// `dbedit.json`. Format version 2 adds `rows`; a file without copied rows is written as version 1.
#[derive(Serialize, Deserialize)]
struct PackageFile {
    format: String,
    format_version: u32,
    #[serde(default)]
    game_build: String,
    edits: Vec<Edit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rows: Vec<RowCopy>,
}

fn dir(store: &Store, id: &str) -> PathBuf {
    store.addons_dir().join(id)
}

fn not_a_package(id: &str) -> Error {
    Error::Other(format!("{id} is not a DB Editor addon"))
}

pub fn is_package(store: &Store, id: &str) -> bool {
    id.starts_with(ID_PREFIX) && valid_id(id) && dir(store, id).join(MARKER).is_file()
}

/// Every installed DB Editor addon that reads, sorted by name; see [`scan`]
/// for the ones that don't.
pub fn list(store: &Store) -> Result<Vec<PackageInfo>> {
    Ok(scan(store)?.packages)
}

#[derive(Debug, Default)]
pub struct Scan {
    /// The addons that read, sorted by name.
    pub packages: Vec<PackageInfo>,
    /// (id, error) for each addon whose files are damaged.
    pub damaged: Vec<(String, String)>,
}

pub fn scan(store: &Store) -> Result<Scan> {
    let rd = match std::fs::read_dir(store.addons_dir()) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
        Err(e) => return Err(io_err(store.addons_dir())(e)),
    };
    let (mut out, mut damaged) = (Vec::new(), Vec::new());
    for entry in rd.flatten() {
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if is_package(store, &id) {
            match Package::open(store, &id) {
                Ok(p) => out.push(p.info()),
                Err(e) => damaged.push((id, e.to_string())),
            }
        }
    }
    out.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    damaged.sort();
    Ok(Scan {
        packages: out,
        damaged,
    })
}

/// `name`, or `name (2)`, `name (3)`… when another addon already has it or
/// `renumber` is set.
fn unique_name(store: &Store, name: &str, renumber: bool) -> Result<String> {
    let names: HashSet<String> = list(store)?.into_iter().map(|p| p.name.to_lowercase()).collect();
    if !renumber && !names.contains(&name.to_lowercase()) {
        return Ok(name.to_owned());
    }
    (2..MAX_TRIES)
        .map(|n| format!("{name} ({n})"))
        .find(|n| !names.contains(&n.to_lowercase()))
        .ok_or_else(|| too_many(name))
}

fn too_many(name: &str) -> Error {
    Error::Other(format!("too many addons are named like {name}; rename one"))
}

/// The modified time of an addon's `dbedit.json`.
pub fn modified(store: &Store, id: &str) -> Option<SystemTime> {
    if !valid_id(id) {
        return None;
    }
    std::fs::metadata(dir(store, id).join(MARKER))
        .and_then(|m| m.modified())
        .ok()
}

/// Lowercase `[a-z0-9-]`, words joined by single dashes.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.truncate(40);
    match out.trim_end_matches('-') {
        "" => "addon".into(),
        s => s.into(),
    }
}

fn taken(store: &Store, id: &str) -> Result<bool> {
    Ok(store.addons()?.contains_key(id) || dir(store, id).exists())
}

fn unique_id(store: &Store, name: &str) -> Result<String> {
    let base = format!("{ID_PREFIX}{}", slug(name));
    let installed = store.addons()?;
    std::iter::once(base.clone())
        .chain((2..MAX_TRIES).map(|n| format!("{base}-{n}")))
        .find(|id| !installed.contains_key(id) && !dir(store, id).exists())
        .ok_or_else(|| too_many(name))
}

/// The oldest launcher a new addon accepts: the first that reads a manifest's
/// `category` and `origin`, or this one when it is older.
fn launcher_floor(v: &semver::Version) -> semver::Version {
    let v = semver::Version {
        pre: v.pre.clone(),
        ..semver::Version::new(v.major, v.minor, v.patch)
    };
    v.min(semver::Version::new(1, 3, 0))
}

/// Raises a `>=` launcher requirement below [`ROWS_LAUNCHER`] to it, or to
/// `current` when this launcher is older (a development build).
fn raise_launcher(req: &mut Option<semver::VersionReq>, current: &semver::Version) {
    let current = semver::Version {
        pre: current.pre.clone(),
        ..semver::Version::new(current.major, current.minor, current.patch)
    };
    let floor = ROWS_LAUNCHER.min(current);
    let low = match req.as_ref().map(|r| r.comparators.as_slice()) {
        None => true,
        Some([c]) if c.op == semver::Op::GreaterEq => {
            let v = semver::Version {
                pre: c.pre.clone(),
                ..semver::Version::new(c.major, c.minor.unwrap_or(0), c.patch.unwrap_or(0))
            };
            v < floor
        }
        Some(_) => false,
    };
    if low {
        *req = semver::VersionReq::parse(&format!(">={floor}")).ok();
    }
}

fn origin() -> crate::manifest::Origin {
    crate::manifest::Origin {
        tool: ORIGIN_TOOL.into(),
        tool_version: Some(crate::update::current_version().to_string()),
        file: None,
        sha256: None,
    }
}

fn manifest(id: &str, name: &str) -> Result<AddonManifest> {
    let launcher = launcher_floor(&crate::update::current_version());
    let m: AddonManifest = serde_json::from_value(serde_json::json!({
        "schema_version": crate::manifest::SCHEMA_VERSION,
        "id": id,
        "name": name,
        "version": "1.0.0",
        "author": DEFAULT_AUTHOR,
        "kind": ["database"],
        "compat": { "game": crate::GAME_ID, "game_builds": [GAME_BUILDS], "launcher": format!(">={launcher}") },
        "operations": { "database": [OPS_FILE] },
    }))
    .map_err(|e| Error::Manifest(e.to_string()))?;
    let m = AddonManifest {
        origin: Some(origin()),
        ..m
    };
    m.validate()?;
    Ok(m)
}

fn read_file(path: &Path) -> Result<PackageFile> {
    let file: PackageFile = crate::read_json(path)?;
    if file.format != FORMAT {
        return Err(Error::Other(format!(
            "{}: format {:?} is not {FORMAT}",
            path.display(),
            file.format
        )));
    }
    Ok(file)
}

fn scratch_dir(store: &Store) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    store
        .root
        .join("cache")
        .join(format!("dbedit-import-{}-{n}", std::process::id()))
}

fn file_label(file: &Path) -> String {
    file.file_name().map_or_else(
        || file.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

fn file_stem(file: &Path) -> String {
    file.file_stem()
        .map(|s| s.to_string_lossy().trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Imported".into())
}

/// What [`read`] found in a file.
#[derive(Debug)]
pub enum Incoming {
    /// A DB Editor addon `.zip`, installed as a new addon.
    Addon(Imported),
    /// Changes without addon details, named after the file.
    Changes {
        name: String,
        edits: Edits,
        report: ImportReport,
    },
}

/// `.zip` with `dbedit.json` installs as a new addon (a taken id gets a new one);
/// a `.json` DB Editor or EGO export, a modded `gamedatabases.2025.bin`, or a
/// `.zip` holding one and no `swofty.json` becomes a new addon named after the file.
pub fn import(store: &Store, catalog: &Catalog, file: &Path) -> Result<Imported> {
    match read(store, catalog, file)? {
        Incoming::Addon(done) => Ok(done),
        Incoming::Changes { name, edits, report } => {
            let mut package = Package::create(store, &unique_name(store, &name, false)?)?;
            package.edits = edits;
            package.save(store, catalog)?;
            Ok(Imported {
                report,
                package: package.info(),
                renamed_from: None,
            })
        }
    }
}

/// Like [`import`], but returns the changes of a `.json`, `.bin` or database
/// `.zip` instead of making an addon of them.
pub fn read(store: &Store, catalog: &Catalog, file: &Path) -> Result<Incoming> {
    let ext = file
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "zip" => import_any_zip(store, catalog, file),
        "json" => import_json(catalog, file),
        "bin" => {
            let bytes = std::fs::read(file).map_err(io_err(file))?;
            import_bin(catalog, &bytes, &file_label(file), &file_stem(file))
        }
        _ => Err(Error::Other(format!(
            "{} is not a DB Editor addon .zip, a .json export or a .bin game database",
            file_label(file)
        ))),
    }
}

fn import_any_zip(store: &Store, catalog: &Catalog, file: &Path) -> Result<Incoming> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(file).map_err(io_err(file))?)?;
    let leaf = |n: &str| n.rsplit(['/', '\\']).next().unwrap_or(n).to_owned();
    let names = z
        .file_names()
        .map(|n| n.map(|n| n.into_owned()))
        .collect::<std::result::Result<Vec<String>, _>>()?;
    if names.iter().any(|n| leaf(n) == MANIFEST_FILE) {
        let scratch = scratch_dir(store);
        let result = import_zip(store, file, &scratch);
        std::fs::remove_dir_all(&scratch).ok();
        return result.map(Incoming::Addon);
    }
    let bins: Vec<&String> = names
        .iter()
        .filter(|n| leaf(n).eq_ignore_ascii_case(GVDB_NAME))
        .collect();
    let label = file_label(file);
    let name = match bins.as_slice() {
        [one] => one.as_str(),
        [] => {
            return Err(Error::Other(format!(
                "{label} is not a DB Editor addon and holds no {GVDB_NAME}"
            )));
        }
        many => {
            return Err(Error::Other(format!(
                "{label} holds {} {GVDB_NAME} files; extract the one you want and import it",
                many.len()
            )));
        }
    };
    let entry = z.by_name(name)?;
    if entry.size() > MAX_GVDB {
        return Err(Error::Other(format!("{label}: {name} is too large")));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .take(MAX_GVDB)
        .read_to_end(&mut bytes)
        .map_err(io_err(file))?;
    import_bin(catalog, &bytes, &label, &file_stem(file))
}

fn import_zip(store: &Store, file: &Path, scratch: &Path) -> Result<Imported> {
    crate::map::unzip(file, scratch, Error::Other)?;
    let root = crate::import::addon_root(scratch).unwrap_or_else(|| scratch.to_path_buf());
    if !root.join(MARKER).is_file() || !root.join(MANIFEST_FILE).is_file() {
        return Err(Error::Other(format!(
            "{} is not a DB Editor addon; install it on the Addons page",
            file_label(file)
        )));
    }
    let (package, renamed_from) = install(store, &root)?;
    Ok(Imported {
        report: ImportReport {
            format: "package",
            imported: package.edits.len(),
            ..ImportReport::default()
        },
        package: package.info(),
        renamed_from,
    })
}

#[derive(Deserialize)]
struct Details {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    category: Option<serde_json::Value>,
}

/// Installs the addon in `root` as a new DB Editor addon: its name, version,
/// author, description and category from `swofty.json`, its edits from
/// `dbedit.json`, and nothing else. A taken id gets a new id and a numbered
/// name, returning the id it asked for.
pub fn install(store: &Store, root: &Path) -> Result<(Package, Option<String>)> {
    let source = read_file(&root.join(MARKER))?;
    let d: Details = crate::read_json(&root.join(MANIFEST_FILE))?;
    let version = semver::Version::parse(d.version.trim())
        .map_err(|_| Error::Other(format!("{:?} is not a version like 1.0.0", d.version)))?;
    let base = d.name.trim();
    if base.is_empty() {
        return Err(Error::Other("an addon needs a name".into()));
    }
    let wanted = format!("{ID_PREFIX}{}", slug(base));
    let renamed_from = taken(store, &wanted)?.then_some(wanted);
    let name = unique_name(store, base, renamed_from.is_some())?;
    let mut package = Package::create(store, &name)?;
    let filled = package
        .set_details(
            &name,
            &version.to_string(),
            &d.author,
            d.description.as_deref().unwrap_or_default(),
        )
        .and_then(|()| {
            package.set_category(d.category.and_then(|v| Category::read(v).ok().flatten()));
            package.edits = Edits {
                schema_version: 1,
                edits: source.edits,
                rows: source.rows,
            };
            package.game_build = source.game_build;
            package.write(store)
        });
    if let Err(e) = filled {
        Package::delete(store, &package.id).ok();
        return Err(e);
    }
    Ok((package, renamed_from))
}

fn import_json(catalog: &Catalog, file: &Path) -> Result<Incoming> {
    let bytes = std::fs::read(file).map_err(io_err(file))?;
    let mut edits = Edits {
        schema_version: 1,
        ..Edits::default()
    };
    let report = edits
        .import(catalog, &bytes, &file_label(file))
        .map_err(Error::Other)?;
    Ok(Incoming::Changes {
        name: file_stem(file),
        edits,
        report,
    })
}

fn import_bin(catalog: &Catalog, bytes: &[u8], label: &str, name: &str) -> Result<Incoming> {
    let (edits, notes) = database_edits(catalog, bytes, label)?;
    Ok(Incoming::Changes {
        name: name.to_owned(),
        report: ImportReport {
            format: "bin",
            imported: edits.len(),
            unchanged: 0,
            skipped: notes,
        },
        edits,
    })
}

/// The edits that turn the stock database into the modded one in `bytes`, and
/// notes on what they change.
fn database_edits(catalog: &Catalog, bytes: &[u8], label: &str) -> Result<(Edits, Vec<String>)> {
    let not_db = || Error::Other(format!("{label} is not an F1 25 game database ({GVDB_NAME})"));
    let modded = Gvdb::parse(bytes).map_err(|_| not_db())?;
    let mut by_key: HashMap<u64, &Record> = HashMap::with_capacity(modded.records.len());
    for r in &modded.records {
        by_key.entry(r.key).or_insert(r);
    }
    let stock: Vec<_> = (0..catalog.records() as u32)
        .map(|i| catalog.record(i))
        .filter(|r| {
            catalog
                .record_by_key(r.key)
                .is_some_and(|first| first.index == r.index)
        })
        .collect();
    if stock.iter().filter(|r| by_key.contains_key(&r.key)).count() * 2 < stock.len() {
        return Err(not_db());
    }
    let origin = Origin::Imported(label.to_owned());
    let text = |key: u64, value: &[u8], notes: &mut Vec<String>| {
        let v = String::from_utf8(value.to_vec()).ok();
        if v.is_none() {
            notes.push(format!("0x{key:016x}: skipped, the value is not text"));
        }
        v
    };
    let (mut edits, mut count_edits, mut notes) = (Vec::new(), Vec::new(), Vec::new());
    let (mut grown, mut resized, mut names) = (Vec::new(), Vec::new(), HashMap::new());
    let (mut changed, mut removed) = (0, 0);
    for s in &stock {
        let edit = match by_key.get(&s.key) {
            None => {
                removed += 1;
                Edits::imported(catalog, s.key, s.tag, None, None, &origin)
            }
            Some(r) if r.tag == s.tag && String::from_utf8_lossy(&r.value) == s.value => continue,
            Some(r) => {
                let Some(value) = text(s.key, &r.value, &mut notes) else {
                    continue;
                };
                changed += 1;
                let Some((container, word)) = catalog.instance_count(s.key) else {
                    edits.push(Edits::imported(catalog, s.key, r.tag, Some(value), None, &origin));
                    continue;
                };
                let table = catalog.table_path(container, word);
                match (s.value.trim().parse::<u32>(), value.trim().parse::<u32>()) {
                    (Ok(from), Ok(to)) if to > from => {
                        names.extend(catalog.row_keys(container, word, from..to));
                        grown.push(table);
                    }
                    _ => resized.push(table),
                }
                count_edits.push(Edits::imported(catalog, s.key, r.tag, Some(value), None, &origin));
                continue;
            }
        };
        edits.push(edit);
    }
    let mut seen = HashSet::new();
    for r in &modded.records {
        if catalog.record_by_key(r.key).is_some() || !seen.insert(r.key) {
            continue;
        }
        if let Some(value) = text(r.key, &r.value, &mut notes) {
            edits.push(Edits::imported(
                catalog,
                r.key,
                r.tag,
                Some(value),
                names.remove(&r.key),
                &origin,
            ));
        }
    }
    let added = edits.len() + count_edits.len() - changed - removed;
    // Row counts go last, so selectors resolve against the stock rows.
    edits.extend(count_edits);
    if edits.is_empty() {
        return Err(Error::Other(format!(
            "{label} is the same as this game's stock database"
        )));
    }
    let mut out = vec![format!(
        "{changed} changed, {added} added and {removed} removed key(s)"
    )];
    if !grown.is_empty() {
        out.push(format!(
            "adds rows to {}; another addon that adds rows to the same tables won't stack with this one",
            grown.join(", ")
        ));
    }
    if !resized.is_empty() {
        out.push(format!("changes the row count of {}", resized.join(", ")));
    }
    out.extend(notes);
    Ok((
        Edits {
            schema_version: 1,
            edits,
            rows: Vec::new(),
        },
        out,
    ))
}

/// Ids of the DB Editor addons enabled in `profile`, in load order.
pub fn enabled_packages(store: &Store, profile: &str) -> Result<Vec<String>> {
    let p = store.profile(profile)?;
    let order = match crate::resolver::resolve(&p, &store.addons()?) {
        Ok(plan) => plan.order,
        Err(_) => p.enabled().map(str::to_owned).collect(),
    };
    Ok(order.into_iter().filter(|id| is_package(store, id)).collect())
}

/// key → (addon id, value) for each DB Editor addon enabled in `profile`, in
/// load order; a removed key has an empty value. Addons that don't read are left out.
pub fn enabled_changes(store: &Store, profile: &str) -> Result<HashMap<u64, Vec<(String, String)>>> {
    let mut out: HashMap<u64, Vec<(String, String)>> = HashMap::new();
    for id in enabled_packages(store, profile)? {
        let Ok(package) = Package::open(store, &id) else {
            continue;
        };
        for e in package.edits.edits {
            out.entry(e.key)
                .or_default()
                .push((id.clone(), e.value.unwrap_or_default()));
        }
    }
    Ok(out)
}

/// Turns off every DB Editor addon enabled in `profile`; returns how many.
pub fn turn_off_all(store: &Store, profile: &str) -> Result<usize> {
    if profile == crate::profile::VANILLA {
        return Err(Error::Other(format!(
            "{profile} stays stock and has no addons on"
        )));
    }
    let on = enabled_packages(store, profile)?;
    store.update_profile(profile, |p| {
        for id in &on {
            p.set_enabled(id, false)?;
        }
        Ok(on.len())
    })
}

/// Applies the addons `ids`, in order, to the stock game database and
/// writes it to a zip at its game-folder path, with a `README.txt`.
pub fn export_game_files(store: &Store, game: &Game, ids: &[String], out: &Path) -> Result<GameExport> {
    let packages = ids
        .iter()
        .map(|id| Package::open(store, id))
        .collect::<Result<Vec<_>>>()?;
    export_packages(store, game, &packages, out)
}

/// [`export_game_files`] of addons in memory, such as [`Package::unsaved`].
pub fn export_packages(store: &Store, game: &Game, packages: &[Package], out: &Path) -> Result<GameExport> {
    if packages.is_empty() {
        return Err(Error::Other("pick at least one addon to export".into()));
    }
    let src = super::stock_database(store, game)?;
    let stock = std::fs::read(&src).map_err(io_err(&src))?;
    let mut db = Gvdb::parse(&stock)?;
    let keys: HashSet<u64> = db.records.iter().map(|r| r.key).collect();
    let fields = std::collections::BTreeMap::new();
    let names = crate::rowkeys::Names {
        fields: &fields,
        keys: crate::rowkeys::RowKeys::builtin(),
        stock: Some(&keys),
        tracks: None,
    };
    let mut export = GameExport::default();
    let mut left_out = 0;
    for p in packages {
        let ops = p.edits.to_ops();
        let mut report = crate::ops::DbReport::default();
        let (_, failed) = crate::ops::apply_db_ops_lenient(&mut db, &ops, &names, &mut report);
        left_out += failed.len() + report.stock_removals;
        export.changes += ops.ops.len() - failed.len() - report.stock_removals;
        let name = &p.manifest.name;
        export.skipped.extend(
            failed
                .into_iter()
                .map(|(_, e)| format!("{name}: change skipped: {e}")),
        );
        if report.stock_removals > 0 {
            export
                .skipped
                .push(crate::planner::stock_removals_line(name, report.stock_removals));
        }
    }
    if let Ok(cat) = Catalog::open(store, game) {
        for t in super::rows::trim_row_counts(&cat, &mut db) {
            let by = packages.iter().rev().find(|p| p.edits.get(t.key).is_some());
            export
                .skipped
                .push(t.line(by.map_or("database", |p| p.manifest.name.as_str())));
            left_out += 1;
        }
    }
    for c in super::calendar::cut_rounds(&db) {
        let by = packages.iter().rev().find(|p| p.edits.get(c.key).is_some());
        export
            .warnings
            .push(c.line(by.map_or("database", |p| p.manifest.name.as_str())));
    }
    let picks: Vec<(String, Vec<String>)> = packages
        .iter()
        .map(|p| (p.manifest.name.clone(), super::driver_manager::picks(&p.edits)))
        .collect();
    if picks.iter().any(|(_, p)| !p.is_empty()) {
        let names = super::driver_manager::names(store);
        export.warnings.extend(super::driver_manager::warnings(
            &db,
            &picks,
            &names,
            super::driver_manager::Why::GameFiles,
        ));
    }
    let bytes = db.to_bytes();
    if bytes == stock {
        return Err(Error::Other(
            "these addons change nothing in this game's database".into(),
        ));
    }
    let build = crate::baseline::sha256_bytes(&stock)[..16].to_owned();
    let readme = readme(packages, game, &build, left_out);
    let tmp = out.with_extension(format!("zip.{}.tmp", std::process::id()));
    let written = write_game_zip(&tmp, &bytes, &readme).and_then(|()| {
        std::fs::rename(&tmp, out).map_err(io_err(out))?;
        std::fs::metadata(out).map_err(io_err(out))
    });
    if written.is_err() {
        std::fs::remove_file(&tmp).ok();
    }
    export.bytes = written?.len();
    Ok(export)
}

fn write_game_zip(path: &Path, db: &[u8], readme: &str) -> Result<()> {
    let file = std::fs::File::create(path).map_err(io_err(path))?;
    let mut z = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in [(README, readme.as_bytes()), (crate::build::GVDB, db)] {
        z.start_file(name, opts)?;
        z.write_all(data).map_err(io_err(path))?;
    }
    z.finish()?.sync_all().map_err(io_err(path))
}

fn readme(packages: &[Package], game: &Game, build: &str, skipped: usize) -> String {
    let mut out = String::from("F1 25 game database with DB Editor addons\n\n");
    out += &format!(
        "This zip holds F1 25's game database, {}, with these Swofty Launcher DB Editor addons applied in this order. Where two change the same value, the later one wins.\n\n",
        crate::build::GVDB
    );
    for p in packages {
        let m = &p.manifest;
        out += &format!(
            "- {} {} by {} ({} change(s))\n",
            m.name,
            m.version,
            m.author,
            p.edits.len()
        );
    }
    out += &format!(
        "\nMade for F1 25 game build {}, stock database {build}.\n",
        game.identity.key()
    );
    if skipped > 0 {
        out += &format!("{skipped} change(s) did not apply to this database and are left out.\n");
    }
    out += "\nInstall\n\
        1. Close F1 25.\n\
        2. In Steam, right-click F1 25 > Manage > Browse local files.\n\
        3. Extract this zip into that folder and replace the file when asked.\n\n\
        Undo\n\
        In Steam, right-click F1 25 > Properties > Installed Files > Verify integrity of game files.\n\n\
        Swofty Launcher\n\
        Swofty Launcher refuses modded launches while this file is replaced.\n\
        With Swofty Launcher, install the addons' .zip files on its Addons page instead.\n";
    out
}

impl Package {
    pub fn create(store: &Store, name: &str) -> Result<Package> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Other("an addon needs a name".into()));
        }
        let id = unique_id(store, name)?;
        let mut package = Package {
            manifest: manifest(&id, name)?,
            id,
            edits: Edits {
                schema_version: 1,
                ..Edits::default()
            },
            game_build: String::new(),
        };
        package.write(store)?;
        Ok(package)
    }

    /// Edits that are not installed, for [`export_packages`].
    pub fn unsaved(name: &str, edits: Edits) -> Result<Package> {
        Ok(Package {
            id: format!("{ID_PREFIX}draft"),
            manifest: manifest(&format!("{ID_PREFIX}draft"), name)?,
            edits,
            game_build: String::new(),
        })
    }

    pub fn open(store: &Store, id: &str) -> Result<Package> {
        if !is_package(store, id) {
            return Err(not_a_package(id));
        }
        let dir = dir(store, id);
        let file = read_file(&dir.join(MARKER))?;
        let manifest = AddonManifest::load(&dir)?;
        Ok(Package {
            id: id.to_owned(),
            manifest,
            edits: Edits {
                schema_version: 1,
                edits: file.edits,
                rows: file.rows,
            },
            game_build: file.game_build,
        })
    }

    pub fn info(&self) -> PackageInfo {
        PackageInfo {
            id: self.id.clone(),
            name: self.manifest.name.clone(),
            version: self.manifest.version.to_string(),
            author: self.manifest.author.clone(),
            description: self.manifest.description.clone().unwrap_or_default(),
            changes: self.edits.len(),
            game_build: self.game_build.clone(),
        }
    }

    /// Writes `dbedit.json`, `database.json` and `swofty.json`.
    pub fn save(&mut self, store: &Store, catalog: &Catalog) -> Result<()> {
        self.game_build = catalog.build_id().to_owned();
        self.write(store)
    }

    /// Writes the three files as they are, keeping `game_build`; an addon
    /// from before 1.3.0 gains its origin.
    pub fn write(&mut self, store: &Store) -> Result<()> {
        let m = &mut self.manifest;
        if m.origin.is_none() {
            m.origin = Some(origin());
            let floor = launcher_floor(&crate::update::current_version());
            m.compat.launcher = semver::VersionReq::parse(&format!(">={floor}")).ok();
        }
        if !self.edits.rows.is_empty() {
            raise_launcher(&mut m.compat.launcher, &crate::update::current_version());
        }
        let dir = dir(store, &self.id);
        crate::write_json_shared(&dir.join(OPS_FILE), &self.edits.to_ops())?;
        crate::write_json_shared(&dir.join(MANIFEST_FILE), &self.manifest)?;
        crate::write_json_shared(
            &dir.join(MARKER),
            &PackageFile {
                format: FORMAT.into(),
                format_version: if self.edits.rows.is_empty() { 1 } else { 2 },
                game_build: self.game_build.clone(),
                edits: self.edits.edits.clone(),
                rows: self.edits.rows.clone(),
            },
        )
    }

    pub fn set_details(&mut self, name: &str, version: &str, author: &str, description: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Other("an addon needs a name".into()));
        }
        let version = semver::Version::parse(version.trim())
            .map_err(|_| Error::Other(format!("{version:?} is not a version like 1.0.0")))?;
        let (author, description) = (author.trim(), description.trim());
        self.manifest.name = name.into();
        self.manifest.version = version;
        self.manifest.author = match author {
            "" => DEFAULT_AUTHOR.into(),
            a => a.into(),
        };
        self.manifest.description = (!description.is_empty()).then(|| description.into());
        Ok(())
    }

    pub fn set_category(&mut self, category: Option<Category>) {
        self.manifest.category = category;
    }

    /// Removes every change and returns how many; [`Package::save`] writes it.
    pub fn clear_edits(&mut self) -> usize {
        let n = self.edits.len();
        self.edits.clear();
        n
    }

    pub fn duplicate(store: &Store, id: &str, name: &str) -> Result<Package> {
        let from = Package::open(store, id)?;
        let mut copy = Package::create(store, name)?;
        let m = &from.manifest;
        let description = m.description.clone().unwrap_or_default();
        copy.set_details(name, &m.version.to_string(), &m.author, &description)?;
        copy.edits = from.edits;
        copy.game_build = from.game_build;
        copy.write(store)?;
        Ok(copy)
    }

    pub fn delete(store: &Store, id: &str) -> Result<()> {
        if !is_package(store, id) {
            return Err(not_a_package(id));
        }
        store.remove_addon(id)
    }

    pub fn export_zip(store: &Store, id: &str, out: &Path) -> Result<()> {
        if !is_package(store, id) {
            return Err(not_a_package(id));
        }
        let dir = dir(store, id);
        let file = std::fs::File::create(out).map_err(io_err(out))?;
        let mut z = zip::ZipWriter::new(file);
        let opts =
            zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for name in [MANIFEST_FILE, OPS_FILE, MARKER] {
            let path = dir.join(name);
            let data = std::fs::read(&path).map_err(io_err(&path))?;
            z.start_file(name, opts)?;
            z.write_all(&data).map_err(io_err(out))?;
        }
        z.finish()?.sync_all().map_err(io_err(out))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::dbedit::catalog::tests::sample;
    use swofty_formats::hash::fnv1a64;

    fn store(name: &str) -> Store {
        let root = std::env::temp_dir().join(format!("swofty-dbpkg-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        Store::new(root)
    }

    fn cat() -> Catalog {
        Catalog::load(&sample()).unwrap()
    }

    fn empty() -> Edits {
        Edits {
            schema_version: 1,
            ..Edits::default()
        }
    }

    const NUM: &str = "entrants.drivers.driver_0.car_number";

    #[test]
    fn slugs_and_unique_ids() {
        assert_eq!(slug("  My Changes!! v2 "), "my-changes-v2");
        assert_eq!(slug("Équipe"), "quipe");
        assert_eq!(slug("!!!"), "addon");
        let s = store("ids");
        let a = Package::create(&s, "My changes").unwrap();
        let b = Package::create(&s, "My changes").unwrap();
        let c = Package::create(&s, "my-changes").unwrap();
        assert_eq!(
            [a.id.as_str(), b.id.as_str(), c.id.as_str()],
            ["dbedit.my-changes", "dbedit.my-changes-2", "dbedit.my-changes-3"]
        );
        assert!(valid_id(&a.id) && is_package(&s, &a.id) && !is_package(&s, "swofty.core"));
        assert!(!is_package(&s, "../x") && Package::create(&s, "  ").is_err());
        std::fs::remove_dir_all(&s.root).ok();
    }

    #[test]
    fn reimports_a_long_named_package_under_new_ids() {
        let s = store("long");
        let cat = cat();
        let name = "Formula One twenty twenty five full season grid and car mods";
        assert_eq!(name.len(), 60);
        let mut p = Package::create(&s, name).unwrap();
        p.edits.set(&cat, fnv1a64(NUM), "33").unwrap();
        p.save(&s, &cat).unwrap();
        let zip_path = s.root.join("long.zip");
        Package::export_zip(&s, &p.id, &zip_path).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let (s2, zip2) = (s.clone(), zip_path.clone());
        std::thread::spawn(move || {
            let cat = Catalog::load(&sample()).unwrap();
            for _ in 0..3 {
                let got = import(&s2, &cat, &zip2).unwrap();
                tx.send((got.package.id, got.package.name)).unwrap();
            }
        });
        let wait = std::time::Duration::from_secs(20);
        let got: Vec<_> = (0..3)
            .map(|_| rx.recv_timeout(wait).expect("import hung"))
            .collect();
        let ids: HashSet<&String> = got.iter().map(|(id, _)| id).chain([&p.id]).collect();
        assert_eq!(ids.len(), 4, "{got:?}");
        let names: Vec<&str> = got.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(names, [2, 3, 4].map(|n| format!("{name} ({n})")));
        assert!(got.iter().all(|(id, _)| id.starts_with(&p.id)));
        std::fs::remove_dir_all(&s.root).ok();
    }

    #[test]
    fn addons_accept_every_launcher_since_the_first_with_categories() {
        let v = |s: &str| semver::Version::parse(s).unwrap();
        for (current, floor) in [
            ("1.3.1", "1.3.0"),
            ("1.3.0", "1.3.0"),
            ("1.3.0-beta.1", "1.3.0-beta.1"),
            ("1.2.5", "1.2.5"),
        ] {
            assert_eq!(launcher_floor(&v(current)), v(floor), "{current}");
        }
        let current = crate::update::current_version();
        let m = manifest("dbedit.x", "x").unwrap();
        let req = m.compat.launcher.as_ref().unwrap();
        assert_eq!(req.to_string(), format!(">={}", launcher_floor(&current)));
        assert!(req.matches(&current) && req.matches(&v("1.3.0")));
    }

    #[test]
    fn copied_rows_raise_a_low_launcher_requirement() {
        let v = |s: &str| semver::Version::parse(s).unwrap();
        let req = |s: &str| Some(semver::VersionReq::parse(s).unwrap());
        for (have, current, want) in [
            (req(">=1.3.0"), "1.4.0", ">=1.3.4"),
            (None, "1.4.0", ">=1.3.4"),
            (req(">=1.3.0"), "1.3.3", ">=1.3.3"),
            (req(">=1.5.0"), "1.5.1", ">=1.5.0"),
            (req("^1.2"), "1.5.1", "^1.2"),
        ] {
            let mut r = have;
            raise_launcher(&mut r, &v(current));
            assert_eq!(r.unwrap().to_string(), want, "{current}");
        }
    }

    #[test]
    fn creates_saves_and_reopens_a_package_as_a_data_addon() {
        let s = store("save");
        let cat = cat();
        let mut p = Package::create(&s, "Grid").unwrap();
        assert_eq!(p.manifest.author, DEFAULT_AUTHOR);
        assert!(
            p.manifest
                .compat
                .accepts_launcher(&crate::update::current_version())
        );
        let before = modified(&s, &p.id).unwrap();
        p.edits.set(&cat, fnv1a64(NUM), "33").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        p.save(&s, &cat).unwrap();
        assert!(modified(&s, &p.id).unwrap() > before);
        let m = &s.addons().unwrap()[&p.id];
        assert_eq!(m.operations.database, [OPS_FILE]);
        assert_eq!(m.kind, [crate::manifest::Kind::Database]);
        let ops: crate::ops::DbOps = crate::read_json(&dir(&s, &p.id).join(OPS_FILE)).unwrap();
        assert_eq!(ops, p.edits.to_ops());
        let marker: serde_json::Value = crate::read_json(&dir(&s, &p.id).join(MARKER)).unwrap();
        assert_eq!(marker["format"], FORMAT);
        assert_eq!(marker["format_version"], 1);
        assert_eq!(marker["game_build"], cat.build_id());
        let again = Package::open(&s, &p.id).unwrap();
        assert_eq!(again.edits, p.edits);
        assert_eq!(again.info().changes, 1);
        p.set_details("Grid 2", "1.2.0", "  ", "Bigger").unwrap();
        assert!(p.set_details("x", "one", "a", "").is_err());
        assert!(p.set_details(" ", "1.0.0", "a", "").is_err());
        p.save(&s, &cat).unwrap();
        let info = Package::open(&s, &p.id).unwrap().info();
        assert_eq!(
            (
                info.name.as_str(),
                info.version.as_str(),
                info.author.as_str(),
                info.description.as_str()
            ),
            ("Grid 2", "1.2.0", DEFAULT_AUTHOR, "Bigger")
        );
        std::fs::write(dir(&s, &p.id).join(MARKER), b"{\"format\": \"swofty-db-pa").unwrap();
        assert!(matches!(Package::open(&s, &p.id), Err(Error::Json { .. })));
        assert!(list(&s).unwrap().is_empty());
        let scan = scan(&s).unwrap();
        assert_eq!(scan.damaged.len(), 1);
        assert_eq!(scan.damaged[0].0, p.id);
        std::fs::remove_dir_all(&s.root).ok();
    }

    #[test]
    fn lists_duplicates_and_deletes() {
        let s = store("list");
        let cat = cat();
        let mut b = Package::create(&s, "beta").unwrap();
        b.edits.set(&cat, fnv1a64(NUM), "7").unwrap();
        b.set_details("beta", "2.0.0", "Max", "d").unwrap();
        b.save(&s, &cat).unwrap();
        Package::create(&s, "Alpha").unwrap();
        let names: Vec<_> = list(&s).unwrap().into_iter().map(|i| i.name).collect();
        assert_eq!(names, ["Alpha", "beta"]);
        let copy = Package::duplicate(&s, &b.id, "beta copy").unwrap();
        let info = Package::open(&s, &copy.id).unwrap().info();
        assert_eq!(
            (
                info.id.as_str(),
                info.version.as_str(),
                info.author.as_str(),
                info.changes
            ),
            ("dbedit.beta-copy", "2.0.0", "Max", 1)
        );
        assert_eq!(info.game_build, cat.build_id());
        Package::delete(&s, &b.id).unwrap();
        assert!(!is_package(&s, &b.id) && Package::open(&s, &b.id).is_err());
        assert!(Package::delete(&s, "swofty.core").is_err());
        assert_eq!(list(&s).unwrap().len(), 2);
        std::fs::remove_dir_all(&s.root).ok();
    }

    #[test]
    fn exports_and_imports_zips() {
        let s = store("zip");
        let cat = cat();
        let mut p = Package::create(&s, "Shared").unwrap();
        p.edits.set(&cat, fnv1a64(NUM), "33").unwrap();
        p.save(&s, &cat).unwrap();
        let zip_path = s.root.join("shared.zip");
        Package::export_zip(&s, &p.id, &zip_path).unwrap();
        let z = zip::ZipArchive::new(std::fs::File::open(&zip_path).unwrap()).unwrap();
        let mut names: Vec<String> = z.file_names().map(|n| n.unwrap().into_owned()).collect();
        names.sort();
        assert_eq!(names, [OPS_FILE, MARKER, MANIFEST_FILE]);

        let got = import(&s, &cat, &zip_path).unwrap();
        assert_eq!(got.renamed_from.as_deref(), Some("dbedit.shared"));
        assert_eq!(
            (
                got.package.id.as_str(),
                got.package.name.as_str(),
                got.package.changes
            ),
            ("dbedit.shared-2", "Shared (2)", 1)
        );
        assert_eq!((got.report.format, got.report.imported), ("package", 1));
        Package::delete(&s, &p.id).unwrap();
        let fresh = import(&s, &cat, &zip_path).unwrap();
        assert_eq!(
            (fresh.package.id.as_str(), fresh.renamed_from),
            ("dbedit.shared", None)
        );
        assert!(std::fs::read_dir(s.root.join("cache")).unwrap().next().is_none());

        let plain = s.root.join("plain.zip");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&plain).unwrap());
        z.start_file(MANIFEST_FILE, zip::write::SimpleFileOptions::default())
            .unwrap();
        z.write_all(b"{}").unwrap();
        z.finish().unwrap();
        let e = import(&s, &cat, &plain).unwrap_err().to_string();
        assert_eq!(
            e,
            "plain.zip is not a DB Editor addon; install it on the Addons page"
        );
        assert!(import(&s, &cat, &s.root.join("x.txt")).is_err());
        std::fs::remove_dir_all(&s.root).ok();
    }

    fn zip_with(path: &Path, files: &[(&str, &[u8])]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (n, d) in files {
            z.start_file(*n, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(d).unwrap();
        }
        z.finish().unwrap();
    }

    fn smuggling_zip(path: &Path, version: &str) {
        let manifest = serde_json::json!({
            "schema_version": 1, "id": "evil.native", "name": "Grid tweaks", "version": version,
            "author": "x", "description": "d", "kind": ["database", "native", "files"],
            "compat": {"game": "f1_25"},
            "operations": {"database": ["database.json"], "native": [{"dll": "evil.dll"}],
                           "files": [{"source": "evil.dll", "target": "evil.dll"}]}
        });
        let mut edits = empty();
        edits.set(&cat(), fnv1a64(NUM), "33").unwrap();
        let marker = serde_json::json!({"format": FORMAT, "format_version": 1, "game_build": "abc",
            "edits": edits.edits});
        let ops = serde_json::json!({"schema_version": 1, "ops": [{"op": "put-record",
            "key": format!("0x{:016x}", fnv1a64("entrants.drivers.driver_1.car_number")), "tag": 0, "value": "3939"}]});
        zip_with(
            path,
            &[
                (MANIFEST_FILE, manifest.to_string().as_bytes()),
                (MARKER, marker.to_string().as_bytes()),
                (OPS_FILE, ops.to_string().as_bytes()),
                ("evil.dll", b"MZ fake"),
            ],
        );
    }

    #[test]
    fn imports_only_the_details_and_edits_of_a_zip() {
        let s = store("smuggle");
        let cat = cat();
        std::fs::create_dir_all(&s.root).unwrap();
        let zip_path = s.root.join("pkg.zip");
        smuggling_zip(&zip_path, "1.4.0");
        let got = import(&s, &cat, &zip_path).unwrap();
        assert_eq!(
            (got.package.id.as_str(), got.package.name.as_str()),
            ("dbedit.grid-tweaks", "Grid tweaks")
        );
        assert_eq!(
            (
                got.package.version.as_str(),
                got.package.author.as_str(),
                got.package.description.as_str(),
                got.package.game_build.as_str()
            ),
            ("1.4.0", "x", "d", "abc")
        );
        let d = dir(&s, &got.package.id);
        let mut files: Vec<String> = std::fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        assert_eq!(files, [OPS_FILE, MARKER, MANIFEST_FILE]);
        let p = Package::open(&s, &got.package.id).unwrap();
        let mut want = manifest(&p.id, "Grid tweaks").unwrap();
        want.version = semver::Version::new(1, 4, 0);
        want.author = "x".into();
        want.description = Some("d".into());
        assert_eq!(p.manifest, want);
        let ops: crate::ops::DbOps = crate::read_json(&d.join(OPS_FILE)).unwrap();
        assert_eq!(ops, p.edits.to_ops());

        smuggling_zip(&zip_path, "one");
        let e = import(&s, &cat, &zip_path).unwrap_err().to_string();
        assert_eq!(e, "\"one\" is not a version like 1.0.0");
        assert_eq!(list(&s).unwrap().len(), 1);

        let foreign = s.addons_dir().join("evil.pkg");
        std::fs::create_dir_all(&foreign).unwrap();
        std::fs::copy(d.join(MARKER), foreign.join(MARKER)).unwrap();
        std::fs::copy(d.join(MANIFEST_FILE), foreign.join(MANIFEST_FILE)).unwrap();
        assert!(!is_package(&s, "evil.pkg") && Package::open(&s, "evil.pkg").is_err());
        std::fs::remove_dir_all(&s.root).ok();
    }

    #[test]
    fn installs_from_the_addons_page_as_a_new_package() {
        let cat = cat();
        let (a, b) = (store("share-a"), store("share-b"));
        let mut pa = Package::create(&a, "My changes").unwrap();
        pa.edits.set(&cat, fnv1a64(NUM), "33").unwrap();
        pa.save(&a, &cat).unwrap();
        let zip_path = a.root.join("my-changes.zip");
        Package::export_zip(&a, &pa.id, &zip_path).unwrap();
        let mut pb = Package::create(&b, "My changes").unwrap();
        pb.edits.set(&cat, fnv1a64(NUM), "44").unwrap();
        pb.edits
            .set(&cat, fnv1a64("entrants.drivers.driver_1.car_number"), "55")
            .unwrap();
        pb.save(&b, &cat).unwrap();

        let m = b.install_addon(&zip_path).unwrap();
        assert_eq!(
            (m.id.as_str(), m.name.as_str()),
            ("dbedit.my-changes-2", "My changes (2)")
        );
        assert_eq!(Package::open(&b, &pb.id).unwrap().edits, pb.edits);
        let got = Package::open(&b, &m.id).unwrap();
        assert_eq!(
            (got.edits, got.game_build),
            (pa.edits.clone(), cat.build_id().to_owned())
        );

        let folder = a.root.join("unzipped").join("pkg");
        std::fs::create_dir_all(&folder).unwrap();
        for f in [MANIFEST_FILE, MARKER] {
            std::fs::copy(dir(&a, &pa.id).join(f), folder.join(f)).unwrap();
        }
        std::fs::write(folder.join("evil.dll"), b"MZ").unwrap();
        let m = b.install_addon(&a.root.join("unzipped")).unwrap();
        assert_eq!(m.id, "dbedit.my-changes-3");
        assert!(!dir(&b, &m.id).join("evil.dll").exists());

        let smuggled = a.root.join("smuggled.zip");
        smuggling_zip(&smuggled, "1.0.0");
        let m = b.install_addon(&smuggled).unwrap();
        assert_eq!(m.kind, [crate::manifest::Kind::Database]);
        assert!(m.operations.native.is_empty() && !dir(&b, &m.id).join("evil.dll").exists());
        assert_eq!(list(&b).unwrap().len(), 4);
        std::fs::remove_dir_all(&a.root).ok();
        std::fs::remove_dir_all(&b.root).ok();
    }

    #[test]
    fn other_addons_cannot_take_a_db_editor_id() {
        let cat = cat();
        let s = store("taken-id");
        let mut p = Package::create(&s, "My changes").unwrap();
        p.edits.set(&cat, fnv1a64(NUM), "33").unwrap();
        p.save(&s, &cat).unwrap();
        let plain = |id: &str| {
            let dir = s.root.join("incoming").join(id);
            std::fs::create_dir_all(&dir).unwrap();
            let m = format!(
                r#"{{"schema_version":1,"id":"{id}","name":"x","version":"1.0.0","author":"x","compat":{{"game":"f1_25"}}}}"#
            );
            std::fs::write(dir.join(MANIFEST_FILE), m).unwrap();
            s.install_addon(&dir).unwrap_err().to_string()
        };
        assert!(plain(&p.id).contains("is a DB Editor addon you have"));
        assert_eq!(Package::open(&s, &p.id).unwrap().edits, p.edits);
        assert!(plain("dbedit.other").contains("prefix is kept"));
        assert!(!dir(&s, "dbedit.other").exists());
        std::fs::remove_dir_all(&s.root).ok();
    }

    #[test]
    fn imports_json_exports_as_new_packages() {
        let s = store("json");
        let cat = cat();
        let mut edits = empty();
        edits.set(&cat, fnv1a64(NUM), "44").unwrap();
        std::fs::create_dir_all(&s.root).unwrap();
        let export = s.root.join("My Grid.json");
        std::fs::write(&export, edits.export(&cat)).unwrap();
        let got = import(&s, &cat, &export).unwrap();
        assert_eq!(
            (got.package.id.as_str(), got.package.name.as_str()),
            ("dbedit.my-grid", "My Grid")
        );
        assert_eq!(
            (got.report.format, got.report.imported, got.package.changes),
            ("swofty", 1, 1)
        );

        let marker = s.root.join("again.json");
        std::fs::copy(dir(&s, &got.package.id).join(MARKER), &marker).unwrap();
        let again = import(&s, &cat, &marker).unwrap();
        assert_eq!(
            (again.package.id.as_str(), again.package.changes),
            ("dbedit.again", 1)
        );

        let ego = s.root.join("ego.json");
        let doc = serde_json::json!({"format": "ego-gamedatabases-editor-records",
            "records": {"modified_records": {fnv1a64(NUM).to_string(): "3434"}, "added_records": []}});
        std::fs::write(&ego, doc.to_string()).unwrap();
        let got = import(&s, &cat, &ego).unwrap();
        assert_eq!((got.report.format, got.package.changes), ("ego", 1));
        let bad = s.root.join("bad.json");
        std::fs::write(&bad, b"{\"format\": \"nope\"}").unwrap();
        assert!(import(&s, &cat, &bad).is_err());
        assert!(!is_package(&s, "dbedit.bad"));
        std::fs::remove_dir_all(&s.root).ok();
    }

    #[test]
    fn enabled_changes_follow_the_load_order() {
        let s = store("enabled");
        let cat = cat();
        let key = fnv1a64(NUM);
        let mut ids = Vec::new();
        for (name, v) in [("a", "10"), ("b", "20"), ("off", "30")] {
            let mut p = Package::create(&s, name).unwrap();
            p.edits.set(&cat, key, v).unwrap();
            p.save(&s, &cat).unwrap();
            ids.push(p.id);
        }
        s.update_profile("default", |p| {
            p.set_enabled(&ids[1], true)?;
            p.set_enabled(&ids[0], true)?;
            p.set_enabled(&ids[2], false)
        })
        .unwrap();
        let got = enabled_changes(&s, "default").unwrap();
        assert_eq!(
            got[&key],
            [
                (ids[1].clone(), "20".to_owned()),
                (ids[0].clone(), "10".to_owned())
            ]
        );
        assert!(enabled_changes(&s, "vanilla").unwrap().is_empty());
        std::fs::remove_dir_all(&s.root).ok();
    }

    fn game(s: &Store) -> Game {
        let dir = s.root.join("game");
        let path = dir.join(crate::build::GVDB);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, sample()).unwrap();
        Game {
            dir,
            user_reg: None,
            identity: crate::baseline::Identity {
                build_id: "25251167".into(),
                exe_sha256: "ab".repeat(32),
            },
        }
    }

    fn records(db: &Gvdb) -> Vec<(u64, u8, Vec<u8>)> {
        let mut out: Vec<_> = db
            .records
            .iter()
            .map(|r| (r.key, r.tag, r.value.clone()))
            .collect();
        out.sort();
        out
    }

    fn unzip_all(path: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut z = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        (0..z.len())
            .map(|i| {
                let mut e = z.by_index(i).unwrap();
                let mut data = Vec::new();
                e.read_to_end(&mut data).unwrap();
                (e.name().unwrap().into_owned(), data)
            })
            .collect()
    }

    #[test]
    fn exports_stacked_packages_as_game_files() {
        let s = store("game");
        let (cat, game) = (cat(), game(&s));
        let team = fnv1a64("entrants.teams.team_0.prestige");
        let pole = fnv1a64("tracks.Track_0.PoleOnRight");
        let mut a = Package::create(&s, "Numbers").unwrap();
        a.edits.set(&cat, fnv1a64(NUM), "33").unwrap();
        a.edits.set(&cat, team, "20").unwrap();
        a.set_details("Numbers", "1.2.0", "Max", "").unwrap();
        a.save(&s, &cat).unwrap();
        let mut b = Package::create(&s, "Grid").unwrap();
        b.edits.set(&cat, fnv1a64(NUM), "44").unwrap();
        let nick = b
            .edits
            .add(&cat, "entrants.drivers.driver_1.nickname", 2, "Lando")
            .unwrap();
        b.edits.remove_key(&cat, pole).unwrap();
        b.edits.edits.push(Edit {
            select: Some("entrants.drivers.driver[_metadata_=nobody]".into()),
            ..b.edits.get(fnv1a64(NUM)).unwrap().clone()
        });
        b.edits.edits.last_mut().unwrap().key = 1;
        b.save(&s, &cat).unwrap();
        let out = s.root.join("export.zip");
        let ids = [a.id.clone(), b.id.clone()];
        let got = export_game_files(&s, &game, &ids, &out).unwrap();
        assert_eq!(got.changes, 4, "{got:?}");
        assert_eq!(got.skipped.len(), 2, "{got:?}");
        assert!(got.skipped[0].starts_with("Grid: change skipped: "), "{got:?}");
        assert_eq!(
            got.skipped[1],
            "Grid: 1 removal(s) of stock database records skipped; the game crashes when a record it reads is missing"
        );
        assert_eq!(got.bytes, std::fs::metadata(&out).unwrap().len());
        let files = unzip_all(&out);
        assert_eq!(files.keys().collect::<Vec<_>>(), [README, crate::build::GVDB]);
        let mut want = Gvdb::parse(&sample()).unwrap();
        want.set("entrants.drivers.driver_0.car_number", "44", None)
            .unwrap();
        want.set("entrants.teams.team_0.prestige", "20", None).unwrap();
        want.records.push(Record {
            key: nick,
            tag: 2,
            value: b"Lando".to_vec(),
        });
        let db = Gvdb::parse(&files[crate::build::GVDB]).unwrap();
        assert_eq!(records(&db), records(&want));
        let readme = String::from_utf8(files[README].clone()).unwrap();
        for part in [
            "- Numbers 1.2.0 by Max (2 change(s))",
            "- Grid 1.0.0 by Unknown (4 change(s))",
            &format!("game build {}", game.identity.key()),
            &format!("stock database {}", cat.build_id()),
            "2 change(s) did not apply",
            "Verify integrity of game files",
            "refuses modded launches",
        ] {
            assert!(readme.contains(part), "{part}\n{readme}");
        }
        assert!(readme.find("Numbers").unwrap() < readme.find("Grid").unwrap());

        let rev = [b.id.clone(), a.id.clone()];
        export_game_files(&s, &game, &rev, &out).unwrap();
        let db = Gvdb::parse(&unzip_all(&out)[crate::build::GVDB]).unwrap();
        assert_eq!(db.get_text(NUM).as_deref(), Some("33"));
        assert_eq!(
            std::fs::read(game.dir.join(crate::build::GVDB)).unwrap(),
            sample()
        );
        let empty = Package::create(&s, "Empty").unwrap();
        assert!(export_game_files(&s, &game, &[empty.id], &out).is_err());
        assert!(export_game_files(&s, &game, &[], &out).is_err());
        assert!(export_game_files(&s, &game, &["swofty.core".into()], &out).is_err());
        let left: Vec<_> = std::fs::read_dir(&s.root)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(left.is_empty());
        std::fs::remove_dir_all(&s.root).ok();
    }

    fn modded() -> Gvdb {
        let mut db = Gvdb::parse(&sample()).unwrap();
        db.set(NUM, "33", None).unwrap();
        db.set("entrants.drivers.driver_1.available", "False", None)
            .unwrap();
        db.set("nobody.knows.this", "2.5", None).unwrap();
        db.remove("tracks.Track_0.PoleOnRight").unwrap();
        let add = crate::ops::DbOps {
            strict: false,
            side_data: Default::default(),
            schema_version: 1,
            fields: BTreeMap::from([(
                "entrants.drivers.driver".to_owned(),
                ["_metadata_", "first_name", "car_number", "available"]
                    .map(str::to_owned)
                    .to_vec(),
            )]),
            ops: vec![crate::ops::DbOp::AddRow {
                table: "entrants.drivers".into(),
                from: "entrants.drivers.driver_1".into(),
                set: BTreeMap::from([
                    ("_metadata_".to_owned(), "new_guy".to_owned()),
                    ("car_number".to_owned(), "77".to_owned()),
                ]),
                copy: None,
                stat_id: None,
            }],
        };
        crate::ops::apply_db_ops(&mut db, &add, &BTreeMap::new()).unwrap();
        db.records.push(Record::new("brand.new.key", 2, "hello"));
        db
    }

    #[test]
    fn imports_a_modded_database_as_a_package() {
        let s = store("bin");
        let cat = cat();
        std::fs::create_dir_all(&s.root).unwrap();
        let modded = modded();
        let bin = s.root.join("Big Mod.bin");
        std::fs::write(&bin, modded.to_bytes()).unwrap();
        let got = import(&s, &cat, &bin).unwrap();
        assert_eq!(
            (got.package.id.as_str(), got.package.name.as_str()),
            ("dbedit.big-mod", "Big Mod")
        );
        assert_eq!(got.report.format, "bin");
        assert_eq!(got.report.imported, 10, "{:?}", got.report);
        assert_eq!(
            got.report.skipped,
            [
                "4 changed, 5 added and 1 removed key(s)",
                "adds rows to entrants.drivers.driver; another addon that adds rows to the same tables won't stack with this one",
            ]
        );
        let p = Package::open(&s, &got.package.id).unwrap();
        let num = p.edits.get(fnv1a64(NUM)).unwrap();
        assert_eq!(
            (num.select.as_deref(), num.stock.as_deref(), &num.origin),
            (
                Some("entrants.drivers.driver[_metadata_=max_verstappen]"),
                Some("1"),
                &Origin::Imported("Big Mod.bin".into())
            )
        );
        let row = p
            .edits
            .get(fnv1a64("entrants.drivers.driver_2.car_number"))
            .unwrap();
        assert_eq!(
            (row.path.as_deref(), row.stock.as_deref(), row.tag),
            (Some("entrants.drivers.driver_2.car_number"), None, 0)
        );
        let unknown = p.edits.get(fnv1a64("brand.new.key")).unwrap();
        assert_eq!((unknown.path.as_deref(), unknown.tag), (None, 2));
        assert_eq!(
            p.edits.get(fnv1a64("tracks.Track_0.PoleOnRight")).unwrap().value,
            None
        );
        let count = fnv1a64("entrants.drivers._driver_instancecount_");
        assert_eq!(p.edits.edits.last().unwrap().key, count);

        let mut db = Gvdb::parse(&sample()).unwrap();
        crate::ops::apply_db_ops(&mut db, &p.edits.to_ops(), &BTreeMap::new()).unwrap();
        assert_eq!(records(&db), records(&modded));

        let zip_path = s.root.join("Old Mod.zip");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        z.start_file("readme.txt", opts).unwrap();
        z.write_all(b"copy into the game folder").unwrap();
        z.start_file(format!("Old Mod/{}", crate::build::GVDB), opts)
            .unwrap();
        z.write_all(&modded.to_bytes()).unwrap();
        z.finish().unwrap();
        let zipped = import(&s, &cat, &zip_path).unwrap();
        assert_eq!(
            (
                zipped.package.name.as_str(),
                zipped.report.format,
                zipped.package.changes
            ),
            ("Old Mod", "bin", 10)
        );
        std::fs::remove_dir_all(&s.root).ok();
    }

    #[test]
    fn refuses_stock_and_foreign_databases() {
        let s = store("binbad");
        let cat = cat();
        std::fs::create_dir_all(&s.root).unwrap();
        let err = |name: &str, bytes: &[u8]| {
            let path = s.root.join(name);
            std::fs::write(&path, bytes).unwrap();
            import(&s, &cat, &path).unwrap_err().to_string()
        };
        assert_eq!(
            err("stock.bin", &sample()),
            "stock.bin is the same as this game's stock database"
        );
        let foreign = Gvdb {
            records: vec![Record::new("a.b", 0, "1")],
        };
        for (name, bytes) in [
            ("junk.bin", b"not a database".to_vec()),
            ("other.bin", foreign.to_bytes()),
        ] {
            assert_eq!(
                err(name, &bytes),
                format!("{name} is not an F1 25 game database ({GVDB_NAME})")
            );
        }
        let zip_with = |name: &str, files: &[&str]| {
            let path = s.root.join(name);
            let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            for f in files {
                z.start_file(*f, zip::write::SimpleFileOptions::default())
                    .unwrap();
                z.write_all(&modded().to_bytes()).unwrap();
            }
            z.finish().unwrap();
            import(&s, &cat, &path).unwrap_err().to_string()
        };
        assert_eq!(
            zip_with("none.zip", &["readme.txt"]),
            format!("none.zip is not a DB Editor addon and holds no {GVDB_NAME}")
        );
        assert!(zip_with("two.zip", &[GVDB_NAME, &format!("x/{GVDB_NAME}")]).contains("holds 2"));
        assert_eq!(
            zip_with("addon.zip", &[MANIFEST_FILE, GVDB_NAME]),
            "addon.zip is not a DB Editor addon; install it on the Addons page"
        );
        assert!(list(&s).unwrap().is_empty());
        std::fs::remove_dir_all(&s.root).ok();
    }

    #[test]
    fn imports_a_modded_copy_of_the_real_database() {
        let Some(cat) = crate::dbedit::catalog::tests::real() else {
            eprintln!("F1_25_DIR is not set; skipping");
            return;
        };
        let game = std::path::PathBuf::from(std::env::var_os("F1_25_DIR").unwrap());
        let stock = std::fs::read(game.join(crate::build::GVDB)).unwrap();
        let mut db = Gvdb::parse(&stock).unwrap();
        let tracks = "tracks._Track_instancecount_";
        let n: u32 = db.get_text(tracks).unwrap().parse().unwrap();
        db.set(tracks, (n + 1).to_string(), None).unwrap();
        let num = cat
            .resolve("entrants.drivers.driver[_metadata_=max_verstappen].car_number")
            .unwrap();
        db.records.iter_mut().find(|r| r.key == num).unwrap().value = b"99".to_vec();
        db.records.push(Record::new(
            &format!("tracks.Track_{n}._metadata_"),
            2,
            "new_track",
        ));
        let s = store("realbin");
        std::fs::create_dir_all(&s.root).unwrap();
        let bin = s.root.join(GVDB_NAME);
        std::fs::write(&bin, db.to_bytes()).unwrap();
        let t = std::time::Instant::now();
        let got = import(&s, &cat, &bin).unwrap();
        eprintln!("imported in {:?}: {:?}", t.elapsed(), got.report);
        assert_eq!(got.package.changes, 3);
        assert!(
            got.report.skipped[1].starts_with("adds rows to tracks.Track;"),
            "{:?}",
            got.report
        );
        let p = Package::open(&s, &got.package.id).unwrap();
        let named = p
            .edits
            .get(fnv1a64(format!("tracks.Track_{n}._metadata_")))
            .unwrap();
        assert_eq!(named.path, Some(format!("tracks.Track_{n}._metadata_")));
        let mut again = Gvdb::parse(&stock).unwrap();
        crate::ops::apply_db_ops(&mut again, &p.edits.to_ops(), &BTreeMap::new()).unwrap();
        assert_eq!(records(&again), records(&db));
        std::fs::remove_dir_all(&s.root).ok();
    }

    #[test]
    fn clears_edits_and_turns_off_every_package() {
        let s = store("off");
        let cat = cat();
        let mut ids = Vec::new();
        for name in ["one", "two", "three"] {
            let mut p = Package::create(&s, name).unwrap();
            p.edits.set(&cat, fnv1a64(NUM), "8").unwrap();
            assert_eq!(p.clear_edits(), 1);
            p.save(&s, &cat).unwrap();
            ids.push(p.id);
        }
        assert_eq!(Package::open(&s, &ids[0]).unwrap().info().changes, 0);
        s.update_profile("default", |p| {
            p.set_enabled(&ids[0], true)?;
            p.set_enabled(&ids[2], true)
        })
        .unwrap();
        assert_eq!(
            enabled_packages(&s, "default").unwrap(),
            [ids[0].clone(), ids[2].clone()]
        );
        assert_eq!(turn_off_all(&s, "default").unwrap(), 2);
        assert!(enabled_packages(&s, "default").unwrap().is_empty());
        assert_eq!(turn_off_all(&s, "default").unwrap(), 0);
        assert!(turn_off_all(&s, crate::profile::VANILLA).is_err());
        std::fs::remove_dir_all(&s.root).ok();
    }
}
