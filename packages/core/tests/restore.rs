use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use swofty_core::baseline::{Identity, sha256_bytes};
use swofty_core::install::{Drift, Game, Installer, OVERRIDE_HEADER, Plan};
use swofty_core::journal::{Journal, Op, State};
use swofty_core::recovery;

const GVDB: &str = "game_package/gamedatabases/gamedatabases.2025.bin";
const UI: &str = "2025_asset_groups/ui_package/flow_frontend.erp";
const CORE: &str = "swofty/addons/core/swofty_core.dll";
const SCREEN: &str = "swofty/ui/swofty_addon_0.ui";
const STOCK_TRACK: &str = "2025_asset_groups/environment_package/tracks/bahrain/wep/bahrain_common.erp";
const MAP_FILES: &[&str] = &[
    "2025_asset_groups/environment_package/tracks/okayama/wep/okayama_common.erp",
    "2025_asset_groups/environment_package/tracks/okayama/wep/okayama_f1_26.erp",
    "2025_asset_groups/environment_package/tracks/okayama/ai/f1_2026_vehicle_package/haas/haas.erp",
    "2025_asset_groups/ui_package/textures/track_maps/okayama.erp",
    "asset_groups/environment_package/tracks/okayama/mipmaps/3b5d3c7d207e_bisect_placeholder.tif.2025.mipmaps",
];
const REG: &str = "WINE REGISTRY Version 2\n;; All keys relative to \\\\User\n\n[Software\\\\Wine] 1700000000\n\"Version\"=\"win10\"\n";

#[test]
fn sweep_rechecks_changed_cached_files_and_preserves_user_edits() {
    use swofty_core::baseline::Hashes;
    use swofty_core::placed::Placed;
    let f = Fixture::new("sweep-cache-edits");
    let mut placed = Placed::default();
    let mut hashes = Hashes::load(&f.data);
    for rel in ["addon/unchanged.bin", "addon/edited.bin"] {
        write(&f.game.join(rel), "mod bytes");
        let sha = hashes.sha256(&f.game.join(rel)).unwrap();
        placed.insert(rel, sha, false);
    }
    placed.save(&f.data).unwrap();
    hashes.save();
    write(&f.game.join("addon/edited.bin"), "user changed these bytes");
    let game = f.game("1");
    Installer::new(&f.data, &game).sweep().unwrap();
    assert!(!f.game.join("addon/unchanged.bin").exists());
    assert_eq!(f.read("addon/edited.bin"), "user changed these bytes");
}

#[test]
#[ignore = "manual large-profile sweep benchmark"]
fn benchmark_large_profile_sweep_hash_cache() {
    use swofty_core::baseline::Hashes;
    use swofty_core::placed::Placed;
    for warm in [false, true] {
        let f = Fixture::new(if warm { "bench-warm" } else { "bench-cold" });
        let bytes = vec![42u8; 256 * 1024];
        let sha = sha256_bytes(&bytes);
        let mut placed = Placed::default();
        let mut hashes = Hashes::load(&f.data);
        for i in 0..512 {
            let rel = format!("mods/assets/{i}.bin");
            let path = f.game.join(&rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &bytes).unwrap();
            placed.insert(&rel, sha.clone(), false);
            if warm {
                hashes.wrote(&path, &sha);
            }
        }
        placed.save(&f.data).unwrap();
        hashes.save();
        let game = f.game("1");
        let start = std::time::Instant::now();
        let removed = Installer::new(&f.data, &game).sweep().unwrap();
        assert_eq!(removed.len(), 512);
        println!("512 files / 128 MiB; cached={warm}: {:?}", start.elapsed());
    }
}

