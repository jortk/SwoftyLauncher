use std::collections::{BTreeMap, BTreeSet};

use crate::builtin::CORE_ID;
use crate::folders::Key;
use crate::manifest::AddonManifest;
use crate::profile::Profile;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Missing,
    Off,
    Below,
    Version { wanted: String, found: String },
    Conflict,
    Waits,
    Career { label: String, why: Box<Kind> },
    Clash { why: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Red,
    Amber,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub addon: String,
    pub other: String,
    pub kind: Kind,
}

impl Problem {
    pub fn blocks_play(&self) -> bool {
        matches!(
            self.kind,
            Kind::Missing | Kind::Off | Kind::Version { .. } | Kind::Conflict
        )
    }

    pub fn severity(&self) -> Severity {
        match self.kind {
            Kind::Clash { .. } => Severity::Amber,
            _ => Severity::Red,
        }
    }

    pub fn fixable_by_reorder(&self) -> bool {
        matches!(self.kind, Kind::Below | Kind::Waits)
    }

    pub fn line(&self, name: &dyn Fn(&str) -> String) -> String {
        let other = name(&self.other);
        match &self.kind {
            Kind::Missing => format!("Needs {other}, which isn't installed"),
            Kind::Off => format!("Needs {other}, which is off"),
            Kind::Below => format!("Needs {other} above it"),
            Kind::Version { wanted, found } => format!("Needs {other} {wanted}; {found} is installed"),
            Kind::Conflict => format!("Can't be on with {other}"),
            Kind::Waits => format!("Changes rows that {other} adds, so it needs {other} above it"),
            Kind::Career { label, why } => {
                let me = name(&self.addon);
                match &**why {
                    Kind::Missing => format!("Your career {label} needs {me}, which isn't installed"),
                    Kind::Off => format!("Your career {label} needs {me}, which is off"),
                    Kind::Version { wanted, found } => {
                        format!("Your career {label} needs {me} {wanted}; {found} is installed")
                    }
                    _ => format!("Your career {label} needs {me}"),
                }
            }
            Kind::Clash { why } => format!("{why} (with {other})"),
        }
    }
}

fn glob_match(pattern: &str, id: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => id.starts_with(prefix),
        None => pattern == id,
    }
}

pub fn is_pinned(id: &str) -> bool {
    id == CORE_ID || crate::managed::is_managed(id)
}

fn enabled_ids(profile: &Profile) -> Vec<&str> {
    match profile.is_vanilla() {
        true => Vec::new(),
        false => profile.enabled().collect(),
    }
}

pub fn problems(
    profile: &Profile,
    installed: &BTreeMap<String, AddonManifest>,
    pinned: &dyn Fn(&str) -> bool,
) -> Vec<Problem> {
    let on = enabled_ids(profile);
    let place: BTreeMap<&str, usize> = on.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut out = Vec::new();
    for id in &on {
        let Some(m) = installed.get(*id) else {
            continue;
        };
        let at = place[*id];
        let movable = !pinned(id);
        let mut push = |other: &str, kind| {
            out.push(Problem {
                addon: (*id).to_owned(),
                other: other.to_owned(),
                kind,
            });
        };
        for r in &m.requires {
            match (place.get(r.id.as_str()), installed.get(&r.id)) {
                (None, Some(_)) => push(&r.id, Kind::Off),
                (None, None) => push(&r.id, Kind::Missing),
                (Some(_), None) => push(&r.id, Kind::Missing),
                (Some(_), Some(dep)) if !crate::version::satisfies(&r.version, &dep.version) => push(
                    &r.id,
                    Kind::Version {
                        wanted: r.version.to_string(),
                        found: dep.version.to_string(),
                    },
                ),
                (Some(&dep_at), Some(_)) if movable && dep_at > at => push(&r.id, Kind::Below),
                _ => {}
            }
        }
        for pat in m.load_after.iter().filter(|_| movable) {
            for (other, &other_at) in &place {
                let listed_requirement = m.requires.iter().any(|r| r.id == *other);
                if *other != *id && !listed_requirement && glob_match(pat, other) && other_at > at {
                    push(other, Kind::Below);
                }
            }
        }
        for c in m.conflicts.iter().filter(|c| c.touches.is_none()) {
            for other in place.keys().filter(|o| **o != *id && glob_match(&c.id, o)) {
                push(other, Kind::Conflict);
            }
        }
    }
    out
}

pub fn from_report(
    profile: &Profile,
    report: &crate::conflicts::Report,
    pinned: &dyn Fn(&str) -> bool,
) -> Vec<Problem> {
    let at: BTreeMap<&str, usize> = profile.enabled().enumerate().map(|(i, id)| (id, i)).collect();
    learned_waits(profile, report, pinned)
        .into_iter()
        .filter(|w| at[w.other.as_str()] > at[w.addon.as_str()])
        .collect()
}

fn learned_waits(
    profile: &Profile,
    report: &crate::conflicts::Report,
    pinned: &dyn Fn(&str) -> bool,
) -> Vec<Problem> {
    if profile.is_vanilla() || !report.is_current(profile) {
        return Vec::new();
    }
    let on: BTreeSet<&str> = profile.enabled().collect();
    report
        .waits
        .iter()
        .filter(|w| !pinned(&w.addon) && w.addon != w.on)
        .filter(|w| on.contains(w.addon.as_str()) && on.contains(w.on.as_str()))
        .map(|w| Problem {
            addon: w.addon.clone(),
            other: w.on.clone(),
            kind: Kind::Waits,
        })
        .collect()
}

