//! Assertions of the stock facts the composer relies on, against the game files, and the checks of
//! a composed result (`swofty hud check`). A game update that moves one of them fails here, not in
//! the game.

use std::collections::BTreeSet;

use swofty_formats::hash::djb2;
use swofty_formats::pkg::Pkg;

use super::dials::{self, Use};
use super::editor;
use super::model::Dial;
use super::scene::Builder;
use super::template::StockTags;
use super::units;
use super::{Stock, first, screen_build, screens, stock_edit};
use crate::ui::idf;
use crate::ui::node::{self, NodeExt};

#[derive(Debug, Clone, Default)]
pub struct Report {
    pub ok: Vec<String>,
    pub problems: Vec<String>,
}

impl Report {
    fn pass(&mut self, line: impl Into<String>) {
        self.ok.push(line.into());
    }

    fn fail(&mut self, line: impl Into<String>) {
        self.problems.push(line.into());
    }

    fn run<T>(&mut self, what: &str, r: crate::Result<T>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                self.fail(format!("{what}: {e}"));
                None
            }
        }
    }

    pub fn is_ok(&self) -> bool {
        self.problems.is_empty()
    }
}

fn identical(stock: &dyn Stock, erps: &[&str], name: &str, r: &mut Report) -> Option<Vec<u8>> {
    let mut found: Option<Vec<u8>> = None;
    for erp in erps {
        let data = match stock.read(erp, name).and_then(|f| first(f, name)) {
            Ok(d) => d,
            Err(e) => {
                r.fail(format!("{name} in {erp}: {e}"));
                return None;
            }
        };
        match &found {
            None => found = Some(data),
            Some(f) if *f != data => {
                r.fail(format!("{name}: {erp} carries a different copy than {}", erps[0]));
                return None;
            }
            _ => {}
        }
    }
    r.pass(format!("{name}: the same in {} archives", erps.len()));
    found
}

/// Checks that every archive carrying `name` holds the same stock copy and that the archives the
/// composer makes it for are among them; the composed copy, made from one of them, goes to all.
fn carried(stock: &dyn Stock, made_for: &[&str], name: &str, r: &mut Report) -> Option<Vec<u8>> {
    let carriers = match stock.carriers(&[name]) {
        Ok(mut c) => c.remove(name).unwrap_or_default(),
        Err(e) => {
            r.fail(format!("{name}: {e}"));
            return None;
        }
    };
    if carriers.is_empty() {
        r.fail(format!("{name}: no stock archive carries it"));
        return None;
    }
    if let Some(e) = made_for.iter().find(|e| !carriers.iter().any(|c| c == *e)) {
        r.fail(format!("{name}: {e} does not carry it"));
        return None;
    }
    let all: Vec<&str> = carriers.iter().map(String::as_str).collect();
    let found = identical(stock, &all, name, r)?;
    r.pass(format!(
        "{}: carried by {}; the composed copy goes to every one",
        short(name),
        all.iter().map(|e| short(e)).collect::<Vec<_>>().join(", ")
    ));
    Some(found)
}

/// Every stock copy a composed result leaves beside a composed one: for each stock resource in
/// `archives`, each archive that carries it and that no archive of the result is overlaid on.
pub fn uncovered(stock: &dyn Stock, archives: &[super::Built]) -> crate::Result<Vec<String>> {
    let mut lands: std::collections::BTreeMap<String, BTreeSet<&str>> = std::collections::BTreeMap::new();
    for a in archives {
        let erp = swofty_formats::erp::Erp::parse(&a.bytes)?;
        for res in erp.resources {
            lands
                .entry(res.name)
                .or_default()
                .extend(a.erps.iter().map(String::as_str));
        }
    }
    let names: Vec<&str> = lands.keys().map(String::as_str).collect();
    let carriers = stock.carriers(&names)?;
    let mut out = Vec::new();
    for (name, at) in &lands {
        for c in carriers.get(name).into_iter().flatten() {
            if !at.contains(c.as_str()) {
                out.push(format!(
                    "{name} is composed, but {c} also carries it and would keep the stock copy"
                ));
            }
        }
    }
    Ok(out)
}

struct ClipBytes {
    name: String,
    temp: Vec<u8>,
    syst: Vec<u8>,
}

fn clips(archives: &[super::Built]) -> crate::Result<Vec<ClipBytes>> {
    let mut out = Vec::new();
    for a in archives {
        let erp = swofty_formats::erp::Erp::parse(&a.bytes)?;
        for res in &erp.resources {
            if res.kind() != "AnimClip" || res.fragments.len() < 2 {
                continue;
            }
            out.push(ClipBytes {
                name: res.name.clone(),
                temp: res.fragments[0].data()?,
                syst: res.fragments[1].data()?,
            });
        }
    }
    Ok(out)
}

