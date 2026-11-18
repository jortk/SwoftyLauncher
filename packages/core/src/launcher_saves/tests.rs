use std::path::PathBuf;

use super::*;
use crate::vault::EventKind;

const DATA: &str = "savegame@profile#NXDSMWW.BWW";
const INDEX: &str = "savegame@profile#QKRHMYXE";
const NOW: i64 = 1_793_477_702;

fn root(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("swofty-launcher-saves-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn steam(root: &Path) -> Location {
    let remote = root.join("steam/userdata/843502855/3059520/remote");
    std::fs::create_dir_all(&remote).unwrap();
    std::fs::write(remote.join(DATA), [1u8; 900]).unwrap();
    std::fs::write(remote.join(INDEX), [7u8; 416]).unwrap();
    std::fs::write(remote.join("savegame@highlights001#QKRHMYXE"), b"replay").unwrap();
    Location::steam(&remote).unwrap()
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn closed() -> Running {
    Running::default()
}

fn profiles() -> Vec<String> {
    ["vanilla", "default", "progressive"].map(String::from).to_vec()
}

#[test]
fn the_store_is_one_folder_per_profile() {
    let data = Path::new("/d");
    assert_eq!(remote_dir(data, "default"), Path::new("/d/saves/default/remote"));
    let loc = location(data, "my-profile");
    assert_eq!(loc.source, "launcher-my-profile");
    assert_eq!(loc.layout, Layout::Launcher);
    assert_eq!(loc.dir, Path::new("/d/saves/my-profile/remote"));
    assert_eq!(loc.remotecache(), None);
    assert_eq!(loc.key_of(DATA).as_deref(), Some("profile/NXDSMWW.BWW"));
    assert_eq!(loc.key_of("savegame@highlights001#QKRHMYXE"), None);
}

#[test]
fn seeding_copies_the_profile_files_and_snapshots_them() {
    let r = root("seed");
    let from = steam(&r);
    let data = r.join("data");
    let got = seed(&data, "default", &from, Reason::Imported, NOW).unwrap();
    let Seeded::Copied {
        files,
        bytes,
        snapshot,
    } = got
    else {
        panic!("{got:?}");
    };
    assert_eq!((files, bytes), (2, 1316));
    let to = location(&data, "default");
    assert_eq!(names(&to.dir), [DATA, INDEX]);
    assert_eq!(std::fs::read(to.dir.join(DATA)).unwrap(), vec![1u8; 900]);
    assert_eq!(names(&from.dir).len(), 3);
    assert_eq!(std::fs::read(from.dir.join(DATA)).unwrap(), vec![1u8; 900]);
    let v = Vault::of(&data, &to).unwrap();
    let snaps = v.list().unwrap();
    assert_eq!(snaps.len(), 1);
    assert_eq!(Some(snaps[0].id.clone()), snapshot);
    assert_eq!(snaps[0].reason, Reason::Imported);
    assert_eq!(snaps[0].reason.text(), "Imported from Steam saves");
    assert_eq!(snaps[0].layout, Layout::Launcher);
    assert_eq!(snaps[0].source, "launcher-default");
    assert!(v.events().iter().any(|e| e.kind == EventKind::Snapshot));
}

#[test]
fn seeding_never_overwrites_a_store_that_has_files() {
    let r = root("keep");
    let from = steam(&r);
    let data = r.join("data");
    let to = location(&data, "default");
    std::fs::create_dir_all(&to.dir).unwrap();
    std::fs::write(to.dir.join(DATA), b"mine").unwrap();
    assert_eq!(
        seed(&data, "default", &from, Reason::Imported, NOW).unwrap(),
        Seeded::NotEmpty
    );
    assert_eq!(std::fs::read(to.dir.join(DATA)).unwrap(), b"mine");
    assert_eq!(names(&to.dir), [DATA]);
}

#[test]
fn seeding_without_a_steam_save_leaves_the_store_empty() {
    let r = root("none");
    let data = r.join("data");
    let empty = Location::steam(&r.join("steam/userdata/1/3059520/remote")).unwrap();
    assert_eq!(
        seed(&data, "default", &empty, Reason::Imported, NOW).unwrap(),
        Seeded::NoSource
    );
    assert!(!has_files(&location(&data, "default")));
}

#[test]
fn the_first_start_seeds_every_profile_once() {
    let r = root("first");
    let from = steam(&r);
    let data = r.join("data");
    let lines = first_start_import_from(
        &data,
        &profiles(),
        || (Some(from.clone()), Vec::new()),
        closed(),
        NOW,
        true,
    );
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.contains("copied the Steam save"))
            .count(),
        2,
        "{lines:?}"
    );
    for p in ["default", "progressive"] {
        assert_eq!(names(&location(&data, p).dir), [DATA, INDEX]);
        let snaps = Vault::of(&data, &location(&data, p)).unwrap().list().unwrap();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].reason, Reason::Imported);
    }
    assert!(!location(&data, "vanilla").dir.exists());
    let m = marker(&data).unwrap();
    assert_eq!(m.profiles, ["default", "progressive"]);
    assert_eq!(names(&from.dir).len(), 3);

    std::fs::remove_file(location(&data, "default").dir.join(INDEX)).unwrap();
    std::fs::write(from.dir.join(DATA), [9u8; 10]).unwrap();
    let again = first_start_import_from(
        &data,
        &profiles(),
        || (Some(from.clone()), Vec::new()),
        closed(),
        NOW + 60,
        true,
    );
    assert!(again.is_empty(), "{again:?}");
    assert_eq!(names(&location(&data, "default").dir), [DATA]);
    assert_eq!(
        std::fs::read(location(&data, "default").dir.join(DATA)).unwrap(),
        vec![1u8; 900]
    );
}

