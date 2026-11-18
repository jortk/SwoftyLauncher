//! The HUD composer: every enabled addon's `hud.element` records, composed per dial into one set of
//! ready overlay archives (`docs/reference/ops/hud.md`).
//!
//! One scene growth, one template pass and one string table per dial, whatever the number of
//! addons, with a unique `<addon>_` prefix on every element and template node. Whole-file overlays
//! of the dial scenes and the template would overwrite each other; this does not.
//!
//! Parts of the pipeline are ported from Project Halo's `halo_native.py` and `build_native.py`; see
//! the headers of `scene.rs`, `template.rs` and `editor.rs`. Uses work from Project Halo by
//! nyannoying (https://github.com/nyannoying1337/f1-25-project-halo).

pub mod check;
pub mod dials;
pub mod editor;
pub mod geom;
pub mod integrate;
pub mod model;
pub mod motion;
pub mod scene;
pub mod screen_build;
pub mod screens;
pub mod stock_edit;
pub mod template;
pub mod textures;
pub mod units;
pub mod walk;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use swofty_addon::asset::{Root, Source};
use swofty_addon::ops::ErpOp;
use swofty_formats::animclip;
use swofty_formats::erp::{Erp, ErpFile, Resource};
use swofty_formats::pkg::Pkg;

use dials::DialFacts;
use model::{Bind, Dial, HudElement, StockMode};
use scene::{Builder, Mode, Plan, SpriteInfo, Sprites};
use template::StockTags;
use textures::Res;

use crate::{Error, Result};

fn err(msg: impl std::fmt::Display) -> Error {
    Error::Other(format!("hud: {msg}"))
}

/// Where the stock resources come from.
pub trait Stock {
    /// Every fragment of a resource of an archive, decoded.
    fn read(&self, erp: &str, resource: &str) -> Result<Vec<Vec<u8>>>;
    /// An index-only resource whose layout (type argument, fragment names, flags, compression) a
    /// new resource of `kind` with `fragments` fragments copies. A resource that exists in the
    /// stock archive under `name` keeps its own layout.
    fn layout(&self, name: &str, kind: &str, fragments: usize) -> Result<Resource>;
    fn header(&self) -> Result<([u8; swofty_formats::erp::HEADER_LEN], u32)>;
    /// For each of `names`, every stock archive of its asset group that carries it, sorted. The game
    /// loads a resource from whichever of them reads it first, so a composed resource goes to all.
    fn carriers(&self, names: &[&str]) -> Result<BTreeMap<String, Vec<String>>>;
}

/// The stock archives of a game folder.
pub struct GameStock {
    game: PathBuf,
    originals: BTreeMap<String, PathBuf>,
    files: RefCell<BTreeMap<String, Rc<ErpFile>>>,
    carried: RefCell<BTreeMap<String, Vec<String>>>,
}

impl GameStock {
    pub fn new(game: impl Into<PathBuf>) -> Self {
        Self {
            game: game.into(),
            originals: BTreeMap::new(),
            files: RefCell::new(BTreeMap::new()),
            carried: RefCell::new(BTreeMap::new()),
        }
    }

    /// The game folder with each file the launcher keeps an original of read from there: a folder
    /// Swofty has built may already carry composed HUD archives, and the composer starts from stock.
    pub fn with_baseline(game: impl Into<PathBuf>, data: &Path) -> Result<Self> {
        let game = game.into();
        let exe = crate::baseline::sha256_file(&game.join(crate::GAME_EXE))?;
        let mut best: Option<crate::baseline::Baseline> = None;
        let dir = data.join("baseline");
        let entries = std::fs::read_dir(&dir).map_err(|e| Error::Io {
            path: dir.clone(),
            source: e,
        })?;
        for e in entries.flatten() {
            let Ok(text) = std::fs::read_to_string(e.path().join("baseline.json")) else {
                continue;
            };
            let Ok(b) = serde_json::from_str::<crate::baseline::Baseline>(&text) else {
                continue;
            };
            if b.exe_sha256 == exe && best.as_ref().is_none_or(|o| b.files.len() > o.files.len()) {
                best = Some(b);
            }
        }
        let base = best.ok_or_else(|| err("the launcher has no record of the game's original files"))?;
        let objects = crate::baseline::Objects {
            root: dir.join("objects"),
        };
        let originals = base
            .files
            .iter()
            .map(|(rel, rec)| (rel.clone(), objects.path(&rec.sha256)))
            .filter(|(_, p)| p.is_file())
            .collect();
        Ok(Self {
            game,
            originals,
            files: RefCell::new(BTreeMap::new()),
            carried: RefCell::new(BTreeMap::new()),
        })
    }

    /// The file the composer reads `rel` from: the launcher's original when it keeps one, else the game folder's.
    pub fn original(&self, rel: &str) -> PathBuf {
        self.originals
            .get(rel)
            .cloned()
            .unwrap_or_else(|| self.game.join(rel))
    }

    fn file(&self, erp: &str) -> Result<Rc<ErpFile>> {
        if let Some(f) = self.files.borrow().get(erp) {
            return Ok(f.clone());
        }
        let path = self
            .originals
            .get(erp)
            .cloned()
            .unwrap_or_else(|| self.game.join(erp));
        let f = Rc::new(ErpFile::open(&path)?);
        self.files.borrow_mut().insert(erp.to_owned(), f.clone());
        Ok(f)
    }

    /// Reads which archives carry each of `names` into the cache, one pass over each asset group
    /// folder; every archive is read from the original the launcher keeps of it when there is one.
    fn scan(&self, names: &[&str]) -> Result<()> {
        let mut by_group: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
        {
            let carried = self.carried.borrow();
            for n in names.iter().filter(|n| !carried.contains_key(**n)) {
                match crate::erpbuild::group_dir(n) {
                    Some(g) => {
                        by_group.entry(g).or_default().insert(n);
                    }
                    None => {
                        by_group.entry(String::new()).or_default().insert(n);
                    }
                }
            }
        }
        let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (group, wanted) in &by_group {
            for n in wanted {
                found.insert((*n).to_owned(), Vec::new());
            }
            if group.is_empty() {
                continue;
            }
            let dir = self.game.join(group);
            if !dir.is_dir() {
                return Err(err(format!("{} is not a folder", dir.display())));
            }
            let mut files = Vec::new();
            crate::erpbuild::erp_files(&dir, group, &mut files);
            files.sort();
            for rel in files {
                let Ok(f) = ErpFile::open(&self.original(&rel)) else {
                    continue;
                };
                for r in &f.erp.resources {
                    if wanted.contains(r.name.as_str())
                        && let Some(list) = found.get_mut(&r.name)
                        && list.last() != Some(&rel)
                    {
                        list.push(rel.clone());
                    }
                }
            }
        }
        self.carried.borrow_mut().extend(found);
        Ok(())
    }
}

impl Stock for GameStock {
    fn read(&self, erp: &str, resource: &str) -> Result<Vec<Vec<u8>>> {
        let f = self.file(erp)?;
        let res = f
            .erp
            .resources
            .iter()
            .find(|r| r.name == resource)
            .ok_or_else(|| Error::ResourceMissing {
                erp: erp.to_owned(),
                resource: resource.to_owned(),
            })?;
        let loaded = f.load(res)?;
        Ok(loaded
            .fragments
            .iter()
            .map(|fr| fr.data())
            .collect::<std::result::Result<_, _>>()?)
    }

    fn layout(&self, name: &str, kind: &str, fragments: usize) -> Result<Resource> {
        let f = self.file(dials::STOCK_ERP)?;
        let same = |r: &&Resource| r.kind() == kind && r.fragments.len() == fragments;
        f.erp
            .resources
            .iter()
            .filter(same)
            .find(|r| r.name == name)
            .or_else(|| f.erp.resources.iter().find(same))
            .cloned()
            .ok_or_else(|| {
                err(format!(
                    "no stock {kind} with {fragments} fragment(s) to copy a layout from"
                ))
            })
    }

    fn header(&self) -> Result<([u8; swofty_formats::erp::HEADER_LEN], u32)> {
        let f = self.file(dials::STOCK_ERP)?;
        Ok((f.erp.header, f.erp.version))
    }

    fn carriers(&self, names: &[&str]) -> Result<BTreeMap<String, Vec<String>>> {
        self.scan(names)?;
        let carried = self.carried.borrow();
        Ok(names
            .iter()
            .map(|n| ((*n).to_owned(), carried.get(*n).cloned().unwrap_or_default()))
            .collect())
    }
}

