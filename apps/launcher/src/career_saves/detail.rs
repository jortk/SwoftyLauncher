use std::path::{Path, PathBuf};

use swofty_core::launcher_saves;
use swofty_core::profile::VANILLA;
use swofty_core::vault::careers::Diff;
use swofty_core::vault::{self, Location, Snapshot, Vault};

#[derive(Clone, Default)]
pub(super) struct Detail {
    pub(super) held: Option<Vec<String>>,
    pub(super) diff: Option<Diff>,
    pub(super) newest: Option<String>,
    pub(super) missing: usize,
}

type Stamp = Option<(u64, std::time::SystemTime)>;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct DetailKey {
    careers_ref: Option<String>,
    index: Stamp,
    now: Stamp,
}

#[derive(Default)]
pub(super) struct DetailCache(pub(super) std::collections::HashMap<PathBuf, (DetailKey, Detail)>);

pub(super) static DETAILS: std::sync::LazyLock<std::sync::Mutex<DetailCache>> =
    std::sync::LazyLock::new(Default::default);

pub(super) fn stamp(path: &Path) -> Stamp {
    let m = std::fs::metadata(path).ok()?;
    Some((m.len(), m.modified().ok()?))
}

pub(super) fn details(
    v: &Vault,
    snaps: &[Snapshot],
    now_path: &Path,
    cache: &std::sync::Mutex<DetailCache>,
) -> Vec<Detail> {
    let now_stamp = stamp(now_path);
    let mut now: Option<Vec<swofty_core::stdlib::careers::Career>> = None;
    let mut parsed: std::collections::HashMap<
        String,
        Option<std::sync::Arc<Vec<swofty_core::stdlib::careers::Career>>>,
    > = Default::default();
    let mut seen = std::collections::HashSet::new();
    let out = snaps
        .iter()
        .map(|snap| {
            let path = v.dir.join(&snap.id);
            seen.insert(path.clone());
            let key = DetailKey {
                careers_ref: snap.careers_ref.clone(),
                index: snap
                    .careers_ref
                    .as_deref()
                    .filter(|r| !r.contains("..") && !r.starts_with('/'))
                    .and_then(|r| stamp(&v.dir.join(r))),
                now: now_stamp,
            };
            if let Some((k, d)) = cache.lock().ok().and_then(|c| c.0.get(&path).cloned())
                && k == key
            {
                return d;
            }
            let now = now.get_or_insert_with(|| swofty_core::stdlib::careers::read(now_path));
            let index = snap.careers_ref.as_deref().and_then(|r| {
                parsed
                    .entry(r.to_owned())
                    .or_insert_with(|| v.index_of(snap).map(std::sync::Arc::new))
                    .clone()
            });
            let held = index.as_ref().map(|i| {
                vault::careers::in_save(i)
                    .map(|c| match c.label.is_empty() {
                        true => c.career.clone(),
                        false => c.label.clone(),
                    })
                    .collect()
            });
            let newest = index
                .as_ref()
                .and_then(|i| vault::careers::newest(i))
                .map(|b| b.name);
            let missing = index
                .as_ref()
                .map_or(0, |i| vault::careers::marked_missing(i).len());
            let diff = index
                .filter(|_| !now.is_empty())
                .map(|i| vault::careers::diff(now, &i));
            let detail = Detail {
                held,
                diff,
                newest,
                missing,
            };
            if let Ok(mut c) = cache.lock() {
                c.0.insert(path, (key, detail.clone()));
            }
            detail
        })
        .collect();
    if let Ok(mut c) = cache.lock() {
        c.0.retain(|p, _| !p.starts_with(&v.dir) || seen.contains(p));
    }
    out
}

pub(super) fn tag(h: &mut impl std::hash::Hasher, path: &Path) {
    use std::hash::Hash;
    path.hash(h);
    stamp(path).hash(h);
}

pub(super) fn tag_folder(h: &mut impl std::hash::Hasher, dir: &Path) {
    use std::hash::Hash;
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name() != ".lock")
        .collect();
    files.sort_by_key(|e| e.file_name());
    for e in files {
        e.file_name().hash(h);
        e.metadata()
            .ok()
            .map(|m| (m.len(), m.modified().ok(), m.is_dir()))
            .hash(h);
    }
}

pub(crate) fn saves_print(root: &Path, profile: &str, game_running: bool) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (profile, game_running).hash(&mut h);
    tag(&mut h, &swofty_core::stdlib::careers::data_path(root));
    let sources = match profile == VANILLA {
        true => Vault::sources(root),
        false => Location::source_of_profile(root, profile).into_iter().collect(),
    };
    for source in sources {
        let dir = Vault::root(root).join(&source);
        tag_folder(&mut h, &dir);
        tag_folder(&mut h, &dir.join("snapshots"));
    }
    if profile != VANILLA {
        tag_folder(&mut h, &launcher_saves::remote_dir(root, profile));
    }
    tag_folder(&mut h, &root.join("profiles"));
    h.finish()
}
