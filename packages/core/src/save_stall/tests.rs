use std::path::{Path, PathBuf};

use super::*;
use crate::vault::restore::Running;

const DATA: &str = "savegame@profile#NXDSMWW.BWW";
const INDEX: &str = "savegame@profile#QKRHMYXE";
const T0: i64 = 1_793_404_800;

/// The perf timeline of a session that read the profile and sat on the start screen until Stop (Jonnie,
/// launcher 1.8.4, 4 Nov 2026: 162 s on splash_transition).
const STALLED_PERF: &str = "\
== Summary
uptime: 212 s

== Timeline (last 15; each row runs until the next one)
at          took ms   frames   slow   worst ms  swofty ms  what
00:00:00      13963        0      0          0      18.19  start  [core assets manifest parse 14.82 ms]
00:00:13        283        0      0          0       6.59  flow #e11edb87 Entering
00:00:23        745       40      0         36      49.85  screen video_1
00:00:24      11384      340      2        140     168.35  screen seizure_warning
00:00:36       8409      451      3        636     106.54  screen splash
00:00:44       3566      356      0         23       9.44  screen render_badge
00:00:48       1716      171      0         13      44.24  flow #a4d3a4b2 Exiting; flow #fd9f4f95 Entering; flow #fd9f4f95 Exiting; flow #04fc8ffa Entering
00:00:50     104956     6070     23       2251     278.68  screen splash_transition  [core flow tick (events, career poll) 250.51 ms]
00:02:35      56379     2930      3       6933          -  flow #04fc8ffa Exiting (current)
";

/// A session that went on to the menus.
const PASSED_PERF: &str = "\
== Summary
uptime: 900 s

== Timeline (last 15; each row runs until the next one)
at          took ms   frames   slow   worst ms  swofty ms  what
00:00:36       8409      451      3        636     106.54  screen splash
00:00:50      14956     6070     23       2251     278.68  screen splash_transition
00:01:05       2585      469      7       1420          -  screen fe_home
00:02:05       2585      469      7       1420          -  screen pause (current)
";

const LOADER: &str = "\
2026-11-04 03:32:34.875 [ 5256] [saves] profile load 1: read from launcher saves (profile colorgrading-testing, C:\\Users\\x\\AppData\\Local\\Swofty\\saves\\colorgrading-testing\\remote); save file savegame@profile#QKRHMYXE: 416 bytes, modified 2026-10-31 03:36:07 UTC
2026-11-04 03:32:34.875 [ 5256] [saves] profile load 1: manager 5 slot 2: gm_my_team id=16 key=66f051ee751e04d1 career=my_team:66f051ee751e04d1 flags=0x20 setup=13 season=1 weekend=0 date=1793403769 name=\"My Team Save 3\"
2026-11-04 03:32:43.658 [24120] [game] ACTOR_WARN (warn): MISSING RESOURCE [.emb?context=noyear-custom-myteam] using fallback [context=default]
";

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("swofty-save-stall-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn store(root: &Path, profiles: &[&str]) -> Store {
    let data = root.join("data");
    std::fs::create_dir_all(data.join("profiles")).unwrap();
    for p in profiles {
        std::fs::write(data.join(format!("profiles/{p}.json")), "{}").unwrap();
    }
    Store::new(data)
}

fn put(loc: &Location, bytes: &[u8]) {
    std::fs::create_dir_all(&loc.dir).unwrap();
    std::fs::write(loc.dir.join(DATA), bytes).unwrap();
    std::fs::write(loc.dir.join(INDEX), [7u8; 416]).unwrap();
}

fn steam(root: &Path, bytes: &[u8]) -> Location {
    let remote = root.join("steam/userdata/221994709/3059520/remote");
    std::fs::create_dir_all(&remote).unwrap();
    let loc = Location::steam(&remote).unwrap();
    put(&loc, bytes);
    loc
}