/// The stock archives with the changes other addons make to the resources the composer writes whole.
/// The composer starts from these, so its overlay carries those changes instead of overwriting them.
pub struct Layered<'a> {
    pub inner: &'a dyn Stock,
    pub build: &'a crate::erpbuild::ErpBuild<'a>,
}

impl Stock for Layered<'_> {
    fn read(&self, erp: &str, resource: &str) -> Result<Vec<Vec<u8>>> {
        match self.build.fragments(erp, resource)? {
            Some(f) => Ok(f),
            None => self.inner.read(erp, resource),
        }
    }

    fn layout(&self, name: &str, kind: &str, fragments: usize) -> Result<Resource> {
        self.inner.layout(name, kind, fragments)
    }

    fn header(&self) -> Result<([u8; swofty_formats::erp::HEADER_LEN], u32)> {
        self.inner.header()
    }

    fn carriers(&self, names: &[&str]) -> Result<BTreeMap<String, Vec<String>>> {
        self.inner.carriers(names)
    }
}

/// An edit another part of the launcher makes to the HUD template, applied to the template before the
/// composer adds its nodes (the ADDONS MFD page).
pub type TemplateBase<'a> = &'a dyn Fn(&[u8]) -> Result<Vec<u8>>;

/// One record to compose.
#[derive(Debug, Clone)]
pub struct Input {
    pub addon: String,
    pub key: String,
    pub dir: PathBuf,
    pub record: HudElement,
}

/// A ready archive and the game archives it is overlaid on.
#[derive(Debug, Clone)]
pub struct Built {
    pub file: String,
    pub erps: Vec<String>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct Output {
    pub archives: Vec<Built>,
    pub lines: Vec<String>,
}

impl Output {
    /// The overlay ops that put the archives written to `dir` (a folder of the cache root) in the game.
    pub fn ops(&self, dir: &str) -> Vec<ErpOp> {
        self.archives
            .iter()
            .map(|a| ErpOp::Overlay {
                erps: a.erps.clone(),
                source: Source {
                    root: Root::Cache,
                    path: format!("{dir}/{}", a.file),
                },
            })
            .collect()
    }

    pub fn write(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir).map_err(|e| Error::Io {
            path: dir.to_path_buf(),
            source: e,
        })?;
        for a in &self.archives {
            let p = dir.join(&a.file);
            std::fs::write(&p, &a.bytes).map_err(|e| Error::Io { path: p, source: e })?;
        }
        Ok(())
    }
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        let c = if c.is_ascii_alphanumeric() { c } else { '_' };
        if c != '_' || !out.ends_with('_') {
            out.push(c);
        }
    }
    out.trim_matches('_').to_owned()
}

/// The prefix of a record's element and template node names.
pub fn prefix_for(addon: &str, key: &str, explicit: Option<&str>) -> Result<String> {
    let p = match explicit {
        Some(p) => p.to_owned(),
        None => {
            let tail = addon.rsplit('.').next().unwrap_or(addon);
            let a = slug(addon);
            if slug(tail) == slug(key) {
                a
            } else {
                format!("{a}_{}", slug(key))
            }
        }
    };
    let ok = !p.is_empty()
        && p.len() <= 24
        && p.starts_with(|c: char| c.is_ascii_lowercase())
        && p.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if ok {
        Ok(p)
    } else {
        Err(err(format!(
            "{addon}/{key}: the prefix {p:?} must be 1 to 24 characters [a-z0-9_] starting with a letter; set `prefix` in the record"
        )))
    }
}

struct Expanded {
    input: usize,
    rec: HudElement,
    switch: Option<Bind>,
}

fn art_bounds(sprite: &model::Sprite, art: &textures::Art) -> [f64; 4] {
    sprite.bounds.unwrap_or_else(|| {
        let ppu = sprite.ppu.unwrap_or(3.0);
        let (w, h) = (art.width as f64 / ppu, art.height as f64 / ppu);
        [-w / 2.0, -h / 2.0, w / 2.0, h / 2.0]
    })
}

struct Loaded {
    sprites: Sprites,
    resources: Vec<Res>,
}

fn bind_slots(resources: &mut Vec<Res>, n: &textures::Names, uses_fill: bool, uses_edge: bool) {
    if uses_fill {
        resources.push(textures::binding_resource(n, "central_bg_fill", false));
    }
    if uses_edge {
        resources.push(textures::binding_resource(n, "central_bg_edge", true));
    }
}

/// What every sprite of a build shares: the stock files, the fill material and the quad slots in use.
struct SpriteEnv<'a> {
    stock: &'a dyn Stock,
    fill_material: &'a [u8],
    uses_fill: bool,
    uses_edge: bool,
}

fn load_sprites(
    env: &SpriteEnv,
    input: &Input,
    pfx: &str,
    tinted: &std::collections::BTreeSet<String>,
    team: [u8; 3],
) -> Result<Loaded> {
    let SpriteEnv {
        stock,
        fill_material,
        uses_fill,
        uses_edge,
    } = *env;
    let mut sprites = Sprites::new();
    let mut resources = Vec::new();
    for (name, s) in &input.record.sprites {
        if let Some(tex) = &s.texture {
            let names = textures::stock_names(&format!("{pfx}_"), name, tex);
            let tint = s.tint.as_deref().map(textures::hex_rgba).transpose()?;
            resources.extend(textures::stock_sprite_resources(&names, fill_material, tint)?);
            bind_slots(&mut resources, &names, uses_fill, uses_edge);
            let bounds = match (s.bounds, s.fit) {
                (Some(b), _) => b,
                (None, Some(f)) => textures::fit(textures::texture_size(stock, dials::STOCK_ERP, tex)?, f),
                (None, None) => {
                    return Err(err(format!(
                        "sprite {name}: a texture sprite needs fit or bounds"
                    )));
                }
            };
            sprites.insert(
                name.clone(),
                SpriteInfo {
                    bounds,
                    names,
                    tinted: None,
                },
            );
            continue;
        }
        if s.png.is_empty() {
            return Err(err(format!("sprite {name}: needs png or texture")));
        }
        let path = input.dir.join(&s.png);
        let data = std::fs::read(&path).map_err(|e| Error::Io {
            path: path.clone(),
            source: e,
        })?;
        let mut art = textures::decode_png(&data).map_err(|e| err(format!("{}: {e}", s.png)))?;
        art.check(name)?;
        if let Some(t) = &s.tint {
            art = art.tinted(textures::hex_colour(t)?);
        }
        let key = format!("{pfx}_");
        let names = textures::names(&key, name);
        resources.extend(textures::sprite_resources(&names, &art, fill_material)?);
        bind_slots(&mut resources, &names, uses_fill, uses_edge);
        let pv = if tinted.contains(name) {
            let pv_names = textures::names(&key, &format!("{name}_pv"));
            resources.extend(textures::sprite_resources(
                &pv_names,
                &art.tinted(team),
                fill_material,
            )?);
            resources.push(textures::binding_resource(&pv_names, "central_bg_fill", false));
            Some(pv_names)
        } else {
            None
        };
        sprites.insert(
            name.clone(),
            SpriteInfo {
                bounds: art_bounds(s, &art),
                names,
                tinted: pv,
            },
        );
    }
    Ok(Loaded { sprites, resources })
}

fn tinted_sprites(elements: &[model::Element], out: &mut std::collections::BTreeSet<String>) {
    for e in elements {
        if e.tint.is_some() {
            out.extend(e.sprite.clone());
            out.extend(e.fill_sprite.clone());
        }
        tinted_sprites(&e.children, out);
    }
}

fn stock_parts(rec: &HudElement) -> Vec<String> {
    if rec.stock_parts.is_empty() {
        dials::STOCK_KIDS.iter().map(|s| (*s).to_owned()).collect()
    } else {
        rec.stock_parts.clone()
    }
}

fn first(v: Vec<Vec<u8>>, what: &str) -> Result<Vec<u8>> {
    v.into_iter()
        .next()
        .ok_or_else(|| err(format!("{what} has no data")))
}

/// Composes `inputs` against the stock files into the overlay archives.
pub fn compose(stock: &dyn Stock, inputs: &[Input]) -> Result<Output> {
    compose_with(stock, inputs, None)
}

