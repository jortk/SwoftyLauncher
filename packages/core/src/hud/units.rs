//! Units: instances (eXRefs) of the game's own logo scenes placed in a composed scene, their
//! `textureOverride` bindings carrying the stock node's tag, and the stock facts both rest on.
//!
//! The table, the geometry read, the placement maths and the tag checks follow Project Halo's
//! `tower_native.py` (`logo_geometry`, `series_geometry`, `Scene.logo`, `Scene.series_logo`,
//! `stock_badge_tag`, `stock_series_tag`); Uses work from Project Halo by nyannoying
//! (https://github.com/nyannoying1337/f1-25-project-halo). The scene and template facts they
//! encode are measurements of the game.

use std::collections::BTreeMap;

use swofty_formats::pkg::{Node, Pkg, Val, Value};

use super::Stock;
use super::dials::{self, Tag};
use super::geom::{self, K};
use super::model::Element;
use super::scene::{self, Builder, Cx, Mode};
use super::template::{self, Usage};
use crate::ui::idf;
use crate::ui::node::{self, NodeExt, ObjExt};
use crate::{Error, Result};

fn err(msg: impl std::fmt::Display) -> Error {
    Error::Other(format!("hud: {msg}"))
}

fn first(v: Vec<Vec<u8>>, what: &str) -> Result<Vec<u8>> {
    v.into_iter()
        .next()
        .ok_or_else(|| err(format!("{what} has no data")))
}

pub const ROW: &str = "eaid://ui_package/art/osd/units/driver_position_table/driver_position_row.idf";
pub const TEAM_LOGO: &str =
    "eaid://ui_package/art/osd/units/driver_position_table/driver_position_team_logo.idf";
pub const LAP_COUNTER: &str = "eaid://ui_package/art/osd/components/lap_counter/lap_counter.idf";
pub const CATEGORY_LOGO: &str = "eaid://ui_package/art/osd/units/lap_counter/car_category_logo.idf";
/// The stock pills under the standings' template parent, each with a `team_badge_image` node.
pub const PILLS: &str = "hud_track_position.osd_visibility_option.track_position";
pub const PILL_ROWS: [&str; 5] = ["lead_entry", "entry_0", "entry_1", "entry_2", "entry_3"];
pub const BADGE_NODE: &str = "team_badge_image";
pub const BADGE_LEAF: &str = "team_badge_texture";
/// The stock lap counter's logo node and the leaf it binds, relative to its view model.
pub const SERIES_NODE: &str = "hud_lap_counter.osd_visibility_option.logo";
pub const SERIES_LEAF: &str = "logo_texture";
/// The same leaf by its absolute path: a HUD element's view model is
/// `#view_model_data.hud.content.<its nameId in hud_layouts.uidl>`.
pub const HUD_SERIES: &str = "#view_model_data.hud.content.lap_counter.logo_texture";
/// What the game writes to `logo_texture` in an F1 session; the default until the leaf is read.
pub const SERIES_DEFAULT: &str = "#overrides.osd.timing_tower.timing_tower_logos.lc_f1_logo";
pub const TEXTURE_PROP: &str = "textureOverride";
/// The key of a `{ "screen": … }` texture binding: the screen's own `logo_texture` leaf.
pub const SCREEN_SERIES: &str = "series_logo";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// `at` is the quad's centre (a square logo).
    Centre,
    /// `at` is the quad's top-left corner.
    TopLeft,
}

#[derive(Debug, Clone, Copy)]
pub struct UnitFacts {
    pub key: &'static str,
    /// The unit scene the eXRef instances.
    pub unit: &'static str,
    /// The stock scene whose reference to the unit is cloned, and that reference's element name.
    pub donor_scene: &'static str,
    pub donor_xref: &'static str,
    /// The drawn quad inside the unit, the target of `textureOverride`.
    pub quad: &'static str,
    pub anchor: Anchor,
    /// The stock template node whose binding gives the texture's tag, and the leaf it binds.
    pub tag_node: &'static str,
    pub tag_leaf: &'static str,
}

pub const UNITS: [UnitFacts; 2] = [
    UnitFacts {
        key: "team_logo",
        unit: TEAM_LOGO,
        donor_scene: ROW,
        donor_xref: "team_logo",
        quad: "team_logo",
        anchor: Anchor::Centre,
        tag_node: BADGE_NODE,
        tag_leaf: BADGE_LEAF,
    },
    UnitFacts {
        key: "series_logo",
        unit: CATEGORY_LOGO,
        donor_scene: LAP_COUNTER,
        donor_xref: "car_category_logo",
        quad: "logo_icon",
        anchor: Anchor::TopLeft,
        tag_node: SERIES_NODE,
        tag_leaf: SERIES_LEAF,
    },
];

pub fn facts(key: &str) -> Option<&'static UnitFacts> {
    UNITS.iter().find(|u| u.key == key)
}

pub fn keys() -> String {
    UNITS.iter().map(|u| u.key).collect::<Vec<_>>().join(", ")
}

/// The unit's drawn quad in the unit's root frame (scene units, y up): the point `at` lands on,
/// its size and its layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    pub anchor: [f64; 2],
    pub width: f64,
    pub height: f64,
    pub layer: i32,
}

fn floats(e: &Node, key: &str) -> Option<Vec<f64>> {
    match &e.as_obj()?.get(key)?.as_val()?.value {
        Value::Raw(b) => Some(
            b.chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().expect("4 bytes")) as f64)
                .collect(),
        ),
        Value::Str(_) => None,
    }
}

