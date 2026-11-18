//! Config checks (DESIGN.md §2).

use std::collections::{BTreeMap, HashSet};
use std::ops::Range;

use crate::config::{self, Elem, Lit, RawEntry, RawValue};
use crate::text::{Lines, did_you_mean, number};
use crate::{Catalog, Config, Diagnostic, Effect, Fix, Instance, Lint, Param, ParamKind, Severity, Value};

const TOP_KEYS: [&str; 4] = ["name", "author", "description", "swfx"];

pub(crate) fn bounds(kind: &ParamKind) -> (Option<f64>, Option<f64>) {
    match kind {
        ParamKind::Float { min, max, .. }
        | ParamKind::Float2 { min, max }
        | ParamKind::Color3 { min, max }
        | ParamKind::Color4 { min, max } => (*min, *max),
        ParamKind::Int { min, max } => (min.map(|v| v as f64), max.map(|v| v as f64)),
        ParamKind::Bool | ParamKind::Enum { .. } => (None, None),
    }
}

pub(crate) struct Sink<'a> {
    lines: Lines<'a>,
    out: Vec<Diagnostic>,
}

impl Sink<'_> {
    pub(crate) fn push(
        &mut self,
        severity: Severity,
        range: Range<usize>,
        message: String,
        fix: Option<Fix>,
    ) {
        let (start, end) = self.lines.range(&range);
        self.out.push(Diagnostic {
            severity,
            range,
            start,
            end,
            message,
            fix,
        });
    }

    pub(crate) fn error(&mut self, range: Range<usize>, message: impl Into<String>) {
        self.push(Severity::Error, range, message.into(), None);
    }
}

pub(crate) fn describe(lit: &Lit) -> &'static str {
    match lit {
        Lit::Int(_) => "a whole number",
        Lit::Float(_) => "a decimal number",
        Lit::Bool(_) => "true or false",
        Lit::Str(_) => "a string",
        Lit::Array(_) => "an array",
        Lit::Strs(_) => "an array of strings",
        Lit::Tables(_) => "an array of tables",
    }
}

fn clamp_one(
    sink: &mut Sink,
    key: &str,
    v: f64,
    range: &Range<usize>,
    (min, max): (Option<f64>, Option<f64>),
    float: bool,
) -> f64 {
    let bound = match (min, max) {
        (Some(m), _) if v < m => Some(("below its minimum", m)),
        (_, Some(m)) if v > m => Some(("above its maximum", m)),
        _ => None,
    };
    let Some((what, to)) = bound else { return v };
    let shown = number(to, float);
    sink.push(
        Severity::Warning,
        range.clone(),
        format!("`{key}` is {what} of {shown}; it is clamped to {shown}"),
        Some(Fix {
            title: format!("Clamp to {shown}"),
            range: range.clone(),
            replacement: shown.clone(),
        }),
    );
    to
}

