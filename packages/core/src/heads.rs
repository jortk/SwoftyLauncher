//! Driver heads that addons put into the game folder. A head archive whose
//! label the database has no driver head row for, such as an imported face
//! mod, gets a row copied from a stock head and its asset groups registered
//! when a profile builds, so drivers can wear it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use swofty_addon::asset::{AssetOp, FileOp, GroupOp, Op, OpFile, Origin, Root};
use swofty_addon::manifest::AddonManifest;
use swofty_addon::ops::{DbOp, StatIdMode};
use swofty_formats::erp::ErpFile;
use swofty_formats::gvdb::Gvdb;
use swofty_formats::hash::fnv1a64;

pub use crate::launcher::{ProfileHead, ProfileHeads};
use crate::store::Store;

pub const TABLE: &str = "entrants.animation.driver_heads";
pub const ROWS: &str = "entrants.animation.driver_heads.driver_head";
const ROW: &str = "driver_head";
const REGISTRY: &str = "driver.head";
const ARCHIVES: &str = "2025_asset_groups/animation_package/scenes/cinematics/drivers";
const GROUPS: &str = "asset_groups/animation_package/scenes/cinematics/drivers";
// Rows no other table has a row of the same label for, so copies raise no side-data findings.
const MALE: &str = "male_ls222";
const FEMALE: &str = "female_ls112";
// Group templates, each with a copy of its groups for the male rig.
const MALE_GROUPS: &str = "lando_norris";
const FEMALE_GROUPS: &str = "female_ls493";
const MALE_RIG: &str = "_malerig";
const FEMALE_MODELS: &str = "eaid://[CHARACTER_PACKAGE]/heads/female_";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Part {
    Head,
    HeadMaleRig,
    Morph,
    MorphMaleRig,
}

impl Part {
    const ALL: [Part; 4] = [Part::Head, Part::HeadMaleRig, Part::Morph, Part::MorphMaleRig];

    fn dir(self) -> &'static str {
        match self {
            Part::Head | Part::HeadMaleRig => "heads",
            Part::Morph | Part::MorphMaleRig => "morph_poses",
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Part::HeadMaleRig | Part::MorphMaleRig => MALE_RIG,
            Part::Head | Part::Morph => "",
        }
    }

    fn group(self, label: &str) -> String {
        format!("{GROUPS}/{}/{label}{}.assetgroup", self.dir(), self.suffix())
    }

    fn archive(self, label: &str) -> String {
        format!("{ARCHIVES}/{}/{label}{}.erp", self.dir(), self.suffix())
    }
}

fn plain(label: &str) -> bool {
    !label.is_empty()
        && !label.starts_with('.')
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
}

/// The head label and archive a game-folder path holds, such as
/// `rio_haryanto` for `.../drivers/heads/rio_haryanto.erp`.
fn part_of(target: &str) -> Option<(String, Part)> {
    let target = target.replace('\\', "/").to_ascii_lowercase();
    let rest = target
        .trim_start_matches('/')
        .strip_prefix(ARCHIVES)?
        .strip_prefix('/')?;
    let (dir, file) = rest.split_once('/')?;
    let stem = file.strip_suffix(".erp")?;
    let (label, rig) = match stem.strip_suffix(MALE_RIG) {
        Some(l) if !l.is_empty() => (l, true),
        _ => (stem, false),
    };
    let part = match (dir, rig) {
        ("heads", false) => Part::Head,
        ("heads", true) => Part::HeadMaleRig,
        ("morph_poses", false) => Part::Morph,
        ("morph_poses", true) => Part::MorphMaleRig,
        _ => return None,
    };
    plain(label).then(|| (label.to_owned(), part))
}

/// The row `model_path` that names a head model resource, such as
/// `eaid://[CHARACTER_PACKAGE]/heads/male_driver/x/idf/x_rig_head_morph_v1`
/// for `eaid://character_package/heads/male_driver/x/idf/x_rig_head_morph_v1.idf?model`.
fn model_path(resource: &str) -> Option<String> {
    let rest = resource
        .strip_prefix("eaid://character_package/")?
        .strip_suffix(".idf?model")?;
    (!rest.contains("/kitpart_")).then(|| format!("eaid://[CHARACTER_PACKAGE]/{rest}"))
}

/// The head model `erp` holds, as a row's `model_path`.
pub fn model(erp: &Path) -> Option<String> {
    let file = ErpFile::open(erp).ok()?;
    file.erp.resources.iter().find_map(|r| model_path(&r.name))
}

