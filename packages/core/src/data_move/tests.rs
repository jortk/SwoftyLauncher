use std::io::ErrorKind;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use super::*;

struct Quiet;

impl Progress for Quiet {
    fn report(&self, _: f32, _: &str) {}
}

#[derive(Default)]
struct Seen(Mutex<Vec<f32>>);

impl Progress for Seen {
    fn report(&self, done: f32, _: &str) {
        self.0.lock().unwrap().push(done);
    }
}

fn temp(name: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "swofty-data-move-{name}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

fn seed(data: &Path) {
    write(
        &data.join("addons/classic-seasons/swofty.json"),
        "{\"id\":\"cs\"}",
    );
    write(&data.join("profiles/default.json"), "{}");
    write(&data.join("state/session.json"), "{\"a\":1}");
    write(&data.join("saves/career/one.bin"), &"x".repeat(3 << 20));
    std::fs::create_dir_all(data.join("empty-folder")).unwrap();
}

fn whole(at: &Path) {
    assert_eq!(
        read(&at.join("addons/classic-seasons/swofty.json")),
        "{\"id\":\"cs\"}"
    );
    assert_eq!(read(&at.join("profiles/default.json")), "{}");
    assert_eq!(read(&at.join("state/session.json")), "{\"a\":1}");
    assert_eq!(read(&at.join("saves/career/one.bin")).len(), 3 << 20);
    assert!(at.join("empty-folder").is_dir());
}

fn loc(default: &Path) -> Location {
    Location {
        default: default.to_path_buf(),
        env: None,
        install: None,
        games: Vec::new(),
    }
}

const fn ops(rename: fn(&Path, &Path) -> std::io::Result<()>, same_volume: bool) -> Ops {
    Ops {
        rename,
        free: |_| None,
        same_volume: if same_volume { |_, _| true } else { |_, _| false },
        link: link::create,
    }
}

const SAME_VOLUME: Ops = ops(|a, b| std::fs::rename(a, b), true);
const OTHER_DRIVE: Ops = ops(|_, _| Err(std::io::Error::from(ErrorKind::CrossesDevices)), false);

fn no_cancel() -> AtomicBool {
    AtomicBool::new(false)
}

fn journal_left(default: &Path) -> bool {
    Journal::path(default).exists()
}

#[cfg(unix)]
fn is_root() -> bool {
    use std::os::unix::fs::PermissionsExt;
    let probe = temp("root-probe");
    let file = probe.join("f");
    write(&file, "x");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
    let readable = std::fs::read(&file).is_ok();
    std::fs::remove_dir_all(&probe).ok();
    readable
}

fn refused(loc: &Location, to: &Path) -> Error {
    plan(loc, to, &SAME_VOLUME).unwrap_err()
}

#[test]
fn refuses_while_the_env_variable_sets_the_folder() {
    let dir = temp("env");
    let default = dir.join("Swofty");
    seed(&default);
    let mut l = loc(&default);
    l.env = Some(dir.join("elsewhere"));
    assert_eq!(
        refused(&l, &dir.join("new")),
        Error::EnvSet(dir.join("elsewhere"))
    );
    assert!(refused(&l, &dir.join("new")).to_string().contains(ENV));
}

#[test]
fn refuses_a_relative_target() {
    let dir = temp("relative");
    seed(&dir.join("Swofty"));
    assert_eq!(
        refused(&loc(&dir.join("Swofty")), Path::new("new/data")),
        Error::NotAbsolute
    );
}

#[test]
fn refuses_when_there_is_nothing_to_move() {
    let dir = temp("nothing");
    assert_eq!(
        refused(&loc(&dir.join("Swofty")), &dir.join("new")),
        Error::NothingToMove
    );
}

#[test]
fn refuses_where_the_data_already_is() {
    let dir = temp("already");
    let default = dir.join("Swofty");
    seed(&default);
    assert_eq!(refused(&loc(&default), &default), Error::AlreadyThere(default));
}

#[test]
fn refuses_inside_and_around_the_current_folder() {
    let dir = temp("current");
    let default = dir.join("Swofty");
    seed(&default);
    let l = loc(&default);
    assert_eq!(
        refused(&l, &default.join("deeper")),
        Error::InsideCurrent(default.clone())
    );
    assert_eq!(refused(&l, &dir), Error::ContainsCurrent(default));
}

#[test]
fn refuses_the_default_path_while_it_links_elsewhere() {
    let dir = temp("is-default");
    let (default, moved) = (dir.join("Swofty"), dir.join("D/Data"));
    seed(&moved);
    link::create(&moved, &default).unwrap();
    assert_eq!(
        refused(&loc(&default), &default),
        Error::IsDefault(default.clone())
    );
    assert_eq!(
        refused(&loc(&default), &default.join("inner")),
        Error::InsideCurrent(moved)
    );
}

#[test]
fn refuses_inside_or_around_the_install_folder() {
    let dir = temp("install");
    let default = dir.join("Swofty");
    seed(&default);
    let install = dir.join("apps/Swofty Launcher");
    std::fs::create_dir_all(&install).unwrap();
    let mut l = loc(&default);
    l.install = Some(install.clone());
    assert_eq!(
        refused(&l, &install.join("data")),
        Error::InsideInstall(install.clone())
    );
    assert_eq!(refused(&l, &dir.join("apps")), Error::ContainsInstall(install));
}

#[test]
fn refuses_inside_or_around_a_game_folder() {
    let dir = temp("game");
    let default = dir.join("Swofty");
    seed(&default);
    let game = dir.join("steam/F1 25");
    std::fs::create_dir_all(&game).unwrap();
    let mut l = loc(&default);
    l.games = vec![game.clone()];
    assert_eq!(
        refused(&l, &game.join("swofty-data")),
        Error::InsideGame(game.clone())
    );
    assert_eq!(refused(&l, &dir.join("steam")), Error::ContainsGame(game));
}

#[test]
fn refuses_a_target_that_is_not_empty_or_not_a_folder() {
    let dir = temp("vacant");
    let default = dir.join("Swofty");
    seed(&default);
    let full = dir.join("full");
    write(&full.join("a.txt"), "a");
    assert_eq!(refused(&loc(&default), &full), Error::NotEmpty(full));
    let file = dir.join("file");
    write(&file, "a");
    assert_eq!(refused(&loc(&default), &file), Error::NotFolder(file));
}

#[test]
fn accepts_an_empty_existing_folder_and_leaves_it_alone() {
    let dir = temp("empty-ok");
    let default = dir.join("Swofty");
    seed(&default);
    let to = dir.join("empty");
    std::fs::create_dir_all(&to).unwrap();
    let p = plan(&loc(&default), &to, &SAME_VOLUME).unwrap();
    assert!(to.is_dir());
    assert_eq!(std::fs::read_dir(&to).unwrap().count(), 0);
    assert_eq!(p.files, 4);
    assert_eq!(p.bytes, (3 << 20) + 2 + 7 + 11);
}

#[test]
fn planning_leaves_no_folder_behind() {
    let dir = temp("plan-clean");
    let default = dir.join("Swofty");
    seed(&default);
    plan(&loc(&default), &dir.join("a/b/new"), &SAME_VOLUME).unwrap();
    assert!(!dir.join("a").exists());
}

#[cfg(unix)]
#[test]
fn refuses_a_target_that_cannot_be_written() {
    use std::os::unix::fs::PermissionsExt;
    if is_root() {
        return;
    }
    let dir = temp("readonly");
    let default = dir.join("Swofty");
    seed(&default);
    let locked = dir.join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let err = refused(&loc(&default), &locked.join("data"));
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(err, Error::NotWritable(..)), "{err:?}");
}

