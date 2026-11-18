use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::baseline::{
    Baseline, DepotHashes, FileRecord, Hashes, Identity, Objects, sha256_bytes, sha256_file, sha256_opt,
};
use crate::journal::{Journal, Op, State};
use crate::placed::{self, Placed};
use crate::profile::VANILLA;
use crate::{Error, Result, addon_storage, io_err, with_retry};

pub const PROXY: &str = "version.dll";
pub const LOADER_DIR: &str = "swofty";
pub const OVERRIDE_HEADER: &str = "[Software\\\\Wine\\\\AppDefaults\\\\F1_25.exe\\\\DllOverrides]";
const OVERRIDE_VALUE: &str = "\"version\"=\"native,builtin\"";

#[derive(Debug, Clone)]
pub struct Game {
    pub dir: PathBuf,
    pub user_reg: Option<PathBuf>,
    pub identity: Identity,
}

impl Game {
    /// The game in `dir`, with the Proton prefix's user.reg when Steam
    /// installed it there, or the EA app's prefix from `launch.ea_prefix`.
    pub fn locate(data: &Path, dir: PathBuf) -> Result<Self> {
        let found = crate::steam::find_game().filter(|g| g.dir == dir);
        let user_reg = found
            .as_ref()
            .and_then(|g| g.prefix())
            .map(|p| p.join("user.reg"))
            .filter(|p| p.is_file())
            .or_else(|| {
                (found.is_none() && !cfg!(windows) && Source::detect(&dir) == Source::EaApp)
                    .then(|| {
                        crate::store::Store::new(data.to_path_buf())
                            .config()
                            .launch
                            .ea_prefix
                    })
                    .flatten()
                    .and_then(|p| crate::ea_app::prefix_user_reg(&p))
            });
        let identity = Identity::detect_cached(data, &dir, found.and_then(|g| g.build_id))?;
        Ok(Self {
            dir,
            user_reg,
            identity,
        })
    }
}

/// The store a game folder came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Steam,
    EaApp,
    Unknown,
}

impl Source {
    pub fn detect(dir: &Path) -> Self {
        if dir.join("EAJavelinInstaller_installscript.vdf").is_file() || crate::steam::in_steam_library(dir) {
            Self::Steam
        } else if dir.join("__Installer/installerdata.xml").is_file() {
            Self::EaApp
        } else {
            Self::Unknown
        }
    }
}

pub const EA_APP_EXPERIMENTAL: &str = "this is the EA app version of F1 25; Swofty's support for it is \
     experimental, so modded play may have bugs. Report them on the Swofty Developments Discord";

/// The `config.news_seen` key for the one-time EA app warning.
pub const EA_APP_SEEN: &str = "ea-app-experimental";

#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub profile: String,
    pub data: BTreeMap<String, PathBuf>,
    pub added: BTreeMap<String, PathBuf>,
    pub loader: BTreeMap<String, PathBuf>,
    pub registry_override: bool,
}

impl Plan {
    pub fn vanilla() -> Self {
        Self {
            profile: VANILLA.into(),
            ..Self::default()
        }
    }

    pub fn is_vanilla(&self) -> bool {
        self.data.is_empty() && self.added.is_empty() && self.loader.is_empty() && !self.registry_override
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Drift {
    Changed(String),
    Missing(String),
    Addition(String),
    RegistryOverride,
    OpenJournal(String),
    NewBuild { baseline: String, current: String },
    LeftInPlace(String),
    Stranded(String),
    NoOriginal(String),
}

impl std::fmt::Display for Drift {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Drift::Changed(p) => write!(f, "{p} differs from the baseline"),
            Drift::Missing(p) => write!(f, "{p} is missing"),
            Drift::Addition(p) => write!(f, "{p} is not part of the game"),
            Drift::RegistryOverride => write!(f, "the prefix has the version.dll override"),
            Drift::OpenJournal(id) => write!(f, "journal {id} is open"),
            Drift::NewBuild { baseline, current } => {
                write!(f, "game build {current} differs from baseline {baseline}")
            }
            Drift::LeftInPlace(p) => write!(
                f,
                "{p} changed since it was written (game update?); left in place"
            ),
            Drift::Stranded(p) => write!(
                f,
                "{p} is not what the earlier session wrote in the old game folder nor its original; left in place"
            ),
            Drift::NoOriginal(p) => write!(
                f,
                "{p} still holds a file swofty wrote and its original is not on record; verify the game files in Steam"
            ),
        }
    }
}

/// A file in the way of a plan: a file the plan adds that is already there,
/// or a game file the plan replaces that is not the original. `owned` means
/// it is provably the launcher's own (the bytes it recorded writing there, or
/// the bytes it is about to write), so it can be cleared without asking.
#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    pub path: String,
    pub sha256: Option<String>,
    pub owned: bool,
    pub replaced: bool,
}

