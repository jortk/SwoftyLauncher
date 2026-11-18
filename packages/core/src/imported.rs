//! The ids imports are installed under. The Simplified Importer names every addon it makes
//! `mod.<name>`, the id any Addon Browser addon may also have. The launcher installs a new import
//! under its track author's namespace, or under `imported-mod.`, and never in place of an addon
//! that isn't an import; an import already installed keeps its id.

use std::path::Path;

use crate::manifest::{AddonManifest, MANIFEST_FILE, valid_id};
use crate::store::Store;
use crate::{Error, Result, io_err};

/// The namespace of the ids the Simplified Importer gives the addons it makes.
pub const IMPORTER_NAMESPACE: &str = "mod";
/// The namespace of an import that names no author, which the Addon Browser doesn't install.
pub const IMPORTED_NAMESPACE: &str = "imported-mod";
/// Namespaces an author name doesn't give an import.
pub const RESERVED: &[&str] = &["swofty", "mod", "local", "core", "std", "imported-mod", "dbedit"];
/// The longest author namespace.
const AUTHOR_MAX: usize = 32;

/// Whether `m` is an addon the Simplified Importer made.
pub fn is_import(m: &AddonManifest) -> bool {
    m.origin.as_ref().is_some_and(|o| o.tool == crate::managed::ID)
}

/// The id the Simplified Importer gave the import `m`, `mod.<name>`, when the launcher installed it
/// under another one ([`candidates`]). None for an addon that isn't an import or that kept its id.
pub fn asked_id(m: &AddonManifest) -> Option<String> {
    if !is_import(m) {
        return None;
    }
    let (ns, name) = m.id.split_once('.')?;
    (ns != IMPORTER_NAMESPACE).then(|| format!("{IMPORTER_NAMESPACE}.{name}"))
}

/// Whether the helper of the addon `helper` installing `m` is the Simplified Importer installing
/// an addon it made.
pub fn made_by(helper: &str, m: &AddonManifest) -> bool {
    crate::managed::is_managed(helper) && is_import(m)
}

/// `author` as an id namespace: lowercase ASCII letters, digits and `_`, with accents taken off, a
/// letter with no plain spelling left out, and anything else between words as one `-`. None when
/// nothing is left, or when it is one of the [`RESERVED`] namespaces.
pub fn author_namespace(author: &str) -> Option<String> {
    let mut out = String::new();
    for c in author.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else if let Some(plain) = unaccented(c) {
            out.push_str(plain);
        } else if !c.is_alphanumeric() && !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let mut ns: String = out.trim_matches(['-', '_']).chars().take(AUTHOR_MAX).collect();
    ns.truncate(ns.trim_end_matches(['-', '_']).len());
    (!ns.is_empty() && !RESERVED.contains(&ns.as_str())).then_some(ns)
}

/// A lowercase letter's spelling without its accent.
fn unaccented(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'ď' | 'đ' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'ĥ' | 'ħ' => "h",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'ĵ' => "j",
        'ķ' => "k",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'œ' => "oe",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'ś' | 'ŝ' | 'ş' | 'š' | 'ș' => "s",
        'ß' => "ss",
        'ţ' | 'ť' | 'ŧ' | 'ț' => "t",
        'þ' => "th",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'ŵ' => "w",
        'ý' | 'ÿ' | 'ŷ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        _ => return None,
    })
}

/// The ids an import that asked for `asked` may have, best first. An id in the Simplified
/// Importer's namespace becomes `<author>.<name>` when its track package names an author, then
/// `imported-mod.<name>`; any other id is kept, then `imported-mod.` replaces its namespace.
pub fn candidates(asked: &str, author: Option<&str>) -> Vec<String> {
    let mut out = Vec::new();
    match asked
        .strip_prefix(IMPORTER_NAMESPACE)
        .and_then(|r| r.strip_prefix('.'))
    {
        Some(name) => {
            if let Some(ns) = author.and_then(author_namespace) {
                out.push(format!("{ns}.{name}"));
            }
            out.push(format!("{IMPORTED_NAMESPACE}.{name}"));
        }
        None => {
            out.push(asked.to_owned());
            if let Some((_, name)) = asked.split_once('.') {
                out.push(format!("{IMPORTED_NAMESPACE}.{name}"));
            }
        }
    }
    out.dedup();
    out.retain(|id| valid_id(id));
    out
}