#[test]
fn refuses_when_the_drive_lacks_room_with_a_margin() {
    let dir = temp("space");
    let default = dir.join("Swofty");
    seed(&default);
    let tight = Ops {
        free: |_| Some((3 << 20) + 100),
        ..OTHER_DRIVE
    };
    let err = plan(&loc(&default), &dir.join("new"), &tight).unwrap_err();
    assert!(matches!(err, Error::NotEnoughSpace { .. }), "{err:?}");
    assert!(err.to_string().contains("free"));
    let roomy = Ops {
        free: |_| Some(64 << 30),
        ..OTHER_DRIVE
    };
    assert!(plan(&loc(&default), &dir.join("new"), &roomy).is_ok());
    let same = Ops {
        free: |_| Some(0),
        ..SAME_VOLUME
    };
    assert!(
        plan(&loc(&default), &dir.join("new"), &same).is_ok(),
        "a rename needs no room"
    );
}

#[test]
fn refuses_when_the_link_target_is_gone() {
    let dir = temp("broken");
    let (default, gone) = (dir.join("Swofty"), dir.join("D/Data"));
    seed(&gone);
    link::create(&gone, &default).unwrap();
    std::fs::remove_dir_all(&gone).unwrap();
    assert_eq!(
        refused(&loc(&default), &dir.join("new")),
        Error::LinkBroken {
            default,
            target: gone
        }
    );
}

