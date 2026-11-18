//! ADDON SETTINGS' home lists its tiles in a row per launcher folder. The home keeps one block (o_block)
//! in the options screen's spawner, whose top-to-bottom stacker 1a holds a label and a row per folder.
//! A label is a copy of the screen title, on its own title scene, as the career content select stacks
//! its Built In and Addons labels; a row's scene is O_ROW, a copy of o_block whose stacker 1a stacks
//! left to right. Rows scroll down by hiding the row and label at the top edge and a row scrolls along
//! by hiding its tile at the left edge; both stackers close up.

use swofty_formats::pkg::{Node, Pkg, Val, Value};

use super::era::place;
use super::idf::{self, elem_name};
use super::node::*;
use crate::Result;

/// The row scene, added to persistent.erp.
pub const O_ROW: &str = "eaid://ui_package/art/components/options/options_spawned/swofty_o_row.idf";
/// The stock block scene O_ROW copies.
pub const O_BLOCK: &str = "eaid://ui_package/art/components/options/options_spawned/o_block.idf";
/// The options screen title's own scene, which a label draws on.
pub const TITLE_SCENE: &str = "eaid://ui_package/art/components/options/o_gen_screen_title.idf";
pub const STACKER: &str = "1a";
const INTO: &str = "block_into_1";
/// Where a row's tiles start: the title scene draws a label this far left of where the block stacks
/// it, so the row starts there too.
const ROW_X: f64 = -8.6;

fn set_scene_name(entry: &mut Node, id: u64) -> Result<()> {
    let o = entry.o_mut()?;
    let mut v = o
        .get("name")
        .and_then(Node::as_val)
        .cloned()
        .ok_or_else(|| err("a scene without a name"))?;
    match &mut v.value {
        Value::Raw(b) if b.len() >= 8 => b[..8].copy_from_slice(&id.to_le_bytes()),
        _ => return Err(err("a scene name that is not a component id")),
    }
    o.set("name", Node::Val(v));
    Ok(())
}

/// O_ROW from the stock o_block scene: its first stacker, turned left to right.
pub fn row_scene(block: &[u8]) -> Result<Vec<u8>> {
    let mut doc = Pkg::parse(block)?;
    let entry = idf::scenes_mut(&mut doc)?
        .first_mut()
        .ok_or_else(|| err(format!("{O_BLOCK} has no scenes")))?;
    if idf::scene_name(entry) != Some(idf::scene_id(O_BLOCK)) {
        return Err(err(format!("not {O_BLOCK}")));
    }
    set_scene_name(entry, idf::scene_id(O_ROW))?;
    let root = entry.o_mut()?.at_mut("scene")?;
    let kids = idf::children_mut(root)?;
    kids.retain(|k| elem_name(k) == INTO);
    let into = kids
        .first_mut()
        .ok_or_else(|| err(format!("{O_BLOCK} has no {INTO}")))?;
    place(into, ROW_X, 0.0, None, None)?;
    let stacker = idf::children_mut(into)?
        .iter_mut()
        .find(|k| elem_name(k) == STACKER)
        .ok_or_else(|| err(format!("{O_BLOCK} has no stacker {STACKER}")))?;
    idf::set_param(stacker, "stackerType", Val::str("l"))?;
    idf::relookup(entry)?;
    let out = doc.to_bytes()?;
    let back = Pkg::parse(&out)?;
    if back.to_bytes()? != out {
        return Err(err(format!("{O_ROW} does not round-trip")));
    }
    let bad = idf::id_problems(&back)?;
    if !bad.is_empty() {
        return Err(err(format!("{O_ROW}: {}", bad.join("; "))));
    }
    Ok(out)
}