fn param_floats(e: &Node, name: &str) -> Option<Vec<f64>> {
    match &idf::param(e, name)?.value {
        Value::Raw(b) => Some(
            b.chunks_exact(4)
                .map(|c| f32::from_le_bytes(c.try_into().expect("4 bytes")) as f64)
                .collect(),
        ),
        Value::Str(_) => None,
    }
}

fn param_i32(e: &Node, name: &str) -> Option<i32> {
    match &idf::param(e, name)?.value {
        Value::Raw(b) if b.len() >= 4 => Some(i32::from_le_bytes(b[..4].try_into().ok()?)),
        _ => None,
    }
}

fn param_bool(e: &Node, name: &str) -> Option<bool> {
    match &idf::param(e, name)?.value {
        Value::Raw(b) if !b.is_empty() => Some(b[0] != 0),
        _ => None,
    }
}

fn has_items(e: &Node, key: &str) -> bool {
    e.as_obj()
        .and_then(|o| o.get(key))
        .and_then(Node::as_arr)
        .is_some_and(|a| !a.items.is_empty())
}

fn chain_to<'a>(root: &'a Node, target: &str, trail: &mut Vec<&'a Node>) -> bool {
    trail.push(root);
    if idf::elem_name(root) == target {
        return true;
    }
    let kids = root
        .as_obj()
        .and_then(|o| o.get("children"))
        .and_then(Node::as_arr)
        .map(|a| a.items.iter().filter(|c| c.as_obj().is_some()));
    for c in kids.into_iter().flatten() {
        if chain_to(c, target, trail) {
            return true;
        }
    }
    trail.pop();
    false
}

fn scene_pkg(bytes: &[u8], want: &str) -> Result<Pkg> {
    let p = Pkg::parse(bytes)?;
    let entry = idf::scenes(&p)?
        .first()
        .ok_or_else(|| err(format!("{want}: the file has no scene")))?;
    if idf::scene_name(entry) != Some(idf::scene_id(want)) {
        return Err(err(format!("{want}: the bytes are not that scene")));
    }
    Ok(p)
}

fn root_of(p: &Pkg) -> Result<&Node> {
    idf::scenes(p)?
        .first()
        .ok_or_else(|| err("the file has no scene"))?
        .o()?
        .at("scene")
}

/// The drawn quad of a unit: one quad named `facts.quad` on the unit's own mesh, ancestors that
/// only translate, an absolute layer; for a centred unit a centred square.
pub fn geometry(f: &UnitFacts, unit_bytes: &[u8]) -> Result<Geometry> {
    let p = scene_pkg(unit_bytes, f.unit)?;
    let root = root_of(&p)?;
    let short = f.unit.rsplit('/').next().unwrap_or(f.unit);
    let quads: Vec<&Node> = idf::elements(root)
        .into_iter()
        .filter(|(_, e)| idf::elem_name(e) == f.quad)
        .map(|(_, e)| e)
        .collect();
    let [q] = quads[..] else {
        return Err(err(format!("{short}: {} quads named {}", quads.len(), f.quad)));
    };
    let mesh = q
        .as_obj()
        .and_then(|o| o.get("geoms"))
        .and_then(Node::as_arr)
        .and_then(|a| a.items.first())
        .and_then(Node::as_obj)
        .and_then(|g| g.get("resourceid"))
        .and_then(Node::as_val)
        .and_then(Val::as_str)
        .unwrap_or_default();
    if !mesh.starts_with(&format!("{}/{}", f.unit, f.quad)) {
        return Err(err(format!("{short}: the quad's mesh is {mesh:?}, not its own")));
    }
    let drawn: Vec<&str> = idf::elements(root)
        .into_iter()
        .filter(|(_, e)| has_items(e, "geoms"))
        .map(|(_, e)| idf::elem_name(e))
        .collect();
    if drawn != [f.quad] {
        return Err(err(format!(
            "{short}: drawn elements {drawn:?}, not only {}",
            f.quad
        )));
    }
    let lo =
        param_floats(q, "minExtents").ok_or_else(|| err(format!("{short}: the quad has no minExtents")))?;
    let hi =
        param_floats(q, "maxExtents").ok_or_else(|| err(format!("{short}: the quad has no maxExtents")))?;
    if lo.len() < 3 || hi.len() < 3 || hi[0] <= lo[0] || hi[2] <= lo[2] {
        return Err(err(format!("{short}: quad extents {lo:?} {hi:?}")));
    }
    let rot = floats(q, "rotation").unwrap_or_default();
    let half = std::f64::consts::FRAC_1_SQRT_2;
    let turned = rot.len() >= 4
        && (rot[0].abs() - half).abs() < 1e-3
        && rot[1].abs() < 1e-5
        && rot[2].abs() < 1e-5
        && (rot[3].abs() - half).abs() < 1e-3;
    if !turned {
        return Err(err(format!(
            "{short}: the quad is not turned a quarter about x ({rot:?})"
        )));
    }
    let unit_scale =
        |e: &Node| floats(e, "scale").is_some_and(|s| s.iter().take(3).all(|v| (v - 1.0).abs() < 1e-6));
    if !unit_scale(q) {
        return Err(err(format!("{short}: the quad is scaled")));
    }
    let mut chain = Vec::new();
    if !chain_to(root, f.quad, &mut chain) {
        return Err(err(format!("{short}: no path to {}", f.quad)));
    }
    let (mut x, mut y) = (0.0, 0.0);
    for e in &chain {
        if idf::elem_name(e) != f.quad && !std::ptr::eq(*e, root) {
            if !unit_scale(e) {
                return Err(err(format!("{short}: a scaled ancestor {}", idf::elem_name(e))));
            }
            let r = floats(e, "rotation").unwrap_or_default();
            if r.len() < 4 || (r[3].abs() - 1.0).abs() > 1e-6 {
                return Err(err(format!("{short}: a rotated ancestor {}", idf::elem_name(e))));
            }
        }
        if let Some(p) = idf::position(e) {
            x += p[0];
            y += p[1];
        }
    }
    let layer = param_i32(q, "eLayer.layer").ok_or_else(|| err(format!("{short}: the quad has no layer")))?;
    if param_bool(q, "eLayer.layerIsOffset").unwrap_or(false) {
        return Err(err(format!("{short}: the quad's layer is an offset")));
    }
    let (w, h) = (hi[0] - lo[0], hi[2] - lo[2]);
    let anchor = match f.anchor {
        Anchor::Centre => {
            let centred =
                (w - h).abs() < 1e-6 && (lo[0] + hi[0]).abs() < 1e-6 && (lo[2] + hi[2]).abs() < 1e-6;
            if !centred {
                return Err(err(format!(
                    "{short}: the quad is not a centred square ({lo:?} {hi:?})"
                )));
            }
            [x, y]
        }
        Anchor::TopLeft => {
            if lo[1].abs() > 1e-6 || hi[1].abs() > 1e-6 {
                return Err(err(format!("{short}: the quad has depth ({lo:?} {hi:?})")));
            }
            [x + lo[0], y - lo[2]]
        }
    };
    Ok(Geometry {
        anchor,
        width: w,
        height: h,
        layer,
    })
}