#[test]
fn renames_on_the_same_volume_and_links_the_default_to_it() {
    let dir = temp("rename");
    let (default, to) = (dir.join("Local/Swofty"), dir.join("D/SwoftyData"));
    seed(&default);
    let l = loc(&default);
    let p = plan(&l, &to, &SAME_VOLUME).unwrap();
    let moved = move_data(&p, &SAME_VOLUME, &Quiet, &no_cancel()).unwrap();
    assert_eq!(
        (moved.method, moved.switched, moved.leftover),
        (Method::Renamed, true, None)
    );
    whole(&to);
    whole(&default);
    assert_eq!(link::target(&default), Some(to.clone()));
    assert_eq!(l.current(), to);
    assert!(!journal_left(&default));
    assert!(!dir.join("Local/Swofty.old").exists());
}

#[test]
fn a_blocked_rename_falls_back_to_a_copy_and_leaves_the_old_folder_alone() {
    let dir = temp("rename-blocked");
    let (default, to) = (dir.join("Swofty"), dir.join("D/Data"));
    seed(&default);
    let blocked = Ops {
        rename: |_, _| Err(std::io::Error::from(ErrorKind::PermissionDenied)),
        ..SAME_VOLUME
    };
    let p = plan(&loc(&default), &to, &blocked).unwrap();
    let moved = move_data(&p, &blocked, &Quiet, &no_cancel()).unwrap();
    assert_eq!((moved.method, moved.switched), (Method::Copied, false));
    assert!(spot_is_folder(&default));
    whole(&default);
    whole(&to);
    assert!(journal_left(&default));
}

fn spot_is_folder(path: &Path) -> bool {
    link::target(path).is_none() && path.is_dir()
}

#[test]
fn a_copy_changes_nothing_until_recover_switches_it() {
    let dir = temp("copy");
    let (default, to) = (dir.join("Local/Swofty"), dir.join("D/SwoftyData"));
    seed(&default);
    let p = plan(&loc(&default), &to, &OTHER_DRIVE).unwrap();
    let seen = Seen::default();
    let moved = move_data(&p, &OTHER_DRIVE, &seen, &no_cancel()).unwrap();
    assert_eq!((moved.method, moved.switched), (Method::Copied, false));
    assert!(spot_is_folder(&default));
    whole(&default);
    whole(&to);
    let steps = seen.0.lock().unwrap().clone();
    assert!(
        steps.windows(2).all(|w| w[0] <= w[1]) && steps.last() == Some(&1.0),
        "{steps:?}"
    );
    let done = recover(&default, &OTHER_DRIVE).unwrap().unwrap();
    assert_eq!(
        done,
        Recovered {
            to: to.clone(),
            leftover: None,
            adopted: false
        }
    );
    assert_eq!(link::target(&default), Some(to.clone()));
    whole(&default);
    whole(&to);
    assert!(!dir.join("Local/Swofty.old").exists());
    assert!(!journal_left(&default));
    assert!(recover(&default, &OTHER_DRIVE).is_none());
}