struct Fixture {
    root: PathBuf,
    game: PathBuf,
    data: PathBuf,
    reg: PathBuf,
    src: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        Self::at(name, None)
    }

    fn at(name: &str, game: Option<PathBuf>) -> Self {
        let root = std::env::temp_dir().join(format!("swofty-restore-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let f = Self {
            game: game.unwrap_or_else(|| root.join("game")),
            data: root.join("data"),
            reg: root.join("pfx/user.reg"),
            src: root.join("src"),
            root,
        };
        for (rel, body) in [
            ("F1_25.exe", "exe-v1"),
            (GVDB, "db-v1"),
            (UI, "ui-v1"),
            ("readme.txt", "r-v1"),
            (STOCK_TRACK, "track-v1"),
        ] {
            write(&f.game.join(rel), body);
        }
        write(&f.reg, REG);
        for (name, body) in [
            ("dbA", "db-A"),
            ("dbB", "db-B"),
            ("uiB", "ui-B"),
            ("proxy", "proxy"),
            ("core", "core"),
            ("screen", "screen"),
        ] {
            write(&f.src.join(name), body);
        }
        for (i, _) in MAP_FILES.iter().enumerate() {
            write(&f.src.join(format!("map{i}")), &format!("map-{i}"));
        }
        f
    }

    fn maps_plan(&self) -> Plan {
        let mut p = self.plan("a");
        p.profile = "maps".into();
        p.data.insert(UI.into(), self.src.join("uiB"));
        p.added = MAP_FILES
            .iter()
            .enumerate()
            .map(|(i, rel)| ((*rel).to_owned(), self.src.join(format!("map{i}"))))
            .collect();
        p
    }

    fn game(&self, build: &str) -> Game {
        Game {
            dir: self.game.clone(),
            user_reg: Some(self.reg.clone()),
            identity: Identity::detect(&self.game, Some(build.parse().unwrap())).unwrap(),
        }
    }

    fn plan(&self, profile: &str) -> Plan {
        let mut data = BTreeMap::from([(
            GVDB.to_owned(),
            self.src.join(if profile == "a" { "dbA" } else { "dbB" }),
        )]);
        let mut loader = BTreeMap::from([
            ("version.dll".to_owned(), self.src.join("proxy")),
            (CORE.to_owned(), self.src.join("core")),
        ]);
        if profile == "b" {
            data.insert(UI.into(), self.src.join("uiB"));
            loader.insert(SCREEN.into(), self.src.join("screen"));
        }
        Plan {
            profile: profile.into(),
            data,
            added: BTreeMap::new(),
            loader,
            registry_override: true,
        }
    }

    fn built(&self, plan: &mut Plan) {
        let dir = self
            .data
            .join("cache")
            .join("overlay")
            .join(&plan.profile)
            .join("0123456789abcdef");
        for (rel, src) in plan.data.iter_mut() {
            let out = dir.join(rel);
            write(&out, &std::fs::read_to_string(&*src).unwrap());
            *src = out;
        }
    }

    fn snapshot(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        walk(&self.game, &self.game, &mut out);
        out.retain(|k, _| !k.starts_with("swofty-cache"));
        out.insert(
            "<user.reg>".into(),
            sha256_bytes(&std::fs::read(&self.reg).unwrap()),
        );
        out
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.game.join(rel)).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn write(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let rel = e
            .path()
            .strip_prefix(base)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if e.path().is_dir() {
            out.insert(format!("{rel}/"), "dir".into());
            walk(base, &e.path(), out);
        } else {
            out.insert(rel, sha256_bytes(&std::fs::read(e.path()).unwrap()));
        }
    }
}

#[test]
fn a_stock_file_missing_from_the_game_folder_goes_back_from_its_backup() {
    let f = Fixture::new("missing-backed-up");
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);
    inst.apply(&f.plan("b")).unwrap();
    inst.apply(&Plan::vanilla()).unwrap();
    std::fs::remove_dir_all(f.game.join("2025_asset_groups/ui_package")).unwrap();

    inst.apply(&f.plan("b")).unwrap();
    assert_eq!(f.read(UI), "ui-B");
    inst.apply(&Plan::vanilla()).unwrap();
    assert_eq!(f.read(UI), "ui-v1");
    assert_eq!(inst.verify().unwrap(), []);
}

#[test]
fn a_stock_file_missing_with_no_backup_stops_with_what_puts_it_back() {
    let f = Fixture::new("missing-no-backup");
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);
    inst.apply(&f.plan("b")).unwrap();
    inst.apply(&Plan::vanilla()).unwrap();
    std::fs::remove_dir_all(f.game.join("2025_asset_groups/ui_package")).unwrap();
    std::fs::remove_dir_all(f.data.join("baseline/objects")).unwrap();
    let before = f.snapshot();

    let err = inst.apply(&f.plan("b")).unwrap_err();
    assert!(
        matches!(&err, swofty_core::Error::MissingStock { files, version: None } if files == &[UI.to_owned()]),
        "{err}"
    );
    let text = err.to_string();
    assert!(
        text.starts_with(
            "flow_frontend.erp is missing from the game folder; nothing in the game folder was changed"
        ),
        "{text}"
    );
    assert!(
        text.contains("update F1 25 in Steam or the EA app, then verify"),
        "{text}"
    );
    assert!(text.contains("steam://validate/3059520"), "{text}");
    assert_eq!(f.snapshot(), before);
    assert!(Journal::open(&f.data).unwrap().is_empty());
}