/// Every composed clip in `archives` that carries a one-key rotation channel. The game copies
/// that quaternion with an aligned 16-byte load straight from the clip's `syst` buffer, which it
/// can place 8 bytes past a 16-byte boundary; the HUD load then faults.
pub fn constant_rotations(archives: &[super::Built]) -> crate::Result<Vec<String>> {
    let mut out = Vec::new();
    for c in clips(archives)? {
        if swofty_formats::animclip::has_constant_rotation(&c.temp, &c.syst)? {
            out.push(c.name);
        }
    }
    Ok(out)
}

/// Every composed clip in `archives` whose `syst` length is not a multiple of 16. The game can
/// place such a buffer 8 bytes past a 16-byte boundary (`animclip::SYST_ALIGN`), and the aligned
/// loads of its rotation keys then fault.
pub fn unpadded_clips(archives: &[super::Built]) -> crate::Result<Vec<String>> {
    Ok(clips(archives)?
        .into_iter()
        .filter(|c| !c.syst.len().is_multiple_of(swofty_formats::animclip::SYST_ALIGN))
        .map(|c| format!("{} ({} bytes)", c.name, c.syst.len()))
        .collect())
}

fn short(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

fn check_parent(root: &swofty_formats::pkg::Node, parent: &str, r: &mut Report) {
    let at = match node::find_path(root, parent) {
        Ok(a) => a,
        Err(e) => return r.fail(format!("{parent}: {e}")),
    };
    let Ok(n) = node::node_at(root, &at) else {
        return r.fail(format!("{parent}: gone"));
    };
    let id = n
        .props()
        .ok()
        .and_then(|p| p.get("id"))
        .and_then(|v| v.as_val())
        .and_then(|v| v.as_u32());
    if id != Some(djb2(parent)) {
        return r.fail(format!("{parent}: the node's id is not the hash of its path"));
    }
    if n.type_code() != "F1HD" {
        return r.fail(format!("{parent}: {} is not an F1HD", n.type_code()));
    }
    for cmd in ["show", "hide"] {
        let hits = n
            .slot(node::COMMANDS)
            .map(|c| {
                c.iter()
                    .filter(|c| {
                        c.as_obj()
                            .and_then(|o| o.get("commandName"))
                            .and_then(|v| v.as_val())
                            .and_then(|v| v.as_str())
                            == Some(cmd)
                    })
                    .count()
            })
            .unwrap_or(0);
        if hits != 1 {
            r.fail(format!("{parent}: {hits} {cmd} commands, one expected"));
            return;
        }
    }
    r.pass(format!("{parent}: an F1HD with show and hide"));
}

/// Checks the stock facts of every dial against `stock`.
pub fn stock_facts(stock: &dyn Stock) -> Report {
    let mut r = Report::default();
    let read = |name: &str| first(stock.read(dials::STOCK_ERP, name)?, name);
    let mods = r.run("dial_mods.idf", read(dials::MODS));
    let bar = r.run("dial_om_bar_complete.idf", read(dials::BAR));
    if let Some(m) = &mods
        && let Ok(p) = Pkg::parse(m)
        && let Ok(root) = idf::scenes(&p).map(|s| s[0].clone())
    {
        let scene = root.as_obj().and_then(|o| o.get("scene"));
        for n in ["mod_1_icon_fill_mask", "mod_1_icon_fill_mask_anim"] {
            match scene.and_then(|s| idf::find(s, n)) {
                Some(_) => r.pass(format!("dial_mods.idf has {n}")),
                None => r.fail(format!("dial_mods.idf has no {n}")),
            }
        }
    }
    if let Some(b) = &bar
        && let Ok(p) = Pkg::parse(b)
    {
        let has = idf::scenes(&p)
            .ok()
            .and_then(|s| {
                s[0].as_obj()?
                    .get("scene")
                    .map(|s| idf::find(s, "ers_battery_fill_mask").is_some())
            })
            .unwrap_or(false);
        if has {
            r.pass("dial_om_bar_complete.idf has ers_battery_fill_mask");
        } else {
            r.fail("dial_om_bar_complete.idf has no ers_battery_fill_mask");
        }
    }
    let clip = r.run("clip template", read(dials::CLIP_TEMPLATE));
    if let Some(c) = clip {
        match Pkg::parse(&c) {
            Ok(_) => r.pass("the clip template parses"),
            Err(e) => r.fail(format!("the clip template: {e}")),
        }
    }
    if let Some(m) = r.run("fill material", read(dials::FILL_MATERIAL)) {
        match super::textures::reencode_material(&m) {
            Ok(b) if b == m => r.pass("the fill material re-encodes to its own bytes"),
            Ok(_) => r.fail("the fill material does not re-encode to its own bytes"),
            Err(e) => r.fail(format!("the fill material: {e}")),
        }
    }
    let om_xm = r.run("dial_om_xm.idf", read(dials::F126.scene));
    let scene_erps: Vec<&str> = dials::SCENE_ERPS.to_vec();
    for dial in Dial::ALL {
        let f = dials::facts(dial);
        let Some(scene) = carried(stock, &scene_erps, f.scene, &mut r) else {
            continue;
        };
        carried(stock, &scene_erps, f.strtab, &mut r);
        let text_from = (dial != Dial::F126).then(|| om_xm.clone()).flatten();
        if let (Some(m), Some(b)) = (&mods, &bar) {
            let built = Builder::new(f, &scene, m, b, text_from.as_deref());
            if let Some(mut b) = r.run(&format!("{}: scene", short(f.scene)), built) {
                r.pass(format!(
                    "{}: the quad {} has the mesh, extents and pivot the composer expects",
                    short(f.scene),
                    f.quad
                ));
                let parts: Vec<String> = dials::STOCK_KIDS.iter().map(|s| (*s).to_owned()).collect();
                if r.run(
                    &format!("{}: stock parts", short(f.scene)),
                    b.stock_parts_present(&parts),
                )
                .is_some()
                {
                    r.pass(format!(
                        "{}: the stock parts {} are under {}",
                        short(f.scene),
                        parts.join(", "),
                        dials::CUSTOMISATION
                    ));
                }
                match b.bottom() {
                    Ok(y) if (y - f.bottom_y).abs() < 1e-3 => {
                        r.pass(format!("{}: br_customisation sits at y {y}", short(f.scene)))
                    }
                    Ok(y) => r.fail(format!(
                        "{}: br_customisation is at y {y}, not {}",
                        short(f.scene),
                        f.bottom_y
                    )),
                    Err(e) => r.fail(format!("{}: {e}", short(f.scene))),
                }
            }
        }
        if let Some(strtab) = stock
            .read(dials::STOCK_ERP, f.strtab)
            .ok()
            .and_then(|v| first(v, "strtab").ok())
        {
            match swofty_formats::animclip::read_strings(&strtab) {
                Ok(s) if s.iter().any(|n| n.starts_with("zh_")) => r.pass(format!(
                    "{}: the anim string table already carries zh_ names (a built game folder)",
                    short(f.scene)
                )),
                Ok(s) => r.pass(format!(
                    "{}: the anim string table has {} stock names",
                    short(f.scene),
                    s.len()
                )),
                Err(e) => r.fail(format!("{}: {e}", short(f.strtab))),
            }
        }
        if dial == Dial::F125
            && let Some(u) = stock
                .read(dials::STOCK_ERP, f.uipre)
                .ok()
                .and_then(|v| first(v, "uipre").ok())
        {
            match editor::uipre(&u, f.scene, &[dials::STOCK_KIDS[0].to_owned()], &[]) {
                Ok(_) => r.pass("dial_complete.uipre: the stock setters to copy are there"),
                Err(e) => r.fail(format!("dial_complete.uipre: {e}")),
            }
        }
    }
    let tmpl_erps: Vec<&str> = dials::TEMPLATE_ERPS.to_vec();
    let spec_erps: Vec<&str> = dials::SPECTATOR_ERPS.to_vec();
    let template = carried(stock, &tmpl_erps, dials::TEMPLATE, &mut r);
    let spectator = carried(stock, &spec_erps, dials::SPECTATOR, &mut r);
    let race = template.as_deref().and_then(|b| Pkg::parse(b).ok());
    let spec = spectator.as_deref().and_then(|b| Pkg::parse(b).ok());
    if let Some(p) = &race {
        for d in Dial::ALL {
            check_parent(&p.root, dials::facts(d).race_parent, &mut r);
        }
    }
    if let Some(p) = &spec {
        for d in Dial::ALL {
            check_parent(&p.root, dials::facts(d).spectator_parent, &mut r);
        }
    }
    if let Some(p) = &race {
        let tags = StockTags::race(&p.root);
        let mut checked = BTreeSet::new();
        for g in dials::GAME_VALUES.iter().chain(dials::SESSION_VALUES) {
            if g.usage == Use::DecisionOnly && g.name == "drs_pit_state" {
                continue;
            }
            match tags.get(g.db) {
                Some(have) if have.contains(&g.tag) => {
                    checked.insert(g.name);
                }
                Some(have) => r.fail(format!(
                    "{}: the table says {}, the stock template binds {}",
                    g.name,
                    dials::tag_name(&g.tag),
                    have.iter().map(dials::tag_name).collect::<Vec<_>>().join("/")
                )),
                None if g.in_stock => r.fail(format!("{}: no stock node binds {}", g.name, g.db)),
                None => {}
            }
        }
        r.pass(format!(
            "{} game values match the stock template's tags",
            checked.len()
        ));
        for d in [Dial::F125, Dial::F2] {
            let parent = dials::facts(d).race_parent;
            let mut paths = Vec::new();
            node::node_paths_of(&p.root, "", &mut paths);
            let drs = paths
                .iter()
                .filter(|(path, n)| {
                    path.starts_with(&format!("{parent}."))
                        && n.type_code() == "MVCD"
                        && n.props()
                            .ok()
                            .and_then(|p| p.get("dbEntry"))
                            .and_then(|v| v.as_obj())
                            .and_then(|o| o.get("value"))
                            .and_then(|v| v.as_val())
                            .and_then(|v| v.as_str())
                            == Some("drs_pit_state")
                })
                .count();
            if drs == 1 {
                r.pass(format!("{parent}: one stock MVCD on drs_pit_state"));
            } else {
                r.fail(format!(
                    "{parent}: {drs} stock MVCDs on drs_pit_state, one expected"
                ));
            }
        }
    }
    screen_facts(stock, &mut r);
    for f in units::stock_facts(stock) {
        match f {
            Ok(line) => r.pass(line),
            Err(e) => r.fail(e.to_string()),
        }
    }
    r
}

fn root_of(bytes: &[u8]) -> Option<(Pkg, swofty_formats::pkg::Node)> {
    let p = Pkg::parse(bytes).ok()?;
    let root = stock_edit::scene_root(&p).ok()?.clone();
    Some((p, root))
}

fn xref_facts(
    root: &swofty_formats::pkg::Node,
    name: &str,
    file_ref: Option<&str>,
    scale: Option<f64>,
    scene: &str,
    r: &mut Report,
) {
    let hits: Vec<&swofty_formats::pkg::Node> = idf::elements(root)
        .into_iter()
        .filter(|(_, e)| idf::elem_name(e) == name)
        .map(|(_, e)| e)
        .collect();
    let [e] = hits[..] else {
        return r.fail(format!(
            "{scene}: {} elements named {name}, one expected",
            hits.len()
        ));
    };
    let ty = crate::ui::catalog::element_type(e);
    if ty != "eXRef" {
        return r.fail(format!("{scene}: {name} is {ty}, not an eXRef"));
    }
    let Some(have) = idf::param_str(e, "fileRef").map(str::to_owned) else {
        return r.fail(format!("{scene}: {name} has no fileRef"));
    };
    let file_ref = file_ref.unwrap_or(&have);
    if have != file_ref || idf::xref(e) != Some(idf::scene_id(file_ref)) {
        return r.fail(format!("{scene}: {name} does not reference {file_ref}"));
    }
    if idf::param_str(e, "XrefMaterialBinding") != Some(idf::material_binding(file_ref).as_str()) {
        return r.fail(format!("{scene}: {name}'s material binding is not {file_ref}'s"));
    }
    if idf::param(e, "isHidden").is_some() {
        return r.fail(format!("{scene}: {name} carries isHidden"));
    }
    if let Some(want) = scale {
        let v = e
            .as_obj()
            .and_then(|o| o.get("scale"))
            .and_then(swofty_formats::pkg::Node::as_val);
        let ok = v.is_some_and(|v| match &v.value {
            swofty_formats::pkg::Value::Raw(b) if b.len() >= 12 => (0..3).all(|i| {
                (f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap_or([0; 4])) as f64 - want).abs()
                    < 1e-5
            }),
            _ => false,
        });
        if !ok {
            return r.fail(format!("{scene}: {name} is not at scale {want}"));
        }
    }
    r.pass(format!("{scene}: {name} references {}", short(file_ref)));
}

