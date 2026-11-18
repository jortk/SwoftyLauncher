//! A repeated row group with a nested per-car repeat composes into a dial and into a stock scene
//! target, and the stacker and centre donors hold the facts the composer relies on. Needs
//! `F1_25_DIR`; the stock files are read through the launcher's baseline, never the game folder.

use std::path::PathBuf;

use swofty_core::hud::{self, GameStock, Stock, dials, geom, scene};
use swofty_core::store::Store;
use swofty_core::ui::idf;
use swofty_core::ui::node::{NodeExt, ObjExt, find_path, node_at};
use swofty_formats::erp::Erp;
use swofty_formats::pkg::{Pkg, Value};

const DAMAGE_SCENE: &str = "eaid://ui_package/art/osd/components/dial/mfd_car_damage_panel.idf";
const DAMAGE_PARENT: &str = "hud_mfd_dynamic.mfd.attacher.car_damage_panel";

fn real() -> Option<GameStock> {
    match std::env::var_os("F1_25_DIR") {
        Some(d) => Some(GameStock::with_baseline(PathBuf::from(d), &Store::open_default().root).unwrap()),
        None => {
            eprintln!("F1_25_DIR is not set; skipping");
            None
        }
    }
}

fn rows(target: serde_json::Value) -> serde_json::Value {
    let mut v = serde_json::json!({
        "elements": [
            { "kind": "stack", "id": "stack", "dir": "down", "gap": 1.5, "children": [
                { "kind": "group", "id": "top", "visible": { "view_model": "view_model_data.hud_tt_hdr" }, "children": [
                    { "kind": "centre", "id": "lapc", "at": [20, 4], "children": [
                        { "kind": "text", "id": "lap", "style": "OSD_19_regular", "cap_height": 6, "literal_id": "lng_lap" } ] } ] },
                { "kind": "group", "id": "rows", "at": [0, 5.73], "children": [
                    { "kind": "group", "id": "r", "repeat": { "count": 3, "var": "N", "pitch": [0, 21.635] },
                      "visible": { "view_model": "view_model_data.hud_tt_r{NN}_on" },
                      "children": [
                        { "kind": "text", "id": "pos", "style": "OSD_19_regular", "cap_height": 5.9, "at": [5.3, 14.6],
                          "value": { "view_model": "view_model_data.hud_tt_r{NN}_pos" } },
                        { "kind": "group", "id": "v", "repeat": { "count": 2, "var": "V" },
                          "visible": { "view_model": "view_model_data.hud_tt_r{NN}_c{VV}" },
                          "children": [
                            { "kind": "text", "id": "code", "style": "OSD_24_bold", "cap_height": 7.5,
                              "value": { "session_car": "position", "slot_var": "V" } } ] } ] } ] } ] }
        ],
        "editor": { "rows": 2, "preview": {
            "view_model_data.hud_tt_hdr": true,
            "view_model_data.hud_tt_r00_on": true, "view_model_data.hud_tt_r00_pos": "10",
            "view_model_data.hud_tt_r01_on": true, "view_model_data.hud_tt_r01_pos": "11",
            "view_model_data.hud_tt_r02_on": true, "view_model_data.hud_tt_r02_pos": "12" } }
    });
    for (k, val) in target.as_object().unwrap() {
        v[k] = val.clone();
    }
    v
}

fn input(record: &serde_json::Value) -> hud::Input {
    hud::Input {
        addon: "a.tower".into(),
        key: "rows".into(),
        dir: std::env::temp_dir(),
        record: hud::model::HudElement::parse(record).unwrap(),
    }
}

fn scene_names(erp: &Erp, scene: &str) -> (Pkg, Vec<String>) {
    let pkg = Pkg::parse(&erp.find(scene).unwrap().fragments[0].data().unwrap()).unwrap();
    let entry = &idf::scenes(&pkg).unwrap()[0];
    let names = idf::elements(entry.o().unwrap().at("scene").unwrap())
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    (pkg, names)
}