#[test]
fn a_running_game_defers_the_first_start_import() {
    let r = root("running");
    let from = steam(&r);
    let data = r.join("data");
    let running = Running {
        game: true,
        ..Running::default()
    };
    let lines = first_start_import_from(
        &data,
        &profiles(),
        || (Some(from.clone()), Vec::new()),
        running,
        NOW,
        true,
    );
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("F1 25 is running"), "{lines:?}");
    assert!(!root_exists(&data));
    assert_eq!(marker(&data), None);
    let later = first_start_import_from(
        &data,
        &profiles(),
        || (Some(from.clone()), Vec::new()),
        closed(),
        NOW + 60,
        true,
    );
    assert_eq!(
        later
            .iter()
            .filter(|l| l.contains("copied the Steam save"))
            .count(),
        2
    );
    assert!(marker(&data).is_some());
}

fn root_exists(data: &Path) -> bool {
    super::root(data).exists()
}

#[test]
fn the_first_start_import_leaves_a_store_with_files_untouched() {
    let r = root("existing");
    let from = steam(&r);
    let data = r.join("data");
    let mine = location(&data, "default");
    std::fs::create_dir_all(&mine.dir).unwrap();
    std::fs::write(mine.dir.join(DATA), b"mine").unwrap();
    let lines = first_start_import_from(
        &data,
        &profiles(),
        || (Some(from.clone()), Vec::new()),
        closed(),
        NOW,
        true,
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("profile default already hold files")),
        "{lines:?}"
    );
    assert_eq!(std::fs::read(mine.dir.join(DATA)).unwrap(), b"mine");
    assert_eq!(names(&location(&data, "progressive").dir), [DATA, INDEX]);
    assert_eq!(marker(&data).unwrap().profiles, ["progressive"]);
}

#[test]
fn no_steam_save_records_nothing_and_the_next_start_imports() {
    let r = root("nosteam");
    let data = r.join("data");
    let lines = first_start_import_from(&data, &profiles(), || (None, Vec::new()), closed(), NOW, true);
    assert!(lines[0].contains("no Steam save found"), "{lines:?}");
    assert_eq!(marker(&data), None);
    assert!(import_pending(&data));
    let from = steam(&r);
    first_start_import_from(
        &data,
        &profiles(),
        || (Some(from.clone()), Vec::new()),
        closed(),
        NOW + 60,
        true,
    );
    assert!(!import_pending(&data));
    assert_eq!(names(&location(&data, "default").dir), [DATA, INDEX]);
}

#[test]
fn an_empty_steam_folder_records_nothing() {
    let r = root("emptysteam");
    let data = r.join("data");
    let remote = r.join("steam/userdata/843502855/3059520/remote");
    std::fs::create_dir_all(&remote).unwrap();
    let from = Location::steam(&remote).unwrap();
    first_start_import_from(
        &data,
        &profiles(),
        || (Some(from.clone()), Vec::new()),
        closed(),
        NOW,
        true,
    );
    assert_eq!(marker(&data), None);
}

