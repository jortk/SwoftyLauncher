//! Where the core writes a crash report, and how the launcher finds it again.
//!
//! The report is `crash-<time>.txt` with a `.dmp` beside it. It goes in the game's `swofty` folder; when that
//! folder refuses the write (a game under `C:\Program Files` run without rights to change it) it goes to
//! `state` in the launcher's data folder, where the loader's fallback log already goes.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const FALLBACK_DIR: &str = "state";

const LINE: &str = "[crash] crash report ";

pub fn is_crash_name(name: &str) -> bool {
    name.starts_with("crash-") && (name.ends_with(".txt") || name.ends_with(".dmp"))
}

/// A load-stall report (`stall-<time>.txt`): every thread's stack, written beside the crash reports when a load
/// stops moving. The game did not crash, so it is kept and sent but never read as a crash.
pub fn is_stall_name(name: &str) -> bool {
    name.starts_with("stall-") && name.ends_with(".txt")
}

/// The path the loader's last `crash report <path>.txt:` line names, as written: `swofty\crash-….txt` for the
/// game folder, a full path for the data folder.
pub fn logged_path(loader: &str) -> Option<String> {
    loader.lines().rev().find_map(|line| {
        let rest = &line[line.find(LINE)? + LINE.len()..];
        let end = rest.find(".txt")? + ".txt".len();
        let path = &rest[..end];
        let name = path.rsplit(['\\', '/']).next()?;
        is_crash_name(name).then(|| path.to_owned())
    })
}

pub fn logged_name(loader: &str) -> Option<String> {
    let path = logged_path(loader)?;
    path.rsplit(['\\', '/']).next().map(str::to_owned)
}

/// Move the crash and load-stall reports the core wrote to the data folder since `launched` into `dest`. Returns
/// how many files moved.
pub fn keep_fallback(data: &Path, dest: &Path, launched: SystemTime) -> usize {
    let dir = data.join(FALLBACK_DIR);
    let mut moved = 0;
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let path = e.path();
        let name = e.file_name();
        let name = name.to_string_lossy();
        if !(is_crash_name(&name) || is_stall_name(&name)) || !path.is_file() {
            continue;
        }
        let fresh = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .is_ok_and(|t| t >= launched);
        if !fresh {
            continue;
        }
        if std::fs::create_dir_all(dest).is_ok() && crate::move_file(&path, &dest.join(e.file_name())).is_ok()
        {
            moved += 1;
        }
    }
    moved
}

pub fn fallback_dir(data: &Path) -> PathBuf {
    data.join(FALLBACK_DIR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn reads_a_game_folder_path_and_a_data_folder_path() {
        let game = "2026-11-01 12:53:50.100 [5] [crash] crash report swofty\\crash-20261101-125350.txt: boom; stack x; minidump y\n";
        assert_eq!(
            logged_path(game).as_deref(),
            Some("swofty\\crash-20261101-125350.txt")
        );
        assert_eq!(logged_name(game).as_deref(), Some("crash-20261101-125350.txt"));
        let data = "2026-11-01 12:53:50.100 [5] [crash] crash report C:\\Users\\id\\AppData\\Local\\Swofty\\state\\crash-20261101-125350.txt: boom\n";
        assert_eq!(
            logged_path(data).as_deref(),
            Some("C:\\Users\\id\\AppData\\Local\\Swofty\\state\\crash-20261101-125350.txt")
        );
        assert_eq!(logged_name(data).as_deref(), Some("crash-20261101-125350.txt"));
        assert_eq!(logged_path("[crash] writing a crash report: fatal\n"), None);
    }

    #[test]
    fn takes_the_last_report_line() {
        let log = "[crash] crash report swofty\\crash-20261101-000001.txt: a\n[crash] crash report swofty\\crash-20261101-000002-2.txt: b\n";
        assert_eq!(logged_name(log).as_deref(), Some("crash-20261101-000002-2.txt"));
    }

    #[test]
    fn tells_stall_reports_from_crash_reports() {
        assert!(is_stall_name("stall-20261112-123300.txt"));
        assert!(is_stall_name("stall-20261112-123300-2.txt"));
        assert!(!is_stall_name("stall-20261112-123300.dmp"));
        assert!(!is_crash_name("stall-20261112-123300.txt"));
        assert!(!is_stall_name("crash-20261112-123300.txt"));
    }

    #[test]
    fn moves_only_this_sessions_fallback_reports() {
        let root = std::env::temp_dir().join(format!("swofty-crash-report-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let state = root.join("state");
        std::fs::create_dir_all(&state).unwrap();
        let launched = SystemTime::now() - Duration::from_secs(5);
        let old = state.join("crash-20260101-000000.txt");
        std::fs::write(&old, "old").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_modified(launched - Duration::from_secs(600))
            .unwrap();
        std::fs::write(state.join("crash-20261101-125350.txt"), "new").unwrap();
        std::fs::write(state.join("crash-20261101-125350.dmp"), "MDMP").unwrap();
        std::fs::write(state.join("loader-fallback.log"), "log").unwrap();
        std::fs::write(state.join("stall-20261101-125300.txt"), "stall").unwrap();
        let dest = root.join("sessions/one/game-logs");
        assert_eq!(keep_fallback(&root, &dest, launched), 3);
        assert!(dest.join("stall-20261101-125300.txt").is_file());
        assert!(dest.join("crash-20261101-125350.txt").is_file());
        assert!(dest.join("crash-20261101-125350.dmp").is_file());
        assert!(old.is_file() && state.join("loader-fallback.log").is_file());
        assert!(!state.join("crash-20261101-125350.txt").exists());
        std::fs::remove_dir_all(&root).ok();
    }
}
