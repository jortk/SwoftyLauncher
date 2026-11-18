//! Moving the data folder to another place or drive.
//!
//! The new place is recorded the way the installer records it, so the two
//! agree: a link at the default data path (a junction on Windows, a symlink
//! elsewhere) that points at the folder. Nothing is read from anywhere new at
//! startup, and every path the launcher and the CLI use stays the same.
//!
//! A move runs in two parts. [`move_data`] checks the target, writes a journal
//! next to the default folder, then either renames the folder (same volume) or
//! copies it and checks the copy; the old folder is not touched by a copy.
//! [`recover`] does the switch: it brings the copy up to date, links the
//! default path to it and deletes the old folder. It runs first thing at
//! startup, when nothing has the old folder open, and it also finishes or
//! undoes a move that a crash cut short.

mod adopt;
mod copy;
pub mod find;
pub mod history;
pub mod link;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::launcher::Progress;
use crate::process::same_path;
use crate::{disk, paths};

pub use adopt::{Adoption, adopt};
pub use copy::{Listing, listing, totals};

pub const ENV: &str = paths::ENV;
const FOLDER: &str = if cfg!(windows) { "Swofty" } else { "swofty" };
const JOURNAL: &str = "swofty-data-move.json";
const MIN_MARGIN: u64 = 1 << 30;
const STARTUP_TRIES: u32 = 40;
const RETRY_AFTER: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("{ENV} sets where your addons and saves are, to {}. Change that variable to move them.", .0.display())]
    EnvSet(PathBuf),
    #[error("Choose a full path.")]
    NotAbsolute,
    #[error("Your addons and saves are already in {}.", .0.display())]
    AlreadyThere(PathBuf),
    #[error("There are no addons and saves to move yet.")]
    NothingToMove,
    #[error("Choose a folder outside {}, where your addons and saves are now.", .0.display())]
    InsideCurrent(PathBuf),
    #[error("Choose a folder that does not contain {}, where your addons and saves are now.", .0.display())]
    ContainsCurrent(PathBuf),
    #[error("Updates cannot install when your addons and saves are inside the launcher folder. Choose a folder outside {}.", .0.display())]
    InsideInstall(PathBuf),
    #[error("Choose a folder that does not contain the launcher folder {}.", .0.display())]
    ContainsInstall(PathBuf),
    #[error("Choose a folder outside the F1 25 folder {}.", .0.display())]
    InsideGame(PathBuf),
    #[error("Choose a folder that does not contain the F1 25 folder {}.", .0.display())]
    ContainsGame(PathBuf),
    #[error("{} is not a folder. Choose an empty folder or a new one.", .0.display())]
    NotFolder(PathBuf),
    #[error("{} is not empty. Choose an empty folder or a new one.", .0.display())]
    NotEmpty(PathBuf),
    #[error("Cannot write to {}: {}", .0.display(), .1)]
    NotWritable(PathBuf, String),
    #[error("Not enough free space in {}: it needs {} GB and {} GB is free.", .path.display(), disk::gb(*.need), disk::gb(*.free))]
    NotEnoughSpace { path: PathBuf, need: u64, free: u64 },
    #[error("{} points to {}, which is missing. Reconnect that drive or folder before moving your addons and saves.", .default.display(), .target.display())]
    LinkBroken { default: PathBuf, target: PathBuf },
    #[error("Choose a folder other than {}. It is where the launcher looks for your addons and saves now.", .0.display())]
    IsDefault(PathBuf),
    #[error("The move was cancelled. Your addons and saves stay in {}.", .0.display())]
    Cancelled(PathBuf),
    #[error("The copy does not match: {reason}. Your addons and saves stay in {}.", .from.display())]
    Mismatch { from: PathBuf, reason: String },
    #[error("{} is a whole drive. Choose a folder on it.", .0.display())]
    DriveRoot(PathBuf),
    #[error("{} does not hold any addons of yours.", .0.display())]
    NoAddons(PathBuf),
    #[error("{0}")]
    Io(String),
}

impl Error {
    fn io(what: impl std::fmt::Display, e: impl std::fmt::Display) -> Self {
        Self::Io(format!("{what}: {e}"))
    }

    fn retryable(&self) -> bool {
        matches!(self, Self::Io(_) | Self::Mismatch { .. })
    }
}