#[test]
fn profile_switches_return_to_the_baseline() {
    let f = Fixture::new("switch");
    let original = f.snapshot();
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);

    inst.apply(&f.plan("a")).unwrap();
    assert_eq!(f.read(GVDB), "db-A");
    assert_eq!(f.read("version.dll"), "proxy");
    assert!(std::fs::read_to_string(&f.reg).unwrap().contains(OVERRIDE_HEADER));

    inst.apply(&f.plan("b")).unwrap();
    assert_eq!(f.read(GVDB), "db-B");
    assert_eq!(f.read(UI), "ui-B");
    assert_eq!(f.read(SCREEN), "screen");
    assert_eq!(Journal::open(&f.data).unwrap().len(), 1);

    inst.apply(&f.plan("a")).unwrap();
    assert!(!f.game.join(SCREEN).exists());
    assert_eq!(f.read(CORE), "core");

    inst.apply(&Plan::vanilla()).unwrap();
    assert_eq!(f.snapshot(), original);
    assert!(Journal::open(&f.data).unwrap().is_empty());
    assert_eq!(inst.verify().unwrap(), []);
    assert_eq!(inst.restore().unwrap(), []);
    assert_eq!(f.snapshot(), original);
}

#[test]
fn the_live_shader_config_comes_out_whatever_the_session_rewrote_it_to() {
    let f = Fixture::new("live-shader");
    let original = f.snapshot();
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);
    let live = swofty_core::shaders::LIVE_FILE;
    write(&f.src.join("active"), "{\"serial\": 1}");
    let mut plan = f.plan("a");
    plan.loader.insert(live.into(), f.src.join("active"));

    inst.apply(&plan).unwrap();
    write(&f.game.join(live), "{\"serial\": 2}");
    inst.apply(&Plan::vanilla()).unwrap();
    assert_eq!(
        f.snapshot(),
        original,
        "the rewritten live file is the launcher's own"
    );

    write(&f.game.join(live), "{\"serial\": 3}");
    inst.apply(&plan).unwrap();
    assert_eq!(
        f.read(live),
        "{\"serial\": 1}",
        "a leftover live file never blocks the next session"
    );
    inst.apply(&Plan::vanilla()).unwrap();
    assert_eq!(f.snapshot(), original);
}

#[test]
fn a_kill_between_any_two_steps_rolls_back_to_the_baseline() {
    let total = 8;
    for k in 0..=total {
        let f = Fixture::new(&format!("kill{k}"));
        let original = f.snapshot();
        let game = f.game("1");
        let mut inst = Installer::new(&f.data, &game);
        inst.stop_after = Some(k);
        let result = inst.apply(&f.plan("a"));
        assert_eq!(result.is_ok(), k == total, "k={k}");

        let open = Journal::open(&f.data).unwrap();
        assert_eq!(open.len(), 1, "k={k}");
        if let Some(e) = open[0].entries.get(k).filter(|e| e.state == State::Pending) {
            match e.op {
                Op::ReplaceFile | Op::CreateFile => write(&f.game.join(&e.path), "partial"),
                Op::CreateDir => std::fs::create_dir_all(f.game.join(&e.path)).unwrap(),
                Op::RegistryOverride => {
                    let mut t = std::fs::read_to_string(&f.reg).unwrap();
                    t.push_str(&format!("\n{OVERRIDE_HEADER}\n"));
                    std::fs::write(&f.reg, t).unwrap();
                }
            }
        }

        let restarted = Installer::new(&f.data, &game);
        assert_eq!(restarted.recover().unwrap(), [], "k={k}");
        assert_eq!(f.snapshot(), original, "k={k}");
        assert_eq!(restarted.verify().unwrap(), [], "k={k}");
    }
}

#[test]
fn an_install_the_free_space_cannot_hold_changes_nothing() {
    let f = Fixture::new("space");
    let original = f.snapshot();
    let game = f.game("1");
    let short = Installer {
        free_space: |_| Some(1),
        ..Installer::new(&f.data, &game)
    };
    let err = short.apply(&f.maps_plan()).unwrap_err();
    assert!(
        matches!(err, swofty_core::Error::NoSpace { free: 1, .. }),
        "{err}"
    );
    assert!(err.to_string().starts_with("not enough free space on "), "{err}");
    assert!(err.to_string().contains(" GB, 0.0 GB free"), "{err}");
    assert_eq!(f.snapshot(), original);
    assert!(Journal::open(&f.data).unwrap().is_empty());
    assert!(
        !f.data.join("baseline/objects").exists(),
        "no original was backed up"
    );

    let roomy = Installer {
        free_space: |_| Some(u64::MAX),
        ..Installer::new(&f.data, &game)
    };
    roomy.apply(&f.maps_plan()).unwrap();
    assert_eq!(f.read(UI), "ui-B");
}