/// The donor scene's reference to the unit: one eXRef with the unit's `fileRef`, xref id and
/// material binding, no children, geoms or `isHidden`.
pub fn donor(f: &UnitFacts, donor_bytes: &[u8]) -> Result<Node> {
    let p = scene_pkg(donor_bytes, f.donor_scene)?;
    let root = root_of(&p)?;
    let short = f.donor_scene.rsplit('/').next().unwrap_or(f.donor_scene);
    let found: Vec<&Node> = idf::elements(root)
        .into_iter()
        .filter(|(_, e)| idf::elem_name(e) == f.donor_xref)
        .map(|(_, e)| e)
        .collect();
    let [x] = found[..] else {
        return Err(err(format!(
            "{short}: {} elements named {}",
            found.len(),
            f.donor_xref
        )));
    };
    let ty = crate::ui::catalog::element_type(x);
    if ty != "eXRef" {
        return Err(err(format!("{short}: {} is an {ty}, not an eXRef", f.donor_xref)));
    }
    if idf::param_str(x, "fileRef") != Some(f.unit) {
        return Err(err(format!(
            "{short}: {} references {:?}, not {}",
            f.donor_xref,
            idf::param_str(x, "fileRef"),
            f.unit
        )));
    }
    if idf::xref(x) != Some(idf::scene_id(f.unit)) {
        return Err(err(format!(
            "{short}: {}'s xref id is not the unit's",
            f.donor_xref
        )));
    }
    if idf::param_str(x, "XrefMaterialBinding") != Some(idf::material_binding(f.unit).as_str()) {
        return Err(err(format!(
            "{short}: {}'s material binding is not the unit's",
            f.donor_xref
        )));
    }
    if has_items(x, "children") || has_items(x, "geoms") {
        return Err(err(format!("{short}: {} has children or geoms", f.donor_xref)));
    }
    if idf::param(x, "isHidden").is_some() {
        return Err(err(format!(
            "{short}: {} sets isHidden; the stock reference never does",
            f.donor_xref
        )));
    }
    Ok(idf::detached(x))
}

/// A unit ready to place: its checked donor reference and the geometry of its quad.
#[derive(Debug, Clone)]
pub struct Unit {
    pub facts: &'static UnitFacts,
    pub xref: Node,
    pub geom: Geometry,
}

pub type Units = BTreeMap<String, Unit>;

/// Every unit of the table, read from `erp`.
pub fn load(stock: &dyn Stock, erp: &str) -> Result<Units> {
    let mut out = Units::new();
    for f in &UNITS {
        let donor_bytes = first(stock.read(erp, f.donor_scene)?, f.donor_scene)?;
        let unit_bytes = first(stock.read(erp, f.unit)?, f.unit)?;
        out.insert(
            f.key.to_owned(),
            Unit {
                facts: f,
                xref: donor(f, &donor_bytes)?,
                geom: geometry(f, &unit_bytes)?,
            },
        );
    }
    Ok(out)
}

/// Where an instance of `u` goes so that its quad is `size` HUD units wide with its anchor on
/// HUD `at` of the parent's frame: the position and the uniform scale.
pub fn place(u: &Unit, at: [f64; 2], size: f64) -> ([f64; 3], f64) {
    let s = K * size / u.geom.width;
    let p = geom::scene_point(at[0], at[1]);
    ([p[0] - s * u.geom.anchor[0], p[1] - s * u.geom.anchor[1], 0.0], s)
}

/// Where a texture binding reads from: the addon's leaf, or the screen's own `logo_texture`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TexSource {
    Leaf(String),
    Screen(String),
}

/// One `textureOverride` node of a plan: `x_<id>` over `[element, quad]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TexItem {
    pub name: String,
    pub element: String,
    pub quad: String,
    pub unit: &'static str,
    pub source: TexSource,
    pub tag: Tag,
    pub default: Option<String>,
}