fn near_tag(root: &swofty_formats::pkg::Node, scope: &str, db: &str) -> Option<BTreeSet<[u8; 4]>> {
    let at = node::find_path(root, scope).ok()?;
    let mut all = Vec::new();
    node::bindings(node::node_at(root, &at).ok()?, &[], &mut all);
    let tags: BTreeSet<[u8; 4]> = all.iter().filter(|b| b.db == db).filter_map(|b| b.tag).collect();
    (!tags.is_empty()).then_some(tags)
}

/// The facts of the `screen` targets (`hud/screens.rs`): the host cap, the standings board, the lap
/// counter, the replay and spectator screens and the Race Control toast scenes.
pub fn screen_facts(stock: &dyn Stock, r: &mut Report) {
    let race = |name: &str| first(stock.read(screens::FLOW_RACE, name)?, name);
    let scene_erps: Vec<&str> = screen_build::SCREEN_SCENE_ERPS.to_vec();
    let cap = carried(stock, &scene_erps, screens::CAP, r);
    carried(stock, &scene_erps, screen_build::CAP_FACTS.strtab, r);
    carried(stock, &scene_erps, screens::STACKED_UIPRE, r);
    let mods = race(dials::MODS).ok();
    let bar = race(dials::BAR).ok();
    let om_xm = race(dials::F126.scene).ok();
    if let (Some(c), Some(m), Some(b), Some(x)) = (&cap, &mods, &bar, &om_xm) {
        match Builder::new(&screen_build::CAP_FACTS, c, m, b, Some(x)) {
            Ok(_) => r.pass(format!(
                "{}: the quad {} has the {} x {} mesh the composer expects",
                short(screens::CAP),
                screens::CAP_MESH,
                screens::CAP_MESH_SIZE.0,
                screens::CAP_MESH_SIZE.1
            )),
            Err(e) => r.fail(format!("{}: {e}", short(screens::CAP))),
        }
        if let Some((_, root)) = root_of(c) {
            for n in [screens::CAP_NULL, screens::CAP_ANIM] {
                match idf::find(&root, n) {
                    Some(_) => r.pass(format!("{}: has {n}", short(screens::CAP))),
                    None => r.fail(format!("{}: has no {n}", short(screens::CAP))),
                }
            }
        }
    }
    if let Some((_, root)) = carried(stock, &scene_erps, screens::STACKED, r).and_then(|b| root_of(&b)) {
        xref_facts(
            &root,
            screens::STACKED_CAP_REF,
            Some(screens::CAP),
            None,
            short(screens::STACKED),
            r,
        );
        match idf::find(&root, screens::STACKED_MASK) {
            Some(_) => r.pass(format!(
                "{}: has {}",
                short(screens::STACKED),
                screens::STACKED_MASK
            )),
            None => r.fail(format!(
                "{}: has no {}",
                short(screens::STACKED),
                screens::STACKED_MASK
            )),
        }
        match idf::find(&root, screens::STACKED_STACKER).and_then(|e| idf::param_str(e, "stackerType")) {
            Some("t") => r.pass(format!(
                "{}: {} stacks top-down",
                short(screens::STACKED),
                screens::STACKED_STACKER
            )),
            other => r.fail(format!(
                "{}: {} stackerType {other:?}, t expected",
                short(screens::STACKED),
                screens::STACKED_STACKER
            )),
        }
    }
    if let Ok(u) = race(screens::STACKED_UIPRE) {
        match editor::uipre(&u, screens::STACKED, &[screens::STACKED_MASK.to_owned()], &[]) {
            Ok(_) => r.pass(format!(
                "{}: the mask can be hidden in the editor",
                short(screens::STACKED_UIPRE)
            )),
            Err(e) => r.fail(format!("{}: {e}", short(screens::STACKED_UIPRE))),
        }
    }
    let template = race(dials::TEMPLATE).ok().and_then(|b| Pkg::parse(&b).ok());
    if let Some(p) = &template {
        let parent = screens::STANDINGS_PARENT;
        match node::find_path(&p.root, parent).and_then(|at| node::node_at(&p.root, &at).cloned()) {
            Ok(n) => {
                let count = |cmd: &str| {
                    n.slot(node::COMMANDS)
                        .map(|c| {
                            c.iter()
                                .filter(|c| {
                                    c.as_obj()
                                        .and_then(|o| o.get("commandName"))
                                        .and_then(swofty_formats::pkg::Node::as_val)
                                        .and_then(swofty_formats::pkg::Val::as_str)
                                        == Some(cmd)
                                })
                                .count()
                        })
                        .unwrap_or(0)
                };
                if count("enable") == 1 && count("disable") == 1 {
                    r.pass(format!("{parent}: one enable and one disable command"));
                } else {
                    r.fail(format!("{parent}: enable/disable commands are not one each"));
                }
            }
            Err(e) => r.fail(format!("{parent}: {e}")),
        }
        match node::find_path(&p.root, screens::STANDINGS_CAP_NODE)
            .and_then(|at| node::node_at(&p.root, &at).cloned())
        {
            Ok(n)
                if n.slot(node::CHILDREN)
                    .map(|k| k.iter().all(|c| !c.is_node()))
                    .unwrap_or(true) =>
            {
                r.pass(format!("{}: has no children", screens::STANDINGS_CAP_NODE));
            }
            Ok(_) => r.fail(format!("{}: has children", screens::STANDINGS_CAP_NODE)),
            Err(e) => r.fail(format!("{}: {e}", screens::STANDINGS_CAP_NODE)),
        }
        if let Some((_, root)) = race(screens::LAPC_SCENE).ok().and_then(|b| root_of(&b)) {
            let mut top: Vec<&str> = root
                .as_obj()
                .and_then(|o| o.get("children"))
                .and_then(swofty_formats::pkg::Node::as_arr)
                .map(|a| a.items.iter().map(idf::elem_name).collect())
                .unwrap_or_default();
            top.sort_unstable();
            let mut want = screens::LAPC_STOCK_KIDS.to_vec();
            want.sort_unstable();
            if top == want {
                r.pass(format!(
                    "{}: the top-level parts are the stock eight",
                    short(screens::LAPC_SCENE)
                ));
            } else {
                r.fail(format!(
                    "{}: the top-level parts changed: {top:?}",
                    short(screens::LAPC_SCENE)
                ));
            }
            let s = stock_edit::StockNodes {
                template: screens::LAPC_PARENT.to_owned(),
                scene: Some(screens::LAPC_SCENE.to_owned()),
                hide: screens::LAPC_HIDE.iter().map(|s| (*s).to_owned()).collect(),
                wrap: Vec::new(),
                while_: None,
            };
            match stock_edit::check_stock_nodes(&root, &p.root, &s) {
                Ok(()) => r.pass(format!(
                    "{}: the {} parts the tower hides are shown by default and bound by no stock node",
                    short(screens::LAPC_SCENE),
                    screens::LAPC_HIDE.len()
                )),
                Err(e) => r.fail(format!("{}: {e}", short(screens::LAPC_SCENE))),
            }
            match stock_edit::scene_of(&p.root, screens::LAPC_PARENT) {
                Ok(Some(sc)) if sc == screens::LAPC_SCENE => {
                    r.pass(format!("{} instances {}", screens::LAPC_PARENT, short(&sc)))
                }
                other => r.fail(format!(
                    "{} instances {other:?}, not the lap counter",
                    screens::LAPC_PARENT
                )),
            }
        }
        for (rel, fills, twin, hide, group) in stock_edit::BANNER_SCENES {
            let scene = stock_edit::banner_scene(rel);
            let Some((_, root)) = race(&scene).ok().and_then(|b| root_of(&b)) else {
                r.fail(format!("{}: missing in flow_race.erp", short(&scene)));
                continue;
            };
            let mut ok = true;
            for (el, slot) in fills {
                let mesh = stock_edit::mesh_resource(&scene, el);
                let mut want = slot.as_bytes().to_vec();
                want.push(0);
                let has = stock
                    .read(screens::FLOW_RACE, &mesh)
                    .map(|f| f.iter().any(|f| f.windows(want.len()).any(|w| w == want)))
                    .unwrap_or(false);
                if !has {
                    ok = false;
                    r.fail(format!("{}: the mesh of {el} has no slot {slot}", short(&scene)));
                }
                let offset = idf::find(&root, el)
                    .and_then(|e| idf::param(e, "eLayer.layerIsOffset"))
                    .and_then(stock_edit::as_bool)
                    .unwrap_or(false);
                if offset != (twin == Some(*el)) {
                    ok = false;
                    r.fail(format!(
                        "{}: {el} layerIsOffset {offset}, twin {twin:?}",
                        short(&scene)
                    ));
                }
            }
            for el in hide {
                let n = idf::elements(&root)
                    .iter()
                    .filter(|(_, e)| idf::elem_name(e) == *el)
                    .count();
                let hidden = idf::find(&root, el)
                    .and_then(|e| idf::param(e, "isHidden"))
                    .and_then(stock_edit::as_bool)
                    .unwrap_or(false);
                if n != 1 || hidden {
                    ok = false;
                    r.fail(format!("{}: {el}: {n} elements, hidden {hidden}", short(&scene)));
                }
            }
            for toast in stock_edit::BANNER_TOASTS {
                let gp = format!("{toast}.osd_visibility_option.{toast}.{group}");
                if node::find_path(&p.root, &gp).is_err() {
                    ok = false;
                    r.fail(format!("{gp}: no such template node"));
                }
            }
            if ok {
                r.pass(format!(
                    "{}: {} fills, {} decorations, both toast groups",
                    short(&scene),
                    fills.len(),
                    hide.len()
                ));
            }
        }
        if let Some((_, root)) = race(stock_edit::BANNER_SYSTEM).ok().and_then(|b| root_of(&b)) {
            let top: Vec<&str> = root
                .as_obj()
                .and_then(|o| o.get("children"))
                .and_then(swofty_formats::pkg::Node::as_arr)
                .map(|a| a.items.iter().map(idf::elem_name).collect())
                .unwrap_or_default();
            if top == stock_edit::BANNER_PARTS {
                r.pass(format!("{}: the four parts", short(stock_edit::BANNER_SYSTEM)));
            } else {
                r.fail(format!("{}: parts {top:?}", short(stock_edit::BANNER_SYSTEM)));
            }
        }
    }
    let replay_erps = [screens::FLOW_REPLAY, screens::REPLAY_SCREEN_ERP];
    if let Some(p) = carried(stock, &replay_erps, screens::REPLAY_UI, r).and_then(|b| Pkg::parse(&b).ok()) {
        match node::find_path(&p.root, screens::REPLAY_ROOT)
            .and_then(|at| node::node_at(&p.root, &at).cloned())
        {
            Ok(n) if n.type_code() == "UISC" => r.pass(format!("{}: a UISC", screens::REPLAY_ROOT)),
            Ok(n) => r.fail(format!(
                "{}: a {}, not a UISC",
                screens::REPLAY_ROOT,
                n.type_code()
            )),
            Err(e) => r.fail(format!("{}: {e}", screens::REPLAY_ROOT)),
        }
        for leaf in ["show_drivers_list", "is_video_sting_enabled"] {
            match near_tag(&p.root, screens::REPLAY_ROOT, leaf) {
                Some(t) if t == BTreeSet::from([*b"bool"]) => {
                    r.pass(format!("{}: binds {leaf} as bool", screens::REPLAY_ROOT))
                }
                other => r.fail(format!("{}: binds {leaf} as {other:?}", screens::REPLAY_ROOT)),
            }
        }
    }
    let replay_scene_erps = [
        screens::FLOW_REPLAY,
        screens::REPLAY_SCREEN_ERP,
        screens::FLOW_FRONTEND,
    ];
    if let Some((_, root)) =
        carried(stock, &replay_scene_erps, screens::REPLAY_SCENE, r).and_then(|b| root_of(&b))
    {
        let sc = short(screens::REPLAY_SCENE);
        match idf::find(&root, screens::REPLAY_GROUP) {
            Some(_) => r.pass(format!("{sc}: has {}", screens::REPLAY_GROUP)),
            None => r.fail(format!("{sc}: has no {}", screens::REPLAY_GROUP)),
        }
        xref_facts(
            &root,
            screens::REPLAY_TABLE,
            Some(screens::REPLAY_TABLE_REF),
            Some(screens::REPLAY_BOARD_SCALE),
            sc,
            r,
        );
        xref_facts(
            &root,
            screens::REPLAY_HEADER,
            Some(screens::REPLAY_HEADER_REF),
            Some(screens::REPLAY_BOARD_SCALE),
            sc,
            r,
        );
        match idf::find(&root, screens::REPLAY_HEADER).and_then(idf::position) {
            Some(p)
                if (p[0] - screens::REPLAY_POS[0]).abs() < 1e-4
                    && (p[1] - screens::REPLAY_POS[1]).abs() < 1e-4 =>
            {
                r.pass(format!("{sc}: the board starts at {:?}", screens::REPLAY_POS));
            }
            other => r.fail(format!("{sc}: the board moved to {other:?}")),
        }
    }
    let spec_erps: Vec<&str> = dials::SPECTATOR_ERPS.to_vec();
    if let Some(p) = race(dials::SPECTATOR).ok().and_then(|b| Pkg::parse(&b).ok()) {
        match node::find_path(&p.root, screens::SPECT_PARENT)
            .and_then(|at| node::node_at(&p.root, &at).cloned())
        {
            Ok(n) if n.type_code() == "UIGP" => r.pass(format!("{}: a UIGP", screens::SPECT_PARENT)),
            Ok(n) => r.fail(format!(
                "{}: a {}, not a UIGP",
                screens::SPECT_PARENT,
                n.type_code()
            )),
            Err(e) => r.fail(format!("{}: {e}", screens::SPECT_PARENT)),
        }
        match near_tag(&p.root, screens::SPECT_PARENT, screens::SPECT_SHOW) {
            Some(t) if t == BTreeSet::from([*b"bool"]) => r.pass(format!(
                "{}: binds {} as bool",
                screens::SPECT_PARENT,
                screens::SPECT_SHOW
            )),
            other => r.fail(format!(
                "{}: binds {} as {other:?}",
                screens::SPECT_PARENT,
                screens::SPECT_SHOW
            )),
        }
    }
    if let Some((_, root)) = carried(stock, &spec_erps, screens::SPECT_TABLE, r).and_then(|b| root_of(&b)) {
        let sc = short(screens::SPECT_TABLE);
        let top: Vec<&str> = root
            .as_obj()
            .and_then(|o| o.get("children"))
            .and_then(swofty_formats::pkg::Node::as_arr)
            .map(|a| a.items.iter().map(idf::elem_name).collect())
            .unwrap_or_default();
        if top == [screens::SPECT_GROUP] {
            r.pass(format!("{sc}: the root holds {} alone", screens::SPECT_GROUP));
        } else {
            r.fail(format!("{sc}: the root holds {top:?}"));
        }
        xref_facts(&root, screens::SPECT_CAP_SRC, None, None, sc, r);
    }
    if let Some((_, root)) = race(screens::SPECT_SCREEN).ok().and_then(|b| root_of(&b)) {
        let sc = short(screens::SPECT_SCREEN);
        xref_facts(
            &root,
            screens::SPECT_TABLE_EL,
            Some(screens::SPECT_TABLE),
            Some(1.0),
            sc,
            r,
        );
        xref_facts(
            &root,
            screens::SPECT_HEADER_EL,
            Some(screens::SPECT_HEADER),
            Some(1.0),
            sc,
            r,
        );
        match idf::find(&root, screens::SPECT_HEADER_EL).and_then(idf::position) {
            Some(p)
                if (p[0] - screens::REPLAY_POS[0]).abs() < 1e-4
                    && (p[1] - screens::REPLAY_POS[1]).abs() < 1e-4 =>
            {
                r.pass(format!("{sc}: the board starts at {:?}", screens::REPLAY_POS));
            }
            other => r.fail(format!("{sc}: the board moved to {other:?}")),
        }
    }
}