/// The labels of `db`'s driver head rows.
pub fn labels(db: &Gvdb) -> BTreeSet<String> {
    let n = db.instance_count(TABLE, ROW).unwrap_or(0);
    let want: std::collections::HashSet<u64> = (0..n)
        .map(|i| fnv1a64(format!("{ROWS}_{i}._metadata_")))
        .collect();
    db.records
        .iter()
        .filter(|r| want.contains(&r.key))
        .map(|r| r.text().into_owned())
        .collect()
}

/// The tables whose rows wear a head, by their `head` field.
const WEARERS: [(&str, &str); 2] = [
    ("entrants.drivers", "driver"),
    ("entrants.characters", "character"),
];

/// A row whose `head` names no driver head row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unworn {
    /// The `head` field's key, for who wrote it.
    pub key: u64,
    pub row: String,
    pub head: String,
    /// The game's own head, put back; none for a row the game doesn't have.
    pub kept: Option<String>,
}

impl Unworn {
    pub fn line(&self, by: &str) -> String {
        match &self.kept {
            Some(stock) => format!(
                "{by}: {} wears head {}, which no addon on in this profile adds, so it keeps the game's head {stock}",
                self.row, self.head
            ),
            None => format!(
                "{by}: {} wears head {}, which no addon on in this profile adds; the game may stop when it loads the driver",
                self.row, self.head
            ),
        }
    }
}

/// The `head` field key of every row of the tables that wear a head, with the row's path.
fn head_fields(db: &Gvdb) -> HashMap<u64, String> {
    let mut fields = HashMap::new();
    for (container, row) in WEARERS {
        for i in 0..db.instance_count(container, row).unwrap_or(0) {
            fields.insert(
                fnv1a64(format!("{container}.{row}_{i}.head")),
                format!("{container}.{row}_{i}"),
            );
        }
    }
    fields
}

fn head_values(db: &Gvdb, fields: &HashMap<u64, String>) -> HashMap<u64, String> {
    db.records
        .iter()
        .filter(|r| fields.contains_key(&r.key))
        .map(|r| (r.key, r.text().into_owned()))
        .collect()
}