/// An element whose `texture` or `unit` field is set is a unit, and a unit has both `unit` and `size`.
pub fn check_element(e: &Element) -> Result<()> {
    let is_unit = e.kind == Some(super::model::Kind::Unit);
    if !is_unit && (e.texture.is_some() || e.unit.is_some() || e.size.is_some()) {
        return Err(err(format!(
            "element {}: texture, unit and size belong to a unit element, not {:?}",
            e.id, e.kind
        )));
    }
    if is_unit {
        let key = e
            .unit
            .as_deref()
            .ok_or_else(|| err(format!("element {}: a unit needs unit, one of {}", e.id, keys())))?;
        if facts(key).is_none() {
            return Err(err(format!(
                "element {}: no unit {key}; the core knows {}",
                e.id,
                keys()
            )));
        }
        if e.size.is_none_or(|s| s <= 0.0) {
            return Err(err(format!(
                "element {}: a unit needs size, its quad's width in HUD units",
                e.id
            )));
        }
        if e.sprite.is_some() || e.layer.is_some() || e.value.is_some() || e.literal.is_some() {
            return Err(err(format!(
                "element {}: a unit takes at, size, visible and texture only",
                e.id
            )));
        }
        if let Some(t) = &e.texture
            && t.screen.as_deref().is_some_and(|s| s != SCREEN_SERIES)
        {
            return Err(err(format!(
                "element {}: a screen texture is {SCREEN_SERIES}, not {:?}",
                e.id, t.screen
            )));
        }
    }
    Ok(())
}

/// Whether any element of `list` (or of its children) is a unit.
pub fn used(list: &[Element]) -> bool {
    list.iter()
        .any(|e| e.kind == Some(super::model::Kind::Unit) || used(&e.children))
}

/// Clones the unit's reference into the scene for element `e` named `name`, scaled to `size`, and
/// records its texture binding. Nothing in a preview: the editor draws no logo.
pub(crate) fn build(b: &mut Builder, cx: &mut Cx, e: &Element, name: &str) -> Result<Option<Node>> {
    check_element(e)?;
    if let Mode::Preview(_) = cx.mode {
        return Ok(None);
    }
    let key = e.unit.as_deref().unwrap_or_default();
    let u = b.units.get(key).cloned().ok_or_else(|| {
        err(format!(
            "element {}: the scene builder has no unit {key} loaded (units::load)",
            e.id
        ))
    })?;
    if b.existing(name) {
        return Err(err(format!(
            "the scene {} already has an element {name}",
            b.facts.scene
        )));
    }
    let (pos, s) = place(&u, e.at.unwrap_or([0.0, 0.0]), e.size.unwrap_or(0.0));
    let mut n = u.xref.clone();
    idf::rename(&mut n, name)?;
    idf::set_param_like(&mut n, "guid", Val::str(scene::guid(b.facts.scene, name)))?;
    idf::set_transform(&mut n, pos, geom::QUAT_IDENTITY, [s, s, 1.0])?;
    idf::clear_list(&mut n, "anims")?;
    b.names.insert(name.to_owned());
    if let Some(t) = &e.texture {
        let (source, tag) = match &t.screen {
            Some(s) => (TexSource::Screen(s.clone()), *b"stri"),
            None => {
                let r = template::resolve(t, Usage::Texture, name)?;
                (TexSource::Leaf(r.db), r.tag)
            }
        };
        cx.plan.textures.push(TexItem {
            name: format!("x_{}", e.id),
            element: name.to_owned(),
            quad: u.facts.quad.to_owned(),
            unit: u.facts.key,
            source,
            tag,
            default: t.default.as_ref().map(|d| match d {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            }),
        });
    }
    Ok(Some(n))
}

