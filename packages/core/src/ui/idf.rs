use swofty_formats::hash::djb2;
use swofty_formats::pkg::{Arr, Node, Obj, Pkg, Val, Value};

use super::node::{NodeExt, ObjExt, component_id, err};
use crate::Result;

pub fn scene_id(path: &str) -> u64 {
    component_id(path.strip_suffix(".idf").unwrap_or(path))
}

pub fn material_binding(file_ref: &str) -> String {
    format!(
        "{}.emb?context=default",
        file_ref.strip_suffix(".idf").unwrap_or(file_ref)
    )
}

pub fn detached(n: &Node) -> Node {
    match n {
        Node::Val(v) => Node::Val(fresh(v)),
        Node::Obj(o) => Node::Obj(Obj {
            items: o.items.iter().map(|(k, v)| (fresh(k), detached(v))).collect(),
            version: o.version,
        }),
        Node::Arr(a) => Node::Arr(Arr {
            items: a.items.iter().map(detached).collect(),
            tag: a.tag,
        }),
        Node::Iar(r) => Node::Iar(*r),
    }
}

fn fresh(v: &Val) -> Val {
    match &v.value {
        Value::Str(s) => Val::str(s.as_str()),
        Value::Raw(b) => Val::raw(v.ty, b.clone()),
    }
}

pub fn floats(ty: [u8; 4], values: &[f64]) -> Val {
    let mut out: Vec<u8> = values.iter().flat_map(|v| (*v as f32).to_le_bytes()).collect();
    out.resize(4 * values.len().max(1), 0);
    Val::raw(ty, out)
}

pub fn vec3(v: [f64; 4]) -> Val {
    floats(*b"vec3", &v)
}

pub fn quat(v: [f64; 4]) -> Val {
    floats(*b"quat", &v)
}

pub fn elem_name(e: &Node) -> &str {
    e.as_obj()
        .and_then(|o| o.get("name"))
        .and_then(Node::as_val)
        .and_then(Val::as_str)
        .unwrap_or_default()
}

fn kids(e: &Node) -> impl Iterator<Item = &Node> {
    e.as_obj()
        .and_then(|o| o.get("children"))
        .and_then(Node::as_arr)
        .into_iter()
        .flat_map(|a| a.items.iter().filter(|c| c.as_obj().is_some()))
}

fn walk<'a>(e: &'a Node, path: &str, out: &mut Vec<(String, &'a Node)>) {
    for c in kids(e) {
        let p = match path {
            "" => elem_name(c).to_owned(),
            _ => format!("{path}.{}", elem_name(c)),
        };
        out.push((p.clone(), c));
        walk(c, &p, out);
    }
}

pub fn elements(root: &Node) -> Vec<(String, &Node)> {
    let mut out = vec![(elem_name(root).to_owned(), root)];
    walk(root, "", &mut out);
    out
}

pub fn find<'a>(e: &'a Node, name: &str) -> Option<&'a Node> {
    if elem_name(e) == name {
        return Some(e);
    }
    kids(e).find_map(|c| find(c, name))
}

pub fn children_mut(e: &mut Node) -> Result<&mut Vec<Node>> {
    e.o_mut()?.at_mut("children")?.a_mut()
}

pub fn param<'a>(e: &'a Node, name: &str) -> Option<&'a Val> {
    e.as_obj()?
        .get("params")?
        .as_arr()?
        .items
        .iter()
        .filter_map(Node::as_obj)
        .find(|p| p.get("name").and_then(Node::as_val).and_then(Val::as_str) == Some(name))?
        .get("value")?
        .as_val()
}

pub fn param_str<'a>(e: &'a Node, name: &str) -> Option<&'a str> {
    param(e, name)?.as_str()
}

fn put_param(e: &mut Node, name: &str, value: Val) -> Result<()> {
    let o = e.o_mut()?;
    if o.get("params").and_then(Node::as_arr).is_none() {
        o.set("params", Node::Arr(Arr::new(Vec::new())));
    }
    let params = o.at_mut("params")?.a_mut()?;
    for p in params.iter_mut().filter_map(Node::as_obj_mut) {
        if p.get("name").and_then(Node::as_val).and_then(Val::as_str) == Some(name) {
            p.set("value", Node::Val(value));
            return Ok(());
        }
    }
    params.push(Node::Obj(Obj::new(vec![
        (Val::str("value"), Node::Val(value)),
        (Val::str("name"), Node::Val(Val::str(name))),
    ])));
    Ok(())
}

pub fn set_param(e: &mut Node, name: &str, value: Val) -> Result<()> {
    if name == "fileRef" {
        let to = value
            .as_str()
            .ok_or_else(|| err("a reference names an eaid://….idf scene"))?
            .to_owned();
        return retarget(e, &to);
    }
    put_param(e, name, value)
}