/// [`compose`] with `base` applied to the template the composer starts from. The composer is the last
/// step: its nodes go on top of `base`'s edit and of whatever `stock` reads.
pub fn compose_with(stock: &dyn Stock, inputs: &[Input], base: Option<TemplateBase>) -> Result<Output> {
    let mut out = Output::default();
    if inputs.is_empty() {
        return Ok(out);
    }
    let mut prefixes = BTreeMap::new();
    for (i, input) in inputs.iter().enumerate() {
        let p = prefix_for(&input.addon, &input.key, input.record.prefix.as_deref())?;
        if let Some(j) = prefixes.insert(p.clone(), i) {
            return Err(err(format!(
                "{} and {} both get the prefix {p}; set `prefix` in one of them",
                inputs[j].addon, input.addon
            )));
        }
    }
    let pfx_of: Vec<String> = {
        let mut v = vec![String::new(); inputs.len()];
        for (p, i) in &prefixes {
            v[*i] = p.clone();
        }
        v
    };

    let mut is_screen = Vec::with_capacity(inputs.len());
    for input in inputs {
        let t = screens::targets(&input.record)
            .map_err(|e| err(format!("{}/{}: {e}", input.addon, input.key)))?;
        is_screen.push(t.is_some());
    }
    let mut live: Vec<Expanded> = Vec::new();
    for (i, input) in inputs.iter().enumerate() {
        if input.record.scene_target().is_some() || is_screen[i] {
            continue;
        }
        live.push(Expanded {
            input: i,
            rec: input.record.clone(),
            switch: input.record.switch.clone(),
        });
        for (name, v) in &input.record.variants {
            live.push(Expanded {
                input: i,
                rec: input.record.with_variant(name)?,
                switch: v.switch.clone().or_else(|| input.record.switch.clone()),
            });
        }
    }

    let read = |name: &str| first(stock.read(dials::STOCK_ERP, name)?, name);
    let fill = read(dials::FILL_MATERIAL)?;
    let mods = read(dials::MODS)?;
    let bar = read(dials::BAR)?;
    let clip_template = read(dials::CLIP_TEMPLATE)?;
    let om_xm = read(dials::F126.scene)?;
    let stacked = read(scene::STACKED)?;
    let lap_counter = read(scene::LAP_COUNTER)?;

    let used: std::collections::BTreeSet<Dial> = live.iter().map(|e| e.rec.dial).collect();
    let mut resources: BTreeMap<String, Res> = BTreeMap::new();
    let mut loaded: Vec<Loaded> = Vec::new();
    for (i, input) in inputs.iter().enumerate() {
        if is_screen[i] {
            loaded.push(Loaded {
                sprites: Sprites::new(),
                resources: Vec::new(),
            });
            continue;
        }
        let mut tinted = std::collections::BTreeSet::new();
        tinted_sprites(&input.record.elements, &mut tinted);
        let team = match input
            .record
            .editor
            .as_ref()
            .and_then(|e| e.team_colour.as_deref())
        {
            Some(c) => textures::hex_colour(c)?,
            None => [0x27, 0xf4, 0xd2],
        };
        let env = SpriteEnv {
            stock,
            fill_material: &fill,
            uses_fill: true,
            uses_edge: used.contains(&Dial::F2),
        };
        let l = load_sprites(&env, input, &pfx_of[i], &tinted, team)?;
        for r in &l.resources {
            resources.insert(r.0.clone(), r.clone());
        }
        loaded.push(l);
    }

    let template_bytes = read(dials::TEMPLATE)?;
    let spectator_bytes = read(dials::SPECTATOR)?;
    let mut template = match base {
        Some(edit) => Pkg::parse(&edit(&template_bytes)?)?,
        None => Pkg::parse(&template_bytes)?,
    };
    let mut spectator = Pkg::parse(&spectator_bytes)?;
    let race_stock = Pkg::parse(&template_bytes)?;
    let spec_stock = Pkg::parse(&spectator_bytes)?;
    let race_tags = StockTags::race(&race_stock.root);

    let mut spectator_used = false;
    let mut preview_hides: Vec<String> = Vec::new();
    let mut pv_texts: Vec<(String, String)> = Vec::new();
    let mut scene_resources: Vec<Res> = Vec::new();
    let mut preview_plans: Vec<(usize, Plan)> = Vec::new();
    let mut live_f125_hides = false;

    for dial in Dial::ALL {
        let facts: &'static DialFacts = dials::facts(dial);
        let here: Vec<&Expanded> = live.iter().filter(|e| e.rec.dial == dial).collect();
        let want_preview = dial == Dial::F125 && !live.is_empty();
        if here.is_empty() && !want_preview {
            continue;
        }
        let scene_bytes = first(stock.read(dials::STOCK_ERP, facts.scene)?, facts.scene)?;
        let text_from = (dial != Dial::F126).then_some(om_xm.as_slice());
        let mut b = Builder::new(facts, &scene_bytes, &mods, &bar, text_from)?;
        b.with_donors(&stacked, &lap_counter)?;
        if here.iter().any(|e| units::used(&e.rec.elements)) {
            b.units = units::load(stock, dials::STOCK_ERP)?;
        }
        let mut parked = false;
        let mut footprint: Option<[f64; 4]> = None;
        let mut plans: Vec<(&Expanded, Plan)> = Vec::new();
        for e in &here {
            let parts = stock_parts(&e.rec);
            b.stock_parts_present(&parts)?;
            let mode = e.rec.effective_stock();
            if mode == StockMode::Park && !parked {
                b.park(&parts)?;
                parked = true;
            }
            let mut plan = b.add_record(&e.rec, &pfx_of[e.input], &loaded[e.input].sprites, Mode::Live)?;
            let sw = e
                .switch
                .as_ref()
                .map(|s| template::resolve(s, template::Usage::Visible, "variant switch"))
                .transpose()?;
            let on = e
                .switch
                .as_ref()
                .and_then(|b| b.default.as_ref())
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            if plan.root_hidden || sw.is_some() {
                plan.vis.insert(
                    0,
                    scene::VisItem {
                        name: format!("v_{}", plan.root),
                        element: plan.root.clone(),
                        db: sw.as_ref().map(|s| s.db.clone()),
                        negate: sw.as_ref().is_some_and(|s| s.negate),
                        default: on,
                    },
                );
            }
            if mode == StockMode::Hide {
                for part in &parts {
                    plan.vis.push(scene::VisItem {
                        name: format!("v_stock_{part}"),
                        element: part.clone(),
                        db: sw.as_ref().map(|s| s.db.clone()),
                        negate: sw.as_ref().is_some_and(|s| !s.negate),
                        default: !on,
                    });
                }
                if dial == Dial::F125 {
                    live_f125_hides = true;
                }
            }
            if mode == StockMode::Park && dial == Dial::F125 {
                live_f125_hides = true;
            }
            let sb = scene::scene_box(&plan);
            footprint = Some(footprint.map_or(sb, |f| scene::union_box(f, sb)));
            plans.push((e, plan));
        }
        if want_preview {
            for (i, input) in inputs.iter().enumerate() {
                let rec = &input.record;
                if rec.scene_target().is_some() || is_screen[i] {
                    continue;
                }
                let state = rec.editor.as_ref().map(|e| e.preview.clone()).unwrap_or_default();
                let plan = b.add_record(rec, &pfx_of[i], &loaded[i].sprites, Mode::Preview(&state))?;
                let sb = match rec.editor.as_ref().and_then(|e| e.rect) {
                    Some(r) => {
                        let mut p = plan.clone();
                        p.footprint = r;
                        scene::scene_box(&p)
                    }
                    None => scene::scene_box(&plan),
                };
                footprint = Some(footprint.map_or(sb, |f| scene::union_box(f, sb)));
                if rec
                    .editor
                    .as_ref()
                    .and_then(|e| e.hide_stock_in_editor)
                    .unwrap_or(true)
                {
                    for p in stock_parts(rec) {
                        if !preview_hides.contains(&p) {
                            preview_hides.push(p);
                        }
                    }
                }
                pv_texts.extend(plan.pv_texts.clone());
                preview_plans.push((i, plan));
            }
        }
        let mut new_scene = b.finish(footprint)?;
        let planned: Vec<&Plan> = plans.iter().map(|(_, p)| p).collect();
        clip_resources(stock, facts, &clip_template, &planned, &mut scene_resources)?;
        scene_resources.push((
            facts.scene.to_owned(),
            "UILayout",
            vec![std::mem::take(&mut new_scene)],
        ));
        for (e, plan) in &plans {
            template::attach(
                &mut template.root,
                &race_stock.root,
                &race_tags,
                facts.race_parent,
                plan,
            )?;
            if e.rec.spectator {
                let tags = StockTags::spectator(
                    &spec_stock.root,
                    facts.spectator_parent,
                    &race_tags,
                    facts.race_parent,
                    &race_stock.root,
                )?;
                template::attach(
                    &mut spectator.root,
                    &spec_stock.root,
                    &tags,
                    facts.spectator_parent,
                    plan,
                )?;
                spectator_used = true;
            }
            out.lines.push(format!(
                "{}: {} in {}: {} template nodes, {} clips",
                pfx_of[e.input],
                facts.dial.key(),
                facts.scene.rsplit('/').next().unwrap_or(facts.scene),
                plan.vis.len()
                    + plan.texts.len()
                    + plan.fractions.len()
                    + plan.colours.len()
                    + plan.shows.len(),
                plan.clips.len()
            ));
        }
    }

    let f125 = dials::facts(Dial::F125);
    let keep_stock: Vec<String> = if live_f125_hides {
        Vec::new()
    } else {
        dials::STOCK_KIDS.iter().map(|s| (*s).to_owned()).collect()
    };
    for (i, plan) in &preview_plans {
        editor::attach_control(&mut template.root, f125.race_parent, plan, &keep_stock)?;
        if inputs[*i].record.spectator {
            editor::attach_control(&mut spectator.root, f125.spectator_parent, plan, &keep_stock)?;
            spectator_used = true;
        }
    }
    if !preview_plans.is_empty() {
        let stock_uipre = first(stock.read(dials::STOCK_ERP, f125.uipre)?, f125.uipre)?;
        let new = editor::uipre(&stock_uipre, f125.scene, &preview_hides, &pv_texts)?;
        scene_resources.push((f125.uipre.to_owned(), "UIPR", vec![new]));
    }
    let mut targets: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for (i, input) in inputs.iter().enumerate() {
        if let Some((scene, parent)) = input.record.scene_target() {
            targets
                .entry((scene.to_owned(), parent.to_owned()))
                .or_default()
                .push(i);
        }
    }
    let mut target_scenes: Vec<Res> = Vec::new();
    for ((scene, parent), members) in &targets {
        let facts = dials::target_facts(scene, parent);
        let bytes = first(stock.read(dials::STOCK_ERP, scene)?, scene)?;
        let mut b = Builder::for_scene(facts, &bytes, &om_xm, &mods, &bar)?;
        b.with_donors(&stacked, &lap_counter)?;
        let mut plans = Vec::new();
        for &i in members {
            let plan = b.add_scene_record(&inputs[i].record, &pfx_of[i], &loaded[i].sprites)?;
            plans.push((i, plan));
        }
        target_scenes.push((scene.clone(), "UILayout", vec![b.finish(None)?]));
        let planned: Vec<&Plan> = plans.iter().map(|(_, p)| p).collect();
        clip_resources(stock, facts, &clip_template, &planned, &mut target_scenes)?;
        for (i, plan) in &plans {
            let checked = template::attach_scene(&mut template.root, &race_tags, parent, scene, plan)?;
            out.lines.push(format!(
                "{}: {} in {}: {} template nodes, {checked} bindings checked against the stock template",
                pfx_of[*i],
                parent,
                scene.rsplit('/').next().unwrap_or(scene),
                plan.vis.len() + plan.texts.len() + plan.shows.len(),
            ));
        }
    }
    let screens_out = screen_build::compose_screens(
        stock,
        inputs,
        &pfx_of,
        &mut template,
        &race_stock,
        &mut spectator,
        &spec_stock,
        &fill,
        &mods,
        &bar,
        &om_xm,
    )?;
    spectator_used |= screens_out.spectator_used;
    out.lines.extend(screens_out.lines.iter().cloned());
    let mut all: Vec<Res> = resources.into_values().collect();
    all.extend(scene_resources);
    if !all.is_empty() {
        out.archives.push(Built {
            file: "hud_scene.erp".into(),
            erps: dials::SCENE_ERPS.iter().map(|s| (*s).to_owned()).collect(),
            bytes: archive(stock, all)?,
        });
    }
    if !target_scenes.is_empty() {
        let erps: Vec<String> = dials::TEMPLATE_ERPS
            .iter()
            .filter(|erp| targets.keys().all(|(scene, _)| stock.read(erp, scene).is_ok()))
            .map(|e| (*e).to_owned())
            .collect();
        if erps.is_empty() {
            return Err(err(
                "no archive that has the template also has every scene a record targets",
            ));
        }
        out.archives.push(Built {
            file: "hud_target_scene.erp".into(),
            erps,
            bytes: archive(stock, target_scenes)?,
        });
    }
    out.archives.push(Built {
        file: "hud_template.erp".into(),
        erps: dials::TEMPLATE_ERPS.iter().map(|s| (*s).to_owned()).collect(),
        bytes: archive(
            stock,
            vec![(dials::TEMPLATE.to_owned(), "UIDF", vec![template.to_bytes()?])],
        )?,
    });
    if spectator_used {
        let mut res = vec![(dials::SPECTATOR.to_owned(), "UIDF", vec![spectator.to_bytes()?])];
        res.extend(screens_out.spectator_scene.iter().cloned());
        out.archives.push(Built {
            file: "hud_spectator.erp".into(),
            erps: dials::SPECTATOR_ERPS.iter().map(|s| (*s).to_owned()).collect(),
            bytes: archive(stock, res)?,
        });
    }
    out.archives.extend(screen_build::archives(stock, &screens_out)?);
    cover_carriers(stock, &mut out.archives, &mut out.lines)?;
    Ok(out)
}

