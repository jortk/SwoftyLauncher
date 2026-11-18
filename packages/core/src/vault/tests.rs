use std::path::{Path, PathBuf};

use super::restore::{Running, restore};
use super::*;

const T0: i64 = 1_793_404_800;
const DATA: &str = "savegame@profile#NXDSMWW.BWW";
const INDEX: &str = "savegame@profile#QKRHMYXE";

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("swofty-vault-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A fake Steam account's save folder with the profile pair, a highlight
/// and Steam's remotecache.vdf.
fn steam(root: &Path, id3: &str, data: &[u8]) -> Location {
    let remote = root.join(format!("steam/userdata/{id3}/3059520/remote"));
    std::fs::create_dir_all(&remote).unwrap();
    std::fs::write(remote.join(DATA), data).unwrap();
    std::fs::write(remote.join(INDEX), [7u8; 416]).unwrap();
    std::fs::write(remote.join("savegame@highlights001#QKRHMYXE"), b"replay").unwrap();
    let (_, _, sha1) = hashes(&remote.join(DATA)).unwrap();
    std::fs::write(
        remote.parent().unwrap().join("remotecache.vdf"),
        format!(
            "\"3059520\"\n{{\n\t\"ChangeNumber\"\t\t\"510\"\n\t\"{DATA}\"\n\t{{\n\t\t\"root\"\t\t\"0\"\n\t\t\"size\"\t\t\"{}\"\n\
             \t\t\"sha\"\t\t\"{sha1}\"\n\t\t\"syncstate\"\t\t\"1\"\n\t}}\n}}\n",
            data.len()
        ),
    )
    .unwrap();
    Location::steam(&remote).unwrap()
}

fn vault(root: &Path, loc: &Location) -> Vault {
    Vault::of(&root.join("data"), loc).unwrap()
}

fn new(t: Taken) -> Snapshot {
    match t {
        Taken::New(s) => s,
        other => panic!("expected a new snapshot, got {other:?}"),
    }
}

fn kinds(v: &Vault) -> Vec<EventKind> {
    v.events().into_iter().map(|e| e.kind).collect()
}

#[test]
fn dedupes_by_content() {
    let root = tmp("dedupe");
    let loc = steam(&root, "843502855", &[1u8; 900]);
    let v = vault(&root, &loc);
    assert_eq!(v.dir, root.join("data/career-vault/steam-843502855"));
    let a = new(v.snapshot(&loc, Meta::new(Reason::LauncherStart), T0).unwrap());
    assert_eq!(a.id, "20261031-000000-launcher-start");
    assert_eq!(a.files.len(), 2, "highlights stay out");
    assert_eq!(a.files[0].name, "profile/NXDSMWW.BWW");
    assert_eq!(a.files[0].file, DATA);
    assert_eq!(a.files[0].remotecache.as_ref().unwrap().syncstate, "1");
    assert_eq!(
        a.cloud(),
        cloud::Status::Unknown,
        "the index has no remotecache entry"
    );
    assert_eq!(a.total(), 900 + 416);
    let again = v
        .snapshot(&loc, Meta::new(Reason::BeforeSession), T0 + 60)
        .unwrap();
    assert_eq!(again, Taken::Unchanged(a.clone()));
    assert_eq!(v.list().unwrap().len(), 1);
    std::fs::write(loc.dir.join(DATA), [2u8; 950]).unwrap();
    let b = new(v
        .snapshot(&loc, Meta::new(Reason::AfterSession), T0 + 120)
        .unwrap());
    assert_eq!(b.files[0].remotecache.as_ref().map(|r| r.size), Some(900));
    assert_eq!(
        cloud::status(b.files[0].remotecache.as_ref(), &b.files[0].sha1),
        cloud::Status::Waiting
    );
    let blobs = std::fs::read_dir(v.dir.join("blobs")).unwrap().count();
    assert_eq!(blobs, 3, "the unchanged index is stored once");
    assert_eq!(v.usage(), 900 + 950 + 416);
    assert_eq!(
        kinds(&v),
        [EventKind::Snapshot, EventKind::Unchanged, EventKind::Snapshot]
    );
    assert_eq!(std::fs::read(v.blob(&b.files[0].sha256)).unwrap(), [2u8; 950]);
}

#[test]
fn refuses_a_file_that_changes_while_it_is_copied() {
    let root = tmp("changing");
    let loc = steam(&root, "1", b"data");
    let v = vault(&root, &loc);
    let err = v.store_blob(&loc.dir.join(DATA), "0000").unwrap_err();
    assert!(
        err.to_string().contains("changed while it was backed up"),
        "{err}"
    );
    assert!(!v.blob("0000").exists());
}

#[test]
fn a_stored_blob_keeps_its_contents_and_leaves_no_temp_file() {
    let root = tmp("blob-flush");
    let loc = steam(&root, "1", b"data");
    let v = vault(&root, &loc);
    let src = loc.dir.join(DATA);
    let sha = crate::baseline::sha256_file(&src).unwrap();
    v.store_blob(&src, &sha).unwrap();
    assert_eq!(std::fs::read(v.blob(&sha)).unwrap(), std::fs::read(&src).unwrap());
    let left: Vec<_> = std::fs::read_dir(v.dir.join("blobs"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

#[test]
fn keeps_accounts_apart() {
    let root = tmp("accounts");
    let a = steam(&root, "11", b"a");
    let b = steam(&root, "22", b"b");
    let va = vault(&root, &a);
    assert_ne!(va.dir, vault(&root, &b).dir);
    assert!(va.snapshot(&b, Meta::new(Reason::Manual), T0).is_err());
    new(va.snapshot(&a, Meta::new(Reason::Manual), T0).unwrap());
    new(vault(&root, &b)
        .snapshot(&b, Meta::new(Reason::Manual), T0)
        .unwrap());
    assert_eq!(Vault::sources(&root.join("data")), ["steam-11", "steam-22"]);
    assert!(Vault::open(&root.join("data"), "../x").is_err());
}

#[test]
fn retention_keeps_pins_and_the_cap() {
    let root = tmp("retention");
    let loc = steam(&root, "5", &[0u8; 1000]);
    let mut v = vault(&root, &loc);
    v.policy.cap_bytes = 4000;
    let mut ids = Vec::new();
    for i in 0..6u8 {
        std::fs::write(loc.dir.join(DATA), [i; 1000]).unwrap();
        let meta = match i {
            1 => Meta::new(Reason::BeforeSession).pin("vanilla-session"),
            _ => Meta::new(Reason::BeforeSession),
        };
        ids.push(new(v.snapshot(&loc, meta, T0 + i64::from(i) * 60).unwrap()).id);
    }
    let left: Vec<String> = v.list().unwrap().into_iter().map(|s| s.id).collect();
    assert!(left.contains(&ids[1]), "pinned");
    assert!(left.contains(&ids[5]), "newest");
    assert!(!left.contains(&ids[0]), "oldest unpinned goes first");
    let blobs: u64 = v.usage();
    assert!(blobs <= 4000, "{blobs}");
    assert!(kinds(&v).contains(&EventKind::Pruned));
    let far = T0 + 400 * retention::DAY;
    v.policy.cap_bytes = u64::MAX;
    std::fs::write(loc.dir.join(DATA), [9u8; 1000]).unwrap();
    let last = new(v.snapshot(&loc, Meta::new(Reason::LauncherStart), far).unwrap());
    let left: Vec<String> = v.list().unwrap().into_iter().map(|s| s.id).collect();
    assert_eq!(
        left,
        [last.id.clone(), ids[1].clone()],
        "old unpinned snapshots age out"
    );
    assert_eq!(v.unpin(&ids[1]).unwrap().pinned, Vec::<String>::new());
}

#[test]
fn finds_a_save_changed_outside_the_launcher() {
    let root = tmp("outside");
    let loc = steam(&root, "7", &[1u8; 90_000]);
    let v = vault(&root, &loc);
    let data = root.join("data");
    let before = new(v
        .before_session(&loc, Meta::new(Reason::BeforeSession).profile("default"), T0)
        .unwrap()
        .taken);
    let after = v
        .after_session(&loc, Meta::new(Reason::AfterSession), Some(&before), T0 + 600)
        .unwrap();
    assert_eq!(after.taken, Taken::Unchanged(before.clone()));
    let quiet = v.check(&loc, Meta::new(Reason::LauncherStart), T0 + 700).unwrap();
    assert!(quiet.outside.is_none() && quiet.notice.is_none());

    std::fs::write(loc.dir.join(DATA), [3u8; 89_000]).unwrap();
    let small = v.check(&loc, Meta::new(Reason::LauncherStart), T0 + 800).unwrap();
    let ch = small.outside.unwrap();
    assert_eq!(ch.from.id, before.id);
    assert_eq!(ch.to.reason, Reason::ChangedOutside);
    assert_eq!(ch.to.note.as_deref(), Some("found when the launcher started"));
    assert!(ch.from.pinned.contains(&"before-outside-change".to_owned()));
    assert!(small.notice.is_none(), "1 KB smaller is ordinary play");
    assert!(Notice::read(&data).is_none());

    std::fs::write(loc.dir.join(DATA), [4u8; 40_000]).unwrap();
    let shrunk = v.check(&loc, Meta::new(Reason::BeforeSession), T0 + 900).unwrap();
    let n = shrunk.notice.unwrap();
    assert_eq!(n.kind, NoticeKind::ChangedOutside);
    assert_eq!(n.snapshot, ch.to.id);
    assert_eq!(Notice::read(&data), Some(n.clone()));
    let text = n.text();
    assert!(text.contains("changed outside a launcher session"), "{text}");
    assert!(
        text.contains(&format!("swofty saves restore {}", ch.to.id)),
        "{text}"
    );
    assert!(!text.contains("gone") && !text.contains("lost"), "{text}");
    let k = kinds(&v);
    assert!(k.contains(&EventKind::ChangedOutside) && k.contains(&EventKind::Shrank));
}

#[test]
fn a_session_that_never_ended_is_not_outside() {
    let root = tmp("unfinished");
    let loc = steam(&root, "8", &[1u8; 90_000]);
    let v = vault(&root, &loc);
    v.before_session(&loc, Meta::new(Reason::BeforeSession), T0)
        .unwrap();
    std::fs::write(loc.dir.join(DATA), [2u8; 10_000]).unwrap();
    let c = v.check(&loc, Meta::new(Reason::LauncherStart), T0 + 60).unwrap();
    assert!(c.outside.is_none());
    let s = c.taken.snapshot().unwrap();
    assert_eq!(s.reason, Reason::AfterSession);
    assert!(
        s.note
            .as_deref()
            .unwrap()
            .contains("ended without its after-session snapshot")
    );
    std::fs::write(loc.dir.join(DATA), [3u8; 10_000]).unwrap();
    assert!(
        v.check(&loc, Meta::new(Reason::LauncherStart), T0 + 120)
            .unwrap()
            .outside
            .is_some(),
        "the next change is outside again"
    );
}

#[test]
fn hints_at_a_session_shrink_only_with_an_addon_left_out() {
    let root = tmp("shrink");
    let loc = steam(&root, "9", &[1u8; 90_000]);
    let v = vault(&root, &loc);
    let before = new(v
        .before_session(&loc, Meta::new(Reason::BeforeSession), T0)
        .unwrap()
        .taken);
    std::fs::write(loc.dir.join(DATA), [2u8; 20_000]).unwrap();
    let c = v
        .after_session(&loc, Meta::new(Reason::AfterSession), Some(&before), T0 + 60)
        .unwrap();
    assert!(c.shrank.is_some() && c.notice.is_none());
    let mut meta = Meta::new(Reason::BeforeSession);
    meta.absent = vec!["swofty.progressive".into()];
    std::fs::write(loc.dir.join(DATA), [1u8; 90_000]).unwrap();
    let before = new(v.before_session(&loc, meta.clone(), T0 + 180).unwrap().taken);
    assert_eq!(
        before.reason,
        Reason::ChangedOutside,
        "the save changed since the last snapshot"
    );
    std::fs::write(loc.dir.join(DATA), [3u8; 20_000]).unwrap();
    meta.reason = Reason::AfterSession;
    let n = v
        .after_session(&loc, meta, Some(&before), T0 + 240)
        .unwrap()
        .notice
        .unwrap();
    assert_eq!(n.kind, NoticeKind::Shrank);
    assert_eq!(n.snapshot, before.id);
    assert!(n.text().contains("in a session without swofty.progressive"));
}

#[test]
fn restore_refuses_while_steam_or_the_game_runs() {
    let root = tmp("refuse");
    let loc = steam(&root, "10", &[1u8; 900]);
    let v = vault(&root, &loc);
    let s = new(v.snapshot(&loc, Meta::new(Reason::Manual), T0).unwrap());
    std::fs::write(loc.dir.join(DATA), [2u8; 500]).unwrap();
    let steam_up = Running {
        steam: true,
        ..Running::default()
    };
    let err = restore(&v, &s.id, &loc, steam_up, T0 + 60)
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("Steam is running") && err.contains("Exit Steam completely"),
        "{err}"
    );
    let game = Running {
        game: true,
        ..Running::default()
    };
    assert!(
        restore(&v, &s.id, &loc, game, T0 + 60)
            .unwrap_err()
            .to_string()
            .contains("F1 25 is running")
    );
    let msg = super::restore::blocker(Layout::Launcher, game).unwrap();
    assert_eq!(msg.matches("F1 25").count(), 1, "{msg}");
    assert_eq!(std::fs::read(loc.dir.join(DATA)).unwrap(), [2u8; 500]);
    assert_eq!(v.list().unwrap().len(), 1, "a refused restore takes no snapshot");
    assert!(kinds(&v).contains(&EventKind::RestoreRefused));
    let ea = Running {
        steam: true,
        ..Running::default()
    };
    assert_eq!(
        restore::blocker(Layout::Ea, ea),
        None,
        "Steam doesn't hold the EA copy's save"
    );
}

#[test]
fn restore_writes_the_files_and_leaves_remotecache_alone() {
    let root = tmp("restore");
    let loc = steam(&root, "12", &[1u8; 90_000]);
    let data = root.join("data");
    let v = vault(&root, &loc);
    let good = new(v.snapshot(&loc, Meta::new(Reason::AfterSession), T0).unwrap());
    std::fs::write(loc.dir.join(DATA), [2u8; 20_000]).unwrap();
    std::fs::write(loc.dir.join("savegame@profile#D1990.UEL"), b"1990").unwrap();
    v.check(&loc, Meta::new(Reason::LauncherStart), T0 + 60).unwrap();
    let cache = loc.remotecache().unwrap();
    let cache_before = std::fs::read(&cache).unwrap();
    let cache_time = std::fs::metadata(&cache).unwrap().modified().unwrap();
    assert!(Notice::read(&data).is_some());

    let r = restore(&v, &good.id, &loc, Running::default(), T0 + 120).unwrap();
    assert_eq!(std::fs::read(loc.dir.join(DATA)).unwrap(), [1u8; 90_000]);
    assert_eq!(std::fs::read(loc.dir.join(INDEX)).unwrap(), [7u8; 416]);
    assert_eq!(std::fs::read(&cache).unwrap(), cache_before);
    assert_eq!(std::fs::metadata(&cache).unwrap().modified().unwrap(), cache_time);
    assert_eq!(
        std::fs::read(loc.dir.join("savegame@highlights001#QKRHMYXE")).unwrap(),
        b"replay"
    );
    assert_eq!(r.kept, ["profile/D1990.UEL"]);
    assert!(r.log_line().starts_with(&format!("[vault] restored {}", good.id)));
    let before = v.find(r.before.as_deref().unwrap()).unwrap();
    assert!(before.pinned.contains(&"before-restore".to_owned()));
    let data_file = before
        .files
        .iter()
        .find(|f| f.name == "profile/NXDSMWW.BWW")
        .unwrap();
    assert_eq!(std::fs::read(v.blob(&data_file.sha256)).unwrap(), [2u8; 20_000]);
    assert_eq!(v.newest().unwrap().unwrap().reason, Reason::Restored);
    assert!(Notice::read(&data).is_none());
    assert!(kinds(&v).contains(&EventKind::Restored));
    let after = v.check(&loc, Meta::new(Reason::LauncherStart), T0 + 180).unwrap();
    assert!(matches!(after.taken, Taken::Unchanged(_)) && after.outside.is_none());
    assert!(
        !loc.dir
            .read_dir()
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().ends_with(".swofty-restore"))
    );

    std::fs::write(v.blob(&good.files[1].sha256), b"tampered").unwrap();
    let err = restore(&v, &good.id, &loc, Running::default(), T0 + 240).unwrap_err();
    assert!(err.to_string().contains("is damaged"), "{err}");
    assert_eq!(std::fs::read(loc.dir.join(INDEX)).unwrap(), [7u8; 416]);
    assert!(restore(&v, "nope", &loc, Running::default(), T0 + 240).is_err());
}

#[test]
fn the_ea_copy_has_its_own_vault() {
    let root = tmp("ea");
    let pfx = root.join("pfx");
    let profile = pfx.join("drive_c/users/steamuser/AppData/Local/F1 25/savegame/ea/profile");
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::create_dir_all(profile.parent().unwrap().join("midsessions0")).unwrap();
    std::fs::write(profile.join("NXDSMWW.BWW"), [5u8; 5000]).unwrap();
    std::fs::write(profile.join("QKRHMYXE"), [6u8; 416]).unwrap();
    let loc = Location::ea_in_prefix(&pfx).unwrap();
    let v = vault(&root, &loc);
    assert!(v.dir.ends_with("career-vault/ea-steamuser"));
    let s = new(v.snapshot(&loc, Meta::new(Reason::BeforeSession), T0).unwrap());
    assert_eq!(s.layout, Layout::Ea);
    assert_eq!(s.cloud(), cloud::Status::Local);
    let names: Vec<&str> = s.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["profile/NXDSMWW.BWW", "profile/QKRHMYXE"]);
    std::fs::write(profile.join("NXDSMWW.BWW"), [0u8; 10]).unwrap();
    let app = Running {
        ea_app: true,
        ..Running::default()
    };
    let err = restore(&v, &s.id, &loc, app, T0 + 60).unwrap_err().to_string();
    assert!(err.contains("The EA app is running"), "{err}");
    let steam_up = Running {
        steam: true,
        ..Running::default()
    };
    restore(&v, &s.id, &loc, steam_up, T0 + 60).unwrap();
    assert_eq!(std::fs::read(profile.join("NXDSMWW.BWW")).unwrap(), [5u8; 5000]);
    assert!(
        Notice {
            kind: NoticeKind::ChangedOutside,
            source: loc.source.clone(),
            snapshot: s.id.clone(),
            kept_at: T0,
            before: 9000,
            after: 10,
            absent: Vec::new(),
            at: T0,
        }
        .text()
        .contains("the EA app are closed")
    );
}

#[test]
fn migrates_save_backups_and_retires_them_at_the_first_prune() {
    let root = tmp("migrate");
    let data = root.join("data");
    let loc = steam(&root, "843502855", &[1u8; 900]);
    let old = crate::save_backup::take(
        &data,
        &loc.dir,
        "default",
        &["swofty.progressive".into()],
        "20261027-010203",
        "session",
    )
    .unwrap()
    .unwrap();
    std::fs::create_dir_all(data.join("state")).unwrap();
    std::fs::write(
        data.join(crate::save_backup::NOTICE),
        format!(
            r#"{{"backup":"{}","profile":"default","absent":["swofty.progressive"],"before":90000,"after":20000}}"#,
            old.name
        ),
    )
    .unwrap();
    let notes = migrate::migrate(&data).unwrap();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("steam-843502855/20261027-010203-migrated"));
    assert!(migrate::migrate(&data).unwrap().is_empty(), "once");
    let v = vault(&root, &loc);
    let s = v.find("20261027-010203-migrated").unwrap();
    assert_eq!(s.reason, Reason::Migrated);
    assert_eq!(s.at, crate::save_backup::parse_stamp("20261027-010203").unwrap());
    assert_eq!(s.profile.as_deref(), Some("default"));
    assert_eq!(s.absent_addons, ["swofty.progressive"]);
    assert_eq!(s.files[0].sha256, old.manifest.files[0].sha256);
    assert_eq!(s.files[0].name, "profile/NXDSMWW.BWW");
    let n = Notice::read(&data).unwrap();
    assert_eq!((n.kind, n.snapshot.as_str()), (NoticeKind::Shrank, s.id.as_str()));
    assert!(!data.join(crate::save_backup::NOTICE).exists());
    assert!(data.join("save-backups").is_dir(), "kept until the first prune");

    let unchanged = v.check(&loc, Meta::new(Reason::LauncherStart), T0).unwrap();
    assert!(matches!(unchanged.taken, Taken::Unchanged(_)));
    assert!(data.join("save-backups").is_dir());
    std::fs::write(loc.dir.join(DATA), [3u8; 900]).unwrap();
    new(v.snapshot(&loc, Meta::new(Reason::AfterSession), T0).unwrap());
    std::fs::write(loc.dir.join(DATA), [2u8; 900]).unwrap();
    new(v
        .snapshot(&loc, Meta::new(Reason::AfterSession), T0 + 400 * retention::DAY)
        .unwrap());
    assert!(
        v.list()
            .unwrap()
            .iter()
            .any(|s| s.reason == Reason::Migrated && s.pinned == ["migrated"]),
        "a migrated snapshot is pinned and never ages out"
    );
    assert!(!data.join("save-backups").exists());
    assert!(migrate::marker(&data).unwrap().retired);
}

