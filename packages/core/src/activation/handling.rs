use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use swofty_formats::bxml::{Document, Element, Kind};
use swofty_formats::erp::{Entry, ErpFile, Resource};
use swofty_formats::pkg::{self, Node, Pkg, Val};

use crate::activation::wheel_check;
use crate::{Result, io_err};

pub const REGISTRY: &str = "vehicle.handling";
const VEHICLES: &str = "vehicle";

const SECTIONS: &[(&str, &str)] = &[
    ("engine", "Engine"),
    ("gearbox", "GearBox"),
    ("fuel_tank", "FuelTank"),
    ("fuel_ecu", "FuelECU"),
    ("brake_ecu", "BrakeECU"),
    ("chassis", "Chassis"),
    ("steering", "Steering"),
    ("core_aero", "CoreAero"),
    ("differential", "Differential"),
    ("suspension", "SuspensionSystem"),
    ("f1_aero", "F1Aero"),
    ("assists", "F1Assists"),
    ("tuning", "F1Tuning"),
    ("plank", "F1Plank"),
    ("suspension_damage", "F1SuspensionDamage"),
    ("wing_damage", "F1WingDamage"),
    ("tyre_damage", "F1TyreDamage"),
    ("body_damage", "F1BodyDamage"),
    ("turbo", "Turbo"),
    ("ers", "EnergyRecoverySystem"),
];

const GEARS: [&str; 9] = ["1st", "2nd", "3rd", "4th", "5th", "6th", "7th", "8th", "9th"];

const DRS_REMOVED: &[&str] = &[
    "DistanceDrivenCumulativeDRSFailureChance",
    "DRSRotationJointAngleRatio",
    "DRSFailTimeMin",
    "DRSFailTimeMax",
];
const DRS_SET: &[(&str, &str)] = &[
    ("DRSRotationAngleRadians", "0.0"),
    ("DRSOpenCloseSpeed", "1"),
    ("m_aero_wing_front_drag", "1.0"),
    ("m_aero_wing_front_force", "1.0"),
    ("m_aero_wing_rear_drag", "1.0"),
    ("m_aero_wing_rear_force", "1.0"),
];
const DRS_UPGRADE: &str = "radUpgradeDRS";

pub fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '_')
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

pub fn section_name(snake: &str) -> Option<&'static str> {
    SECTIONS.iter().find(|(k, _)| *k == snake).map(|(_, v)| *v)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Set(String),
    Scale(f64),
    Add(f64),
    Curve(Vec<(f64, f64)>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub element: &'static str,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Leaf {
    pub path: String,
    pub section: &'static str,
    pub group: Option<Group>,
    pub key: String,
    pub op: Op,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Leaf(Leaf),
    Remove(&'static str),
    Gears {
        ratios: Vec<f64>,
        up: Vec<f64>,
        down: Vec<f64>,
    },
    OwnGears,
    DrsHardware(bool),
    Upgrade {
        name: String,
        section: String,
        key: String,
        op: char,
        value: f64,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    pub steps: Vec<Step>,
    pub notes: Vec<String>,
}

fn axle_key(field: &str) -> Option<(&'static str, String)> {
    match field {
        "grip_multiplier" => Some(("Tyre", "GripMultiplier".into())),
        "tyre_radius" => Some(("Tyre", "TyreRadius".into())),
        "rest_x" => Some(("Suspension", "LocalRestPositionX".into())),
        "rest_z" => Some(("Suspension", "LocalRestPositionZ".into())),
        _ => Some(("Suspension", field.into())),
    }
}

fn mode_text(v: &str) -> Option<&'static str> {
    match v {
        "off" => Some("Off"),
        "full" => Some("Full"),
        _ => None,
    }
}

fn drive_text(v: &str) -> Option<&'static str> {
    match v {
        "rwd" => Some("RWD"),
        "fwd" => Some("FWD"),
        "awd" => Some("4WD"),
        _ => None,
    }
}

fn leaf(path: &[&str], section: &'static str, group: Option<Group>, key: &str, op: Op) -> Leaf {
    Leaf {
        path: path.join("."),
        section,
        group,
        key: key.to_owned(),
        op,
    }
}

pub fn resolve_path(path: &[&str], op: &Op) -> std::result::Result<Vec<Leaf>, String> {
    let Some(head) = path.first() else {
        return Err("an empty path".into());
    };
    let section = section_name(head).ok_or_else(|| format!("{head} is not a handling section"))?;
    let rest = &path[1..];
    let one = |l: Leaf| Ok(vec![l]);
    match (*head, rest) {
        (_, []) => Err(format!("{head} names a section, not a field")),
        ("suspension", [axle @ ("front" | "rear"), field]) => {
            let (element, key) = axle_key(field).expect("an axle key");
            let name = if *axle == "front" { "Front" } else { "Rear" };
            one(leaf(
                path,
                section,
                Some(Group {
                    element: if element == "Tyre" { "Tyre" } else { "Suspension" },
                    name: Some(name.into()),
                }),
                &key,
                op.clone(),
            ))
        }
        (
            "suspension",
            [
                axle @ ("front_tyre" | "rear_tyre" | "front_brake" | "rear_brake"),
                field,
            ],
        ) => {
            let (side, element) = axle.split_once('_').expect("an underscore");
            one(leaf(
                path,
                section,
                Some(Group {
                    element: if element == "tyre" { "Tyre" } else { "Brake" },
                    name: Some(if side == "front" { "Front" } else { "Rear" }.into()),
                }),
                field,
                op.clone(),
            ))
        }
        ("suspension", ["use_model_positions"]) => Ok(["Front", "Rear"]
            .iter()
            .map(|n| {
                leaf(
                    path,
                    section,
                    Some(Group {
                        element: "Suspension",
                        name: Some((*n).into()),
                    }),
                    "UseModelPositions",
                    op.clone(),
                )
            })
            .collect()),
        ("f1_aero", ["slipstream", f]) => {
            let key = match *f {
                "drag" => "m_aeroSlipStreamAirDensityScaleDrag",
                "front_wing" => "m_aeroSlipStreamAirDensityScaleFrontWing",
                "rear_wing" => "m_aeroSlipStreamAirDensityScaleRearWing",
                "lift" => "m_aeroSlipStreamAirDensityScaleLift",
                "car_width" => "m_aeroSlipStreamCarWidth",
                other => return Err(format!("f1_aero.slipstream.{other} is not a slipstream field")),
            };
            one(leaf(path, section, None, key, op.clone()))
        }
        ("f1_aero", ["drs", f]) => {
            let key = match *f {
                "angle" => "DRSRotationAngleRadians",
                "open_close_speed" => "DRSOpenCloseSpeed",
                "hardware" => return Err("f1_aero.drs.hardware is a switch, not a value".into()),
                other => return Err(format!("f1_aero.drs.{other} is not a DRS field")),
            };
            one(leaf(path, section, None, key, op.clone()))
        }
        ("gearbox", ["final_drive"]) | ("gearbox", ["final_drive_ratio"]) => {
            one(leaf(path, section, None, "FinalDriveRatio", op.clone()))
        }
        ("gearbox", ["allow_gear_ratio_tuning"]) => {
            one(leaf(path, section, None, "AllowGearRatioTuning", op.clone()))
        }
        ("gearbox", ["allow_h_pattern"]) => one(leaf(path, section, None, "AllowHPattern", op.clone())),
        ("gearbox", ["reverse"]) | ("gearbox", ["gear_ratio_reverse"]) => {
            one(leaf(path, section, None, "GearRatioReverse", op.clone()))
        }
        ("turbo", ["mode"]) => {
            let Op::Set(v) = op else {
                return Err("turbo.mode is set, not scaled".into());
            };
            let text = mode_text(v).ok_or_else(|| {
                format!(
                    "turbo.mode {v}: only off and full are read from a stock file; lag_spoof is not applied"
                )
            })?;
            Ok(vec![Leaf {
                path: path.join("."),
                section: "Engine",
                group: None,
                key: "TurboMode".into(),
                op: Op::Set(text.into()),
            }])
        }
        ("differential", ["drive_type"]) => {
            let Op::Set(v) = op else {
                return Err("differential.drive_type is set, not scaled".into());
            };
            let text =
                drive_text(v).ok_or_else(|| format!("differential.drive_type {v} is not rwd, fwd or awd"))?;
            one(leaf(path, section, None, "DriveType", Op::Set(text.into())))
        }
        ("ers", [f]) if *f != "modes" => one(leaf(
            path,
            section,
            Some(Group {
                element: "Common",
                name: None,
            }),
            f,
            op.clone(),
        )),
        (_, [f]) => one(leaf(path, section, None, f, op.clone())),
        _ => Err(format!("{} is not a handling field", path.join("."))),
    }
}

