//! The HUD composer inside a profile build: two addons' `operations.hud` records, the ADDONS MFD page and
//! another addon's whole-resource replacement of the template must all end up in one template. Needs
//! `F1_25_DIR`; the stock files are read through the launcher's baseline, never the Halo-modified folder.

use std::path::{Path, PathBuf};

use swofty_addon::asset::{AssetOp, Root};
use swofty_addon::ops::ErpOp;
use swofty_core::baseline::Identity;
use swofty_core::build::{GVDB, build};
use swofty_core::hud::{self, GameStock, Stock, dials};
use swofty_core::install::Game;
use swofty_core::store::Store;
use swofty_core::ui::node::{NodeExt, ObjExt, clone_node, edit, find_path, root_obj};
use swofty_formats::erp::{Erp, ErpFile};
use swofty_formats::gvdb::Gvdb;
use swofty_formats::pkg::Pkg;

const FLOW_RACE: &str = "2025_asset_groups/ui_package/flow_race.erp";
const MARKER: &str = "planked_brake_panel";

fn write(path: &Path, body: impl AsRef<[u8]>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn png(path: &Path, w: u32, h: u32) {
    write(path, png_bytes(w, h));
}

fn png_bytes(w: u32, h: u32) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let data: Vec<u8> = [255u8; 4]
            .iter()
            .copied()
            .cycle()
            .take((w * h * 4) as usize)
            .collect();
        enc.write_header().unwrap().write_image_data(&data).unwrap();
    }
    out
}

fn record() -> serde_json::Value {
    serde_json::json!({
        "dial": "f1_26",
        "sprites": {
            "base": { "png": "art/base.png", "bounds": [-60, -20, 60, 20] },
            "lit": { "png": "art/lit.png", "bounds": [-50, -10, -34, -2] }
        },
        "elements": [
            { "kind": "sprite", "id": "base", "sprite": "base", "layer": 20 },
            { "kind": "text", "id": "speed", "style": "OSD_24_bold", "align": "centre", "box": 40,
              "cap_height": 9.5, "at": [0, -6], "value": { "game": "speed_str" } },
            { "kind": "sprite", "id": "boost", "sprite": "lit",
              "visible": { "view_model": "view_model_data.hud_t_boost" } }
        ]
    })
}

struct Fixture {
    root: PathBuf,
    store: Store,
    game: Game,
    _pin: swofty_core::builtin::LoaderPin,
}