/// Puts the game's own head back on every row of `db` whose changed `head`
/// names no driver head row, such as a head from a face mod that is now off.
pub fn keep_wearable(db: &mut Gvdb, shipped: &Gvdb) -> Vec<Unworn> {
    let heads = labels(db);
    let fields = head_fields(db);
    let (now, stock) = (head_values(db, &fields), head_values(shipped, &fields));
    let mut out: Vec<Unworn> = now
        .into_iter()
        .filter(|(key, head)| !head.is_empty() && !heads.contains(head) && stock.get(key) != Some(head))
        .map(|(key, head)| {
            let row = db
                .get_text(&format!("{}._metadata_", fields[&key]))
                .unwrap_or_else(|| fields[&key].clone());
            Unworn {
                key,
                row,
                head,
                kept: stock.get(&key).cloned(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.row.cmp(&b.row));
    for u in &out {
        if let Some(stock) = &u.kept
            && let Some(r) = db.records.iter_mut().find(|r| r.key == u.key)
        {
            r.value = stock.clone().into_bytes();
        }
    }
    out
}

/// The head a row the game ships headless wears: a generic male head of the game's, which no driver wears.
pub const STAND_IN: &str = MALE;

/// A row whose `head`, as the game ships it, names no driver head row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Headless {
    pub row: String,
    pub head: String,
    /// The head it wears now; empty when the database has no [`STAND_IN`] row either.
    pub worn: String,
}

impl Headless {
    pub fn line(&self) -> String {
        let worn = match self.worn.is_empty() {
            true => "its head is left empty".to_owned(),
            false => format!("it wears the game's head {}", self.worn),
        };
        format!(
            "database: {} wears head {}, which the game has no driver head row for, so {worn} (the game stops at \
             start-up when an addon puts the driver in an entrant list)",
            self.row, self.head
        )
    }
}

/// Gives [`STAND_IN`] to every row of `db` whose `head` is still the game's
/// own and names no driver head row of `db` and none of `added`, the heads
/// the core adds in game. The game ships `nico_rosberg` so and never loads
/// him while he is unavailable; an addon's entrant list that names him makes
/// him available, and the game then reads his head with no check for a head
/// it cannot find.
pub fn wear_stand_ins(db: &mut Gvdb, shipped: &Gvdb, added: &BTreeSet<String>) -> Vec<Headless> {
    let heads = labels(db);
    let fields = head_fields(db);
    let (now, stock) = (head_values(db, &fields), head_values(shipped, &fields));
    let worn = match heads.contains(STAND_IN) {
        true => STAND_IN.to_owned(),
        false => String::new(),
    };
    let mut found: Vec<(String, u64, String)> = now
        .into_iter()
        .filter(|(key, head)| {
            !head.is_empty() && !heads.contains(head) && !added.contains(head) && stock.get(key) == Some(head)
        })
        .map(|(key, head)| {
            let row = db
                .get_text(&format!("{}._metadata_", fields[&key]))
                .unwrap_or_else(|| fields[&key].clone());
            (row, key, head)
        })
        .collect();
    found.sort();
    for (_, key, _) in &found {
        if let Some(r) = db.records.iter_mut().find(|r| r.key == *key) {
            r.value = worn.clone().into_bytes();
        }
    }
    found
        .into_iter()
        .map(|(row, _, head)| Headless {
            row,
            head,
            worn: worn.clone(),
        })
        .collect()
}

/// The labels of the driver head rows the core adds in game from an addon's
/// data file `text`: each `driver.head` record's `label`, or its key.
pub fn registry_labels(text: &str) -> BTreeSet<String> {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(text) else {
        return BTreeSet::new();
    };
    map.get(REGISTRY)
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(key, _)| !key.contains('/') && !key.starts_with('$'))
        .map(|(key, rec)| {
            rec.get("label")
                .and_then(serde_json::Value::as_str)
                .filter(|l| !l.is_empty())
                .unwrap_or(key)
                .to_owned()
        })
        .collect()
}

/// The labels of the driver head rows the core adds in game for the addons
/// of `order`, from the data files of those with `driver.head` records.
pub fn registry_heads(
    addons_dir: &Path,
    order: &[String],
    std: &BTreeMap<String, crate::stdlib::deps::Std>,
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for id in order {
        let Some(s) = std
            .get(id)
            .filter(|s| s.records.iter().any(|r| r.registry == REGISTRY))
        else {
            continue;
        };
        for rel in &s.data_files {
            if let Ok(text) = std::fs::read_to_string(addons_dir.join(id).join(rel)) {
                out.extend(registry_labels(&text));
            }
        }
    }
    out
}

/// A head an addon puts: its `heads/<label>.erp` and the archives beside it.
#[derive(Debug, Clone, PartialEq)]
pub struct Head {
    pub label: String,
    /// The addon that puts the head archive.
    pub addon: String,
    archives: BTreeMap<Part, PathBuf>,
}

/// A head's row models: `model_path`, `model_path_female_rig`, and whether it is female.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Models {
    model: String,
    female_rig: String,
    female: bool,
}

impl Head {
    /// A female head's model sits under `heads/female_*`, and it may come with
    /// a copy of each archive for the male rig.
    fn models(&self) -> Option<Models> {
        let head = model(&self.archives[&Part::Head])?;
        let rig = match self.archives.get(&Part::HeadMaleRig) {
            Some(path) => Some(model(path)?),
            None => None,
        };
        let female = rig.is_some() || head.starts_with(FEMALE_MODELS);
        Some(match (female, rig) {
            (false, _) => Models {
                model: head,
                female_rig: String::new(),
                female,
            },
            (true, rig) => Models {
                model: rig.unwrap_or_default(),
                female_rig: head,
                female,
            },
        })
    }
}

/// What a profile's addons put and add for driver heads, op by op in load order.
#[derive(Debug, Default)]
pub struct Puts {
    archives: BTreeMap<(String, Part), (String, PathBuf)>,
    rows: BTreeSet<String>,
    groups: BTreeSet<String>,
}

impl Puts {
    /// Notes `op`, which `addon` gives; `dir` is where its sources of each root live.
    pub fn op(&mut self, addon: &str, op: &AssetOp, dir: &dyn Fn(Root) -> PathBuf) {
        match op {
            AssetOp::File(FileOp::Put { source, target }) => {
                if let Some(key) = part_of(target)
                    && let Ok(path) = source.resolve(&dir(source.root))
                {
                    self.archives.insert(key, (addon.to_owned(), path));
                }
            }
            AssetOp::Db(DbOp::AddRow { table, set, .. }) if table == TABLE => {
                if let Some(label) = set.get("_metadata_") {
                    self.rows.insert(label.clone());
                }
            }
            AssetOp::Group(GroupOp::Register { group, .. }) => {
                self.groups.insert(group.to_ascii_lowercase());
            }
            _ => {}
        }
    }

