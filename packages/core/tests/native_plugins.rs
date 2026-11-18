use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use swofty_core::baseline::Identity;
use swofty_core::build::{GVDB, Plugins, plugin_files};
use swofty_core::install::{Game, Installer, Plan};
use swofty_core::profile::Profile;
use swofty_core::resolver::resolve;
use swofty_core::store::Store;

const CORE: &str = "swofty/addons/core/swofty_core.dll";

fn write(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn manifest(id: &str, dll: &str, early: bool) -> String {
    format!(
        r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t","compat":{{"game":"f1_25"}},
        "operations":{{"native":[{{"dll":"{dll}","capabilities":["log","assets"],"early":{early},"files":["lang/"]}}]}}}}"#
    )
}

struct Fixture {
    root: PathBuf,
    store: Store,
    game: Game,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("swofty-native-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        write(&dir.join("F1_25.exe"), "exe");
        write(&dir.join(GVDB), "db");
        for f in ["proxy", "core"] {
            write(&root.join("loader").join(f), f);
        }
        let store = Store::new(root.join("data"));
        for (id, dll, early) in [
            ("t.native", "native/t_native.dll", true),
            ("t.other", "native/t_other.dll", false),
        ] {
            let src = root.join("src").join(id);
            write(&src.join("swofty.json"), &manifest(id, dll, early));
            write(&src.join(dll), id);
            write(&src.join("lang/extra.lng"), "LNGT");
            write(&src.join("lang/.cache/x"), "x");
            write(&src.join("src/plugin.cpp"), "src");
            store.install_addon(&src).unwrap();
        }
        for (name, native) in [("on", true), ("off", false)] {
            let mut p = Profile::new(name);
            p.set_enabled("t.other", true).unwrap();
            p.set_enabled("t.native", native).unwrap();
            store.save_profile(&p).unwrap();
        }
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        Self { root, store, game }
    }

    fn plugins(&self, profile: &str) -> swofty_core::Result<Plugins> {
        let addons = self.store.addons()?;
        let order = resolve(&self.store.profile(profile)?, &addons)?.order;
        plugin_files(
            &self.store,
            order.iter().map(|id| &addons[id]),
            ["swofty_core.dll".to_owned()],
        )
    }

    fn plan(&self, profile: &str) -> Plan {
        let mut loader = BTreeMap::from([
            ("version.dll".to_owned(), self.root.join("loader/proxy")),
            (CORE.to_owned(), self.root.join("loader/core")),
        ]);
        loader.extend(self.plugins(profile).unwrap().files);
        Plan {
            profile: profile.into(),
            loader,
            ..Plan::default()
        }
    }

