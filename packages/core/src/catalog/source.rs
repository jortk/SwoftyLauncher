//! Which installed addons the play bar updates from the Addon Browser, and the versions the player
//! skipped. Every addon takes the Addon Browser's newer versions of its id except an import, which
//! keeps its own files.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::Result;
use crate::manifest::AddonManifest;

/// The skipped Addon Browser versions, by addon id.
pub const SKIPPED_FILE: &str = "state/skipped-updates.json";

/// Whether the play bar offers `m` the Addon Browser's newer versions of its id: an addon installed
/// from the Addon Browser or from a file does, an import ([`crate::imported::is_import`]) doesn't.
pub fn takes_updates(m: &AddonManifest) -> bool {
    !crate::imported::is_import(m)
}

fn skipped_path(root: &Path) -> PathBuf {
    root.join(SKIPPED_FILE)
}

/// The Addon Browser version of each addon the play bar doesn't offer.
pub fn skipped(root: &Path) -> BTreeMap<String, semver::Version> {
    crate::read_json(&skipped_path(root)).unwrap_or_default()
}

/// Stops the play bar offering each addon the version given with it; a newer one is offered again.
pub fn skip(
    root: &Path,
    versions: impl IntoIterator<Item = (String, semver::Version)>,
) -> Result<BTreeMap<String, semver::Version>> {
    let mut all = skipped(root);
    all.extend(versions);
    crate::write_json_shared(&skipped_path(root), &all)?;
    Ok(all)
}
