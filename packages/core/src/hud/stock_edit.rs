//! Changes to the game's own parts: `stock_nodes` (hide elements of a stock scene from a template
//! node, while a leaf is true or for good) and `restyle` (edit a stock scene in place: refill its
//! quads from a sprite, hide its decorations, scale and restyle its texts; no element added, moved or
//! renamed, so every stock id stays).
//!
//! The rules follow Project Halo's `tower_native.py` (`lap_counter_nodes`, `check_lap_counter`) and
//! `banner_native.py` (`build_banner`, `toast_hides`); Uses work from Project Halo by nyannoying
//! (https://github.com/nyannoying1337/f1-25-project-halo). The game facts they encode (`isHidden`
//! alone does not hide in game; a second `visible` on one element fights the game's own; an
//! animated fill at an offset layer draws nothing) are measurements of the game.

use std::collections::{BTreeMap, BTreeSet};

use swofty_formats::pkg::{Node, Pkg, Val};

#[cfg(test)]
use super::model::Bind;
pub use super::model::{Fill, Param, Restyle, Scale, StockNodes, TextStyle};
use super::model::{HudElement, RestyleField};
use super::scene::guid;
use super::template::{self, Usage};
use crate::ui::idf;
use crate::ui::node::{self, NodeExt, ObjExt};
use crate::{Error, Result};

fn err(msg: impl std::fmt::Display) -> Error {
    Error::Other(format!("hud: {msg}"))
}

/// The record's `stock_nodes`, checked.
pub fn stock_nodes(rec: &HudElement) -> Result<Vec<StockNodes>> {
    let list = rec.stock_nodes.clone();
    for s in &list {
        if s.template.split('.').any(str::is_empty) {
            return Err(err(format!(
                "stock_nodes: template {} is a dotted template path",
                s.template
            )));
        }
        if s.hide.is_empty() && s.wrap.is_empty() {
            return Err(err(format!("stock_nodes: {} hides nothing", s.template)));
        }
        if !s.wrap.is_empty() && s.scene.is_none() {
            return Err(err(format!(
                "stock_nodes {}: wrap rewrites the scene, so name it with scene",
                s.template
            )));
        }
        if let Some(w) = s.wrap.iter().find(|w| s.hide.contains(w)) {
            return Err(err(format!(
                "stock_nodes {}: {w} is both hidden and wrapped",
                s.template
            )));
        }
        if let Some(b) = &s.while_ {
            b.check()?;
        }
    }
    Ok(list)
}

/// The name of the null wrapped around a stock element.
pub fn wrap_name(pfx: &str, element: &str) -> String {
    format!("{pfx}_w_{element}")
}

fn find_parent_index(e: &Node, name: &str) -> Option<(Vec<usize>, usize)> {
    let kids = e.as_obj()?.get("children")?.as_arr()?;
    for (i, c) in kids.items.iter().enumerate() {
        if c.as_obj().is_none() {
            continue;
        }
        if idf::elem_name(c) == name {
            return Some((Vec::new(), i));
        }
        if let Some((mut path, j)) = find_parent_index(c, name) {
            path.insert(0, i);
            return Some((path, j));
        }
    }
    None
}

fn child_at_mut<'a>(e: &'a mut Node, path: &[usize]) -> Result<&'a mut Node> {
    let mut cur = e;
    for i in path {
        cur = cur
            .o_mut()?
            .at_mut("children")?
            .a_mut()?
            .get_mut(*i)
            .ok_or_else(|| err("the wrap path left the scene"))?;
    }
    Ok(cur)
}

