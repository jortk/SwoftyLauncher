use std::path::{Path, PathBuf};

use swofty_core::settings::SessionSettings;
use swofty_formats::erp::Erp;

fn dirs() -> Option<(PathBuf, PathBuf)> {
    let game = std::env::var_os("F1_25_DIR").map(PathBuf::from);
    let oracle = std::env::var_os("SWOFTY_MENU_ORACLE").map(PathBuf::from);
    match (game, oracle) {
        (Some(g), Some(o)) => Some((g, o)),
        _ => {
            eprintln!("F1_25_DIR and SWOFTY_MENU_ORACLE are not both set; skipping");
            None
        }
    }
}

fn session() -> SessionSettings {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/menu_session.json");
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn first_difference(a: &[u8], b: &[u8]) -> String {
    let at = a
        .iter()
        .zip(b)
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len()));
    format!("sizes {} and {}, first difference at {at:#x}", a.len(), b.len())
}

const POPUP: [&str; 2] = [
    "eaid://ui_package/screens/persistent.ui",
    "eaid://ui_package/message_data/onlineservices_messages.uimsg",
];

fn resource_report(ours: &Path, theirs: &Path, skip: &[&str]) -> String {
    let (a, b) = (
        Erp::parse(&std::fs::read(ours).unwrap()).unwrap(),
        Erp::parse(&std::fs::read(theirs).unwrap()).unwrap(),
    );
    let mut out = Vec::new();
    for (x, y) in a.resources.iter().zip(&b.resources) {
        if x.name != y.name {
            out.push(format!("order: {} vs {}", x.name, y.name));
            continue;
        }
        if skip.contains(&x.name.as_str()) {
            continue;
        }
        for (i, (fx, fy)) in x.fragments.iter().zip(&y.fragments).enumerate() {
            if fx.raw != fy.raw {
                let (dx, dy) = (fx.data().unwrap(), fy.data().unwrap());
                let what = if dx == dy {
                    "same data, different compression".to_owned()
                } else {
                    first_difference(&dx, &dy)
                };
                out.push(format!("{} fragment {i}: {what}", x.name));
            }
        }
    }
    if a.resources.len() != b.resources.len() {
        out.push(format!(
            "{} vs {} resources",
            a.resources.len(),
            b.resources.len()
        ));
    }
    out.join("\n")
}

#[test]
fn menus_match_the_prototype_byte_for_byte() {
    let Some((game, oracle)) = dirs() else { return };
    let out = std::env::temp_dir().join(format!("swofty-menus-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let built = swofty_core::ui::build(&session(), &|rel| game.join(rel), &|_, _| None, &out, true).unwrap();
    let mut bad = Vec::new();
    for (rel, path) in &built.erps {
        if swofty_core::ui::era::ARCHIVES.iter().any(|a| rel.ends_with(a)) {
            continue;
        }
        let theirs = oracle.join(rel);
        if std::fs::read(path).unwrap() != std::fs::read(&theirs).unwrap() {
            let skip: &[&str] = if rel.ends_with("/flow_persistent.erp") {
                &POPUP
            } else {
                &[]
            };
            let report = resource_report(path, &theirs, skip);
            if !report.is_empty() {
                bad.push(format!("{rel}:\n{report}"));
            }
        }
    }
    for (rel, path) in &built.screens {
        let ours = std::fs::read(path).unwrap();
        let theirs = std::fs::read(oracle.join("native_ui").join(rel)).unwrap();
        if ours != theirs {
            bad.push(format!("{rel}: {}", first_difference(&ours, &theirs)));
        }
    }
    let ui: serde_json::Value =
        serde_json::from_slice(&std::fs::read(oracle.join("native_ui/swofty/ui.json")).unwrap()).unwrap();
    let want: Vec<(String, bool)> = ui["ui"]["screens"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["name"].as_str().unwrap().to_owned(),
                s["view_model"].as_bool().unwrap_or(false),
            )
        })
        .collect();
    let got: Vec<(String, bool)> = built
        .session
        .iter()
        .map(|s| (s.name.clone(), s.view_model))
        .collect();
    assert_eq!(got, want);
    for s in &built.session {
        assert_eq!(
            (s.flow.as_str(), s.anchor.as_deref()),
            ("flow_options", Some("settings_container"))
        );
    }
    std::fs::remove_dir_all(&out).ok();
    assert!(bad.is_empty(), "differs from the prototype:\n{}", bad.join("\n"));
}

