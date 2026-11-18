//! A copy of the built overlay on the game's own drive. Hard links cannot
//! cross drives, so when the data folder is on another drive Play links the
//! game folder's files to this copy instead of copying every file again. The
//! copy is kept between sessions and brought up to date before each install.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The folder in the game folder that holds the cache. Not inside the loader
/// folder, which a restore deletes.
pub const DIR: &str = "swofty-cache";
const OVERLAY: &str = "overlay";
const INDEX: &str = "index.json";
/// Room left on the game's drive after the cache is filled.
pub const MARGIN: u64 = 2 << 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct Stamp {
    len: u64,
    modified: u128,
}

/// What a sync did, and where each overlay file is in the cache.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Synced {
    pub copied: usize,
    pub reused: usize,
    pub removed: usize,
    pub files: BTreeMap<String, PathBuf>,
}

pub fn dir(game: &Path) -> PathBuf {
    game.join(DIR)
}

pub fn root(game: &Path) -> PathBuf {
    dir(game).join(OVERLAY)
}

fn index_path(game: &Path) -> PathBuf {
    dir(game).join(INDEX)
}

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some(Stamp {
        len: meta.len(),
        modified,
    })
}

/// A cached file: the source it was copied from, that source's stamp, and the
/// copy as written. A copy the game wrote through its link no longer matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Kept {
    from: PathBuf,
    source: Stamp,
    copy: Stamp,
}

fn load(game: &Path) -> BTreeMap<String, Kept> {
    std::fs::read(index_path(game))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(game: &Path, index: &BTreeMap<String, Kept>) -> std::io::Result<()> {
    let path = index_path(game);
    std::fs::create_dir_all(dir(game))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(index).unwrap_or_default())?;
    std::fs::rename(&tmp, &path)
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = e.path();
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn prune_empty(dir: &Path) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            prune_empty(&e.path());
            std::fs::remove_dir(e.path()).ok();
        }
    }
}

/// Brings the cache in `game` up to date with `sources`, each a game path and
/// the file that goes there: copies the files that are new, come from another
/// source, or whose size or modified time changed, leaves the rest, and deletes
/// what the overlay no longer has. Fails
/// with the reason when the game's drive hasn't room for what is left to copy
/// plus `MARGIN`, or a copy fails; the cache then holds what was copied.
pub fn sync(
    game: &Path,
    sources: &[(&str, &Path)],
    free_space: fn(&Path) -> Option<u64>,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<Synced, String> {
    let cache = root(game);
    let wanted: BTreeMap<&str, &Path> = sources.iter().copied().collect();
    let mut out = Synced::default();
    let mut present = Vec::new();
    files(&cache, &mut present);
    for path in present {
        let known = path
            .strip_prefix(&cache)
            .is_ok_and(|r| wanted.contains_key(r.to_string_lossy().replace('\\', "/").as_str()));
        if !known && std::fs::remove_file(&path).is_ok() {
            out.removed += 1;
        }
    }
    prune_empty(&cache);
    let mut index = load(game);
    index.retain(|k, _| wanted.contains_key(k.as_str()));
    let mut todo = Vec::new();
    for (k, src) in &wanted {
        let now = stamp(src).ok_or_else(|| format!("cannot read {}", src.display()))?;
        let at = cache.join(k);
        let kept = index
            .get(*k)
            .is_some_and(|i| i.from == *src && i.source == now && stamp(&at) == Some(i.copy));
        if kept {
            out.reused += 1;
            out.files.insert((*k).to_owned(), at);
        } else {
            index.remove(*k);
            todo.push((*k, *src, now));
        }
    }
    let need: u64 = todo.iter().map(|(_, _, s)| s.len).sum();
    if let Some(free) = free_space(game)
        && free < need.saturating_add(MARGIN)
    {
        save(game, &index).ok();
        return Err(format!(
            "the game drive has {} MB free and the cache needs {} MB more, plus {} MB to spare",
            free >> 20,
            need >> 20,
            MARGIN >> 20
        ));
    }
    let total = todo.len();
    for (n, (k, src, now)) in todo.into_iter().enumerate() {
        let at = cache.join(k);
        let made = at
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| crate::install::copy_replacing(src, &at));
        let copy = stamp(&at);
        let Some(copy) = copy.filter(|_| made.is_ok()) else {
            save(game, &index).ok();
            let why = made.err().map_or("it is missing".to_owned(), |e| e.to_string());
            return Err(format!("cannot copy {}: {why}", src.display()));
        };
        index.insert(
            k.to_owned(),
            Kept {
                from: src.to_path_buf(),
                source: now,
                copy,
            },
        );
        out.files.insert(k.to_owned(), at);
        out.copied += 1;
        progress(n + 1, total);
    }
    save(game, &index).map_err(|e| format!("cannot write the cache index: {e}"))?;
    Ok(out)
}

