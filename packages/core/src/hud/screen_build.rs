//! Composing `screen` records: the elements grow the host scene (the standings board's bottom cap)
//! once; every screen the record names gets a clone of the host instanced into its own stock scene
//! and a `<prefix>_ctl` group in its template that shows the instance and masks the stock board
//! while the record's `show` leaf is true. `stock_nodes` and `restyle` ride along.
//!
//! The instance, mask and control-group shapes are Project Halo's `tower_native.py`
//! (`build_stacked`, `build_template`, `build_replay_scene`, `build_replay_template`,
//! `build_spectator_scene`, `build_spectator_template`); Uses work from Project Halo by nyannoying
//! (https://github.com/nyannoying1337/f1-25-project-halo).

use std::collections::{BTreeMap, BTreeSet};

use swofty_formats::emb;
use swofty_formats::pkg::{Node, Pkg, Val};

use super::dials::{self, DialFacts};
use super::model::{Bind, Decision, Dial};
use super::scene::{Builder, Mode, Plan, SpriteInfo, Sprites};
use super::screens::{self, At, ScreenFacts, Target};
use super::stock_edit::{self, Restyle, StockNodes};
use super::template::{self, StockTags, Usage};
use super::textures::{self, Res};
use super::units::{self, TexFacts};
use super::{Input, Stock, art_bounds, editor, first, walk};
use crate::ui::idf;
use crate::ui::node::{self, NodeExt, ObjExt};
use crate::{Error, Result};

fn err(msg: impl std::fmt::Display) -> Error {
    Error::Other(format!("hud: {msg}"))
}

/// The host scene as the scene builder sees it: the bottom cap, its `pill_base` quad as the sprite
/// mesh, the standings parent as its template node.
pub const CAP_FACTS: DialFacts = DialFacts {
    dial: Dial::F126,
    scene: screens::CAP,
    strtab: "eaid://ui_package/art/osd/units/driver_position_table/driver_position_bottom_cap.idf?context=animstringtable&file=0",
    uipre: "eaid://ui_package/art/osd/units/driver_position_table/driver_position_bottom_cap.uipre",
    quad: screens::CAP_MESH,
    race_parent: screens::STANDINGS_PARENT,
    spectator_parent: screens::SPECT_PARENT,
    bottom_y: 0.0,
    quad_size: Some(screens::CAP_MESH_SIZE),
};

/// A stock board instances the host scene once and the composer adds one more beside it.
const HOST_INSTANCES: usize = 2;

/// What the screen records added, per archive.
#[derive(Debug, Default)]
pub struct Out {
    /// Into `hud_screen_scene.erp`: the host scene, the instance scene of the standings, its `.uipre`,
    /// the sprites and the restyled scenes.
    pub scene: Vec<Res>,
    pub replay_template: Vec<Res>,
    pub replay_scene: Vec<Res>,
    /// Into `hud_spectator.erp` beside the spectator template.
    pub spectator_scene: Vec<Res>,
    pub spectator_used: bool,
    pub lines: Vec<String>,
}

struct Rec<'a> {
    input: &'a Input,
    pfx: String,
    targets: Vec<Target>,
    sprites: Sprites,
    stock_nodes: Vec<StockNodes>,
    restyles: Vec<Restyle>,
}

/// The editor copy of a record's root in the host scene.
pub fn preview_root(pfx: &str) -> String {
    format!("{pfx}_pv")
}

fn load_sprites(
    stock: &dyn Stock,
    input: &Input,
    pfx: &str,
    fill_material: &[u8],
    out: &mut Vec<Res>,
) -> Result<Sprites> {
    let mut sprites = Sprites::new();
    let key = format!("{pfx}_");
    for (name, s) in &input.record.sprites {
        if let Some(tex) = &s.texture {
            let names = textures::Names {
                tex: tex.clone(),
                ..textures::mesh_names(&key, name, screens::CAP_MESH)
            };
            let tint = s.tint.as_deref().map(textures::hex_rgba).transpose()?;
            out.extend(textures::stock_sprite_resources(&names, fill_material, tint)?);
            out.push(textures::binding_resource(&names, screens::CAP_MESH, false));
            let bounds = match (s.bounds, s.fit) {
                (Some(b), _) => b,
                (None, Some(f)) => textures::fit(textures::texture_size(stock, screens::FLOW_RACE, tex)?, f),
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
        let names = textures::mesh_names(&key, name, screens::CAP_MESH);
        out.extend(textures::sprite_resources(&names, &art, fill_material)?);
        out.push(textures::binding_resource(&names, screens::CAP_MESH, false));
        sprites.insert(
            name.clone(),
            SpriteInfo {
                bounds: art_bounds(s, &art),
                names,
                tinted: None,
            },
        );
    }
    Ok(sprites)
}

fn scene_root_mut(p: &mut Pkg) -> Result<&mut Node> {
    idf::scenes_mut(p)?
        .first_mut()
        .ok_or_else(|| err("the file has no scene"))?
        .o_mut()?
        .at_mut("scene")
}

fn type_name(e: &Node) -> String {
    crate::ui::catalog::element_type(e)
}

fn one<'a>(root: &'a Node, name: &str, scene: &str) -> Result<&'a Node> {
    let hits: Vec<&Node> = idf::elements(root)
        .into_iter()
        .filter(|(_, e)| idf::elem_name(e) == name)
        .map(|(_, e)| e)
        .collect();
    match hits[..] {
        [e] => Ok(e),
        _ => Err(err(format!(
            "{}: {} elements named {name}, one expected",
            scene.rsplit('/').next().unwrap_or(scene),
            hits.len()
        ))),
    }
}