impl std::fmt::Display for Conflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let p = &self.path;
        match (self.replaced, self.owned) {
            (true, true) => write!(f, "{p} still holds what an unfinished session wrote"),
            (false, true) => write!(f, "{p} was left behind by an unfinished session"),
            (true, false) => write!(f, "{p} is not the original, and swofty did not write it"),
            (false, false) => write!(
                f,
                "{p} is already in the game folder, and swofty did not put it there"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RolledBack {
    pub id: String,
    pub profile: String,
    pub session: bool,
    pub changes: usize,
    pub drift: Vec<Drift>,
    /// The files addons wrote that went to `addon-storage/` in the data
    /// folder, as game paths.
    pub kept: Vec<String>,
}

pub struct Installer<'a> {
    pub data: &'a Path,
    pub game: &'a Game,
    pub depot: &'a dyn DepotHashes,
    pub stop_after: Option<usize>,
    pub session: bool,
    /// Where a long install says how far it got.
    pub out: &'a dyn Fn(&str),
    /// Files copied so far, of how many, after each one.
    pub progress: &'a dyn Fn(usize, usize),
    pub free_space: fn(&Path) -> Option<u64>,
    /// The volume a folder is on; tells Play whether the build output and the
    /// game folder are on different drives.
    pub volume: fn(&Path) -> Option<crate::disk::Volume>,
    /// Copy every file instead of hard-linking build outputs; set by
    /// `SWOFTY_INSTALL_COPY=1`.
    pub copy_only: bool,
}

const PROGRESS_EVERY: std::time::Duration = std::time::Duration::from_secs(10);
/// Room on the data folder's volume per file an install places, for the
/// journal, `placed.json` and the hash cache, each rewritten through a
/// temporary copy.
const STATE_PER_FILE: u64 = 2048;

/// Where each placed file is in the link cache by game path, how many files
/// the sync copied and reused, and why there is no cache when it failed.
#[derive(Default)]
struct LinkCache {
    via: BTreeMap<String, PathBuf>,
    synced: Option<(usize, usize)>,
    failure: Option<String>,
}

fn quiet(_: &str) {}

fn unwatched(_: usize, _: usize) {}

fn link_replacing(src: &Path, path: &Path) -> std::io::Result<()> {
    let tmp = path.with_file_name(format!(
        ".{}.swofty-link",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    std::fs::remove_file(&tmp).ok();
    std::fs::hard_link(src, &tmp)?;
    let renamed = match std::fs::rename(&tmp, path) {
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && crate::clear_readonly(path) => {
            std::fs::rename(&tmp, path)
        }
        r => r,
    };
    if renamed.is_err() {
        std::fs::remove_file(&tmp).ok();
    }
    renamed
}

pub(crate) fn copy_replacing(src: &Path, path: &Path) -> std::io::Result<()> {
    let tmp = path.with_file_name(format!(
        ".{}.swofty-copy",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    std::fs::remove_file(&tmp).ok();
    if let Err(e) = std::fs::copy(src, &tmp) {
        std::fs::remove_file(&tmp).ok();
        return Err(e);
    }
    let renamed = match std::fs::rename(&tmp, path) {
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && crate::clear_readonly(path) => {
            std::fs::rename(&tmp, path)
        }
        r => r,
    };
    if renamed.is_err() {
        std::fs::remove_file(&tmp).ok();
    }
    renamed
}

fn rel_parts(rel: &str) -> Vec<String> {
    let parts: Vec<&str> = rel.split('/').collect();
    (1..parts.len()).map(|i| parts[..i].join("/")).collect()
}

fn remove_section(text: &str, header: &str) -> Option<String> {
    let start = text.find(header)?;
    let from = if start > 0 && text.as_bytes()[start - 1] == b'\n' {
        start - 1
    } else {
        start
    };
    let end = text[start + header.len()..]
        .find("\n[")
        .map_or(text.len(), |i| start + header.len() + i);
    let mut out = String::with_capacity(text.len());
    out.push_str(&text[..from]);
    out.push_str(&text[end..]);
    if !text[end..].is_empty() && from == start {
        out = format!("{}{}", &text[..from], &text[end + 1..]);
    }
    if text.ends_with('\n') && !out.ends_with('\n') && !out.is_empty() {
        out.push('\n');
    }
    Some(out)
}

fn override_section(text: &str) -> Option<std::ops::Range<usize>> {
    let start = text.find(OVERRIDE_HEADER)?;
    let end = text[start..].find("\n[").map_or(text.len(), |i| start + i);
    Some(start..end)
}

fn version_value(line: &str) -> Option<&str> {
    let (name, value) = line.split_once('=')?;
    let name = name.trim().trim_matches('"').trim_start_matches('*');
    name.eq_ignore_ascii_case("version")
        .then(|| value.trim().trim_matches('"'))
}

/// Whether a prefix's user.reg has F1_25.exe load a native version.dll
/// first. A section for F1_25.exe without a version.dll value leaves Wine
/// on its builtin one.
pub fn has_override(text: &str) -> bool {
    override_section(text).is_some_and(|s| {
        text[s]
            .lines()
            .skip(1)
            .filter_map(version_value)
            .any(|v| v.starts_with('n'))
    })
}

/// user.reg with the version.dll override, in F1_25.exe's section when the
/// prefix has one; None when that section already sets version.dll.
pub fn add_override(text: &str) -> Option<String> {
    let Some(s) = override_section(text) else {
        let mut out = text.to_owned();
        if !out.ends_with('\n') && !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("\n{OVERRIDE_HEADER}\n{OVERRIDE_VALUE}\n"));
        return Some(out);
    };
    if text[s.clone()]
        .lines()
        .skip(1)
        .any(|l| version_value(l).is_some())
    {
        return None;
    }
    let mut at = text[s.clone()].find('\n').map_or(s.end, |i| s.start + i + 1);
    while text[at..s.end].starts_with('#') {
        at = text[at..s.end].find('\n').map_or(s.end, |i| at + i + 1);
    }
    let sep = if at == text.len() && !text.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    Some(format!("{}{sep}{OVERRIDE_VALUE}\n{}", &text[..at], &text[at..]))
}

/// Whether `path` holds, byte for byte, the proxy this launcher ships. A
/// proxy the launcher's records account for is gone before this is asked:
/// the rollback and the sweep removed it.
fn shipped_proxy(path: &Path) -> Result<bool> {
    let Some(shipped) = crate::builtin::loader_dir().and_then(|d| sha256_opt(&d.join(PROXY)).ok().flatten())
    else {
        return Ok(false);
    };
    Ok(sha256_opt(path)?.is_some_and(|now| now == shipped))
}

/// user.reg without the version.dll override, and without F1_25.exe's
/// section when nothing else is set in it; None when the section sets other
/// DLLs but not version.dll.
fn remove_override(text: &str) -> Option<String> {
    let s = override_section(text)?;
    let lines = || text[s.clone()].lines().skip(1).map(str::trim);
    if !lines().any(|l| !l.is_empty() && !l.starts_with('#') && version_value(l).is_none()) {
        return remove_section(text, OVERRIDE_HEADER);
    }
    if !lines().any(|l| version_value(l).is_some()) {
        return None;
    }
    let kept: String = text[s.clone()]
        .split_inclusive('\n')
        .enumerate()
        .filter(|(i, l)| *i == 0 || version_value(l).is_none())
        .map(|(_, l)| l)
        .collect();
    Some(format!("{}{kept}{}", &text[..s.start], &text[s.end..]))
}

impl<'a> Installer<'a> {
    pub fn new(data: &'a Path, game: &'a Game) -> Self {
        Self {
            data,
            game,
            depot: &(),
            stop_after: None,
            session: false,
            out: &quiet,
            progress: &unwatched,
            free_space: crate::disk::free_space,
            volume: crate::disk::volume,
            copy_only: std::env::var_os("SWOFTY_INSTALL_COPY").is_some_and(|v| v == "1"),
        }
    }

    fn objects(&self) -> Objects {
        Objects {
            root: self.data.join("baseline/objects"),
        }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.game.dir.join(rel)
    }

    pub fn baseline(&self) -> Result<Baseline> {
        Baseline::load_or_new(self.data, &self.game.identity)
    }

    pub fn recover(&self) -> Result<Vec<Drift>> {
        Ok(self.roll_back(false)?.into_iter().flat_map(|r| r.drift).collect())
    }

    /// Roll back every open journal, or only the ones a `swofty run` session
    /// opened, oldest first.
    pub fn roll_back(&self, sessions_only: bool) -> Result<Vec<RolledBack>> {
        let mut hashes = Hashes::load(self.data);
        let out = self.roll_back_with(sessions_only, &mut hashes);
        hashes.save();
        out
    }

    fn roll_back_with(&self, sessions_only: bool, hashes: &mut Hashes) -> Result<Vec<RolledBack>> {
        let mut out = Vec::new();
        for j in Journal::open(self.data)? {
            if sessions_only && !j.session {
                continue;
            }
            let (id, profile, session) = (j.id.clone(), j.profile.clone(), j.session);
            let changes = j.entries.len();
            let (drift, kept) = self.rollback(j, hashes)?;
            out.push(RolledBack {
                id,
                profile,
                session,
                changes,
                drift,
                kept,
            });
        }
        Ok(out)
    }

    /// A journal written for a game folder that no longer has the game, as
    /// after Steam moved the library or the game was reinstalled elsewhere.
    /// Only the files here that still hold exactly what it wrote are its to
    /// roll back; the rest stay and are reported.
    fn stranded(&self, j: &Journal) -> bool {
        j.game.as_deref().is_some_and(|was| {
            !was.join(crate::GAME_EXE).is_file() && !crate::copies::same_dir(was, &self.game.dir)
        })
    }

    fn rollback(&self, j: Journal, hashes: &mut Hashes) -> Result<(Vec<Drift>, Vec<String>)> {
        let ids = addon_storage::ids(j.entries.iter().map(|e| e.path.as_str()));
        let kept = self.keep_storage(&ids, std::slice::from_ref(&j))?;
        for id in &ids {
            addon_storage::save_current(self.data, id)?;
        }
        crate::stdlib::careers::sync(self.data, Some(&self.game.dir), Some(&j.profile)).ok();
        if j.session {
            let log =
                std::fs::read_to_string(self.game.dir.join(crate::session::LOADER_LOG)).unwrap_or_default();
            crate::stdlib::careers::record_session(self.data, &j.profile, &log, || {
                crate::build::stock_max_ids(self.data, self.game)
            })
            .ok();
        }
        let mut report = Vec::new();
        let stranded = self.stranded(&j);
        let same_build = j.build_id == self.game.identity.build_id;
        let left = |path: &str| {
            if stranded {
                Drift::Stranded(path.to_owned())
            } else {
                Drift::LeftInPlace(path.to_owned())
            }
        };
        for e in j.entries.iter().rev() {
            match e.op {
                Op::ReplaceFile => {
                    let path = self.path(&e.path);
                    let now = hashes.sha256_opt(&path)?;
                    let before = e.before.as_deref().expect("replace-file has before");
                    if now.as_deref() == Some(before) {
                        continue;
                    }
                    let ours = now == e.after || (!stranded && e.state == State::Pending);
                    if ours && same_build {
                        self.restore_original(before, &path)?;
                        hashes.wrote(&path, before);
                    } else {
                        report.push(left(&e.path));
                    }
                }
                Op::CreateFile => {
                    let path = self.path(&e.path);
                    match hashes.sha256_opt(&path)? {
                        None => {}
                        Some(now)
                            if Some(&now) == e.after.as_ref()
                                || (!stranded
                                    && (e.state == State::Pending
                                        || e.path == crate::shaders::LIVE_FILE)) =>
                        {
                            with_retry(&path, || crate::remove_placed(&path))?;
                        }
                        Some(_) => report.push(left(&e.path)),
                    }
                }
                Op::CreateDir => {
                    let path = self.path(&e.path);
                    if path.is_dir() && std::fs::read_dir(&path).map_err(io_err(&path))?.next().is_none() {
                        with_retry(&path, || std::fs::remove_dir(&path))?;
                    }
                }
                Op::RegistryOverride => {
                    let path = PathBuf::from(&e.path);
                    let text = std::fs::read_to_string(&path).map_err(io_err(&path))?;
                    if let Some(out) = remove_override(&text) {
                        std::fs::write(&path, out).map_err(io_err(&path))?;
                    }
                }
            }
        }
        let mut placed = Placed::load(self.data)?;
        for e in &j.entries {
            match e.op {
                Op::ReplaceFile | Op::CreateFile => placed.remove(&e.path),
                Op::RegistryOverride => placed.registry = None,
                Op::CreateDir => {}
            }
        }
        placed.save(self.data)?;
        j.close()?;
        Ok((report, kept))
    }

    /// Move what the addons `ids` wrote through the storage API out of their
    /// game folders to `addon-storage/` in the data folder: every file there
    /// that neither `placed.json` nor one of `open` records the launcher
    /// placing.
    fn keep_storage(&self, ids: &BTreeSet<String>, open: &[Journal]) -> Result<Vec<String>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut managed: BTreeSet<String> = Placed::load(self.data)?.files.into_keys().collect();
        managed.extend(
            open.iter()
                .flat_map(|j| j.entries.iter().map(|e| placed::key(&e.path))),
        );
        let mut kept = Vec::new();
        for id in ids {
            kept.extend(addon_storage::keep(self.data, &self.game.dir, id, &managed)?);
        }
        Ok(kept)
    }

    fn restore_original(&self, sha: &str, path: &Path) -> Result<()> {
        const DELAYS_MS: [u64; 5] = [50, 150, 300, 600, 1000];
        let mut attempt = 0;
        loop {
            match self.objects().restore_to(sha, path) {
                Err(Error::Io { source, .. })
                    if crate::is_locked_file_error(&source) && attempt < DELAYS_MS.len() =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(DELAYS_MS[attempt]));
                    attempt += 1;
                }
                r => return r,
            }
        }
    }

    /// Clear the conflicts a failed [`apply`](Self::apply) reported that are
    /// provably the launcher's own: originals go back over replaced files and
    /// added files are removed, with any folders that held only them. Anything
    /// not owned is left alone.
    pub fn clear(&self, conflicts: &[Conflict]) -> Result<Vec<String>> {
        let baseline = self.baseline()?;
        let mut placed = Placed::load(self.data)?;
        let mut notes = Vec::new();
        for c in conflicts.iter().filter(|c| c.owned) {
            let path = self.path(&c.path);
            if c.replaced {
                let rec = baseline
                    .files
                    .get(&c.path)
                    .ok_or_else(|| Error::Other(format!("{} has no backed-up original", c.path)))?;
                self.restore_original(&rec.sha256, &path)?;
                notes.push(format!("put the original {} back", c.path));
            } else {
                if path.is_file() {
                    with_retry(&path, || std::fs::remove_file(&path))?;
                }
                self.prune_dirs(&c.path)?;
                notes.push(format!("removed {}", c.path));
            }
            placed.remove(&c.path);
        }
        placed.save(self.data)?;
        Ok(notes)
    }

    /// Clean up files the placed-files record says the launcher wrote and no
    /// open journal accounts for: a session that lost its journal left them.
    /// Only files that still hold exactly the recorded bytes are touched.
    pub fn sweep(&self) -> Result<Vec<String>> {
        let mut hashes = Hashes::load(self.data);
        let result = self.sweep_with(&mut hashes);
        hashes.save();
        result
    }

    fn sweep_with(&self, hashes: &mut Hashes) -> Result<Vec<String>> {
        let mut placed = Placed::load(self.data)?;
        if placed.files.is_empty() && placed.registry.is_none() {
            return Ok(Vec::new());
        }
        let open = Journal::open(self.data)?;
        let journaled: BTreeSet<String> = open
            .iter()
            .flat_map(|j| j.entries.iter().map(|e| placed::key(&e.path)))
            .collect();
        let baseline = self.baseline()?;
        let mut notes = Vec::new();
        let overrides = open
            .iter()
            .flat_map(|j| &j.entries)
            .any(|e| e.op == Op::RegistryOverride);
        if let Some(reg) = placed.registry.clone().filter(|_| !overrides) {
            let path = PathBuf::from(&reg);
            if let Some(out) = std::fs::read_to_string(&path)
                .ok()
                .and_then(|t| remove_override(&t))
            {
                std::fs::write(&path, out).map_err(io_err(&path))?;
                notes.push("removed the version.dll override from the Proton prefix".to_owned());
            }
            placed.registry = None;
        }
        for (key, f) in placed.files.clone() {
            if journaled.contains(&key) {
                continue;
            }
            let path = self.path(&f.path);
            if hashes.sha256_opt(&path)?.as_deref() == Some(f.sha256.as_str()) {
                if f.replaced {
                    match baseline.files.get(&f.path) {
                        Some(rec) if self.objects().has(&rec.sha256) => {
                            self.restore_original(&rec.sha256, &path)?;
                            hashes.wrote(&path, &rec.sha256);
                            notes.push(format!("put the original {} back", f.path));
                        }
                        _ => {
                            notes.push(format!(
                                "{} holds an unfinished session's file and has no backed-up original; left in place",
                                f.path
                            ));
                            continue;
                        }
                    }
                } else {
                    with_retry(&path, || std::fs::remove_file(&path))?;
                    self.prune_dirs(&f.path)?;
                    notes.push(format!("removed {}", f.path));
                }
            }
            placed.files.remove(&key);
        }
        placed.save(self.data)?;
        Ok(notes)
    }

    fn prune_dirs(&self, rel: &str) -> Result<()> {
        for dir in rel_parts(rel).iter().rev() {
            let path = self.path(dir);
            let empty = path.is_dir() && std::fs::read_dir(&path).map_err(io_err(&path))?.next().is_none();
            if !empty {
                break;
            }
            with_retry(&path, || std::fs::remove_dir(&path))?;
        }
        Ok(())
    }

    /// Move what the session left in the loader folder (its logs, diagnostic
    /// dumps, load-stall reports and crash reports with their minidumps) to `logs`, and remove the
    /// folders it leaves empty, once no journal is open. Returns how many files
    /// moved.
    pub fn tidy(&self, logs: &Path) -> Result<usize> {
        let loader = self.path(LOADER_DIR);
        let prefix = format!("{LOADER_DIR}/");
        if !loader.is_dir()
            || !Journal::open(self.data)?.is_empty()
            || self.baseline()?.files.keys().any(|k| k.starts_with(&prefix))
        {
            return Ok(0);
        }
        let status = self.path(crate::launcher_saves::STATUS_FILE);
        if status.is_file() {
            with_retry(&status, || std::fs::remove_file(&status))?;
        }
        let mut moved = 0;
        for e in std::fs::read_dir(&loader).map_err(io_err(&loader))?.flatten() {
            let path = e.path();
            let name = e.file_name();
            let name = name.to_string_lossy();
            let crash =
                crate::crash_report::is_crash_name(&name) || crate::crash_report::is_stall_name(&name);
            let output = crash
                || path
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("log") || x.eq_ignore_ascii_case("dump"));
            if output && path.is_file() {
                std::fs::create_dir_all(logs).map_err(io_err(logs))?;
                crate::move_file(&path, &logs.join(e.file_name()))?;
                moved += 1;
            }
        }
        remove_empty(&loader)?;
        Ok(moved)
    }

    /// Whether the file at `rel`, which the baseline has no original for, may
    /// be recorded as one. Never when it holds exactly what the launcher wrote
    /// there in place of an original (a session whose journal and baseline
    /// were both lost); and while a damaged baseline is being rebuilt, only
    /// when its bytes are already a backed-up original.
    fn may_record(&self, baseline: &Baseline, objects: &Objects, placed: &Placed, rel: &str) -> Result<bool> {
        let ours = placed.get(rel).filter(|p| p.replaced);
        if ours.is_none() && !baseline.checking() {
            return Ok(true);
        }
        let now = sha256_file(&self.path(rel))?;
        if ours.is_some_and(|p| p.sha256 == now) {
            return Ok(false);
        }
        Ok(!baseline.checking() || objects.has(&now))
    }

    fn step(&self, n: &mut usize) -> Result<()> {
        if self.session {
            crate::busy::check(self.data)?;
        }
        *n += 1;
        match self.stop_after {
            Some(limit) if *n > limit => Err(Error::Other(format!("stopped after {limit} step(s)"))),
            _ => Ok(()),
        }
    }

    pub fn apply(&self, plan: &Plan) -> Result<()> {
        let mut hashes = Hashes::load(self.data);
        let out = self.apply_with(plan, &mut hashes);
        hashes.save();
        out
    }

    fn apply_with(&self, plan: &Plan, hashes: &mut Hashes) -> Result<()> {
        self.roll_back_with(false, hashes)?;
        // A live shader config left behind by a session that ended without its rollback is the launcher's
        // own; no session runs during an apply.
        let live = self.path(crate::shaders::LIVE_FILE);
        if live.is_file() {
            with_retry(&live, || crate::remove_placed(&live))?;
            for dir in live.ancestors().skip(1).take(2) {
                if std::fs::remove_dir(dir).is_err() {
                    break;
                }
            }
        }
        if plan.is_vanilla() {
            return Ok(());
        }
        let mut with_state;
        let plan = if plan.loader.is_empty() {
            plan
        } else {
            let file = self.data.join("state").join("core-state.json");
            let dir = file.parent().expect("has parent");
            std::fs::create_dir_all(dir).map_err(io_err(dir))?;
            std::fs::write(&file, crate::stdlib::state::text(self.data)).map_err(io_err(&file))?;
            with_state = plan.clone();
            with_state
                .loader
                .insert(crate::stdlib::state::FILE.to_owned(), file);
            &with_state
        };
        let loaded = self.baseline()?;
        let space = self.space(plan, &loaded);
        for (dir, v, need) in &space {
            if let Some(free) = (self.free_space)(dir).filter(|free| need > free) {
                return Err(Error::NoSpace {
                    drive: v.name.clone(),
                    need: *need,
                    free,
                });
            }
        }
        self.place(plan, hashes, loaded).map_err(|e| self.full(e, &space))
    }

    /// The bytes an install of `plan` writes on each volume: its copies, less
    /// the files they replace, on the game folder's; the originals it backs
    /// up for the first time and its state files on the data folder's.
    fn space(&self, plan: &Plan, baseline: &Baseline) -> Vec<(PathBuf, crate::disk::Volume, u64)> {
        let size = |p: &Path| std::fs::metadata(p).map_or(0, |m| m.len());
        let (mut game, mut data) = (0, 0);
        for (rel, src) in &plan.data {
            let now = size(&self.path(rel));
            game += size(src).saturating_sub(now);
            if !baseline.files.contains_key(rel) {
                data += now;
            }
        }
        for src in plan.added.values().chain(plan.loader.values()) {
            game += size(src);
        }
        data += (plan.data.len() + plan.added.len() + plan.loader.len()) as u64 * STATE_PER_FILE;
        crate::disk::volumes([(self.game.dir.as_path(), game), (self.data, data)])
    }

    /// A write that failed on a full volume, as the volume and the room the
    /// install needed there.
    fn full(&self, e: Error, space: &[(PathBuf, crate::disk::Volume, u64)]) -> Error {
        let Error::Io { path, source } = &e else {
            return e;
        };
        let on = crate::disk::volume(path).filter(|_| crate::disk::is_full(source));
        match on.and_then(|on| space.iter().find(|(_, v, _)| v.id == on.id)) {
            Some((dir, v, need)) => Error::NoSpace {
                drive: v.name.clone(),
                need: *need,
                free: (self.free_space)(dir).unwrap_or(0),
            },
            None => e,
        }
    }

    fn place(&self, plan: &Plan, hashes: &mut Hashes, loaded: Baseline) -> Result<()> {
        crate::now_doing("checking the game files");
        if let Some(rel) = plan
            .data
            .keys()
            .chain(plan.added.keys())
            .chain(plan.loader.keys())
            .find(|rel| crate::stock::is_upscaler_drop_in(rel))
        {
            return Err(Error::Resolve(format!(
                "{rel} is an upscaler drop-in file and stays as the player put it; no addon or profile can replace it"
            )));
        }
        let objects = self.objects();
        let mut baseline = loaded.clone();
        let mut placed = Placed::load(self.data)?;
        let mut after = BTreeMap::new();
        for (rel, src) in plan.data.iter().chain(&plan.added).chain(&plan.loader) {
            after.insert(rel.as_str(), hashes.sha256(src)?);
        }
        let (mut backed_up, mut missing) = (Vec::new(), Vec::new());
        for rel in plan.data.keys() {
            if std::fs::symlink_metadata(self.path(rel)).is_ok() {
                continue;
            }
            match baseline.files.get(rel).filter(|r| objects.has(&r.sha256)) {
                Some(rec) => backed_up.push((rel, rec.sha256.clone())),
                None => missing.push(rel.clone()),
            }
        }
        if !missing.is_empty() {
            return Err(Error::MissingStock {
                files: missing,
                version: crate::exe_version::read(&self.path(crate::GAME_EXE)),
            });
        }
        for (rel, sha) in backed_up {
            self.restore_original(&sha, &self.path(rel))?;
            (self.out)(&format!(
                "{rel} was missing from the game folder; put back its backed-up original"
            ));
        }
        let mut conflicts = Vec::new();
        let mut unproven = Vec::new();
        for rel in plan.data.keys() {
            let path = self.path(rel);
            if !baseline.files.contains_key(rel)
                && let Some(left) = placed.get(rel).filter(|p| !p.replaced)
                && hashes.sha256_opt(&path)?.as_deref() == Some(left.sha256.as_str())
            {
                conflicts.push(Conflict {
                    path: rel.clone(),
                    sha256: Some(left.sha256.clone()),
                    owned: true,
                    replaced: false,
                });
                continue;
            }
            if !baseline.files.contains_key(rel) && !self.may_record(&baseline, &objects, &placed, rel)? {
                unproven.push(rel.clone());
                continue;
            }
            let rec = baseline
                .record(&objects, &self.game.dir, rel, self.depot)?
                .clone();
            let now = hashes.sha256(&path)?;
            if now != rec.sha256 {
                let owned = placed.wrote(rel, &now) || after[rel.as_str()] == now;
                if !owned {
                    match self.depot.stock(rel, &path)? {
                        Some(true) => {
                            let size = std::fs::metadata(&path).map_err(io_err(&path))?.len();
                            let sha256 = objects.put_file(&path)?;
                            baseline.files.insert(
                                rel.clone(),
                                FileRecord {
                                    size,
                                    sha256,
                                    source: crate::baseline::Source::Depot,
                                },
                            );
                            (self.out)(&format!(
                                "{rel} matches Steam's copy; its recorded original was out of date and has been replaced"
                            ));
                            continue;
                        }
                        Some(false) => {
                            (self.out)(&format!(
                                "{rel} is not Steam's copy and swofty did not write it: {}",
                                describe_file(&path, &now)
                            ));
                            return Err(Error::NotStock(vec![rel.clone()]));
                        }
                        None => (self.out)(&format!(
                            "{rel} differs from its recorded original and cannot be checked against Steam's list: {}",
                            describe_file(&path, &now)
                        )),
                    }
                }
                conflicts.push(Conflict {
                    owned,
                    path: rel.clone(),
                    sha256: Some(now),
                    replaced: true,
                });
            }
        }
        if !unproven.is_empty() {
            let rebuilt = baseline.rebuild.is_some();
            if let Some(r) = baseline.rebuild.as_mut() {
                r.told_to_verify = true;
            }
            baseline.save(self.data)?;
            let first = &unproven[0];
            let more = match unproven.len() {
                1 => String::new(),
                n => format!(" (and {} more)", n - 1),
            };
            return Err(Error::Other(if rebuilt {
                format!(
                    "{}. {first}{more} could not be checked against a backed-up original; nothing in the game folder was changed",
                    crate::baseline::DAMAGED
                )
            } else {
                format!(
                    "{}. {first}{more} still holds a file swofty wrote, and no original is on record for it; nothing in the game folder was changed",
                    crate::baseline::DAMAGED
                )
            }));
        }
        if baseline != loaded {
            baseline.save(self.data)?;
        }
        for rel in plan.added.keys().chain(plan.loader.keys()) {
            let path = self.path(rel);
            if std::fs::symlink_metadata(&path).is_err() {
                continue;
            }
            let now = hashes.sha256_opt(&path)?;
            let stock = baseline.files.contains_key(rel);
            conflicts.push(Conflict {
                owned: now
                    .as_deref()
                    .is_some_and(|now| placed.wrote(rel, now) || (!stock && after[rel.as_str()] == now)),
                path: rel.clone(),
                sha256: now,
                replaced: false,
            });
        }
        if !conflicts.is_empty() {
            return Err(Error::Leftovers(conflicts));
        }
        let mut j = Journal::create(
            self.data,
            &plan.profile,
            &self.game.identity.build_id,
            self.session,
        )?;
        j.game = Some(self.game.dir.clone());
        for rel in plan.data.keys() {
            placed.insert(rel, after[rel.as_str()].clone(), true);
        }
        for rel in plan.added.keys().chain(plan.loader.keys()) {
            placed.insert(rel, after[rel.as_str()].clone(), false);
        }
        let registry = match &self.game.user_reg {
            Some(reg) if plan.registry_override => {
                let text = std::fs::read_to_string(reg).map_err(io_err(reg))?;
                let name = reg.display().to_string();
                let ours = placed.registry.as_deref() == Some(name.as_str());
                match add_override(&text) {
                    Some(out) => Some((reg, name, text, out)),
                    None => ours.then(|| (reg, name, text.clone(), text)),
                }
            }
            _ => None,
        };
        if let Some((_, name, _, _)) = &registry {
            placed.registry = Some(name.clone());
        }
        placed.save(self.data)?;
        let mut steps = Vec::new();
        for rel in plan.data.keys() {
            let before = baseline.files[rel].sha256.clone();
            steps.push((
                Op::ReplaceFile,
                rel.clone(),
                Some(before),
                Some(after[rel.as_str()].clone()),
            ));
        }
        let mut dirs = BTreeSet::new();
        for rel in plan.added.keys().chain(plan.loader.keys()) {
            for dir in rel_parts(rel) {
                if !self.path(&dir).is_dir() && dirs.insert(dir.clone()) {
                    steps.push((Op::CreateDir, dir, None, None));
                }
            }
            steps.push((
                Op::CreateFile,
                rel.clone(),
                None,
                Some(after[rel.as_str()].clone()),
            ));
        }
        let registry = match registry {
            Some((reg, name, text, out)) => {
                let (before, now) = (objects.put_file(reg)?, sha256_bytes(out.as_bytes()));
                steps.push((Op::RegistryOverride, name, Some(before), Some(now)));
                Some((reg, text, out))
            }
            None => None,
        };
        j.begin_all(steps)?;
        let sources: BTreeMap<&str, &PathBuf> = plan
            .data
            .iter()
            .chain(&plan.added)
            .chain(&plan.loader)
            .map(|(rel, src)| (rel.as_str(), src))
            .collect();
        crate::now_doing("copying the mods into the game folder");
        let (mut n, mut copied, mut told) = (0, 0, std::time::Instant::now());
        let built = crate::store::overlay_root(self.data);
        let mut linking = !self.copy_only;
        let (mut linked, mut linked_cache, mut link_failure) = (0usize, 0usize, None::<String>);
        let LinkCache {
            via,
            synced: cached,
            failure: cache_failure,
        } = if linking {
            self.link_cache(&built, &j.entries, &sources)
        } else {
            LinkCache::default()
        };
        for i in 0..j.entries.len() {
            self.step(&mut n)?;
            let e = &j.entries[i];
            let path = self.path(&e.path);
            let mut link = |src: &Path, replacing: bool| -> bool {
                let cache = via.get(e.path.as_str());
                if !linking || (cache.is_none() && !src.starts_with(&built)) {
                    return false;
                }
                let from_cache = cache.is_some();
                let src = cache.map_or(src, |p| p.as_path());
                let made = if replacing {
                    link_replacing(src, &path)
                } else {
                    std::fs::hard_link(src, &path)
                };
                match made {
                    Ok(()) => {
                        linked_cache += usize::from(from_cache);
                        true
                    }
                    Err(err) => {
                        link_failure = Some(err.to_string());
                        linking = false;
                        false
                    }
                }
            };
            match e.op {
                Op::ReplaceFile => {
                    let src = sources[e.path.as_str()];
                    if link(src, true) {
                        linked += 1;
                        hashes.wrote(src, e.after.as_deref().expect("a placed file has after"));
                    } else {
                        copy_replacing(src, &path)
                            .map_err(|err| crate::quarantine::copy_error(&e.path, &path, err))?;
                        crate::clear_readonly(&path);
                    }
                }
                Op::CreateDir => std::fs::create_dir(&path).map_err(io_err(&path))?,
                // Windows refused the file as a threat. The entry stays pending, so
                // the rollback clears whatever was left, and the check before the
                // game starts finds the file missing and refuses the launch with
                // the antivirus report instead of a bare copy error here.
                Op::CreateFile => {
                    let src = sources[e.path.as_str()];
                    if link(src, false) {
                        linked += 1;
                        hashes.wrote(src, e.after.as_deref().expect("a placed file has after"));
                    } else {
                        match copy_replacing(src, &path) {
                            Err(err) if crate::quarantine::is_virus_error(&err) => continue,
                            r => {
                                r.map_err(io_err(&path))?;
                                crate::clear_readonly(&path);
                            }
                        }
                    }
                }
                Op::RegistryOverride => {
                    if let Some((reg, text, out)) = &registry
                        && out != text
                    {
                        std::fs::write(reg, out).map_err(io_err(*reg))?;
                    }
                }
            }
            if matches!(e.op, Op::ReplaceFile | Op::CreateFile) {
                hashes.wrote(&path, e.after.as_deref().expect("a placed file has after"));
                copied += 1;
            }
            j.done(i)?;
            (self.progress)(copied, sources.len());
            if told.elapsed() >= PROGRESS_EVERY {
                (self.out)(&format!(
                    "installing: {copied} of {} file(s) copied",
                    sources.len()
                ));
                told = std::time::Instant::now();
            }
        }
        let mode = match (&link_failure, self.copy_only) {
            (_, true) => "copy (SWOFTY_INSTALL_COPY=1)".to_owned(),
            (Some(why), _) if cached.is_some() => format!("copy (link cache link failed: {why})"),
            (None, _) if cached.is_some() => {
                let (made, kept) = cached.unwrap_or_default();
                (self.out)(&format!(
                    "installing: {linked} linked ({linked_cache} from the link cache: {made} copied into it, {kept} reused), {} copied",
                    copied - linked
                ));
                String::new()
            }
            (None, _) if linked == 0 && cache_failure.is_some() => {
                format!(
                    "copy (link cache unavailable: {})",
                    cache_failure.as_deref().unwrap_or_default()
                )
            }
            (Some(why), _) => format!("copy (hard link failed: {why})"),
            (None, _) if linked == 0 => "copy (no build output to link)".to_owned(),
            (None, _) => "hard links".to_owned(),
        };
        if !mode.is_empty() {
            (self.out)(&format!(
                "installing: {mode}; {linked} file(s) linked, {} copied",
                copied - linked
            ));
        }
        let planned: BTreeSet<String> = sources.keys().map(|rel| placed::key(rel)).collect();
        for id in addon_storage::ids(sources.keys().copied()) {
            addon_storage::load_profile(self.data, &id, &plan.profile)?;
            let moved = addon_storage::put_back(self.data, &self.game.dir, &id, &planned)?;
            if !moved.is_empty() {
                (self.out)(&format!(
                    "storage: put back {} file(s) {id} wrote, from {}",
                    moved.len(),
                    addon_storage::addon_dir(self.data, &id).display()
                ));
            }
        }
        if !plan.loader.is_empty() {
            for line in crate::stdlib::state::migrate(self.data, &self.game.dir)? {
                (self.out)(&line);
            }
        }
        j.commit()
    }

    /// Brings the link cache on the game's drive up to date for the files
    /// that can't be linked straight from the build output: all of them when
    /// the build output is on another drive than the game folder, else the
    /// ones from outside it, such as loose addon files. Returns where each is
    /// in the cache, what the sync did, and why there is no cache when it
    /// could not be made.
    fn link_cache(
        &self,
        built: &Path,
        entries: &[crate::journal::Entry],
        sources: &BTreeMap<&str, &PathBuf>,
    ) -> LinkCache {
        let apart = crate::link_cache::apart((self.volume)(built), (self.volume)(&self.game.dir));
        let srcs: Vec<(&str, &Path)> = entries
            .iter()
            .filter(|e| matches!(e.op, Op::ReplaceFile | Op::CreateFile))
            .map(|e| (e.path.as_str(), sources[e.path.as_str()].as_path()))
            .filter(|(_, s)| apart == Some(true) || !s.starts_with(built))
            .collect();
        if srcs.is_empty() {
            if crate::link_cache::dir(&self.game.dir).exists() {
                crate::link_cache::remove(&self.game.dir);
            }
            return LinkCache::default();
        }
        crate::now_doing("copying the mods to the game drive");
        let mut told = std::time::Instant::now();
        let synced = crate::link_cache::sync(&self.game.dir, &srcs, self.free_space, &mut |done, total| {
            (self.progress)(done, total);
            if told.elapsed() >= PROGRESS_EVERY {
                (self.out)(&format!(
                    "installing: {done} of {total} file(s) copied into the link cache"
                ));
                told = std::time::Instant::now();
            }
        });
        match synced {
            Ok(s) => LinkCache {
                via: s.files,
                synced: Some((s.copied, s.reused)),
                failure: None,
            },
            Err(why) => {
                (self.out)(&format!(
                    "installing: link cache unavailable ({why}); copying instead"
                ));
                LinkCache {
                    failure: Some(why),
                    ..LinkCache::default()
                }
            }
        }
    }

    pub fn restore(&self) -> Result<Vec<Drift>> {
        self.restore_setting_aside().map(|(report, _)| report)
    }

    /// [`restore`](Self::restore), and where in the set-aside folder it moved
    /// a `version.dll` that is not the launcher's own: another tool's proxy,
    /// such as an ASI loader or a renamed upscaler, is kept, never deleted.
    pub fn restore_setting_aside(&self) -> Result<(Vec<Drift>, Option<PathBuf>)> {
        let mut report = self.recover()?;
        self.sweep()?;
        let proxy = self.path(PROXY);
        let baseline = self.baseline()?;
        let mut set_aside = None;
        if proxy.is_file() && !baseline.files.contains_key(PROXY) {
            if shipped_proxy(&proxy)? {
                with_retry(&proxy, || std::fs::remove_file(&proxy))?;
            } else {
                set_aside = Some(self.set_proxy_aside(&proxy)?);
            }
            report.retain(|d| !matches!(d, Drift::LeftInPlace(p) | Drift::Stranded(p) if p == PROXY));
        }
        let loader = self.path(LOADER_DIR);
        if loader.is_dir() {
            crate::stdlib::careers::keep(self.data, &self.game.dir)?;
            let ids = addon_storage::present(&self.game.dir)?;
            self.keep_storage(&ids, &Journal::open(self.data)?)?;
            let stranded = addon_storage::addon_dir(self.data, addon_storage::LOGO_CACHE);
            if stranded.is_dir() {
                with_retry(&stranded, || std::fs::remove_dir_all(&stranded))?;
            }
            let logs = self.data.join("sessions/orphaned-logs");
            for e in std::fs::read_dir(&loader).map_err(io_err(&loader))?.flatten() {
                if e.path().extension().is_some_and(|x| x == "log") {
                    std::fs::create_dir_all(&logs).map_err(io_err(&logs))?;
                    crate::move_file(&e.path(), &logs.join(e.file_name()))?;
                }
            }
            addon_storage::clear_tree(&loader)?;
        }
        if let Some(reg) = &self.game.user_reg
            && let Ok(text) = std::fs::read_to_string(reg)
            && let Some(out) = remove_override(&text)
        {
            self.objects().put_file(reg)?;
            std::fs::write(reg, out).map_err(io_err(reg))?;
        }
        let mut hashes = Hashes::load(self.data);
        for (rel, rec) in &baseline.files {
            let path = self.path(rel);
            if hashes.sha256_opt(&path)?.as_deref() != Some(rec.sha256.as_str()) {
                self.objects().restore_to(&rec.sha256, &path)?;
                hashes.wrote(&path, &rec.sha256);
            }
        }
        hashes.save();
        report.extend(self.verify()?);
        Ok((report, set_aside))
    }

    /// Move `proxy` to a folder of its own under the set-aside folder.
    fn set_proxy_aside(&self, proxy: &Path) -> Result<PathBuf> {
        let dir = crate::recovery::set_aside_dir(self.data);
        let mut stamp = crate::update::now_secs();
        while dir.join(stamp.to_string()).join(PROXY).exists() {
            stamp += 1;
        }
        let to = dir.join(stamp.to_string()).join(PROXY);
        let parent = to.parent().expect("has parent");
        std::fs::create_dir_all(parent).map_err(io_err(parent))?;
        crate::move_file(proxy, &to)?;
        Ok(to)
    }

    pub fn verify(&self) -> Result<Vec<Drift>> {
        let mut out: Vec<Drift> = Journal::open(self.data)?
            .into_iter()
            .map(|j| Drift::OpenJournal(j.id))
            .collect();
        let baseline = self.baseline()?;
        for (rel, rec) in &baseline.files {
            match sha256_opt(&self.path(rel))? {
                None => out.push(Drift::Missing(rel.clone())),
                Some(s) if s != rec.sha256 => out.push(Drift::Changed(rel.clone())),
                Some(_) => {}
            }
        }
        if self.path(PROXY).exists() && !baseline.files.contains_key(PROXY) {
            out.push(Drift::Addition(PROXY.into()));
        }
        if self.path(LOADER_DIR).exists()
            && !baseline.files.contains_key(LOADER_DIR)
            && let Some(rel) = self.added_file(&baseline, LOADER_DIR)
        {
            out.push(Drift::Addition(rel));
        }
        for f in Placed::load(self.data)?.files.values() {
            if f.replaced
                && !baseline.files.contains_key(&f.path)
                && sha256_opt(&self.path(&f.path))?.as_deref() == Some(f.sha256.as_str())
            {
                out.push(Drift::NoOriginal(f.path.clone()));
            }
        }
        if let Some(reg) = &self.game.user_reg
            && std::fs::read_to_string(reg).is_ok_and(|t| has_override(&t))
        {
            out.push(Drift::RegistryOverride);
        }
        let latest = std::fs::read_dir(self.data.join("baseline"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| crate::read_json::<Baseline>(&e.path().join("baseline.json")).ok())
            .find(|b| {
                b.build_id != self.game.identity.build_id && b.files.keys().any(|k| self.path(k).exists())
            });
        if let Some(old) = latest
            && baseline.files.is_empty()
        {
            out.push(Drift::NewBuild {
                baseline: old.build_id,
                current: self.game.identity.build_id.clone(),
            });
        }
        Ok(out)
    }

    /// The first file under the folder `rel` that the baseline has no original for, or `rel` itself
    /// when it is not a folder that can be listed. A folder that holds only empty folders has none.
    fn added_file(&self, baseline: &Baseline, rel: &str) -> Option<String> {
        let Ok(rd) = std::fs::read_dir(self.path(rel)) else {
            return Some(rel.to_owned());
        };
        let mut entries: Vec<std::fs::DirEntry> = rd.flatten().collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        entries.into_iter().find_map(|e| {
            let child = format!("{rel}/{}", e.file_name().to_string_lossy());
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                self.added_file(baseline, &child)
            } else {
                (!baseline.files.contains_key(&child)).then_some(child)
            }
        })
    }
}

