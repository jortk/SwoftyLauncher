//! `.uipre` (`UIPR`): the preview state the HUD editor shows for a scene.
//!
//! The editor draws a dial scene with no template running, so what it shows is the scene's own
//! state plus the setters of the scene's `.uipre`. A `UIPR` is a [`Pkg`] whose root holds
//! `Elements`, each an `Id` (djb2 of the element's name) with `Setters`, each a `Key` (a hash of
//! the property) and a typed `Value`; `TargetId` is the scene's component id. Setters seen:
//! [`KEY_TEXT`] with a string (the `text` of a label) and [`KEY_VISIBLE`] with a bool.
//!
//! ```
//! use swofty_formats::uipre::{Uipre, element_id};
//! # use swofty_formats::pkg::{Arr, Node, Obj, Pkg, Val};
//! # let empty = Pkg { name: *b"UIPR", root: Node::Obj(Obj::new(vec![
//! #     (Val::str("Elements"), Node::Arr(Arr::new(vec![]))),
//! #     (Val::str("TargetId"), Node::Val(Val::raw(*b"ui64", 7u64.to_le_bytes()))),
//! # ])), vca: vec![], layout: None };
//! # let bytes = empty.to_bytes().unwrap();
//! let mut pre = Uipre::parse(&bytes).unwrap();
//! pre.set_visible("fuel_icon_empty", false).unwrap();
//! pre.set_text("speedo_number", "135").unwrap();
//! assert_eq!(pre.element_ids(), [element_id("fuel_icon_empty"), element_id("speedo_number")]);
//! ```

use crate::hash::djb2;
use crate::pkg::{Arr, Node, Obj, Pkg, Val};
use crate::{Error, Result};

/// The setter key of a label's text (a `stri` value).
pub const KEY_TEXT: u32 = 4_045_306_521;
/// The setter key of an element's visibility (a `bool` value).
pub const KEY_VISIBLE: u32 = 839_438_211;
/// The setter key of a label's localised text: a `stri` value holding the game string id.
pub const KEY_LOC: u32 = 3_919_839_003;

/// The `Id` an element is listed under: djb2 of its name (the scene element's `name`, not the
/// dotted path).
pub fn element_id(name: &str) -> u32 {
    djb2(name)
}

/// A parsed `.uipre`.
#[derive(Debug, Clone)]
pub struct Uipre {
    pkg: Pkg,
}

fn bad(why: &str) -> Error {
    Error::Format(format!("not a UIPR: {why}"))
}