/// The scene with a new null `<pfx>_w_<element>` around each of `s.wrap`, in the element's place
/// in its parent's children, so every stock name and the drawing order stay; the ids of the moved
/// subtrees follow their new paths and the lookup grows by the wrappers. Returns the bytes and the
/// wrapper names.
pub fn wrap_scene(bytes: &[u8], s: &StockNodes, pfx: &str) -> Result<(Vec<u8>, Vec<String>)> {
    let mut pkg = Pkg::parse(bytes)?;
    let root = scene_root(&pkg)?;
    let all = idf::elements(root);
    let tmpl = all
        .iter()
        .find(|(_, e)| crate::ui::catalog::element_type(e) == "eNull")
        .map(|(_, e)| idf::detached(e))
        .ok_or_else(|| {
            err(format!(
                "stock_nodes {}: the scene has no eNull to clone a wrapper from",
                s.template
            ))
        })?;
    let scene = s.scene.clone().unwrap_or_default();
    let mut names = Vec::new();
    for el in &s.wrap {
        let hits: Vec<&Node> = all
            .iter()
            .filter(|(_, e)| idf::elem_name(e) == el)
            .map(|(_, e)| *e)
            .collect();
        let [e] = hits[..] else {
            return Err(err(format!(
                "stock_nodes {}: {} elements named {el} in the scene, one expected",
                s.template,
                hits.len()
            )));
        };
        if is_hidden(e) {
            return Err(err(format!(
                "stock_nodes {}: {el} is hidden by default already",
                s.template
            )));
        }
        let name = wrap_name(pfx, el);
        if idf::find(root, &name).is_some() {
            return Err(err(format!(
                "stock_nodes {}: the scene already has {name}",
                s.template
            )));
        }
        names.push(name);
    }
    let stock_ids = idf::id_map(root);
    let wrapped: BTreeSet<String> = s.wrap.iter().cloned().collect();
    let root = idf::scenes_mut(&mut pkg)?
        .first_mut()
        .ok_or_else(|| err("the file has no scene"))?
        .o_mut()?
        .at_mut("scene")?;
    for (el, name) in s.wrap.iter().zip(&names) {
        let (path, i) = find_parent_index(root, el).ok_or_else(|| err(format!("{el} went missing")))?;
        let parent = child_at_mut(root, &path)?;
        let kids = parent.o_mut()?.at_mut("children")?.a_mut()?;
        let inner = std::mem::replace(&mut kids[i], Node::Val(Val::bool(false)));
        let mut w = idf::detached(&tmpl);
        idf::rename(&mut w, name)?;
        idf::set_param_like(&mut w, "guid", Val::str(guid(&scene, name)))?;
        idf::set_transform(&mut w, [0.0; 3], super::geom::QUAT_IDENTITY, [1.0; 3])?;
        idf::clear_list(&mut w, "children")?;
        idf::clear_list(&mut w, "anims")?;
        if idf::param(&w, "isHidden").is_some() {
            idf::set_param_like(&mut w, "isHidden", Val::bool(false))?;
        }
        idf::children_mut(&mut w)?.push(inner);
        kids[i] = w;
    }
    for kid in idf::children_mut(root)? {
        let n = idf::elem_name(kid).to_owned();
        idf::recompute_ids(kid, &n)?;
    }
    let entry = &mut idf::scenes_mut(&mut pkg)?[0];
    idf::relookup(entry)?;
    let after = idf::id_map(scene_root(&pkg)?);
    for (k, v) in &stock_ids {
        let last = k.rsplit('.').next().unwrap_or(k);
        let moved = k.split('.').any(|part| wrapped.contains(part));
        if !moved && after.get(k) != Some(v) {
            return Err(err(format!(
                "stock_nodes {}: the stock element {last} changed its id",
                s.template
            )));
        }
    }
    Ok((pkg.to_bytes()?, names))
}

/// The scene a template node instances, by the component `debugName` of the node or its nearest
/// ancestor that has one.
pub fn scene_of(template_root: &Node, path: &str) -> Result<Option<String>> {
    node::find_path(template_root, path)?;
    let parts: Vec<&str> = path.split('.').collect();
    for k in (1..=parts.len()).rev() {
        let at = node::find_path(template_root, &parts[..k].join("."))?;
        let n = node::node_at(template_root, &at)?;
        let found = n.slot(node::COMPONENT)?.iter().find_map(|c| {
            c.as_obj()?
                .get("debugName")?
                .as_val()?
                .as_str()
                .map(|s| format!("{s}.idf"))
        });
        if found.is_some() {
            return Ok(found);
        }
    }
    Ok(None)
}

/// The root element of a scene file.
pub fn scene_root(p: &Pkg) -> Result<&Node> {
    idf::scenes(p)?
        .first()
        .ok_or_else(|| err("the file has no scene"))?
        .as_obj()
        .and_then(|o| o.get("scene"))
        .ok_or_else(|| err("the scene entry has no scene"))
}

pub fn as_bool(v: &Val) -> Option<bool> {
    match &v.value {
        swofty_formats::pkg::Value::Raw(b) if v.ty == *b"bool" => b.first().map(|x| *x != 0),
        _ => None,
    }
}

fn is_hidden(e: &Node) -> bool {
    idf::param(e, "isHidden").is_some_and(|v| as_bool(v) == Some(true))
}

/// Nodes under `path` that set `visible` on one of `elements` (the last link of a layout chain).
pub fn visible_fights(
    template_root: &Node,
    path: &str,
    elements: &[String],
) -> Result<Vec<(String, String)>> {
    let at = node::find_path(template_root, path)?;
    let top = node::node_at(template_root, &at)?;
    let mut paths = Vec::new();
    node::node_paths_of(top, "", &mut paths);
    let mut out = Vec::new();
    for (p, n) in paths {
        let sets = n.props().ok().is_some_and(|pr| pr.get("visible").is_some());
        if !sets {
            continue;
        }
        let Ok(lay) = n.slot(node::LAYOUT) else { continue };
        for chain in lay {
            let last = chain
                .as_arr()
                .and_then(|a| a.items.last())
                .and_then(Node::as_obj)
                .and_then(|o| o.get("debugName"))
                .and_then(Node::as_val)
                .and_then(Val::as_str)
                .unwrap_or_default();
            if elements.iter().any(|e| e == last) {
                out.push((p.clone(), last.to_owned()));
            }
        }
    }
    Ok(out)
}

