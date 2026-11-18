//! Resolved shader events: the `rules` of `active.json` and the engine that evaluates them each
//! presented frame. The game core runs the same algorithm (`postfx::RuleEngine`) against the same
//! vectors (`tests/rules_vectors.json`).

use serde::{Deserialize, Serialize};

use crate::ActiveInstance;
use crate::signals::{SIGNALS, SignalKind, signal};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    True,
    Is,
    Above,
    Below,
    Between,
    Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Blend,
    Round,
    Switch,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleCond {
    pub signal: String,
    pub op: Op,
    /// `is`: indices into the signal's names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<u32>,
    /// `is` on car.surface: wheels that must match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wheels: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lo: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hi: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleRamp {
    pub signal: String,
    pub from: f64,
    pub to: f64,
}

/// One override: a param's packed values for `blend`, `round` and `switch`, or `enabled: false`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Override {
    pub instance: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<Mode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

fn fade_in_default() -> f64 {
    150.0
}

fn fade_out_default() -> f64 {
    300.0
}

fn for_default() -> f64 {
    400.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub order: u32,
    #[serde(default)]
    pub event: bool,
    #[serde(default)]
    pub when: Vec<RuleCond>,
    #[serde(default)]
    pub ramp: Option<RuleRamp>,
    #[serde(default)]
    pub min_ms: f64,
    #[serde(default)]
    pub hold_ms: f64,
    #[serde(default = "for_default")]
    pub for_ms: f64,
    #[serde(default = "fade_in_default")]
    pub fade_in_ms: f64,
    #[serde(default = "fade_out_default")]
    pub fade_out_ms: f64,
    #[serde(default)]
    pub set: Vec<Override>,
}

/// What one signal reads this frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Sample {
    /// Not read, or not available right now: conditions on it fail and its events never occur.
    #[default]
    Missing,
    Bool(bool),
    Name(u32),
    /// car.surface: one name index per wheel (FL FR RL RR).
    Surface([u32; 4]),
    Number(f64),
    /// An event's occurrences this frame, each with its magnitude (0 when it has none).
    Events(Vec<f64>),
}

/// The signal values of one frame, indexed like [`SIGNALS`]. Everything starts missing.
#[derive(Debug, Clone, PartialEq)]
pub struct Signals {
    samples: Vec<Sample>,
}

impl Default for Signals {
    fn default() -> Self {
        Self::new()
    }
}

fn name_index(id: &str, v: &serde_json::Value) -> Result<u32, String> {
    let sig = signal(id).ok_or_else(|| format!("unknown signal {id}"))?.1;
    match v {
        serde_json::Value::String(s) => sig
            .values
            .iter()
            .position(|n| n == s)
            .map(|i| i as u32)
            .ok_or_else(|| format!("{id}: no value {s}")),
        serde_json::Value::Number(n) => n
            .as_u64()
            .map(|i| i as u32)
            .ok_or_else(|| format!("{id}: bad index")),
        _ => Err(format!("{id}: expected a name")),
    }
}

impl Signals {
    pub fn new() -> Self {
        Self {
            samples: vec![Sample::Missing; SIGNALS.len()],
        }
    }

    pub fn set(&mut self, id: &str, sample: Sample) {
        if let Some((i, _)) = signal(id) {
            self.samples[i] = sample;
        }
    }

    pub fn set_bool(&mut self, id: &str, on: bool) {
        self.set(id, Sample::Bool(on));
    }

    pub fn set_number(&mut self, id: &str, x: f64) {
        self.set(id, Sample::Number(x));
    }

    /// A name signal by value, such as `("view.camera", "cockpit")`.
    pub fn set_name(&mut self, id: &str, value: &str) {
        if let Some((i, sig)) = signal(id)
            && let Some(v) = sig.values.iter().position(|n| *n == value)
        {
            self.samples[i] = Sample::Name(v as u32);
        }
    }

    /// car.surface, one name per wheel (FL FR RL RR).
    pub fn set_surface(&mut self, wheels: [&str; 4]) {
        let Some((i, sig)) = signal("car.surface") else {
            return;
        };
        let idx = wheels.map(|w| sig.values.iter().position(|n| *n == w).unwrap_or(0) as u32);
        self.samples[i] = Sample::Surface(idx);
    }

    /// Adds one occurrence of an event this frame.
    pub fn fire(&mut self, id: &str, magnitude: f64) {
        if let Some((i, _)) = signal(id) {
            match &mut self.samples[i] {
                Sample::Events(list) => list.push(magnitude),
                other => *other = Sample::Events(vec![magnitude]),
            }
        }
    }

    pub fn get(&self, index: usize) -> &Sample {
        &self.samples[index]
    }

