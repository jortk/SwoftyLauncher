use std::path::{Path, PathBuf};

use swofty_addon::launcher::Claim;

use crate::manifest::MANIFEST_FILE;
use crate::profile::Profile;
use crate::registry::{Entry, Registry};
use crate::{Error, Result};

pub const ADDON_EXTENSION: &str = "swoftyaddon";
/// Accepted for packages made before the mods-to-addons rename.
pub const ADDON_EXTENSION_LEGACY: &str = "swoftymod";
/// How many folders down an archive may hold an addon.
const MAX_DEPTH: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    Addon,
    Importer(String),
}

/// Who may be asked about a drop. Claiming and importing run a helper's own
/// program, so a helper must be on in `profile`, or always on: an installed
/// addon runs nothing until the player turns it on.
fn may_import(entry: &Entry, profile: &Profile) -> bool {
    entry.plugin.is_none() || entry.locked || profile.enabled().any(|id| id == entry.id())
}

pub fn route(registry: &Registry, profile: &Profile, path: &Path) -> Option<Route> {
    let mut claims = vec![(Route::Addon, addon_claim(path))];
    claims.extend(
        registry
            .iter()
            .filter(|e| may_import(e, profile))
            .filter_map(|e| {
                let claim = e.addon.importer()?.claims(path);
                Some((Route::Importer(e.id().to_owned()), claim))
            }),
    );
    if let Some((route, _)) = claims.iter().find(|(_, c)| *c == Claim::Yes) {
        return Some(route.clone());
    }
    let mut maybe = claims.into_iter().filter(|(_, c)| *c == Claim::Maybe);
    match (maybe.next(), maybe.next()) {
        (Some((route, _)), None) => Some(route),
        _ => None,
    }
}

pub fn route_all(
    registry: &Registry,
    profile: &Profile,
    paths: &[PathBuf],
) -> (Vec<(Route, Vec<PathBuf>)>, Vec<PathBuf>) {
    let mut batches: Vec<(Route, Vec<PathBuf>)> = Vec::new();
    let mut unclaimed = Vec::new();
    for path in paths {
        match route(registry, profile, path) {
            Some(r) => match batches.iter_mut().find(|(b, _)| *b == r) {
                Some((_, batch)) => batch.push(path.clone()),
                None => batches.push((r, vec![path.clone()])),
            },
            None => unclaimed.push(path.clone()),
        }
    }
    (batches, unclaimed)
}

pub fn addon_claim(path: &Path) -> Claim {
    if path.is_dir() {
        return claim(addon_root(path).is_some());
    }
    if is_package(path) {
        return Claim::Yes;
    }
    claim(archive_names(path).is_some_and(|n| holds_addon(&n)))
}

pub fn is_package(path: &Path) -> bool {
    has_extension(path, ADDON_EXTENSION) || has_extension(path, ADDON_EXTENSION_LEGACY)
}

pub fn addon_root(dir: &Path) -> Option<PathBuf> {
    if dir.join(MANIFEST_FILE).is_file() {
        return Some(dir.to_path_buf());
    }
    let entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| Some(e.ok()?.path()))
        .collect();
    match entries.as_slice() {
        [only] if only.join(MANIFEST_FILE).is_file() => Some(only.clone()),
        _ => None,
    }
}

/// The addon folders and packages in an unpacked archive, outermost first: a
/// folder with a swofty.json is one addon, whatever it holds.
pub fn found_addons(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    find_addons(dir, 0, &mut found);
    found
}

fn find_addons(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if dir.join(MANIFEST_FILE).is_file() {
        found.push(dir.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| Some(e.ok()?.path())).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if depth < MAX_DEPTH {
                find_addons(&path, depth + 1, found);
            }
        } else if is_package(&path) {
            found.push(path);
        }
    }
}

fn holds_addon(names: &[String]) -> bool {
    names.iter().any(|n| {
        let parts: Vec<&str> = n.split('/').filter(|p| !p.is_empty()).collect();
        let Some((file, dirs)) = parts.split_last() else {
            return false;
        };
        let ext = Path::new(file).extension().and_then(|e| e.to_str());
        dirs.len() <= MAX_DEPTH
            && (*file == MANIFEST_FILE
                || ext.is_some_and(|e| {
                    e.eq_ignore_ascii_case(ADDON_EXTENSION) || e.eq_ignore_ascii_case(ADDON_EXTENSION_LEGACY)
                }))
    })
}

