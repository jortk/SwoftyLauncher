//! A group with a slide and a group with a fade compose into a dial: the scene holds the holder
//! nulls and the slide's clip, the template the `UIXA` and `UIMO` players and the `MVCD` triggers,
//! typed as the stock HUD template types them, and the editor copy stays static. The curve names
//! the composer accepts are ones the stock UI templates play. Needs `F1_25_DIR`; the stock files
//! are read through the launcher's baseline, never the game folder.

use std::collections::BTreeSet;
use std::path::PathBuf;

use swofty_core::hud::{self, GameStock, Stock, dials, motion};
use swofty_core::store::Store;
use swofty_core::ui::idf;
use swofty_core::ui::node::{self, NodeExt, ObjExt, find_path, node_at};
use swofty_formats::animclip;
use swofty_formats::erp::Erp;
use swofty_formats::pkg::Pkg;

const FADE_SCREEN: &str = "eaid://ui_package/screens/garage_monitor_container.ui";
const FADE_ERP: &str = "2025_asset_groups/ui_package/flow_car_monitor.erp";

fn real() -> Option<GameStock> {
    match std::env::var_os("F1_25_DIR") {
        Some(d) => Some(GameStock::with_baseline(PathBuf::from(d), &Store::open_default().root).unwrap()),
        None => {
            eprintln!("F1_25_DIR is not set; skipping");
            None
        }
    }
}

fn record() -> serde_json::Value {
    serde_json::json!({
        "dial": "f1_26", "box": [0, 0, 120, 80],
        "elements": [
            { "kind": "group", "id": "rows", "at": [10, 20],
              "motion": { "id": "slide", "keys": [[0, [0, 0]], [0.75, [0, -12.98]]], "curve": "generic_fade_on",
                          "play_on": { "view_model": "view_model_data.hud_t_swap" } },
              "children": [
                { "kind": "text", "id": "pos", "style": "OSD_19_regular", "cap_height": 5.9,
                  "value": { "view_model": "view_model_data.hud_t_pos" } } ] },
            { "kind": "group", "id": "up", "at": [40, 20], "visible": { "view_model": "view_model_data.hud_t_up" },
              "motion": { "id": "in", "keys": [[0, 0], [0.15, 1]], "curve": "generic_fade_on_slow",
                          "play_on": { "view_model": "view_model_data.hud_t_up" } },
              "children": [
                { "kind": "text", "id": "arrow", "style": "OSD_19_regular", "cap_height": 5.9, "literal": "^" } ] }
        ],
        "editor": { "preview": { "view_model_data.hud_t_pos": "10", "view_model_data.hud_t_up": true } }
    })
}

