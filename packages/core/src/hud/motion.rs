//! Timed motion on a group: a slide (position keys played as a clip by a `UIXA` node) or a fade
//! (an alpha override by a pair of `UIMO` nodes), started and reversed by an `MVCD` node on a bool
//! leaf of the addon's view model; or a turn (rotation keys played as a clip by a `UIXA` node),
//! moved to one of `steps` places by an `MVCD` node on an int leaf of the addon's view model.
//!
//! The group keeps its own null; its children move into a holder null the motion targets, so a
//! slide moves them in the group's local space and HUD customisation's move, resize and opacity
//! still apply. A fade multiplies the holder's alpha (`BlendType` Multiplicative), so the
//! player's opacity setting holds. The editor copy is built without motion.
//!
//! Model: the stock HUD template's `show_mfd_background` / `hide_mfd_background` `UIMO` pair
//! (Alpha, Multiplicative, Cascade) started by `activateOverride`, its `show_penalty_group`
//! `MVCD` on a bool with events `1` and `0`, and the `UIXA` players `goToFraction` and
//! `playToFraction` reach (`playToFraction` names its curve by the djb2 hash of the curve name).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use swofty_formats::animclip::Channel;
use swofty_formats::hash::djb2;
use swofty_formats::pkg::Node;

use super::geom;
use super::model::{Bind, Element, Kind, Ty};
use super::scene::Plan;
use super::template::{self, Usage};
use crate::ui::node::{self, Attr, NodeExt};
use crate::{Error, Result};

fn err(msg: impl std::fmt::Display) -> Error {
    Error::Other(format!("hud: {msg}"))
}

/// The curve names the stock UI templates play: every `AnimationCurve`, `curveName`, `curve`,
/// `curveOnName` and `curveOffName` value in the game's UI archives. The game knows a curve by the
/// djb2 hash of its name; the curves themselves are the game's own.
pub const STOCK_CURVES: &[&str] = &[
    "broadcast_dotd_close",
    "broadcast_fade_off_track_stats",
    "broadcast_fade_on_track_stats",
    "broadcast_large_fade_off",
    "broadcast_large_fade_on",
    "close_screen_content",
    "close_screen_fade",
    "default_curve_1",
    "fe_pfo_fade_up_curve",
    "full_screen_fade",
    "generic_fade_off",
    "generic_fade_on",
    "generic_fade_on_slow",
    "loading_screen_full_screen_fade",
    "negotiation_slider",
    "open_screen_fade",
    "rwds_xp_sum_bar_diff_change",
    "rwds_xp_sum_bar_fill_change",
    "smart_text_fade",
    "tree_zoom_in",
    "vc_team_bar_slide_in",
];

/// The template properties that name a curve.
pub const CURVE_PROPS: &[&str] = &[
    "AnimationCurve",
    "curveName",
    "curve",
    "curveOnName",
    "curveOffName",
];

pub fn curve_known(name: &str) -> bool {
    STOCK_CURVES.contains(&name)
}

/// Every curve name the nodes under `root` play, by the properties in [`CURVE_PROPS`].
pub fn curves_in(root: &Node) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut paths = Vec::new();
    node::node_paths_of(root, "", &mut paths);
    for (_, n) in paths {
        let Ok(props) = n.props() else { continue };
        for key in CURVE_PROPS {
            let v = props
                .get(key)
                .and_then(|p| p.as_obj().and_then(|o| o.get("value")).or(Some(p)))
                .and_then(Node::as_val)
                .and_then(|v| v.as_str());
            if let Some(v) = v
                && !v.is_empty()
            {
                out.insert(v.to_owned());
            }
        }
    }
    out
}