/// A launcher session on `loc`: the vault's before and after snapshots, and the session folder with its
/// loader log and perf timeline.
fn session(store: &Store, loc: &Location, stamp: &str, perf: &str, at: i64) {
    let id = format!("req-{stamp}");
    let v = Vault::of(&store.root, loc).unwrap();
    let mut meta = Meta::new(Reason::BeforeSession).profile("colorgrading-testing");
    meta.session_id = Some(id.clone());
    v.before_session(loc, meta.clone(), at).unwrap();
    meta.reason = Reason::AfterSession;
    v.after_session(loc, meta, None, at + 200).unwrap();
    let dir = store.sessions_dir().join(stamp);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("request_id"), format!("{id}\n")).unwrap();
    std::fs::write(dir.join("loader.log"), LOADER).unwrap();
    std::fs::write(dir.join("perf.log"), perf).unwrap();
}

fn data_of(loc: &Location) -> Vec<u8> {
    std::fs::read(loc.dir.join(DATA)).unwrap()
}

fn sha(bytes: &[u8]) -> String {
    let p = tmp("sha").join("f");
    std::fs::write(&p, bytes).unwrap();
    sha256_file(&p).unwrap()
}

fn seen(session: &str, before: &str, outcome: Outcome) -> Seen {
    Seen {
        session: session.into(),
        before: before.into(),
        size: 932_920,
        after: Some(before.into()),
        profile: Some("colorgrading-testing".into()),
        outcome,
    }
}

#[test]
fn a_session_that_read_the_profile_and_sat_on_the_start_screen_stalled() {
    let root = tmp("outcome");
    let s = store(&root, &[]);
    let dir = s.sessions_dir().join("20261104-023146");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("loader.log"), LOADER).unwrap();
    std::fs::write(dir.join("perf.log"), STALLED_PERF).unwrap();
    assert_eq!(outcome(&dir, &s), Outcome::Stalled(162.0));
    std::fs::write(dir.join("perf.log"), PASSED_PERF).unwrap();
    assert_eq!(outcome(&dir, &s), Outcome::Passed);
    let badge = STALLED_PERF.replace("screen splash_transition", "flow #1");
    std::fs::write(dir.join("perf.log"), badge).unwrap();
    assert_eq!(
        outcome(&dir, &s),
        Outcome::Unknown,
        "ended on render_badge, before the main menu"
    );
    let slow = STALLED_PERF.replace("uptime: 212 s", "uptime: 150 s");
    std::fs::write(dir.join("perf.log"), slow).unwrap();
    assert_eq!(
        outcome(&dir, &s),
        Outcome::Unknown,
        "100 s can still be a slow menu load"
    );
    std::fs::write(dir.join("perf.log"), STALLED_PERF).unwrap();
    std::fs::write(dir.join("loader.log"), "no profile load here\n").unwrap();
    assert_eq!(
        outcome(&dir, &s),
        Outcome::Unknown,
        "a stall before any profile load is not the save's"
    );
}

#[test]
fn two_stalls_on_the_same_save_learn_it() {
    let mut ledger = Ledger::default();
    let one = [seen("a", "bad", Outcome::Stalled(162.0))];
    assert!(
        learn(&mut ledger, &one, T0).is_empty(),
        "one short stall can be a slow load"
    );
    let two = [
        seen("a", "bad", Outcome::Stalled(162.0)),
        seen("b", "bad", Outcome::Stalled(130.0)),
        seen("c", "good", Outcome::Passed),
    ];
    let lines = learn(&mut ledger, &two, T0);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains("2 session(s) (a, b), up to 162 s"),
        "{}",
        lines[0]
    );
    assert_eq!(ledger.saves.len(), 1);
    assert_eq!(ledger.saves[0].sha256, "bad");
    assert_eq!(ledger.saves[0].secs, 162);
    assert!(learn(&mut ledger, &two, T0).is_empty(), "learned once");
    let long = [seen("d", "worse", Outcome::Stalled(LONG_SECS))];
    assert_eq!(learn(&mut ledger, &long, T0).len(), 1, "one long stall is enough");
}

