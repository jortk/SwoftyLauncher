use std::collections::BTreeMap;

use serde_json::Value as Json;
use swofty_formats::pkg::{Node, Tag, Val, Value};

use crate::ops::{PkgEdit, PkgTarget};
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
enum Step {
    Key(String),
    Index(usize),
    Match(String, String),
}

fn fail(msg: impl Into<String>) -> Error {
    Error::Resolve(msg.into())
}

fn steps(path: &str) -> Result<Vec<Step>> {
    let mut out = Vec::new();
    let mut rest = path.trim();
    while !rest.is_empty() {
        if let Some(inner) = rest.strip_prefix('[') {
            let end = inner
                .find(']')
                .ok_or_else(|| fail(format!("{path}: unclosed [")))?;
            let sel = &inner[..end];
            out.push(match sel.split_once('=') {
                Some((k, v)) => Step::Match(k.trim().to_owned(), v.to_owned()),
                None => Step::Index(
                    sel.trim()
                        .parse()
                        .map_err(|_| fail(format!("{path}: bad selector [{sel}]")))?,
                ),
            });
            rest = &inner[end + 1..];
        } else {
            let end = rest.find(['.', '[']).unwrap_or(rest.len());
            if end > 0 {
                out.push(Step::Key(rest[..end].to_owned()));
            }
            rest = &rest[end..];
        }
        rest = rest.strip_prefix('.').unwrap_or(rest);
    }
    Ok(out)
}

fn val_text(v: &Val) -> Option<String> {
    if let Some(s) = v.as_str() {
        return Some(s.to_owned());
    }
    match &v.ty {
        b"ui32" | b"si32" | b"ui16" | b"ui08" => v.as_u32().map(|n| match &v.ty {
            b"si32" => (n as i32).to_string(),
            _ => n.to_string(),
        }),
        _ => None,
    }
}

fn matches(node: &Node, key: &str, want: &str) -> bool {
    node.as_obj()
        .and_then(|o| o.get(key))
        .and_then(Node::as_val)
        .and_then(val_text)
        .is_some_and(|t| t == want)
}

fn position(items: &[Node], step: &Step) -> Option<usize> {
    match step {
        Step::Index(i) => (*i < items.len()).then_some(*i),
        Step::Match(k, v) => items.iter().position(|n| matches(n, k, v)),
        Step::Key(_) => None,
    }
}

fn child<'a>(node: &'a Node, step: &Step) -> Option<&'a Node> {
    match (node, step) {
        (Node::Obj(o), Step::Key(k)) => o.get(k),
        (Node::Arr(a), s) => position(&a.items, s).map(|i| &a.items[i]),
        _ => None,
    }
}

fn child_mut<'a>(node: &'a mut Node, step: &Step) -> Option<&'a mut Node> {
    match (node, step) {
        (Node::Obj(o), Step::Key(k)) => o.get_mut(k),
        (Node::Arr(a), s) => position(&a.items, s).map(|i| &mut a.items[i]),
        _ => None,
    }
}

fn walk<'a>(node: &'a Node, path: &str) -> Result<&'a Node> {
    let mut cur = node;
    for s in steps(path)? {
        cur = child(cur, &s).ok_or_else(|| fail(format!("{path}: no {s:?}")))?;
    }
    Ok(cur)
}

fn walk_mut<'a>(node: &'a mut Node, path: &str) -> Result<&'a mut Node> {
    let mut cur = node;
    for s in steps(path)? {
        cur = child_mut(cur, &s).ok_or_else(|| fail(format!("{path}: no {s:?}")))?;
    }
    Ok(cur)
}

fn floats(v: &Json, n: &[usize]) -> Option<Vec<u8>> {
    let a = v.as_array()?;
    if !n.contains(&a.len()) {
        return None;
    }
    let mut out = Vec::with_capacity(4 * n[n.len() - 1]);
    for x in a {
        out.extend_from_slice(&(x.as_f64()? as f32).to_le_bytes());
    }
    out.resize(4 * n[n.len() - 1], 0);
    Some(out)
}

fn hex(s: &str, len: usize) -> Option<Vec<u8>> {
    let s = s.trim_start_matches('#');
    if s.len() != 2 * len || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..len)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}