fn migrated(name: &str) -> (PathBuf, PathBuf, Location, Vault, String) {
    let root = tmp(name);
    let data = root.join("data");
    let loc = steam(&root, "843502855", &[1u8; 900]);
    crate::save_backup::take(&data, &loc.dir, "default", &[], "20261027-010203", "session")
        .unwrap()
        .unwrap();
    migrate::migrate(&data).unwrap();
    let v = vault(&root, &loc);
    let id = v.find("20261027-010203-migrated").unwrap().id;
    (root, data, loc, v, id)
}

fn age_out(v: &Vault, loc: &Location) {
    std::fs::write(loc.dir.join(DATA), [5u8; 900]).unwrap();
    new(v.snapshot(loc, Meta::new(Reason::AfterSession), T0).unwrap());
    std::fs::write(loc.dir.join(DATA), [6u8; 900]).unwrap();
    new(v
        .snapshot(loc, Meta::new(Reason::AfterSession), T0 + 400 * retention::DAY)
        .unwrap());
}

#[test]
fn a_migrated_snapshot_survives_every_prune_and_the_cap() {
    let (_root, data, loc, mut v, id) = migrated("migrate-pin");
    assert_eq!(v.find(&id).unwrap().pinned, ["migrated"]);
    v.policy.cap_bytes = 10;
    age_out(&v, &loc);
    assert!(v.find(&id).is_ok(), "pinned snapshots outlive age and the cap");
    let blob = v.find(&id).unwrap().files[0].sha256.clone();
    assert!(v.blob(&blob).is_file());
    assert!(
        v.events()
            .iter()
            .any(|e| e.detail.as_deref().is_some_and(|d| d.contains("byte cap")))
    );
    assert!(migrate::marker(&data).is_some());
}

