//! Copying a data folder: resumable and cancellable, with a size and content
//! check of what was written.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};

const CHUNK: usize = 1 << 20;
const HASHED_UP_TO: u64 = 1 << 20;
const CLOCK_SLACK: Duration = Duration::from_secs(2);
const LOCKED_TRIES: u32 = 5;
const LOCKED_PAUSE: Duration = Duration::from_secs(1);
const SHARING_VIOLATION: i32 = 32;
const LOCK_VIOLATION: i32 = 33;

#[derive(Debug, PartialEq)]
pub enum Entry {
    Dir,
    File { len: u64, modified: Option<SystemTime> },
    Link(PathBuf),
}

pub type Listing = BTreeMap<PathBuf, Entry>;

/// What a copy or sync stopped on.
#[derive(Debug)]
pub enum Stop {
    Cancelled,
    Io(String),
}

impl From<io::Error> for Stop {
    fn from(e: io::Error) -> Self {
        Stop::Io(e.to_string())
    }
}

pub fn listing(root: &Path) -> io::Result<Listing> {
    fn walk(path: &Path, rel: PathBuf, out: &mut Listing) -> io::Result<()> {
        if webview_profile(&rel) {
            return Ok(());
        }
        let meta = std::fs::symlink_metadata(path)?;
        let entry = if meta.file_type().is_symlink() {
            Entry::Link(std::fs::read_link(path)?)
        } else if meta.is_dir() {
            for child in std::fs::read_dir(path)? {
                let child = child?;
                walk(&child.path(), rel.join(child.file_name()), out)?;
            }
            Entry::Dir
        } else if lock_file(&rel) {
            return Ok(());
        } else {
            Entry::File {
                len: meta.len(),
                modified: meta.modified().ok(),
            }
        };
        out.insert(rel, entry);
        Ok(())
    }
    let mut out = Listing::new();
    walk(root, PathBuf::new(), &mut out)?;
    Ok(out)
}

pub fn totals(list: &Listing) -> (u64, usize) {
    list.values().fold((0, 0), |(bytes, files), e| match e {
        Entry::File { len, .. } => (bytes + len, files + 1),
        _ => (bytes, files),
    })
}

fn same_stamp(a: Option<SystemTime>, b: Option<SystemTime>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a
            .duration_since(b)
            .or_else(|_| b.duration_since(a))
            .is_ok_and(|d| d <= CLOCK_SLACK),
        _ => false,
    }
}

/// Makes `dst` match `src`: copies the files that are missing or differ in
/// size or time, and removes what `src` no longer has. Safe to run again after
/// it stopped half way. `seen` gets each file's path and bytes as they are
/// written.
pub fn sync(
    src: &Path,
    dst: &Path,
    cancel: &AtomicBool,
    seen: &mut dyn FnMut(&Path, u64),
) -> Result<(), Stop> {
    let from = listing(src)?;
    std::fs::create_dir_all(dst)?;
    let have = listing(dst)?;
    for (rel, entry) in have.iter().rev() {
        let keep = match (from.get(rel), entry) {
            (Some(Entry::Dir), Entry::Dir) => true,
            (Some(Entry::File { .. }), Entry::File { .. }) => true,
            (Some(Entry::Link(a)), Entry::Link(b)) => a == b,
            _ => false,
        };
        if !keep && !rel.as_os_str().is_empty() {
            remove(&dst.join(rel))?;
        }
    }
    for (rel, entry) in &from {
        if cancel.load(Ordering::Relaxed) {
            return Err(Stop::Cancelled);
        }
        let (from, to) = (src.join(rel), dst.join(rel));
        match entry {
            Entry::Dir => std::fs::create_dir_all(&to)?,
            Entry::Link(_) => {
                if to.symlink_metadata().is_err() {
                    copy_link(&from, &to)?;
                }
            }
            Entry::File { len, modified } => {
                let current = matches!(
                    have.get(rel),
                    Some(Entry::File { len: l, modified: m }) if l == len && same_stamp(*m, *modified)
                );
                if !current {
                    copy_file(&from, &to, cancel, &mut |n| seen(rel, n))?;
                }
            }
        }
    }
    Ok(())
}

pub fn webview_profile(rel: &Path) -> bool {
    rel.starts_with(Path::new("ads").join("EBWebView"))
}

pub fn lock_file(rel: &Path) -> bool {
    let Some(name) = rel.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    matches!(name, ".lock" | ".migrate.lock" | ".update.lock")
        || (name.starts_with(".swap-") && name.ends_with(".lock"))
        || rel == Path::new("state").join("session.lock")
}

