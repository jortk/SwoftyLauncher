use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn old_data_dir(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("swofty-cli-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    let write = |rel: &str, text: &str| {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    write("config.json", r#"{"data_version":2}"#);
    write(
        "mods/a.old/swofty.json",
        r#"{"schema_version":1,"id":"a.old","name":"Old","version":"1.0.0","author":"t","compat":{"game":"f1_25"}}"#,
    );
    write(
        "profiles/default.json",
        r#"{"schema_version":1,"name":"default","addons":[]}"#,
    );
    root
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = path.strip_prefix(root).unwrap().to_path_buf();
            if path.is_dir() {
                out.insert(rel, None);
                walk(&path, root, out);
            } else {
                out.insert(rel, Some(std::fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn swofty(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_swofty"))
        .args(args)
        .env("SWOFTY_DATA_DIR", root)
        .output()
        .unwrap()
}

#[test]
fn only_commands_migrate_the_data_dir() {
    let root = old_data_dir("parse-first");
    let before = snapshot(&root);
    let dir = root.to_str().unwrap();
    let runs: [&[&str]; 12] = [
        &["--version"],
        &["-V"],
        &["--help"],
        &["help"],
        &["help", "mods"],
        &["mods", "--help"],
        &["maps", "--help"],
        &["addons", "enable"],
        &["biuld"],
        &[],
        &["--data-dir", dir, "--version"],
        &["--data-dir", dir, "--help"],
    ];
    for args in runs {
        let out = swofty(&root, args);
        assert!(
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .all(|l| !l.starts_with("swofty: ")),
            "swofty {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(snapshot(&root), before, "swofty {args:?} changed the data dir");
    }
    let version = swofty(&root, &["--version"]);
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains(env!("CARGO_PKG_VERSION")));

    let out = swofty(&root, &["addons", "list"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stdout).contains("a.old"));
    assert!(!root.join("mods").exists());
    assert!(root.join("addons/a.old/swofty.json").is_file());
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("config.json")).unwrap()).unwrap();
    assert_eq!(config["data_version"], 4);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn swoftymod_cannot_be_disabled() {
    let root = old_data_dir("locked");
    let out = swofty(&root, &["addons", "disable", "swofty.core"]);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains("SwoftyMod (swofty.core) is always on and cannot be disabled")
    );
    let profile: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("profiles/default.json")).unwrap()).unwrap();
    let addons = profile["addons"].as_array().unwrap();
    assert!(
        addons
            .iter()
            .all(|a| a["id"] != "swofty.core" || a["enabled"] != false),
        "{profile}"
    );
    std::fs::remove_dir_all(&root).ok();
}