fn is_rar(path: &Path) -> bool {
    has_extension(path, "rar")
}

fn archive_names(path: &Path) -> Option<Vec<String>> {
    if is_rar(path) {
        rar_names(path)
    } else if has_extension(path, "zip") {
        zip_names(path)
    } else {
        None
    }
}

fn zip_names(path: &Path) -> Option<Vec<String>> {
    let zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    zip.file_names()
        .map(|n| n.ok().map(|n| n.replace('\\', "/")))
        .collect()
}

fn rar_names(path: &Path) -> Option<Vec<String>> {
    let rar = rars::ArchiveReader::read_path(path).ok()?;
    Some(
        rar.members()
            .map(|m| m.meta.name_lossy().replace('\\', "/"))
            .collect(),
    )
}

/// Unpacks a zip (an addon package is one) or a rar into `dest`.
pub fn unpack(archive: &Path, dest: &Path) -> Result<()> {
    if is_rar(archive) {
        unrar(archive, dest)
    } else {
        crate::map::unzip(archive, dest, Error::Other)
    }
}

fn unrar(path: &Path, dest: &Path) -> Result<()> {
    let fail = |e: rars::Error| {
        Error::Other(format!(
            "{} is not a RAR archive the launcher can read: {e}",
            path.display()
        ))
    };
    let rar = rars::ArchiveReader::read_path(path).map_err(fail)?;
    let mut refused = None;
    rar.extract_to(None, |entry| {
        let name = entry.name_lossy().replace('\\', "/");
        let Some(rel) = enclosed(&name) else {
            refused.get_or_insert(name);
            return Ok(Box::new(std::io::sink()));
        };
        let out = dest.join(rel);
        if entry.is_directory {
            std::fs::create_dir_all(&out)?;
            return Ok(Box::new(std::io::sink()));
        }
        if let Some(dir) = out.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Ok(Box::new(std::fs::File::create(&out)?))
    })
    .map_err(fail)?;
    match refused {
        Some(name) => Err(Error::Other(format!("{}: unsafe entry {name}", path.display()))),
        None => Ok(()),
    }
}

fn enclosed(name: &str) -> Option<PathBuf> {
    if name.starts_with('/') {
        return None;
    }
    let mut rel = PathBuf::new();
    for part in name.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            p if p.contains(':') => return None,
            p => rel.push(p),
        }
    }
    (!rel.as_os_str().is_empty()).then_some(rel)
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