#[test]
fn retire_keeps_the_backups_when_the_vault_lacks_a_blob() {
    let (_root, data, loc, v, id) = migrated("retire-blob");
    let sha = v.find(&id).unwrap().files[0].sha256.clone();
    std::fs::remove_file(v.blob(&sha)).unwrap();
    migrate::retire(&data);
    assert!(data.join("save-backups/20261027-010203-default").is_dir());
    assert!(!migrate::marker(&data).unwrap().retired);
    std::fs::write(v.blob(&sha), b"wrong").unwrap();
    migrate::retire(&data);
    assert!(
        data.join("save-backups/20261027-010203-default").is_dir(),
        "wrong hash"
    );
    drop(loc);
}

#[test]
fn retire_deletes_only_the_folders_it_imported() {
    let (_root, data, loc, v, _id) = migrated("retire-only");
    let old = data.join("save-backups");
    std::fs::create_dir_all(old.join(".hidden")).unwrap();
    std::fs::write(old.join(".hidden/keep"), b"x").unwrap();
    std::fs::create_dir_all(old.join("no-manifest")).unwrap();
    std::fs::write(old.join("no-manifest/f"), b"y").unwrap();
    age_out(&v, &loc);
    assert!(
        !old.join("20261027-010203-default").exists(),
        "the imported one goes"
    );
    assert!(old.join(".hidden/keep").is_file());
    assert!(old.join("no-manifest/f").is_file());
    assert!(!migrate::marker(&data).unwrap().retired);
}