/// Where the launcher keeps its data now, and what the target must stay clear of.
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    pub default: PathBuf,
    pub env: Option<PathBuf>,
    pub install: Option<PathBuf>,
    pub games: Vec<PathBuf>,
}

impl Location {
    pub fn find(install: Option<PathBuf>, games: Vec<PathBuf>) -> Self {
        Self {
            default: paths::platform_data_dir(),
            env: std::env::var_os(ENV).filter(|v| !v.is_empty()).map(PathBuf::from),
            install,
            games,
        }
    }

    pub fn current(&self) -> PathBuf {
        self.env
            .clone()
            .or_else(|| link::target(&self.default))
            .unwrap_or_else(|| self.default.clone())
    }
}

/// The filesystem and the volume checks a move uses, replaceable in tests.
pub struct Ops {
    pub rename: fn(&Path, &Path) -> std::io::Result<()>,
    pub free: fn(&Path) -> Option<u64>,
    pub same_volume: fn(&Path, &Path) -> bool,
    pub link: fn(&Path, &Path) -> std::io::Result<()>,
}

impl Ops {
    pub fn system() -> Self {
        Self {
            rename: |a, b| std::fs::rename(a, b),
            free: disk::free_space,
            same_volume: |a, b| matches!((disk::volume(a), disk::volume(b)), (Some(a), Some(b)) if a == b),
            link: link::create,
        }
    }
}

/// A checked move, ready to run.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub from: PathBuf,
    pub to: PathBuf,
    pub default: PathBuf,
    pub linked: bool,
    pub bytes: u64,
    pub files: usize,
    pub free: Option<u64>,
    pub same_volume: bool,
    to_existed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Renamed,
    Copied,
}

/// What [`move_data`] did. A copy is not switched until [`recover`] runs, at the next start.
#[derive(Debug, Clone, PartialEq)]
pub struct Moved {
    pub method: Method,
    pub switched: bool,
    pub leftover: Option<PathBuf>,
}

/// A move that [`recover`] finished.
#[derive(Debug, Clone, PartialEq)]
pub struct Recovered {
    pub to: PathBuf,
    pub leftover: Option<PathBuf>,
    pub adopted: bool,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
enum Kind {
    #[default]
    Move,
    Adopt,
    Relocate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Journal {
    from: PathBuf,
    to: PathBuf,
    default: PathBuf,
    linked: bool,
    aside: PathBuf,
    to_existed: bool,
    #[serde(default)]
    kind: Kind,
}

impl Journal {
    fn path(default: &Path) -> PathBuf {
        default.with_file_name(JOURNAL)
    }

    fn save(&self) -> Result<(), Error> {
        crate::write_json(&Self::path(&self.default), self)
            .map_err(|e| Error::io("Could not record the move", e))
    }