pub(crate) fn check_param(sink: &mut Sink, p: &Param, v: &RawValue) -> Option<Value> {
    let key = &p.id;
    let range = &v.range;
    let wrong = |sink: &mut Sink, want: &str| {
        sink.error(
            range.clone(),
            format!("`{key}` expects {want}, not {}", describe(&v.lit)),
        );
        None
    };
    match (&p.kind, &v.lit) {
        (ParamKind::Float { .. }, Lit::Int(_) | Lit::Float(_)) => {
            let n = match v.lit {
                Lit::Int(i) => i as f64,
                Lit::Float(f) => f,
                _ => unreachable!(),
            };
            Some(Value::Number(clamp_one(
                sink,
                key,
                n,
                range,
                bounds(&p.kind),
                true,
            )))
        }
        (ParamKind::Float { .. }, _) => wrong(sink, "a number"),
        (ParamKind::Int { .. }, Lit::Int(i)) => Some(Value::Number(clamp_one(
            sink,
            key,
            *i as f64,
            range,
            bounds(&p.kind),
            false,
        ))),
        (ParamKind::Int { .. }, Lit::Float(_)) => wrong(sink, "a whole number"),
        (ParamKind::Int { .. }, _) => wrong(sink, "a whole number"),
        (ParamKind::Bool, Lit::Bool(b)) => Some(Value::Bool(*b)),
        (ParamKind::Bool, _) => wrong(sink, "true or false"),
        (ParamKind::Enum { values }, Lit::Str(s)) => {
            if values.contains(s) {
                return Some(Value::Text(s.clone()));
            }
            let list = values
                .iter()
                .map(|v| format!("\"{v}\""))
                .collect::<Vec<_>>()
                .join(", ");
            let fix = did_you_mean(s, values.iter().map(String::as_str)).map(|c| Fix {
                title: format!("Change to \"{c}\""),
                range: range.clone(),
                replacement: format!("\"{c}\""),
            });
            sink.push(
                Severity::Error,
                range.clone(),
                format!("\"{s}\" is not a value of `{key}`; use one of {list}"),
                fix,
            );
            None
        }
        (ParamKind::Enum { values }, _) => {
            let list = values
                .iter()
                .map(|v| format!("\"{v}\""))
                .collect::<Vec<_>>()
                .join(", ");
            sink.error(
                range.clone(),
                format!("`{key}` expects one of {list}, not {}", describe(&v.lit)),
            );
            None
        }
        (
            k @ (ParamKind::Float2 { .. } | ParamKind::Color3 { .. } | ParamKind::Color4 { .. }),
            Lit::Array(items),
        ) => {
            let n = k.width();
            if items.len() != n {
                sink.error(
                    range.clone(),
                    format!("`{key}` expects {n} numbers, not {}", items.len()),
                );
                return None;
            }
            let out: Vec<f64> = items
                .iter()
                .map(|Elem { num, range, .. }| clamp_one(sink, key, *num, range, bounds(k), true))
                .collect();
            Some(Value::Vector(out))
        }
        (k, _) => wrong(sink, &format!("an array of {} numbers", k.width())),
    }
}

fn first_line(text: &str) -> Range<usize> {
    let start = text.len() - text.trim_start().len();
    let end = text[start..].find('\n').map_or(text.len(), |i| start + i);
    start..text[..end].trim_end().len().max(start)
}

fn top_level(
    sink: &mut Sink,
    text: &str,
    top: &[RawEntry],
) -> (Option<String>, Option<String>, Option<String>) {
    let (mut name, mut author, mut description) = (None, None, None);
    for e in top {
        let slot = match e.key.as_str() {
            "name" => &mut name,
            "author" => &mut author,
            "description" => &mut description,
            "swfx" => {
                if e.value.lit != Lit::Int(1) {
                    sink.error(
                        e.value.range.clone(),
                        "`swfx` must be 1, the only config format version",
                    );
                }
                continue;
            }
            other => {
                let fix = did_you_mean(other, TOP_KEYS.into_iter()).map(|c| Fix {
                    title: format!("Change to `{c}`"),
                    range: e.key_range.clone(),
                    replacement: c.to_owned(),
                });
                sink.push(
                    Severity::Warning,
                    e.key_range.clone(),
                    format!("Unknown top-level key `{other}`; it is ignored"),
                    fix,
                );
                continue;
            }
        };
        match &e.value.lit {
            Lit::Str(s) if e.key != "name" || !s.trim().is_empty() => *slot = Some(s.clone()),
            Lit::Str(_) => sink.error(e.value.range.clone(), "`name` must not be empty"),
            other => sink.error(
                e.value.range.clone(),
                format!("`{}` expects a string, not {}", e.key, describe(other)),
            ),
        }
    }
    if name.is_none() && !top.iter().any(|e| e.key == "name") {
        let at = first_line(text);
        let fix = Fix {
            title: "Add a name".into(),
            range: 0..0,
            replacement: "name = \"Untitled\"\n".into(),
        };
        sink.push(
            Severity::Error,
            at,
            "A config needs a `name = \"...\"`".into(),
            Some(fix),
        );
    }
    (name, author, description)
}

