use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use swofty_formats::gvdb::Gvdb;

use crate::asset::{AssetOp, FileKind, FileOp, GroupOp, LangOp, Op, OpFile, Origin, Source};
pub use crate::asset::{Features, Kind};
use crate::conflicts::Changes;
use crate::dbedit::deps::Copy;
use crate::entrant_lists::StockLists;
use crate::ops::{
    DbOps, DbReport, ErpOp, FieldNames, MovedStatId, Rewrite, RowIds, apply_db_ops_lenient,
    explicit_stat_ids, ids_exhausted, no_row, no_row_selectors, stat_id_ceiling, unlock_list,
};
use crate::owned::{self, Added};
use crate::rowkeys::{Names, RowKeys};
use crate::{Error, Result};

pub const FEATURES_FILE: &str = "swofty/addons/core/features.json";

const BUILD_DELIVERIES: [Kind; 3] = [Kind::Erp, Kind::Db, Kind::File];
const RUNTIME_DELIVERIES: [Kind; 5] = [Kind::Group, Kind::Erp, Kind::Db, Kind::Lang, Kind::File];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Build,
    Runtime,
    Waiting,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Level {
    Stable,
    Experimental,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct FeaturesFile {
    #[serde(default)]
    features: BTreeMap<String, Level>,
}

pub fn load_features(loader: &Path) -> Result<Features> {
    let path = loader.join(FEATURES_FILE);
    if !path.is_file() {
        return Ok(Features::default());
    }
    let file: FeaturesFile = crate::read_json(&path)?;
    let mut out = Features::default();
    for (name, level) in file.features {
        let Ok(kind) = serde_json::from_value::<Kind>(name.into()) else {
            continue;
        };
        match level {
            Level::Stable => out.stable.insert(kind),
            Level::Experimental => out.experimental.insert(kind),
            Level::Other => false,
        };
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Target {
    Group(String),
    Resource { erp: String, uri: String },
    Slice { erp: String, uri: String, slice: usize },
    Record(u64),
    String { language: String, key: String },
    File(String),
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Group(g) => write!(f, "asset group {g}"),
            Target::Resource { erp, uri } => write!(f, "{uri} in {erp}"),
            Target::Slice { erp, uri, slice } => write!(f, "slice {slice} of {uri} in {erp}"),
            Target::Record(key) => write!(f, "database record {key:#018x}"),
            Target::String { language, key } => write!(f, "text {key} ({language})"),
            Target::File(rel) => f.write_str(rel),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub target: Target,
    pub winner: Origin,
    pub overridden: Vec<Origin>,
}

impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lost: Vec<String> = self.overridden.iter().map(Origin::to_string).collect();
        write!(
            f,
            "{}: {} overrides {}",
            self.target,
            self.winner,
            lost.join(", ")
        )
    }
}

/// The build warning for `n` removals of stock records that addon `name` asked for.
pub fn stock_removals_line(name: &str, n: usize) -> String {
    format!(
        "{name}: {n} removal(s) of stock database records skipped; the game crashes when a record it reads is missing"
    )
}

/// The build warning for `n` removals of records the tracks add that addon `name` asked for.
pub fn track_removals_line(name: &str, n: usize) -> String {
    format!(
        "{name}: {n} removal(s) of database records the tracks add skipped; the game crashes when a record it reads is missing"
    )
}

// Field names, the ids that the addons after this one set on the rows they add, and the install's kept row ids.
struct Reserving<'a> {
    names: Names<'a>,
    later: &'a HashSet<(String, i64)>,
    row_ids: Option<crate::row_ids::Scoped<'a>>,
}

impl FieldNames for Reserving<'_> {
    fn fields(&self, table: &str, row: &str) -> Option<Vec<String>> {
        self.names.fields(table, row)
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
        self.later.contains(&(format!("{table}.{row}"), id))
    }

    fn row_ids(&self) -> Option<&dyn RowIds> {
        self.row_ids.as_ref().map(|s| s as &dyn RowIds)
    }
}

// What every addon's database ops share: field names, stock and track keys, and the kept row ids.
struct Shared<'a> {
    fields: &'a BTreeMap<String, BTreeMap<String, Vec<String>>>,
    stock: &'a HashSet<u64>,
    tracks: &'a HashSet<u64>,
    row_ids: Option<&'a RefCell<crate::row_ids::Store>>,
}

impl<'a> Shared<'a> {
    fn names(&self, addon: &'a str, later: &'a HashSet<(String, i64)>) -> Reserving<'a> {
        static NONE: BTreeMap<String, Vec<String>> = BTreeMap::new();
        Reserving {
            names: Names {
                fields: self.fields.get(addon).unwrap_or(&NONE),
                keys: RowKeys::builtin(),
                stock: Some(self.stock),
                tracks: Some(self.tracks),
            },
            later,
            row_ids: self.row_ids.map(|store| crate::row_ids::Scoped { addon, store }),
        }
    }
}

// An addon's ops waiting for their row, with the error each gave last.
struct Waiting {
    origin: Origin,
    ops: DbOps,
    later: HashSet<(String, i64)>,
    errors: Vec<swofty_addon::Error>,
    since: usize,
}

fn stat_row(row: &str) -> &str {
    row.rsplit_once('_').map_or(row, |(table, _)| table)
}

/// The build warning for a row of `addon` whose `stat_id` a row of `holder` already had, or
/// which was above the table's ceiling when `holder` is `None`.
pub fn moved_stat_id_line(addon: &str, holder: Option<&str>, m: &MovedStatId) -> String {
    let label = |l: &Option<String>, row: &str| l.clone().unwrap_or_else(|| row.to_owned());
    let why = match (holder, &m.holder, &m.record) {
        (Some(holder), Some(row), _) => {
            format!("is taken by {} from {holder}", label(&m.holder_label, row))
        }
        (_, _, Some(record)) => {
            format!("is the id careers hold for {record}")
        }
        _ => {
            let table = stat_row(&m.row);
            let max = table
                .rsplit_once('.')
                .and_then(|(t, r)| stat_id_ceiling(t, r))
                .map_or_else(String::new, |c| format!(" {}", c.max));
            format!("is above{max}, the highest the game reads in {table}")
        }
    };
    let field = m.field;
    let advice = match field {
        "stat_id" => "leave stat_id out of add-row",
        _ if stat_row(&m.row)
            .rsplit_once('.')
            .is_some_and(|(t, _)| unlock_list(t).is_some()) =>
        {
            "leave statId out of add-row"
        }
        _ => "give the row a statId no other addon uses",
    };
    format!(
        "{addon}: {field} {} of {} {why}; the build gave it {field} {}. The addon's author should {advice}",
        m.old,
        label(&m.label, &m.row),
        m.new
    )
}

/// The build warning for the entrant `lists` addon `name` renames or removes.
pub fn kept_lists_line(name: &str, lists: &[String]) -> String {
    let shown: Vec<&str> = lists.iter().take(8).map(String::as_str).collect();
    let more = match lists.len() > shown.len() {
        true => format!(" and {} more", lists.len() - shown.len()),
        false => String::new(),
    };
    format!(
        "{name}: renames or removes the entrant list(s) {}{more}; F2 menus may crash with it",
        shown.join(", ")
    )
}

#[derive(Debug, Clone, Default)]
pub struct Planned {
    pub runtime: Vec<Op>,
    pub waiting: Vec<Op>,
    pub conflicts: Vec<Conflict>,
    /// Names for conflicting records of rows the build added, which the catalogue doesn't know.
    pub record_names: BTreeMap<u64, String>,
}

pub struct Planner<'a> {
    features: &'a Features,
    opt_in: &'a BTreeSet<Kind>,
    build_only: &'a BTreeSet<Kind>,
    db: Vec<(Origin, DbOps)>,
    erp: Vec<Op>,
    files: Vec<Op>,
    tracks: Vec<Op>,
    items: Vec<Op>,
    planned: Planned,
    writers: BTreeMap<Target, Vec<Origin>>,
    // Origins in the order their first op came, which is load order.
    ranks: Vec<Origin>,
    removals: usize,
    fields: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    skipped: Vec<String>,
    id_left_out: BTreeMap<String, usize>,
    waits: BTreeSet<(String, String)>,
    stock_removals: BTreeMap<String, usize>,
    track_removals: BTreeMap<String, usize>,
    lists: Option<StockLists>,
    kept_lists: BTreeMap<String, Vec<String>>,
    copies: Vec<Copy>,
    lenient: BTreeMap<String, String>,
    names: BTreeMap<String, String>,
    // The tracks the build leaves out because the game's track lists are full, by code, each with its name.
    left_out_tracks: BTreeMap<String, String>,
    // The records each origin's database ops changed, by the origin's rank.
    rewrites: Vec<(usize, Rewrite)>,
    core_rows: BTreeMap<String, BTreeSet<String>>,
    touched: BTreeMap<String, Touched>,
    overlays: Option<&'a OverlayNames<'a>>,
    mipmaps: Option<&'a MipmapNames<'a>>,
    row_ids: Option<RefCell<crate::row_ids::Store>>,
}

/// The resource names in an `erp.overlay` op's archive.
pub type OverlayNames<'a> = dyn Fn(&Origin, &Source) -> Vec<String> + 'a;

/// The file names in an `erp.add-mipmaps` op's source folder; empty when the source is a single file.
pub type MipmapNames<'a> = dyn Fn(&Origin, &Source) -> Vec<String> + 'a;

#[derive(Default)]
struct Touched {
    records: HashSet<u64>,
    other: HashSet<Target>,
}

fn erp_target(op: &ErpOp) -> Option<(Target, bool)> {
    let resource = |erp: &str, uri: &str| Target::Resource {
        erp: erp.to_owned(),
        uri: uri.to_owned(),
    };
    match op {
        ErpOp::ReplaceResource { erp, resource: r, .. } | ErpOp::RemoveResource { erp, resource: r } => {
            Some((resource(erp, r), true))
        }
        ErpOp::PutResource {
            erp,
            resource: r,
            rename,
            ..
        } => Some((resource(erp, rename.as_deref().unwrap_or(r)), true)),
        ErpOp::CopyResource {
            erp,
            resource: r,
            rename,
            ..
        } => Some((resource(erp, rename.as_deref().unwrap_or(r)), false)),
        ErpOp::AddResource { erp, resource: r, .. } => Some((resource(erp, r), true)),
        ErpOp::PatchPkg { erp, resource: r, .. } | ErpOp::SetMaterial { erp, resource: r, .. } => {
            Some((resource(erp, r), false))
        }
        ErpOp::AddMipmaps { target, .. } => {
            Some((Target::File(target.trim_end_matches('/').to_owned()), true))
        }
        // Its resources come from the archive's index, through `Planner::overlays`.
        ErpOp::Overlay { .. } => None,
        // The build writes one target per slice it changes, through `Planner::replaced`.
        ErpOp::MergeSlices { .. } => None,
        // It adds slices after the array as earlier addons left it, so it replaces nothing.
        ErpOp::AppendSlices { .. } => None,
    }
}

