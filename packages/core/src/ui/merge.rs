use swofty_formats::pkg::{Arr, Node, Obj, Pkg, Val, Value};

use super::idf::{elem_name, relookup};
use super::node::NodeExt;
use crate::Result;

/// Which kind of UI resource a merge works on: it decides the key of a scene's entries and the lookup
/// table a scene carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Template,
    Scene,
}

impl Kind {
    /// The kind of a resource by its name, when it is a UI resource the merge can work on.
    pub fn of(resource: &str) -> Option<Self> {
        if resource.ends_with(".ui") {
            Some(Self::Template)
        } else if resource.ends_with(".idf") {
            Some(Self::Scene)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
struct Seg {
    key: String,
    shown: bool,
    label: String,
}

/// A path through a UI resource: the keys that find the node again, and how it reads in a warning.
#[derive(Debug, Clone, Default)]
pub struct Path(Vec<Seg>);

impl Path {
    pub fn keys(&self) -> Vec<String> {
        self.0.iter().map(|s| s.key.clone()).collect()
    }

    /// The dotted names of the UI nodes on the path, then what inside the last one is meant.
    pub fn show(&self) -> String {
        let mut names: Vec<&str> = Vec::new();
        let mut inside: Vec<&str> = Vec::new();
        for s in &self.0 {
            if s.shown {
                names.push(&s.label);
                inside.clear();
            } else {
                inside.push(&s.label);
            }
        }
        match (names.is_empty(), inside.is_empty()) {
            (_, true) => names.join("."),
            (true, false) => inside.join("/"),
            (false, false) => format!("{} [{}]", names.join("."), inside.join("/")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Conflict {
    pub path: Path,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stats {
    pub added: usize,
    pub changed: usize,
    pub removed: usize,
}

pub struct Merged {
    pub pkg: Pkg,
    pub conflicts: Vec<Conflict>,
    pub stats: Stats,
}

struct Cx {
    kind: Kind,
    path: Vec<Seg>,
    conflicts: Vec<Conflict>,
    stats: Stats,
}

impl Cx {
    fn conflict(&mut self) {
        self.conflicts.push(Conflict {
            path: Path(self.path.clone()),
        });
    }

    fn within<T>(&mut self, seg: Seg, f: impl FnOnce(&mut Self) -> T) -> T {
        self.path.push(seg);
        let out = f(self);
        self.path.pop();
        out
    }
}

pub fn same(a: &Node, b: &Node) -> bool {
    match (a, b) {
        (Node::Val(x), Node::Val(y)) => same_val(x, y),
        (Node::Obj(x), Node::Obj(y)) => {
            x.version == y.version
                && x.items.len() == y.items.len()
                && x.items
                    .iter()
                    .zip(&y.items)
                    .all(|((kx, vx), (ky, vy))| same_val(kx, ky) && same(vx, vy))
        }
        (Node::Arr(x), Node::Arr(y)) => {
            x.tag == y.tag
                && x.items.len() == y.items.len()
                && x.items.iter().zip(&y.items).all(|(p, q)| same(p, q))
        }
        (Node::Iar(x), Node::Iar(y)) => format!("{x:?}") == format!("{y:?}"),
        _ => false,
    }
}

fn same_val(a: &Val, b: &Val) -> bool {
    a.ty == b.ty && a.value == b.value
}

fn val_key(v: &Val) -> String {
    match &v.value {
        Value::Str(s) => s.clone(),
        Value::Raw(b) => b.iter().map(|x| format!("{x:02x}")).collect(),
    }
}

fn key_of(n: &Node) -> Option<String> {
    if n.is_node() {
        return Some(n.name().to_owned());
    }
    n.as_obj()?.get("name")?.as_val().map(val_key)
}

fn keyed(items: &[Node]) -> Option<Vec<String>> {
    let keys: Vec<String> = items.iter().map(key_of).collect::<Option<_>>()?;
    let mut seen = std::collections::BTreeSet::new();
    keys.iter().all(|k| seen.insert(k.as_str())).then_some(keys)
}

const SLOTS: [&str; 7] = [
    "props",
    "children",
    "commands",
    "events",
    "layout",
    "component",
    "behaviours",
];

fn slot_label(i: usize) -> String {
    SLOTS
        .get(i)
        .map_or_else(|| format!("slot {i}"), |s| (*s).to_owned())
}

fn is_slotted(n: &Node) -> bool {
    n.is_node()
}

/// The node under `root` that `keys` (from [`Path::keys`]) lead to.
pub fn lookup<'a>(root: &'a Node, keys: &[String]) -> Option<&'a Node> {
    let mut n = root;
    for k in keys {
        n = match n {
            Node::Obj(o) => o.get(k)?,
            Node::Arr(a) => match k.strip_prefix('#').and_then(|i| i.parse::<usize>().ok()) {
                Some(i) if is_slotted(n) => a.items.get(i)?,
                _ => a.items.iter().find(|c| key_of(c).as_deref() == Some(k))?,
            },
            _ => return None,
        };
    }
    Some(n)
}

fn take(n: &Node) -> Node {
    super::idf::detached(n)
}

fn merge3(cx: &mut Cx, base: Option<&Node>, a: Option<&Node>, b: Option<&Node>) -> Option<Node> {
    match (a, b) {
        (None, None) => None,
        (Some(a), Some(b)) if same(a, b) => Some(a.clone()),
        (Some(a), Some(b)) => match base {
            Some(base) if same(base, a) => {
                cx.stats.changed += 1;
                Some(take(b))
            }
            Some(base) if same(base, b) => Some(a.clone()),
            Some(base) => Some(merge_both(cx, base, a, b)),
            None => Some(atomic(cx, b)),
        },
        (None, Some(b)) => match base {
            None => {
                cx.stats.added += 1;
                Some(take(b))
            }
            Some(base) if same(base, b) => None,
            Some(_) => Some(atomic(cx, b)),
        },
        (Some(a), None) => match base {
            None => Some(a.clone()),
            Some(base) if same(base, a) => {
                cx.stats.removed += 1;
                None
            }
            Some(_) => {
                cx.conflict();
                cx.stats.removed += 1;
                None
            }
        },
    }
}

fn atomic(cx: &mut Cx, b: &Node) -> Node {
    cx.conflict();
    take(b)
}

fn seg(key: &str, shown: bool, label: &str) -> Seg {
    Seg {
        key: key.to_owned(),
        shown,
        label: label.to_owned(),
    }
}

fn merge_both(cx: &mut Cx, base: &Node, a: &Node, b: &Node) -> Node {
    match (base, a, b) {
        (Node::Arr(xbase), Node::Arr(xa), Node::Arr(xb))
            if is_slotted(base) && is_slotted(a) && is_slotted(b) =>
        {
            if xa.items.len() != xbase.items.len() || xb.items.len() != xbase.items.len() {
                return atomic(cx, b);
            }
            let mut out = xa.clone();
            for i in 0..xbase.items.len() {
                let at = seg(&format!("#{i}"), false, &slot_label(i));
                let (pb, pa, pc) = (&xbase.items[i], &xa.items[i], &xb.items[i]);
                if let Some(m) = cx.within(at, |cx| merge3(cx, Some(pb), Some(pa), Some(pc))) {
                    out.items[i] = m;
                }
            }
            Node::Arr(out)
        }
        (Node::Obj(xbase), Node::Obj(xa), Node::Obj(xb)) => merge_obj(cx, xbase, xa, xb, b),
        (Node::Arr(xbase), Node::Arr(xa), Node::Arr(xb)) if xbase.tag == xa.tag && xa.tag == xb.tag => {
            merge_list(cx, xbase, xa, xb, b)
        }
        _ => atomic(cx, b),
    }
}

fn merge_obj(cx: &mut Cx, base: &Obj, a: &Obj, b: &Obj, whole: &Node) -> Node {
    if base.version != a.version || a.version != b.version {
        return atomic(cx, whole);
    }
    let mut out = Vec::new();
    for (k, va) in &a.items {
        let Some(name) = k.as_str() else {
            out.push((k.clone(), va.clone()));
            continue;
        };
        if cx.kind == Kind::Scene && name == "nodeidlookup" {
            out.push((k.clone(), va.clone()));
            continue;
        }
        let at = seg(name, false, name);
        if let Some(m) = cx.within(at, |cx| merge3(cx, base.get(name), Some(va), b.get(name))) {
            out.push((k.clone(), m));
        }
    }
    for (k, vb) in &b.items {
        let Some(name) = k.as_str() else { continue };
        if a.get(name).is_some() {
            continue;
        }
        let at = seg(name, false, name);
        if let Some(m) = cx.within(at, |cx| merge3(cx, base.get(name), None, Some(vb))) {
            out.push((fresh_key(k), m));
        }
    }
    Node::Obj(Obj {
        items: out,
        version: a.version,
    })
}

fn fresh_key(k: &Val) -> Val {
    match super::idf::detached(&Node::Val(k.clone())) {
        Node::Val(v) => v,
        _ => k.clone(),
    }
}

fn merge_list(cx: &mut Cx, base: &Arr, a: &Arr, b: &Arr, whole: &Node) -> Node {
    let (Some(kbase), Some(ka), Some(kb)) = (keyed(&base.items), keyed(&a.items), keyed(&b.items)) else {
        return atomic(cx, whole);
    };
    let pick =
        |keys: &[String], items: &[Node], k: &str| keys.iter().position(|x| x == k).map(|i| items[i].clone());
    let mut out = Vec::new();
    for (item, k) in a.items.iter().zip(&ka) {
        let at = item_seg(cx.kind, item, k);
        let merged = cx.within(at, |cx| {
            let (bi, ci) = (pick(&kbase, &base.items, k), pick(&kb, &b.items, k));
            merge3(cx, bi.as_ref(), Some(item), ci.as_ref())
        });
        out.extend(merged);
    }
    for (item, k) in b.items.iter().zip(&kb) {
        if ka.contains(k) {
            continue;
        }
        let at = item_seg(cx.kind, item, k);
        let merged = cx.within(at, |cx| {
            merge3(cx, pick(&kbase, &base.items, k).as_ref(), None, Some(item))
        });
        out.extend(merged);
    }
    Node::Arr(Arr {
        items: out,
        tag: a.tag,
    })
}

fn item_seg(kind: Kind, item: &Node, key: &str) -> Seg {
    if item.is_node() {
        return seg(key, true, key);
    }
    let obj = item.as_obj();
    if kind == Kind::Scene
        && let Some(o) = obj
    {
        if o.get("elementId").is_some() || o.get("children").is_some() {
            let n = elem_name(item);
            return seg(key, true, if n.is_empty() { key } else { n });
        }
        if let Some(scene) = o.get("scene") {
            let n = elem_name(scene);
            return seg(key, true, if n.is_empty() { key } else { n });
        }
    }
    seg(key, false, key)
}

/// Merges two addons' whole replacements of one UI resource against the game's own copy: the nodes
/// either addon added stay, an addon's change to a stock node stays, a deletion stays unless the other
/// addon edited the node, and a node the two changed differently is a [`Conflict`] that `theirs`
/// (the later addon) wins. The result is `ours` with `theirs`' changes laid in.
pub fn merge(kind: Kind, base: &Pkg, ours: &Pkg, theirs: &Pkg) -> Result<Merged> {
    let mut cx = Cx {
        kind,
        path: Vec::new(),
        conflicts: Vec::new(),
        stats: Stats::default(),
    };
    let root = merge3(&mut cx, Some(&base.root), Some(&ours.root), Some(&theirs.root))
        .unwrap_or_else(|| ours.root.clone());
    let mut pkg = ours.clone();
    pkg.root = root;
    if kind == Kind::Scene
        && let Some(list) = pkg
            .root
            .as_obj_mut()
            .and_then(|o| o.get_mut("scenelist"))
            .and_then(Node::as_arr_mut)
    {
        for entry in list.items.iter_mut().filter(|e| e.as_obj().is_some()) {
            relookup(entry)?;
        }
    }
    Ok(Merged {
        pkg,
        conflicts: cx.conflicts,
        stats: cx.stats,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::node::{NodeExt, arr, boolv, group_node, node_paths_of, obj, s, set_prop, u32v};
    use swofty_formats::pkg::Arr;

    fn template(nodes: Vec<Node>) -> Pkg {
        Pkg {
            name: [0; 4],
            root: obj(vec![("Templates", arr(nodes))]),
            vca: Vec::new(),
            layout: None,
        }
    }

    fn leaf(name: &str) -> Node {
        group_node(name, vec![("visible", boolv(true))], &[], vec![])
    }

    fn parent(kids: Vec<Node>) -> Node {
        group_node("panel", vec![("visible", boolv(true))], &[], kids)
    }

    fn with_prop(mut n: Node, key: &str, value: u32) -> Node {
        set_prop(&mut n, key, u32v(value)).unwrap();
        n
    }

    fn names(p: &Pkg) -> Vec<String> {
        let mut v = Vec::new();
        node_paths_of(&p.root, "", &mut v);
        v.into_iter().map(|(k, _)| k).collect()
    }

    fn merged(base: &Pkg, a: &Pkg, b: &Pkg) -> Merged {
        let m = merge(Kind::Template, base, a, b).unwrap();
        let back = Pkg::parse(&m.pkg.to_bytes().unwrap()).unwrap();
        assert_eq!(
            names(&back),
            names(&m.pkg),
            "the merged tree survives a round trip"
        );
        m
    }

    #[test]
    fn nodes_both_addons_add_stay_and_each_changed_property_stays() {
        let base = template(vec![parent(vec![leaf("a"), leaf("b")])]);
        let ours = template(vec![parent(vec![
            with_prop(leaf("a"), "x", 1),
            leaf("b"),
            leaf("ours_new"),
        ])]);
        let theirs = template(vec![parent(vec![
            leaf("a"),
            with_prop(leaf("b"), "y", 2),
            leaf("theirs_new"),
        ])]);
        let m = merged(&base, &ours, &theirs);
        assert!(m.conflicts.is_empty(), "{:?}", m.conflicts);
        assert_eq!(
            names(&m.pkg),
            [
                "panel",
                "panel.a",
                "panel.b",
                "panel.ours_new",
                "panel.theirs_new"
            ]
        );
        let kids = m
            .pkg
            .root
            .as_obj()
            .unwrap()
            .get("Templates")
            .unwrap()
            .as_arr()
            .unwrap();
        let panel = &kids.items[0];
        let a = panel.child("a").unwrap().props().unwrap().get("x").is_some();
        let b = panel.child("b").unwrap().props().unwrap().get("y").is_some();
        assert!(a && b, "both addons' edits of stock nodes stay");
        assert_eq!(m.stats.added, 1);
    }

    #[test]
    fn a_property_changed_two_ways_is_a_conflict_the_later_addon_wins() {
        let base = template(vec![parent(vec![with_prop(leaf("a"), "x", 0)])]);
        let ours = template(vec![parent(vec![with_prop(leaf("a"), "x", 1)])]);
        let theirs = template(vec![parent(vec![with_prop(leaf("a"), "x", 2), leaf("kept")])]);
        let m = merged(&base, &ours, &theirs);
        assert_eq!(m.conflicts.len(), 1);
        assert_eq!(m.conflicts[0].path.show(), "panel.a [props/x]");
        let panel = &m
            .pkg
            .root
            .as_obj()
            .unwrap()
            .get("Templates")
            .unwrap()
            .as_arr()
            .unwrap()
            .items[0];
        let x = panel.child("a").unwrap().props().unwrap().get("x").unwrap();
        assert_eq!(x.as_val().unwrap().as_u32(), Some(2));
        assert!(panel.child("kept").is_ok(), "the conflict is only that node");
    }

    #[test]
    fn a_deletion_stands_unless_the_other_addon_edited_the_node() {
        let base = template(vec![parent(vec![leaf("gone"), leaf("edited"), leaf("same")])]);
        let ours = template(vec![parent(vec![leaf("edited"), leaf("same")])]);
        let theirs = template(vec![parent(vec![
            leaf("gone"),
            with_prop(leaf("edited"), "x", 1),
            leaf("same"),
        ])]);
        let m = merged(&base, &ours, &theirs);
        assert!(m.conflicts.is_empty(), "{:?}", m.conflicts);
        assert_eq!(names(&m.pkg), ["panel", "panel.edited", "panel.same"]);
        let ours = template(vec![parent(vec![leaf("edited"), leaf("same")])]);
        let theirs = template(vec![parent(vec![
            leaf("gone"),
            with_prop(leaf("edited"), "x", 1),
            with_prop(leaf("same"), "z", 3),
        ])]);
        let base = template(vec![parent(vec![leaf("gone"), leaf("edited"), leaf("same")])]);
        let m = merged(&base, &ours, &theirs);
        assert!(m.conflicts.is_empty());
        let ours = template(vec![parent(vec![leaf("same")])]);
        let theirs = template(vec![parent(vec![
            leaf("gone"),
            with_prop(leaf("edited"), "x", 1),
            leaf("same"),
        ])]);
        let m = merged(&base, &ours, &theirs);
        assert_eq!(m.conflicts.len(), 1);
        assert_eq!(m.conflicts[0].path.show(), "panel.edited");
        assert!(
            names(&m.pkg).contains(&"panel.edited".to_owned()),
            "the later addon's edit wins there"
        );
    }

    #[test]
    fn an_unchanged_side_takes_the_other_side_whole() {
        let base = template(vec![parent(vec![leaf("a")])]);
        let theirs = template(vec![parent(vec![leaf("a"), leaf("b")])]);
        let m = merged(&base, &base, &theirs);
        assert!(same(&m.pkg.root, &theirs.root));
        let m = merged(&base, &theirs, &base);
        assert!(same(&m.pkg.root, &theirs.root));
    }

    fn scene(elements: &[&str], hidden: u32) -> Pkg {
        let el = |name: &str| {
            obj(vec![
                ("name", s(name)),
                ("elementId", u32v(swofty_formats::hash::djb2(name))),
                ("hidden", u32v(hidden)),
                ("children", Node::Arr(Arr::new(vec![]))),
            ])
        };
        let mut root = el("root");
        root.as_obj_mut()
            .unwrap()
            .set("children", arr(elements.iter().map(|n| el(n)).collect()));
        let entry = obj(vec![
            (
                "name",
                Node::Val(Val::raw(*b"u64 ", vec![1, 0, 0, 0, 0, 0, 0, 0])),
            ),
            ("scene", root),
            ("nodeidlookup", arr(vec![])),
        ]);
        Pkg {
            name: [0; 4],
            root: obj(vec![("scenelist", arr(vec![entry]))]),
            vca: Vec::new(),
            layout: None,
        }
    }

    #[test]
    fn scenes_merge_by_element_name_and_the_lookup_follows() {
        let base = scene(&["a"], 0);
        let ours = scene(&["a", "ours_box"], 0);
        let theirs = scene(&["a", "theirs_box"], 0);
        let m = merge(Kind::Scene, &base, &ours, &theirs).unwrap();
        assert!(m.conflicts.is_empty(), "{:?}", m.conflicts);
        let entry = &m
            .pkg
            .root
            .as_obj()
            .unwrap()
            .get("scenelist")
            .unwrap()
            .as_arr()
            .unwrap()
            .items[0];
        let kids: Vec<&str> = crate::ui::idf::elements(entry.as_obj().unwrap().get("scene").unwrap())
            .into_iter()
            .map(|(p, _)| p)
            .map(|p| Box::leak(p.into_boxed_str()) as &str)
            .collect();
        assert_eq!(kids, ["root", "a", "ours_box", "theirs_box"]);
        let lookup: Vec<u32> = entry
            .as_obj()
            .unwrap()
            .get("nodeidlookup")
            .unwrap()
            .as_arr()
            .unwrap()
            .items
            .iter()
            .filter_map(|v| v.as_val().and_then(Val::as_u32))
            .collect();
        let mut want: Vec<u32> = ["root", "a", "ours_box", "theirs_box"]
            .iter()
            .map(swofty_formats::hash::djb2)
            .collect();
        want.sort_unstable();
        assert_eq!(lookup, want);
    }

    #[test]
    fn a_scene_element_both_addons_move_differently_is_a_conflict() {
        let base = scene(&["a"], 0);
        let m = merge(Kind::Scene, &base, &scene(&["a"], 1), &scene(&["a"], 2)).unwrap();
        assert!(
            m.conflicts.iter().any(|c| c.path.show().contains("root.a")),
            "{:?}",
            m.conflicts.iter().map(|c| c.path.show()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn lookup_finds_a_conflict_path_again() {
        let base = template(vec![parent(vec![with_prop(leaf("a"), "x", 0)])]);
        let m = merged(
            &base,
            &template(vec![parent(vec![with_prop(leaf("a"), "x", 1)])]),
            &template(vec![parent(vec![with_prop(leaf("a"), "x", 2)])]),
        );
        let keys = m.conflicts[0].path.keys();
        let found = lookup(&base.root, &keys).unwrap();
        assert_eq!(found.as_val().unwrap().as_u32(), Some(0));
    }
}
