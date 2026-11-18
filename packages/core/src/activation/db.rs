use std::collections::BTreeMap;

use serde::Deserialize;
use swofty_formats::gvdb::Gvdb;

use super::dbx::{Db, at, field, key, row, sub_count, sub_table};
use crate::map::{Override, TrackPackage};
use crate::ops::first_segment;
use crate::{Error, Result};

pub const REGISTRY_JSON: &str = include_str!("track_tables.json");

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Schema {
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default)]
    pub tables: BTreeMap<String, Schema>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TableSpec {
    pub id: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub count: Option<String>,
    #[serde(default = "default_labels")]
    pub labels: Vec<String>,
    #[serde(default)]
    pub unique: Vec<String>,
    #[serde(default)]
    pub schema: Schema,
}

fn default_labels() -> Vec<String> {
    vec!["_metadata_".into()]
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ListSpec {
    pub table: TableSpec,
    pub sub_table: String,
    pub field: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Registry {
    pub track: TableSpec,
    pub datasets: TableSpec,
    pub track_data: TableSpec,
    #[serde(default)]
    pub per_track: Vec<TableSpec>,
    #[serde(default)]
    pub sorted_orders: Vec<TableSpec>,
    #[serde(default)]
    pub track_lists: Vec<ListSpec>,
    #[serde(default)]
    pub track_resources: Option<TableSpec>,
}

impl Registry {
    pub fn builtin() -> Self {
        serde_json::from_str(REGISTRY_JSON).expect("track_tables.json is valid")
    }
}

fn hex(s: &str) -> Result<u64> {
    u64::from_str_radix(s.trim_start_matches("0x"), 16).map_err(|_| Error::Resolve(format!("bad hash {s}")))
}

impl TableSpec {
    pub fn base(&self) -> Result<u64> {
        match (&self.path, &self.base) {
            (_, Some(b)) => hex(b),
            (Some(p), None) => Ok(key(&format!("{p}_"))),
            _ => Err(Error::Resolve(format!("{}: needs path or base", self.id))),
        }
    }

    pub fn count_key(&self) -> Result<Option<u64>> {
        match (&self.path, &self.count) {
            (_, Some(c)) => hex(c).map(Some),
            (Some(p), None) => Ok(p
                .rsplit_once('.')
                .map(|(container, row)| key(&format!("{container}._{row}_instancecount_")))),
            _ => Ok(None),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Table {
    pub base: u64,
    pub count: Option<u64>,
}

impl Table {
    pub fn of(spec: &TableSpec) -> Result<Self> {
        Ok(Self {
            base: spec.base()?,
            count: spec.count_key()?,
        })
    }

    fn nested(parent_row: u64, name: &str) -> Self {
        Self {
            base: sub_table(parent_row, name),
            count: Some(sub_count(parent_row, name)),
        }
    }

    fn len(&self, db: &Db) -> usize {
        match self.count {
            Some(c) => db.count(c).unwrap_or(0),
            None => (0..)
                .take_while(|&i| db.has(field(row(self.base, i), "_metadata_")))
                .count(),
        }
    }

    fn row_of(&self, db: &Db, label: &str, label_field: &str) -> Option<u64> {
        (0..self.len(db))
            .map(|i| row(self.base, i))
            .find(|&r| db.text(field(r, label_field)).as_deref() == Some(label))
    }

    fn push(&self, db: &mut Db) -> Result<u64> {
        let n = self.len(db);
        let count = self
            .count
            .filter(|&c| db.has(c))
            .ok_or_else(|| Error::Resolve(format!("{:016x}: table has no row count", self.base)))?;
        db.set_text(count, &(n + 1).to_string(), "row count")?;
        Ok(row(self.base, n))
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
}

impl Report {
    fn note(&mut self, s: String) {
        self.lines.push(s);
    }
}

pub fn clone_row(db: &mut Db, schema: &Schema, src: u64, dst: u64) -> usize {
    let mut n = 0;
    for f in &schema.fields {
        if db.copy(field(src, f), field(dst, f)) {
            n += 1;
        }
    }
    for (name, sub) in &schema.tables {
        let (from, to) = (sub_count(src, name), sub_count(dst, name));
        let Some(count) = db.count(from) else { continue };
        db.copy(from, to);
        n += 1;
        for i in 0..count {
            n += clone_row(
                db,
                sub,
                row(sub_table(src, name), i),
                row(sub_table(dst, name), i),
            );
        }
    }
    n
}

fn relabel(db: &mut Db, row_key: u64, labels: &[String], code: &str) -> Result<()> {
    for l in labels {
        let k = field(row_key, l);
        if db.has(k) {
            db.set_text(k, code, l)?;
        }
    }
    Ok(())
}

fn fresh_unique(db: &mut Db, table: Table, dst: u64, unique: &[String]) -> Result<()> {
    for u in unique {
        let max = (0..table.len(db))
            .map(|i| row(table.base, i))
            .filter(|&r| r != dst)
            .filter_map(|r| db.int(field(r, u)))
            .max()
            .unwrap_or(-1);
        let k = field(dst, u);
        if db.has(k) {
            db.set_text(k, &(max + 1).to_string(), u)?;
        }
    }
    Ok(())
}

fn with_paths(schema: &Schema, paths: &[&str]) -> Schema {
    let mut s = schema.clone();
    for p in paths {
        if !s.fields.iter().any(|f| f == p) && !p.contains(['*', '[']) {
            s.fields.push((*p).to_owned());
        }
    }
    s
}

fn copy_paths(db: &mut Db, src: u64, dst: u64, paths: &[&str]) {
    for p in paths {
        if !p.contains(['*', '[']) {
            db.copy(field(src, p), field(dst, p));
        }
    }
}

fn add_row_from(db: &mut Db, spec: &TableSpec, table: Table, src: u64, code: &str) -> Result<u64> {
    let dst = table.push(db)?;
    clone_row(db, &spec.schema, src, dst);
    relabel(db, dst, &spec.labels, code)?;
    fresh_unique(db, table, dst, &spec.unique)?;
    Ok(dst)
}

fn pick(db: &Db, row_key: u64, name: &str, by: &str, want: &str) -> Result<Option<u64>> {
    let table = Table::nested(row_key, name);
    let hits: Vec<u64> = (0..table.len(db))
        .map(|i| row(table.base, i))
        .filter(|&r| db.has(field(r, "_metadata_")) && db.text(field(r, by)).as_deref() == Some(want))
        .collect();
    match hits.as_slice() {
        [] => Ok(None),
        [r] => Ok(Some(*r)),
        _ => Err(Error::Resolve(format!(
            "{name}[{by}={want}]: {} rows match",
            hits.len()
        ))),
    }
}

fn expand(db: &Db, row_key: u64, path: &str) -> Result<Vec<u64>> {
    let (seg, tail) = first_segment(path)?;
    if let Some((by, want)) = seg.select {
        let tail = tail.ok_or_else(|| Error::Resolve(format!("{path}: names a row, not a field")))?;
        return match pick(db, row_key, seg.name, by, want)? {
            Some(r) => expand(db, r, tail),
            None => Ok(Vec::new()),
        };
    }
    let Some(tail) = tail else {
        let k = field(row_key, path);
        return Ok(if db.has(k) { vec![k] } else { Vec::new() });
    };
    let head = seg.name;
    if let Some((name, idx)) = head.rsplit_once('_')
        && (idx == "*" || idx.parse::<usize>().is_ok())
        && let Some(n) = db.count(sub_count(row_key, name))
    {
        let rows: Vec<usize> = match idx.parse::<usize>() {
            Ok(i) if i < n => vec![i],
            Ok(_) => Vec::new(),
            Err(_) => (0..n).collect(),
        };
        let mut keys = Vec::new();
        for i in rows {
            keys.extend(expand(db, row(sub_table(row_key, name), i), tail)?);
        }
        return Ok(keys);
    }
    if tail.contains('[') {
        return expand(db, at(row_key, &format!(".{head}")), tail);
    }
    let k = field(row_key, path);
    Ok(if db.has(k) { vec![k] } else { Vec::new() })
}

pub const WEEKEND_TABLE: &str = "TrackAuthenticWeekendData";

pub fn weekend_format(db: &Db, data_row: u64) -> Option<String> {
    db.text(field(
        row(sub_table(data_row, WEEKEND_TABLE), 0),
        "weekend_format",
    ))
}

fn set_weekend_format(db: &mut Db, data_row: u64, format: &str) -> Result<bool> {
    let n = db.count(sub_count(data_row, WEEKEND_TABLE)).unwrap_or(0);
    let mut set = false;
    for i in 0..n {
        let k = field(row(sub_table(data_row, WEEKEND_TABLE), i), "weekend_format");
        if db.has(k) {
            db.set_text(k, format, "weekend_format")?;
            set = true;
        }
    }
    Ok(set)
}

pub struct Dataset {
    pub label: String,
    pub table: Table,
}

pub fn datasets(db: &Db, reg: &Registry) -> Result<Vec<Dataset>> {
    let t = Table::of(&reg.datasets)?;
    let n = t.len(db);
    if n == 0 {
        return Err(Error::Resolve("the dataset table is missing or empty".into()));
    }
    let name = reg
        .track_data
        .path
        .as_deref()
        .ok_or_else(|| Error::Resolve("track_data needs a row name in path".into()))?;
    Ok((0..n)
        .map(|i| {
            let r = row(t.base, i);
            Dataset {
                label: db.text(field(r, "_metadata_")).unwrap_or_default(),
                table: Table::nested(r, name),
            }
        })
        .collect())
}

pub const RESOURCE_GROUP_FIELDS: &[&str] = &[
    "worldAssetGroup",
    "aoBakeAssetGroup",
    "radianceBakeAssetGroup",
    "lightListAssetGroup",
    "ddgiAssetGroup",
    "trackMapAssetGroup",
];

pub enum TrackHost<'a> {
    Slot {
        asset_group: &'a str,
        asset_root: &'a str,
        key: &'a str,
    },
    Natural,
}

fn natural_group(code: &str, field: &str) -> String {
    let rel = match field {
        "worldAssetGroup" => format!("wep/{code}_common"),
        "aoBakeAssetGroup" => format!("bake/{code}_ao.bake"),
        "radianceBakeAssetGroup" => format!("bake/{code}_radiance.bake"),
        "lightListAssetGroup" => format!("bake/{code}_light_list.bake"),
        "ddgiAssetGroup" => "probes/config.ddgi".into(),
        _ => code.into(),
    };
    format!("asset_groups/environment_package/tracks/{code}/{rel}.assetgroup")
}

fn route_resource(
    db: &mut Db,
    reg: &Registry,
    track_row: u64,
    code: &str,
    host: &TrackHost,
    report: &mut Report,
) -> Result<()> {
    let spec = reg
        .track_resources
        .as_ref()
        .ok_or_else(|| Error::Resolve("the registry has no track resource table".into()))?;
    let t = Table::of(spec)?;
    let current = db
        .text(field(track_row, "track_resource"))
        .ok_or_else(|| Error::Resolve(format!("{code}: track row has no track_resource")))?;
    let label = format!("swofty_{code}");
    let src = t
        .row_of(db, &current, "_metadata_")
        .or_else(|| t.row_of(db, "2019", "_metadata_"))
        .ok_or_else(|| Error::Resolve(format!("track resource {current} not found")))?;
    let dst = match t.row_of(db, &label, "_metadata_") {
        Some(r) => r,
        None => t.push(db)?,
    };
    if dst != src {
        clone_row(db, &spec.schema, src, dst);
    }
    db.set_text(field(dst, "_metadata_"), &label, "_metadata_")?;
    let (root, key, from) = match host {
        TrackHost::Slot {
            asset_group,
            asset_root,
            key,
        } => {
            for f in RESOURCE_GROUP_FIELDS {
                let k = field(dst, f);
                if db.text(k).is_some_and(|v| !v.is_empty()) {
                    db.set_text(k, asset_group, f)?;
                }
            }
            (asset_root.to_string(), key.to_string(), asset_group.to_string())
        }
        TrackHost::Natural => {
            for f in RESOURCE_GROUP_FIELDS {
                db.set_text(field(dst, f), &natural_group(code, f), f)?;
            }
            (
                format!("asset_groups/environment_package/tracks/{code}/probes/"),
                "key_2019.assetgroup".to_string(),
                natural_group(code, ""),
            )
        }
    };
    db.set_text(field(dst, "iblAssetGroupsRoot"), &root, "iblAssetGroupsRoot")?;
    db.set_text(field(dst, "iblKey"), &key, "iblKey")?;
    db.set_text(field(track_row, "track_resource"), &label, "track_resource")?;
    report.note(format!(
        "track resource {label} (from {current}) loads {code} from {from}"
    ));
    Ok(())
}

fn point_textures(
    db: &mut Db,
    keys: Vec<u64>,
    code: &str,
    lists: &std::collections::BTreeSet<String>,
) -> Result<usize> {
    let mut n = 0;
    for k in keys {
        let Some(v) = db.text(k) else { continue };
        let Some((list, _)) = v.strip_prefix("overrides.").and_then(|r| r.rsplit_once('.')) else {
            continue;
        };
        if lists.contains(list) {
            let to = format!("overrides.{list}.{code}");
            if to != v {
                db.set_text(k, &to, "TrackTextures")?;
                n += 1;
            }
        }
    }
    Ok(n)
}

/// Picks the stat id of a track row the build adds: the table's highest plus one, and whether a row holds an id; gives
/// the id and what to log.
pub type StatIds<'a> =
    &'a mut dyn FnMut(i64, &dyn Fn(i64) -> bool) -> std::result::Result<(i64, String), String>;

pub fn activate(
    gvdb: &mut Gvdb,
    pkg: &TrackPackage,
    reg: &Registry,
    route: Option<&TrackHost>,
    ui_lists: &std::collections::BTreeSet<String>,
) -> Result<Report> {
    activate_with(gvdb, pkg, reg, route, ui_lists, None)
}

pub fn activate_with(
    gvdb: &mut Gvdb,
    pkg: &TrackPackage,
    reg: &Registry,
    route: Option<&TrackHost>,
    ui_lists: &std::collections::BTreeSet<String>,
    ids: Option<StatIds>,
) -> Result<Report> {
    let mut db = Db::new(gvdb);
    let mut report = Report::default();
    let code = pkg.track_code.as_str();
    let from = pkg.base_track()?;
    let slot = pkg.activation.copy_slot.as_deref().unwrap_or(from);
    let tracks = Table::of(&reg.track)?;
    let label = |s: &TableSpec| s.labels.first().cloned().unwrap_or_else(|| "_metadata_".into());
    let src_track = tracks
        .row_of(&db, from, &label(&reg.track))
        .ok_or_else(|| Error::Map(format!("{from} is not a track in this game")))?;
    let track_row = match tracks.row_of(&db, code, &label(&reg.track)) {
        Some(r) => {
            report.note(format!("{code}: uses its existing track slot"));
            r
        }
        None => {
            let slot_row = tracks
                .row_of(&db, slot, &label(&reg.track))
                .ok_or_else(|| Error::Map(format!("copy_slot {slot} is not a track in this game")))?;
            let paths: Vec<&str> = pkg
                .activation
                .track_overrides
                .iter()
                .map(|o| o.path.as_str())
                .collect();
            let spec = TableSpec {
                schema: with_paths(&reg.track.schema, &paths),
                ..reg.track.clone()
            };
            let r = add_row_from(&mut db, &spec, tracks, slot_row, code)?;
            report.note(format!("{code}: new track row cloned from {slot}"));
            if let (Some(ids), Some(u)) = (ids, spec.unique.first()) {
                let rows: Vec<u64> = (0..tracks.len(&db))
                    .map(|i| row(tracks.base, i))
                    .filter(|&o| o != r)
                    .collect();
                let taken: std::collections::BTreeSet<i64> =
                    rows.iter().filter_map(|&o| db.int(field(o, u))).collect();
                let top = taken.iter().max().map_or(0, |m| m + 1);
                let (id, why) = ids(top, &|id| taken.contains(&id)).map_err(Error::Resolve)?;
                db.set_text(field(r, u), &id.to_string(), u)?;
                report.note(format!("{code}: {why}"));
            }
            r
        }
    };
    let _ = src_track;
    if let Some(route) = route {
        route_resource(&mut db, reg, track_row, code, route, &mut report)?;
    }
    for spec in &reg.per_track {
        let t = Table::of(spec)?;
        if t.len(&db) == 0 {
            report
                .warnings
                .push(format!("{}: table not found in this database", spec.id));
            continue;
        }
        let l = label(spec);
        if t.row_of(&db, code, &l).is_some() {
            continue;
        }
        let Some(src) = t.row_of(&db, slot, &l).or_else(|| t.row_of(&db, from, &l)) else {
            continue;
        };
        add_row_from(&mut db, spec, t, src, code)?;
        report.note(format!("{}: added {code}", spec.id));
    }
    let sets = datasets(&db, reg)?;
    let mut in_sets = Vec::new();
    let mut sprint_sets = Vec::new();
    for ds in &sets {
        let Some(src) = ds.table.row_of(&db, from, "track") else {
            continue;
        };
        let held = ds.table.row_of(&db, code, "track");
        let own = held.and_then(|r| weekend_format(&db, r));
        let dst = match held {
            Some(r) => r,
            None => ds.table.push(&mut db)?,
        };
        clone_row(&mut db, &reg.track_data.schema, src, dst);
        let paths: Vec<&str> = pkg.activation.overrides.iter().map(|o| o.path.as_str()).collect();
        copy_paths(&mut db, src, dst, &paths);
        relabel(&mut db, dst, &reg.track_data.labels, code)?;
        if dst != src {
            let sprint = pkg.activation.sprint.contains(&ds.label);
            let format = match sprint {
                true => "sprint".to_owned(),
                false => own.unwrap_or_else(|| "default".to_owned()),
            };
            if set_weekend_format(&mut db, dst, &format)? && sprint {
                sprint_sets.push(ds.label.clone());
            }
        }
        in_sets.push(ds.label.clone());
    }
    report.note(format!(
        "track data: {code} cloned from {from} in {} dataset(s): {}",
        in_sets.len(),
        in_sets.join(", ")
    ));
    if !sprint_sets.is_empty() {
        report.note(format!(
            "weekend format: {code} runs sprint weekends in {}",
            sprint_sets.join(", ")
        ));
    }
    for set in pkg.activation.sprint.iter().filter(|s| !sprint_sets.contains(s)) {
        report.warnings.push(format!(
            "sprint {set}: {code} has no weekend format in that data set, so it takes its parent data set's"
        ));
    }
    for spec in &reg.sorted_orders {
        let t = Table::of(spec)?;
        let l = label(spec);
        if t.row_of(&db, code, &l).is_some() {
            continue;
        }
        if let Some(src) = t.row_of(&db, from, &l) {
            add_row_from(&mut db, spec, t, src, code)?;
            report.note(format!("{}: appended {code}", spec.id));
        }
    }
    for list in &reg.track_lists {
        let t = Table::of(&list.table)?;
        for i in 0..t.len(&db) {
            let set = row(t.base, i);
            let inner = Table::nested(set, &list.sub_table);
            for j in 0..inner.len(&db) {
                let k = field(row(inner.base, j), &list.field);
                let Some(v) = db.text(k) else { continue };
                let items: Vec<&str> = v.split(',').filter(|s| !s.is_empty()).collect();
                if items.contains(&from) && !items.contains(&code) {
                    let mut items = items.clone();
                    items.push(code);
                    let joined = items.join(",");
                    db.set_text(k, &joined, &list.table.id)?;
                    let set_name = db.text(field(set, "_metadata_")).unwrap_or_default();
                    report.note(format!("{} {set_name}: added {code}", list.sub_table));
                }
            }
        }
    }
    if !ui_lists.is_empty() {
        let mut keys = Vec::new();
        for ds in &sets {
            if let Some(r) = ds.table.row_of(&db, code, "track") {
                for f in reg
                    .track_data
                    .schema
                    .tables
                    .get("TrackTextures")
                    .map_or(&[][..], |s| &s.fields[..])
                {
                    keys.extend(expand(&db, r, &format!("TrackTextures_*.{f}"))?);
                }
            }
        }
        keys.extend(expand(&db, track_row, "TrackTextures_*.TrackTexture_*.value")?);
        let n = point_textures(&mut db, keys, code, ui_lists)?;
        report.note(format!("{code}: {n} texture path(s) point at its own pictures"));
    }
    apply_overrides(&mut db, &sets, pkg, &mut report)?;
    for o in &pkg.activation.track_overrides {
        let keys = expand(&db, track_row, &o.path)
            .map_err(|e| Error::Map(format!("track override {}: {e}", o.path)))?;
        if keys.is_empty() {
            report
                .warnings
                .push(format!("track override {}: no such field", o.path));
        }
        for k in keys {
            db.set_text(k, &o.value, &o.path)?;
        }
    }
    report.note(format!(
        "database: {} record(s) added, {} changed",
        db.added, db.changed
    ));
    Ok(report)
}

fn dataset_matches(o: &Override, label: &str) -> bool {
    o.datasets.is_empty() || o.datasets.iter().any(|d| d == label)
}

fn apply_overrides(db: &mut Db, sets: &[Dataset], pkg: &TrackPackage, report: &mut Report) -> Result<()> {
    let code = pkg.track_code.as_str();
    for o in &pkg.activation.overrides {
        let mut hit = 0;
        for ds in sets.iter().filter(|d| dataset_matches(o, &d.label)) {
            let Some(r) = ds.table.row_of(db, code, "track") else {
                continue;
            };
            let keys = expand(db, r, &o.path)
                .map_err(|e| Error::Map(format!("override {} in {}: {e}", o.path, ds.label)))?;
            for k in keys {
                db.set_text(k, &o.value, &o.path)?;
                hit += 1;
            }
        }
        if hit == 0 {
            report.warnings.push(format!(
                "override {} ({}): no matching field",
                o.path,
                if o.datasets.is_empty() {
                    "all datasets".to_owned()
                } else {
                    o.datasets.join(",")
                }
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use swofty_formats::gvdb::Record;

    #[test]
    fn override_paths_take_selectors() {
        let mut gvdb = Gvdb {
            records: vec![Record::new("t.Row_0.Weather._Slot_instancecount_", 0, "3")],
        };
        for (i, label) in ["dry", "wet", "wet"].iter().enumerate() {
            for (f, v) in [("_metadata_", *label), ("rain", &*i.to_string())] {
                gvdb.records
                    .push(Record::new(&format!("t.Row_0.Weather.Slot_{i}.{f}"), 0, v));
            }
        }
        gvdb.records
            .push(Record::new("t.Row_0.Weather.Slot_0.Sub._metadata_", 0, "x"));
        let db = Db::new(&mut gvdb);
        let r = key("t.Row_0");
        let want = vec![key("t.Row_0.Weather.Slot_0.rain")];
        assert_eq!(expand(&db, r, "Weather.Slot[_metadata_=dry].rain").unwrap(), want);
        assert_eq!(expand(&db, r, "Weather.Slot[rain=0].rain").unwrap(), want);
        assert!(
            expand(&db, r, "Weather.Slot[_metadata_=snow].rain")
                .unwrap()
                .is_empty()
        );
        let err = expand(&db, r, "Weather.Slot[_metadata_=wet].rain").unwrap_err();
        assert_eq!(err.to_string(), "Slot[_metadata_=wet]: 2 rows match");
        assert!(expand(&db, r, "Weather.Slot[_metadata_=dry]").is_err());
        assert_eq!(expand(&db, r, "Weather.Slot_0.rain").unwrap(), want);
    }
}