#[test]
fn retire_waits_while_a_migration_runs() {
    let (_root, data, loc, v, _id) = migrated("retire-lock");
    let held = Vault::migrate_lock(&data, true).unwrap().unwrap();
    assert!(Vault::migrate_lock(&data, false).unwrap().is_none());
    age_out(&v, &loc);
    assert!(data.join("save-backups/20261027-010203-default").is_dir());
    drop(held);
    migrate::retire(&data);
    assert!(!data.join("save-backups").exists());
}

#[test]
fn an_unreadable_snapshot_stops_the_prune_and_keeps_every_blob() {
    let root = tmp("unreadable");
    let loc = steam(&root, "7", &[1u8; 900]);
    let v = vault(&root, &loc);
    let a = new(v.snapshot(&loc, Meta::new(Reason::AfterSession), T0).unwrap());
    std::fs::write(loc.dir.join(DATA), [2u8; 900]).unwrap();
    let b = new(v
        .snapshot(&loc, Meta::new(Reason::AfterSession), T0 + 60)
        .unwrap());
    let odd = v.dir.join("snapshots/20261031-000500-future.json");
    std::fs::write(&odd, br#"{"schema": 99, "files": "other shape"}"#).unwrap();
    let blobs = |v: &Vault| std::fs::read_dir(v.dir.join("blobs")).unwrap().count();
    let before = blobs(&v);
    let far = T0 + 400 * retention::DAY;
    assert!(v.prune(far).unwrap().is_empty());
    assert_eq!(blobs(&v), before);
    assert!(v.find(&a.id).is_ok() && v.find(&b.id).is_ok());
    assert!(odd.is_file());
    assert!(
        v.events()
            .iter()
            .any(|e| e.detail.as_deref().is_some_and(|d| d.contains("future.json")))
    );
}

#[test]
fn restore_waits_while_a_session_holds_the_lock() {
    let root = tmp("restore-session");
    let loc = steam(&root, "12", &[1u8; 900]);
    let v = vault(&root, &loc);
    let good = new(v.snapshot(&loc, Meta::new(Reason::AfterSession), T0).unwrap());
    std::fs::write(loc.dir.join(DATA), [2u8; 900]).unwrap();
    let held = crate::recovery::SessionLock::try_acquire(&root.join("data"))
        .unwrap()
        .unwrap();
    let err = restore(&v, &good.id, &loc, Running::default(), T0 + 60).unwrap_err();
    assert!(err.to_string().contains("Play session"), "{err}");
    assert_eq!(std::fs::read(loc.dir.join(DATA)).unwrap(), [2u8; 900]);
    drop(held);
    restore(&v, &good.id, &loc, Running::default(), T0 + 60).unwrap();
}

#[test]
fn a_restore_that_cannot_stage_every_file_leaves_the_save_alone() {
    let root = tmp("restore-stage");
    let loc = steam(&root, "12", &[1u8; 900]);
    let v = vault(&root, &loc);
    let good = new(v.snapshot(&loc, Meta::new(Reason::AfterSession), T0).unwrap());
    std::fs::write(loc.dir.join(DATA), [2u8; 900]).unwrap();
    std::fs::write(loc.dir.join(INDEX), [3u8; 416]).unwrap();
    std::fs::create_dir(loc.dir.join(format!(".{INDEX}.swofty-restore"))).unwrap();
    let err = restore(&v, &good.id, &loc, Running::default(), T0 + 60).unwrap_err();
    assert!(err.to_string().contains("left as it was"), "{err}");
    assert_eq!(std::fs::read(loc.dir.join(DATA)).unwrap(), [2u8; 900]);
    assert_eq!(std::fs::read(loc.dir.join(INDEX)).unwrap(), [3u8; 416]);
    assert!(!loc.dir.join(format!(".{DATA}.swofty-restore")).exists());
}

#[test]
fn a_restore_that_fails_mid_rename_names_the_snapshot_that_undoes_it() {
    let root = tmp("restore-rename");
    let loc = steam(&root, "12", &[1u8; 900]);
    let v = vault(&root, &loc);
    let good = new(v.snapshot(&loc, Meta::new(Reason::AfterSession), T0).unwrap());
    std::fs::write(loc.dir.join(DATA), [2u8; 900]).unwrap();
    std::fs::remove_file(loc.dir.join(INDEX)).unwrap();
    std::fs::create_dir(loc.dir.join(INDEX)).unwrap();
    let err = restore(&v, &good.id, &loc, Running::default(), T0 + 60)
        .unwrap_err()
        .to_string();
    let before = v
        .list()
        .unwrap()
        .into_iter()
        .find(|s| s.reason == Reason::BeforeRestore)
        .unwrap();
    assert!(
        err.contains(&format!("swofty saves restore {}", before.id)),
        "{err}"
    );
    assert!(err.contains("mixes old and restored"), "{err}");
    assert!(
        !loc.dir
            .read_dir()
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().ends_with(".swofty-restore"))
    );
}