fn strip(n: &mut swofty_formats::pkg::Node, ours: &BTreeSet<String>) {
    use swofty_formats::pkg::Node;
    if n.is_node() {
        if let Ok(kids) = n.slot_mut(node::CHILDREN) {
            kids.retain(|c| !(c.is_node() && ours.contains(c.name())));
            kids.iter_mut().for_each(|c| strip(c, ours));
        }
        for slot in [node::COMMANDS, node::EVENTS] {
            let Ok(list) = n.slot_mut(slot) else { continue };
            for entry in list.iter_mut() {
                let Some(sigs) = entry
                    .as_obj_mut()
                    .and_then(|o| o.get_mut("signals"))
                    .and_then(Node::as_arr_mut)
                else {
                    continue;
                };
                sigs.items.retain(|s| {
                    let to = s
                        .as_obj()
                        .and_then(|o| o.get("to_object_name"))
                        .and_then(Node::as_val)
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    !ours.contains(to.rsplit('.').next().unwrap_or(to))
                });
            }
        }
        return;
    }
    match n {
        Node::Arr(a) => a.items.iter_mut().for_each(|c| strip(c, ours)),
        Node::Obj(o) => o.items.iter_mut().for_each(|(_, v)| strip(v, ours)),
        _ => {}
    }
}

/// True when `built`, without the groups named in `ours` and the signals that target them, is
/// the stock file node for node and value for value (a file's value tables are not compared: new
/// values interleave with the stock ones).
pub fn stock_kept(stock: &Pkg, built: &Pkg, ours: &BTreeSet<String>) -> crate::Result<bool> {
    let mut b = built.clone();
    strip(&mut b.root, ours);
    let (mut x, mut y) = (Vec::new(), Vec::new());
    node::node_paths_of(&stock.root, "", &mut x);
    node::node_paths_of(&b.root, "", &mut y);
    if x.len() != y.len() {
        return Ok(false);
    }
    let slots = regex::Regex::new(r"slot: (Some\(\([0-9, ]+\)\)|None)").expect("a valid pattern");
    let plain = |n: &swofty_formats::pkg::Node| slots.replace_all(&format!("{n:?}"), "").into_owned();
    Ok(x.iter()
        .zip(&y)
        .all(|((p, a), (q, b))| p == q && plain(a) == plain(b)))
}