/// Fails unless every element is in the scene once, not hidden by default, and no stock node under
/// the template path shows or hides it.
pub fn check_stock_nodes(scene_root: &Node, template_root: &Node, s: &StockNodes) -> Result<()> {
    let all = idf::elements(scene_root);
    for el in &s.hide {
        let hits: Vec<&Node> = all
            .iter()
            .filter(|(_, e)| idf::elem_name(e) == el)
            .map(|(_, e)| *e)
            .collect();
        let [e] = hits[..] else {
            return Err(err(format!(
                "stock_nodes {}: {} elements named {el} in the scene, one expected",
                s.template,
                hits.len()
            )));
        };
        if is_hidden(e) {
            return Err(err(format!(
                "stock_nodes {}: {el} is hidden by default already",
                s.template
            )));
        }
    }
    let fights = visible_fights(template_root, &s.template, &s.hide)?;
    if let Some((n, el)) = fights.first() {
        return Err(err(format!(
            "stock_nodes {}: the stock node {n} already shows or hides {el}; a second visible binding would fight the game's",
            s.template
        )));
    }
    Ok(())
}

/// The group of hide nodes for one `stock_nodes` entry: `visible <- NOT while` (default true) per
/// element, or a static `visible = false` when there is no leaf.
pub fn stock_nodes_group(s: &StockNodes, name: &str, wraps: &[String]) -> Result<Node> {
    let sw = s
        .while_
        .as_ref()
        .map(|b| template::resolve(b, Usage::Visible, "stock_nodes while"))
        .transpose()?;
    let mut kids = Vec::new();
    for el in &s.hide {
        kids.push(match &sw {
            Some(r) => node::visible_node(&format!("v_{el}"), el, true, Some(&r.db), !r.negate),
            None => node::hidden(&format!("v_{el}"), &[el]),
        });
    }
    for w in wraps {
        let el = w.rsplit("_w_").next().unwrap_or(w);
        kids.push(match &sw {
            Some(r) => node::visible_node(&format!("v_w_{el}"), w, true, Some(&r.db), !r.negate),
            None => node::hidden(&format!("v_w_{el}"), &[w]),
        });
    }
    Ok(node::group_node(name, vec![], &[], kids))
}

/// Adds the group under its template node after the checks.
pub fn attach_stock_nodes(
    template: &mut Node,
    stock_template: &Node,
    scene_root: &Node,
    s: &StockNodes,
    name: &str,
    wraps: &[String],
) -> Result<usize> {
    check_stock_nodes(scene_root, stock_template, s)?;
    let at = node::find_path(template, &s.template)?;
    let target = node::node_at_mut(template, &at)?;
    if target
        .slot(node::CHILDREN)?
        .iter()
        .any(|c| c.is_node() && c.name() == name)
    {
        return Err(err(format!("{} already has a {name}", s.template)));
    }
    let mut grp = stock_nodes_group(s, name, wraps)?;
    node::fix_ids(&mut grp, Some(&s.template));
    target.slot_mut(node::CHILDREN)?.push(grp);
    Ok(s.hide.len() + wraps.len())
}

pub const TWIN_LAYER: i32 = 60;

const MS_DIR: &str = "eaid://ui_package/art/osd/";
pub const BANNER_SYSTEM: &str = "eaid://ui_package/art/osd/components/message_system/message_system.idf";
pub const BANNER_PARTS: [&str; 4] = ["centre", "safety_car", "red_flag", "safety_rating"];
/// The stock toast box height in scene units and the banner's height in HUD units (Halo's decision 6).
pub const BANNER_STOCK_H: f64 = 0.86;
pub const BANNER_SCALE: f64 = 22.0 * 0.03 / BANNER_STOCK_H;
pub const BANNER_TOASTS: [&str; 2] = ["hud_notifications_dynamic", "hud_notifications_dynamic_p2"];

/// Scene, fills `(element, slot)`, twin, decorations to hide, the toast group that shows it.
pub type BannerScene = (
    &'static str,
    &'static [(&'static str, &'static str)],
    Option<&'static str>,
    &'static [&'static str],
    &'static str,
);