fn template_of(out: &hud::Output, file: &str, name: &str) -> Pkg {
    let a = out.archives.iter().find(|a| a.file == file).unwrap();
    Pkg::parse(
        &Erp::parse(&a.bytes).unwrap().find(name).unwrap().fragments[0]
            .data()
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn the_stock_templates_play_the_curves_the_composer_accepts() {
    let Some(stock) = real() else { return };
    let bytes = stock.read(FADE_ERP, FADE_SCREEN).unwrap().remove(0);
    let screen = Pkg::parse(&bytes).unwrap();
    let played = motion::curves_in(&screen.root);
    for c in ["generic_fade_on", "generic_fade_off", "generic_fade_on_slow"] {
        assert!(played.contains(c), "{FADE_SCREEN} plays {c}: {played:?}");
        assert!(motion::curve_known(c));
    }
    let hud_bytes = stock.read(dials::STOCK_ERP, dials::TEMPLATE).unwrap().remove(0);
    let hud_root = Pkg::parse(&hud_bytes).unwrap().root;
    let mut paths = Vec::new();
    node::node_paths_of(&hud_root, "", &mut paths);
    let alpha: BTreeSet<String> = paths
        .iter()
        .filter(|(_, n)| n.type_code() == "UIMO")
        .map(|(p, _)| p.clone())
        .collect();
    assert!(
        alpha.iter().any(|p| p.ends_with("show_mfd_background")),
        "{alpha:?}"
    );
    let on_bool: Vec<&(String, &swofty_formats::pkg::Node)> = paths
        .iter()
        .filter(|(p, n)| n.type_code() == "MVCD" && p.ends_with("lead_entry.show_penalty_group"))
        .collect();
    assert_eq!(
        on_bool.len(),
        1,
        "the stock MVCD on a bool the triggers are modelled on"
    );
    let events: Vec<String> = on_bool[0]
        .1
        .slot(node::EVENTS)
        .unwrap()
        .iter()
        .filter_map(|e| {
            e.as_obj()?
                .get("eventName")?
                .as_val()?
                .as_str()
                .map(str::to_owned)
        })
        .collect();
    assert_eq!(events, ["1", "0"]);
}

#[test]
fn a_slide_and_a_fade_compose_into_a_dial_and_the_preview_stays_static() {
    let Some(stock) = real() else { return };
    let input = hud::Input {
        addon: "a.motion".into(),
        key: "motion".into(),
        dir: std::env::temp_dir(),
        record: hud::model::HudElement::parse(&record()).unwrap(),
    };
    let out = hud::compose(&stock, &[input]).unwrap();
    for l in &out.lines {
        eprintln!("{l}");
    }
    let pfx = "a_motion";
    let archive = out.archives.iter().find(|a| a.file == "hud_scene.erp").unwrap();
    let erp = Erp::parse(&archive.bytes).unwrap();
    let scene = Pkg::parse(&erp.find(dials::F126.scene).unwrap().fragments[0].data().unwrap()).unwrap();
    let entry = &idf::scenes(&scene).unwrap()[0];
    let root = entry.o().unwrap().at("scene").unwrap();
    let names: Vec<String> = idf::elements(root).into_iter().map(|(p, _)| p).collect();
    let base = format!("{}.{pfx}", dials::CUSTOMISATION);
    let slide_holder = format!("zh_{pfx}_rows_slide");
    let fade_holder = format!("zh_{pfx}_up_in");
    for n in [
        format!("{base}.{pfx}_rows.{slide_holder}.{pfx}_pos_n"),
        format!("{base}.{pfx}_up.{fade_holder}.{pfx}_arrow_n"),
    ] {
        assert!(names.contains(&n), "{n} in {names:#?}");
    }
    let holder = idf::find(root, &slide_holder).unwrap();
    let anims = holder
        .as_obj()
        .and_then(|o| o.get("anims"))
        .and_then(swofty_formats::pkg::Node::as_arr)
        .map(|a| a.items.len())
        .unwrap_or(0);
    assert_eq!(anims, 1, "the slide's holder carries its clip");
    assert!(idf::id_problems(&scene).unwrap().is_empty());

    let clip_name = format!("{}/{slide_holder}?context=animation", dials::F126.scene);
    let clip = erp.find(&clip_name).unwrap();
    let read = animclip::read_clip(
        &clip.fragments[0].data().unwrap(),
        &clip.fragments[1].data().unwrap(),
    )
    .unwrap();
    assert_eq!(read.node, slide_holder);
    assert_eq!(read.channels[0].times, vec![0.0, 0.75]);
    let strings =
        animclip::read_strings(&erp.find(dials::F126.strtab).unwrap().fragments[0].data().unwrap()).unwrap();
    assert_eq!(strings[read.name_index as usize], slide_holder);
    assert!(
        erp.find(&format!("{}/{fade_holder}?context=animation", dials::F126.scene))
            .is_err(),
        "a fade writes no clip"
    );

    let f125 = Pkg::parse(&erp.find(dials::F125.scene).unwrap().fragments[0].data().unwrap()).unwrap();
    let pv_entry = &idf::scenes(&f125).unwrap()[0];
    let pv_names: Vec<String> = idf::elements(pv_entry.o().unwrap().at("scene").unwrap())
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    assert!(pv_names.contains(&format!(
        "{}.{pfx}_pv.{pfx}_pv_up.{pfx}_pv_arrow_n",
        dials::CUSTOMISATION
    )));
    assert!(
        !pv_names.iter().any(|n| n.contains("zh_")),
        "the editor copy has no motion holders: {pv_names:#?}"
    );

    let t = template_of(&out, "hud_template.erp", dials::TEMPLATE);
    let ctl = format!("{}.{pfx}_ctl", dials::F126.race_parent);
    let typed = |name: &str, ty: &str| {
        let at = find_path(&t.root, &format!("{ctl}.{name}")).unwrap_or_else(|_| panic!("{ctl}.{name}"));
        let n = node_at(&t.root, &at).unwrap();
        assert_eq!(n.type_code(), ty, "{name}");
        n.clone()
    };
    typed("m_rows_slide", "UIXA");
    let on = typed("m_up_in_on", "UIMO");
    let off = typed("m_up_in_off", "UIMO");
    let canon_on = node::canonical(&on);
    assert!(canon_on.contains("generic_fade_on_slow") && canon_on.contains("Multiplicative"));
    assert!(node::canonical(&off).contains("StartingValue"));
    let slide = typed("mv_rows_slide", "MVCD");
    let s = node::canonical(&slide);
    assert!(s.contains("#view_model_data.hud_t_swap") && s.contains("playToFraction"));
    assert!(s.contains(&format!("{ctl}.m_rows_slide")));
    let fade = typed("mv_up_in", "MVCD");
    let f = node::canonical(&fade);
    assert!(f.contains("#view_model_data.hud_t_up") && f.contains("activateOverride"));
    let grp = node_at(&t.root, &find_path(&t.root, &ctl).unwrap()).unwrap();
    let commands = node::canonical(grp);
    assert!(
        commands.contains(&format!("{ctl}.mv_rows_slide")) && commands.contains("register"),
        "the control group registers the triggers"
    );
    let pv_ctl = format!("{}.{pfx}_pv_ctl", dials::F125.race_parent);
    let pv = node_at(&t.root, &find_path(&t.root, &pv_ctl).unwrap()).unwrap();
    let pv_text = node::canonical(pv);
    assert!(
        !pv_text.contains("UIXA") && !pv_text.contains("UIMO") && !pv_text.contains("MVCD"),
        "the preview control has no players or triggers"
    );
}

#[test]
fn a_turn_composes_into_a_dial_with_one_event_per_step() {
    let Some(stock) = real() else { return };
    let record = serde_json::json!({
        "dial": "f1_26", "box": [-40, -40, 40, 40],
        "elements": [
            { "kind": "group", "id": "needle", "at": [0, 0],
              "motion": { "id": "rpm", "kind": "turn", "keys": [[0, 225], [0.4, -45]], "curve": "default_curve_1",
                          "play_to": { "view_model": "view_model_data.hud_t_rpm" }, "steps": 9 },
              "children": [
                { "kind": "text", "id": "tip", "style": "OSD_19_regular", "cap_height": 5.9, "at": [20, 0],
                  "literal": "-" } ] }
        ]
    });
    let input = hud::Input {
        addon: "a.needle".into(),
        key: "needle".into(),
        dir: std::env::temp_dir(),
        record: hud::model::HudElement::parse(&record).unwrap(),
    };
    let out = hud::compose(&stock, &[input]).unwrap();
    for l in &out.lines {
        eprintln!("{l}");
    }
    assert!(hud::check::constant_rotations(&out.archives).unwrap().is_empty());
    assert!(hud::check::unpadded_clips(&out.archives).unwrap().is_empty());
    let pfx = "a_needle";
    let holder = format!("zh_{pfx}_needle_rpm");
    let archive = out.archives.iter().find(|a| a.file == "hud_scene.erp").unwrap();
    let erp = Erp::parse(&archive.bytes).unwrap();
    let scene = Pkg::parse(&erp.find(dials::F126.scene).unwrap().fragments[0].data().unwrap()).unwrap();
    let root = idf::scenes(&scene).unwrap()[0].o().unwrap().at("scene").unwrap();
    let names: Vec<String> = idf::elements(root).into_iter().map(|(p, _)| p).collect();
    let tip = format!("{}.{pfx}.{pfx}_needle.{holder}.{pfx}_tip_n", dials::CUSTOMISATION);
    assert!(names.contains(&tip), "{tip} in {names:#?}");
    let clip = erp
        .find(&format!("{}/{holder}?context=animation", dials::F126.scene))
        .unwrap();
    let (temp, syst) = (
        clip.fragments[0].data().unwrap(),
        clip.fragments[1].data().unwrap(),
    );
    assert!(syst.len().is_multiple_of(animclip::SYST_ALIGN));
    assert!(!animclip::has_constant_rotation(&temp, &syst).unwrap());
    let read = animclip::read_clip(&temp, &syst).unwrap();
    assert_eq!(read.node, holder);
    assert_eq!(read.channels.len(), 1);
    assert_eq!(read.channels[0].target, animclip::Target::Rotation);
    assert_eq!(
        read.channels[0].times.len(),
        4,
        "270 degrees in three spans of 90"
    );

    let t = template_of(&out, "hud_template.erp", dials::TEMPLATE);
    let ctl = format!("{}.{pfx}_ctl", dials::F126.race_parent);
    let at = |name: &str| node_at(&t.root, &find_path(&t.root, &format!("{ctl}.{name}")).unwrap()).unwrap();
    assert_eq!(at("m_needle_rpm").type_code(), "UIXA");
    let mv = at("mv_needle_rpm");
    assert_eq!(mv.type_code(), "MVCD");
    let events: Vec<String> = mv
        .slot(node::EVENTS)
        .unwrap()
        .iter()
        .filter_map(|e| {
            e.as_obj()?
                .get("eventName")?
                .as_val()?
                .as_str()
                .map(str::to_owned)
        })
        .collect();
    assert_eq!(events, (0..9).map(|i| i.to_string()).collect::<Vec<_>>());
    let canon = node::canonical(mv);
    assert!(canon.contains("#view_model_data.hud_t_rpm") && canon.contains("playToFraction"));
}