fn text_of(v: &Value) -> Option<String> {
    match v {
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(_) => crate::activation::ers::format_value(v).ok(),
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn curve_of(v: &Value) -> Option<Vec<(f64, f64)>> {
    let a = v.as_array()?;
    if a.is_empty() {
        return None;
    }
    a.iter()
        .map(|p| {
            let p = p.as_array()?;
            (p.len() == 2).then_some(())?;
            Some((p[0].as_f64()?, p[1].as_f64()?))
        })
        .collect()
}

fn numbers(v: &Value) -> Option<Vec<f64>> {
    v.as_array()?.iter().map(Value::as_f64).collect()
}

impl Plan {
    pub fn from_record(record: &Value) -> Plan {
        let mut plan = Plan::default();
        let Some(map) = record.as_object() else {
            return plan;
        };
        for (snake, value) in map {
            let Some(section) = section_name(snake) else {
                continue;
            };
            match (snake.as_str(), value) {
                ("ers", Value::Null) => plan.steps.push(Step::Remove(section)),
                (_, Value::Object(fields)) => plan.walk(&[snake.as_str()], fields),
                _ => {}
            }
        }
        for u in map
            .get("rad_upgrades")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(name) = u.get("name").and_then(Value::as_str) else {
                plan.notes.push("a rad_upgrade has no name".into());
                continue;
            };
            for c in u.get("changes").and_then(Value::as_array).into_iter().flatten() {
                let (Some(section), Some(key), Some(value)) = (
                    c.get("section").and_then(Value::as_str),
                    c.get("key").and_then(Value::as_str),
                    c.get("value").and_then(Value::as_f64),
                ) else {
                    plan.notes.push(format!(
                        "rad_upgrade {name}: a change needs section, key and value"
                    ));
                    continue;
                };
                let op = c
                    .get("op")
                    .and_then(Value::as_str)
                    .and_then(|o| o.chars().next())
                    .unwrap_or('=');
                plan.steps.push(Step::Upgrade {
                    name: name.to_owned(),
                    section: section_name(section).unwrap_or(section).to_owned(),
                    key: key.to_owned(),
                    op,
                    value,
                });
            }
        }
        for name in map.get("remove").and_then(Value::as_array).into_iter().flatten() {
            match name.as_str().and_then(section_name) {
                Some(section) => plan.steps.push(Step::Remove(section)),
                None => plan.notes.push(format!("remove {name}: not a handling section")),
            }
        }
        for e in map.get("edits").and_then(Value::as_array).into_iter().flatten() {
            let Some(path) = e.get("path").and_then(Value::as_str) else {
                plan.notes.push("an edit has no path".into());
                continue;
            };
            let Some(v) = e.get("value").and_then(Value::as_f64) else {
                plan.notes.push(format!("edit {path}: no numeric value"));
                continue;
            };
            let op = match e.get("op").and_then(Value::as_str).unwrap_or("set") {
                "set" => Op::Set(text_of(&e["value"]).unwrap_or_default()),
                "scale" => Op::Scale(v),
                "add" => Op::Add(v),
                other => {
                    plan.notes
                        .push(format!("edit {path}: {other} is not set, scale or add"));
                    continue;
                }
            };
            let parts: Vec<&str> = path.split('.').collect();
            match resolve_path(&parts, &op) {
                Ok(leaves) => plan.steps.extend(leaves.into_iter().map(Step::Leaf)),
                Err(e) => plan.notes.push(format!("edit {path}: {e}")),
            }
        }
        plan
    }

    fn walk(&mut self, at: &[&str], fields: &Map<String, Value>) {
        for (key, value) in fields {
            if key == "extra" {
                continue;
            }
            let mut path = at.to_vec();
            path.push(key.as_str());
            if at == ["gearbox"]
                && matches!(
                    key.as_str(),
                    "ratios" | "up_speed_multiple" | "down_speed_multiple"
                )
            {
                continue;
            }
            match value {
                Value::Object(inner)
                    if at[0] == "tuning" && inner.keys().all(|k| k == "min" || k == "max") =>
                {
                    for (bound, v) in inner {
                        let name = format!("{key}_{bound}");
                        self.leaf_of(&[at[0], name.as_str()], v);
                    }
                }
                Value::Object(inner) if at[0] == "f1_aero" && at.len() == 1 && key == "drs" => {
                    for (k, v) in inner {
                        if k == "hardware" {
                            if let Some(b) = v.as_bool() {
                                self.steps.push(Step::DrsHardware(b));
                            }
                            continue;
                        }
                        self.leaf_of(&["f1_aero", "drs", k.as_str()], v);
                    }
                }
                Value::Object(inner) => self.walk(&path, inner),
                _ => self.leaf_of(&path, value),
            }
        }
        if at == ["gearbox"] {
            let ratios = fields.get("ratios").and_then(numbers);
            let up = fields
                .get("up_speed_multiple")
                .and_then(numbers)
                .unwrap_or_default();
            let down = fields
                .get("down_speed_multiple")
                .and_then(numbers)
                .unwrap_or_default();
            if let Some(ratios) = ratios {
                self.steps.push(Step::Gears { ratios, up, down });
            }
            if fields.contains_key("ratios") || fields.contains_key("final_drive_ratio") {
                self.steps.push(Step::OwnGears);
            }
        }
    }

    fn leaf_of(&mut self, path: &[&str], value: &Value) {
        let op = match (curve_of(value), text_of(value)) {
            (Some(c), _) => Op::Curve(c),
            (None, Some(t)) => Op::Set(t),
            _ => {
                if !value.is_null() && !value.as_array().is_some_and(|a| a.is_empty()) {
                    self.notes
                        .push(format!("{}: not a value the build applies", path.join(".")));
                }
                return;
            }
        };
        match resolve_path(path, &op) {
            Ok(leaves) => self.steps.extend(leaves.into_iter().map(Step::Leaf)),
            Err(e) => self.notes.push(e),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

fn is_el(e: &Element, name: &str) -> bool {
    e.kind == Kind::Element && e.name == name.as_bytes()
}

fn fmt_number(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let s = format!("{v:.9}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.to_owned() }
}

fn new_leaf(like: Option<&Element>, name: &str, value: &str) -> Element {
    let mut e = like.cloned().unwrap_or(Element {
        kind: Kind::Element,
        pad: 0,
        name: Vec::new(),
        attrs: Vec::new(),
        children: Vec::new(),
        end_pad: 0,
    });
    e.name = name.as_bytes().to_vec();
    e.attrs = vec![(b"value".to_vec(), value.as_bytes().to_vec())];
    e.children.clear();
    e
}

fn find_key(node: &Element, key: &str) -> Option<usize> {
    let n = norm(key);
    node.children
        .iter()
        .position(|c| c.kind == Kind::Element && c.name == key.as_bytes())
        .or_else(|| {
            node.children
                .iter()
                .position(|c| c.kind == Kind::Element && norm(&String::from_utf8_lossy(&c.name)) == n)
        })
}

fn curve_leaf(node: &mut Element, points: &[(f64, f64)]) -> String {
    let template = node.children.iter().find(|c| is_el(c, "SplineElement")).cloned();
    let before = node.children.iter().filter(|c| is_el(c, "SplineElement")).count();
    node.children.retain(|c| !is_el(c, "SplineElement"));
    for (x, y) in points {
        let mut e = template.clone().unwrap_or(Element {
            kind: Kind::Element,
            pad: 0,
            name: b"SplineElement".to_vec(),
            attrs: vec![],
            children: vec![],
            end_pad: 0,
        });
        e.attrs = vec![
            (b"x".to_vec(), fmt_number(*x).into_bytes()),
            (b"y".to_vec(), fmt_number(*y).into_bytes()),
            (b"op".to_vec(), b"=".to_vec()),
        ];
        node.children.push(e);
    }
    node.set_attr(b"numVertices", points.len().to_string().as_bytes());
    format!("{before} -> {} vertices", points.len())
}

pub struct Applied {
    pub changed: bool,
    pub lines: Vec<String>,
    pub notes: Vec<String>,
}

fn upgrade_blocks(root: &mut Element) -> impl Iterator<Item = &mut Element> {
    root.children.iter_mut().filter(|c| is_el(c, "Upgrade"))
}

impl Plan {
    pub fn apply(&self, root: &mut Element) -> Applied {
        let mut out = Applied {
            changed: false,
            lines: Vec::new(),
            notes: Vec::new(),
        };
        for step in &self.steps {
            match step {
                Step::Leaf(l) => apply_leaf(root, l, &mut out),
                Step::Remove(section) => remove_section(root, section, &mut out),
                Step::Gears { ratios, up, down } => apply_gears(root, ratios, up, down, &mut out),
                Step::OwnGears => own_gears(root, &mut out),
                Step::Upgrade {
                    name,
                    section,
                    key,
                    op,
                    value,
                } => apply_upgrade(root, name, section, key, *op, *value, &mut out),
                Step::DrsHardware(on) => {
                    if !*on {
                        remove_drs(root, &mut out);
                    }
                }
            }
        }
        out
    }
}

fn apply_upgrade(
    root: &mut Element,
    name: &str,
    section: &str,
    key: &str,
    op: char,
    value: f64,
    out: &mut Applied,
) {
    let block = match root
        .children
        .iter()
        .position(|c| is_el(c, "Upgrade") && c.attr(b"name") == Some(name.as_bytes()))
    {
        Some(i) => &mut root.children[i],
        None => {
            out.notes.push(format!("radUpgrade {name}: no such block"));
            return;
        }
    };
    if !block.children.iter().any(|c| is_el(c, section)) {
        let mut s = new_leaf(None, section, "");
        s.attrs.clear();
        block.children.push(s);
    }
    let s = block
        .children
        .iter_mut()
        .find(|c| is_el(c, section))
        .expect("a section");
    let text = fmt_number(value);
    let label = format!("radUpgrade {name}: {section}/{key}");
    match find_key(s, key) {
        Some(i) => {
            let old = s.children[i]
                .attr(b"value")
                .map(|v| String::from_utf8_lossy(v).into_owned());
            let old_op = s.children[i]
                .attr(b"op")
                .map(|v| String::from_utf8_lossy(v).into_owned());
            s.children[i].set_attr(b"value", text.as_bytes());
            s.children[i].set_attr(b"op", op.to_string().as_bytes());
            out.lines.push(format!(
                "{label}: {}{} -> {op}{text}",
                old_op.unwrap_or_default(),
                old.unwrap_or_default()
            ));
        }
        None => {
            let mut l = new_leaf(None, key, &text);
            l.attrs.push((b"op".to_vec(), op.to_string().into_bytes()));
            s.children.push(l);
            out.lines.push(format!("{label}: added {op}{text}"));
        }
    }
    out.changed = true;
}

fn remove_section(root: &mut Element, section: &str, out: &mut Applied) {
    let before = root.children.len();
    root.children.retain(|c| !is_el(c, section));
    if root.children.len() == before {
        out.notes.push(format!("{section}: no such section to remove"));
        return;
    }
    let mut parts = 0;
    for block in upgrade_blocks(root) {
        let n = block.children.len();
        block.children.retain(|c| !is_el(c, section));
        parts += n - block.children.len();
    }
    out.changed = true;
    out.lines.push(format!(
        "{section}: section removed, {parts} part(s) dropped from the radUpgrade blocks"
    ));
}

fn remove_drs(root: &mut Element, out: &mut Applied) {
    let Some(aero) = root.children.iter_mut().find(|c| is_el(c, "F1Aero")) else {
        out.notes.push("F1Aero: no such section for drs.hardware".into());
        return;
    };
    let before = aero.children.len();
    aero.children
        .retain(|c| !(c.kind == Kind::Element && DRS_REMOVED.iter().any(|k| c.name == k.as_bytes())));
    let removed = before - aero.children.len();
    for (k, v) in DRS_SET {
        match find_key(aero, k) {
            Some(i) => aero.children[i].set_attr(b"value", v.as_bytes()),
            None => out.notes.push(format!("F1Aero/{k}: missing")),
        }
    }
    for block in upgrade_blocks(root) {
        if block.attr(b"name") == Some(DRS_UPGRADE.as_bytes()) {
            block.children.retain(|c| c.kind != Kind::Element);
        }
    }
    out.changed = true;
    out.lines.push(format!(
        "F1Aero: DRS hardware removed ({removed} keys dropped, flap and wing factors neutral)"
    ));
}

fn container<'a>(root: &'a mut Element, l: &Leaf) -> Option<&'a mut Element> {
    let section = root.children.iter_mut().find(|c| is_el(c, l.section))?;
    match &l.group {
        None => Some(section),
        Some(g) => section.children.iter_mut().find(|c| {
            is_el(c, g.element)
                && match &g.name {
                    None => true,
                    Some(n) => c.attr(b"name") == Some(n.as_bytes()),
                }
        }),
    }
}

fn apply_leaf(root: &mut Element, l: &Leaf, out: &mut Applied) {
    let Some(node) = container(root, l) else {
        out.notes
            .push(format!("{}: no {} section in this file", l.path, l.section));
        return;
    };
    let at = find_key(node, &l.key);
    let label = match at {
        Some(i) => format!(
            "{}/{}",
            l.section,
            String::from_utf8_lossy(&node.children[i].name)
        ),
        None => format!("{}/{}", l.section, l.key),
    };
    if let Op::Curve(points) = &l.op {
        match at {
            Some(i) => {
                let note = curve_leaf(&mut node.children[i], points);
                out.changed = true;
                out.lines.push(format!("{label}: {note}"));
            }
            None => out
                .notes
                .push(format!("{}: {} has no key {}", l.path, l.section, l.key)),
        }
        return;
    }
    let Some(i) = at else {
        if (l.key.chars().any(|c| c.is_ascii_uppercase()) || l.key.starts_with("m_"))
            && let Op::Set(v) = &l.op
        {
            let like = node
                .children
                .iter()
                .find(|c| c.kind == Kind::Element && c.attr(b"value").is_some());
            node.children.push(new_leaf(like, &l.key, v));
            out.changed = true;
            out.lines.push(format!("{label}: added {v}"));
            return;
        }
        out.notes
            .push(format!("{}: {} has no key {}", l.path, l.section, l.key));
        return;
    };
    let old = node.children[i]
        .attr(b"value")
        .map(|v| String::from_utf8_lossy(v).into_owned());
    let new = match (&l.op, old.as_deref().and_then(|o| o.trim().parse::<f64>().ok())) {
        (Op::Set(v), _) => v.clone(),
        (Op::Scale(f), Some(o)) => fmt_number(o * f),
        (Op::Add(d), Some(o)) => fmt_number(o + d),
        _ => {
            out.notes.push(format!("{}: {label} is not a number", l.path));
            return;
        }
    };
    if old.as_deref() == Some(new.as_str()) {
        return;
    }
    node.children[i].set_attr(b"value", new.as_bytes());
    out.changed = true;
    out.lines
        .push(format!("{label}: {} -> {new}", old.unwrap_or_default()));
}

fn apply_gears(root: &mut Element, ratios: &[f64], up: &[f64], down: &[f64], out: &mut Applied) {
    let Some(box_) = root.children.iter_mut().find(|c| is_el(c, "GearBox")) else {
        out.notes
            .push("GearBox: no such section for gearbox.ratios".into());
        return;
    };
    let n = ratios.len();
    if n == 0 || n > GEARS.len() {
        out.notes
            .push(format!("GearBox: {n} ratios is not 1 to {}", GEARS.len()));
        return;
    }
    let set = |box_: &mut Element, key: String, v: f64, out: &mut Applied| {
        match find_key(box_, &key) {
            Some(i) => box_.children[i].set_attr(b"value", fmt_number(v).as_bytes()),
            None => {
                let like = box_
                    .children
                    .iter()
                    .find(|c| c.kind == Kind::Element && c.attr(b"value").is_some());
                let leaf = new_leaf(like, &key, &fmt_number(v));
                box_.children.push(leaf);
            }
        }
        out.changed = true;
    };
    for (i, g) in GEARS.iter().enumerate() {
        let key = format!("GearRatio{g}");
        if i < n {
            set(box_, key, ratios[i], out);
        } else if find_key(box_, &key).is_some() {
            set(box_, key, 0.0, out);
        }
    }
    for (i, v) in up.iter().take(n - 1).enumerate() {
        set(box_, format!("GearUpSpeedMultiple{}", GEARS[i]), *v, out);
    }
    for (i, v) in down.iter().take(n - 1).enumerate() {
        set(box_, format!("GearDnSpeedMultiple{}", GEARS[i + 1]), *v, out);
    }
    let before = box_.children.len();
    box_.children.retain(|c| {
        if c.kind != Kind::Element {
            return true;
        }
        let name = String::from_utf8_lossy(&c.name);
        for (i, g) in GEARS.iter().enumerate() {
            if name == format!("GearUpSpeedMultiple{g}") {
                return i + 1 < n;
            }
            if name == format!("GearDnSpeedMultiple{g}") {
                return i < n && i >= 1;
            }
        }
        true
    });
    let dropped = before - box_.children.len();
    if dropped > 0 {
        out.changed = true;
    }
    out.lines.push(format!(
        "GearBox: {n} gears, ratios past the top gear 0, {dropped} shift-speed multiple(s) past it removed"
    ));
}

fn own_gears(root: &mut Element, out: &mut Applied) {
    let Some(box_) = root.children.iter_mut().find(|c| is_el(c, "GearBox")) else {
        return;
    };
    match find_key(box_, "AllowGearRatioTuning") {
        Some(i) if box_.children[i].attr(b"value") == Some(&b"false"[..]) => return,
        Some(i) => box_.children[i].set_attr(b"value", b"false"),
        None => {
            let like = box_
                .children
                .iter()
                .find(|c| c.kind == Kind::Element && c.attr(b"value").is_some());
            let leaf = new_leaf(like, "AllowGearRatioTuning", "false");
            box_.children.push(leaf);
        }
    }
    out.changed = true;
    out.lines.push(
        "GearBox: AllowGearRatioTuning false, so the car drives these ratios and not the setup's".into(),
    );
}

pub fn edit_vtf_data(data: &[u8], plan: &Plan) -> Result<Option<(Vec<u8>, Applied)>> {
    let mut doc = Document::parse(data)?;
    let applied = plan.apply(&mut doc.root);
    let bytes = doc.to_bytes();
    if !applied.changed || bytes == data {
        return Ok(None);
    }
    Ok(Some((bytes, applied)))
}

pub fn edit_resource(res: &Resource, plan: &Plan) -> Result<Option<(Resource, Applied)>> {
    let data = res.fragments[0].data()?;
    let Some((bytes, applied)) = edit_vtf_data(&data, plan)? else {
        return Ok(None);
    };
    let mut out = res.clone();
    crate::activation::ers::set_data(&mut out, 0, &bytes)?;
    if let Some(stats) = out.fragments.get(1) {
        let old = stats.data()?;
        let version = old
            .get(4..8)
            .map_or(1, |b| u32::from_le_bytes(b.try_into().expect("4 bytes")));
        let doc = Document::parse(&bytes)?;
        crate::activation::ers::set_data(&mut out, 1, &doc.string_stats(version))?;
    }
    Ok(Some((out, applied)))
}

#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Stock(String),
    Addon { addon: String, path: String },
}

#[derive(Debug, Clone)]
pub struct HandlingMap {
    pub id: String,
    pub target: Target,
    pub plan: Plan,
    pub resources: Option<Vec<String>>,
    pub base: Option<String>,
}

const BASE_KEY: &str = "baseVtfAsset";
const CAR_LAYER: &str = "-carmodel.layer";

fn base_of<'a>(bases: &[(&str, &'a str)], vtf: &str) -> Option<&'a str> {
    bases
        .iter()
        .find(|(name, _)| vtf == *name || vtf.ends_with(name))
        .map(|(_, base)| *base)
}

