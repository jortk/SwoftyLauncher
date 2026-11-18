//! The `hud.element` record (`packages/sdk/std/registries/hud.element/1.0.json`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

fn err(msg: impl std::fmt::Display) -> Error {
    Error::Other(format!("hud: {msg}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Dial {
    #[serde(rename = "f1_26")]
    F126,
    #[serde(rename = "f1_25")]
    F125,
    #[serde(rename = "f2")]
    F2,
}

impl Dial {
    pub const ALL: [Dial; 3] = [Dial::F126, Dial::F125, Dial::F2];

    pub fn key(self) -> &'static str {
        match self {
            Dial::F126 => "f1_26",
            Dial::F125 => "f1_25",
            Dial::F2 => "f2",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StockMode {
    Keep,
    Hide,
    Park,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Units {
    #[default]
    Hud,
    Tower,
}

impl Units {
    pub fn k(self) -> f64 {
        match self {
            Units::Hud => super::geom::K,
            Units::Tower => super::geom::KT,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sprite {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub png: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<[f64; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ppu: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<[f64; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ty {
    Bool,
    Text,
    I32,
    Si32,
    Ui32,
    F32,
}

/// A value a template node binds: the addon's own leaf, a value of the dial's own view model
/// (`game`), a `#session` path, one car of the session, or any other path of the game's view model
/// (`path`, with its tag declared by `ty`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bind {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_car: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot_var: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<Ty>,
    #[serde(default, rename = "not", skip_serializing_if = "std::ops::Not::not")]
    pub negate: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

impl Bind {
    pub fn leaf(name: &str) -> Self {
        Self {
            view_model: Some(name.to_owned()),
            ..Self::default()
        }
    }

    pub fn game(name: &str) -> Self {
        Self {
            game: Some(name.to_owned()),
            ..Self::default()
        }
    }

    pub fn check(&self) -> Result<()> {
        let n = [
            self.view_model.is_some(),
            self.path.is_some(),
            self.game.is_some(),
            self.session.is_some(),
            self.session_car.is_some(),
            self.screen.is_some(),
        ]
        .iter()
        .filter(|b| **b)
        .count();
        if n != 1 {
            return Err(err(
                "a binding names exactly one of view_model, path, game, session, session_car and screen",
            ));
        }
        if let Some(p) = &self.path {
            let body = p.trim_start_matches('#');
            if body.is_empty() || body.split('.').any(str::is_empty) || body.contains(char::is_whitespace) {
                return Err(err(format!(
                    "path {p} is a dotted view-model path, such as #session.number_of_laps"
                )));
            }
        }
        let slots = self.slot.is_some() as u8 + self.slot_var.is_some() as u8;
        if self.session_car.is_some() != (slots == 1) {
            return Err(err("session_car and one of slot and slot_var go together"));
        }
        Ok(())
    }

    pub fn default_bool(&self) -> bool {
        self.default
            .as_ref()
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(self.negate)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Sprite,
    Text,
    Group,
    Levels,
    Slats,
    Gauge,
    Sweep,
    Stack,
    Centre,
    Unit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repeat {
    pub count: u32,
    pub var: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Element {
    pub id: String,
    pub kind: Option<Kind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rot: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<Bind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tint: Option<Bind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(default, rename = "box", skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, rename = "case", skip_serializing_if = "Option::is_none")]
    pub text_case: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cap_height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Bind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub literal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub literal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Element>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeat: Option<Repeat>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<Bind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axis: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sprites: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<[[f64; 2]; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask_sprite: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_sprite: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pivot: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arc: Vec<[f64; 2]>,
    /// Variant `add` entries only: the existing element this one goes in front of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    /// Variant `add` entries only: the existing element this one goes behind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Variant `add` entries only: the existing element this one takes the place of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<super::motion::Motion>,
}

/// Where a variant's `add` entry goes: in front of, behind or in place of an existing element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Before,
    After,
    Replace,
}

impl Place {
    pub fn key(self) -> &'static str {
        match self {
            Place::Before => "before",
            Place::After => "after",
            Place::Replace => "replace",
        }
    }
}

impl Element {
    /// The one placement an `add` entry names, or none (it goes last at the top level).
    pub fn placement(&self) -> Result<Option<(Place, &str)>> {
        let set: Vec<(Place, &str)> = [
            (Place::Before, &self.before),
            (Place::After, &self.after),
            (Place::Replace, &self.replace),
        ]
        .into_iter()
        .filter_map(|(p, v)| v.as_deref().map(|id| (p, id)))
        .collect();
        match set[..] {
            [] => Ok(None),
            [one] => Ok(Some(one)),
            _ => Err(err(format!(
                "element {}: before, after and replace are one placement, not two",
                self.id
            ))),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub game: String,
    /// In place of `game`: any value of the game's view model, decided on by its own states (an MVCD on
    /// a bool has the states `0` and `1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<Bind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ty: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub states: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub yes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub no: Vec<String>,
    /// The state (or `yes` / `no`) whose elements start shown, before the value first fires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial: Option<String>,
}

impl Decision {
    /// Every element the decision shows or hides.
    pub fn targets(&self) -> impl Iterator<Item = &String> {
        self.yes
            .iter()
            .chain(&self.no)
            .chain(self.states.values().flatten())
    }

    /// The elements of the `initial` state: shown from the first frame, until the value fires.
    pub fn initial_targets(&self) -> impl Iterator<Item = &String> {
        let list: Option<&Vec<String>> = match (self.initial.as_deref(), self.kind.as_str()) {
            (Some("yes"), "svcd") => Some(&self.yes),
            (Some("no"), "svcd") => Some(&self.no),
            (Some(s), "mvcd") => self.states.get(s),
            _ => None,
        };
        list.into_iter().flatten()
    }

    fn check(&self, known: &std::collections::BTreeSet<&str>) -> Result<()> {
        let ok = matches!(self.kind.as_str(), "svcd" | "mvcd");
        if !ok {
            return Err(err(format!("decision {}: type is svcd or mvcd", self.id)));
        }
        if self.game.is_empty() == self.on.is_none() {
            return Err(err(format!(
                "decision {}: one of game (a value of the dial's table) and on (a path binding)",
                self.id
            )));
        }
        if let Some(b) = &self.on {
            b.check()?;
            if b.path.is_none() && b.view_model.is_none() {
                return Err(err(format!(
                    "decision {}: on is a path or view_model binding",
                    self.id
                )));
            }
            if self.kind == "mvcd" && self.states.is_empty() {
                return Err(err(format!(
                    "decision {}: an mvcd on a path lists its states (0 and 1 for a bool)",
                    self.id
                )));
            }
            if self.kind == "svcd" && self.ty.is_none() {
                return Err(err(format!(
                    "decision {}: an svcd on a path declares ty, the tag of the value",
                    self.id
                )));
            }
        }
        for id in self.targets() {
            if !known.contains(id.as_str()) {
                return Err(err(format!(
                    "decision {} shows {id}, which is no element",
                    self.id
                )));
            }
        }
        if self.kind == "svcd" && self.equals.is_none() {
            return Err(err(format!("decision {}: an svcd needs equals", self.id)));
        }
        if let Some(i) = &self.initial {
            let ok = match self.kind.as_str() {
                "svcd" => i == "yes" || i == "no",
                _ => self.states.contains_key(i),
            };
            if !ok {
                return Err(err(format!(
                    "decision {}: initial names one of its states ({})",
                    self.id,
                    if self.kind == "svcd" {
                        "yes or no".to_owned()
                    } else {
                        self.states.keys().cloned().collect::<Vec<_>>().join(", ")
                    }
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Editor {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub preview: BTreeMap<String, serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_colour: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_stock_in_editor: Option<bool>,
    #[serde(default, rename = "box", skip_serializing_if = "Option::is_none")]
    pub rect: Option<[f64; 4]>,
}

/// Elements of one stock scene hidden from one template node: statically, or while `while` is true.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StockNodes {
    pub template: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hide: Vec<String>,
    /// Elements a stock node already targets: each gets a new null around it in the scene (its name
    /// and order kept) and the wrapper is hidden instead.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wrap: Vec<String>,
    #[serde(default, rename = "while", skip_serializing_if = "Option::is_none")]
    pub while_: Option<Bind>,
}

/// One fill quad refilled from a sprite through its mesh slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fill {
    pub sprite: String,
    pub slot: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scale {
    pub element: String,
    pub by: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pivot_y: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, rename = "box", skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<[f64; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(default, rename = "case", skip_serializing_if = "Option::is_none")]
    pub text_case: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Param {
    pub element: String,
    pub name: String,
    pub value: serde_json::Value,
}

/// Edits of one stock scene in place: no element added, moved or renamed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Restyle {
    pub scene: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fill: BTreeMap<String, Fill>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub twins: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub template_parents: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hide: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scale: Vec<Scale>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub text: BTreeMap<String, TextStyle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<Param>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock_erp: Option<String>,
}

/// A record's `restyle`: a list of scenes, or a preset the core spells out (`banner`, with the fill sprite).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RestyleField {
    Preset(RestylePreset),
    List(Vec<Restyle>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestylePreset {
    pub preset: String,
    pub sprite: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Also {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show: Option<Bind>,
    /// Element ids hidden for good on this screen (a static `visible = false` inside the instance).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hide: Vec<String>,
}

/// The session clock a record's texts read through the stock formatter clone: the leaf it writes
/// and a bool leaf whose change re-formats it, besides the record's own trigger.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clock {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<Bind>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    pub dial: Dial,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch: Option<Bind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add: Vec<Element>,
    /// Decisions built for this variant's dial only; they may show the variant's own `add` elements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<Decision>,
    /// This variant's root y (record units, `v` down from the dial centre), over the record's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_y: Option<f64>,
    /// This variant's stock mode, over the record's: with a `switch`, `hide` shows the variant
    /// dial's stock parts while the switch is off and hides them while it is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock: Option<StockMode>,
    /// The stock parts this variant's `stock` acts on, over the record's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stock_parts: Vec<String>,
}

/// One `hud.element` record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HudElement {
    #[serde(default = "no_dial")]
    pub dial: Dial,
    /// The advanced target: a stock scene (`.idf`) the elements grow, and the template node that
    /// instances it, instead of one of the dials.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_ui: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show: Option<Bind>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub also: BTreeMap<String, Also>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stock_nodes: Vec<StockNodes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restyle: Option<RestyleField>,
    #[serde(default, skip_serializing_if = "is_default_units")]
    pub units: Units,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub spectator: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stock: Option<StockMode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stock_parts: Vec<String>,
    /// A dial record's own switch: the record shows while the leaf is on and, with stock `hide`,
    /// the dial's stock parts show while it is off. A variant without a switch inherits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub switch: Option<Bind>,
    #[serde(default, rename = "box", skip_serializing_if = "Option::is_none")]
    pub rect: Option<[f64; 4]>,
    /// Where the root lands, in record units `v` down from the dial centre. Absent: the bottom of
    /// the box on the stock bottom customisation point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sprites: BTreeMap<String, Sprite>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<Element>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<Decision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<Editor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<Clock>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variants: BTreeMap<String, Variant>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

fn yes() -> bool {
    true
}

fn no_dial() -> Dial {
    Dial::F126
}

fn is_true(b: &bool) -> bool {
    *b
}

fn is_default_units(u: &Units) -> bool {
    *u == Units::Hud
}

impl HudElement {
    pub fn parse(value: &serde_json::Value) -> Result<Self> {
        let mut v = value.clone();
        if let Some(o) = v.as_object_mut() {
            o.remove("$comment");
            let targets = ["dial", "scene", "screen"]
                .iter()
                .filter(|k| o.contains_key(**k))
                .count();
            match targets {
                0 => {
                    return Err(err(
                        "record: missing field `dial` (or `scene` and `parent_ui`, or `screen`)",
                    ));
                }
                1 => {}
                _ => {
                    return Err(err(
                        "record: a record has a dial, a scene target or a screen, not two",
                    ));
                }
            }
        }
        let mut rec: Self = serde_json::from_value(v).map_err(|e| err(format!("record: {e}")))?;
        rec.check()?;
        rec.to_hud_units();
        Ok(rec)
    }

    /// Rewrites a `units: tower` record in HUD units, so every later step reads one unit.
    pub fn to_hud_units(&mut self) {
        if self.units == Units::Hud {
            return;
        }
        let f = self.units.k() / super::geom::K;
        self.units = Units::Hud;
        self.rect = self.rect.map(|r| r.map(|v| v * f));
        self.root_y = self.root_y.map(|y| y * f);
        if let Some(ed) = &mut self.editor {
            ed.rect = ed.rect.map(|r| r.map(|v| v * f));
        }
        for sp in self.sprites.values_mut() {
            sp.bounds = sp.bounds.map(|r| r.map(|v| v * f));
            sp.fit = sp.fit.map(|r| r.map(|v| v * f));
            sp.ppu = sp.ppu.map(|p| p / f);
        }
        for e in &mut self.elements {
            scale_element(e, f);
        }
        for v in self.variants.values_mut() {
            v.root_y = v.root_y.map(|y| y * f);
            for e in &mut v.add {
                scale_element(e, f);
            }
        }
    }

    /// The scene and template parent of a record that targets a scene instead of a dial.
    pub fn scene_target(&self) -> Option<(&str, &str)> {
        Some((self.scene.as_deref()?, self.parent_ui.as_deref()?))
    }

    fn check_scene_target(&self) -> Result<()> {
        match (&self.scene, &self.parent_ui) {
            (None, None) => return Ok(()),
            (Some(_), Some(_)) => {}
            _ => return Err(err("scene and parent_ui go together")),
        }
        let scene = self.scene.as_deref().unwrap_or_default();
        if !scene.starts_with("eaid://") || !scene.ends_with(".idf") {
            return Err(err(format!("scene {scene} is the eaid:// path of a stock .idf")));
        }
        let parent = self.parent_ui.as_deref().unwrap_or_default();
        if parent.split('.').any(str::is_empty) {
            return Err(err(format!(
                "parent_ui {parent} is a dotted template path, such as hud_mfd_dynamic.mfd.attacher.car_damage_panel"
            )));
        }
        let what = |w: &str| err(format!("a scene target does not take {w}"));
        if !self.sprites.is_empty() {
            return Err(what("sprites"));
        }
        if !self.decisions.is_empty() {
            return Err(what("decisions"));
        }
        if !self.variants.is_empty() {
            return Err(what("variants"));
        }
        if self.stock.is_some() || !self.stock_parts.is_empty() {
            return Err(what("stock or stock_parts"));
        }
        if self.editor.is_some() || self.rect.is_some() {
            return Err(what("editor or box"));
        }
        fn only_text(list: &[Element]) -> Result<()> {
            for e in list {
                if !matches!(
                    e.kind,
                    Some(Kind::Text | Kind::Group | Kind::Stack | Kind::Centre)
                ) {
                    return Err(err(format!(
                        "element {}: a scene target takes text, group, stack and centre elements, not {:?}",
                        e.id, e.kind
                    )));
                }
                if e.layer.is_some() || e.tint.is_some() {
                    return Err(err(format!(
                        "element {}: a scene target does not take layer or tint",
                        e.id
                    )));
                }
                only_text(&e.children)?;
            }
            Ok(())
        }
        only_text(&self.elements)
    }

    /// Checks the record, then each variant as the record it patches into: an id the variant
    /// removes or replaces is free inside that variant, its placements name elements that are
    /// there, and its decisions show elements that are there.
    pub fn check(&self) -> Result<()> {
        self.check_scene_target()?;
        check_tree(&self.elements, &self.decisions)?;
        for e in &self.elements {
            no_placement(e)?;
        }
        if self.switch.is_some() {
            if self.screen.is_some() || self.scene_target().is_some() {
                return Err(err(
                    "switch is for a dial record; a screen record takes show and a scene target is not switched",
                ));
            }
            if self.stock == Some(StockMode::Park) {
                return Err(err(
                    "stock park is not switched (it moves the dial's stock parts for good); a record with a switch takes keep or hide",
                ));
            }
        }
        for name in self.variants.keys() {
            let what = |e: Error| err(format!("variant {name}: {e}"));
            let v = &self.variants[name];
            if v.stock == Some(StockMode::Park) && (v.switch.is_some() || self.switch.is_some()) {
                return Err(err(format!(
                    "variant {name}: stock park is not switched (it moves the dial's stock parts for good); a variant with a switch takes keep or hide"
                )));
            }
            for c in self.variants[name].add.iter().flat_map(|e| &e.children) {
                no_placement(c).map_err(what)?;
            }
            let patched = self.with_variant(name).map_err(what)?;
            check_tree(&patched.elements, &patched.decisions).map_err(what)?;
            super::motion::check_elements(&self.variants[name].add, None).map_err(what)?;
        }
        super::motion::check_elements(&self.elements, None)?;
        if self.scene_target().is_some() && has_motion(&self.elements) {
            return Err(err(
                "a scene target does not take motion: its triggers need the record's control group (a dial or screen record)",
            ));
        }
        Ok(())
    }

    pub fn effective_stock(&self) -> StockMode {
        self.stock.unwrap_or(match self.dial {
            Dial::F126 => StockMode::Park,
            _ => StockMode::Hide,
        })
    }

    /// The record patched for one variant: its dial, the removed elements gone (from the
    /// decisions too), the added ones in place, its decisions after the record's and its root y.
    pub fn with_variant(&self, name: &str) -> Result<Self> {
        let v = self
            .variants
            .get(name)
            .ok_or_else(|| err(format!("no variant {name}")))?;
        let mut out = self.clone();
        out.dial = v.dial;
        out.variants.clear();
        remove_ids(&mut out.elements, &v.remove);
        for e in &v.add {
            let mut add = e.clone();
            add.before = None;
            add.after = None;
            add.replace = None;
            match e.placement()? {
                None => out.elements.push(add),
                Some((place, id)) => {
                    if !place_element(&mut out.elements, place, id, &add) {
                        return Err(err(format!(
                            "element {}: {} names {id}, which is no element here",
                            e.id,
                            place.key()
                        )));
                    }
                }
            }
        }
        let mut present = std::collections::BTreeSet::new();
        collect_ids(&out.elements, &mut present);
        let gone = |id: &String| present.contains(id);
        out.decisions.iter_mut().for_each(|d| {
            d.yes.retain(gone);
            d.no.retain(gone);
            d.states.values_mut().for_each(|s| s.retain(gone));
        });
        out.decisions.extend(v.decisions.iter().cloned());
        if v.root_y.is_some() {
            out.root_y = v.root_y;
        }
        if v.stock.is_some() {
            out.stock = v.stock;
        }
        if !v.stock_parts.is_empty() {
            out.stock_parts = v.stock_parts.clone();
        }
        Ok(out)
    }
}

/// Puts `add` next to (or in place of) the element `id`, wherever it sits in the tree.
fn place_element(list: &mut Vec<Element>, place: Place, id: &str, add: &Element) -> bool {
    if let Some(i) = list.iter().position(|e| e.id == id) {
        match place {
            Place::Before => list.insert(i, add.clone()),
            Place::After => list.insert(i + 1, add.clone()),
            Place::Replace => list[i] = add.clone(),
        }
        return true;
    }
    list.iter_mut()
        .any(|e| place_element(&mut e.children, place, id, add))
}

/// Checks one element tree and the decisions over it: ids once, kinds complete, every decision
/// target an element of the tree.
fn check_tree(elements: &[Element], decisions: &[Decision]) -> Result<()> {
    let mut ids = std::collections::BTreeSet::new();
    for e in elements {
        check_element(e, &mut ids)?;
    }
    let known: std::collections::BTreeSet<&str> = ids.iter().map(String::as_str).collect();
    let mut seen = std::collections::BTreeSet::new();
    for d in decisions {
        if !seen.insert(d.id.as_str()) {
            return Err(err(format!("decision id {} is used twice", d.id)));
        }
        d.check(&known)?;
    }
    Ok(())
}

fn collect_ids(list: &[Element], out: &mut std::collections::BTreeSet<String>) {
    for e in list {
        out.insert(e.id.clone());
        collect_ids(&e.children, out);
    }
}

fn no_placement(e: &Element) -> Result<()> {
    if e.before.is_some() || e.after.is_some() || e.replace.is_some() {
        return Err(err(format!(
            "element {}: before, after and replace place a variant's add entries only",
            e.id
        )));
    }
    e.children.iter().try_for_each(no_placement)
}

fn scale_element(e: &mut Element, f: f64) {
    let pt = |p: Option<[f64; 2]>| p.map(|p| [p[0] * f, p[1] * f]);
    e.at = pt(e.at);
    e.window = pt(e.window);
    e.pivot = pt(e.pivot);
    e.width = e.width.map(|v| v * f);
    e.cap_height = e.cap_height.map(|v| v * f);
    e.gap = e.gap.map(|v| v * f);
    e.size = e.size.map(|v| v * f);
    e.from = e.from.map(|v| v * f);
    e.to = e.to.map(|v| v * f);
    e.path = e.path.map(|p| p.map(|q| [q[0] * f, q[1] * f]));
    if let Some(r) = &mut e.repeat {
        r.pitch = pt(r.pitch);
    }
    for c in &mut e.children {
        scale_element(c, f);
    }
}

pub const REPEAT_DEPTH: usize = 2;
pub const REPEAT_COPIES: usize = 1024;

/// The elements with every `repeat` group unrolled: copy `i` of group `g` is the element `gNN` at
/// `at + pitch * (i - from)`, its subtree's ids start with `gNN_`, and `{VV}` / `{V}` in the subtree's
/// bindings, literals and `show` patterns read the index. With `rows`, a top-level repeat makes at
/// most that many copies (the editor preview).
pub fn expand(list: &[Element], rows: Option<u32>) -> Result<Vec<Element>> {
    let mut copies = 0usize;
    let mut out = Vec::with_capacity(list.len());
    for e in list {
        expand_into(e, "", &[], 0, rows, &mut copies, &mut out)?;
    }
    Ok(out)
}

fn expand_into(
    e: &Element,
    prefix: &str,
    vars: &[(char, u32)],
    depth: usize,
    rows: Option<u32>,
    copies: &mut usize,
    out: &mut Vec<Element>,
) -> Result<()> {
    let Some(r) = &e.repeat else {
        let mut c = e.clone();
        c.id = format!("{prefix}{}", e.id);
        c.children.clear();
        substitute_element(&mut c, vars)?;
        for ch in &e.children {
            expand_into(ch, prefix, vars, depth, rows, copies, &mut c.children)?;
        }
        out.push(c);
        return Ok(());
    };
    if depth >= REPEAT_DEPTH {
        return Err(err(format!(
            "element {}: repeats nest at most {REPEAT_DEPTH} deep",
            e.id
        )));
    }
    let var = r.var.chars().next().unwrap_or('N');
    if vars.iter().any(|(v, _)| *v == var) {
        return Err(err(format!(
            "element {}: the repeat var {var} is already in use by an enclosing repeat",
            e.id
        )));
    }
    let count = match rows {
        Some(n) if depth == 0 => n.min(r.count),
        _ => r.count,
    };
    let from = r.from.unwrap_or(0);
    let pitch = r.pitch.unwrap_or([0.0, 0.0]);
    let at = e.at.unwrap_or([0.0, 0.0]);
    for k in 0..count {
        *copies += 1;
        if *copies > REPEAT_COPIES {
            return Err(err(format!(
                "element {}: the record's repeats make more than {REPEAT_COPIES} copies",
                e.id
            )));
        }
        let i = from + k;
        let mut vs = vars.to_vec();
        vs.push((var, i));
        let mut c = e.clone();
        c.repeat = None;
        c.id = format!("{prefix}{}{i:02}", e.id);
        c.at = Some([at[0] + pitch[0] * k as f64, at[1] + pitch[1] * k as f64]);
        c.children.clear();
        substitute_element(&mut c, &vs)?;
        let inner = format!("{}_", c.id);
        for ch in &e.children {
            expand_into(ch, &inner, &vs, depth + 1, None, copies, &mut c.children)?;
        }
        out.push(c);
    }
    Ok(())
}

fn has_motion(list: &[Element]) -> bool {
    list.iter().any(|e| e.motion.is_some() || has_motion(&e.children))
}

fn substitute_element(e: &mut Element, vars: &[(char, u32)]) -> Result<()> {
    let id = e.id.clone();
    let text = |s: &mut Option<String>| -> Result<()> {
        if let Some(t) = s {
            *t = super::geom::substitute(t, vars);
            if super::geom::has_placeholder(t) {
                return Err(err(format!(
                    "element {id}: {t} names a repeat var that no enclosing repeat declares"
                )));
            }
        }
        Ok(())
    };
    text(&mut e.literal)?;
    text(&mut e.literal_id)?;
    text(&mut e.show)?;
    for b in [&mut e.visible, &mut e.tint, &mut e.value, &mut e.texture]
        .into_iter()
        .flatten()
        .chain(
            e.motion
                .iter_mut()
                .flat_map(|m| m.play_on.as_mut().into_iter().chain(m.play_to.as_mut())),
        )
    {
        text(&mut b.view_model)?;
        text(&mut b.path)?;
        text(&mut b.session)?;
        text(&mut b.session_car)?;
        if let Some(sv) = &b.slot_var
            && let Some((_, i)) = vars.iter().find(|(v, _)| sv.starts_with(*v) && sv.len() == 1)
        {
            b.slot = Some(*i);
            b.slot_var = None;
        }
    }
    Ok(())
}

fn remove_ids(list: &mut Vec<Element>, ids: &[String]) {
    list.retain(|e| !ids.contains(&e.id));
    for e in list {
        remove_ids(&mut e.children, ids);
    }
}

fn check_element(e: &Element, ids: &mut std::collections::BTreeSet<String>) -> Result<()> {
    if !ids.insert(e.id.clone()) {
        return Err(err(format!("element id {} is used twice", e.id)));
    }
    let kind = e
        .kind
        .ok_or_else(|| err(format!("element {} has no kind", e.id)))?;
    let need = |what: &str, ok: bool| -> Result<()> {
        if ok {
            Ok(())
        } else {
            Err(err(format!("element {} ({kind:?}) needs {what}", e.id)))
        }
    };
    if let Some(r) = &e.repeat {
        need("kind group for repeat", kind == Kind::Group)?;
        need("repeat.count 1..99", (1..=99).contains(&r.count))?;
        need(
            "repeat.var of one upper-case letter",
            r.var.len() == 1 && r.var.chars().all(|c| c.is_ascii_uppercase()),
        )?;
    }
    match kind {
        Kind::Sprite => need("sprite", e.sprite.is_some())?,
        Kind::Text => {
            need(
                "value, literal or literal_id",
                e.value.is_some() || e.literal.is_some() || e.literal_id.is_some(),
            )?;
            need(
                "one of literal and literal_id",
                !(e.literal.is_some() && e.literal_id.is_some()),
            )?;
        }
        Kind::Group => {}
        Kind::Stack => {
            need(
                "dir down, up, right or left",
                matches!(e.dir.as_deref(), None | Some("down" | "up" | "right" | "left")),
            )?;
            if let Some(c) = e.children.iter().find(|c| c.hidden == Some(true)) {
                return Err(err(format!(
                    "element {}: the stack child {} is hidden; a stacker lays out drawn bounds, so put a hidden part inside a child",
                    e.id, c.id
                )));
            }
        }
        Kind::Centre => {}
        Kind::Levels => {
            need(
                "sprite, count and show",
                e.sprite.is_some() && e.count.is_some() && e.show.is_some(),
            )?;
            need("axis y", e.axis.as_deref().is_none_or(|a| a == "y"))?;
        }
        Kind::Slats => need("sprites and show", !e.sprites.is_empty() && e.show.is_some())?,
        Kind::Gauge => need(
            "sprite, path and value",
            e.sprite.is_some() && e.path.is_some() && e.value.is_some(),
        )?,
        Kind::Sweep => need(
            "mask_sprite, fill_sprite, pivot, arc and value",
            e.mask_sprite.is_some()
                && e.fill_sprite.is_some()
                && e.pivot.is_some()
                && e.arc.len() >= 2
                && e.value.is_some(),
        )?,
        Kind::Unit => need("unit and size", e.unit.is_some() && e.size.is_some())?,
    }
    super::units::check_element(e)?;
    if let Some(c) = &e.colour {
        need(
            "kind text or sprite for colour",
            matches!(kind, Kind::Text | Kind::Sprite),
        )?;
        super::textures::hex_rgba(c).map_err(|m| err(format!("element {}: colour: {m}", e.id)))?;
    }
    for b in [&e.visible, &e.tint, &e.value].into_iter().flatten() {
        b.check()?;
    }
    for c in &e.children {
        check_element(c, ids)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn halo_like() -> serde_json::Value {
        serde_json::json!({
            "dial": "f1_26",
            "sprites": {
                "base": { "png": "art/base.png", "bounds": [-100, -48, 110, 38] },
                "solid": { "png": "art/solid.png" }
            },
            "elements": [
                { "kind": "sprite", "id": "base", "sprite": "base", "layer": 20 },
                { "kind": "text", "id": "speed", "style": "OSD_24_bold", "align": "centre", "box": 40,
                  "cap_height": 9.5, "at": [0, -14.9], "value": { "game": "speed_str" } }
            ]
        })
    }

    #[test]
    fn parses_a_record_and_defaults_the_stock_mode_by_dial() {
        let r = HudElement::parse(&halo_like()).unwrap();
        assert_eq!(r.dial, Dial::F126);
        assert!(r.spectator);
        assert_eq!(r.effective_stock(), StockMode::Park);
        assert_eq!(r.elements[1].width, Some(40.0));
        let mut f2 = halo_like();
        f2["dial"] = "f2".into();
        assert_eq!(HudElement::parse(&f2).unwrap().effective_stock(), StockMode::Hide);
    }

    #[test]
    fn a_variant_sets_its_own_stock_mode_and_parts() {
        let mut v = halo_like();
        v["stock"] = "keep".into();
        v["variants"] = serde_json::json!({
            "f2": { "dial": "f2", "stock": "hide", "stock_parts": ["holder_central"],
                    "switch": { "view_model": "view_model_data.hud_x_on", "default": false } },
            "f1_25": { "dial": "f1_25" }
        });
        let r = HudElement::parse(&v).unwrap();
        assert_eq!(r.effective_stock(), StockMode::Keep);
        assert!(r.stock_parts.is_empty());
        let f2 = r.with_variant("f2").unwrap();
        assert_eq!(f2.effective_stock(), StockMode::Hide);
        assert_eq!(f2.stock_parts, ["holder_central"]);
        let f125 = r.with_variant("f1_25").unwrap();
        assert_eq!(
            f125.effective_stock(),
            StockMode::Keep,
            "no stock in the variant: the record's"
        );
        assert!(f125.stock_parts.is_empty());
        let back = serde_json::to_value(&r).unwrap();
        assert_eq!(back["variants"]["f2"]["stock"], "hide");
        assert!(back["variants"]["f1_25"].get("stock").is_none());

        v["variants"]["f2"]["stock"] = "park".into();
        let e = HudElement::parse(&v).unwrap_err().to_string();
        assert!(e.contains("variant f2: stock park is not switched"), "{e}");
        v["variants"]["f2"].as_object_mut().unwrap().remove("switch");
        assert_eq!(
            HudElement::parse(&v)
                .unwrap()
                .with_variant("f2")
                .unwrap()
                .effective_stock(),
            StockMode::Park,
            "park without a switch stays allowed"
        );
    }

    #[test]
    fn refuses_broken_records() {
        let mut v = halo_like();
        v["elements"][1]["value"] = serde_json::json!({ "game": "a", "view_model": "view_model_data.b" });
        assert!(
            HudElement::parse(&v)
                .unwrap_err()
                .to_string()
                .contains("exactly one")
        );
        let mut v = halo_like();
        v["elements"][0]["id"] = "speed".into();
        assert!(HudElement::parse(&v).unwrap_err().to_string().contains("twice"));
        let mut v = halo_like();
        v["elements"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({ "id": "g", "kind": "gauge" }));
        assert!(HudElement::parse(&v).unwrap_err().to_string().contains("needs"));
        let mut v = halo_like();
        v["decisions"] = serde_json::json!([{ "id": "d", "type": "svcd", "game": "ers_mode", "equals": 1, "yes": ["nope"] }]);
        assert!(
            HudElement::parse(&v)
                .unwrap_err()
                .to_string()
                .contains("no element")
        );
    }

    pub fn tower_like(rows: u32, cars: u32) -> serde_json::Value {
        let text = r##"{
            "dial": "f1_26",
            "sprites": { "strip": { "png": "art/strip.png", "bounds": [0, 0, 100, 20] }, "bar": { "png": "art/bar.png" } },
            "elements": [
                { "kind": "stack", "id": "stack", "dir": "down", "gap": 0, "children": [
                    { "kind": "group", "id": "top", "visible": { "view_model": "view_model_data.hud_tt_hdr" }, "children": [
                        { "kind": "text", "id": "lap", "style": "OSD_19_regular", "cap_height": 6, "literal_id": "lng_lap" } ] },
                    { "kind": "group", "id": "rows", "at": [0, 5.73], "children": [
                        { "kind": "group", "id": "r", "repeat": { "count": __ROWS__, "var": "N", "pitch": [0, 21.635] },
                          "visible": { "view_model": "view_model_data.hud_tt_r{NN}_on" },
                          "children": [
                            { "kind": "sprite", "id": "s", "sprite": "strip", "layer": 20 },
                            { "kind": "group", "id": "pa", "visible": { "view_model": "view_model_data.hud_tt_r{NN}_up", "not": true, "default": true },
                              "children": [ { "kind": "text", "id": "pos", "style": "OSD_19_regular", "cap_height": 5.9, "literal": "r{N}",
                                              "value": { "view_model": "view_model_data.hud_tt_r{NN}_pos" } } ] },
                            { "kind": "group", "id": "cn", "at": [14.2, 14.6], "children": [
                                { "kind": "group", "id": "v", "repeat": { "count": __CARS__, "var": "V" },
                                  "visible": { "view_model": "view_model_data.hud_tt_r{NN}_c{VV}" },
                                  "children": [
                                    { "kind": "text", "id": "code", "style": "OSD_24_bold", "cap_height": 7.5,
                                      "value": { "session_car": "driver.name_code", "slot_var": "V" } },
                                    { "kind": "sprite", "id": "bar", "sprite": "bar", "layer": 30,
                                      "tint": { "session_car": "nub_colour", "slot_var": "V", "default": "#00000000" } } ] } ] } ] } ] } ] }
            ],
            "editor": { "rows": 5, "preview": { "view_model_data.hud_tt_r00_on": true } }
        }"##;
        serde_json::from_str(
            &text
                .replace("__ROWS__", &rows.to_string())
                .replace("__CARS__", &cars.to_string()),
        )
        .unwrap()
    }

    fn ids(list: &[Element]) -> Vec<&str> {
        list.iter().map(|e| e.id.as_str()).collect()
    }

    #[test]
    fn a_repeat_unrolls_with_padded_names_pitch_and_substitution() {
        let r = HudElement::parse(&tower_like(22, 24)).unwrap();
        let out = expand(&r.elements, None).unwrap();
        let rows = &out[0].children[1].children;
        assert_eq!(rows.len(), 22);
        assert_eq!(rows[0].id, "r00");
        assert_eq!(rows[21].id, "r21");
        assert!(rows.iter().all(|r| r.repeat.is_none()));
        assert_eq!(rows[0].at, Some([0.0, 0.0]));
        assert!((rows[5].at.unwrap()[1] - 5.0 * 21.635).abs() < 1e-9);
        assert_eq!(
            rows[3].visible.as_ref().unwrap().view_model.as_deref(),
            Some("view_model_data.hud_tt_r03_on")
        );
        assert_eq!(ids(&rows[3].children), ["r03_s", "r03_pa", "r03_cn"]);
        let pos = &rows[3].children[1].children[0];
        assert_eq!(pos.id, "r03_pos");
        assert_eq!(pos.literal.as_deref(), Some("r3"));
        assert_eq!(
            pos.value.as_ref().unwrap().view_model.as_deref(),
            Some("view_model_data.hud_tt_r03_pos")
        );
        let cars = &rows[3].children[2].children;
        assert_eq!(cars.len(), 24);
        assert_eq!(cars[0].id, "r03_v00");
        assert_eq!(cars[23].id, "r03_v23");
        let v17 = &cars[17];
        assert_eq!(
            v17.visible.as_ref().unwrap().view_model.as_deref(),
            Some("view_model_data.hud_tt_r03_c17")
        );
        assert_eq!(ids(&v17.children), ["r03_v17_code", "r03_v17_bar"]);
        let code = v17.children[0].value.as_ref().unwrap();
        assert_eq!(code.slot, Some(17));
        assert_eq!(code.slot_var, None);
        let bar = v17.children[1].tint.as_ref().unwrap();
        assert_eq!(bar.slot, Some(17));
        let nested: usize = rows.iter().map(|r| r.children[2].children.len()).sum();
        assert_eq!(nested, 528);
        assert_eq!(
            out[0].children[0].children[0].literal_id.as_deref(),
            Some("lng_lap")
        );
    }

    #[test]
    fn the_preview_expands_rows_copies_only() {
        let r = HudElement::parse(&tower_like(22, 24)).unwrap();
        let out = expand(&r.elements, Some(5)).unwrap();
        let rows = &out[0].children[1].children;
        assert_eq!(rows.len(), 5);
        assert_eq!(rows[4].children[2].children.len(), 24);
    }

    #[test]
    fn repeats_are_capped_by_depth_and_product() {
        let mut v = tower_like(40, 30);
        assert!(
            expand(&HudElement::parse(&v).unwrap().elements, None)
                .unwrap_err()
                .to_string()
                .contains("more than 1024")
        );
        v = tower_like(2, 2);
        v["elements"][0]["children"][1]["children"][0]["children"][2]["children"][0]["children"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({ "kind": "group", "id": "deep", "repeat": { "count": 2, "var": "D" } }));
        assert!(
            expand(&HudElement::parse(&v).unwrap().elements, None)
                .unwrap_err()
                .to_string()
                .contains("deep")
        );
        let v = tower_like(2, 2);
        let mut r = HudElement::parse(&v).unwrap();
        r.elements[0].children[1].children[0].children[0].visible = Some(Bind::leaf("view_model_data.x_{Q}"));
        assert!(expand(&r.elements, None).unwrap_err().to_string().contains("{Q}"));
    }

    #[test]
    fn a_stack_child_may_not_switch_and_a_repeat_sits_on_a_group() {
        let mut v = tower_like(2, 2);
        v["elements"][0]["children"][1]["hidden"] = true.into();
        let e = HudElement::parse(&v).unwrap_err().to_string();
        assert!(e.contains("stack child rows is hidden"), "{e}");
        let mut v = tower_like(2, 2);
        v["elements"][0]["children"][1]["children"][0]["kind"] = "sprite".into();
        v["elements"][0]["children"][1]["children"][0]["sprite"] = "bar".into();
        let e = HudElement::parse(&v).unwrap_err().to_string();
        assert!(e.contains("kind group for repeat"), "{e}");
    }

    #[test]
    fn tower_units_scale_to_hud_units_once() {
        let mut v = tower_like(2, 2);
        v["units"] = "tower".into();
        let r = HudElement::parse(&v).unwrap();
        assert_eq!(r.units, Units::Hud);
        let f = 0.03 / 0.018;
        let rows = &r.elements[0].children[1];
        assert!((rows.at.unwrap()[1] - 5.73 * f).abs() < 1e-9);
        let rep = rows.children[0].repeat.as_ref().unwrap();
        assert!((rep.pitch.unwrap()[1] - 21.635 * f).abs() < 1e-9);
        assert!((r.sprites["strip"].bounds.unwrap()[2] - 100.0 * f).abs() < 1e-9);
        let plain = HudElement::parse(&tower_like(2, 2)).unwrap();
        assert_eq!(plain.sprites["strip"].bounds.unwrap()[2], 100.0);
    }

    #[test]
    fn a_variant_patches_the_base() {
        let mut v = halo_like();
        v["variants"] = serde_json::json!({ "f2": { "dial": "f2", "remove": ["speed"] } });
        let r = HudElement::parse(&v).unwrap();
        let f2 = r.with_variant("f2").unwrap();
        assert_eq!(f2.dial, Dial::F2);
        assert_eq!(f2.elements.len(), 1);
        assert!(r.with_variant("nope").is_err());
    }

    /// A base with a draw order (back plate, two bars, a lamp) and a pill group only one dial has.
    fn layered() -> serde_json::Value {
        serde_json::json!({
            "dial": "f1_26",
            "sprites": {
                "base": { "png": "art/base.png", "bounds": [-100, -48, 110, 38] },
                "bar": { "png": "art/bar.png", "bounds": [-40, 0, 40, 8] },
                "lamp": { "png": "art/lamp.png", "bounds": [-8, -8, 8, 8] }
            },
            "elements": [
                { "kind": "sprite", "id": "base", "sprite": "base", "layer": 20 },
                { "kind": "group", "id": "energy", "children": [
                    { "kind": "sprite", "id": "bar_a", "sprite": "bar" },
                    { "kind": "sprite", "id": "bar_b", "sprite": "bar", "at": [0, 10] } ] },
                { "kind": "sprite", "id": "lamp", "sprite": "lamp", "visible": { "view_model": "view_model_data.hud_x_lamp" } }
            ],
            "decisions": [
                { "id": "mode", "type": "svcd", "game": "ers_mode", "equals": 2, "ty": "ui32", "yes": ["lamp"], "no": [] }
            ],
            "variants": {
                "f1_25": {
                    "dial": "f1_25",
                    "remove": ["bar_b"],
                    "add": [
                        { "kind": "sprite", "id": "bar_a", "sprite": "bar", "at": [0, 4], "replace": "bar_a" },
                        { "kind": "sprite", "id": "plate", "sprite": "bar", "before": "base" },
                        { "kind": "group", "id": "pill", "after": "energy", "children": [
                            { "kind": "sprite", "id": "pill_idle", "sprite": "lamp" },
                            { "kind": "sprite", "id": "pill_live", "sprite": "lamp" } ] },
                        { "kind": "sprite", "id": "tail", "sprite": "lamp" }
                    ],
                    "decisions": [
                        { "id": "drs", "type": "mvcd", "game": "drs_pit_state", "initial": "invalid",
                          "states": { "invalid": ["pill_idle"], "drs_live": ["pill_live"] } }
                    ],
                    "root_y": 16
                }
            }
        })
    }

    fn top_ids(r: &HudElement) -> Vec<&str> {
        ids(&r.elements)
    }

    #[test]
    fn a_variant_places_its_adds_and_a_removed_or_replaced_id_is_free_in_it() {
        let r = HudElement::parse(&layered()).unwrap();
        assert_eq!(top_ids(&r), ["base", "energy", "lamp"]);
        let v = r.with_variant("f1_25").unwrap();
        assert_eq!(top_ids(&v), ["plate", "base", "energy", "pill", "lamp", "tail"]);
        let energy = &v.elements[2];
        assert_eq!(ids(&energy.children), ["bar_a"]);
        assert_eq!(energy.children[0].at, Some([0.0, 4.0]));
        assert!(
            v.elements
                .iter()
                .all(|e| e.replace.is_none() && e.before.is_none() && e.after.is_none())
        );
        assert_eq!(ids(&v.elements[3].children), ["pill_idle", "pill_live"]);
        let mut bad = layered();
        bad["variants"]["f1_25"]["add"][0]["replace"] = serde_json::Value::Null;
        let e = HudElement::parse(&bad).unwrap_err().to_string();
        assert!(
            e.contains("variant f1_25") && e.contains("bar_a is used twice"),
            "{e}"
        );
        let mut bad = layered();
        bad["variants"]["f1_25"]["add"][1]["before"] = "bar_b".into();
        let e = HudElement::parse(&bad).unwrap_err().to_string();
        assert!(
            e.contains("variant f1_25") && e.contains("before names bar_b"),
            "{e}"
        );
        let mut bad = layered();
        bad["variants"]["f1_25"]["add"][1]["after"] = "base".into();
        let e = HudElement::parse(&bad).unwrap_err().to_string();
        assert!(e.contains("one placement, not two"), "{e}");
        let mut bad = layered();
        bad["elements"][2]["after"] = "base".into();
        let e = HudElement::parse(&bad).unwrap_err().to_string();
        assert!(e.contains("place a variant's add entries only"), "{e}");
    }

    #[test]
    fn a_variant_carries_its_own_decisions_and_root_y() {
        let r = HudElement::parse(&layered()).unwrap();
        assert_eq!(r.decisions.len(), 1);
        assert_eq!(r.root_y, None);
        let v = r.with_variant("f1_25").unwrap();
        assert_eq!(
            v.decisions.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
            ["mode", "drs"]
        );
        assert_eq!(v.root_y, Some(16.0));
        assert_eq!(
            v.decisions[1].initial_targets().collect::<Vec<_>>(),
            [&"pill_idle".to_owned()]
        );
        let mut bad = layered();
        bad["variants"]["f1_25"]["decisions"][0]["states"]["drs_live"] = serde_json::json!(["bar_b"]);
        let e = HudElement::parse(&bad).unwrap_err().to_string();
        assert!(
            e.contains("variant f1_25") && e.contains("bar_b, which is no element"),
            "{e}"
        );
        let mut bad = layered();
        bad["decisions"][0]["yes"] = serde_json::json!(["pill_idle"]);
        let e = HudElement::parse(&bad).unwrap_err().to_string();
        assert!(
            e.contains("decision mode shows pill_idle, which is no element"),
            "{e}"
        );
        let mut bad = layered();
        bad["variants"]["f1_25"]["decisions"][0]["id"] = "mode".into();
        let e = HudElement::parse(&bad).unwrap_err().to_string();
        assert!(
            e.contains("variant f1_25") && e.contains("decision id mode is used twice"),
            "{e}"
        );
        let mut same = layered();
        same["variants"]["f1_25"]["add"][3] =
            serde_json::json!({ "kind": "sprite", "id": "lamp", "sprite": "bar", "replace": "lamp" });
        let v = HudElement::parse(&same).unwrap().with_variant("f1_25").unwrap();
        assert_eq!(
            v.decisions[0].yes,
            ["lamp"],
            "a same-id replacement stays a target"
        );
        same["variants"]["f1_25"]["add"][3]["id"] = "lamp2".into();
        let v = HudElement::parse(&same).unwrap().with_variant("f1_25").unwrap();
        assert!(v.decisions[0].yes.is_empty(), "a replaced id leaves the decision");
        let mut tower = layered();
        tower["units"] = "tower".into();
        tower["root_y"] = 6.into();
        let t = HudElement::parse(&tower).unwrap();
        let f = 0.03 / 0.018;
        assert!((t.root_y.unwrap() - 6.0 * f).abs() < 1e-9);
        assert!((t.with_variant("f1_25").unwrap().root_y.unwrap() - 16.0 * f).abs() < 1e-9);
    }

    #[test]
    fn initial_names_a_state_of_its_decision() {
        let r = HudElement::parse(&layered()).unwrap();
        assert_eq!(r.decisions[0].initial_targets().count(), 0);
        let mut v = layered();
        v["decisions"][0]["initial"] = "yes".into();
        let r = HudElement::parse(&v).unwrap();
        assert_eq!(
            r.decisions[0].initial_targets().collect::<Vec<_>>(),
            [&"lamp".to_owned()]
        );
        v["decisions"][0]["initial"] = "maybe".into();
        let e = HudElement::parse(&v).unwrap_err().to_string();
        assert!(e.contains("initial names one of its states (yes or no)"), "{e}");
        let mut v = layered();
        v["variants"]["f1_25"]["decisions"][0]["initial"] = "pit_live".into();
        let e = HudElement::parse(&v).unwrap_err().to_string();
        assert!(
            e.contains("initial names one of its states (drs_live, invalid)"),
            "{e}"
        );
    }
}
