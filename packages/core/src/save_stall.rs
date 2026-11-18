//! Profile saves that keep F1 25 on Loading... after Press any button.
//!
//! The game reads the profile save, then loads the main menu for it. A save last written with content that
//! is not there now (an addon's career, livery or item that the profile or Vanilla leaves out) can keep that
//! load from ever finishing: the spinner turns and team radio plays, with or without the launcher. The save is
//! encrypted, so the launcher learns such a save from the sessions that read it and then sat on the start
//! screen, and keys it by the SHA-256 of its data file, `savegame@profile#NXDSMWW.BWW`.
//!
//! ```text
//! $DATA/state/stalled-saves.json   the saves learned so far and where the launcher moved them from
//! ```
//!
//! Before Play and at launcher start, [`sweep`] moves a learned save out of every save folder the launcher
//! knows (each profile's launcher saves, and the Steam or EA app save of the copy Play uses) into the career
//! vault, pinned [`PIN`], and puts back the newest version the vault holds that is not a learned one. The
//! launcher's copies between saves ([`refuse`]) never copy a learned save.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::baseline::sha256_file;
use crate::session::SessionRecord;
use crate::store::Store;
use crate::vault::restore::{Running, blocker, restore_held};
use crate::vault::{EventKind, Layout, Location, Meta, Reason, Snapshot, Taken, Vault};
use crate::{Error, Result};

pub const FILE: &str = "state/stalled-saves.json";
/// The pin on the snapshot that holds a save moved aside.
pub const PIN: &str = "stalled-at-load";
/// Stalled sessions on the same save before it counts as one that stalls.
pub const SESSIONS: usize = 2;
/// How long a session must sit on the start screen after a profile load to count as stalled. Of the sessions
/// that reached the main menu, 99 in 100 left the start screen within 82 s.
pub const STALL_SECS: f64 = 120.0;
/// One stalled session at least this long is enough on its own; the slowest load that still reached the main
/// menu took 656 s.
pub const LONG_SECS: f64 = 900.0;
/// The screens F1 25 shows before the main menu: a session whose last screen is another one got past the load.
const BOOT_SCREENS: &[&str] = &[
    "persistent",
    "audio_themes",
    "tutorials_masked",
    "video_1",
    "legal",
    "legal_2",
    "seizure_warning",
    "splash",
    "render_badge",
    "splash_transition",
];
/// How many of the newest sessions a sweep reads.
const RECENT: usize = 60;
/// What the core logs for every profile load it sees.
const PROFILE_LOAD: &str = "[saves] profile load ";
const DATA_FILE: &str = "NXDSMWW.BWW";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    #[serde(default)]
    pub saves: Vec<Stalled>,
    #[serde(default)]
    pub moved: Vec<Moved>,
}

/// A save that kept the game on the start screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stalled {
    /// SHA-256 of the data file.
    pub sha256: String,
    pub size: u64,
    /// The sessions that stalled on it.
    pub sessions: Vec<String>,
    /// The longest any of them sat on the start screen, in seconds.
    pub secs: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    pub found_at: i64,
}

/// A save the sweep moved out of a save folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Moved {
    pub source: String,
    pub sha256: String,
    /// The snapshot that holds the moved save.
    #[serde(default)]
    pub kept: Option<String>,
    /// The snapshot put back in its place.
    #[serde(default)]
    pub restored: Option<String>,
    pub at: i64,
}

impl Ledger {
    pub fn path(data: &Path) -> PathBuf {
        data.join(FILE)
    }

    pub fn read(data: &Path) -> Self {
        crate::read_json(&Self::path(data)).unwrap_or_default()
    }

    pub fn write(&self, data: &Path) -> Result<()> {
        let path = Self::path(data);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(crate::io_err(dir))?;
        }
        crate::write_json(&path, self)
    }

    pub fn stalled(&self, sha256: &str) -> Option<&Stalled> {
        self.saves.iter().find(|s| s.sha256 == sha256)
    }

    fn sizes(&self) -> impl Iterator<Item = u64> + '_ {
        self.saves.iter().map(|s| s.size)
    }

    fn moved_from(&self, source: &str, sha256: &str) -> bool {
        self.moved
            .iter()
            .any(|m| m.source == source && m.sha256 == sha256)
    }
}

/// One launcher session as the sweep reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    pub session: String,
    /// The data file's SHA-256 before the session.
    pub before: String,
    pub size: u64,
    /// The data file's SHA-256 after it, when the vault took one.
    pub after: Option<String>,
    pub profile: Option<String>,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Outcome {
    /// The game read the profile and sat on the start screen this many seconds until the session ended.
    Stalled(f64),
    /// The game read the profile and went on past the start screen.
    Passed,
    /// Neither can be told.
    Unknown,
}