fn base_layer(data: &[u8], bases: &[(&str, &str)]) -> Result<Option<(Vec<u8>, Vec<String>)>> {
    if !pkg::is_pkg(data) {
        return Ok(None);
    }
    let mut doc = Pkg::parse(data)?;
    let Some(instances) = doc
        .root
        .as_obj_mut()
        .and_then(|o| o.get_mut("instances"))
        .and_then(Node::as_arr_mut)
    else {
        return Ok(None);
    };
    let mut lines = Vec::new();
    for inst in &mut instances.items {
        let Some(physics) = inst
            .as_obj_mut()
            .and_then(|o| o.get_mut("overrides"))
            .and_then(Node::as_obj_mut)
            .and_then(|o| o.get_mut("CarPhysics2"))
            .and_then(Node::as_obj_mut)
        else {
            continue;
        };
        if physics.get(BASE_KEY).is_some() {
            continue;
        }
        let Some(at) = physics
            .items
            .iter()
            .position(|(k, _)| k.as_str() == Some("vtfAsset"))
        else {
            continue;
        };
        let Some(vtf) = physics.items[at]
            .1
            .as_val()
            .and_then(Val::as_str)
            .map(str::to_owned)
        else {
            continue;
        };
        let Some(base) = base_of(bases, &vtf) else {
            continue;
        };
        physics
            .items
            .insert(at + 1, (Val::str(BASE_KEY), Node::Val(Val::str(base))));
        lines.push(format!(
            "CarPhysics2/{BASE_KEY}: {base}, so equal performance drives the season's base car"
        ));
    }
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(Some((doc.to_bytes()?, lines)))
}

