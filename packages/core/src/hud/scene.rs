//! Growing a dial scene (`.idf`) with the elements of `hud.element` records, and the plan of
//! template nodes, clips and preview texts that go with them.
//!
//! The element templates, the placement maths, the clip keys and the node patterns follow Project
//! Halo's `halo_native.py` (`SceneBuilder`, `_halo`, `Geo.rpm`); Uses work from Project Halo by
//! nyannoying (https://github.com/nyannoying1337/f1-25-project-halo). The scene facts they encode
//! (templates to clone, text and mask params, the 0.672 em cap height) are measurements of the game.

use std::collections::{BTreeMap, BTreeSet};

use md5::{Digest, Md5};
use sha1::Sha1;
use swofty_formats::animclip::Channel;
use swofty_formats::pkg::{Node, Pkg, Val, Value};

use super::dials::{self, DialFacts};
use super::geom::{self, K};
use super::model::{self, Bind, Decision, Element, HudElement, Kind};
use super::template::{self, Resolved, Usage};
use super::textures::Names;
use super::units;
use crate::ui::idf;
use crate::ui::node::{NodeExt, ObjExt};
use crate::{Error, Result};

fn err(msg: impl std::fmt::Display) -> Error {
    Error::Other(format!("hud: {msg}"))
}

#[derive(Debug, Clone)]
pub struct SpriteInfo {
    pub bounds: [f64; 4],
    pub names: Names,
    pub tinted: Option<Names>,
}

pub type Sprites = BTreeMap<String, SpriteInfo>;

pub enum Mode<'a> {
    Live,
    Preview(&'a BTreeMap<String, serde_json::Value>),
}

#[derive(Debug, Clone)]
pub struct VisItem {
    pub name: String,
    pub element: String,
    pub db: Option<String>,
    pub negate: bool,
    pub default: bool,
}

#[derive(Debug, Clone)]
pub struct ColourItem {
    pub name: String,
    pub element: String,
    pub db: String,
    pub default: Option<[u8; 4]>,
}

#[derive(Debug, Clone)]
pub struct TextItem {
    pub name: String,
    pub element: String,
    pub tag: [u8; 4],
    pub db: Option<String>,
    pub literal: String,
    pub localise: bool,
    pub visible: Option<Resolved>,
}

/// The stock scenes the stacker and centre elements are cloned from, in `flow_race.erp`.
pub const STACKED: &str =
    "eaid://ui_package/art/osd/components/driver_position_table/driver_position_table_stacked.idf";
pub const LAP_COUNTER: &str = "eaid://ui_package/art/osd/components/lap_counter/lap_counter.idf";
pub const STACKER_DONOR: &str = "full_stacker";
pub const CENTRE_DONOR: &str = "centre_lap_value";

#[derive(Debug, Clone)]
pub struct FracItem {
    pub name: String,
    pub element: String,
    pub db: String,
}

#[derive(Debug, Clone)]
pub struct ShowItem {
    pub name: String,
    pub element: String,
    pub default: bool,
}

/// A fixed colour on one element (`colour` on a text or sprite).
#[derive(Debug, Clone)]
pub struct StaticColour {
    pub name: String,
    pub element: String,
    pub colour: [u8; 4],
}

/// The record's `clock`, resolved: the output leaf path and the start bool.
#[derive(Debug, Clone)]
pub struct ClockSpec {
    pub output: Option<String>,
    pub start: Option<Resolved>,
}

#[derive(Debug, Clone)]
pub struct ClipSpec {
    pub node: String,
    pub channels: Vec<Channel>,
}

fn clip_channel(mut c: Channel) -> Channel {
    c.max_time = geom::ANIM_T as f32;
    c
}

/// What one record put in a scene, for the template, the clips and the editor preview.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub pfx: String,
    pub root: String,
    pub preview: bool,
    pub vis: Vec<VisItem>,
    pub colours: Vec<ColourItem>,
    pub textures: Vec<units::TexItem>,
    pub texts: Vec<TextItem>,
    pub fractions: Vec<FracItem>,
    pub shows: Vec<ShowItem>,
    pub decisions: Vec<Decision>,
    pub clips: Vec<ClipSpec>,
    pub motions: Vec<super::motion::MotionItem>,
    pub pv_texts: Vec<(String, String)>,
    pub known: Vec<(String, [u8; 4])>,
    pub statics: Vec<StaticColour>,
    pub clock: Option<ClockSpec>,
    pub root_hidden: bool,
    pub root_y: f64,
    pub footprint: [f64; 4],
    switched: BTreeMap<String, String>,
}

impl Plan {
    pub fn ctl_name(&self) -> String {
        if self.preview {
            format!("{}_pv_ctl", self.pfx)
        } else {
            format!("{}_ctl", self.pfx)
        }
    }

    pub fn switched_node(&self, id: &str) -> Result<String> {
        self.switched
            .get(id)
            .cloned()
            .ok_or_else(|| err(format!("{id} is not an element a decision shows")))
    }

    pub fn anim_names(&self) -> Vec<String> {
        self.clips.iter().map(|c| c.node.clone()).collect()
    }
}

struct Templates {
    quad: Node,
    text: Node,
    null: Node,
    window: Node,
    mask: Node,
    mesh: (f64, f64),
    stacker: Option<Node>,
    centre: Option<Node>,
}

pub struct Builder {
    pub facts: &'static DialFacts,
    pub pkg: Pkg,
    t: Templates,
    /// The stock units a `unit` element may instance (`units::load`); empty until loaded.
    pub units: units::Units,
    pub names: BTreeSet<String>,
    stock_ids: BTreeMap<String, u32>,
    stock_names: BTreeSet<String>,
}

fn scene_root(p: &Pkg) -> Result<&Node> {
    idf::scenes(p)?
        .first()
        .ok_or_else(|| err("the file has no scene"))?
        .o()?
        .at("scene")
}

fn scene_root_mut(p: &mut Pkg) -> Result<&mut Node> {
    idf::scenes_mut(p)?
        .first_mut()
        .ok_or_else(|| err("the file has no scene"))?
        .o_mut()?
        .at_mut("scene")
}

fn vec3_param(e: &Node, name: &str) -> Option<[f64; 3]> {
    match &idf::param(e, name)?.value {
        Value::Raw(b) if b.len() >= 12 => {
            let f = |i: usize| f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().expect("4 bytes")) as f64;
            Some([f(0), f(1), f(2)])
        }
        _ => None,
    }
}

