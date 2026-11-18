use std::path::PathBuf;

use swofty_core::vault::careers::{Brief, Row, Status};
use swofty_core::vault::{self, Vault};

use swofty_core::launcher_saves::{self, Marker};

use super::{
    Banner, Missing, SavesPage, awaiting_import, banner_from, build_saves, list, reason_label, steam_save_in,
    steam_save_missing, whose,
};

#[test]
fn a_saves_view_builds_without_the_shell_so_it_can_run_off_the_ui_thread() {
    fn is_send<T: Send>() {}
    is_send::<super::Built>();
    let root = std::env::temp_dir().join(format!("swofty-build-saves-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(&root).unwrap();
    let store = swofty_core::store::Store::new(&root);
    let built = build_saves(&store, None, "Race", false);
    assert_eq!(built.view.profile, "Race");
    std::fs::remove_dir_all(&root).ok();
}

fn synthetic_vault(root: &std::path::Path, profile: &str, snapshots: usize) {
    let index = swofty_core::stdlib::careers::data_path(root);
    std::fs::create_dir_all(index.parent().unwrap()).unwrap();
    let loc = vault::Location::launcher(root, profile);
    std::fs::create_dir_all(&loc.dir).unwrap();
    let v = Vault::of(root, &loc).unwrap();
    for n in 0..snapshots {
        let careers: Vec<String> = (0..400 + n)
            .map(|c| {
                format!(
                    r#"{{"career":"driver_career:s{c}","profile":"{profile}","slot":"s{c}","status":"present","mode":"driver_career","driver":"Driver {c}","team":"Team {c}","driver_raw":{c},"team_raw":{c},"slot_season":3,"slot_weekend":4,"last_played":1761600000,"slot_name":"Slot {c}",{}}}"#,
                    (0..40).map(|f| format!(r#""field_{f}":"value {f} of career {c}""#)).collect::<Vec<_>>().join(",")
                )
            })
            .collect();
        std::fs::write(
            &index,
            format!(r#"{{"version":1,"careers":[{}]}}"#, careers.join(",")),
        )
        .unwrap();
        std::fs::write(
            loc.dir.join("savegame@profile#NXDSMWW.BWW"),
            vec![n as u8; 4096 + n],
        )
        .unwrap();
        v.snapshot(
            &loc,
            vault::Meta::new(vault::Reason::AfterSession),
            1_793_404_800 + n as i64 * 600,
        )
        .unwrap();
    }
}

#[test]
fn a_hidden_or_unchanged_saves_page_is_not_refreshed() {
    let mut page = SavesPage::default();
    assert!(page.refresh_due(true, Some(1)));
    assert!(!page.refresh_due(false, Some(1)));
    assert!(!page.refresh_due(true, None));
    page.print = Some(1);
    assert!(!page.refresh_due(true, Some(1)));
    assert!(page.refresh_due(true, Some(2)));
    page.loading = true;
    assert!(!page.refresh_due(true, Some(2)));
}

#[test]
fn the_print_follows_the_vault_and_the_save_folder_and_nothing_else() {
    let root = std::env::temp_dir().join(format!("swofty-print-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(&root).unwrap();
    synthetic_vault(&root, "Race", 2);
    let print = |game| super::saves_print(&root, "Race", game);
    let first = print(false);
    assert_eq!(first, print(false));
    assert_ne!(first, print(true));
    assert_eq!(print(false), super::saves_print(&root, "Race", false));
    let loc = vault::Location::launcher(&root, "Race");
    std::fs::write(loc.dir.join("savegame@profile#NXDSMWW.BWW"), vec![9u8; 7000]).unwrap();
    let changed = print(false);
    assert_ne!(first, changed);
    let v = Vault::of(&root, &loc).unwrap();
    v.snapshot(&loc, vault::Meta::new(vault::Reason::Manual), 1_793_500_000)
        .unwrap();
    assert_ne!(changed, print(false));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn snapshot_details_are_cached_until_the_index_or_the_current_careers_change() {
    let root = std::env::temp_dir().join(format!("swofty-details-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(&root).unwrap();
    synthetic_vault(&root, "Race", 3);
    let loc = vault::Location::launcher(&root, "Race");
    let v = Vault::of(&root, &loc).unwrap();
    let snaps = v.list().unwrap();
    let now = swofty_core::stdlib::careers::data_path(&root);
    let cache = std::sync::Mutex::new(super::DetailCache::default());
    let first = super::details(&v, &snaps, &now, &cache);
    assert_eq!(first.len(), 3);
    assert_eq!(cache.lock().unwrap().0.len(), 3);

    let index = v.dir.join(snaps[0].careers_ref.as_deref().unwrap());
    let held = first[0].held.clone();
    std::fs::write(
        &index,
        r#"{"careers":[{"career":"x:1","slot":"1","status":"present"}]}"#,
    )
    .unwrap();
    let again = super::details(&v, &snaps, &now, &cache);
    assert_ne!(again[0].held, held, "a changed index file is read again");
    assert_eq!(again[1].held, first[1].held, "the others stay cached");

    let before = again[2].diff.clone();
    std::fs::write(
        &now,
        r#"{"careers":[{"career":"y:2","slot":"2","status":"present"}]}"#,
    )
    .unwrap();
    let after = super::details(&v, &snaps, &now, &cache);
    assert_ne!(
        after[2].diff, before,
        "a changed current index recomputes every diff"
    );

    let fewer = super::details(&v, &snaps[..1], &now, &cache);
    assert_eq!(fewer.len(), 1);
    assert_eq!(
        cache.lock().unwrap().0.len(),
        1,
        "snapshots that are gone leave the cache"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
#[ignore]
fn bench_saves_page_with_25_snapshots() {
    let root = std::path::PathBuf::from(
        std::env::var("SAVES_BENCH_DIR").unwrap_or_else(|_| "/mnt/work/.saves-perf-bench".into()),
    );
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(&root).unwrap();
    synthetic_vault(&root, "Race", 25);
    let store = swofty_core::store::Store::new(&root);
    for round in 0..3 {
        let started = std::time::Instant::now();
        let built = build_saves(&store, None, "Race", false);
        assert_eq!(built.view.versions.len(), 25);
        eprintln!("bench round {round}: {} ms", started.elapsed().as_millis());
    }
}

#[test]
fn an_empty_launcher_save_with_a_steam_save_waits_for_an_import() {
    assert!(awaiting_import(false, false, true));
    assert!(!awaiting_import(true, false, true));
    assert!(!awaiting_import(false, true, true));
    assert!(!awaiting_import(false, false, false));
}

#[test]
fn a_missed_steam_save_is_told_apart_from_an_empty_one() {
    assert!(steam_save_missing(false, false, false, true));
    assert!(!steam_save_missing(false, false, false, false));
    assert!(!steam_save_missing(false, false, true, true));
    assert!(!steam_save_missing(false, true, false, true));
    assert!(!steam_save_missing(true, false, false, true));
}

#[test]
fn a_picked_folder_may_be_remote_the_app_folder_or_the_steam_root() {
    let base = std::env::temp_dir().join(format!("swofty-pick-{}", std::process::id()));
    let remote = base.join("Steam Ünï/userdata/843502855/3059520/remote");
    std::fs::create_dir_all(&remote).unwrap();
    assert!(steam_save_in(&remote).is_none());
    std::fs::write(remote.join("savegame@profile#X"), b"x").unwrap();
    for pick in [
        remote.clone(),
        remote.parent().unwrap().to_path_buf(),
        base.join("Steam Ünï/userdata"),
        base.join("Steam Ünï"),
    ] {
        assert_eq!(
            steam_save_in(&pick).map(|l| l.dir),
            Some(remote.clone()),
            "{pick:?}"
        );
    }
    assert!(steam_save_in(&base).is_none());
    std::fs::remove_dir_all(&base).ok();
}

fn row(id: &str, status: Status, at: i64) -> Row {
    Row {
        career: id.into(),
        mode: None,
        label: format!("{id} label"),
        driver: None,
        team: None,
        season: Some(2016),
        round: Some(7),
        profile: None,
        addons: Vec::new(),
        first_seen: "a".into(),
        first_seen_at: 1,
        last_seen: format!("snap-{at}"),
        last_seen_at: at,
        last_played: None,
        status,
        found_at: None,
        dismissed: false,
    }
}

fn data(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("swofty-launcher-saves-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(d.join("career-vault/steam-1")).unwrap();
    d
}

#[test]
fn the_banner_names_an_addon_career_a_start_outside_the_launcher_probably_removed() {
    let d = std::env::temp_dir().join(format!("swofty-launcher-saves-lost-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    let remote = d
        .join("steam")
        .join("userdata")
        .join("843502855")
        .join("3059520")
        .join("remote");
    std::fs::create_dir_all(&remote).unwrap();
    std::fs::write(remote.join("savegame@profile#NXDSMWW.BWW"), vec![1u8; 4000]).unwrap();
    let index = swofty_core::stdlib::careers::data_path(&d);
    std::fs::create_dir_all(index.parent().unwrap()).unwrap();
    std::fs::write(
        &index,
        r#"{"careers":[{"career":"progressive:7f","slot":"7f","status":"present","mode":"progressive","label":"Progressive 2016","addons":[{"id":"swofty.progressive"}]}]}"#,
    )
    .unwrap();
    let loc = vault::Location::steam(&remote).unwrap();
    let v = Vault::of(&d, &loc).unwrap();
    let meta = |r| vault::Meta::new(r);
    v.snapshot(&loc, meta(vault::Reason::AfterSession), 1_000)
        .unwrap();
    std::fs::write(remote.join("savegame@profile#NXDSMWW.BWW"), vec![2u8; 900]).unwrap();
    v.snapshot(&loc, meta(vault::Reason::ForeignExit), 2_000).unwrap();
    match banner_from(&d, loc.source.clone(), "p".into()) {
        Some(Banner::Lost(Missing {
            names,
            kept_at,
            source,
            ..
        })) => {
            assert_eq!(names, ["Progressive 2016"]);
            assert_eq!(kept_at, 1_000);
            assert_eq!(source, loc.source);
        }
        _ => panic!("expected a lost banner"),
    }
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn the_banner_names_the_missing_careers_and_the_version_to_restore() {
    let d = data("drop");
    let table = vault::careers::Careers {
        version: 1,
        careers: vec![
            row("a", Status::MissingSince("later".into()), 10),
            row("b", Status::MissingSince("later".into()), 20),
            row("c", Status::Present, 30),
        ],
    };
    table.write(&d.join("career-vault/steam-1")).unwrap();
    match banner_from(&d, "steam-1".into(), "vanilla".into()) {
        Some(Banner::Dropped(Missing {
            snapshot,
            names,
            kept_at,
            profile,
            ..
        })) => {
            assert_eq!(profile, "vanilla");
            assert_eq!(snapshot, "snap-20");
            assert_eq!(kept_at, 20);
            assert_eq!(
                names,
                ["a label, season 2016, round 7", "b label, season 2016, round 7"]
            );
        }
        _ => panic!("expected a drop banner"),
    }
    Vault::open(&d, "steam-1").unwrap().dismiss_drop().unwrap();
    assert!(banner_from(&d, "steam-1".into(), "vanilla".into()).is_none());
}

#[test]
fn no_careers_and_no_notice_means_no_banner() {
    let d = data("none");
    assert!(banner_from(&d, "steam-1".into(), "vanilla".into()).is_none());
}

#[test]
fn mode_names_read_like_the_game() {
    use swofty_core::vault::careers::mode_label;
    assert_eq!(mode_label(Some("driver_career")), "Driver Career");
    assert_eq!(mode_label(Some("my_team")), "My Team");
    assert_eq!(mode_label(Some("progressive")), "Progressive");
    assert_eq!(mode_label(Some("sprint_cup")), "Sprint Cup");
    assert_eq!(mode_label(None), "Career");
}

#[test]
fn versions_and_places_read_short() {
    use super::{careers_held_line, season_round};
    use swofty_core::vault::careers::listing;
    assert_eq!(careers_held_line(10, None, 0), "10 careers");
    assert_eq!(
        careers_held_line(1, Some("Vettel"), 0),
        "1 career, newest: Vettel"
    );
    assert_eq!(
        careers_held_line(3, Some("Vettel"), 2),
        "3 careers, newest: Vettel, 2 marked missing"
    );
    assert_eq!(careers_held_line(0, None, 0), "0 careers");
    let l = listing(
        &swofty_core::stdlib::careers::parse(
            r#"{"careers":[{"career":"f1_career_2026","slot":"0","slot_season":1,"slot_weekend":2}]}"#,
        ),
        None,
    );
    assert_eq!(season_round(&l[0]).as_deref(), Some("Season 1 · Round 3"));
    assert_eq!(l[0].title(), "Driver Career · 2026 season");
}

#[test]
fn lists_name_the_place() {
    let b = |n: &str, p: Option<&str>| Brief {
        career: n.into(),
        name: n.into(),
        place: p.map(str::to_owned),
    };
    assert_eq!(
        list(&[b("A", Some("season 2016, round 7")), b("B", None)]),
        "A (season 2016, round 7), B"
    );
}

fn marker(profiles: &[&str], seen: bool) -> Marker {
    Marker {
        at: 1,
        launcher: "1.7.0".into(),
        profiles: profiles.iter().map(|p| p.to_string()).collect(),
        note: String::new(),
        seen,
    }
}

#[test]
fn the_first_start_notice_shows_once_and_is_remembered() {
    let d = data("notice");
    let mut page = SavesPage::default();
    assert!(!page.take_notice(&d));
    swofty_core::write_json(&launcher_saves::marker_path(&d), &marker(&["default"], false)).unwrap();
    assert!(page.take_notice(&d));
    assert!(!page.take_notice(&d));
    assert!(launcher_saves::marker(&d).unwrap().seen);
    assert!(!SavesPage::default().take_notice(&d));
}

#[test]
fn no_notice_when_the_import_copied_nothing_or_was_seen() {
    let d = data("notice-none");
    swofty_core::write_json(&launcher_saves::marker_path(&d), &marker(&[], false)).unwrap();
    assert!(!SavesPage::default().take_notice(&d));
    swofty_core::write_json(&launcher_saves::marker_path(&d), &marker(&["default"], true)).unwrap();
    assert!(!SavesPage::default().take_notice(&d));
}

#[test]
fn versions_read_in_plain_words() {
    use swofty_core::vault::Reason;
    assert_eq!(reason_label(Reason::BeforeSession), "Before Play");
    assert_eq!(reason_label(Reason::Migrated), "Old backup");
    assert_eq!(reason_label(Reason::Imported), "Imported from Steam");
    assert_eq!(whose("vanilla"), "your Steam save");
    assert_eq!(whose("default"), "profile default's save");
}

#[test]
fn an_imported_save_with_no_listing_gets_a_titled_placeholder() {
    use swofty_core::vault::{Meta, Reason, Taken};
    let d = data("imported");
    let loc = swofty_core::vault::Location::launcher(&d, "default");
    std::fs::create_dir_all(&loc.dir).unwrap();
    std::fs::write(loc.dir.join("savegame@profile#NXDSMWW.BWW"), [1u8; 64]).unwrap();
    std::fs::write(loc.dir.join("savegame@profile#QKRHMYXE"), [2u8; 16]).unwrap();
    let v = Vault::of(&d, &loc).unwrap();
    let note = format!("copied from {}", d.join("saves/progressive/remote").display());
    let Taken::New(_) = v
        .snapshot(&loc, Meta::new(Reason::ImportedProfile).note(note), 700)
        .unwrap()
    else {
        panic!()
    };
    let i = super::imported_from(&v.list().unwrap());
    assert_eq!(i.title, "Copied from progressive");
    assert_eq!(i.at, Some(700));
    assert_eq!(super::imported_from(&[]).title, "Imported Steam save");
}

#[test]
fn the_fixed_columns_leave_room_at_the_narrowest_window() {
    let content = 960. - 220. - 2. * 28.;
    let career_fixed = super::MODE_W + super::PLAYED_W + super::CHIP_W + 3. * 10. + 24.;
    let version_fixed = super::PIN_W + super::WHEN_W + super::SIZE_W + super::ACTIONS_W + 4. * 10. + 24.;
    assert!(content - career_fixed >= 150., "{}", content - career_fixed);
    assert!(content - version_fixed >= 150., "{}", content - version_fixed);
}

#[test]
fn restore_waits_for_what_is_running() {
    use super::short_block;
    use swofty_core::vault::Layout;
    let run = |game, steam| swofty_core::vault::restore::Running {
        game,
        steam,
        ea_app: false,
    };
    assert_eq!(
        short_block(Layout::Launcher, run(true, false)),
        Some("Close F1 25 first")
    );
    assert_eq!(short_block(Layout::Launcher, run(false, true)), None);
    assert_eq!(
        short_block(Layout::Steam, run(false, true)),
        Some("Close Steam first")
    );
    assert_eq!(short_block(Layout::Steam, run(false, false)), None);
}

fn copy_of(name: &str, ea: bool) -> PathBuf {
    let root = std::env::temp_dir().join(format!("swofty-saves-copy-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    let game = root.join("game");
    std::fs::create_dir_all(game.join("__Installer")).unwrap();
    std::fs::write(game.join(swofty_core::GAME_EXE), b"exe").unwrap();
    match ea {
        true => std::fs::write(game.join("__Installer/installerdata.xml"), b"<game/>").unwrap(),
        false => std::fs::write(game.join("EAJavelinInstaller_installscript.vdf"), b"").unwrap(),
    }
    let pfx = root.join("pfx");
    let save = pfx.join("drive_c/users/steamuser/AppData/Local/F1 25/savegame/ea/profile");
    std::fs::create_dir_all(&save).unwrap();
    std::fs::write(save.join("NXDSMWW.BWW"), b"data").unwrap();
    std::fs::write(pfx.join("user.reg"), b"").unwrap();
    let mut config = swofty_core::session::Config::default();
    config.games.active = Some(game.clone());
    config.games.added = vec![game];
    config.launch.ea_prefix = Some(pfx);
    swofty_core::store::Store::new(&root)
        .save_config(&config)
        .unwrap();
    std::fs::create_dir_all(root.join("profiles")).unwrap();
    for p in ["hi", "default"] {
        std::fs::write(
            root.join("profiles").join(format!("{p}.json")),
            format!(r#"{{"name":"{p}","launcher_saves":true}}"#),
        )
        .unwrap();
    }
    root
}

#[cfg(not(windows))]
#[test]
fn an_ea_copy_profile_shows_its_own_launcher_save() {
    let root = copy_of("ea", true);
    let store = swofty_core::store::Store::new(&root);
    for p in ["hi", "default"] {
        let built = build_saves(&store, None, p, false);
        let loc = built.view.loc.as_ref().expect("the EA save");
        assert_eq!(
            (loc.source.as_str(), loc.layout),
            (format!("launcher-ea.{p}").as_str(), vault::Layout::LauncherEa),
            "{p}"
        );
        assert!(!built.view.shared_by_every_profile(), "{p}");
    }
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_steam_copy_profile_shows_its_own_launcher_save() {
    let root = copy_of("steam", false);
    let store = swofty_core::store::Store::new(&root);
    let built = build_saves(&store, None, "hi", false);
    let loc = built.view.loc.as_ref().expect("the launcher save");
    assert_eq!(
        (loc.source.as_str(), loc.layout),
        ("launcher-hi", vault::Layout::Launcher)
    );
    assert!(!built.view.shared_by_every_profile());
    std::fs::remove_dir_all(&root).ok();
}