pub fn edit_archive(src: &Path, maps: &[&HandlingMap], out: &Path, lines: &mut Vec<String>) -> Result<usize> {
    let file = ErpFile::open(src)?;
    let mut replace = BTreeMap::new();
    let bases: Vec<(&str, &str)> = maps
        .iter()
        .filter_map(|m| Some((m.resources.as_ref()?, m.base.as_deref()?)))
        .flat_map(|(names, base)| names.iter().map(move |n| (n.as_str(), base)))
        .collect();
    for (i, r) in file.erp.resources.iter().enumerate() {
        if r.name.ends_with(CAR_LAYER) && !bases.is_empty() {
            let res = file.load(r)?;
            if let Some((bytes, applied)) = base_layer(&res.fragments[0].data()?, &bases)? {
                let mut new = res.clone();
                crate::activation::ers::set_data(&mut new, 0, &bytes)?;
                let layer = r.name.rsplit('/').next().unwrap_or(&r.name);
                lines.extend(applied.iter().map(|l| format!("handling {layer}: {l}")));
                replace.insert(i, new);
            }
            continue;
        }
        if r.kind() != "VTF" {
            continue;
        }
        let mut res = file.load(r)?;
        let stock = res.fragments[0].data()?;
        let mut changed = false;
        let mut by: Vec<&str> = Vec::new();
        for m in maps {
            if let Some(names) = &m.resources
                && !names.iter().any(|n| r.name.ends_with(n.as_str()))
            {
                continue;
            }
            if let Some((new, applied)) = edit_resource(&res, &m.plan)? {
                let vehicle = r.name.rsplit('/').next().unwrap_or(&r.name).to_owned();
                lines.push(format!(
                    "handling {} -> {vehicle}: {} change(s)",
                    m.id,
                    applied.lines.len()
                ));
                lines.extend(applied.lines.iter().map(|l| format!("handling {vehicle}: {l}")));
                lines.extend(
                    applied
                        .notes
                        .iter()
                        .map(|l| format!("handling {} {vehicle}: {l}", m.id)),
                );
                res = new;
                changed = true;
                by.push(m.id.as_str());
            }
        }
        if changed && by.iter().all(|id| id.ends_with(BASE_ID)) {
            replace.insert(i, res);
        } else if changed {
            let moved = wheel_check::changes_in_data(&stock, &res.fragments[0].data()?).unwrap_or_default();
            let vehicle = r.name.rsplit('/').next().unwrap_or(&r.name);
            lines.extend(
                wheel_check::warning(&by.join(", "), vehicle, &moved)
                    .map(|w| format!("{}{w}", wheel_check::WARNING_PREFIX)),
            );
            replace.insert(i, res);
        }
    }
    let n = replace.len();
    if n == 0 {
        return Ok(0);
    }
    let entries = file
        .erp
        .resources
        .iter()
        .enumerate()
        .map(|(i, r)| match replace.remove(&i) {
            Some(new) => Entry::Owned(new),
            None => Entry::Copy(r),
        })
        .collect();
    let dir = out.parent().expect("has parent");
    std::fs::create_dir_all(dir).map_err(io_err(dir))?;
    file.write(entries, out)?;
    Ok(n)
}

#[derive(Debug, Default)]
pub struct HandlingBuild {
    pub dir: Option<PathBuf>,
    pub replaced: BTreeMap<String, PathBuf>,
    pub lines: Vec<String>,
}

pub fn stamp(maps: &[HandlingMap], sources: &[PathBuf]) -> Result<String> {
    let mut h = <sha2::Sha256 as sha2::Digest>::new();
    for m in maps {
        sha2::Digest::update(
            &mut h,
            format!("{}|{:?}|{:?}|{:?}", m.id, m.target, m.plan, m.base),
        );
    }
    for p in sources {
        let meta = std::fs::metadata(p).map_err(io_err(p))?;
        sha2::Digest::update(
            &mut h,
            format!("{}|{}|{:?}", p.display(), meta.len(), meta.modified().ok()),
        );
    }
    Ok(crate::baseline::hex(&sha2::Digest::finalize(h))[..16].to_owned())
}

pub fn build_stock(
    maps: &[HandlingMap],
    original: &dyn Fn(&str) -> PathBuf,
    cache: &Path,
) -> Result<HandlingBuild> {
    let mut build = HandlingBuild::default();
    let mut by_rel: BTreeMap<&str, Vec<&HandlingMap>> = BTreeMap::new();
    for m in maps {
        if let Target::Stock(rel) = &m.target {
            by_rel.entry(rel.as_str()).or_default().push(m);
        }
    }
    if by_rel.is_empty() {
        return Ok(build);
    }
    let sources: Vec<PathBuf> = by_rel.keys().map(|r| original(r)).collect();
    let stock: Vec<HandlingMap> = maps
        .iter()
        .filter(|m| matches!(m.target, Target::Stock(_)))
        .cloned()
        .collect();
    let out = cache.join(stamp(&stock, &sources)?);
    build.dir = Some(out.clone());
    let log = out.join(".log");
    if !log.is_file() {
        let mut lines = Vec::new();
        for (rel, group) in &by_rel {
            edit_archive(&original(rel), group, &out.join(rel), &mut lines)?;
        }
        std::fs::create_dir_all(&out).map_err(io_err(&out))?;
        std::fs::write(&log, lines.join("\n")).map_err(io_err(&log))?;
    }
    for rel in by_rel.keys() {
        let dst = out.join(rel);
        if dst.is_file() {
            build.replaced.insert((*rel).to_owned(), dst);
        }
    }
    build.lines = std::fs::read_to_string(&log)
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect();
    Ok(build)
}

pub fn merge_patch(base: &mut Value, patch: &Value) {
    let Value::Object(p) = patch else {
        *base = patch.clone();
        return;
    };
    if !base.is_object() {
        *base = Value::Object(Map::new());
    }
    let b = base.as_object_mut().expect("an object");
    for (k, v) in p {
        if v.is_null() && k != "ers" {
            b.remove(k);
        } else if v.is_null() {
            b.insert(k.clone(), Value::Null);
        } else {
            merge_patch(b.entry(k.clone()).or_insert(Value::Null), v);
        }
    }
}

pub type Options<'a> = &'a dyn Fn(&str, &str) -> Option<Value>;

pub struct Files<'a> {
    pub addon: &'a str,
    pub values: Vec<Value>,
}

fn id_of(addon: &str, key: &str) -> String {
    format!("{addon}/{key}")
}

fn matches_ids(entry: &Value, id: &str) -> bool {
    let ids: Vec<&str> = match entry.get("ids") {
        Some(Value::String(s)) => vec![s.as_str()],
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
        _ => return true,
    };
    let except: Vec<&str> = entry
        .get("except")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    if except.contains(&id) {
        return false;
    }
    ids.iter().any(|p| match p.strip_suffix('*') {
        Some(prefix) => id.starts_with(prefix),
        None => *p == id,
    })
}

fn matches_where(entry: &Value, record: &Value) -> bool {
    let Some(w) = entry.get("where").and_then(Value::as_object) else {
        return true;
    };
    w.iter().all(|(field, want)| {
        let have = record
            .get(field)
            .cloned()
            .unwrap_or_else(|| match field.as_str() {
                "class" => Value::String("f1".into()),
                _ => Value::Null,
            });
        match want {
            Value::Array(a) => a.contains(&have),
            other => *other == have,
        }
    })
}

fn option_active(when: &Value, addon: &str, options: Options) -> bool {
    let Some(name) = when.get("option").and_then(Value::as_str) else {
        return false;
    };
    let Some(have) = options(addon, name) else {
        return false;
    };
    if let Some(is) = when.get("is") {
        return *is == have;
    }
    if let Some(Value::Array(any)) = when.get("in") {
        return any.contains(&have);
    }
    if when.get("on").is_some() {
        return have.as_bool() == Some(true);
    }
    if when.get("off").is_some() {
        return have.as_bool() == Some(false);
    }
    false
}

#[derive(Debug, Default)]
pub struct Resolved {
    pub records: BTreeMap<String, Value>,
    pub notes: Vec<String>,
}

pub fn resolve(files: &[Files], options: Options) -> Resolved {
    let mut out = Resolved::default();
    let mut vehicles: Vec<(String, Value)> = Vec::new();
    for f in files {
        let mut selects: Vec<&Value> = Vec::new();
        let mut layers: Vec<(&str, &Value)> = Vec::new();
        for file in &f.values {
            if let Some(Value::Object(m)) = file.get(VEHICLES) {
                for (k, v) in m.iter().filter(|(k, _)| !k.contains('/') && !k.starts_with('$')) {
                    vehicles.push((id_of(f.addon, k), v.clone()));
                }
            }
            let Some(Value::Object(reg)) = file.get(REGISTRY) else {
                continue;
            };
            for (key, value) in reg {
                if key == "$select" {
                    selects.extend(value.as_array().into_iter().flatten());
                } else if key.starts_with('$') {
                } else if key.contains('/') {
                    layers.push((key, value));
                } else {
                    let id = id_of(f.addon, key);
                    let mut record = Value::Object(Map::new());
                    if let Some(base) = value.get("extends").and_then(Value::as_str) {
                        match out.records.get(base) {
                            Some(b) => record = b.clone(),
                            None => out.notes.push(format!(
                                "{id}: extends {base}, which isn't a record this build knows"
                            )),
                        }
                    }
                    let mut v = value.clone();
                    if let Value::Object(m) = &mut v {
                        m.remove("extends");
                        m.remove("$protect");
                    }
                    merge_patch(&mut record, &v);
                    out.records.insert(id, record);
                }
            }
        }
        for entry in selects {
            if entry.get("tag").is_some() || entry.get("in").is_some() {
                out.notes.push(format!(
                    "{}: a $select entry with tag or in isn't resolved at build; skipped",
                    f.addon
                ));
                continue;
            }
            if let Some(when) = entry.get("when")
                && !option_active(when, f.addon, options)
            {
                continue;
            }
            let Some(patch) = entry.get("patch") else {
                continue;
            };
            let ids: Vec<String> = out.records.keys().cloned().collect();
            for id in ids {
                let rec = &out.records[&id];
                if matches_ids(entry, &id) && matches_where(entry, rec) {
                    merge_patch(out.records.get_mut(&id).expect("a record"), patch);
                }
            }
        }
        for (id, patch) in layers {
            let rec = out
                .records
                .entry(id.to_owned())
                .or_insert_with(|| Value::Object(Map::new()));
            merge_patch(rec, patch);
        }
    }
    for (_, v) in vehicles {
        let (Some(group), Some(h)) = (
            v.get("tyre_group")
                .and_then(Value::as_str)
                .filter(|g| !g.is_empty()),
            v.get("handling").and_then(Value::as_str),
        ) else {
            continue;
        };
        if let Some(rec) = out.records.get_mut(h)
            && rec.pointer("/chassis/tyre_physics_group").is_none()
        {
            merge_patch(
                rec,
                &serde_json::json!({ "chassis": { "tyre_physics_group": group } }),
            );
        }
    }
    out
}