fn template_of(bytes: &[u8], name: &str) -> Result<Node> {
    let p = Pkg::parse(bytes)?;
    Ok(idf::expect(scene_root(&p)?, name)?.clone())
}

pub fn guid(scene: &str, name: &str) -> String {
    let mut h = Sha1::new();
    h.update([
        0x6b, 0xa7, 0xb8, 0x11, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
    ]);
    h.update(format!("swofty.hud/{scene}/{name}").as_bytes());
    let d = h.finalize();
    let mut b: [u8; 16] = d[..16].try_into().expect("16 bytes");
    b[6] = (b[6] & 0x0f) | 0x50;
    b[8] = (b[8] & 0x3f) | 0x80;
    let x = |r: std::ops::Range<usize>| b[r].iter().map(|v| format!("{v:02x}")).collect::<String>();
    format!("{}-{}-{}-{}-{}", x(0..4), x(4..6), x(6..8), x(8..10), x(10..16))
}

impl Builder {
    pub fn new(
        facts: &'static DialFacts,
        scene: &[u8],
        mods: &[u8],
        bar: &[u8],
        text_from: Option<&[u8]>,
    ) -> Result<Self> {
        let pkg = Pkg::parse(scene)?;
        let root = scene_root(&pkg)?;
        let entry = &idf::scenes(&pkg)?[0];
        if idf::scene_name(entry) != Some(idf::scene_id(facts.scene)) {
            return Err(err(format!("{} is not the scene {}", facts.scene, facts.scene)));
        }
        let quad = idf::expect(root, facts.quad)?.clone();
        let text = match text_from {
            Some(b) => template_of(b, "speedo_number")?,
            None => idf::expect(root, "speedo_number")?.clone(),
        };
        let null = template_of(mods, "mod_1_icon_fill_mask_anim")?;
        let window = template_of(mods, "mod_1_icon_fill_mask")?;
        let mask = template_of(bar, "ers_battery_fill_mask")?;
        let mesh_name = format!("{}/{}?context=render", facts.scene, facts.quad);
        let g = quad
            .o()?
            .at("geoms")?
            .a()?
            .first()
            .ok_or_else(|| err("the quad has no geoms"))?
            .o()?;
        if g.str_at("resourceid")? != mesh_name {
            return Err(err(format!("{}: the quad's mesh is not {mesh_name}", facts.quad)));
        }
        let digest = Md5::digest(mesh_name.as_bytes());
        let asset = match &g.at("asset")?.v()?.value {
            Value::Raw(b) if b.len() >= 8 => u64::from_le_bytes(b[..8].try_into().expect("8 bytes")),
            _ => 0,
        };
        let want = u64::from_be_bytes(digest[..8].try_into().expect("8 bytes"));
        if asset != want {
            return Err(err(format!(
                "{}: the quad's asset is not the hash of its mesh",
                facts.quad
            )));
        }
        let lo = vec3_param(&quad, "minExtents").ok_or_else(|| err("the quad has no minExtents"))?;
        let hi = vec3_param(&quad, "maxExtents").ok_or_else(|| err("the quad has no maxExtents"))?;
        let piv = vec3_param(&quad, "pivotTrans").ok_or_else(|| err("the quad has no pivotTrans"))?;
        let centred = (lo[0] + hi[0]).abs() < 1e-4
            && (lo[2] + hi[2]).abs() < 1e-4
            && piv[0].abs() < 1e-4
            && piv[2].abs() < 1e-4;
        if !centred {
            return Err(err(format!(
                "{}: the quad's mesh is not centred on its pivot",
                facts.quad
            )));
        }
        let mesh = (hi[0] - lo[0], hi[2] - lo[2]);
        if let Some((w, h)) = facts.quad_size
            && ((mesh.0 - w).abs() > 1e-4 || (mesh.1 - h).abs() > 1e-4)
        {
            return Err(err(format!(
                "{}: the quad is {mesh:?}, not {w} x {h}",
                facts.quad
            )));
        }
        let stock_ids = idf::id_map(root);
        let stock_names = idf::elements(root)
            .into_iter()
            .map(|(_, e)| idf::elem_name(e).to_owned())
            .collect();
        Ok(Self {
            facts,
            pkg,
            t: Templates {
                quad,
                text,
                null,
                window,
                mask,
                mesh,
                stacker: None,
                centre: None,
            },
            units: units::Units::new(),
            names: BTreeSet::new(),
            stock_ids,
            stock_names,
        })
    }

    /// The stacker and centre donors: `full_stacker` of the stacked standings table and
    /// `centre_lap_value` of the lap counter, both with the facts the composer relies on.
    pub fn with_donors(&mut self, stacked: &[u8], lap_counter: &[u8]) -> Result<()> {
        let stacker = template_of(stacked, STACKER_DONOR)?;
        if idf::param_str(&stacker, "stackerType") != Some("t") {
            return Err(err(format!(
                "{STACKER_DONOR} of {STACKED} is not a top-down stacker"
            )));
        }
        let centre = template_of(lap_counter, CENTRE_DONOR)?;
        let centre_x = idf::param(&centre, "centreX")
            .is_some_and(|v| matches!(&v.value, Value::Raw(b) if b.first() == Some(&1)));
        if !centre_x {
            return Err(err(format!(
                "{CENTRE_DONOR} of {LAP_COUNTER} does not centre on x"
            )));
        }
        self.t.stacker = Some(stacker);
        self.t.centre = Some(centre);
        Ok(())
    }

    /// A builder over a stock scene that is no dial: its templates (text, null, window, mask) come from
    /// the dial scene `dial_scene`, so only text and group elements make sense in it.
    pub fn for_scene(
        facts: &'static DialFacts,
        scene: &[u8],
        dial_scene: &[u8],
        mods: &[u8],
        bar: &[u8],
    ) -> Result<Self> {
        let pkg = Pkg::parse(scene)?;
        let root = scene_root(&pkg)?;
        let entry = &idf::scenes(&pkg)?[0];
        if idf::scene_name(entry) != Some(idf::scene_id(facts.scene)) {
            return Err(err(format!("{} is not the scene {}", facts.scene, facts.scene)));
        }
        let dials = Pkg::parse(dial_scene)?;
        let dial_root = scene_root(&dials)?;
        let stock_ids = idf::id_map(root);
        let stock_names = idf::elements(root)
            .into_iter()
            .map(|(_, e)| idf::elem_name(e).to_owned())
            .collect();
        Ok(Self {
            facts,
            t: Templates {
                quad: idf::expect(dial_root, "central_bg_fill")?.clone(),
                text: idf::expect(dial_root, "speedo_number")?.clone(),
                null: template_of(mods, "mod_1_icon_fill_mask_anim")?,
                window: template_of(mods, "mod_1_icon_fill_mask")?,
                mask: template_of(bar, "ers_battery_fill_mask")?,
                mesh: (1.0, 1.0),
                stacker: None,
                centre: None,
            },
            pkg,
            units: units::Units::new(),
            names: BTreeSet::new(),
            stock_ids,
            stock_names,
        })
    }