fn instance(sink: &mut Sink, effect: &Effect, table: &config::RawTable) -> Instance {
    let mut enabled = true;
    let mut values = BTreeMap::new();
    for e in &table.entries {
        if e.key == "enabled" {
            match e.value.lit {
                Lit::Bool(b) => enabled = b,
                _ => sink.error(
                    e.value.range.clone(),
                    format!("`enabled` expects true or false, not {}", describe(&e.value.lit)),
                ),
            }
            continue;
        }
        let Some(p) = effect.params.iter().find(|p| p.id == e.key) else {
            let names = effect.params.iter().map(|p| p.id.as_str()).chain(["enabled"]);
            let fix = did_you_mean(&e.key, names).map(|c| Fix {
                title: format!("Change to `{c}`"),
                range: e.key_range.clone(),
                replacement: c.to_owned(),
            });
            sink.push(
                Severity::Error,
                e.key_range.clone(),
                format!("`{}` has no param `{}`", effect.id, e.key),
                fix,
            );
            continue;
        };
        if let Some(v) = check_param(sink, p, &e.value) {
            values.insert(p.id.clone(), v);
        }
    }
    Instance {
        effect: effect.id.clone(),
        label: table.label.clone(),
        enabled,
        values,
        span: table.span.clone(),
    }
}

/// Parses and checks a config against the catalog (DESIGN.md §2).
pub fn lint(text: &str, catalog: &Catalog) -> Lint {
    let doc = config::parse(text);
    let mut sink = Sink {
        lines: Lines::new(text),
        out: Vec::new(),
    };
    for e in &doc.errors {
        sink.error(e.range.clone(), e.message.clone());
    }
    let (name, author, description) = top_level(&mut sink, text, &doc.top);

    let mut instances = Vec::new();
    let mut seen: HashSet<(String, Option<String>)> = HashSet::new();
    for table in doc.tables.iter().filter(|t| !t.bad) {
        if table.effect == "when" && table.label.is_none() {
            let fix = Fix {
                title: "Change to `[[when]]`".into(),
                range: table.header.clone(),
                replacement: "[[when]]".into(),
            };
            let msg = "`[when]` needs double brackets: `[[when]]`".to_owned();
            sink.push(Severity::Error, table.header.clone(), msg, Some(fix));
            continue;
        }
        let Some(effect) = catalog.get(&table.effect) else {
            let fix = catalog.did_you_mean(&table.effect).map(|c| Fix {
                title: format!("Change to `{c}`"),
                range: table.effect_range.clone(),
                replacement: c.to_owned(),
            });
            let msg = format!(
                "Unknown effect `{}`; if it comes from an addon, that addon is not installed",
                table.effect
            );
            sink.push(Severity::Error, table.effect_range.clone(), msg, fix);
            continue;
        };
        if !seen.insert((table.effect.clone(), table.label.clone())) {
            sink.error(
                table.header.clone(),
                format!(
                    "`[{}]` appears twice; give the second a label, like `[{}.second]`",
                    table.effect, table.effect
                ),
            );
            continue;
        }
        if let Some(label) = &table.label
            && effect.params.iter().any(|p| &p.id == label)
        {
            sink.error(
                table.header.clone(),
                format!("The label `{label}` is also a param of `{}`", effect.id),
            );
        }
        instances.push(instance(&mut sink, effect, table));
    }

    let rules = crate::when::check(&mut sink, &doc, &instances, catalog);

    let has_errors = sink.out.iter().any(|d| d.severity == Severity::Error);
    if !has_errors && !instances.iter().any(|i| i.enabled) {
        let at = doc
            .top
            .iter()
            .find(|e| e.key == "name")
            .map_or_else(|| first_line(text), |e| e.range.clone());
        sink.push(Severity::Info, at, "This config changes nothing".into(), None);
    }
    let mut diagnostics = sink.out;
    diagnostics.sort_by_key(|d| (d.range.start, d.range.end));
    let config = (!has_errors).then(|| Config {
        name: name.unwrap_or_default(),
        author,
        description,
        instances,
        rules,
    });
    Lint { config, diagnostics }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{Origin, parse_effect};

    pub(crate) fn catalog() -> Catalog {
        let ambient = parse_effect(crate::effect::tests::AMBIENT, Origin::Library).unwrap();
        let grain = parse_effect(
            r#"{"id":"grain","name":"Grain","description":"Film grain.","params":[
                {"id":"amount","type":"float","default":0.1,"min":0,"max":1,"label":"Amount","doc":"Strength."},
                {"id":"size","type":"int","default":2,"min":1,"max":8}],
              "passes":[{"name":"p","shader":"grain.hlsl"}]}"#,
            Origin::Library,
        )
        .unwrap();
        let bloom = parse_effect(
            r#"{"id":"bloom_wide","name":"Wide bloom","params":[],"passes":[{"name":"p","shader":"b.hlsl"}]}"#,
            Origin::Addon { id: "a.b".into(), name: "Addon".into() },
        )
        .unwrap();
        Catalog {
            effects: vec![ambient, grain, bloom],
        }
    }

    pub(crate) const GOOD: &str = "# Realistic look