fn remove_empty(dir: &Path) -> Result<bool> {
    let mut empty = true;
    for e in std::fs::read_dir(dir).map_err(io_err(dir))?.flatten() {
        let path = e.path();
        let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
        if !is_dir || !remove_empty(&path)? {
            empty = false;
        }
    }
    if empty {
        with_retry(dir, || std::fs::remove_dir(dir))?;
    }
    Ok(empty)
}

fn describe_file(path: &Path, sha256: &str) -> String {
    let meta = std::fs::metadata(path).ok();
    let size = meta
        .as_ref()
        .map_or_else(|| "?".to_owned(), |m| m.len().to_string());
    let mtime = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or_else(|| "?".to_owned(), |d| d.as_secs().to_string());
    format!("size {size}, modified {mtime} (unix seconds), sha256 {sha256}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_steam_and_ea_app_folders_apart() {
        let root = std::env::temp_dir().join(format!("swofty-source-{}", std::process::id()));
        let touch = |rel: &str| {
            let p = root.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "x").unwrap();
        };
        touch("ea/F1 25/F1_25.exe");
        touch("ea/F1 25/__Installer/installerdata.xml");
        touch("steam/F1 25/F1_25.exe");
        touch("steam/F1 25/EAJavelinInstaller_installscript.vdf");
        touch("lib/steamapps/appmanifest_3059520.acf");
        touch("lib/steamapps/common/F1 25/F1_25.exe");
        touch("lib/steamapps/common/F1 25/__Installer/installerdata.xml");
        touch("bare/F1 25/F1_25.exe");
        assert_eq!(Source::detect(&root.join("ea/F1 25")), Source::EaApp);
        assert_eq!(Source::detect(&root.join("steam/F1 25")), Source::Steam);
        assert_eq!(
            Source::detect(&root.join("lib/steamapps/common/F1 25")),
            Source::Steam
        );
        assert_eq!(Source::detect(&root.join("bare/F1 25")), Source::Unknown);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_link_cache_copy_reports_its_progress() {
        let root = std::env::temp_dir().join(format!("swofty-link-progress-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let built = root.join("built");
        let outside = root.join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let game = Game {
            dir: root.join("game"),
            user_reg: None,
            identity: Identity {
                build_id: "1".into(),
                exe_sha256: String::new(),
            },
        };
        std::fs::create_dir_all(&game.dir).unwrap();
        let mut entries = Vec::new();
        let mut files = Vec::new();
        for n in 0..3u32 {
            let src = outside.join(format!("f{n}.bin"));
            std::fs::write(&src, [n as u8; 8]).unwrap();
            entries.push(crate::journal::Entry {
                seq: n,
                op: Op::CreateFile,
                path: format!("mods/f{n}.bin"),
                before: None,
                after: None,
                state: crate::journal::State::Pending,
            });
            files.push(src);
        }
        let sources: BTreeMap<&str, &PathBuf> = entries
            .iter()
            .map(|e| e.path.as_str())
            .zip(files.iter())
            .collect();
        let seen = std::cell::RefCell::new(Vec::new());
        let progress = |done: usize, total: usize| seen.borrow_mut().push((done, total));
        let inst = Installer {
            progress: &progress,
            free_space: |_| None,
            ..Installer::new(&root, &game)
        };
        let cache = inst.link_cache(&built, &entries, &sources);
        assert_eq!(cache.synced, Some((3, 0)), "{:?}", cache.failure);
        assert_eq!(seen.into_inner(), [(1, 3), (2, 3), (3, 3)]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_full_volume_is_named_with_what_the_install_needed_there() {
        let root = std::env::temp_dir().join(format!("swofty-full-{}", std::process::id()));
        let game = Game {
            dir: root.join("game"),
            user_reg: None,
            identity: Identity {
                build_id: "1".into(),
                exe_sha256: String::new(),
            },
        };
        let inst = Installer {
            free_space: |_| Some(5 << 20),
            ..Installer::new(&root, &game)
        };
        let space = crate::disk::volumes([(game.dir.as_path(), 3 << 30)]);
        let v = space[0].1.clone();
        let full = |source: std::io::Error| Error::Io {
            path: root.join("state/journal/x.json.tmp"),
            source,
        };
        match inst.full(full(std::io::ErrorKind::StorageFull.into()), &space) {
            Error::NoSpace { drive, need, free } => {
                assert_eq!((drive, need, free), (v.name, 3 << 30, 5 << 20))
            }
            e => panic!("{e}"),
        }
        assert!(matches!(
            inst.full(full(std::io::ErrorKind::PermissionDenied.into()), &space),
            Error::Io { .. }
        ));
        assert!(matches!(
            inst.full(full(std::io::ErrorKind::StorageFull.into()), &[]),
            Error::Io { .. }
        ));
        let shown = Error::NoSpace {
            drive: "C:".into(),
            need: 3 << 30,
            free: 1 << 29,
        };
        assert_eq!(
            shown.to_string(),
            "not enough free space on C: this profile needs about 3.0 GB, 0.5 GB free"
        );
    }

    #[test]
    fn removes_appended_section_exactly() {
        let orig = "WINE REGISTRY Version 2\n\n[Software\\\\Wine] 1\n\"a\"=\"b\"\n";
        let with = format!("{orig}\n{OVERRIDE_HEADER}\n{OVERRIDE_VALUE}\n");
        assert_eq!(remove_section(&with, OVERRIDE_HEADER).unwrap(), orig);
        let middle = format!("{orig}\n{OVERRIDE_HEADER}\n{OVERRIDE_VALUE}\n\n[Other] 2\n\"x\"=\"y\"\n");
        assert_eq!(
            remove_section(&middle, OVERRIDE_HEADER).unwrap(),
            format!("{orig}\n[Other] 2\n\"x\"=\"y\"\n")
        );
    }

    #[test]
    fn an_empty_section_gets_the_override_and_loses_it_again() {
        let orig = format!(
            "WINE REGISTRY Version 2\n\n{OVERRIDE_HEADER} 1792606023\n#time=1dd6186fa06f04c\n\n[Other] 2\n\"x\"=\"y\"\n"
        );
        assert!(!has_override(&orig));
        let with = add_override(&orig).unwrap();
        assert_eq!(
            with,
            format!(
                "WINE REGISTRY Version 2\n\n{OVERRIDE_HEADER} 1792606023\n#time=1dd6186fa06f04c\n{OVERRIDE_VALUE}\n\n[Other] 2\n\"x\"=\"y\"\n"
            )
        );
        assert!(has_override(&with));
        assert_eq!(add_override(&with), None);
        assert_eq!(
            remove_override(&with).unwrap(),
            "WINE REGISTRY Version 2\n\n[Other] 2\n\"x\"=\"y\"\n"
        );
        assert_eq!(
            remove_override(&orig).unwrap(),
            "WINE REGISTRY Version 2\n\n[Other] 2\n\"x\"=\"y\"\n"
        );
    }

    #[test]
    fn the_override_joins_and_leaves_a_section_with_other_dlls() {
        let orig = format!("WINE REGISTRY Version 2\n\n{OVERRIDE_HEADER} 5\n\"d3d11\"=\"native\"\n");
        let with = add_override(&orig).unwrap();
        assert_eq!(
            with,
            format!(
                "WINE REGISTRY Version 2\n\n{OVERRIDE_HEADER} 5\n{OVERRIDE_VALUE}\n\"d3d11\"=\"native\"\n"
            )
        );
        assert!(has_override(&with));
        assert_eq!(remove_override(&with).unwrap(), orig);
        assert_eq!(remove_override(&orig), None);
        let header_only = format!("WINE REGISTRY Version 2\n\n{OVERRIDE_HEADER}");
        assert_eq!(
            add_override(&header_only).unwrap(),
            format!("{header_only}\n{OVERRIDE_VALUE}\n")
        );
    }

    #[test]
    fn a_version_value_of_the_prefix_is_left_alone() {
        for (value, native) in [
            ("\"version\"=\"native,builtin\"", true),
            ("\"*version\"=\"n,b\"", true),
            ("\"Version\"=\"builtin\"", false),
            ("\"version\"=\"\"", false),
        ] {
            let text = format!("WINE REGISTRY Version 2\n\n{OVERRIDE_HEADER} 5\n{value}\n");
            assert_eq!(has_override(&text), native, "{value}");
            assert_eq!(add_override(&text), None, "{value}");
        }
        let fresh = add_override("WINE REGISTRY Version 2\n").unwrap();
        assert_eq!(
            fresh,
            format!("WINE REGISTRY Version 2\n\n{OVERRIDE_HEADER}\n{OVERRIDE_VALUE}\n")
        );
        assert_eq!(remove_override(&fresh).unwrap(), "WINE REGISTRY Version 2\n");
    }

    struct Stranded {
        root: PathBuf,
        data: PathBuf,
        game: Game,
    }

    impl Stranded {
        /// A journal for `old/F1 25`, which is gone, whose files are now in
        /// `moved/F1 25`: `a` and `c` hold what it wrote, `b` and `d` hold
        /// something else, `e` was still pending, and `f` already holds its
        /// original.
        fn new(name: &str, build_id: &str) -> Self {
            let root = std::env::temp_dir().join(format!("swofty-stranded-{name}-{}", std::process::id()));
            std::fs::remove_dir_all(&root).ok();
            let data = root.join("data");
            let dir = root.join("moved/F1 25");
            let objects = Objects {
                root: data.join("baseline/objects"),
            };
            let mut shas = BTreeMap::new();
            for (key, body) in [
                ("orig-a", "orig-a"),
                ("orig-b", "orig-b"),
                ("orig-e", "orig-e"),
                ("orig-f", "orig-f"),
            ] {
                let src = root.join("src").join(key);
                std::fs::create_dir_all(src.parent().unwrap()).unwrap();
                std::fs::write(&src, body).unwrap();
                shas.insert(key, objects.put_file(&src).unwrap());
            }
            for key in ["mod-a", "mod-b", "mod-c", "mod-d", "mod-e"] {
                shas.insert(key, sha256_bytes(key.as_bytes()));
            }
            for (rel, body) in [
                (crate::GAME_EXE, "exe"),
                ("a.erp", "mod-a"),
                ("b.erp", "from-elsewhere"),
                ("swofty/c.dll", "mod-c"),
                ("swofty/d.dll", "another-d"),
                ("e.erp", "partial-e"),
                ("f.erp", "orig-f"),
            ] {
                let p = dir.join(rel);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, body).unwrap();
            }
            let mut j = Journal::create(&data, "MODDED", "1", true).unwrap();
            j.game = Some(root.join("old/F1 25"));
            let sha = |k: &str| Some(shas[k].clone());
            j.begin_all([
                (Op::ReplaceFile, "a.erp".to_owned(), sha("orig-a"), sha("mod-a")),
                (Op::ReplaceFile, "b.erp".to_owned(), sha("orig-b"), sha("mod-b")),
                (Op::CreateFile, "swofty/c.dll".to_owned(), None, sha("mod-c")),
                (Op::CreateFile, "swofty/d.dll".to_owned(), None, sha("mod-d")),
                (Op::ReplaceFile, "e.erp".to_owned(), sha("orig-e"), sha("mod-e")),
                (Op::ReplaceFile, "f.erp".to_owned(), sha("orig-f"), sha("mod-e")),
            ])
            .unwrap();
            for i in [0, 1, 2, 3, 5] {
                j.done(i).unwrap();
            }
            j.commit().unwrap();
            let game = Game {
                dir,
                user_reg: None,
                identity: Identity {
                    build_id: build_id.into(),
                    exe_sha256: String::new(),
                },
            };
            Self { root, data, game }
        }

        fn read(&self, rel: &str) -> Option<String> {
            std::fs::read_to_string(self.game.dir.join(rel)).ok()
        }
    }

    impl Drop for Stranded {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).ok();
        }
    }

    #[test]
    fn a_journal_for_a_folder_that_is_gone_rolls_back_only_what_is_provably_ours() {
        let f = Stranded::new("moved", "1");
        let rolled = Installer::new(&f.data, &f.game).roll_back(true).unwrap();
        assert_eq!(rolled.len(), 1);
        let mut drift = rolled[0].drift.clone();
        drift.sort_by_key(|d| d.to_string());
        assert_eq!(
            drift,
            [
                Drift::Stranded("b.erp".into()),
                Drift::Stranded("e.erp".into()),
                Drift::Stranded("swofty/d.dll".into()),
            ]
        );
        assert_eq!(f.read("a.erp").as_deref(), Some("orig-a"));
        assert_eq!(f.read("b.erp").as_deref(), Some("from-elsewhere"));
        assert_eq!(f.read("swofty/c.dll"), None);
        assert_eq!(f.read("swofty/d.dll").as_deref(), Some("another-d"));
        assert_eq!(f.read("e.erp").as_deref(), Some("partial-e"));
        assert_eq!(f.read("f.erp").as_deref(), Some("orig-f"));
        assert!(Journal::open(&f.data).unwrap().is_empty());
        assert_eq!(
            Drift::Stranded("b.erp".into()).to_string(),
            "b.erp is not what the earlier session wrote in the old game folder nor its original; left in place"
        );
    }

    #[test]
    fn a_journal_for_a_folder_that_is_gone_leaves_a_newer_build_alone_and_closes() {
        let f = Stranded::new("rebuilt", "2");
        let rolled = Installer::new(&f.data, &f.game).roll_back(true).unwrap();
        let mut drift = rolled[0].drift.clone();
        drift.sort_by_key(|d| d.to_string());
        assert_eq!(
            drift,
            [
                Drift::Stranded("a.erp".into()),
                Drift::Stranded("b.erp".into()),
                Drift::Stranded("e.erp".into()),
                Drift::Stranded("swofty/d.dll".into()),
            ]
        );
        assert_eq!(f.read("a.erp").as_deref(), Some("mod-a"));
        assert_eq!(
            f.read("swofty/c.dll"),
            None,
            "an added file it wrote is removed on any build"
        );
        assert!(Journal::open(&f.data).unwrap().is_empty());
    }

    #[test]
    fn a_journal_whose_folder_still_has_the_game_treats_pending_files_as_its_own() {
        let f = Stranded::new("still-there", "1");
        let old = f.root.join("old/F1 25");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join(crate::GAME_EXE), "exe").unwrap();
        let rolled = Installer::new(&f.data, &f.game).roll_back(true).unwrap();
        let mut drift = rolled[0].drift.clone();
        drift.sort_by_key(|d| d.to_string());
        assert_eq!(
            drift,
            [
                Drift::LeftInPlace("b.erp".into()),
                Drift::LeftInPlace("swofty/d.dll".into()),
            ]
        );
        assert_eq!(f.read("e.erp").as_deref(), Some("orig-e"));
        assert!(Journal::open(&f.data).unwrap().is_empty());
    }

    #[test]
    fn dir_prefixes() {
        assert_eq!(
            rel_parts("swofty/addons/core/x.dll"),
            ["swofty", "swofty/addons", "swofty/addons/core"]
        );
        assert!(rel_parts("version.dll").is_empty());
    }
}