    /// Adds one record's elements under the scene's root element, in HUD units from the scene's origin,
    /// and returns its plan.
    fn clock_of(rec: &HudElement) -> Result<Option<ClockSpec>> {
        let Some(c) = &rec.clock else { return Ok(None) };
        let output = match &c.output {
            Some(o) => {
                let r = template::resolve(&Bind::leaf(o), Usage::Text, "clock output")?;
                Some(r.db)
            }
            None => None,
        };
        let start = match &c.start {
            Some(b) => Some(template::resolve(b, Usage::Visible, "clock start")?),
            None => None,
        };
        Ok(Some(ClockSpec { output, start }))
    }

    pub fn add_scene_record(&mut self, rec: &HudElement, pfx: &str, sprites: &Sprites) -> Result<Plan> {
        self.add_scene_record_in(rec, pfx, sprites, Mode::Live)
    }

    /// [`Self::add_scene_record`] in a mode: in preview mode the root is `<pfx>_pv`, shown, and
    /// holds `editor.rows` copies of a repeated group and only the elements the preview state lights.
    pub fn add_scene_record_in(
        &mut self,
        rec: &HudElement,
        pfx: &str,
        sprites: &Sprites,
        mode: Mode,
    ) -> Result<Plan> {
        let preview = matches!(mode, Mode::Preview(_));
        let root_name = if preview {
            format!("{pfx}_pv")
        } else {
            pfx.to_owned()
        };
        let mut plan = Plan {
            pfx: pfx.to_owned(),
            root: root_name.clone(),
            preview,
            clock: Self::clock_of(rec)?,
            ..Plan::default()
        };
        let mut root = self.null(&root_name, [0.0; 3], None, false, false)?;
        let rows = if preview {
            rec.editor.as_ref().and_then(|e| e.rows)
        } else {
            None
        };
        let mut cx = Cx {
            names: format!("{root_name}_"),
            sprites,
            mode,
            switched: switched_by(&rec.decisions),
            plan: &mut plan,
        };
        cx.plan.decisions = rec.decisions.clone();
        let mut kids = Vec::new();
        for e in &model::expand(&rec.elements, rows)? {
            if let Some(n) = self.element(&mut cx, e)? {
                kids.push(n);
            }
        }
        idf::children_mut(&mut root)?.extend(kids);
        idf::children_mut(scene_root_mut(&mut self.pkg)?)?.push(root);
        Ok(plan)
    }

    pub fn bottom(&self) -> Result<f64> {
        let root = scene_root(&self.pkg)?;
        let br = idf::expect(root, "br_customisation")?;
        idf::position(br)
            .map(|p| p[1])
            .ok_or_else(|| err("br_customisation has no position"))
    }

    pub fn existing(&self, name: &str) -> bool {
        self.stock_names.contains(name) || self.names.contains(name)
    }

    fn base(
        &mut self,
        tmpl: &Node,
        name: &str,
        pos: [f64; 3],
        rot: [f64; 4],
        scale: [f64; 3],
    ) -> Result<Node> {
        if self.existing(name) {
            return Err(err(format!(
                "the scene {} already has an element {name}",
                self.facts.scene
            )));
        }
        self.names.insert(name.to_owned());
        let mut o = idf::detached(tmpl);
        idf::rename(&mut o, name)?;
        idf::set_param_like(&mut o, "guid", Val::str(guid(self.facts.scene, name)))?;
        idf::set_transform(&mut o, pos, rot, scale)?;
        idf::clear_list(&mut o, "children")?;
        idf::clear_list(&mut o, "anims")?;
        Ok(o)
    }

    fn null(
        &mut self,
        name: &str,
        pos: [f64; 3],
        rot_deg: Option<f64>,
        anim: bool,
        hidden: bool,
    ) -> Result<Node> {
        let rot = rot_deg.map_or(geom::QUAT_IDENTITY, geom::quat_z);
        let tmpl = self.t.null.clone();
        let mut o = self.base(&tmpl, name, pos, rot, [1.0; 3])?;
        if anim {
            idf::set_anim(
                &mut o,
                &tmpl,
                &format!("{}/{name}?context=animation", self.facts.scene),
            )?;
        }
        if hidden {
            idf::set_param_like(&mut o, "isHidden", Val::bool(true))?;
        }
        Ok(o)
    }

    fn scaled_null(&mut self, name: &str, pos: [f64; 3], rot_deg: f64, scale: f64) -> Result<Node> {
        let tmpl = self.t.null.clone();
        self.base(&tmpl, name, pos, geom::quat_z(rot_deg), [scale, scale, 1.0])
    }

    fn quad(&mut self, name: &str, emb: &str, pos: [f64; 3], w: f64, h: f64, layer: u32) -> Result<Node> {
        let tmpl = self.t.quad.clone();
        let (mw, mh) = self.t.mesh;
        let mut o = self.base(&tmpl, name, pos, geom::QUAT_QUAD, [w / mw, 1.0, h / mh])?;
        o.o_mut()?
            .at_mut("geoms")?
            .a_mut()?
            .first_mut()
            .ok_or_else(|| err("the quad has no geoms"))?
            .o_mut()?
            .set("materialid", Node::Val(Val::str(emb)));
        idf::set_param_like(&mut o, "eLayer.layer", idf::si32(layer as i32))?;
        Ok(o)
    }

    fn window(&mut self, name: &str, pos: [f64; 3], w: f64, l: f64) -> Result<Node> {
        let tmpl = self.t.window.clone();
        let mut o = self.base(&tmpl, name, pos, geom::QUAT_IDENTITY, [1.0; 3])?;
        idf::set_param_like(&mut o, "width", idf::fp32(w))?;
        idf::set_param_like(&mut o, "length", idf::fp32(l))?;
        idf::set_param_like(&mut o, "minExtents", idf::vec3([-w / 2.0, -l / 2.0, -0.0, 0.0]))?;
        idf::set_param_like(&mut o, "maxExtents", idf::vec3([w / 2.0, l / 2.0, -0.0, 0.0]))?;
        Ok(o)
    }