impl Fixture {
    fn new(name: &str, real: &Path) -> Self {
        let root = std::env::temp_dir().join(format!("swofty-hud-build-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        write(&dir.join("F1_25.exe"), "exe");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let originals = GameStock::with_baseline(real, &Store::open_default().root).unwrap();
        let ui = |rel: &str| format!("2025_asset_groups/ui_package/{rel}");
        let mut rels: Vec<String> = swofty_core::ui::ERPS
            .iter()
            .chain(swofty_core::ui::READS)
            .chain(["flow_race.erp"].iter())
            .map(|r| ui(r))
            .collect();
        rels.extend(dials::SCENE_ERPS.iter().map(|s| (*s).to_owned()));
        rels.sort();
        rels.dedup();
        for rel in rels {
            let to = dir.join(&rel);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            #[cfg(unix)]
            std::os::unix::fs::symlink(originals.original(&rel), &to).unwrap();
            #[cfg(not(unix))]
            std::fs::copy(originals.original(&rel), &to).unwrap();
        }
        let loader = root.join("loader");
        write(&loader.join(swofty_core::install::PROXY), "proxy");
        write(&loader.join("swofty/addons/core/swofty_core.dll"), "core");
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        Self {
            store: Store::new(root.join("data")),
            root,
            game,
            _pin: swofty_core::builtin::LoaderPin::to(loader),
        }
    }

    fn install(&self, id: &str, extra: serde_json::Value, files: Vec<(&str, Vec<u8>)>) {
        let src = self.root.join("src").join(id);
        let mut manifest = serde_json::json!({
            "schema_version": 1, "id": id, "name": id, "version": "1.0.0", "author": "t",
            "compat": { "game": "f1_25" }
        });
        for (k, v) in extra.as_object().unwrap() {
            manifest[k] = v.clone();
        }
        write(&src.join("swofty.json"), manifest.to_string());
        for (rel, data) in files {
            write(&src.join(rel), data);
        }
        self.store.install_addon(&src).unwrap();
        self.store
            .update_profile("default", |p| p.set_enabled(id, true))
            .unwrap();
    }

    fn hud_addon(&self, id: &str) {
        let hud = serde_json::json!({ "hud.element": { "hud": record() } });
        self.install(
            id,
            serde_json::json!({ "operations": { "hud": ["hud.json"] } }),
            vec![
                ("hud.json", hud.to_string().into_bytes()),
                ("art/base.png", png_bytes(64, 28)),
                ("art/lit.png", png_bytes(16, 8)),
            ],
        );
    }

    fn mfd_addon(&self) {
        self.install(
            "c.mfd",
            serde_json::json!({
                "options": [{ "id": "livery", "type": "toggle", "label": "Livery", "default": false, "binds": "native" }],
                "hud": { "mfd": { "title": "LIVERY", "rows": ["livery"] } }
            }),
            vec![],
        );
    }

    fn planked(&self, real: &Path) {
        let originals = GameStock::with_baseline(real, &Store::open_default().root).unwrap();
        let stock = ErpFile::open(&originals.original(FLOW_RACE)).unwrap();
        let res = stock.erp.find(dials::TEMPLATE).unwrap();
        let bytes = stock.load(res).unwrap().fragments[0].data().unwrap();
        let replaced = edit(&bytes, |p| {
            let list = root_obj(p)?.at_mut("Templates")?.a_mut()?;
            let first = list.iter().find(|t| t.is_node()).unwrap().clone();
            list.push(clone_node(&first, "", MARKER)?);
            Ok(())
        })
        .unwrap();
        let ops = serde_json::json!({ "schema_version": 1, "ops": [
            { "op": "erp.replace-resource", "erp": FLOW_RACE, "resource": dials::TEMPLATE, "source": "template.ui" }
        ] });
        self.install(
            "d.planked",
            serde_json::json!({ "operations": { "assets": ["ops.json"] } }),
            vec![
                ("ops.json", ops.to_string().into_bytes()),
                ("template.ui", replaced),
            ],
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn real() -> Option<PathBuf> {
    match std::env::var_os("F1_25_DIR") {
        Some(d) => Some(PathBuf::from(d)),
        None => {
            eprintln!("F1_25_DIR is not set; skipping");
            None
        }
    }
}

fn template_of(archive: &Path) -> Pkg {
    let file = ErpFile::open(archive).unwrap();
    let res = file.erp.find(dials::TEMPLATE).unwrap();
    Pkg::parse(&file.load(res).unwrap().fragments[0].data().unwrap()).unwrap()
}

fn has(t: &Pkg, path: &str) -> bool {
    find_path(&t.root, path).is_ok()
}

const PANEL: &str = "hud_mfd_dynamic.mfd.attacher.swofty_addons_panel";

#[test]
fn two_hud_addons_the_mfd_page_and_a_replaced_template_compose_into_one() {
    let Some(game) = real() else { return };
    let f = Fixture::new("all", &game);
    f.hud_addon("one.hud");
    f.hud_addon("two.hud");
    f.mfd_addon();
    f.planked(&game);
    let built = build(&f.store, "default", &f.game).unwrap();
    let parent = dials::F126.race_parent;

    let cached = f.store.addon_cache_dir("one.hud").join("hud/hud_template.erp");
    for (what, t) in [
        ("the composed overlay archive", template_of(&cached)),
        (
            "the built flow_race.erp",
            template_of(&built.plan.data[FLOW_RACE]),
        ),
    ] {
        assert!(has(&t, &format!("{parent}.one_hud_ctl")), "{what}: one.hud");
        assert!(has(&t, &format!("{parent}.two_hud_ctl")), "{what}: two.hud");
        assert!(has(&t, PANEL), "{what}: the MFD page");
        assert!(
            t.root.as_obj().is_some() && find_path(&t.root, MARKER).is_ok(),
            "{what}: the replaced template's own node"
        );
    }
    assert!(
        built
            .lines
            .iter()
            .any(|l| l.contains("d.planked") && l.contains("on top of that change")),
        "{:#?}",
        built.lines
    );

    let scene =
        Erp::parse(&std::fs::read(f.store.addon_cache_dir("one.hud").join("hud/hud_scene.erp")).unwrap())
            .unwrap();
    assert!(scene.resources.iter().any(|r| r.name.contains("one_hud_base")));
    assert!(scene.resources.iter().any(|r| r.name.contains("two_hud_base")));
}

#[test]
fn the_overlay_ops_point_at_the_composed_archives() {
    let Some(game) = real() else { return };
    let stock = GameStock::with_baseline(&game, &Store::open_default().root).unwrap();
    let dir = std::env::temp_dir().join(format!("swofty-hud-ops-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    png(&dir.join("art/base.png"), 64, 28);
    png(&dir.join("art/lit.png"), 16, 8);
    let input = hud::Input {
        addon: "one.hud".into(),
        key: "hud".into(),
        dir: dir.clone(),
        record: hud::model::HudElement::parse(&record()).unwrap(),
    };
    let out = hud::compose(&stock, &[input]).unwrap();
    let ops = out.ops("hud");
    assert_eq!(ops.len(), out.archives.len());
    for (op, archive) in ops.iter().zip(&out.archives) {
        let ErpOp::Overlay { erps, source } = op else {
            panic!("{op:?}")
        };
        assert_eq!(source.root, Root::Cache);
        assert_eq!(source.path, format!("hud/{}", archive.file));
        assert_eq!(erps, &archive.erps);
    }
    let _ = AssetOp::Erp(ops[0].clone());
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_compose_error_stops_the_build_with_the_reason() {
    let Some(game) = real() else { return };
    let f = Fixture::new("error", &game);
    let mut bad = record();
    bad["elements"][1]["value"] = serde_json::json!({ "game": "no_such_value" });
    let hud = serde_json::json!({ "hud.element": { "hud": bad } });
    f.install(
        "bad.hud",
        serde_json::json!({ "operations": { "hud": ["hud.json"] } }),
        vec![
            ("hud.json", hud.to_string().into_bytes()),
            ("art/base.png", png_bytes(64, 28)),
            ("art/lit.png", png_bytes(16, 8)),
        ],
    );
    let err = build(&f.store, "default", &f.game).unwrap_err().to_string();
    assert!(err.contains("the HUD elements do not compose"), "{err}");
    assert!(err.contains("no_such_value"), "{err}");
}

const DAMAGE_SCENE: &str = "eaid://ui_package/art/osd/components/dial/mfd_car_damage_panel.idf";
const DAMAGE_PARENT: &str = "hud_mfd_dynamic.mfd.attacher.car_damage_panel";

fn damage_record() -> serde_json::Value {
    serde_json::json!({
        "scene": DAMAGE_SCENE,
        "parent_ui": DAMAGE_PARENT,
        "elements": [
            { "kind": "text", "id": "fl", "style": "OSD_17_bold", "align": "left", "box": 30,
              "cap_height": 6.0, "at": [-60, 40],
              "value": { "view_model": "ui_menu_data.hud_brake_check_fl" },
              "visible": { "view_model": "ui_menu_data.hud_brake_check_visible" } },
            { "kind": "text", "id": "speed", "cap_height": 6.0, "at": [0, 50],
              "value": { "game": "speed_str" } }
        ]
    })
}

fn damage_input(record: &serde_json::Value) -> hud::Input {
    hud::Input {
        addon: "a.panel".into(),
        key: "readouts".into(),
        dir: std::env::temp_dir(),
        record: hud::model::HudElement::parse(record).unwrap(),
    }
}

#[test]
fn a_scene_target_adds_readouts_to_the_stock_damage_panel() {
    let Some(game) = real() else { return };
    let stock = GameStock::with_baseline(&game, &Store::open_default().root).unwrap();
    let out = hud::compose(&stock, &[damage_input(&damage_record())]).unwrap();
    let files: Vec<&str> = out.archives.iter().map(|a| a.file.as_str()).collect();
    assert_eq!(files, ["hud_target_scene.erp", "hud_template.erp"], "{files:?}");
    let pfx = "a_panel_readouts";

    let scene_archive = out
        .archives
        .iter()
        .find(|a| a.file == "hud_target_scene.erp")
        .unwrap();
    let erp = Erp::parse(&scene_archive.bytes).unwrap();
    let scene = Pkg::parse(&erp.find(DAMAGE_SCENE).unwrap().fragments[0].data().unwrap()).unwrap();
    let entry = &swofty_core::ui::idf::scenes(&scene).unwrap()[0];
    let names: Vec<String> = swofty_core::ui::idf::elements(entry.o().unwrap().at("scene").unwrap())
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    for n in [
        pfx.to_owned(),
        format!("{pfx}.{pfx}_fl_n"),
        format!("{pfx}.{pfx}_fl_n.{pfx}_fl"),
        format!("{pfx}.{pfx}_speed_n.{pfx}_speed"),
    ] {
        assert!(names.contains(&n), "{n} in {names:?}");
    }
    assert!(
        names.iter().any(|n| n == "boundless.brake_check_fl")
            || names.iter().any(|n| n.starts_with("boundless"))
    );
    assert!(swofty_core::ui::idf::id_problems(&scene).unwrap().is_empty());

    let template = out
        .archives
        .iter()
        .find(|a| a.file == "hud_template.erp")
        .unwrap();
    let erp = Erp::parse(&template.bytes).unwrap();
    let t = Pkg::parse(&erp.find(dials::TEMPLATE).unwrap().fragments[0].data().unwrap()).unwrap();
    let at = find_path(&t.root, &format!("{DAMAGE_PARENT}.{pfx}_t_fl")).unwrap();
    let label = swofty_core::ui::node::node_at(&t.root, &at).unwrap();
    assert_eq!(label.type_code(), "UILB");
    assert!(
        swofty_core::ui::node::canonical(label).contains("#ui_menu_data.hud_brake_check_fl"),
        "the label binds the leaf"
    );
    assert!(find_path(&t.root, &format!("{DAMAGE_PARENT}.{pfx}_t_speed")).is_ok());
    assert!(
        !has(&t, &format!("{DAMAGE_PARENT}.{pfx}_ctl")),
        "no ctl group on a scene target"
    );

    let race = template_of(&stock.original(FLOW_RACE));
    assert!(
        !has(&race, &format!("{DAMAGE_PARENT}.{pfx}_t_fl")),
        "the stock template is untouched"
    );
}

#[test]
fn a_scene_target_is_held_to_the_stock_scene_the_parent_and_the_binding_types() {
    let Some(game) = real() else { return };
    let stock = GameStock::with_baseline(&game, &Store::open_default().root).unwrap();
    let compose = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut rec = damage_record();
        edit(&mut rec);
        hud::compose(&stock, &[damage_input(&rec)])
            .map(|_| ())
            .unwrap_err()
            .to_string()
    };
    let e = compose(&|r| r["parent_ui"] = "hud_mfd_dynamic.mfd.attacher".into());
    assert!(e.contains("instances") && e.contains("not eaid://"), "{e}");
    let e = compose(&|r| r["parent_ui"] = "hud_mfd_dynamic.mfd.no_such_node".into());
    assert!(e.contains("no_such_node"), "{e}");
    let e = compose(&|r| {
        r["scene"] = "eaid://ui_package/art/osd/components/dial/no_such_scene.idf".into();
    });
    assert!(e.contains("no_such_scene"), "{e}");
    let e = compose(&|r| r["elements"][1]["value"] = serde_json::json!({ "game": "revs" }));
    assert!(e.contains("revs"), "{e}");
    let e = compose(&|r| r["elements"][1]["value"] = serde_json::json!({ "game": "ers_mode" }));
    assert!(e.contains("ers_mode"), "{e}");
}

#[test]
fn a_scene_target_record_is_checked_when_it_is_read() {
    let parse = |edit: &dyn Fn(&mut serde_json::Value)| {
        let mut rec = damage_record();
        edit(&mut rec);
        hud::model::HudElement::parse(&rec).unwrap_err().to_string()
    };
    assert!(parse(&|r| r["dial"] = "f2".into()).contains("not two"));
    assert!(
        parse(&|r| {
            r.as_object_mut().unwrap().remove("parent_ui");
        })
        .contains("go together")
    );
    assert!(parse(&|r| r["elements"][0]["kind"] = "sprite".into()).contains("text, group"));
    assert!(parse(&|r| r["sprites"] = serde_json::json!({ "a": { "png": "a.png" } })).contains("sprites"));
    assert!(parse(&|r| r["scene"] = "dial_om_xm.idf".into()).contains("eaid://"));
    let neither = hud::model::HudElement::parse(&serde_json::json!({ "elements": [] }))
        .unwrap_err()
        .to_string();
    assert!(neither.contains("dial"), "{neither}");
}

fn with_marker(scene: &[u8]) -> Vec<u8> {
    use swofty_core::ui::idf;
    let mut p = Pkg::parse(scene).unwrap();
    let entry = &mut idf::scenes_mut(&mut p).unwrap()[0];
    let root = entry.o_mut().unwrap().at_mut("scene").unwrap();
    let mut marker = idf::find(root, "point_top").unwrap().clone();
    idf::rename(&mut marker, "planked_marker").unwrap();
    idf::recompute_ids(&mut marker, "planked_marker").unwrap();
    idf::children_mut(root).unwrap().push(marker);
    idf::relookup(entry).unwrap();
    p.to_bytes().unwrap()
}

#[test]
fn a_scene_target_grows_the_scene_another_addon_replaced() {
    let Some(game) = real() else { return };
    let f = Fixture::new("scene-target", &game);
    let stock = GameStock::with_baseline(&game, &Store::open_default().root).unwrap();
    let scene = ErpFile::open(&stock.original(FLOW_RACE)).unwrap();
    let bytes = scene
        .load(scene.erp.find(DAMAGE_SCENE).unwrap())
        .unwrap()
        .fragments[0]
        .data()
        .unwrap();
    let ops = serde_json::json!({ "schema_version": 1, "ops": [
        { "op": "erp.replace-resource", "erp": FLOW_RACE, "resource": DAMAGE_SCENE, "source": "damage.idf" }
    ] });
    f.install(
        "d.damage",
        serde_json::json!({ "operations": { "assets": ["ops.json"] } }),
        vec![
            ("ops.json", ops.to_string().into_bytes()),
            ("damage.idf", with_marker(&bytes)),
        ],
    );
    let hud = serde_json::json!({ "hud.element": { "readouts": damage_record() } });
    f.install(
        "c.readouts",
        serde_json::json!({ "operations": { "hud": ["hud.json"] } }),
        vec![("hud.json", hud.to_string().into_bytes())],
    );
    let built = build(&f.store, "default", &f.game).unwrap();
    let cached = f
        .store
        .addon_cache_dir("c.readouts")
        .join("hud/hud_target_scene.erp");
    let erp = Erp::parse(&std::fs::read(cached).unwrap()).unwrap();
    let grown = Pkg::parse(&erp.find(DAMAGE_SCENE).unwrap().fragments[0].data().unwrap()).unwrap();
    let entry = &swofty_core::ui::idf::scenes(&grown).unwrap()[0];
    let names: Vec<String> = swofty_core::ui::idf::elements(entry.o().unwrap().at("scene").unwrap())
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    assert!(
        names.contains(&"planked_marker".to_owned()),
        "the other addon's element stays: {names:?}"
    );
    assert!(names.contains(&"c_readouts".to_owned()), "{names:?}");
    assert!(
        built
            .lines
            .iter()
            .any(|l| l.contains("d.damage") && l.contains("on top of that change")),
        "{:#?}",
        built.lines
    );
}

fn signals_of(entry: &swofty_formats::pkg::Node) -> Vec<(String, String)> {
    use swofty_formats::pkg::Node;
    let text = |o: &Node, key: &str| -> String {
        o.as_obj()
            .and_then(|o| o.get(key))
            .and_then(Node::as_val)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned()
    };
    entry
        .as_obj()
        .and_then(|o| o.get("signals"))
        .and_then(Node::as_arr)
        .map(|a| {
            a.items
                .iter()
                .map(|s| (text(s, "to_object_name"), text(s, "to_pin_name")))
                .collect()
        })
        .unwrap_or_default()
}

fn command_signals(n: &swofty_formats::pkg::Node, name: &str) -> Vec<(String, String)> {
    use swofty_formats::pkg::Node;
    n.slot(swofty_core::ui::node::COMMANDS)
        .unwrap()
        .iter()
        .find(|c| {
            c.as_obj()
                .and_then(|o| o.get("commandName"))
                .and_then(Node::as_val)
                .and_then(|v| v.as_str())
                == Some(name)
        })
        .map(signals_of)
        .unwrap_or_else(|| panic!("{} has no {name} command", n.name()))
}

/// The stock fact behind a decision's `initial`: a dial's `show` triggers the group that holds the
/// stock MVCD on `drs_pit_state`, and that trigger registers and queries the MVCD, so the current
/// state fires again on every show, but only once the value has one. The composer's `<prefix>_ctl`
/// is wired the same way, so decision targets are re-synced on show and `initial` covers the frames
/// before the value first exists.
#[test]
fn the_dials_show_registers_and_queries_the_stock_drs_decision() {
    let Some(game) = real() else { return };
    let stock = GameStock::with_baseline(&game, &Store::open_default().root).unwrap();
    let t = template_of(&stock.original(FLOW_RACE));
    let mut paths = Vec::new();
    swofty_core::ui::node::node_paths_of(&t.root, "", &mut paths);
    for d in [hud::model::Dial::F125, hud::model::Dial::F2] {
        let parent = dials::facts(d).race_parent;
        let mvcds: Vec<&String> = paths
            .iter()
            .filter(|(p, n)| {
                p.starts_with(&format!("{parent}.")) && n.type_code() == "MVCD" && {
                    n.props()
                        .ok()
                        .and_then(|p| p.get("dbEntry"))
                        .and_then(|v| v.as_obj())
                        .and_then(|o| o.get("value"))
                        .and_then(|v| v.as_val())
                        .and_then(|v| v.as_str())
                        == Some("drs_pit_state")
                }
            })
            .map(|(p, _)| p)
            .collect();
        let [mvcd] = mvcds[..] else {
            panic!("{parent}: {} MVCDs on drs_pit_state", mvcds.len())
        };
        let owner = mvcd.rsplit_once('.').unwrap().0.to_owned();
        let (_, f1hd) = paths.iter().find(|(p, _)| *p == parent).unwrap();
        let show = command_signals(f1hd, "show");
        assert!(
            show.contains(&(owner.clone(), "trigger".to_owned())),
            "{parent}: show -> {show:?}"
        );
        let hide = command_signals(f1hd, "hide");
        assert!(
            hide.contains(&(owner.clone(), "untrigger".to_owned())),
            "{hide:?}"
        );
        let (_, grp) = paths.iter().find(|(p, _)| *p == owner).unwrap();
        let trigger = command_signals(grp, "trigger");
        assert!(
            trigger.contains(&(mvcd.clone(), "register".to_owned())),
            "{trigger:?}"
        );
        assert!(
            trigger.contains(&(mvcd.clone(), "query".to_owned())),
            "{trigger:?}"
        );
    }
}

/// A record whose f1_26 base has a plate, a bar and a speed text; the f1_25 and f2 variants add a
/// two-state pill switched by a decision of their own, place their adds in the draw order and the
/// f2 one sets the root y.
fn pill_record() -> serde_json::Value {
    let variant = |dial: &str| {
        serde_json::json!({
            "dial": dial,
            "remove": ["bar_b"],
            "add": [
                { "kind": "sprite", "id": "plate", "sprite": "bar", "before": "base" },
                { "kind": "sprite", "id": "bar_a", "sprite": "bar", "at": [0, 4], "replace": "bar_a" },
                { "kind": "group", "id": "pill", "after": "base", "children": [
                    { "kind": "sprite", "id": "pill_idle", "sprite": "lit" },
                    { "kind": "sprite", "id": "pill_live", "sprite": "lit" } ] }
            ],
            "decisions": [
                { "id": "drs", "type": "mvcd", "game": "drs_pit_state", "initial": "invalid",
                  "states": { "invalid": ["pill_idle"], "drs_live": ["pill_live"] } }
            ]
        })
    };
    let mut f2 = variant("f2");
    f2["root_y"] = 16.into();
    serde_json::json!({
        "dial": "f1_26",
        "sprites": {
            "base": { "png": "art/base.png", "bounds": [-60, -20, 60, 20] },
            "bar": { "png": "art/lit.png", "bounds": [-40, 0, 40, 8] },
            "lit": { "png": "art/lit.png", "bounds": [-8, -8, 8, 8] }
        },
        "elements": [
            { "kind": "sprite", "id": "base", "sprite": "base", "layer": 20 },
            { "kind": "group", "id": "energy", "children": [
                { "kind": "sprite", "id": "bar_a", "sprite": "bar" },
                { "kind": "sprite", "id": "bar_b", "sprite": "bar", "at": [0, 10] } ] },
            { "kind": "text", "id": "speed", "style": "OSD_24_bold", "align": "centre", "box": 40,
              "cap_height": 9.5, "at": [0, -6], "value": { "game": "speed_str" } }
        ],
        "variants": { "f1_25": variant("f1_25"), "f2": f2 }
    })
}

fn bool_value(n: &swofty_formats::pkg::Node, prop: &str) -> Option<bool> {
    use swofty_formats::pkg::Value;
    let v = n.props().ok()?.get(prop)?.as_obj()?.get("value")?.as_val()?;
    match &v.value {
        Value::Raw(b) => Some(b.first() == Some(&1)),
        _ => None,
    }
}

#[test]
fn a_variants_decision_placed_adds_and_root_y_compose_for_its_dial_only() {
    let Some(game) = real() else { return };
    let stock = GameStock::with_baseline(&game, &Store::open_default().root).unwrap();
    let dir = std::env::temp_dir().join(format!("swofty-hud-pill-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    png(&dir.join("art/base.png"), 64, 28);
    png(&dir.join("art/lit.png"), 16, 8);
    let input = hud::Input {
        addon: "one.pill".into(),
        key: "dial".into(),
        dir: dir.clone(),
        record: hud::model::HudElement::parse(&pill_record()).unwrap(),
    };
    let out = hud::compose(&stock, &[input]).unwrap();
    let pfx = "one_pill_dial";
    let scene = Erp::parse(
        &out.archives
            .iter()
            .find(|a| a.file == "hud_scene.erp")
            .unwrap()
            .bytes,
    )
    .unwrap();
    let tmpl = Erp::parse(
        &out.archives
            .iter()
            .find(|a| a.file == "hud_template.erp")
            .unwrap()
            .bytes,
    )
    .unwrap();
    let t = Pkg::parse(&tmpl.find(dials::TEMPLATE).unwrap().fragments[0].data().unwrap()).unwrap();
    let k = hud::geom::K;
    for dial in hud::model::Dial::ALL {
        let f = dials::facts(dial);
        let p = Pkg::parse(&scene.find(f.scene).unwrap().fragments[0].data().unwrap()).unwrap();
        let root = swofty_core::ui::idf::scenes(&p).unwrap()[0]
            .o()
            .unwrap()
            .at("scene")
            .unwrap();
        let ours = swofty_core::ui::idf::find(root, pfx).unwrap();
        let kids: Vec<String> = ours
            .o()
            .unwrap()
            .at("children")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .map(|c| swofty_core::ui::idf::elem_name(c).to_owned())
            .collect();
        let y = swofty_core::ui::idf::position(ours).unwrap()[1];
        let ctl = format!("{}.{pfx}_ctl", f.race_parent);
        let drs = find_path(&t.root, &format!("{ctl}.d_drs"));
        if dial == hud::model::Dial::F126 {
            assert_eq!(
                kids,
                [
                    format!("{pfx}_base"),
                    format!("{pfx}_energy"),
                    format!("{pfx}_speed_n")
                ],
                "{}",
                f.scene
            );
            assert!(drs.is_err(), "the base dial has no pill decision");
            assert!(
                (y - (f.bottom_y + k * 20.0)).abs() < 1e-4,
                "{}: root y {y}",
                f.scene
            );
            continue;
        }
        assert_eq!(
            kids,
            [
                format!("{pfx}_plate"),
                format!("{pfx}_base"),
                format!("{pfx}_pill"),
                format!("{pfx}_energy"),
                format!("{pfx}_speed_n")
            ],
            "{}",
            f.scene
        );
        let energy = swofty_core::ui::idf::find(root, &format!("{pfx}_energy")).unwrap();
        let bars: Vec<String> = energy
            .o()
            .unwrap()
            .at("children")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .map(|c| swofty_core::ui::idf::elem_name(c).to_owned())
            .collect();
        assert_eq!(bars, [format!("{pfx}_bar_a")], "{}", f.scene);
        if dial == hud::model::Dial::F2 {
            let want = -k * 16.0;
            assert_eq!(
                y as f32, want as f32,
                "{}: an explicit root y lands exactly",
                f.scene
            );
        } else {
            let want = f.bottom_y + k * 20.0;
            assert!((y - want).abs() < 1e-4, "{}: root y {y}, want {want}", f.scene);
        }
        let node = swofty_core::ui::node::node_at(&t.root, &drs.unwrap()).unwrap();
        assert_eq!(node.type_code(), "MVCD", "{ctl}");
        let idle = find_path(&t.root, &format!("{ctl}.v_pill_idle")).unwrap();
        let live = find_path(&t.root, &format!("{ctl}.v_pill_live")).unwrap();
        assert_eq!(
            bool_value(swofty_core::ui::node::node_at(&t.root, &idle).unwrap(), "visible"),
            Some(true),
            "{ctl}: the initial state's element starts shown"
        );
        assert_eq!(
            bool_value(swofty_core::ui::node::node_at(&t.root, &live).unwrap(), "visible"),
            Some(false),
            "{ctl}: the other state's element starts hidden"
        );
        assert!(
            swofty_core::ui::idf::id_problems(&p).unwrap().is_empty(),
            "{}",
            f.scene
        );
    }
    std::fs::remove_dir_all(dir).ok();
}

/// A record that keeps the 2026 dial's stock parts and hides the f2 dial's behind its variant's
/// switch: the f2 template binds every `v_stock_<part>` to NOT the switch, the f1_26 dial has no
/// stock node and its parts sit where the stock scene has them.
#[test]
fn a_variants_own_stock_mode_hides_its_dials_parts_behind_the_switch_only() {
    let Some(game) = real() else { return };
    let stock = GameStock::with_baseline(&game, &Store::open_default().root).unwrap();
    let dir = std::env::temp_dir().join(format!("swofty-hud-vstock-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    png(&dir.join("art/base.png"), 64, 28);
    png(&dir.join("art/lit.png"), 16, 8);
    let leaf = "view_model_data.hud_vs_on";
    let mut rec = record();
    rec["stock"] = "keep".into();
    rec["variants"] = serde_json::json!({
        "f2": { "dial": "f2", "stock": "hide", "switch": { "view_model": leaf, "default": false } }
    });
    let input = hud::Input {
        addon: "one.vstock".into(),
        key: "dial".into(),
        dir: dir.clone(),
        record: hud::model::HudElement::parse(&rec).unwrap(),
    };
    let out = hud::compose(&stock, &[input]).unwrap();
    let pfx = "one_vstock_dial";
    let archive =
        |file: &str| Erp::parse(&out.archives.iter().find(|a| a.file == file).unwrap().bytes).unwrap();
    let scene = archive("hud_scene.erp");
    let tmpl = archive("hud_template.erp");
    let t = Pkg::parse(&tmpl.find(dials::TEMPLATE).unwrap().fragments[0].data().unwrap()).unwrap();
    let visible = |path: &str| {
        let n = swofty_core::ui::node::node_at(&t.root, &find_path(&t.root, path).unwrap()).unwrap();
        n.props().unwrap().get("visible").unwrap().clone()
    };
    let text =
        |n: Option<&swofty_formats::pkg::Node>| n.and_then(|v| v.as_val()).map(|v| format!("{:?}", v.value));
    let part_y = |dial: hud::model::Dial, stock_scene: bool, part: &str| {
        let f = dials::facts(dial);
        let bytes = if stock_scene {
            stock.read(dials::STOCK_ERP, f.scene).unwrap().remove(0)
        } else {
            scene.find(f.scene).unwrap().fragments[0].data().unwrap()
        };
        let p = Pkg::parse(&bytes).unwrap();
        let root = swofty_core::ui::idf::scenes(&p).unwrap()[0]
            .o()
            .unwrap()
            .at("scene")
            .unwrap();
        swofty_core::ui::idf::position(swofty_core::ui::idf::find(root, part).unwrap())
    };

    let f2 = dials::facts(hud::model::Dial::F2);
    let ctl = format!("{}.{pfx}_ctl", f2.race_parent);
    let root = visible(&format!("{ctl}.v_{pfx}"));
    let root = root.as_obj().unwrap();
    assert!(
        text(root.get("dbentry")).unwrap().contains("hud_vs_on"),
        "{root:?}"
    );
    assert!(
        root.get("dbentrynegate").is_none(),
        "the root shows while the switch is on"
    );
    for part in dials::STOCK_KIDS {
        let v = visible(&format!("{ctl}.v_stock_{part}"));
        let v = v.as_obj().unwrap();
        assert!(
            text(v.get("dbentry")).unwrap().contains("hud_vs_on"),
            "{part}: {v:?}"
        );
        assert!(
            v.get("dbentrynegate").is_some(),
            "{part}: shown while the switch is off"
        );
        assert_eq!(
            part_y(hud::model::Dial::F2, false, part),
            part_y(hud::model::Dial::F2, true, part),
            "{part}: hidden, not parked"
        );
    }

    let f126 = dials::facts(hud::model::Dial::F126);
    let ctl = format!("{}.{pfx}_ctl", f126.race_parent);
    assert!(find_path(&t.root, &ctl).is_ok(), "{ctl}");
    for part in dials::STOCK_KIDS {
        assert!(
            find_path(&t.root, &format!("{ctl}.v_stock_{part}")).is_err(),
            "{part}: the f1_26 base keeps its stock part"
        );
        assert_eq!(
            part_y(hud::model::Dial::F126, false, part),
            part_y(hud::model::Dial::F126, true, part),
            "{part}: the f1_26 base does not park"
        );
    }
    std::fs::remove_dir_all(dir).ok();
}

/// A dial record that carries the switch itself (schema 1.4): the f1_25 template binds its root to
/// the leaf with the switch's `default`, every `v_stock_<part>` to NOT the leaf with the inverse,
/// and a variant without a switch of its own inherits the record's.
#[test]
fn a_record_level_switch_scopes_the_dial_and_gives_the_stock_parts_back_by_default() {
    let Some(game) = real() else { return };
    let stock = GameStock::with_baseline(&game, &Store::open_default().root).unwrap();
    let dir = std::env::temp_dir().join(format!("swofty-hud-rswitch-{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    png(&dir.join("art/base.png"), 64, 28);
    png(&dir.join("art/lit.png"), 16, 8);
    let leaf = "view_model_data.hud_rs_on";
    let mut rec = record();
    rec["dial"] = "f1_25".into();
    rec["stock"] = "hide".into();
    rec["switch"] = serde_json::json!({ "view_model": leaf, "default": false });
    rec["variants"] = serde_json::json!({ "f2": { "dial": "f2" } });
    let input = hud::Input {
        addon: "one.rswitch".into(),
        key: "dial".into(),
        dir: dir.clone(),
        record: hud::model::HudElement::parse(&rec).unwrap(),
    };
    let out = hud::compose(&stock, &[input]).unwrap();
    let pfx = "one_rswitch_dial";
    let tmpl = Erp::parse(
        &out.archives
            .iter()
            .find(|a| a.file == "hud_template.erp")
            .unwrap()
            .bytes,
    )
    .unwrap();
    let t = Pkg::parse(&tmpl.find(dials::TEMPLATE).unwrap().fragments[0].data().unwrap()).unwrap();
    let visible = |path: &str| {
        let n = swofty_core::ui::node::node_at(&t.root, &find_path(&t.root, path).unwrap()).unwrap();
        n.props().unwrap().get("visible").unwrap().clone()
    };
    let text =
        |n: Option<&swofty_formats::pkg::Node>| n.and_then(|v| v.as_val()).map(|v| format!("{:?}", v.value));
    let flag = |n: Option<&swofty_formats::pkg::Node>| {
        n.and_then(|v| v.as_val())
            .map(|v| format!("{:?}", v.value).contains("[1]"))
    };

    for dial in [hud::model::Dial::F125, hud::model::Dial::F2] {
        let f = dials::facts(dial);
        let ctl = format!("{}.{pfx}_ctl", f.race_parent);
        let root = visible(&format!("{ctl}.v_{pfx}"));
        let root = root.as_obj().unwrap();
        assert!(
            text(root.get("dbentry")).unwrap().contains("hud_rs_on"),
            "{dial:?}: {root:?}"
        );
        assert!(
            root.get("dbentrynegate").is_none(),
            "{dial:?}: the root shows while the leaf is on"
        );
        assert!(
            flag(root.get("value")) == Some(false),
            "{dial:?}: the root starts hidden with the switch default false: {root:?}"
        );
        for part in dials::STOCK_KIDS {
            let v = visible(&format!("{ctl}.v_stock_{part}"));
            let v = v.as_obj().unwrap();
            assert!(
                text(v.get("dbentry")).unwrap().contains("hud_rs_on"),
                "{dial:?} {part}: {v:?}"
            );
            assert!(
                v.get("dbentrynegate").is_some(),
                "{dial:?} {part}: shown while the leaf is off"
            );
            assert!(
                flag(v.get("value")) == Some(true),
                "{dial:?} {part}: the stock part starts shown: {v:?}"
            );
        }
    }
    std::fs::remove_dir_all(dir).ok();
}