pub const BANNER_SCENES: [BannerScene; 11] = [
    (
        "units/message_system/central_message.idf",
        &[("pill", "central_message_bg")],
        Some("pill"),
        &["edge_border", "edge_base", "ms_colour"],
        "notification_0.hud_message.right.central_message",
    ),
    (
        "units/message_system/flags.idf",
        &[("bg", "central_message_bg_start")],
        None,
        &["edge_border", "edge_base", "ms_colour", "divider"],
        "notification_0.hud_message.left.flags",
    ),
    (
        "components/message_system/ms_fia_logo.idf",
        &[("bg", "central_message_bg_start")],
        Some("bg"),
        &["edge_border", "edge_base", "ms_colour", "divider"],
        "notification_0.hud_message.left.fia_icon",
    ),
    (
        "units/message_system/race_control_icon.idf",
        &[("icon", "race_control_icon")],
        Some("icon"),
        &["edge_border", "edge_base", "ms_colour", "bg_frame"],
        "notification_0.hud_message.left.race_control_icon",
    ),
    (
        "units/message_system/race_winner_icon.idf",
        &[],
        None,
        &["edge_border", "edge_base", "ms_colour", "bg_frame"],
        "notification_0.hud_message.left.race_winner_icon",
    ),
    (
        "units/message_system/pit_lane_icon.idf",
        &[],
        None,
        &["edge_border", "edge_base", "ms_colour", "bg_frame"],
        "notification_0.hud_message.left.pit_speed_icon",
    ),
    (
        "components/illegal_overtake/illegal_overtake.idf",
        &[("pill", "illegal_overtake_bg")],
        Some("pill"),
        &["edge_border", "edge_base", "ms_colour", "bg_frame"],
        "notification_0.hud_message.left.illegal_overtake_icon",
    ),
    (
        "components/message_system/central_message_winner.idf",
        &[("pill_bg", "central_message_bg")],
        Some("pill_bg"),
        &["edge_border", "edge_base", "ms_colour", "bg_frame"],
        "notification_0.hud_message.right.central_message_winner",
    ),
    (
        "components/message_system/ms_safety_car.idf",
        &[("content_bg", "bg_1")],
        None,
        &["edge_border", "edge_base", "sc_divider", "bg_frame", "divider_1"],
        "safety_car_notifications.hud_message.content",
    ),
    (
        "components/message_system/ms_red_flag.idf",
        &[("content_bg", "bg_1")],
        None,
        &["edge_border", "edge_base", "sc_divider", "bg_frame", "divider_1"],
        "red_flag_notifications.hud_message.content",
    ),
    (
        "components/message_system/ms_safety_rating.idf",
        &[("content_bg", "bg_1")],
        None,
        &["edge_border", "edge_base", "bg_frame", "divider_1"],
        "safety_rating_notifications.hud_message.content",
    ),
];

pub fn banner_scene(rel: &str) -> String {
    format!("{MS_DIR}{rel}")
}

fn toast_parents(group: &str) -> Vec<String> {
    BANNER_TOASTS
        .iter()
        .map(|t| format!("{t}.osd_visibility_option.{t}.{group}"))
        .collect()
}

/// The Race Control banner preset: Halo's `banner_native.py` restyle of the 12 toast scenes with the
/// fills refilled from `sprite`, the four parts of `message_system.idf` scaled about their own
/// position, the title and second line restyled.
pub fn banner(sprite: &str) -> Vec<Restyle> {
    let mut out = vec![Restyle {
        scene: BANNER_SYSTEM.to_owned(),
        fill: BTreeMap::new(),
        twins: Vec::new(),
        template_parents: Vec::new(),
        hide: Vec::new(),
        scale: BANNER_PARTS
            .iter()
            .map(|p| Scale {
                element: (*p).to_owned(),
                by: BANNER_SCALE,
                pivot_y: None,
            })
            .collect(),
        text: BTreeMap::new(),
        params: Vec::new(),
        stock_erp: None,
    }];
    for (rel, fills, twin, hide, group) in BANNER_SCENES {
        let mut text = BTreeMap::new();
        let mut params = Vec::new();
        if rel.ends_with("/central_message.idf") {
            text.insert(
                "text_line_1".to_owned(),
                TextStyle {
                    style: Some("OSD_34_bold".into()),
                    ..TextStyle::default()
                },
            );
            text.insert(
                "text_line_2".to_owned(),
                TextStyle {
                    style: Some("34_regular".into()),
                    ..TextStyle::default()
                },
            );
            params.push(Param {
                element: "stacker_content".into(),
                name: "stackerType".into(),
                value: serde_json::Value::String("l".into()),
            });
        }
        if rel.ends_with("/race_control_icon.idf") {
            text.insert(
                "text".to_owned(),
                TextStyle {
                    style: Some("OSD_34_bold".into()),
                    width: Some(3.2),
                    ..TextStyle::default()
                },
            );
        }
        out.push(Restyle {
            scene: banner_scene(rel),
            fill: fills
                .iter()
                .map(|(el, slot)| {
                    (
                        (*el).to_owned(),
                        Fill {
                            sprite: sprite.to_owned(),
                            slot: (*slot).to_owned(),
                        },
                    )
                })
                .collect(),
            twins: twin.iter().map(|t| (*t).to_owned()).collect(),
            template_parents: toast_parents(group),
            hide: hide.iter().map(|h| (*h).to_owned()).collect(),
            scale: Vec::new(),
            text,
            params,
            stock_erp: None,
        });
    }
    out
}

