//! The career vault: content-addressed snapshots of F1 25's profile save at
//! `$DATA/career-vault/<source>/`, one folder per Steam account
//! (`steam-<id3>`) or EA app copy (`ea-<user>`).
//!
//! ```text
//! career-vault/steam-843502855/
//!   blobs/<sha256>                  one per distinct file content, stored as-is
//!   snapshots/<stamp>-<reason>.json what the save held then (Snapshot)
//!   events.jsonl                    snapshot / unchanged / changed-outside / shrank / restored / pruned
//! ```

pub mod career_names;
pub mod careers;
pub mod cloud;
pub mod guard;
pub mod locate;
pub mod migrate;
pub mod restore;
pub mod retention;
pub mod watch;

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};

pub use locate::{Layout, Location};
pub use retention::Policy;

use crate::baseline::{hex, sha256_file};
use crate::{Error, Result, io_err};

pub const DIR: &str = "career-vault";
pub const DEFAULT_CAP_MB: u64 = 500;
/// A save at least this much smaller than the version before it is worth a
/// hint. Size alone never says a career is gone: ordinary play moves the file
/// by more, and a dropped container can be smaller.
pub const SHRINK_HINT: u64 = 24 * 1024;
const NOTICE: &str = "state/vault-notice.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    LauncherStart,
    BeforeSession,
    AfterSession,
    ChangedOutside,
    ForeignLaunch,
    ForeignExit,
    Manual,
    BeforeRestore,
    Restored,
    Migrated,
    Imported,
    ImportedProfile,
    #[serde(other)]
    Other,
}

impl Reason {
    pub fn slug(self) -> &'static str {
        match self {
            Self::LauncherStart => "launcher-start",
            Self::BeforeSession => "before-session",
            Self::AfterSession => "after-session",
            Self::ChangedOutside => "changed-outside",
            Self::ForeignLaunch => "foreign-launch",
            Self::ForeignExit => "foreign-exit",
            Self::Manual => "manual",
            Self::BeforeRestore => "before-restore",
            Self::Restored => "restored",
            Self::Migrated => "migrated",
            Self::Imported => "imported",
            Self::ImportedProfile => "imported-profile",
            Self::Other => "other",
        }
    }

    /// Where a check found a change, for its note.
    pub fn found(self) -> &'static str {
        match self {
            Self::LauncherStart => "found when the launcher started",
            Self::BeforeSession => "found before Play",
            Self::ForeignLaunch => "found when F1 25 started outside the launcher",
            Self::ForeignExit => "found after F1 25 started outside the launcher closed",
            Self::Manual => "found at a backup",
            _ => "found at a check",
        }
    }

    pub fn text(self) -> &'static str {
        match self {
            Self::LauncherStart => "launcher start",
            Self::BeforeSession => "before Play",
            Self::AfterSession => "after Play",
            Self::ChangedOutside => "changed outside the launcher",
            Self::ForeignLaunch => "F1 25 started outside the launcher",
            Self::ForeignExit => "after F1 25 started outside the launcher closed",
            Self::Manual => "backup",
            Self::BeforeRestore => "before a restore",
            Self::Restored => "restored",
            Self::Migrated => "from save-backups",
            Self::Imported => "Imported from Steam saves",
            Self::ImportedProfile => "Imported from another profile",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// `profile/<file>`, the same for the Steam and EA layouts.
    pub name: String,
    /// The name in the save folder.
    pub file: String,
    pub size: u64,
    pub sha256: String,
    pub sha1: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remotecache: Option<cloud::Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(skip)]
    pub id: String,
    pub created: String,
    pub at: i64,
    pub reason: Reason,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    pub launcher: String,
    #[serde(default)]
    pub exe_sha256: Option<String>,
    pub source: String,
    pub layout: Layout,
    pub save_dir: PathBuf,
    pub files: Vec<FileEntry>,
    #[serde(default)]
    pub absent_addons: Vec<String>,
    #[serde(default)]
    pub careers_ref: Option<String>,
    #[serde(default)]
    pub pinned: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Snapshot {
    pub fn total(&self) -> u64 {
        self.files.iter().map(|f| f.size).sum()
    }

    /// The cloud status recorded when the snapshot was taken.
    pub fn cloud(&self) -> cloud::Status {
        match self.layout {
            Layout::Ea | Layout::Launcher | Layout::LauncherEa => cloud::Status::Local,
            Layout::Steam => cloud::combine(
                self.files
                    .iter()
                    .map(|f| cloud::status(f.remotecache.as_ref(), &f.sha1)),
            ),
        }
    }

    fn same_files(&self, files: &[Hashed]) -> bool {
        self.files.len() == files.len()
            && self
                .files
                .iter()
                .zip(files)
                .all(|(a, b)| a.name == b.key && a.sha256 == b.sha256)
    }
}