pub fn locked(code: Option<i32>) -> bool {
    matches!(code, Some(SHARING_VIOLATION | LOCK_VIOLATION))
}

fn retrying<T>(tries: u32, pause: Duration, mut attempt: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    let mut left = tries;
    loop {
        match attempt() {
            Err(e) if left > 1 && locked(e.raw_os_error()) => {
                left -= 1;
                sleep(pause);
            }
            other => return other,
        }
    }
}

fn copy_file(src: &Path, dst: &Path, cancel: &AtomicBool, wrote: &mut dyn FnMut(u64)) -> Result<(), Stop> {
    let finished = retrying(LOCKED_TRIES, LOCKED_PAUSE, || copy_once(src, dst, cancel, wrote))
        .map_err(|e| Stop::Io(format!("{}: {e}", src.display())))?;
    if finished { Ok(()) } else { Err(Stop::Cancelled) }
}

fn copy_once(src: &Path, dst: &Path, cancel: &AtomicBool, wrote: &mut dyn FnMut(u64)) -> io::Result<bool> {
    let mut input = File::open(src)?;
    let modified = input.metadata()?.modified().ok();
    let mut output = File::create(dst)?;
    let mut buffer = vec![0u8; CHUNK];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        output.write_all(&buffer[..n])?;
        wrote(n as u64);
    }
    output.flush()?;
    if let Some(modified) = modified {
        output.set_modified(modified)?;
    }
    Ok(true)
}

#[cfg(not(windows))]
fn copy_link(src: &Path, dst: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(std::fs::read_link(src)?, dst)
}

#[cfg(windows)]
fn copy_link(src: &Path, dst: &Path) -> io::Result<()> {
    let target = std::fs::read_link(src)?;
    if std::fs::metadata(src).is_ok_and(|m| m.is_dir()) {
        super::link::create(&target, dst)
    } else {
        std::os::windows::fs::symlink_file(&target, dst)
    }
}

/// Whether the launcher appends to `rel` while it runs: its own logs under
/// `state/` and the kept Send Logs under `logs/`. These grow during a copy
/// made from a running launcher, so the copy is checked as a prefix of the
/// source; the sync at the next start brings them up to date.
pub fn live(rel: &Path) -> bool {
    let mut parts = rel.components().map(|c| c.as_os_str());
    match parts.next() {
        Some(first) if first == "logs" => parts.next().is_some(),
        Some(first) if first == "state" => {
            parts.next().is_some() && rel.extension().is_some_and(|e| e == "log")
        }
        _ => false,
    }
}

/// Checks that `dst` holds what `src` holds: the same entries and sizes, and
/// the same bytes in every file up to a megabyte. A [`live`] log may have
/// grown since it was copied: its copy must be a prefix of the source.
pub fn verify(src: &Path, dst: &Path) -> Result<(), String> {
    let (a, b) = (
        listing(src).map_err(|e| format!("could not read {}: {e}", src.display()))?,
        listing(dst).map_err(|e| format!("could not read {}: {e}", dst.display()))?,
    );
    if a.len() != b.len() {
        return Err(format!("{} entries were copied of {}", b.len(), a.len()));
    }
    for (rel, entry) in &a {
        match (entry, b.get(rel)) {
            (Entry::Dir, Some(Entry::Dir)) => {}
            (Entry::Link(x), Some(Entry::Link(y))) if x == y => {}
            (Entry::File { len: x, .. }, Some(Entry::File { len: y, .. })) if live(rel) => {
                let prefix = y <= x
                    && (*y > HASHED_UP_TO || digest_up_to(&src.join(rel), *y) == digest(&dst.join(rel)));
                if !prefix {
                    return Err(format!("{} changed during the copy", rel.display()));
                }
            }
            (Entry::File { len: x, .. }, Some(Entry::File { len: y, .. })) if x == y => {
                if *x <= HASHED_UP_TO && digest(&src.join(rel)) != digest(&dst.join(rel)) {
                    return Err(format!("{} differs after the copy", rel.display()));
                }
            }
            _ => return Err(format!("{} does not match after the copy", rel.display())),
        }
    }
    Ok(())
}

fn digest(path: &Path) -> Option<Vec<u8>> {
    Some(Sha256::digest(std::fs::read(path).ok()?).to_vec())
}

fn digest_up_to(path: &Path, len: u64) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(len as usize);
    File::open(path).ok()?.take(len).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 == len).then(|| Sha256::digest(&bytes).to_vec())
}