impl Uipre {
    /// Parses a `.uipre` resource's bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let pkg = Pkg::parse(bytes)?;
        let root = pkg
            .root
            .as_obj()
            .ok_or_else(|| bad("the root is not an object"))?;
        root.get("Elements")
            .and_then(Node::as_arr)
            .ok_or_else(|| bad("no Elements"))?;
        Ok(Uipre { pkg })
    }

    /// The bytes of the `.uipre`; unchanged parts keep the stock layout.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.pkg.to_bytes()
    }

    fn elements(&self) -> &Vec<Node> {
        &self
            .pkg
            .root
            .as_obj()
            .and_then(|o| o.get("Elements"))
            .and_then(Node::as_arr)
            .expect("checked by parse")
            .items
    }

    fn elements_mut(&mut self) -> &mut Vec<Node> {
        &mut self
            .pkg
            .root
            .as_obj_mut()
            .and_then(|o| o.get_mut("Elements"))
            .and_then(Node::as_arr_mut)
            .expect("checked by parse")
            .items
    }

    /// The component id of the scene this preview belongs to.
    pub fn target_id(&self) -> Option<u64> {
        let v = self.pkg.root.as_obj()?.get("TargetId")?.as_val()?;
        match &v.value {
            crate::pkg::Value::Raw(b) if b.len() >= 8 => Some(u64::from_le_bytes(b[..8].try_into().ok()?)),
            _ => None,
        }
    }

    /// Every listed element's `Id`, in file order.
    pub fn element_ids(&self) -> Vec<u32> {
        self.elements().iter().filter_map(id_of).collect()
    }

    /// The setters of element `id` as (key, value) pairs.
    pub fn setters(&self, id: u32) -> Vec<(u32, &Val)> {
        let Some(e) = self.elements().iter().find(|e| id_of(e) == Some(id)) else {
            return Vec::new();
        };
        let Some(arr) = e.as_obj().and_then(|o| o.get("Setters")).and_then(Node::as_arr) else {
            return Vec::new();
        };
        arr.items
            .iter()
            .filter_map(|s| {
                let o = s.as_obj()?;
                Some((o.get("Key")?.as_val()?.as_u32()?, o.get("Value")?.as_val()?))
            })
            .collect()
    }

    /// Sets setter `key` of element `id` to `value`, replacing it when the element has it and
    /// adding it (and the element) otherwise.
    pub fn set(&mut self, id: u32, key: u32, value: Val) -> Result<()> {
        let setter = Node::Obj(Obj::new(vec![
            (Val::str("Key"), Node::Val(Val::u32(key))),
            (Val::str("Value"), Node::Val(value)),
        ]));
        let els = self.elements_mut();
        let Some(el) = els.iter_mut().find(|e| id_of(e) == Some(id)) else {
            els.push(Node::Obj(Obj::new(vec![
                (Val::str("Id"), Node::Val(Val::u32(id))),
                (Val::str("Setters"), Node::Arr(Arr::new(vec![setter]))),
            ])));
            return Ok(());
        };
        let arr = el
            .as_obj_mut()
            .and_then(|o| o.get_mut("Setters"))
            .and_then(Node::as_arr_mut)
            .ok_or_else(|| bad("an element has no Setters"))?;
        match arr.items.iter_mut().find(|s| {
            s.as_obj()
                .and_then(|o| o.get("Key"))
                .and_then(Node::as_val)
                .and_then(Val::as_u32)
                == Some(key)
        }) {
            Some(slot) => *slot = setter,
            None => arr.items.push(setter),
        }
        Ok(())
    }

    /// Makes the editor show element `name` as `visible` or hidden.
    pub fn set_visible(&mut self, name: &str, visible: bool) -> Result<()> {
        self.set(element_id(name), KEY_VISIBLE, Val::bool(visible))
    }

    /// Makes the editor show label `name` with `text`.
    pub fn set_text(&mut self, name: &str, text: &str) -> Result<()> {
        self.set(element_id(name), KEY_TEXT, Val::str(text))
    }

    /// Sets setter `key` of the element at `path`, where every name after the first is an element
    /// inside the instance before it: `["tt_host", "tt_root"]` lists `tt_root` in the `Elements` of
    /// `tt_host`, the way the stock standings table lists the elements of its rows. Elements and
    /// instances missing from the file are added.
    pub fn set_at(&mut self, path: &[&str], key: u32, value: Val) -> Result<()> {
        if path.is_empty() {
            return Err(bad("an element path is at least one name"));
        }
        let setter = Node::Obj(Obj::new(vec![
            (Val::str("Key"), Node::Val(Val::u32(key))),
            (Val::str("Value"), Node::Val(value)),
        ]));
        let mut els = self.elements_mut();
        for (i, name) in path.iter().enumerate() {
            let id = element_id(name);
            let last = i + 1 == path.len();
            let pos = match els.iter().position(|e| id_of(e) == Some(id)) {
                Some(p) => p,
                None => {
                    els.push(Node::Obj(Obj::new(vec![
                        (Val::str("Id"), Node::Val(Val::u32(id))),
                        (Val::str("Setters"), Node::Arr(Arr::new(vec![]))),
                    ])));
                    els.len() - 1
                }
            };
            let el = els[pos]
                .as_obj_mut()
                .ok_or_else(|| bad("an element is not an object"))?;
            if last {
                if el.get("Setters").is_none() {
                    el.set("Setters", Node::Arr(Arr::new(vec![])));
                }
                let arr = el
                    .get_mut("Setters")
                    .and_then(Node::as_arr_mut)
                    .ok_or_else(|| bad("an element has no Setters"))?;
                match arr.items.iter_mut().find(|s| {
                    s.as_obj()
                        .and_then(|o| o.get("Key"))
                        .and_then(Node::as_val)
                        .and_then(Val::as_u32)
                        == Some(key)
                }) {
                    Some(slot) => *slot = setter,
                    None => arr.items.push(setter),
                }
                return Ok(());
            }
            if el.get("Elements").is_none() {
                el.set("Elements", Node::Arr(Arr::new(vec![])));
            }
            els = &mut el
                .get_mut("Elements")
                .and_then(Node::as_arr_mut)
                .ok_or_else(|| bad("an instance's Elements is not a list"))?
                .items;
        }
        Ok(())
    }

    /// Shows or hides the element at `path` (see [`Self::set_at`]).
    pub fn set_visible_at(&mut self, path: &[&str], visible: bool) -> Result<()> {
        self.set_at(path, KEY_VISIBLE, Val::bool(visible))
    }

    /// Fills the label at `path` with `text`.
    pub fn set_text_at(&mut self, path: &[&str], text: &str) -> Result<()> {
        self.set_at(path, KEY_TEXT, Val::str(text))
    }

    /// Makes the label at `path` show the game string `id`.
    pub fn set_loc_at(&mut self, path: &[&str], id: &str) -> Result<()> {
        self.set_at(path, KEY_LOC, Val::str(id))
    }

    /// The `Id`s listed inside instance `name`, in file order; empty when it is not listed or holds
    /// no `Elements`.
    pub fn element_ids_in(&self, name: &str) -> Vec<u32> {
        let id = element_id(name);
        self.elements()
            .iter()
            .find(|e| id_of(e) == Some(id))
            .and_then(|e| e.as_obj()?.get("Elements")?.as_arr())
            .map(|a| a.items.iter().filter_map(id_of).collect())
            .unwrap_or_default()
    }

    /// Removes element `id` and all its setters; true when it was listed.
    pub fn remove(&mut self, id: u32) -> bool {
        let els = self.elements_mut();
        let before = els.len();
        els.retain(|e| id_of(e) != Some(id));
        els.len() != before
    }
}