/// One key: at a time, an alpha or a position offset in HUD units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum KeyValue {
    Alpha(f64),
    Offset([f64; 2]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Track {
    Slide,
    Fade,
    /// Rotation about the group's own place, keys `[time, degrees]` counter-clockwise, moved to
    /// one of `steps` places by an int leaf (a needle).
    Turn,
}

/// The most places a turn's int leaf can name: one `MVCD` event each. The stock UI templates
/// name numbered events up to `30` (0 to 20 in a row); every event is one signal, no node.
pub const MAX_STEPS: u32 = 64;

/// The widest turn between two keys of a clip. A wider span is split into equal keys, so the
/// game's interpolation between neighbouring quaternions never has a way round to choose.
pub const MAX_KEY_TURN: f64 = 90.0;

/// The kinds a turning group may hold: what is drawn by a quad or a text and follows its
/// parent's rotation. Masked and windowed kinds and units are laid out unrotated.
const TURNS_WITH: &[Kind] = &[Kind::Sprite, Kind::Text, Kind::Group, Kind::Stack, Kind::Centre];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Motion {
    pub id: String,
    pub keys: Vec<(f64, KeyValue)>,
    pub curve: String,
    /// `slide`, `fade` or `turn`; slide and fade are also told by their keys, a turn is named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play_on: Option<Bind>,
    /// A turn's int leaf: value `i` (0 to `steps - 1`) turns the group to `i / (steps - 1)` of
    /// its keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub play_to: Option<Bind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps: Option<u32>,
}

impl Motion {
    pub fn track(&self) -> Result<Track> {
        if self.kind == Some(Track::Turn) {
            if self.keys.iter().any(|(_, k)| matches!(k, KeyValue::Offset(_))) {
                return Err(err(format!(
                    "motion {}: a turn's keys are [time, degrees], not position offsets",
                    self.id
                )));
            }
            return Ok(Track::Turn);
        }
        let mut track = None;
        for (_, k) in &self.keys {
            let t = match k {
                KeyValue::Alpha(_) => Track::Fade,
                KeyValue::Offset(_) => Track::Slide,
            };
            if track.is_some_and(|have| have != t) {
                return Err(err(format!(
                    "motion {}: its keys mix alpha values and position offsets",
                    self.id
                )));
            }
            track = Some(t);
        }
        let track = track.ok_or_else(|| err(format!("motion {}: no keys", self.id)))?;
        if let Some(k) = self.kind
            && k != track
        {
            return Err(err(format!(
                "motion {}: kind {k:?} but its keys make a {track:?}",
                self.id
            )));
        }
        Ok(track)
    }

    pub fn check(&self, what: &str) -> Result<()> {
        let ok_id = self.id.len() <= 16
            && self.id.starts_with(|c: char| c.is_ascii_lowercase())
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !ok_id {
            return Err(err(format!(
                "{what}: a motion id is lower case letters, digits and _, 16 at most, not {:?}",
                self.id
            )));
        }
        if self.keys.len() < 2 {
            return Err(err(format!("{what}: motion {} needs two keys or more", self.id)));
        }
        if self.keys[0].0 != 0.0 {
            return Err(err(format!("{what}: motion {} starts at time 0", self.id)));
        }
        for w in self.keys.windows(2) {
            if w[1].0 <= w[0].0 {
                return Err(err(format!(
                    "{what}: motion {}: key times go up, {} does not follow {}",
                    self.id, w[1].0, w[0].0
                )));
            }
        }
        let track = self.track()?;
        if track == Track::Fade {
            if self.keys.len() != 2 {
                return Err(err(format!(
                    "{what}: motion {} fades alpha, which takes exactly two keys (from, to); the curve times it",
                    self.id
                )));
            }
            for (_, k) in &self.keys {
                if let KeyValue::Alpha(a) = k
                    && !(0.0..=1.0).contains(a)
                {
                    return Err(err(format!(
                        "{what}: motion {}: alpha {a} is outside 0..1",
                        self.id
                    )));
                }
            }
        }
        if !curve_known(&self.curve) {
            return Err(err(format!(
                "{what}: motion {}: {} is not a curve the stock UI templates play ({})",
                self.id,
                self.curve,
                STOCK_CURVES.join(", ")
            )));
        }
        if track == Track::Turn {
            return self.check_turn(what);
        }
        if self.play_to.is_some() || self.steps.is_some() {
            return Err(err(format!(
                "{what}: motion {}: play_to and steps go with a turn; a {} is started by play_on",
                self.id,
                if track == Track::Fade { "fade" } else { "slide" }
            )));
        }
        let on = self.play_on.as_ref().ok_or_else(|| {
            err(format!(
                "{what}: motion {} needs play_on, the bool leaf that starts it",
                self.id
            ))
        })?;
        on.check()?;
        if on.view_model.is_none() || on.negate || on.default.is_some() {
            return Err(err(format!(
                "{what}: motion {}: play_on names a view_model bool leaf, without not or default",
                self.id
            )));
        }
        Ok(())
    }

    fn check_turn(&self, what: &str) -> Result<()> {
        if self.play_on.is_some() {
            return Err(err(format!(
                "{what}: motion {}: a turn is moved by play_to, an int leaf, not started by play_on",
                self.id
            )));
        }
        let steps = self.steps.ok_or_else(|| {
            err(format!(
                "{what}: motion {} needs steps, the number of places its int leaf names",
                self.id
            ))
        })?;
        if !(2..=MAX_STEPS).contains(&steps) {
            return Err(err(format!(
                "{what}: motion {}: steps is 2 to {MAX_STEPS}, not {steps}",
                self.id
            )));
        }
        let to = self.play_to.as_ref().ok_or_else(|| {
            err(format!(
                "{what}: motion {} needs play_to, the int leaf that moves it",
                self.id
            ))
        })?;
        to.check()?;
        if to.view_model.is_none() || to.negate || to.default.is_some() {
            return Err(err(format!(
                "{what}: motion {}: play_to names a view_model int leaf, without not or default",
                self.id
            )));
        }
        if !matches!(to.ty, None | Some(Ty::I32 | Ty::Si32)) {
            return Err(err(format!(
                "{what}: motion {}: play_to is an int leaf (ty i32), not {:?}",
                self.id, to.ty
            )));
        }
        Ok(())
    }

    /// A turn's int leaf as the template binds it: declared i32, so it resolves to si32, the tag
    /// the core's int leaves are bound with.
    pub fn play_to_bind(&self) -> Option<Bind> {
        self.play_to.clone().map(|mut b| {
            b.ty = Some(Ty::I32);
            b
        })
    }

    /// The turn's keys as `(time, degrees)`, a span wider than [`MAX_KEY_TURN`] split into equal
    /// keys.
    pub fn turn_keys(&self) -> Vec<(f64, f64)> {
        let mut out: Vec<(f64, f64)> = Vec::new();
        for (t, k) in &self.keys {
            let d = match k {
                KeyValue::Alpha(d) => *d,
                KeyValue::Offset(_) => 0.0,
            };
            if let Some(&(t0, d0)) = out.last() {
                let parts = ((d - d0).abs() / MAX_KEY_TURN).ceil().max(1.0) as usize;
                for j in 1..parts {
                    let f = j as f64 / parts as f64;
                    out.push((t0 + (t - t0) * f, d0 + (d - d0) * f));
                }
            }
            out.push((*t, d));
        }
        out
    }

    /// The fade's alpha from and to.
    pub fn alphas(&self) -> (f64, f64) {
        let a = |k: &KeyValue| match k {
            KeyValue::Alpha(a) => *a,
            KeyValue::Offset(_) => 1.0,
        };
        (
            self.keys.first().map(|(_, k)| a(k)).unwrap_or(1.0),
            self.keys.last().map(|(_, k)| a(k)).unwrap_or(1.0),
        )
    }

    /// The slide's clip channels: the holder's position keys in scene units, and nothing else. The
    /// game evaluates a quaternion channel with aligned 16-byte loads straight from the clip's
    /// `syst` buffer, which it does not always place on 16 bytes, so a clip carries no rotation it
    /// does not need (the stock loading spinners are position-only clips too).
    ///
    /// A turn's clip is its rotation keys only, two or more (a one-key rotation is the load the
    /// game faulted on): the holder sits at the group's place with no scale, so it needs no
    /// position or scale channel. Each quaternion is [`geom::quat_z`] of its angle, signed to the
    /// same side as the key before it, so the turn between two keys is the short one whichever way
    /// the game interpolates.
    pub fn channels(&self) -> Vec<Channel> {
        if self.kind == Some(Track::Turn) {
            let keys = self.turn_keys();
            let times: Vec<f32> = keys.iter().map(|(t, _)| *t as f32).collect();
            let mut quats: Vec<[f64; 4]> = Vec::with_capacity(keys.len());
            for (_, d) in &keys {
                let mut q = geom::quat_z(*d);
                if let Some(p) = quats.last()
                    && p.iter().zip(&q).map(|(a, b)| a * b).sum::<f64>() < 0.0
                {
                    q = q.map(|x| -x);
                }
                quats.push(q);
            }
            let quats = quats.iter().map(|q| q.map(|v| v as f32)).collect();
            return vec![Channel::rotation(times, quats)];
        }
        let times: Vec<f32> = self.keys.iter().map(|(t, _)| *t as f32).collect();
        let keys: Vec<[f32; 3]> = self
            .keys
            .iter()
            .map(|(_, k)| match k {
                KeyValue::Offset([u, v]) => {
                    let p = geom::scene_point(*u, *v);
                    [p[0] as f32, p[1] as f32, 0.0]
                }
                KeyValue::Alpha(_) => [0.0; 3],
            })
            .collect();
        vec![Channel::position(times, keys)]
    }
}

/// Checks every motion in `list`: on a group only, never on a stack's child (the stacker lays the
/// child out itself), and well formed.
pub fn check_elements(list: &[Element], parent: Option<Kind>) -> Result<()> {
    for e in list {
        if let Some(m) = &e.motion {
            let what = format!("element {}", e.id);
            if e.kind != Some(Kind::Group) {
                return Err(err(format!(
                    "{what}: a motion goes on a group, not a {:?}",
                    e.kind
                )));
            }
            if parent == Some(Kind::Stack) {
                return Err(err(format!(
                    "{what}: a stack lays its children out itself, so a child of a stack takes no motion; wrap the moving group in a group"
                )));
            }
            m.check(&what)?;
            if m.track()? == Track::Turn {
                turning_kinds(&e.children, &what)?;
            }
        }
        check_elements(&e.children, e.kind)?;
    }
    Ok(())
}

fn turning_kinds(list: &[Element], what: &str) -> Result<()> {
    for c in list {
        if let Some(k) = c.kind
            && !TURNS_WITH.contains(&k)
        {
            return Err(err(format!(
                "{what}: a turning group holds sprites, texts and groups; {} is a {k:?}, which is laid out unrotated",
                c.id
            )));
        }
        turning_kinds(&c.children, what)?;
    }
    Ok(())
}

/// The name of the holder null a motion targets, in the scene's anim string table order
/// (`zh_` sorts after every stock name).
pub fn holder_name(element: &str, m: &Motion) -> String {
    format!("zh_{element}_{}", m.id)
}

/// What one motion put in a scene, for the template.
#[derive(Debug, Clone)]
pub struct MotionItem {
    pub id: String,
    pub mid: String,
    pub element: String,
    pub track: Track,
    pub curve: String,
    pub from: f64,
    pub to: f64,
    pub db: String,
    /// A turn's places (events `0` to `steps - 1`); 0 for a slide or fade.
    pub steps: u32,
}

impl MotionItem {
    pub fn player(&self) -> String {
        format!("m_{}_{}", self.id, self.mid)
    }

    pub fn trigger(&self) -> String {
        format!("mv_{}_{}", self.id, self.mid)
    }
}

/// Records a live group's motion in its plan: the clip when it slides, the item the template
/// nodes come from, and the leaf's type.
pub fn plan(plan: &mut Plan, e: &Element, m: &Motion, holder: &str) -> Result<()> {
    let track = m.track()?;
    let what = format!("element {} motion {}", e.id, m.id);
    let r = if track == Track::Turn {
        let to = m
            .play_to_bind()
            .ok_or_else(|| err(format!("motion {}: no play_to", m.id)))?;
        template::resolve(&to, Usage::Text, &what)?
    } else {
        let on = m
            .play_on
            .as_ref()
            .ok_or_else(|| err(format!("motion {}: no play_on", m.id)))?;
        template::resolve(on, Usage::Visible, &what)?
    };
    plan.known.push((r.db.clone(), r.tag));
    if track != Track::Fade {
        plan.clips.push(super::scene::ClipSpec {
            node: holder.to_owned(),
            channels: m.channels(),
        });
    }
    let (from, to) = m.alphas();
    plan.motions.push(MotionItem {
        id: e.id.clone(),
        mid: m.id.clone(),
        element: holder.to_owned(),
        track,
        curve: m.curve.clone(),
        from,
        to,
        db: r.db,
        steps: if track == Track::Turn {
            m.steps.unwrap_or(0)
        } else {
            0
        },
    });
    Ok(())
}

fn alpha_node(name: &str, element: &str, curve: &str, from: f64, to: f64) -> Node {
    node::node(
        name,
        "UIMO",
        vec![
            ("Cascade", node::prop(node::boolv(true), None)),
            ("ParameterName", node::prop(node::s("Alpha"), None)),
            ("BlendType", node::prop(node::s("Multiplicative"), None)),
            ("Value", node::prop(node::f32v(to as f32), None)),
            ("StartingValue", node::prop(node::f32v(from as f32), None)),
            ("AnimationCurve", node::prop(node::s(curve), None)),
        ],
        vec![],
        &[element],
        &["NBEL", "NBMG", "NBOS", "NBXA"],
    )
}

/// The player nodes of a plan's motions: a `UIXA` over the holder for a slide, a `UIMO` pair
/// (`_on` plays from to to, `_off` back) for a fade.
pub fn player_nodes(plan: &Plan) -> Vec<Node> {
    let mut out = Vec::new();
    for m in &plan.motions {
        let name = m.player();
        match m.track {
            Track::Slide | Track::Turn => out.push(node::node(
                &name,
                "UIXA",
                vec![],
                vec![],
                &[&m.element],
                node::FRACTION_BEHAVIOURS,
            )),
            Track::Fade => {
                out.push(alpha_node(
                    &format!("{name}_on"),
                    &m.element,
                    &m.curve,
                    m.from,
                    m.to,
                ));
                out.push(alpha_node(
                    &format!("{name}_off"),
                    &m.element,
                    &m.curve,
                    m.to,
                    m.from,
                ));
            }
        }
    }
    out
}

/// The trigger nodes of a plan's motions: an `MVCD` on the leaf whose event `1` starts the motion
/// and `0` reverses it, or for a turn one event per place `i` playing to `i / (steps - 1)`; `base`
/// is the path of the node the players sit under.
pub fn trigger_nodes(plan: &Plan, base: &str) -> Vec<Node> {
    let mut out = Vec::new();
    for m in &plan.motions {
        let player = format!("{base}.{}", m.player());
        let to = |ev: &str, value: f32| {
            node::event(
                ev,
                vec![node::sig_with(
                    &player,
                    "playToFraction",
                    ev,
                    &[
                        ("curveName", Attr::U32(djb2(&m.curve))),
                        ("value", Attr::F32(value)),
                    ],
                )],
            )
        };
        let events = match m.track {
            Track::Turn => {
                let last = m.steps.saturating_sub(1).max(1);
                (0..m.steps)
                    .map(|i| to(&i.to_string(), i as f32 / last as f32))
                    .collect()
            }
            Track::Slide => vec![to("1", 1.0), to("0", 0.0)],
            Track::Fade => {
                let act = |ev: &str, which: &str| {
                    node::event(
                        ev,
                        vec![node::sig_with(
                            &format!("{player}_{which}"),
                            "activateOverride",
                            ev,
                            &[("overrideInstant", Attr::Bool(false))],
                        )],
                    )
                };
                vec![act("1", "on"), act("0", "off")]
            }
        };
        out.push(node::mvcd_node(&m.trigger(), &m.db, events));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use swofty_formats::animclip::{SYST_ALIGN, Target, build_clip, has_constant_rotation, read_clip};

    fn clip_template() -> Vec<u8> {
        use swofty_formats::pkg::{Arr, Obj, Pkg, Val};
        let raw = |ty: &[u8; 4], n: usize| Node::Val(Val::raw(*ty, vec![0u8; n]));
        let channel = Node::Obj(Obj::new(vec![
            (Val::str("name_index"), raw(b"si32", 4)),
            (Val::str("mins"), raw(b"ui64", 8)),
            (Val::str("misc"), raw(b"ui64", 8)),
            (Val::str("name"), Node::Val(Val::str("x"))),
            (Val::str("keyptr"), raw(b"ui32", 4)),
            (Val::str("timesptr"), raw(b"ui32", 4)),
        ]));
        let packet = Node::Obj(Obj::new(vec![
            (Val::str("maxtime"), raw(b"fp32", 4)),
            (Val::str("mintime"), raw(b"fp32", 4)),
            (Val::str("interpolator"), raw(b"ui32", 4)),
            (Val::str("channels"), Node::Arr(Arr::new(vec![channel]))),
        ]));
        Pkg {
            name: *b"ANIM",
            root: Node::Obj(Obj::new(vec![
                (Val::str("packets"), Node::Arr(Arr::new(vec![packet]))),
                (Val::str("string_table"), Node::Val(Val::str("t"))),
            ])),
            vca: vec![],
            layout: None,
        }
        .to_bytes()
        .unwrap()
    }

    fn motion(v: serde_json::Value) -> Motion {
        serde_json::from_value(v).unwrap()
    }

    fn slide() -> Motion {
        motion(serde_json::json!({
            "id": "slide", "keys": [[0, [0, 0]], [0.75, [0, -38.94]]], "curve": "generic_fade_on",
            "play_on": { "view_model": "view_model_data.hud_t_r00_swap" }
        }))
    }

    fn fade() -> Motion {
        motion(serde_json::json!({
            "id": "in", "keys": [[0, 0], [0.15, 1]], "curve": "generic_fade_on_slow",
            "play_on": { "view_model": "view_model_data.hud_t_arrow" }
        }))
    }

    #[test]
    fn keys_tell_a_slide_from_a_fade() {
        assert_eq!(slide().track().unwrap(), Track::Slide);
        assert_eq!(fade().track().unwrap(), Track::Fade);
        assert_eq!(fade().alphas(), (0.0, 1.0));
        slide().check("t").unwrap();
        fade().check("t").unwrap();
    }

    #[test]
    fn a_slide_round_trips_through_a_clip() {
        let channels = slide().channels();
        assert_eq!(channels[0].times, vec![0.0, 0.75]);
        assert_eq!(channels[0].max_time, 0.75);
        let dy = (geom::K * 38.94) as f32;
        assert_eq!(channels[0].keys, vec![0.0, 0.0, 0.0, 0.0, dy, 0.0]);
        let template = clip_template();
        let (temp, syst) = build_clip(&template, "zh_t_rows_slide", 9, "tbl", &channels).unwrap();
        let clip = read_clip(&temp, &syst).unwrap();
        assert_eq!(clip.node, "zh_t_rows_slide");
        assert_eq!(clip.channels, channels);
    }

    #[test]
    fn a_slide_clip_carries_no_rotation() {
        let channels = slide().channels();
        assert_eq!(channels.len(), 1);
        assert!(channels.iter().all(|c| c.target == Target::Position));
    }

    #[test]
    fn refusals() {
        let bad = |f: fn(&mut serde_json::Value)| {
            let mut v = serde_json::json!({
                "id": "slide", "keys": [[0, [0, 0]], [0.75, [0, -38.94]]], "curve": "generic_fade_on",
                "play_on": { "view_model": "view_model_data.hud_t_x" }
            });
            f(&mut v);
            let m: Motion = serde_json::from_value(v).unwrap();
            m.check("t").unwrap_err().to_string()
        };
        assert!(bad(|v| v["curve"] = "rwds_rival_state_open".into()).contains("not a curve"));
        assert!(bad(|v| v["keys"] = serde_json::json!([[0, [0, 0]]])).contains("two keys"));
        assert!(bad(|v| v["keys"] = serde_json::json!([[0, [0, 0]], [0.5, 1]])).contains("mix"));
        assert!(bad(|v| v["keys"] = serde_json::json!([[0.1, [0, 0]], [0.5, [1, 1]]])).contains("time 0"));
        assert!(bad(|v| v["keys"] = serde_json::json!([[0, [0, 0]], [0, [1, 1]]])).contains("go up"));
        assert!(
            bad(|v| v["keys"] = serde_json::json!([[0, 0], [0.1, 0.5], [0.2, 1]])).contains("exactly two")
        );
        assert!(bad(|v| v["keys"] = serde_json::json!([[0, 0], [0.1, 2]])).contains("outside"));
        assert!(bad(|v| v["id"] = "Slide".into()).contains("motion id"));
        assert!(
            bad(|v| v.as_object_mut().unwrap().remove("play_on").map(|_| ()).unwrap()).contains("play_on")
        );
        assert!(
            bad(|v| v["play_on"] = serde_json::json!({ "game": "drs_pit_state" })).contains("view_model")
        );
        assert!(bad(|v| v["play_on"]["not"] = true.into()).contains("without not"));
    }

    #[test]
    fn a_motion_goes_on_a_group_outside_a_stack() {
        let parse = |v: serde_json::Value| -> Vec<Element> { serde_json::from_value(v).unwrap() };
        let m = serde_json::json!({ "id": "s", "keys": [[0, [0, 0]], [0.5, [0, 4]]], "curve": "generic_fade_on",
            "play_on": { "view_model": "view_model_data.hud_t_go" } });
        let ok = parse(serde_json::json!([{ "id": "g", "kind": "group", "motion": m, "children": [] }]));
        check_elements(&ok, None).unwrap();
        let text = parse(serde_json::json!([{ "id": "t", "kind": "text", "motion": m }]));
        assert!(
            check_elements(&text, None)
                .unwrap_err()
                .to_string()
                .contains("on a group")
        );
        let stacked = parse(serde_json::json!([{ "id": "s", "kind": "stack", "children": [
            { "id": "g", "kind": "group", "motion": m, "children": [] } ] }]));
        assert!(
            check_elements(&stacked, None)
                .unwrap_err()
                .to_string()
                .contains("stack")
        );
        let wrapped = parse(serde_json::json!([{ "id": "s", "kind": "stack", "children": [
            { "id": "w", "kind": "group", "children": [ { "id": "g", "kind": "group", "motion": m, "children": [] } ] } ] }]));
        check_elements(&wrapped, None).unwrap();
    }

    #[test]
    fn players_and_triggers_take_the_stock_shapes() {
        let mut p = Plan::default();
        p.motions.push(MotionItem {
            id: "rows".into(),
            mid: "slide".into(),
            element: "zh_t_rows_slide".into(),
            track: Track::Slide,
            curve: "generic_fade_on".into(),
            from: 1.0,
            to: 1.0,
            db: "#view_model_data.hud_t_swap".into(),
            steps: 0,
        });
        p.motions.push(MotionItem {
            id: "up".into(),
            mid: "in".into(),
            element: "zh_t_up_in".into(),
            track: Track::Fade,
            curve: "generic_fade_on_slow".into(),
            from: 0.0,
            to: 1.0,
            db: "#view_model_data.hud_t_arrow".into(),
            steps: 0,
        });
        let players = player_nodes(&p);
        let types: Vec<(String, String)> = players
            .iter()
            .map(|n| (n.name().to_owned(), n.type_code()))
            .collect();
        assert_eq!(
            types,
            [
                ("m_rows_slide".to_owned(), "UIXA".to_owned()),
                ("m_up_in_on".to_owned(), "UIMO".to_owned()),
                ("m_up_in_off".to_owned(), "UIMO".to_owned()),
            ]
        );
        let on = node::canonical(&players[1]);
        assert!(on.contains("Alpha") && on.contains("Multiplicative") && on.contains("generic_fade_on_slow"));
        let triggers = trigger_nodes(&p, "hud.x_ctl");
        assert_eq!(triggers.len(), 2);
        assert_eq!(triggers[0].type_code(), "MVCD");
        let t = node::canonical(&triggers[0]);
        assert!(t.contains("hud.x_ctl.m_rows_slide") && t.contains("playToFraction"));
        let curve = triggers[0].slot(node::EVENTS).unwrap()[0]
            .as_obj()
            .and_then(|e| e.get("signals"))
            .and_then(Node::as_arr)
            .and_then(|a| a.items.first())
            .and_then(Node::as_obj)
            .and_then(|sg| sg.get("attributes"))
            .and_then(Node::as_obj)
            .and_then(|at| at.get("curveName"))
            .and_then(Node::as_obj)
            .and_then(|c| c.get("value"))
            .and_then(Node::as_val)
            .and_then(|v| v.as_u32());
        assert_eq!(curve, Some(djb2("generic_fade_on")));
        let f = node::canonical(&triggers[1]);
        assert!(f.contains("hud.x_ctl.m_up_in_on") && f.contains("hud.x_ctl.m_up_in_off"));
        assert!(f.contains("activateOverride"));
    }
    fn turn() -> Motion {
        motion(serde_json::json!({
            "id": "rpm", "kind": "turn", "keys": [[0, 135], [0.5, -135]], "curve": "default_curve_1",
            "play_to": { "view_model": "view_model_data.hud_t_rpm" }, "steps": 5
        }))
    }

    fn turn_quats(c: &Channel) -> Vec<[f32; 4]> {
        c.keys.chunks(4).map(|q| [q[0], q[1], q[2], q[3]]).collect()
    }

    #[test]
    fn a_turn_is_named_and_checked() {
        let t = turn();
        assert_eq!(t.track().unwrap(), Track::Turn);
        t.check("t").unwrap();
        assert_eq!(t.play_to_bind().unwrap().ty, Some(Ty::I32));
        let r = template::resolve(&t.play_to_bind().unwrap(), Usage::Text, "t").unwrap();
        assert_eq!((r.db.as_str(), r.tag), ("#view_model_data.hud_t_rpm", *b"si32"));
    }

    #[test]
    fn a_turn_splits_wide_spans_and_keeps_its_quaternions_on_one_side() {
        let keys = turn().turn_keys();
        assert_eq!(keys.len(), 4, "270 degrees in three keys of 90: {keys:?}");
        assert_eq!(keys[0], (0.0, 135.0));
        assert_eq!(keys[3], (0.5, -135.0));
        assert!(
            keys.windows(2)
                .all(|w| (w[1].1 - w[0].1).abs() <= MAX_KEY_TURN + 1e-9)
        );
        assert!(keys.windows(2).all(|w| w[1].0 > w[0].0));
        let channels = turn().channels();
        assert_eq!(channels.len(), 1);
        let c = &channels[0];
        assert_eq!(c.target, Target::Rotation);
        assert_eq!(c.times.len(), 4);
        assert_eq!(c.max_time, 0.5);
        let q = turn_quats(c);
        for w in q.windows(2) {
            let dot: f32 = w[0].iter().zip(&w[1]).map(|(a, b)| a * b).sum();
            assert!(dot > 0.0, "neighbouring keys on one side: {q:?}");
        }
        assert!(q[0][3] <= 0.0, "the first key keeps w <= 0 like the stock files");
        for (k, (_, d)) in q.iter().zip(&keys) {
            let want = geom::quat_z(*d);
            let same = k.iter().zip(&want).all(|(a, b)| (f64::from(*a) - b).abs() < 1e-6);
            let flipped = k.iter().zip(&want).all(|(a, b)| (f64::from(*a) + b).abs() < 1e-6);
            assert!(same || flipped, "{d} degrees: {k:?}");
        }
    }

    #[test]
    fn a_turn_clip_has_no_one_key_rotation_and_a_padded_syst() {
        let template = clip_template();
        for keys in [
            serde_json::json!([[0, 0], [1, 30]]),
            serde_json::json!([[0, 120], [0.2, 90], [0.6, -30], [1, -150]]),
            serde_json::json!([[0, 0], [1, -300]]),
        ] {
            let mut m = turn();
            m.keys = serde_json::from_value(keys).unwrap();
            m.check("t").unwrap();
            let channels = m.channels();
            assert!(channels.iter().all(|c| c.times.len() >= 2));
            let (temp, syst) = build_clip(&template, "zh_t_dial_rpm", 9, "tbl", &channels).unwrap();
            assert!(syst.len().is_multiple_of(SYST_ALIGN), "{} bytes", syst.len());
            assert!(!has_constant_rotation(&temp, &syst).unwrap());
            assert_eq!(read_clip(&temp, &syst).unwrap().channels, channels);
        }
    }

    #[test]
    fn a_turn_plans_a_clip_and_an_int_leaf() {
        let e: Element = serde_json::from_value(serde_json::json!({
            "id": "dial", "kind": "group", "motion": serde_json::to_value(turn()).unwrap(), "children": []
        }))
        .unwrap();
        let mut p = Plan::default();
        plan(&mut p, &e, e.motion.as_ref().unwrap(), "zh_t_dial_rpm").unwrap();
        assert_eq!(p.clips.len(), 1);
        assert_eq!(p.clips[0].node, "zh_t_dial_rpm");
        assert_eq!(p.known, [("#view_model_data.hud_t_rpm".to_owned(), *b"si32")]);
        assert_eq!(p.motions[0].track, Track::Turn);
        assert_eq!(p.motions[0].steps, 5);
    }

    #[test]
    fn a_turn_is_played_to_one_place_per_value() {
        let mut p = Plan::default();
        p.motions.push(MotionItem {
            id: "dial".into(),
            mid: "rpm".into(),
            element: "zh_t_dial_rpm".into(),
            track: Track::Turn,
            curve: "default_curve_1".into(),
            from: 1.0,
            to: 1.0,
            db: "#view_model_data.hud_t_rpm".into(),
            steps: 5,
        });
        let players = player_nodes(&p);
        assert_eq!(players.len(), 1);
        assert_eq!(
            (players[0].name(), players[0].type_code().as_str()),
            ("m_dial_rpm", "UIXA")
        );
        let triggers = trigger_nodes(&p, "hud.x_ctl");
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].type_code(), "MVCD");
        let events = triggers[0].slot(node::EVENTS).unwrap();
        let read: Vec<(String, f32, u32, String)> = events
            .iter()
            .map(|e| {
                let o = e.as_obj().unwrap();
                let name = o
                    .get("eventName")
                    .and_then(Node::as_val)
                    .and_then(|v| v.as_str())
                    .unwrap();
                let sg = o
                    .get("signals")
                    .and_then(Node::as_arr)
                    .and_then(|a| a.items.first())
                    .and_then(Node::as_obj)
                    .unwrap();
                let at = sg.get("attributes").and_then(Node::as_obj).unwrap();
                let val = |k: &str| {
                    at.get(k)
                        .and_then(Node::as_obj)
                        .and_then(|c| c.get("value"))
                        .and_then(Node::as_val)
                };
                (
                    name.to_owned(),
                    val("value").and_then(|v| v.as_u32()).map(f32::from_bits).unwrap(),
                    val("curveName").and_then(|v| v.as_u32()).unwrap(),
                    node::canonical(&Node::Obj(sg.clone())),
                )
            })
            .collect();
        let names: Vec<&str> = read.iter().map(|r| r.0.as_str()).collect();
        assert_eq!(names, ["0", "1", "2", "3", "4"]);
        let values: Vec<f32> = read.iter().map(|r| r.1).collect();
        assert_eq!(values, [0.0, 0.25, 0.5, 0.75, 1.0]);
        assert!(read.iter().all(|r| r.2 == djb2("default_curve_1")));
        assert!(
            read.iter()
                .all(|r| r.3.contains("hud.x_ctl.m_dial_rpm") && r.3.contains("playToFraction"))
        );
        assert!(node::canonical(&triggers[0]).contains("#view_model_data.hud_t_rpm"));
    }

    #[test]
    fn turn_refusals() {
        let bad = |f: fn(&mut serde_json::Value)| {
            let mut v = serde_json::to_value(turn()).unwrap();
            f(&mut v);
            let m: Motion = serde_json::from_value(v).unwrap();
            m.check("t").unwrap_err().to_string()
        };
        assert!(bad(|v| v["steps"] = 1.into()).contains("steps is 2 to 64"));
        assert!(bad(|v| v["steps"] = 65.into()).contains("steps is 2 to 64"));
        assert!(
            bad(|v| v.as_object_mut().unwrap().remove("steps").map(|_| ()).unwrap()).contains("needs steps")
        );
        assert!(
            bad(|v| v.as_object_mut().unwrap().remove("play_to").map(|_| ()).unwrap())
                .contains("needs play_to")
        );
        assert!(
            bad(|v| v["play_on"] = serde_json::json!({ "view_model": "view_model_data.hud_t_x" }))
                .contains("not started by play_on")
        );
        assert!(bad(|v| v["play_to"]["ty"] = "text".into()).contains("int leaf"));
        assert!(bad(|v| v["play_to"]["not"] = true.into()).contains("without not"));
        assert!(
            bad(|v| v["play_to"] = serde_json::json!({ "game": "revs" })).contains("view_model int leaf")
        );
        assert!(bad(|v| v["keys"] = serde_json::json!([[0, 10]])).contains("two keys"));
        assert!(bad(|v| v["keys"] = serde_json::json!([[0, 10], [0, 20]])).contains("go up"));
        assert!(
            bad(|v| v["keys"] = serde_json::json!([[0, [0, 0]], [1, [0, 4]]])).contains("[time, degrees]")
        );
        assert!(bad(|v| v["curve"] = "needle_swing".into()).contains("not a curve"));
        let slide_with_steps = |f: fn(&mut serde_json::Value)| {
            let mut v = serde_json::to_value(slide()).unwrap();
            f(&mut v);
            serde_json::from_value::<Motion>(v)
                .unwrap()
                .check("t")
                .unwrap_err()
                .to_string()
        };
        assert!(slide_with_steps(|v| v["steps"] = 4.into()).contains("go with a turn"));
        assert!(slide_with_steps(|v| v["kind"] = "fade".into()).contains("kind Fade"));
    }

    #[test]
    fn a_turning_group_holds_quads_and_texts_only() {
        let parse = |v: serde_json::Value| -> Vec<Element> { serde_json::from_value(v).unwrap() };
        let m = serde_json::to_value(turn()).unwrap();
        let ok = parse(
            serde_json::json!([{ "id": "g", "kind": "group", "motion": m, "children": [
            { "id": "n", "kind": "sprite", "sprite": "needle" },
            { "id": "h", "kind": "group", "children": [ { "id": "t", "kind": "text", "literal": "x" } ] } ] }]),
        );
        check_elements(&ok, None).unwrap();
        let swept = parse(
            serde_json::json!([{ "id": "g", "kind": "group", "motion": m, "children": [
            { "id": "h", "kind": "group", "children": [ { "id": "s", "kind": "sweep" } ] } ] }]),
        );
        assert!(
            check_elements(&swept, None)
                .unwrap_err()
                .to_string()
                .contains("laid out unrotated")
        );
    }
}