pub fn retarget(e: &mut Node, file_ref: &str) -> Result<()> {
    if !(file_ref.starts_with("eaid://") && file_ref.ends_with(".idf")) {
        return Err(err(format!(
            "a reference names an eaid://….idf scene, not {file_ref:?}"
        )));
    }
    let old = param_str(e, "fileRef").map(str::to_owned);
    let binding = param_str(e, "XrefMaterialBinding").map(str::to_owned);
    put_param(e, "fileRef", Val::str(file_ref))?;
    let o = e.o_mut()?;
    let ty = o.get("xref").and_then(Node::as_val).map_or(*b"ui64", |v| v.ty);
    o.set("xref", Node::Val(Val::raw(ty, scene_id(file_ref).to_le_bytes())));
    if let Some(old) = old
        && binding.as_deref() == Some(material_binding(&old).as_str())
    {
        put_param(e, "XrefMaterialBinding", Val::str(material_binding(file_ref)))?;
    }
    Ok(())
}

pub fn xref(e: &Node) -> Option<u64> {
    match &e.as_obj()?.get("xref")?.as_val()?.value {
        Value::Raw(b) if b.len() >= 8 => Some(u64::from_le_bytes(b[..8].try_into().ok()?)),
        _ => None,
    }
}

pub fn recompute_ids(e: &mut Node, path: &str) -> Result<()> {
    e.o_mut()?.set("elementId", Node::Val(Val::u32(djb2(path))));
    if let Some(Node::Arr(a)) = e.o_mut()?.get_mut("children") {
        for c in a.items.iter_mut().filter(|c| c.as_obj().is_some()) {
            let p = format!("{path}.{}", elem_name(c));
            recompute_ids(c, &p)?;
        }
    }
    Ok(())
}

fn leaf_ids(e: &Node, out: &mut Vec<u32>) {
    out.push(djb2(elem_name(e)));
    for c in kids(e) {
        leaf_ids(c, out);
    }
}

pub fn relookup(entry: &mut Node) -> Result<()> {
    let mut ids = Vec::new();
    leaf_ids(entry.o()?.at("scene")?, &mut ids);
    ids.sort_unstable();
    ids.dedup();
    let o = entry.o_mut()?;
    let tag = o
        .get("nodeidlookup")
        .and_then(Node::as_arr)
        .map_or(*b"!ili", |a| a.tag);
    let items = ids.into_iter().map(|h| Node::Val(Val::u32(h))).collect();
    o.set("nodeidlookup", Node::Arr(Arr { items, tag }));
    Ok(())
}

pub fn scenes(p: &Pkg) -> Result<&Vec<Node>> {
    p.root.o()?.at("scenelist")?.a()
}

pub fn scenes_mut(p: &mut Pkg) -> Result<&mut Vec<Node>> {
    p.root.o_mut()?.at_mut("scenelist")?.a_mut()
}

pub fn scene_name(entry: &Node) -> Option<u64> {
    match &entry.as_obj()?.get("name")?.as_val()?.value {
        Value::Raw(b) if b.len() >= 8 => Some(u64::from_le_bytes(b[..8].try_into().ok()?)),
        _ => None,
    }
}

pub fn xref_mismatches(p: &Pkg) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for entry in scenes(p)? {
        let Some(root) = entry.as_obj().and_then(|o| o.get("scene")) else {
            continue;
        };
        for (path, e) in elements(root) {
            if let (Some(x), Some(f)) = (xref(e), param_str(e, "fileRef"))
                && x != scene_id(f)
            {
                out.push(format!("{path}: xref {x} is not the scene id of {f}"));
            }
        }
    }
    Ok(out)
}

pub fn id_problems(p: &Pkg) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for entry in scenes(p)? {
        let Some(root) = entry.as_obj().and_then(|o| o.get("scene")) else {
            continue;
        };
        for (path, e) in elements(root).into_iter().skip(1) {
            let id = e
                .as_obj()
                .and_then(|o| o.get("elementId"))
                .and_then(Node::as_val)
                .and_then(Val::as_u32);
            if id != Some(djb2(&path)) {
                out.push(format!("{path}: elementId"));
            }
        }
        let mut want = Vec::new();
        leaf_ids(root, &mut want);
        want.sort_unstable();
        want.dedup();
        let have: Vec<u32> = entry
            .o()?
            .at("nodeidlookup")?
            .a()?
            .iter()
            .filter_map(|v| v.as_val().and_then(Val::as_u32))
            .collect();
        if have != want {
            out.push("nodeidlookup".into());
        }
    }
    Ok(out)
}