fn target(op: &AssetOp) -> Option<(Target, bool)> {
    match op {
        AssetOp::Group(GroupOp::Register { group, .. }) => Some((Target::Group(group.clone()), false)),
        AssetOp::Erp(op) => erp_target(op),
        AssetOp::Lang(LangOp::Set { language, key, .. }) => Some((
            Target::String {
                language: language.clone(),
                key: key.clone(),
            },
            true,
        )),
        AssetOp::File(FileOp::Put { target, .. }) => Some((Target::File(target.clone()), true)),
        AssetOp::Db(_)
        | AssetOp::Lang(LangOp::Table { .. } | LangOp::Default { .. })
        | AssetOp::Track(_)
        | AssetOp::Item(_)
        | AssetOp::Warn { .. } => None,
    }
}

impl<'a> Planner<'a> {
    pub fn new(features: &'a Features, opt_in: &'a BTreeSet<Kind>) -> Self {
        static NONE: BTreeSet<Kind> = BTreeSet::new();
        Self {
            features,
            opt_in,
            build_only: &NONE,
            db: Vec::new(),
            erp: Vec::new(),
            files: Vec::new(),
            tracks: Vec::new(),
            items: Vec::new(),
            planned: Planned::default(),
            writers: BTreeMap::new(),
            ranks: Vec::new(),
            removals: 0,
            fields: BTreeMap::new(),
            skipped: Vec::new(),
            id_left_out: BTreeMap::new(),
            waits: BTreeSet::new(),
            stock_removals: BTreeMap::new(),
            track_removals: BTreeMap::new(),
            lists: None,
            kept_lists: BTreeMap::new(),
            copies: Vec::new(),
            lenient: BTreeMap::new(),
            names: BTreeMap::new(),
            left_out_tracks: BTreeMap::new(),
            rewrites: Vec::new(),
            core_rows: BTreeMap::new(),
            touched: BTreeMap::new(),
            overlays: None,
            mipmaps: None,
            row_ids: None,
        }
    }

    pub fn overlays(mut self, names: &'a OverlayNames<'a>) -> Self {
        self.overlays = Some(names);
        self
    }

    pub fn mipmaps(mut self, names: &'a MipmapNames<'a>) -> Self {
        self.mipmaps = Some(names);
        self
    }

    pub fn build_only(mut self, kinds: &'a BTreeSet<Kind>) -> Self {
        self.build_only = kinds;
        self
    }

    /// Addons (id → name) whose failing database ops are skipped with a
    /// warning instead of failing the build: the DB Editor's packages.
    pub fn lenient(mut self, addons: BTreeMap<String, String>) -> Self {
        self.lenient = addons;
        self
    }

    /// The addons' names (id → name), for the errors that name an addon.
    pub fn names(mut self, names: BTreeMap<String, String>) -> Self {
        self.names = names;
        self
    }

    fn name<'s>(&'s self, id: &'s str) -> &'s str {
        self.names
            .get(id)
            .or_else(|| self.lenient.get(id))
            .map_or(id, String::as_str)
    }

    /// Skips `id`'s failing database ops from now on: an addon whose track a
    /// build leaves out has ops on that track's rows.
    pub fn add_lenient(&mut self, id: &str, name: &str) {
        self.lenient
            .entry(id.to_owned())
            .or_insert_with(|| name.to_owned());
    }

    /// Skips, with a warning, any addon's change whose row of track `code` is still missing after every addon's
    /// changes: the build leaves that track, named `track`, out because the game's track lists are full.
    pub fn leave_out_track(&mut self, code: &str, track: &str) {
        self.left_out_tracks.insert(code.to_owned(), track.to_owned());
    }

    // The name of the left-out track whose row `e`'s selector matches no row of; `None` for any other failure.
    fn left_out_track(&self, e: &swofty_addon::Error) -> Option<&str> {
        if !no_row(e) {
            return None;
        }
        let failing = no_row_selectors(e).pop()?;
        self.left_out_tracks.get(failing.want.trim()).map(String::as_str)
    }

    /// Records that the core adds rows to `tables` at the game's start, from
    /// the records of `addon` that the build leaves to it. An op that selects
    /// such a row finds none while the build runs.
    pub fn add_core_rows(&mut self, addon: &str, tables: impl IntoIterator<Item = String>) {
        self.core_rows.entry(addon.to_owned()).or_default().extend(tables);
    }

    fn core_row_adders(&self, e: &swofty_addon::Error) -> Vec<&str> {
        let swofty_addon::Error::Resolve(m) = e else {
            return Vec::new();
        };
        if !no_row(e) {
            return Vec::new();
        }
        let path = m.split(['[', ':']).next().unwrap_or_default();
        self.core_rows
            .iter()
            .filter(|(_, tables)| {
                tables
                    .iter()
                    .any(|t| path == t || path.strip_prefix(t.as_str()).is_some_and(|r| r.starts_with('.')))
            })
            .map(|(addon, _)| addon.as_str())
            .collect()
    }

    pub fn route(&self, kind: Kind) -> Route {
        match self.delivery(kind) {
            Route::Runtime if kind == Kind::Erp && self.removals > 0 => Route::Build,
            route => route,
        }
    }

    pub fn route_note(&self, kind: Kind) -> Option<String> {
        (self.route(kind) != self.delivery(kind)).then(|| {
            format!(
                "built into the game files; {} resource removal(s) cannot apply at runtime",
                self.removals
            )
        })
    }

    fn delivery(&self, kind: Kind) -> Route {
        let stable = self.features.stable.contains(&kind);
        let experimental = self.features.experimental.contains(&kind);
        let build = BUILD_DELIVERIES.contains(&kind);
        if build && self.build_only.contains(&kind) {
            return Route::Build;
        }
        let runtime =
            RUNTIME_DELIVERIES.contains(&kind) && (stable || (experimental && self.opt_in.contains(&kind)));
        match (runtime, build) {
            (true, _) => Route::Runtime,
            (false, true) => Route::Build,
            (false, false) => Route::Waiting,
        }
    }

    pub fn push(&mut self, op: Op) {
        self.rank(&op.origin);
        let placed: Vec<String> = match (&op.op, self.mipmaps) {
            (AssetOp::Erp(ErpOp::AddMipmaps { source, target }), Some(names)) => names(&op.origin, source)
                .iter()
                .map(|rel| crate::erpbuild::mipmap_path(target, rel))
                .collect(),
            _ => Vec::new(),
        };
        if placed.is_empty()
            && let Some((target, replaces)) = target(&op.op)
        {
            self.write(target, &op.origin, replaces);
        }
        for path in placed {
            self.write(Target::File(path), &op.origin, true);
        }
        if let (AssetOp::Erp(ErpOp::Overlay { erps, source }), Some(names)) = (&op.op, self.overlays) {
            for uri in names(&op.origin, source) {
                for erp in erps {
                    let target = Target::Resource {
                        erp: erp.clone(),
                        uri: uri.clone(),
                    };
                    self.write(target, &op.origin, true);
                }
            }
        }
        if matches!(op.op, AssetOp::Erp(ErpOp::RemoveResource { .. })) {
            self.removals += 1;
        }
        if matches!(op.op, AssetOp::Track(_)) {
            self.tracks.push(op);
            return;
        }
        if matches!(op.op, AssetOp::Item(_)) {
            self.items.push(op);
            return;
        }
        let Some(kind) = op.op.kind() else {
            self.erp.push(op);
            return;
        };
        match self.route(kind) {
            Route::Runtime if !matches!(kind, Kind::Db | Kind::Erp) => self.planned.runtime.push(op),
            Route::Waiting => self.planned.waiting.push(op),
            Route::Build | Route::Runtime => match op.op {
                AssetOp::Db(db) => match self.db.last_mut() {
                    Some((origin, ops)) if *origin == op.origin => ops.ops.push(db),
                    _ => self.db.push((
                        op.origin,
                        DbOps {
                            schema_version: 1,
                            fields: BTreeMap::new(),
                            strict: false,
                            side_data: Default::default(),
                            ops: vec![db],
                        },
                    )),
                },
                AssetOp::Erp(_) => self.erp.push(op),
                _ => self.files.push(op),
            },
        }
    }

    pub fn push_file(&mut self, origin: &Origin, kind: FileKind, file: OpFile) {
        let db = file.ops.iter().any(|op| matches!(op, AssetOp::Db(_)));
        if kind == FileKind::Database || db {
            let ops = DbOps {
                schema_version: file.schema_version,
                fields: file.fields,
                strict: file.strict,
                side_data: file.side_data,
                ops: Vec::new(),
            };
            self.db.push((origin.clone(), ops));
        }
        for op in file.ops {
            self.push(Op {
                op,
                origin: origin.clone(),
            });
        }
    }

    pub fn add_fields(&mut self, addon: &str, fields: BTreeMap<String, Vec<String>>) {
        if !fields.is_empty() {
            self.fields.entry(addon.to_owned()).or_default().extend(fields);
        }
    }

    fn rank(&mut self, origin: &Origin) -> usize {
        if self.ranks.last() == Some(origin) {
            return self.ranks.len() - 1;
        }
        match self.ranks.iter().position(|o| o == origin) {
            Some(at) => at,
            None => {
                self.ranks.push(origin.clone());
                self.ranks.len() - 1
            }
        }
    }

    fn overrides(&mut self, target: &Target, winner: &Origin, lost: Vec<Origin>) {
        if lost.is_empty() {
            return;
        }
        let conflicts = &mut self.planned.conflicts;
        match conflicts.iter_mut().find(|c| c.target == *target) {
            Some(c) => {
                let before = std::mem::replace(&mut c.winner, winner.clone());
                for o in std::iter::once(before).chain(lost) {
                    if o != *winner && !c.overridden.contains(&o) {
                        c.overridden.push(o);
                    }
                }
                c.overridden.retain(|o| o != winner);
            }
            None => conflicts.push(Conflict {
                target: target.clone(),
                winner: winner.clone(),
                overridden: lost,
            }),
        }
    }

    // Splits the other writers of `target` into those before `origin` in load
    // order, which it overrides and which leave the list, and the latest after it.
    fn split(
        &mut self,
        target: &Target,
        origin: &Origin,
        keep_earlier: bool,
    ) -> (Vec<Origin>, Option<Origin>) {
        let mine = self.rank(origin);
        let ws = self.writers.get(target).cloned().unwrap_or_default();
        let (mut earlier, mut later, mut kept) = (Vec::new(), None, Vec::new());
        for w in ws {
            let rank = self.rank(&w);
            if w == *origin || rank > mine {
                if rank > mine && later.as_ref().is_none_or(|(r, _)| rank > *r) {
                    later = Some((rank, w.clone()));
                }
                kept.push(w);
            } else {
                if keep_earlier {
                    kept.push(w.clone());
                }
                earlier.push(w);
            }
        }
        if let Some(ws) = self.writers.get_mut(target) {
            *ws = kept;
        }
        (earlier, later.map(|(_, w)| w))
    }