#[test]
fn humane_times() {
    assert_eq!(human(T0), "2026-10-31 00:00 UTC");
    assert_eq!(human(T0 + 3723), "2026-10-31 01:02 UTC");
}

fn write_index(root: &Path, careers: &[(&str, &str, i64, i64)]) {
    let list: Vec<serde_json::Value> = careers
        .iter()
        .map(|(id, status, season, round)| {
            serde_json::json!({
                "career": id,
                "status": status,
                "label": format!("{id} label"),
                "mode": "progressive",
                "driver": "Verstappen",
                "team": "Red Bull",
                "season": season,
                "round": round,
                "last_saved": 1_793_400_000_000i64,
                "addons": [{"id": "swofty.progressive"}]
            })
        })
        .collect();
    let path = root.join("data/careers/index.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        serde_json::json!({ "version": 1, "careers": list }).to_string(),
    )
    .unwrap();
}

fn take(v: &Vault, loc: &Location, reason: Reason, at: i64, data: &[u8]) -> Snapshot {
    std::fs::write(loc.dir.join(DATA), data).unwrap();
    new(v.snapshot(loc, Meta::new(reason), at).unwrap())
}

#[test]
fn a_career_goes_missing_and_comes_back() {
    let root = tmp("career-drop");
    let loc = steam(&root, "843502855", &[1u8; 900]);
    let v = vault(&root, &loc);
    write_index(&root, &[("p:1", "present", 2016, 7), ("p:2", "present", 2017, 1)]);
    let a = take(&v, &loc, Reason::AfterSession, T0, &[2u8; 900]);
    assert!(a.careers_ref.is_some());
    let t = v.careers();
    assert_eq!(t.careers.len(), 2);
    assert!(t.careers.iter().all(|r| r.status == careers::Status::Present));
    assert_eq!(t.find("p:1").unwrap().place().unwrap(), "season 2016, round 7");
    assert_eq!(t.find("p:1").unwrap().last_played, Some(1_793_400_000));

    write_index(&root, &[("p:2", "present", 2017, 2)]);
    let b = take(&v, &loc, Reason::BeforeSession, T0 + 100, &[3u8; 900]);
    let t = v.careers();
    assert_eq!(
        t.find("p:1").unwrap().status.slug(),
        format!("missing-since:{}", b.id)
    );
    assert_eq!(t.find("p:1").unwrap().last_seen, a.id);
    assert_eq!(t.find("p:1").unwrap().found_at, Some(T0 + 100));
    assert_eq!(t.find("p:2").unwrap().status, careers::Status::Present);
    assert_eq!(t.drop_to_show().len(), 1);
    assert!(
        v.find(&a.id)
            .unwrap()
            .pinned
            .contains(&"holds-missing-career".to_owned())
    );
    assert!(kinds(&v).contains(&EventKind::CareersMissing));

    let diff = v.preview(&v.find(&a.id).unwrap()).unwrap();
    assert_eq!(diff.get_back.len(), 1);
    assert_eq!(diff.get_back[0].career, "p:1");
    assert_eq!(diff.progress.len(), 1);
    assert!(diff.lose.is_empty());

    write_index(&root, &[("p:1", "present", 2016, 7), ("p:2", "present", 2017, 2)]);
    let c = take(&v, &loc, Reason::AfterSession, T0 + 200, &[4u8; 900]);
    let t = v.careers();
    assert_eq!(t.find("p:1").unwrap().status, careers::Status::Present);
    assert_eq!(t.find("p:1").unwrap().last_seen, c.id);
    assert!(t.drop_to_show().is_empty());
}