fn layout_names(n: &Node) -> Vec<Vec<String>> {
    n.slot(node::LAYOUT)
        .map(|lay| {
            lay.iter()
                .map(|grp| {
                    grp.as_arr()
                        .map(|a| {
                            a.items
                                .iter()
                                .filter_map(|b| {
                                    b.as_obj()?
                                        .get("debugName")?
                                        .as_val()?
                                        .as_str()
                                        .map(str::to_owned)
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The tag a stock template node binds `TEXTURE_PROP` with, on layout `[xref, quad]` from `leaf`.
fn texture_tag(root: &Node, path: &str, xref: &str, quad: &str, leaf: &str) -> Result<Tag> {
    let at = node::find_path(root, path)?;
    let n = node::node_at(root, &at)?;
    let lay = layout_names(n);
    if lay != [[xref.to_owned(), quad.to_owned()]] {
        return Err(err(format!("{path}: layout {lay:?}, not [{xref} > {quad}]")));
    }
    let p = n
        .props()?
        .get(TEXTURE_PROP)
        .and_then(Node::as_obj)
        .ok_or_else(|| err(format!("{path}: no {TEXTURE_PROP}")))?;
    let db = p.get("dbentry").and_then(Node::as_val).and_then(Val::as_str);
    if db != Some(leaf) {
        return Err(err(format!("{path}: {TEXTURE_PROP} binds {db:?}, not {leaf}")));
    }
    if p.get("dbentrynegate").is_some() {
        return Err(err(format!("{path}: {TEXTURE_PROP} is negated")));
    }
    p.get("value")
        .and_then(Node::as_val)
        .map(|v| v.ty)
        .ok_or_else(|| err(format!("{path}: {TEXTURE_PROP} has no value")))
}

/// The stock pills' badge tag: `team_badge_image.textureOverride <- team_badge_texture` on
/// `team_logo > team_logo`, the same on all five pills.
pub fn badge_tag(hud_template: &Node) -> Result<Tag> {
    let f = facts("team_logo").expect("in the table");
    let mut tags = std::collections::BTreeSet::new();
    for row in PILL_ROWS {
        tags.insert(texture_tag(
            hud_template,
            &format!("{PILLS}.{row}.{}", f.tag_node),
            f.donor_xref,
            f.quad,
            f.tag_leaf,
        )?);
    }
    let mut it = tags.into_iter();
    match (it.next(), it.next()) {
        (Some(t), None) => Ok(t),
        (a, b) => Err(err(format!(
            "the stock pills bind {BADGE_LEAF} with different tags: {:?}",
            [a, b].iter().flatten().map(dials::tag_name).collect::<Vec<_>>()
        ))),
    }
}

/// The stock lap counter's logo tag: `logo.textureOverride <- logo_texture` on
/// `car_category_logo > logo_icon`, the only stock node under `hud_lap_counter` binding that leaf.
pub fn series_tag(hud_template: &Node) -> Result<Tag> {
    let f = facts("series_logo").expect("in the table");
    let tag = texture_tag(hud_template, f.tag_node, f.donor_xref, f.quad, f.tag_leaf)?;
    let top = f.tag_node.split('.').next().unwrap_or_default();
    let at = node::find_path(hud_template, top)?;
    let mut all = Vec::new();
    node::bindings(node::node_at(hud_template, &at)?, &[], &mut all);
    let users: Vec<&str> = all
        .iter()
        .filter(|b| b.db == f.tag_leaf)
        .map(|b| b.node.as_str())
        .collect();
    if users.len() != 1 {
        return Err(err(format!(
            "{top}: {} nodes bind {}: {users:?}",
            users.len(),
            f.tag_leaf
        )));
    }
    Ok(tag)
}

/// The texture tags of the stock HUD template, and the `logo_texture` leaf of the screen being built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TexFacts {
    pub badge_tag: Tag,
    pub series_tag: Tag,
    /// The screen's own `logo_texture` leaf (absolute) and its default while unread.
    pub series: Option<(String, String)>,
}

impl TexFacts {
    /// From the stock `hud_dynamic_templates.ui`: the HUD's own lap counter leaf is the series leaf.
    pub fn hud(hud_template: &Node) -> Result<Self> {
        Ok(Self {
            badge_tag: badge_tag(hud_template)?,
            series_tag: series_tag(hud_template)?,
            series: Some((HUD_SERIES.to_owned(), SERIES_DEFAULT.to_owned())),
        })
    }

    /// The same tags with another screen's `logo_texture` leaf (`#replay_flow…`, the spectator VM's).
    pub fn for_screen(hud_template: &Node, series_db: &str) -> Result<Self> {
        let mut f = Self::hud(hud_template)?;
        f.series = Some((series_db.to_owned(), SERIES_DEFAULT.to_owned()));
        Ok(f)
    }

    fn tag_for(&self, unit: &str) -> Tag {
        if unit == "series_logo" {
            self.series_tag
        } else {
            self.badge_tag
        }
    }
}

/// The `x_<id>` node of one texture item: `textureOverride` on `[element, quad]`, the stock tag, the
/// item's default or the stock default (an empty string; the series leaf's F1 value).
pub fn texture_node(item: &TexItem, facts: &TexFacts) -> Result<Node> {
    let (db, tag, default) = match &item.source {
        TexSource::Screen(key) => {
            if key != SCREEN_SERIES {
                return Err(err(format!(
                    "{}: no screen texture {key}; the core knows {SCREEN_SERIES}",
                    item.name
                )));
            }
            let (db, dflt) = facts.series.clone().ok_or_else(|| {
                err(format!(
                    "{}: a {SCREEN_SERIES} texture needs the screen's logo_texture leaf",
                    item.name
                ))
            })?;
            (db, facts.series_tag, item.default.clone().unwrap_or(dflt))
        }
        TexSource::Leaf(db) => {
            let want = facts.tag_for(item.unit);
            if item.tag != want {
                return Err(err(format!(
                    "{}: bound as {}, the stock {} node binds {} as {}",
                    item.name,
                    dials::tag_name(&item.tag),
                    item.unit,
                    facts
                        .tag_for(item.unit)
                        .iter()
                        .map(|b| *b as char)
                        .collect::<String>(),
                    dials::tag_name(&want)
                )));
            }
            (db.clone(), want, item.default.clone().unwrap_or_default())
        }
    };
    Ok(node::texture_node(
        &item.name,
        &[&item.element, &item.quad],
        tag,
        &default,
        &db,
    ))
}

/// The facts of this module against the stock files, one line per fact; `Err` is a failed fact.
pub fn stock_facts(stock: &dyn Stock) -> Vec<Result<String>> {
    let mut out = Vec::new();
    let erp = dials::STOCK_ERP;
    let read = |name: &str| first(stock.read(erp, name)?, name);
    for f in &UNITS {
        let short = |s: &str| s.rsplit('/').next().unwrap_or(s).to_owned();
        out.push(read(f.donor_scene).and_then(|b| donor(f, &b)).map(|_| {
            format!(
                "{}: {} is the stock eXRef of {}",
                short(f.donor_scene),
                f.donor_xref,
                short(f.unit)
            )
        }));
        out.push(read(f.unit).and_then(|b| geometry(f, &b)).map(|g| {
            format!(
                "{}: quad {} at {:?}, {:.3} x {:.3}, layer {}",
                short(f.unit),
                f.quad,
                g.anchor.map(|v| (v * 1000.0).round() / 1000.0),
                g.width,
                g.height,
                g.layer
            )
        }));
    }
    let template = read(dials::TEMPLATE).and_then(|b| Ok(Pkg::parse(&b)?));
    match &template {
        Ok(t) => {
            out.push(badge_tag(&t.root).map(|tag| {
                format!(
                    "{}: the five pills bind {BADGE_LEAF} as {}",
                    dials::TEMPLATE.rsplit('/').next().unwrap_or_default(),
                    dials::tag_name(&tag)
                )
            }));
            out.push(
                series_tag(&t.root)
                    .map(|tag| format!("{SERIES_NODE} binds {SERIES_LEAF} as {}", dials::tag_name(&tag))),
            );
            let tags = template::StockTags::race(&t.root);
            let want = |db: &str, tag: Tag| -> Result<String> {
                match tags.get(db) {
                    Some(s) if s.contains(&tag) && s.len() == 1 => Ok(format!(
                        "the stock template binds {db} as {}",
                        dials::tag_name(&tag)
                    )),
                    Some(s) => Err(err(format!(
                        "the stock template binds {db} as {}, the table says {}",
                        s.iter().map(dials::tag_name).collect::<Vec<_>>().join("/"),
                        dials::tag_name(&tag)
                    ))),
                    None => Err(err(format!("no stock node binds {db}"))),
                }
            };
            out.push(want(dials::TEAM_COLOUR, *b"rgba"));
            out.push(want("#session.number_of_laps", *b"si32"));
            out.push(match tags.get("#session.current_car.driver.name_code") {
                None => Ok(
                    "no stock node binds #session.current_car.driver.name_code (the table's stri is trusted)"
                        .to_owned(),
                ),
                Some(s) => Err(err(format!(
                    "the stock template binds #session.current_car.driver.name_code as {}",
                    s.iter().map(dials::tag_name).collect::<Vec<_>>().join("/")
                ))),
            });
            out.push(template::clock_formatter(&t.root, "x").map(|_| {
                format!(
                    "{}.{} is the one stock UITF of the session clock",
                    dials::CLOCK_MODEL,
                    dials::CLOCK_FORMATTER
                )
            }));
        }
        Err(e) => out.push(Err(err(format!("{}: {e}", dials::TEMPLATE)))),
    }
    for tex in super::textures::STOCK_TEXTURES {
        out.push(
            super::textures::texture_size(stock, erp, tex)
                .map(|(w, h)| format!("{}: {w} x {h} px", tex.rsplit('/').next().unwrap_or(tex))),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_names_two_units_with_their_stock_paths() {
        assert_eq!(facts("team_logo").unwrap().quad, "team_logo");
        assert_eq!(facts("series_logo").unwrap().anchor, Anchor::TopLeft);
        assert!(facts("tyre").is_none());
        assert_eq!(keys(), "team_logo, series_logo");
    }

    #[test]
    fn placement_scales_the_quad_to_size_and_offsets_by_the_anchor() {
        let u = Unit {
            facts: facts("team_logo").unwrap(),
            xref: Node::Arr(swofty_formats::pkg::Arr::new(vec![])),
            geom: Geometry {
                anchor: [0.12, -0.18],
                width: 0.24,
                height: 0.24,
                layer: 30,
            },
        };
        let (pos, s) = place(&u, [10.0, 20.0], 14.1);
        let want_s = K * 14.1 / 0.24;
        assert!((s - want_s).abs() < 1e-9);
        assert!((pos[0] - (K * 10.0 - want_s * 0.12)).abs() < 1e-9);
        assert!((pos[1] - (-K * 20.0 + want_s * 0.18)).abs() < 1e-9);
    }

    #[test]
    fn texture_nodes_take_the_stock_tag_and_the_screen_leaf() {
        let facts = TexFacts {
            badge_tag: *b"stri",
            series_tag: *b"stri",
            series: Some((
                "#replay_flow.replays_and_highlights.logo_texture".into(),
                SERIES_DEFAULT.into(),
            )),
        };
        let badge = TexItem {
            name: "x_lg".into(),
            element: "t_r00_lg".into(),
            quad: "team_logo".into(),
            unit: "team_logo",
            source: TexSource::Leaf("#view_model_data.hud_tt_r00_badge".into()),
            tag: *b"stri",
            default: None,
        };
        let n = texture_node(&badge, &facts).unwrap();
        let mut found = Vec::new();
        node::bindings(&n, &[], &mut found);
        assert_eq!(found[0].prop, TEXTURE_PROP);
        assert_eq!(found[0].db, "#view_model_data.hud_tt_r00_badge");
        assert_eq!(found[0].tag, Some(*b"stri"));
        assert_eq!(layout_names(&n), [["t_r00_lg", "team_logo"]]);
        let series = TexItem {
            unit: "series_logo",
            quad: "logo_icon".into(),
            source: TexSource::Screen(SCREEN_SERIES.into()),
            ..badge.clone()
        };
        let n = texture_node(&series, &facts).unwrap();
        found.clear();
        node::bindings(&n, &[], &mut found);
        assert_eq!(found[0].db, "#replay_flow.replays_and_highlights.logo_texture");
        let mut wrong = badge.clone();
        wrong.tag = *b"ui32";
        assert!(
            texture_node(&wrong, &facts)
                .unwrap_err()
                .to_string()
                .contains("bound as ui32")
        );
        let mut none = facts.clone();
        none.series = None;
        assert!(
            texture_node(&series, &none)
                .unwrap_err()
                .to_string()
                .contains("logo_texture")
        );
        let mut other = series.clone();
        other.source = TexSource::Screen("flag".into());
        assert!(texture_node(&other, &facts).is_err());
    }

    #[test]
    fn a_texture_belongs_to_a_unit_with_a_known_key_and_a_size() {
        use super::super::model::{Bind, Kind};
        let mut e = Element {
            id: "lg".into(),
            kind: Some(Kind::Sprite),
            sprite: Some("x".into()),
            texture: Some(Bind::leaf("view_model_data.hud_x_badge")),
            ..Element::default()
        };
        assert!(
            check_element(&e)
                .unwrap_err()
                .to_string()
                .contains("belong to a unit")
        );
        e.kind = Some(Kind::Unit);
        e.sprite = None;
        assert!(check_element(&e).unwrap_err().to_string().contains("needs unit"));
        e.unit = Some("flag".into());
        assert!(
            check_element(&e)
                .unwrap_err()
                .to_string()
                .contains("no unit flag")
        );
        e.unit = Some("team_logo".into());
        assert!(check_element(&e).unwrap_err().to_string().contains("needs size"));
        e.size = Some(14.1);
        check_element(&e).unwrap();
        e.texture = Some(Bind {
            screen: Some("flag".into()),
            ..Bind::default()
        });
        assert!(check_element(&e).is_err());
    }
}

#[cfg(test)]
mod game_tests {
    use swofty_formats::pkg::Pkg;

    use super::super::model::HudElement;
    use super::super::scene::{Builder, Mode, SpriteInfo, Sprites};
    use super::super::template::{self, StockTags};
    use super::super::{GameStock, Stock, dials, textures};
    use super::*;

    fn game() -> Option<GameStock> {
        let Some(dir) = std::env::var_os("F1_25_DIR") else {
            eprintln!("F1_25_DIR is not set; skipping");
            return None;
        };
        let data = crate::store::Store::open_default().root;
        Some(GameStock::with_baseline(std::path::PathBuf::from(dir), &data).expect("the launcher's baseline"))
    }

    fn read(stock: &dyn Stock, name: &str) -> Vec<u8> {
        first(stock.read(dials::STOCK_ERP, name).unwrap(), name).unwrap()
    }

    #[test]
    fn the_unit_facts_hold_in_the_stock_files() {
        let Some(stock) = game() else { return };
        let lines = stock_facts(&stock);
        let mut bad = Vec::new();
        for l in &lines {
            match l {
                Ok(l) => eprintln!("ok   {l}"),
                Err(e) => bad.push(e.to_string()),
            }
        }
        assert!(bad.is_empty(), "{bad:#?}");
        assert_eq!(lines.len(), 2 * UNITS.len() + 6 + textures::STOCK_TEXTURES.len());
        let logo = geometry(facts("team_logo").unwrap(), &read(&stock, TEAM_LOGO)).unwrap();
        assert!(
            (logo.anchor[0] - 0.12).abs() < 1e-4 && (logo.anchor[1] + 0.18).abs() < 1e-4,
            "{logo:?}"
        );
        assert!((logo.width - 0.24).abs() < 1e-4 && logo.layer == 30, "{logo:?}");
        let units = load(&stock, dials::STOCK_ERP).unwrap();
        assert_eq!(units.len(), 2);
        assert_eq!(
            idf::param_str(&units["series_logo"].xref, "fileRef"),
            Some(CATEGORY_LOGO)
        );
    }

    #[test]
    fn a_logo_unit_a_bar_and_a_series_logo_compose_with_the_stock_tags() {
        let Some(stock) = game() else { return };
        let f = dials::facts(super::super::model::Dial::F126);
        let mut b = Builder::new(
            f,
            &read(&stock, f.scene),
            &read(&stock, dials::MODS),
            &read(&stock, dials::BAR),
            None,
        )
        .unwrap();
        b.units = load(&stock, dials::STOCK_ERP).unwrap();
        let fill = read(&stock, dials::FILL_MATERIAL);
        let size = textures::texture_size(&stock, dials::STOCK_ERP, textures::F1_LOGO).unwrap();
        let f1 = textures::stock_names("t_", "f1", textures::F1_LOGO);
        let res = textures::stock_sprite_resources(&f1, &fill, Some([0, 0, 0, 255])).unwrap();
        let m = swofty_formats::material::Material::parse(&res[0].2[0]).unwrap();
        assert_eq!(m.texture("me_diffuseTex"), Some(textures::F1_LOGO));
        assert_eq!(
            swofty_formats::material::get(&res[0].2[0], "me_colour").unwrap(),
            [0.0, 0.0, 0.0, 1.0]
        );
        let mut sprites = Sprites::new();
        sprites.insert(
            "f1".into(),
            SpriteInfo {
                bounds: textures::fit(size, [34.2, 12.0]),
                names: f1,
                tinted: None,
            },
        );
        sprites.insert(
            "bar".into(),
            SpriteInfo {
                bounds: [0.0, -6.0, 2.0, 6.0],
                names: textures::names("t_", "bar"),
                tinted: None,
            },
        );
        let rec = HudElement::parse(&serde_json::json!({
            "dial": "f1_26",
            "sprites": { "bar": { "png": "art/bar.png", "bounds": [0, -6, 2, 6] },
                         "f1": { "texture": textures::F1_LOGO, "fit": [34.2, 12.0] } },
            "elements": [
                { "kind": "sprite", "id": "f1", "sprite": "f1", "layer": 20, "at": [0, -30] },
                { "kind": "unit", "id": "logo", "unit": "series_logo", "at": [8.3, 1.9], "size": 20.0,
                  "texture": { "screen": "series_logo" } },
                { "kind": "group", "id": "r03", "visible": { "view_model": "view_model_data.hud_tt_r03_on" }, "children": [
                    { "kind": "text", "id": "code", "style": "OSD_24_bold", "align": "left", "box": 24.5, "cap_height": 7.5,
                      "value": { "session_car": "driver.name_code", "slot_var": "3" } },
                    { "kind": "sprite", "id": "bar", "sprite": "bar", "layer": 30,
                      "tint": { "session_car": "nub_colour", "slot_var": "3", "default": "#00000000" } },
                    { "kind": "unit", "id": "lg", "unit": "team_logo", "at": [11.0, 10.8], "size": 14.1,
                      "visible": { "view_model": "view_model_data.hud_tt_r03_bdg" },
                      "texture": { "view_model": "view_model_data.hud_tt_r03_badge" } },
                    { "kind": "text", "id": "laps", "style": "OSD_24_bold", "align": "left", "box": 20, "cap_height": 7.5,
                      "value": { "session": "number_of_laps" } },
                    { "kind": "text", "id": "clock", "style": "OSD_24_bold", "align": "left", "box": 20, "cap_height": 7.5,
                      "value": { "session": "time_remaining_str" } }
                ] }
            ]
        }))
        .unwrap();
        let plan = b.add_record(&rec, "t", &sprites, Mode::Live).unwrap();
        assert_eq!(plan.textures.len(), 2);
        assert_eq!(plan.colours[0].default, Some([0, 0, 0, 0]));
        let preview = b
            .add_record(&rec, "t", &sprites, Mode::Preview(&Default::default()))
            .unwrap();
        assert!(preview.textures.is_empty());
        let scene_bytes = b.finish(None).unwrap();
        let scene = Pkg::parse(&scene_bytes).unwrap();
        let root = idf::scenes(&scene).unwrap()[0].o().unwrap().at("scene").unwrap();
        let lg = idf::find(root, "t_lg").unwrap();
        assert_eq!(idf::param_str(lg, "fileRef"), Some(TEAM_LOGO));
        assert_eq!(idf::xref(lg), Some(idf::scene_id(TEAM_LOGO)));
        let s = floats(lg, "scale").unwrap();
        assert!(
            (s[0] - K * 14.1 / 0.24).abs() < 1e-6 && (s[0] - s[1]).abs() < 1e-9,
            "{s:?}"
        );
        assert!(idf::find(root, "t_pv_lg").is_none());
        assert_eq!(
            idf::param_str(idf::find(root, "t_logo").unwrap(), "fileRef"),
            Some(CATEGORY_LOGO)
        );

        let template_bytes = read(&stock, dials::TEMPLATE);
        let stock_tpl = Pkg::parse(&template_bytes).unwrap();
        let mut tpl = Pkg::parse(&template_bytes).unwrap();
        let tags = StockTags::race(&stock_tpl.root);
        template::attach(&mut tpl.root, &stock_tpl.root, &tags, f.race_parent, &plan).unwrap();
        let at = node::find_path(&tpl.root, &format!("{}.{}", f.race_parent, plan.ctl_name())).unwrap();
        let ctl = node::node_at(&tpl.root, &at).unwrap();
        let kid = |name: &str| {
            ctl.slot(node::CHILDREN)
                .unwrap()
                .iter()
                .find(|k| k.is_node() && k.name() == name)
                .unwrap_or_else(|| panic!("no node {name}"))
                .clone()
        };
        let bind = |n: &Node| {
            let mut out = Vec::new();
            node::bindings(n, &[], &mut out);
            out
        };
        let badge = badge_tag(&stock_tpl.root).unwrap();
        let x_lg = kid("x_lg");
        assert_eq!(layout_names(&x_lg), [["t_lg", "team_logo"]]);
        let bl = bind(&x_lg);
        assert_eq!(
            (bl[0].db.as_str(), bl[0].tag),
            ("#view_model_data.hud_tt_r03_badge", Some(badge))
        );
        let x_logo = kid("x_logo");
        assert_eq!(layout_names(&x_logo), [["t_logo", "logo_icon"]]);
        let bl = bind(&x_logo);
        assert_eq!(
            (bl[0].db.as_str(), bl[0].tag),
            (HUD_SERIES, Some(series_tag(&stock_tpl.root).unwrap()))
        );
        let dflt = x_logo
            .props()
            .unwrap()
            .get(TEXTURE_PROP)
            .unwrap()
            .as_obj()
            .unwrap();
        assert_eq!(
            dflt.get("value").unwrap().as_val().unwrap().as_str(),
            Some(SERIES_DEFAULT)
        );
        let c_bar = kid("c_bar");
        let bl = bind(&c_bar);
        assert_eq!(
            (bl[0].db.as_str(), bl[0].tag),
            ("#session.cars.car3.nub_colour", Some(*b"rgba"))
        );
        let v = c_bar
            .props()
            .unwrap()
            .get("colourOverride")
            .unwrap()
            .as_obj()
            .unwrap();
        assert!(matches!(&v.get("value").unwrap().as_val().unwrap().value, Value::Raw(b) if b == &[0u8; 4]));
        let bl = bind(&kid("t_code"));
        assert_eq!(
            (bl[0].db.as_str(), bl[0].tag),
            ("#session.cars.car3.driver.name_code", Some(*b"stri"))
        );
        assert_eq!(bind(&kid("t_laps"))[0].tag, Some(*b"si32"));
        assert_eq!(bind(&kid("t_clock"))[0].db, dials::CLOCK_STR);
        assert_eq!(kid("clock_fmt").type_code(), "UITF");
    }
}