#[test]
fn an_install_of_thousands_of_files_writes_the_journal_once() {
    let f = Fixture::new("many");
    let original = f.snapshot();
    let game = f.game("1");
    let mut plan = f.plan("a");
    for i in 0..10_000 {
        let src = f.src.join(format!("many/{i}"));
        write(&src, &i.to_string());
        plan.added
            .insert(format!("2025_asset_groups/many/{}/{i}.erp", i % 50), src);
    }
    let started = std::time::Instant::now();
    Installer::new(&f.data, &game).apply(&plan).unwrap();
    assert!(
        started.elapsed() < std::time::Duration::from_secs(60),
        "{:?}",
        started.elapsed()
    );
    let open = Journal::open(&f.data).unwrap();
    assert!(open[0].committed && open[0].entries.iter().all(|e| e.state == State::Done));
    assert_eq!(open[0].entries.len(), 8 + 10_000 + 51);
    assert_eq!(f.read("2025_asset_groups/many/7/9957.erp"), "9957");

    let mut stopped = Installer::new(&f.data, &game);
    stopped.stop_after = Some(5_000);
    assert!(stopped.apply(&plan).is_err());
    assert!(
        Journal::open(&f.data).unwrap()[0]
            .entries
            .iter()
            .all(|e| e.state == State::Pending)
    );
    assert_eq!(Installer::new(&f.data, &game).recover().unwrap(), []);
    assert_eq!(f.snapshot(), original);
}

#[test]
fn a_game_update_in_between_never_restores_old_originals() {
    let f = Fixture::new("update");
    let game1 = f.game("1");
    Installer::new(&f.data, &game1).apply(&f.plan("a")).unwrap();
    assert_eq!(f.read(GVDB), "db-A");

    for (rel, body) in [("F1_25.exe", "exe-v2"), (GVDB, "db-v2"), ("readme.txt", "r-v2")] {
        write(&f.game.join(rel), body);
    }
    let game2 = f.game("2");
    let inst = Installer::new(&f.data, &game2);
    let report = inst.restore().unwrap();
    assert!(report.contains(&Drift::LeftInPlace(GVDB.into())), "{report:?}");
    assert_eq!(f.read(GVDB), "db-v2");
    assert!(!f.game.join("version.dll").exists());
    assert!(!f.game.join("swofty").exists());

    let updated = f.snapshot();
    assert_eq!(updated["<user.reg>"], sha256_bytes(REG.as_bytes()));

    inst.apply(&f.plan("a")).unwrap();
    assert_eq!(f.read(GVDB), "db-A");
    assert_eq!(
        inst.baseline().unwrap().files[GVDB].sha256,
        sha256_bytes(b"db-v2")
    );
    inst.apply(&Plan::vanilla()).unwrap();
    assert_eq!(f.snapshot(), updated);
    assert_eq!(inst.verify().unwrap(), []);
}

#[test]
fn an_empty_loader_folder_is_not_drift() {
    let f = Fixture::new("empty-loader");
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);
    inst.apply(&f.plan("a")).unwrap();
    inst.apply(&Plan::vanilla()).unwrap();
    assert!(!f.game.join("swofty").exists());

    std::fs::create_dir_all(f.game.join("swofty")).unwrap();
    assert_eq!(inst.verify().unwrap(), []);
}

#[test]
fn a_loader_folder_of_empty_folders_is_not_drift() {
    let f = Fixture::new("empty-loader-dirs");
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);
    inst.apply(&f.plan("a")).unwrap();
    inst.apply(&Plan::vanilla()).unwrap();

    for dir in [
        "swofty/addons/core",
        "swofty/addons/t.empty",
        "swofty/tt/replays/inbox",
    ] {
        std::fs::create_dir_all(f.game.join(dir)).unwrap();
    }
    assert_eq!(inst.verify().unwrap(), []);
}

#[test]
fn a_loader_folder_holding_a_file_is_drift_named_by_the_file() {
    let f = Fixture::new("loader-with-file");
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);
    inst.apply(&f.plan("a")).unwrap();
    inst.apply(&Plan::vanilla()).unwrap();

    std::fs::create_dir_all(f.game.join("swofty/addons/t.empty")).unwrap();
    write(&f.game.join("swofty/tt/inbox/1.json"), "{}");
    assert_eq!(
        inst.verify().unwrap(),
        [Drift::Addition("swofty/tt/inbox/1.json".into())]
    );
    assert_eq!(
        Drift::Addition("swofty/tt/inbox/1.json".into()).to_string(),
        "swofty/tt/inbox/1.json is not part of the game"
    );

    assert_eq!(inst.restore().unwrap(), []);
    assert!(!f.game.join("swofty").exists());
}

#[cfg(unix)]
#[test]
fn a_restore_that_leaves_an_empty_loader_folder_in_place_reports_nothing_left() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new("loader-left-in-place");
    let original = f.snapshot();
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);
    inst.apply(&f.plan("a")).unwrap();
    inst.apply(&Plan::vanilla()).unwrap();

    let loader = f.game.join("swofty");
    std::fs::create_dir_all(loader.join("addons")).unwrap();
    std::fs::set_permissions(&loader, std::fs::Permissions::from_mode(0o555)).unwrap();
    let restored = inst.restore();
    std::fs::set_permissions(&loader, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(restored.unwrap(), []);
    let mut left = f.snapshot();
    left.retain(|k, _| !k.starts_with("swofty/"));
    assert_eq!(left, original);
}