/// Deletes a file, link or folder; a link is unlinked, never followed.
pub fn remove(path: &Path) -> io::Result<()> {
    let meta = path.symlink_metadata()?;
    if meta.is_dir() && !meta.file_type().is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        super::link::remove(path).or_else(|_| crate::remove_placed(path))
    }
}

/// Deletes what is inside `dir` and keeps `dir`.
pub fn empty(dir: &Path) -> io::Result<()> {
    for child in std::fs::read_dir(dir)? {
        remove(&child?.path())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn the_launchers_own_lock_files_are_not_copied() {
        for rel in [
            "state/session.lock",
            "vault/.lock",
            "vault/save-backups/.lock",
            ".migrate.lock",
            "vault/.migrate.lock",
            ".update.lock",
            "addons/.swap-abc.lock",
        ] {
            assert!(lock_file(Path::new(rel)), "{rel}");
        }
        for rel in [
            "state/session.json",
            "session.lock",
            "addons/a/.lock.txt",
            "addons/swap.lock",
            "logs/x.log",
        ] {
            assert!(!lock_file(Path::new(rel)), "{rel}");
        }
    }

    #[test]
    fn listing_leaves_lock_files_out() {
        let dir = std::env::temp_dir().join(format!("swofty-lock-listing-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("state")).unwrap();
        std::fs::write(dir.join("state/session.lock"), "").unwrap();
        std::fs::write(dir.join("state/session.json"), "{}").unwrap();
        let list = listing(&dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert!(list.contains_key(Path::new("state/session.json")));
        assert!(!list.contains_key(Path::new("state/session.lock")));
    }

    #[test]
    fn the_ads_webview_profile_is_not_copied_but_other_ads_files_are() {
        assert!(webview_profile(Path::new("ads/EBWebView")));
        assert!(webview_profile(Path::new("ads/EBWebView/Default/Cookies")));
        assert!(!webview_profile(Path::new("ads/other.json")));
        assert!(!webview_profile(Path::new("ads/EBWebViewX/a")));
        assert!(!webview_profile(Path::new("EBWebView/a")));
        let dir = std::env::temp_dir().join(format!("swofty-webview-listing-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(dir.join("ads/EBWebView/Default")).unwrap();
        std::fs::write(dir.join("ads/EBWebView/Default/Cookies"), "x").unwrap();
        std::fs::write(dir.join("ads/other.json"), "{}").unwrap();
        let list = listing(&dir).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert!(list.contains_key(Path::new("ads/other.json")));
        assert!(list.contains_key(Path::new("ads")));
        assert!(!list.keys().any(|k| k.starts_with("ads/EBWebView")));
    }

    #[test]
    fn only_sharing_and_lock_violations_count_as_locked() {
        assert!(locked(Some(32)));
        assert!(locked(Some(33)));
        assert!(!locked(Some(2)));
        assert!(!locked(None));
    }

    fn violation(code: i32) -> io::Error {
        io::Error::from_raw_os_error(code)
    }

    #[test]
    fn a_locked_file_is_retried_until_it_opens() {
        let calls = Cell::new(0);
        let got = retrying(5, Duration::ZERO, || {
            calls.set(calls.get() + 1);
            if calls.get() < 4 {
                Err(violation(33))
            } else {
                Ok(7)
            }
        });
        assert_eq!(got.unwrap(), 7);
        assert_eq!(calls.get(), 4);
    }

    #[test]
    fn a_file_that_stays_locked_fails_after_the_last_try() {
        let calls = Cell::new(0);
        let got: io::Result<()> = retrying(5, Duration::ZERO, || {
            calls.set(calls.get() + 1);
            Err(violation(32))
        });
        assert_eq!(got.unwrap_err().raw_os_error(), Some(32));
        assert_eq!(calls.get(), 5);
    }

    #[test]
    fn other_errors_are_not_retried() {
        let calls = Cell::new(0);
        let got: io::Result<()> = retrying(5, Duration::ZERO, || {
            calls.set(calls.get() + 1);
            Err(violation(2))
        });
        assert!(got.is_err());
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn a_failed_copy_names_the_file() {
        let dir = std::env::temp_dir().join(format!("swofty-lock-name-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("gone.bin");
        let cancel = AtomicBool::new(false);
        let err = copy_file(&missing, &dir.join("out.bin"), &cancel, &mut |_| {}).unwrap_err();
        std::fs::remove_dir_all(&dir).ok();
        match err {
            Stop::Io(text) => assert!(text.contains("gone.bin"), "{text}"),
            Stop::Cancelled => panic!("cancelled"),
        }
    }
}