pub fn size(game: &Path) -> u64 {
    crate::data_usage::dir_size(&dir(game))
}

/// Deletes the cache; the bytes it held.
pub fn remove(game: &Path) -> u64 {
    let held = size(game);
    let path = dir(game);
    if path.exists() && std::fs::remove_dir_all(&path).is_err() {
        let mut left = Vec::new();
        files(&path, &mut left);
        for f in left {
            crate::clear_readonly(&f);
            std::fs::remove_file(&f).ok();
        }
        prune_empty(&path);
        std::fs::remove_dir(&path).ok();
    }
    held.saturating_sub(size(game))
}

/// Whether `a` and `b` are on different volumes, when both are known.
pub fn apart(a: Option<crate::disk::Volume>, b: Option<crate::disk::Volume>) -> Option<bool> {
    Some(a?.id != b?.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("swofty-linkcache-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn roomy(_: &Path) -> Option<u64> {
        Some(1 << 40)
    }

    fn run(built: &Path, game: &Path, rels: &[&str]) -> Result<Synced, String> {
        let srcs: Vec<PathBuf> = rels.iter().map(|r| built.join(r)).collect();
        let refs: Vec<(&str, &Path)> = rels
            .iter()
            .copied()
            .zip(srcs.iter().map(|p| p.as_path()))
            .collect();
        sync(game, &refs, roomy, &mut |_, _| {})
    }

    #[test]
    fn new_files_are_copied_and_unchanged_ones_reused() {
        let root = tmp("new");
        let (built, game) = (root.join("data/cache/overlay/p"), root.join("game"));
        put(&built.join("a.bin"), "one");
        put(&built.join("sub/b.bin"), "two");
        let first = run(&built, &game, &["a.bin", "sub/b.bin"]).unwrap();
        assert_eq!((first.copied, first.reused, first.removed), (2, 0, 0));
        let again = run(&built, &game, &["a.bin", "sub/b.bin"]).unwrap();
        assert_eq!((again.copied, again.reused), (0, 2));
        let at = &again.files["sub/b.bin"];
        assert_eq!(at, &root_of(&game).join("sub/b.bin"));
        assert_eq!(std::fs::read_to_string(at).unwrap(), "two");
        std::fs::remove_dir_all(&root).ok();
    }

    fn root_of(game: &Path) -> PathBuf {
        super::root(game)
    }

    #[test]
    fn a_changed_file_is_copied_again() {
        let root = tmp("changed");
        let (built, game) = (root.join("built"), root.join("game"));
        put(&built.join("a.bin"), "one");
        put(&built.join("b.bin"), "keep");
        run(&built, &game, &["a.bin", "b.bin"]).unwrap();
        put(&built.join("a.bin"), "three");
        let s = run(&built, &game, &["a.bin", "b.bin"]).unwrap();
        assert_eq!((s.copied, s.reused), (1, 1));
        assert_eq!(
            std::fs::read_to_string(root_of(&game).join("a.bin")).unwrap(),
            "three"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_file_from_another_source_is_copied_again() {
        let root = tmp("source");
        let game = root.join("game");
        let (a, b) = (root.join("a/db.bin"), root.join("b/db.bin"));
        put(&a, "db-A");
        put(&b, "db-B");
        let when = std::fs::metadata(&a).unwrap().modified().unwrap();
        std::fs::File::options()
            .write(true)
            .open(&b)
            .unwrap()
            .set_modified(when)
            .unwrap();
        sync(&game, &[("db.bin", a.as_path())], roomy, &mut |_, _| {}).unwrap();
        let s = sync(&game, &[("db.bin", b.as_path())], roomy, &mut |_, _| {}).unwrap();
        assert_eq!((s.copied, s.reused), (1, 0));
        assert_eq!(
            std::fs::read_to_string(root_of(&game).join("db.bin")).unwrap(),
            "db-B"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_file_the_overlay_no_longer_has_is_deleted() {
        let root = tmp("stale");
        let (built, game) = (root.join("built"), root.join("game"));
        put(&built.join("a.bin"), "one");
        put(&built.join("dir/b.bin"), "two");
        run(&built, &game, &["a.bin", "dir/b.bin"]).unwrap();
        let s = run(&built, &game, &["a.bin"]).unwrap();
        assert_eq!((s.copied, s.reused, s.removed), (0, 1, 1));
        assert!(!root_of(&game).join("dir/b.bin").exists());
        assert!(!root_of(&game).join("dir").exists());
        assert!(root_of(&game).join("a.bin").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_cache_file_the_game_wrote_through_its_link_is_copied_again() {
        let root = tmp("written");
        let (built, game) = (root.join("built"), root.join("game"));
        put(&built.join("a.bin"), "one");
        run(&built, &game, &["a.bin"]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        put(&root_of(&game).join("a.bin"), "two");
        let s = run(&built, &game, &["a.bin"]).unwrap();
        assert_eq!((s.copied, s.reused), (1, 0));
        assert_eq!(
            std::fs::read_to_string(root_of(&game).join("a.bin")).unwrap(),
            "one"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_damaged_cache_file_is_copied_again() {
        let root = tmp("damaged");
        let (built, game) = (root.join("built"), root.join("game"));
        put(&built.join("a.bin"), "one");
        run(&built, &game, &["a.bin"]).unwrap();
        std::fs::remove_file(root_of(&game).join("a.bin")).unwrap();
        let s = run(&built, &game, &["a.bin"]).unwrap();
        assert_eq!((s.copied, s.reused), (1, 0));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn too_little_room_on_the_game_drive_fails_and_leaves_the_cache_alone() {
        let root = tmp("space");
        let (built, game) = (root.join("built"), root.join("game"));
        put(&built.join("a.bin"), "one");
        let src = built.join("a.bin");
        let refs = [("a.bin", src.as_path())];
        let err = sync(&game, &refs, |_| Some(MARGIN), &mut |_, _| {}).unwrap_err();
        assert!(err.contains("game drive"), "{err}");
        assert!(!root_of(&game).join("a.bin").exists());
        let ok = sync(&game, &refs, |_| Some(MARGIN + 3), &mut |_, _| {}).unwrap();
        assert_eq!(ok.copied, 1);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn removing_the_cache_reports_what_it_held() {
        let root = tmp("remove");
        let (built, game) = (root.join("built"), root.join("game"));
        put(&built.join("a.bin"), "12345");
        run(&built, &game, &["a.bin"]).unwrap();
        assert!(size(&game) >= 5);
        assert!(remove(&game) >= 5);
        assert!(!dir(&game).exists());
        assert_eq!(remove(&game), 0);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn volumes_are_compared_by_id() {
        let v = |id: &str| {
            Some(crate::disk::Volume {
                id: id.into(),
                name: id.into(),
            })
        };
        assert_eq!(apart(v("1"), v("2")), Some(true));
        assert_eq!(apart(v("1"), v("1")), Some(false));
        assert_eq!(apart(v("1"), None), None);
    }
}