    // A whole-resource write overrides the earlier slice writes into that
    // resource, and a slice write overrides the earlier whole-resource writes on
    // that slice, while a later write of either kind overrides it.
    fn across(&mut self, target: &Target, origin: &Origin) -> Vec<Origin> {
        let others: Vec<Target> = match target {
            Target::Slice { erp, uri, .. } => vec![Target::Resource {
                erp: erp.clone(),
                uri: uri.clone(),
            }],
            Target::Resource { erp, uri } => {
                let slice = |slice| Target::Slice {
                    erp: erp.clone(),
                    uri: uri.clone(),
                    slice,
                };
                self.writers
                    .range(slice(0)..=slice(usize::MAX))
                    .map(|(t, _)| t.clone())
                    .collect()
            }
            _ => return Vec::new(),
        };
        let mut lost = Vec::new();
        for other in others {
            let whole = matches!(other, Target::Resource { .. });
            let (earlier, later) = self.split(&other, origin, whole);
            if let Some(winner) = later {
                self.overrides(&other, &winner, vec![origin.clone()]);
            }
            lost.extend(earlier);
        }
        lost
    }

    fn write(&mut self, target: Target, origin: &Origin, replaces: bool) {
        if !self.touched.contains_key(&origin.addon) {
            self.touched.insert(origin.addon.clone(), Touched::default());
        }
        let touched = self.touched.get_mut(&origin.addon).expect("inserted above");
        match target {
            Target::Record(key) => touched.records.insert(key),
            Target::Group(_) => false,
            _ => touched.other.insert(target.clone()),
        };
        if replaces {
            let (mut lost, later) = self.split(&target, origin, false);
            for w in self.across(&target, origin) {
                if !lost.contains(&w) {
                    lost.push(w);
                }
            }
            match later {
                Some(winner) => {
                    lost.push(origin.clone());
                    self.overrides(&target, &winner, lost);
                }
                None => self.overrides(&target, origin, lost),
            }
        }
        let writers = self.writers.entry(target).or_default();
        if !writers.contains(origin) {
            writers.push(origin.clone());
        }
    }

    /// What each addon writes so far: distinct records, resources, files and strings.
    pub fn changes(&self) -> BTreeMap<String, Changes> {
        self.touched
            .iter()
            .map(|(addon, t)| {
                let mut c = Changes {
                    database: t.records.len(),
                    ..Changes::default()
                };
                for target in &t.other {
                    c.count(target);
                }
                (addon.clone(), c)
            })
            .collect()
    }

    /// Records that `origin` replaced `target` (a merged texture-array slice, say).
    pub fn replaced(&mut self, target: Target, origin: &Origin) {
        self.write(target, origin, true);
    }

    /// Records that the writers of `target` were merged: none of them overrides another there.
    pub fn merged(&mut self, target: &Target) {
        self.planned.conflicts.retain(|c| c.target != *target);
    }

    /// Records that `keeper` keeps `target`, which `lost` replaces later in load order.
    pub fn kept(&mut self, target: Target, keeper: &Origin, lost: &Origin) {
        let writers = self.writers.entry(target.clone()).or_default();
        writers.retain(|w| w != lost);
        if !writers.contains(keeper) {
            writers.push(keeper.clone());
        }
        self.overrides(&target, keeper, vec![lost.clone()]);
    }

    /// Records that `origin` puts the whole archive `erp` with a `file.put`: it
    /// writes every resource other ops write in it, and load order decides.
    pub fn put_archive(&mut self, erp: &str, origin: &Origin) {
        let inside: Vec<Target> = self
            .writers
            .keys()
            .filter(
                |t| matches!(t, Target::Resource { erp: e, .. } | Target::Slice { erp: e, .. } if e == erp),
            )
            .cloned()
            .collect();
        for target in inside {
            self.write(target, origin, true);
        }
    }

    /// The origin that writes any of `targets` last in load order.
    pub fn last_writer(&self, targets: &[Target]) -> Option<&Origin> {
        let rank = |o: &Origin| self.ranks.iter().position(|r| r == o);
        targets
            .iter()
            .filter_map(|t| self.writers.get(t))
            .flatten()
            .max_by_key(|o| rank(o))
    }

    pub fn features(&self) -> Vec<Kind> {
        self.features
            .experimental
            .intersection(self.opt_in)
            .copied()
            .collect()
    }

    pub fn has_db(&self) -> bool {
        !self.db.is_empty()
    }

    /// [`Planner::apply_db_report`] with `db` as the stock database.
    pub fn apply_db(&mut self, db: &mut Gvdb) -> Result<Vec<Added>> {
        let stock = db.records.iter().map(|r| r.key).collect();
        self.apply_db_report(db, &stock, &HashSet::new(), &mut DbReport::default())
    }

    /// Applies every addon's database ops in load order. A `remove-record` of a
    /// key in `stock` is left out unless the op allows it, and one of a key in
    /// `tracks`, the records track activation added, always; see
    /// [`Planner::take_stock_removals`] and [`Planner::take_track_removals`].
    ///
    /// An op whose selector matches no row waits, with the ops after it in its
    /// file that fail, and applies right after the first later addon whose ops
    /// add records, if its row exists then; the addons after that still override
    /// it. One still failing after every addon is skipped or fails the build, as
    /// the addon's other failing ops are.
    pub fn apply_db_report(
        &mut self,
        db: &mut Gvdb,
        stock: &HashSet<u64>,
        tracks: &HashSet<u64>,
        report: &mut DbReport,
    ) -> Result<Vec<Added>> {
        let fields = std::mem::take(&mut self.fields);
        let mut added = Vec::new();
        let pending = std::mem::take(&mut self.db);
        let row_ids = self.row_ids.take();
        if let Some(ids) = &row_ids {
            let mut ids = ids.borrow_mut();
            ids.building(pending.iter().map(|(o, _)| o.addon.clone()));
            ids.plan(
                db,
                stock,
                tracks,
                pending.iter().map(|(o, ops)| (o.addon.as_str(), ops)),
            );
        }
        let mut later = vec![HashSet::new(); pending.len()];
        for k in (1..pending.len()).rev() {
            let mut ids = later[k].clone();
            ids.extend(explicit_stat_ids(&pending[k].1));
            later[k - 1] = ids;
        }
        let shared = Shared {
            fields: &fields,
            stock,
            tracks,
            row_ids: row_ids.as_ref(),
        };
        let mut waiting = Vec::new();
        let mut grew: Vec<String> = Vec::new();
        for ((origin, ops), later) in pending.into_iter().zip(later) {
            let size = db.records.len();
            let names = shared.names(&origin.addon, &later);
            let (failed, rows) = self.apply_ops(db, &origin, &ops, &names, report, false);
            added.extend(rows);
            let since = grew.len();
            let adder = origin.addon.clone();
            waiting.extend(self.wait(origin, &ops, later, failed, since)?);
            if db.records.len() != size {
                grew.push(adder);
                self.retry(db, &mut waiting, &shared, report, &mut added, &grew)?;
            }
        }
        self.retry(db, &mut waiting, &shared, report, &mut added, &grew)?;
        for w in waiting {
            self.settle(&w.origin, w.errors, Some(db))?;
        }
        self.row_ids = row_ids;
        Ok(added)
    }

    fn retry(
        &mut self,
        db: &mut Gvdb,
        waiting: &mut Vec<Waiting>,
        shared: &Shared,
        report: &mut DbReport,
        added: &mut Vec<Added>,
        grew: &[String],
    ) -> Result<()> {
        for Waiting {
            origin,
            ops,
            later,
            since,
            ..
        } in std::mem::take(waiting)
        {
            let names = shared.names(&origin.addon, &later);
            let (failed, rows) = self.apply_ops(db, &origin, &ops, &names, report, true);
            added.extend(rows);
            if failed.len() < ops.ops.len() {
                for adder in grew[since.min(grew.len())..]
                    .iter()
                    .filter(|a| **a != origin.addon)
                {
                    self.waits.insert((origin.addon.clone(), adder.clone()));
                }
            }
            waiting.extend(self.wait(origin, &ops, later, failed, since)?);
        }
        Ok(())
    }

    // The failures before `failed`'s first selector that matches no row settle now;
    // that op and the failing ops after it wait.
    fn wait(
        &mut self,
        origin: Origin,
        ops: &DbOps,
        later: HashSet<(String, i64)>,
        mut failed: Vec<(usize, swofty_addon::Error)>,
        since: usize,
    ) -> Result<Option<Waiting>> {
        let at = failed.iter().position(|(_, e)| no_row(e)).unwrap_or(failed.len());
        let rest = failed.split_off(at);
        self.settle(&origin, failed.into_iter().map(|(_, e)| e).collect(), None)?;
        if rest.is_empty() {
            return Ok(None);
        }
        let (picked, errors): (Vec<usize>, Vec<_>) = rest.into_iter().unzip();
        let ops = DbOps {
            schema_version: ops.schema_version,
            fields: ops.fields.clone(),
            strict: ops.strict,
            side_data: ops.side_data.clone(),
            ops: picked.into_iter().map(|i| ops.ops[i].clone()).collect(),
        };
        Ok(Some(Waiting {
            origin,
            ops,
            later,
            errors,
            since,
        }))
    }