#[test]
fn a_save_that_ever_loaded_is_never_learned_and_is_forgotten() {
    let mut ledger = Ledger::default();
    let mut wrote = seen("c", "bad", Outcome::Unknown);
    wrote.after = Some("newer".into());
    let s = [
        seen("a", "bad", Outcome::Stalled(400.0)),
        seen("b", "bad", Outcome::Stalled(400.0)),
        wrote,
    ];
    assert!(
        learn(&mut ledger, &s, T0).is_empty(),
        "the game wrote it, so it got past the load"
    );
    assert!(ledger.saves.is_empty());
    learn(&mut ledger, &s[..2], T0);
    assert_eq!(ledger.saves.len(), 1);
    learn(&mut ledger, &[seen("e", "bad", Outcome::Passed)], T0);
    assert!(
        ledger.saves.is_empty(),
        "a later session got past the start screen with it"
    );
}

#[test]
fn the_sweep_moves_a_stalled_store_save_aside_and_puts_back_the_last_good_one() {
    let root = tmp("sweep");
    let s = store(&root, &["colorgrading-testing"]);
    let loc = crate::launcher_saves::location(&s.root, "colorgrading-testing");
    let good = [3u8; 715_128 / 100];
    let bad = [9u8; 932_920 / 100];
    put(&loc, &good);
    session(&s, &loc, "20261101-120000", PASSED_PERF, T0);
    put(&loc, &bad);
    session(&s, &loc, "20261104-023037", STALLED_PERF, T0 + 3 * 86_400);
    session(&s, &loc, "20261104-023146", STALLED_PERF, T0 + 3 * 86_400 + 300);

    let lines = sweep(&s, None, Running::default(), T0 + 4 * 86_400);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("in 2 session(s)"), "{}", lines[0]);
    assert!(
        lines[1].contains("kept F1 25 on Loading... after Press any button"),
        "{}",
        lines[1]
    );
    assert!(lines[1].contains("is back in its place"), "{}", lines[1]);
    assert_eq!(data_of(&loc), good, "the version that loaded is back");

    let ledger = Ledger::read(&s.root);
    assert_eq!(ledger.saves[0].sha256, sha(&bad));
    assert_eq!(ledger.moved.len(), 1);
    let kept = ledger.moved[0].kept.clone().unwrap();
    let v = Vault::of(&s.root, &loc).unwrap();
    let snap = v.find(&kept).unwrap();
    assert!(snap.pinned.iter().any(|p| p == PIN), "{:?}", snap.pinned);
    assert_eq!(
        data_entry(&snap).unwrap().sha256,
        sha(&bad),
        "the vault keeps the stalled save"
    );

    assert!(
        sweep(&s, None, Running::default(), T0 + 5 * 86_400).is_empty(),
        "nothing left to do"
    );
    put(&loc, &bad);
    assert!(
        sweep(&s, None, Running::default(), T0 + 5 * 86_400).is_empty(),
        "a save the player put back on purpose stays"
    );
}

#[test]
fn the_same_bytes_in_the_steam_save_go_too_once_steam_is_closed() {
    let root = tmp("steam");
    let s = store(&root, &["colorgrading-testing"]);
    let good = [3u8; 7_151];
    let bad = [9u8; 9_329];
    let steam = steam(&root, &good);
    Vault::of(&s.root, &steam)
        .unwrap()
        .snapshot(&steam, Meta::new(Reason::LauncherStart), T0)
        .unwrap();
    put(&steam, &bad);
    let loc = crate::launcher_saves::location(&s.root, "colorgrading-testing");
    put(&loc, &bad);
    session(&s, &loc, "20261104-023037", STALLED_PERF, T0 + 100);
    session(&s, &loc, "20261104-023146", STALLED_PERF, T0 + 400);

    let running = Running {
        steam: true,
        ..Running::default()
    };
    let lines = sweep(&s, Some(&steam), running, T0 + 1_000);
    assert!(
        lines
            .iter()
            .any(|l| l.contains("moved aside once nothing holds it") && l.contains("Steam is running")),
        "{lines:?}"
    );
    assert_eq!(data_of(&steam), bad, "Steam running: the save waits");
    assert_eq!(
        data_of(&loc),
        good,
        "the profile store takes the Steam save's older version"
    );

    let lines = sweep(&s, Some(&steam), Running::default(), T0 + 2_000);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(data_of(&steam), good);
}