/// The record's `restyle`: the list as written, or the preset spelled out.
pub fn restyles(rec: &HudElement) -> Result<Vec<Restyle>> {
    let list: Vec<Restyle> = match &rec.restyle {
        None => return Ok(Vec::new()),
        Some(RestyleField::Preset(p)) if p.preset == "banner" => banner(&p.sprite),
        Some(RestyleField::Preset(p)) => {
            return Err(err(format!(
                "restyle: no preset {}; the core knows banner",
                p.preset
            )));
        }
        Some(RestyleField::List(l)) => l.clone(),
    };
    let mut seen = BTreeSet::new();
    for r in &list {
        if !r.scene.starts_with("eaid://") || !r.scene.ends_with(".idf") {
            return Err(err(format!("restyle: scene {} is an eaid://….idf path", r.scene)));
        }
        if !seen.insert(r.scene.clone()) {
            return Err(err(format!("restyle: {} is restyled twice", r.scene)));
        }
        for t in &r.twins {
            if !r.fill.contains_key(t) {
                return Err(err(format!("restyle {}: twin {t} is not a fill", r.scene)));
            }
        }
        for f in r.fill.values() {
            if !rec.sprites.contains_key(&f.sprite) {
                return Err(err(format!("restyle {}: no sprite {}", r.scene, f.sprite)));
            }
        }
        for s in &r.scale {
            if s.by <= 0.0 || s.by.is_nan() {
                return Err(err(format!("restyle {}: scale by is positive", r.scene)));
            }
        }
        if !r.hide.is_empty() && r.template_parents.is_empty() {
            return Err(err(format!(
                "restyle {}: hide needs template_parents, the template nodes that show the scene (isHidden alone does not hold in game)",
                r.scene
            )));
        }
    }
    Ok(list)
}

/// The shared material binding of a record's restyle sprite.
pub fn emb_name(pfx: &str, sprite: &str) -> String {
    format!("eaid://ui_package/art/osd/components/hud/{pfx}_restyle_{sprite}.emb?context=default")
}

/// The mesh resource of a fill quad, whose bytes name its material slot.
pub fn mesh_resource(scene: &str, element: &str) -> String {
    format!("{scene}/{element}?context=render&lod=0&mesh")
}

fn one_mut<'a>(root: &'a mut Node, name: &str, scene: &str) -> Result<&'a mut Node> {
    let n = idf::elements(root)
        .iter()
        .filter(|(_, e)| idf::elem_name(e) == name)
        .count();
    if n != 1 {
        return Err(err(format!("{scene}: {n} elements named {name}, one expected")));
    }
    idf::find_mut(root, name).ok_or_else(|| err(format!("{scene} has no {name}")))
}

fn set_vec(e: &mut Node, key: &str, f: impl FnOnce(&mut [f64; 3])) -> Result<()> {
    let mut v = match key {
        "position" => idf::position(e),
        _ => e
            .as_obj()
            .and_then(|o| o.get(key))
            .and_then(Node::as_val)
            .and_then(|v| match &v.value {
                swofty_formats::pkg::Value::Raw(b) if b.len() >= 12 => {
                    let f = |i: usize| {
                        f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap_or([0; 4])) as f64
                    };
                    Some([f(0), f(1), f(2)])
                }
                _ => None,
            }),
    }
    .unwrap_or([0.0; 3]);
    f(&mut v);
    e.o_mut()?.set(key, Node::Val(idf::vec3([v[0], v[1], v[2], 0.0])));
    Ok(())
}

/// What one restyled scene came out as, for the build lines and the tests.
#[derive(Debug, Clone, Default)]
pub struct Restyled {
    pub fills: Vec<(String, String, String)>,
    pub twins: Vec<String>,
    pub hidden: Vec<String>,
}