/// Overlays every stock resource of `archives` on every stock archive that carries it, not only on
/// the archives each is made for: the game loads a resource from whichever archive reads it first,
/// so a stock copy left beside a composed one mixes the two. Each added archive holds the stock
/// resources its targets carry and every new resource of the archive it comes from, so a composed
/// copy keeps the clips, sprites and materials it references.
pub fn cover_carriers(stock: &dyn Stock, archives: &mut Vec<Built>, lines: &mut Vec<String>) -> Result<()> {
    let parsed: Vec<Erp> = archives
        .iter()
        .map(|a| Erp::parse(&a.bytes).map_err(|e| err(format!("{}: {e}", a.file))))
        .collect::<Result<_>>()?;
    let names: Vec<&str> = parsed
        .iter()
        .flat_map(|e| e.resources.iter().map(|r| r.name.as_str()))
        .collect();
    let carriers = stock.carriers(&names)?;
    let mut lands: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (a, e) in archives.iter().zip(&parsed) {
        for r in &e.resources {
            lands
                .entry(r.name.clone())
                .or_default()
                .extend(a.erps.iter().cloned());
        }
    }
    let mut added = Vec::new();
    for (a, e) in archives.iter().zip(&parsed) {
        let mut held: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
        let mut new = BTreeSet::new();
        for (i, r) in e.resources.iter().enumerate() {
            let of = carriers.get(&r.name).map(Vec::as_slice).unwrap_or_default();
            if of.is_empty() {
                new.insert(i);
            }
            for erp in of.iter().filter(|erp| !lands[&r.name].contains(*erp)) {
                held.entry(erp.clone()).or_default().insert(i);
            }
        }
        let mut groups: BTreeMap<BTreeSet<usize>, Vec<String>> = BTreeMap::new();
        for (erp, idx) in held {
            groups.entry(idx).or_default().push(erp);
        }
        let stem = a.file.strip_suffix(".erp").unwrap_or(&a.file);
        for (n, (idx, erps)) in groups.into_iter().enumerate() {
            for i in &idx {
                if let Some(l) = lands.get_mut(&e.resources[*i].name) {
                    l.extend(erps.iter().cloned());
                }
            }
            let resources: Vec<Resource> = e
                .resources
                .iter()
                .enumerate()
                .filter(|(i, _)| idx.contains(i) || new.contains(i))
                .map(|(_, r)| r.clone())
                .collect();
            let file = format!("{stem}_{}.erp", n + 2);
            lines.push(format!(
                "{file}: {} also carry {} of the stock resources {} writes ({}), so they get the composed copies and the {} new resources beside them",
                erps.iter().map(|e| e.rsplit('/').next().unwrap_or(e)).collect::<Vec<_>>().join(", "),
                idx.len(),
                a.file,
                idx.iter()
                    .map(|i| e.resources[*i].name.rsplit('/').next().unwrap_or_default())
                    .collect::<Vec<_>>()
                    .join(", "),
                new.len()
            ));
            let mut erp = Erp {
                header: e.header,
                version: e.version,
                resources,
            };
            added.push(Built {
                file,
                erps,
                bytes: erp.to_bytes(),
            });
        }
    }
    archives.extend(added);
    Ok(())
}