#[test]
fn with_no_other_version_a_store_is_emptied_and_a_steam_save_stays() {
    let root = tmp("empty");
    let s = store(&root, &["colorgrading-testing"]);
    let bad = [9u8; 9_329];
    let loc = crate::launcher_saves::location(&s.root, "colorgrading-testing");
    put(&loc, &bad);
    let steam = steam(&root, &bad);
    session(&s, &loc, "20261104-023037", STALLED_PERF, T0 + 100);
    session(&s, &loc, "20261104-023146", STALLED_PERF, T0 + 400);

    let lines = sweep(&s, Some(&steam), Running::default(), T0 + 1_000);
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(
        lines[1].contains("F1 25 starts a new profile save"),
        "{}",
        lines[1]
    );
    assert!(
        lines[1].contains("--source launcher-colorgrading-testing"),
        "{}",
        lines[1]
    );
    assert!(!crate::launcher_saves::has_files(&loc));
    assert!(
        lines[2].contains("Steam Cloud would bring it back, so it stays"),
        "{}",
        lines[2]
    );
    assert_eq!(data_of(&steam), bad);

    let kept = Ledger::read(&s.root).moved[0].kept.clone().unwrap();
    let snap = Vault::of(&s.root, &loc).unwrap().find(&kept).unwrap();
    assert!(snap.pinned.iter().any(|p| p == PIN));
}

#[test]
fn the_launcher_never_copies_a_stalled_save() {
    let root = tmp("copy");
    let s = store(&root, &["colorgrading-testing", "default"]);
    let bad = [9u8; 9_329];
    let steam = steam(&root, &bad);
    let mut ledger = Ledger::default();
    learn(
        &mut ledger,
        &[seen("a", &sha(&bad), Outcome::Stalled(LONG_SECS))],
        T0,
    );
    ledger.saves[0].size = bad.len() as u64;
    ledger.write(&s.root).unwrap();

    let e = crate::launcher_saves::seed(&s.root, "default", &steam, Reason::Imported, T0).unwrap_err();
    assert!(e.to_string().contains("does not copy it"), "{e}");
    assert!(!crate::launcher_saves::has_files(
        &crate::launcher_saves::location(&s.root, "default")
    ));

    let e = crate::launcher_saves::copy_to(&s.root, &steam, "default", Running::default(), T0).unwrap_err();
    assert!(e.to_string().contains("does not copy it"), "{e}");

    let from = crate::launcher_saves::location(&s.root, "colorgrading-testing");
    put(&from, &bad);
    let e = crate::launcher_saves::copy_to(&s.root, &from, "default", Running::default(), T0).unwrap_err();
    assert!(e.to_string().contains("does not copy it"), "{e}");
    let e = crate::launcher_saves::export(&s.root, "colorgrading-testing", &steam, Running::default(), T0)
        .unwrap_err();
    assert!(e.to_string().contains("does not copy it"), "{e}");

    put(&steam, &[3u8; 7_151]);
    assert!(crate::launcher_saves::seed(&s.root, "default", &steam, Reason::Imported, T0).is_ok());
}

#[test]
#[ignore]
fn reads_real_bundles() {
    let Some(roots) = std::env::var_os("SWOFTY_STALL_CASES") else {
        return;
    };
    let s = store(&tmp("bundles"), &[]);
    let (mut stalled, mut passed, mut unknown) = (0, 0, 0);
    for root in std::env::split_paths(&roots) {
        for case in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let sessions = case.path().join("logs/sessions");
            for dir in std::fs::read_dir(&sessions).into_iter().flatten().flatten() {
                match outcome(&dir.path(), &s) {
                    Outcome::Stalled(secs) => {
                        stalled += 1;
                        println!("STALL {} {secs}", dir.path().display());
                    }
                    Outcome::Passed => passed += 1,
                    Outcome::Unknown => unknown += 1,
                }
            }
        }
    }
    println!("stalled {stalled} passed {passed} unknown {unknown}");
}
