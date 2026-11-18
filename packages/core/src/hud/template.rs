//! The dial template side: binding resolution with typed checks, the `<prefix>_ctl` group with its
//! bound nodes and decision nodes, and the checks against the stock template's own bindings.
//!
//! Binding vocabulary and the stock-tag checks follow Project Halo's `halo_native.py`
//! (`check_binding_types`, `check_decisions`, `_binding_nodes`, `_decision_nodes`); Uses work from
//! Project Halo by nyannoying (https://github.com/nyannoying1337/f1-25-project-halo). The game
//! facts they encode (tags, node patterns) are measurements of the game.

use std::collections::{BTreeMap, BTreeSet};

use swofty_formats::pkg::{Node, Val};

use super::dials::{self, GameValue, Tag, Use};
use super::model::{Bind, Decision, Ty};
use super::scene::Plan;
use super::units::{self, TexFacts};
use crate::ui::node::{self, NodeExt, ObjExt};
use crate::{Error, Result};

fn err(msg: impl std::fmt::Display) -> Error {
    Error::Other(format!("hud: {msg}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Usage {
    Visible,
    Text,
    Fraction,
    Colour,
    Texture,
}

/// A binding resolved to the path the template binds and the tag of the value it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub db: String,
    pub tag: Tag,
    pub negate: bool,
    pub default: bool,
    pub key: String,
    pub leaf: bool,
    /// A colour binding's value while the game's is absent (`default: "#RRGGBBAA"`).
    pub colour: Option<[u8; 4]>,
}

fn leaf_path(name: &str) -> String {
    format!("#{}", name.trim_start_matches('#'))
}

pub fn resolve(b: &Bind, usage: Usage, what: &str) -> Result<Resolved> {
    b.check()?;
    let want: &[Tag] = match usage {
        Usage::Visible => &[*b"bool"],
        Usage::Text => &[*b"stri", *b"ui32", *b"si32"],
        Usage::Fraction => &[*b"fp32"],
        Usage::Colour => &[*b"rgba"],
        Usage::Texture => &[*b"stri"],
    };
    if let Some(s) = &b.screen {
        return Err(err(format!(
            "{what}: a screen value ({s}) is read by a unit's texture only"
        )));
    }
    let colour = match (usage, &b.default) {
        (Usage::Colour, Some(d)) => Some(
            d.as_str()
                .ok_or_else(|| err(format!("{what}: a colour default is a #RRGGBBAA string")))
                .and_then(super::textures::hex_rgba)
                .map_err(|e| err(format!("{what}: {e}")))?,
        ),
        _ => None,
    };
    let from_value = |g: &GameValue, key: String, db: String| -> Result<Resolved> {
        if g.usage == Use::DecisionOnly {
            return Err(err(format!(
                "{what}: {} is read by a decision node only, never bound to a property",
                g.name
            )));
        }
        if !want.contains(&g.tag) {
            return Err(err(format!(
                "{what}: {} is {}, a property here takes {}",
                g.name,
                dials::tag_name(&g.tag),
                want.iter().map(dials::tag_name).collect::<Vec<_>>().join(" or ")
            )));
        }
        Ok(Resolved {
            db,
            tag: g.tag,
            negate: b.negate,
            default: b.default_bool(),
            key,
            leaf: false,
            colour,
        })
    };
    if let Some(l) = &b.view_model {
        // An int leaf is written through Vm_Set_Int (an int64 node), the family the stock template
        // binds as si32; ui32 is the tag of the game's own unsigned values only.
        let tag = match (b.ty, usage) {
            (None | Some(Ty::Bool), Usage::Visible) => *b"bool",
            (None | Some(Ty::Text), Usage::Text | Usage::Texture) => *b"stri",
            (Some(Ty::I32 | Ty::Si32), Usage::Text) => *b"si32",
            (None | Some(Ty::F32), Usage::Fraction) => *b"fp32",
            (Some(Ty::Ui32), _) => {
                return Err(err(format!(
                    "{what}: leaf {l} is declared ui32, the tag of the game's own unsigned values; an \
                     int leaf is i32 (the core writes it as a signed int, bound si32)"
                )));
            }
            (ty, _) => {
                return Err(err(format!(
                    "{what}: leaf {l} is declared {ty:?}, which this binding cannot read"
                )));
            }
        };
        return Ok(Resolved {
            db: leaf_path(l),
            tag,
            negate: b.negate,
            default: b.default_bool(),
            key: l.trim_start_matches('#').to_owned(),
            leaf: true,
            colour,
        });
    }
    if let Some(p) = &b.path {
        let tag = match (b.ty, usage) {
            (None | Some(Ty::Bool), Usage::Visible) => *b"bool",
            (Some(Ty::Text), Usage::Text | Usage::Texture) => *b"stri",
            (None, Usage::Texture) => *b"stri",
            (Some(Ty::I32 | Ty::Si32), Usage::Text) => *b"si32",
            (Some(Ty::Ui32), Usage::Text) => *b"ui32",
            (None | Some(Ty::F32), Usage::Fraction) => *b"fp32",
            (None, Usage::Colour) => *b"rgba",
            (None, Usage::Text) => {
                return Err(err(format!(
                    "{what}: path {p} needs ty (text, si32 or ui32), the tag the game gives the value; a \
                     wrong tag crashes the game, so read it from a stock node that binds the path"
                )));
            }
            (ty, _) => {
                return Err(err(format!(
                    "{what}: path {p} is declared {ty:?}, which this binding cannot read"
                )));
            }
        };
        return Ok(Resolved {
            db: leaf_path(p),
            tag,
            negate: b.negate,
            default: b.default_bool(),
            key: p.trim_start_matches('#').to_owned(),
            leaf: false,
            colour,
        });
    }
    if let Some(g) = &b.game {
        let v = dials::game_value(g).ok_or_else(|| err(format!("{what}: no game value {g}")))?;
        return from_value(v, format!("game.{g}"), v.db.to_owned());
    }
    if let Some(s) = &b.session {
        let v = dials::session_value(s).ok_or_else(|| err(format!("{what}: no session value {s}")))?;
        return from_value(v, format!("session.{}", v.name), v.db.to_owned());
    }
    let field = b.session_car.as_deref().unwrap_or_default();
    let slot = match (b.slot, &b.slot_var) {
        (Some(s), _) => s,
        (None, Some(v)) => v.parse::<u32>().map_err(|_| {
            err(format!(
                "{what}: slot_var {v} was not substituted; slot_var names the var of a repeat the \
                 binding sits in"
            ))
        })?,
        (None, None) => 0,
    };
    if slot > 63 {
        return Err(err(format!("{what}: slot {slot} is past the session's 64 cars")));
    }
    let v = dials::session_car_value(field)
        .ok_or_else(|| err(format!("{what}: no session car value {field}")))?;
    from_value(
        v,
        format!("session_car.{field}.{slot}"),
        v.db.replace("{vid}", &slot.to_string()),
    )
}

pub fn parse_tag(t: &str) -> Tag {
    let mut out = *b"    ";
    out.copy_from_slice(&t.as_bytes()[..4]);
    out
}

fn tags_of(root: &Node, skip: &[&str]) -> BTreeMap<String, BTreeSet<Tag>> {
    let mut all = Vec::new();
    node::bindings(root, skip, &mut all);
    let mut out: BTreeMap<String, BTreeSet<Tag>> = BTreeMap::new();
    for b in all {
        if let Some(t) = b.tag {
            out.entry(b.db).or_default().insert(t);
        }
    }
    out
}

fn subtree_tags(root: &Node, path: &str) -> Result<BTreeMap<String, BTreeSet<Tag>>> {
    let at = node::find_path(root, path)?;
    Ok(tags_of(node::node_at(root, &at)?, &[]))
}

/// What the stock template binds, as `{ path: tags }`, for checking ours against.
pub struct StockTags {
    all: BTreeMap<String, BTreeSet<Tag>>,
    near: BTreeMap<String, BTreeSet<Tag>>,
    fallback: BTreeMap<String, BTreeSet<Tag>>,
}

impl StockTags {
    pub fn race(root: &Node) -> Self {
        Self {
            all: tags_of(root, &[]),
            near: BTreeMap::new(),
            fallback: BTreeMap::new(),
        }
    }

    pub fn spectator(
        root: &Node,
        scope: &str,
        race: &StockTags,
        race_scope: &str,
        race_root: &Node,
    ) -> Result<Self> {
        let near = subtree_tags(root, scope).unwrap_or_default();
        let mut fallback = race.all.clone();
        if let Ok(own) = subtree_tags(race_root, race_scope) {
            fallback.extend(own);
        }
        Ok(Self {
            all: tags_of(root, &[]),
            near,
            fallback,
        })
    }

    pub fn get(&self, db: &str) -> Option<&BTreeSet<Tag>> {
        let key = normalise(db);
        self.near
            .get(&key)
            .or_else(|| self.all.get(&key))
            .or_else(|| self.fallback.get(&key))
    }
}

fn normalise(db: &str) -> String {
    match db.strip_prefix("#session.cars.car") {
        Some(rest) => match rest.split_once('.') {
            Some((_, tail)) => format!("#session.current_car.{tail}"),
            None => db.to_owned(),
        },
        None => db.to_owned(),
    }
}

/// Fails unless every property of `kids` binds a value of the tag the value has. A binding no
/// stock node makes is held to the table; a leaf is held to its declared type, which `resolve`
/// already matched to its use.
pub fn check_types(stock: &StockTags, kids: &[Node], table: &[(String, Tag)]) -> Result<usize> {
    let mut found = Vec::new();
    for k in kids {
        node::bindings(k, &[], &mut found);
    }
    let mut bad = Vec::new();
    let mut checked = 0;
    for b in &found {
        let Some(have) = b.tag else { continue };
        checked += 1;
        if b.db.starts_with("#view_model_data.") {
            continue;
        }
        let known = table
            .iter()
            .find(|(d, _)| normalise(d) == normalise(&b.db))
            .map(|(_, t)| *t);
        let want: BTreeSet<Tag> = match (stock.get(&b.db), known) {
            (Some(s), Some(t)) if !s.contains(&t) => {
                bad.push(format!(
                    "{}.{} <- {}: the table says {}, the stock template binds {}",
                    b.node,
                    b.prop,
                    b.db,
                    dials::tag_name(&t),
                    join_tags(s)
                ));
                continue;
            }
            (Some(s), _) => s.clone(),
            (None, Some(t)) => BTreeSet::from([t]),
            (None, None) => {
                bad.push(format!("{}.{} <- {}: type unknown", b.node, b.prop, b.db));
                continue;
            }
        };
        if !want.contains(&have) {
            bad.push(format!(
                "{}.{} <- {}: {}, the value is {}",
                b.node,
                b.prop,
                b.db,
                dials::tag_name(&have),
                join_tags(&want)
            ));
        }
    }
    if bad.is_empty() {
        Ok(checked)
    } else {
        Err(err(format!("template binding types:\n  {}", bad.join("\n  "))))
    }
}

fn join_tags(s: &BTreeSet<Tag>) -> String {
    s.iter().map(dials::tag_name).collect::<Vec<_>>().join("/")
}

pub fn table_of(plan: &Plan) -> Vec<(String, Tag)> {
    let mut out = Vec::new();
    for g in dials::GAME_VALUES
        .iter()
        .chain(dials::SESSION_VALUES)
        .chain(dials::SESSION_CAR_VALUES)
    {
        out.push((g.db.to_owned(), g.tag));
    }
    out.extend(plan.known.iter().cloned());
    out
}

fn node_type(n: &Node) -> String {
    n.type_code()
}

fn prop_str<'a>(n: &'a Node, key: &str) -> Option<&'a str> {
    n.props()
        .ok()?
        .get(key)?
        .as_obj()?
        .get("value")?
        .as_val()?
        .as_str()
}