/// The position of the host instance in the instance scene.
pub fn instance_at(
    facts: &ScreenFacts,
    root: &Node,
    read: &dyn Fn(&str) -> Result<Vec<u8>>,
) -> Result<[f64; 2]> {
    Ok(match &facts.instance.at {
        At::Point(p) => *p,
        At::Of(el) => {
            let p = idf::position(one(root, el, &facts.instance.scene)?)
                .ok_or_else(|| err(format!("{el} has no position")))?;
            [p[0], p[1]]
        }
        At::Offset { scene, a, b } => {
            let other = Pkg::parse(&read(scene)?)?;
            let oroot = stock_edit::scene_root(&other)?;
            let pa =
                idf::position(one(oroot, a, scene)?).ok_or_else(|| err(format!("{a} has no position")))?;
            let pb =
                idf::position(one(oroot, b, scene)?).ok_or_else(|| err(format!("{b} has no position")))?;
            [pa[0] - pb[0], pa[1] - pb[1]]
        }
    })
}

/// The instance scene with one host instance per prefix: a clone of the facts' reference, retargeted
/// to the host scene when it is not one already, at the facts' position and scale. Every stock id
/// stays; the lookup grows by the new names.
pub fn add_instances(
    bytes: &[u8],
    facts: &ScreenFacts,
    prefixes: &[String],
    read: &dyn Fn(&str) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    let mut pkg = Pkg::parse(bytes)?;
    let entry = &idf::scenes(&pkg)?[0];
    if idf::scene_name(entry) != Some(idf::scene_id(&facts.instance.scene)) {
        return Err(err(format!("the file is not the scene {}", facts.instance.scene)));
    }
    let root = stock_edit::scene_root(&pkg)?;
    let stock_ids = idf::id_map(root);
    let at = instance_at(facts, root, read)?;
    let src = one(root, &facts.instance.clone, &facts.instance.scene)?;
    if type_name(src) != "eXRef" {
        return Err(err(format!(
            "{}: {} is {}, not an eXRef",
            facts.instance.scene,
            facts.instance.clone,
            type_name(src)
        )));
    }
    if !facts.instance.retarget && idf::param_str(src, "fileRef") != Some(facts.host.scene.as_str()) {
        return Err(err(format!(
            "{}: {} references {:?}, not the host {}",
            facts.instance.scene,
            facts.instance.clone,
            idf::param_str(src, "fileRef").unwrap_or_default(),
            facts.host.scene
        )));
    }
    let mut hosts = Vec::new();
    for pfx in prefixes {
        let name = facts.host_name(pfx);
        if idf::find(root, &name).is_some() {
            return Err(err(format!(
                "{} already has an element {name}",
                facts.instance.scene
            )));
        }
        let mut h = idf::detached(src);
        idf::rename(&mut h, &name)?;
        idf::set_param_like(
            &mut h,
            "guid",
            Val::str(super::scene::guid(&facts.instance.scene, &name)),
        )?;
        if facts.instance.retarget {
            idf::retarget(&mut h, &facts.host.scene)?;
        }
        idf::clear_list(&mut h, "children")?;
        idf::clear_list(&mut h, "anims")?;
        let s = facts.instance.scale;
        h.o_mut()?
            .set("position", Node::Val(idf::vec3([at[0], at[1], 0.0, 0.0])));
        h.o_mut()?.set("scale", Node::Val(idf::vec3([s, s, s, 0.0])));
        hosts.push(h);
    }
    let root = scene_root_mut(&mut pkg)?;
    let parent = match &facts.instance.under {
        Some(u) => {
            idf::find_mut(root, u).ok_or_else(|| err(format!("{} has no {u}", facts.instance.scene)))?
        }
        None => root,
    };
    idf::children_mut(parent)?.extend(hosts);
    let root = scene_root_mut(&mut pkg)?;
    for kid in idf::children_mut(root)? {
        let n = idf::elem_name(kid).to_owned();
        idf::recompute_ids(kid, &n)?;
    }
    let entry = &mut idf::scenes_mut(&mut pkg)?[0];
    idf::relookup(entry)?;
    let after = idf::id_map(stock_edit::scene_root(&pkg)?);
    if let Some((k, _)) = stock_ids.iter().find(|(k, v)| after.get(*k) != Some(*v)) {
        return Err(err(format!(
            "{}: the stock element {k} changed its id",
            facts.instance.scene
        )));
    }
    Ok(pkg.to_bytes()?)
}

/// Bindings the control group adds beyond the plan's, with their tags.
pub type Known = Vec<(String, [u8; 4])>;

/// A decision of a screen record: an MVCD or SVCD on the path `on` names (no dial value, no stock
/// copy), its events showing and hiding the plan's switched elements under `base`.
fn path_decision(d: &Decision, plan: &Plan, base: &str) -> Result<Node> {
    let on = d.on.as_ref().ok_or_else(|| {
        err(format!(
            "decision {}: a screen record decides on a path (on), not a dial value (game)",
            d.id
        ))
    })?;
    let usage = match on.ty {
        None | Some(super::model::Ty::Bool) => Usage::Visible,
        _ => Usage::Text,
    };
    let r = template::resolve(on, usage, &format!("decision {}", d.id))?;
    let signals = |show: &BTreeSet<&String>, all: &[&String], ev: &str| -> Result<Vec<Node>> {
        let mut out = Vec::new();
        for id in all {
            let pin = if show.contains(id) { "show" } else { "hide" };
            out.push(node::sig(&format!("{base}.{}", plan.switched_node(id)?), pin, ev));
        }
        Ok(out)
    };
    let name = format!("d_{}", d.id);
    if d.kind == "mvcd" {
        let all: Vec<&String> = d
            .states
            .values()
            .flatten()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut events = Vec::new();
        for (ev, shown) in &d.states {
            let show: BTreeSet<&String> = shown.iter().collect();
            events.push(node::event(ev, signals(&show, &all, ev)?));
        }
        return Ok(node::mvcd_node(&name, &r.db, events));
    }
    let tag = template::parse_tag(d.ty.as_deref().unwrap_or("ui32"));
    let all: Vec<&String> = d.yes.iter().chain(&d.no).collect();
    let yes: BTreeSet<&String> = d.yes.iter().collect();
    let no: BTreeSet<&String> = d.no.iter().collect();
    let events = vec![
        node::event("yes", signals(&yes, &all, "yes")?),
        node::event("no", signals(&no, &all, "no")?),
    ];
    Ok(node::svcd_node(
        &name,
        tag,
        &d.equals.unwrap_or(0).to_string(),
        &r.db,
        events,
    ))
}