/// The data file of a snapshot: `profile/NXDSMWW.BWW`, else the largest file.
pub fn data_entry(s: &Snapshot) -> Option<&crate::vault::FileEntry> {
    s.files
        .iter()
        .find(|f| f.name.ends_with(DATA_FILE))
        .or_else(|| s.files.iter().max_by_key(|f| f.size))
}

/// The data file of the save in `loc`.
fn data_file(loc: &Location) -> Option<PathBuf> {
    let files = loc.files().ok()?;
    files
        .iter()
        .find(|f| f.key.ends_with(DATA_FILE))
        .map(|f| f.path.clone())
        .or_else(|| {
            files
                .iter()
                .max_by_key(|f| std::fs::metadata(&f.path).map(|m| m.len()).unwrap_or(0))
                .map(|f| f.path.clone())
        })
}

/// Whether the session's loader log has a profile load.
fn loaded_profile(dir: &Path) -> bool {
    ["loader.log", "game-logs/loader.log"]
        .iter()
        .map(|rel| dir.join(rel))
        .filter_map(|p| std::fs::File::open(p).ok())
        .any(|f| {
            BufReader::new(f)
                .split(b'\n')
                .map_while(std::result::Result::ok)
                .any(|l| {
                    l.windows(PROFILE_LOAD.len())
                        .any(|w| w == PROFILE_LOAD.as_bytes())
                })
        })
}

/// How the session in `dir` went past the profile load. A start-screen stall that ended in a crash (the game's
/// hang watchdog closes a frozen game) or that an addon known to cause one explains is not the save's.
pub fn outcome(dir: &Path, store: &Store) -> Outcome {
    let Some(perf) = crate::known_problems::read_perf(dir) else {
        return Outcome::Unknown;
    };
    match crate::known_problems::perf_start_screen(&perf) {
        Some(secs) if secs >= STALL_SECS => {
            let record = SessionRecord::read(dir);
            match !record.crashed()
                && loaded_profile(dir)
                && !crate::known_problems::addon_stall(&record, store)
            {
                true => Outcome::Stalled(secs),
                false => Outcome::Unknown,
            }
        }
        Some(_) => Outcome::Unknown,
        None => match crate::known_problems::last_screen(&perf) {
            Some(screen) if !BOOT_SCREENS.contains(&screen) => Outcome::Passed,
            Some(_) | None => Outcome::Unknown,
        },
    }
}

/// The vault snapshots each launcher session started and ended on, by session id.
fn session_snapshots(data: &Path) -> Vec<(String, Snapshot, Option<Snapshot>)> {
    let mut out = Vec::new();
    for source in Vault::sources(data) {
        let Ok(v) = Vault::open(data, &source) else {
            continue;
        };
        let snaps = v.list().unwrap_or_default();
        let find = |id: &Option<String>| {
            id.as_ref()
                .and_then(|id| snaps.iter().find(|s| &s.id == id))
                .cloned()
        };
        let events = v.events();
        for e in events.iter().filter(|e| e.kind == EventKind::SessionStart) {
            let (Some(session), Some(before)) = (e.detail.clone(), find(&e.snapshot)) else {
                continue;
            };
            let after = events
                .iter()
                .find(|x| x.kind == EventKind::SessionEnd && x.detail.as_ref() == Some(&session))
                .and_then(|x| find(&x.snapshot));
            out.push((session, before, after));
        }
    }
    out
}

