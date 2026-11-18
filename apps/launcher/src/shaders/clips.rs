use std::path::Path;

use swofty_i18n::tr;
use swofty_preview::{CachedClip, ClipStore};

pub(super) type ClipItem = CachedClip;

fn store(root: &Path) -> ClipStore {
    ClipStore::new(&root.join("cache").join("preview"))
}

pub(super) fn scan(root: &Path) -> Vec<ClipItem> {
    store(root).cached()
}

/// Downloads clips from the CDN index that aren't cached yet, reporting `(fraction, label)`: only the
/// first missing one with `first_only`, so the preview can start while the rest come in.
pub(super) fn download(root: &Path, first_only: bool, progress: &dyn Fn(f32, &str)) -> Result<(), String> {
    progress(0., tr("Looking for clips"));
    let clips = store(root);
    let cached: Vec<String> = clips.cached().into_iter().map(|c| c.id).collect();
    let mut todo: Vec<_> = clips
        .index()?
        .into_iter()
        .filter(|e| !cached.contains(&e.id))
        .collect();
    if first_only {
        todo.truncate(1);
    }
    let count = todo.len().max(1) as f32;
    for (i, entry) in todo.iter().enumerate() {
        let base = i as f32 / count;
        let label = if entry.title.is_empty() {
            &entry.id
        } else {
            &entry.title
        };
        clips.download(entry, |done, total| {
            let f = if total > 0 { done as f32 / total as f32 } else { 0. };
            progress(base + f.min(1.) / count, label);
        })?;
    }
    progress(1., tr("Done"));
    Ok(())
}