    /// Notes every op an installed addon's op files and manifest give.
    pub fn addon(&mut self, store: &Store, id: &str, m: &AddonManifest) {
        let dir = store.addons_dir().join(id);
        let roots = |root: Root| crate::build::source_dir(store, id, root);
        for (kind, rel) in m.operations.op_files() {
            let Some(file) = std::fs::read(dir.join(rel))
                .ok()
                .and_then(|d| serde_json::from_slice(&d).ok())
                .and_then(|doc| OpFile::from_value(doc, kind).ok())
            else {
                continue;
            };
            for op in &file.ops {
                self.op(id, op, &roots);
            }
        }
        for op in m.operations.file_ops() {
            self.op(id, &op, &roots);
        }
    }

    /// The registrations addons already make, lowercased.
    pub fn registered(&self) -> &BTreeSet<String> {
        &self.groups
    }

    /// Whether an addon puts a head archive no addon adds a row for.
    pub fn pending(&self) -> bool {
        self.archives
            .keys()
            .any(|(label, part)| *part == Part::Head && !self.rows.contains(label))
    }

    fn heads(&self) -> Vec<Head> {
        let mut heads: BTreeMap<&str, Head> = BTreeMap::new();
        for ((label, part), (addon, _)) in &self.archives {
            if *part == Part::Head {
                heads.insert(
                    label,
                    Head {
                        label: label.clone(),
                        addon: addon.clone(),
                        archives: BTreeMap::new(),
                    },
                );
            }
        }
        for ((label, part), (_, path)) in &self.archives {
            if let Some(h) = heads.get_mut(label.as_str()) {
                h.archives.insert(*part, path.clone());
            }
        }
        heads.into_values().collect()
    }

    /// The heads put that neither `stock` nor any addon has a row for: the ones a build adds.
    pub fn new_heads(&self, stock: &BTreeSet<String>) -> Vec<Head> {
        self.heads()
            .into_iter()
            .filter(|h| !stock.contains(&h.label) && !self.rows.contains(&h.label))
            .collect()
    }

    /// Every head put that drivers can wear once the profile builds: one an
    /// addon adds a row for, or one a build adds a row for.
    pub fn wearable(&self, stock: &BTreeSet<String>, names: &HashMap<String, String>) -> Vec<ProfileHead> {
        self.heads()
            .into_iter()
            .filter_map(|h| {
                let models = h.models();
                let has_row = stock.contains(&h.label) || self.rows.contains(&h.label);
                (has_row || models.is_some()).then(|| ProfileHead {
                    female: models.is_some_and(|m| m.female),
                    addon_name: names.get(&h.addon).cloned(),
                    label: h.label,
                    addon: Some(h.addon),
                })
            })
            .collect()
    }
}

/// What [`ops`] gives a build.
#[derive(Debug, Default)]
pub struct Added {
    pub ops: Vec<Op>,
    /// (addon, line) for the build's lines.
    pub lines: Vec<(String, String)>,
    /// (addon, warning) for the build's warnings.
    pub warnings: Vec<(String, String)>,
}

