//! The `templates/hud-tower` example: its record parses and expands within the caps on any host,
//! and (with `F1_25_DIR`, through the launcher's baseline originals) composes into the host cap, the
//! three screen instances, their control groups, the lap counter wraps, the static hides, the clock
//! and the editor preview.

use std::collections::BTreeSet;
use std::path::PathBuf;

use swofty_core::hud::{self, GameStock, Stock, dials, model, screens, stock_edit};
use swofty_core::store::Store;
use swofty_core::ui::idf;
use swofty_core::ui::node::{self, NodeExt, ObjExt, canonical, find_path, node_at};
use swofty_formats::erp::Erp;
use swofty_formats::pkg::{Node, Pkg};

const PFX: &str = "example_hud_tower_tower";

fn template_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../templates/hud-tower")
}

fn record() -> (String, model::HudElement) {
    let text = std::fs::read_to_string(template_dir().join("registry/tower@1.1.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let recs = v["hud.element"].as_object().unwrap();
    let (key, body) = recs.iter().next().unwrap();
    (key.clone(), model::HudElement::parse(body).unwrap())
}

fn real() -> Option<GameStock> {
    match std::env::var_os("F1_25_DIR") {
        Some(d) => Some(GameStock::with_baseline(PathBuf::from(d), &Store::open_default().root).unwrap()),
        None => {
            eprintln!("F1_25_DIR is not set; skipping");
            None
        }
    }
}

fn count(list: &[model::Element]) -> usize {
    list.iter().map(|e| 1 + count(&e.children)).sum()
}

#[test]
fn the_template_record_parses_and_expands_within_the_caps() {
    let (key, rec) = record();
    assert_eq!(key, "tower");
    assert_eq!(rec.screen.as_ref().and_then(|s| s.as_str()), Some("standings"));
    assert_eq!(rec.also.len(), 2);
    assert_eq!(rec.also["replay"].hide, ["trial"]);
    let live = model::expand(&rec.elements, None).unwrap();
    let rows = &live[0].children[2].children;
    assert_eq!(rows.len(), 30);
    assert_eq!(rows[29].id, "r29");
    let cars: Vec<&model::Element> = rows[29]
        .children
        .iter()
        .filter(|c| c.id.starts_with("r29_v"))
        .collect();
    assert_eq!(cars.len(), 30);
    assert_eq!(cars[29].id, "r29_v29");
    let code = cars[29].children[1].value.as_ref().unwrap();
    assert_eq!(code.slot, Some(29));
    assert!(count(&live) < 10_000, "{}", count(&live));
    let preview = model::expand(&rec.elements, Some(5)).unwrap();
    assert_eq!(preview[0].children[2].children.len(), 5);
    let lapc = &rec.stock_nodes[0];
    assert_eq!(lapc.hide.len(), 6);
    assert_eq!(lapc.wrap, ["edge_base", "edge_border", "lap_bg", "lockup"]);
    assert_eq!(
        rec.decisions[0].on.as_ref().unwrap().path.as_deref(),
        Some("#view_model_data.hud.content.lap_times.pill_0.active")
    );
    let clock = rec.clock.as_ref().unwrap();
    assert_eq!(clock.output.as_deref(), Some("view_model_data.mytower_clock"));
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(template_dir().join("swofty.json")).unwrap()).unwrap();
    assert_eq!(manifest["operations"]["hud"][0], "registry/tower@1.1.json");
    for s in rec.sprites.values() {
        if !s.png.is_empty() {
            assert!(template_dir().join(&s.png).is_file(), "{}", s.png);
        }
    }
}

fn pkg_of(archive: &hud::Built, name: &str) -> Pkg {
    Pkg::parse(
        &Erp::parse(&archive.bytes)
            .unwrap()
            .find(name)
            .unwrap_or_else(|e| panic!("{name} in {}: {e}", archive.file))
            .fragments[0]
            .data()
            .unwrap(),
    )
    .unwrap()
}

fn elements(pkg: &Pkg) -> Vec<String> {
    let entry = &idf::scenes(pkg).unwrap()[0];
    idf::elements(entry.o().unwrap().at("scene").unwrap())
        .into_iter()
        .map(|(p, _)| p)
        .collect()
}

fn at<'a>(root: &'a Node, path: &str) -> &'a Node {
    let a = find_path(root, path).unwrap_or_else(|e| panic!("{path}: {e}"));
    node_at(root, &a).unwrap()
}

fn kid_names(n: &Node) -> Vec<String> {
    n.slot(node::CHILDREN)
        .unwrap()
        .iter()
        .filter(|c| c.is_node())
        .map(|c| c.name().to_owned())
        .collect()
}