pub fn target_of(id: &str, record: &Value) -> Option<(Target, Option<Vec<String>>)> {
    let addon = id.split('/').next().unwrap_or("");
    let resources = record
        .pointer("/vtf/resources")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect());
    match record.get("vtf")? {
        Value::String(p) => Some((
            Target::Addon {
                addon: addon.to_owned(),
                path: p.clone(),
            },
            None,
        )),
        Value::Object(o) => match o.get("stock").and_then(Value::as_str) {
            Some(rel) => Some((Target::Stock(rel.to_owned()), None)),
            None => o.get("erp").and_then(Value::as_str).map(|p| {
                (
                    Target::Addon {
                        addon: addon.to_owned(),
                        path: p.to_owned(),
                    },
                    resources,
                )
            }),
        },
        _ => None,
    }
}

pub fn maps_of(resolved: &Resolved) -> (Vec<HandlingMap>, Vec<String>) {
    let mut maps = Vec::new();
    let mut notes = resolved.notes.clone();
    let mut seen = BTreeSet::new();
    for (id, rec) in &resolved.records {
        let plan = Plan::from_record(rec);
        notes.extend(plan.notes.iter().map(|n| format!("handling {id}: {n}")));
        if plan.is_empty() {
            continue;
        }
        match target_of(id, rec) {
            Some((target, resources)) => {
                seen.insert(id.clone());
                maps.push(HandlingMap {
                    id: id.clone(),
                    target,
                    plan,
                    resources,
                    base: None,
                });
            }
            None => notes.push(format!(
                "handling {id}: no vtf names the handling file to change; nothing applied"
            )),
        }
    }
    (maps, notes)
}

pub fn read_maps(addons: &[(String, PathBuf)], options: Options) -> (Vec<HandlingMap>, Vec<String>) {
    let mut notes = Vec::new();
    let mut files = Vec::new();
    for (id, dir) in addons {
        let mut values = Vec::new();
        for rel in crate::stdlib::data::files(dir) {
            let path = dir.join(&rel);
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if !text.contains(REGISTRY) && !text.contains("\"vehicle\"") {
                continue;
            }
            match serde_json::from_str::<Value>(&text) {
                Ok(v) => values.push(v),
                Err(e) => notes.push(format!("{id}: {rel} isn't JSON ({e}); skipped")),
            }
        }
        if !values.is_empty() {
            files.push((id.as_str(), values));
        }
    }
    let files: Vec<Files> = files
        .into_iter()
        .map(|(addon, values)| Files { addon, values })
        .collect();
    let resolved = resolve(&files, options);
    let (mut maps, more) = maps_of(&resolved);
    notes.extend(more);
    let (base, bases) = base_maps(&resolved, addons, &maps);
    link_bases(&mut maps, &bases);
    maps.extend(base);
    (maps, notes)
}

pub fn link_bases(maps: &mut [HandlingMap], bases: &Bases) {
    for m in maps {
        let Target::Addon { addon, .. } = &m.target else {
            continue;
        };
        m.base = m
            .resources
            .iter()
            .flatten()
            .filter_map(|n| package_of(n))
            .find_map(|p| bases.get(&(addon.clone(), p.to_owned())).cloned());
    }
}

const BASE_VTF: &str = "teams/fom_car/handling/fom_car.vtf";
const BASE_ID: &str = " equal performance";
const NOT_HANDLING: [&str; 3] = ["vtf", "extends", "within"];

fn package_of(resource: &str) -> Option<&str> {
    resource
        .strip_prefix("eaid://")?
        .split('/')
        .next()
        .filter(|p| !p.is_empty())
}

pub type Bases = BTreeMap<(String, String), String>;

pub fn base_maps(
    resolved: &Resolved,
    addons: &[(String, PathBuf)],
    maps: &[HandlingMap],
) -> (Vec<HandlingMap>, Bases) {
    let named: BTreeSet<&str> = maps
        .iter()
        .filter_map(|m| m.resources.as_ref())
        .flatten()
        .map(String::as_str)
        .collect();
    let mut wanted: Vec<(&str, &str, Vec<&Value>)> = Vec::new();
    for (addon, _) in addons {
        let mut by_package: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
        for m in maps {
            if !matches!(&m.target, Target::Addon { addon: a, .. } if a == addon) {
                continue;
            }
            let Some(rec) = resolved.records.get(&m.id) else {
                continue;
            };
            let packages: BTreeSet<&str> = m
                .resources
                .iter()
                .flatten()
                .filter_map(|n| package_of(n))
                .collect();
            for package in packages {
                by_package.entry(package).or_default().push(rec);
            }
        }
        wanted.extend(by_package.into_iter().map(|(p, r)| (addon.as_str(), p, r)));
    }
    let mut out = Vec::new();
    let mut bases = Bases::new();
    let mut made: BTreeSet<String> = BTreeSet::new();
    for own_only in [true, false] {
        for (addon, package, records) in &wanted {
            let key = ((*addon).to_owned(), (*package).to_owned());
            if bases.contains_key(&key) {
                continue;
            }
            let base = format!("eaid://{package}/{BASE_VTF}");
            if named.contains(base.as_str()) || made.contains(&base) {
                bases.insert(key, base);
                continue;
            }
            let Some((owner, path)) = addons
                .iter()
                .filter(|(a, _)| (a.as_str() == *addon) == own_only)
                .find_map(|(a, dir)| archive_with(dir, &base).map(|path| (a.clone(), path)))
            else {
                continue;
            };
            bases.insert(key, base.clone());
            made.insert(base.clone());
            let plan = Plan::from_record(&consensus(records));
            if plan.is_empty() {
                continue;
            }
            out.push(HandlingMap {
                id: format!("{addon}/{package}{BASE_ID}"),
                target: Target::Addon { addon: owner, path },
                plan,
                resources: Some(vec![base]),
                base: None,
            });
        }
    }
    (out, bases)
}

fn archive_with(dir: &Path, resource: &str) -> Option<String> {
    let mut found = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|x| x.eq_ignore_ascii_case("erp")) {
                found.push(path);
            }
        }
    }
    found.sort_by_key(|p| (p.file_name().is_none_or(|n| n != "common.erp"), p.clone()));
    found.into_iter().find_map(|path| {
        let file = ErpFile::open(&path).ok()?;
        if !file.erp.resources.iter().any(|r| r.name == resource) {
            return None;
        }
        let rel = path.strip_prefix(dir).ok()?;
        Some(rel.to_string_lossy().replace('\\', "/"))
    })
}

fn consensus(records: &[&Value]) -> Value {
    let mut out = Map::new();
    let mut keys: BTreeSet<&str> = BTreeSet::new();
    for r in records {
        if let Some(o) = r.as_object() {
            keys.extend(o.keys().map(String::as_str).filter(|k| !NOT_HANDLING.contains(k)));
        }
    }
    for key in keys {
        let values: Vec<&Value> = records.iter().filter_map(|r| r.get(key)).collect();
        if let Some(v) = agree(&values, records.len()) {
            out.insert(key.to_owned(), v);
        }
    }
    Value::Object(out)
}

fn agree(values: &[&Value], total: usize) -> Option<Value> {
    if values.len() * 2 < total {
        return None;
    }
    if values.iter().all(|v| v.is_object()) {
        let mut keys: BTreeSet<&str> = BTreeSet::new();
        for v in values {
            keys.extend(v.as_object().expect("an object").keys().map(String::as_str));
        }
        let mut out = Map::new();
        for key in keys {
            let inner: Vec<&Value> = values.iter().filter_map(|v| v.get(key)).collect();
            if let Some(v) = agree(&inner, total) {
                out.insert(key.to_owned(), v);
            }
        }
        return (!out.is_empty()).then_some(Value::Object(out));
    }
    let mut counts: Vec<(String, usize, &Value)> = Vec::new();
    for v in values {
        let text = v.to_string();
        match counts.iter_mut().find(|(t, _, _)| *t == text) {
            Some((_, n, _)) => *n += 1,
            None => counts.push((text, 1, v)),
        }
    }
    let best = counts.iter().map(|(_, n, _)| *n).max()?;
    counts
        .into_iter()
        .find(|(_, n, _)| *n == best)
        .map(|(_, _, v)| v.clone())
}