#[test]
fn drift_from_outside_is_reported_and_restored() {
    let f = Fixture::new("drift");
    let original = f.snapshot();
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);
    inst.apply(&f.plan("a")).unwrap();
    inst.apply(&Plan::vanilla()).unwrap();

    write(&f.game.join(GVDB), "edited by another tool");
    write(&f.game.join("version.dll"), "someone else's proxy");
    let drift = inst.verify().unwrap();
    assert!(drift.contains(&Drift::Changed(GVDB.into())), "{drift:?}");
    assert!(
        drift.contains(&Drift::Addition("version.dll".into())),
        "{drift:?}"
    );
    assert!(inst.apply(&f.plan("a")).is_err());

    assert_eq!(inst.restore().unwrap(), []);
    assert_eq!(f.snapshot(), original);
}

#[test]
fn a_restore_sets_aside_a_version_dll_swofty_did_not_place() {
    let f = Fixture::new("foreign-proxy");
    let original = f.snapshot();
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);
    inst.apply(&f.plan("a")).unwrap();
    inst.apply(&Plan::vanilla()).unwrap();
    write(&f.game.join("version.dll"), "an asi loader");

    let done = recovery::restore(&f.data, &game).unwrap();
    assert_eq!(done.left, []);
    assert_eq!(done.set_aside.len(), 1, "{:?}", done.set_aside);
    assert!(done.set_aside[0].starts_with(recovery::set_aside_dir(&f.data)));
    assert!(done.set_aside[0].ends_with("version.dll"));
    assert_eq!(
        std::fs::read_to_string(&done.set_aside[0]).unwrap(),
        "an asi loader"
    );
    assert_eq!(f.snapshot(), original);

    // One put over the launcher's own proxy mid-session is kept the same way.
    inst.apply(&f.plan("a")).unwrap();
    write(&f.game.join("version.dll"), "an upscaler renamed");
    let done = recovery::restore(&f.data, &game).unwrap();
    assert_eq!(done.left, []);
    let kept: Vec<String> = done
        .set_aside
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect();
    assert_eq!(kept, ["an upscaler renamed"]);
    assert_eq!(f.snapshot(), original);
}

#[test]
fn a_restore_removes_the_proxy_swofty_ships_without_setting_it_aside() {
    let f = Fixture::new("shipped-proxy");
    let loader = f.root.join("loader");
    write(&loader.join("version.dll"), "proxy");
    write(&loader.join(swofty_core::builtin::CORE_DLL), "core");
    let _pin = swofty_core::builtin::LoaderPin::to(loader);
    let original = f.snapshot();
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);
    inst.apply(&f.plan("a")).unwrap();
    inst.apply(&Plan::vanilla()).unwrap();
    write(&f.game.join("version.dll"), "proxy");

    let done = recovery::restore(&f.data, &game).unwrap();
    assert_eq!(done.left, []);
    assert!(done.set_aside.is_empty(), "{:?}", done.set_aside);
    assert!(!recovery::set_aside_dir(&f.data).exists());
    assert_eq!(f.snapshot(), original);
}

#[test]
fn a_restore_leaves_an_upscaler_drop_in_alone() {
    let f = Fixture::new("upscaler");
    write(&f.game.join("amd_fidelityfx_dx12.dll"), "steam's loader");
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);
    inst.apply(&f.plan("a")).unwrap();
    write(&f.game.join("amd_fidelityfx_dx12.dll"), "a drop-in fsr 4 loader");
    assert_eq!(inst.restore().unwrap(), []);
    assert_eq!(f.read("amd_fidelityfx_dx12.dll"), "a drop-in fsr 4 loader");
    inst.apply(&f.plan("b")).unwrap();
    inst.apply(&Plan::vanilla()).unwrap();
    assert_eq!(f.read("amd_fidelityfx_dx12.dll"), "a drop-in fsr 4 loader");
    assert_eq!(inst.verify().unwrap(), []);
}

#[test]
fn a_plan_cannot_replace_an_upscaler_drop_in() {
    let f = Fixture::new("upscaler-plan");
    write(&f.game.join("amd_fidelityfx_dx12.dll"), "a drop-in fsr 4 loader");
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);
    let mut plan = f.plan("a");
    plan.data
        .insert("amd_fidelityfx_dx12.dll".into(), f.src.join("dbB"));
    let err = inst.apply(&plan).unwrap_err();
    assert!(err.to_string().contains("upscaler drop-in"), "{err}");
    assert_eq!(f.read("amd_fidelityfx_dx12.dll"), "a drop-in fsr 4 loader");
    let mut plan = f.plan("a");
    plan.added
        .insert("AMD_FidelityFX_DX12.dll".into(), f.src.join("dbB"));
    assert!(inst.apply(&plan).is_err());
    assert_eq!(f.read("amd_fidelityfx_dx12.dll"), "a drop-in fsr 4 loader");
}

