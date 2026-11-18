//! Checks for `[[when]]` rules (shader events): the conditions, timing and `[when.set.*]` overrides.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::config::{Lit, RawDoc, RawEntry, RawRule};
use crate::lint::{Sink, check_param, describe};
use crate::signals::{SIGNALS, Signal, SignalKind, signal};
use crate::text::did_you_mean;
use crate::{Catalog, Cond, CondOp, Fix, Instance, Ramp, RuleSet, Severity, WhenRule};

pub(crate) const RULE_KEYS: [&str; 15] = [
    "name",
    "on",
    "is",
    "wheels",
    "above",
    "below",
    "from",
    "to",
    "all",
    "min_ms",
    "hold_ms",
    "for_ms",
    "fade_in_ms",
    "fade_out_ms",
    "priority",
];
pub(crate) const INLINE_KEYS: [&str; 5] = ["on", "is", "above", "below", "wheels"];
pub(crate) const MAX_RULES: usize = 16;
pub(crate) const MAX_OVERRIDES: usize = 32;
const MAX_MS: f64 = 60_000.0;

/// Editor docs of the rule keys: (value type, doc, default text).
pub(crate) fn rule_key_doc(key: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match key {
        "name" => (
            "string",
            "A name for the rule, shown in the editor, the Preview and the log.",
            "\"\"",
        ),
        "on" => ("signal", "The signal the rule watches.", "\"\""),
        "is" => (
            "name or list of names",
            "On while a name signal has this value, or any of a list of values.",
            "\"\"",
        ),
        "wheels" => (
            "1-4",
            "car.surface only: how many wheels must match. Default 1.",
            "1",
        ),
        "above" => (
            "number",
            "On above this value; for an event, its magnitude must be above it.",
            "0",
        ),
        "below" => (
            "number",
            "On below this value; for an event, its magnitude must be below it.",
            "0",
        ),
        "from" => (
            "number",
            "Ramp start: the rule is off at `from` and fully on at `to`.",
            "0",
        ),
        "to" => ("number", "Ramp end: the rule is fully on at `to` and above.", "1"),
        "all" => (
            "list of conditions",
            "Every listed condition must hold: `[{ on = \"car.surface\", is = \"kerb\" }, ...]`.",
            "[]",
        ),
        "min_ms" => (
            "ms",
            "The condition must hold this long before the rule turns on. Default 0.",
            "0",
        ),
        "hold_ms" => (
            "ms",
            "The rule stays on this long after the condition clears. Default 0.",
            "0",
        ),
        "for_ms" => (
            "ms",
            "Events only: how long one event keeps the rule on. Default 400.",
            "400",
        ),
        "fade_in_ms" => (
            "ms",
            "How long the rule takes to fade in. Default 150, or 0 for a `from`/`to` ramp.",
            "150",
        ),
        "fade_out_ms" => (
            "ms",
            "How long the rule takes to fade out. Default 300, or 0 for a `from`/`to` ramp.",
            "300",
        ),
        "priority" => (
            "-100 to 100",
            "Higher wins on a shared option; equal priority: later in the file wins. Default 0.",
            "0",
        ),
        _ => return None,
    })
}

fn get<'a>(entries: &'a [RawEntry], key: &str) -> Option<&'a RawEntry> {
    entries.iter().find(|e| e.key == key)
}

fn names_of(sig: &Signal) -> String {
    sig.values.join(", ")
}

fn quoted_fix(title_word: &str, range: &Range<usize>, word: &str) -> Fix {
    Fix {
        title: format!("Change to `{title_word}`"),
        range: range.clone(),
        replacement: format!("\"{word}\""),
    }
}

fn number(sink: &mut Sink, e: &RawEntry) -> Option<f64> {
    match e.value.lit {
        Lit::Int(i) => Some(i as f64),
        Lit::Float(f) => Some(f),
        ref other => {
            sink.error(
                e.value.range.clone(),
                format!("`{}` expects a number, not {}", e.key, describe(other)),
            );
            None
        }
    }
}