fn element<'a>(pkg: &'a Pkg, name: &str) -> &'a swofty_formats::pkg::Node {
    let entry = &idf::scenes(pkg).unwrap()[0];
    idf::find(entry.o().unwrap().at("scene").unwrap(), name).unwrap()
}

#[test]
fn the_stacker_and_centre_donors_hold_their_facts() {
    let Some(stock) = real() else { return };
    let read = |name: &str| stock.read(dials::STOCK_ERP, name).unwrap().remove(0);
    let stacked = Pkg::parse(&read(scene::STACKED)).unwrap();
    let stacker = element(&stacked, scene::STACKER_DONOR);
    assert_eq!(idf::param_str(stacker, "stackerType"), Some("t"));
    assert!(idf::param(stacker, "gap").is_some());
    let lap = Pkg::parse(&read(scene::LAP_COUNTER)).unwrap();
    let centre = element(&lap, scene::CENTRE_DONOR);
    assert!(matches!(&idf::param(centre, "centreX").unwrap().value, Value::Raw(b) if b.first() == Some(&1)));
    let om_xm = Pkg::parse(&read(dials::F126.scene)).unwrap();
    let text = element(&om_xm, "speedo_number");
    assert!(idf::param_str(text, "style").is_some());
    for (style, _) in geom::STYLES {
        assert!(geom::style_height(style).is_some(), "{style}");
    }
}

#[test]
fn a_repeated_row_group_composes_into_a_dial_with_the_preview_rows() {
    let Some(stock) = real() else { return };
    let out = hud::compose(
        &stock,
        &[input(&rows(
            serde_json::json!({ "dial": "f1_26", "box": [0, 0, 120, 80] }),
        ))],
    )
    .unwrap();
    let pfx = "a_tower_rows";
    let archive = out.archives.iter().find(|a| a.file == "hud_scene.erp").unwrap();
    let erp = Erp::parse(&archive.bytes).unwrap();
    let (pkg, names) = scene_names(&erp, dials::F126.scene);
    let root = format!("{}.{pfx}", dials::CUSTOMISATION);
    for n in [
        format!("{root}.{pfx}_stack"),
        format!("{root}.{pfx}_stack.{pfx}_top.{pfx}_lapc.{pfx}_lap_n.{pfx}_lap"),
        format!("{root}.{pfx}_stack.{pfx}_rows.{pfx}_r00"),
        format!("{root}.{pfx}_stack.{pfx}_rows.{pfx}_r02.{pfx}_r02_pos_n.{pfx}_r02_pos"),
        format!("{root}.{pfx}_stack.{pfx}_rows.{pfx}_r02.{pfx}_r02_v01.{pfx}_r02_v01_code_n"),
    ] {
        assert!(names.contains(&n), "{n} in {names:#?}");
    }
    assert!(!names.iter().any(|n| n.ends_with("_r03")));
    let stack = element(&pkg, &format!("{pfx}_stack"));
    assert_eq!(idf::param_str(stack, "stackerType"), Some("t"));
    let gap = match &idf::param(stack, "gap").unwrap().value {
        Value::Raw(b) => f32::from_le_bytes(b[..4].try_into().unwrap()) as f64,
        _ => panic!("gap is a float"),
    };
    assert!((gap - geom::K * 1.5).abs() < 1e-6);
    let rows_at = idf::position(element(&pkg, &format!("{pfx}_rows"))).unwrap();
    assert!((rows_at[1] + geom::K * 5.73).abs() < 1e-5, "{rows_at:?}");
    let r2 = idf::position(element(&pkg, &format!("{pfx}_r02"))).unwrap();
    assert!((r2[1] + geom::K * 2.0 * 21.635).abs() < 1e-5, "{r2:?}");
    assert!(idf::id_problems(&pkg).unwrap().is_empty());

    let (_, f125) = scene_names(&erp, dials::F125.scene);
    let pv = format!("{}.{pfx}_pv", dials::CUSTOMISATION);
    assert!(f125.contains(&format!("{pv}.{pfx}_pv_stack.{pfx}_pv_rows.{pfx}_pv_r01")));
    assert!(
        !f125.iter().any(|n| n.ends_with("_pv_r02")),
        "rows: 2 caps the preview"
    );

    let template = out
        .archives
        .iter()
        .find(|a| a.file == "hud_template.erp")
        .unwrap();
    let t = Pkg::parse(
        &Erp::parse(&template.bytes)
            .unwrap()
            .find(dials::TEMPLATE)
            .unwrap()
            .fragments[0]
            .data()
            .unwrap(),
    )
    .unwrap();
    let ctl = format!("{}.{pfx}_ctl", dials::F126.race_parent);
    for node in [
        "v_r00",
        "v_r02",
        "v_r02_v01",
        "t_r02_pos",
        "t_r02_v01_code",
        "t_lap",
    ] {
        assert!(
            find_path(&t.root, &format!("{ctl}.{node}")).is_ok(),
            "{ctl}.{node}"
        );
    }
    let at = find_path(&t.root, &format!("{ctl}.t_r02_v01_code")).unwrap();
    let text = swofty_core::ui::node::canonical(node_at(&t.root, &at).unwrap());
    assert!(text.contains("#session.cars.car1.position"), "{text}");
    let at = find_path(&t.root, &format!("{ctl}.t_lap")).unwrap();
    let lap = swofty_core::ui::node::canonical(node_at(&t.root, &at).unwrap());
    assert!(lap.contains("lng_lap") && lap.contains("localise"), "{lap}");
    let at = find_path(&t.root, &format!("{ctl}.v_r02_v01")).unwrap();
    let vis = swofty_core::ui::node::canonical(node_at(&t.root, &at).unwrap());
    assert!(vis.contains("#view_model_data.hud_tt_r02_c01"), "{vis}");
}