/// An add-row copying a stock head and the group registrations for each of
/// `heads`, given by the addon that puts it. A head is left out, with a
/// warning, when its archive names no head model or `stock` lacks the head to copy.
pub fn ops(heads: &[Head], stock: &BTreeSet<String>, registered: &BTreeSet<String>) -> Added {
    let mut out = Added::default();
    for h in heads {
        let archive = Part::Head.archive(&h.label);
        let Some(models) = h.models() else {
            out.warnings.push((
                h.addon.clone(),
                format!("{archive} gets no driver head row: it holds no head model"),
            ));
            continue;
        };
        let (template, groups_from) = match models.female {
            true => (FEMALE, FEMALE_GROUPS),
            false => (MALE, MALE_GROUPS),
        };
        if !stock.contains(template) {
            out.warnings.push((
                h.addon.clone(),
                format!("{archive} gets no driver head row: the game has no {template} head to copy"),
            ));
            continue;
        }
        let morph = h.archives.contains_key(&Part::Morph);
        let flag = |on: bool| if on { "True" } else { "False" }.to_owned();
        let set = BTreeMap::from([
            ("_metadata_".to_owned(), h.label.clone()),
            ("model_path".to_owned(), models.model),
            ("model_path_female_rig".to_owned(), models.female_rig),
            ("has_morph_data".to_owned(), flag(morph)),
            ("available_for_customisation".to_owned(), flag(false)),
        ]);
        let origin = Origin {
            addon: h.addon.clone(),
            item: None,
        };
        out.ops.push(Op {
            op: AssetOp::Db(DbOp::AddRow {
                table: TABLE.to_owned(),
                from: format!("{ROWS}[_metadata_={template}]"),
                set,
                copy: None,
                stat_id: Some(StatIdMode::Fresh),
            }),
            origin: origin.clone(),
        });
        let mut groups = 0;
        for part in Part::ALL.into_iter().filter(|p| h.archives.contains_key(p)) {
            let group = part.group(&h.label);
            if registered.contains(&group.to_ascii_lowercase()) {
                continue;
            }
            groups += 1;
            out.ops.push(Op {
                op: AssetOp::Group(GroupOp::Register {
                    group,
                    erp: Some(part.archive(&h.label)),
                    erps: Vec::new(),
                    deps: Vec::new(),
                    template: Some(part.group(groups_from)),
                }),
                origin: origin.clone(),
            });
        }
        let kind = if models.female { "female" } else { "male" };
        out.lines.push((
            h.addon.clone(),
            format!(
                "adds driver head {} ({kind}, copied from {template}) for {archive} and registers {groups} asset group(s)",
                h.label
            ),
        ));
    }
    out
}

fn stock_labels(store: &Store, game: &crate::install::Game) -> BTreeSet<String> {
    let path = crate::build::original(&store.root, game, crate::build::GVDB);
    std::fs::read(&path)
        .ok()
        .and_then(|b| Gvdb::parse(&b).ok())
        .map(|db| labels(&db))
        .unwrap_or_default()
}

/// The heads the addons on in `profile` put that drivers can wear.
pub fn for_profile(store: &Store, game: &crate::install::Game, profile: &str) -> crate::Result<ProfileHeads> {
    let p = store.profile(profile)?;
    let on: BTreeSet<&str> = p.enabled().collect();
    let mut puts = Puts::default();
    let mut names = HashMap::new();
    for (id, m) in store.addons()? {
        if on.contains(id.as_str()) {
            puts.addon(store, &id, &m);
            names.insert(id, m.name);
        }
    }
    if puts.archives.is_empty() {
        return Ok(ProfileHeads::default());
    }
    Ok(ProfileHeads {
        heads: puts.wearable(&stock_labels(store, game), &names),
    })
}