fn event_names(n: &Node) -> Vec<String> {
    n.slot(node::EVENTS)
        .map(|evs| {
            evs.iter()
                .filter_map(|e| {
                    e.as_obj()?
                        .get("eventName")?
                        .as_val()?
                        .as_str()
                        .map(str::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The decision node `d` of a plan: its events, from the stock MVCD (every state of it, the ones
/// the record doesn't list hiding everything) or the SVCD's yes and no.
fn decision_node(d: &Decision, plan: &Plan, stock_root: &Node, parent: &str, base: &str) -> Result<Node> {
    let v = dials::game_value(&d.game)
        .ok_or_else(|| err(format!("decision {}: no game value {}", d.id, d.game)))?;
    if v.usage != Use::DecisionOnly {
        return Err(err(format!(
            "decision {}: {} is bound as a property, not decided on",
            d.id, d.game
        )));
    }
    let target = |id: &str| plan.switched_node(id);
    let signals = |show: &BTreeSet<&String>, all: &[&String], ev: &str| -> Result<Vec<Node>> {
        let mut out = Vec::new();
        for id in all {
            let pin = if show.contains(id) { "show" } else { "hide" };
            out.push(node::sig(&format!("{base}.{}", target(id)?), pin, ev));
        }
        Ok(out)
    };
    let name = format!("d_{}", d.id);
    if d.kind == "mvcd" {
        let mut paths = Vec::new();
        node::node_paths_of(stock_root, "", &mut paths);
        let same: Vec<&Node> = paths
            .iter()
            .filter(|(p, n)| {
                p.starts_with(&format!("{parent}."))
                    && node_type(n) == "MVCD"
                    && prop_str(n, "dbEntry") == Some(v.db)
            })
            .map(|(_, n)| *n)
            .collect();
        let [stock] = &same[..] else {
            return Err(err(format!(
                "decision {}: {} stock MVCDs on {} under {parent}",
                d.id,
                same.len(),
                v.db
            )));
        };
        let have = event_names(stock);
        for s in d.states.keys() {
            if !have.contains(s) {
                return Err(err(format!("decision {}: {} has no state {s}", d.id, v.db)));
            }
        }
        let all: Vec<&String> = d
            .states
            .values()
            .flatten()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut events = Vec::new();
        for ev in &have {
            let show: BTreeSet<&String> = d.states.get(ev).map(|s| s.iter().collect()).unwrap_or_default();
            events.push(node::event(ev, signals(&show, &all, ev)?));
        }
        return Ok(node::mvcd_node(&name, v.db, events));
    }
    let tag = parse_tag(d.ty.as_deref().unwrap_or("ui32"));
    if tag != v.tag {
        return Err(err(format!(
            "decision {}: {} is {}, not {}",
            d.id,
            d.game,
            dials::tag_name(&v.tag),
            dials::tag_name(&tag)
        )));
    }
    let stock = subtree_tags(stock_root, parent)?;
    if stock.get(v.db).is_some_and(|t| !t.contains(&tag)) {
        return Err(err(format!(
            "decision {}: the stock dial binds {} with another tag",
            d.id, v.db
        )));
    }
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
        v.db,
        events,
    ))
}

/// Whether a plan's text reads the session clock, so its group needs the formatter.
pub fn needs_clock(plan: &Plan) -> bool {
    let out = plan.clock.as_ref().and_then(|c| c.output.as_deref());
    plan.texts
        .iter()
        .any(|t| t.db.as_deref() == Some(dials::CLOCK_STR) || (out.is_some() && t.db.as_deref() == out))
}

/// The stock session timer's formatter (the one UITF `hud_session_timer_dynamic…
/// time_remaining_formatter`), cloned as `name`: it reads `#session.time_remaining` and writes
/// `time_remaining_str` for the texts of the group it sits in.
pub fn clock_formatter(stock_root: &Node, name: &str) -> Result<Node> {
    let mut paths = Vec::new();
    node::node_paths_of(stock_root, "", &mut paths);
    let found: Vec<&Node> = paths
        .iter()
        .filter(|(p, n)| {
            p.starts_with(&format!("{}.", dials::CLOCK_MODEL))
                && n.name() == dials::CLOCK_FORMATTER
                && node_type(n) == "UITF"
        })
        .map(|(_, n)| *n)
        .collect();
    let [src] = found[..] else {
        return Err(err(format!(
            "{} stock UITF {} under {}",
            found.len(),
            dials::CLOCK_FORMATTER,
            dials::CLOCK_MODEL
        )));
    };
    let mut n = src.clone();
    node::rename(&mut n, name)?;
    Ok(n)
}

/// The clock formatter of a plan with its record's `clock` applied: the clone writes `output` when
/// one is named (and the plan's texts read it), and the signals that re-format it: the group's own
/// `trigger`, and an MVCD on the `start` bool (state `1`) when there is one. Returns the nodes to add
/// and the signals for the group's `trigger` command, both addressed under `base`.
pub fn clock_nodes(stock_root: &Node, plan: &Plan, base: &str) -> Result<(Vec<Node>, Vec<Node>)> {
    let mut fmt = clock_formatter(stock_root, "clock_fmt")?;
    let fmt_path = format!("{base}.clock_fmt");
    let mut nodes = Vec::new();
    let mut sigs = vec![node::sig(&fmt_path, "formatTime", "trigger")];
    if let Some(c) = &plan.clock {
        if let Some(out) = &c.output {
            let p = fmt.props_mut()?;
            p.set("database_output", node::typed_prop(*b"stri", "", out));
        }
        if let Some(start) = &c.start {
            let name = "mv_clock";
            let mv = node::mvcd_node(
                name,
                &start.db,
                vec![
                    node::event("1", vec![node::sig(&fmt_path, "formatTime", "1")]),
                    node::event("0", vec![]),
                ],
            );
            let mv_path = format!("{base}.{name}");
            sigs.push(node::sig(&mv_path, "register", "trigger"));
            sigs.push(node::sig(&mv_path, "query", "trigger"));
            nodes.push(mv);
        }
    }
    nodes.insert(0, fmt);
    Ok((nodes, sigs))
}

/// Adds `sigs` to the group's `trigger` command, making the command when there is none yet.
pub fn add_trigger_signals(grp: &mut Node, sigs: Vec<Node>) -> Result<()> {
    add_command_signals(grp, "trigger", sigs)
}

/// Adds `sigs` to the group's command `name`, making the command when there is none yet, so one
/// group keeps one `trigger` and one `untrigger` however many parts feed them.
pub fn add_command_signals(grp: &mut Node, name: &str, sigs: Vec<Node>) -> Result<()> {
    if sigs.is_empty() {
        return Ok(());
    }
    let cmds = grp.slot_mut(node::COMMANDS)?;
    for c in cmds.iter_mut() {
        let same = c
            .as_obj()
            .and_then(|o| o.get("commandName"))
            .and_then(Node::as_val)
            .and_then(Val::as_str)
            == Some(name);
        if same {
            c.o_mut()?.at_mut("signals")?.a_mut()?.extend(sigs);
            return Ok(());
        }
    }
    cmds.push(node::command(name, sigs));
    Ok(())
}

/// The plan's texts that read the stock clock, pointed at the record's own `clock.output` leaf.
pub fn with_clock_output(plan: &Plan) -> Plan {
    let Some(out) = plan.clock.as_ref().and_then(|c| c.output.as_deref()) else {
        return plan.clone();
    };
    let mut p = plan.clone();
    for t in &mut p.texts {
        if t.db.as_deref() == Some(dials::CLOCK_STR) {
            t.db = Some(out.to_owned());
        }
    }
    p
}

/// The bound nodes of a plan: visibility, colour, texture, text, fraction and switch nodes. A plan
/// with texture items needs `tex`, the stock texture tags of the screen it is built for.
pub fn bound_nodes(plan: &Plan, tex: Option<&TexFacts>) -> Result<Vec<Node>> {
    let mut kids = Vec::new();
    for v in &plan.vis {
        kids.push(node::visible_node(
            &v.name,
            &v.element,
            v.default,
            v.db.as_deref(),
            v.negate,
        ));
    }
    for c in &plan.colours {
        kids.push(node::colour_node(
            &c.name,
            &c.element,
            &c.db,
            c.default.unwrap_or([255; 4]),
        ));
    }
    if !plan.textures.is_empty() {
        let facts = tex.ok_or_else(|| {
            err(format!(
                "{}: texture bindings need the stock HUD template's texture tags",
                plan.ctl_name()
            ))
        })?;
        for t in &plan.textures {
            kids.push(units::texture_node(t, facts)?);
        }
    }
    for t in &plan.texts {
        let visible = t
            .visible
            .as_ref()
            .map(|v| node::bool_prop(v.default, Some(&v.db), v.negate));
        let mut label = node::label_node(&t.name, &t.element, t.tag, t.db.as_deref(), &t.literal, visible);
        if t.localise
            && let Ok(p) = label.props_mut()
        {
            p.set("localise", node::bool_prop(true, None, false));
        }
        kids.push(label);
    }
    for f in &plan.fractions {
        kids.push(node::fraction_node(&f.name, &f.element, &f.db));
    }
    kids.extend(super::motion::player_nodes(plan));
    for s in &plan.shows {
        kids.push(node::group_node(
            &s.name,
            vec![("visible", node::prop(node::boolv(s.default), None))],
            &[&s.element],
            vec![],
        ));
    }
    for c in &plan.statics {
        kids.push(node::static_colour_node(&c.name, &c.element, c.colour));
    }
    Ok(kids)
}

fn tex_facts(plan: &Plan, hud_template: &Node) -> Result<Option<TexFacts>> {
    if plan.textures.is_empty() {
        return Ok(None);
    }
    TexFacts::hud(hud_template).map(Some)
}

/// The `<prefix>_ctl` group of one record's plan: the root's switch, the bound nodes and the
/// decision nodes, with the `trigger` and `untrigger` commands.
pub fn group(plan: &Plan, stock_root: &Node, parent: &str) -> Result<Node> {
    let base = format!("{parent}.{}", plan.ctl_name());
    let plan = &with_clock_output(plan);
    let mut kids = bound_nodes(plan, tex_facts(plan, stock_root)?.as_ref())?;
    let mut trigger_sigs = Vec::new();
    if needs_clock(plan) {
        let (nodes, sigs) = clock_nodes(stock_root, plan, &base)?;
        kids.extend(nodes);
        trigger_sigs = sigs;
    }
    let mut decision_names = Vec::new();
    for d in &plan.decisions {
        let n = decision_node(d, plan, stock_root, parent, &base)?;
        decision_names.push(n.name().to_owned());
        kids.push(n);
    }
    for n in super::motion::trigger_nodes(plan, &base) {
        decision_names.push(n.name().to_owned());
        kids.push(n);
    }
    let names: BTreeSet<&str> = kids.iter().map(|k| k.name()).collect();
    if names.len() != kids.len() {
        return Err(err(format!(
            "{}: a template node name is used twice",
            plan.ctl_name()
        )));
    }
    let mut grp = node::group_node(&plan.ctl_name(), vec![], &[], kids);
    if !decision_names.is_empty() {
        node::decision_commands(&mut grp, &base, &decision_names)?;
    }
    add_trigger_signals(&mut grp, trigger_sigs)?;
    Ok(grp)
}

/// Adds the record's group under `parent`, a dial's `[F1HD]` node, and wires its trigger to the
/// dial's `show` and `hide` commands. The binding types are checked against `stock` first.
pub fn attach(
    template: &mut Node,
    stock_root: &Node,
    stock: &StockTags,
    parent: &str,
    plan: &Plan,
) -> Result<usize> {
    let at = node::find_path(template, parent)?;
    let target = node::node_at_mut(template, &at)?;
    if node_type(target) != "F1HD" {
        return Err(err(format!("{parent} is a {}, not an F1HD", node_type(target))));
    }
    let id = target.props()?.at("id")?.v()?.as_u32();
    if id != Some(swofty_formats::hash::djb2(parent)) {
        return Err(err(format!(
            "{parent}: the node's id is not the hash of its path"
        )));
    }
    let ctl = plan.ctl_name();
    if target
        .slot(node::CHILDREN)?
        .iter()
        .any(|c| c.is_node() && c.name() == ctl)
    {
        return Err(err(format!("{parent} already has a {ctl}")));
    }
    let mut grp = group(plan, stock_root, parent)?;
    let kids: Vec<Node> = grp.slot(node::CHILDREN)?.clone();
    let checked = check_types(stock, &kids, &table_of(plan))?;
    node::fix_ids(&mut grp, Some(parent));
    target.slot_mut(node::CHILDREN)?.push(grp);
    let base = format!("{parent}.{ctl}");
    node::wire_command(target, "show", &base, "trigger")?;
    node::wire_command(target, "hide", &base, "untrigger")?;
    Ok(checked)
}

/// Adds the bound nodes of a scene target's plan straight under `parent`, the template node that
/// instances `scene`, each named `<prefix>_<name>`. The binding types are checked against `stock`.
pub fn attach_scene(
    template: &mut Node,
    stock: &StockTags,
    parent: &str,
    scene: &str,
    plan: &Plan,
) -> Result<usize> {
    let plan = &with_clock_output(plan);
    let mut kids = bound_nodes(plan, tex_facts(plan, template)?.as_ref())?;
    if needs_clock(plan) {
        // a scene target has no commands to register a start node from: the formatter clone only
        let (mut nodes, _) = clock_nodes(template, plan, parent)?;
        kids.extend(nodes.drain(..1));
    }
    let at = node::find_path(template, parent)?;
    let target = node::node_at_mut(template, &at)?;
    let want = scene.strip_suffix(".idf").unwrap_or(scene);
    let have: Vec<String> = target
        .slot(node::COMPONENT)?
        .iter()
        .filter_map(|c| {
            c.as_obj()?
                .get("debugName")?
                .as_val()?
                .as_str()
                .map(str::to_owned)
        })
        .collect();
    if !have.iter().any(|h| h == want) {
        return Err(err(format!(
            "{parent} instances {}, not {scene}; parent_ui is the template node of that scene",
            if have.is_empty() {
                "no scene".to_owned()
            } else {
                have.join(", ")
            }
        )));
    }
    let id = target.props()?.at("id")?.v()?.as_u32();
    if id != Some(swofty_formats::hash::djb2(parent)) {
        return Err(err(format!(
            "{parent}: the node's id is not the hash of its path"
        )));
    }
    for k in &mut kids {
        let name = format!("{}_{}", plan.pfx, k.name());
        node::rename(k, &name).map_err(|e| err(format!("{name}: {e}")))?;
    }
    let mut seen = BTreeSet::new();
    for k in &kids {
        let taken = target
            .slot(node::CHILDREN)?
            .iter()
            .any(|c| c.is_node() && c.name() == k.name());
        if taken || !seen.insert(k.name().to_owned()) {
            return Err(err(format!("{parent} already has a node {}", k.name())));
        }
    }
    let checked = check_types(stock, &kids, &table_of(plan))?;
    for mut k in kids {
        node::fix_ids(&mut k, Some(parent));
        target.slot_mut(node::CHILDREN)?.push(k);
    }
    Ok(checked)
}

pub fn literal_default(b: &Bind) -> Val {
    Val::bool(b.default_bool())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_take_their_tag_from_their_use() {
        let b = Bind::leaf("view_model_data.hud_x_boost");
        let r = resolve(&b, Usage::Visible, "t").unwrap();
        assert_eq!(
            (r.db.as_str(), r.tag, r.leaf),
            ("#view_model_data.hud_x_boost", *b"bool", true)
        );
        assert_eq!(resolve(&b, Usage::Text, "t").unwrap().tag, *b"stri");
        assert_eq!(resolve(&b, Usage::Fraction, "t").unwrap().tag, *b"fp32");
        let mut t = b.clone();
        t.ty = Some(Ty::Text);
        assert!(
            resolve(&t, Usage::Fraction, "t")
                .unwrap_err()
                .to_string()
                .contains("declared")
        );
        let mut i = b.clone();
        i.ty = Some(Ty::I32);
        assert_eq!(resolve(&i, Usage::Text, "t").unwrap().tag, *b"si32");
        assert!(resolve(&i, Usage::Visible, "t").is_err());
        i.ty = Some(Ty::Si32);
        assert_eq!(resolve(&i, Usage::Text, "t").unwrap().tag, *b"si32");
        i.ty = Some(Ty::Ui32);
        let e = resolve(&i, Usage::Text, "t").unwrap_err().to_string();
        assert!(e.contains("ui32"), "{e}");
    }

    #[test]
    fn a_path_binding_carries_the_declared_tag() {
        let p = |path: &str, ty: Option<Ty>| Bind {
            path: Some(path.into()),
            ty,
            ..Bind::default()
        };
        let r = resolve(
            &p("#view_model_data.hud.content.lap_times.pill_0.active", None),
            Usage::Visible,
            "t",
        )
        .unwrap();
        assert_eq!(
            (r.db.as_str(), r.tag, r.leaf),
            (
                "#view_model_data.hud.content.lap_times.pill_0.active",
                *b"bool",
                false
            )
        );
        let e = resolve(&p("#session.number_of_laps", None), Usage::Text, "t")
            .unwrap_err()
            .to_string();
        assert!(e.contains("needs ty"), "{e}");
        assert_eq!(
            resolve(&p("#session.number_of_laps", Some(Ty::Si32)), Usage::Text, "t")
                .unwrap()
                .tag,
            *b"si32"
        );
        assert_eq!(
            resolve(
                &p("#session.current_car.position", Some(Ty::Ui32)),
                Usage::Text,
                "t"
            )
            .unwrap()
            .tag,
            *b"ui32"
        );
        assert_eq!(
            resolve(&p("x.gap_text", Some(Ty::Text)), Usage::Text, "t")
                .unwrap()
                .db,
            "#x.gap_text"
        );
        assert_eq!(
            resolve(&p("x.nub_colour", None), Usage::Colour, "t").unwrap().tag,
            *b"rgba"
        );
        assert_eq!(
            resolve(&p("x.team_badge_texture", None), Usage::Texture, "t")
                .unwrap()
                .tag,
            *b"stri"
        );
        let mut both = p("x.a", None);
        both.view_model = Some("view_model_data.hud_x".into());
        assert!(resolve(&both, Usage::Visible, "t").is_err());
        assert!(resolve(&p("#a..b", None), Usage::Visible, "t").is_err());
    }

    #[test]
    fn game_values_are_held_to_their_tags() {
        let pos = Bind {
            session: Some("current_car.position".into()),
            ..Bind::default()
        };
        assert_eq!(resolve(&pos, Usage::Text, "t").unwrap().tag, *b"ui32");
        assert!(resolve(&pos, Usage::Visible, "t").is_err());
        let revs = Bind::game("revs");
        assert!(resolve(&revs, Usage::Text, "t").is_err());
        assert_eq!(resolve(&revs, Usage::Fraction, "t").unwrap().tag, *b"fp32");
        let mode = Bind::game("ers_mode");
        let e = resolve(&mode, Usage::Visible, "t").unwrap_err().to_string();
        assert!(e.contains("decision node only"), "{e}");
        let car = Bind {
            session_car: Some("position".into()),
            slot: Some(5),
            ..Bind::default()
        };
        assert_eq!(
            resolve(&car, Usage::Text, "t").unwrap().db,
            "#session.cars.car5.position"
        );
        assert!(resolve(&Bind::game("nope"), Usage::Visible, "t").is_err());
    }

    #[test]
    fn textures_colours_slots_and_si32_are_held_to_their_rules() {
        let badge = Bind::leaf("view_model_data.hud_tt_r00_badge");
        let r = resolve(&badge, Usage::Texture, "t").unwrap();
        assert_eq!(
            (r.db.as_str(), r.tag),
            ("#view_model_data.hud_tt_r00_badge", *b"stri")
        );
        let mut i32_badge = badge.clone();
        i32_badge.ty = Some(Ty::I32);
        assert!(resolve(&i32_badge, Usage::Texture, "t").is_err());
        let screen = Bind {
            screen: Some("series_logo".into()),
            ..Bind::default()
        };
        assert!(
            resolve(&screen, Usage::Texture, "t")
                .unwrap_err()
                .to_string()
                .contains("unit's texture")
        );
        let mut bar = Bind {
            session_car: Some("nub_colour".into()),
            slot_var: Some("V".into()),
            default: Some("#00000000".into()),
            ..Bind::default()
        };
        let e = resolve(&bar, Usage::Colour, "bar").unwrap_err().to_string();
        assert!(e.contains("slot_var V was not substituted"), "{e}");
        bar.slot_var = Some("7".into());
        let r = resolve(&bar, Usage::Colour, "bar").unwrap();
        assert_eq!(r.db, "#session.cars.car7.nub_colour");
        assert_eq!(r.colour, Some([0, 0, 0, 0]));
        bar.default = Some("red".into());
        assert!(resolve(&bar, Usage::Colour, "bar").is_err());
        bar.default = Some(serde_json::json!(true));
        assert!(resolve(&bar, Usage::Colour, "bar").is_err());
        bar.default = None;
        bar.slot_var = Some("64".into());
        assert!(resolve(&bar, Usage::Colour, "bar").is_err());
        let code = Bind {
            session_car: Some("driver.name_code".into()),
            slot_var: Some("23".into()),
            ..Bind::default()
        };
        assert_eq!(
            resolve(&code, Usage::Text, "code").unwrap().db,
            "#session.cars.car23.driver.name_code"
        );
        let laps = Bind {
            session: Some("number_of_laps".into()),
            ..Bind::default()
        };
        assert_eq!(resolve(&laps, Usage::Text, "laps").unwrap().tag, *b"si32");
        let mut leaf = Bind::leaf("view_model_data.hud_tt_n");
        leaf.ty = Some(Ty::Si32);
        assert_eq!(resolve(&leaf, Usage::Text, "n").unwrap().tag, *b"si32");
        let clock = Bind {
            session: Some(dials::CLOCK_STR.into()),
            ..Bind::default()
        };
        assert_eq!(
            resolve(&clock, Usage::Text, "clock").unwrap().db,
            "time_remaining_str"
        );
    }

    #[test]
    fn car_paths_normalise_to_the_current_car() {
        assert_eq!(
            normalise("#session.cars.car7.position"),
            "#session.current_car.position"
        );
        assert_eq!(normalise("revs"), "revs");
    }

    #[test]
    fn a_wrongly_tagged_binding_fails_the_type_check() {
        let stock = StockTags {
            all: BTreeMap::from([(
                "#session.current_car.position".to_owned(),
                BTreeSet::from([*b"ui32"]),
            )]),
            near: BTreeMap::new(),
            fallback: BTreeMap::new(),
        };
        let table = vec![("#session.current_car.position".to_owned(), *b"ui32")];
        let ok = node::label_node(
            "a",
            "e",
            *b"ui32",
            Some("#session.current_car.position"),
            "",
            None,
        );
        assert_eq!(check_types(&stock, &[ok], &table).unwrap(), 1);
        let bad = node::label_node(
            "a",
            "e",
            *b"stri",
            Some("#session.current_car.position"),
            "",
            None,
        );
        let e = check_types(&stock, &[bad], &table).unwrap_err().to_string();
        assert!(e.contains("stri, the value is ui32"), "{e}");
        let unknown = node::label_node("a", "e", *b"stri", Some("#session.other"), "", None);
        assert!(check_types(&stock, &[unknown], &table).is_err());
    }
}