#[test]
fn an_explicit_import_replaces_the_store_after_keeping_it() {
    let r = root("import");
    let from = steam(&r);
    let data = r.join("data");
    let to = location(&data, "default");
    std::fs::create_dir_all(&to.dir).unwrap();
    std::fs::write(to.dir.join(DATA), b"old career").unwrap();
    let (files, bytes, snap) = import(&data, "default", &from, Reason::Imported, closed(), NOW).unwrap();
    assert_eq!((files, bytes), (2, 1316));
    assert!(snap.is_some());
    assert_eq!(std::fs::read(to.dir.join(DATA)).unwrap(), vec![1u8; 900]);
    let snaps = Vault::of(&data, &to).unwrap().list().unwrap();
    assert!(
        snaps
            .iter()
            .any(|s| s.reason == Reason::BeforeRestore && s.pinned == ["before-import"])
    );
    assert!(snaps.iter().any(|s| s.reason == Reason::Imported));
    let busy = Running {
        game: true,
        ..Running::default()
    };
    let err = import(&data, "default", &from, Reason::Imported, busy, NOW + 1).unwrap_err();
    assert!(err.to_string().contains("F1 25 is running"), "{err}");
}

#[test]
fn export_goes_through_the_vault_restore_and_spares_remotecache() {
    let r = root("export");
    let steam_loc = steam(&r);
    let data = r.join("data");
    let cache = steam_loc.remotecache().unwrap();
    std::fs::write(&cache, "\"3059520\"\n{\n}\n").unwrap();
    let cache_before = std::fs::read(&cache).unwrap();
    let to = location(&data, "default");
    std::fs::create_dir_all(&to.dir).unwrap();
    std::fs::write(to.dir.join(DATA), b"launcher career").unwrap();
    std::fs::write(to.dir.join(INDEX), b"index").unwrap();

    let busy = Running {
        steam: true,
        ..Running::default()
    };
    let err = export(&data, "default", &steam_loc, busy, NOW).unwrap_err();
    assert!(err.to_string().contains("Steam is running"), "{err}");
    assert_eq!(std::fs::read(steam_loc.dir.join(DATA)).unwrap(), vec![1u8; 900]);

    let done = export(&data, "default", &steam_loc, closed(), NOW + 5).unwrap();
    assert_eq!(done.dir, steam_loc.dir);
    assert_eq!(
        std::fs::read(steam_loc.dir.join(DATA)).unwrap(),
        b"launcher career"
    );
    assert_eq!(std::fs::read(steam_loc.dir.join(INDEX)).unwrap(), b"index");
    assert_eq!(
        std::fs::read(steam_loc.dir.join("savegame@highlights001#QKRHMYXE")).unwrap(),
        b"replay"
    );
    assert_eq!(std::fs::read(&cache).unwrap(), cache_before);
    let sv = Vault::of(&data, &steam_loc).unwrap();
    let before = sv.find(done.before.as_ref().unwrap()).unwrap();
    assert_eq!(before.reason, Reason::BeforeRestore);
    assert_eq!(before.layout, Layout::Steam);
    assert_eq!(before.files.iter().map(|f| f.size).sum::<u64>(), 1316);
    assert!(sv.list().unwrap().iter().any(|s| s.reason == Reason::Restored));
}

#[test]
fn a_launcher_snapshot_restores_into_the_launcher_store() {
    let r = root("restore");
    let data = r.join("data");
    let to = location(&data, "default");
    std::fs::create_dir_all(&to.dir).unwrap();
    std::fs::write(to.dir.join(DATA), b"career one").unwrap();
    let v = Vault::of(&data, &to).unwrap();
    let Taken::New(s) = v.snapshot(&to, Meta::new(Reason::Manual), NOW).unwrap() else {
        panic!()
    };
    std::fs::write(to.dir.join(DATA), b"career two, longer").unwrap();
    crate::vault::restore::restore(&v, &s.id, &to, closed(), NOW + 9).unwrap();
    assert_eq!(std::fs::read(to.dir.join(DATA)).unwrap(), b"career one");
    let steam_only = Running {
        steam: true,
        ..Running::default()
    };
    crate::vault::restore::restore(&v, &s.id, &to, steam_only, NOW + 10).unwrap();
}