/// The anim string table and the clips the plans put in `facts.scene`: the new names appended
/// to the stock table in order, one `AnimClip` per clip, built on `clip_template`. Nothing when
/// no plan has a clip.
pub fn clip_resources(
    stock: &dyn Stock,
    facts: &DialFacts,
    clip_template: &[u8],
    plans: &[&Plan],
    out: &mut Vec<Res>,
) -> Result<()> {
    let mut names: Vec<String> = plans.iter().flat_map(|p| p.anim_names()).collect();
    names.sort();
    if names.is_empty() {
        return Ok(());
    }
    let strtab = first(stock.read(dials::STOCK_ERP, facts.strtab)?, facts.strtab)?;
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let (tab, first_index) = animclip::append_strings(&strtab, &refs)?;
    out.push((facts.strtab.to_owned(), "ACStringTable", vec![tab]));
    for plan in plans {
        for c in &plan.clips {
            let at = names.iter().position(|n| *n == c.node).expect("collected above");
            let (temp, syst) = animclip::build_clip(
                clip_template,
                &c.node,
                (first_index + at) as i32,
                facts.strtab,
                &c.channels,
            )?;
            out.push((
                format!("{}/{}?context=animation", facts.scene, c.node),
                "AnimClip",
                vec![temp, syst],
            ));
        }
    }
    Ok(())
}

/// A ready archive of `resources`, each laid out like the stock resource it is made after.
pub fn archive(stock: &dyn Stock, resources: Vec<Res>) -> Result<Vec<u8>> {
    let (header, version) = stock.header()?;
    let mut out = Vec::with_capacity(resources.len());
    let mut seen = std::collections::BTreeSet::new();
    for (name, kind, frags) in resources {
        if !seen.insert(name.clone()) {
            return Err(err(format!("the resource {name} is made twice")));
        }
        let template = stock.layout(&name, kind, frags.len())?;
        out.push(Resource::like(&name, &template, &frags)?);
    }
    let mut erp = Erp {
        header,
        version,
        resources: out,
    };
    Ok(erp.to_bytes())
}