    /// The vector files' shape: an object of signal id to value. A bool signal is `true`/`false`, a name
    /// signal its name (or index), a number signal a number, car.surface four names, an event the list
    /// of its occurrences' magnitudes (`[]` none, `[0]` one without magnitude). A missing key or `null`
    /// is a missing signal.
    pub fn from_json(v: &serde_json::Value) -> Result<Self, String> {
        let mut out = Self::new();
        let Some(map) = v.as_object() else {
            return Err("signals must be an object".into());
        };
        for (id, val) in map {
            let (i, sig) = signal(id).ok_or_else(|| format!("unknown signal {id}"))?;
            if val.is_null() {
                continue;
            }
            out.samples[i] = match sig.kind {
                SignalKind::Bool => match val {
                    serde_json::Value::Bool(b) => Sample::Bool(*b),
                    serde_json::Value::Number(n) => Sample::Bool(n.as_f64().unwrap_or(0.0) != 0.0),
                    _ => return Err(format!("{id}: expected true or false")),
                },
                SignalKind::Name if sig.per_wheel() => {
                    let list = val.as_array().filter(|a| a.len() == 4);
                    let list = list.ok_or_else(|| format!("{id}: expected 4 wheel names"))?;
                    let mut w = [0u32; 4];
                    for (slot, item) in w.iter_mut().zip(list) {
                        *slot = name_index(id, item)?;
                    }
                    Sample::Surface(w)
                }
                SignalKind::Name => Sample::Name(name_index(id, val)?),
                SignalKind::Number => {
                    Sample::Number(val.as_f64().ok_or_else(|| format!("{id}: expected a number"))?)
                }
                SignalKind::Event => match val {
                    serde_json::Value::Array(a) => Sample::Events(
                        a.iter()
                            .map(|m| m.as_f64().ok_or_else(|| format!("{id}: expected magnitudes")))
                            .collect::<Result<_, _>>()?,
                    ),
                    serde_json::Value::Number(n) => Sample::Events(vec![n.as_f64().unwrap_or(0.0)]),
                    serde_json::Value::Bool(true) => Sample::Events(vec![0.0]),
                    serde_json::Value::Bool(false) => Sample::Events(Vec::new()),
                    _ => return Err(format!("{id}: expected a list of magnitudes")),
                },
            };
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct State {
    left: f64,
    true_for: f64,
    false_for: f64,
    on: bool,
    w: f64,
}

/// Evaluates the rules once per presented frame (contract section 4).
#[derive(Debug, Clone)]
pub struct Engine {
    rules: Vec<Rule>,
    cond_sig: Vec<Vec<Option<usize>>>,
    ramp_sig: Vec<Option<usize>>,
    sorted: Vec<usize>,
    state: Vec<State>,
    weights: Vec<f64>,
}

fn smooth(w: f64) -> f64 {
    w * w * (3.0 - 2.0 * w)
}

fn magnitude_ok(c: &RuleCond, m: f64) -> bool {
    match c.op {
        Op::Above => m > c.value.unwrap_or(f64::INFINITY),
        Op::Below => m < c.value.unwrap_or(f64::NEG_INFINITY),
        Op::Between => m > c.lo.unwrap_or(f64::INFINITY) && m < c.hi.unwrap_or(f64::NEG_INFINITY),
        _ => true,
    }
}

fn holds(c: &RuleCond, sample: &Sample) -> bool {
    match (c.op, sample) {
        (Op::True, Sample::Bool(b)) => *b,
        (Op::Is, Sample::Name(n)) => c.values.contains(n),
        (Op::Is, Sample::Surface(w)) => {
            let hits = w.iter().filter(|n| c.values.contains(n)).count();
            hits >= usize::from(c.wheels.unwrap_or(1))
        }
        (Op::Above | Op::Below | Op::Between, Sample::Number(x)) => magnitude_ok(c, *x),
        _ => false,
    }
}

impl Engine {
    pub fn new(rules: &[Rule]) -> Self {
        let index = |id: &str| signal(id).map(|(i, _)| i);
        let mut sorted: Vec<usize> = (0..rules.len()).collect();
        sorted.sort_by_key(|&i| (rules[i].priority, rules[i].order));
        Self {
            cond_sig: rules
                .iter()
                .map(|r| r.when.iter().map(|c| index(&c.signal)).collect())
                .collect(),
            ramp_sig: rules
                .iter()
                .map(|r| r.ramp.as_ref().and_then(|ramp| index(&ramp.signal)))
                .collect(),
            sorted,
            state: vec![State::default(); rules.len()],
            weights: vec![0.0; rules.len()],
            rules: rules.to_vec(),
        }
    }

    /// Every rule off, w = 0: what a new `active.json` or a Preview jump does.
    pub fn reset(&mut self) {
        self.state.fill(State::default());
        self.weights.fill(0.0);
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Advances every rule by one frame of `dt_ms` (clamped to 0-100).
    pub fn step(&mut self, dt_ms: f64, signals: &Signals) {
        let dt = if dt_ms.is_nan() {
            0.0
        } else {
            dt_ms.clamp(0.0, 100.0)
        };
        for (i, r) in self.rules.iter().enumerate() {
            let st = &mut self.state[i];
            let sample = |k: usize| -> &Sample {
                static MISSING: Sample = Sample::Missing;
                match self.cond_sig[i].get(k).copied().flatten() {
                    Some(s) => signals.get(s),
                    None => &MISSING,
                }
            };
            let target;
            if r.event {
                let passes = match (r.when.first(), sample(0)) {
                    (Some(c), Sample::Events(list)) => list.iter().any(|m| magnitude_ok(c, *m)),
                    _ => false,
                };
                st.left = if passes { r.for_ms } else { (st.left - dt).max(0.0) };
                st.on = st.left > 0.0;
                target = if st.on { 1.0 } else { 0.0 };
            } else if let Some(ramp) = &r.ramp {
                let x = match self.ramp_sig[i].map(|s| signals.get(s)) {
                    Some(Sample::Number(x)) => Some(*x),
                    _ => None,
                };
                target = x.map_or(0.0, |x| ((x - ramp.from) / (ramp.to - ramp.from)).clamp(0.0, 1.0));
                st.on = target > 0.0;
            } else {
                let cond = r.when.iter().enumerate().all(|(k, c)| holds(c, sample(k)));
                if cond {
                    st.true_for += dt;
                    st.false_for = 0.0;
                } else {
                    st.true_for = 0.0;
                    st.false_for += dt;
                }
                if !st.on && cond && st.true_for >= r.min_ms {
                    st.on = true;
                }
                if st.on && !cond && st.false_for > r.hold_ms {
                    st.on = false;
                }
                target = if st.on { 1.0 } else { 0.0 };
            }
            if target > st.w {
                st.w = if r.fade_in_ms <= 0.0 {
                    target
                } else {
                    (st.w + dt / r.fade_in_ms).min(target)
                };
            } else if target < st.w {
                st.w = if r.fade_out_ms <= 0.0 {
                    target
                } else {
                    (st.w - dt / r.fade_out_ms).max(target)
                };
            }
            self.weights[i] = st.w;
        }
    }

    /// Each rule's fade state w (0-1), in rule order.
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    /// Whether each rule is on (what the core logs transitions of), in rule order.
    pub fn is_on(&self, rule: usize) -> bool {
        self.state.get(rule).is_some_and(|s| s.on)
    }

    /// The base instances with every active rule's overrides applied, and which instances are off.
    pub fn apply(&self, base: &[ActiveInstance]) -> (Vec<ActiveInstance>, Vec<bool>) {
        let mut options: Vec<Vec<f64>> = base
            .iter()
            .map(|b| b.params.iter().map(|&p| f64::from(p)).collect())
            .collect();
        let mut off = vec![false; base.len()];
        for &ri in &self.sorted {
            let a = smooth(self.weights[ri]);
            if a <= 0.0 {
                continue;
            }
            for o in &self.rules[ri].set {
                let Some(params) = options.get_mut(o.instance) else {
                    continue;
                };
                if o.enabled == Some(false) {
                    if a >= 0.5 {
                        off[o.instance] = true;
                    }
                    continue;
                }
                let (Some(slot), Some(mode)) = (o.slot, o.mode) else {
                    continue;
                };
                for (k, &t) in o.values.iter().enumerate() {
                    let Some(v) = params.get_mut(slot + k) else {
                        continue;
                    };
                    let t = f64::from(t);
                    match mode {
                        Mode::Blend => *v += (t - *v) * a,
                        Mode::Round => *v = (*v + (t - *v) * a).round(),
                        Mode::Switch => {
                            if a >= 0.5 {
                                *v = t;
                            }
                        }
                    }
                }
            }
        }
        let out = base
            .iter()
            .zip(options)
            .map(|(b, p)| ActiveInstance {
                effect: b.effect.clone(),
                label: b.label.clone(),
                params: p.into_iter().map(|v| v as f32).collect(),
            })
            .collect();
        (out, off)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    const SLOTS: usize = 32;

    fn instance(set: &[(usize, f32)]) -> ActiveInstance {
        let mut params = vec![0.0f32; SLOTS];
        for &(i, v) in set {
            params[i] = v;
        }
        ActiveInstance {
            effect: "fx".into(),
            label: String::new(),
            params,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rule(name: &str, priority: i32, order: u32, when: Value, timing: [f64; 5], set: Value) -> Value {
        let [min_ms, hold_ms, for_ms, fade_in_ms, fade_out_ms] = timing;
        json!({
            "name": name, "priority": priority, "order": order,
            "event": when.get(0).is_some_and(|c| c["op"] == "event" || c["event"] == true),
            "when": when, "ramp": null,
            "min_ms": min_ms, "hold_ms": hold_ms, "for_ms": for_ms,
            "fade_in_ms": fade_in_ms, "fade_out_ms": fade_out_ms,
            "set": set,
        })
    }

    fn event_rule(
        name: &str,
        priority: i32,
        order: u32,
        cond: Value,
        for_ms: f64,
        fades: [f64; 2],
        set: Value,
    ) -> Value {
        let mut r = rule(
            name,
            priority,
            order,
            json!([cond]),
            [0.0, 0.0, for_ms, fades[0], fades[1]],
            set,
        );
        r["event"] = json!(true);
        r
    }

    fn blend(instance: usize, slot: usize, values: &[f32]) -> Value {
        json!({ "instance": instance, "slot": slot, "mode": "blend", "values": values })
    }

    fn bool_on(signal: &str) -> Value {
        json!({ "signal": signal, "op": "true" })
    }

    fn frame(dt_ms: f64, signals: Value) -> (f64, Value) {
        (dt_ms, signals)
    }

    struct Case {
        name: &'static str,
        note: &'static str,
        rules: Vec<Value>,
        base: Vec<ActiveInstance>,
        frames: Vec<(f64, Value)>,
    }

    fn cases() -> Vec<Case> {
        let kerb2 = json!({ "signal": "car.surface", "op": "is", "values": [1], "wheels": 2 });
        let speed150 = json!({ "signal": "car.speed", "op": "above", "value": 150 });
        vec![
            Case {
                name: "min_ms_and_fades",
                note: "FORMAT: each frame lists the complete signals of that frame (no state carries over). Keys are signal ids; bool: true/false; name: the name (or its index); number: a number; car.surface: 4 names (FL FR RL RR); event: the list of this frame's occurrence magnitudes ([] none, [0] one without magnitude). A missing key or null is a missing signal. expect.weights = each rule's fade state w after the frame; expect.options = every instance's 32 params after apply(); expect.off = per instance. Rule JSON is the active.json rule. Compare floats with 1e-5.",
                rules: vec![rule(
                    "a",
                    0,
                    0,
                    json!([bool_on("view.replay")]),
                    [100.0, 0.0, 0.0, 100.0, 200.0],
                    json!([blend(0, 0, &[1.0])]),
                )],
                base: vec![instance(&[])],
                frames: vec![
                    frame(50.0, json!({"view.replay": true})),
                    frame(50.0, json!({"view.replay": true})),
                    frame(50.0, json!({"view.replay": true})),
                    frame(50.0, json!({"view.replay": true})),
                    frame(50.0, json!({"view.replay": false})),
                    frame(100.0, json!({"view.replay": false})),
                    frame(100.0, json!({"view.replay": false})),
                ],
            },
            Case {
                name: "brief_touch_under_min_ms",
                note: "A condition that clears before min_ms never turns the rule on.",
                rules: vec![rule(
                    "a",
                    0,
                    0,
                    json!([bool_on("view.paused")]),
                    [100.0, 0.0, 0.0, 100.0, 100.0],
                    json!([blend(0, 3, &[2.0])]),
                )],
                base: vec![instance(&[(3, 0.5)])],
                frames: vec![
                    frame(40.0, json!({"view.paused": true})),
                    frame(40.0, json!({"view.paused": true})),
                    frame(40.0, json!({"view.paused": false})),
                    frame(40.0, json!({"view.paused": true})),
                    frame(40.0, json!({})),
                ],
            },
            Case {
                name: "hold_ms",
                note: "false_for must exceed hold_ms (strictly) before the rule goes off.",
                rules: vec![rule(
                    "a",
                    0,
                    0,
                    json!([bool_on("view.photo")]),
                    [0.0, 100.0, 0.0, 0.0, 0.0],
                    json!([blend(0, 0, &[1.0])]),
                )],
                base: vec![instance(&[])],
                frames: vec![
                    frame(10.0, json!({"view.photo": true})),
                    frame(50.0, json!({"view.photo": false})),
                    frame(50.0, json!({"view.photo": false})),
                    frame(10.0, json!({"view.photo": false})),
                    frame(10.0, json!({"view.photo": false})),
                ],
            },
            Case {
                name: "ramp_follows_the_signal",
                note: "Ramp rules have an empty `when`; zero fades make w jump to the target.",
                rules: vec![{
                    let mut x = rule(
                        "flat out",
                        0,
                        0,
                        json!([]),
                        [0.0, 0.0, 0.0, 0.0, 0.0],
                        json!([blend(0, 1, &[0.5, 1.0])]),
                    );
                    x["ramp"] = json!({ "signal": "car.speed", "from": 250, "to": 320 });
                    x
                }],
                base: vec![instance(&[(1, 0.0), (2, 0.25)])],
                frames: vec![
                    frame(16.0, json!({"car.speed": 200})),
                    frame(16.0, json!({"car.speed": 267.5})),
                    frame(16.0, json!({"car.speed": 285})),
                    frame(16.0, json!({"car.speed": 400})),
                    frame(16.0, json!({"car.speed": 100})),
                ],
            },
            Case {
                name: "event_for_ms_and_magnitude",
                note: "car.impact above 3 fires a 300 ms window; a 2.0 hit is ignored.",
                rules: vec![event_rule(
                    "impact",
                    0,
                    0,
                    json!({ "signal": "car.impact", "op": "above", "value": 3.0 }),
                    300.0,
                    [0.0, 200.0],
                    json!([blend(0, 0, &[1.0])]),
                )],
                base: vec![instance(&[])],
                frames: vec![
                    frame(100.0, json!({"car.impact": [2.0]})),
                    frame(100.0, json!({"car.impact": [2.0, 3.5]})),
                    frame(100.0, json!({"car.impact": []})),
                    frame(100.0, json!({})),
                    frame(100.0, json!({})),
                    frame(100.0, json!({})),
                    frame(100.0, json!({})),
                ],
            },
            Case {
                name: "event_without_magnitude",
                note: "A plain event rule fires on any occurrence, each occurrence restarts the window.",
                rules: vec![event_rule(
                    "lap",
                    0,
                    0,
                    json!({ "signal": "session.lap", "op": "event" }),
                    150.0,
                    [0.0, 0.0],
                    json!([blend(0, 0, &[1.0])]),
                )],
                base: vec![instance(&[])],
                frames: vec![
                    frame(100.0, json!({"session.lap": [0]})),
                    frame(100.0, json!({})),
                    frame(100.0, json!({"session.lap": [0]})),
                    frame(100.0, json!({})),
                    frame(100.0, json!({})),
                ],
            },
            Case {
                name: "all_conditions",
                note: "Every entry must hold; a missing car.speed fails its entry.",
                rules: vec![rule(
                    "a",
                    0,
                    0,
                    json!([kerb2.clone(), speed150.clone()]),
                    [0.0, 0.0, 0.0, 0.0, 0.0],
                    json!([blend(0, 0, &[1.0])]),
                )],
                base: vec![instance(&[])],
                frames: vec![
                    frame(
                        16.0,
                        json!({"car.surface": ["kerb","kerb","tarmac","tarmac"], "car.speed": 100}),
                    ),
                    frame(
                        16.0,
                        json!({"car.surface": ["kerb","kerb","tarmac","tarmac"], "car.speed": 200}),
                    ),
                    frame(
                        16.0,
                        json!({"car.surface": ["kerb","tarmac","tarmac","tarmac"], "car.speed": 200}),
                    ),
                    frame(16.0, json!({"car.surface": ["kerb","kerb","tarmac","tarmac"]})),
                    frame(
                        16.0,
                        json!({"car.surface": ["tarmac","tarmac","kerb","kerb"], "car.speed": 151}),
                    ),
                ],
            },
            Case {
                name: "surface_wheels",
                note: "wheels = 3 over [kerb, rumble]: at least 3 wheels on either surface.",
                rules: vec![rule(
                    "a",
                    0,
                    0,
                    json!([{ "signal": "car.surface", "op": "is", "values": [1, 2], "wheels": 3 }]),
                    [0.0, 0.0, 0.0, 0.0, 0.0],
                    json!([blend(0, 0, &[1.0])]),
                )],
                base: vec![instance(&[])],
                frames: vec![
                    frame(16.0, json!({"car.surface": ["kerb","rumble","grass","grass"]})),
                    frame(16.0, json!({"car.surface": ["kerb","rumble","kerb","grass"]})),
                    frame(16.0, json!({"car.surface": [1, 2, 2, 1]})),
                    frame(16.0, json!({})),
                ],
            },
            Case {
                name: "camera_name_list",
                note: "A name signal given as a name; `is` holds when it is any listed index (cockpit 0, tcam 1).",
                rules: vec![rule(
                    "a",
                    0,
                    0,
                    json!([{ "signal": "view.camera", "op": "is", "values": [0, 1] }]),
                    [0.0, 0.0, 0.0, 0.0, 0.0],
                    json!([blend(0, 0, &[1.0])]),
                )],
                base: vec![instance(&[])],
                frames: vec![
                    frame(16.0, json!({"view.camera": "cockpit"})),
                    frame(16.0, json!({"view.camera": "chase_far"})),
                    frame(16.0, json!({"view.camera": 1})),
                ],
            },
            Case {
                name: "priority_orders_a_shared_slot",
                note: "Applied by (priority, order) ascending, so the highest priority ends on top; equal priority: later in the file wins.",
                rules: vec![
                    rule(
                        "hi",
                        10,
                        0,
                        json!([bool_on("view.replay")]),
                        [0.0, 0.0, 0.0, 0.0, 0.0],
                        json!([blend(0, 0, &[0.9]), blend(0, 4, &[0.1])]),
                    ),
                    rule(
                        "lo",
                        0,
                        1,
                        json!([bool_on("view.replay")]),
                        [0.0, 0.0, 0.0, 100.0, 0.0],
                        json!([blend(0, 0, &[0.2])]),
                    ),
                    rule(
                        "lo2",
                        0,
                        2,
                        json!([bool_on("view.replay")]),
                        [0.0, 0.0, 0.0, 0.0, 0.0],
                        json!([blend(0, 4, &[0.7])]),
                    ),
                ],
                base: vec![instance(&[(0, 0.5), (4, 0.5)])],
                frames: vec![
                    frame(50.0, json!({"view.replay": true})),
                    frame(50.0, json!({"view.replay": true})),
                    frame(50.0, json!({"view.replay": false})),
                ],
            },
            Case {
                name: "switch_at_half_and_enabled",
                note: "switch values and enabled=false apply once the smoothed strength a is at least 0.5 (w = 0.5).",
                rules: vec![rule(
                    "a",
                    0,
                    0,
                    json!([bool_on("view.menu")]),
                    [0.0, 0.0, 0.0, 100.0, 100.0],
                    json!([
                        { "instance": 0, "slot": 5, "mode": "switch", "values": [1.0] },
                        { "instance": 1, "enabled": false },
                    ]),
                )],
                base: vec![instance(&[]), instance(&[])],
                frames: vec![
                    frame(25.0, json!({"view.menu": true})),
                    frame(25.0, json!({"view.menu": true})),
                    frame(25.0, json!({"view.menu": true})),
                    frame(25.0, json!({"view.menu": true})),
                    frame(25.0, json!({"view.menu": false})),
                    frame(25.0, json!({"view.menu": false})),
                    frame(25.0, json!({"view.menu": false})),
                ],
            },
            Case {
                name: "round_mode",
                note: "round: blend then round to a whole number; base 4 to 8 at a = 0.15625 is 4.625, rounds to 5.",
                rules: vec![rule(
                    "a",
                    0,
                    0,
                    json!([bool_on("view.cutscene")]),
                    [0.0, 0.0, 0.0, 100.0, 100.0],
                    json!([{ "instance": 0, "slot": 6, "mode": "round", "values": [8.0] }]),
                )],
                base: vec![instance(&[(6, 4.0)])],
                frames: vec![
                    frame(25.0, json!({"view.cutscene": true})),
                    frame(25.0, json!({"view.cutscene": true})),
                    frame(25.0, json!({"view.cutscene": true})),
                    frame(25.0, json!({"view.cutscene": true})),
                ],
            },
            Case {
                name: "dt_clamp",
                note: "dt is clamped to 0-100 ms: a 1000 ms frame moves a 1000 ms fade by 0.1, a negative dt moves nothing.",
                rules: vec![rule(
                    "a",
                    0,
                    0,
                    json!([bool_on("view.replay")]),
                    [0.0, 0.0, 0.0, 1000.0, 1000.0],
                    json!([blend(0, 0, &[1.0])]),
                )],
                base: vec![instance(&[])],
                frames: vec![
                    frame(1000.0, json!({"view.replay": true})),
                    frame(-50.0, json!({"view.replay": true})),
                    frame(100.0, json!({"view.replay": true})),
                    frame(5000.0, json!({"view.replay": false})),
                ],
            },
            Case {
                name: "missing_signals",
                note: "Conditions on a missing signal are false; a ramp on a missing signal targets 0; events never occur.",
                rules: vec![
                    rule(
                        "a",
                        0,
                        0,
                        json!([speed150]),
                        [0.0, 0.0, 0.0, 0.0, 0.0],
                        json!([blend(0, 0, &[1.0])]),
                    ),
                    {
                        let mut x = rule(
                            "b",
                            0,
                            1,
                            json!([]),
                            [0.0, 0.0, 0.0, 0.0, 0.0],
                            json!([blend(0, 1, &[1.0])]),
                        );
                        x["ramp"] = json!({ "signal": "car.rpm", "from": 0.5, "to": 1.0 });
                        x
                    },
                    event_rule(
                        "c",
                        0,
                        2,
                        json!({ "signal": "car.damage", "op": "event" }),
                        200.0,
                        [0.0, 0.0],
                        json!([blend(0, 2, &[1.0])]),
                    ),
                ],
                base: vec![instance(&[])],
                frames: vec![
                    frame(
                        16.0,
                        json!({"car.speed": 200, "car.rpm": 1.0, "car.damage": [0.2]}),
                    ),
                    frame(16.0, json!({"car.speed": null})),
                    frame(16.0, json!({})),
                ],
            },
        ]
    }

    fn round(x: f64, places: i32) -> f64 {
        let k = 10f64.powi(places);
        (x * k).round() / k
    }

    fn run_case(c: &Case) -> Vec<Value> {
        let rules: Vec<Rule> = c
            .rules
            .iter()
            .map(|r| serde_json::from_value(r.clone()).unwrap())
            .collect();
        let mut engine = Engine::new(&rules);
        c.frames
            .iter()
            .map(|(dt, signals)| {
                engine.step(*dt, &Signals::from_json(signals).unwrap());
                let (out, off) = engine.apply(&c.base);
                json!({
                    "weights": engine.weights().iter().map(|w| round(*w, 9)).collect::<Vec<_>>(),
                    "options": out.iter().map(|i| i.params.iter().map(|p| round(f64::from(*p), 6)).collect::<Vec<_>>()).collect::<Vec<_>>(),
                    "off": off,
                })
            })
            .collect()
    }

    fn line(v: &Value) -> String {
        serde_json::to_string(v).unwrap()
    }

    fn render(cases: &[Case]) -> String {
        let mut out = String::from(
            "{\n  \"format\": \"Shader rule vectors. Per case: build an engine from `rules`, then for each frame step(dt_ms, signals) and apply(base); compare weights, options (32 floats per instance) and off. See the first case's note for the signals shape.\",\n  \"cases\": [\n",
        );
        for (n, c) in cases.iter().enumerate() {
            let expects = run_case(c);
            out.push_str("    {\n");
            out.push_str(&format!("      \"name\": {},\n", line(&json!(c.name))));
            out.push_str(&format!("      \"note\": {},\n", line(&json!(c.note))));
            out.push_str("      \"rules\": [\n");
            let rules: Vec<String> = c.rules.iter().map(|r| format!("        {}", line(r))).collect();
            out.push_str(&rules.join(",\n"));
            out.push_str("\n      ],\n");
            let base: Vec<Value> = c.base.iter().map(|b| json!(b.params)).collect();
            out.push_str(&format!("      \"base\": {},\n", line(&json!(base))));
            out.push_str("      \"frames\": [\n");
            let frames: Vec<String> = c
                .frames
                .iter()
                .zip(&expects)
                .map(|((dt, signals), expect)| {
                    format!(
                        "        {}",
                        line(&json!({ "dt_ms": dt, "signals": signals, "expect": expect }))
                    )
                })
                .collect();
            out.push_str(&frames.join(",\n"));
            out.push_str("\n      ]\n    }");
            out.push_str(if n + 1 < cases.len() { ",\n" } else { "\n" });
        }
        out.push_str("  ]\n}\n");
        out
    }

    const FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rules_vectors.json");

    #[test]
    fn write_vectors_when_asked() {
        if std::env::var_os("SWOFTY_WRITE_VECTORS").is_some() {
            std::fs::write(FILE, render(&cases())).unwrap();
        }
    }

    fn close(a: &Value, b: &Value, what: &str) {
        match (a, b) {
            (Value::Array(x), Value::Array(y)) => {
                assert_eq!(x.len(), y.len(), "{what}");
                for (i, (p, q)) in x.iter().zip(y).enumerate() {
                    close(p, q, &format!("{what}[{i}]"));
                }
            }
            (Value::Object(x), Value::Object(y)) => {
                assert_eq!(x.len(), y.len(), "{what}");
                for (k, p) in x {
                    close(p, &y[k], &format!("{what}.{k}"));
                }
            }
            (Value::Number(x), Value::Number(y)) => {
                let (x, y) = (x.as_f64().unwrap(), y.as_f64().unwrap());
                assert!((x - y).abs() < 1e-5, "{what}: {x} vs {y}");
            }
            _ => assert_eq!(a, b, "{what}"),
        }
    }

    fn load() -> Value {
        serde_json::from_str(&std::fs::read_to_string(FILE).expect("rules_vectors.json")).unwrap()
    }

    #[test]
    fn the_vector_file_matches_the_engine() {
        let file = load();
        let cases = file["cases"].as_array().unwrap();
        assert!(cases.len() >= 10);
        for c in cases {
            let name = c["name"].as_str().unwrap();
            let rules: Vec<Rule> = serde_json::from_value(c["rules"].clone()).unwrap();
            let base: Vec<ActiveInstance> = c["base"]
                .as_array()
                .unwrap()
                .iter()
                .map(|b| ActiveInstance {
                    effect: "fx".into(),
                    label: String::new(),
                    params: serde_json::from_value(b.clone()).unwrap(),
                })
                .collect();
            let mut engine = Engine::new(&rules);
            for (n, f) in c["frames"].as_array().unwrap().iter().enumerate() {
                let signals = Signals::from_json(&f["signals"]).unwrap();
                engine.step(f["dt_ms"].as_f64().unwrap(), &signals);
                let (out, off) = engine.apply(&base);
                let got = json!({
                    "weights": engine.weights(),
                    "options": out.iter().map(|i| i.params.clone()).collect::<Vec<_>>(),
                    "off": off,
                });
                close(&got, &f["expect"], &format!("{name} frame {n}"));
            }
        }
    }

    fn frame_of<'a>(file: &'a Value, case: &str, n: usize) -> &'a Value {
        let c = file["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == case)
            .unwrap();
        &c["frames"][n]["expect"]
    }

    #[test]
    fn hand_checked_fade_and_min_ms() {
        let f = load();
        let w = |n: usize| {
            frame_of(&f, "min_ms_and_fades", n)["weights"][0]
                .as_f64()
                .unwrap()
        };
        let o = |n: usize| {
            frame_of(&f, "min_ms_and_fades", n)["options"][0][0]
                .as_f64()
                .unwrap()
        };
        assert_eq!([w(0), w(1)], [0.0, 0.5]);
        assert_eq!(w(2), 1.0);
        assert_eq!((o(0), o(1), o(2)), (0.0, 0.5, 1.0));
        assert_eq!(w(4), 0.75);
        assert!((o(4) - 0.84375).abs() < 1e-6);
        assert_eq!(w(6), 0.0);
    }

    #[test]
    fn hand_checked_ramp_quarter_is_smoothstep() {
        let f = load();
        let e = frame_of(&f, "ramp_follows_the_signal", 1);
        assert_eq!(e["weights"][0], 0.25);
        assert!((e["options"][0][1].as_f64().unwrap() - 0.5 * 0.15625).abs() < 1e-6);
        assert!((e["options"][0][2].as_f64().unwrap() - (0.25 + (1.0 - 0.25) * 0.15625)).abs() < 1e-6);
        assert_eq!(frame_of(&f, "ramp_follows_the_signal", 3)["weights"][0], 1.0);
    }

    #[test]
    fn hand_checked_switch_round_and_dt_clamp() {
        let f = load();
        let sw = |n: usize| frame_of(&f, "switch_at_half_and_enabled", n);
        assert_eq!(sw(0)["options"][0][5], 0.0);
        assert_eq!(sw(0)["off"], json!([false, false]));
        assert_eq!(sw(1)["options"][0][5], 1.0);
        assert_eq!(sw(1)["off"], json!([false, true]));
        assert_eq!(sw(3)["weights"][0], 1.0);
        assert_eq!(sw(4)["weights"][0], 0.75);
        assert_eq!(sw(5)["off"], json!([false, true]));
        assert_eq!(sw(6)["off"], json!([false, false]));
        let r = frame_of(&f, "round_mode", 0);
        assert_eq!(r["options"][0][6], 5.0);
        let d = |n: usize| frame_of(&f, "dt_clamp", n)["weights"][0].as_f64().unwrap();
        assert!((d(0) - 0.1).abs() < 1e-9 && (d(1) - 0.1).abs() < 1e-9 && (d(2) - 0.2).abs() < 1e-9);
        assert!((d(3) - 0.1).abs() < 1e-9);
    }

    #[test]
    fn the_engine_resets_and_reports_on() {
        let c = &cases()[0];
        let rules: Vec<Rule> = c
            .rules
            .iter()
            .map(|r| serde_json::from_value(r.clone()).unwrap())
            .collect();
        let mut e = Engine::new(&rules);
        let mut s = Signals::new();
        s.set_bool("view.replay", true);
        for _ in 0..3 {
            e.step(50.0, &s);
        }
        assert!(e.is_on(0));
        assert!(e.weights()[0] > 0.0);
        e.reset();
        assert!(!e.is_on(0));
        assert_eq!(e.weights()[0], 0.0);
    }

    #[test]
    fn rules_round_trip_through_json_with_the_contract_shape() {
        let text = r#"{"name":"kerb tracking","priority":0,"order":0,"event":false,
          "when":[{"signal":"car.surface","op":"is","values":[1],"wheels":2},{"signal":"car.speed","op":"above","value":150}],
          "ramp":null,"min_ms":40,"hold_ms":120,"for_ms":0,"fade_in_ms":60,"fade_out_ms":350,
          "set":[{"instance":0,"slot":3,"mode":"blend","values":[0.12]},{"instance":0,"slot":7,"mode":"switch","values":[1]},{"instance":1,"enabled":false}]}"#;
        let r: Rule = serde_json::from_str(text).unwrap();
        assert_eq!(r.when[0].wheels, Some(2));
        assert_eq!(r.set[2].enabled, Some(false));
        let back: Value = serde_json::to_value(&r).unwrap();
        let want: Value = serde_json::from_str(text).unwrap();
        close(&back, &want, "rule");
    }
}