#[test]
fn a_dismissed_drop_stays_dismissed_until_the_next_one() {
    let root = tmp("career-dismiss");
    let loc = steam(&root, "843502855", &[1u8; 900]);
    let v = vault(&root, &loc);
    write_index(&root, &[("p:1", "present", 2016, 7)]);
    take(&v, &loc, Reason::AfterSession, T0, &[2u8; 900]);
    write_index(&root, &[("p:9", "present", 2016, 7)]);
    take(&v, &loc, Reason::BeforeSession, T0 + 10, &[3u8; 900]);
    assert_eq!(v.careers().drop_to_show().len(), 1);
    v.dismiss_drop().unwrap();
    assert!(v.careers().drop_to_show().is_empty());
    take(&v, &loc, Reason::AfterSession, T0 + 20, &[4u8; 900]);
    assert!(v.careers().drop_to_show().is_empty());
    assert_eq!(v.careers().missing().count(), 1);
}

#[test]
fn a_career_deleted_in_game_is_not_a_drop() {
    let root = tmp("career-deleted");
    let loc = steam(&root, "843502855", &[1u8; 900]);
    let v = vault(&root, &loc);
    write_index(&root, &[("p:1", "present", 2016, 7), ("p:2", "present", 2017, 1)]);
    take(&v, &loc, Reason::AfterSession, T0, &[2u8; 900]);
    write_index(&root, &[("p:1", "deleted", 2016, 7), ("p:2", "present", 2017, 1)]);
    take(&v, &loc, Reason::AfterSession, T0 + 10, &[3u8; 900]);
    let t = v.careers();
    assert_eq!(t.find("p:1").unwrap().status, careers::Status::DeletedInGame);
    assert!(t.drop_to_show().is_empty());
    take(&v, &loc, Reason::AfterSession, T0 + 20, &[4u8; 900]);
    assert_eq!(
        v.careers().find("p:1").unwrap().status,
        careers::Status::DeletedInGame
    );
}