#[test]
fn recover_picks_up_files_changed_after_the_copy() {
    let dir = temp("delta");
    let (default, to) = (dir.join("Swofty"), dir.join("D/Data"));
    seed(&default);
    let p = plan(&loc(&default), &to, &OTHER_DRIVE).unwrap();
    move_data(&p, &OTHER_DRIVE, &Quiet, &no_cancel()).unwrap();
    write(&default.join("config.json"), "{\"new\":true}");
    write(&default.join("profiles/default.json"), "{\"edited\":true}");
    std::fs::remove_file(default.join("state/session.json")).unwrap();
    recover(&default, &OTHER_DRIVE).unwrap().unwrap();
    assert_eq!(read(&to.join("config.json")), "{\"new\":true}");
    assert_eq!(read(&default.join("profiles/default.json")), "{\"edited\":true}");
    assert!(!to.join("state/session.json").exists());
}

#[test]
fn a_cancelled_copy_removes_the_partial_copy_and_the_journal() {
    let dir = temp("cancel");
    let (default, to) = (dir.join("Swofty"), dir.join("D/Data"));
    seed(&default);
    let p = plan(&loc(&default), &to, &OTHER_DRIVE).unwrap();
    let cancel = AtomicBool::new(true);
    let err = move_data(&p, &OTHER_DRIVE, &Quiet, &cancel).unwrap_err();
    assert_eq!(err, Error::Cancelled(default.clone()));
    whole(&default);
    assert!(!to.exists());
    assert!(!journal_left(&default));
}

#[test]
fn a_cancelled_copy_into_an_existing_empty_folder_leaves_it_empty() {
    let dir = temp("cancel-empty");
    let (default, to) = (dir.join("Swofty"), dir.join("Data"));
    seed(&default);
    std::fs::create_dir_all(&to).unwrap();
    let p = plan(&loc(&default), &to, &OTHER_DRIVE).unwrap();
    move_data(&p, &OTHER_DRIVE, &Quiet, &AtomicBool::new(true)).unwrap_err();
    assert!(to.is_dir() && std::fs::read_dir(&to).unwrap().next().is_none());
}