/// The `<prefix>_ctl` group of one record on one screen: the masks (visible <- NOT show, default
/// true), the host switch `host_n` (visible <- show, default false) holding the cap's own graphics
/// and the editor copy hidden, the roots of the `others` (every other screen record in the shared
/// host scene) and their editor copies hidden, the root shown (or following the screen's own switch)
/// and every bound node of the plan, then the screen's extra hide on the instance and the session
/// clock's formatter when a text reads it. Bound nodes keep their own layout paths: a node's chain continues its
/// parent's, so under `host_n` they address elements inside the instance. `hud_root` is the stock
/// HUD template, where the texture tags and the clock formatter come from.
pub fn control_group(
    facts: &ScreenFacts,
    show: &Bind,
    hide: &[String],
    plan: &Plan,
    pfx: &str,
    others: &[String],
    hud_root: &Node,
) -> Result<(Node, Known)> {
    let sw = template::resolve(show, Usage::Visible, "show")?;
    let host = facts.host_name(pfx);
    let base = format!("{}.{pfx}_ctl", facts.parent_ui);
    let plan = &template::with_clock_output(plan);
    let mut known = Vec::new();
    let mut kids = Vec::new();
    for (i, chain) in facts.masks.iter().enumerate() {
        let names: Vec<&str> = chain.iter().map(String::as_str).collect();
        kids.push(node::group_node(
            &format!("mask_{i}"),
            vec![("visible", node::bool_prop(true, Some(&sw.db), !sw.negate))],
            &names,
            vec![],
        ));
    }
    let pv = preview_root(pfx);
    let mut inner = vec![
        node::hidden("v_cap", &[screens::CAP_ANIM]),
        node::hidden("v_pv", &[&pv]),
    ];
    // every record's root sits in the one host scene, so each host instance holds the others' roots
    // too; `isHidden` does not hold in game, so they get static hides here
    for o in others.iter().filter(|o| o.as_str() != pfx) {
        inner.push(node::hidden(&format!("o_{o}"), &[o]));
        inner.push(node::hidden(&format!("o_{o}_pv"), &[&preview_root(o)]));
    }
    inner.push(match &facts.root_switch {
        Some(db) => {
            known.push((db.clone(), *b"bool"));
            node::visible_node("root_n", &plan.root, true, Some(db), false)
        }
        None => node::group_node(
            "root_n",
            vec![("visible", node::prop(node::boolv(true), None))],
            &[&plan.root],
            vec![],
        ),
    });
    let tex = if plan.textures.is_empty() {
        None
    } else {
        Some(match &facts.series_logo {
            Some(db) => TexFacts::for_screen(hud_root, db)?,
            None => TexFacts::hud(hud_root)?,
        })
    };
    if let Some(t) = &tex
        && let Some((db, _)) = &t.series
    {
        known.push((db.clone(), t.series_tag));
    }
    for id in hide {
        inner.push(node::hidden(&format!("h_{id}"), &[&format!("{pfx}_{id}")]));
    }
    inner.extend(template::bound_nodes(plan, tex.as_ref())?);
    let inner_base = format!("{}.{pfx}_ctl.host_n", facts.parent_ui);
    let triggers = super::motion::trigger_nodes(plan, &inner_base);
    let trigger_names: Vec<String> = triggers.iter().map(|n| n.name().to_owned()).collect();
    inner.extend(triggers);
    kids.push(node::group_node(
        "host_n",
        vec![("visible", node::bool_prop(false, Some(&sw.db), sw.negate))],
        &[&host],
        inner,
    ));
    if let Some(x) = &facts.extra_hide {
        known.push((x.db.clone(), *b"bool"));
        let mut chain = vec![host.as_str()];
        chain.extend(x.layout.iter().map(String::as_str));
        kids.push(node::group_node(
            "extra_hide",
            vec![("visible", node::bool_prop(x.default, Some(&x.db), x.negate))],
            &chain,
            vec![],
        ));
    }
    let mut trigger_sigs = Vec::new();
    if template::needs_clock(plan) {
        let (nodes, sigs) = template::clock_nodes(hud_root, plan, &base)?;
        kids.extend(nodes);
        trigger_sigs = sigs;
    }
    let mut decision_names = Vec::new();
    if facts.wire.is_some() {
        for d in &plan.decisions {
            let n = path_decision(d, plan, &base)?;
            decision_names.push(n.name().to_owned());
            kids.push(n);
        }
    }
    let mut names = BTreeSet::new();
    fn walk(n: &Node, names: &mut BTreeSet<String>) -> Result<()> {
        if !names.insert(n.name().to_owned()) {
            return Err(err(format!("the template node name {} is used twice", n.name())));
        }
        for c in n.slot(node::CHILDREN)? {
            if c.is_node() {
                walk(c, names)?;
            }
        }
        Ok(())
    }
    let mut grp = node::group_node(&format!("{pfx}_ctl"), vec![], &[], kids);
    if !trigger_names.is_empty() {
        node::decision_commands(&mut grp, &inner_base, &trigger_names)?;
    }
    for c in grp.slot(node::CHILDREN)? {
        walk(c, &mut names)?;
    }
    let mut untrigger_sigs = Vec::new();
    for n in &decision_names {
        let path = format!("{base}.{n}");
        trigger_sigs.push(node::sig(&path, "register", "trigger"));
        trigger_sigs.push(node::sig(&path, "query", "trigger"));
        untrigger_sigs.push(node::sig(&path, "unregister", "untrigger"));
    }
    template::add_command_signals(&mut grp, "trigger", trigger_sigs)?;
    template::add_command_signals(&mut grp, "untrigger", untrigger_sigs)?;
    for d in &plan.decisions {
        if let Some(on) = &d.on {
            let r = template::resolve(
                on,
                if matches!(on.ty, None | Some(super::model::Ty::Bool)) {
                    Usage::Visible
                } else {
                    Usage::Text
                },
                "decision",
            )?;
            known.push((r.db, r.tag));
        }
    }
    Ok((grp, known))
}