fn claim(yes: bool) -> Claim {
    if yes { Claim::Yes } else { Claim::No }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use swofty_addon::launcher::{Addon, Builtin, Cx, Importer, Progress};

    use super::*;
    use crate::manifest::AddonManifest;
    use crate::map::tests::{OKAYAMA_LIKE, write_zip};
    use crate::store::Store;

    struct Fake {
        manifest: AddonManifest,
        yes: &'static str,
        maybe: &'static str,
    }

    impl Addon for Fake {
        fn manifest(&self) -> &AddonManifest {
            &self.manifest
        }

        fn importer(&self) -> Option<&dyn Importer> {
            Some(self)
        }
    }

    impl Importer for Fake {
        fn claims(&self, path: &Path) -> Claim {
            match path.extension().and_then(|e| e.to_str()) {
                Some(e) if e == self.yes => Claim::Yes,
                Some(e) if e == self.maybe => Claim::Maybe,
                _ => Claim::No,
            }
        }

        fn import(&self, _: &Cx, paths: &[PathBuf], _: &dyn Progress) -> swofty_addon::Result<Vec<String>> {
            Ok(paths.iter().map(|p| p.display().to_string()).collect())
        }
    }

    fn fake(id: &str, yes: &'static str, maybe: &'static str) -> Builtin {
        let manifest = AddonManifest::parse(&format!(
            r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t","compat":{{"game":"f1_25"}}}}"#
        ))
        .unwrap();
        Builtin {
            addon: Arc::new(Fake { manifest, yes, maybe }),
            locked: false,
            default_on: true,
        }
    }

    fn zip_of(path: &Path, files: &[&str]) {
        let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for name in files {
            z.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut z, b"{}").unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn routes_drops_to_core_then_the_claiming_addon() {
        let root = std::env::temp_dir().join(format!("swofty-import-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(root.join("my-addon")).unwrap();
        std::fs::write(root.join("my-addon").join(MANIFEST_FILE), "{}").unwrap();
        std::fs::create_dir_all(root.join("loose")).unwrap();
        zip_of(&root.join("pack.swoftyaddon"), &[MANIFEST_FILE]);
        zip_of(&root.join("legacy.swoftymod"), &[MANIFEST_FILE]);
        zip_of(
            &root.join("wrapped.zip"),
            &["my-addon/swofty.json", "my-addon/plugin.dll"],
        );
        write_zip(&root.join("track.zip"), OKAYAMA_LIKE, true);
        zip_of(&root.join("other.zip"), &["readme.txt"]);
        zip_of(
            &root.join("nested.zip"),
            &["Pack/inner.swoftyaddon", "Pack/readme.txt"],
        );
        zip_of(&root.join("deep.zip"), &["a/b/c/d/swofty.json"]);
        std::fs::write(root.join("livery.serps"), "x").unwrap();
        std::fs::write(root.join("notes.txt"), "x").unwrap();
        let store = Store::new(root.join("data")).with_builtins(vec![fake("t.mods", "serps", "zip")]);
        let reg = Registry::load(&store).unwrap();
        let profile = Profile::new("t");
        let at = |name: &str| route(&reg, &profile, &root.join(name));
        assert_eq!(at("my-addon"), Some(Route::Addon));
        assert_eq!(at("pack.swoftyaddon"), Some(Route::Addon));
        assert_eq!(at("legacy.swoftymod"), Some(Route::Addon));
        assert_eq!(at("wrapped.zip"), Some(Route::Addon));
        assert_eq!(at("track.zip"), Some(Route::Importer("t.mods".into())));
        assert_eq!(at("livery.serps"), Some(Route::Importer("t.mods".into())));
        assert_eq!(at("other.zip"), Some(Route::Importer("t.mods".into())));
        assert_eq!(at("nested.zip"), Some(Route::Addon));
        assert_eq!(at("deep.zip"), Some(Route::Importer("t.mods".into())));
        assert_eq!(at("loose"), None);
        assert_eq!(at("notes.txt"), None);
        let paths: Vec<PathBuf> = ["livery.serps", "notes.txt", "legacy.swoftymod", "other.zip"]
            .iter()
            .map(|n| root.join(n))
            .collect();
        let (batches, unclaimed) = route_all(&reg, &profile, &paths);
        assert_eq!(
            batches,
            [
                (
                    Route::Importer("t.mods".into()),
                    vec![paths[0].clone(), paths[3].clone()]
                ),
                (Route::Addon, vec![paths[2].clone()]),
            ]
        );
        assert_eq!(unclaimed, [paths[1].clone()]);
        let two = Store::new(root.join("data"))
            .with_builtins(vec![fake("t.mods", "serps", "zip"), fake("t.more", "lng", "zip")]);
        let reg = Registry::load(&two).unwrap();
        assert_eq!(route(&reg, &profile, &root.join("other.zip")), None);
        assert_eq!(
            route(&reg, &profile, &root.join("livery.serps")),
            Some(Route::Importer("t.mods".into()))
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn keeps_rar_entries_inside_the_folder() {
        assert_eq!(
            enclosed("Pack/a/swofty.json"),
            Some(PathBuf::from("Pack/a/swofty.json"))
        );
        assert_eq!(enclosed("./Pack//a"), Some(PathBuf::from("Pack/a")));
        assert_eq!(enclosed("../evil.dll"), None);
        assert_eq!(enclosed("Pack/../../evil.dll"), None);
        assert_eq!(enclosed("/etc/evil"), None);
        assert_eq!(enclosed("C:/evil.dll"), None);
        assert_eq!(enclosed(""), None);
    }
}