name = \"Realistic\"
author = 'Swofty'
description = \"Flatter contrast.\"

[ambient_light]
intensity = 1.5
lens = true
adapt_black_level = \"low\"
tint = [1, 1.5, 0.25]   # r g b
taps = 8

[grain]
amount = 0.2

[grain.heavy]
enabled = false
amount = 1
size = 4
";

    fn msgs(text: &str) -> Vec<(Severity, String)> {
        lint(text, &catalog())
            .diagnostics
            .into_iter()
            .map(|d| (d.severity, d.message))
            .collect()
    }

    fn has(text: &str, sev: Severity, needle: &str) -> bool {
        msgs(text).iter().any(|(s, m)| *s == sev && m.contains(needle))
    }

    #[test]
    fn a_good_config_lints_clean_and_matches_a_real_toml_parse() {
        let l = lint(GOOD, &catalog());
        assert!(l.diagnostics.is_empty(), "{:?}", l.diagnostics);
        let c = l.config.unwrap();
        assert_eq!(c.name, "Realistic");
        assert_eq!(c.author.as_deref(), Some("Swofty"));
        assert_eq!(c.instances.len(), 3);
        assert_eq!(c.instances[2].label.as_deref(), Some("heavy"));
        assert!(!c.instances[2].enabled);
        assert_eq!(c.instances[0].values["tint"], Value::Vector(vec![1.0, 1.5, 0.25]));
        let t: toml::Table = GOOD.parse().unwrap();
        assert_eq!(t["name"].as_str(), Some("Realistic"));
        assert_eq!(t["ambient_light"]["taps"].as_integer(), Some(8));
        assert_eq!(t["grain"]["heavy"]["size"].as_integer(), Some(4));
    }

    #[test]
    fn every_lint_clean_text_is_valid_toml() {
        for text in [
            GOOD,
            "name = \"x\"\n",
            "name=\"x\"\n[grain]\namount=1e-1\n",
            "name = \"t\\u00e9\"\r\n[grain]\r\nsize = 1_0\r\n",
        ] {
            let l = lint(text, &catalog());
            if l.config.is_some() {
                text.parse::<toml::Table>()
                    .unwrap_or_else(|e| panic!("{text:?}: {e}"));
            }
        }
    }

    #[test]
    fn the_name_is_required_with_a_fix() {
        let l = lint("[grain]\namount = 1\n", &catalog());
        assert!(l.config.is_none());
        let d = &l.diagnostics[0];
        assert_eq!(d.severity, Severity::Error);
        let fix = d.fix.as_ref().unwrap();
        assert_eq!(
            (fix.range.clone(), fix.replacement.as_str()),
            (0..0, "name = \"Untitled\"\n")
        );
        let mut fixed = String::from("[grain]\namount = 1\n");
        fixed.insert_str(0, &fix.replacement);
        assert!(lint(&fixed, &catalog()).config.is_some());
    }

    #[test]
    fn name_type_and_emptiness_are_checked() {
        assert!(has("name = 3", Severity::Error, "expects a string"));
        assert!(has("name = \"  \"", Severity::Error, "must not be empty"));
        assert!(has(
            "name = \"a\"\nauthor = 1",
            Severity::Error,
            "`author` expects a string"
        ));
        assert!(has("name = \"a\"\nswfx = 2", Severity::Error, "must be 1"));
        assert!(!has("name = \"a\"\nswfx = 1\n[grain]", Severity::Error, "swfx"));
    }

    #[test]
    fn unknown_top_level_keys_warn_with_a_suggestion() {
        let l = lint("name = \"a\"\nauthr = \"x\"\n[grain]\n", &catalog());
        let d = &l.diagnostics[0];
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.fix.as_ref().unwrap().replacement, "author");
        assert!(l.config.is_some());
    }

    #[test]
    fn unknown_effects_get_did_you_mean() {
        let text = "name = \"a\"\n[grane]\namount = 1\n";
        let l = lint(text, &catalog());
        assert!(l.config.is_none());
        let d = &l.diagnostics[0];
        assert!(d.message.contains("Unknown effect `grane`") && d.message.contains("addon"));
        assert_eq!(&text[d.range.clone()], "grane");
        let fix = d.fix.as_ref().unwrap();
        assert_eq!(
            (&text[fix.range.clone()], fix.replacement.as_str()),
            ("grane", "grain")
        );
        assert!(
            lint("name = \"a\"\n[qqqqqqqq]\n", &catalog()).diagnostics[0]
                .fix
                .is_none()
        );
    }

    #[test]
    fn unknown_params_get_did_you_mean() {
        let text = "name = \"a\"\n[grain]\namout = 0.5\n";
        let d = &lint(text, &catalog()).diagnostics[0];
        assert_eq!(&text[d.range.clone()], "amout");
        assert_eq!(d.fix.as_ref().unwrap().replacement, "amount");
        assert!(has(
            "name = \"a\"\n[grain]\nenabld = true\n",
            Severity::Error,
            "no param"
        ));
    }

    #[test]
    fn the_same_table_twice_is_an_error() {
        assert!(has(
            "name = \"a\"\n[grain]\n[grain]\n",
            Severity::Error,
            "appears twice"
        ));
        assert!(!has(
            "name = \"a\"\n[grain]\n[grain.b]\n",
            Severity::Error,
            "twice"
        ));
        assert!(has(
            "name = \"a\"\n[ambient_light]\n[ambient_light.lens]\n",
            Severity::Error,
            "also a param"
        ));
    }

    #[test]
    fn types_are_checked() {
        let g = |line: &str| format!("name = \"a\"\n[ambient_light]\n{line}\n");
        assert!(has(&g("intensity = \"x\""), Severity::Error, "expects a number"));
        assert!(has(&g("intensity = true"), Severity::Error, "expects a number"));
        assert!(!has(&g("intensity = 2"), Severity::Error, ""));
        assert!(has(&g("taps = 2.5"), Severity::Error, "whole number"));
        assert!(has(&g("lens = 1"), Severity::Error, "true or false"));
        assert!(has(&g("tint = [1, 2]"), Severity::Error, "expects 3 numbers"));
        assert!(has(&g("tint = 1"), Severity::Error, "array of 3"));
        assert!(has(
            &g("adapt_black_level = 2"),
            Severity::Error,
            "expects one of"
        ));
        assert!(has(&g("enabled = 1"), Severity::Error, "`enabled`"));
    }

    #[test]
    fn enum_values_get_did_you_mean() {
        let text = "name = \"a\"\n[ambient_light]\nadapt_black_level = \"hig\"\n";
        let d = &lint(text, &catalog()).diagnostics[0];
        assert!(d.message.contains("\"off\", \"low\", \"high\""));
        let fix = d.fix.as_ref().unwrap();
        assert_eq!(fix.replacement, "\"high\"");
        assert_eq!(&text[fix.range.clone()], "\"hig\"");
    }

    #[test]
    fn out_of_range_values_warn_clamp_and_offer_a_fix() {
        let text = "name = \"a\"\n[grain]\namount = 5\nsize = 0\n";
        let l = lint(text, &catalog());
        assert!(l.config.is_some());
        let warns: Vec<_> = l
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .collect();
        assert_eq!(warns.len(), 2);
        assert_eq!(warns[0].fix.as_ref().unwrap().title, "Clamp to 1.0");
        assert_eq!(warns[1].fix.as_ref().unwrap().replacement, "1");
        assert_eq!(&text[warns[0].fix.as_ref().unwrap().range.clone()], "5");
        let c = l.config.unwrap();
        assert_eq!(c.instances[0].values["amount"], Value::Number(1.0));
        assert_eq!(c.instances[0].values["size"], Value::Number(1.0));
        let v = lint("name = \"a\"\n[ambient_light]\ntint = [3, 1, -1]\n", &catalog());
        assert_eq!(v.diagnostics.len(), 2);
        assert_eq!(
            v.config.unwrap().instances[0].values["tint"],
            Value::Vector(vec![2.0, 1.0, 0.0])
        );
    }

    #[test]
    fn a_config_with_nothing_enabled_says_so() {
        assert!(has("name = \"a\"\n", Severity::Info, "changes nothing"));
        assert!(has(
            "name = \"a\"\n[grain]\nenabled = false\n",
            Severity::Info,
            "changes nothing"
        ));
        assert!(!has(GOOD, Severity::Info, "nothing"));
        assert!(lint("name = \"a\"\n", &catalog()).config.is_some());
    }

    #[test]
    fn syntax_errors_and_semantic_errors_come_together_sorted() {
        let text = "name = \"a\"\n[grane]\nx = \n[grain]\namout = 1\n";
        let l = lint(text, &catalog());
        assert!(l.errors() >= 3, "{:?}", l.diagnostics);
        let starts: Vec<_> = l.diagnostics.iter().map(|d| d.range.start).collect();
        assert!(starts.windows(2).all(|w| w[0] <= w[1]));
        assert!(l.config.is_none());
    }

    #[test]
    fn positions_are_utf16_columns() {
        let text = "name = \"é😀\"\n[grane]\n";
        let l = lint(text, &catalog());
        let d = l
            .diagnostics
            .iter()
            .find(|d| d.message.contains("Unknown effect"))
            .unwrap();
        assert_eq!((d.start.line, d.start.character), (1, 1));
        assert_eq!((d.end.line, d.end.character), (1, 6));
        let text = "name = \"é😀\"\n[grain]\nsize = 0.5\n";
        let d = &lint(text, &catalog()).diagnostics[0];
        assert_eq!((d.start.line, d.start.character), (2, 7));
    }

    #[test]
    fn instance_spans_cover_header_to_last_line() {
        let text = "name = \"a\"\n[grain]\namount = 1\n\n[ambient_light]\nlens = true\n";
        let c = lint(text, &catalog()).config.unwrap();
        assert_eq!(&text[c.instances[0].span.clone()], "[grain]\namount = 1");
        assert_eq!(&text[c.instances[1].span.clone()], "[ambient_light]\nlens = true");
    }

    #[test]
    fn addon_effects_resolve_like_library_ones() {
        let l = lint("name = \"a\"\n[bloom_wide]\n", &catalog());
        assert!(l.config.is_some(), "{:?}", l.diagnostics);
    }
}