pub fn json_to_val(ty: Tag, v: &Json) -> Result<Val> {
    let int = || v.as_i64().or_else(|| v.as_str()?.trim().parse().ok());
    let uint = || v.as_u64().or_else(|| v.as_str()?.trim().parse().ok());
    let num = || v.as_f64().or_else(|| v.as_str()?.trim().parse().ok());
    let bytes = match &ty {
        b"stri" => {
            return v
                .as_str()
                .map(Val::str)
                .ok_or_else(|| fail(format!("{v} is not a string")));
        }
        b"ui32" => uint()
            .and_then(|n| u32::try_from(n).ok())
            .map(|n| n.to_le_bytes().to_vec()),
        b"si32" => int()
            .and_then(|n| i32::try_from(n).ok())
            .map(|n| n.to_le_bytes().to_vec()),
        b"ui16" => uint()
            .and_then(|n| u16::try_from(n).ok())
            .map(|n| n.to_le_bytes().to_vec()),
        b"ui08" => uint().and_then(|n| u8::try_from(n).ok()).map(|n| vec![n]),
        b"ui64" => uint().map(|n| n.to_le_bytes().to_vec()),
        b"si64" => int().map(|n| n.to_le_bytes().to_vec()),
        b"bool" => v.as_bool().map(|b| vec![b as u8]),
        b"fp32" => num().map(|x| (x as f32).to_le_bytes().to_vec()),
        b"vec3" | b"vec4" | b"quat" => floats(v, &[3, 4]),
        b"mat4" => floats(v, &[16]),
        b"bbox" => floats(v, &[8]),
        b"woid" => v.as_str().and_then(|s| hex(s, 16)),
        b"rgba" => v.as_str().and_then(|s| hex(s, 4)),
        b"shnm" => v.as_str().filter(|s| s.len() <= 16).map(|s| {
            let mut b = s.as_bytes().to_vec();
            b.resize(16, 0);
            b
        }),
        _ => {
            return Err(fail(format!(
                "values of type {} cannot be set yet",
                String::from_utf8_lossy(&ty)
            )));
        }
    };
    bytes.map(|b| Val::raw(ty, b)).ok_or_else(|| {
        fail(format!(
            "{v} is not a valid {} value",
            String::from_utf8_lossy(&ty)
        ))
    })
}

fn set_leaf(node: &mut Node, path: &str, v: &Json) -> Result<()> {
    let leaf = walk_mut(node, path)?;
    let Node::Val(old) = leaf else {
        return Err(fail(format!("{path} is not a value")));
    };
    *old = json_to_val(old.ty, v).map_err(|e| fail(format!("{path}: {e}")))?;
    Ok(())
}

fn set_all(node: &mut Node, set: &BTreeMap<String, Json>) -> Result<()> {
    set.iter().try_for_each(|(path, v)| set_leaf(node, path, v))
}

pub fn supported(edit: &PkgEdit) -> bool {
    matches!(edit, PkgEdit::CloneEntry { .. } | PkgEdit::SetValue { .. })
}

pub fn sources(edits: &[PkgEdit]) -> impl Iterator<Item = &PkgTarget> {
    edits.iter().filter_map(|e| match e {
        PkgEdit::CloneEntry { from: Some(t), .. } => Some(t),
        _ => None,
    })
}

pub fn edit_name(edit: &PkgEdit) -> &'static str {
    match edit {
        PkgEdit::CloneNode { .. } => "clone-node",
        PkgEdit::SetText { .. } => "set-text",
        PkgEdit::SetProps { .. } => "set-props",
        PkgEdit::AddSignal { .. } => "add-signal",
        PkgEdit::RemoveSignal { .. } => "remove-signal",
        PkgEdit::CloneEntry { .. } => "clone-entry",
        PkgEdit::SetValue { .. } => "set-value",
    }
}