/// Whether an import may be installed as `id`: no addon has it, or an import that wasn't
/// installed from the Addon Browser.
fn free_for_import(store: &Store, id: &str) -> bool {
    if store.is_builtin(id) || crate::managed::is_managed(id) {
        return false;
    }
    let dir = store.addons_dir().join(id);
    if !dir.exists() {
        return true;
    }
    AddonManifest::load(&dir).is_ok_and(|m| is_import(&m)) && !store.registry_install_path(id).is_file()
}

/// The id the launcher installs the import `m`, unpacked in `src`, under. An import already
/// installed as `m.id` stays there, so its profiles and saves keep finding it; otherwise the first
/// of its [`candidates`] that no addon but an import has.
pub(crate) fn install_id(store: &Store, src: &Path, m: &AddonManifest) -> Result<String> {
    if store.addons_dir().join(&m.id).exists() && free_for_import(store, &m.id) {
        return Ok(m.id.clone());
    }
    let author = crate::tracks::package_author(src, m);
    candidates(&m.id, author.as_deref())
        .into_iter()
        .find(|id| free_for_import(store, id))
        .ok_or_else(|| {
            Error::Other(format!(
                "{} can't be installed: the ids it could have belong to addons that aren't imports",
                m.name
            ))
        })
}

/// Gives the addon unpacked in `dir` the id `id`, and reads it back.
pub(crate) fn rename(dir: &Path, id: &str) -> Result<AddonManifest> {
    let path = dir.join(MANIFEST_FILE);
    let data = std::fs::read(&path).map_err(io_err(&path))?;
    let mut doc: serde_json::Value = serde_json::from_slice(&data).map_err(|source| Error::Json {
        path: path.clone(),
        source,
    })?;
    doc["id"] = id.into();
    let mut data = serde_json::to_vec_pretty(&doc).expect("serialisable");
    data.push(b'\n');
    std::fs::write(&path, data).map_err(io_err(&path))?;
    Ok(AddonManifest::load(dir)?)
}

