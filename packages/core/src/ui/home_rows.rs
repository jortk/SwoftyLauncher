use swofty_formats::pkg::{Node, Pkg, Val};

use super::era::set_layout;
use super::idf::{self, elem_name};
use super::node::*;
use crate::Result;

pub const SCENE: &str = "eaid://ui_package/art/screens/frontend/fe_home.idf";
pub const TEMPLATES: &str = "eaid://ui_package/screens/fe_home.ui";
pub const TEMPLATES_FRAGMENT: usize = 1;
pub const ARCHIVE: &str = "flow_frontend.erp";
pub const FLAG: &str = "home_rows.flag";
pub const MENU_FLAG: &str = "home_menu.flag";
pub const NAV_FLAG: &str = "home_nav.flag";
pub const CUSTOMISATION_FLAG: &str = "home_customisation.flag";

const STOCK_TEMPLATE: &str = "fe_tile_button_default";
const STOCK_SLOT: &str = "spawner_stacker_items";
const DESCRIPTION: &str = "text_stacker";
const PATH: &str = "transition_anim_items.transition_fade_items";

pub struct Row {
    pub stock: &'static str,
    pub template: &'static str,
    pub slot: &'static str,
    pub x: f64,
    pub y: f64,
    pub guid: &'static str,
}

const LEFT: f64 = -8.6;
const RIGHT: f64 = -4.0;
const THIRD: f64 = 0.6;

const fn row(template: &'static str, slot: &'static str, x: f64, y: f64, guid: &'static str) -> Row {
    Row {
        stock: STOCK_TEMPLATE,
        template,
        slot,
        x,
        y,
        guid,
    }
}

pub const ROWS: [Row; 10] = [
    row(
        "swofty_home_top_left",
        "swofty_home_slot_top_left",
        LEFT,
        3.2,
        "a1c3e5f7-1b2d-4f60-8a91-b3c5d7e9f101",
    ),
    row(
        "swofty_home_top_right",
        "swofty_home_slot_top_right",
        RIGHT,
        3.2,
        "a2c3e5f7-1b2d-4f60-8a91-b3c5d7e9f102",
    ),
    row(
        "swofty_home_top_third",
        "swofty_home_slot_top_third",
        THIRD,
        3.2,
        "a3c3e5f7-1b2d-4f60-8a91-b3c5d7e9f103",
    ),
    row(
        "swofty_home_modes_left",
        "swofty_home_slot_modes_left",
        LEFT,
        2.45,
        "b1d4f6a8-2c3e-4a71-9ba2-c4d6e8f0a201",
    ),
    row(
        "swofty_home_modes_right",
        "swofty_home_slot_modes_right",
        RIGHT,
        2.45,
        "b2d4f6a8-2c3e-4a71-9ba2-c4d6e8f0a202",
    ),
    row(
        "swofty_home_modes_third",
        "swofty_home_slot_modes_third",
        THIRD,
        2.45,
        "b3d4f6a8-2c3e-4a71-9ba2-c4d6e8f0a203",
    ),
    row(
        "swofty_home_wide_a",
        "swofty_home_row_wide_a",
        LEFT,
        1.7,
        "c3e5a7b9-3d4f-4b82-8cb3-d5e7f9a1b303",
    ),
    row(
        "swofty_home_wide_b",
        "swofty_home_row_wide_b",
        LEFT,
        0.95,
        "d4f6b8c0-4e5a-4c93-9dc4-e6f8a0b2c404",
    ),
    Row {
        stock: "fe_tile_button_divider",
        template: "swofty_home_divider",
        slot: "swofty_home_row_divider",
        x: LEFT,
        y: 0.5,
        guid: "f6b8d0e2-6a7c-4eb5-9fe6-a8b0c2d4e606",
    },
    row(
        "swofty_home_wide_c",
        "swofty_home_row_wide_c",
        LEFT,
        0.0,
        "e5a7c9d1-5f6b-4da4-8ed5-f7a9b1c3d505",
    ),
];

const DESCRIPTION_Y: f64 = -1.1;

fn templates(doc: &mut Pkg) -> Result<&mut Vec<Node>> {
    doc.root.o_mut()?.at_mut("Templates")?.a_mut()
}

pub fn has_templates(data: &[u8]) -> Result<bool> {
    let mut doc = Pkg::parse(data)?;
    Ok(templates(&mut doc)?
        .iter()
        .any(|t| t.is_node() && ROWS.iter().any(|r| r.template == t.name())))
}

pub fn build_templates(data: &[u8]) -> Result<Vec<u8>> {
    let mut doc = Pkg::parse(data)?;
    if doc.to_bytes()? != data {
        return Err(err(format!(
            "{TEMPLATES} fragment {TEMPLATES_FRAGMENT} does not round-trip"
        )));
    }
    let list = templates(&mut doc)?;
    if list
        .iter()
        .any(|t| t.is_node() && ROWS.iter().any(|r| r.template == t.name()))
    {
        return Err(err(format!("{TEMPLATES} already has the main menu templates")));
    }
    for row in &ROWS {
        let mut tpl = template_named(list, row.stock)?.clone();
        rename_refs(&mut tpl, row.stock, row.template);
        rename(&mut tpl, row.template)?;
        set_layout(&mut tpl, &[row.slot])?;
        fix_ids(&mut tpl, None);
        list.push(tpl);
    }
    let out = doc.to_bytes()?;
    if Pkg::parse(&out)?.to_bytes()? != out {
        return Err(err(format!("the patched {TEMPLATES} does not round-trip")));
    }
    Ok(out)
}