/// The newest sessions with the save each read and how it went.
pub fn seen(store: &Store) -> Vec<Seen> {
    let data = &store.root;
    let snaps = session_snapshots(data);
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(store.sessions_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    let mut out = Vec::new();
    for dir in dirs.iter().rev().take(RECENT) {
        let Some(id) = SessionRecord::read(dir).request_id else {
            continue;
        };
        let Some((_, before, after)) = snaps.iter().find(|(s, _, _)| *s == id) else {
            continue;
        };
        let Some(entry) = data_entry(before) else {
            continue;
        };
        out.push(Seen {
            session: dir.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            before: entry.sha256.clone(),
            size: entry.size,
            after: after.as_ref().and_then(data_entry).map(|f| f.sha256.clone()),
            profile: before.profile.clone(),
            outcome: outcome(dir, store),
        });
    }
    out
}

/// Fold the sessions into the ledger: a save stalls after [`SESSIONS`] stalled sessions, or one of
/// [`LONG_SECS`], and never once any session went past the start screen with it or the game wrote it.
pub fn learn(ledger: &mut Ledger, seen: &[Seen], now: i64) -> Vec<String> {
    let passed = |sha: &str| {
        seen.iter().any(|s| {
            s.before == sha
                && (s.outcome == Outcome::Passed || s.after.as_ref().is_some_and(|a| a != &s.before))
        })
    };
    ledger.saves.retain(|s| !passed(&s.sha256));
    let mut out = Vec::new();
    let mut shas: Vec<&str> = seen.iter().map(|s| s.before.as_str()).collect();
    shas.sort_unstable();
    shas.dedup();
    for sha in shas {
        if passed(sha) {
            continue;
        }
        let stalls: Vec<(&Seen, f64)> = seen
            .iter()
            .filter(|s| s.before == sha)
            .filter_map(|s| match s.outcome {
                Outcome::Stalled(secs) => Some((s, secs)),
                Outcome::Passed | Outcome::Unknown => None,
            })
            .collect();
        let longest = stalls.iter().map(|(_, secs)| *secs).fold(0.0, f64::max);
        if stalls.len() < SESSIONS && longest < LONG_SECS {
            continue;
        }
        let mut sessions: Vec<String> = stalls.iter().map(|(s, _)| s.session.clone()).collect();
        sessions.sort();
        let (first, _) = stalls[0];
        match ledger.saves.iter_mut().find(|s| s.sha256 == sha) {
            Some(known) => {
                for s in sessions {
                    if !known.sessions.contains(&s) {
                        known.sessions.push(s);
                    }
                }
                known.secs = known.secs.max(longest as u64);
            }
            None => {
                out.push(format!(
                    "saves: the profile save {} ({} bytes) kept F1 25 on Loading... after Press any button in {} session(s) ({}), up to {} s; \
                     the launcher no longer copies it and moves it out of every save folder before Play",
                    short(sha),
                    first.size,
                    sessions.len(),
                    sessions.join(", "),
                    longest as u64
                ));
                ledger.saves.push(Stalled {
                    sha256: sha.to_owned(),
                    size: first.size,
                    sessions,
                    secs: longest as u64,
                    profile: first.profile.clone(),
                    found_at: now,
                });
            }
        }
    }
    out
}

fn short(sha: &str) -> &str {
    sha.get(..12).unwrap_or(sha)
}

/// The SHA-256 of the save in `loc` when it is a learned one.
pub fn stalled_in(ledger: &Ledger, loc: &Location) -> Option<String> {
    let path = data_file(loc)?;
    let size = std::fs::metadata(&path).ok()?.len();
    if !ledger.sizes().any(|s| s == size) {
        return None;
    }
    let sha = sha256_file(&path).ok()?;
    ledger.stalled(&sha).map(|_| sha)
}

/// The error a copy of a learned save gets.
pub fn refusal(loc: &Location) -> String {
    format!(
        "the profile save in {} kept F1 25 on Loading... after Press any button, so the launcher does not copy it; \
         the career vault keeps it to restore once the addons it was saved with are back",
        loc.dir.display()
    )
}

/// Refuse to copy the save in `from` when it is a learned one.
pub fn refuse(data: &Path, from: &Location) -> Result<()> {
    let ledger = Ledger::read(data);
    if ledger.saves.is_empty() {
        return Ok(());
    }
    match stalled_in(&ledger, from) {
        Some(_) => Err(Error::Other(refusal(from))),
        None => Ok(()),
    }
}

/// The vaults whose snapshots can go back into `loc`: its own first, then the game's saves for a launcher
/// store, the save it was copied from.
fn donors(data: &Path, loc: &Location) -> Vec<Vault> {
    let mut out: Vec<Vault> = Vault::of(data, loc).into_iter().collect();
    let prefix = match loc.layout {
        Layout::Launcher => Some("steam-"),
        Layout::LauncherEa => Some("ea-"),
        Layout::Steam | Layout::Ea => None,
    };
    if let Some(prefix) = prefix {
        out.extend(
            Vault::sources(data)
                .into_iter()
                .filter(|s| s.starts_with(prefix))
                .filter_map(|s| Vault::open(data, &s).ok()),
        );
    }
    out
}

/// The newest snapshot that can go back into `loc` and is not a learned save.
fn newest_good(data: &Path, loc: &Location, ledger: &Ledger) -> Option<(Vault, Snapshot)> {
    for v in donors(data, loc) {
        let good = v.list().unwrap_or_default().into_iter().find(|s| {
            s.layout.exchanges_with(loc.layout)
                && data_entry(s).is_some_and(|f| ledger.stalled(&f.sha256).is_none())
                && s.files.iter().all(|f| v.blob(&f.sha256).is_file())
        });
        if let Some(s) = good {
            return Some((v, s));
        }
    }
    None
}

/// Move a learned save out of `loc`: the vault keeps it pinned [`PIN`], and the newest good version goes back
/// in. Without one, a launcher store or an EA save is left empty, so the game starts a new profile save; a
/// Steam save stays, since Steam Cloud would bring it back. The caller holds the session lock.
fn repair(data: &Path, loc: &Location, ledger: &mut Ledger, running: Running, now: i64) -> Option<String> {
    let sha = stalled_in(ledger, loc)?;
    if ledger.moved_from(&loc.source, &sha) {
        return None;
    }
    let what = format!(
        "saves: the profile save in {} kept F1 25 on Loading... after Press any button",
        loc.dir.display()
    );
    if let Some(why) = blocker(loc.layout, running) {
        return Some(format!("{what}; it is moved aside once nothing holds it: {why}"));
    }
    let line = match newest_good(data, loc, ledger) {
        Some((from, good)) => {
            let into = match Vault::of(data, loc) {
                Ok(v) => v,
                Err(e) => return Some(format!("{what}; the career vault could not be opened: {e}")),
            };
            match restore_held(&from, &good.id, loc, &into, now) {
                Ok(r) => {
                    let kept = r.before.clone();
                    if let Some(k) = &kept {
                        into.pin(k, PIN, now).ok();
                    }
                    ledger.moved.push(Moved {
                        source: loc.source.clone(),
                        sha256: sha,
                        kept: kept.clone(),
                        restored: Some(good.id.clone()),
                        at: now,
                    });
                    format!(
                        "{what}; the career vault keeps it as snapshot {} and the version from {} ({}) is back in its place",
                        kept.as_deref().unwrap_or("?"),
                        crate::vault::human(good.at),
                        good.id
                    )
                }
                Err(e) => format!("{what}; putting back {} failed: {e}", good.id),
            }
        }
        None => move_out(data, loc, ledger, sha, &what, now),
    };
    Some(line)
}

fn move_out(data: &Path, loc: &Location, ledger: &mut Ledger, sha: String, what: &str, now: i64) -> String {
    if loc.layout == Layout::Steam {
        ledger.moved.push(Moved {
            source: loc.source.clone(),
            sha256: sha,
            kept: None,
            restored: None,
            at: now,
        });
        return format!(
            "{what}; the career vault has no other version of it, and Steam Cloud would bring it back, so it stays"
        );
    }
    let kept = Vault::of(data, loc).and_then(|v| {
        v.snapshot(
            loc,
            Meta::new(Reason::BeforeRestore)
                .pin(PIN)
                .note("kept F1 25 on Loading... after Press any button; moved out of the save folder"),
            now,
        )
    });
    let kept = match kept {
        Ok(Taken::New(s) | Taken::Unchanged(s)) => s.id,
        Ok(Taken::NoSave) => return format!("{what}; it is gone already"),
        Err(e) => return format!("{what}; the career vault could not keep it, so it stays: {e}"),
    };
    let files = loc.files().unwrap_or_default();
    if let Some(f) = files.iter().find(|f| std::fs::remove_file(&f.path).is_err()) {
        return format!(
            "{what}; the career vault keeps it as snapshot {kept}, but {} could not be removed",
            f.path.display()
        );
    }
    ledger.moved.push(Moved {
        source: loc.source.clone(),
        sha256: sha,
        kept: Some(kept.clone()),
        restored: None,
        at: now,
    });
    format!(
        "{what}; the career vault keeps it as snapshot {kept} and holds no other version, so F1 25 starts a new profile save; \
         `swofty saves restore {kept} --source {}` puts it back",
        loc.source
    )
}

/// The save folders a sweep looks at: every profile's launcher saves and `game`, the save of the copy
/// Play uses.
fn locations(store: &Store, game: Option<&Location>) -> Vec<Location> {
    let data = &store.root;
    let mut out: Vec<Location> = Vec::new();
    for p in store.profile_names().unwrap_or_default() {
        out.push(crate::launcher_saves::location(data, &p));
        out.push(crate::launcher_saves::location_ea(data, &p));
    }
    out.extend(game.cloned());
    out.retain(crate::launcher_saves::has_files);
    out.dedup_by(|a, b| a.source == b.source);
    out
}

/// Learn from the newest sessions, then move every learned save out of the save folders. Returns the lines
/// to log. The caller holds the session lock.
pub fn sweep(store: &Store, game: Option<&Location>, running: Running, now: i64) -> Vec<String> {
    let data = &store.root;
    let mut ledger = Ledger::read(data);
    let before = ledger.clone();
    let mut out = learn(&mut ledger, &seen(store), now);
    if !ledger.saves.is_empty() {
        for loc in locations(store, game) {
            out.extend(repair(data, &loc, &mut ledger, running, now));
        }
    }
    if ledger != before
        && let Err(e) = ledger.write(data)
    {
        out.push(format!(
            "saves: could not record the saves that stall at load: {e}"
        ));
    }
    out
}

#[cfg(test)]
mod tests;