    fn mask(&mut self, name: &str, pos: [f64; 3], w: f64, h: f64, tex: &str) -> Result<Node> {
        let tmpl = self.t.mask.clone();
        let mut o = self.base(&tmpl, name, pos, geom::QUAT_IDENTITY, [1.0; 3])?;
        idf::set_param_like(&mut o, "width", idf::fp32(w))?;
        idf::set_param_like(&mut o, "height", idf::fp32(h))?;
        idf::set_param_like(&mut o, "minExtents", idf::vec3([-w / 2.0, -h / 2.0, 5e-05, 0.0]))?;
        idf::set_param_like(&mut o, "maxExtents", idf::vec3([w / 2.0, h / 2.0, 5e-05, 0.0]))?;
        idf::set_param_like(
            &mut o,
            "eMaskTexture.maskHalfVec",
            idf::vec3([w / 2.0, h / 2.0, 0.0, 0.0]),
        )?;
        idf::set_param_like(&mut o, "eMaskTexture.maskTextureResource", Val::str(tex))?;
        Ok(o)
    }

    fn text(&mut self, name: &str, pos: [f64; 3], el: &Element, box_w: f64) -> Result<Node> {
        let tmpl = self.t.text.clone();
        let mut o = self.base(&tmpl, name, pos, geom::QUAT_IDENTITY, [1.0; 3])?;
        if let Some(Node::Arr(a)) = o.o_mut()?.get_mut("params") {
            a.items.retain(|p| {
                p.as_obj()
                    .and_then(|o| o.get("name"))
                    .and_then(Node::as_val)
                    .and_then(Val::as_str)
                    != Some("isHidden")
            });
        }
        idf::set_param_like(&mut o, "boxWidth", idf::fp32(box_w))?;
        idf::set_param_like(&mut o, "halign", Val::str(el.align.as_deref().unwrap_or("left")))?;
        idf::set_param_like(
            &mut o,
            "style",
            Val::str(el.style.as_deref().unwrap_or("OSD_24_bold")),
        )?;
        idf::set_param_like(&mut o, "wrapping", Val::str(el.wrap.as_deref().unwrap_or("none")))?;
        idf::set_param_like(
            &mut o,
            "textcase",
            Val::str(el.text_case.as_deref().unwrap_or("upper")),
        )?;
        idf::set_param_like(&mut o, "eLayer.layer", idf::si32(40))?;
        Ok(o)
    }

    fn stack(&mut self, name: &str, pos: [f64; 3], dir: &str, gap: f64) -> Result<Node> {
        let tmpl = self
            .t
            .stacker
            .clone()
            .ok_or_else(|| err(format!("{name}: a stack needs the stacker donor ({STACKED})")))?;
        let kind = match dir {
            "down" => "t",
            "up" => "b",
            "right" => "l",
            "left" => "r",
            other => return Err(err(format!("{name}: no stack dir {other}"))),
        };
        let mut o = self.base(&tmpl, name, pos, geom::QUAT_IDENTITY, [1.0; 3])?;
        idf::set_param_like(&mut o, "stackerType", Val::str(kind))?;
        idf::set_param_like(&mut o, "gap", idf::fp32(K * gap))?;
        Ok(o)
    }

    fn centre(&mut self, name: &str, pos: [f64; 3]) -> Result<Node> {
        let tmpl = self
            .t
            .centre
            .clone()
            .ok_or_else(|| err(format!("{name}: a centre needs the centre donor ({LAP_COUNTER})")))?;
        self.base(&tmpl, name, pos, geom::QUAT_IDENTITY, [1.0; 3])
    }

    pub fn customisation_mut(&mut self) -> Result<&mut Node> {
        let root = scene_root_mut(&mut self.pkg)?;
        idf::find_mut(root, dials::CUSTOMISATION)
            .ok_or_else(|| err(format!("{} has no {}", self.facts.scene, dials::CUSTOMISATION)))
    }

    /// Moves the stock parts off screen and shrinks them.
    pub fn park(&mut self, parts: &[String]) -> Result<()> {
        let bc = self.customisation_mut()?;
        for kid in idf::children_mut(bc)? {
            if parts.iter().any(|p| p == idf::elem_name(kid)) {
                let p = idf::position(kid).unwrap_or([0.0; 3]);
                let o = kid.o_mut()?;
                o.set("position", Node::Val(idf::vec3([p[0], p[1] - 60.0, p[2], 0.0])));
                o.set("scale", Node::Val(idf::vec3([1e-3, 1e-3, 1e-3, 0.0])));
            }
        }
        Ok(())
    }

    pub fn stock_parts_present(&mut self, parts: &[String]) -> Result<()> {
        let bc = self.customisation_mut()?;
        let have: BTreeSet<String> = idf::children_mut(bc)?
            .iter()
            .map(|k| idf::elem_name(k).to_owned())
            .collect();
        for p in parts {
            if !have.contains(p) {
                return Err(err(format!(
                    "the dial has no stock part {p} under {}",
                    dials::CUSTOMISATION
                )));
            }
        }
        Ok(())
    }

    /// Adds one record's root under the customisation element and returns its plan.
    pub fn add_record(&mut self, rec: &HudElement, pfx: &str, sprites: &Sprites, mode: Mode) -> Result<Plan> {
        let footprint = box_of(rec, sprites)?;
        let root_y = match rec.root_y {
            Some(y) => -K * y,
            None => self.bottom()? + K * footprint[3],
        };
        let preview = matches!(mode, Mode::Preview(_));
        let root_name = if preview {
            format!("{pfx}_pv")
        } else {
            pfx.to_owned()
        };
        let live_hidden = !preview && rec.dial != super::model::Dial::F126;
        let mut plan = Plan {
            pfx: pfx.to_owned(),
            root: root_name.clone(),
            preview,
            root_y,
            footprint,
            clock: Self::clock_of(rec)?,
            ..Plan::default()
        };
        let mut root = self.null(&root_name, [0.0, root_y, 0.0], None, false, live_hidden)?;
        let mut cx = Cx {
            names: format!("{root_name}_"),
            sprites,
            mode,
            switched: switched_by(&rec.decisions),
            plan: &mut plan,
        };
        cx.plan.decisions = rec.decisions.clone();
        let rows = if preview {
            rec.editor.as_ref().and_then(|e| e.rows)
        } else {
            None
        };
        let mut kids = Vec::new();
        for e in &model::expand(&rec.elements, rows)? {
            if let Some(n) = self.element(&mut cx, e)? {
                kids.push(n);
            }
        }
        idf::children_mut(&mut root)?.extend(kids);
        plan.root_hidden = live_hidden;
        self.customisation_mut()?
            .o_mut()?
            .at_mut("children")?
            .a_mut()?
            .push(root);
        Ok(plan)
    }