#[test]
fn the_first_run_with_no_index_makes_no_claims() {
    let root = tmp("career-first");
    let loc = steam(&root, "843502855", &[1u8; 900]);
    let v = vault(&root, &loc);
    let a = take(&v, &loc, Reason::LauncherStart, T0, &[2u8; 900]);
    assert_eq!(a.careers_ref, None);
    assert!(v.careers().careers.is_empty());
    assert!(!v.dir.join(careers::FILE).exists());
    assert!(v.preview(&a).is_none());
    write_index(&root, &[("p:1", "present", 2016, 7)]);
    let b = take(&v, &loc, Reason::AfterSession, T0 + 10, &[3u8; 900]);
    assert!(b.careers_ref.is_some());
    assert!(v.careers().drop_to_show().is_empty());
    assert!(v.preview(&a).is_none());
}

#[test]
fn an_unchanged_save_still_sees_the_index_and_prune_keeps_referenced_indexes() {
    let root = tmp("career-unchanged");
    let loc = steam(&root, "843502855", &[1u8; 900]);
    let v = vault(&root, &loc);
    write_index(&root, &[("p:1", "present", 2016, 7)]);
    let a = take(&v, &loc, Reason::AfterSession, T0, &[2u8; 900]);
    write_index(&root, &[("p:1", "missing", 2016, 7)]);
    assert!(matches!(
        v.snapshot(&loc, Meta::new(Reason::BeforeSession), T0 + 5)
            .unwrap(),
        Taken::Unchanged(_)
    ));
    assert_eq!(v.careers().missing().count(), 1);
    let rel = a.careers_ref.unwrap();
    v.prune(T0 + 6).unwrap();
    assert!(v.dir.join(rel).is_file());
    std::fs::write(v.dir.join("index/stray.json"), "{}").unwrap();
    v.prune(T0 + 7).unwrap();
    assert!(!v.dir.join("index/stray.json").exists());
}

#[test]
fn pin_and_unpin_only_touch_the_players_pin() {
    let root = tmp("career-pin");
    let loc = steam(&root, "843502855", &[1u8; 900]);
    let v = vault(&root, &loc);
    let a = take(&v, &loc, Reason::AfterSession, T0, &[2u8; 900]);
    v.pin(&a.id, "backup", T0).unwrap();
    v.pin(&a.id, "pinned", T0).unwrap();
    let s = v.unpin_why(&a.id, "pinned", T0 + 1).unwrap();
    assert_eq!(s.pinned, vec!["backup".to_owned()]);
}