#[test]
fn the_template_composes_through_the_baseline_originals() {
    let Some(stock) = real() else { return };
    let (key, rec) = record();
    let input = hud::Input {
        addon: "example.hud-tower".into(),
        key,
        dir: template_dir(),
        record: rec,
    };
    let out = hud::compose(&stock, &[input]).unwrap();
    let files: BTreeSet<&str> = out.archives.iter().map(|a| a.file.as_str()).collect();
    for f in [
        "hud_screen_scene.erp",
        "hud_template.erp",
        "hud_spectator.erp",
        "hud_replay.erp",
        "hud_replay_scene.erp",
    ] {
        assert!(files.contains(f), "{f} in {files:?}");
    }
    let scene = out
        .archives
        .iter()
        .find(|a| a.file == "hud_screen_scene.erp")
        .unwrap();

    // the host cap: the live root hidden, the preview root beside it, 30 rows of 30 cars
    let cap = pkg_of(scene, screens::CAP);
    let names = elements(&cap);
    for n in [
        format!(
            "{PFX}.{PFX}_stack.{PFX}_rows.{PFX}_r29.{PFX}_r29_v29.{PFX}_r29_v29_code_n.{PFX}_r29_v29_code"
        ),
        format!("{PFX}.{PFX}_stack.{PFX}_rows.{PFX}_r00.{PFX}_r00_lg"),
        format!("{PFX}.{PFX}_stack.{PFX}_top.{PFX}_logo"),
        format!("{PFX}.{PFX}_stack.{PFX}_top.{PFX}_fia"),
        format!("{PFX}.{PFX}_stack.{PFX}_trial"),
        format!("{PFX}_pv.{PFX}_pv_stack.{PFX}_pv_rows.{PFX}_pv_r04"),
    ] {
        assert!(names.contains(&n), "{n}");
    }
    assert!(!names.iter().any(|n| n.ends_with("_pv_r05")));
    assert!(idf::id_problems(&cap).unwrap().is_empty());

    // the standings instance and the lap counter with its four wrappers
    let stacked = pkg_of(scene, screens::STACKED);
    assert!(elements(&stacked).contains(&format!("{PFX}_host")));
    let lapc = pkg_of(scene, screens::LAPC_SCENE);
    let lc = elements(&lapc);
    for w in ["edge_base", "edge_border", "lap_bg", "lockup"] {
        let wrapper = stock_edit::wrap_name(PFX, w);
        assert!(lc.contains(&wrapper), "{wrapper} in {lc:#?}");
        assert!(lc.contains(&format!("{wrapper}.{w}")), "{w} inside its wrapper");
    }
    assert!(
        !lc.contains(&"lap_bg".to_owned()),
        "lap_bg is no longer top level"
    );
    assert!(lc.contains(&format!(
        "{}.lap_bg.lap_bg_overlay",
        stock_edit::wrap_name(PFX, "lap_bg")
    )));
    assert!(lc.contains(&"holder_standard.centre_lap_value".to_owned()));
    assert!(idf::id_problems(&lapc).unwrap().is_empty());
    let stock_lc = Pkg::parse(
        &stock
            .read(dials::STOCK_ERP, screens::LAPC_SCENE)
            .unwrap()
            .remove(0),
    )
    .unwrap();
    assert_eq!(elements(&stock_lc).len() + 4, lc.len());

    // the HUD template: the control group, its hides, clock, decision and the stock-node groups
    let template = out
        .archives
        .iter()
        .find(|a| a.file == "hud_template.erp")
        .unwrap();
    let t = pkg_of(template, dials::TEMPLATE);
    let ctl = format!("{}.{PFX}_ctl", screens::STANDINGS_PARENT);
    let grp = at(&t.root, &ctl);
    let kids = kid_names(grp);
    assert_eq!(kids[0], "mask_0");
    assert!(kids.contains(&"host_n".to_owned()));
    assert!(kids.contains(&"clock_fmt".to_owned()));
    assert!(kids.contains(&"mv_clock".to_owned()));
    assert!(kids.contains(&"d_tt".to_owned()));
    let host = at(&t.root, &format!("{ctl}.host_n"));
    let inner = kid_names(host);
    for n in [
        "v_cap",
        "v_pv",
        "root_n",
        "v_r29",
        "v_r29_v29",
        "t_r29_v29_code",
        "c_r29_v29_bar",
        "x_r29_lg",
        "t_laps",
        "t_clk",
        "c_r00_pb",
        "c_sc_t",
        "v_notes",
        "v_trial",
    ] {
        assert!(inner.contains(&n.to_owned()), "{n} in host_n");
    }
    assert!(
        !inner.contains(&"h_trial".to_owned()),
        "the standings keep the trial line to its decision"
    );
    let code = canonical(at(&t.root, &format!("{ctl}.host_n.t_r29_v29_code")));
    assert!(code.contains("#session.cars.car29.driver.name_code"), "{code}");
    let mut laps = Vec::new();
    node::bindings(at(&t.root, &format!("{ctl}.host_n.t_laps")), &[], &mut laps);
    assert_eq!(laps.len(), 1);
    assert_eq!(
        (laps[0].db.as_str(), laps[0].tag),
        ("#session.number_of_laps", Some(*b"si32"))
    );
    let clk = canonical(at(&t.root, &format!("{ctl}.host_n.t_clk")));
    assert!(clk.contains("#view_model_data.mytower_clock"), "{clk}");
    let fmt = canonical(at(&t.root, &format!("{ctl}.clock_fmt")));
    assert!(
        fmt.contains("#view_model_data.mytower_clock") && fmt.contains("#session.time_remaining"),
        "{fmt}"
    );
    let mv = canonical(at(&t.root, &format!("{ctl}.mv_clock")));
    assert!(
        mv.contains("#view_model_data.mytower_timed") && mv.contains("formatTime"),
        "{mv}"
    );
    let pb = canonical(at(&t.root, &format!("{ctl}.host_n.c_r00_pb")));
    assert!(pb.contains("colourOverride") && !pb.contains("dbentry"), "{pb}");
    let notes = canonical(at(&t.root, &format!("{ctl}.host_n.v_notes")));
    assert!(notes.contains("visible") && !notes.contains("dbentry"), "{notes}");
    let d = canonical(at(&t.root, &format!("{ctl}.d_tt")));
    assert!(
        d.contains("#view_model_data.hud.content.lap_times.pill_0.active") && d.contains("v_trial"),
        "{d}"
    );
    let trig = canonical(grp);
    assert!(trig.contains("formatTime") && trig.contains("register"), "{trig}");
    let sn0 = at(&t.root, &format!("{}.{PFX}_sn0", screens::LAPC_PARENT));
    let sn0_kids = kid_names(sn0);
    assert_eq!(sn0_kids.len(), 10, "{sn0_kids:?}");
    assert!(
        sn0_kids.contains(&"v_w_lap_bg".to_owned()) && sn0_kids.contains(&"v_centre_lap_value".to_owned())
    );
    let w = canonical(at(
        &t.root,
        &format!("{}.{PFX}_sn0.v_w_lap_bg", screens::LAPC_PARENT),
    ));
    assert!(
        w.contains(&stock_edit::wrap_name(PFX, "lap_bg")) && w.contains("#view_model_data.mytower_show"),
        "{w}"
    );
    let sn1 = at(
        &t.root,
        &format!(
            "hud_left_notification_dynamic.osd_visibility_option.left_notification.left_message.{PFX}_sn1"
        ),
    );
    assert_eq!(kid_names(sn1), ["v_boundless"]);

    // the replay and spectator copies: the trial line hidden for good, no decision
    let replay = out.archives.iter().find(|a| a.file == "hud_replay.erp").unwrap();
    let r = pkg_of(replay, screens::REPLAY_UI);
    let rctl = format!("{}.{PFX}_ctl", screens::REPLAY_ROOT);
    let rk = kid_names(at(&r.root, &rctl));
    assert!(!rk.contains(&"d_tt".to_owned()));
    let rh = kid_names(at(&r.root, &format!("{rctl}.host_n")));
    assert!(
        rh.contains(&"h_trial".to_owned()) && rh.contains(&"v_trial".to_owned()),
        "{rh:?}"
    );
    let h = canonical(at(&r.root, &format!("{rctl}.host_n.h_trial")));
    assert!(
        h.contains(&format!("{PFX}_trial")) && !h.contains("dbentry"),
        "{h}"
    );
    let rs = pkg_of(replay, screens::REPLAY_SCENE);
    assert!(elements(&rs).iter().any(|n| n.ends_with(&format!("{PFX}_host"))));
    let spectator = out
        .archives
        .iter()
        .find(|a| a.file == "hud_spectator.erp")
        .unwrap();
    let s = pkg_of(spectator, screens::SPECT_UI);
    let sh = kid_names(at(
        &s.root,
        &format!("{}.{PFX}_ctl.host_n", screens::SPECT_PARENT),
    ));
    assert!(sh.contains(&"h_trial".to_owned()));

    // the editor copy's .uipre names the host and the preview texts
    let uipre = Erp::parse(&scene.bytes)
        .unwrap()
        .find(screens::STACKED_UIPRE)
        .is_ok();
    assert!(uipre);
    assert!(
        out.lines.iter().any(|l| l.contains("the HUD editor shows")),
        "{:#?}",
        out.lines
    );
    assert!(
        out.lines
            .iter()
            .any(|l| l.contains("4 through a new wrapper null")),
        "{:#?}",
        out.lines
    );
}