    fn lit(cx: &Cx, b: &Bind) -> Result<bool> {
        let Mode::Preview(state) = &cx.mode else {
            return Ok(true);
        };
        let key = preview_key(b);
        Ok(state.get(&key).is_some_and(truthy) != b.negate)
    }

    fn sprite_quad(
        &mut self,
        cx: &Cx,
        name: &str,
        sprite: &str,
        layer: u32,
        origin: [f64; 3],
        tinted: bool,
    ) -> Result<Node> {
        let s = cx
            .sprites
            .get(sprite)
            .ok_or_else(|| err(format!("{name}: no sprite {sprite}")))?;
        let n = if tinted {
            s.tinted.as_ref().unwrap_or(&s.names)
        } else {
            &s.names
        };
        let emb = if self.facts.quad == "central_bg_edge" {
            &n.emb_edge
        } else {
            &n.emb
        };
        let [x0, y0, x1, y1] = s.bounds;
        let c = geom::scene_point((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        self.quad(
            name,
            emb,
            [c[0] - origin[0], c[1] - origin[1], -origin[2]],
            K * (x1 - x0),
            K * (y1 - y0),
            layer,
        )
    }

    fn element(&mut self, cx: &mut Cx, e: &Element) -> Result<Option<Node>> {
        let kind = e
            .kind
            .ok_or_else(|| err(format!("element {} has no kind", e.id)))?;
        let name = format!("{}{}", cx.names, e.id);
        let switched = cx.switched.contains_key(&e.id);
        if switched && e.visible.is_some() {
            return Err(err(format!(
                "element {} is shown by a decision and has its own visible",
                e.id
            )));
        }
        if let Mode::Preview(_) = cx.mode {
            if switched && cx.switched.get(&e.id) != Some(&true) {
                return Ok(None);
            }
            if let Some(b) = &e.visible
                && !Self::lit(cx, b)?
            {
                return Ok(None);
            }
        }
        let preview = cx.plan.preview;
        let layer = e.layer.unwrap_or(22);
        let at = e.at.unwrap_or([0.0, 0.0]);
        let node = match kind {
            Kind::Sprite => {
                let sprite = e.sprite.clone().unwrap_or_default();
                let tinted = preview && e.tint.is_some();
                let mut q = self.sprite_quad(cx, &name, &sprite, layer, [0.0; 3], tinted)?;
                let off = geom::scene_point(at[0], at[1]);
                if at != [0.0, 0.0] {
                    let p = idf::position(&q).unwrap_or([0.0; 3]);
                    q.o_mut()?.set(
                        "position",
                        Node::Val(idf::vec3([p[0] + off[0], p[1] + off[1], p[2], 0.0])),
                    );
                }
                if let Some(deg) = e.rot.filter(|d| *d != 0.0) {
                    let p = idf::position(&q).unwrap_or([0.0; 3]);
                    let mut holder = self.null(&format!("{name}_n"), p, Some(-deg), false, false)?;
                    q.o_mut()?.set("position", Node::Val(idf::vec3([0.0; 4])));
                    idf::children_mut(&mut holder)?.push(q);
                    return self.finish_leaf(cx, e, &name, holder, switched);
                }
                q
            }
            Kind::Group | Kind::Stack | Kind::Centre => {
                let pos = geom::scene_point(at[0], at[1]);
                let mut g = match kind {
                    Kind::Stack => self.stack(
                        &name,
                        pos,
                        e.dir.as_deref().unwrap_or("down"),
                        e.gap.unwrap_or(0.0),
                    )?,
                    Kind::Centre => self.centre(&name, pos)?,
                    _ => self.null(&name, pos, None, false, e.hidden.unwrap_or(false) && !preview)?,
                };
                let mut holder = match (&e.motion, preview) {
                    (Some(m), false) => {
                        let hn = super::motion::holder_name(&name, m);
                        let clip = m.track()? != super::motion::Track::Fade;
                        let h = self.null(&hn, [0.0; 3], None, clip, false)?;
                        super::motion::plan(cx.plan, e, m, &hn)?;
                        Some(h)
                    }
                    _ => None,
                };
                for c in &e.children {
                    if let Some(n) = self.element(cx, c)? {
                        idf::children_mut(holder.as_mut().unwrap_or(&mut g))?.push(n);
                    }
                }
                if let Some(h) = holder {
                    idf::children_mut(&mut g)?.push(h);
                }
                g
            }
            Kind::Text => return self.text_element(cx, e, &name),
            Kind::Levels => {
                let sprite = e.sprite.clone().unwrap_or_default();
                let s = cx
                    .sprites
                    .get(&sprite)
                    .ok_or_else(|| err(format!("{name}: no sprite {sprite}")))?;
                let [x0, y0, x1, y1] = s.bounds;
                let count = e.count.unwrap_or(1);
                let (from, to) = (e.from.unwrap_or(y1), e.to.unwrap_or(y0));
                let show = e.show.clone().unwrap_or_default();
                let mut g = self.null(&name, [0.0; 3], None, false, false)?;
                for i in 1..=count {
                    let leaf = geom::leaf_name(&show, i);
                    if preview && !Self::lit(cx, &Bind::leaf(&leaf))? {
                        continue;
                    }
                    let top = if i == count {
                        y0
                    } else {
                        from + (to - from) * i as f64 / count as f64
                    };
                    let wc = {
                        let c = geom::scene_point((x0 + x1) / 2.0, (top + y1) / 2.0);
                        [c[0], c[1], -0.12]
                    };
                    let wname = format!("{name}_w{i:02}");
                    let mut w = self.window(&wname, wc, K * (x1 - x0), K * (y1 - top))?;
                    let q = self.sprite_quad(cx, &format!("{name}_q{i:02}"), &sprite, layer, wc, false)?;
                    idf::children_mut(&mut w)?.push(q);
                    idf::children_mut(&mut g)?.push(w);
                    if !preview {
                        let r = template::resolve(&Bind::leaf(&leaf), Usage::Visible, &name)?;
                        cx.plan.vis.push(VisItem {
                            name: format!("v_{}{i:02}", e.id),
                            element: wname,
                            db: Some(r.db),
                            negate: false,
                            default: false,
                        });
                    }
                }
                g
            }
            Kind::Slats => {
                let mut g = self.null(&name, [0.0; 3], None, false, false)?;
                let show = e.show.clone().unwrap_or_default();
                for (i, sprite) in e.sprites.iter().enumerate() {
                    let leaf = geom::leaf_name(&show, i as u32 + 1);
                    if preview && !Self::lit(cx, &Bind::leaf(&leaf))? {
                        continue;
                    }
                    let qname = format!("{name}_{:02}", i + 1);
                    let q = self.sprite_quad(cx, &qname, sprite, layer, [0.0; 3], false)?;
                    idf::children_mut(&mut g)?.push(q);
                    if !preview {
                        let r = template::resolve(&Bind::leaf(&leaf), Usage::Visible, &name)?;
                        cx.plan.vis.push(VisItem {
                            name: format!("v_{}_{:02}", e.id, i + 1),
                            element: qname,
                            db: Some(r.db),
                            negate: false,
                            default: false,
                        });
                    }
                }
                g
            }
            Kind::Gauge => self.gauge(cx, e, &name)?,
            Kind::Sweep => self.sweep(cx, e, &name)?,
            Kind::Unit => match units::build(self, cx, e, &name)? {
                Some(n) => n,
                None => return Ok(None),
            },
        };
        self.finish_leaf(cx, e, &name, node, switched)
    }

    fn finish_leaf(
        &mut self,
        cx: &mut Cx,
        e: &Element,
        name: &str,
        node: Node,
        switched: bool,
    ) -> Result<Option<Node>> {
        let outer = idf::elem_name(&node).to_owned();
        if cx.plan.preview {
            return Ok(Some(node));
        }
        if switched {
            let vn = format!("v_{}", e.id);
            cx.plan.shows.push(ShowItem {
                name: vn.clone(),
                element: outer.clone(),
                default: cx.switched.get(&e.id) == Some(&true),
            });
            cx.plan.switched.insert(e.id.clone(), vn);
        } else if let Some(b) = &e.visible {
            let r = template::resolve(b, Usage::Visible, name)?;
            cx.plan.known.push((r.db.clone(), r.tag));
            cx.plan.vis.push(VisItem {
                name: format!("v_{}", e.id),
                element: outer.clone(),
                db: Some(r.db),
                negate: r.negate,
                default: r.default,
            });
        } else if e.hidden == Some(true) {
            // isHidden alone does not hide in game: a static visible = false node does
            cx.plan.shows.push(ShowItem {
                name: format!("v_{}", e.id),
                element: outer.clone(),
                default: false,
            });
        }
        if let Some(c) = &e.colour {
            cx.plan.statics.push(StaticColour {
                name: format!("c_{}", e.id),
                element: outer.clone(),
                colour: super::textures::hex_rgba(c)?,
            });
        }
        if let Some(b) = &e.tint {
            let r = template::resolve(b, Usage::Colour, name)?;
            cx.plan.known.push((r.db.clone(), r.tag));
            let target = if e.kind == Some(Kind::Sweep) {
                format!("{name}_fill")
            } else {
                outer
            };
            cx.plan.colours.push(ColourItem {
                name: format!("c_{}", e.id),
                element: target,
                db: r.db,
                default: r.colour,
            });
        }
        Ok(Some(node))
    }

    fn text_element(&mut self, cx: &mut Cx, e: &Element, name: &str) -> Result<Option<Node>> {
        let style = e.style.as_deref().unwrap_or("OSD_24_bold");
        let sh = geom::style_height(style).ok_or_else(|| {
            err(format!(
                "text {}: no style {style}; known: {}",
                e.id,
                geom::STYLES
                    .iter()
                    .map(|(n, _)| *n)
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;
        let cap = e
            .cap_height
            .ok_or_else(|| err(format!("text {} needs cap_height", e.id)))?;
        let place = geom::text_place(cap, sh, e.width.unwrap_or(40.0));
        let at = e.at.unwrap_or([0.0, 0.0]);
        let p = geom::scene_point(at[0], at[1]);
        let mut holder = self.scaled_null(&format!("{name}_n"), p, -e.rot.unwrap_or(0.0), place.scale)?;
        let t = self.text(name, [0.0, place.origin_y, 0.0], e, place.box_width)?;
        idf::children_mut(&mut holder)?.push(t);
        let plan = &mut *cx.plan;
        let preview_value = if let Mode::Preview(state) = &cx.mode {
            match (&e.value, &e.literal, &e.literal_id) {
                (Some(b), _, _) => state.get(&preview_key(b)).map(show_value),
                (None, Some(l), _) => Some(l.clone()),
                (None, None, Some(id)) => Some(id.clone()),
                _ => None,
            }
        } else {
            None
        };
        if plan.preview {
            let Some(text) = preview_value else {
                return Ok(None);
            };
            plan.pv_texts.push((name.to_owned(), text));
            return Ok(Some(holder));
        }
        let (tag, db, literal, localise) = match (&e.value, &e.literal, &e.literal_id) {
            (Some(b), _, _) => {
                let r = template::resolve(b, Usage::Text, name)?;
                plan.known.push((r.db.clone(), r.tag));
                (r.tag, Some(r.db), String::new(), false)
            }
            (None, Some(l), _) => (*b"stri", None, l.clone(), false),
            (None, None, Some(id)) => (*b"stri", None, id.clone(), true),
            _ => return Err(err(format!("text {} has no value", e.id))),
        };
        let visible = match &e.visible {
            Some(b) => {
                let r = template::resolve(b, Usage::Visible, name)?;
                plan.known.push((r.db.clone(), r.tag));
                Some(r)
            }
            None => None,
        };
        plan.texts.push(TextItem {
            name: format!("t_{}", e.id),
            element: name.to_owned(),
            tag,
            db,
            literal,
            localise,
            visible,
        });
        if e.hidden == Some(true) && e.visible.is_none() {
            plan.shows.push(ShowItem {
                name: format!("v_{}", e.id),
                element: idf::elem_name(&holder).to_owned(),
                default: false,
            });
        }
        if let Some(c) = &e.colour {
            plan.statics.push(StaticColour {
                name: format!("c_{}", e.id),
                element: name.to_owned(),
                colour: super::textures::hex_rgba(c)?,
            });
        }
        Ok(Some(holder))
    }

    fn gauge(&mut self, cx: &mut Cx, e: &Element, name: &str) -> Result<Node> {
        let preview = cx.plan.preview;
        let sprite = e.sprite.clone().unwrap_or_default();
        let s = cx
            .sprites
            .get(&sprite)
            .ok_or_else(|| err(format!("{name}: no sprite {sprite}")))?;
        let [fx0, fy0, fx1, fy1] = s.bounds;
        let [ww, wh] = e.window.unwrap_or([fx1 - fx0 + 2.0, fy1 - fy0 + 1.0]);
        let path = e
            .path
            .ok_or_else(|| err(format!("gauge {} needs a path", e.id)))?;
        let keys: Vec<[f64; 3]> = path.iter().map(|p| geom::scene_point(p[0], p[1])).collect();
        let bind = e
            .value
            .clone()
            .ok_or_else(|| err(format!("gauge {} needs a value", e.id)))?;
        let value = match &cx.mode {
            Mode::Preview(state) => state
                .get(&preview_key(&bind))
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0),
            Mode::Live => 0.0,
        };
        let bp = if preview {
            let f = value.clamp(0.0, 1.0);
            [0, 1, 2].map(|i| keys[0][i] + (keys[1][i] - keys[0][i]) * f)
        } else {
            keys[0]
        };
        let (wn, cn) = if preview {
            (format!("zs_{name}_win"), format!("zs_{name}_ctr"))
        } else {
            (format!("zh_{name}_win"), format!("zh_{name}_ctr"))
        };
        let mut win_anim = self.null(&wn, bp, None, !preview, false)?;
        let win = self.window(&format!("{name}_win"), [0.0, 0.0, -0.12], K * ww, K * wh)?;
        let mut ctr = self.null(&cn, [-bp[0], -bp[1], 0.12], None, !preview, false)?;
        let q = self.sprite_quad(
            cx,
            &format!("{name}_fill"),
            &sprite,
            e.layer.unwrap_or(23),
            [0.0; 3],
            false,
        )?;
        idf::children_mut(&mut ctr)?.push(q);
        let mut win = win;
        idf::children_mut(&mut win)?.push(ctr);
        idf::children_mut(&mut win_anim)?.push(win);
        if !preview {
            let r = template::resolve(&bind, Usage::Fraction, name)?;
            cx.plan.known.push((r.db.clone(), r.tag));
            let times = vec![0.0, geom::ANIM_T as f32];
            let win_keys: Vec<[f32; 3]> = keys
                .iter()
                .map(|k| [k[0] as f32, k[1] as f32, k[2] as f32])
                .collect();
            let ctr_keys: Vec<[f32; 3]> = keys.iter().map(|k| [-k[0] as f32, -k[1] as f32, 0.12]).collect();
            for (node, ks) in [(wn.clone(), win_keys), (cn.clone(), ctr_keys)] {
                cx.plan.clips.push(ClipSpec {
                    node: node.clone(),
                    channels: vec![clip_channel(Channel::position(times.clone(), ks))],
                });
                cx.plan.fractions.push(FracItem {
                    name: format!("x_{}_{}", e.id, if node == wn { "win" } else { "ctr" }),
                    element: node,
                    db: r.db.clone(),
                });
            }
        }
        Ok(win_anim)
    }

    fn sweep(&mut self, cx: &mut Cx, e: &Element, name: &str) -> Result<Node> {
        let preview = cx.plan.preview;
        let mask_sprite = e.mask_sprite.clone().unwrap_or_default();
        let fill_sprite = e.fill_sprite.clone().unwrap_or_default();
        let ms = cx
            .sprites
            .get(&mask_sprite)
            .ok_or_else(|| err(format!("{name}: no sprite {mask_sprite}")))?;
        let [x0, y0, x1, y1] = ms.bounds;
        let tex = ms.names.tex.clone();
        let pivot = e.pivot.unwrap_or([0.0, 0.0]);
        let sw = geom::sweep(pivot, e.start.unwrap_or(0.0), &e.arc, ms.bounds);
        let mc = geom::scene_point((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let cs = geom::scene_point(pivot[0], pivot[1]);
        let bind = e
            .value
            .clone()
            .ok_or_else(|| err(format!("sweep {} needs a value", e.id)))?;
        let mut mask = self.mask(&format!("{name}_mask"), mc, K * (x1 - x0), K * (y1 - y0), &tex)?;
        let rel = [cs[0] - mc[0], cs[1] - mc[1], 0.0];
        let anim_name = if preview {
            format!("zs_{name}_rot")
        } else {
            format!("zh_{name}_rot")
        };
        let rot = if let Mode::Preview(state) = &cx.mode {
            let f = state
                .get(&preview_key(&bind))
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0);
            Some(angle_at(&sw.keys, f).to_degrees())
        } else {
            None
        };
        let mut anim = self.null(&anim_name, rel, rot, !preview, false)?;
        let mut frame = self.null(
            &format!("{name}_frame"),
            [0.0; 3],
            Some(sw.frame_deg),
            false,
            false,
        )?;
        let fe = cx
            .sprites
            .get(&fill_sprite)
            .ok_or_else(|| err(format!("{name}: no sprite {fill_sprite}")))?;
        let n = if preview && e.tint.is_some() {
            fe.tinted.as_ref().unwrap_or(&fe.names)
        } else {
            &fe.names
        };
        let emb = if self.facts.quad == "central_bg_edge" {
            n.emb_edge.clone()
        } else {
            n.emb.clone()
        };
        let quad = self.quad(
            &format!("{name}_fill"),
            &emb,
            [0.0, -sw.sigma * sw.reach / 2.0, 0.0],
            2.0 * sw.reach,
            sw.reach,
            e.layer.unwrap_or(22),
        )?;
        idf::children_mut(&mut frame)?.push(quad);
        idf::children_mut(&mut anim)?.push(frame);
        idf::children_mut(&mut mask)?.push(anim);
        if !preview {
            let r = template::resolve(&bind, Usage::Fraction, name)?;
            cx.plan.known.push((r.db.clone(), r.tag));
            let times: Vec<f32> = sw.keys.iter().map(|(f, _)| (*f * geom::ANIM_T) as f32).collect();
            let quats: Vec<[f32; 4]> = sw
                .keys
                .iter()
                .map(|(_, a)| geom::quat_z(a.to_degrees()).map(|v| v as f32))
                .collect();
            cx.plan.clips.push(ClipSpec {
                node: anim_name.clone(),
                channels: vec![
                    clip_channel(Channel::position(
                        vec![0.0],
                        vec![[rel[0] as f32, rel[1] as f32, rel[2] as f32]],
                    )),
                    clip_channel(Channel::rotation(times, quats)),
                    clip_channel(Channel::scale(vec![0.0], vec![[1.0; 3]])),
                ],
            });
            cx.plan.fractions.push(FracItem {
                name: format!("x_{}", e.id),
                element: anim_name,
                db: r.db,
            });
        }
        Ok(mask)
    }

    /// Writes the scene: the customisation box over `footprint` (scene units `[x0, y_top, x1,
    /// y_bottom]`), the ids and the lookup, after checking no stock id moved and no name repeats.
    pub fn finish(mut self, footprint: Option<[f64; 4]>) -> Result<Vec<u8>> {
        if let Some(f) = footprint {
            let root = scene_root_mut(&mut self.pkg)?;
            idf::set_box(root, [f[0], f[1], -0.0], [f[2], f[3], -0.0])?;
        }
        let root = scene_root_mut(&mut self.pkg)?;
        for kid in idf::children_mut(root)? {
            let n = idf::elem_name(kid).to_owned();
            idf::recompute_ids(kid, &n)?;
        }
        let entry = &mut idf::scenes_mut(&mut self.pkg)?[0];
        idf::relookup(entry)?;
        let root = scene_root(&self.pkg)?;
        let after = idf::id_map(root);
        let moved: Vec<&String> = self
            .stock_ids
            .iter()
            .filter(|(k, v)| after.get(*k) != Some(*v))
            .map(|(k, _)| k)
            .collect();
        if let Some(k) = moved.first() {
            return Err(err(format!(
                "{}: the stock element {k} changed its id ({} in all)",
                self.facts.scene,
                moved.len()
            )));
        }
        let all = idf::elements(root);
        let mut seen = BTreeSet::new();
        for (_, e) in &all {
            let id = e
                .as_obj()
                .and_then(|o| o.get("elementId"))
                .and_then(Node::as_val)
                .and_then(Val::as_u32);
            if let Some(id) = id
                && !seen.insert(id)
            {
                return Err(err(format!("{}: an element id is used twice", self.facts.scene)));
            }
        }
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for (_, e) in &all {
            *counts.entry(idf::elem_name(e)).or_default() += 1;
        }
        if let Some(d) = self
            .names
            .iter()
            .find(|n| counts.get(n.as_str()).copied().unwrap_or(0) > 1)
        {
            return Err(err(format!(
                "{}: the element name {d} is used twice",
                self.facts.scene
            )));
        }
        Ok(self.pkg.to_bytes()?)
    }
}

pub(crate) struct Cx<'a, 'p> {
    pub names: String,
    pub sprites: &'a Sprites,
    pub mode: Mode<'a>,
    /// The elements the decisions show, and whether each starts shown (its decision's `initial`).
    pub switched: BTreeMap<String, bool>,
    pub plan: &'p mut Plan,
}

/// The elements `decisions` switch, each with whether it starts shown.
pub fn switched_by(decisions: &[Decision]) -> BTreeMap<String, bool> {
    let mut out: BTreeMap<String, bool> = BTreeMap::new();
    for d in decisions {
        for id in d.targets() {
            out.entry(id.clone()).or_insert(false);
        }
        for id in d.initial_targets() {
            out.insert(id.clone(), true);
        }
    }
    out
}

pub fn preview_key(b: &Bind) -> String {
    if let Some(l) = b.view_model.as_ref().or(b.path.as_ref()) {
        return l.trim_start_matches('#').to_owned();
    }
    if let Some(g) = &b.game {
        return format!("game.{g}");
    }
    if let Some(s) = &b.session {
        return format!("session.{}", s.trim_start_matches("#session."));
    }
    format!(
        "session_car.{}.{}",
        b.session_car.as_deref().unwrap_or_default(),
        b.slot.unwrap_or(0)
    )
}

fn truthy(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Bool(b) => *b,
        serde_json::Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        serde_json::Value::String(s) => !s.is_empty(),
        serde_json::Value::Null => false,
        _ => true,
    }
}

fn show_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// The angle (radians) the sweep's keys give at fraction `f`, linearly interpolated.
pub fn angle_at(keys: &[(f64, f64)], f: f64) -> f64 {
    let f = f.clamp(0.0, 1.0);
    for w in keys.windows(2) {
        if f <= w[1].0 {
            let span = w[1].0 - w[0].0;
            return if span == 0.0 {
                w[1].1
            } else {
                w[0].1 + (w[1].1 - w[0].1) * (f - w[0].0) / span
            };
        }
    }
    keys.last().map_or(0.0, |k| k.1)
}

/// The record's footprint in HUD units: its `box`, the bounds of the sprite named base, or of
/// every sprite.
pub fn box_of(rec: &HudElement, sprites: &Sprites) -> Result<[f64; 4]> {
    if let Some(b) = rec.rect {
        return Ok(b);
    }
    if let Some(s) = sprites.get("base") {
        return Ok(s.bounds);
    }
    let mut it = sprites.values().map(|s| s.bounds);
    let first = it.next().ok_or_else(|| err("the record has no sprites"))?;
    Ok(it.fold(first, |a, b| {
        [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
    }))
}

/// A record's footprint as the selection box's corners: `[x0, y_top, x1, y_bottom]` in scene units.
pub fn scene_box(plan: &Plan) -> [f64; 4] {
    let f = plan.footprint;
    [K * f[0], plan.root_y - K * f[1], K * f[2], plan.root_y - K * f[3]]
}

pub fn union_box(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [a[0].min(b[0]), a[1].max(b[1]), a[2].max(b[2]), a[3].min(b[3])]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switched_elements_start_shown_by_their_decisions_initial_state() {
        let v = serde_json::json!([
            { "id": "drs", "type": "mvcd", "game": "drs_pit_state", "initial": "invalid",
              "states": { "invalid": ["idle"], "drs_live": ["live", "shared"] } },
            { "id": "mode", "type": "svcd", "game": "ers_mode", "equals": 2, "yes": ["hot"], "no": ["shared"] }
        ]);
        let decisions: Vec<Decision> = serde_json::from_value(v).unwrap();
        let s = switched_by(&decisions);
        assert_eq!(
            s.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>(),
            [("hot", false), ("idle", true), ("live", false), ("shared", false)]
        );
        let mut d = decisions;
        d[1].initial = Some("no".into());
        assert!(switched_by(&d)["shared"]);
        d[1].initial = Some("yes".into());
        assert!(!switched_by(&d)["shared"]);
        assert!(switched_by(&d)["hot"]);
    }
}