    fn has(&self, rel: &str) -> bool {
        self.game.dir.join(rel).exists()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

#[test]
fn places_the_plugins_of_enabled_addons_in_load_order() {
    let f = Fixture::new("order");
    let on = f.plugins("on").unwrap();
    assert_eq!(
        on.dlls,
        [
            "addons/t.other/native/t_other.dll",
            "addons/t.native/native/t_native.dll"
        ]
    );
    assert_eq!(
        on.files
            .keys()
            .filter(|k| k.starts_with("swofty/addons/t.native/"))
            .collect::<Vec<_>>(),
        [
            "swofty/addons/t.native/lang/extra.lng",
            "swofty/addons/t.native/native/t_native.dll",
        ]
    );
    assert_eq!(
        on.files["swofty/addons/t.native/native/t_native.dll"],
        f.store.addons_dir().join("t.native/native/t_native.dll")
    );
    let off = f.plugins("off").unwrap();
    assert_eq!(off.dlls, ["addons/t.other/native/t_other.dll"]);
    assert!(off.files.keys().all(|k| k.starts_with("swofty/addons/t.other/")));
    let session = f.store.session_settings("on").unwrap();
    assert_eq!(session.early_plugins, ["t_native.dll"]);
    assert!(f.store.session_settings("off").unwrap().early_plugins.is_empty());
}

#[test]
fn a_profile_switch_and_restore_remove_the_plugin_files() {
    let f = Fixture::new("switch");
    let inst = Installer::new(&f.store.root, &f.game);
    inst.apply(&f.plan("on")).unwrap();
    for rel in [
        CORE,
        "swofty/addons/t.native/native/t_native.dll",
        "swofty/addons/t.native/lang/extra.lng",
        "swofty/addons/t.other/native/t_other.dll",
    ] {
        assert!(f.has(rel), "{rel} is placed");
    }
    assert!(!f.has("swofty/addons/t.native/lang/.cache"));
    assert!(!f.has("swofty/addons/t.native/src"));
    inst.apply(&f.plan("off")).unwrap();
    assert!(!f.has("swofty/addons/t.native"));
    assert!(f.has("swofty/addons/t.other/native/t_other.dll"));
    inst.apply(&f.plan("on")).unwrap();
    assert!(f.has("swofty/addons/t.native/native/t_native.dll"));
    inst.apply(&Plan::vanilla()).unwrap();
    assert!(!f.has("swofty"));
    inst.apply(&f.plan("on")).unwrap();
    assert!(inst.restore().unwrap().is_empty());
    assert!(!f.has("swofty"));
    assert!(!f.has("version.dll"));
}

#[test]
fn rejects_a_missing_plugin_and_a_clashing_name() {
    let f = Fixture::new("reject");
    std::fs::remove_file(f.store.addons_dir().join("t.native/native/t_native.dll")).unwrap();
    let e = f.plugins("on").unwrap_err().to_string();
    assert!(e.contains("t.native: native/t_native.dll is missing"), "{e}");
    assert_eq!(f.plugins("off").unwrap().dlls.len(), 1);
    std::fs::remove_dir_all(f.store.addons_dir().join("t.other/lang")).unwrap();
    let e = f.plugins("off").unwrap_err().to_string();
    assert!(e.contains("t.other: lang is missing"), "{e}");
    let src = f.root.join("src/t.clash");
    write(
        &src.join("swofty.json"),
        &manifest("t.clash", "Swofty_Core.dll", false),
    );
    write(&src.join("Swofty_Core.dll"), "clash");
    f.store.install_addon(&src).unwrap();
    let mut p = Profile::new("clash");
    p.set_enabled("t.clash", true).unwrap();
    f.store.save_profile(&p).unwrap();
    let e = f.plugins("clash").unwrap_err().to_string();
    assert!(
        e.contains("plugin Swofty_Core.dll has the same file name as a plugin of swofty.core"),
        "{e}"
    );
}

#[test]
fn the_integrity_module_is_placed_for_a_session_and_rolled_back_with_it() {
    use swofty_core::timetrial::integrity::{self, Module};
    const DLL: &str = "swofty/addons/swofty.integrity/swofty_integrity.dll";
    let f = Fixture::new("integrity");
    let cached = f.root.join("data/timetrial/integrity/v1/swofty_integrity.dll");
    write(&cached, "attest");
    let module = Module {
        version: "v1".into(),
        dll: cached,
        sha256: String::new(),
    };
    let mut plan = f.plan("on");
    let mut session = f.store.session_settings("on").unwrap();
    integrity::place(&module, &mut plan.loader, &mut session.plugins);
    assert_eq!(session.plugins, ["addons/swofty.integrity/swofty_integrity.dll"]);

    let inst = Installer::new(&f.store.root, &f.game);
    inst.apply(&plan).unwrap();
    assert_eq!(std::fs::read_to_string(f.game.dir.join(DLL)).unwrap(), "attest");
    assert!(f.has("swofty/addons/t.native/native/t_native.dll"));

    inst.apply(&f.plan("on")).unwrap();
    assert!(!f.has("swofty/addons/swofty.integrity"));
    assert!(f.has("swofty/addons/t.native/native/t_native.dll"));

    inst.apply(&plan).unwrap();
    assert!(f.has(DLL));
    assert!(inst.restore().unwrap().is_empty());
    assert!(!f.has("swofty"));
}

#[test]
fn a_server_plugin_is_placed_for_a_session_and_rolled_back_with_it() {
    use swofty_core::modules::{self, Module, TT_GAME};
    const DLL: &str = "swofty/addons/swofty.tt/swofty_tt.dll";
    let f = Fixture::new("server-plugin");
    let cached = f
        .root
        .join("data/modules/tt-game/windows-x86_64/1.0/swofty_tt.dll");
    write(&cached, "plugin");
    let module = Module {
        name: TT_GAME.name.into(),
        version: "1.0".into(),
        path: cached,
        sha256: String::new(),
    };
    let mut plan = f.plan("on");
    let mut session = f.store.session_settings("on").unwrap();
    modules::place(&module, TT_GAME.id, &mut plan.loader, &mut session.plugins);
    assert_eq!(session.plugins, ["addons/swofty.tt/swofty_tt.dll"]);

    let inst = Installer::new(&f.store.root, &f.game);
    inst.apply(&plan).unwrap();
    assert_eq!(std::fs::read_to_string(f.game.dir.join(DLL)).unwrap(), "plugin");

    inst.apply(&f.plan("on")).unwrap();
    assert!(!f.has("swofty/addons/swofty.tt"));
    assert!(f.has("swofty/addons/t.native/native/t_native.dll"));

    inst.apply(&plan).unwrap();
    assert!(f.has(DLL));
    assert!(inst.restore().unwrap().is_empty());
    assert!(!f.has("swofty"));
    let ids: Vec<String> = f.store.addons().unwrap().keys().cloned().collect();
    assert!(!ids.iter().any(|id| id == TT_GAME.id));
}

#[test]
fn the_integrity_module_is_not_a_user_addon() {
    let f = Fixture::new("not-an-addon");
    let ids: Vec<String> = f.store.addons().unwrap().keys().cloned().collect();
    assert!(
        !ids.iter()
            .any(|id| id == swofty_core::timetrial::integrity::PLUGIN_ID)
    );
    for profile in ["on", "off"] {
        let session = f.store.session_settings(profile).unwrap();
        assert!(session.plugins.is_empty());
        assert!(
            session
                .addons
                .iter()
                .all(|a| a.id != swofty_core::timetrial::integrity::PLUGIN_ID)
        );
    }
}