/// Adds the control group under the screen's parent node, checks every binding's tag against the
/// stock template, and wires the parent's commands to it when the screen has them.
#[allow(clippy::too_many_arguments)]
pub fn attach(
    template: &mut Node,
    stock: &StockTags,
    facts: &ScreenFacts,
    show: &Bind,
    hide: &[String],
    plan: &Plan,
    pfx: &str,
    others: &[String],
    hud_root: &Node,
) -> Result<usize> {
    let parent = facts.parent_ui.as_str();
    let at = node::find_path(template, parent)?;
    let target = node::node_at_mut(template, &at)?;
    let id = target.props()?.at("id")?.v()?.as_u32();
    if id != Some(swofty_formats::hash::djb2(parent)) {
        return Err(err(format!(
            "{parent}: the node's id is not the hash of its path"
        )));
    }
    let (mut grp, known) = control_group(facts, show, hide, plan, pfx, others, hud_root)?;
    if target
        .slot(node::CHILDREN)?
        .iter()
        .any(|c| c.is_node() && c.name() == grp.name())
    {
        return Err(err(format!("{parent} already has a {}", grp.name())));
    }
    let mut table = template::table_of(plan);
    table.extend(known);
    let checked = template::check_types(stock, std::slice::from_ref(&grp), &table)?;
    node::fix_ids(&mut grp, Some(parent));
    let base = format!("{parent}.{}", grp.name());
    target.slot_mut(node::CHILDREN)?.push(grp);
    if let Some((on, off)) = &facts.wire {
        node::wire_command(target, on, &base, "trigger")?;
        node::wire_command(target, off, &base, "untrigger")?;
    }
    Ok(checked)
}

fn template_of_path(path: &str) -> &'static str {
    match path.split('.').next().unwrap_or_default() {
        screens::SPECT_ROOT => "spectator",
        screens::REPLAY_ROOT => "replay",
        _ => "race",
    }
}