#[test]
fn a_repeated_row_group_composes_into_a_scene_target() {
    let Some(stock) = real() else { return };
    let mut rec = rows(serde_json::json!({ "scene": DAMAGE_SCENE, "parent_ui": DAMAGE_PARENT }));
    rec.as_object_mut().unwrap().remove("editor");
    let out = hud::compose(&stock, &[input(&rec)]).unwrap();
    let pfx = "a_tower_rows";
    let archive = out
        .archives
        .iter()
        .find(|a| a.file == "hud_target_scene.erp")
        .unwrap();
    let (pkg, names) = scene_names(&Erp::parse(&archive.bytes).unwrap(), DAMAGE_SCENE);
    for n in [
        format!("{pfx}.{pfx}_stack.{pfx}_rows.{pfx}_r00"),
        format!(
            "{pfx}.{pfx}_stack.{pfx}_rows.{pfx}_r02.{pfx}_r02_v01.{pfx}_r02_v01_code_n.{pfx}_r02_v01_code"
        ),
        format!("{pfx}.{pfx}_stack.{pfx}_top.{pfx}_lapc"),
    ] {
        assert!(names.contains(&n), "{n} in {names:#?}");
    }
    assert!(idf::id_problems(&pkg).unwrap().is_empty());
    let template = out
        .archives
        .iter()
        .find(|a| a.file == "hud_template.erp")
        .unwrap();
    let t = Pkg::parse(
        &Erp::parse(&template.bytes)
            .unwrap()
            .find(dials::TEMPLATE)
            .unwrap()
            .fragments[0]
            .data()
            .unwrap(),
    )
    .unwrap();
    for node in ["v_r00", "v_r02_v01", "t_r02_pos", "t_r02_v01_code", "t_lap"] {
        assert!(
            find_path(&t.root, &format!("{DAMAGE_PARENT}.{pfx}_{node}")).is_ok(),
            "{DAMAGE_PARENT}.{pfx}_{node}"
        );
    }
}