#[test]
fn map_files_in_new_folders_are_removed_by_switches_and_restore() {
    let f = Fixture::new("maps");
    let original = f.snapshot();
    let game = f.game("1");
    let inst = Installer::new(&f.data, &game);

    inst.apply(&f.maps_plan()).unwrap();
    for (i, rel) in MAP_FILES.iter().enumerate() {
        assert_eq!(f.read(rel), format!("map-{i}"));
    }
    assert_eq!(f.read(STOCK_TRACK), "track-v1");

    inst.apply(&f.plan("a")).unwrap();
    assert!(!f.game.join("asset_groups").exists());
    assert!(
        !f.game
            .join("2025_asset_groups/environment_package/tracks/okayama")
            .exists()
    );
    assert!(!f.game.join("2025_asset_groups/ui_package/textures").exists());
    assert!(f.game.join(STOCK_TRACK).is_file());

    inst.apply(&f.maps_plan()).unwrap();
    assert_eq!(inst.restore().unwrap(), []);
    assert_eq!(f.snapshot(), original);

    inst.apply(&f.maps_plan()).unwrap();
    inst.apply(&Plan::vanilla()).unwrap();
    assert_eq!(f.snapshot(), original);
    assert_eq!(inst.verify().unwrap(), []);
}

#[test]
fn a_kill_while_installing_maps_rolls_back_to_the_baseline() {
    let total = {
        let f = Fixture::new("mapcount");
        let game = f.game("1");
        Installer::new(&f.data, &game).apply(&f.maps_plan()).unwrap();
        Journal::open(&f.data).unwrap()[0].entries.len()
    };
    for k in 0..=total {
        let f = Fixture::new(&format!("mapkill{k}"));
        let original = f.snapshot();
        let game = f.game("1");
        let mut inst = Installer::new(&f.data, &game);
        inst.stop_after = Some(k);
        assert_eq!(inst.apply(&f.maps_plan()).is_ok(), k == total, "k={k}");
        let open = Journal::open(&f.data).unwrap();
        if let Some(e) = open[0].entries.get(k).filter(|e| e.state == State::Pending) {
            match e.op {
                Op::ReplaceFile | Op::CreateFile => write(&f.game.join(&e.path), "partial"),
                Op::CreateDir => std::fs::create_dir_all(f.game.join(&e.path)).unwrap(),
                Op::RegistryOverride => {}
            }
        }
        let restarted = Installer::new(&f.data, &game);
        assert_eq!(restarted.recover().unwrap(), [], "k={k}");
        assert_eq!(f.snapshot(), original, "k={k}");
        assert_eq!(restarted.verify().unwrap(), [], "k={k}");
    }
}

fn logged() -> (std::cell::RefCell<Vec<String>>,) {
    (std::cell::RefCell::new(Vec::new()),)
}

#[cfg(unix)]
fn inode(path: &Path) -> u64 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).unwrap().ino()
}

#[cfg(unix)]
#[test]
fn build_outputs_on_the_game_volume_are_linked_and_the_rest_copied() {
    let f = Fixture::new("link-same");
    let game = f.game("1");
    let mut plan = f.plan("b");
    f.built(&mut plan);
    let (lines,) = logged();
    let out = |l: &str| lines.borrow_mut().push(l.to_owned());
    let inst = Installer {
        out: &out,
        copy_only: false,
        ..Installer::new(&f.data, &game)
    };
    inst.apply(&plan).unwrap();
    assert_eq!(f.read(GVDB), "db-B");
    assert_eq!(f.read(UI), "ui-B");
    assert_eq!(inode(&f.game.join(GVDB)), inode(&plan.data[GVDB]));
    assert_eq!(inode(&f.game.join(UI)), inode(&plan.data[UI]));
    assert_ne!(inode(&f.game.join(CORE)), inode(&f.src.join("core")));
    assert_eq!(
        inode(&f.game.join(CORE)),
        inode(&f.game.join("swofty-cache/overlay").join(CORE))
    );
    assert!(
        lines.borrow().iter().any(
            |l| l == "installing: 6 linked (4 from the link cache: 4 copied into it, 0 reused), 0 copied"
        ),
        "{:?}",
        lines.borrow()
    );
}