enum Parsed {
    Cond(Cond, &'static Signal),
    Ramp(Ramp, &'static Signal),
}

fn one_kind(sink: &mut Sink, entries: &[RawEntry], with_all: bool) -> bool {
    let kind_of = |key: &str| match key {
        "is" => Some("is"),
        "above" | "below" => Some("above/below"),
        "from" | "to" => Some("from/to"),
        "all" if with_all => Some("all"),
        _ => None,
    };
    let mut first: Option<(&str, &str)> = None;
    for e in entries {
        let Some(kind) = kind_of(&e.key) else { continue };
        match first {
            None => first = Some((kind, &e.key)),
            Some((k, _)) if k == kind => {}
            Some((_, a)) => {
                sink.error(
                    e.key_range.clone(),
                    format!(
                        "A rule has one kind of condition; `{a}` and `{}` can't be used together",
                        e.key
                    ),
                );
                return false;
            }
        }
    }
    true
}

fn lookup(sink: &mut Sink, on: &RawEntry) -> Option<&'static Signal> {
    let Lit::Str(id) = &on.value.lit else {
        sink.error(
            on.value.range.clone(),
            format!(
                "`on` expects a signal id in quotes, not {}",
                describe(&on.value.lit)
            ),
        );
        return None;
    };
    match signal(id) {
        Some((_, s)) => Some(s),
        None => {
            let near = did_you_mean(id, SIGNALS.iter().map(|s| s.id));
            let message = match near {
                Some(c) => format!("unknown signal `{id}`; did you mean `{c}`?"),
                None => format!("unknown signal `{id}`"),
            };
            let fix = near.map(|c| quoted_fix(c, &on.value.range, c));
            sink.push(Severity::Error, on.value.range.clone(), message, fix);
            None
        }
    }
}

fn condition(
    sink: &mut Sink,
    entries: &[RawEntry],
    at: &Range<usize>,
    inline: bool,
    warned: &mut Vec<&'static str>,
) -> Option<Parsed> {
    let Some(on) = get(entries, "on") else {
        sink.error(at.clone(), "Each condition in `all` needs `on = \"<signal>\"`");
        return None;
    };
    let sig = lookup(sink, on)?;
    if !sig.in_game && !warned.contains(&sig.id) {
        warned.push(sig.id);
        sink.push(
            Severity::Warning,
            on.value.range.clone(),
            format!(
                "{} isn't read in game yet; this rule only fires in the Preview",
                sig.id
            ),
            None,
        );
    }
    let id = sig.id;
    let word = sig.kind_word();
    let is = get(entries, "is");
    let above = get(entries, "above");
    let below = get(entries, "below");
    let from = get(entries, "from");
    let to = get(entries, "to");
    let wheels_e = get(entries, "wheels");
    if inline {
        if let Some(e) = from.or(to) {
            sink.error(e.key_range.clone(), "A ramp (`from`/`to`) can't go inside `all`");
            return None;
        }
        if sig.kind == SignalKind::Event {
            sink.error(
                on.value.range.clone(),
                format!("{id} is an event; events can't go inside `all`"),
            );
            return None;
        }
    }
    let mut wheels = 1u8;
    if let Some(w) = wheels_e {
        if !sig.per_wheel() {
            sink.error(
                w.key_range.clone(),
                format!("`wheels` is for car.surface; {id} has none"),
            );
            return None;
        }
        match w.value.lit {
            Lit::Int(n) if (1..=4).contains(&n) => wheels = n as u8,
            _ => {
                sink.error(
                    w.value.range.clone(),
                    "`wheels` must be a whole number from 1 to 4",
                );
                return None;
            }
        }
    }
    let cond = |op| {
        Some(Parsed::Cond(
            Cond {
                signal: id.to_owned(),
                op,
            },
            sig,
        ))
    };
    if let Some(e) = is {
        if sig.kind != SignalKind::Name {
            sink.error(
                e.key_range.clone(),
                format!("`is` needs a name signal; {id} is {word}"),
            );
            return None;
        }
        let items: Vec<(String, Range<usize>)> = match &e.value.lit {
            Lit::Str(s) => vec![(s.clone(), e.value.range.clone())],
            Lit::Strs(list) if !list.is_empty() => {
                list.iter().map(|s| (s.text.clone(), s.range.clone())).collect()
            }
            _ => {
                sink.error(
                    e.value.range.clone(),
                    format!(
                        "`is` expects a name or a list of names, not {}",
                        describe(&e.value.lit)
                    ),
                );
                return None;
            }
        };
        let mut values = Vec::new();
        let mut ok = true;
        for (text, range) in items {
            if sig.values.contains(&text.as_str()) {
                values.push(text);
            } else {
                ok = false;
                let fix = did_you_mean(&text, sig.values.iter().copied()).map(|c| quoted_fix(c, &range, c));
                sink.push(
                    Severity::Error,
                    range,
                    format!("`is` needs a value of {id}: {}", names_of(sig)),
                    fix,
                );
            }
        }
        return ok.then(|| {
            Parsed::Cond(
                Cond {
                    signal: id.to_owned(),
                    op: CondOp::Is { values, wheels },
                },
                sig,
            )
        });
    }
    if above.is_some() || below.is_some() {
        let numeric = matches!(sig.kind, SignalKind::Number)
            || (sig.kind == SignalKind::Event && sig.magnitude.is_some());
        if !numeric {
            let key = above.or(below).expect("one is set");
            let why = if sig.kind == SignalKind::Event {
                format!("{id} is an event with no magnitude")
            } else {
                format!("{id} is {word}")
            };
            sink.error(
                key.key_range.clone(),
                format!("`{}` needs a number signal; {why}", key.key),
            );
            return None;
        }
        let a = above.and_then(|e| number(sink, e));
        let b = below.and_then(|e| number(sink, e));
        if (above.is_some() && a.is_none()) || (below.is_some() && b.is_none()) {
            return None;
        }
        return match (a, b) {
            (Some(lo), Some(hi)) => {
                if lo >= hi {
                    sink.error(
                        above.expect("set").value.range.clone(),
                        "`above` must be below `below`",
                    );
                    return None;
                }
                cond(CondOp::Between(lo, hi))
            }
            (Some(lo), None) => cond(CondOp::Above(lo)),
            (None, Some(hi)) => cond(CondOp::Below(hi)),
            (None, None) => None,
        };
    }
    if from.is_some() || to.is_some() {
        let key = from.or(to).expect("one is set");
        if sig.kind != SignalKind::Number {
            sink.error(
                key.key_range.clone(),
                format!("`{}` needs a number signal; {id} is {word}", key.key),
            );
            return None;
        }
        let (Some(f), Some(t)) = (from, to) else {
            let (have, missing) = if from.is_some() {
                ("from", "to")
            } else {
                ("to", "from")
            };
            sink.error(key.key_range.clone(), format!("`{have}` needs `{missing}` too"));
            return None;
        };
        let (fv, tv) = (number(sink, f)?, number(sink, t)?);
        if fv >= tv {
            sink.error(f.value.range.clone(), "`from` must be below `to`");
            return None;
        }
        return Some(Parsed::Ramp(
            Ramp {
                signal: id.to_owned(),
                from: fv,
                to: tv,
            },
            sig,
        ));
    }
    match sig.kind {
        SignalKind::Bool => cond(CondOp::True),
        SignalKind::Event => cond(CondOp::Event),
        SignalKind::Number => {
            sink.error(
                on.value.range.clone(),
                format!("{id} is a number; add `above`, `below`, or `from` and `to`"),
            );
            None
        }
        SignalKind::Name => {
            sink.error(
                on.value.range.clone(),
                format!("{id} is a name; add `is = \"...\"` with {}", names_of(sig)),
            );
            None
        }
    }
}

fn timing(sink: &mut Sink, entries: &[RawEntry], key: &str, default: f64) -> f64 {
    let Some(e) = get(entries, key) else {
        return default;
    };
    match number(sink, e) {
        Some(v) if (0.0..=MAX_MS).contains(&v) => v,
        Some(_) => {
            sink.error(
                e.value.range.clone(),
                format!("`{key}` must be a number from 0 to 60000"),
            );
            default
        }
        None => default,
    }
}

fn check_rule(
    sink: &mut Sink,
    rule: &RawRule,
    order: usize,
    instances: &[Instance],
    catalog: &Catalog,
) -> WhenRule {
    let entries = &rule.entries;
    if order >= MAX_RULES {
        sink.error(
            rule.header.clone(),
            format!(
                "A config has at most {MAX_RULES} rules; this is rule {}",
                order + 1
            ),
        );
    }
    for e in entries {
        if !RULE_KEYS.contains(&e.key.as_str()) {
            let near = did_you_mean(&e.key, RULE_KEYS.into_iter());
            let message = match near {
                Some(c) => format!("Unknown key `{}` in a rule; did you mean `{c}`?", e.key),
                None => format!("Unknown key `{}` in a rule", e.key),
            };
            let fix = near.map(|c| Fix {
                title: format!("Change to `{c}`"),
                range: e.key_range.clone(),
                replacement: c.to_owned(),
            });
            sink.push(Severity::Error, e.key_range.clone(), message, fix);
        }
    }

    let name = get(entries, "name").and_then(|e| match &e.value.lit {
        Lit::Str(s) => Some(s.clone()),
        other => {
            sink.error(
                e.value.range.clone(),
                format!("`name` expects a string, not {}", describe(other)),
            );
            None
        }
    });
    let priority = get(entries, "priority").map_or(0, |e| match e.value.lit {
        Lit::Int(n) if (-100..=100).contains(&n) => n as i32,
        _ => {
            sink.error(
                e.value.range.clone(),
                "`priority` must be a whole number from -100 to 100",
            );
            0
        }
    });
    let min_ms = timing(sink, entries, "min_ms", 0.0);
    let hold_ms = timing(sink, entries, "hold_ms", 0.0);
    let for_ms = timing(sink, entries, "for_ms", 400.0);
    let ramped = get(entries, "from").is_some() || get(entries, "to").is_some();
    let fade_in_ms = timing(sink, entries, "fade_in_ms", if ramped { 0.0 } else { 150.0 });
    let fade_out_ms = timing(sink, entries, "fade_out_ms", if ramped { 0.0 } else { 300.0 });

    let mut when = Vec::new();
    let mut ramp = None;
    let mut event = false;
    let mut warned = Vec::new();
    let mut top_sig: Option<&'static Signal> = None;
    let on = get(entries, "on");
    let all = get(entries, "all");
    if let (Some(on), Some(_)) = (on, all) {
        sink.error(
            on.key_range.clone(),
            "`on` and `all` can't be used together; name each signal inside `all`",
        );
    } else if on.is_none() && all.is_none() {
        sink.error(
            rule.header.clone(),
            "A rule needs `on = \"<signal>\"` or `all = [...]`",
        );
    } else if one_kind(sink, entries, true) {
        if on.is_some() {
            let parsed = condition(sink, entries, &rule.header, false, &mut warned);
            match parsed {
                Some(Parsed::Cond(c, sig)) => {
                    event = sig.kind == SignalKind::Event;
                    top_sig = Some(sig);
                    when.push(c);
                }
                Some(Parsed::Ramp(r, sig)) => {
                    top_sig = Some(sig);
                    ramp = Some(r);
                }
                None => {}
            }
        } else if let Some(all) = all {
            match &all.value.lit {
                Lit::Tables(list) if !list.is_empty() => {
                    for t in list {
                        if let Some(bad) = t.entries.iter().find(|e| !INLINE_KEYS.contains(&e.key.as_str())) {
                            let message = if matches!(bad.key.as_str(), "from" | "to") {
                                "A ramp (`from`/`to`) can't go inside `all`".to_owned()
                            } else {
                                format!(
                                    "`{}` isn't allowed inside `all`; use `on`, `is`, `above`, `below` or `wheels`",
                                    bad.key
                                )
                            };
                            sink.error(bad.key_range.clone(), message);
                            continue;
                        }
                        if !one_kind(sink, &t.entries, false) {
                            continue;
                        }
                        if let Some(Parsed::Cond(c, _)) =
                            condition(sink, &t.entries, &t.range, true, &mut warned)
                        {
                            when.push(c);
                        }
                    }
                }
                Lit::Tables(_) | Lit::Array(_) => {
                    sink.error(all.value.range.clone(), "`all` needs at least one condition");
                }
                other => sink.error(
                    all.value.range.clone(),
                    format!(
                        "`all` expects a list like `[{{ on = \"car.speed\", above = 150 }}]`, not {}",
                        describe(other)
                    ),
                ),
            }
        }
    }

    if let Some(e) = get(entries, "for_ms")
        && !event
    {
        let message = match top_sig {
            Some(sig) => format!("`for_ms` is for events; {} is {}", sig.id, sig.kind_word()),
            None => "`for_ms` is for events; this rule isn't one".to_owned(),
        };
        sink.error(e.key_range.clone(), message);
    }
    if let Some(e) = get(entries, "wheels")
        && on.is_none()
    {
        sink.error(e.key_range.clone(), "`wheels` goes with `on = \"car.surface\"`");
    }

    let mut sets = Vec::new();
    let mut overrides = 0;
    let mut seen: Vec<(&str, Option<&str>)> = Vec::new();
    for t in rule.sets.iter().filter(|t| !t.bad) {
        let shown = match &t.label {
            Some(l) => format!("[when.set.{}.{l}]", t.effect),
            None => format!("[when.set.{}]", t.effect),
        };
        let key = (t.effect.as_str(), t.label.as_deref());
        if seen.contains(&key) {
            sink.error(t.header.clone(), format!("{shown} appears twice in this rule"));
            continue;
        }
        seen.push(key);
        let Some(idx) = instances
            .iter()
            .position(|i| i.effect == t.effect && i.label == t.label)
        else {
            sink.error(
                t.header.clone(),
                format!("{shown} names no instance of this config"),
            );
            continue;
        };
        if !instances[idx].enabled {
            sink.error(
                t.header.clone(),
                format!("{shown} names an instance that's off in this config"),
            );
            continue;
        }
        let Some(effect) = catalog.get(&t.effect) else {
            continue;
        };
        let mut values = BTreeMap::new();
        let mut off = false;
        overrides += t.entries.len();
        for e in &t.entries {
            if e.key == "enabled" {
                match e.value.lit {
                    Lit::Bool(false) => off = true,
                    Lit::Bool(true) => sink.error(
                        e.value.range.clone(),
                        "`enabled` can only be false in a rule; a rule turns an instance off, never on",
                    ),
                    ref other => sink.error(
                        e.value.range.clone(),
                        format!("`enabled` expects true or false, not {}", describe(other)),
                    ),
                }
                continue;
            }
            let Some(p) = effect.params.iter().find(|p| p.id == e.key) else {
                let near = did_you_mean(&e.key, effect.params.iter().map(|p| p.id.as_str()));
                let message = match near {
                    Some(c) => format!("{} has no option `{}`; did you mean `{c}`?", effect.id, e.key),
                    None => format!("{} has no option `{}`", effect.id, e.key),
                };
                let fix = near.map(|c| Fix {
                    title: format!("Change to `{c}`"),
                    range: e.key_range.clone(),
                    replacement: c.to_owned(),
                });
                sink.push(Severity::Error, e.key_range.clone(), message, fix);
                continue;
            };
            if let Some(v) = check_param(sink, p, &e.value) {
                values.insert(p.id.clone(), v);
            }
        }
        sets.push(RuleSet {
            instance: idx,
            values,
            off,
            span: t.span.clone(),
        });
    }
    if overrides > MAX_OVERRIDES {
        sink.error(
            rule.header.clone(),
            format!("A rule has at most {MAX_OVERRIDES} overrides; this one has {overrides}"),
        );
    }
    if rule.sets.is_empty() {
        sink.push(
            Severity::Warning,
            rule.header.clone(),
            "This rule sets nothing; add a `[when.set.<effect>]` table".into(),
            None,
        );
    }

    WhenRule {
        name,
        priority,
        order,
        event,
        when,
        ramp,
        min_ms,
        hold_ms,
        for_ms: if event { for_ms } else { 0.0 },
        fade_in_ms,
        fade_out_ms,
        sets,
        span: rule.span.clone(),
    }
}

pub(crate) fn check(
    sink: &mut Sink,
    doc: &RawDoc,
    instances: &[Instance],
    catalog: &Catalog,
) -> Vec<WhenRule> {
    doc.rules
        .iter()
        .enumerate()
        .map(|(i, r)| check_rule(sink, r, i, instances, catalog))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::lint::tests::catalog;
    use crate::{Severity, lint};

    const HEAD: &str = "name = \"a\"\n[grain]\namount = 0.1\n[ambient_light]\nintensity = 1\n";

    fn run(rule: &str) -> crate::Lint {
        lint(&format!("{HEAD}{rule}"), &catalog())
    }

    fn msgs(rule: &str) -> Vec<(Severity, String)> {
        run(rule)
            .diagnostics
            .into_iter()
            .map(|d| (d.severity, d.message))
            .collect()
    }

    fn only(rule: &str, sev: Severity, message: &str) {
        let m = msgs(rule);
        assert!(
            m.iter().any(|(s, t)| *s == sev && t == message),
            "{rule:?} -> {m:?}"
        );
    }

    fn clean(rule: &str) {
        let l = run(rule);
        assert!(l.diagnostics.is_empty(), "{rule:?} -> {:?}", l.diagnostics);
        assert!(l.config.is_some());
    }

    #[test]
    fn a_full_rule_lints_clean_and_fills_defaults() {
        let text = format!(
            "{HEAD}[[when]]\nname = \"k\"\non = \"car.surface\"\nis = [\"kerb\", \"grass\"]\nmin_ms = 40\npriority = 5\n[when.set.grain]\namount = 0.5\nsize = 3\n[when.set.ambient_light]\nenabled = false\nlens = true\n"
        );
        let l = lint(&text, &catalog());
        assert!(l.diagnostics.is_empty(), "{:?}", l.diagnostics);
        let r = &l.config.unwrap().rules[0];
        assert_eq!(r.name.as_deref(), Some("k"));
        assert_eq!(
            (r.min_ms, r.hold_ms, r.fade_in_ms, r.fade_out_ms, r.priority),
            (40.0, 0.0, 150.0, 300.0, 5)
        );
        assert!(!r.event);
        assert_eq!(r.sets.len(), 2);
        assert_eq!(r.sets[0].instance, 0);
        assert!(r.sets[1].off);
        assert_eq!(&text[r.span.clone()][..8], "[[when]]");
    }

    #[test]
    fn every_condition_kind_lints_clean() {
        let set = "[when.set.grain]\namount = 0.5\n";
        clean(&format!("[[when]]\non = \"view.replay\"\n{set}"));
        clean(&format!("[[when]]\non = \"session.start\"\n{set}"));
        for body in ["above = 100\nbelow = 200", "from = 250\nto = 320"] {
            clean(&format!("[[when]]\non = \"car.speed\"\n{body}\n{set}"));
        }
        let m = msgs(&format!(
            "[[when]]\non = \"car.impact\"\nabove = 3.0\nfor_ms = 500\n{set}"
        ));
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].0, Severity::Warning);
        let m = msgs(&format!(
            "[[when]]\nall = [\n  {{ on = \"car.surface\", is = \"kerb\" }},\n  {{ on = \"view.paused\" }},\n]\n{set}"
        ));
        assert!(m.is_empty(), "{m:?}");
        clean(&format!("[[when]]\non = \"session.lap\"\n{set}"));
    }

    #[test]
    fn signal_typos_get_a_suggestion_and_a_fix() {
        let text =
            format!("{HEAD}[[when]]\non = \"car.surfce\"\nis = \"kerb\"\n[when.set.grain]\namount = 1\n");
        let l = lint(&text, &catalog());
        let d = &l.diagnostics[0];
        assert_eq!(
            d.message,
            "unknown signal `car.surfce`; did you mean `car.surface`?"
        );
        let fix = d.fix.as_ref().unwrap();
        assert_eq!(fix.replacement, "\"car.surface\"");
        assert_eq!(&text[fix.range.clone()], "\"car.surfce\"");
        only(
            "[[when]]\non = \"zzzzzzzz\"\n",
            Severity::Error,
            "unknown signal `zzzzzzzz`",
        );
    }

    #[test]
    fn is_checks_the_signals_names() {
        let s = "[when.set.grain]\namount = 1\n";
        only(
            &format!("[[when]]\non = \"car.surface\"\nis = \"curb\"\n{s}"),
            Severity::Error,
            "`is` needs a value of car.surface: tarmac, kerb, rumble, grass, gravel, sand",
        );
        let l = run(&format!("[[when]]\non = \"car.surface\"\nis = \"curb\"\n{s}"));
        assert_eq!(
            l.diagnostics
                .iter()
                .find(|d| d.severity == Severity::Error)
                .unwrap()
                .fix
                .as_ref()
                .unwrap()
                .replacement,
            "\"kerb\""
        );
        only(
            &format!("[[when]]\non = \"car.speed\"\nis = \"x\"\n{s}"),
            Severity::Error,
            "`is` needs a name signal; car.speed is a number",
        );
        only(
            &format!("[[when]]\non = \"car.surface\"\n{s}"),
            Severity::Error,
            "car.surface is a name; add `is = \"...\"` with tarmac, kerb, rumble, grass, gravel, sand",
        );
    }

    #[test]
    fn number_and_event_conditions_are_typed() {
        let s = "[when.set.grain]\namount = 1\n";
        only(
            &format!("[[when]]\non = \"car.surface\"\nabove = 3\n{s}"),
            Severity::Error,
            "`above` needs a number signal; car.surface is a name",
        );
        only(
            &format!("[[when]]\non = \"car.speed\"\nfor_ms = 5\nabove = 3\n{s}"),
            Severity::Error,
            "`for_ms` is for events; car.speed is a number",
        );
        only(
            &format!("[[when]]\non = \"car.speed\"\n{s}"),
            Severity::Error,
            "car.speed is a number; add `above`, `below`, or `from` and `to`",
        );
        only(
            &format!("[[when]]\non = \"session.lap\"\nabove = 3\n{s}"),
            Severity::Error,
            "`above` needs a number signal; session.lap is an event with no magnitude",
        );
        only(
            &format!("[[when]]\non = \"car.speed\"\nfrom = 320\nto = 250\n{s}"),
            Severity::Error,
            "`from` must be below `to`",
        );
        only(
            &format!("[[when]]\non = \"car.speed\"\nfrom = 3\n{s}"),
            Severity::Error,
            "`from` needs `to` too",
        );
        only(
            &format!("[[when]]\non = \"car.speed\"\nabove = 9\nbelow = 3\n{s}"),
            Severity::Error,
            "`above` must be below `below`",
        );
        only(
            &format!("[[when]]\non = \"car.rpm\"\nfrom = 0\nto = 1\nis = \"x\"\n{s}"),
            Severity::Error,
            "A rule has one kind of condition; `from` and `is` can't be used together",
        );
    }

    #[test]
    fn all_and_wheels_follow_the_contract() {
        let s = "[when.set.grain]\namount = 1\n";
        only(
            &format!("[[when]]\nall = [{{ on = \"car.impact\" }}]\n{s}"),
            Severity::Error,
            "car.impact is an event; events can't go inside `all`",
        );
        only(
            &format!("[[when]]\nall = [{{ on = \"car.speed\", from = 1, to = 2 }}]\n{s}"),
            Severity::Error,
            "A ramp (`from`/`to`) can't go inside `all`",
        );
        only(
            &format!("[[when]]\non = \"view.replay\"\nall = [{{ on = \"view.menu\" }}]\n{s}"),
            Severity::Error,
            "`on` and `all` can't be used together; name each signal inside `all`",
        );
        only(
            &format!("[[when]]\nall = []\n{s}"),
            Severity::Error,
            "`all` needs at least one condition",
        );
        only(
            &format!("[[when]]\n{s}"),
            Severity::Error,
            "A rule needs `on = \"<signal>\"` or `all = [...]`",
        );
        only(
            &format!("[[when]]\non = \"view.camera\"\nis = \"cockpit\"\nwheels = 2\n{s}"),
            Severity::Error,
            "`wheels` is for car.surface; view.camera has none",
        );
        only(
            &format!("[[when]]\non = \"car.surface\"\nis = \"kerb\"\nwheels = 5\n{s}"),
            Severity::Error,
            "`wheels` must be a whole number from 1 to 4",
        );
        let l = run(&format!(
            "[[when]]\non = \"car.surface\"\nis = [\"kerb\", \"rumble\"]\nwheels = 2\n{s}"
        ));
        let r = &l.config.as_ref().unwrap().rules[0];
        assert!(matches!(&r.when[0].op, crate::CondOp::Is { values, wheels: 2 } if values.len() == 2));
    }

    #[test]
    fn timing_priority_and_unknown_keys_are_checked() {
        let s = "[when.set.grain]\namount = 1\n";
        only(
            &format!("[[when]]\non = \"view.replay\"\nmin_ms = 70000\n{s}"),
            Severity::Error,
            "`min_ms` must be a number from 0 to 60000",
        );
        only(
            &format!("[[when]]\non = \"view.replay\"\nhold_ms = -1\n{s}"),
            Severity::Error,
            "`hold_ms` must be a number from 0 to 60000",
        );
        only(
            &format!("[[when]]\non = \"view.replay\"\npriority = 101\n{s}"),
            Severity::Error,
            "`priority` must be a whole number from -100 to 100",
        );
        only(
            &format!("[[when]]\non = \"view.replay\"\nfade_inn_ms = 1\n{s}"),
            Severity::Error,
            "Unknown key `fade_inn_ms` in a rule; did you mean `fade_in_ms`?",
        );
        only(
            &format!("[[when]]\non = \"view.replay\"\nfor_ms = 1\n{s}"),
            Severity::Error,
            "`for_ms` is for events; view.replay is a yes/no signal",
        );
    }

    #[test]
    fn set_tables_are_checked_against_the_configs_instances() {
        only(
            "[[when]]\non = \"view.replay\"\n[when.set.bloom]\nx = 1\n",
            Severity::Error,
            "[when.set.bloom] names no instance of this config",
        );
        only(
            "[[when]]\non = \"view.replay\"\n[when.set.grain.heavy]\nsize = 1\n",
            Severity::Error,
            "[when.set.grain.heavy] names no instance of this config",
        );
        let off = "name = \"a\"\n[grain]\nenabled = false\n[ambient_light]\n[[when]]\non = \"view.replay\"\n[when.set.grain]\nsize = 1\n";
        let m: Vec<_> = lint(off, &catalog())
            .diagnostics
            .into_iter()
            .map(|d| d.message)
            .collect();
        assert!(
            m.contains(&"[when.set.grain] names an instance that's off in this config".to_owned()),
            "{m:?}"
        );
        only(
            "[[when]]\non = \"view.replay\"\n[when.set.grain]\namout = 1\n",
            Severity::Error,
            "grain has no option `amout`; did you mean `amount`?",
        );
        let text = format!("{HEAD}[[when]]\non = \"view.replay\"\n[when.set.grain]\namout = 1\n");
        let l = lint(&text, &catalog());
        assert_eq!(l.diagnostics[0].fix.as_ref().unwrap().replacement, "amount");
        only(
            "[[when]]\non = \"view.replay\"\n[when.set.grain]\nenabled = true\n",
            Severity::Error,
            "`enabled` can only be false in a rule; a rule turns an instance off, never on",
        );
        only(
            "[[when]]\non = \"view.replay\"\n[when.set.grain]\namount = 5\n",
            Severity::Warning,
            "`amount` is above its maximum of 1.0; it is clamped to 1.0",
        );
        only(
            "[[when]]\non = \"view.replay\"\n[when.set.grain]\nsize = 0.5\n",
            Severity::Error,
            "`size` expects a whole number, not a decimal number",
        );
        only(
            "[[when]]\non = \"view.replay\"\n[when.set.grain]\namount = 1\n[when.set.grain]\namount = 2\n",
            Severity::Error,
            "[when.set.grain] appears twice in this rule",
        );
        only(
            "[[when]]\non = \"view.replay\"\n",
            Severity::Warning,
            "This rule sets nothing; add a `[when.set.<effect>]` table",
        );
    }

    #[test]
    fn signals_that_the_game_does_not_read_warn() {
        let s = "[when.set.grain]\namount = 1\n";
        only(
            &format!("[[when]]\non = \"session.weather\"\nis = \"storm\"\n{s}"),
            Severity::Warning,
            "session.weather isn't read in game yet; this rule only fires in the Preview",
        );
        let m = msgs(&format!("[[when]]\non = \"view.replay\"\n{s}"));
        assert!(m.is_empty(), "{m:?}");
        let both = msgs(&format!(
            "[[when]]\nall = [{{ on = \"session.weather\", is = \"storm\" }}, {{ on = \"session.weather\", is = \"heavy_rain\" }}]\n{s}"
        ));
        assert_eq!(both.len(), 1, "{both:?}");
    }

    #[test]
    fn limits_are_enforced() {
        let one = "[[when]]\non = \"view.replay\"\n[when.set.grain]\namount = 1\n";
        let seventeen = one.repeat(17);
        let m = msgs(&seventeen);
        assert_eq!(
            m.iter()
                .filter(|(_, t)| t.starts_with("A config has at most 16 rules"))
                .count(),
            1,
            "{m:?}"
        );
        assert!(
            lint(&format!("{HEAD}{}", one.repeat(16)), &catalog())
                .config
                .is_some()
        );
        let mut big = String::from("name = \"a\"\n");
        for i in 0..6 {
            big.push_str(&format!("[ambient_light.a{i}]\n"));
        }
        big.push_str("[[when]]\non = \"view.replay\"\n");
        for i in 0..6 {
            big.push_str(&format!(
                "[when.set.ambient_light.a{i}]\nintensity = 1\nlens = true\nadapt_black_level = \"low\"\ntint = [1, 1, 1]\ntaps = 4\noffset = [0, 0]\n"
            ));
        }
        let m: Vec<(Severity, String)> = lint(&big, &catalog())
            .diagnostics
            .into_iter()
            .map(|d| (d.severity, d.message))
            .collect();
        assert!(
            m.iter()
                .any(|(_, t)| t == "A rule has at most 32 overrides; this one has 36"),
            "{m:?}"
        );
    }

    #[test]
    fn the_bare_when_table_has_a_double_bracket_fix() {
        let text = format!("{HEAD}[when]\non = \"view.replay\"\n");
        let l = lint(&text, &catalog());
        let d = l
            .diagnostics
            .iter()
            .find(|d| d.message.contains("[[when]]"))
            .unwrap();
        assert_eq!(d.message, "`[when]` needs double brackets: `[[when]]`");
        let fix = d.fix.as_ref().unwrap();
        assert_eq!(
            (&text[fix.range.clone()], fix.replacement.as_str()),
            ("[when]", "[[when]]")
        );
        assert!(l.config.is_none());
    }

    #[test]
    fn a_config_with_rules_is_still_valid_toml_for_the_real_parser() {
        let text = format!(
            "{HEAD}[[when]]\nname = \"k\"\nall = [\n  {{ on = \"view.camera\", is = [\"cockpit\"] }},\n  {{ on = \"car.speed\", above = 150 }}, # c\n]\n[when.set.grain]\namount = 0.5\n"
        );
        let l = lint(&text, &catalog());
        assert!(l.config.is_some(), "{:?}", l.diagnostics);
        text.parse::<toml::Table>().unwrap();
    }
}