#[test]
fn layouts_exchange_only_where_the_names_match() {
    assert!(Layout::Steam.exchanges_with(Layout::Launcher));
    assert!(Layout::Launcher.exchanges_with(Layout::Steam));
    assert!(!Layout::Ea.exchanges_with(Layout::Launcher));
    assert!(Layout::Ea.exchanges_with(Layout::Ea));
}

#[test]
fn the_ea_store_follows_the_ea_layout() {
    let r = root("ea");
    let profile = r.join("pfx/drive_c/users/steamuser/AppData/Local/F1 25/savegame/ea/profile");
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(profile.join("NXDSMWW.BWW"), [3u8; 64]).unwrap();
    std::fs::write(profile.join("QKRHMYXE"), [4u8; 16]).unwrap();
    let ea = Location::ea_in_prefix(&r.join("pfx")).unwrap();
    let data = r.join("data");
    let to = store_for(&data, "default", &ea).unwrap();
    assert_eq!(to.layout, Layout::LauncherEa);
    assert_eq!(to.source, "launcher-ea.default");
    assert_eq!(to.dir, data.join("saves/default/ea/profile"));
    assert!(supported(Layout::Ea) && supported(Layout::Steam));
    let Seeded::Copied { files, .. } = seed(&data, "default", &ea, Reason::Imported, NOW).unwrap() else {
        panic!()
    };
    assert_eq!(files, 2);
    assert_eq!(names(&to.dir), ["NXDSMWW.BWW", "QKRHMYXE"]);
    let (files, _, _) = import(&data, "default", &ea, Reason::Imported, closed(), NOW).unwrap();
    assert_eq!(files, 2);
}

#[test]
fn a_profile_save_copies_over_another_after_the_vault_keeps_the_target() {
    let r = root("copy-to");
    let data = r.join("data");
    let from = steam(&r);
    seed(&data, "default", &from, Reason::Imported, NOW).unwrap();
    seed(&data, "progressive", &from, Reason::Imported, NOW).unwrap();
    let default = location(&data, "default");
    std::fs::write(default.dir.join(DATA), [9u8; 900]).unwrap();
    let target = location(&data, "progressive");
    std::fs::write(target.dir.join(DATA), [5u8; 900]).unwrap();
    let (files, _, snapshot) = copy_to(&data, &default, "progressive", closed(), NOW + 5).unwrap();
    assert_eq!(files, 2);
    assert_eq!(std::fs::read(target.dir.join(DATA)).unwrap(), vec![9u8; 900]);
    let list = Vault::of(&data, &target).unwrap().list().unwrap();
    assert!(
        list.iter()
            .any(|s| s.reason == Reason::BeforeRestore && !s.pinned.is_empty())
    );
    let kept = list.iter().find(|s| Some(&s.id) == snapshot.as_ref()).unwrap();
    assert_eq!(kept.reason, Reason::ImportedProfile);
    assert_eq!(std::fs::read(default.dir.join(DATA)).unwrap(), vec![9u8; 900]);
}

#[test]
fn copying_the_steam_save_in_is_an_import_and_copying_onto_itself_is_refused() {
    let r = root("copy-steam");
    let data = r.join("data");
    let from = steam(&r);
    let (_, _, snapshot) = copy_to(&data, &from, "default", closed(), NOW).unwrap();
    let list = Vault::of(&data, &location(&data, "default"))
        .unwrap()
        .list()
        .unwrap();
    assert_eq!(
        list.iter()
            .find(|s| Some(&s.id) == snapshot.as_ref())
            .unwrap()
            .reason,
        Reason::Imported
    );
    let own = location(&data, "default");
    let e = copy_to(&data, &own, "default", closed(), NOW + 1).unwrap_err();
    assert!(e.to_string().contains("the save comes from"), "{e}");
    let running = Running {
        game: true,
        ..Running::default()
    };
    assert!(copy_to(&data, &from, "default", running, NOW + 2).is_err());
}

#[test]
fn the_first_start_notice_is_due_once() {
    let r = root("notice");
    let data = r.join("data");
    let from = steam(&r);
    assert!(!notice_due(&data));
    first_start_import_from(
        &data,
        &profiles(),
        || (Some(from.clone()), Vec::new()),
        closed(),
        NOW,
        true,
    );
    assert!(notice_due(&data));
    mark_seen(&data).unwrap();
    assert!(!notice_due(&data));
    assert!(marker(&data).unwrap().seen);
    first_start_import_from(
        &data,
        &profiles(),
        || (Some(from.clone()), Vec::new()),
        closed(),
        NOW + 9,
        true,
    );
    assert!(!notice_due(&data));
}