pub fn sources(
    maps: &[HandlingMap],
    original: &dyn Fn(&str) -> PathBuf,
    addon_dir: &dyn Fn(&str) -> PathBuf,
) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = maps
        .iter()
        .map(|m| match &m.target {
            Target::Stock(rel) => original(rel),
            Target::Addon { addon, path } => addon_dir(addon).join(path),
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

pub fn for_source<'a>(
    maps: &'a [HandlingMap],
    addon_dir: &dyn Fn(&str) -> PathBuf,
    src: &Path,
) -> Vec<&'a HandlingMap> {
    let same = |a: &Path, b: &Path| {
        a == b || matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
    };
    maps.iter()
        .filter(|m| match &m.target {
            Target::Addon { addon, path } => same(&addon_dir(addon).join(path), src),
            Target::Stock(_) => false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn el(name: &str, attrs: &[(&str, &str)], children: Vec<Element>) -> Element {
        Element {
            kind: Kind::Element,
            pad: 0,
            name: name.as_bytes().to_vec(),
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.as_bytes().to_vec(), v.as_bytes().to_vec()))
                .collect(),
            children,
            end_pad: 0,
        }
    }

    fn lf(name: &str, v: &str) -> Element {
        el(name, &[("value", v)], vec![])
    }

    fn car() -> Element {
        let gears: Vec<Element> = ["1st", "2nd", "3rd", "4th", "5th", "6th", "7th", "8th"]
            .iter()
            .enumerate()
            .map(|(i, g)| lf(&format!("GearRatio{g}"), &format!("{}", 3.5 - i as f64 * 0.3)))
            .collect();
        let ups: Vec<Element> = ["1st", "2nd", "3rd", "4th", "5th", "6th", "7th"]
            .iter()
            .map(|g| lf(&format!("GearUpSpeedMultiple{g}"), "1.0"))
            .collect();
        let dns: Vec<Element> = ["2nd", "3rd", "4th", "5th", "6th", "7th", "8th"]
            .iter()
            .map(|g| lf(&format!("GearDnSpeedMultiple{g}"), "0.9"))
            .collect();
        let mut gb = vec![lf("FinalDriveRatio", "4.78")];
        gb.extend(gears);
        gb.extend(dns);
        gb.extend(ups);
        el(
            "VTF",
            &[],
            vec![
                el("GearBox", &[], gb),
                el(
                    "Chassis",
                    &[],
                    vec![
                        lf("Mass", "771"),
                        lf("COMZ", "0.171"),
                        lf("FFBScale", "1"),
                        lf("TyrePhysicsGroup", "F1_2026"),
                    ],
                ),
                el(
                    "SuspensionSystem",
                    &[],
                    vec![
                        el(
                            "Suspension",
                            &[("name", "Rear")],
                            vec![lf("RestLength", "0.1"), lf("AntiRollConstant", "50000")],
                        ),
                        el(
                            "Suspension",
                            &[("name", "Front")],
                            vec![lf("RestLength", "0.05"), lf("AntiRollConstant", "300000")],
                        ),
                        el(
                            "Tyre",
                            &[("name", "Front")],
                            vec![lf("GripMultiplier", "1.0"), lf("TyreRadius", "0.351")],
                        ),
                    ],
                ),
                el(
                    "EnergyRecoverySystem",
                    &[],
                    vec![el("Common", &[], vec![lf("MaxDeployPower", "126300")])],
                ),
                el(
                    "F1Aero",
                    &[],
                    vec![
                        lf("DRSRotationAngleRadians", "3.857"),
                        lf("DRSOpenCloseSpeed", "5"),
                        lf("DRSFailTimeMin", "300"),
                        lf("m_aero_wing_front_drag", "0.41"),
                        lf("m_aero_wing_front_force", "0.4"),
                        lf("m_aero_wing_rear_drag", "0.246"),
                        lf("m_aero_wing_rear_force", "0.06"),
                        lf("m_aeroSlipStreamAirDensityScaleDrag", "0.8"),
                    ],
                ),
                el(
                    "Upgrade",
                    &[("name", "radUpgradeERS")],
                    vec![el(
                        "EnergyRecoverySystem",
                        &[],
                        vec![el("Common", &[], vec![lf("X", "1")])],
                    )],
                ),
                el(
                    "Upgrade",
                    &[("name", "radUpgradeDRS")],
                    vec![el("F1Aero", &[], vec![lf("m_aero_wing_front_drag", "0.75")])],
                ),
            ],
        )
    }

    fn value(root: &Element, section: &str, key: &str) -> String {
        let s = root.child(section.as_bytes()).unwrap();
        String::from_utf8_lossy(s.child(key.as_bytes()).unwrap().attr(b"value").unwrap()).into_owned()
    }

    #[test]
    fn a_gear_count_zeroes_the_ratios_past_it_and_drops_their_shift_speeds() {
        let record = json!({ "gearbox": { "final_drive_ratio": 3.9, "ratios": [3.0, 2.2, 1.7, 1.3, 1.0, 0.8],
            "up_speed_multiple": [0.8, 0.9, 0.95, 0.96, 0.96], "down_speed_multiple": [0.6, 0.7, 0.8, 0.85, 0.9] } });
        let plan = Plan::from_record(&record);
        let mut root = car();
        let out = plan.apply(&mut root);
        assert!(out.changed, "{:?}", out.notes);
        let g = root.child(b"GearBox").unwrap();
        assert_eq!(value(&root, "GearBox", "FinalDriveRatio"), "3.9");
        assert_eq!(value(&root, "GearBox", "GearRatio6th"), "0.8");
        assert_eq!(value(&root, "GearBox", "GearRatio7th"), "0");
        assert_eq!(value(&root, "GearBox", "GearRatio8th"), "0");
        assert!(g.child(b"GearUpSpeedMultiple5th").is_some());
        assert!(g.child(b"GearUpSpeedMultiple6th").is_none());
        assert!(g.child(b"GearUpSpeedMultiple7th").is_none());
        assert!(g.child(b"GearDnSpeedMultiple6th").is_some());
        assert!(g.child(b"GearDnSpeedMultiple7th").is_none());
        assert!(out.lines.iter().any(|l| l.contains("6 gears")));
        assert_eq!(value(&root, "GearBox", "AllowGearRatioTuning"), "false");
    }

    #[test]
    fn a_final_drive_alone_also_turns_setup_ratios_off() {
        let plan = Plan::from_record(&json!({ "gearbox": { "final_drive_ratio": 4.1 } }));
        let mut root = car();
        let out = plan.apply(&mut root);
        assert!(out.changed, "{:?}", out.notes);
        assert_eq!(value(&root, "GearBox", "FinalDriveRatio"), "4.1");
        assert_eq!(value(&root, "GearBox", "AllowGearRatioTuning"), "false");
        let again = plan.apply(&mut root);
        assert!(!again.lines.iter().any(|l| l.contains("AllowGearRatioTuning")));
    }

    #[test]
    fn a_handling_without_a_gearbox_leaves_setup_ratios_on() {
        let plan = Plan::from_record(&json!({ "chassis": { "mass": 640 } }));
        let mut root = car();
        plan.apply(&mut root);
        assert!(
            root.child(b"GearBox")
                .unwrap()
                .child(b"AllowGearRatioTuning")
                .is_none()
        );
    }

    #[test]
    fn the_base_takes_each_value_most_records_share() {
        let a = json!({ "vtf": "a.erp", "extends": "x", "engine": { "limiter_rotation_rate": 1885 },
            "gearbox": { "ratios": [3.4, 2.2, 1.1] }, "chassis": { "mass": 642, "moi_x": 1 }, "only_here": 1 });
        let b = json!({ "vtf": "b.erp", "engine": { "limiter_rotation_rate": 1885 },
            "gearbox": { "ratios": [3.4, 2.2, 1.1] }, "chassis": { "mass": 642, "moi_x": 2 } });
        let c = json!({ "vtf": "c.erp", "engine": { "limiter_rotation_rate": 1700 },
            "gearbox": { "ratios": [3.0, 2.0] }, "chassis": { "mass": 700, "moi_x": 3 } });
        let base = consensus(&[&a, &b, &c]);
        assert_eq!(
            base,
            json!({ "engine": { "limiter_rotation_rate": 1885 }, "gearbox": { "ratios": [3.4, 2.2, 1.1] },
                "chassis": { "mass": 642, "moi_x": 1 } })
        );
    }

    fn erp_with(names: &[&str]) -> Vec<u8> {
        use swofty_formats::erp::{Erp, Fragment, HEADER_LEN, MAGIC, STORED};
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
                    kind: swofty_formats::erp::kind("VTF").unwrap(),
                    unk: vec![0; 4],
                    fragments: vec![Fragment {
                        name: *b"temp",
                        offset: 0,
                        size: 1,
                        flags: 0x10,
                        compression: STORED,
                        packed: 1,
                        raw: vec![0],
                    }],
                    hash: [0; 16],
                })
                .collect(),
        }
        .to_bytes()
    }

    #[test]
    fn an_addon_package_base_car_gets_the_season_handling() {
        let dir = std::env::temp_dir().join(format!("swofty-handling-base-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("cars/teams")).unwrap();
        let base = "eaid://sw_2013_vehicle_package/teams/fom_car/handling/fom_car.vtf";
        std::fs::write(dir.join("cars/teams/common.erp"), erp_with(&[base])).unwrap();
        let team = |name: &str| {
            let res = format!("eaid://sw_2013_vehicle_package/teams/{name}/handling/{name}.vtf");
            json!({ "vtf": { "erp": format!("cars/teams/{name}/wep/{name}.erp"), "resources": [res] },
                "gearbox": { "ratios": [3.4, 2.2, 1.9, 1.6, 1.4, 1.2, 1.1] } })
        };
        let mut resolved = Resolved::default();
        resolved.records.insert("t.s/redbull".into(), team("redbull"));
        resolved.records.insert("t.s/ferrari".into(), team("ferrari"));
        let (maps, _) = maps_of(&resolved);
        let addons = vec![("t.s".to_owned(), dir.clone())];
        let (extra, bases) = base_maps(&resolved, &addons, &maps);
        assert_eq!(
            bases
                .get(&("t.s".to_owned(), "sw_2013_vehicle_package".to_owned()))
                .map(String::as_str),
            Some(base)
        );
        let mut linked = maps.clone();
        link_bases(&mut linked, &bases);
        assert!(linked.iter().all(|m| m.base.as_deref() == Some(base)));
        assert_eq!(extra.len(), 1);
        assert_eq!(extra[0].id, "t.s/sw_2013_vehicle_package equal performance");
        assert_eq!(
            extra[0].target,
            Target::Addon {
                addon: "t.s".into(),
                path: "cars/teams/common.erp".into()
            }
        );
        assert_eq!(extra[0].resources.as_deref(), Some(&[base.to_owned()][..]));
        assert!(
            extra[0]
                .plan
                .steps
                .iter()
                .any(|s| matches!(s, Step::Gears { ratios, .. } if ratios.len() == 7))
        );

        let mut own = team("redbull");
        own["vtf"] = json!({ "erp": "cars/teams/common.erp", "resources": [base] });
        resolved.records.insert("t.s/base".into(), own);
        let (maps, _) = maps_of(&resolved);
        let (extra, bases) = base_maps(&resolved, &addons, &maps);
        assert!(extra.is_empty());
        assert_eq!(bases.len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_livery_addon_without_a_base_car_uses_its_season_base() {
        let root = std::env::temp_dir().join(format!("swofty-handling-real-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let season = root.join("season");
        let real = root.join("real");
        std::fs::create_dir_all(season.join("cars/teams")).unwrap();
        std::fs::create_dir_all(real.join("cars")).unwrap();
        let base = "eaid://sw_2013_vehicle_package/teams/fom_car/handling/fom_car.vtf";
        std::fs::write(season.join("cars/teams/common.erp"), erp_with(&[base])).unwrap();
        let res = "eaid://sw_2013_vehicle_package/teams/redbull/handling/redbull.vtf";
        let mut resolved = Resolved::default();
        resolved.records.insert(
            "t.real/redbull".into(),
            json!({ "vtf": { "erp": "cars/redbull.erp", "resources": [res] }, "chassis": { "mass": 640 } }),
        );
        resolved.records.insert(
            "t.season/redbull".into(),
            json!({ "vtf": { "erp": "cars/teams/redbull/wep/redbull.erp", "resources": [res] },
                "chassis": { "mass": 642 } }),
        );
        let (mut maps, _) = maps_of(&resolved);
        let addons = vec![("t.real".to_owned(), real), ("t.season".to_owned(), season)];
        let (extra, bases) = base_maps(&resolved, &addons, &maps);
        assert_eq!(extra.len(), 1);
        assert_eq!(extra[0].id, "t.season/sw_2013_vehicle_package equal performance");
        assert_eq!(
            extra[0].target,
            Target::Addon {
                addon: "t.season".into(),
                path: "cars/teams/common.erp".into()
            }
        );
        link_bases(&mut maps, &bases);
        assert!(maps.iter().all(|m| m.base.as_deref() == Some(base)), "{maps:?}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    fn car_layer(physics: Vec<(&str, &str)>) -> Vec<u8> {
        use swofty_formats::pkg::{Arr, Obj};
        let physics = Obj::new(
            physics
                .into_iter()
                .map(|(k, v)| (Val::str(k), Node::Val(Val::str(v))))
                .collect(),
        );
        let overrides = Obj::new(vec![(Val::str("CarPhysics2"), Node::Obj(physics))]);
        let inst = Obj::new(vec![
            (Val::str("name"), Node::Val(Val::str("car"))),
            (Val::str("overrides"), Node::Obj(overrides)),
        ]);
        Pkg {
            name: *b"INST",
            root: Node::Obj(Obj::new(vec![(
                Val::str("instances"),
                Node::Arr(Arr::new(vec![Node::Obj(inst)])),
            )])),
            vca: Vec::new(),
            layout: None,
        }
        .to_bytes()
        .unwrap()
    }

    fn physics_keys(data: &[u8]) -> Vec<(String, String)> {
        let doc = Pkg::parse(data).unwrap();
        let inst = &doc
            .root
            .as_obj()
            .unwrap()
            .get("instances")
            .unwrap()
            .as_arr()
            .unwrap()
            .items[0];
        let physics = inst
            .as_obj()
            .unwrap()
            .get("overrides")
            .unwrap()
            .as_obj()
            .unwrap()
            .get("CarPhysics2")
            .unwrap()
            .as_obj()
            .unwrap();
        physics
            .items
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().unwrap().to_owned(),
                    v.as_val().unwrap().as_str().unwrap().to_owned(),
                )
            })
            .collect()
    }

    #[test]
    fn a_season_car_layer_gets_the_season_base_car() {
        let team = "eaid://sw_2013_vehicle_package/teams/mclaren/handling/mclaren.vtf";
        let base = "eaid://sw_2013_vehicle_package/teams/fom_car/handling/fom_car.vtf";
        let data = car_layer(vec![("vtfAsset", team), ("mechanicsAsset", "m")]);
        let (out, lines) = base_layer(&data, &[(team, base)]).unwrap().unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(
            physics_keys(&out),
            vec![
                ("vtfAsset".to_owned(), team.to_owned()),
                ("baseVtfAsset".to_owned(), base.to_owned()),
                ("mechanicsAsset".to_owned(), "m".to_owned()),
            ]
        );
        assert!(base_layer(&out, &[(team, base)]).unwrap().is_none());
        let other = "eaid://sw_2013_vehicle_package/teams/ferrari/handling/ferrari.vtf";
        assert!(base_layer(&data, &[(other, base)]).unwrap().is_none());
        let own = car_layer(vec![("vtfAsset", team), ("baseVtfAsset", "eaid://x/own.vtf")]);
        assert!(base_layer(&own, &[(team, base)]).unwrap().is_none());
    }

    #[test]
    fn a_null_ers_removes_the_section_and_its_upgrade_parts() {
        let plan = Plan::from_record(&json!({ "ers": null }));
        let mut root = car();
        let out = plan.apply(&mut root);
        assert!(out.changed);
        assert!(root.child(b"EnergyRecoverySystem").is_none());
        let upgrade = root
            .children
            .iter()
            .find(|c| c.attr(b"name") == Some(&b"radUpgradeERS"[..]))
            .unwrap();
        assert!(upgrade.children.is_empty());
        assert!(out.lines.iter().any(|l| l.contains("1 part(s) dropped")));
    }

    #[test]
    fn a_suspension_raise_sets_and_scales_one_axle() {
        let record = json!({
            "suspension": { "front": { "rest_length": 0.08, "tyre_radius": 0.34 }, "rear": { "anti_roll_constant": 60000 } },
            "chassis": { "ffb_scale": 1.3, "tyre_physics_group": "F1_1990" },
            "edits": [ { "path": "suspension.rear.rest_length", "op": "add", "value": 0.03 },
                       { "path": "suspension.front.anti_roll_constant", "op": "scale", "value": 0.85 },
                       { "path": "chassis.mass", "op": "scale", "value": 0.8 } ]
        });
        let plan = Plan::from_record(&record);
        assert!(plan.notes.is_empty(), "{:?}", plan.notes);
        let mut root = car();
        let out = plan.apply(&mut root);
        let sys = root.child(b"SuspensionSystem").unwrap();
        let axle = |name: &str, key: &str, el: &str| -> String {
            let a = sys
                .children
                .iter()
                .find(|c| c.name == el.as_bytes() && c.attr(b"name") == Some(name.as_bytes()))
                .unwrap();
            String::from_utf8_lossy(a.child(key.as_bytes()).unwrap().attr(b"value").unwrap()).into_owned()
        };
        assert_eq!(axle("Front", "RestLength", "Suspension"), "0.08");
        assert_eq!(axle("Front", "TyreRadius", "Tyre"), "0.34");
        assert_eq!(axle("Rear", "RestLength", "Suspension"), "0.13");
        assert_eq!(axle("Rear", "AntiRollConstant", "Suspension"), "60000");
        assert_eq!(axle("Front", "AntiRollConstant", "Suspension"), "255000");
        assert_eq!(value(&root, "Chassis", "FFBScale"), "1.3");
        assert_eq!(value(&root, "Chassis", "Mass"), "616.8");
        assert_eq!(value(&root, "Chassis", "TyrePhysicsGroup"), "F1_1990");
        assert!(
            out.lines
                .iter()
                .any(|l| l.contains("SuspensionSystem/RestLength: 0.1 -> 0.13"))
        );
    }

    #[test]
    fn drs_hardware_off_takes_the_keys_out_and_neutralises_the_wings() {
        let plan = Plan::from_record(&json!({ "f1_aero": { "drs": { "hardware": false },
            "slipstream": { "drag": 0.65 } } }));
        let mut root = car();
        let out = plan.apply(&mut root);
        assert!(out.changed, "{:?}", out.notes);
        let aero = root.child(b"F1Aero").unwrap();
        assert!(aero.child(b"DRSFailTimeMin").is_none());
        assert_eq!(value(&root, "F1Aero", "DRSRotationAngleRadians"), "0.0");
        assert_eq!(value(&root, "F1Aero", "m_aero_wing_front_drag"), "1.0");
        assert_eq!(
            value(&root, "F1Aero", "m_aeroSlipStreamAirDensityScaleDrag"),
            "0.65"
        );
        let drs = root
            .children
            .iter()
            .find(|c| c.attr(b"name") == Some(&b"radUpgradeDRS"[..]))
            .unwrap();
        assert!(drs.children.is_empty());
    }

    #[test]
    fn unknown_keys_and_sections_are_notes_not_errors() {
        let plan = Plan::from_record(&json!({ "nonsense": { "a": 1 }, "chassis": { "no_such_key": 1 },
            "remove": ["no_such_section"] }));
        let mut root = car();
        let out = plan.apply(&mut root);
        assert!(!out.changed);
        assert!(out.notes.iter().any(|n| n.contains("no_such_key")));
        assert!(plan.notes.iter().any(|n| n.contains("no_such_section")));
    }

    #[test]
    fn layers_extends_selects_and_options_resolve_in_load_order() {
        let a = json!({ "vehicle.handling": {
            "base": { "vtf": { "stock": "x/ferrari.erp" }, "chassis": { "mass": 800 } },
            "light": { "extends": "swofty.a/base", "chassis": { "mass": 700 } },
            "$select": [
                { "ids": "swofty.a/*", "when": { "option": "damage", "is": "heavy" }, "patch": { "damage_scale": 1.6 } },
                { "ids": "swofty.a/*", "when": { "option": "damage", "is": "brutal" }, "patch": { "damage_scale": 2.4 } }
            ] } });
        let b =
            json!({ "vehicle.handling": { "swofty.a/base": { "chassis": { "mass": 790 }, "ers": null } } });
        let files = [
            Files {
                addon: "swofty.a",
                values: vec![a],
            },
            Files {
                addon: "other.b",
                values: vec![b],
            },
        ];
        let opts =
            |addon: &str, name: &str| (addon == "swofty.a" && name == "damage").then(|| json!("heavy"));
        let r = resolve(&files, &opts);
        assert_eq!(r.records["swofty.a/base"]["chassis"]["mass"], 790);
        assert_eq!(r.records["swofty.a/base"]["damage_scale"], 1.6);
        assert!(
            r.records["swofty.a/base"]["ers"].is_null() && r.records["swofty.a/base"].get("ers").is_some()
        );
        assert_eq!(r.records["swofty.a/light"]["chassis"]["mass"], 700);
        assert_eq!(r.records["swofty.a/light"]["vtf"]["stock"], "x/ferrari.erp");
        let (maps, notes) = maps_of(&r);
        assert_eq!(maps.len(), 2, "{notes:?}");
        assert_eq!(maps[0].target, Target::Stock("x/ferrari.erp".into()));
    }

    #[test]
    fn a_vehicle_tyre_group_reaches_its_handling_record() {
        let a = json!({
            "vehicle.handling": { "h": { "vtf": "cars/h.erp" } },
            "vehicle": { "car": { "handling": "swofty.a/h", "tyre_group": "F1_1990P" } } });
        let r = resolve(
            &[Files {
                addon: "swofty.a",
                values: vec![a],
            }],
            &|_, _| None,
        );
        assert_eq!(
            r.records["swofty.a/h"]["chassis"]["tyre_physics_group"],
            "F1_1990P"
        );
    }

    const TYPED: &[&str] = &[
        "chassis.mass",
        "chassis.moi_x",
        "chassis.moi_y",
        "chassis.moi_z",
        "chassis.com_y",
        "chassis.com_z",
        "chassis.ffb_scale",
        "chassis.tyre_physics_group",
        "f1_aero.front_coeff_lift_multiplier",
        "f1_aero.rear_coeff_lift_multiplier",
        "f1_aero.body_drag_coeff_min",
        "f1_aero.body_drag_coeff_max",
        "f1_aero.front_coeff_lift_vs_ride_height",
        "f1_aero.rear_coeff_lift_vs_ride_height",
        "f1_aero.slipstream.drag",
        "f1_aero.slipstream.front_wing",
        "f1_aero.slipstream.rear_wing",
        "f1_aero.slipstream.lift",
        "f1_aero.slipstream.car_width",
        "f1_aero.drs.angle",
        "f1_aero.drs.open_close_speed",
        "engine.revs_power_spline",
        "engine.idle_rotation_rate",
        "engine.limiter_rotation_rate",
        "engine.mass",
        "engine.inertia",
        "engine.wear_ice_rate",
        "engine.wear_tc_rate",
        "engine.wear_mguh_rate",
        "engine.wear_ce_rate",
        "fuel_tank.fuel_bsfc_min",
        "fuel_tank.fuel_capacity",
        "gearbox.final_drive_ratio",
        "gearbox.gear_ratio_reverse",
        "gearbox.upshift_clutch_time",
        "gearbox.upshift_gear_delay",
        "gearbox.time_off_throttle_gear_up",
        "gearbox.downshift_clutch_time",
        "gearbox.downshift_gear_delay",
        "gearbox.time_on_throttle_gear_down",
        "gearbox.max_clutch_torque",
        "turbo.max_boost",
        "turbo.mode",
        "differential.rear_power_proportion",
        "differential.diff_rear_max_locking_torque",
        "differential.drive_type",
        "brake_ecu.brake_front",
        "brake_ecu.brake_rear",
        "brake_ecu.brake_front_backwards",
        "brake_ecu.brake_rear_backwards",
        "brake_ecu.brake_front_forwards",
        "brake_ecu.brake_rear_forwards",
        "brake_ecu.brake_migration_torque",
        "brake_ecu.brake_migration_torque_ai",
        "brake_ecu.brake_pressure",
        "tuning.differential_on_throttle_multiple_min",
        "tuning.chassis_com_z_min",
        "tuning.chassis_com_z_max",
        "tuning.engine_braking_min",
        "tuning.differential_high_speed_kph_max",
        "tuning.front_suspension_camber_min",
        "tuning.rear_suspension_toe_max",
        "tuning.front_suspension_rest_length_min",
        "tuning.rear_ride_height_max",
        "tuning.front_suspension_anti_roll_bar_max",
        "tuning.brake_pressure_min",
        "tuning.brake_migration_max",
        "tuning.front_tyre_pressure_min",
        "tuning.rear_tyre_pressure_max",
        "plank.offset_x",
        "plank.offset_y",
        "plank.offset_z",
        "plank.extent_x",
        "plank.extent_y",
        "plank.extent_z",
        "core_aero.slip_streaming",
        "core_aero.slip_stream_while_braking",
        "steering.max_steering_lock",
        "wing_damage.speed_threshold",
        "suspension_damage.speed_threshold",
        "ers.max_deploy_power",
        "ers.energy_store_capacity",
        "ers.max_brake_torque_mguk",
    ];

    const AXLE: &[&str] = &[
        "rest_x",
        "rest_z",
        "rest_length",
        "min_height",
        "max_height",
        "bump_stop_height",
        "ride_height_offset",
        "spring_constant",
        "third_spring_constant",
        "anti_roll_constant",
        "slow_bump_damping",
        "fast_bump_damping",
        "slow_rebound_damping",
        "fast_rebound_damping",
        "bump_stop_rising_spring",
        "bump_stop_rising_damping",
        "grip_multiplier",
        "tyre_radius",
    ];

    fn stock_vtf() -> Option<Vec<u8>> {
        let dir = std::env::var("STATIC_PROOF_DIR").ok()?;
        std::fs::read(Path::new(&dir).join("stock.vtf")).ok()
    }

    fn stock_value(leaf: &Leaf, op: &Op) -> Op {
        match (&leaf.key[..], op) {
            ("TurboMode", _) => Op::Set("Off".into()),
            ("DriveType", _) => Op::Set("RWD".into()),
            (_, op) => op.clone(),
        }
    }

    #[test]
    fn every_typed_field_names_an_element_of_a_stock_vtf() {
        let Some(data) = stock_vtf() else {
            eprintln!("STATIC_PROOF_DIR not set; skipped");
            return;
        };
        let mut paths: Vec<String> = TYPED.iter().map(|s| (*s).to_owned()).collect();
        for side in ["front", "rear"] {
            for f in AXLE {
                paths.push(format!("suspension.{side}.{f}"));
            }
        }
        let mut missing = Vec::new();
        let mut root = Document::parse(&data).unwrap().root;
        for p in &paths {
            let parts: Vec<&str> = p.split('.').collect();
            let op = match p.as_str() {
                "turbo.mode" => Op::Set("off".into()),
                "differential.drive_type" => Op::Set("rwd".into()),
                _ if p.ends_with("vs_ride_height") || p.ends_with("spline") => Op::Curve(vec![(0.0, 0.0)]),
                _ => Op::Scale(1.0),
            };
            let leaves = match resolve_path(&parts, &op) {
                Ok(l) => l,
                Err(e) => {
                    missing.push(format!("{p}: {e}"));
                    continue;
                }
            };
            for l in leaves {
                let l = Leaf {
                    op: stock_value(&l, &l.op),
                    ..l
                };
                let Some(node) = container(&mut root, &l) else {
                    missing.push(format!("{p}: no {} container", l.section));
                    continue;
                };
                if find_key(node, &l.key).is_none() {
                    missing.push(format!("{p}: {}/{} is not in the stock file", l.section, l.key));
                }
            }
        }
        assert!(missing.is_empty(), "{}", missing.join("\n"));
    }

    fn compare(a: &Element, b: &Element, path: &str, out: &mut Vec<String>) {
        let name = String::from_utf8_lossy(&a.name).into_owned();
        let tag = match a.attr(b"name") {
            Some(n) => format!("{path}/{name}@{}", String::from_utf8_lossy(n)),
            None => format!("{path}/{name}"),
        };
        if a.name != b.name {
            out.push(format!(
                "{tag}: element {} vs {}",
                name,
                String::from_utf8_lossy(&b.name)
            ));
            return;
        }
        for (k, v) in &a.attrs {
            match b.attr(k) {
                None => out.push(format!("{tag}: attribute {} missing", String::from_utf8_lossy(k))),
                Some(w) if w == v.as_slice() => {}
                Some(w) => {
                    let (x, y) = (String::from_utf8_lossy(v), String::from_utf8_lossy(w));
                    match (x.trim().parse::<f64>(), y.trim().parse::<f64>()) {
                        (Ok(p), Ok(q)) if (p - q).abs() <= 1e-5 * p.abs().max(q.abs()).max(1e-9) => {}
                        _ => out.push(format!("{tag}: {} {x} vs {y}", String::from_utf8_lossy(k))),
                    }
                }
            }
        }
        if a.attrs.len() != b.attrs.len() {
            out.push(format!("{tag}: attribute count"));
        }
        if a.children.len() != b.children.len() {
            out.push(format!(
                "{tag}: {} children vs {}",
                a.children.len(),
                b.children.len()
            ));
            return;
        }
        for (x, y) in a.children.iter().zip(&b.children) {
            if x.kind == Kind::Element {
                compare(x, y, &tag, out);
            }
        }
    }

    #[test]
    fn a_ferrari_1990_record_reproduces_build_cars_py() {
        let Ok(dir) = std::env::var("STATIC_PROOF_DIR") else {
            eprintln!("STATIC_PROOF_DIR not set; skipped");
            return;
        };
        let dir = PathBuf::from(dir);
        let (Ok(stock), Ok(built), Ok(record)) = (
            std::fs::read(dir.join("stock.vtf")),
            std::fs::read(dir.join("built.vtf")),
            std::fs::read_to_string(dir.join("record.json")),
        ) else {
            eprintln!("no proof files; skipped");
            return;
        };
        let file: Value = serde_json::from_str(&record).unwrap();
        let files = [Files {
            addon: "swofty.f1-1990",
            values: vec![file],
        }];
        let resolved = resolve(&files, &|_, _| None);
        let rec = &resolved.records["swofty.f1-1990/ferrari_1990"];
        let plan = Plan::from_record(rec);
        let mut doc = Document::parse(&stock).unwrap();
        let out = plan.apply(&mut doc.root);
        let want = Document::parse(&built).unwrap();
        let mut diff = Vec::new();
        compare(&want.root, &doc.root, "", &mut diff);
        std::fs::write(dir.join("applied.log"), out.lines.join("\n")).ok();
        std::fs::write(
            dir.join("notes.log"),
            [plan.notes.clone(), out.notes.clone()].concat().join("\n"),
        )
        .ok();
        std::fs::write(dir.join("diff.log"), diff.join("\n")).ok();
        eprintln!(
            "proof: {} steps, {} log lines, {} notes, {} differences",
            plan.steps.len(),
            out.lines.len(),
            plan.notes.len() + out.notes.len(),
            diff.len()
        );
        let allowed = |d: &String| d.contains("ConvexHull") || d.contains("radUpgradeEngine");
        let unexpected: Vec<&String> = diff.iter().filter(|d| !allowed(d)).collect();
        assert!(
            unexpected.is_empty(),
            "{}",
            unexpected
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