#[cfg(unix)]
#[test]
fn rolling_back_a_linked_install_restores_stock_and_leaves_the_build_output() {
    let f = Fixture::new("link-rollback");
    let game = f.game("1");
    let original = f.snapshot();
    let mut plan = f.plan("b");
    f.built(&mut plan);
    let inst = Installer::new(&f.data, &game);
    inst.apply(&plan).unwrap();
    assert_eq!(inode(&f.game.join(GVDB)), inode(&plan.data[GVDB]));

    inst.apply(&Plan::vanilla()).unwrap();
    assert_eq!(f.snapshot(), original);
    assert_eq!(f.read(GVDB), "db-v1");
    assert_eq!(f.read(UI), "ui-v1");
    assert_eq!(std::fs::read_to_string(&plan.data[GVDB]).unwrap(), "db-B");
    assert_eq!(std::fs::read_to_string(&plan.data[UI]).unwrap(), "ui-B");
    assert_ne!(inode(&f.game.join(GVDB)), inode(&plan.data[GVDB]));
    assert_eq!(inst.verify().unwrap(), []);

    inst.apply(&plan).unwrap();
    assert_eq!(f.read(GVDB), "db-B");
}

#[test]
fn copy_only_and_sources_outside_the_build_cache_are_copied() {
    let f = Fixture::new("link-copy");
    let game = f.game("1");
    let mut plan = f.plan("b");
    f.built(&mut plan);
    let (lines,) = logged();
    let out = |l: &str| lines.borrow_mut().push(l.to_owned());
    let inst = Installer {
        out: &out,
        copy_only: true,
        ..Installer::new(&f.data, &game)
    };
    inst.apply(&plan).unwrap();
    assert_eq!(f.read(GVDB), "db-B");
    #[cfg(unix)]
    assert_ne!(inode(&f.game.join(GVDB)), inode(&plan.data[GVDB]));
    assert!(
        lines
            .borrow()
            .iter()
            .any(|l| l == "installing: copy (SWOFTY_INSTALL_COPY=1); 0 file(s) linked, 6 copied"),
        "{:?}",
        lines.borrow()
    );
    inst.apply(&Plan::vanilla()).unwrap();

    let elsewhere = Installer {
        copy_only: false,
        out: &out,
        ..Installer::new(&f.data, &game)
    };
    elsewhere.apply(&f.plan("b")).unwrap();
    assert!(
        lines.borrow().iter().any(
            |l| l == "installing: 6 linked (6 from the link cache: 6 copied into it, 0 reused), 0 copied"
        ),
        "{:?}",
        lines.borrow()
    );
}

#[cfg(target_os = "linux")]
#[test]
fn a_build_output_on_another_volume_than_the_game_is_copied() {
    use std::os::unix::fs::MetadataExt;
    let shm = PathBuf::from("/dev/shm");
    let here = std::env::temp_dir();
    if !shm.is_dir() || std::fs::metadata(&shm).unwrap().dev() == std::fs::metadata(&here).unwrap().dev() {
        return;
    }
    let game_dir = shm.join(format!("swofty-link-game-{}", std::process::id()));
    let f = Fixture::at("link-cross", Some(game_dir.clone()));
    let game = f.game("1");
    let mut plan = f.plan("b");
    f.built(&mut plan);
    let (lines,) = logged();
    let out = |l: &str| lines.borrow_mut().push(l.to_owned());
    let inst = Installer {
        out: &out,
        copy_only: false,
        ..Installer::new(&f.data, &game)
    };
    let applied = inst.apply(&plan);
    let read = f.read(GVDB);
    inst.apply(&Plan::vanilla()).unwrap();
    std::fs::remove_dir_all(&game_dir).ok();
    applied.unwrap();
    assert_eq!(read, "db-B");
    assert!(
        lines.borrow().iter().any(
            |l| l == "installing: 6 linked (6 from the link cache: 6 copied into it, 0 reused), 0 copied"
        ),
        "{:?}",
        lines.borrow()
    );
    assert_eq!(std::fs::read_to_string(&plan.data[GVDB]).unwrap(), "db-B");
}

fn two_drives(p: &Path) -> Option<swofty_core::disk::Volume> {
    let id = if p.components().any(|c| c.as_os_str() == "overlay") {
        "data"
    } else {
        "game"
    };
    Some(swofty_core::disk::Volume {
        id: id.into(),
        name: id.into(),
    })
}

#[cfg(unix)]
fn one_drive(_: &Path) -> Option<swofty_core::disk::Volume> {
    two_drives(Path::new("elsewhere"))
}