pub fn careers(
    profile: &Profile,
    installed: &BTreeMap<String, AddonManifest>,
    list: &[crate::stdlib::careers::Career],
) -> Vec<Problem> {
    if profile.is_vanilla() {
        return Vec::new();
    }
    let order: Vec<String> = profile.enabled().map(str::to_owned).collect();
    let mut out = Vec::new();
    for c in list.iter().filter(|c| !c.is_gone()) {
        if c.profile.as_deref().is_some_and(|p| p != profile.name) {
            continue;
        }
        let label = match c.label.is_empty() {
            true => c.career.clone(),
            false => c.label.clone(),
        };
        for a in &c.addons {
            let range = a
                .need
                .as_deref()
                .and_then(|n| crate::stdlib::range::Range::parse(n).ok())
                .unwrap_or_else(crate::stdlib::range::Range::any);
            let why = match installed.get(&a.id) {
                None => Kind::Missing,
                Some(_) if !order.contains(&a.id) => Kind::Off,
                Some(m) if !range.matches(&m.version) => Kind::Version {
                    wanted: range.to_string(),
                    found: m.version.to_string(),
                },
                Some(_) => continue,
            };
            out.push(Problem {
                addon: a.id.clone(),
                other: c.career.clone(),
                kind: Kind::Career {
                    label: label.clone(),
                    why: Box::new(why),
                },
            });
        }
    }
    out
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ClashRule {
    a: String,
    b: String,
    why: String,
}

const CLASHES: &str = include_str!("order_clashes.json");

pub fn clashes(
    profile: &Profile,
    installed: &BTreeMap<String, AddonManifest>,
    is_track: &dyn Fn(&str) -> bool,
) -> Vec<Problem> {
    let rules: Vec<ClashRule> = serde_json::from_str(CLASHES).unwrap_or_default();
    let on: Vec<&str> = enabled_ids(profile);
    let says = |id: &str, what: &str| match what {
        "track" => is_track(id),
        key => {
            let name = installed
                .get(id)
                .map(|m| m.name.to_lowercase())
                .unwrap_or_default();
            id.to_lowercase().contains(key) || name.contains(key)
        }
    };
    let mut out = Vec::new();
    for r in &rules {
        for a in on.iter().filter(|id| says(id, &r.a)) {
            for b in on.iter().filter(|id| **id != *a && says(id, &r.b)) {
                out.push(Problem {
                    addon: (*a).to_owned(),
                    other: (*b).to_owned(),
                    kind: Kind::Clash { why: r.why.clone() },
                });
            }
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixPlan {
    pub order: Vec<String>,
    pub ejected: Vec<(String, Key)>,
    pub moved: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixError {
    Cycle(Vec<String>),
}

impl FixError {
    pub fn members(&self) -> &[String] {
        match self {
            Self::Cycle(ids) => ids,
        }
    }

    /// The addon to turn off to break the cycle: the one that loads last.
    pub fn turn_off(&self) -> Option<&str> {
        self.members().last().map(String::as_str)
    }

    pub fn line(&self, name: &dyn Fn(&str) -> String) -> String {
        let names: Vec<String> = self.members().iter().map(|id| name(id)).collect();
        format!(
            "These addons need each other above themselves, so no order works: {}. Turn one of them off.",
            names.join(", ")
        )
    }
}

impl std::fmt::Display for FixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.line(&str::to_owned))
    }
}

impl From<FixError> for Error {
    fn from(e: FixError) -> Self {
        Error::Resolve(e.to_string())
    }
}

fn requirements(
    ids: &[String],
    installed: &BTreeMap<String, AddonManifest>,
    learned: &[Problem],
    pinned: &dyn Fn(&str) -> bool,
) -> Vec<BTreeSet<usize>> {
    let index: BTreeMap<&str, usize> = ids.iter().enumerate().map(|(i, id)| (id.as_str(), i)).collect();
    let mut before: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); ids.len()];
    for (i, id) in ids.iter().enumerate().filter(|(_, id)| !pinned(id)) {
        let m = &installed[id];
        for r in &m.requires {
            if let Some(&d) = index.get(r.id.as_str()) {
                before[i].insert(d);
            }
        }
        for pat in &m.load_after {
            before[i].extend(
                ids.iter()
                    .enumerate()
                    .filter(|(j, o)| *j != i && glob_match(pat, o))
                    .map(|(j, _)| j),
            );
        }
    }
    for p in learned
        .iter()
        .filter(|p| p.kind == Kind::Waits && !pinned(&p.addon))
    {
        if let (Some(&a), Some(&o)) = (index.get(p.addon.as_str()), index.get(p.other.as_str()))
            && a != o
            && !reaches(&before, o, a)
        {
            before[a].insert(o);
        }
    }
    before
}

fn topological(
    ids: &[String],
    before: &[BTreeSet<usize>],
    pinned: &dyn Fn(&str) -> bool,
) -> std::result::Result<Vec<usize>, FixError> {
    let mut order: Vec<usize> = Vec::with_capacity(ids.len());
    let mut done = vec![false; ids.len()];
    while order.len() < ids.len() {
        let ready = |i: &usize| !done[*i] && before[*i].iter().all(|&d| done[d]);
        let next = (0..ids.len())
            .filter(ready)
            .find(|&i| pinned(&ids[i]))
            .or_else(|| (0..ids.len()).find(ready));
        let Some(next) = next else {
            let stuck: Vec<usize> = (0..ids.len()).filter(|&i| !done[i]).collect();
            let cycle = stuck
                .iter()
                .copied()
                .filter(|&n| before[n].iter().any(|&p| reaches(before, p, n)))
                .map(|n| ids[n].clone())
                .collect();
            return Err(FixError::Cycle(cycle));
        };
        done[next] = true;
        order.push(next);
    }
    Ok(order)
}

