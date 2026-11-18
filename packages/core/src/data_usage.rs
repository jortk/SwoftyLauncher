//! What fills the data folder: its size by part, the room left on its drive
//! and on the drive F1 25 itself needs, and pruning the game session logs,
//! which nothing else removes.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The parts of the data folder the Settings page names, by folder.
pub const PARTS: &[(&str, &str)] = &[
    ("Addons", "addons"),
    ("Game backup", "baseline"),
    ("Session logs", "sessions"),
    ("Cache", "cache"),
    ("Career vault", "career-vault"),
    ("Save snapshots", "saves"),
    ("Addon data", "addon-data"),
];

/// F1 25 refuses to start with less than this free on the drive with the
/// player's profile ("cannot run with less than 2048 free storage space").
pub const GAME_NEEDS: u64 = 2 << 30;

/// Sessions pruning keeps: the newest this many, and every one younger than
/// `KEEP_FOR`.
pub const KEEP_SESSIONS: usize = 20;
pub const KEEP_FOR: Duration = Duration::from_secs(2 * 24 * 3600);
/// A session with a file written this recently may belong to a running game.
const IN_USE: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub name: &'static str,
    pub path: PathBuf,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    pub root: PathBuf,
    pub parts: Vec<Part>,
    /// Everything in the data folder outside `parts`.
    pub other: u64,
    pub total: u64,
    /// Free bytes on the data folder's drive.
    pub free: Option<u64>,
    /// Free bytes on the drive with the player's profile, which F1 25 checks.
    pub profile_free: Option<u64>,
    pub sessions: usize,
}

/// Free space under which Settings and Play warn: what F1 25 needs plus 1 GB.
pub const WARN_BELOW: u64 = GAME_NEEDS + (1 << 30);

/// Free bytes on F1 25's own drive and on the data folder's drive, without
/// walking any files: quick enough for Play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Room {
    pub game: Option<u64>,
    pub data: Option<u64>,
}

impl Room {
    pub fn now(root: &Path) -> Self {
        let real = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        Room {
            game: game_drive().and_then(|d| crate::disk::free_space(&d)),
            data: crate::disk::free_space(&real),
        }
    }

    /// Free space on F1 25's drive when it is under `WARN_BELOW`.
    pub fn game_low(&self) -> Option<u64> {
        self.game.filter(|f| *f < WARN_BELOW)
    }

    /// Free space on the data folder's drive when it is under `WARN_BELOW`
    /// and isn't the game's drive (the same number means the same drive).
    pub fn data_low(&self) -> Option<u64> {
        self.data.filter(|f| *f < WARN_BELOW && self.game != Some(*f))
    }
}

/// The drive `Usage::profile_free` measures, as the Settings page names it.
pub const GAME_DRIVE: &str = if cfg!(windows) {
    "the drive with your Windows profile"
} else {
    "the drive with the game's Proton prefix"
};

/// Measures the data folder at `root`. Walks every file, so run it off the UI thread.
pub fn measure(root: &Path) -> Usage {
    // A moved data folder leaves a junction (Windows) or symlink (Linux) at
    // `root`; `dir_size` doesn't follow links, so walk the folder it points to.
    let real = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let parts: Vec<Part> = PARTS
        .iter()
        .map(|(name, dir)| Part {
            name,
            path: root.join(dir),
            bytes: dir_size(&real.join(dir)),
        })
        .collect();
    let total = dir_size(&real);
    let named: u64 = parts.iter().map(|p| p.bytes).sum();
    let room = Room::now(root);
    Usage {
        root: root.to_path_buf(),
        other: total.saturating_sub(named),
        total,
        parts,
        free: room.data,
        profile_free: room.game,
        sessions: sessions(&real.join("sessions")).len(),
    }
}

/// A folder on the drive F1 25 checks for 2 GB free: its `C:`. On Windows
/// that is the profile's drive; under Proton, `C:` is the prefix, which lives
/// in the game's Steam library.
fn game_drive() -> Option<PathBuf> {
    if cfg!(windows) {
        return std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    }
    crate::steam::find_game().map(|g| g.compatdata.filter(|c| c.is_dir()).unwrap_or(g.library))
}

/// The bytes of every file under `path`; links (junctions, symlinks) are not followed.
pub fn dir_size(path: &Path) -> u64 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    if !meta.is_dir() {
        return 0;
    }
    std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| dir_size(&e.path()))
        .sum()
}

/// Whether a folder name is a session's `yyyymmdd-hhmmss`. Anything else in
/// `sessions/`, such as recovery's `orphaned-logs`, is not a session.
fn is_session_name(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() == 15 && b[8] == b'-' && b.iter().enumerate().all(|(i, c)| i == 8 || c.is_ascii_digit())
}

/// The session folders, newest first.
fn sessions(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| e.file_name().to_str().is_some_and(is_session_name))
        .map(|e| e.path())
        .collect();
    out.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    out
}

fn newest_write(path: &Path) -> Option<SystemTime> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    let own = meta.modified().ok();
    if !meta.is_dir() {
        return own;
    }
    std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| newest_write(&e.path()))
        .chain(own)
        .max()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pruned {
    pub sessions: usize,
    pub bytes: u64,
}

/// The session folders pruning would remove: all but the newest `keep`, and
/// none written within `keep_for` of `now` (nor within the last hour, which a
/// running game may still be writing).
pub fn old_sessions(dir: &Path, keep: usize, keep_for: Duration, now: SystemTime) -> Vec<PathBuf> {
    sessions(dir)
        .into_iter()
        .skip(keep)
        .filter(|s| {
            let age = newest_write(s).and_then(|t| now.duration_since(t).ok());
            age.is_some_and(|a| a > keep_for.max(IN_USE))
        })
        .collect()
}