pub fn apply_edits<'s>(
    root: &mut Node,
    edits: &[PkgEdit],
    source: &dyn Fn(&PkgTarget) -> Option<&'s Node>,
) -> Result<usize> {
    for edit in edits {
        match edit {
            PkgEdit::CloneEntry {
                at,
                from,
                src,
                name,
                set,
            } => {
                let origin = match from {
                    Some(t) => {
                        source(t).ok_or_else(|| fail(format!("{}: {} was not loaded", t.erp, t.resource)))?
                    }
                    None => &*root,
                };
                let sel = steps(src)?;
                let [sel] = sel.as_slice() else {
                    return Err(fail(format!("{src}: clone-entry takes one key or selector")));
                };
                let mut entry = child(walk(origin, at)?, sel)
                    .ok_or_else(|| fail(format!("{at}: no entry {src}")))?
                    .clone();
                let container = walk_mut(root, at)?;
                match (container, sel) {
                    (Node::Obj(o), Step::Key(_)) => {
                        if o.get(name).is_some() {
                            return Err(fail(format!("{at}: {name} already exists")));
                        }
                        set_all(&mut entry, set)?;
                        o.items.push((Val::str(name.clone()), entry));
                    }
                    (Node::Arr(a), Step::Match(key, _)) => {
                        if a.items.iter().any(|n| matches(n, key, name)) {
                            return Err(fail(format!("{at}: an entry with {key}={name} already exists")));
                        }
                        set_leaf(&mut entry, key, &Json::String(name.clone()))?;
                        set_all(&mut entry, set)?;
                        a.items.push(entry);
                    }
                    _ => {
                        return Err(fail(format!(
                            "{at}: {src} does not select an entry of this container"
                        )));
                    }
                }
            }
            PkgEdit::SetValue { path, value } => set_leaf(root, path, value)?,
            other => return Err(fail(format!("{} is not built yet", edit_name(other)))),
        }
    }
    Ok(edits.len())
}