/// Composes every `screen` record of `inputs` into the race template, the spectator template and
/// the archives of [`Out`].
#[allow(clippy::too_many_arguments)]
pub fn compose_screens(
    stock: &dyn Stock,
    inputs: &[Input],
    pfx_of: &[String],
    template: &mut Pkg,
    race_stock: &Pkg,
    spectator: &mut Pkg,
    spec_stock: &Pkg,
    fill_material: &[u8],
    mods: &[u8],
    bar: &[u8],
    om_xm: &[u8],
) -> Result<Out> {
    let mut out = Out::default();
    let mut recs: Vec<Rec> = Vec::new();
    for (i, input) in inputs.iter().enumerate() {
        let Some(targets) = screens::targets(&input.record)? else {
            continue;
        };
        let what = format!("{}/{}", input.addon, input.key);
        let rec = &input.record;
        if !rec.variants.is_empty() || rec.stock.is_some() || !rec.stock_parts.is_empty() {
            return Err(err(format!(
                "{what}: a screen record takes no variants, stock or stock_parts yet"
            )));
        }
        if let Some(d) = rec.decisions.iter().find(|d| d.on.is_none()) {
            return Err(err(format!(
                "{what}: decision {}: a screen record decides on a path (on), not a dial value (game)",
                d.id
            )));
        }
        let sprites = load_sprites(stock, input, &pfx_of[i], fill_material, &mut out.scene)?;
        recs.push(Rec {
            input,
            pfx: pfx_of[i].clone(),
            targets,
            sprites,
            stock_nodes: stock_edit::stock_nodes(rec).map_err(|e| err(format!("{what}: {e}")))?,
            restyles: stock_edit::restyles(rec).map_err(|e| err(format!("{what}: {e}")))?,
        });
    }
    if recs.is_empty() {
        return Ok(out);
    }
    let read_from = |erp: &str, name: &str| first(stock.read(erp, name)?, name);
    let race_read = |name: &str| read_from(screens::FLOW_RACE, name);
    let race_tags = StockTags::race(&race_stock.root);

    let cap_bytes = race_read(screens::CAP)?;
    let mut cap = Builder::new(&CAP_FACTS, &cap_bytes, mods, bar, Some(om_xm))?;
    cap.with_donors(&race_read(screens::STACKED)?, &race_read(screens::LAPC_SCENE)?)?;
    if recs.iter().any(|r| units::used(&r.input.record.elements)) {
        cap.units = units::load(stock, screens::FLOW_RACE)?;
    }
    let mut plans: Vec<Plan> = Vec::new();
    let mut previews: Vec<Plan> = Vec::new();
    for r in &recs {
        let plan = cap.add_scene_record(&r.input.record, &r.pfx, &r.sprites)?;
        let root = scene_root_mut(&mut cap.pkg)?;
        let mine =
            idf::find_mut(root, &r.pfx).ok_or_else(|| err(format!("{}: the root went missing", r.pfx)))?;
        idf::set_param_like(mine, "isHidden", Val::bool(true))?;
        plans.push(plan);
        let state = r
            .input
            .record
            .editor
            .as_ref()
            .map(|e| e.preview.clone())
            .unwrap_or_default();
        previews.push(cap.add_scene_record_in(&r.input.record, &r.pfx, &r.sprites, Mode::Preview(&state))?);
    }
    let host = cap.finish(None)?;
    if let Ok(host_pkg) = Pkg::parse(&host)
        && let Ok(host_root) = stock_edit::scene_root(&host_pkg)
    {
        for r in &recs {
            let cost = |name: &str| idf::find(host_root, name).map(walk::cost).unwrap_or_default();
            out.lines.extend(walk::lines(
                &r.input.addon,
                &r.pfx,
                cost(&r.pfx),
                cost(&preview_root(&r.pfx)),
                HOST_INSTANCES,
            ));
        }
    }
    out.scene.push((screens::CAP.to_owned(), "UILayout", vec![host]));
    let planned: Vec<&Plan> = plans.iter().collect();
    if planned.iter().any(|p| !p.clips.is_empty()) {
        let clip_template = race_read(super::dials::CLIP_TEMPLATE)?;
        super::clip_resources(stock, &CAP_FACTS, &clip_template, &planned, &mut out.scene)?;
    }

    let mut by_screen: BTreeMap<String, (ScreenFacts, Vec<usize>)> = BTreeMap::new();
    for (ri, r) in recs.iter().enumerate() {
        for t in &r.targets {
            let e = by_screen
                .entry(t.facts.key.clone())
                .or_insert_with(|| (t.facts.clone(), Vec::new()));
            if e.0 != t.facts {
                return Err(err(format!(
                    "two records spell the screen {} differently",
                    t.facts.key
                )));
            }
            e.1.push(ri);
        }
    }
    let all_pfx: Vec<String> = recs.iter().map(|r| r.pfx.clone()).collect();
    let mut replay: Option<(Pkg, Pkg)> = None;
    let mut standings = false;
    for (key, (facts, members)) in &by_screen {
        let read = |name: &str| read_from(&facts.stock_erp, name);
        let prefixes: Vec<String> = members.iter().map(|i| recs[*i].pfx.clone()).collect();
        let instance = add_instances(&read(&facts.instance.scene)?, facts, &prefixes, &read)?;
        let res: Res = (facts.instance.scene.clone(), "UILayout", vec![instance]);
        match key.as_str() {
            "standings" => {
                standings = true;
                out.scene.push(res);
            }
            "replay" => {
                out.replay_template.push(res.clone());
                out.replay_scene.push(res);
            }
            "spectator" => out.spectator_scene.push(res),
            _ => out.scene.push(res),
        }
        for &ri in members {
            let r = &recs[ri];
            let target = r.targets.iter().find(|t| t.facts.key == *key).expect("a member");
            let show = &target.show;
            let hide = &target.hide;
            let others: Vec<String> = all_pfx.iter().filter(|p| **p != r.pfx).cloned().collect();
            let checked = match template_of_path(&facts.parent_ui) {
                "spectator" => {
                    let tags = StockTags::spectator(
                        &spec_stock.root,
                        &facts.parent_ui,
                        &race_tags,
                        screens::STANDINGS_PARENT,
                        &race_stock.root,
                    )?;
                    out.spectator_used = true;
                    attach(
                        &mut spectator.root,
                        &tags,
                        facts,
                        show,
                        hide,
                        &plans[ri],
                        &r.pfx,
                        &others,
                        &race_stock.root,
                    )?
                }
                "replay" => {
                    if replay.is_none() {
                        let bytes = read_from(&facts.template_erp, &facts.template)?;
                        replay = Some((Pkg::parse(&bytes)?, Pkg::parse(&bytes)?));
                    }
                    let (live, stock_pkg) = replay.as_mut().expect("set above");
                    let tags = StockTags::spectator(
                        &stock_pkg.root,
                        &facts.parent_ui,
                        &race_tags,
                        screens::STANDINGS_PARENT,
                        &race_stock.root,
                    )?;
                    attach(
                        &mut live.root,
                        &tags,
                        facts,
                        show,
                        hide,
                        &plans[ri],
                        &r.pfx,
                        &others,
                        &race_stock.root,
                    )?
                }
                _ => attach(
                    &mut template.root,
                    &race_tags,
                    facts,
                    show,
                    hide,
                    &plans[ri],
                    &r.pfx,
                    &others,
                    &race_stock.root,
                )?,
            };
            let plan = &plans[ri];
            if !others.is_empty() {
                out.lines.push(format!(
                    "{}: screen {key}: host_n hides the roots of {} other screen records ({} static template nodes)",
                    r.pfx,
                    others.len(),
                    2 * others.len()
                ));
            }
            out.lines.push(format!(
                "{}: screen {key} in {}: {} bound nodes, {} masks, {checked} bindings checked",
                r.pfx,
                facts
                    .instance
                    .scene
                    .rsplit('/')
                    .next()
                    .unwrap_or(&facts.instance.scene),
                plan.vis.len() + plan.texts.len() + plan.colours.len() + plan.shows.len(),
                facts.masks.len()
            ));
        }
    }
    if let Some((live, _)) = &replay {
        out.replay_template
            .push((screens::REPLAY_UI.to_owned(), "UIDF", vec![live.to_bytes()?]));
    }
    if standings {
        let uipre = race_read(screens::STACKED_UIPRE)?;
        let hide = vec![screens::STACKED_MASK.to_owned()];
        let mut instances = Vec::new();
        for (ri, r) in recs.iter().enumerate() {
            if !r.targets.iter().any(|t| t.facts.key == "standings") {
                continue;
            }
            let pv = &previews[ri];
            instances.push(editor::Instanced {
                host: screens::standings().host_name(&r.pfx),
                hide: [screens::CAP_ANIM.to_owned(), r.pfx.clone()]
                    .into_iter()
                    .chain(
                        all_pfx
                            .iter()
                            .filter(|p| **p != r.pfx)
                            .flat_map(|p| [p.clone(), preview_root(p)]),
                    )
                    .collect(),
                show: vec![pv.root.clone()],
                texts: pv.pv_texts.clone(),
                loc: Vec::new(),
            });
            out.lines.push(format!(
                "{}: the HUD editor shows {} with {} texts filled",
                r.pfx,
                pv.root,
                pv.pv_texts.len()
            ));
        }
        let new = editor::uipre_in(&uipre, screens::STACKED, &hide, &instances)?;
        out.scene
            .push((screens::STACKED_UIPRE.to_owned(), "UIPR", vec![new]));
    }

    let mut scene_cache: BTreeMap<String, Node> = BTreeMap::new();
    let mut wrapped: BTreeSet<String> = BTreeSet::new();
    for r in &recs {
        let mut nodes: Vec<StockNodes> = r.stock_nodes.clone();
        if r.targets.iter().any(|t| t.facts.key == "standings") {
            nodes.push(StockNodes {
                template: screens::STANDINGS_CAP_NODE.to_owned(),
                scene: Some(screens::CAP.to_owned()),
                hide: vec![r.pfx.clone(), preview_root(&r.pfx)],
                wrap: Vec::new(),
                while_: None,
            });
        }
        for (i, s) in nodes.iter().enumerate() {
            let (live, stock_pkg): (&mut Pkg, &Pkg) = match template_of_path(&s.template) {
                "spectator" => (&mut *spectator, spec_stock),
                "replay" => {
                    return Err(err(format!(
                        "{}: stock_nodes on the replay template are not supported yet",
                        r.pfx
                    )));
                }
                _ => (&mut *template, race_stock),
            };
            let scene = match &s.scene {
                Some(sc) => sc.clone(),
                None => stock_edit::scene_of(&stock_pkg.root, &s.template)?.ok_or_else(|| {
                    err(format!(
                        "stock_nodes {}: the template node instances no scene; name the scene",
                        s.template
                    ))
                })?,
            };
            if scene == screens::CAP {
                // the host scene's stock parts, not ours: check against the stock cap
                let _ = &cap_bytes;
            }
            let root = match scene_cache.get(&scene) {
                Some(n) => n.clone(),
                None => {
                    let bytes = race_read(&scene).or_else(|_| read_from(screens::FLOW_REPLAY, &scene))?;
                    let p = Pkg::parse(&bytes)?;
                    let n = stock_edit::scene_root(&p)?.clone();
                    scene_cache.insert(scene.clone(), n.clone());
                    n
                }
            };
            let own =
                scene == screens::CAP && s.hide.iter().all(|h| *h == r.pfx || *h == preview_root(&r.pfx));
            let s = if own {
                // our own roots are not stock elements: hide them statically without the stock checks
                let name = format!("{}_sn{i}", r.pfx);
                let at = node::find_path(&live.root, &s.template)?;
                let target = node::node_at_mut(&mut live.root, &at)?;
                let mut grp = stock_edit::stock_nodes_group(s, &name, &[])?;
                node::fix_ids(&mut grp, Some(&s.template));
                target.slot_mut(node::CHILDREN)?.push(grp);
                out.lines.push(format!(
                    "{}: {} hides the host root and its editor copy in the stock cap instance",
                    r.pfx, s.template
                ));
                continue;
            } else {
                s
            };
            let mut wraps = Vec::new();
            if !s.wrap.is_empty() {
                if wrapped.contains(&scene) {
                    return Err(err(format!(
                        "{}: {} is wrapped by another record already; one record wraps a scene",
                        r.pfx,
                        scene.rsplit('/').next().unwrap_or(&scene)
                    )));
                }
                let bytes = race_read(&scene).or_else(|_| read_from(screens::FLOW_REPLAY, &scene))?;
                let (new, names) = stock_edit::wrap_scene(&bytes, s, &r.pfx)?;
                wraps = names;
                wrapped.insert(scene.clone());
                match template_of_path(&s.template) {
                    "spectator" => out.spectator_scene.push((scene.clone(), "UILayout", vec![new])),
                    _ => out.scene.push((scene.clone(), "UILayout", vec![new])),
                }
            }
            let n = stock_edit::attach_stock_nodes(
                &mut live.root,
                &stock_pkg.root,
                &root,
                s,
                &format!("{}_sn{i}", r.pfx),
                &wraps,
            )?;
            out.lines.push(format!(
                "{}: {} hides {n} elements of {}{}{}",
                r.pfx,
                s.template,
                scene.rsplit('/').next().unwrap_or(&scene),
                if wraps.is_empty() {
                    String::new()
                } else {
                    format!(" ({} through a new wrapper null)", wraps.len())
                },
                if s.while_.is_some() {
                    " while the leaf is true"
                } else {
                    ""
                }
            ));
        }
        let mut emb_slots: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for rs in &r.restyles {
            let erp = rs
                .stock_erp
                .clone()
                .unwrap_or_else(|| screens::FLOW_RACE.to_owned());
            let bytes = read_from(&erp, &rs.scene)?;
            let emb_of = |sprite: &str| stock_edit::emb_name(&r.pfx, sprite);
            let slot_ok = |el: &str, slot: &str| -> Result<()> {
                let mesh = stock_edit::mesh_resource(&rs.scene, el);
                let frags = stock.read(&erp, &mesh)?;
                let mut want = slot.as_bytes().to_vec();
                want.push(0);
                if frags.iter().any(|f| f.windows(want.len()).any(|w| w == want)) {
                    Ok(())
                } else {
                    Err(err(format!("{}: the mesh of {el} has no slot {slot}", rs.scene)))
                }
            };
            let (new, info) = stock_edit::apply(&bytes, rs, &emb_of, &slot_ok)?;
            for (_, slot, _) in &info.fills {
                let sprite = rs
                    .fill
                    .values()
                    .find(|f| f.slot == *slot)
                    .map(|f| f.sprite.clone())
                    .unwrap_or_default();
                emb_slots.entry(sprite).or_default().insert(slot.clone());
            }
            out.scene.push((rs.scene.clone(), "UILayout", vec![new]));
            let hides = stock_edit::attach_restyle_hides(&mut template.root, &race_stock.root, rs, &r.pfx)?;
            out.lines.push(format!(
                "{}: restyled {}: {} fills, {} twins, {} hidden ({hides} static hide nodes)",
                r.pfx,
                rs.scene.rsplit('/').next().unwrap_or(&rs.scene),
                info.fills.len(),
                info.twins.len(),
                info.hidden.len()
            ));
        }
        for (sprite, slots) in emb_slots {
            let names = textures::names(&format!("{}_", r.pfx), &sprite);
            let pairs: Vec<(String, String)> =
                slots.into_iter().map(|s| (s, names.material.clone())).collect();
            out.scene.push((
                stock_edit::emb_name(&r.pfx, &sprite),
                "GfxMatBindRes",
                vec![emb::write(&pairs)],
            ));
        }
    }
    Ok(out)
}