#[cfg(unix)]
#[test]
fn another_drive_links_the_game_folder_to_a_cache_that_survives_a_restore() {
    let f = Fixture::new("cache-links");
    let game = f.game("1");
    let original = f.snapshot();
    let mut plan = f.plan("b");
    f.built(&mut plan);
    let (lines,) = logged();
    let out = |l: &str| lines.borrow_mut().push(l.to_owned());
    let inst = Installer {
        out: &out,
        volume: two_drives,
        ..Installer::new(&f.data, &game)
    };
    inst.apply(&plan).unwrap();
    let cached = f.game.join("swofty-cache/overlay").join(GVDB);
    assert_eq!(f.read(GVDB), "db-B");
    assert_eq!(inode(&f.game.join(GVDB)), inode(&cached));
    assert_ne!(inode(&f.game.join(GVDB)), inode(&plan.data[GVDB]));
    assert!(
        lines.borrow().iter().any(
            |l| l == "installing: 6 linked (6 from the link cache: 6 copied into it, 0 reused), 0 copied"
        ),
        "{:?}",
        lines.borrow()
    );

    inst.apply(&Plan::vanilla()).unwrap();
    assert_eq!(f.snapshot(), original);
    assert_eq!(f.read(GVDB), "db-v1");
    assert_eq!(std::fs::read_to_string(&cached).unwrap(), "db-B");
    assert_eq!(inst.verify().unwrap(), []);

    inst.apply(&plan).unwrap();
    assert_eq!(f.read(GVDB), "db-B");
    assert!(
        lines.borrow().iter().any(
            |l| l == "installing: 6 linked (6 from the link cache: 1 copied into it, 5 reused), 0 copied"
        ),
        "{:?}",
        lines.borrow()
    );
}

#[test]
fn a_game_drive_without_room_for_the_cache_gets_direct_copies() {
    let f = Fixture::new("cache-space");
    let game = f.game("1");
    let mut plan = f.plan("b");
    f.built(&mut plan);
    let (lines,) = logged();
    let out = |l: &str| lines.borrow_mut().push(l.to_owned());
    let inst = Installer {
        out: &out,
        volume: two_drives,
        free_space: |_| Some(1 << 20),
        ..Installer::new(&f.data, &game)
    };
    inst.apply(&plan).unwrap();
    assert_eq!(f.read(GVDB), "db-B");
    assert!(
        !f.game
            .join("swofty-cache/overlay")
            .join("0123456789abcdef")
            .exists()
    );
    let lines = lines.borrow();
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("installing: link cache unavailable (the game drive has")),
        "{lines:?}"
    );
    assert!(!f.game.join("swofty-cache/overlay").exists());
}

#[cfg(unix)]
#[test]
fn loose_addon_files_go_through_the_cache_on_one_drive_and_a_second_play_reuses_them() {
    let f = Fixture::new("cache-same");
    let game = f.game("1");
    let mut plan = f.plan("b");
    f.built(&mut plan);
    let (lines,) = logged();
    let out = |l: &str| lines.borrow_mut().push(l.to_owned());
    let inst = Installer {
        out: &out,
        volume: one_drive,
        ..Installer::new(&f.data, &game)
    };
    inst.apply(&plan).unwrap();
    assert_eq!(inode(&f.game.join(GVDB)), inode(&plan.data[GVDB]));
    let cached = f.game.join("swofty-cache/overlay").join(CORE);
    assert_eq!(inode(&f.game.join(CORE)), inode(&cached));
    assert_ne!(inode(&f.game.join(CORE)), inode(&f.src.join("core")));
    assert!(!f.game.join("swofty-cache/overlay").join(GVDB).exists());
    inst.apply(&Plan::vanilla()).unwrap();
    assert!(cached.is_file());
    inst.apply(&plan).unwrap();
    let lines = lines.borrow();
    assert!(
        lines.iter().any(
            |l| l == "installing: 6 linked (4 from the link cache: 4 copied into it, 0 reused), 0 copied"
        ),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(
            |l| l == "installing: 6 linked (4 from the link cache: 1 copied into it, 3 reused), 0 copied"
        ),
        "{lines:?}"
    );
}

#[cfg(unix)]
#[test]
fn the_copy_fallback_never_writes_through_a_hard_link_in_the_game_folder() {
    let f = Fixture::new("copy-through");
    let game = f.game("1");
    let cache = f.src.join("cache-twin");
    std::fs::hard_link(f.game.join(GVDB), &cache).unwrap();
    let mut plan = f.plan("b");
    f.built(&mut plan);
    let inst = Installer {
        copy_only: true,
        ..Installer::new(&f.data, &game)
    };
    inst.apply(&plan).unwrap();
    assert_eq!(f.read(GVDB), "db-B");
    assert_eq!(std::fs::read_to_string(&cache).unwrap(), "db-v1");
    assert_ne!(inode(&f.game.join(GVDB)), inode(&cache));
    inst.apply(&Plan::vanilla()).unwrap();
    assert_eq!(f.read(GVDB), "db-v1");
    assert_eq!(std::fs::read_to_string(&cache).unwrap(), "db-v1");
}