    // A lenient addon's failures are skipped with a warning; another's first fails the build.
    // An op on a row that only the core adds at the game's start cannot apply in the build: it is skipped with a warning.
    // So is any addon's op on a row of a track the build leaves out because the game's track lists are full.
    // `last` is the database after every addon's ops, once nothing is left to wait for.
    fn settle(
        &mut self,
        origin: &Origin,
        errors: Vec<swofty_addon::Error>,
        last: Option<&Gvdb>,
    ) -> Result<()> {
        let mut errors = errors;
        let no_id = errors.iter().filter(|e| ids_exhausted(e)).count();
        if no_id > 0 || (!errors.is_empty() && self.id_left_out.contains_key(&origin.addon)) {
            *self.id_left_out.entry(origin.addon.clone()).or_default() += no_id;
            self.skipped.extend(errors.iter().map(|e| {
                format!(
                    "{origin}: left out: {}; a change that depends on a row left out is left out too",
                    e.to_string().trim_start_matches("applying ")
                )
            }));
            return Ok(());
        }
        if last.is_some() {
            let (core, rest): (Vec<_>, Vec<_>) = errors
                .into_iter()
                .partition(|e| !self.core_row_adders(e).is_empty());
            for e in &core {
                let adders = self.core_row_adders(e).join(", ");
                self.skipped.push(format!(
                    "{origin}: change skipped: {e}; the core adds that row from {adders}'s records when the game starts, after the build"
                ));
            }
            errors = Vec::new();
            for e in rest {
                let Some(track) = self.left_out_track(&e).map(str::to_owned) else {
                    errors.push(e);
                    continue;
                };
                let name = self.name(&origin.addon).to_owned();
                self.skipped.push(format!(
                    "{name}: change skipped: {e}; it changes track {track}, which is left out of this build because \
                     the game has room for {} tracks. Turn some track addons off to make room for it",
                    crate::tracks::TRACK_LIST_ROOM
                ));
            }
        }
        let lines: Vec<String> = errors
            .iter()
            .map(|e| match last {
                Some(db) if no_row(e) => format!(
                    "{e}, even after every addon's changes{}",
                    self.removed_note(origin, e, db)
                ),
                _ => e.to_string(),
            })
            .collect();
        match self.lenient.get(&origin.addon) {
            Some(name) => {
                let name = name.clone();
                self.skipped
                    .extend(lines.iter().map(|l| format!("{name}: change skipped: {l}")));
                Ok(())
            }
            None => match lines.first() {
                Some(l) => Err(Error::Other(format!("applying {origin}: {l}"))),
                None => Ok(()),
            },
        }
    }