fn id_of(e: &Node) -> Option<u32> {
    e.as_obj()?.get("Id")?.as_val()?.as_u32()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> Vec<u8> {
        Pkg {
            name: *b"UIPR",
            root: Node::Obj(Obj::new(vec![
                (Val::str("Elements"), Node::Arr(Arr::new(vec![]))),
                (
                    Val::str("TargetId"),
                    Node::Val(Val::raw(*b"ui64", 99u64.to_le_bytes())),
                ),
            ])),
            vca: vec![],
            layout: None,
        }
        .to_bytes()
        .unwrap()
    }

    #[test]
    fn setters_are_added_replaced_and_removed() {
        let mut p = Uipre::parse(&empty()).unwrap();
        assert_eq!(p.target_id(), Some(99));
        p.set_text("speedo_number", "135").unwrap();
        p.set_visible("speedo_number", false).unwrap();
        p.set_text("speedo_number", "88").unwrap();
        p.set_visible("fuel", true).unwrap();
        let back = Uipre::parse(&p.to_bytes().unwrap()).unwrap();
        let id = element_id("speedo_number");
        assert_eq!(back.element_ids(), [id, element_id("fuel")]);
        let s = back.setters(id);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].0, s[0].1.as_str()), (KEY_TEXT, Some("88")));
        assert_eq!(s[1].0, KEY_VISIBLE);
        assert!(back.clone().remove(id));
        assert!(!back.clone().remove(1));
    }

    #[test]
    fn nested_setters_sit_inside_their_instance() {
        let mut p = Uipre::parse(&empty()).unwrap();
        p.set_visible_at(&["host", "root"], false).unwrap();
        p.set_text_at(&["host", "pos"], "10").unwrap();
        p.set_loc_at(&["host", "lap"], "lng_lap").unwrap();
        p.set_visible("mask", false).unwrap();
        assert_eq!(p.element_ids(), [element_id("host"), element_id("mask")]);
        assert_eq!(
            p.element_ids_in("host"),
            [element_id("root"), element_id("pos"), element_id("lap")]
        );
        assert!(p.setters(element_id("host")).is_empty());
        assert!(p.element_ids_in("mask").is_empty());
        p.set_text_at(&["host", "pos"], "11").unwrap();
        assert_eq!(p.element_ids_in("host").len(), 3);
        let back = Uipre::parse(&p.to_bytes().unwrap()).unwrap();
        assert_eq!(back.element_ids_in("host").len(), 3);
        assert!(p.set_at(&[], KEY_TEXT, Val::str("x")).is_err());
    }

    #[test]
    fn rejects_other_packages() {
        let p = Pkg {
            name: *b"ANIM",
            root: Node::Obj(Obj::new(vec![])),
            vca: vec![],
            layout: None,
        };
        assert!(Uipre::parse(&p.to_bytes().unwrap()).is_err());
        assert!(Uipre::parse(b"nope").is_err());
    }

    #[test]
    fn the_stock_keys_are_the_game_hashes() {
        assert_eq!(element_id("speedo_number"), 1_073_894_029);
    }
}