#[test]
fn menus_and_a_map_patch_one_databases_erp() {
    use swofty_core::activation::place::track_group;
    use swofty_core::activation::ui::{MapUi, build_ui};
    use swofty_core::build::{Built, build_menus};
    use swofty_core::map::TrackPackage;
    use swofty_formats::pkg::Pkg;

    let Some(game) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let Some(dir) = std::env::var_os("SWOFTY_MAPS_DIR")
        .map(PathBuf::from)
        .map(|d| d.join("bahrain24"))
        .filter(|d| d.join("manifest.json").is_file())
    else {
        eprintln!("SWOFTY_MAPS_DIR has no bahrain24 package; skipping");
        return;
    };
    let pkg = TrackPackage::parse(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("swofty-menus-map-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let resolve = |rel: &str| game.join(rel);
    let maps = [MapUi {
        pkg: &pkg,
        dir: &dir,
        asset_group: track_group("bahrain24", "bahrain24"),
    }];
    let map_ui = build_ui(&maps, &resolve, &out.join("cache")).unwrap();
    let rel = "2025_asset_groups/ui_package/databases.erp";
    assert!(map_ui.replaced.contains_key(rel));
    let mut built = Built {
        overlay: out.join("overlay"),
        ..Built::default()
    };
    built.plan.data.extend(map_ui.replaced.clone());
    let mut session = session();
    build_menus(&mut built, &mut session, &resolve, &|_, _| None, true).unwrap();
    assert_eq!(built.plan.data[rel], out.join("overlay").join(rel));
    for (r, src) in &map_ui.replaced {
        if !swofty_core::ui::ERPS.iter().any(|e| r.ends_with(e)) {
            assert_eq!(&built.plan.data[r], src, "{r} keeps the map build");
        }
    }

    let data = |path: &Path, name: &str| {
        let res = Erp::read_matching(path, |n| n == name).unwrap();
        res[0].fragments[0].data().unwrap()
    };
    let (ours, maps_only, stock) = (&built.plan.data[rel], &map_ui.replaced[rel], game.join(rel));
    for name in [
        "eaid://ui_package/databases/overrides.uidb",
        "eaid://ui_package/databases/minimap.uidb",
    ] {
        assert_eq!(
            data(ours, name),
            data(maps_only, name),
            "{name} lost the map edits"
        );
        assert_ne!(data(ours, name), data(&stock, name), "{name} has no map edits");
    }
    for (r, maps_only) in &map_ui.replaced {
        let Some(ours) = built.plan.data.get(r).filter(|o| *o != maps_only) else {
            continue;
        };
        let stock = Erp::parse(&std::fs::read(game.join(r)).unwrap()).unwrap();
        for res in Erp::parse(&std::fs::read(maps_only).unwrap()).unwrap().resources {
            let theirs = res.content_hash().unwrap();
            let edited = stock.find(&res.name).ok().map(|s| s.hash) != Some(theirs);
            if edited {
                assert_eq!(
                    data(ours, &res.name),
                    data(maps_only, &res.name),
                    "{r}: {} lost the map edits",
                    res.name
                );
            }
        }
    }
    let has = |bytes: Vec<u8>, path: &[&str]| {
        let p = Pkg::parse(&bytes).unwrap();
        let mut n = &p.root;
        for k in path {
            match n.as_obj().and_then(|o| o.get(k)) {
                Some(x) => n = x,
                None => return false,
            }
        }
        true
    };
    let ui_data = "eaid://ui_package/databases/ui_data.uidb";
    assert!(has(data(ours, ui_data), &["ui_menu_data", "Contents", "swofty"]));
    assert!(!has(
        data(maps_only, ui_data),
        &["ui_menu_data", "Contents", "swofty"]
    ));
    let tips = "eaid://ui_package/databases/tool_tips.uidb";
    for t in ["mod_menu", "mod_menu_offline", "swofty_adjust"] {
        let path = ["tool_tips", "Contents", "tooltips_dictionary", "Contents", t];
        assert!(has(data(ours, tips), &path));
    }
    std::fs::remove_dir_all(&out).ok();
}

#[test]
fn stock_dialogs_keep_their_text() {
    use swofty_core::ui::check::stock_kept;
    use swofty_core::ui::patch::{POPUP_BODY, POPUP_TITLE};
    use swofty_formats::pkg::Pkg;

    let Some(game) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let out = std::env::temp_dir().join(format!("swofty-menus-dialogs-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let built = swofty_core::ui::build(&session(), &|rel| game.join(rel), &|_, _| None, &out, true).unwrap();
    let rel = "2025_asset_groups/ui_package/flow_persistent.erp";
    let report = resource_report(&built.erps[rel], &game.join(rel), &POPUP);
    assert!(report.is_empty(), "{rel} changed outside the popup:\n{report}");

    let data = |path: &Path, name: &str| {
        let res = Erp::read_matching(path, |n| n == name).unwrap();
        res[0].fragments[0].data().unwrap()
    };
    let (stock, ours) = (game.join(rel), &built.erps[rel]);
    let changed = stock_kept(&data(&stock, POPUP[0]), &data(ours, POPUP[0]), "swofty_").unwrap();
    assert!(
        changed.is_empty(),
        "the stock popup changed:\n{}",
        changed.join("\n")
    );

    let messages = |path: &Path| -> Vec<(String, String, String)> {
        let p = Pkg::parse(&data(path, POPUP[1])).unwrap();
        let text = |m: &swofty_formats::pkg::Node, k: &str| {
            m.as_obj()
                .and_then(|o| o.get(k))
                .and_then(|v| v.as_val())
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned()
        };
        p.root
            .as_obj()
            .unwrap()
            .get("messages")
            .unwrap()
            .as_arr()
            .unwrap()
            .items
            .iter()
            .map(|m| (text(m, "id"), text(m, "title"), text(m, "messageBody")))
            .collect()
    };
    let (before, after) = (messages(&stock), messages(ours));
    assert_eq!(before.len(), after.len());
    let mut ours_only = 0;
    for (b, a) in before.iter().zip(&after) {
        if a.0.starts_with("online_services_generic_exception_") {
            assert_eq!((a.1.as_str(), a.2.as_str()), (POPUP_TITLE, POPUP_BODY), "{}", a.0);
            ours_only += 1;
        } else {
            assert_eq!(a, b, "{} lost its stock text", a.0);
        }
    }
    assert_eq!(ours_only, 4);
    let keys: Vec<&str> = built.strings.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        keys,
        [POPUP_TITLE, POPUP_BODY, swofty_core::ui::patch::FRIENDS_TAB_LABEL]
    );
    assert!(built.strings.iter().all(|(_, v)| !v.is_empty()));
    std::fs::remove_dir_all(&out).ok();
}

fn stock(game: &Path, rel: &str) -> PathBuf {
    let path = game.join(rel);
    let Some(base) = std::env::var_os("SWOFTY_BASELINE").map(PathBuf::from) else {
        return path;
    };
    let listed: serde_json::Value =
        serde_json::from_slice(&std::fs::read(base.join("baseline.json")).unwrap()).unwrap();
    match listed["files"][rel]["sha256"].as_str() {
        Some(sha) if swofty_core::baseline::sha256_file(&path).unwrap() != sha => {
            base.parent().unwrap().join("objects").join(&sha[..2]).join(sha)
        }
        _ => path,
    }
}

fn fragment(path: &Path, name: &str, index: usize) -> Vec<u8> {
    let res = Erp::read_matching(path, |n| n == name).unwrap();
    res[0].fragments[index].data().unwrap()
}

fn signals(e: &swofty_formats::pkg::Obj) -> Vec<(String, String)> {
    use swofty_core::ui::node::{NodeExt, ObjExt};
    e.at("signals")
        .unwrap()
        .a()
        .unwrap()
        .iter()
        .map(|s| {
            let s = s.o().unwrap();
            (
                s.str_at("to_object_name").unwrap().to_owned(),
                s.str_at("to_pin_name").unwrap().to_owned(),
            )
        })
        .collect()
}

fn event<'a>(
    n: &'a swofty_formats::pkg::Node,
    slot: usize,
    key: &str,
    name: &str,
) -> &'a swofty_formats::pkg::Obj {
    use swofty_core::ui::node::{NodeExt, ObjExt};
    n.slot(slot)
        .unwrap()
        .iter()
        .filter_map(|e| e.as_obj())
        .find(|e| e.str_at(key).ok() == Some(name))
        .unwrap_or_else(|| panic!("{}: no {name}", n.name()))
}

#[test]
fn the_core_build_adds_the_era_rows() {
    use swofty_core::ui::era;
    use swofty_core::ui::idf;
    use swofty_core::ui::node::{CHILDREN, COMMANDS, EVENTS, LAYOUT, NodeExt, ObjExt};
    use swofty_formats::pkg::{Node, Pkg};

    let Some(game) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let out = std::env::temp_dir().join(format!("swofty-menus-era-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let built =
        swofty_core::ui::build(&session(), &|rel| stock(&game, rel), &|_, _| None, &out, true).unwrap();
    assert!(
        built
            .lines
            .iter()
            .any(|l| l.contains("era picker gets Built In and Addons rows in 2 archive(s)")),
        "{:?}",
        built.lines
    );
    let world = stock(&game, &format!("{}/{}", swofty_core::ui::UI, era::ARCHIVES[0]));
    let title = fragment(&world, era::TITLE_SCENE, 0);
    let pair = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    let t = |row: &str| format!("dynamic_era_select.{row}");
    for rel in era::ARCHIVES {
        let rel = format!("{}/{rel}", swofty_core::ui::UI);
        let (ours, stock) = (&built.erps[&rel], stock(&game, &rel));
        let ui = fragment(ours, era::UI, era::UI_FRAGMENT);
        assert_eq!(
            ui,
            era::build_ui(&fragment(&stock, era::UI, era::UI_FRAGMENT)).unwrap()
        );
        assert_eq!(fragment(ours, era::UI, 0), fragment(&stock, era::UI, 0));
        let scene = era::build_scene(&fragment(&stock, era::SCENE, 0), &title).unwrap();
        assert_eq!(fragment(ours, era::SCENE, 0), scene);
        let parsed = Pkg::parse(&scene).unwrap();
        assert!(idf::id_problems(&parsed).unwrap().is_empty());
        assert!(era::taken(&ui, &scene).unwrap());
        let root = idf::scenes(&parsed).unwrap()[0].o().unwrap().at("scene").unwrap();
        let tops: Vec<&str> = root
            .o()
            .unwrap()
            .at("children")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .map(idf::elem_name)
            .collect();
        assert!(tops.contains(&"play_categories_row"), "{tops:?}");
        assert_eq!(
            tops[tops.len() - 3..],
            ["addon_categories_row", "label_builtin", "label_addons"]
        );
        assert!(!idf::elements(root).iter().any(|(p, _)| p.contains("mask")));

        let (a, b) = (
            Erp::parse(&std::fs::read(ours).unwrap()).unwrap(),
            Erp::parse(&std::fs::read(&stock).unwrap()).unwrap(),
        );
        let fade = if era::ROW_FADE { 2 } else { 0 };
        assert_eq!(a.resources.len(), b.resources.len() + fade, "{rel}");
        for (x, y) in a.resources.iter().zip(&b.resources) {
            assert_eq!(x.name, y.name);
            if x.name != era::UI && x.name != era::SCENE {
                assert_eq!(
                    x.content_hash().unwrap(),
                    y.content_hash().unwrap(),
                    "{rel}: {}",
                    x.name
                );
            }
        }

        let (nodes, sigs, bad) = swofty_core::ui::check::integrity(&ui).unwrap();
        assert!(bad.is_empty() && nodes > 0 && sigs > 0, "{bad:?}");
        let p = Pkg::parse(&ui).unwrap();
        let tpl = p
            .root
            .o()
            .unwrap()
            .at("Templates")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .find(|t| t.is_node() && t.name() == "dynamic_era_select")
            .unwrap();
        assert!(tpl.props().unwrap().get("visualNav").is_some());
        let kids: Vec<&str> = tpl
            .slot(CHILDREN)
            .unwrap()
            .iter()
            .filter(|k| k.is_node())
            .map(|k| k.name())
            .collect();
        assert_eq!(
            kids,
            [
                "tiles",
                era::ADDON_TILES,
                "label_builtin",
                "label_addons",
                "addon_row_has_tiles",
                "trigger_nav_on_up_down"
            ]
        );
        let layout = |n: &Node| -> Vec<String> {
            n.slot(LAYOUT).unwrap()[0]
                .a()
                .unwrap()
                .iter()
                .map(|b| b.o().unwrap().str_at("debugName").unwrap().to_owned())
                .collect()
        };
        let (top, bottom) = (tpl.child("tiles").unwrap(), tpl.child(era::ADDON_TILES).unwrap());
        assert_eq!(layout(top), ["play_categories_row"]);
        assert_eq!(layout(bottom), ["addon_categories_row"]);
        let sound = pair(&t("trigger_nav_on_up_down"), "navigate");
        let navs = [
            (
                top,
                era::ADDON_TILES,
                [
                    vec![],
                    vec![pair(&t("addon_row_has_tiles"), "query")],
                    vec![],
                    vec![],
                ],
            ),
            (
                bottom,
                "tiles",
                [
                    vec![pair(&t("tiles"), "focus"), sound.clone()],
                    vec![],
                    vec![],
                    vec![],
                ],
            ),
        ];
        for (row, other, nav) in navs {
            let path = t(row.name());
            let spawner = row.child("tile_spawner").unwrap();
            let sp = spawner.props().unwrap();
            assert!(sp.get("consumeStrategy").is_none());
            assert!(sp.get("InputOnActPrt").is_some());
            let pins = ["navUp", "navDown", "navLeft", "navRight"];
            for (dir, pin) in ["up", "down", "left", "right"].into_iter().zip(pins) {
                let failed = format!("navigate_{dir}_failed");
                assert_eq!(
                    signals(event(spawner, EVENTS, "eventName", &failed)),
                    [pair(&path, pin)]
                );
            }
            for (cmd, pin) in [
                ("focus", "focus"),
                ("block_input", "inputDisallow"),
                ("unblock_input", "inputAllow"),
            ] {
                assert_eq!(
                    signals(event(row, COMMANDS, "commandName", cmd)),
                    [pair(&format!("{path}.tile_spawner"), pin)]
                );
            }
            assert_eq!(
                signals(event(row, EVENTS, "eventName", "focused")),
                [pair(&t(other), "unfocus")]
            );
            for (pin, want) in pins.into_iter().zip(nav) {
                assert_eq!(
                    signals(event(row, EVENTS, "eventName", pin)),
                    want,
                    "{path} {pin}"
                );
            }
        }
        let entry = |n: &Node, key: &str| {
            n.props()
                .unwrap()
                .at(key)
                .unwrap()
                .o()
                .unwrap()
                .str_at("dbentry")
                .unwrap()
                .to_owned()
        };
        let (ts, bs) = (
            top.child("tile_spawner").unwrap(),
            bottom.child("tile_spawner").unwrap(),
        );
        assert_eq!(
            (entry(ts, "dataContext"), entry(ts, "selection")),
            ("tiles".into(), "selected_tile".into())
        );
        assert_eq!(
            (entry(bs, "dataContext"), entry(bs, "selection")),
            (era::ADDON_TILES.to_owned(), era::ADDON_SELECTION.to_owned())
        );
        let press = event(bs, EVENTS, "eventName", "select_pressed")
            .at("signals")
            .unwrap()
            .a()
            .unwrap()[0]
            .o()
            .unwrap()
            .at("attributes")
            .unwrap()
            .o()
            .unwrap()
            .at("command")
            .unwrap()
            .o()
            .unwrap()
            .at("name")
            .unwrap()
            .o()
            .unwrap()
            .str_at("value")
            .unwrap()
            .to_owned();
        assert_eq!(press, era::ADDON_PRESSED);
        let resets = |n: &Node| {
            signals(event(n, EVENTS, "eventName", "spawned"))
                .iter()
                .any(|(_, p)| p == "focus_and_reset")
        };
        assert!(resets(ts) && !resets(bs));
        let both = |pin: &str| vec![pair(&t("tiles"), pin), pair(&t(era::ADDON_TILES), pin)];
        let mut show = vec![
            pair(&t("tiles"), "trigger"),
            pair(&t(era::ADDON_TILES), "trigger"),
        ];
        show.extend(both("unblock_input"));
        assert_eq!(
            signals(event(tpl, COMMANDS, "commandName", "onShowCompleted")),
            show
        );
        let mut hide = vec![pair(&t("tiles"), "detach"), pair(&t(era::ADDON_TILES), "detach")];
        hide.extend(both("block_input"));
        assert_eq!(
            signals(event(tpl, COMMANDS, "commandName", "onTabDeselectBegin")),
            hide
        );
        for cmd in ["onTabSelected", "onTabActivated"] {
            assert_eq!(
                signals(event(tpl, COMMANDS, "commandName", cmd)),
                both("unblock_input")
            );
        }
        let gate = tpl.child("addon_row_has_tiles").unwrap();
        assert_eq!(gate.type_code(), "SVCD");
        let gp = gate.props().unwrap();
        assert_eq!(
            gp.at("dbEntry").unwrap().o().unwrap().str_at("value").unwrap(),
            era::ADDON_VISIBLE
        );
        assert_eq!(
            signals(event(gate, EVENTS, "eventName", "yes")),
            [pair(&t(era::ADDON_TILES), "focus"), sound]
        );
        assert_eq!(tpl.child("trigger_nav_on_up_down").unwrap().type_code(), "UIAM");
        for (label, text) in [("label_builtin", "Built In"), ("label_addons", "Addons")] {
            let t = tpl
                .child(label)
                .unwrap()
                .child("label_title")
                .unwrap()
                .props()
                .unwrap();
            assert_eq!(t.at("text").unwrap().o().unwrap().str_at("value").unwrap(), text);
        }
    }
    let again = out.join("again");
    let input = |rel: &str| match era::ARCHIVES.iter().any(|a| rel.ends_with(a)) {
        true => built.erps[rel].clone(),
        false => stock(&game, rel),
    };
    let twice = swofty_core::ui::build(&session(), &input, &|_, _| None, &again, true).unwrap();
    for rel in era::ARCHIVES {
        assert!(
            !twice.erps.contains_key(&format!("{}/{rel}", swofty_core::ui::UI)),
            "{rel}"
        );
        assert!(
            twice
                .lines
                .iter()
                .any(|l| l.contains(rel) && l.contains("already has an Addons row"))
        );
    }
    std::fs::remove_dir_all(&out).ok();
}

fn ui_of(path: &Path) -> Vec<u8> {
    fragment(path, swofty_core::ui::era::UI, swofty_core::ui::era::UI_FRAGMENT)
}

#[test]
fn the_era_rows_keep_what_an_addon_put_in_the_same_archive() {
    use swofty_core::build::{Built, build_menus};
    use swofty_core::ui::era;
    use swofty_formats::erp::{Entry, ErpFile, Resource};

    let Some(game) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let out = std::env::temp_dir().join(format!("swofty-menus-era-addon-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let overlay = out.join("overlay");
    let rel = format!("{}/{}", swofty_core::ui::UI, era::ARCHIVES[0]);
    let slice = "eaid://ui_package/art/textures/test_addon/slice.tif";
    let addon = overlay.join(&rel);
    std::fs::create_dir_all(addon.parent().unwrap()).unwrap();
    let file = ErpFile::open(&stock(&game, &rel)).unwrap();
    let res = Resource::like(
        slice,
        file.erp.find(era::FADE_SOURCE).unwrap(),
        &[b"slice\0".to_vec()],
    )
    .unwrap();
    let mut entries: Vec<Entry> = file.erp.resources.iter().map(Entry::Copy).collect();
    entries.push(Entry::Owned(res));
    file.write(entries, &addon).unwrap();

    let mut built = Built {
        overlay: overlay.clone(),
        ..Built::default()
    };
    built.plan.data.insert(rel.clone(), addon.clone());
    let mut session = session();
    build_menus(&mut built, &mut session, &|r| stock(&game, r), &|_, _| None, true).unwrap();
    assert_eq!(built.plan.data[&rel], addon);
    let names: Vec<String> = Erp::parse(&std::fs::read(&addon).unwrap())
        .unwrap()
        .resources
        .into_iter()
        .map(|r| r.name)
        .collect();
    assert!(names.iter().any(|n| n == slice), "{slice} is missing");
    assert_eq!(fragment(&addon, slice, 0), b"slice\0");
    assert!(era::has_rows(&ui_of(&addon)).unwrap());
    assert!(
        built
            .lines
            .iter()
            .any(|l| l.contains("era picker gets Built In and Addons rows"))
    );
    std::fs::remove_dir_all(&out).ok();
}

#[test]
fn an_addon_era_picker_swofty_cannot_edit_is_left_as_it_is() {
    use swofty_core::build::{Built, build_menus};
    use swofty_core::ui::era;
    use swofty_formats::erp::{Entry, ErpFile};

    let Some(game) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let out = std::env::temp_dir().join(format!("swofty-menus-era-odd-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let overlay = out.join("overlay");
    let ui = |rel: &str| format!("{}/{rel}", swofty_core::ui::UI);
    let (odd_rel, other) = (ui(era::ARCHIVES[1]), ui(era::ARCHIVES[0]));
    let addon = out.join("addon").join(&odd_rel);
    std::fs::create_dir_all(addon.parent().unwrap()).unwrap();
    let file = ErpFile::open(&stock(&game, &odd_rel)).unwrap();
    let mut res = file.load(file.erp.find(era::UI).unwrap()).unwrap();
    let mut odd = res.fragments[era::UI_FRAGMENT].data().unwrap();
    odd.extend_from_slice(&[0; 4]);
    res.set_fragment(era::UI_FRAGMENT, &odd).unwrap();
    let entries: Vec<Entry> = file
        .erp
        .resources
        .iter()
        .map(|r| match r.name == era::UI {
            true => Entry::Owned(res.clone()),
            false => Entry::Copy(r),
        })
        .collect();
    file.write(entries, &addon).unwrap();

    let mut built = Built {
        overlay: overlay.clone(),
        ..Built::default()
    };
    built.plan.data.insert(odd_rel.clone(), addon.clone());
    let writer = |erp: &str, res: &str| (res == era::UI).then(|| format!("Season mod ({erp})"));
    build_menus(&mut built, &mut session(), &|r| stock(&game, r), &writer, true).unwrap();
    assert_eq!(built.plan.data[&odd_rel], addon);
    assert_eq!(ui_of(&addon), odd);
    assert_eq!(built.plan.data[&other], overlay.join(&other));
    assert!(era::has_rows(&ui_of(&built.plan.data[&other])).unwrap());
    let skipped: Vec<&String> = built
        .warnings
        .iter()
        .filter(|w| w.contains("era picker"))
        .collect();
    assert_eq!(skipped.len(), 1, "{:?}", built.warnings);
    assert!(
        skipped[0].starts_with(&format!(
            "ui: Season mod ({odd_rel}) has its own copy of the era picker in {},",
            era::ARCHIVES[1]
        )),
        "{}",
        skipped[0]
    );
    assert!(
        built
            .lines
            .iter()
            .any(|l| l.contains("era picker gets Built In and Addons rows in 1 archive(s)")),
        "{:?}",
        built.lines
    );

    let mut built = Built {
        overlay: out.join("again"),
        ..Built::default()
    };
    built.plan.data.insert(odd_rel.clone(), addon.clone());
    let e = build_menus(
        &mut built,
        &mut session(),
        &|r| stock(&game, r),
        &|_, _| None,
        true,
    );
    std::fs::remove_dir_all(&out).ok();
    assert!(e.unwrap_err().to_string().contains("does not round-trip"));
}

fn named<'a>(n: &'a swofty_formats::pkg::Node, name: &str) -> Option<&'a swofty_formats::pkg::Node> {
    use swofty_core::ui::node::NodeExt;
    if n.is_node() && n.name() == name {
        return Some(n);
    }
    match n {
        swofty_formats::pkg::Node::Arr(a) => a.items.iter().find_map(|c| named(c, name)),
        swofty_formats::pkg::Node::Obj(o) => o.items.iter().find_map(|(_, c)| named(c, name)),
        _ => None,
    }
}

fn event_signals(n: &swofty_formats::pkg::Node, event: &str) -> Vec<(String, String)> {
    use swofty_core::ui::node::{EVENTS, NodeExt, ObjExt};
    n.slot(EVENTS)
        .unwrap()
        .iter()
        .filter(|e| e.o().unwrap().str_at("eventName").unwrap() == event)
        .flat_map(|e| e.o().unwrap().at("signals").unwrap().a().unwrap().clone())
        .map(|s| {
            let o = s.o().unwrap().clone();
            (
                o.str_at("to_object_name").unwrap().to_owned(),
                o.str_at("to_pin_name").unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn addon_settings_scrolls_sideways_and_shows_details_on_the_focused_tile() {
    use swofty_core::ui::node::{CHILDREN, NodeExt, ObjExt};
    let Some(game) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/menu_session.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
    let extra = v["addons"][1].clone();
    for i in 0..10 {
        let mut a = extra.clone();
        a["id"] = format!("test.addon_{i}").into();
        a["name"] = format!("Addon {i}").into();
        v["addons"].as_array_mut().unwrap().push(a);
    }
    let settings: SessionSettings = serde_json::from_value(v).unwrap();
    let out = std::env::temp_dir().join(format!("swofty-scroll-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let built = swofty_core::ui::build(&settings, &|rel| game.join(rel), &|_, _| None, &out, false).unwrap();
    let pkg = swofty_formats::pkg::Pkg::parse(
        &std::fs::read(
            built
                .screens
                .iter()
                .find(|(rel, _)| rel.ends_with("/swofty_settings.ui"))
                .unwrap()
                .1,
        )
        .unwrap(),
    )
    .unwrap();
    let home = named(&pkg.root, "swofty_settings").unwrap();
    let cols: Vec<_> = home
        .slot(CHILDREN)
        .unwrap()
        .iter()
        .filter(|c| c.is_node() && c.name().starts_with("col_"))
        .collect();
    assert!(cols.len() > 3, "{} columns", cols.len());
    let col = |c: usize| format!("swofty_settings.col_{c}");
    for (ci, column) in cols.iter().enumerate() {
        for t in column.slot(CHILDREN).unwrap().iter().filter(|t| t.is_node()) {
            let info = t.child("info").unwrap();
            let visible = info
                .props()
                .unwrap()
                .at("visible")
                .unwrap()
                .o()
                .unwrap()
                .at("value")
                .unwrap();
            assert!(matches!(&visible.v().unwrap().value, swofty_formats::pkg::Value::Raw(b) if b == &[0]));
            let path = format!("{}.{}.info", col(ci), t.name());
            assert!(
                event_signals(t, "focused").contains(&(path.clone(), "show".into())),
                "{}",
                t.name()
            );
            assert!(
                event_signals(t, "unfocused").contains(&(path, "hide".into())),
                "{}",
                t.name()
            );
            let right = event_signals(t, "navRight");
            if ci + 1 < cols.len() && ci + 1 >= 3 {
                assert!(
                    right.contains(&(col(ci - 2), "hide".into())),
                    "{}: {right:?}",
                    t.name()
                );
            } else {
                assert!(
                    right.iter().all(|(_, pin)| pin != "hide"),
                    "{}: {right:?}",
                    t.name()
                );
            }
            if ci > 0 {
                assert!(
                    event_signals(t, "navLeft").contains(&(col(ci - 1), "show".into())),
                    "{}",
                    t.name()
                );
            }
        }
    }
    let restore = home.child("restore_focus").unwrap();
    let last = cols.len() - 1;
    let tile = cols[last]
        .slot(CHILDREN)
        .unwrap()
        .iter()
        .find(|t| t.is_node())
        .unwrap()
        .name()
        .to_owned();
    let back = event_signals(restore, &tile);
    for c in 0..cols.len() {
        let pin = if c + 3 <= last { "hide" } else { "show" };
        assert!(back.contains(&(col(c), pin.into())), "{tile} col_{c}: {back:?}");
    }
    assert_eq!(back.last().unwrap().1, "focus");
    std::fs::remove_dir_all(&out).ok();
}

#[test]
fn a_long_addon_page_scrolls_to_keep_the_focused_row_on_screen() {
    use swofty_core::ui::node::{CHILDREN, NodeExt};
    let Some(game) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/menu_session.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
    let id = v["addons"][1]["id"].as_str().unwrap().to_owned();
    let template = v["addons"][1]["options"][1].clone();
    let options: Vec<serde_json::Value> = (0..30)
        .map(|i| {
            let mut o = template.clone();
            o["id"] = format!("opt_{i}").into();
            o["label"] = format!("Option {i}").into();
            for (s, state) in o["states"].as_array_mut().unwrap().iter_mut().enumerate() {
                state["command"] = format!("swofty_set:{id}:opt_{i}:{s}").into();
            }
            o
        })
        .collect();
    v["addons"][1]["options"] = options.into();
    let settings: SessionSettings = serde_json::from_value(v).unwrap();
    let out = std::env::temp_dir().join(format!("swofty-page-scroll-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let built = swofty_core::ui::build(&settings, &|rel| game.join(rel), &|_, _| None, &out, false).unwrap();
    let pages: Vec<_> = built
        .screens
        .iter()
        .filter(|(rel, _)| rel.contains("/swofty_addon_"))
        .map(|(_, path)| swofty_formats::pkg::Pkg::parse(&std::fs::read(path).unwrap()).unwrap())
        .collect();
    let rows_of = |p: &swofty_formats::pkg::Pkg| -> Option<(String, Vec<String>)> {
        let mut found = None;
        let mut stack = vec![&p.root];
        while let Some(n) = stack.pop() {
            if n.is_node() && n.name().starts_with("swofty_addon_") {
                let rows = n
                    .slot(CHILDREN)
                    .unwrap()
                    .iter()
                    .filter(|c| c.is_node() && c.name().starts_with("row_"))
                    .map(|c| c.name().to_owned())
                    .collect();
                found = Some((n.name().to_owned(), rows));
                break;
            }
            match n {
                swofty_formats::pkg::Node::Arr(a) => stack.extend(a.items.iter()),
                swofty_formats::pkg::Node::Obj(o) => stack.extend(o.items.iter().map(|(_, c)| c)),
                _ => {}
            }
        }
        found
    };
    let (long, short): (Vec<_>, Vec<_>) = pages
        .iter()
        .filter_map(|p| rows_of(p).map(|r| (p, r)))
        .partition(|(_, (_, rows))| rows.len() == 30);
    let (pkg, (sname, rows)) = long.first().expect("the 30-option page");
    let path = |i: usize| format!("{sname}.{}", rows[i]);
    for (i, name) in rows.iter().enumerate() {
        let row = named(&pkg.root, name).unwrap();
        let down = event_signals(row, "navDown");
        if i + 1 < rows.len() {
            assert_eq!(down[0], (path(i + 1), "focus".into()), "{name}: {down:?}");
        }
        let hides: Vec<_> = down.iter().filter(|(_, pin)| pin == "hide").collect();
        if i + 1 < rows.len() && i + 1 >= 18 {
            assert_eq!(hides, [&(path(i + 1 - 18), "hide".to_owned())], "{name}");
        } else {
            assert!(hides.is_empty(), "{name}: {down:?}");
        }
        let up = event_signals(row, "navUp");
        if i > 0 {
            assert_eq!(
                up,
                [(path(i - 1), "focus".into()), (path(i - 1), "show".into())],
                "{name}"
            );
        } else {
            assert!(up.is_empty(), "{name}: {up:?}");
        }
    }
    for (p, (_, rows)) in &short {
        for name in rows {
            let row = named(&p.root, name).unwrap();
            let pins: Vec<_> = ["navDown", "navUp"]
                .iter()
                .flat_map(|e| event_signals(row, e))
                .filter(|(_, pin)| pin == "hide" || pin == "show")
                .collect();
            assert!(pins.is_empty(), "{name}: {pins:?}");
        }
    }
    std::fs::remove_dir_all(&out).ok();
}

fn node_count(n: &swofty_formats::pkg::Node) -> usize {
    use swofty_core::ui::node::{CHILDREN, NodeExt};
    1 + n
        .slot(CHILDREN)
        .map(|k| k.iter().filter(|c| c.is_node()).map(node_count).sum())
        .unwrap_or(0)
}

fn route_key(e: &swofty_formats::pkg::Obj) -> Vec<String> {
    use swofty_core::ui::node::{NodeExt, ObjExt};
    e.at("signals")
        .unwrap()
        .a()
        .unwrap()
        .iter()
        .filter_map(|s| s.o().unwrap().at("attributes").ok())
        .filter_map(|a| a.o().unwrap().at("key").ok())
        .map(|k| k.o().unwrap().str_at("value").unwrap().to_owned())
        .collect()
}

#[test]
fn addon_settings_pages_its_home_so_each_home_screen_stays_small() {
    use swofty_core::ui::node::{CHILDREN, EVENTS, NodeExt, ObjExt};
    use swofty_core::ui::pages::{HOME_PAGE_TILES, NEXT_PAGE, PREV_PAGE};
    let Some(game) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/menu_session.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(fixture).unwrap()).unwrap();
    let extra = v["addons"][1].clone();
    for i in 0..70 {
        let mut a = extra.clone();
        a["id"] = format!("test.addon_{i}").into();
        a["name"] = format!("Addon {i}").into();
        a["description"] =
            "An addon with a description long enough to wrap over a few lines of its tile.".into();
        v["addons"].as_array_mut().unwrap().push(a);
    }
    let settings: SessionSettings = serde_json::from_value(v).unwrap();
    let tiles =
        settings.addons.iter().filter(|a| a.enabled).count() + settings.maps.len() + settings.mods.len();
    assert!(tiles >= 72, "{tiles} tiles");
    let out = std::env::temp_dir().join(format!("swofty-home-pages-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let built = swofty_core::ui::build(&settings, &|rel| game.join(rel), &|_, _| None, &out, false).unwrap();
    let homes: Vec<String> = (0..tiles.div_ceil(HOME_PAGE_TILES))
        .map(swofty_core::ui::pages::home_screen)
        .collect();
    assert_eq!(homes[..2], ["swofty_settings", "swofty_settings_2"]);
    let read = |name: &str| {
        let rel = format!("swofty/ui/{name}.ui");
        let data = std::fs::read(&built.screens[&rel]).unwrap();
        let pkg = swofty_formats::pkg::Pkg::parse(&data).unwrap();
        (data.len(), pkg)
    };
    for (p, home) in homes.iter().enumerate() {
        let s = built.session.iter().find(|s| &s.name == home).unwrap();
        assert_eq!((s.flow.as_str(), s.view_model), ("flow_options", false));
        let (size, pkg) = read(home);
        let scr = named(&pkg.root, home).unwrap();
        let nodes = node_count(scr);
        let restore = scr.child("restore_focus").unwrap();
        let restore_signals: usize = restore
            .slot(EVENTS)
            .unwrap()
            .iter()
            .map(|e| e.o().unwrap().at("signals").unwrap().a().unwrap().len())
            .sum();
        eprintln!("{home}: {size} bytes, {nodes} nodes, {restore_signals} restore signals");
        assert!(size <= 360_000, "{home} is {size} bytes");
        assert!(nodes <= 520, "{home} has {nodes} nodes");
        assert!(
            restore_signals <= 270,
            "{home} has {restore_signals} restore signals"
        );
        let cols: Vec<_> = scr
            .slot(CHILDREN)
            .unwrap()
            .iter()
            .filter(|c| c.is_node() && c.name().starts_with("col_"))
            .collect();
        let names: Vec<String> = cols
            .iter()
            .flat_map(|c| c.slot(CHILDREN).unwrap().iter().filter(|t| t.is_node()))
            .map(|t| t.name().to_owned())
            .collect();
        let first = p * HOME_PAGE_TILES;
        let want: Vec<String> = (first..tiles.min(first + HOME_PAGE_TILES))
            .map(|i| format!("t{i}"))
            .chain((p + 1 < homes.len()).then(|| NEXT_PAGE.to_owned()))
            .chain((p > 0).then(|| PREV_PAGE.to_owned()))
            .collect();
        assert_eq!(names, want, "{home}");
        let title = format!("{:?}", named(scr, "screen_title").unwrap());
        assert!(
            title.contains(&format!("PAGE {}/{}", p + 1, homes.len())),
            "{home}"
        );
        let nav = cols.last().unwrap();
        let back = scr.child("back_listener").unwrap();
        let back_sigs = event_signals(back, "pressed");
        let remember_prev = (format!("{home}.remember_{PREV_PAGE}"), "trigger".to_owned());
        assert_eq!(back_sigs.contains(&remember_prev), p > 0, "{home}: {back_sigs:?}");
        assert_eq!(
            restore
                .slot(EVENTS)
                .unwrap()
                .iter()
                .any(|e| e.o().unwrap().str_at("eventName").unwrap() == "page_first"),
            p > 0
        );
        if p + 1 < homes.len() {
            let next = nav.child(NEXT_PAGE).unwrap();
            let pressed = event(next, EVENTS, "eventName", "pressed");
            assert_eq!(route_key(pressed), [NEXT_PAGE]);
            assert!(signals(pressed).contains(&(format!("{home}.remember_{NEXT_PAGE}"), "trigger".into())));
            let back_here = event_signals(restore, NEXT_PAGE);
            assert_eq!(
                back_here.last().unwrap(),
                &(format!("{home}.{}.{NEXT_PAGE}", nav.name()), "focus".into())
            );
        }
        if p > 0 {
            let arrived = event_signals(restore, "page_first");
            assert_eq!(
                arrived.last().unwrap(),
                &(format!("{home}.col_0.t{first}"), "focus".into())
            );
            assert!(arrived.iter().all(|(_, pin)| pin != "hide"), "{arrived:?}");
            let prev = nav.child(PREV_PAGE).unwrap();
            assert_eq!(
                route_key(event(prev, EVENTS, "eventName", "pressed")),
                [PREV_PAGE]
            );
            let remember = scr.child(&format!("remember_{PREV_PAGE}")).unwrap();
            let source = remember
                .props()
                .unwrap()
                .at("source")
                .unwrap()
                .o()
                .unwrap()
                .str_at("value")
                .unwrap()
                .to_owned();
            assert_eq!(source, NEXT_PAGE);
        }
        let t = cols[0]
            .slot(CHILDREN)
            .unwrap()
            .iter()
            .find(|t| t.is_node())
            .unwrap();
        let key = route_key(event(t, EVENTS, "eventName", "pressed"));
        let addons = settings.addons.iter().filter(|a| a.enabled).count();
        let want = match first.checked_sub(addons) {
            None => format!("open_addon_{first}"),
            Some(m) => format!("open_map_{m}"),
        };
        assert_eq!(key, [want], "{home}");
    }
    let options = game.join("2025_asset_groups/ui_package/flow_options.erp");
    let stock_ui = swofty_core::ui::check::ArchiveUi::read("flow_options.erp", &options).unwrap();
    let mut edited = swofty_core::ui::check::ArchiveUi::read("flow_options.erp", &options).unwrap();
    for home in &homes {
        let data = std::fs::read(&built.screens[&format!("swofty/ui/{home}.ui")]).unwrap();
        edited.resources.insert(
            format!("eaid://ui_package/screens/{home}.ui"),
            ("UIDF".to_owned(), vec![data]),
        );
    }
    let report = swofty_core::ui::check::archives(
        &[edited],
        &[stock_ui],
        swofty_core::ui::catalog::stock(),
        swofty_core::ui::catalog::engine(),
    )
    .unwrap();
    let hard: Vec<String> = report.hard.iter().map(ToString::to_string).collect();
    assert!(hard.is_empty(), "{hard:?}");
    let routes = fragment(
        &built.erps["2025_asset_groups/ui_package/persistent.erp"],
        "eaid://ui_package/routes/routes_frontend_general.uir",
        0,
    );
    let routes = swofty_formats::pkg::Pkg::parse(&routes).unwrap();
    let group = |id: &str| -> Vec<String> {
        routes
            .root
            .o()
            .unwrap()
            .at("RouteGroupings")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .find(|g| g.o().unwrap().str_at("Id").ok() == Some(id))
            .unwrap_or_else(|| panic!("no route group {id}"))
            .o()
            .unwrap()
            .at("Routes")
            .unwrap()
            .a()
            .unwrap()
            .iter()
            .map(|r| r.o().unwrap().str_at("Id").unwrap().to_owned())
            .collect()
    };
    let second = group("swofty_settings_2");
    for want in ["back", NEXT_PAGE, PREV_PAGE, "open_addon_30"] {
        assert!(second.contains(&want.to_owned()), "{want}: {second:?}");
    }
    assert!(!group("swofty_settings").contains(&"open_addon_30".to_owned()));
    assert!(group("swofty_addon_30").contains(&"back".to_owned()));
    std::fs::remove_dir_all(&out).ok();
}

#[test]
fn the_mfd_page_and_a_map_patch_share_flow_race_and_persistent() {
    use swofty_core::activation::place::track_group;
    use swofty_core::activation::ui::{MapUi, build_ui};
    use swofty_core::build::{Built, build_menus};
    use swofty_core::map::TrackPackage;
    use swofty_core::settings::SessionAddon;
    use swofty_core::ui::mfd;

    let Some(game) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let Some(dir) = std::env::var_os("SWOFTY_MAPS_DIR")
        .map(PathBuf::from)
        .map(|d| d.join("bahrain24"))
        .filter(|d| d.join("manifest.json").is_file())
    else {
        eprintln!("SWOFTY_MAPS_DIR has no bahrain24 package; skipping");
        return;
    };
    let pkg = TrackPackage::parse(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("swofty-menus-mfd-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let resolve = |rel: &str| stock(&game, rel);
    let maps = [MapUi {
        pkg: &pkg,
        dir: &dir,
        asset_group: track_group("bahrain24", "bahrain24"),
    }];
    let map_ui = build_ui(&maps, &resolve, &out.join("cache")).unwrap();
    let race = format!("{}/flow_race.erp", swofty_core::ui::UI);
    let persistent = format!("{}/persistent.erp", swofty_core::ui::UI);
    assert!(map_ui.replaced.contains_key(&race) && map_ui.replaced.contains_key(&persistent));
    let mut built = Built {
        overlay: out.join("overlay"),
        ..Built::default()
    };
    built.plan.data.extend(map_ui.replaced.clone());
    let mut session = session();
    let addon: SessionAddon = serde_json::from_value(serde_json::json!({
        "id": "t.mfd", "name": "MFD test", "version": "1.0.0", "enabled": true, "locked": false,
        "enable_commands": ["swofty_enable:t.mfd:0", "swofty_enable:t.mfd:1"],
        "options": [{ "id": "limiter", "type": "toggle", "binds": "native", "label": "Pit limiter", "current": 1,
                      "states": [{ "value": false, "text": "Off", "command": "swofty_set:t.mfd:limiter:0" },
                                 { "value": true, "text": "On", "command": "swofty_set:t.mfd:limiter:1" }] }],
        "mfd": { "title": "PIT LANE", "rows": ["limiter"] }
    }))
    .unwrap();
    session.addons.push(addon);
    build_menus(&mut built, &mut session, &resolve, &|_, _| None, true).unwrap();
    assert!(
        built
            .lines
            .iter()
            .any(|l| l == "ADDONS MFD page: 1 row(s) from t.mfd"),
        "{:?}",
        built.lines
    );
    for rel in [&race, &persistent] {
        let ours = &built.plan.data[rel];
        assert_eq!(
            *ours,
            out.join("overlay").join(rel),
            "{rel} is built by the menus"
        );
        let stock_erp = Erp::parse(&std::fs::read(resolve(rel)).unwrap()).unwrap();
        let ours_erp = Erp::parse(&std::fs::read(ours).unwrap()).unwrap();
        let mut kept = 0;
        for res in Erp::parse(&std::fs::read(&map_ui.replaced[rel]).unwrap())
            .unwrap()
            .resources
        {
            let theirs = res.content_hash().unwrap();
            if stock_erp.find(&res.name).ok().map(|s| s.hash) != Some(theirs) {
                assert_eq!(
                    ours_erp.find(&res.name).unwrap().hash,
                    res.hash,
                    "{rel}: {} lost the map edits",
                    res.name
                );
                kept += 1;
            }
        }
        assert!(kept > 0, "{rel} has no map edits");
    }
    let templates = fragment(&built.plan.data[&race], mfd::TEMPLATES, 0);
    assert!(
        templates
            .windows(mfd::PANEL.len())
            .any(|w| w == mfd::PANEL.as_bytes()),
        "flow_race.erp has no ADDONS panel"
    );
    let metadata = fragment(&built.plan.data[&persistent], mfd::METADATA, 0);
    let doc = swofty_formats::bxml::Document::parse(&metadata).unwrap();
    let names: Vec<String> = doc.root.children[0]
        .children
        .iter()
        .filter_map(|c| c.attr(b"Name").map(|v| String::from_utf8_lossy(v).into_owned()))
        .collect();
    assert_eq!(names.last().map(String::as_str), Some(mfd::PANEL), "{names:?}");
    std::fs::remove_dir_all(&out).ok();
}