/// The heads installed addons put that drivers can wear, for the DB Editor,
/// which offers the ones its stock heads lack beside them.
pub fn installed(store: &Store) -> Vec<ProfileHead> {
    let Ok(addons) = store.addons() else {
        return Vec::new();
    };
    let mut puts = Puts::default();
    let mut names = HashMap::new();
    for (id, m) in addons {
        puts.addon(store, &id, &m);
        names.insert(id, m.name);
    }
    puts.wearable(&BTreeSet::new(), &names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_head_archives_from_game_folder_paths() {
        let at = |rel: &str| part_of(&format!("{ARCHIVES}/{rel}"));
        assert_eq!(
            at("heads/rio_haryanto.erp"),
            Some(("rio_haryanto".into(), Part::Head))
        );
        assert_eq!(
            at("heads/female_ls493_malerig.erp"),
            Some(("female_ls493".into(), Part::HeadMaleRig))
        );
        assert_eq!(
            at("morph_poses/Rio_Haryanto.erp"),
            Some(("rio_haryanto".into(), Part::Morph))
        );
        assert_eq!(
            part_of("\\2025_asset_groups\\animation_package\\scenes\\cinematics\\drivers\\heads\\x.erp"),
            Some(("x".into(), Part::Head))
        );
        assert_eq!(at("heads/x.erp.bak"), None);
        assert_eq!(at("helmets/x.erp"), None);
        assert_eq!(at("heads/_malerig.erp"), Some(("_malerig".into(), Part::Head)));
        assert_eq!(at("heads/a b.erp"), None);
    }

    #[test]
    fn turns_a_model_resource_into_a_row_model_path() {
        assert_eq!(
            model_path("eaid://character_package/heads/male_driver/rio_haryanto/idf/rio_haryanto_rig_head_morph_v1.idf?model")
                .as_deref(),
            Some("eaid://[CHARACTER_PACKAGE]/heads/male_driver/rio_haryanto/idf/rio_haryanto_rig_head_morph_v1")
        );
        assert_eq!(
            model_path("eaid://character_package/heads/male_driver/x/idf/kitpart_hair/x_hair.idf?model"),
            None
        );
        assert_eq!(
            model_path("eaid://character_package/heads/male_driver/x/idf/x_rig_head_morph_v1.idf?render"),
            None
        );
    }

    fn db(drivers: &[(&str, &str)], heads: &[&str]) -> Gvdb {
        use swofty_formats::gvdb::Record;
        let mut records = vec![
            Record::new(
                "entrants.drivers._driver_instancecount_",
                0,
                drivers.len().to_string(),
            ),
            Record::new(
                &format!("{TABLE}._{ROW}_instancecount_"),
                0,
                heads.len().to_string(),
            ),
        ];
        for (i, (label, head)) in drivers.iter().enumerate() {
            records.push(Record::new(
                &format!("entrants.drivers.driver_{i}._metadata_"),
                2,
                *label,
            ));
            records.push(Record::new(
                &format!("entrants.drivers.driver_{i}.head"),
                4,
                *head,
            ));
        }
        for (i, label) in heads.iter().enumerate() {
            records.push(Record::new(&format!("{ROWS}_{i}._metadata_"), 2, *label));
        }
        Gvdb { records }
    }

    fn head_of(db: &Gvdb, i: usize) -> Option<String> {
        db.get_text(&format!("entrants.drivers.driver_{i}.head"))
    }

    #[test]
    fn a_driver_the_game_ships_headless_wears_the_stand_in() {
        let drivers = [
            ("lando_norris", "lando_norris"),
            ("nico_rosberg", "nico_rosberg"),
            ("lukas_weber", ""),
        ];
        let shipped = db(&drivers, &["lando_norris", STAND_IN]);
        let mut now = shipped.clone();
        let found = wear_stand_ins(&mut now, &shipped, &BTreeSet::new());
        assert_eq!(
            found,
            [Headless {
                row: "nico_rosberg".into(),
                head: "nico_rosberg".into(),
                worn: STAND_IN.into(),
            }]
        );
        assert_eq!(head_of(&now, 1).as_deref(), Some(STAND_IN));
        assert_eq!(head_of(&now, 0).as_deref(), Some("lando_norris"));
        assert_eq!(head_of(&now, 2).as_deref(), Some(""));
        assert_eq!(now.records.len(), shipped.records.len());
        assert_eq!(wear_stand_ins(&mut now, &shipped, &BTreeSet::new()), []);
        assert!(found[0].line().contains("wears the game's head male_ls222"));
    }

    #[test]
    fn a_head_the_core_adds_or_an_addon_changed_is_left_alone() {
        let shipped = db(&[("nico_rosberg", "nico_rosberg"), ("x", "x")], &["x", STAND_IN]);
        let mut now = shipped.clone();
        let added = BTreeSet::from(["nico_rosberg".to_owned()]);
        assert_eq!(wear_stand_ins(&mut now, &shipped, &added), []);
        assert_eq!(head_of(&now, 0).as_deref(), Some("nico_rosberg"));
        now.set("entrants.drivers.driver_1.head", "face_mod_x", None)
            .unwrap();
        assert_eq!(wear_stand_ins(&mut now, &shipped, &BTreeSet::new()).len(), 1);
        assert_eq!(head_of(&now, 1).as_deref(), Some("face_mod_x"));
    }

    #[test]
    fn a_database_without_the_stand_in_leaves_the_head_empty() {
        let shipped = db(&[("nico_rosberg", "nico_rosberg")], &["lando_norris"]);
        let mut now = shipped.clone();
        let found = wear_stand_ins(&mut now, &shipped, &BTreeSet::new());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].worn, "");
        assert_eq!(head_of(&now, 0).as_deref(), Some(""));
        assert!(found[0].line().contains("its head is left empty"));
    }

    #[test]
    fn registry_heads_go_by_label_or_key() {
        let labels = registry_labels(
            r#"{ "driver.head": { "sw15_nico_rosberg": { "archive": "heads/x.erp" },
                                  "senna": { "label": "ayrton_senna" },
                                  "game/male_ls222": { "label": "layer" } },
                 "driver.identity": { "sw15_rosberg": { "head": "sw15_nico_rosberg" } } }"#,
        );
        assert_eq!(
            labels,
            BTreeSet::from(["ayrton_senna".to_owned(), "sw15_nico_rosberg".to_owned()])
        );
        assert!(registry_labels("not json").is_empty());
    }
}