#[test]
fn a_restore_puts_the_snapshots_careers_back_and_dates_the_files() {
    let root = tmp("restore-careers");
    let loc = steam(&root, "7", &[1u8; 900]);
    let v = vault(&root, &loc);
    write_index(&root, &[("p:1", "present", 2016, 7), ("p:2", "present", 2017, 1)]);
    let a = take(&v, &loc, Reason::AfterSession, T0, &[2u8; 900]);
    let other = serde_json::json!({ "career": "q:9", "profile": "other", "label": "q", "status": "present" });
    let path = root.join("data/careers/index.json");
    let mut now: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    now["careers"].as_array_mut().unwrap().push(other);
    std::fs::write(&path, now.to_string()).unwrap();
    let a = v.find(&a.id).unwrap();
    write_index(&root, &[("p:2", "present", 2017, 5), ("p:3", "present", 2018, 1)]);
    take(&v, &loc, Reason::AfterSession, T0 + 100, &[3u8; 900]);

    restore(&v, &a.id, &loc, Running::default(), T0 + 200).unwrap();
    let listed = crate::stdlib::careers::read(&crate::stdlib::careers::data_path(&root.join("data")));
    let ids: Vec<&str> = listed.iter().map(|c| c.career.as_str()).collect();
    assert!(ids.contains(&"p:1") && ids.contains(&"p:2"), "{ids:?}");
    assert!(!ids.contains(&"p:3"), "{ids:?}");
    let mtime = std::fs::metadata(loc.dir.join(DATA)).unwrap().modified().unwrap();
    assert_eq!(
        mtime,
        std::time::UNIX_EPOCH + std::time::Duration::from_secs((T0 + 200) as u64)
    );
}

#[test]
fn folder_listing_names_each_file_with_size_and_time() {
    let dir = tmp("listing");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("savegame@profile#a"), b"12345").unwrap();
    std::fs::write(dir.join("remotecache.vdf"), b"x").unwrap();
    let line = folder_listing(&dir);
    assert!(line.contains("remotecache.vdf 1 bytes 20"), "{line}");
    assert!(line.contains("savegame@profile#a 5 bytes 20"), "{line}");
    assert!(line.find("remotecache").unwrap() < line.find("savegame").unwrap());
    assert!(folder_listing(&dir.join("missing")).contains("could not be read"));
    std::fs::remove_dir_all(&dir).ok();
}

fn launcher_loc(root: &Path, profile: &str, data: &[u8]) -> Location {
    let loc = Location::launcher(&root.join("data"), profile);
    std::fs::create_dir_all(&loc.dir).unwrap();
    std::fs::write(loc.dir.join(DATA), data).unwrap();
    std::fs::write(loc.dir.join(INDEX), [7u8; 416]).unwrap();
    loc
}

#[test]
fn a_launcher_vault_lists_only_what_it_restores_and_restores_each() {
    let root = tmp("launcher-list");
    let loc = launcher_loc(&root, "2016-2025", &[1u8; 900]);
    let v = vault(&root, &loc);
    let a = new(v.snapshot(&loc, Meta::new(Reason::Imported), T0).unwrap());
    std::fs::write(loc.dir.join(DATA), [2u8; 1200]).unwrap();
    let b = new(v
        .snapshot(&loc, Meta::new(Reason::ChangedOutside), T0 + 60)
        .unwrap());
    std::fs::write(loc.dir.join(DATA), [3u8; 600]).unwrap();
    for snap in v.list().unwrap() {
        assert_eq!(snap.source, v.source);
        let r = restore(&v, &snap.id, &loc, Running::default(), T0 + 600).unwrap();
        assert!(r.log_line().contains(&snap.id));
    }
    assert!(v.find(&a.id).is_ok() && v.find(&b.id).is_ok());
}

#[test]
fn snapshots_copied_between_profiles_restore_into_the_profile_that_lists_them() {
    let root = tmp("launcher-copied");
    let from = launcher_loc(&root, "2016-2025", &[1u8; 900]);
    let vf = vault(&root, &from);
    let snap = new(vf.snapshot(&from, Meta::new(Reason::AfterSession), T0).unwrap());
    std::fs::write(from.dir.join(DATA), [2u8; 1200]).unwrap();
    new(vf
        .snapshot(&from, Meta::new(Reason::ChangedOutside), T0 + 60)
        .unwrap());

    let to = launcher_loc(&root, "2016-2025-12", &[9u8; 300]);
    let vt = vault(&root, &to);
    copy_dir(&vf.dir, &vt.dir);

    let listed = vt.list().unwrap();
    assert_eq!(listed.len(), 2);
    for s in &listed {
        assert_eq!(
            s.source, vt.source,
            "the page keys a version by the vault that lists it"
        );
        let open = Vault::open(&root.join("data"), &s.source).unwrap();
        assert_eq!(open.find(&s.id).unwrap().id, s.id);
    }
    let r = restore(&vt, &snap.id, &to, Running::default(), T0 + 600).unwrap();
    assert!(r.log_line().contains(&snap.id));
    assert_eq!(std::fs::read(to.dir.join(DATA)).unwrap(), [1u8; 900]);
    assert_eq!(std::fs::read(from.dir.join(DATA)).unwrap(), [2u8; 1200]);
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let dest = to.join(e.file_name());
        match e.file_type().unwrap().is_dir() {
            true => copy_dir(&e.path(), &dest),
            false => {
                std::fs::copy(e.path(), dest).unwrap();
            }
        }
    }
}