pub fn find_mut<'a>(e: &'a mut Node, name: &str) -> Option<&'a mut Node> {
    if elem_name(e) == name {
        return Some(e);
    }
    let kids = e.as_obj_mut()?.get_mut("children")?.as_arr_mut()?;
    kids.items.iter_mut().find_map(|c| find_mut(c, name))
}

/// The one element named `name` below `e`, or the way it isn't.
pub fn expect<'a>(e: &'a Node, name: &str) -> Result<&'a Node> {
    find(e, name).ok_or_else(|| err(format!("the scene has no element {name}")))
}

pub fn rename(e: &mut Node, name: &str) -> Result<()> {
    e.o_mut()?.set("name", Node::Val(Val::str(name)));
    Ok(())
}

pub fn fp32(v: f64) -> Val {
    Val::raw(*b"fp32", (v as f32).to_le_bytes())
}

pub fn si32(v: i32) -> Val {
    Val::raw(*b"si32", v.to_le_bytes())
}

/// Sets a param, keeping the tag the element's template gave it when it has the param (a string
/// param keeps its own string tag).
pub fn set_param_like(e: &mut Node, name: &str, mut value: Val) -> Result<()> {
    if let Some(old) = param(e, name)
        && matches!(value.value, Value::Str(_))
        && matches!(old.value, Value::Str(_))
    {
        value.ty = old.ty;
    }
    put_param(e, name, value)
}

pub fn set_transform(e: &mut Node, pos: [f64; 3], rot: [f64; 4], scale: [f64; 3]) -> Result<()> {
    let o = e.o_mut()?;
    o.set("position", Node::Val(vec3([pos[0], pos[1], pos[2], 0.0])));
    o.set("scale", Node::Val(vec3([scale[0], scale[1], scale[2], 0.0])));
    o.set("rotation", Node::Val(quat(rot)));
    Ok(())
}

pub fn position(e: &Node) -> Option<[f64; 3]> {
    let v = e.as_obj()?.get("position")?.as_val()?;
    match &v.value {
        Value::Raw(b) if b.len() >= 12 => {
            let f = |i: usize| f32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().expect("4 bytes")) as f64;
            Some([f(0), f(1), f(2)])
        }
        _ => None,
    }
}

/// Moves the stock customisation selection box: `tl_customisation` and `br_customisation`.
pub fn set_box(root: &mut Node, tl: [f64; 3], br: [f64; 3]) -> Result<()> {
    for (name, p) in [("tl_customisation", tl), ("br_customisation", br)] {
        let e = find_mut(root, name).ok_or_else(|| err(format!("the scene has no {name}")))?;
        e.o_mut()?
            .set("position", Node::Val(vec3([p[0], p[1], p[2], 0.0])));
    }
    Ok(())
}

/// The element ids of a scene by path, to check later that none of the stock ones moved.
pub fn id_map(root: &Node) -> std::collections::BTreeMap<String, u32> {
    elements(root)
        .into_iter()
        .filter_map(|(p, e)| {
            let id = e.as_obj()?.get("elementId")?.as_val().and_then(Val::as_u32)?;
            Some((p, id))
        })
        .collect()
}

/// Gives `e` one animation: a copy of `template`'s first `anims` entry whose asset is `asset`.
pub fn set_anim(e: &mut Node, template: &Node, asset: &str) -> Result<()> {
    let list = template
        .as_obj()
        .and_then(|o| o.get("anims"))
        .and_then(Node::as_arr)
        .ok_or_else(|| err("the template element has no anims"))?;
    let first = list
        .items
        .first()
        .ok_or_else(|| err("the template element has no animation entry"))?;
    let mut entry = detached(first);
    entry.o_mut()?.set("asset", Node::Val(Val::str(asset)));
    let tag = list.tag;
    e.o_mut()?.set(
        "anims",
        Node::Arr(Arr {
            items: vec![entry],
            tag,
        }),
    );
    Ok(())
}

pub fn children_tag(e: &Node) -> [u8; 4] {
    e.as_obj()
        .and_then(|o| o.get("children"))
        .and_then(Node::as_arr)
        .map_or(*b"!obj", |a| a.tag)
}