/// What pruning `dir` would free, without removing anything.
pub fn prunable(dir: &Path, keep: usize, keep_for: Duration, now: SystemTime) -> Pruned {
    let old = old_sessions(dir, keep, keep_for, now);
    Pruned {
        sessions: old.len(),
        bytes: old.iter().map(|s| dir_size(s)).sum(),
    }
}

/// Removes the old session folders (see `old_sessions`).
pub fn prune_sessions(dir: &Path, keep: usize, keep_for: Duration, now: SystemTime) -> Pruned {
    let mut out = Pruned::default();
    for s in old_sessions(dir, keep, keep_for, now) {
        let bytes = dir_size(&s);
        if std::fs::remove_dir_all(&s).is_ok() {
            out.sessions += 1;
            out.bytes += bytes;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("swofty-data-usage-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn session(dir: &Path, name: &str, bytes: usize) {
        std::fs::create_dir_all(dir.join(name)).unwrap();
        std::fs::write(dir.join(name).join("loader.log"), vec![b'x'; bytes]).unwrap();
    }

    #[test]
    fn measures_each_part_and_the_rest() {
        let root = scratch("measure");
        std::fs::create_dir_all(root.join("addons/a")).unwrap();
        std::fs::write(root.join("addons/a/swofty.json"), [0u8; 100]).unwrap();
        session(&root.join("sessions"), "20261101-100000", 50);
        std::fs::write(root.join("config.json"), [0u8; 7]).unwrap();
        let u = measure(&root);
        let bytes = |n: &str| u.parts.iter().find(|p| p.name == n).unwrap().bytes;
        assert_eq!(
            (bytes("Addons"), bytes("Session logs"), bytes("Cache")),
            (100, 50, 0)
        );
        assert_eq!((u.other, u.total, u.sessions), (7, 157, 1));
    }

    #[test]
    fn prunes_all_but_the_newest_and_the_recent_sessions() {
        let dir = scratch("prune");
        for n in [
            "20261001-100000",
            "20261002-100000",
            "20261003-100000",
            "20261004-100000",
        ] {
            session(&dir, n, 10);
        }
        let later = SystemTime::now() + Duration::from_secs(30 * 24 * 3600);
        // nothing is old yet: every session was written just now
        assert_eq!(prunable(&dir, 1, KEEP_FOR, SystemTime::now()), Pruned::default());
        // a month on, the newest two stay and the oldest two go
        assert_eq!(
            prunable(&dir, 2, KEEP_FOR, later),
            Pruned {
                sessions: 2,
                bytes: 20
            }
        );
        assert_eq!(
            prune_sessions(&dir, 2, KEEP_FOR, later),
            Pruned {
                sessions: 2,
                bytes: 20
            }
        );
        let left: Vec<String> = sessions(&dir)
            .iter()
            .map(|s| s.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, ["20261004-100000", "20261003-100000"]);
    }

    #[test]
    fn only_timestamped_folders_are_sessions_and_others_are_never_pruned() {
        let dir = scratch("names");
        session(&dir, "orphaned-logs", 10);
        session(&dir, "20261001-100000", 10);
        session(&dir, "20261002-100000", 10);
        let later = SystemTime::now() + Duration::from_secs(30 * 24 * 3600);
        assert_eq!(sessions(&dir).len(), 2);
        // `orphaned-logs` takes no keep slot, so with one kept one session goes
        assert_eq!(
            prune_sessions(&dir, 1, KEEP_FOR, later),
            Pruned {
                sessions: 1,
                bytes: 10
            }
        );
        assert!(dir.join("orphaned-logs").is_dir());
        assert!(dir.join("20261002-100000").is_dir());
    }

    /// The installer leaves a junction (Windows) or symlink (Linux) where a
    /// moved data folder was; it is measured where it really is.
    #[test]
    fn measures_a_moved_data_folder_through_its_link() {
        let root = scratch("moved");
        let real = root.join("real");
        std::fs::create_dir_all(real.join("addons/a")).unwrap();
        std::fs::write(real.join("addons/a/swofty.json"), [0u8; 100]).unwrap();
        session(&real.join("sessions"), "20261101-100000", 50);
        let link = root.join("link");
        #[cfg(windows)]
        assert!(
            std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&link)
                .arg(&real)
                .output()
                .unwrap()
                .status
                .success()
        );
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let u = measure(&link);
        assert_eq!((u.total, u.sessions, u.root), (150, 1, link));
    }

    #[test]
    fn room_warns_per_drive_and_names_a_shared_drive_once() {
        let gb = 1u64 << 30;
        let same = Room {
            game: Some(gb),
            data: Some(gb),
        };
        assert_eq!((same.game_low(), same.data_low()), (Some(gb), None));
        let data_only = Room {
            game: Some(50 * gb),
            data: Some(2 * gb),
        };
        assert_eq!((data_only.game_low(), data_only.data_low()), (None, Some(2 * gb)));
        let plenty = Room {
            game: None,
            data: Some(WARN_BELOW),
        };
        assert_eq!((plenty.game_low(), plenty.data_low()), (None, None));
    }

    #[test]
    fn a_missing_sessions_folder_prunes_nothing() {
        let dir = scratch("none").join("sessions");
        assert_eq!(
            prune_sessions(&dir, 0, Duration::ZERO, SystemTime::now()),
            Pruned::default()
        );
    }
}