    fn clear(&self) {
        std::fs::remove_file(Self::path(&self.default)).ok();
    }
}

/// Whether a move that was started has not finished at `default`.
pub fn journal_pending(default: &Path) -> bool {
    Journal::path(default).exists()
}

fn resolved(path: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut at = path;
    loop {
        if let Ok(real) = std::fs::canonicalize(at) {
            return rest.iter().rev().fold(real, |p, part| p.join(part));
        }
        match (at.parent(), at.file_name()) {
            (Some(parent), Some(name)) => {
                rest.push(name.to_owned());
                at = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

fn within(inner: &Path, outer: &Path) -> bool {
    let outer = resolved(outer);
    resolved(inner).ancestors().any(|a| same_path(a, &outer))
}

fn vacant(to: &Path) -> Result<bool, Error> {
    let Ok(meta) = to.symlink_metadata() else {
        return Ok(false);
    };
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(Error::NotFolder(to.to_path_buf()));
    }
    match std::fs::read_dir(to).map(|mut d| d.next().is_none()) {
        Ok(true) => Ok(true),
        Ok(false) => Err(Error::NotEmpty(to.to_path_buf())),
        Err(e) => Err(Error::NotWritable(to.to_path_buf(), e.to_string())),
    }
}

fn writable(to: &Path) -> Result<(), Error> {
    let made = to
        .ancestors()
        .take_while(|a| !a.exists())
        .last()
        .map(Path::to_path_buf);
    let fail = |e: std::io::Error| Error::NotWritable(to.to_path_buf(), e.to_string());
    std::fs::create_dir_all(to).map_err(fail)?;
    let probe = to.join(format!(".swofty-write-test-{}", std::process::id()));
    let wrote = std::fs::write(&probe, b"").map_err(fail);
    std::fs::remove_file(&probe).ok();
    if let Some(made) = made {
        std::fs::remove_dir_all(made).ok();
    }
    wrote
}

/// The data folder for a folder the player picked: the folder itself when it is
/// new, empty or already called Swofty, else a Swofty folder inside it.
pub fn choose(picked: &Path) -> PathBuf {
    let empty = std::fs::read_dir(picked).is_ok_and(|mut d| d.next().is_none());
    let named = picked.file_name().is_some_and(|n| n.eq_ignore_ascii_case(FOLDER));
    if empty || named || !picked.exists() {
        picked.to_path_buf()
    } else {
        picked.join(FOLDER)
    }
}

/// Checks that the data folder can move to `to`, and measures what moving it takes.
pub fn plan(loc: &Location, to: &Path, ops: &Ops) -> Result<Plan, Error> {
    if let Some(env) = &loc.env {
        return Err(Error::EnvSet(env.clone()));
    }
    if !to.is_absolute() {
        return Err(Error::NotAbsolute);
    }
    if to.parent().is_none() {
        return Err(Error::DriveRoot(to.to_path_buf()));
    }
    let linked = link::target(&loc.default);
    if let Some(target) = &linked
        && !target.exists()
    {
        return Err(Error::LinkBroken {
            default: loc.default.clone(),
            target: target.clone(),
        });
    }
    if linked.is_some() && same_path(to, &loc.default) {
        return Err(Error::IsDefault(loc.default.clone()));
    }
    let from = linked.clone().unwrap_or_else(|| loc.default.clone());
    if same_path(to, &from) || crate::copies::same_dir(to, &from) {
        return Err(Error::AlreadyThere(from));
    }
    if from.symlink_metadata().is_err() {
        return Err(Error::NothingToMove);
    }
    if within(to, &from) {
        return Err(Error::InsideCurrent(from));
    }
    if within(to, &loc.default) {
        return Err(Error::InsideCurrent(loc.default.clone()));
    }
    if within(&from, to) {
        return Err(Error::ContainsCurrent(from));
    }
    if within(&loc.default, to) {
        return Err(Error::ContainsCurrent(loc.default.clone()));
    }
    if let Some(install) = &loc.install {
        if within(to, install) {
            return Err(Error::InsideInstall(install.clone()));
        }
        if within(install, to) {
            return Err(Error::ContainsInstall(install.clone()));
        }
    }
    for game in &loc.games {
        if within(to, game) {
            return Err(Error::InsideGame(game.clone()));
        }
        if within(game, to) {
            return Err(Error::ContainsGame(game.clone()));
        }
    }
    let to_existed = vacant(to)?;
    writable(to)?;
    if !to_existed {
        std::fs::remove_dir(to).ok();
    }
    let list = listing(&from).map_err(|e| Error::io(format!("Could not read {}", from.display()), e))?;
    let (bytes, files) = totals(&list);
    let same_volume = (ops.same_volume)(&from, to);
    let free = (ops.free)(to);
    if !same_volume && let Some(free) = free {
        let need = bytes + MIN_MARGIN.max(bytes / 20);
        if free < need {
            return Err(Error::NotEnoughSpace {
                path: to.to_path_buf(),
                need,
                free,
            });
        }
    }
    Ok(Plan {
        from,
        to: to.to_path_buf(),
        default: loc.default.clone(),
        linked: linked.is_some(),
        bytes,
        files,
        free,
        same_volume,
        to_existed,
    })
}

fn aside(default: &Path) -> PathBuf {
    let name = default
        .file_name()
        .map_or_else(|| "swofty".to_owned(), |n| n.to_string_lossy().into_owned());
    (1..)
        .map(|i| match i {
            1 => default.with_file_name(format!("{name}.old")),
            i => default.with_file_name(format!("{name}.old-{i}")),
        })
        .find(|p| p.symlink_metadata().is_err())
        .unwrap_or_else(|| default.with_file_name(format!("{name}.old")))
}

/// Moves the data folder as `plan` says. A rename finishes the move; a copy
/// leaves it for [`recover`], and until then nothing has changed for the
/// launcher. On any failure or cancel the old folder is as it was and the
/// partial copy is gone.
pub fn move_data(
    plan: &Plan,
    ops: &Ops,
    progress: &dyn Progress,
    cancel: &AtomicBool,
) -> Result<Moved, Error> {
    let journal = Journal {
        from: plan.from.clone(),
        to: plan.to.clone(),
        default: plan.default.clone(),
        linked: plan.linked,
        aside: aside(&plan.default),
        to_existed: plan.to_existed,
        kind: Kind::Move,
    };
    journal.save()?;
    history::record(&plan.default, &plan.from, "moved the data folder");
    if plan.same_volume && renamed(&journal, ops) {
        progress.report(1.0, "");
        return match finish(&journal, ops, 1) {
            Ok(done) => Ok(Moved {
                method: Method::Renamed,
                switched: true,
                leftover: done.leftover,
            }),
            Err(e) => {
                abort(&journal, ops);
                Err(e)
            }
        };
    }
    progress.report(0.0, "");
    let total = plan.bytes.max(1) as f32;
    let mut done = 0u64;
    let copied = copy::sync(&plan.from, &plan.to, cancel, &mut |rel, n| {
        done += n;
        progress.report((done as f32 / total).min(1.0), &rel.display().to_string());
    });
    let failed = match copied {
        Ok(()) => copy::verify(&plan.from, &plan.to)
            .err()
            .map(|reason| Error::Mismatch {
                from: plan.from.clone(),
                reason,
            }),
        Err(copy::Stop::Cancelled) => Some(Error::Cancelled(plan.from.clone())),
        Err(copy::Stop::Io(e)) => Some(Error::Io(format!(
            "Could not copy {} to {}: {e}. Your addons and saves stay in {}.",
            plan.from.display(),
            plan.to.display(),
            plan.from.display()
        ))),
    };
    match failed {
        None => Ok(Moved {
            method: Method::Copied,
            switched: false,
            leftover: None,
        }),
        Some(e) => {
            abort(&journal, ops);
            Err(e)
        }
    }
}

fn renamed(j: &Journal, ops: &Ops) -> bool {
    if j.to_existed && std::fs::remove_dir(&j.to).is_err() {
        return false;
    }
    if let Some(parent) = j.to.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if (ops.rename)(&j.from, &j.to).is_ok() {
        return true;
    }
    if j.to_existed {
        std::fs::create_dir(&j.to).ok();
    }
    false
}

enum Spot {
    Missing,
    Link(PathBuf),
    Folder,
}

fn spot(path: &Path) -> Spot {
    match link::target(path) {
        Some(t) => Spot::Link(t),
        None if path.symlink_metadata().is_ok() => Spot::Folder,
        None => Spot::Missing,
    }
}

fn exists(path: &Path) -> bool {
    path.symlink_metadata().is_ok()
}

/// The steps from "data is in the old place" to "the default path links to the
/// new one", read from what is on disk so they can start anywhere.
fn finish(j: &Journal, ops: &Ops, tries: u32) -> Result<Recovered, Error> {
    let mut last = None;
    for attempt in 0..tries {
        if attempt > 0 {
            std::thread::sleep(RETRY_AFTER);
        }
        match switch(j, ops) {
            Ok(done) => return Ok(done),
            Err(e) if e.retryable() => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| Error::Io("The move did not finish.".into())))
}

fn switch(j: &Journal, ops: &Ops) -> Result<Recovered, Error> {
    if j.kind != Kind::Move {
        return adopt::switch(j, ops);
    }
    let state = spot(&j.default);
    if matches!(&state, Spot::Link(t) if same_path(t, &j.to)) {
        return Ok(cleanup(j));
    }
    let source = if j.linked {
        exists(&j.from).then(|| j.from.clone())
    } else if matches!(state, Spot::Folder) {
        Some(j.default.clone())
    } else {
        exists(&j.aside).then(|| j.aside.clone())
    };
    match &source {
        Some(source) => {
            let never = AtomicBool::new(false);
            copy::sync(source, &j.to, &never, &mut |_, _| {}).map_err(|e| match e {
                copy::Stop::Cancelled => Error::Io("The move was cancelled.".into()),
                copy::Stop::Io(e) => Error::Io(format!(
                    "Could not copy {} to {}: {e}",
                    source.display(),
                    j.to.display()
                )),
            })?;
            copy::verify(source, &j.to).map_err(|reason| Error::Mismatch {
                from: source.clone(),
                reason,
            })?;
        }
        None if exists(&j.to) => {}
        None => return Err(Error::Io("The folder to move is missing.".into())),
    }
    match state {
        Spot::Folder => std::fs::rename(&j.default, &j.aside)
            .map_err(|e| Error::io(format!("Could not set {} aside", j.default.display()), e))?,
        Spot::Link(_) => link::remove(&j.default)
            .map_err(|e| Error::io(format!("Could not replace {}", j.default.display()), e))?,
        Spot::Missing => {}
    }
    if let Some(parent) = j.default.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Err(e) = (ops.link)(&j.to, &j.default) {
        undo_switch(j, ops, source.is_some());
        return Err(Error::io(
            format!("Could not point {} to {}", j.default.display(), j.to.display()),
            e,
        ));
    }
    Ok(cleanup(j))
}

fn undo_switch(j: &Journal, ops: &Ops, kept_old: bool) {
    if exists(&j.default) {
        return;
    }
    if j.linked {
        if kept_old {
            (ops.link)(&j.from, &j.default).ok();
        }
    } else if exists(&j.aside) {
        std::fs::rename(&j.aside, &j.default).ok();
    }
}

fn cleanup(j: &Journal) -> Recovered {
    let old = if j.linked { &j.from } else { &j.aside };
    let leftover = (exists(old) && !same_path(old, &j.to))
        .then(|| copy::remove(old).err().map(|_| old.clone()))
        .flatten();
    j.clear();
    Recovered {
        to: j.to.clone(),
        leftover,
        adopted: false,
    }
}

/// Puts everything back as it was before the move and removes the partial copy.
fn abort(j: &Journal, ops: &Ops) {
    if j.kind != Kind::Move {
        j.clear();
        return;
    }
    if !j.linked && !exists(&j.default) {
        if exists(&j.aside) {
            std::fs::rename(&j.aside, &j.default).ok();
        } else if exists(&j.to) && std::fs::rename(&j.to, &j.default).is_ok() && j.to_existed {
            std::fs::create_dir(&j.to).ok();
        }
    }
    if j.linked {
        if !exists(&j.from) && exists(&j.to) && std::fs::rename(&j.to, &j.from).is_ok() && j.to_existed {
            std::fs::create_dir(&j.to).ok();
        }
        if exists(&j.from) && !matches!(spot(&j.default), Spot::Link(t) if same_path(&t, &j.from)) {
            if exists(&j.default) {
                link::remove(&j.default).ok();
            }
            (ops.link)(&j.from, &j.default).ok();
        }
    }
    let old_intact = if j.linked {
        exists(&j.from)
    } else {
        matches!(spot(&j.default), Spot::Folder)
    };
    if old_intact && exists(&j.to) {
        if j.to_existed {
            copy::empty(&j.to).ok();
        } else {
            copy::remove(&j.to).ok();
        }
    }
    j.clear();
}

/// Finishes or undoes a move that was started and not switched: because the
/// launcher restarted after a copy, or crashed part way. `None` when no move is
/// waiting. Call it before anything opens a file in the data folder.
pub fn recover(default: &Path, ops: &Ops) -> Option<Result<Recovered, Error>> {
    let path = Journal::path(default);
    let text = std::fs::read_to_string(&path).ok()?;
    let journal: Journal = match serde_json::from_str(&text) {
        Ok(j) => j,
        Err(e) => {
            std::fs::remove_file(&path).ok();
            return Some(Err(Error::io("Ignored an unreadable data folder move", e)));
        }
    };
    Some(match finish(&journal, ops, STARTUP_TRIES) {
        Ok(done) => Ok(done),
        Err(e) => {
            abort(&journal, ops);
            Err(e)
        }
    })
}

/// What the startup recovery reported, for the shell to show once.
pub fn startup() -> &'static std::sync::OnceLock<Option<Result<Recovered, Error>>> {
    static RESULT: std::sync::OnceLock<Option<Result<Recovered, Error>>> = std::sync::OnceLock::new();
    &RESULT
}

/// Runs [`recover`] once for the platform's default data folder and keeps the result.
pub fn recover_at_start() {
    startup().get_or_init(|| recover(&paths::platform_data_dir(), &Ops::system()));
}

#[cfg(test)]
mod tests;