pub fn clear_list(e: &mut Node, key: &str) -> Result<()> {
    if let Some(Node::Arr(a)) = e.o_mut()?.get_mut(key) {
        a.items.clear();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::node::{arr, obj, s, u32v};

    const PATH: &str = "eaid://ui_package/art/screens/test/test.idf";

    fn elem(name: &str, kids: Vec<Node>) -> Node {
        obj(vec![
            ("name", s(name)),
            ("elementId", u32v(0)),
            (
                "params",
                arr(vec![obj(vec![("value", s("old")), ("name", s("guid"))])]),
            ),
            ("children", arr(kids)),
        ])
    }

    fn scene(root: Node) -> Pkg {
        let entry = obj(vec![
            ("scene", root),
            (
                "name",
                Node::Val(Val::raw(*b"ui64", scene_id(PATH).to_le_bytes())),
            ),
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
    fn scene_ids_drop_the_idf_suffix() {
        assert_eq!(scene_id(PATH), component_id(PATH.strip_suffix(".idf").unwrap()));
        assert_eq!(
            material_binding(PATH),
            "eaid://ui_package/art/screens/test/test.emb?context=default"
        );
    }

    #[test]
    fn a_cloned_slot_gets_its_ids_and_lookup() {
        let mut p = scene(elem("Scene Root", vec![elem("row", vec![elem("tile", vec![])])]));
        let entry = &mut scenes_mut(&mut p).unwrap()[0];
        let root = entry.o_mut().unwrap().at_mut("scene").unwrap();
        let mut clone = detached(find(root, "row").unwrap());
        clone.o_mut().unwrap().set("name", s("other"));
        set_param(&mut clone, "guid", Val::str("new")).unwrap();
        set_param(&mut clone, "width", Val::raw(*b"fp32", 2.5f32.to_le_bytes())).unwrap();
        recompute_ids(&mut clone, "other").unwrap();
        children_mut(root).unwrap().push(clone);
        let row = children_mut(root).unwrap().first_mut().unwrap();
        recompute_ids(row, "row").unwrap();
        relookup(entry).unwrap();
        let back = Pkg::parse(&p.to_bytes().unwrap()).unwrap();
        assert!(id_problems(&back).unwrap().is_empty());
        let root = scenes(&back).unwrap()[0].o().unwrap().at("scene").unwrap();
        let paths: Vec<String> = elements(root).into_iter().map(|(p, _)| p).collect();
        assert_eq!(paths, ["Scene Root", "row", "row.tile", "other", "other.tile"]);
        let other = find(root, "other").unwrap();
        assert_eq!(param_str(other, "guid"), Some("new"));
        assert_eq!(param(other, "width").unwrap().ty, *b"fp32");
        let names: Vec<&str> = other
            .o()
            .unwrap()
            .at("params")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .map(|p| p.o().unwrap().str_at("name").unwrap())
            .collect();
        assert_eq!(names, ["guid", "width"]);
        assert_eq!(param_str(find(root, "row").unwrap(), "guid"), Some("old"));
        let lookup: Vec<u32> = scenes(&back).unwrap()[0]
            .o()
            .unwrap()
            .at("nodeidlookup")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .map(|v| v.v().unwrap().as_u32().unwrap())
            .collect();
        let mut want: Vec<u32> = ["Scene Root", "row", "tile", "other"].map(djb2).to_vec();
        want.sort_unstable();
        assert_eq!(lookup, want);
    }

    #[test]
    fn retargeting_moves_the_xref_and_the_material_binding() {
        let old = "eaid://ui_package/art/a/old.idf";
        let new = "eaid://ui_package/art/b/new.idf";
        let mut x = elem("ref", vec![]);
        set_param(&mut x, "fileRef", Val::str(old)).unwrap();
        set_param(&mut x, "XrefMaterialBinding", Val::str(material_binding(old))).unwrap();
        assert_eq!(xref(&x), Some(scene_id(old)));
        retarget(&mut x, new).unwrap();
        assert_eq!(param_str(&x, "fileRef"), Some(new));
        assert_eq!(xref(&x), Some(scene_id(new)));
        assert_eq!(
            param_str(&x, "XrefMaterialBinding"),
            Some(material_binding(new).as_str())
        );
        assert!(retarget(&mut x, "eaid://ui_package/art/b/new.png").is_err());
        let mut p = scene(elem("Scene Root", vec![x]));
        assert!(xref_mismatches(&p).unwrap().is_empty());
        let root = scenes_mut(&mut p).unwrap()[0]
            .o_mut()
            .unwrap()
            .at_mut("scene")
            .unwrap();
        put_param(&mut children_mut(root).unwrap()[0], "fileRef", Val::str(old)).unwrap();
        assert_eq!(xref_mismatches(&p).unwrap().len(), 1);
    }

    #[test]
    fn detached_values_lose_their_table_slots() {
        let p = scene(elem("Scene Root", vec![elem("row", vec![])]));
        let back = Pkg::parse(&p.to_bytes().unwrap()).unwrap();
        let root = scenes(&back).unwrap()[0].o().unwrap().at("scene").unwrap();
        let fresh = format!("{:?}", detached(root));
        assert!(!fresh.contains("slot: Some"), "{fresh}");
        assert!(format!("{root:?}").contains("slot: Some"));
    }
}