#[test]
fn no_notice_when_nothing_was_copied() {
    let r = root("notice-empty");
    let data = r.join("data");
    first_start_import_from(&data, &profiles(), || (None, Vec::new()), closed(), NOW, true);
    assert!(!notice_due(&data));
}

#[test]
fn a_miss_lists_where_it_looked_once_and_retries_quietly() {
    let r = root("miss-log");
    let data = r.join("data");
    let tried = || vec!["Steam root C:/Steam (a known Steam install)".to_owned()];
    let first = first_start_import_from(&data, &profiles(), || (None, tried()), closed(), NOW, true);
    assert!(first[0].contains("play on Steam's saves"), "{first:?}");
    assert!(
        first
            .iter()
            .any(|l| l.contains("looked for the Steam save: Steam root C:/Steam")),
        "{first:?}"
    );
    let quiet = first_start_import_from(&data, &profiles(), || (None, tried()), closed(), NOW + 30, false);
    assert!(quiet.is_empty(), "{quiet:?}");
    assert!(import_pending(&data));
}

#[test]
fn the_steam_save_is_pending_until_an_import_found_it() {
    let r = root("pending");
    let data = r.join("data");
    assert!(steam_save_pending(&data, "default"));
    assert!(!steam_save_pending(&data, "vanilla"));
    let to = location(&data, "default");
    std::fs::create_dir_all(&to.dir).unwrap();
    std::fs::write(to.dir.join(DATA), b"x").unwrap();
    assert!(!steam_save_pending(&data, "default"));
    assert!(steam_save_pending(&data, "progressive"));
    std::fs::remove_file(to.dir.join(DATA)).unwrap();
    let from = steam(&r);
    first_start_import_from(
        &data,
        &profiles(),
        || (Some(from.clone()), Vec::new()),
        closed(),
        NOW,
        true,
    );
    assert!(!steam_save_pending(&data, "default"));
}

fn choice() -> Choice<'static> {
    Choice {
        build_blocked: None,
        vanilla: false,
        profile_uses_launcher_saves: true,
        steam_saves_once: false,
        layout: Layout::Steam,
    }
}

#[test]
fn a_profile_with_launcher_saves_on_a_steam_copy_plays_on_the_launcher_save() {
    assert_eq!(area(choice()), Area::Launcher);
    assert_eq!(
        area(Choice {
            layout: Layout::Launcher,
            ..choice()
        }),
        Area::Launcher
    );
}

#[test]
fn each_reason_for_the_copys_own_save_is_named() {
    let own = |c| area(c);
    assert_eq!(
        own(Choice {
            vanilla: true,
            ..choice()
        }),
        Area::Own(Why::Vanilla)
    );
    assert_eq!(
        own(Choice {
            profile_uses_launcher_saves: false,
            ..choice()
        }),
        Area::Own(Why::LauncherSavesOff)
    );
    assert_eq!(
        own(Choice {
            steam_saves_once: true,
            ..choice()
        }),
        Area::Own(Why::SteamOnce)
    );
    assert_eq!(
        own(Choice {
            layout: Layout::Ea,
            ..choice()
        }),
        Area::Launcher
    );
    assert_eq!(
        own(Choice {
            build_blocked: Some("1.0.145.2472"),
            ..choice()
        }),
        Area::Own(Why::UnsupportedBuild("1.0.145.2472".into()))
    );
}

#[test]
fn an_unsupported_build_outranks_every_other_reason() {
    let all = Choice {
        build_blocked: Some("1.0.145.2472"),
        vanilla: true,
        profile_uses_launcher_saves: false,
        steam_saves_once: true,
        layout: Layout::Ea,
    };
    assert_eq!(area(all), Area::Own(Why::UnsupportedBuild("1.0.145.2472".into())));
}

#[test]
fn a_staged_copy_keeps_its_contents_and_leaves_no_temp_file() {
    let r = root("stage-copy");
    let from = r.join("from.bin");
    std::fs::write(&from, [9u8; 4096]).unwrap();
    let to = r.join("to.bin");
    stage_copy(&from, &to).unwrap();
    assert_eq!(std::fs::read(&to).unwrap(), [9u8; 4096]);
    assert_eq!(names(&r), ["from.bin", "to.bin"]);
}