/// Reads the records of an addon's hud file into inputs.
pub fn inputs_of(addon: &str, dir: &Path, file: &str) -> Result<Vec<Input>> {
    let path = dir.join(file);
    let text = std::fs::read_to_string(&path).map_err(|e| Error::Io {
        path: path.clone(),
        source: e,
    })?;
    let doc = swofty_addon::ops::HudFile::parse(&text)?;
    let mut out = Vec::new();
    for (id, value) in doc.elements {
        let key = id.rsplit('/').next().unwrap_or(&id).to_owned();
        let record = HudElement::parse(&value).map_err(|e| err(format!("{path:?}: {id}: {e}")))?;
        out.push(Input {
            addon: addon.to_owned(),
            key,
            dir: dir.to_path_buf(),
            record,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::node::{NodeExt, ObjExt};

    fn game() -> Option<GameStock> {
        let Some(dir) = std::env::var_os("F1_25_DIR") else {
            eprintln!("F1_25_DIR is not set; skipping");
            return None;
        };
        let game = PathBuf::from(dir);
        let data = crate::store::Store::open_default().root;
        Some(GameStock::with_baseline(&game, &data).unwrap_or_else(|_| GameStock::new(game)))
    }

    fn png(path: &Path, w: u32, h: u32, rgba: [u8; 4]) {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let data: Vec<u8> = rgba.iter().copied().cycle().take((w * h * 4) as usize).collect();
            enc.write_header().unwrap().write_image_data(&data).unwrap();
        }
        std::fs::write(path, out).unwrap();
    }

    fn addon_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-hud-{tag}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("art")).unwrap();
        for (n, w, h) in [
            ("base", 64, 28),
            ("lit", 16, 8),
            ("fill", 16, 8),
            ("mask", 32, 32),
            ("solid", 8, 8),
        ] {
            png(&dir.join(format!("art/{n}.png")), w, h, [255, 255, 255, 255]);
        }
        dir
    }

    fn record() -> serde_json::Value {
        serde_json::json!({
            "dial": "f1_26",
            "sprites": {
                "base": { "png": "art/base.png", "bounds": [-60, -20, 60, 20] },
                "lit": { "png": "art/lit.png", "bounds": [-50, -10, -34, -2] },
                "fill": { "png": "art/fill.png", "bounds": [-20, 8, -4, 16] },
                "mask": { "png": "art/mask.png", "bounds": [10, -30, 42, 2] },
                "solid": { "png": "art/solid.png", "bounds": [-4, -4, 4, 4] }
            },
            "elements": [
                { "kind": "sprite", "id": "base", "sprite": "base", "layer": 20 },
                { "kind": "group", "id": "std", "visible": { "view_model": "view_model_data.hud_t_std", "default": true }, "children": [
                    { "kind": "sprite", "id": "boost", "sprite": "lit", "visible": { "view_model": "view_model_data.hud_t_boost" } },
                    { "kind": "text", "id": "speed", "style": "OSD_24_bold", "align": "centre", "box": 40, "cap_height": 9.5, "at": [0, -6], "value": { "game": "speed_str" } },
                    { "kind": "text", "id": "pos", "style": "17_bold", "align": "right", "box": 14, "cap_height": 6, "at": [-40, 12], "value": { "session": "current_car.position" } },
                    { "kind": "levels", "id": "dp", "sprite": "lit", "count": 4, "show": "view_model_data.hud_t_dp{nn}" },
                    { "kind": "slats", "id": "th", "sprites": ["lit", "lit"], "show": "view_model_data.hud_t_th{nn}" },
                    { "kind": "gauge", "id": "batt", "sprite": "fill", "path": [[-20, 12], [-4, 12]], "value": { "game": "ers_stored" } },
                    { "kind": "sweep", "id": "rpm", "mask_sprite": "mask", "fill_sprite": "solid", "pivot": [0, 0], "start": 225, "arc": [[0, 0], [1, -270]], "value": { "game": "revs" }, "tint": { "session": "current_car.nub_colour" } }
                ] }
            ],
            "editor": { "preview": { "game.revs": 0.5, "game.speed_str": "135", "game.ers_stored": 0.75,
                                     "view_model_data.hud_t_boost": true, "view_model_data.hud_t_dp02": true,
                                     "session.current_car.position": "1" }, "team_colour": "#27F4D2" },
            "variants": { "f2": { "dial": "f2", "remove": ["batt"] } }
        })
    }

    #[test]
    fn prefixes_come_from_the_addon_and_the_key() {
        assert_eq!(
            prefix_for("nyannoying.halo", "halo", None).unwrap(),
            "nyannoying_halo"
        );
        assert_eq!(
            prefix_for("swofty.gauges", "dial_a", None).unwrap(),
            "swofty_gauges_dial_a"
        );
        assert_eq!(prefix_for("a.b", "c", Some("mine")).unwrap(), "mine");
        assert!(prefix_for("a.b", "c", Some("Mine")).is_err());
        assert!(prefix_for("swofty.a-very-long-addon-name", "and_a_long_key", None).is_err());
    }

    #[test]
    fn the_stock_facts_hold_in_the_game_files() {
        let Some(stock) = game() else { return };
        let r = check::stock_facts(&stock);
        for l in &r.ok {
            eprintln!("ok   {l}");
        }
        for l in &r.problems {
            eprintln!("FAIL {l}");
        }
        assert!(r.is_ok(), "{:#?}", r.problems);
    }

    #[test]
    fn composes_every_element_kind_against_the_stock_dials() {
        let Some(stock) = game() else { return };
        let dir = addon_dir("all");
        let rec = HudElement::parse(&record()).unwrap();
        let input = Input {
            addon: "test.hud".into(),
            key: "hud".into(),
            dir: dir.clone(),
            record: rec,
        };
        let out = compose(&stock, &[input]).unwrap();
        for l in &out.lines {
            eprintln!("{l}");
        }
        let names: Vec<&str> = out.archives.iter().map(|a| a.file.as_str()).collect();
        assert_eq!(names, ["hud_scene.erp", "hud_template.erp", "hud_spectator.erp"]);
        assert!(check::unpadded_clips(&out.archives).unwrap().is_empty());
        let scene = Erp::parse(&out.archives[0].bytes).unwrap();
        for dial in Dial::ALL {
            let f = dials::facts(dial);
            let res = scene.find(f.scene).unwrap();
            let p = Pkg::parse(&res.fragments[0].data().unwrap()).unwrap();
            assert!(crate::ui::idf::id_problems(&p).unwrap().is_empty(), "{}", f.scene);
            let root = crate::ui::idf::scenes(&p).unwrap()[0]
                .as_obj()
                .unwrap()
                .get("scene")
                .unwrap();
            assert!(
                crate::ui::idf::find(root, "test_hud_speed").is_some() || dial == Dial::F125,
                "{}",
                f.scene
            );
        }
        let f126 = dials::facts(Dial::F126);
        let anim = scene.find(&format!(
            "{}/zh_test_hud_batt_win_win?context=animation",
            f126.scene
        ));
        assert!(
            anim.is_ok()
                || scene
                    .resources
                    .iter()
                    .any(|r| r.name.contains("context=animation"))
        );
        assert!(
            scene
                .resources
                .iter()
                .any(|r| r.name.ends_with("test_hud_base.tif.image"))
        );
        let tmpl = Erp::parse(&out.archives[1].bytes).unwrap();
        let t = Pkg::parse(&tmpl.find(dials::TEMPLATE).unwrap().fragments[0].data().unwrap()).unwrap();
        let at = crate::ui::node::find_path(&t.root, &format!("{}.test_hud_ctl", dials::F126.race_parent))
            .unwrap();
        assert!(!at.is_empty());
        let ours: std::collections::BTreeSet<String> =
            ["test_hud_ctl", "test_hud_pv_ctl"].map(String::from).into();
        let stock_t = Pkg::parse(&stock.read(dials::STOCK_ERP, dials::TEMPLATE).unwrap()[0]).unwrap();
        assert!(
            check::stock_kept(&stock_t, &t, &ours).unwrap(),
            "a stock template node changed"
        );
        let spec = Erp::parse(&out.archives[2].bytes).unwrap();
        let s = Pkg::parse(&spec.find(dials::SPECTATOR).unwrap().fragments[0].data().unwrap()).unwrap();
        let stock_s = Pkg::parse(&stock.read(dials::STOCK_ERP, dials::SPECTATOR).unwrap()[0]).unwrap();
        assert!(
            check::stock_kept(&stock_s, &s, &ours).unwrap(),
            "a stock spectator node changed"
        );
        for (d, ctl) in [
            (Dial::F126, "test_hud_ctl"),
            (Dial::F125, "test_hud_pv_ctl"),
            (Dial::F2, "test_hud_ctl"),
        ] {
            let parent = dials::facts(d).spectator_parent;
            assert!(
                crate::ui::node::find_path(&s.root, &format!("{parent}.{ctl}")).is_ok(),
                "{parent}"
            );
        }
        std::fs::remove_dir_all(dir).ok();
    }

    fn tower_record() -> serde_json::Value {
        serde_json::json!({
            "screen": "standings",
            "show": { "view_model": "view_model_data.hud_tt_show" },
            "also": { "replay": { "show": { "view_model": "view_model_data.hud_tt_on" } },
                      "spectator": { "show": { "view_model": "view_model_data.hud_tt_on" } } },
            "sprites": {
                "strip": { "png": "art/base.png", "bounds": [0, 0, 64, 20] },
                "bn_m": { "png": "art/solid.png", "bounds": [0, 0, 8, 8] }
            },
            "elements": [
                { "kind": "group", "id": "r00", "visible": { "view_model": "view_model_data.hud_tt_r00_on" }, "children": [
                    { "kind": "sprite", "id": "s", "sprite": "strip", "layer": 20 },
                    { "kind": "text", "id": "pos", "style": "OSD_24_bold", "align": "centre", "box": 12, "cap_height": 6,
                      "at": [6, 10], "value": { "view_model": "view_model_data.hud_tt_r00_pos" } },
                    { "kind": "text", "id": "code", "style": "OSD_24_bold", "align": "left", "box": 24, "cap_height": 7,
                      "at": [20, 10], "value": { "session_car": "driver.second_name", "slot": 0 } }
                ] }
            ],
            "stock_nodes": [
                { "template": screens::LAPC_PARENT, "hide": screens::LAPC_HIDE,
                  "while": { "view_model": "view_model_data.hud_tt_show" } }
            ],
            "restyle": { "preset": "banner", "sprite": "bn_m" },
            "editor": { "rows": 5, "preview": { "view_model_data.hud_tt_r00_on": true, "view_model_data.hud_tt_r00_pos": "10" } }
        })
    }

    fn pkg_in(erp: &Erp, name: &str) -> Pkg {
        Pkg::parse(&erp.find(name).unwrap().fragments[0].data().unwrap()).unwrap()
    }

    #[test]
    fn a_screen_record_composes_the_three_screens_and_the_banner() {
        let Some(stock) = game() else { return };
        let dir = addon_dir("tower");
        let rec = HudElement::parse(&tower_record()).unwrap();
        let input = Input {
            addon: "test.tower".into(),
            key: "tower".into(),
            dir: dir.clone(),
            record: rec,
        };
        let out = compose(&stock, &[input]).unwrap();
        for l in &out.lines {
            eprintln!("{l}");
        }
        let names: Vec<&str> = out.archives.iter().map(|a| a.file.as_str()).collect();
        assert_eq!(
            names,
            [
                "hud_template.erp",
                "hud_spectator.erp",
                "hud_screen_scene.erp",
                "hud_replay.erp",
                "hud_replay_scene.erp",
                "hud_screen_scene_2.erp"
            ]
        );
        assert_eq!(out.archives[2].erps, screen_build::SCREEN_SCENE_ERPS);
        assert_eq!(
            out.archives[5].erps,
            [
                "2025_asset_groups/ui_package/screens/hud.erp",
                "2025_asset_groups/ui_package/screens/hud_player_1.erp",
                "2025_asset_groups/ui_package/screens/hud_player_2.erp"
            ]
        );
        assert!(check::uncovered(&stock, &out.archives).unwrap().is_empty());
        assert!(check::constant_rotations(&out.archives).unwrap().is_empty());
        assert!(check::unpadded_clips(&out.archives).unwrap().is_empty());
        let also = Erp::parse(&out.archives[5].bytes).unwrap();
        let main = Erp::parse(&out.archives[2].bytes).unwrap();
        for name in [screens::CAP, screens::STACKED, screens::STACKED_UIPRE] {
            assert_eq!(
                also.find(name).unwrap().fragments[0].data().unwrap(),
                main.find(name).unwrap().fragments[0].data().unwrap(),
                "{name}"
            );
        }
        assert!(
            also.resources
                .iter()
                .any(|r| r.name.ends_with("test_tower_strip.tif.image"))
        );
        let scene = Erp::parse(&out.archives[2].bytes).unwrap();
        let cap = pkg_in(&scene, screens::CAP);
        assert!(crate::ui::idf::id_problems(&cap).unwrap().is_empty());
        let root = stock_edit::scene_root(&cap).unwrap();
        let mine = crate::ui::idf::find(root, "test_tower").unwrap();
        assert_eq!(
            crate::ui::idf::param(mine, "isHidden").and_then(stock_edit::as_bool),
            Some(true)
        );
        assert!(crate::ui::idf::find(root, "test_tower_s").is_some());
        assert!(crate::ui::idf::find(root, "test_tower_pos").is_some());
        let stacked = pkg_in(&scene, screens::STACKED);
        assert!(crate::ui::idf::id_problems(&stacked).unwrap().is_empty());
        let host =
            crate::ui::idf::find(stock_edit::scene_root(&stacked).unwrap(), "test_tower_host").unwrap();
        assert_eq!(crate::ui::idf::param_str(host, "fileRef"), Some(screens::CAP));
        let pv = crate::ui::idf::find(root, "test_tower_pv").expect("the editor copy");
        assert_ne!(
            crate::ui::idf::param(pv, "isHidden").and_then(stock_edit::as_bool),
            Some(true)
        );
        assert!(crate::ui::idf::find(root, "test_tower_pv_pos").is_some());
        let pre = swofty_formats::uipre::Uipre::parse(
            &scene.find(screens::STACKED_UIPRE).unwrap().fragments[0]
                .data()
                .unwrap(),
        )
        .unwrap();
        let host = screens::standings().host_name("test_tower");
        let inside = pre.element_ids_in(&host);
        for name in [
            "pill_rotation_anim",
            "test_tower",
            "test_tower_pv",
            "test_tower_pv_pos",
        ] {
            assert!(
                inside.contains(&swofty_formats::uipre::element_id(name)),
                "{name} in the host's preview"
            );
        }
        assert!(
            pre.element_ids()
                .contains(&swofty_formats::uipre::element_id(screens::STACKED_MASK))
        );
        assert!(
            scene
                .resources
                .iter()
                .any(|r| r.name.ends_with("test_tower_strip.tif.image"))
        );
        assert!(scene.find(&stock_edit::emb_name("test_tower", "bn_m")).is_ok());
        for (rel, fills, twin, hide, _) in stock_edit::BANNER_SCENES {
            let sc = stock_edit::banner_scene(rel);
            let p = pkg_in(&scene, &sc);
            assert!(crate::ui::idf::id_problems(&p).unwrap().is_empty(), "{sc}");
            let root = stock_edit::scene_root(&p).unwrap();
            for (el, _) in fills {
                let e = crate::ui::idf::find(root, el).unwrap();
                let m = e.o().unwrap().at("geoms").unwrap().a().unwrap()[0]
                    .o()
                    .unwrap()
                    .str_at("materialid")
                    .unwrap();
                assert_eq!(m, stock_edit::emb_name("test_tower", "bn_m"), "{sc}/{el}");
            }
            if let Some(t) = twin {
                assert!(crate::ui::idf::find(root, &format!("{t}_bn")).is_some(), "{sc}");
            }
            for el in hide {
                let e = crate::ui::idf::find(root, el).unwrap();
                assert_eq!(
                    crate::ui::idf::param(e, "isHidden").and_then(stock_edit::as_bool),
                    Some(true),
                    "{sc}/{el}"
                );
            }
        }
        assert!(scene.find(stock_edit::BANNER_SYSTEM).is_ok());

        let mut ours: std::collections::BTreeSet<String> =
            ["test_tower_ctl", "test_tower_sn0", "test_tower_sn1"]
                .map(String::from)
                .into();
        for (_, _, _, hide, _) in stock_edit::BANNER_SCENES {
            ours.extend(hide.iter().map(|h| format!("test_tower_hide_{h}")));
        }
        let tmpl = Erp::parse(&out.archives[0].bytes).unwrap();
        let t = pkg_in(&tmpl, dials::TEMPLATE);
        let base = format!("{}.test_tower_ctl", screens::STANDINGS_PARENT);
        for sub in [
            "mask_0",
            "host_n",
            "host_n.v_cap",
            "host_n.v_pv",
            "host_n.root_n",
            "host_n.v_r00",
            "host_n.t_pos",
        ] {
            assert!(
                crate::ui::node::find_path(&t.root, &format!("{base}.{sub}")).is_ok(),
                "{sub}"
            );
        }
        let at = crate::ui::node::find_path(&t.root, screens::STANDINGS_PARENT).unwrap();
        let parent = crate::ui::node::node_at(&t.root, &at).unwrap();
        let enable = parent
            .slot(crate::ui::node::COMMANDS)
            .unwrap()
            .iter()
            .find(|c| c.as_obj().unwrap().str_at("commandName").unwrap() == "enable")
            .unwrap();
        assert!(format!("{enable:?}").contains("test_tower_ctl"));
        let lapc =
            crate::ui::node::find_path(&t.root, &format!("{}.test_tower_sn0", screens::LAPC_PARENT)).unwrap();
        let grp = crate::ui::node::node_at(&t.root, &lapc).unwrap();
        assert_eq!(
            grp.slot(crate::ui::node::CHILDREN).unwrap().len(),
            screens::LAPC_HIDE.len()
        );
        for root in ["v_test_tower", "v_test_tower_pv"] {
            assert!(
                crate::ui::node::find_path(
                    &t.root,
                    &format!("{}.test_tower_sn1.{root}", screens::STANDINGS_CAP_NODE)
                )
                .is_ok(),
                "{root}"
            );
        }
        assert!(crate::ui::node::find_path(
            &t.root,
            "hud_notifications_dynamic_p2.osd_visibility_option.hud_notifications_dynamic_p2.notification_0.hud_message.right.central_message.test_tower_hide_ms_colour"
        )
        .is_ok());
        let stock_t = Pkg::parse(&stock.read(dials::STOCK_ERP, dials::TEMPLATE).unwrap()[0]).unwrap();
        assert!(
            check::stock_kept(&stock_t, &t, &ours).unwrap(),
            "a stock template node changed"
        );

        let spec = Erp::parse(&out.archives[1].bytes).unwrap();
        let s = pkg_in(&spec, dials::SPECTATOR);
        assert!(
            crate::ui::node::find_path(
                &s.root,
                &format!("{}.test_tower_ctl.host_n", screens::SPECT_PARENT)
            )
            .is_ok()
        );
        let stock_s = Pkg::parse(&stock.read(dials::STOCK_ERP, dials::SPECTATOR).unwrap()[0]).unwrap();
        assert!(
            check::stock_kept(&stock_s, &s, &ours).unwrap(),
            "a stock spectator node changed"
        );
        let table = pkg_in(&spec, screens::SPECT_TABLE);
        assert!(crate::ui::idf::id_problems(&table).unwrap().is_empty());
        let host = crate::ui::idf::find(stock_edit::scene_root(&table).unwrap(), "test_tower_host").unwrap();
        assert_eq!(crate::ui::idf::param_str(host, "fileRef"), Some(screens::CAP));

        let replay = Erp::parse(&out.archives[3].bytes).unwrap();
        assert_eq!(
            out.archives[3].erps,
            [screens::FLOW_REPLAY, screens::REPLAY_SCREEN_ERP]
        );
        assert_eq!(out.archives[4].erps, [screens::FLOW_FRONTEND]);
        let rt = pkg_in(&replay, screens::REPLAY_UI);
        assert!(
            crate::ui::node::find_path(
                &rt.root,
                &format!("{}.test_tower_ctl.extra_hide", screens::REPLAY_ROOT)
            )
            .is_ok()
        );
        let stock_r = Pkg::parse(&stock.read(screens::FLOW_REPLAY, screens::REPLAY_UI).unwrap()[0]).unwrap();
        assert!(
            check::stock_kept(&stock_r, &rt, &ours).unwrap(),
            "a stock replay node changed"
        );
        let rs = pkg_in(&replay, screens::REPLAY_SCENE);
        assert!(crate::ui::idf::id_problems(&rs).unwrap().is_empty());
        let host = crate::ui::idf::find(stock_edit::scene_root(&rs).unwrap(), "test_tower_host").unwrap();
        assert_eq!(crate::ui::idf::param_str(host, "fileRef"), Some(screens::CAP));
        let p = crate::ui::idf::position(host).unwrap();
        assert!((p[0] - screens::REPLAY_POS[0]).abs() < 1e-4 && (p[1] - screens::REPLAY_POS[1]).abs() < 1e-4);
        let rs2 = Erp::parse(&out.archives[4].bytes).unwrap();
        assert!(rs2.find(screens::REPLAY_SCENE).is_ok());
        std::fs::remove_dir_all(dir).ok();
    }

    fn fake_archive(names: &[&str]) -> Vec<u8> {
        use swofty_formats::erp::{Fragment, HEADER_LEN, MAGIC, STORED};
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
                        raw: vec![n.len() as u8],
                    }],
                    hash: [0; 16],
                })
                .collect(),
        }
        .to_bytes()
    }

    #[test]
    fn a_clip_whose_syst_is_not_a_multiple_of_16_bytes_is_reported() {
        use swofty_formats::erp::{Fragment, HEADER_LEN, MAGIC, STORED};
        let frag = |name: &[u8; 4], raw: Vec<u8>| Fragment {
            name: *name,
            offset: 0,
            size: raw.len() as u64,
            flags: 0x10,
            compression: STORED,
            packed: raw.len() as u64,
            raw,
        };
        let clip = |name: &str, syst: usize| Resource {
            name: name.into(),
            kind: swofty_formats::erp::kind("AnimClip").unwrap(),
            unk: vec![0; 4],
            fragments: vec![frag(b"temp", vec![0]), frag(b"syst", vec![0; syst])],
            hash: [0; 16],
        };
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        header[4..8].copy_from_slice(&3u32.to_le_bytes());
        let bytes = Erp {
            header,
            version: 3,
            resources: vec![clip("slide", 92), clip("needle", 112), clip("short", 108)],
        }
        .to_bytes();
        let built = [Built {
            file: "hud_scene.erp".into(),
            erps: vec![],
            bytes,
        }];
        assert_eq!(
            check::unpadded_clips(&built).unwrap(),
            ["slide (92 bytes)", "short (108 bytes)"]
        );
    }

    #[test]
    fn a_resource_carried_outside_the_made_for_archives_goes_there_too() {
        let dir = std::env::temp_dir().join(format!("swofty-hud-carriers-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let ui = "2025_asset_groups/ui_package";
        let toast = "eaid://ui_package/art/osd/units/message_system/central_message.idf";
        let sprite = "eaid://ui_package/art/osd/tt_strip.tif.image";
        for (rel, names) in [
            ("flow_race.erp", vec![screens::CAP, toast]),
            ("flow_options.erp", vec![screens::CAP]),
            ("persistent.erp", vec![screens::CAP]),
            ("screens/hud.erp", vec![screens::CAP, "eaid://ui_package/x.idf"]),
            ("screens/hud_player_1.erp", vec![screens::CAP]),
            ("screens/other.erp", vec!["eaid://ui_package/y.idf"]),
        ] {
            let path = dir.join(ui).join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, fake_archive(&names)).unwrap();
        }
        std::fs::write(dir.join(ui).join("screens/junk.erp"), b"not an archive").unwrap();
        let stock = GameStock::new(&dir);
        let made_for: Vec<String> = screen_build::SCREEN_SCENE_ERPS
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let mut archives = vec![Built {
            file: "hud_screen_scene.erp".into(),
            erps: made_for.clone(),
            bytes: fake_archive(&[screens::CAP, toast, sprite]),
        }];
        let left = check::uncovered(&stock, &archives).unwrap();
        assert_eq!(
            left,
            [
                format!(
                    "{} is composed, but {ui}/screens/hud.erp also carries it and would keep the stock copy",
                    screens::CAP
                ),
                format!(
                    "{} is composed, but {ui}/screens/hud_player_1.erp also carries it and would keep the stock copy",
                    screens::CAP
                ),
            ]
        );
        let mut lines = Vec::new();
        cover_carriers(&stock, &mut archives, &mut lines).unwrap();
        assert_eq!(archives.len(), 2);
        assert_eq!(archives[0].erps, made_for);
        assert_eq!(archives[1].file, "hud_screen_scene_2.erp");
        assert_eq!(
            archives[1].erps,
            [
                format!("{ui}/screens/hud.erp"),
                format!("{ui}/screens/hud_player_1.erp")
            ]
        );
        let added = Erp::parse(&archives[1].bytes).unwrap();
        let names: Vec<&str> = added.resources.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, [screens::CAP, sprite]);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(check::uncovered(&stock, &archives).unwrap().is_empty());

        let mut shared = vec![
            Built {
                file: "a.erp".into(),
                erps: vec![format!("{ui}/flow_race.erp"), format!("{ui}/flow_options.erp")],
                bytes: fake_archive(&[screens::CAP]),
            },
            Built {
                file: "b.erp".into(),
                erps: vec![
                    format!("{ui}/persistent.erp"),
                    format!("{ui}/screens/hud.erp"),
                    format!("{ui}/screens/hud_player_1.erp"),
                ],
                bytes: fake_archive(&[screens::CAP]),
            },
        ];
        cover_carriers(&stock, &mut shared, &mut lines).unwrap();
        assert_eq!(shared.len(), 2, "every carrier is covered between the two");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn two_screen_records_hide_each_others_roots_in_their_hosts() {
        let Some(stock) = game() else { return };
        let dir = addon_dir("two-screens");
        let mk = |addon: &str| {
            let mut v = tower_record();
            let o = v.as_object_mut().unwrap();
            o.remove("stock_nodes");
            o.remove("restyle");
            Input {
                addon: addon.into(),
                key: "tower".into(),
                dir: dir.clone(),
                record: HudElement::parse(&v).unwrap(),
            }
        };
        let out = compose(&stock, &[mk("one.season"), mk("two.season")]).unwrap();
        let ins: Vec<String> = [("one.season", "tower"), ("two.season", "tower")]
            .iter()
            .map(|(a, k)| prefix_for(a, k, None).unwrap())
            .collect();
        let (a, b) = (&ins[0], &ins[1]);
        let file = |name: &str| {
            let built = out.archives.iter().find(|x| x.file == name).unwrap();
            Erp::parse(&built.bytes).unwrap()
        };
        let checks = [
            (
                pkg_in(&file("hud_template.erp"), dials::TEMPLATE),
                screens::STANDINGS_PARENT,
            ),
            (
                pkg_in(&file("hud_spectator.erp"), dials::SPECTATOR),
                screens::SPECT_PARENT,
            ),
            (
                pkg_in(&file("hud_replay.erp"), screens::REPLAY_UI),
                screens::REPLAY_ROOT,
            ),
        ];
        for (t, parent) in &checks {
            for (me, other) in [(a, b), (b, a)] {
                for (node, el) in [
                    (format!("o_{other}"), other.clone()),
                    (format!("o_{other}_pv"), screen_build::preview_root(other)),
                ] {
                    let path = format!("{parent}.{me}_ctl.host_n.{node}");
                    let at = crate::ui::node::find_path(&t.root, &path).unwrap_or_else(|_| panic!("{path}"));
                    let n = crate::ui::node::node_at(&t.root, &at).unwrap();
                    let layout: Vec<String> = n
                        .slot(crate::ui::node::LAYOUT)
                        .unwrap()
                        .iter()
                        .flat_map(|c| c.as_arr().unwrap().items.iter())
                        .map(|o| o.as_obj().unwrap().str_at("debugName").unwrap().to_owned())
                        .collect();
                    assert_eq!(layout, std::slice::from_ref(&el), "{path}");
                    let vis = n.props().unwrap().get("visible").unwrap().as_obj().unwrap();
                    assert_eq!(
                        vis.get("value")
                            .and_then(swofty_formats::pkg::Node::as_val)
                            .and_then(stock_edit::as_bool),
                        Some(false),
                        "{path}"
                    );
                }
                assert!(
                    crate::ui::node::find_path(&t.root, &format!("{parent}.{me}_ctl.host_n.o_{me}")).is_err(),
                    "{me} does not hide its own root"
                );
            }
        }
        let scene = file("hud_screen_scene.erp");
        let pre = swofty_formats::uipre::Uipre::parse(
            &scene.find(screens::STACKED_UIPRE).unwrap().fragments[0]
                .data()
                .unwrap(),
        )
        .unwrap();
        for (me, other) in [(a, b), (b, a)] {
            let inside = pre.element_ids_in(&screens::standings().host_name(me));
            for name in [other.clone(), screen_build::preview_root(other)] {
                assert!(
                    inside.contains(&swofty_formats::uipre::element_id(&name)),
                    "{name} hidden in {me}'s host in the editor"
                );
            }
        }
        assert!(out.lines.iter().any(|l| l.contains("other screen records")));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn two_addons_share_one_scene_and_one_template_pass() {
        let Some(stock) = game() else { return };
        let dir = addon_dir("two");
        let mk = |addon: &str| Input {
            addon: addon.into(),
            key: "hud".into(),
            dir: dir.clone(),
            record: HudElement::parse(&record()).unwrap(),
        };
        let out = compose(&stock, &[mk("one.hud"), mk("two.hud")]).unwrap();
        let tmpl = Erp::parse(&out.archives[1].bytes).unwrap();
        let t = Pkg::parse(&tmpl.find(dials::TEMPLATE).unwrap().fragments[0].data().unwrap()).unwrap();
        for p in ["one_hud", "two_hud"] {
            assert!(
                crate::ui::node::find_path(&t.root, &format!("{}.{p}_ctl", dials::F126.race_parent)).is_ok()
            );
        }
        let clash = compose(&stock, &[mk("one.hud"), mk("one.hud")]);
        assert!(clash.unwrap_err().to_string().contains("both get the prefix"));
        std::fs::remove_dir_all(dir).ok();
    }
}