/// Applies a restyle to the stock scene bytes. `emb_of(sprite)` is the binding every fill of that
/// sprite points at; `slot_ok(element, slot)` fails when the quad's mesh has no such slot.
pub fn apply(
    bytes: &[u8],
    r: &Restyle,
    emb_of: &dyn Fn(&str) -> String,
    slot_ok: &dyn Fn(&str, &str) -> Result<()>,
) -> Result<(Vec<u8>, Restyled)> {
    let mut pkg = Pkg::parse(bytes)?;
    let entry = &idf::scenes(&pkg)?[0];
    if idf::scene_name(entry) != Some(idf::scene_id(&r.scene)) {
        return Err(err(format!("restyle: the file is not the scene {}", r.scene)));
    }
    let stock_ids = idf::id_map(scene_root(&pkg)?);
    let mut info = Restyled::default();
    let short = r.scene.rsplit('/').next().unwrap_or(&r.scene).to_owned();
    let root = idf::scenes_mut(&mut pkg)?[0].o_mut()?.at_mut("scene")?;
    for (el, f) in &r.fill {
        let want = format!("{}/{el}?context=render", r.scene);
        let e = one_mut(root, el, &r.scene)?;
        let geoms = e.o_mut()?.at_mut("geoms")?.a_mut()?;
        let [g] = &mut geoms[..] else {
            return Err(err(format!(
                "{short}: {el} has {} geoms, one expected",
                geoms.len()
            )));
        };
        let have = g.o()?.str_at("resourceid")?.to_owned();
        if have != want {
            return Err(err(format!("{short}: {el}'s mesh is {have}, not {want}")));
        }
        slot_ok(el, &f.slot)?;
        let emb = emb_of(&f.sprite);
        let ty = g.o()?.at("materialid")?.v()?.ty;
        let mut v = Val::str(emb.clone());
        v.ty = ty;
        g.o_mut()?.set("materialid", Node::Val(v));
        info.fills.push((el.clone(), f.slot.clone(), emb));
    }
    for el in &r.hide {
        let e = one_mut(root, el, &r.scene)?;
        if e.as_obj()
            .and_then(|o| o.get("children"))
            .and_then(Node::as_arr)
            .is_some_and(|a| !a.items.is_empty())
        {
            return Err(err(format!("{short}: {el} has children; hide leaves only")));
        }
        idf::set_param_like(e, "isHidden", Val::bool(true))?;
        info.hidden.push(el.clone());
    }
    for s in &r.scale {
        let e = one_mut(root, &s.element, &r.scene)?;
        let y0 = idf::position(e).unwrap_or([0.0; 3])[1];
        let pivot = s.pivot_y.unwrap_or(y0);
        set_vec(e, "position", |p| p[1] = pivot + (y0 - pivot) * s.by)?;
        set_vec(e, "scale", |v| {
            v[0] *= s.by;
            v[1] *= s.by;
        })?;
    }
    for (el, t) in &r.text {
        let e = one_mut(root, el, &r.scene)?;
        if idf::param_str(e, "typeName").is_some_and(|t| t != "eText") && idf::param(e, "style").is_none() {
            return Err(err(format!("{short}: {el} is no text")));
        }
        if let Some(st) = &t.style {
            idf::set_param_like(e, "style", Val::str(st))?;
        }
        if let Some(w) = t.width {
            idf::set_param_like(e, "boxWidth", idf::fp32(w))?;
        }
        if let Some(a) = &t.align {
            idf::set_param_like(e, "halign", Val::str(a))?;
        }
        if let Some(c) = &t.text_case {
            idf::set_param_like(e, "textcase", Val::str(c))?;
        }
        if let Some([x, y]) = t.at {
            set_vec(e, "position", |p| {
                p[0] = x;
                p[1] = y;
            })?;
        }
    }
    for p in &r.params {
        let e = one_mut(root, &p.element, &r.scene)?;
        let v = match &p.value {
            serde_json::Value::Bool(b) => Val::bool(*b),
            serde_json::Value::Number(n) => idf::fp32(n.as_f64().unwrap_or(0.0)),
            serde_json::Value::String(s) => Val::str(s),
            other => {
                return Err(err(format!(
                    "{short}: param {} takes a bool, number or string, not {other}",
                    p.name
                )));
            }
        };
        idf::set_param_like(e, &p.name, v)?;
    }
    for el in &r.twins {
        let twin = format!("{el}_bn");
        if idf::find(root, &twin).is_some() {
            return Err(err(format!("{short}: {twin} exists")));
        }
        let src = one_mut(root, el, &r.scene)?;
        if src
            .as_obj()
            .and_then(|o| o.get("children"))
            .and_then(Node::as_arr)
            .is_some_and(|a| !a.items.is_empty())
        {
            return Err(err(format!(
                "{short}: {el} has children; a twin copies a childless quad"
            )));
        }
        let mut c = idf::detached(src);
        idf::rename(&mut c, &twin)?;
        if let Some(Node::Arr(a)) = c.o_mut()?.get_mut("params") {
            a.items.retain(|p| {
                !matches!(
                    p.as_obj()
                        .and_then(|o| o.get("name"))
                        .and_then(Node::as_val)
                        .and_then(Val::as_str),
                    Some("isHidden" | "eAnimationGroup.AnimGroup")
                )
            });
        }
        idf::set_param_like(&mut c, "guid", Val::str(guid(&r.scene, &twin)))?;
        idf::set_param_like(&mut c, "eLayer.layer", idf::si32(TWIN_LAYER))?;
        idf::set_param_like(&mut c, "eLayer.layerIsOffset", Val::bool(false))?;
        idf::clear_list(&mut c, "anims")?;
        let parent_path = idf::elements(root)
            .into_iter()
            .find(|(_, e)| idf::elem_name(e) == el)
            .map(|(p, _)| p)
            .unwrap_or_default();
        let parent_name = parent_path
            .rsplit_once('.')
            .map(|(p, _)| p.rsplit('.').next().unwrap_or(p));
        let parent = match parent_name {
            Some(pn) => one_mut(root, pn, &r.scene)?,
            None => &mut *root,
        };
        let kids = idf::children_mut(parent)?;
        let i = kids
            .iter()
            .position(|k| idf::elem_name(k) == el)
            .ok_or_else(|| err(format!("{short}: {el} is not where its path says")))?;
        kids.insert(i + 1, c);
        info.twins.push(twin);
    }
    for kid in idf::children_mut(root)? {
        let n = idf::elem_name(kid).to_owned();
        idf::recompute_ids(kid, &n)?;
    }
    let entry = &mut idf::scenes_mut(&mut pkg)?[0];
    idf::relookup(entry)?;
    let after = idf::id_map(scene_root(&pkg)?);
    if let Some((k, _)) = stock_ids.iter().find(|(k, v)| after.get(*k) != Some(*v)) {
        return Err(err(format!("{short}: the stock element {k} changed its id")));
    }
    Ok((pkg.to_bytes()?, info))
}