/// The archives the screen records' scene archive is made for: the host, the standings instance, the
/// `.uipre`, the sprites and the restyled scenes go here. Six stock archives carry the cap and the
/// standings board; [`super::cover_carriers`] adds `screens/hud.erp`, `hud_player_1.erp` and
/// `hud_player_2.erp`, and any other carrier, so no stock copy stays beside a composed one.
pub const SCREEN_SCENE_ERPS: [&str; 3] = [
    "2025_asset_groups/ui_package/flow_race.erp",
    "2025_asset_groups/ui_package/flow_options.erp",
    "2025_asset_groups/ui_package/persistent.erp",
];

/// The archives of [`Out`] that are not the spectator template's.
pub fn archives(stock: &dyn Stock, out: &Out) -> Result<Vec<super::Built>> {
    let mut built = Vec::new();
    if !out.scene.is_empty() {
        built.push(super::Built {
            file: "hud_screen_scene.erp".into(),
            erps: SCREEN_SCENE_ERPS.iter().map(|s| (*s).to_owned()).collect(),
            bytes: super::archive(stock, out.scene.clone())?,
        });
    }
    if !out.replay_template.is_empty() {
        let f = screens::replay();
        built.push(super::Built {
            file: f.template_file.clone(),
            erps: f.erps.template.clone(),
            bytes: super::archive(stock, out.replay_template.clone())?,
        });
        built.push(super::Built {
            file: f.scene_file.clone(),
            erps: f.erps.scene.clone(),
            bytes: super::archive(stock, out.replay_scene.clone())?,
        });
    }
    Ok(built)
}