#[cfg(unix)]
#[test]
fn a_failure_part_way_through_the_copy_keeps_the_old_folder_whole() {
    use std::os::unix::fs::PermissionsExt;
    if is_root() {
        return;
    }
    let dir = temp("mid-copy");
    let (default, to) = (dir.join("Swofty"), dir.join("D/Data"));
    seed(&default);
    let stuck = default.join("state/session.json");
    let p = plan(&loc(&default), &to, &OTHER_DRIVE).unwrap();
    std::fs::set_permissions(&stuck, std::fs::Permissions::from_mode(0o000)).unwrap();
    let err = move_data(&p, &OTHER_DRIVE, &Quiet, &no_cancel()).unwrap_err();
    std::fs::set_permissions(&stuck, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(matches!(err, Error::Io(_)), "{err:?}");
    assert!(err.to_string().contains("stay in"));
    whole(&default);
    assert!(!to.exists());
    assert!(!journal_left(&default));
    assert!(spot_is_folder(&default));
}

#[test]
fn a_link_that_cannot_be_made_restores_the_default_folder() {
    let dir = temp("link-fails");
    let (default, to) = (dir.join("Swofty"), dir.join("D/Data"));
    seed(&default);
    let failing = Ops {
        link: |_, _| Err(std::io::Error::other("no permission")),
        ..OTHER_DRIVE
    };
    let p = plan(&loc(&default), &to, &failing).unwrap();
    move_data(&p, &failing, &Quiet, &no_cancel()).unwrap();
    let err = recover(&default, &failing).unwrap().unwrap_err();
    assert!(err.to_string().contains("Could not point"), "{err}");
    assert!(spot_is_folder(&default));
    whole(&default);
    assert!(!to.exists());
    assert!(!dir.join("Swofty.old").exists());
    assert!(!journal_left(&default));
}

#[test]
fn a_link_that_cannot_be_made_after_a_rename_puts_the_folder_back() {
    let dir = temp("rename-link-fails");
    let (default, to) = (dir.join("Swofty"), dir.join("D/Data"));
    seed(&default);
    let failing = Ops {
        link: |_, _| Err(std::io::Error::other("no permission")),
        ..SAME_VOLUME
    };
    let p = plan(&loc(&default), &to, &failing).unwrap();
    move_data(&p, &failing, &Quiet, &no_cancel()).unwrap_err();
    assert!(spot_is_folder(&default));
    whole(&default);
    assert!(!to.exists());
    assert!(!journal_left(&default));
}

#[test]
fn recover_finishes_a_move_that_crashed_after_setting_the_default_aside() {
    let dir = temp("crash-aside");
    let (default, to) = (dir.join("Swofty"), dir.join("D/Data"));
    seed(&default);
    let p = plan(&loc(&default), &to, &OTHER_DRIVE).unwrap();
    move_data(&p, &OTHER_DRIVE, &Quiet, &no_cancel()).unwrap();
    std::fs::rename(&default, dir.join("Swofty.old")).unwrap();
    let done = recover(&default, &OTHER_DRIVE).unwrap().unwrap();
    assert_eq!(done.to, to);
    assert_eq!(link::target(&default), Some(to.clone()));
    whole(&default);
    assert!(!dir.join("Swofty.old").exists());
}

#[test]
fn recover_finishes_a_rename_that_crashed_before_the_link() {
    let dir = temp("crash-rename");
    let (default, to) = (dir.join("Swofty"), dir.join("D/Data"));
    seed(&default);
    let p = plan(&loc(&default), &to, &SAME_VOLUME).unwrap();
    let journal = Journal {
        from: p.from.clone(),
        to: to.clone(),
        default: default.clone(),
        linked: false,
        aside: dir.join("Swofty.old"),
        to_existed: false,
        kind: Kind::Move,
    };
    journal.save().unwrap();
    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
    std::fs::rename(&default, &to).unwrap();
    recover(&default, &SAME_VOLUME).unwrap().unwrap();
    assert_eq!(link::target(&default), Some(to));
    whole(&default);
}

#[test]
fn recover_rolls_back_a_copy_that_no_longer_matches() {
    let dir = temp("rollback-mismatch");
    let (default, to) = (dir.join("Swofty"), dir.join("D/Data"));
    seed(&default);
    let p = plan(&loc(&default), &to, &OTHER_DRIVE).unwrap();
    let journal = Journal {
        from: p.from.clone(),
        to: to.clone(),
        default: default.clone(),
        linked: false,
        aside: dir.join("Swofty.old"),
        to_existed: false,
        kind: Kind::Move,
    };
    journal.save().unwrap();
    std::fs::create_dir_all(&to).unwrap();
    write(&to.join("stale.bin"), "left over from an older try");
    let ok = recover(&default, &OTHER_DRIVE).unwrap();
    assert!(ok.is_ok(), "stale files are mirrored away: {ok:?}");
    whole(&to);
    assert!(!to.join("stale.bin").exists());
}

#[test]
fn recover_ignores_an_unreadable_journal() {
    let dir = temp("garbage");
    let default = dir.join("Swofty");
    seed(&default);
    write(&Journal::path(&default), "{ not json");
    assert!(matches!(recover(&default, &SAME_VOLUME), Some(Err(Error::Io(_)))));
    assert!(!journal_left(&default));
    whole(&default);
}

#[test]
fn moves_a_linked_folder_again_by_rename() {
    let dir = temp("linked-rename");
    let (default, first, second) = (dir.join("Swofty"), dir.join("D/Data"), dir.join("E/Data"));
    seed(&first);
    link::create(&first, &default).unwrap();
    let l = loc(&default);
    assert_eq!(l.current(), first);
    let p = plan(&l, &second, &SAME_VOLUME).unwrap();
    assert!(p.linked);
    move_data(&p, &SAME_VOLUME, &Quiet, &no_cancel()).unwrap();
    assert_eq!(link::target(&default), Some(second.clone()));
    whole(&default);
    assert!(!first.exists());
}

#[test]
fn moves_a_linked_folder_again_by_copy_and_deletes_the_first_place() {
    let dir = temp("linked-copy");
    let (default, first, second) = (dir.join("Swofty"), dir.join("D/Data"), dir.join("E/Data"));
    seed(&first);
    link::create(&first, &default).unwrap();
    let p = plan(&loc(&default), &second, &OTHER_DRIVE).unwrap();
    move_data(&p, &OTHER_DRIVE, &Quiet, &no_cancel()).unwrap();
    assert_eq!(link::target(&default), Some(first.clone()));
    recover(&default, &OTHER_DRIVE).unwrap().unwrap();
    assert_eq!(link::target(&default), Some(second.clone()));
    whole(&default);
    assert!(!first.exists());
}

#[test]
fn a_failed_switch_of_a_linked_folder_relinks_the_first_place() {
    let dir = temp("linked-fail");
    let (default, first, second) = (dir.join("Swofty"), dir.join("D/Data"), dir.join("E/Data"));
    seed(&first);
    link::create(&first, &default).unwrap();
    let p = plan(&loc(&default), &second, &OTHER_DRIVE).unwrap();
    move_data(&p, &OTHER_DRIVE, &Quiet, &no_cancel()).unwrap();
    let failing = Ops {
        link: |to, at| {
            if to.ends_with("E/Data") {
                Err(std::io::Error::other("no permission"))
            } else {
                link::create(to, at)
            }
        },
        ..OTHER_DRIVE
    };
    recover(&default, &failing).unwrap().unwrap_err();
    assert_eq!(link::target(&default), Some(first));
    whole(&default);
    assert!(!second.exists());
    assert!(!journal_left(&default));
}

#[test]
fn verify_catches_a_short_or_altered_copy() {
    let dir = temp("verify");
    let (a, b) = (dir.join("a"), dir.join("b"));
    seed(&a);
    let never = no_cancel();
    copy::sync(&a, &b, &never, &mut |_, _| {}).ok();
    assert!(copy::verify(&a, &b).is_ok());
    write(&b.join("profiles/default.json"), "[]");
    assert!(copy::verify(&a, &b).unwrap_err().contains("differs"));
    write(&b.join("profiles/default.json"), "{}");
    std::fs::remove_file(b.join("state/session.json")).unwrap();
    assert!(copy::verify(&a, &b).is_err());
}

#[test]
fn only_the_launchers_own_logs_are_live() {
    assert!(copy::live(Path::new("state/registry.log")));
    assert!(copy::live(Path::new("state/launcher-app.log")));
    assert!(copy::live(Path::new("logs/swofty-logs-2026-11-12.zip")));
    assert!(!copy::live(Path::new("state/session.json")));
    assert!(!copy::live(Path::new("state")));
    assert!(!copy::live(Path::new("logs")));
    assert!(!copy::live(Path::new("addons/x/swofty.json")));
    assert!(!copy::live(Path::new("addons/x/state/debug.log")));
    assert!(!copy::live(Path::new("saves/career/one.bin")));
}

#[test]
fn verify_accepts_a_live_log_that_grew_and_rejects_one_that_changed() {
    let dir = temp("verify-live");
    let (a, b) = (dir.join("a"), dir.join("b"));
    seed(&a);
    write(&a.join("state/registry.log"), "one\ntwo\n");
    write(&a.join("logs/kept.txt"), "kept\n");
    let never = no_cancel();
    copy::sync(&a, &b, &never, &mut |_, _| {}).unwrap();
    assert!(copy::verify(&a, &b).is_ok());
    write(&a.join("state/registry.log"), "one\ntwo\nthree\n");
    write(&a.join("logs/kept.txt"), "kept\nmore\n");
    assert!(copy::verify(&a, &b).is_ok());
    write(&b.join("state/registry.log"), "one\nxxx\n");
    assert!(
        copy::verify(&a, &b)
            .unwrap_err()
            .contains("changed during the copy")
    );
    write(&b.join("state/registry.log"), "one\ntwo\n");
    write(&a.join("state/registry.log"), "one\n");
    assert!(
        copy::verify(&a, &b)
            .unwrap_err()
            .contains("changed during the copy")
    );
    write(&a.join("state/registry.log"), "one\ntwo\n");
    write(&a.join("state/session.json"), "{\"a\":2}");
    assert!(copy::verify(&a, &b).unwrap_err().contains("session.json differs"));
}

struct AppendOnce {
    path: PathBuf,
    done: AtomicBool,
}

impl Progress for AppendOnce {
    fn report(&self, _: f32, text: &str) {
        if text.replace('\\', "/").ends_with("state/session.json") && !self.done.swap(true, Ordering::Relaxed)
        {
            let mut file = std::fs::OpenOptions::new().append(true).open(&self.path).unwrap();
            std::io::Write::write_all(&mut file, b"after the copy\n").unwrap();
        }
    }
}

#[test]
fn a_log_the_launcher_appends_to_during_the_copy_does_not_fail_the_move() {
    let dir = temp("live-log");
    let (default, to) = (dir.join("Swofty"), dir.join("D/Data"));
    seed(&default);
    write(&default.join("state/registry.log"), "before the copy\n");
    let p = plan(&loc(&default), &to, &OTHER_DRIVE).unwrap();
    let appender = AppendOnce {
        path: default.join("state/registry.log"),
        done: AtomicBool::new(false),
    };
    let moved = move_data(&p, &OTHER_DRIVE, &appender, &no_cancel()).unwrap();
    assert_eq!(moved.method, Method::Copied);
    assert!(appender.done.load(Ordering::Relaxed));
    assert_eq!(read(&to.join("state/registry.log")), "before the copy\n");
    recover(&default, &OTHER_DRIVE).unwrap().unwrap();
    assert_eq!(
        read(&to.join("state/registry.log")),
        "before the copy\nafter the copy\n"
    );
    whole(&to);
}

struct EditOnce {
    path: PathBuf,
    done: AtomicBool,
}

impl Progress for EditOnce {
    fn report(&self, _: f32, text: &str) {
        if text.ends_with("default.json") && !self.done.swap(true, Ordering::Relaxed) {
            write(&self.path, "[]");
        }
    }
}

#[test]
fn a_profile_changed_during_the_copy_still_fails_the_move() {
    let dir = temp("live-profile");
    let (default, to) = (dir.join("Swofty"), dir.join("D/Data"));
    seed(&default);
    let p = plan(&loc(&default), &to, &OTHER_DRIVE).unwrap();
    let editor = EditOnce {
        path: default.join("profiles/default.json"),
        done: AtomicBool::new(false),
    };
    let e = move_data(&p, &OTHER_DRIVE, &editor, &no_cancel()).unwrap_err();
    assert!(editor.done.load(Ordering::Relaxed));
    assert!(
        matches!(&e, Error::Mismatch { reason, .. } if reason.contains("default.json differs")),
        "{e}"
    );
    assert!(!to.exists());
    assert!(!journal_left(&default));
    assert_eq!(read(&default.join("profiles/default.json")), "[]");
}

#[test]
fn choose_uses_the_picked_folder_when_it_is_new_empty_or_named_for_swofty() {
    let dir = temp("choose");
    let empty = dir.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert_eq!(choose(&empty), empty);
    assert_eq!(choose(&dir.join("new")), dir.join("new"));
    let named = dir.join(FOLDER.to_ascii_uppercase());
    write(&named.join("a"), "a");
    assert_eq!(choose(&named), named);
    write(&dir.join("other/file"), "a");
    assert_eq!(choose(&dir.join("other")), dir.join("other").join(FOLDER));
}

#[test]
fn a_copy_keeps_each_files_modified_time() {
    let dir = temp("mtime");
    let (a, b) = (dir.join("a"), dir.join("b"));
    seed(&a);
    let old = std::time::SystemTime::now() - Duration::from_secs(86_400 * 30);
    let file = a.join("profiles/default.json");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(old)
        .unwrap();
    copy::sync(&a, &b, &no_cancel(), &mut |_, _| {}).ok();
    let copied = std::fs::metadata(b.join("profiles/default.json"))
        .unwrap()
        .modified()
        .unwrap();
    let gap = copied
        .duration_since(old)
        .or_else(|_| old.duration_since(copied))
        .unwrap();
    assert!(gap <= Duration::from_secs(2), "{gap:?}");
}

#[cfg(windows)]
mod windows {
    use super::*;

    #[test]
    fn a_junction_can_be_made_read_and_removed_without_touching_its_target() {
        let dir = temp("junction");
        let (target, at) = (dir.join("D/Data"), dir.join("Swofty"));
        write(&target.join("state/session.json"), "{}");
        link::create(&target, &at).unwrap();
        let read = link::target(&at).expect("a junction reads back as a link");
        assert!(same_path(&read, &target), "{read:?}");
        assert_eq!(read_to(&at.join("state/session.json")), "{}");
        assert!(link::target(&target).is_none());
        link::remove(&at).unwrap();
        assert!(at.symlink_metadata().is_err());
        assert_eq!(read_to(&target.join("state/session.json")), "{}");
    }

    fn read_to(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn an_open_file_stops_the_rename_and_the_move_still_ends_whole() {
        let dir = temp("open-handle");
        let (default, to) = (dir.join("Local/Swofty"), dir.join("D/Data"));
        seed(&default);
        let held = std::fs::File::open(default.join("state/session.json")).unwrap();
        let p = plan(&loc(&default), &to, &SAME_VOLUME).unwrap();
        let moved = move_data(&p, &SAME_VOLUME, &Quiet, &no_cancel()).unwrap();
        drop(held);
        if !moved.switched {
            assert_eq!(moved.method, Method::Copied);
            whole(&default);
            recover(&default, &SAME_VOLUME).unwrap().unwrap();
        }
        assert!(same_path(&link::target(&default).unwrap(), &to));
        whole(&default);
        whole(&to);
        assert!(!dir.join("Local/Swofty.old").exists());
        assert!(!journal_left(&default));
    }

    #[test]
    fn a_junction_to_a_second_place_is_retargeted_by_a_copy_and_recover() {
        let dir = temp("junction-copy");
        let (default, first, second) = (dir.join("Swofty"), dir.join("D/Data"), dir.join("E/Data"));
        seed(&first);
        link::create(&first, &default).unwrap();
        let p = plan(&loc(&default), &second, &OTHER_DRIVE).unwrap();
        move_data(&p, &OTHER_DRIVE, &Quiet, &no_cancel()).unwrap();
        recover(&default, &OTHER_DRIVE).unwrap().unwrap();
        assert!(same_path(&link::target(&default).unwrap(), &second));
        whole(&default);
        assert!(!first.exists());
    }
}

mod adopt;
mod find;

#[test]
fn the_real_folder_is_where_the_default_path_links() {
    let dir = temp("real-folder");
    let (default, moved) = (dir.join("Swofty"), dir.join("E/Swofty"));
    seed(&moved);
    link::create(&moved, &default).unwrap();
    assert_eq!(link::real_folder(&default), moved);
    let plain = dir.join("Plain");
    seed(&plain);
    assert_eq!(link::real_folder(&plain), plain);
}
