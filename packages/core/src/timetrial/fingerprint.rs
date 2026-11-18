//! The addon set a lap was recorded with, in the shape the Time Trial boards server reads.

use serde_json::{Value, json};

use crate::builtin::CORE_ID;
use crate::multiplayer::fingerprint::{Fingerprint, Source};

pub const CORE: &str = "core";
pub const BUILTIN: &str = "builtin";
pub const REGISTRY: &str = "registry";
pub const LOCAL: &str = "local";

/// `source` is `core` for the launcher's own core, `builtin` for an addon shipped inside the launcher,
/// `registry` (with the SHA-256 of the Addon Browser download as `registry`) for an installed registry
/// copy, and `local` for anything else; the server ranks only laps whose addons are all approved ones.
pub fn wire(fp: &Fingerprint) -> Value {
    let addons: Vec<Value> = fp
        .addons
        .iter()
        .map(|a| {
            let (source, registry) = if a.id == CORE_ID {
                (CORE, None)
            } else if matches!(a.source, Source::Builtin(_)) {
                (BUILTIN, None)
            } else if let Some(sha) = &a.registry {
                (REGISTRY, Some(sha.clone()))
            } else {
                (LOCAL, None)
            };
            let mut doc = json!({"id": a.id, "version": a.version.to_string(), "source": source});
            if let Some(sha) = registry {
                doc["registry"] = sha.into();
            }
            doc
        })
        .collect();
    json!({
        "digest": fp.digest,
        "launcher": fp.launcher,
        "game_build": fp.game_build.exe_sha256,
        "addons": addons,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use semver::Version;

    use super::*;
    use crate::baseline::Identity;
    use crate::multiplayer::fingerprint::AddonPrint;

    fn print(id: &str, source: Source, registry: Option<&str>) -> AddonPrint {
        AddonPrint {
            id: id.into(),
            name: id.into(),
            version: Version::new(1, 2, 0),
            source,
            registry: registry.map(str::to_owned),
            inputs: None,
            order: None,
            detail: None,
            stat_ids: BTreeMap::new(),
        }
    }

    #[test]
    fn names_each_addon_kind_for_the_server() {
        let fp = Fingerprint::new(
            Identity {
                build_id: "1".into(),
                exe_sha256: "exe".into(),
            },
            vec![
                print(CORE_ID, Source::Builtin("a".into()), None),
                print("swofty.std", Source::Builtin("b".into()), None),
                print("a.reg", Source::Built("c".into()), Some("zipsha")),
                print("a.local", Source::Content("d".into()), None),
            ],
            BTreeMap::new(),
            None,
        );
        let doc = wire(&fp);
        let kinds: Vec<(&str, &str)> = doc["addons"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| (a["id"].as_str().unwrap(), a["source"].as_str().unwrap()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("a.local", "local"),
                ("a.reg", "registry"),
                (CORE_ID, "core"),
                ("swofty.std", "builtin")
            ]
        );
        assert_eq!(doc["addons"][1]["registry"], "zipsha");
        assert!(doc["addons"][0].get("registry").is_none());
        assert_eq!(doc["game_build"], "exe");
    }
}