/// The stock resources screen records read and write whole, for the build's layering.
pub fn written() -> Vec<&'static str> {
    vec![
        screens::CAP,
        screens::STACKED,
        screens::STACKED_UIPRE,
        screens::REPLAY_UI,
        screens::REPLAY_SCENE,
        screens::SPECT_TABLE,
        dials::TEMPLATE,
        dials::SPECTATOR,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_template() -> Node {
        let mut cmds = node::group_node("track_position", vec![], &[], vec![]);
        cmds.slot_mut(node::COMMANDS)
            .unwrap()
            .extend([node::command("enable", vec![]), node::command("disable", vec![])]);
        let mut t = node::group_node(
            "hud_track_position",
            vec![],
            &[],
            vec![node::group_node("osd_visibility_option", vec![], &[], vec![cmds])],
        );
        node::fix_ids(&mut t, None);
        node::arr(vec![t])
    }

    fn plan(pfx: &str) -> Plan {
        let mut p = Plan::default();
        p.pfx = pfx.to_owned();
        p.root = pfx.to_owned();
        p.vis.push(super::super::scene::VisItem {
            name: format!("v_{pfx}_r00"),
            element: format!("{pfx}_r00"),
            db: Some("#view_model_data.hud_tt_r00_on".into()),
            negate: false,
            default: false,
        });
        p
    }

    fn layout_of(n: &Node) -> Vec<Vec<String>> {
        n.slot(node::LAYOUT)
            .unwrap()
            .iter()
            .map(|chain| {
                chain
                    .as_arr()
                    .unwrap()
                    .items
                    .iter()
                    .map(|o| o.as_obj().unwrap().str_at("debugName").unwrap().to_owned())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn the_control_group_masks_the_board_and_switches_the_host() {
        let show = Bind::leaf("view_model_data.hud_tt_show");
        let (grp, known) = control_group(
            &screens::standings(),
            &show,
            &[],
            &plan("tt"),
            "tt",
            &[],
            &fake_template(),
        )
        .unwrap();
        assert_eq!(grp.name(), "tt_ctl");
        let kids = grp.slot(node::CHILDREN).unwrap();
        let names: Vec<&str> = kids.iter().map(|k| k.name()).collect();
        assert_eq!(names, ["mask_0", "host_n"]);
        assert_eq!(layout_of(&kids[0]), vec![vec!["mask".to_owned()]]);
        assert_eq!(layout_of(&kids[1]), vec![vec!["tt_host".to_owned()]]);
        let inner: Vec<&str> = kids[1]
            .slot(node::CHILDREN)
            .unwrap()
            .iter()
            .map(|k| k.name())
            .collect();
        assert_eq!(inner, ["v_cap", "v_pv", "root_n", "v_tt_r00"]);
        assert!(known.is_empty());
        let mut found = Vec::new();
        node::bindings(&grp, &[], &mut found);
        let mask = found.iter().find(|b| b.node == "mask_0").unwrap();
        let host = found.iter().find(|b| b.node == "host_n").unwrap();
        assert_eq!(
            (mask.db.as_str(), mask.tag),
            ("#view_model_data.hud_tt_show", Some(*b"bool"))
        );
        assert_eq!(host.db, mask.db);
        let neg = |n: &Node| {
            n.props()
                .unwrap()
                .get("visible")
                .unwrap()
                .as_obj()
                .unwrap()
                .get("dbentrynegate")
                .and_then(Node::as_val)
                .and_then(stock_edit::as_bool)
        };
        assert_eq!(neg(&kids[0]), Some(true));
        assert_eq!(neg(&kids[1]), None);

        let (grp, known) = control_group(
            &screens::replay(),
            &show,
            &[],
            &plan("tt"),
            "tt",
            &[],
            &fake_template(),
        )
        .unwrap();
        let kids = grp.slot(node::CHILDREN).unwrap();
        let names: Vec<&str> = kids.iter().map(|k| k.name()).collect();
        assert_eq!(names, ["mask_0", "mask_1", "host_n", "extra_hide"]);
        assert_eq!(
            layout_of(&kids[0]),
            vec![vec!["driver_table".to_owned(), "Scene Root".to_owned()]]
        );
        assert_eq!(
            layout_of(&kids[3]),
            vec![vec!["tt_host".to_owned(), "Scene Root".to_owned()]]
        );
        assert_eq!(known.len(), 2);
        let (grp, _) = control_group(
            &screens::spectator(),
            &show,
            &[],
            &plan("tt"),
            "tt",
            &[],
            &fake_template(),
        )
        .unwrap();
        let kids = grp.slot(node::CHILDREN).unwrap();
        assert_eq!(
            layout_of(&kids[0]),
            vec![
                ["spectator_mode_2", "driver_position_table", "transition"]
                    .map(String::from)
                    .to_vec()
            ]
        );
    }

    #[test]
    fn the_host_hides_the_other_records_roots() {
        let show = Bind::leaf("view_model_data.hud_tt_show");
        let others = ["tt".to_owned(), "uu".to_owned(), "vv".to_owned()];
        let (grp, _) = control_group(
            &screens::standings(),
            &show,
            &[],
            &plan("tt"),
            "tt",
            &others,
            &fake_template(),
        )
        .unwrap();
        let host = &grp.slot(node::CHILDREN).unwrap()[1];
        let inner: Vec<&Node> = host.slot(node::CHILDREN).unwrap().iter().collect();
        let names: Vec<&str> = inner.iter().map(|k| k.name()).collect();
        assert_eq!(
            names,
            [
                "v_cap", "v_pv", "o_uu", "o_uu_pv", "o_vv", "o_vv_pv", "root_n", "v_tt_r00"
            ]
        );
        for (n, el) in [(2, "uu"), (3, "uu_pv"), (4, "vv"), (5, "vv_pv")] {
            assert_eq!(layout_of(inner[n]), vec![vec![el.to_owned()]]);
            assert_eq!(
                inner[n]
                    .props()
                    .unwrap()
                    .get("visible")
                    .unwrap()
                    .as_obj()
                    .unwrap()
                    .get("value")
                    .and_then(Node::as_val)
                    .and_then(stock_edit::as_bool),
                Some(false)
            );
        }
    }

    #[test]
    fn attach_wires_the_parents_commands_and_checks_the_ids() {
        let show = Bind::leaf("view_model_data.hud_tt_show");
        let stock = fake_template();
        let mut t = stock.clone();
        let tags = StockTags::race(&stock);
        attach(
            &mut t,
            &tags,
            &screens::standings(),
            &show,
            &[],
            &plan("tt"),
            "tt",
            &[],
            &fake_template(),
        )
        .unwrap();
        let at = node::find_path(
            &t,
            "hud_track_position.osd_visibility_option.track_position.tt_ctl.host_n",
        )
        .unwrap();
        assert!(!at.is_empty());
        let at = node::find_path(&t, screens::STANDINGS_PARENT).unwrap();
        let parent = node::node_at(&t, &at).unwrap();
        for cmd in ["enable", "disable"] {
            let c = parent
                .slot(node::COMMANDS)
                .unwrap()
                .iter()
                .find(|c| c.as_obj().unwrap().str_at("commandName").unwrap() == cmd)
                .unwrap();
            let sigs = c.as_obj().unwrap().at("signals").unwrap().a().unwrap();
            assert_eq!(sigs.len(), 1);
            assert_eq!(
                sigs[0].as_obj().unwrap().str_at("to_object_name").unwrap(),
                "hud_track_position.osd_visibility_option.track_position.tt_ctl"
            );
        }
        let e = attach(
            &mut t,
            &tags,
            &screens::standings(),
            &show,
            &[],
            &plan("tt"),
            "tt",
            &[],
            &fake_template(),
        )
        .unwrap_err();
        assert!(e.to_string().contains("already has"));
        let ours: BTreeSet<String> = ["tt_ctl".to_owned()].into();
        let sp = Pkg {
            name: [0; 4],
            root: stock.clone(),
            vca: Vec::new(),
            layout: None,
        };
        let bp = Pkg {
            name: [0; 4],
            root: t.clone(),
            vca: Vec::new(),
            layout: None,
        };
        assert!(super::super::check::stock_kept(&sp, &bp, &ours).unwrap());
    }

    #[test]
    fn the_cap_facts_name_the_host() {
        assert_eq!(CAP_FACTS.scene, screens::CAP);
        assert_eq!(CAP_FACTS.quad, "pill_base");
        assert!(written().contains(&screens::STACKED));
    }
}