/// Refuses an Addon Browser download that would replace an import with its id, or that has an id
/// in [`IMPORTED_NAMESPACE`].
pub(crate) fn refuse_listed(store: &Store, m: &AddonManifest) -> Result<()> {
    if m.id
        .strip_prefix(IMPORTED_NAMESPACE)
        .is_some_and(|r| r.starts_with('.'))
    {
        return Err(Error::Other(format!(
            "{}: the {IMPORTED_NAMESPACE}. prefix is kept for imports",
            m.id
        )));
    }
    let dir = store.addons_dir().join(&m.id);
    match AddonManifest::load(&dir) {
        Ok(have) if is_import(&have) && !store.registry_install_path(&m.id).is_file() => {
            Err(Error::Other(format!(
                "{} is an import with the id of {}; remove it on the Addons page to install {} from the Addon Browser",
                have.name, m.name, m.name
            )))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use swofty_addon::launcher::AddonInfo;

    use super::*;
    use crate::catalog::source;
    use crate::catalog::{Addon, Version};
    use crate::registry::Host;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-imported-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// An addon folder as the Simplified Importer writes one for a track package.
    fn track(dir: &Path, id: &str, code: &str, author: Option<&str>, version: &str) -> PathBuf {
        std::fs::remove_dir_all(dir).ok();
        std::fs::create_dir_all(dir.join("ops")).unwrap();
        std::fs::create_dir_all(dir.join("track")).unwrap();
        let manifest = serde_json::json!({
            "schema_version": 1, "id": id, "name": format!("Track {code}"), "version": version,
            "author": author.unwrap_or("Unknown"), "compat": { "game": "f1_25" },
            "operations": { "assets": ["ops/track.json"] },
            "origin": { "tool": crate::managed::ID, "file": format!("{code}.zip") },
        });
        std::fs::write(dir.join(MANIFEST_FILE), manifest.to_string()).unwrap();
        std::fs::write(
            dir.join("ops/track.json"),
            r#"{"schema_version":1,"ops":[{"op":"track.add","source":"track"}]}"#,
        )
        .unwrap();
        let mut pkg: serde_json::Value = serde_json::from_str(crate::map::tests::OKAYAMA_LIKE).unwrap();
        pkg["track_code"] = code.into();
        match author {
            Some(a) => pkg["author_name"] = a.into(),
            None => _ = pkg.as_object_mut().unwrap().remove("author_name"),
        }
        std::fs::write(dir.join("track/manifest.json"), pkg.to_string()).unwrap();
        dir.to_path_buf()
    }

    fn import(store: &Store, src: &Path) -> swofty_addon::Result<AddonInfo> {
        let p = store.profile(crate::store::DEFAULT_PROFILE).unwrap();
        let host = Host::new(store, crate::managed::ID, &p, None).unwrap();
        host.cx().addons().unwrap().install(src)
    }

    fn ids(store: &Store) -> Vec<String> {
        store
            .addons()
            .unwrap()
            .into_keys()
            .filter(|id| id != crate::builtin::CORE_ID)
            .collect()
    }

    fn listed(id: &str, version: &str) -> Addon {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": format!("Listed {id}"), "author": "Ashton", "summary": "", "description": "",
            "versions": [{ "version": version, "size": 1, "sha256": "00", "url": "http://x/a.zip",
                "published_at": "2026-11-17T00:00:00Z" }],
        }))
        .unwrap()
    }

    /// `id` at `version` installed from the Addon Browser.
    fn from_registry(store: &Store, root: &Path, id: &str, version: &str) {
        let dir = root.join(format!("download-{id}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(MANIFEST_FILE),
            format!(
                r#"{{"schema_version":1,"id":"{id}","name":"Listed {id}","version":"{version}","author":"Ashton","compat":{{"game":"f1_25"}}}}"#
            ),
        )
        .unwrap();
        let v: semver::Version = version.parse().unwrap();
        store.install_listed(&dir, None, id, &v).unwrap();
        let listed: Version = serde_json::from_value(serde_json::json!({
            "version": version, "size": 1, "sha256": "00", "url": "http://x/a.zip",
            "published_at": "2026-11-17T00:00:00Z",
        }))
        .unwrap();
        crate::catalog::record_install(store, id, &listed).unwrap();
    }

    fn update_ids(store: &Store, catalog: &[Addon]) -> Vec<String> {
        crate::catalog::store_updates(store, catalog, &crate::update::current_version())
            .unwrap()
            .into_iter()
            .map(|u| u.id)
            .collect()
    }

    #[test]
    fn an_author_name_becomes_a_namespace_of_id_characters() {
        let ns = |a: &str| author_namespace(a);
        assert_eq!(ns("Pajko").as_deref(), Some("pajko"));
        assert_eq!(ns("RM Designs").as_deref(), Some("rm-designs"));
        assert_eq!(ns("  José Álvarez ").as_deref(), Some("jose-alvarez"));
        assert_eq!(ns("Ştefan.K_").as_deref(), Some("stefan-k"));
        assert_eq!(ns("Ølsen & Søn (2026)").as_deref(), Some("olsen-son-2026"));
        assert_eq!(ns("Пётр Pavel").as_deref(), Some("pavel"));
        assert_eq!(ns(&"a".repeat(40)).map(|n| n.len()), Some(AUTHOR_MAX));
        for none in [
            "",
            "   ",
            "___",
            "日本",
            "Swofty",
            "MOD",
            "imported-mod",
            "Local",
            "core",
            "STD",
            "dbedit",
        ] {
            assert_eq!(ns(none), None, "{none:?}");
        }
    }

    #[test]
    fn an_import_asks_for_its_author_s_id_then_the_imported_namespace() {
        let c = |asked: &str, author: Option<&str>| candidates(asked, author);
        assert_eq!(
            c("mod.sepang", Some("Pajko")),
            ["pajko.sepang", "imported-mod.sepang"]
        );
        assert_eq!(
            c("mod.sepang-track", Some("Pajko")),
            ["pajko.sepang-track", "imported-mod.sepang-track"]
        );
        assert_eq!(c("mod.sepang", None), ["imported-mod.sepang"]);
        assert_eq!(c("mod.sepang", Some("swofty")), ["imported-mod.sepang"]);
        assert_eq!(c("mod.sepang", Some("日本")), ["imported-mod.sepang"]);
        assert_eq!(
            c("mod.rm-designs.realism", None),
            ["imported-mod.rm-designs.realism"]
        );
        assert_eq!(
            c("pajko.sepang", Some("Other")),
            ["pajko.sepang", "imported-mod.sepang"]
        );
        assert_eq!(c("imported-mod.sepang", None), ["imported-mod.sepang"]);
    }

    #[test]
    fn an_import_without_an_author_is_installed_in_the_imported_namespace_and_again_in_place() {
        let root = tmp("plain");
        let store = Store::new(root.join("data"));
        let src = track(&root.join("out"), "mod.sepang", "sepang", None, "1.0.0");
        let info = import(&store, &src).unwrap();
        assert_eq!(info.id, "imported-mod.sepang");
        assert_eq!(info.dir, store.addons_dir().join("imported-mod.sepang"));
        let m = &store.addons().unwrap()["imported-mod.sepang"];
        assert!(is_import(m));
        assert!(!source::takes_updates(m));
        assert_eq!(asked_id(m).as_deref(), Some("mod.sepang"));
        assert!(
            store
                .profile(crate::store::DEFAULT_PROFILE)
                .unwrap()
                .has("imported-mod.sepang")
        );

        // The importer asks for the id it was given, or for mod.<code> again when it lost its record.
        for (asked, version) in [("imported-mod.sepang", "1.1.0"), ("mod.sepang", "1.2.0")] {
            let src = track(&root.join("out"), asked, "sepang", None, version);
            assert_eq!(import(&store, &src).unwrap().id, "imported-mod.sepang");
            assert_eq!(ids(&store), ["imported-mod.sepang"]);
            let m = &store.addons().unwrap()["imported-mod.sepang"];
            assert_eq!(m.version.to_string(), version);
            assert_eq!(asked_id(m).as_deref(), Some("mod.sepang"));
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_track_by_an_author_is_installed_in_the_author_s_namespace() {
        let root = tmp("author");
        let store = Store::new(root.join("data"));
        let src = track(&root.join("out"), "mod.sepang", "sepang", Some("Pajko"), "1.0.0");
        assert_eq!(import(&store, &src).unwrap().id, "pajko.sepang");
        let src = track(
            &root.join("out"),
            "pajko.sepang",
            "sepang",
            Some("Pajko"),
            "1.1.0",
        );
        assert_eq!(import(&store, &src).unwrap().id, "pajko.sepang");
        let src = track(&root.join("out"), "mod.sepang", "sepang", Some("Pajko"), "1.2.0");
        assert_eq!(import(&store, &src).unwrap().id, "pajko.sepang");
        assert_eq!(ids(&store), ["pajko.sepang"]);
        let m = &store.addons().unwrap()["pajko.sepang"];
        assert_eq!(asked_id(m).as_deref(), Some("mod.sepang"));
        let src = track(&root.join("out"), "mod.monza", "monza", Some("Swofty"), "1.0.0");
        assert_eq!(import(&store, &src).unwrap().id, "imported-mod.monza");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_import_and_an_addon_browser_addon_with_its_id_keep_their_own_files() {
        let root = tmp("clash");
        let store = Store::new(root.join("data"));
        from_registry(&store, &root, "mod.sepang", "4.4.2");
        from_registry(&store, &root, "ashton.monza", "1.0.0");

        let src = track(&root.join("out"), "mod.sepang", "sepang", None, "1.0.0");
        assert_eq!(import(&store, &src).unwrap().id, "imported-mod.sepang");
        let src = track(&root.join("out"), "mod.monza", "monza", Some("Ashton"), "9.0.0");
        assert_eq!(import(&store, &src).unwrap().id, "imported-mod.monza");
        let addons = store.addons().unwrap();
        assert_eq!(addons["mod.sepang"].version.to_string(), "4.4.2");
        assert!(addons["mod.sepang"].origin.is_none());
        assert_eq!(addons["ashton.monza"].version.to_string(), "1.0.0");

        // The imports take no Addon Browser updates, whatever their id or version.
        let catalog = [
            listed("mod.sepang", "4.4.3"),
            listed("ashton.monza", "2.0.0"),
            listed("imported-mod.sepang", "9.0.0"),
        ];
        assert_eq!(update_ids(&store, &catalog), ["ashton.monza", "mod.sepang"]);

        // The Addon Browser refuses a download in the imports' namespace.
        let dir = root.join("listed-imported");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(MANIFEST_FILE),
            r#"{"schema_version":1,"id":"imported-mod.sepang","name":"Taken","version":"9.0.0","author":"x","compat":{"game":"f1_25"}}"#,
        )
        .unwrap();
        let err = store
            .install_listed(&dir, None, "imported-mod.sepang", &"9.0.0".parse().unwrap())
            .unwrap_err();
        assert!(err.to_string().contains("kept for imports"), "{err}");
        assert_eq!(
            store.addons().unwrap()["imported-mod.sepang"].version.to_string(),
            "1.0.0"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_import_installed_with_the_importer_s_id_keeps_it_and_its_files() {
        let root = tmp("legacy");
        let store = Store::new(root.join("data"));
        // As a launcher that kept the importer's ids installed it.
        let src = track(&root.join("out"), "mod.sepang", "sepang", Some("Pajko"), "1.0.0");
        store.install_addon(&src).unwrap();
        store
            .update_profile(crate::store::DEFAULT_PROFILE, |p| {
                p.set_enabled("mod.sepang", true)
            })
            .unwrap();
        let m = &store.addons().unwrap()["mod.sepang"];
        assert!(is_import(m));
        assert_eq!(asked_id(m), None);
        assert!(update_ids(&store, &[listed("mod.sepang", "4.4.3")]).is_empty());

        let src = track(&root.join("out"), "mod.sepang", "sepang", Some("Pajko"), "1.1.0");
        assert_eq!(import(&store, &src).unwrap().id, "mod.sepang");
        assert_eq!(ids(&store), ["mod.sepang"]);
        let p = store.profile(crate::store::DEFAULT_PROFILE).unwrap();
        assert!(p.enabled().any(|id| id == "mod.sepang"));

        // The Addon Browser's mod.sepang doesn't install over it.
        let dir = root.join("download");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(MANIFEST_FILE),
            r#"{"schema_version":1,"id":"mod.sepang","name":"Sepang International Circuit","version":"4.4.3","author":"Ashton","compat":{"game":"f1_25"}}"#,
        )
        .unwrap();
        let err = store
            .install_listed(&dir, None, "mod.sepang", &"4.4.3".parse().unwrap())
            .unwrap_err();
        assert!(err.to_string().contains("remove it on the Addons page"), "{err}");
        assert_eq!(store.addons().unwrap()["mod.sepang"].version.to_string(), "1.1.0");

        // Once the player removes the import, it installs.
        store.remove_addon("mod.sepang").unwrap();
        store
            .install_listed(&dir, None, "mod.sepang", &"4.4.3".parse().unwrap())
            .unwrap();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_file_install_takes_updates_and_an_import_never_does() {
        let root = tmp("sources");
        let store = Store::new(root.join("data"));
        let write = |id: &str| {
            let dir = root.join(id);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(MANIFEST_FILE),
                format!(
                    r#"{{"schema_version":1,"id":"{id}","name":"{id}","version":"1.0.0","author":"t","compat":{{"game":"f1_25"}}}}"#
                ),
            )
            .unwrap();
            dir
        };
        store.install_addon(&write("t.file")).unwrap();
        from_registry(&store, &root, "t.listed", "1.0.0");
        from_registry(&store, &root, "t.old", "1.0.0");
        // As a launcher that recorded no Addon Browser installs left it.
        std::fs::remove_file(store.registry_install_path("t.old")).unwrap();
        let src = track(&root.join("out"), "mod.sepang", "sepang", None, "1.0.0");
        assert_eq!(import(&store, &src).unwrap().id, "imported-mod.sepang");
        let src = track(&root.join("legacy"), "t.import", "monza", None, "1.0.0");
        store.install_addon(&src).unwrap();
        let addons = store.addons().unwrap();
        for id in ["t.file", "t.listed", "t.old"] {
            assert!(source::takes_updates(&addons[id]), "{id}");
        }
        for id in ["imported-mod.sepang", "t.import"] {
            assert!(!source::takes_updates(&addons[id]), "{id}");
        }
        let catalog = [
            listed("t.file", "2.0.0"),
            listed("t.listed", "2.0.0"),
            listed("t.old", "2.0.0"),
            listed("imported-mod.sepang", "2.0.0"),
            listed("t.import", "2.0.0"),
        ];
        assert_eq!(update_ids(&store, &catalog), ["t.file", "t.listed", "t.old"]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_skipped_version_stays_skipped_until_a_newer_one() {
        let root = tmp("skip");
        assert!(source::skipped(&root).is_empty());
        let v = |s: &str| semver::Version::parse(s).unwrap();
        source::skip(&root, [("mod.sepang".to_owned(), v("4.4.3"))]).unwrap();
        let all = source::skip(&root, [("t.other".to_owned(), v("2.0.0"))]).unwrap();
        assert_eq!(all, source::skipped(&root));
        assert_eq!(all["mod.sepang"], v("4.4.3"));
        assert_eq!(all["t.other"], v("2.0.0"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_build_refuses_two_addons_adding_one_track_and_names_both() {
        let root = tmp("both");
        let store = Store::new(root.join("data"));
        let src = track(&root.join("out"), "mod.sepang", "sepang", None, "1.0.0");
        assert_eq!(import(&store, &src).unwrap().id, "imported-mod.sepang");
        let listed = track(&root.join("listed"), "mod.sepang", "sepang", None, "4.4.3");
        let mut doc: serde_json::Value =
            serde_json::from_slice(&std::fs::read(listed.join(MANIFEST_FILE)).unwrap()).unwrap();
        doc.as_object_mut().unwrap().remove("origin");
        std::fs::write(listed.join(MANIFEST_FILE), doc.to_string()).unwrap();
        store
            .install_listed(&listed, None, "mod.sepang", &"4.4.3".parse().unwrap())
            .unwrap();
        let add = |addon: &str| crate::asset::Op {
            op: crate::asset::AssetOp::Track(swofty_addon::asset::TrackOp::Add {
                source: serde_json::from_value(serde_json::json!("track")).unwrap(),
            }),
            origin: crate::asset::Origin {
                addon: addon.into(),
                item: None,
            },
        };
        let err = crate::build::track_packages(&store, vec![add("imported-mod.sepang"), add("mod.sepang")])
            .unwrap_err()
            .to_string();
        assert!(
            err.ends_with(
                "track sepang is added by both imported-mod.sepang and mod.sepang; turn one of them off on the Addons page"
            ),
            "{err}"
        );
        let (one, _) = crate::build::track_packages(&store, vec![add("mod.sepang")]).unwrap();
        assert_eq!(one.len(), 1);
        std::fs::remove_dir_all(&root).ok();
    }
}