/// Orders the profile so every addon loads below what it needs, the way the
/// Addons page will gather it: an addon that its folder would pull above a
/// requirement leaves the folder.
pub fn plan_fix(
    profile: &Profile,
    installed: &BTreeMap<String, AddonManifest>,
    learned: &[Problem],
    pinned: &dyn Fn(&str) -> bool,
    place: &dyn Fn(&str) -> Option<Key>,
) -> std::result::Result<FixPlan, FixError> {
    let current: Vec<String> = profile.addons.iter().map(|a| a.id.clone()).collect();
    if profile.is_vanilla() {
        return Ok(FixPlan {
            order: current,
            ejected: Vec::new(),
            moved: 0,
        });
    }
    let slots: Vec<usize> = profile
        .addons
        .iter()
        .enumerate()
        .filter(|(_, a)| (a.enabled || a.id == CORE_ID) && installed.contains_key(&a.id))
        .map(|(i, _)| i)
        .collect();
    let ids: Vec<String> = slots.iter().map(|&i| current[i].clone()).collect();
    let before = requirements(&ids, installed, learned, pinned);
    let permutation = topological(&ids, &before, pinned)?;
    let mut candidate = current.clone();
    for (slot, &o) in slots.iter().zip(&permutation) {
        candidate[*slot] = ids[o].clone();
    }
    let edges: Vec<(&str, &str)> = before
        .iter()
        .enumerate()
        .flat_map(|(i, deps)| deps.iter().map(move |&d| (i, d)))
        .map(|(i, d)| (ids[i].as_str(), ids[d].as_str()))
        .collect();
    let mut ejected: Vec<(String, Key)> = Vec::new();
    let order = loop {
        let placed = |id: &str| match ejected.iter().any(|(e, _)| e == id) {
            true => None,
            false => place(id),
        };
        let regrouped = crate::folders::flatten(&crate::folders::group(candidate.iter().cloned(), placed));
        let at: BTreeMap<&str, usize> = regrouped
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        let mut offenders: BTreeSet<&str> = BTreeSet::new();
        for (a, d) in &edges {
            if at[a] < at[d] && !pinned(a) && placed(a).is_some_and(|k| k != Key::BuiltIn) {
                offenders.insert(a);
            }
        }
        if offenders.is_empty() {
            break regrouped;
        }
        let leaving: Vec<(String, Key)> = offenders
            .into_iter()
            .filter_map(|a| place(a).map(|k| (a.to_owned(), k)))
            .collect();
        ejected.extend(leaving);
    };
    let moved = order.iter().zip(&current).filter(|(a, b)| a != b).count();
    Ok(FixPlan {
        order,
        ejected,
        moved,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixState {
    Nothing,
    Ready(FixPlan, usize),
    Blocked(FixError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnosis {
    pub problems: Vec<Problem>,
    pub fix: FixState,
}

impl Diagnosis {
    pub fn empty() -> Self {
        Self {
            problems: Vec::new(),
            fix: FixState::Nothing,
        }
    }

    /// Whether every problem listed has a move: the order ones need a ready fix.
    pub fn actionable(&self) -> bool {
        let ordered = self.problems.iter().any(Problem::fixable_by_reorder);
        !ordered || matches!(self.fix, FixState::Ready(..) | FixState::Blocked(_))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn diagnose(
    profile: &Profile,
    installed: &BTreeMap<String, AddonManifest>,
    report: Option<&crate::conflicts::Report>,
    careers_list: &[crate::stdlib::careers::Career],
    is_track: &dyn Fn(&str) -> bool,
    pinned: &dyn Fn(&str) -> bool,
    place: &dyn Fn(&str) -> Option<Key>,
) -> Diagnosis {
    if profile.is_vanilla() {
        return Diagnosis::empty();
    }
    let mut found = problems(profile, installed, pinned);
    let mut learned = Vec::new();
    if let Some(r) = report {
        found.extend(from_report(profile, r, pinned));
        learned = learned_waits(profile, r, pinned);
    }
    let fix = match plan_fix(profile, installed, &learned, pinned, place) {
        Err(e) => FixState::Blocked(e),
        Ok(plan) => {
            let mut after = profile.clone();
            crate::folders::reorder(&mut after, &plan.order, &|_| false);
            let still = problems(&after, installed, pinned);
            let unhonoured: Vec<Problem> = learned
                .iter()
                .filter(|w| !wait_holds(&after, w))
                .cloned()
                .collect();
            found.retain(|p| !unhonoured.contains(p));
            let clears: BTreeSet<&str> = found
                .iter()
                .filter(|p| p.fixable_by_reorder())
                .map(|p| p.addon.as_str())
                .collect();
            let leftover = still.iter().any(|p| p.kind == Kind::Below);
            match plan.moved > 0 && !clears.is_empty() && !leftover {
                true => FixState::Ready(plan.clone(), clears.len()),
                false => FixState::Nothing,
            }
        }
    };
    found.extend(careers(profile, installed, careers_list));
    found.extend(clashes(profile, installed, is_track));
    Diagnosis { problems: found, fix }
}

fn wait_holds(profile: &Profile, w: &Problem) -> bool {
    let at = |id: &str| profile.enabled().position(|x| x == id);
    matches!((at(&w.addon), at(&w.other)), (Some(a), Some(o)) if o < a)
}

fn reaches(before: &[BTreeSet<usize>], from: usize, to: usize) -> bool {
    let mut seen = BTreeSet::new();
    let mut stack = vec![from];
    while let Some(n) = stack.pop() {
        if n == to {
            return true;
        }
        if seen.insert(n) {
            stack.extend(before[n].iter().copied());
        }
    }
    false
}

pub fn move_above(profile: &mut Profile, id: &str, anchor: &str) -> Result<()> {
    if is_pinned(id) {
        return Err(Error::Other(format!("{id} always loads first")));
    }
    if is_pinned(anchor) {
        return Err(Error::Other(format!("nothing loads above {anchor}")));
    }
    let from = profile.addons.iter().position(|a| a.id == id);
    let anchor_at = profile.addons.iter().position(|a| a.id == anchor);
    let (Some(from), Some(anchor_at)) = (from, anchor_at) else {
        return Err(Error::Other(format!(
            "{id} and {anchor} must both be in {}",
            profile.name
        )));
    };
    if from > anchor_at {
        let a = profile.addons.remove(from);
        profile.addons.insert(anchor_at, a);
    }
    Ok(())
}

pub fn lines(profile: &Profile, installed: &BTreeMap<String, AddonManifest>) -> Vec<String> {
    let name = |id: &str| {
        installed
            .get(id)
            .map_or_else(|| id.to_owned(), |m| m.name.clone())
    };
    problems(profile, installed, &is_pinned)
        .iter()
        .map(|p| format!("{}: {}", name(&p.addon), p.line(&name)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, extra: &str) -> (String, AddonManifest) {
        let json = format!(
            r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t",
            "compat":{{"game":"f1_25"}}{extra}}}"#
        );
        (id.to_owned(), AddonManifest::parse(&json).unwrap())
    }

    fn profile(ids: &[&str]) -> Profile {
        let mut p = Profile::new("t");
        for id in ids {
            p.set_enabled(id, true).unwrap();
        }
        p
    }

    fn ids(p: &Profile) -> Vec<&str> {
        p.addons.iter().map(|a| a.id.as_str()).collect()
    }

    fn installed() -> BTreeMap<String, AddonManifest> {
        BTreeMap::from([
            m(CORE_ID, ""),
            m("cs.base", ""),
            m("cs.2016", r#","requires":[{"id":"cs.base"}]"#),
            m(
                "c.career",
                r#","requires":[{"id":"cs.base","version":">=2.0.0"}]"#,
            ),
            m("c.grid", r#","load_after":["track.*"]"#),
            m("track.a", ""),
            m("track.b", ""),
            m("c.rival", r#","conflicts":[{"id":"cs.*"}]"#),
        ])
    }

    fn kinds(p: &Profile) -> Vec<(String, String, Kind)> {
        problems(p, &installed(), &is_pinned)
            .into_iter()
            .map(|x| (x.addon, x.other, x.kind))
            .collect()
    }

    #[test]
    fn a_dependency_below_is_reported() {
        let p = profile(&["cs.2016", "cs.base"]);
        assert_eq!(kinds(&p), [("cs.2016".into(), "cs.base".into(), Kind::Below)]);
        assert!(!problems(&p, &installed(), &is_pinned)[0].blocks_play());
        assert!(problems(&profile(&["cs.base", "cs.2016"]), &installed(), &is_pinned).is_empty());
    }

    #[test]
    fn a_dependency_that_is_off_or_missing_is_reported() {
        let mut p = profile(&["cs.2016"]);
        p.set_enabled("cs.base", false).unwrap();
        assert_eq!(kinds(&p), [("cs.2016".into(), "cs.base".into(), Kind::Off)]);
        let mut gone = installed();
        gone.remove("cs.base");
        let found = problems(&profile(&["cs.2016"]), &gone, &is_pinned);
        assert_eq!(found[0].kind, Kind::Missing);
        assert!(found[0].blocks_play());
    }

    #[test]
    fn a_version_outside_the_range_is_reported() {
        let p = profile(&["cs.base", "c.career"]);
        let found = kinds(&p);
        assert_eq!(found.len(), 1);
        assert!(matches!(&found[0].2, Kind::Version { found, .. } if found == "1.0.0"));
    }

    #[test]
    fn load_after_below_and_conflicts_are_reported() {
        let p = profile(&["c.grid", "track.a", "track.b"]);
        assert_eq!(
            kinds(&p),
            [
                ("c.grid".into(), "track.a".into(), Kind::Below),
                ("c.grid".into(), "track.b".into(), Kind::Below)
            ]
        );
        assert_eq!(
            kinds(&profile(&["c.rival", "cs.base"])),
            [("c.rival".into(), "cs.base".into(), Kind::Conflict)]
        );
    }

    #[test]
    fn a_built_in_addon_that_loads_after_others_is_not_asked_to_move() {
        let mut all = installed();
        all.extend([m(
            crate::managed::ID,
            r#","load_after":["cs.*","track.*"],"requires":[{"id":"cs.base"}]"#,
        )]);
        let p = profile(&[crate::managed::ID, "cs.base", "cs.2016", "track.a"]);
        let report = crate::conflicts::Report {
            order: p.enabled().map(str::to_owned).collect(),
            waits: vec![crate::conflicts::Wait {
                addon: crate::managed::ID.into(),
                on: "track.a".into(),
            }],
            ..Default::default()
        };
        let mut found = problems(&p, &all, &is_pinned);
        found.extend(from_report(&p, &report, &is_pinned));
        assert!(found.is_empty(), "{found:?}");
        let before = ids(&p).join(",");
        let plan = plan_fix(&p, &all, &found, &is_pinned, &no_place).unwrap();
        assert_eq!(plan.moved, 0);
        assert_eq!(plan.order.join(","), before);
        let plan = crate::resolver::resolve(&p, &all).unwrap();
        let at = |id: &str| plan.order.iter().position(|x| x == id).unwrap();
        assert!(at("cs.2016") < at(crate::managed::ID) && at("track.a") < at(crate::managed::ID));
        assert!(
            plan.warnings.iter().all(|w| !w.contains("above it")),
            "{:?}",
            plan.warnings
        );
    }

    #[test]
    fn nothing_is_reported_for_addons_that_are_off_or_in_vanilla() {
        let mut p = profile(&["cs.2016"]);
        p.set_enabled("cs.2016", false).unwrap();
        assert!(problems(&p, &installed(), &is_pinned).is_empty());
        assert!(problems(&Profile::vanilla(), &installed(), &is_pinned).is_empty());
    }

    #[test]
    fn move_above_puts_a_dependency_directly_above() {
        let mut p = profile(&["cs.2016", "track.a", "cs.base"]);
        move_above(&mut p, "cs.base", "cs.2016").unwrap();
        let order = ids(&p);
        let base = order.iter().position(|x| *x == "cs.base").unwrap();
        assert_eq!(order[base + 1], "cs.2016");
        let before = ids(&p).join(",");
        move_above(&mut p, "cs.base", "cs.2016").unwrap();
        assert_eq!(ids(&p).join(","), before);
    }

    #[test]
    fn lines_use_names() {
        let (_, mut base) = m("cs.base", "");
        base.name = "Classic Seasons (shared)".into();
        let mut all = installed();
        all.insert("cs.base".into(), base);
        let l = lines(&profile(&["cs.2016", "cs.base"]), &all);
        assert_eq!(l, ["cs.2016: Needs Classic Seasons (shared) above it"]);
    }

    #[test]
    fn a_career_needing_an_off_addon_is_reported_on_that_addon() {
        let list = crate::stdlib::careers::parse(
            r#"{"careers":[{"career":"k1","label":"My Classic","addons":[{"id":"cs.base"},{"id":"cs.2016","need":">=1.0.0"},{"id":"gone.addon"}]}]}"#,
        );
        let p = profile(&["cs.2016"]);
        let found = careers(&p, &installed(), &list);
        let kinds: Vec<(&str, Option<&Kind>)> = found
            .iter()
            .map(|x| match &x.kind {
                Kind::Career { why, .. } => (x.addon.as_str(), Some(&**why)),
                _ => (x.addon.as_str(), None),
            })
            .collect();
        assert_eq!(
            kinds,
            [
                ("cs.base", Some(&Kind::Off)),
                ("gone.addon", Some(&Kind::Missing))
            ]
        );
        let name = |id: &str| installed().get(id).map_or(id.to_owned(), |m| m.name.clone());
        assert_eq!(
            found[0].line(&name),
            "Your career My Classic needs cs.base, which is off"
        );
    }

    #[test]
    fn known_clashes_are_amber() {
        let mut all = installed();
        let json = r#"{"schema_version":1,"id":"f.f2012","name":"F1 2012 Season","version":"1.0.0",
            "author":"t","compat":{"game":"f1_25"}}"#;
        all.extend([
            ("f.f2012".to_owned(), AddonManifest::parse(json).unwrap()),
            m("t.mugello", ""),
        ]);
        let p = profile(&["f.f2012", "t.mugello", "cs.base"]);
        let found = clashes(&p, &all, &|id| id.starts_with("t."));
        assert_eq!(found.len(), 1);
        assert_eq!(
            (found[0].addon.as_str(), found[0].other.as_str()),
            ("f.f2012", "t.mugello")
        );
        assert_eq!(found[0].severity(), Severity::Amber);
        assert!(clashes(&profile(&["f.f2012", "cs.base"]), &all, &|_| false).is_empty());
    }

    fn no_place(_: &str) -> Option<Key> {
        None
    }

    fn apply(p: &mut Profile, plan: &FixPlan) {
        crate::folders::reorder(p, &plan.order, &|_| false);
    }

    fn folder_of(members: &'static [&'static str]) -> impl Fn(&str) -> Option<Key> {
        move |id| members.contains(&id).then(|| Key::Own("f".into()))
    }

    #[test]
    fn fix_moves_only_what_the_dependencies_need() {
        let mut p = profile(&["cs.2016", "track.a", "cs.base", "c.grid", "track.b"]);
        let plan = plan_fix(&p, &installed(), &[], &is_pinned, &no_place).unwrap();
        assert!(plan.moved > 0 && plan.ejected.is_empty());
        apply(&mut p, &plan);
        let order = ids(&p);
        let at = |id: &str| order.iter().position(|x| *x == id).unwrap();
        assert!(at("cs.base") < at("cs.2016"));
        assert!(at("track.a") < at("c.grid") && at("track.b") < at("c.grid"));
        assert!(
            problems(&p, &installed(), &is_pinned)
                .iter()
                .all(|x| x.kind != Kind::Below)
        );
        let fine = profile(&["cs.base", "cs.2016"]);
        assert_eq!(
            plan_fix(&fine, &installed(), &[], &is_pinned, &no_place)
                .unwrap()
                .moved,
            0
        );
    }

    #[test]
    fn fix_keeps_off_addons_where_they_are() {
        let mut p = profile(&["cs.2016", "track.a", "cs.base"]);
        p.set_enabled("track.a", false).unwrap();
        let off_at = ids(&p).iter().position(|x| *x == "track.a").unwrap();
        let plan = plan_fix(&p, &installed(), &[], &is_pinned, &no_place).unwrap();
        apply(&mut p, &plan);
        assert_eq!(ids(&p)[off_at], "track.a");
    }

    #[test]
    fn cycle_message_names_the_addons() {
        let all = BTreeMap::from([
            m(CORE_ID, ""),
            m("x.a", r#","requires":[{"id":"x.b"}]"#),
            m("x.b", r#","requires":[{"id":"x.a"}]"#),
            m("x.c", r#","requires":[{"id":"x.a"}]"#),
        ]);
        let p = profile(&["x.a", "x.b", "x.c"]);
        let err = plan_fix(&p, &all, &[], &is_pinned, &no_place).unwrap_err();
        assert_eq!(err.members(), ["x.a", "x.b"]);
        assert_eq!(err.turn_off(), Some("x.b"));
        let text = err.to_string();
        assert!(
            text.contains("need each other") && text.contains("x.a, x.b"),
            "{text}"
        );
        assert_eq!(ids(&p)[p.addons.len() - 3..], ["x.a", "x.b", "x.c"]);
    }

    #[test]
    fn move_above_refuses_pinned_anchor() {
        let mut p = profile(&["cs.2016", "track.a"]);
        assert!(move_above(&mut p, "track.a", CORE_ID).is_err());
        assert!(move_above(&mut p, "track.a", crate::managed::ID).is_err());
        assert!(move_above(&mut p, CORE_ID, "cs.2016").is_err());
        assert!(move_above(&mut p, "track.a", "cs.2016").is_ok());
    }

    #[test]
    fn fix_survives_folder_gathering() {
        let all = BTreeMap::from([
            m(CORE_ID, ""),
            m("f.a", ""),
            m("f.b", ""),
            m("f.c", r#","requires":[{"id":"f.b"}]"#),
        ]);
        let mut p = profile(&["f.a", "f.c", "f.b"]);
        let place = folder_of(&["f.a", "f.c"]);
        assert_eq!(
            diagnose(&p, &all, None, &[], &|_| false, &is_pinned, &place)
                .problems
                .len(),
            1
        );
        let plan = plan_fix(&p, &all, &[], &is_pinned, &place).unwrap();
        assert_eq!(plan.ejected, [("f.c".to_owned(), Key::Own("f".into()))]);
        apply(&mut p, &plan);
        let after = |id: &str| (id != "f.c").then(|| place(id)).flatten();
        crate::folders::arrange(&mut p, after);
        assert!(
            problems(&p, &all, &is_pinned)
                .iter()
                .all(|x| x.kind != Kind::Below)
        );
        let again = plan_fix(&p, &all, &[], &is_pinned, &after).unwrap();
        assert_eq!((again.moved, again.ejected.len()), (0, 0));
        let d = diagnose(&p, &all, None, &[], &|_| false, &is_pinned, &after);
        assert!(d.problems.is_empty(), "{:?}", d.problems);
        assert_eq!(d.fix, FixState::Nothing);
    }

    #[test]
    fn the_old_fix_was_undone_by_folders() {
        let all = BTreeMap::from([
            m(CORE_ID, ""),
            m("f.a", ""),
            m("f.b", ""),
            m("f.c", r#","requires":[{"id":"f.b"}]"#),
        ]);
        let place = folder_of(&["f.a", "f.c"]);
        let mut p = profile(&["f.a", "f.b", "f.c"]);
        crate::folders::arrange(&mut p, &place);
        assert!(
            problems(&p, &all, &is_pinned)
                .iter()
                .any(|x| x.kind == Kind::Below)
        );
    }

    fn pinned_report(p: &Profile, waiter: &str, on: &str) -> crate::conflicts::Report {
        crate::conflicts::Report {
            order: p.enabled().map(str::to_owned).collect(),
            waits: vec![crate::conflicts::Wait {
                addon: waiter.into(),
                on: on.into(),
            }],
            pairs: vec![crate::conflicts::Pair {
                winner: waiter.into(),
                loser: on.into(),
                database: 204,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn a_pinned_winner_is_not_a_problem() {
        let mut all = installed();
        all.extend([m(crate::managed::ID, "")]);
        let p = profile(&[crate::managed::ID, "track.a"]);
        let report = pinned_report(&p, crate::managed::ID, "track.a");
        let d = diagnose(&p, &all, Some(&report), &[], &|_| false, &is_pinned, &no_place);
        assert!(d.problems.is_empty(), "{:?}", d.problems);
        assert_eq!(d.fix, FixState::Nothing);
    }

    #[test]
    fn overlaps_are_not_problems() {
        let p = profile(&["cs.base", "c.grid"]);
        let mut report = pinned_report(&p, "c.grid", "cs.base");
        report.waits.clear();
        let d = diagnose(
            &p,
            &installed(),
            Some(&report),
            &[],
            &|_| false,
            &is_pinned,
            &no_place,
        );
        assert!(d.problems.is_empty());
        assert_eq!(report.pairs[0].real_items(), 204);
    }

    #[test]
    fn renumbered_stat_ids_are_not_real_items() {
        let pair = crate::conflicts::Pair {
            database: 3,
            items: vec!["a.stat_id".into(), "b.stat_id".into(), "t.laps".into()],
            ..Default::default()
        };
        assert_eq!(pair.real_items(), 1);
    }

    #[test]
    fn waits_put_the_waiter_below_what_it_waits_on() {
        let mut p = profile(&["c.grid", "cs.base"]);
        let report = pinned_report(&p, "c.grid", "cs.base");
        let d = diagnose(
            &p,
            &installed(),
            Some(&report),
            &[],
            &|_| false,
            &is_pinned,
            &no_place,
        );
        assert_eq!(d.problems[0].kind, Kind::Waits);
        let FixState::Ready(plan, 1) = d.fix else {
            panic!("{:?}", d.fix)
        };
        apply(&mut p, &plan);
        let at = |id: &str| ids(&p).iter().position(|x| *x == id).unwrap();
        assert!(at("cs.base") < at("c.grid"));
        let stale = crate::conflicts::Report {
            order: vec![],
            ..report
        };
        assert!(from_report(&p, &stale, &is_pinned).is_empty());
    }

    #[test]
    fn learned_orders_that_would_make_a_cycle_are_dropped() {
        let p = profile(&["cs.base", "cs.2016"]);
        let report = pinned_report(&p, "cs.base", "cs.2016");
        let d = diagnose(
            &p,
            &installed(),
            Some(&report),
            &[],
            &|_| false,
            &is_pinned,
            &no_place,
        );
        assert!(d.problems.is_empty(), "{:?}", d.problems);
        assert_eq!(d.fix, FixState::Nothing);
    }

    #[test]
    fn diagnose_counts_only_what_the_fix_clears() {
        let mut all = installed();
        all.extend([m("c.needy", r#","requires":[{"id":"gone.addon"}]"#)]);
        let p = profile(&["cs.2016", "c.grid", "cs.base", "track.a", "c.needy"]);
        let d = diagnose(&p, &installed(), None, &[], &|_| false, &is_pinned, &no_place);
        assert!(matches!(d.fix, FixState::Ready(_, 2)), "{:?}", d.fix);
        let d = diagnose(&p, &all, None, &[], &|_| false, &is_pinned, &no_place);
        assert!(d.problems.iter().any(|x| x.kind == Kind::Missing));
        assert!(matches!(d.fix, FixState::Ready(_, 2)), "{:?}", d.fix);
        assert!(d.actionable());
    }

    #[test]
    fn a_cycle_blocks_the_fix() {
        let all = BTreeMap::from([
            m(CORE_ID, ""),
            m("x.a", r#","requires":[{"id":"x.b"}]"#),
            m("x.b", r#","requires":[{"id":"x.a"}]"#),
        ]);
        let p = profile(&["x.a", "x.b"]);
        let d = diagnose(&p, &all, None, &[], &|_| false, &is_pinned, &no_place);
        assert!(matches!(d.fix, FixState::Blocked(_)));
        assert!(d.actionable());
    }

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }

        fn chance(&mut self, percent: usize) -> bool {
            self.below(100) < percent
        }
    }

    struct World {
        installed: BTreeMap<String, AddonManifest>,
        profile: Profile,
        places: BTreeMap<String, Option<Key>>,
        pinned: BTreeSet<String>,
        cyclic: bool,
        folders: bool,
    }

    fn acyclic(on: &[String], edges: &BTreeSet<(usize, usize)>, names: &[String]) -> bool {
        let at: BTreeMap<&str, usize> = names.iter().enumerate().map(|(i, n)| (n.as_str(), i)).collect();
        let live: Vec<usize> = on.iter().map(|n| at[n.as_str()]).collect();
        let mut done: BTreeSet<usize> = BTreeSet::new();
        loop {
            let ready: Vec<usize> = live
                .iter()
                .copied()
                .filter(|i| !done.contains(i))
                .filter(|i| {
                    edges
                        .iter()
                        .filter(|(a, _)| a == i)
                        .all(|(_, d)| done.contains(d) || !live.contains(d))
                })
                .collect();
            if ready.is_empty() {
                return done.len() == live.len();
            }
            done.extend(ready);
        }
    }

    fn world(seed: u64) -> World {
        let mut rng = Rng((seed + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let n = 2 + rng.below(13);
        let names: Vec<String> = (0..n).map(|i| format!("x.{i}")).collect();
        let mut rank: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            rank.swap(i, rng.below(i + 1));
        }
        let mut edges: BTreeSet<(usize, usize)> = BTreeSet::new();
        for i in 0..n {
            for j in 0..n {
                if rank[j] < rank[i] && rng.chance(25) {
                    edges.insert((i, j));
                }
            }
        }
        let cyclic = rng.chance(12);
        let enabled: Vec<bool> = (0..n).map(|_| rng.chance(80)).collect();
        if cyclic {
            let (a, b) = (rng.below(n), rng.below(n));
            if a != b {
                edges.insert((a, b));
                edges.insert((b, a));
            }
        }
        let mut installed = BTreeMap::from([m(CORE_ID, "")]);
        for (i, name) in names.iter().enumerate() {
            let mut extra = String::new();
            let needs: Vec<usize> = edges.iter().filter(|(a, _)| *a == i).map(|(_, d)| *d).collect();
            let (req, after): (Vec<usize>, Vec<usize>) = needs.into_iter().partition(|d| d % 2 == 0);
            if !req.is_empty() {
                let list: Vec<String> = req
                    .iter()
                    .map(|d| format!(r#"{{"id":"{}"}}"#, names[*d]))
                    .collect();
                extra.push_str(&format!(r#","requires":[{}]"#, list.join(",")));
            }
            if !after.is_empty() {
                let list: Vec<String> = after.iter().map(|d| format!(r#""{}""#, names[*d])).collect();
                extra.push_str(&format!(r#","load_after":[{}]"#, list.join(",")));
            }
            installed.extend([m(name, &extra)]);
        }
        let builtins = rng.below(3);
        let mut pinned: BTreeSet<String> =
            BTreeSet::from([CORE_ID.to_owned(), crate::managed::ID.to_owned()]);
        installed.extend([m(
            crate::managed::ID,
            &format!(r#","requires":[{{"id":"{}"}}]"#, names[0]),
        )]);
        let mut all: Vec<String> = names.clone();
        all.push(crate::managed::ID.to_owned());
        for i in 0..builtins {
            let id = format!("b.{i}");
            installed.extend([m(&id, "")]);
            pinned.insert(id.clone());
            all.push(id);
        }
        for i in (1..all.len()).rev() {
            all.swap(i, rng.below(i + 1));
        }
        let mut profile = Profile::new("t");
        for id in &all {
            profile.set_enabled(id, true).unwrap();
        }
        for (i, name) in names.iter().enumerate() {
            profile.set_enabled(name, enabled[i]).unwrap();
        }
        let folders = rng.chance(65);
        let own = [Key::Own("f1".into()), Key::Own("f2".into())];
        let mut places: BTreeMap<String, Option<Key>> = BTreeMap::new();
        for id in installed.keys() {
            let key = match (pinned.contains(id), folders) {
                (true, _) => Some(Key::BuiltIn),
                (false, false) => None,
                (false, true) => match rng.below(5) {
                    0 | 1 => None,
                    2 => Some(Key::Category(swofty_addon::manifest::Category::Seasons)),
                    3 => Some(own[0].clone()),
                    _ => Some(own[1].clone()),
                },
            };
            places.insert(id.clone(), key);
        }
        let live: Vec<String> = names
            .iter()
            .zip(&enabled)
            .filter(|(_, e)| **e)
            .map(|(n, _)| n.clone())
            .collect();
        let cyclic = cyclic && !acyclic(&live, &edges, &names);
        let place = |id: &str| places.get(id).cloned().flatten();
        crate::folders::arrange(&mut profile, place);
        World {
            installed,
            profile,
            places,
            pinned,
            cyclic,
            folders,
        }
    }

    fn waits(profile: &Profile, rng: &mut Rng) -> crate::conflicts::Report {
        let on: Vec<String> = profile.enabled().map(str::to_owned).collect();
        let mut waits = Vec::new();
        for _ in 0..rng.below(4) {
            let (a, b) = (rng.below(on.len()), rng.below(on.len()));
            if a != b {
                waits.push(crate::conflicts::Wait {
                    addon: on[a].clone(),
                    on: on[b].clone(),
                });
            }
        }
        crate::conflicts::Report {
            order: on,
            waits,
            ..Default::default()
        }
    }

    #[test]
    fn fix_always_reaches_no_fixable_problems() {
        for seed in 0..2000 {
            let mut w = world(seed);
            let mut rng = Rng(seed.wrapping_mul(7919) + 13);
            let pinned = |id: &str| w.pinned.contains(id);
            let report = waits(&w.profile, &mut rng);
            let place = |id: &str| w.places.get(id).cloned().flatten();
            let diag = diagnose(
                &w.profile,
                &w.installed,
                Some(&report),
                &[],
                &|_| false,
                &pinned,
                &place,
            );
            assert!(diag.actionable(), "seed {seed}");
            let before = w.profile.clone();
            if w.cyclic {
                assert!(
                    matches!(diag.fix, FixState::Blocked(_)),
                    "seed {seed}: {:?}",
                    diag.fix
                );
                assert!(
                    plan_fix(&w.profile, &w.installed, &[], &pinned, &place).is_err(),
                    "seed {seed}"
                );
                assert_eq!(w.profile, before);
                continue;
            }
            let fixable = diag.problems.iter().any(Problem::fixable_by_reorder);
            let FixState::Ready(plan, count) = diag.fix.clone() else {
                assert!(!fixable, "seed {seed}: {:?} {:?}", diag.fix, diag.problems);
                continue;
            };
            assert!(count > 0 && fixable, "seed {seed}");
            let learned: Vec<Problem> = diag
                .problems
                .iter()
                .filter(|p| p.kind == Kind::Waits)
                .cloned()
                .collect();
            apply(&mut w.profile, &plan);
            let ejected: BTreeSet<&str> = plan.ejected.iter().map(|(id, _)| id.as_str()).collect();
            let after = |id: &str| match ejected.contains(id) {
                true => None,
                false => place(id),
            };
            assert!(
                !crate::folders::arrange(&mut w.profile, after),
                "seed {seed}: arrange moved the fixed order"
            );
            let report = crate::conflicts::Report {
                order: w.profile.enabled().map(str::to_owned).collect(),
                waits: report.waits.clone(),
                ..Default::default()
            };
            let again = diagnose(
                &w.profile,
                &w.installed,
                Some(&report),
                &[],
                &|_| false,
                &pinned,
                &after,
            );
            assert!(
                again.problems.iter().all(|p| !p.fixable_by_reorder()),
                "seed {seed}: {:?} {:?} {:?} {:?}",
                again.problems,
                again.fix,
                w.profile.enabled().collect::<Vec<_>>(),
                report.waits
            );
            assert_eq!(again.fix, FixState::Nothing, "seed {seed}");
            let twice = plan_fix(&w.profile, &w.installed, &learned, &pinned, &after).unwrap();
            assert_eq!((twice.moved, twice.ejected.len()), (0, 0), "seed {seed}");
            let k = before.addons.iter().filter(|a| pinned(&a.id)).count();
            assert_eq!(
                before.addons.iter().take(k).map(|a| &a.id).collect::<Vec<_>>(),
                w.profile.addons.iter().take(k).map(|a| &a.id).collect::<Vec<_>>(),
                "seed {seed}: pinned prefix"
            );
            let off = |p: &Profile| -> Vec<(usize, String)> {
                p.addons
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| !a.enabled && a.id != CORE_ID)
                    .map(|(i, a)| (i, a.id.clone()))
                    .collect()
            };
            match w.folders {
                false => assert_eq!(off(&before), off(&w.profile), "seed {seed}: disabled slots"),
                true => assert_eq!(
                    off(&before)
                        .into_iter()
                        .map(|(_, id)| id)
                        .collect::<BTreeSet<_>>(),
                    off(&w.profile)
                        .into_iter()
                        .map(|(_, id)| id)
                        .collect::<BTreeSet<_>>(),
                    "seed {seed}: disabled addons"
                ),
            }
        }
    }

    #[test]
    fn every_problem_the_diagnosis_shows_is_actionable() {
        for seed in 0..2000 {
            let w = world(seed);
            let mut rng = Rng(seed.wrapping_mul(104_729) + 5);
            let report = waits(&w.profile, &mut rng);
            let pinned = |id: &str| w.pinned.contains(id);
            let place = |id: &str| w.places.get(id).cloned().flatten();
            let d = diagnose(
                &w.profile,
                &w.installed,
                Some(&report),
                &[],
                &|_| false,
                &pinned,
                &place,
            );
            assert!(d.actionable(), "seed {seed}");
            for p in &d.problems {
                match &p.kind {
                    Kind::Below | Kind::Waits => {
                        assert!(
                            matches!(d.fix, FixState::Ready(..) | FixState::Blocked(_)),
                            "seed {seed}"
                        )
                    }
                    Kind::Off | Kind::Missing | Kind::Version { .. } | Kind::Conflict => {
                        assert!(w.installed.contains_key(&p.other) || p.kind == Kind::Missing);
                    }
                    Kind::Career { .. } | Kind::Clash { .. } => {}
                }
            }
        }
    }
}