pub fn value_text(node: &Node, path: &str) -> Result<String> {
    match walk(node, path)? {
        Node::Val(v) => match &v.value {
            Value::Str(s) => Ok(s.clone()),
            Value::Raw(b) => Ok(b.iter().map(|x| format!("{x:02x}")).collect()),
        },
        _ => Err(fail(format!("{path} is not a value"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swofty_formats::pkg::{Arr, Obj, Pkg};

    fn inst(name: &str, id: u32) -> Node {
        let mut m = vec![0u8; 64];
        m[..4].copy_from_slice(&1f32.to_le_bytes());
        Node::Obj(Obj::new(vec![
            (
                Val::str("uniqueid"),
                Node::Val(Val::raw(*b"woid", vec![id as u8; 16])),
            ),
            (Val::str("name"), Node::Val(Val::str(name))),
            (
                Val::str("overrides"),
                Node::Obj(Obj::new(vec![
                    (
                        Val::str("WorldObject"),
                        Node::Obj(Obj::new(vec![(
                            Val::str("transform"),
                            Node::Val(Val::raw(*b"mat4", m)),
                        )])),
                    ),
                    (
                        Val::str("VehPlacement"),
                        Node::Obj(Obj::new(vec![(Val::str("Id"), Node::Val(Val::u32(id)))])),
                    ),
                ])),
            ),
        ]))
    }

    fn layer(names: &[(&str, u32)]) -> Pkg {
        Pkg {
            name: *b"INST",
            root: Node::Obj(Obj::new(vec![(
                Val::str("instances"),
                Node::Arr(Arr::new(names.iter().map(|(n, i)| inst(n, *i)).collect())),
            )])),
            vca: Vec::new(),
            layout: None,
        }
    }

    fn edits(json: &str) -> Vec<PkgEdit> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn parses_paths_with_selectors() {
        assert_eq!(
            steps("instances[name=a.b$c].overrides.F1Garage.left car placement").unwrap(),
            vec![
                Step::Key("instances".into()),
                Step::Match("name".into(), "a.b$c".into()),
                Step::Key("overrides".into()),
                Step::Key("F1Garage".into()),
                Step::Key("left car placement".into()),
            ]
        );
        assert_eq!(steps("[3]").unwrap(), vec![Step::Index(3)]);
        assert!(steps("a[name=x").is_err());
    }

    #[test]
    fn clones_an_array_entry_with_typed_values() {
        let mut pkg = layer(&[("v$PitBox12", 12)]);
        let e = edits(
            r#"[{"op":"clone-entry","at":"instances","src":"[name=v$PitBox12]","as":"v$PitBox13",
            "set":{"uniqueid":"00112233445566778899aabbccddeeff","overrides.VehPlacement.Id":13,
            "overrides.WorldObject.transform":[1,0,0,0, 0,1,0,0, 0,0,1,0, -346.04,-15.4,-190.41,1]}}]"#,
        );
        apply_edits(&mut pkg.root, &e, &|_| None).unwrap();
        let bytes = pkg.to_bytes().unwrap();
        let back = Pkg::parse(&bytes).unwrap();
        assert_eq!(back.to_bytes().unwrap(), bytes);
        let new = walk(&back.root, "instances[name=v$PitBox13]").unwrap();
        assert_eq!(
            value_text(new, "uniqueid").unwrap(),
            "00112233445566778899aabbccddeeff"
        );
        assert_eq!(value_text(new, "overrides.VehPlacement.Id").unwrap(), "0d000000");
        let Node::Val(m) = walk(new, "overrides.WorldObject.transform").unwrap() else {
            panic!()
        };
        let Value::Raw(m) = &m.value else { panic!() };
        assert_eq!(f32::from_le_bytes(m[48..52].try_into().unwrap()), -346.04f32);
        assert!(walk(&back.root, "instances[name=v$PitBox12]").is_ok());
        assert!(
            apply_edits(&mut pkg.root, &e, &|_| None).is_err(),
            "a second clone with the same name"
        );
    }

    #[test]
    fn clones_from_another_resource() {
        let mut pkg = layer(&[("v$PitBox12", 12)]);
        let common = layer(&[("c$GridSlot23", 23)]);
        let e = edits(
            r#"[{"op":"clone-entry","at":"instances","from":{"erp":"x.erp","resource":"c.layer"},
            "src":"[name=c$GridSlot23]","as":"v$GridSlot25","set":{"overrides.VehPlacement.Id":25}}]"#,
        );
        assert_eq!(sources(&e).count(), 1);
        apply_edits(&mut pkg.root, &e, &|t| {
            (t.resource == "c.layer").then_some(&common.root)
        })
        .unwrap();
        let new = walk(&pkg.root, "instances[name=v$GridSlot25]").unwrap();
        assert_eq!(value_text(new, "overrides.VehPlacement.Id").unwrap(), "19000000");
        assert!(walk(&common.root, "instances[name=v$GridSlot25]").is_err());
    }

    #[test]
    fn rejects_bad_values_and_paths() {
        let mut pkg = layer(&[("v$PitBox12", 12)]);
        for bad in [
            r#"[{"op":"clone-entry","at":"instances","src":"[name=nope]","as":"x"}]"#,
            r#"[{"op":"clone-entry","at":"instances","src":"[name=v$PitBox12]","as":"x","set":{"uniqueid":"abc"}}]"#,
            r#"[{"op":"clone-entry","at":"instances","src":"[name=v$PitBox12]","as":"x","set":{"overrides.VehPlacement.Id":-1}}]"#,
            r#"[{"op":"clone-entry","at":"instances","src":"[name=v$PitBox12]","as":"x","set":{"overrides.Missing.Id":1}}]"#,
            r#"[{"op":"set-value","path":"instances[0].overrides.WorldObject.transform","value":[1,2,3]}]"#,
        ] {
            assert!(
                apply_edits(&mut pkg.root, &edits(bad), &|_| None).is_err(),
                "{bad}"
            );
        }
        apply_edits(
            &mut pkg.root,
            &edits(r#"[{"op":"set-value","path":"instances[0].name","value":"renamed"}]"#),
            &|_| None,
        )
        .unwrap();
        assert_eq!(value_text(&pkg.root, "instances[0].name").unwrap(), "renamed");
    }

    #[test]
    fn refuses_a_hex_value_with_a_multibyte_character() {
        assert_eq!(hex("ff€000", 4), None);
        assert!(json_to_val(*b"rgba", &Json::from("ff€000")).is_err());
        assert!(json_to_val(*b"woid", &Json::from("€€00112233445566778899aabbcc")).is_err());
        assert_eq!(hex("#ff8000aa", 4), Some(vec![0xff, 0x80, 0x00, 0xaa]));
    }
}