/// The static hide nodes of a restyle under every template node it names, in `template`.
pub fn attach_restyle_hides(
    template: &mut Node,
    stock_template: &Node,
    r: &Restyle,
    pfx: &str,
) -> Result<usize> {
    let mut n = 0;
    for parent in &r.template_parents {
        let fights = visible_fights(stock_template, parent, &r.hide)?;
        if let Some((node_path, el)) = fights.first() {
            return Err(err(format!(
                "restyle {}: the stock node {node_path} under {parent} already shows or hides {el}",
                r.scene
            )));
        }
        let at = node::find_path(template, parent)?;
        let target = node::node_at_mut(template, &at)?;
        let have: BTreeSet<String> = target
            .slot(node::CHILDREN)?
            .iter()
            .filter(|c| c.is_node())
            .map(|c| c.name().to_owned())
            .collect();
        for el in &r.hide {
            let name = format!("{pfx}_hide_{el}");
            if have.contains(&name) {
                return Err(err(format!("{parent} already has a {name}")));
            }
            let mut k = node::hidden(&name, &[el]);
            node::fix_ids(&mut k, Some(parent));
            target.slot_mut(node::CHILDREN)?.push(k);
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::node::{arr, obj, s, u32v};

    fn elem(name: &str, kids: Vec<Node>) -> Node {
        obj(vec![
            ("name", s(name)),
            ("elementId", u32v(0)),
            ("params", arr(vec![])),
            ("children", arr(kids)),
        ])
    }

    fn template(kids: Vec<Node>) -> Node {
        let mut t = node::group_node(
            "hud_lap_counter",
            vec![],
            &[],
            vec![node::group_node("osd_visibility_option", vec![], &[], kids)],
        );
        node::fix_ids(&mut t, None);
        arr(vec![t])
    }

    fn entry() -> StockNodes {
        StockNodes {
            wrap: Vec::new(),
            template: "hud_lap_counter.osd_visibility_option".into(),
            scene: None,
            hide: vec!["lap_bg".into(), "lockup".into()],
            while_: Some(Bind::leaf("view_model_data.hud_tt_show")),
        }
    }

    #[test]
    fn stock_nodes_hide_while_the_leaf_is_true_and_refuse_a_bound_element() {
        let scene = elem("Scene Root", vec![elem("lap_bg", vec![]), elem("lockup", vec![])]);
        let stock = template(vec![]);
        let mut t = stock.clone();
        let n = attach_stock_nodes(&mut t, &stock, &scene, &entry(), "tt_sn0", &[]).unwrap();
        assert_eq!(n, 2);
        let at = node::find_path(&t, "hud_lap_counter.osd_visibility_option.tt_sn0").unwrap();
        let grp = node::node_at(&t, &at).unwrap();
        let mut found = Vec::new();
        node::bindings(grp, &[], &mut found);
        assert_eq!(found.len(), 2);
        assert!(
            found
                .iter()
                .all(|b| b.db == "#view_model_data.hud_tt_show" && b.tag == Some(*b"bool"))
        );
        let v = grp.slot(node::CHILDREN).unwrap()[0]
            .props()
            .unwrap()
            .get("visible")
            .unwrap()
            .clone();
        let o = v.as_obj().unwrap();
        assert_eq!(
            o.get("dbentrynegate").and_then(Node::as_val).and_then(as_bool),
            Some(true)
        );
        assert_eq!(
            o.get("value").and_then(Node::as_val).and_then(as_bool),
            Some(true)
        );
        let bound = template(vec![node::visible_node(
            "lap_bg_switch",
            "lap_bg",
            true,
            Some("#x.y"),
            false,
        )]);
        let e = attach_stock_nodes(&mut bound.clone(), &bound, &scene, &entry(), "tt_sn0", &[])
            .unwrap_err()
            .to_string();
        assert!(e.contains("already shows or hides lap_bg"), "{e}");
        let twice = elem(
            "Scene Root",
            vec![
                elem("lap_bg", vec![]),
                elem("lap_bg", vec![]),
                elem("lockup", vec![]),
            ],
        );
        assert!(attach_stock_nodes(&mut stock.clone(), &stock, &twice, &entry(), "tt_sn0", &[]).is_err());
        let mut st = entry();
        st.while_ = None;
        let mut t = stock.clone();
        attach_stock_nodes(&mut t, &stock, &scene, &st, "tt_sn1", &[]).unwrap();
        let at = node::find_path(&t, "hud_lap_counter.osd_visibility_option.tt_sn1.v_lap_bg").unwrap();
        let v = node::node_at(&t, &at)
            .unwrap()
            .props()
            .unwrap()
            .get("visible")
            .unwrap()
            .clone();
        assert_eq!(
            v.as_obj()
                .unwrap()
                .get("value")
                .and_then(Node::as_val)
                .and_then(as_bool),
            Some(false)
        );
        assert!(v.as_obj().unwrap().get("dbentry").is_none());
    }

    #[test]
    fn restyle_records_are_checked() {
        let v = serde_json::json!({
            "screen": "standings", "show": { "view_model": "view_model_data.hud_tt_show" },
            "sprites": { "bn_m": { "png": "art/bn_m.png" } },
            "restyle": [
                { "scene": "eaid://ui_package/art/osd/units/message_system/central_message.idf",
                  "fill": { "pill": { "sprite": "bn_m", "slot": "central_message_bg" } }, "twins": ["pill"],
                  "template_parents": ["hud_notifications_dynamic.osd_visibility_option.hud_notifications_dynamic.notification_0.hud_message.right.central_message"],
                  "hide": ["edge_border"], "scale": [{ "element": "centre", "by": 0.7674, "pivot_y": 0.43 }],
                  "text": { "text_line_1": { "style": "OSD_34_bold" } } }
            ]
        });
        let rec = HudElement::parse(&v).unwrap();
        let r = restyles(&rec).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].fill["pill"].slot, "central_message_bg");
        let mut bad = v.clone();
        bad["restyle"][0]["twins"] = serde_json::json!(["nope"]);
        assert!(restyles(&HudElement::parse(&bad).unwrap()).is_err());
        let mut bad = v.clone();
        bad["restyle"][0]["fill"]["pill"]["sprite"] = "ghost".into();
        assert!(restyles(&HudElement::parse(&bad).unwrap()).is_err());
        let mut bad = v.clone();
        bad["restyle"][0]["template_parents"] = serde_json::json!([]);
        assert!(restyles(&HudElement::parse(&bad).unwrap()).is_err());
        let mut preset = v.clone();
        preset["restyle"] = serde_json::json!({ "preset": "banner", "sprite": "bn_m" });
        let b = restyles(&HudElement::parse(&preset).unwrap()).unwrap();
        assert_eq!(b.len(), 12);
        assert_eq!(b[0].scale.len(), 4);
        assert!((b[0].scale[0].by - 0.7674).abs() < 1e-3);
        assert_eq!(b[1].twins, ["pill"]);
        assert_eq!(b[1].template_parents.len(), 2);
        assert!(b.iter().skip(1).all(|r| !r.hide.is_empty()));
        assert_eq!(
            emb_name("tt", "bn_m"),
            "eaid://ui_package/art/osd/components/hud/tt_restyle_bn_m.emb?context=default"
        );
        assert_eq!(
            mesh_resource("eaid://a/b.idf", "pill"),
            "eaid://a/b.idf/pill?context=render&lod=0&mesh"
        );
    }
}