fn template_named<'a>(list: &'a [Node], name: &str) -> Result<&'a Node> {
    list.iter()
        .find(|t| t.is_node() && t.name() == name)
        .ok_or_else(|| err(format!("{TEMPLATES} has no {name}")))
}

fn find_mut<'a>(e: &'a mut Node, name: &str) -> Option<&'a mut Node> {
    if elem_name(e) == name {
        return Some(e);
    }
    let kids = idf::children_mut(e).ok()?;
    kids.iter_mut().find_map(|k| find_mut(k, name))
}

fn set_position(e: &mut Node, x: f64, y: f64) -> Result<()> {
    e.o_mut()?.set("position", Node::Val(idf::vec3([x, y, 0.0, 0.0])));
    Ok(())
}

pub fn has_slots(data: &[u8]) -> Result<bool> {
    let doc = Pkg::parse(data)?;
    let scenes = idf::scenes(&doc)?;
    let root = scenes
        .first()
        .ok_or_else(|| err("a scene file without scenes"))?
        .o()?
        .at("scene")?;
    Ok(ROWS.iter().any(|r| idf::find(root, r.slot).is_some()))
}

pub fn build_scene(data: &[u8]) -> Result<Vec<u8>> {
    let mut doc = Pkg::parse(data)?;
    let entry = idf::scenes_mut(&mut doc)?
        .first_mut()
        .ok_or_else(|| err(format!("{SCENE} has no scenes")))?;
    if idf::scene_name(entry) != Some(idf::scene_id(SCENE)) {
        return Err(err(format!("not {SCENE}")));
    }
    let root = entry.o_mut()?.at_mut("scene")?;
    if ROWS.iter().any(|r| idf::find(root, r.slot).is_some()) {
        return Err(err(format!("{SCENE} already has the main menu rows")));
    }
    let parent = find_mut(root, "transition_fade_items")
        .ok_or_else(|| err(format!("{SCENE} has no transition_fade_items")))?;
    let kids = idf::children_mut(parent)?;
    let at = kids
        .iter()
        .position(|k| elem_name(k) == STOCK_SLOT)
        .ok_or_else(|| err(format!("{SCENE}: no {STOCK_SLOT}")))?;
    let mut added = Vec::new();
    for row in &ROWS {
        let mut slot = idf::detached(&kids[at]);
        slot.o_mut()?.set("name", s(row.slot));
        idf::set_param(&mut slot, "guid", Val::str(row.guid))?;
        idf::set_param(&mut slot, "stackerType", Val::str("t"))?;
        set_position(&mut slot, row.x, row.y)?;
        idf::recompute_ids(&mut slot, &format!("{PATH}.{}", row.slot))?;
        added.push(slot);
    }
    for (k, slot) in added.into_iter().enumerate() {
        kids.insert(at + 1 + k, slot);
    }
    let text = kids
        .iter_mut()
        .find(|k| elem_name(k) == DESCRIPTION)
        .ok_or_else(|| err(format!("{SCENE}: no {DESCRIPTION}")))?;
    set_position(text, LEFT, DESCRIPTION_Y)?;
    idf::relookup(entry)?;
    let out = doc.to_bytes()?;
    let back = Pkg::parse(&out)?;
    if back.to_bytes()? != out {
        return Err(err(format!("the patched {SCENE} does not round-trip")));
    }
    let mut bad = idf::xref_mismatches(&back)?;
    bad.extend(idf::id_problems(&back)?);
    if !bad.is_empty() {
        return Err(err(format!("{SCENE}: {}", bad.join("; "))));
    }
    Ok(out)
}

pub const PLAYERCARD: &str = "eaid://ui_package/screens/playercard.ui";
pub const PLAYERCARD_ARCHIVE: &str = "common_front_end.erp";
pub const STRIP_PATH: &str = "#ui_menu_data.swofty_strip_visible";
const STRIP: &str = "paddock_pass_panel";

pub fn has_strip_binding(data: &[u8]) -> Result<bool> {
    let mut doc = Pkg::parse(data)?;
    let screen = first_screen(&mut doc)?;
    let panel = screen
        .slot(CHILDREN)?
        .iter()
        .find(|k| k.is_node() && k.name() == STRIP)
        .ok_or_else(|| err(format!("{PLAYERCARD} has no {STRIP}")))?;
    Ok(panel
        .props()?
        .get("visible")
        .is_some_and(|v| format!("{v:?}").contains("swofty_strip_visible")))
}

/// playercard.ui with the top-right strip's group shown or hidden by `ui_menu_data.swofty_strip_visible`,
/// which the core sets false while the main menu is up and true otherwise (true until it is set).
pub fn hide_strip(data: &[u8]) -> Result<Vec<u8>> {
    let mut doc = Pkg::parse(data)?;
    if doc.to_bytes()? != data {
        return Err(err(format!("{PLAYERCARD} does not round-trip")));
    }
    let screen = first_screen(&mut doc)?;
    let owner = screen.name().to_owned();
    let panel = screen
        .slot_mut(CHILDREN)?
        .iter_mut()
        .find(|k| k.is_node() && k.name() == STRIP)
        .ok_or_else(|| err(format!("{PLAYERCARD} has no {STRIP}")))?;
    set_prop(panel, "visible", prop(boolv(true), Some(STRIP_PATH)))?;
    let _ = owner;
    let out = doc.to_bytes()?;
    if Pkg::parse(&out)?.to_bytes()? != out {
        return Err(err(format!("the patched {PLAYERCARD} does not round-trip")));
    }
    Ok(out)
}