/// What a snapshot records besides the files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    pub reason: Reason,
    pub profile: Option<String>,
    pub session_id: Option<String>,
    pub absent: Vec<String>,
    pub exe_sha256: Option<String>,
    pub pin: Option<String>,
    pub note: Option<String>,
}

impl Meta {
    pub fn new(reason: Reason) -> Self {
        Self {
            reason,
            profile: None,
            session_id: None,
            absent: Vec::new(),
            exe_sha256: None,
            pin: None,
            note: None,
        }
    }

    pub fn profile(mut self, p: impl Into<String>) -> Self {
        self.profile = Some(p.into());
        self
    }

    pub fn pin(mut self, why: impl Into<String>) -> Self {
        self.pin = Some(why.into());
        self
    }

    pub fn note(mut self, n: impl Into<String>) -> Self {
        self.note = Some(n.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Taken {
    New(Snapshot),
    /// The save matches the newest snapshot; only an event was written.
    Unchanged(Snapshot),
    NoSave,
}

impl Taken {
    pub fn snapshot(&self) -> Option<&Snapshot> {
        match self {
            Self::New(s) | Self::Unchanged(s) => Some(s),
            Self::NoSave => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventKind {
    Snapshot,
    Unchanged,
    SessionStart,
    SessionEnd,
    ChangedOutside,
    Shrank,
    Pinned,
    Unpinned,
    CareersMissing,
    Pruned,
    Restored,
    RestoreRefused,
    Migrated,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub at: i64,
    pub kind: EventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Event {
    pub fn new(at: i64, kind: EventKind) -> Self {
        Self {
            at,
            kind,
            snapshot: None,
            reason: None,
            from: None,
            before: None,
            after: None,
            detail: None,
        }
    }

    fn snapshot(mut self, id: &str) -> Self {
        self.snapshot = Some(id.to_owned());
        self
    }

    fn detail(mut self, d: impl Into<String>) -> Self {
        self.detail = Some(d.into());
        self
    }
}

/// A save smaller or changed since a snapshot the vault holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub from: Snapshot,
    pub to: Snapshot,
}

impl Change {
    pub fn shrank(&self) -> bool {
        self.to.total() + SHRINK_HINT <= self.from.total()
    }
}

/// What a check found besides the snapshot it took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub taken: Taken,
    pub outside: Option<Change>,
    pub shrank: Option<Change>,
    pub notice: Option<Notice>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoticeKind {
    ChangedOutside,
    Shrank,
}

/// The Addons page notice: a weak hint, never a claim that careers are gone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    pub kind: NoticeKind,
    pub source: String,
    pub snapshot: String,
    pub kept_at: i64,
    pub before: u64,
    pub after: u64,
    #[serde(default)]
    pub absent: Vec<String>,
    pub at: i64,
}

impl Notice {
    /// The notice's first sentence: what changed and which version the
    /// vault kept.
    pub fn headline(&self) -> String {
        let when = human(self.kept_at);
        let smaller = self.before.saturating_sub(self.after).div_ceil(1024);
        match self.kind {
            NoticeKind::ChangedOutside => format!(
                "Your F1 25 save changed outside a launcher session (a Steam or EA app launch) and is {smaller} KB \
                 smaller than before. The career vault kept the version from {when}"
            ),
            NoticeKind::Shrank => format!(
                "Your F1 25 save got {smaller} KB smaller in a session without {}. The career vault kept the \
                 version from before that session, {when}",
                self.absent.join(", ")
            ),
        }
    }

    pub fn text(&self) -> String {
        if self.source.starts_with("launcher-") {
            return format!(
                "{}: {}. If a career is missing, restore it with `swofty saves restore {} --source {}` while F1 25 is \
                 closed",
                self.headline(),
                self.snapshot,
                self.snapshot,
                self.source
            );
        }
        format!(
            "{}: {}. If a career is missing, restore it with `swofty saves restore {}` while F1 25 and {} are \
             closed",
            self.headline(),
            self.snapshot,
            self.snapshot,
            match self.source.starts_with("ea-") {
                true => "the EA app",
                false => "Steam",
            }
        )
    }

    pub fn read(data: &Path) -> Option<Self> {
        serde_json::from_slice(&std::fs::read(data.join(NOTICE)).ok()?).ok()
    }

    pub fn write(&self, data: &Path) -> Result<()> {
        crate::write_json(&data.join(NOTICE), self)
    }

    pub fn clear(data: &Path) {
        std::fs::remove_file(data.join(NOTICE)).ok();
    }

    pub fn path(data: &Path) -> PathBuf {
        data.join(NOTICE)
    }
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `YYYY-MM-DD HH:MM UTC`.
/// The files in a save folder, one line: name, size and modified time to the second, so a change
/// made during or after a session shows up between two listings.
pub fn folder_listing(dir: &Path) -> String {
    let Ok(read) = std::fs::read_dir(dir) else {
        return format!("{} could not be read", dir.display());
    };
    let mut files: Vec<(String, u64, i64)> = read
        .filter_map(|e| {
            let e = e.ok()?;
            let meta = e.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            let secs = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
            Some((e.file_name().to_string_lossy().into_owned(), meta.len(), secs))
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return format!("{} is empty", dir.display());
    }
    let list: Vec<String> = files
        .iter()
        .map(|(name, len, secs)| {
            let s = crate::save_backup::stamp(*secs);
            format!(
                "{name} {len} bytes {}-{}-{} {}:{}:{} UTC",
                &s[0..4],
                &s[4..6],
                &s[6..8],
                &s[9..11],
                &s[11..13],
                &s[13..15]
            )
        })
        .collect();
    format!("{}: {}", dir.display(), list.join("; "))
}

pub fn human(secs: i64) -> String {
    let s = crate::save_backup::stamp(secs);
    format!(
        "{}-{}-{} {}:{} UTC",
        &s[0..4],
        &s[4..6],
        &s[6..8],
        &s[9..11],
        &s[11..13]
    )
}

#[derive(Debug, Clone)]
struct Hashed {
    key: String,
    path: PathBuf,
    size: u64,
    sha256: String,
    sha1: String,
    modified: Option<SystemTime>,
}

fn hashes(path: &Path) -> Result<(u64, String, String)> {
    let mut f = File::open(path).map_err(io_err(path))?;
    let (mut a, mut b) = (Sha256::new(), Sha1::new());
    let mut buf = vec![0; 1 << 16];
    let mut size = 0u64;
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                a.update(&buf[..n]);
                b.update(&buf[..n]);
                size += n as u64;
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(io_err(path)(e)),
        }
    }
    Ok((size, hex(&a.finalize()), hex(&b.finalize())))
}

fn hash_all(loc: &Location) -> Result<Vec<Hashed>> {
    loc.files()?
        .into_iter()
        .map(|f| {
            let modified = std::fs::metadata(&f.path).and_then(|m| m.modified()).ok();
            let (size, sha256, sha1) = hashes(&f.path)?;
            Ok(Hashed {
                key: f.key,
                path: f.path,
                size,
                sha256,
                sha1,
                modified,
            })
        })
        .collect()
}

/// The save at `loc` now: its size and Steam Cloud status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveStatus {
    pub files: usize,
    pub total: u64,
    pub cloud: cloud::Status,
}

pub fn save_status(loc: &Location) -> Result<SaveStatus> {
    let files = hash_all(loc)?;
    let cache = loc.remotecache().map(|p| cloud::read(&p)).unwrap_or_default();
    let status = match loc.layout {
        Layout::Ea | Layout::Launcher | Layout::LauncherEa => cloud::Status::Local,
        Layout::Steam => cloud::combine(files.iter().map(|f| {
            let name = f.path.file_name().unwrap_or_default().to_string_lossy();
            cloud::status(cache.get(name.as_ref()), &f.sha1)
        })),
    };
    Ok(SaveStatus {
        files: files.len(),
        total: files.iter().map(|f| f.size).sum(),
        cloud: status,
    })
}

fn changed_during(path: &Path) -> Error {
    Error::Other(format!(
        "{} changed while it was backed up; try again with F1 25 closed",
        path.display()
    ))
}

fn valid_source(source: &str) -> bool {
    !source.is_empty()
        && source
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && !source.starts_with('.')
}

fn open_lock_file(path: &Path) -> Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(io_err(path))
}

fn lock_error(path: &Path, e: std::io::Error) -> Error {
    Error::Other(format!(
        "could not lock {} ({e}); the vault was left alone rather than written without its lock",
        path.display()
    ))
}

pub struct Vault {
    pub data: PathBuf,
    pub dir: PathBuf,
    pub source: String,
    pub policy: Policy,
}

pub struct Lock {
    _file: File,
}

impl Vault {
    pub fn root(data: &Path) -> PathBuf {
        data.join(DIR)
    }

    pub fn open(data: &Path, source: &str) -> Result<Self> {
        if !valid_source(source) {
            return Err(Error::Other(format!("{source} is not a vault source")));
        }
        let cap = crate::store::Store::new(data.to_path_buf())
            .config()
            .vault_cap_mb
            .unwrap_or(DEFAULT_CAP_MB);
        Ok(Self {
            data: data.to_path_buf(),
            dir: Self::root(data).join(source),
            source: source.to_owned(),
            policy: Policy::with_cap_mb(cap),
        })
    }

    pub fn of(data: &Path, loc: &Location) -> Result<Self> {
        Self::open(data, &loc.source)
    }

    /// Every source with a vault folder.
    pub fn sources(data: &Path) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(Self::root(data))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().to_str().map(str::to_owned))
            .filter(|s| valid_source(s))
            .collect();
        out.sort();
        out
    }

    pub fn lock(&self) -> Result<Lock> {
        std::fs::create_dir_all(&self.dir).map_err(io_err(&self.dir))?;
        let path = self.dir.join(".lock");
        let file = open_lock_file(&path)?;
        file.lock().map_err(|e| lock_error(&path, e))?;
        Ok(Lock { _file: file })
    }

    pub fn migrate_lock(data: &Path, wait: bool) -> Result<Option<Lock>> {
        let root = Self::root(data);
        std::fs::create_dir_all(&root).map_err(io_err(&root))?;
        let path = root.join(".migrate.lock");
        let file = open_lock_file(&path)?;
        if wait {
            file.lock().map_err(|e| lock_error(&path, e))?;
        } else {
            match file.try_lock() {
                Ok(()) => {}
                Err(std::fs::TryLockError::WouldBlock) => return Ok(None),
                Err(std::fs::TryLockError::Error(e)) => return Err(lock_error(&path, e)),
            }
        }
        Ok(Some(Lock { _file: file }))
    }

    pub fn blob(&self, sha256: &str) -> PathBuf {
        self.dir.join("blobs").join(sha256)
    }

    fn snapshots_dir(&self) -> PathBuf {
        self.dir.join("snapshots")
    }

    /// Every readable snapshot, newest first.
    pub fn list(&self) -> Result<Vec<Snapshot>> {
        Ok(self.list_all().0)
    }

    fn list_all(&self) -> (Vec<Snapshot>, Vec<String>) {
        let Ok(rd) = std::fs::read_dir(self.snapshots_dir()) else {
            return (Vec::new(), Vec::new());
        };
        let mut out: Vec<Snapshot> = Vec::new();
        let mut unreadable = Vec::new();
        for e in rd.flatten() {
            let Some(name) = e.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(id) = name.strip_suffix(".json").map(str::to_owned) else {
                continue;
            };
            if id.starts_with('.') {
                continue;
            }
            match std::fs::read(e.path())
                .ok()
                .and_then(|d| serde_json::from_slice::<Snapshot>(&d).ok())
            {
                Some(mut s) => {
                    s.id = id;
                    s.source.clone_from(&self.source);
                    out.push(s);
                }
                None => unreadable.push(name),
            }
        }
        out.sort_by(|a, b| (b.at, &b.id).cmp(&(a.at, &a.id)));
        unreadable.sort();
        (out, unreadable)
    }

    pub fn newest(&self) -> Result<Option<Snapshot>> {
        Ok(self.list()?.into_iter().next())
    }

    pub fn find(&self, id: &str) -> Result<Snapshot> {
        self.list()?.into_iter().find(|s| s.id == id).ok_or_else(|| {
            Error::Other(format!(
                "no snapshot named {id} in {}; `swofty saves list` lists them",
                self.dir.display()
            ))
        })
    }

    /// The bytes the vault's blobs take.
    pub fn usage(&self) -> u64 {
        std::fs::read_dir(self.dir.join("blobs"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum()
    }

    pub fn events(&self) -> Vec<Event> {
        std::fs::read_to_string(self.dir.join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    pub fn log(&self, e: &Event) {
        std::fs::create_dir_all(&self.dir).ok();
        let mut line = serde_json::to_vec(e).expect("serialisable");
        line.push(b'\n');
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("events.jsonl"))
        {
            f.write_all(&line).ok();
        }
    }

    fn write_snapshot(&self, s: &Snapshot) -> Result<()> {
        crate::write_json(&self.snapshots_dir().join(format!("{}.json", s.id)), s)
    }

    fn unique_id(&self, at: i64, reason: Reason) -> String {
        let base = format!("{}-{}", crate::save_backup::stamp(at), reason.slug());
        let mut id = base.clone();
        let mut n = 2;
        while self.snapshots_dir().join(format!("{id}.json")).exists() {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }

    fn store_blob(&self, src: &Path, sha256: &str) -> Result<()> {
        let blob = self.blob(sha256);
        if blob.is_file() {
            return Ok(());
        }
        let dir = self.dir.join("blobs");
        std::fs::create_dir_all(&dir).map_err(io_err(&dir))?;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = dir.join(format!(".{sha256}.{}-{n}.part", std::process::id()));
        std::fs::copy(src, &tmp).map_err(io_err(src))?;
        if sha256_file(&tmp)? != sha256 {
            std::fs::remove_file(&tmp).ok();
            return Err(changed_during(src));
        }
        if let Err(e) = crate::sync_file(&tmp) {
            std::fs::remove_file(&tmp).ok();
            return Err(e);
        }
        match std::fs::rename(&tmp, &blob) {
            Ok(()) => {
                crate::sync_dir(Some(&dir));
                Ok(())
            }
            Err(_) if blob.is_file() => {
                std::fs::remove_file(&tmp).ok();
                Ok(())
            }
            Err(e) => {
                std::fs::remove_file(&tmp).ok();
                Err(io_err(&blob)(e))
            }
        }
    }

    /// Snapshot the save at `loc`. When it matches the newest snapshot, only
    /// an event is written.
    pub fn snapshot(&self, loc: &Location, meta: Meta, now: i64) -> Result<Taken> {
        let _lock = self.lock()?;
        let taken = self.snapshot_locked(loc, meta, now)?;
        self.prune_locked(now)?;
        Ok(taken)
    }

    pub(crate) fn snapshot_locked(&self, loc: &Location, meta: Meta, now: i64) -> Result<Taken> {
        if loc.source != self.source {
            return Err(Error::Other(format!(
                "the save at {} belongs to {}, not {}",
                loc.dir.display(),
                loc.source,
                self.source
            )));
        }
        let files = hash_all(loc)?;
        if files.is_empty() {
            return Ok(Taken::NoSave);
        }
        if let Some(newest) = self.newest()?
            && newest.same_files(&files)
        {
            let mut e = Event::new(now, EventKind::Unchanged).snapshot(&newest.id);
            e.reason = Some(meta.reason);
            self.log(&e);
            let newest = match meta.pin {
                Some(why) => self.pin_locked(&newest.id, &why, now)?,
                None => newest,
            };
            self.observe_careers(&newest.id, now);
            return Ok(Taken::Unchanged(newest));
        }
        let cache = loc.remotecache().map(|p| cloud::read(&p)).unwrap_or_default();
        let mut entries = Vec::new();
        for f in &files {
            self.store_blob(&f.path, &f.sha256)?;
            let file = f
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            entries.push(FileEntry {
                name: f.key.clone(),
                remotecache: cache.get(&file).cloned(),
                file,
                size: f.size,
                sha256: f.sha256.clone(),
                sha1: f.sha1.clone(),
            });
        }
        for f in &files {
            let meta = std::fs::metadata(&f.path).map_err(io_err(&f.path))?;
            if meta.len() != f.size || meta.modified().ok() != f.modified {
                return Err(changed_during(&f.path));
            }
        }
        let careers_ref = self.store_index();
        let s = Snapshot {
            id: self.unique_id(now, meta.reason),
            created: crate::save_backup::stamp(now),
            at: now,
            reason: meta.reason,
            session_id: meta.session_id,
            profile: meta.profile,
            launcher: env!("CARGO_PKG_VERSION").to_owned(),
            exe_sha256: meta.exe_sha256,
            source: self.source.clone(),
            layout: loc.layout,
            save_dir: loc.dir.clone(),
            files: entries,
            absent_addons: meta.absent,
            careers_ref,
            pinned: meta.pin.into_iter().collect(),
            note: meta.note,
        };
        self.write_snapshot(&s)?;
        let mut e = Event::new(now, EventKind::Snapshot).snapshot(&s.id);
        e.reason = Some(s.reason);
        e.after = Some(s.total());
        self.log(&e);
        self.observe_careers(&s.id, now);
        Ok(Taken::New(s))
    }

    pub fn pin(&self, id: &str, why: &str, now: i64) -> Result<Snapshot> {
        let _lock = self.lock()?;
        self.pin_locked(id, why, now)
    }

    fn pin_locked(&self, id: &str, why: &str, now: i64) -> Result<Snapshot> {
        let mut s = self.find(id)?;
        if !s.pinned.iter().any(|p| p == why) {
            s.pinned.push(why.to_owned());
            self.write_snapshot(&s)?;
            self.log(&Event::new(now, EventKind::Pinned).snapshot(id).detail(why));
        }
        Ok(s)
    }

    pub fn unpin_why(&self, id: &str, why: &str, now: i64) -> Result<Snapshot> {
        let _lock = self.lock()?;
        let mut s = self.find(id)?;
        if s.pinned.iter().any(|p| p == why) {
            s.pinned.retain(|p| p != why);
            self.write_snapshot(&s)?;
            self.log(&Event::new(now, EventKind::Unpinned).snapshot(id).detail(why));
        }
        Ok(s)
    }

    pub fn unpin(&self, id: &str) -> Result<Snapshot> {
        let _lock = self.lock()?;
        let mut s = self.find(id)?;
        s.pinned.clear();
        self.write_snapshot(&s)?;
        Ok(s)
    }

    /// Apply retention and the cap, then delete blobs no snapshot uses.
    pub fn prune(&self, now: i64) -> Result<Vec<String>> {
        let _lock = self.lock()?;
        self.prune_locked(now)
    }

    fn prune_locked(&self, now: i64) -> Result<Vec<String>> {
        let (snaps, unreadable) = self.list_all();
        if !unreadable.is_empty() {
            self.log(&Event::new(now, EventKind::Other).detail(format!(
                "prune skipped: could not read {}; their blobs stay",
                unreadable.join(", ")
            )));
            return Ok(Vec::new());
        }
        let items: Vec<retention::Item> = snaps
            .iter()
            .map(|s| retention::Item {
                id: s.id.clone(),
                at: s.at,
                pinned: !s.pinned.is_empty(),
                blobs: s.files.iter().map(|f| (f.sha256.clone(), f.size)).collect(),
            })
            .collect();
        let gone = retention::plan(&items, now, &self.policy);
        let left = retention::bytes(items.iter().filter(|i| !gone.contains(&i.id)));
        if left > self.policy.cap_bytes {
            let detail = format!(
                "over the {} byte cap by {} bytes: the rest are pinned or the newest, so they stay",
                self.policy.cap_bytes,
                left - self.policy.cap_bytes
            );
            if self.events().last().and_then(|e| e.detail.as_deref()) != Some(detail.as_str()) {
                self.log(&Event::new(now, EventKind::Other).detail(detail));
            }
        }
        for id in &gone {
            let path = self.snapshots_dir().join(format!("{id}.json"));
            std::fs::remove_file(&path).map_err(io_err(&path))?;
            let mut e = Event::new(now, EventKind::Pruned).snapshot(id);
            e.reason = snaps.iter().find(|s| &s.id == id).map(|s| s.reason);
            self.log(&e);
        }
        let used: BTreeSet<String> = snaps
            .iter()
            .filter(|s| !gone.contains(&s.id))
            .flat_map(|s| s.files.iter().map(|f| f.sha256.clone()))
            .collect();
        for e in std::fs::read_dir(self.dir.join("blobs"))
            .into_iter()
            .flatten()
            .flatten()
        {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.starts_with('.') && !used.contains(&name) {
                std::fs::remove_file(e.path()).ok();
            }
        }
        let kept_refs: BTreeSet<String> = snaps
            .iter()
            .filter(|s| !gone.contains(&s.id))
            .filter_map(|s| s.careers_ref.clone())
            .collect();
        for e in std::fs::read_dir(self.dir.join(careers::INDEX_DIR))
            .into_iter()
            .flatten()
            .flatten()
        {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.starts_with('.') && !kept_refs.contains(&format!("{}/{name}", careers::INDEX_DIR)) {
                std::fs::remove_file(e.path()).ok();
            }
        }
        if !gone.is_empty() {
            migrate::retire(&self.data);
        }
        Ok(gone)
    }

    /// Whether the last launcher session started and never ended: its
    /// rollback-time snapshot is missing, so a change since is its own.
    fn session_unfinished(&self) -> bool {
        self.events()
            .iter()
            .rev()
            .find(|e| matches!(e.kind, EventKind::SessionStart | EventKind::SessionEnd))
            .is_some_and(|e| e.kind == EventKind::SessionStart)
    }

    /// Snapshot at launcher start, before a session or after a foreign
    /// launch, and report a save changed outside the launcher since the
    /// newest snapshot.
    pub fn check(&self, loc: &Location, meta: Meta, now: i64) -> Result<Checked> {
        let _lock = self.lock()?;
        let prev = self.newest()?;
        let trigger = meta.reason;
        let unfinished = self.session_unfinished();
        let reason = match (&prev, unfinished) {
            (None, _) => trigger,
            (Some(_), true) => Reason::AfterSession,
            (Some(_), false) => Reason::ChangedOutside,
        };
        let mut m = meta.clone();
        m.reason = reason;
        if reason != trigger {
            m.note = Some(match unfinished {
                true => format!(
                    "{}: the session before it ended without its after-session snapshot",
                    trigger.found()
                ),
                false => trigger.found().to_owned(),
            });
        }
        if unfinished {
            self.log(&Event::new(now, EventKind::SessionEnd).detail("unfinished"));
        }
        let taken = self.snapshot_locked(loc, m, now)?;
        let mut out = Checked {
            taken,
            outside: None,
            shrank: None,
            notice: None,
        };
        if let (Taken::New(new), Some(prev), false) = (&out.taken, prev, unfinished) {
            let prev = self.pin_locked(&prev.id, "before-outside-change", now)?;
            let change = Change {
                from: prev,
                to: new.clone(),
            };
            let mut e = Event::new(now, EventKind::ChangedOutside).snapshot(&new.id);
            e.from = Some(change.from.id.clone());
            e.before = Some(change.from.total());
            e.after = Some(new.total());
            e.detail = Some(trigger.found().to_owned());
            self.log(&e);
            if change.shrank() {
                let mut e = e.clone();
                e.kind = EventKind::Shrank;
                self.log(&e);
                let n = Notice {
                    kind: NoticeKind::ChangedOutside,
                    source: self.source.clone(),
                    snapshot: change.from.id.clone(),
                    kept_at: change.from.at,
                    before: change.from.total(),
                    after: new.total(),
                    absent: Vec::new(),
                    at: now,
                };
                n.write(&self.data)?;
                out.notice = Some(n);
                out.shrank = Some(change.clone());
            }
            out.outside = Some(change);
        }
        self.prune_locked(now)?;
        Ok(out)
    }

    /// Before a launcher session: `check`, then mark the session started.
    pub fn before_session(&self, loc: &Location, meta: Meta, now: i64) -> Result<Checked> {
        let c = self.check(loc, meta.clone(), now)?;
        let mut e = Event::new(now, EventKind::SessionStart);
        e.snapshot = c.taken.snapshot().map(|s| s.id.clone());
        e.detail = meta.session_id;
        self.log(&e);
        Ok(c)
    }

    /// After a launcher session, once the game exited and the rollback ran:
    /// snapshot, and hint at a save that got smaller in a session that left
    /// out an addon affecting saves.
    pub fn after_session(
        &self,
        loc: &Location,
        meta: Meta,
        before: Option<&Snapshot>,
        now: i64,
    ) -> Result<Checked> {
        let _lock = self.lock()?;
        let absent = meta.absent.clone();
        let session = meta.session_id.clone();
        let taken = self.snapshot_locked(loc, meta, now)?;
        let mut e = Event::new(now, EventKind::SessionEnd);
        e.snapshot = taken.snapshot().map(|s| s.id.clone());
        e.detail = session;
        self.log(&e);
        let mut out = Checked {
            taken,
            outside: None,
            shrank: None,
            notice: None,
        };
        if let (Taken::New(new), Some(before)) = (&out.taken, before) {
            let change = Change {
                from: before.clone(),
                to: new.clone(),
            };
            if change.shrank() {
                let mut e = Event::new(now, EventKind::Shrank).snapshot(&new.id);
                e.from = Some(before.id.clone());
                e.before = Some(before.total());
                e.after = Some(new.total());
                self.log(&e);
                if !absent.is_empty() {
                    let n = Notice {
                        kind: NoticeKind::Shrank,
                        source: self.source.clone(),
                        snapshot: before.id.clone(),
                        kept_at: before.at,
                        before: before.total(),
                        after: new.total(),
                        absent,
                        at: now,
                    };
                    n.write(&self.data)?;
                    out.notice = Some(n);
                }
                out.shrank = Some(change);
            }
        }
        self.prune_locked(now)?;
        Ok(out)
    }

    /// The core's career index as it is now, stored beside the snapshot that
    /// is about to be written. `None` before the core has written one.
    fn store_index(&self) -> Option<String> {
        let list = crate::stdlib::careers::read(&crate::stdlib::careers::data_path(&self.data));
        if list.is_empty() {
            return None;
        }
        let value = serde_json::json!({ "version": 1, "careers": list });
        let sha = hex(&Sha256::digest(serde_json::to_vec(&value).ok()?));
        let rel = format!("{}/{sha}.json", careers::INDEX_DIR);
        let path = self.dir.join(&rel);
        if !path.is_file() {
            crate::write_json(&path, &value).ok()?;
        }
        Some(rel)
    }

    /// The career index stored with a snapshot; `None` when it has none.
    pub fn index_of(&self, s: &Snapshot) -> Option<Vec<crate::stdlib::careers::Career>> {
        let rel = s.careers_ref.as_deref()?;
        if rel.contains("..") || rel.starts_with('/') {
            return None;
        }
        Some(crate::stdlib::careers::read(&self.dir.join(rel)))
    }

    pub fn careers(&self) -> careers::Careers {
        careers::Careers::read(&self.dir)
    }

    /// Fold the core's career index as it is now into `careers.json`, as
    /// seen at snapshot `id`. A career that was in the last index and isn't
    /// in this one is `missing-since:<id>`, and the snapshot that last held
    /// it is pinned so retention keeps it.
    fn observe_careers(&self, id: &str, now: i64) {
        let index = crate::stdlib::careers::read(&crate::stdlib::careers::data_path(&self.data));
        if index.is_empty() {
            return;
        }
        let mut table = self.careers();
        let before = table.clone();
        let dropped = table.observe(id, now, &index);
        for (career, held) in &dropped {
            self.pin_locked(held, "holds-missing-career", now).ok();
            self.log(
                &Event::new(now, EventKind::CareersMissing)
                    .snapshot(id)
                    .detail(format!("{career} was last in {held}")),
            );
        }
        if table != before {
            table.write(&self.dir).ok();
        }
    }

    pub fn dismiss_drop(&self) -> Result<()> {
        let _lock = self.lock()?;
        let mut table = self.careers();
        if table.dismiss() {
            table.write(&self.dir)?;
        }
        Ok(())
    }

    pub fn archive_career(&self, id: &str) -> Result<bool> {
        let _lock = self.lock()?;
        let mut table = self.careers();
        let found = table.archive(id);
        if found {
            table.write(&self.dir)?;
        }
        Ok(found)
    }

    /// What restoring `s` does to the careers: `None` when the vault has no
    /// index for it or none for the save as it is.
    pub fn preview(&self, s: &Snapshot) -> Option<careers::Diff> {
        let version = self.index_of(s)?;
        let now = crate::stdlib::careers::read(&crate::stdlib::careers::data_path(&self.data));
        (!now.is_empty()).then(|| careers::diff(&now, &version))
    }

    /// Copy a migrated `save-backups/` backup in as a snapshot.
    fn import(&self, b: &crate::save_backup::Backup, loc: &Location) -> Result<String> {
        let _lock = self.lock()?;
        let mut entries = Vec::new();
        for f in &b.manifest.files {
            let src = b.dir.join(&f.name);
            let (size, sha256, sha1) = hashes(&src)?;
            if sha256 != f.sha256 || size != f.size {
                return Err(Error::Other(format!(
                    "backup {} is damaged: {} does not match its manifest",
                    b.name, f.name
                )));
            }
            let Some(key) = loc.key_of(&f.name) else {
                continue;
            };
            self.store_blob(&src, &sha256)?;
            entries.push(FileEntry {
                name: key,
                file: f.name.clone(),
                size,
                sha256,
                sha1,
                remotecache: None,
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let at = crate::save_backup::parse_stamp(&b.manifest.created).unwrap_or_else(now);
        let s = Snapshot {
            id: self.unique_id(at, Reason::Migrated),
            created: crate::save_backup::stamp(at),
            at,
            reason: Reason::Migrated,
            session_id: None,
            profile: Some(b.manifest.profile.clone()),
            launcher: env!("CARGO_PKG_VERSION").to_owned(),
            exe_sha256: None,
            source: self.source.clone(),
            layout: loc.layout,
            save_dir: b.manifest.save_dir.clone(),
            files: entries,
            absent_addons: b.manifest.absent.clone(),
            careers_ref: None,
            pinned: vec!["migrated".to_owned()],
            note: Some(format!("save-backups/{}: {}", b.name, b.manifest.reason)),
        };
        self.write_snapshot(&s)?;
        self.log(
            &Event::new(now(), EventKind::Migrated)
                .snapshot(&s.id)
                .detail(format!("save-backups/{}", b.name)),
        );
        Ok(s.id)
    }
}

#[cfg(test)]
mod tests;
