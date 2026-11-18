//! Project Halo 0.1.2 and the Planked + Brake Check Damage Panel 1.0.5 both replace the HUD template
//! whole. Built together, without the bridge addon, the template has the panel's six readouts under
//! the damage panel and everything of Halo's. Needs `F1_25_DIR` (read through the launcher's baseline,
//! never as the stock base), `PLANKED_PANEL_DIR` (the unzipped panel) and Project Halo, from
//! `PROJECT_HALO_DIR` or the launcher's addon store; skipped without them.

use std::path::{Path, PathBuf};

use swofty_core::baseline::Identity;
use swofty_core::build::{GVDB, build};
use swofty_core::hud::{GameStock, dials};
use swofty_core::install::Game;
use swofty_core::store::Store;
use swofty_core::ui::node::{NodeExt, canonical, find_path, node_at_mut};
use swofty_formats::erp::ErpFile;
use swofty_formats::gvdb::Gvdb;
use swofty_formats::pkg::Pkg;

const UI: &str = "2025_asset_groups/ui_package";
const DAMAGE: &str = "eaid://ui_package/art/osd/components/dial/mfd_car_damage_panel.idf";
const PANEL: &str = "hud_mfd_dynamic.mfd.attacher.car_damage_panel";
const READOUTS: [&str; 6] = [
    "brake_check_fl",
    "brake_check_fr",
    "brake_check_rl",
    "brake_check_rr",
    "planked_value",
    "planked_detail",
];

fn write(path: &Path, body: impl AsRef<[u8]>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn copy_dir(from: &Path, to: &Path) {
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        let t = to.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &t);
        } else {
            write(&t, std::fs::read(&p).unwrap());
        }
    }
}

struct Fixture {
    root: PathBuf,
    store: Store,
    game: Game,
    _pin: swofty_core::builtin::LoaderPin,
}

impl Fixture {
    fn new(name: &str, real: &Path) -> Self {
        let root = std::env::temp_dir().join(format!("swofty-ui-merge-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        write(&dir.join("F1_25.exe"), "exe");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(GVDB), Gvdb { records: Vec::new() }.to_bytes()).unwrap();
        let originals = GameStock::with_baseline(real, &Store::open_default().root).unwrap();
        let mut rels: Vec<String> = swofty_core::ui::ERPS
            .iter()
            .chain(swofty_core::ui::READS)
            .chain(
                [
                    "flow_race.erp",
                    "flow_replay.erp",
                    "screens/hud_spectator_2.erp",
                    "screens/replays_and_highlights.erp",
                ]
                .iter(),
            )
            .map(|r| format!("{UI}/{r}"))
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

    fn install_copy(&self, from: &Path, name: &str, drop_native: bool) {
        let src = self.root.join("src").join(name);
        copy_dir(from, &src);
        let manifest = src.join("swofty.json");
        let mut m: serde_json::Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
        let id = m["id"].as_str().unwrap().to_owned();
        m.as_object_mut().unwrap().remove("requires");
        if drop_native {
            m["kind"] = serde_json::json!(["erp"]);
            m["operations"].as_object_mut().unwrap().remove("native");
            m.as_object_mut().unwrap().remove("options");
            let _ = std::fs::remove_dir_all(src.join("native"));
        }
        std::fs::write(&manifest, m.to_string()).unwrap();
        self.store.install_addon(&src).unwrap();
        self.store
            .update_profile("default", |p| p.set_enabled(&id, true))
            .unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn inputs() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let game = std::env::var_os("F1_25_DIR").map(PathBuf::from);
    let panel = std::env::var_os("PLANKED_PANEL_DIR").map(PathBuf::from);
    let halo = std::env::var_os("PROJECT_HALO_DIR")
        .map(PathBuf::from)
        .or_else(|| Some(Store::open_default().addons_dir().join("nick.f1-2026-halo")));
    match (game, panel, halo) {
        (Some(g), Some(p), Some(h)) if p.is_dir() && h.join("erp/tower_template.erp").is_file() => {
            Some((g, p, h))
        }
        _ => {
            eprintln!("F1_25_DIR, PLANKED_PANEL_DIR or Project Halo is missing; skipping");
            None
        }
    }
}

fn resource_of(archive: &Path, name: &str) -> Vec<u8> {
    let file = ErpFile::open(archive).unwrap();
    let res = file.erp.find(name).unwrap();
    file.load(res).unwrap().fragments[0].data().unwrap()
}

fn without_readouts(p: &mut Pkg) {
    let at = find_path(&p.root, PANEL).unwrap();
    let panel = node_at_mut(&mut p.root, &at).unwrap();
    let kids = panel.slot_mut(swofty_core::ui::node::CHILDREN).unwrap();
    let before = kids.len();
    kids.retain(|k| !READOUTS.contains(&k.name()));
    assert_eq!(before - kids.len(), READOUTS.len());
}

#[test]
fn halo_and_the_panel_build_into_the_bridge_addons_template() {
    halo_then_panel(true);
}

#[test]
fn the_merge_does_not_depend_on_which_addon_loads_first() {
    halo_then_panel(false);
}

fn halo_then_panel(halo_first: bool) {
    let Some((game, panel, halo)) = inputs() else {
        return;
    };
    let f = Fixture::new(if halo_first { "halo-first" } else { "panel-first" }, &game);
    if halo_first {
        f.install_copy(&halo, "halo", true);
        f.install_copy(&panel, "panel", false);
    } else {
        f.install_copy(&panel, "panel", false);
        f.install_copy(&halo, "halo", true);
    }
    let built = build(&f.store, "default", &f.game).unwrap();
    let halo_tower = halo.join("erp/tower_template.erp");
    for (erp, layout) in [
        ("flow_race.erp", "ui/flow_race.damage-layout.pkg"),
        ("flow_options.erp", "ui/flow_options.damage-layout.pkg"),
    ] {
        let rel = format!("{UI}/{erp}");
        let archive = &built.plan.data[&rel];
        let mut merged = Pkg::parse(&resource_of(archive, dials::TEMPLATE)).unwrap();
        for name in READOUTS {
            assert!(
                find_path(&merged.root, &format!("{PANEL}.{name}")).is_ok(),
                "{erp}: {name} is missing under the damage panel"
            );
        }
        without_readouts(&mut merged);
        let halo_only = Pkg::parse(&resource_of(&halo_tower, dials::TEMPLATE)).unwrap();
        assert_eq!(
            canonical(&merged.root),
            canonical(&halo_only.root),
            "{erp}: Halo's tree is not intact"
        );
        let slots = Pkg::parse(&resource_of(archive, DAMAGE)).unwrap();
        let panel_slots = Pkg::parse(&std::fs::read(panel.join(layout)).unwrap()).unwrap();
        assert_eq!(
            canonical(&slots.root),
            canonical(&panel_slots.root),
            "{erp}: the panel's damage layout"
        );
    }
    let line = built
        .lines
        .iter()
        .find(|l| l.contains("hud_dynamic_templates.ui merged node by node"))
        .expect("the merge is recorded in the build lines");
    let order = match halo_first {
        true => "Project Halo then Planked + Brake Check Damage Panel",
        false => "Planked + Brake Check Damage Panel then Project Halo",
    };
    assert!(line.contains(order), "{line}");
    assert!(
        !built
            .warnings
            .iter()
            .any(|w| w.contains("both change") || w.contains("overrides Project Halo")),
        "{:#?}",
        built.warnings
    );
}
