//! Whole-profile restore from a vault snapshot. Steam-safe: it refuses while
//! F1 25 or Steam runs (the EA app for an EA copy), snapshots the current
//! save first, writes each file through a temporary name and checks its
//! hash, and never touches `remotecache.vdf`.

use std::path::{Path, PathBuf};

use super::{Event, EventKind, Layout, Location, Meta, Notice, Reason, Taken, Vault};
use crate::baseline::sha256_file;
use crate::recovery::SessionLock;
use crate::{Error, Result, io_err};

const SESSION_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

pub const CLOSE_STEAM: &str = "Exit Steam completely (Steam menu > Exit), then restore again. Starting Steam \
                               afterwards uploads the restored save.";
pub const CLOSE_GAME: &str = "F1 25 is running; close it, then try again.";
pub const CLOSE_EA: &str = "Exit the EA app completely (its tray icon > Quit), then restore again.";

/// What runs that a restore must wait for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Running {
    pub game: bool,
    pub steam: bool,
    pub ea_app: bool,
}

impl Running {
    pub const TEST_ENV: &'static str = "SWOFTY_TEST_RUNNING";

    /// `SWOFTY_TEST_RUNNING` (a list of `game`, `steam`, `ea`; empty for none) replaces
    /// the process check, for tests that must not depend on the machine they run on.
    pub fn now() -> Self {
        if let Ok(v) = std::env::var(Self::TEST_ENV) {
            let has = |n: &str| v.split(',').any(|p| p.trim() == n);
            return Self {
                game: has("game"),
                steam: has("steam"),
                ea_app: has("ea"),
            };
        }
        Self {
            game: crate::process::game_running(),
            steam: crate::steam_client::status().state != crate::steam_client::State::Stopped,
            ea_app: crate::ea_app::running(),
        }
    }
}