    // What to do about `origin`'s op that matches no row when an addon before it removed or renamed that row; empty when none did.
    fn removed_note(&self, origin: &Origin, e: &swofty_addon::Error, db: &Gvdb) -> String {
        let by = self.removers(origin, e, db);
        let Some((_, first)) = by.first() else {
            return String::new();
        };
        let verb = match by.iter().all(|(_, v)| v == first) {
            true => *first,
            false => "removed or renamed",
        };
        let names: Vec<&str> = by.iter().map(|(id, _)| self.name(id)).collect();
        let list = match names.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
            _ => names.join(""),
        };
        let them = match names.len() {
            1 => "it",
            _ => "them",
        };
        let edits = self.name(&origin.addon);
        format!(
            ": {edits} edits that row, but {list} {verb} it earlier in the load. Turn {list} off, or move {them} after {edits}"
        )
    }

    // The addons before `origin` in load order whose ops removed or renamed a row that `e`'s path selects, in load
    // order, each with what it did: removed the row, renamed it, or gave its name to another row on the way to it.
    fn removers(&self, origin: &Origin, e: &swofty_addon::Error, db: &Gvdb) -> Vec<(String, &'static str)> {
        let Some(mine) = self.ranks.iter().position(|o| o == origin) else {
            return Vec::new();
        };
        let earlier: Vec<&(usize, Rewrite)> = self
            .rewrites
            .iter()
            .filter(|(rank, _)| *rank < mine && self.ranks[*rank].addon != origin.addon)
            .collect();
        let selectors = no_row_selectors(e);
        let mut found: Vec<(usize, &'static str)> = Vec::new();
        for (n, s) in selectors.iter().enumerate() {
            let failing = n + 1 == selectors.len();
            let count_key = s.count_key();
            let counted = |r: &Rewrite| {
                [&r.was, &r.now]
                    .into_iter()
                    .filter_map(|v| v.as_deref()?.trim().parse::<usize>().ok())
                    .max()
            };
            let rows = earlier
                .iter()
                .filter(|(_, r)| Some(r.key) == count_key)
                .filter_map(|(_, r)| counted(r))
                .fold(4096, usize::max);
            let rows: HashSet<u64> = (0..rows).map(|i| s.field_key(i)).collect();
            let is = |v: &Option<String>| v.as_deref() == Some(s.want.as_str());
            for (rank, r) in &earlier {
                let did = match (rows.contains(&r.key), is(&r.was), is(&r.now)) {
                    (true, true, false) if r.now.is_none() => Some("removed"),
                    (true, true, false) => Some("renamed"),
                    (true, false, true) if !failing => Some("renamed"),
                    (false, ..) if failing && Some(r.key) == count_key && hid(r, s, db) => Some("removed"),
                    _ => None,
                };
                if let Some(did) = did {
                    found.push((*rank, did));
                }
            }
        }
        found.sort_by_key(|(rank, _)| *rank);
        let mut out: Vec<(String, &'static str)> = Vec::new();
        for (rank, did) in found {
            let addon = &self.ranks[rank].addon;
            if !out.iter().any(|(a, _)| a == addon) {
                out.push((addon.clone(), did));
            }
        }
        out
    }

    fn apply_ops(
        &mut self,
        db: &mut Gvdb,
        origin: &Origin,
        ops: &DbOps,
        names: &Reserving,
        report: &mut DbReport,
        late: bool,
    ) -> (Vec<(usize, swofty_addon::Error)>, Vec<Added>) {
        let rank = self.rank(origin);
        let before = owned::counts(db);
        let guarded = match late {
            true => self.later_records(db, origin),
            false => Vec::new(),
        };
        let mut ours = DbReport::default();
        let lists = self.lists.as_ref().filter(|l| l.touched(ops));
        let was = lists.map(|l| l.before(db));
        let (replaced, failed) = apply_db_ops_lenient(db, ops, names, &mut ours);
        report
            .lines
            .extend(ours.lines.into_iter().map(|l| format!("{}: {l}", origin.addon)));
        report.warnings.extend(
            ours.warnings
                .into_iter()
                .map(|w| format!("{}: {w}", origin.addon)),
        );
        self.rewrites.extend(ours.rewrites.into_iter().map(|r| (rank, r)));
        if ours.stock_removals > 0 {
            *self.stock_removals.entry(origin.addon.clone()).or_default() += ours.stock_removals;
        }
        if ours.track_removals > 0 {
            *self.track_removals.entry(origin.addon.clone()).or_default() += ours.track_removals;
        }
        if let (Some(lists), Some(was)) = (lists, was) {
            let kept = lists.missing(db, &was);
            if !kept.is_empty() {
                self.kept_lists
                    .entry(origin.addon.clone())
                    .or_default()
                    .extend(kept);
            }
        }
        self.copies.extend(ours.added_rows.into_iter().map(|r| Copy {
            addon: origin.addon.clone(),
            from: r.from,
            row: r.row,
            ignore: ops.side_data.ignore.clone(),
            quiet: false,
        }));
        for m in ours.moved_stat_ids {
            let Some(held) = &m.holder else {
                report.warnings.push(moved_stat_id_line(&origin.addon, None, &m));
                continue;
            };
            let copy = self.copies.iter().rev().find(|c| &c.row == held);
            // An unlock row's id moves off a stock row too; no addon added that row.
            let game = copy.is_none()
                && stat_row(held)
                    .rsplit_once('.')
                    .is_some_and(|(t, _)| unlock_list(t).is_some());
            let holder = copy.map_or_else(|| origin.addon.clone(), |c| c.addon.clone());
            let named = if game { "the game" } else { holder.as_str() };
            report
                .warnings
                .push(moved_stat_id_line(&origin.addon, Some(named), &m));
            if game {
                continue;
            }
            let keeper = Origin {
                addon: holder,
                item: None,
            };
            if keeper.addon != origin.addon {
                let field = m.field;
                let key = swofty_formats::hash::fnv1a64(format!("{held}.{field}"));
                let name = match &m.holder_label {
                    Some(l) => format!("{}[_metadata_={l}].{field}", stat_row(held)),
                    None => format!("{held}.{field}"),
                };
                self.planned.record_names.insert(key, name);
                self.kept(Target::Record(key), &keeper, origin);
            }
        }
        for key in replaced {
            self.write(Target::Record(key), origin, true);
        }
        restore_later_records(db, guarded);
        (failed, owned::added(db, &origin.addon, &before))
    }

    fn later_records(&self, db: &Gvdb, origin: &Origin) -> Vec<(usize, swofty_formats::gvdb::Record)> {
        let rank = |o: &Origin| self.ranks.iter().position(|r| r == o);
        let mine = rank(origin);
        let keys: HashSet<u64> = self
            .writers
            .iter()
            .filter_map(|(t, ws)| match t {
                Target::Record(k) if ws.iter().any(|w| rank(w) > mine) => Some(*k),
                _ => None,
            })
            .collect();
        db.records
            .iter()
            .enumerate()
            .filter(|(_, r)| keys.contains(&r.key))
            .map(|(i, r)| (i, r.clone()))
            .collect()
    }

    /// Gives the rows `add-row` gives an id of its own the ids `ids` keeps; see [`Planner::take_row_ids`].
    pub fn keep_row_ids(&mut self, ids: crate::row_ids::Store) {
        self.row_ids = Some(RefCell::new(ids));
    }

    /// The kept row ids, with the ids the last `apply_db` gave.
    pub fn take_row_ids(&mut self) -> Option<crate::row_ids::Store> {
        self.row_ids.take().map(RefCell::into_inner)
    }

    /// The addon whose write of the record `key` wins, the last in load order.
    pub fn record_writer(&self, key: u64) -> Option<&str> {
        self.writers
            .get(&Target::Record(key))?
            .iter()
            .max_by_key(|w| self.ranks.iter().position(|o| o == *w))
            .map(|w| w.addon.as_str())
    }

    /// The addons whose cars or teams the last `apply_db` left out for want of a free stat id, with how many
    /// of their rows got none.
    pub fn take_id_left_out(&mut self) -> BTreeMap<String, usize> {
        std::mem::take(&mut self.id_left_out)
    }

    /// Lenient addons' changes the last `apply_db` could not apply.
    pub fn take_skipped(&mut self) -> Vec<String> {
        std::mem::take(&mut self.skipped)
    }

    /// Per addon id, the removals of stock records the last `apply_db` left out.
    /// The (addon, other) pairs where `addon` had ops wait for rows that `other` added, so `addon`
    /// has to load below `other`.
    pub fn take_waits(&mut self) -> BTreeSet<(String, String)> {
        std::mem::take(&mut self.waits)
    }

    pub fn take_stock_removals(&mut self) -> BTreeMap<String, usize> {
        std::mem::take(&mut self.stock_removals)
    }

    /// Per addon id, the removals of records the tracks added that the last `apply_db` left out.
    pub fn take_track_removals(&mut self) -> BTreeMap<String, usize> {
        std::mem::take(&mut self.track_removals)
    }

    /// Watches the game's entrant `lists` for an addon's ops that rename or remove one; see [`Planner::take_kept_lists`].
    pub fn keep_entrant_lists(&mut self, lists: StockLists) {
        self.lists = Some(lists);
    }

    /// Per addon id, the entrant lists the last `apply_db` found the addon renaming or removing.
    pub fn take_kept_lists(&mut self) -> BTreeMap<String, Vec<String>> {
        std::mem::take(&mut self.kept_lists)
    }

    /// The `add-row` copies the last `apply_db` made, in load order.
    pub fn take_copies(&mut self) -> Vec<Copy> {
        std::mem::take(&mut self.copies)
    }

    pub fn erp_ops(&self) -> &[Op] {
        &self.erp
    }

    pub fn take_erp(&mut self) -> Vec<Op> {
        std::mem::take(&mut self.erp)
    }

    pub fn take_files(&mut self) -> Vec<Op> {
        std::mem::take(&mut self.files)
    }

    pub fn take_tracks(&mut self) -> Vec<Op> {
        std::mem::take(&mut self.tracks)
    }

    pub fn take_items(&mut self) -> Vec<Op> {
        std::mem::take(&mut self.items)
    }

    pub fn finish(self) -> Planned {
        self.planned
    }
}

pub fn waiting_lines(waiting: &[Op], features: &Features) -> Vec<String> {
    let mut counts: BTreeMap<(String, Kind), usize> = BTreeMap::new();
    for op in waiting {
        if let Some(kind) = op.op.kind() {
            *counts.entry((op.origin.to_string(), kind)).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .map(|((origin, kind), n)| match features.experimental.contains(&kind) {
            true => format!(
                "{origin}: {n} {kind} op(s) wait; the runtime {kind} feature is experimental and \"runtime_assets\" in config.json does not turn it on"
            ),
            false => format!("{origin}: {n} {kind} op(s) wait for {kind} support in swofty_core"),
        })
        .collect()
}

// Whether `r`, a write of `s`'s table's row count, lowered it past a row whose field `s` selects by still has its value.
fn hid(r: &Rewrite, s: &crate::ops::Selector, db: &Gvdb) -> bool {
    let count = |v: &Option<String>| v.as_deref().and_then(|v| v.trim().parse::<usize>().ok());
    let (Some(was), now) = (count(&r.was), count(&r.now).unwrap_or(0)) else {
        return false;
    };
    let hidden: HashSet<u64> = (now..was).map(|i| s.field_key(i)).collect();
    db.records
        .iter()
        .any(|rec| hidden.contains(&rec.key) && rec.text() == s.want.as_str())
}

fn restore_later_records(db: &mut Gvdb, guarded: Vec<(usize, swofty_formats::gvdb::Record)>) {
    if guarded.is_empty() {
        return;
    }
    let at: HashMap<u64, usize> = db.records.iter().enumerate().map(|(i, r)| (r.key, i)).collect();
    for (was, record) in guarded {
        match at.get(&record.key) {
            Some(&i) => db.records[i] = record,
            None => db.records.insert(was.min(db.records.len()), record),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::Source;

    fn origin(addon: &str) -> Origin {
        Origin {
            addon: addon.into(),
            item: None,
        }
    }

    fn op(addon: &str, json: &str) -> Op {
        Op {
            op: serde_json::from_str(json).unwrap(),
            origin: origin(addon),
        }
    }

    #[test]
    fn reads_features_and_routes_each_kind() {
        let dir = std::env::temp_dir().join(format!("swofty-features-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(load_features(&dir).unwrap(), Features::default());
        let path = dir.join(FEATURES_FILE);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{ "schema": 1, "features": { "group": "stable", "erp": "experimental", "db": "experimental",
                 "lang": "experimental", "file": "experimental", "audio": "stable", "tex": "soon" } }"#,
        )
        .unwrap();
        let features = load_features(&dir).unwrap();
        assert_eq!(features.stable, BTreeSet::from([Kind::Group]));
        assert_eq!(features.experimental.len(), 4);
        let opt_in = BTreeSet::from([Kind::Erp, Kind::Group]);
        let p = Planner::new(&features, &opt_in);
        assert_eq!(p.route(Kind::Erp), Route::Runtime);
        assert_eq!(p.route(Kind::Db), Route::Build);
        assert_eq!(p.route(Kind::File), Route::Build);
        assert_eq!(p.route(Kind::Lang), Route::Waiting);
        assert_eq!(p.route(Kind::Group), Route::Runtime);
        assert_eq!(p.features(), [Kind::Erp]);
        assert_eq!(
            waiting_lines(
                &[op(
                    "a",
                    r#"{"op":"lang.set","language":"*","key":"k","value":"v"}"#
                )],
                &features
            ),
            [
                "a: 1 lang op(s) wait; the runtime lang feature is experimental and \"runtime_assets\" in config.json does not turn it on"
            ]
        );
        std::fs::write(
            &path,
            r#"{ "schema": 1, "features": { "group": "stable", "erp": "stable", "db": "stable", "lang": "stable", "file": "experimental" } }"#,
        )
        .unwrap();
        let shipped = load_features(&dir).unwrap();
        let none = BTreeSet::new();
        let p = Planner::new(&shipped, &none);
        for kind in [Kind::Group, Kind::Erp, Kind::Db, Kind::Lang] {
            assert_eq!(p.route(kind), Route::Runtime);
        }
        assert_eq!(p.route(Kind::File), Route::Build);
        assert!(p.features().is_empty());
        let file = BTreeSet::from([Kind::File]);
        let p = Planner::new(&shipped, &file);
        assert_eq!(p.route(Kind::File), Route::Runtime);
        assert_eq!(p.features(), [Kind::File]);
        let forced = BTreeSet::from([Kind::Erp, Kind::Db, Kind::Lang]);
        let p = Planner::new(&shipped, &none).build_only(&forced);
        assert_eq!(p.route(Kind::Erp), Route::Build);
        assert_eq!(p.route(Kind::Db), Route::Build);
        assert_eq!(p.route(Kind::Lang), Route::Runtime);
        assert_eq!(p.route(Kind::Group), Route::Runtime);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn records_conflicts_for_replacing_writes_only() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in);
        let put = r#"{"op":"erp.put-resource","erp":"a.erp","resource":"r","source":"x.erp"}"#;
        let patch = r#"{"op":"erp.patch-pkg","erp":"a.erp","resource":"r","edits":[]}"#;
        p.push(op("a", put));
        p.push(op("b", patch));
        p.push(op("c", put));
        p.push(op("c", r#"{"op":"file.put","source":"f","target":"x/y.dds"}"#));
        p.push(op("d", r#"{"op":"file.put","source":"f","target":"x/y.dds"}"#));
        p.push(op(
            "a",
            r#"{"op":"lang.set","language":"*","key":"k","value":"v"}"#,
        ));
        p.push(op("a", r#"{"op":"warn","message":"m"}"#));
        assert_eq!(p.take_erp().len(), 4);
        let files = p.take_files();
        assert_eq!(files.len(), 2);
        assert!(
            matches!(&files[0].op, AssetOp::File(FileOp::Put { source, .. }) if *source == Source::addon("f"))
        );
        let planned = p.finish();
        assert_eq!(planned.waiting.len(), 1);
        assert_eq!(
            waiting_lines(&planned.waiting, &features),
            ["a: 1 lang op(s) wait for lang support in swofty_core"]
        );
        let shown: Vec<String> = planned.conflicts.iter().map(Conflict::to_string).collect();
        assert_eq!(shown, ["r in a.erp: c overrides a, b", "x/y.dds: d overrides c"]);
    }

    #[test]
    fn the_last_writer_of_a_resource_or_its_archive_comes_last_in_load_order() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let put = r#"{"op":"erp.put-resource","erp":"a.erp","resource":"r","source":"x.erp"}"#;
        let file = |erp: &str| format!(r#"{{"op":"file.put","source":"f","target":"{erp}"}}"#);
        let both = |erp: &str| {
            [
                Target::Resource {
                    erp: erp.into(),
                    uri: "r".into(),
                },
                Target::File(erp.into()),
            ]
        };
        let mut p = Planner::new(&features, &opt_in);
        p.push(op("a", put));
        p.push(op("b", &file("b.erp")));
        p.push(op("c", put));
        assert_eq!(p.last_writer(&both("a.erp")), Some(&origin("c")));
        assert_eq!(p.last_writer(&both("b.erp")), Some(&origin("b")));
        assert_eq!(p.last_writer(&both("c.erp")), None);
        p.push(op("d", &file("a.erp")));
        assert_eq!(p.last_writer(&both("a.erp")), Some(&origin("d")));
        p.push(op("a", &file("b.erp")));
        assert_eq!(p.last_writer(&both("b.erp")), Some(&origin("b")));
    }

    #[test]
    fn whole_resource_and_slice_writes_override_each_other_by_load_order() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let put = r#"{"op":"erp.put-resource","erp":"a.erp","resource":"r","source":"x.erp"}"#;
        let merge = r#"{"op":"erp.merge-slices","erp":"a.erp","resource":"r","source":"x.erp"}"#;
        let slice = |slice| Target::Slice {
            erp: "a.erp".into(),
            uri: "r".into(),
            slice,
        };
        let shown =
            |p: Planner| -> Vec<String> { p.finish().conflicts.iter().map(Conflict::to_string).collect() };

        let mut p = Planner::new(&features, &opt_in);
        p.push(op("a", merge));
        p.push(op("b", put));
        p.replaced(slice(1), &origin("a"));
        assert_eq!(shown(p), ["r in a.erp: b overrides a"]);

        let mut p = Planner::new(&features, &opt_in);
        p.push(op("a", put));
        p.push(op("b", merge));
        p.replaced(slice(1), &origin("b"));
        p.replaced(slice(2), &origin("b"));
        assert_eq!(
            shown(p),
            [
                "slice 1 of r in a.erp: b overrides a",
                "slice 2 of r in a.erp: b overrides a"
            ]
        );

        let mut p = Planner::new(&features, &opt_in);
        p.push(op("a", merge));
        p.replaced(slice(0), &origin("a"));
        p.push(op("b", put));
        p.push(op("c", merge));
        p.replaced(slice(0), &origin("c"));
        assert_eq!(
            shown(p),
            [
                "r in a.erp: b overrides a",
                "slice 0 of r in a.erp: c overrides b"
            ]
        );
    }

    #[test]
    fn a_resource_removal_puts_every_erp_op_on_the_build_route() {
        let features = Features {
            stable: BTreeSet::from([Kind::Erp, Kind::Db]),
            experimental: BTreeSet::new(),
        };
        let none = BTreeSet::new();
        let mut p = Planner::new(&features, &none);
        let remove = |r: &str| format!(r#"{{"op":"erp.remove-resource","erp":"a.erp","resource":"{r}"}}"#);
        p.push(op(
            "a",
            r#"{"op":"erp.put-resource","erp":"a.erp","resource":"r","source":"x.erp"}"#,
        ));
        p.push(op(
            "a",
            r#"{"op":"erp.add-mipmaps","source":"m","target":"x/r.tif.mipmaps"}"#,
        ));
        assert_eq!(p.route(Kind::Erp), Route::Runtime);
        assert_eq!(p.route_note(Kind::Erp), None);
        p.push(op("b", &remove("r")));
        p.push(op("b", &remove("s")));
        p.push(op(
            "b",
            r#"{"op":"file.put","source":"f","target":"x/r.tif.mipmaps"}"#,
        ));
        assert_eq!(p.route(Kind::Erp), Route::Build);
        assert_eq!(p.route(Kind::Db), Route::Runtime);
        assert_eq!(
            p.route_note(Kind::Erp).as_deref(),
            Some("built into the game files; 2 resource removal(s) cannot apply at runtime")
        );
        assert_eq!(p.route_note(Kind::Db), None);
        assert_eq!(p.take_erp().len(), 4);
        let shown: Vec<String> = p.finish().conflicts.iter().map(Conflict::to_string).collect();
        assert_eq!(
            shown,
            ["r in a.erp: b overrides a", "x/r.tif.mipmaps: b overrides a"]
        );
        let mut p = Planner::new(&features, &none).build_only(&features.stable);
        p.push(op("b", &remove("r")));
        assert_eq!(p.route(Kind::Erp), Route::Build);
        assert_eq!(p.route_note(Kind::Erp), None);
    }

    fn mipmap_conflicts(files: &[(&str, &[&str])], ops: &[(&str, &str)]) -> Vec<String> {
        let features = Features {
            stable: BTreeSet::from([Kind::Erp, Kind::Db]),
            experimental: BTreeSet::new(),
        };
        let none = BTreeSet::new();
        let names = |_: &Origin, src: &Source| -> Vec<String> {
            files
                .iter()
                .find(|(path, _)| *path == src.path)
                .map(|(_, names)| names.iter().map(|n| (*n).to_owned()).collect())
                .unwrap_or_default()
        };
        let mut p = Planner::new(&features, &none).mipmaps(&names);
        for (addon, source) in ops {
            p.push(op(
                addon,
                &format!(
                    r#"{{"op":"erp.add-mipmaps","source":"{source}","target":"environment_package/core_assets/objects/textures/main_front_end/names/"}}"#
                ),
            ));
        }
        p.finish().conflicts.iter().map(Conflict::to_string).collect()
    }

    #[test]
    fn folder_mipmaps_with_different_names_do_not_overlap() {
        let shown = mipmap_conflicts(
            &[("m", &["s0_a.mipmaps", "s0_b.mipmaps"]), ("n", &["s1_a.mipmaps"])],
            &[("a", "m"), ("b", "n")],
        );
        assert!(shown.is_empty(), "{shown:?}");
    }

    #[test]
    fn folder_mipmaps_with_the_same_name_overlap_once() {
        let shown = mipmap_conflicts(
            &[
                ("m", &["s0_a.mipmaps", "shared.mipmaps"]),
                ("n", &["s1_a.mipmaps", "shared.mipmaps"]),
            ],
            &[("a", "m"), ("b", "n")],
        );
        assert_eq!(
            shown,
            [
                "environment_package/core_assets/objects/textures/main_front_end/names/shared.mipmaps: b overrides a"
            ]
        );
    }

    fn lists() -> Gvdb {
        use swofty_formats::gvdb::Record;
        Gvdb {
            records: vec![
                Record::new("t._list_instancecount_", 0, "1"),
                Record::new("t.list_0._metadata_", 4, "f1"),
                Record::new("t.list_0.name", 4, "stock"),
                Record::new("t.list_0.colour", 4, "red"),
            ],
        }
    }

    fn set(row: &str, field: &str, value: &str) -> String {
        format!(
            r#"{{"op":"db.set","select":"t.list[_metadata_={row}]","field":"{field}","value":"{value}"}}"#
        )
    }

    fn add(label: &str) -> String {
        format!(
            r#"{{"op":"db.add-row","table":"t","from":"t.list[_metadata_=f1]","set":{{"_metadata_":"{label}","name":"{label}"}}}}"#
        )
    }

    fn list_fields(p: &mut Planner, addon: &str) {
        let names = ["_metadata_", "name", "colour"].map(str::to_owned).to_vec();
        p.add_fields(addon, BTreeMap::from([("t.list".to_owned(), names)]));
    }

    #[test]
    fn an_op_on_a_row_a_later_addon_adds_applies_after_that_addon() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let grid = Origin {
            addon: "simplified".into(),
            item: Some("grid-editor".into()),
        };
        let mut p = Planner::new(&features, &opt_in);
        list_fields(&mut p, "cs");
        for json in [
            set("cs_f1_2016", "name", "edited"),
            set("cs_f1_2016", "colour", "grid"),
            set("f1", "name", "grid"),
        ] {
            p.push(Op {
                op: serde_json::from_str(&json).unwrap(),
                origin: grid.clone(),
            });
        }
        p.push(op("cs", &add("cs_f1_2016")));
        p.push(op("cs", &set("f1", "name", "cs")));
        p.push(op("cs", &set("cs_f1_2016", "name", "cs")));
        p.push(op("late", &set("cs_f1_2016", "colour", "late")));
        let mut db = lists();
        p.apply_db(&mut db).unwrap();
        assert_eq!(db.get_text("t.list_1._metadata_").as_deref(), Some("cs_f1_2016"));
        assert_eq!(db.get_text("t.list_1.name").as_deref(), Some("cs"));
        assert_eq!(db.get_text("t.list_1.colour").as_deref(), Some("late"));
        assert_eq!(db.get_text("t.list_0.name").as_deref(), Some("cs"));
        assert!(p.take_skipped().is_empty());
        assert_eq!(
            p.take_waits(),
            BTreeSet::from([("simplified".to_owned(), "cs".to_owned())])
        );
        let key = |path: &str| Target::Record(swofty_formats::hash::fnv1a64(path));
        let winner = |p: &Planned, path: &str| {
            let c = p.conflicts.iter().find(|c| c.target == key(path)).unwrap();
            (
                c.winner.to_string(),
                c.overridden.iter().map(Origin::to_string).collect::<Vec<_>>(),
            )
        };
        let planned = p.finish();
        assert_eq!(
            winner(&planned, "t.list_0.name"),
            ("cs".to_owned(), vec!["simplified (grid-editor)".to_owned()])
        );
        assert_eq!(
            winner(&planned, "t.list_1.name"),
            ("cs".to_owned(), vec!["simplified (grid-editor)".to_owned()])
        );
        assert_eq!(
            winner(&planned, "t.list_1.colour"),
            ("late".to_owned(), vec!["simplified (grid-editor)".to_owned()])
        );
    }

    #[test]
    fn an_op_that_waits_never_overwrites_what_a_later_addon_wrote() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in);
        list_fields(&mut p, "cs");
        p.push(op("early", &set("cs_f1_2016", "name", "early")));
        p.push(op("early", &set("cs_f1_2016", "colour", "early")));
        p.push(op("cs", &add("cs_f1_2016")));
        p.push(op("cs", &set("cs_f1_2016", "name", "cs")));
        let mut db = lists();
        p.apply_db(&mut db).unwrap();
        assert_eq!(db.get_text("t.list_1.name").as_deref(), Some("cs"));
        assert_eq!(db.get_text("t.list_1.colour").as_deref(), Some("early"));
        let key = Target::Record(swofty_formats::hash::fnv1a64("t.list_1.name"));
        let planned = p.finish();
        let c = planned.conflicts.iter().find(|c| c.target == key).unwrap();
        assert_eq!(c.winner.to_string(), "cs");
        assert!(c.overridden.iter().any(|o| o.addon == "early"));
    }

    #[test]
    fn a_waiting_op_loses_every_record_a_later_addon_wrote_and_keeps_the_rest() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in);
        list_fields(&mut p, "cs");
        p.push(op("early", &set("cs_f1_2016", "name", "early")));
        p.push(op("early", &set("f1", "name", "early")));
        p.push(op("cs", &add("cs_f1_2016")));
        p.push(op("cs", &set("f1", "name", "cs")));
        let mut db = lists();
        p.apply_db(&mut db).unwrap();
        assert_eq!(db.get_text("t.list_0.name").as_deref(), Some("cs"));
        assert_eq!(db.get_text("t.list_1.name").as_deref(), Some("early"));
    }

    #[test]
    fn an_op_on_a_row_the_core_adds_at_the_start_is_skipped_with_a_warning() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let grid = Origin {
            addon: "simplified".into(),
            item: Some("grid-editor".into()),
        };
        let mut p = Planner::new(&features, &opt_in);
        list_fields(&mut p, "cs");
        p.add_core_rows("cs", ["t.list".to_owned()]);
        for json in [set("cs_f1_2020", "name", "edited"), set("f1", "name", "grid")] {
            p.push(Op {
                op: serde_json::from_str(&json).unwrap(),
                origin: grid.clone(),
            });
        }
        let mut db = lists();
        p.apply_db(&mut db).unwrap();
        assert_eq!(db.get_text("t.list_0.name").as_deref(), Some("grid"));
        assert_eq!(
            p.take_skipped(),
            [
                "simplified (grid-editor): change skipped: t.list[_metadata_=cs_f1_2020]: no row matches; the core adds that row from cs's records when the game starts, after the build"
            ]
        );

        let mut p = Planner::new(&features, &opt_in);
        list_fields(&mut p, "cs");
        p.add_core_rows("cs", ["t.other".to_owned(), "t.lis".to_owned()]);
        p.push(op("simplified", &set("cs_f1_2020", "name", "edited")));
        let e = p.apply_db(&mut lists()).unwrap_err();
        assert!(e.to_string().contains("no row matches, even after"), "{e}");
    }

    #[test]
    fn an_op_on_a_row_no_addon_adds_is_skipped_or_fails_as_before() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let lenient = BTreeMap::from([("edits".to_owned(), "My DB Edits".to_owned())]);
        let mut p = Planner::new(&features, &opt_in).lenient(lenient);
        list_fields(&mut p, "cs");
        p.push(op("edits", &set("gone", "name", "x")));
        p.push(op("edits", &set("f1", "name", "edits")));
        p.push(op("cs", &add("cs_f1_2016")));
        let mut db = lists();
        p.apply_db(&mut db).unwrap();
        assert_eq!(db.get_text("t.list_0.name").as_deref(), Some("edits"));
        assert_eq!(
            p.take_skipped(),
            [
                "My DB Edits: change skipped: t.list[_metadata_=gone]: no row matches, even after every addon's changes"
            ]
        );

        let mut p = Planner::new(&features, &opt_in);
        list_fields(&mut p, "cs");
        p.push(op("a", &set("gone", "name", "x")));
        p.push(op("cs", &add("cs_f1_2016")));
        let e = p.apply_db(&mut lists()).unwrap_err();
        assert_eq!(
            e.to_string(),
            "applying a: t.list[_metadata_=gone]: no row matches, even after every addon's changes"
        );

        let mut p = Planner::new(&features, &opt_in);
        list_fields(&mut p, "cs");
        p.push(op("a", &set("cs_f1_2016", "missing", "x")));
        p.push(op("cs", &add("cs_f1_2016")));
        let e = p.apply_db(&mut lists()).unwrap_err();
        assert_eq!(
            e.to_string(),
            "applying a: t.list[_metadata_=cs_f1_2016].missing: no such key"
        );
    }

    const SETS: &str = "track_data.game_setup";

    fn track_sets() -> Gvdb {
        use swofty_formats::gvdb::Record;
        let mut records = vec![Record::new(
            &format!("{SETS}._TrackDataSet_instancecount_"),
            0,
            "3",
        )];
        for (i, label) in ["f1_2026", "f2_2023", "f2_2026"].into_iter().enumerate() {
            let set = format!("{SETS}.TrackDataSet_{i}");
            records.push(Record::new(&format!("{set}._metadata_"), 4, label));
            records.push(Record::new(&format!("{set}._TrackData_instancecount_"), 0, "1"));
            records.push(Record::new(
                &format!("{set}.TrackData_0._metadata_"),
                4,
                "bahrain",
            ));
            records.push(Record::new(&format!("{set}.TrackData_0.round"), 0, "1"));
        }
        Gvdb { records }
    }

    fn put(path: &str, value: &str) -> String {
        let hex: String = value.bytes().map(|b| format!("{b:02x}")).collect();
        format!(r#"{{"op":"db.put-record","key":"{SETS}.{path}","type":4,"value":"{hex}"}}"#)
    }

    fn set_round(select: &str) -> String {
        format!(r#"{{"op":"db.set","select":"{SETS}.{select}","field":"round","value":"9"}}"#)
    }

    fn named(ids: &[(&str, &str)]) -> BTreeMap<String, String> {
        ids.iter()
            .map(|(id, name)| ((*id).to_owned(), (*name).to_owned()))
            .collect()
    }

    const SWAP: &str = "mod.serps-ineos-mercedes-concept";
    const KOREA: &str = "pajko.korea";

    fn swap_and_korea() -> BTreeMap<String, String> {
        named(&[(SWAP, "SERPS Ineos Mercedes Concept"), (KOREA, "Korea")])
    }

    #[test]
    fn a_row_an_earlier_import_renamed_names_the_import() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in).names(swap_and_korea());
        p.push(op("mod.other", &set_round("TrackDataSet_0.TrackData_0")));
        p.push(op(SWAP, &put("TrackDataSet_2._metadata_", "f2_2025")));
        p.push(op(
            KOREA,
            &set_round("TrackDataSet[_metadata_=f2_2026].TrackData[_metadata_=bahrain]"),
        ));
        let e = p.apply_db(&mut track_sets()).unwrap_err().to_string();
        assert_eq!(
            e,
            "applying pajko.korea: track_data.game_setup.TrackDataSet[_metadata_=f2_2026]: no row matches, \
             even after every addon's changes: Korea edits that row, but SERPS Ineos Mercedes Concept renamed it \
             earlier in the load. Turn SERPS Ineos Mercedes Concept off, or move it after Korea"
        );
    }

    #[test]
    fn rows_an_earlier_import_cut_from_the_count_name_the_import() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in).names(swap_and_korea());
        p.push(op(SWAP, &put("_TrackDataSet_instancecount_", "2")));
        p.push(op(
            KOREA,
            &set_round("TrackDataSet[_metadata_=f2_2026].TrackData_0"),
        ));
        let e = p.apply_db(&mut track_sets()).unwrap_err().to_string();
        assert!(
            e.ends_with(
                ": Korea edits that row, but SERPS Ineos Mercedes Concept removed it earlier in the load. \
                 Turn SERPS Ineos Mercedes Concept off, or move it after Korea"
            ),
            "{e}"
        );
        assert!(
            e.starts_with(
                "applying pajko.korea: track_data.game_setup.TrackDataSet[_metadata_=f2_2026]: no row matches"
            ),
            "{e}"
        );
    }

    #[test]
    fn a_child_row_under_a_table_an_earlier_import_renamed_names_the_import() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let names = named(&[
            (SWAP, "SERPS Ineos Mercedes Concept"),
            ("mod.lgsgp", "Laguna Seca Grand Prix"),
        ]);
        let mut p = Planner::new(&features, &opt_in).names(names);
        p.push(op(SWAP, &put("TrackDataSet_1._metadata_", "f2_2022")));
        p.push(op(SWAP, &put("TrackDataSet_2._metadata_", "f2_2023")));
        p.push(op(
            "mod.lgsgp",
            &put("TrackDataSet_1.TrackData_0._metadata_", "lgsgp"),
        ));
        p.push(op(
            "mod.lgsgp",
            &set_round("TrackDataSet[_metadata_=f2_2023].TrackData[_metadata_=lgsgp]"),
        ));
        let e = p.apply_db(&mut track_sets()).unwrap_err().to_string();
        assert!(
            e.ends_with(
                ": Laguna Seca Grand Prix edits that row, but SERPS Ineos Mercedes Concept renamed it earlier in the \
                 load. Turn SERPS Ineos Mercedes Concept off, or move it after Laguna Seca Grand Prix"
            ),
            "{e}"
        );
    }

    const CAREER: &str = "career.classic-career";

    #[test]
    fn any_addons_change_to_a_track_left_out_for_room_is_skipped_with_a_warning() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in).names(named(&[(CAREER, "Classic Career")]));
        p.leave_out_track("anderst", "Anderstorp");
        p.push(op(
            CAREER,
            &set_round("TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=anderst]"),
        ));
        p.push(op(
            CAREER,
            &set_round("TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=bahrain]"),
        ));
        let mut db = track_sets();
        p.apply_db(&mut db).unwrap();
        assert_eq!(
            db.get_text(&format!("{SETS}.TrackDataSet_0.TrackData_0.round"))
                .as_deref(),
            Some("9")
        );
        assert_eq!(
            p.take_skipped(),
            [
                "Classic Career: change skipped: track_data.game_setup.TrackDataSet[_metadata_=f1_2026]\
                 .TrackData[_metadata_=anderst]: no row matches in track_data.game_setup.TrackDataSet_0.TrackData; \
                 it changes track Anderstorp, which is left out of this build because the game has room for 64 \
                 tracks. Turn some track addons off to make room for it"
            ]
        );
    }

    #[test]
    fn a_change_to_a_missing_track_that_is_not_left_out_still_fails_the_build() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in).names(named(&[(CAREER, "Classic Career")]));
        p.leave_out_track("anderst", "Anderstorp");
        p.push(op(
            CAREER,
            &set_round("TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=imola]"),
        ));
        assert_eq!(
            p.apply_db(&mut track_sets()).unwrap_err().to_string(),
            "applying career.classic-career: track_data.game_setup.TrackDataSet[_metadata_=f1_2026]\
             .TrackData[_metadata_=imola]: no row matches in track_data.game_setup.TrackDataSet_0.TrackData, \
             even after every addon's changes"
        );

        let mut p = Planner::new(&features, &opt_in).names(named(&[(CAREER, "Classic Career")]));
        p.push(op(
            CAREER,
            &set_round("TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=anderst]"),
        ));
        assert_eq!(
            p.apply_db(&mut track_sets()).unwrap_err().to_string(),
            "applying career.classic-career: track_data.game_setup.TrackDataSet[_metadata_=f1_2026]\
             .TrackData[_metadata_=anderst]: no row matches in track_data.game_setup.TrackDataSet_0.TrackData, \
             even after every addon's changes"
        );
    }

    #[test]
    fn a_lenient_addons_skipped_change_names_the_import_too() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let lenient = named(&[("dbedit.mine", "My DB Edits")]);
        let mut p = Planner::new(&features, &opt_in)
            .lenient(lenient)
            .names(named(&[(SWAP, "A"), ("mod.second", "B")]));
        p.push(op(SWAP, &put("TrackDataSet_2._metadata_", "f2_2025")));
        p.push(op("mod.second", &put("_TrackDataSet_instancecount_", "2")));
        p.push(op(
            "dbedit.mine",
            &set_round("TrackDataSet[_metadata_=f2_2026].TrackData_0"),
        ));
        p.apply_db(&mut track_sets()).unwrap();
        assert_eq!(
            p.take_skipped(),
            [
                "My DB Edits: change skipped: track_data.game_setup.TrackDataSet[_metadata_=f2_2026]: no row matches, \
                 even after every addon's changes: My DB Edits edits that row, but A renamed it earlier in the load. \
                 Turn A off, or move it after My DB Edits"
            ]
        );
    }

    #[test]
    fn a_row_nothing_removed_or_only_a_later_addon_renamed_keeps_the_plain_error() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let plain = "applying pajko.korea: track_data.game_setup.TrackDataSet[_metadata_=f2_2030]: no row matches, \
                     even after every addon's changes";
        let mut p = Planner::new(&features, &opt_in).names(swap_and_korea());
        p.push(op(SWAP, &put("TrackDataSet_1._metadata_", "f2_2025")));
        p.push(op(
            KOREA,
            &set_round("TrackDataSet[_metadata_=f2_2030].TrackData_0"),
        ));
        assert_eq!(p.apply_db(&mut track_sets()).unwrap_err().to_string(), plain);

        let mut p = Planner::new(&features, &opt_in).names(swap_and_korea());
        p.push(op(
            KOREA,
            &set_round("TrackDataSet[_metadata_=f2_2026].TrackData_0"),
        ));
        p.push(op(SWAP, &put("TrackDataSet_2._metadata_", "f2_2030")));
        let mut db = track_sets();
        p.apply_db(&mut db).unwrap();
        assert_eq!(
            db.get_text(&format!("{SETS}.TrackDataSet_2.TrackData_0.round"))
                .as_deref(),
            Some("9")
        );

        let mut p = Planner::new(&features, &opt_in).names(swap_and_korea());
        p.push(op(KOREA, &put("TrackDataSet_2._metadata_", "f2_2030")));
        p.push(op(
            KOREA,
            &set_round("TrackDataSet[_metadata_=f2_2026].TrackData_0"),
        ));
        let e = p.apply_db(&mut track_sets()).unwrap_err().to_string();
        assert_eq!(e, plain.replace("f2_2030", "f2_2026"));
    }

    #[test]
    fn build_fields_apply_to_their_own_addon_only() {
        use swofty_formats::gvdb::Record;
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let add = r#"{"op":"db.add-row","table":"t","from":"t.r_0"}"#;
        let db = || Gvdb {
            records: vec![
                Record::new("t._r_instancecount_", 0, "1"),
                Record::new("t.r_0.a", 0, "7"),
            ],
        };
        let mut p = Planner::new(&features, &opt_in);
        p.add_fields("a", BTreeMap::from([("t.r".to_owned(), vec!["a".to_owned()])]));
        p.push(op("a", add));
        let mut built = db();
        p.apply_db(&mut built).unwrap();
        assert_eq!(built.get_text("t.r_1.a").as_deref(), Some("7"));
        let mut p = Planner::new(&features, &opt_in);
        p.add_fields("a", BTreeMap::from([("t.r".to_owned(), vec!["a".to_owned()])]));
        p.push(op("b", add));
        let e = p.apply_db(&mut db()).unwrap_err();
        assert!(e.to_string().contains("field names unknown"), "{e}");
    }

    #[test]
    fn a_stat_id_an_earlier_addon_pins_moves_and_counts_as_a_conflict() {
        use swofty_formats::gvdb::Record;
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let fields = || {
            BTreeMap::from([(
                "t.r".to_owned(),
                vec!["_metadata_".to_owned(), "stat_id".to_owned()],
            )])
        };
        let mut p = Planner::new(&features, &opt_in);
        p.add_fields("a", fields());
        p.add_fields("b", fields());
        p.push(op(
            "a",
            r#"{"op":"db.add-row","table":"t","from":"t.r_0","set":{"_metadata_":"a_row","stat_id":"6"}}"#,
        ));
        p.push(op(
            "b",
            r#"{"op":"db.add-row","table":"t","from":"t.r_0","set":{"_metadata_":"b_row","stat_id":"6"}}"#,
        ));
        let mut db = Gvdb {
            records: vec![
                Record::new("t._r_instancecount_", 0, "1"),
                Record::new("t.r_0._metadata_", 4, "base"),
                Record::new("t.r_0.stat_id", 0, "5"),
            ],
        };
        let stock = db.records.iter().map(|r| r.key).collect();
        let mut report = DbReport::default();
        p.apply_db_report(&mut db, &stock, &HashSet::new(), &mut report)
            .unwrap();
        assert_eq!(db.get_text("t.r_1.stat_id").as_deref(), Some("6"));
        assert_eq!(db.get_text("t.r_2.stat_id").as_deref(), Some("7"));
        assert!(
            report.warnings.iter().any(|w| w
                == "b: stat_id 6 of b_row is taken by a_row from a; the build gave it stat_id 7. The addon's author should leave stat_id out of add-row"),
            "{:?}",
            report.warnings
        );
        let planned = p.finish();
        assert_eq!(planned.conflicts.len(), 1);
        assert_eq!(
            planned.conflicts[0].target,
            Target::Record(swofty_formats::hash::fnv1a64("t.r_1.stat_id"))
        );
        assert_eq!(planned.conflicts[0].winner, origin("a"));
        assert_eq!(planned.conflicts[0].overridden, [origin("b")]);
        assert_eq!(
            planned.record_names.values().collect::<Vec<_>>(),
            ["t.r[_metadata_=a_row].stat_id"]
        );
        let (pairs, _) = crate::conflicts::summarise("p", &planned.conflicts, &|_| None);
        assert_eq!(
            (
                pairs.pairs[0].winner.as_str(),
                pairs.pairs[0].loser.as_str(),
                pairs.pairs[0].database
            ),
            ("a", "b", 1)
        );
    }

    #[test]
    fn a_free_stat_id_of_an_earlier_addon_leaves_the_one_a_later_addon_pins() {
        use swofty_formats::gvdb::Record;
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let fields = || {
            BTreeMap::from([(
                "t.r".to_owned(),
                vec!["_metadata_".to_owned(), "stat_id".to_owned()],
            )])
        };
        let mut p = Planner::new(&features, &opt_in);
        p.add_fields("a", fields());
        p.add_fields("b", fields());
        p.push(op(
            "a",
            r#"{"op":"db.add-row","table":"t","from":"t.r_0","set":{"_metadata_":"a_row"}}"#,
        ));
        p.push(op(
            "b",
            r#"{"op":"db.add-row","table":"t","from":"t.r_0","set":{"_metadata_":"b_row","stat_id":"6"}}"#,
        ));
        let mut db = Gvdb {
            records: vec![
                Record::new("t._r_instancecount_", 0, "1"),
                Record::new("t.r_0._metadata_", 4, "base"),
                Record::new("t.r_0.stat_id", 0, "5"),
            ],
        };
        let stock = db.records.iter().map(|r| r.key).collect();
        let mut report = DbReport::default();
        p.apply_db_report(&mut db, &stock, &HashSet::new(), &mut report)
            .unwrap();
        assert_eq!(db.get_text("t.r_1.stat_id").as_deref(), Some("7"));
        assert_eq!(db.get_text("t.r_2.stat_id").as_deref(), Some("6"));
        assert!(
            !report.warnings.iter().any(|w| w.contains("is taken by")),
            "{:?}",
            report.warnings
        );
        assert!(p.finish().conflicts.is_empty());
    }

    #[test]
    fn a_package_stat_id_above_the_byte_ceiling_moves_with_a_warning() {
        use swofty_formats::gvdb::Record;
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let t = "entrants.vehicles.vehicle_packages";
        let mut p = Planner::new(&features, &opt_in);
        p.add_fields(
            "a",
            BTreeMap::from([(
                format!("{t}.vehicle_package"),
                vec!["_metadata_".to_owned(), "stat_id".to_owned()],
            )]),
        );
        p.push(op(
            "a",
            &format!(
                r#"{{"op":"db.add-row","table":"{t}","from":"{t}.vehicle_package_0","set":{{"_metadata_":"era","stat_id":"300"}}}}"#
            ),
        ));
        let mut db = Gvdb {
            records: vec![
                Record::new(&format!("{t}._vehicle_package_instancecount_"), 0, "1"),
                Record::new(&format!("{t}.vehicle_package_0._metadata_"), 4, "base"),
                Record::new(&format!("{t}.vehicle_package_0.stat_id"), 0, "58"),
            ],
        };
        let stock = db.records.iter().map(|r| r.key).collect();
        let mut report = DbReport::default();
        p.apply_db_report(&mut db, &stock, &HashSet::new(), &mut report)
            .unwrap();
        assert_eq!(
            db.get_text(&format!("{t}.vehicle_package_1.stat_id")).as_deref(),
            Some("59")
        );
        assert_eq!(
            report.warnings,
            [
                "a: stat_id 300 of era is above 254, the highest the game reads in entrants.vehicles.vehicle_packages.vehicle_package; the build gave it stat_id 59. The addon's author should leave stat_id out of add-row"
            ]
        );
        assert!(p.finish().conflicts.is_empty());
    }

    #[test]
    fn a_scenario_stat_id_an_earlier_addon_holds_moves_and_counts_as_a_conflict() {
        use swofty_formats::gvdb::Record;
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let t = "game_setup.scenarios";
        let fields = || {
            BTreeMap::from([(
                format!("{t}.scenario"),
                vec!["_metadata_".to_owned(), "statId".to_owned()],
            )])
        };
        let mut p = Planner::new(&features, &opt_in);
        p.add_fields("cs", fields());
        p.add_fields("era", fields());
        p.add_fields("cs22", fields());
        let add = |label: &str, id: u32| {
            format!(
                r#"{{"op":"db.add-row","table":"{t}","from":"{t}.scenario[_metadata_=f1]","set":{{"_metadata_":"{label}","statId":"{id}"}}}}"#
            )
        };
        p.push(op("cs", &add("cs_f1_2021", 20)));
        p.push(op("era", &add("era_2012", 20)));
        p.push(op(
            "era",
            &format!(r#"{{"op":"db.set","select":"{t}.scenario[statId=20]","field":"_metadata_","value":"era_found"}}"#),
        ));
        p.push(op("cs22", &add("cs_f1_2022", 21)));
        let mut db = Gvdb {
            records: vec![
                Record::new(&format!("{t}._scenario_instancecount_"), 0, "2"),
                Record::new(&format!("{t}.scenario_0._metadata_"), 4, "f1"),
                Record::new(&format!("{t}.scenario_0.statId"), 0, "14"),
                Record::new(&format!("{t}.scenario_1._metadata_"), 4, "f1_alt"),
                Record::new(&format!("{t}.scenario_1.statId"), 0, "14"),
            ],
        };
        let stock = db.records.iter().map(|r| r.key).collect();
        let mut report = DbReport::default();
        p.apply_db_report(&mut db, &stock, &HashSet::new(), &mut report)
            .unwrap();
        assert_eq!(
            db.get_text(&format!("{t}.scenario_2.statId")).as_deref(),
            Some("20")
        );
        assert_eq!(
            db.get_text(&format!("{t}.scenario_3.statId")).as_deref(),
            Some("22")
        );
        assert_eq!(
            db.get_text(&format!("{t}.scenario_4.statId")).as_deref(),
            Some("21")
        );
        assert_eq!(
            db.get_text(&format!("{t}.scenario_3._metadata_")).as_deref(),
            Some("era_found")
        );
        assert_eq!(
            report.warnings,
            [
                "era: statId 20 of era_2012 is taken by cs_f1_2021 from cs; the build gave it statId 22. The addon's author should give the row a statId no other addon uses"
            ]
        );
        let planned = p.finish();
        assert_eq!(planned.conflicts.len(), 1);
        assert_eq!(
            planned.conflicts[0].target,
            Target::Record(swofty_formats::hash::fnv1a64(format!("{t}.scenario_2.statId")))
        );
        assert_eq!(
            planned.record_names.values().collect::<Vec<_>>(),
            ["game_setup.scenarios.scenario[_metadata_=cs_f1_2021].statId"]
        );
    }

    #[test]
    fn database_files_keep_their_schema_and_report_replaced_records() {
        let features = Features::default();
        let opt_in = BTreeSet::new();
        let mut p = Planner::new(&features, &opt_in);
        let empty: OpFile = serde_json::from_str(r#"{"schema_version":1,"ops":[]}"#).unwrap();
        p.push_file(&origin("a"), FileKind::Database, empty.clone());
        p.push_file(&origin("a"), FileKind::Assets, empty);
        assert_eq!(p.db.len(), 1);
        let key = "rules.x";
        p.push(op(
            "a",
            &format!(r#"{{"op":"db.put-record","key":"{key}","type":0,"value":"31"}}"#),
        ));
        p.push(op(
            "b",
            &format!(r#"{{"op":"db.put-record","key":"{key}","type":0,"value":"32"}}"#),
        ));
        assert_eq!(p.db.len(), 2);
        let mut db = Gvdb::default();
        p.apply_db(&mut db).unwrap();
        assert_eq!(db.get_text(key).as_deref(), Some("2"));
        let planned = p.finish();
        assert_eq!(planned.conflicts.len(), 1);
        assert_eq!(planned.conflicts[0].winner, origin("b"));
        assert_eq!(planned.conflicts[0].overridden, [origin("a")]);
    }
}