/// Why a restore into a save of `layout` must wait.
pub fn blocker(layout: Layout, r: Running) -> Option<String> {
    if r.game {
        return Some(match layout {
            Layout::Steam => format!("F1 25 is running; close it first. {CLOSE_STEAM}"),
            Layout::Ea => format!("F1 25 is running; close it first. {CLOSE_EA}"),
            Layout::Launcher | Layout::LauncherEa => CLOSE_GAME.to_owned(),
        });
    }
    match layout {
        Layout::Steam if r.steam => Some(format!(
            "Steam is running and would call the restored save corrupted. {CLOSE_STEAM}"
        )),
        Layout::Ea if r.ea_app => Some(format!("The EA app is running. {CLOSE_EA}")),
        Layout::Steam | Layout::Ea | Layout::Launcher | Layout::LauncherEa => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    pub snapshot: String,
    /// The snapshot holding the save the restore replaced.
    pub before: Option<String>,
    pub dir: PathBuf,
    /// Profile files in the save folder that the snapshot doesn't have; they
    /// stay as they are.
    pub kept: Vec<String>,
}

impl Restored {
    pub fn log_line(&self) -> String {
        format!("[vault] restored {} into {}", self.snapshot, self.dir.display())
    }
}

/// The launcher profile whose save `loc` is; the Steam save is Vanilla's.
fn profile_of(loc: &Location) -> String {
    match (loc.layout, loc.source.strip_prefix("launcher-")) {
        (Layout::Launcher, Some(p)) => p.to_owned(),
        _ => crate::profile::VANILLA.to_owned(),
    }
}

/// Set a restored file's modified time to `now`, so it doesn't carry the epoch.
fn touch(path: &Path, now: i64) -> Result<()> {
    let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(now.max(0) as u64);
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .and_then(|f| f.set_modified(at))
        .map_err(io_err(path))
}

pub fn restore(vault: &Vault, id: &str, loc: &Location, running: Running, now: i64) -> Result<Restored> {
    restore_across(vault, id, loc, vault, running, now)
}

/// Restore snapshot `id` of `from` into the save at `loc`, which `into` keeps the vault of. Between a
/// Steam save and a launcher store the files carry the same names, so a snapshot of either restores
/// into the other.
pub fn restore_across(
    from: &Vault,
    id: &str,
    loc: &Location,
    into: &Vault,
    running: Running,
    now: i64,
) -> Result<Restored> {
    if let Some(why) = blocker(loc.layout, running) {
        from.log(
            &Event::new(now, EventKind::RestoreRefused)
                .snapshot(id)
                .detail(why.clone()),
        );
        return Err(Error::Other(why));
    }
    let _session = SessionLock::wait(&from.data, SESSION_GRACE)?.ok_or_else(|| {
        Error::Other(
            "a Play session or another restore is running; wait for it to end, then restore again".to_owned(),
        )
    })?;
    restore_held(from, id, loc, into, now)
}

/// [`restore_across`] for a caller that holds the session lock and checked [`blocker`] itself.
pub(crate) fn restore_held(
    from: &Vault,
    id: &str,
    loc: &Location,
    into: &Vault,
    now: i64,
) -> Result<Restored> {
    let vault = from;
    let same = from.source == into.source;
    let (first, second) = match from.source <= into.source {
        true => (from, into),
        false => (into, from),
    };
    let _lock = first.lock()?;
    let _lock2 = match same {
        true => None,
        false => Some(second.lock()?),
    };
    let snap = vault.find(id)?;
    if (same && (snap.layout != loc.layout || snap.source != loc.source))
        || !snap.layout.exchanges_with(loc.layout)
    {
        return Err(Error::Other(format!(
            "snapshot {id} is a save of {}, not of {}",
            snap.source, loc.source
        )));
    }
    let mut plan = Vec::new();
    for f in &snap.files {
        let blob = vault.blob(&f.sha256);
        if !blob.is_file() || sha256_file(&blob)? != f.sha256 {
            return Err(Error::Other(format!(
                "snapshot {id} is damaged: {} does not match its manifest",
                f.name
            )));
        }
        let to = loc
            .path_of(&f.name)
            .ok_or_else(|| Error::Other(format!("snapshot {id} names an odd file, {}", f.name)))?;
        plan.push((blob, to, f));
    }
    let before = into.snapshot_locked(
        loc,
        Meta::new(Reason::BeforeRestore)
            .pin("before-restore")
            .note(format!("before restoring {id}")),
        now,
    )?;
    let before = match before {
        Taken::New(s) | Taken::Unchanged(s) => Some(s.id),
        Taken::NoSave => None,
    };
    std::fs::create_dir_all(&loc.dir).map_err(io_err(&loc.dir))?;
    let mut staged: Vec<(PathBuf, &PathBuf, &str)> = Vec::new();
    for (blob, to, f) in &plan {
        let name = to.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let tmp = loc.dir.join(format!(".{name}.swofty-restore"));
        staged.push((tmp.clone(), to, &f.sha256));
        let ok = std::fs::copy(blob, &tmp)
            .map_err(io_err(&tmp))
            .and_then(|_| sha256_file(&tmp))
            .and_then(|h| match h == f.sha256 {
                true => Ok(()),
                false => Err(Error::Other(format!("{} did not copy cleanly", tmp.display()))),
            })
            .and_then(|()| touch(&tmp, now))
            .and_then(|()| crate::sync_file(&tmp));
        if let Err(e) = ok {
            for (tmp, _, _) in &staged {
                std::fs::remove_file(tmp).ok();
            }
            return Err(Error::Other(format!("{e}; the save was left as it was")));
        }
    }
    let undo = match &before {
        Some(b) => format!("`swofty saves restore {b}` puts back the save as it was (snapshot {b})"),
        None => "there was no save before the restore".to_owned(),
    };
    for (n, (tmp, to, sha)) in staged.iter().enumerate() {
        let done =
            std::fs::rename(tmp, to)
                .map_err(io_err(to))
                .and_then(|()| match sha256_file(to)? == **sha {
                    true => Ok(()),
                    false => Err(Error::Other(format!("{} did not restore cleanly", to.display()))),
                });
        if done.is_ok() {
            crate::sync_dir(to.parent());
        }
        if let Err(e) = done {
            for (tmp, _, _) in &staged[n..] {
                std::fs::remove_file(tmp).ok();
            }
            let state = match n {
                0 => "No file was replaced yet",
                _ => "The save now mixes old and restored files",
            };
            return Err(Error::Other(format!(
                "the restore stopped at {}: {e}. {state}; {undo}",
                to.display()
            )));
        }
    }
    let kept = loc
        .files()?
        .into_iter()
        .filter(|f| !snap.files.iter().any(|s| s.name == f.key))
        .map(|f| f.key)
        .collect();
    if let Some(index) = from.index_of(&snap) {
        crate::stdlib::careers::replace_profile(&into.data, &profile_of(loc), &index).ok();
    }
    let mut e = Event::new(now, EventKind::Restored).snapshot(id);
    e.from = before.clone();
    e.detail = Some(loc.dir.display().to_string());
    into.log(&e);
    into.snapshot_locked(
        loc,
        Meta::new(Reason::Restored).note(format!("restored from {id}")),
        now,
    )?;
    if Notice::read(&into.data).is_some_and(|n| n.source == into.source) {
        Notice::clear(&into.data);
    }
    into.prune_locked(now)?;
    if !same {
        from.prune_locked(now)?;
    }
    Ok(Restored {
        snapshot: id.to_owned(),
        before,
        dir: loc.dir.clone(),
        kept,
    })
}
