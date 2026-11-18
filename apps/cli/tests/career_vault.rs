use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use swofty_core::save_backup;

const DATA: &str = "savegame@profile#NXDSMWW.BWW";
const INDEX: &str = "savegame@profile#QKRHMYXE";

fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("swofty-cli-vault-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(root.join("home")).unwrap();
    std::fs::create_dir_all(root.join("data")).unwrap();
    std::fs::write(
        root.join("data").join("config.json"),
        r#"{"active_profile":"vanilla"}"#,
    )
    .unwrap();
    root
}

/// A fake Steam install with one account's save, so nothing outside `root`
/// is read.
fn steam(root: &Path) -> PathBuf {
    let steam = root.join("steam");
    std::fs::create_dir_all(steam.join("steamapps")).unwrap();
    let remote = steam
        .join("userdata")
        .join("843502855")
        .join("3059520")
        .join("remote");
    std::fs::create_dir_all(&remote).unwrap();
    std::fs::write(remote.join(DATA), [1u8; 900]).unwrap();
    std::fs::write(remote.join(INDEX), [7u8; 416]).unwrap();
    std::fs::write(remote.join("savegame@highlights001#QKRHMYXE"), b"replay").unwrap();
    std::fs::write(
        steam.join("userdata").join("843502855").join("3059520").join("remotecache.vdf"),
        "\"3059520\"\n{\n\t\"savegame@profile#QKRHMYXE\"\n\t{\n\t\t\"size\"\t\t\"416\"\n\t\t\"sha\"\t\t\"00\"\n\t\t\"syncstate\"\t\t\"1\"\n\t}\n}\n",
    )
    .unwrap();
    remote
}

fn swofty(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_swofty"))
        .args(args)
        .env("SWOFTY_DATA_DIR", root.join("data"))
        .env("SWOFTY_STEAM_ROOT_ONLY", root.join("steam"))
        .env("HOME", root.join("home"))
        .env_remove("LOCALAPPDATA")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn lists_nothing_without_a_save() {
    let root = root("empty");
    let out = stdout(&swofty(&root, &["saves", "list"]));
    assert!(
        out.starts_with("no snapshots: no F1 25 profile save found"),
        "{out}"
    );
}

#[test]
fn backs_up_lists_shows_and_restores() {
    let root = root("flow");
    let remote = steam(&root);
    let out = stdout(&swofty(&root, &["saves", "backup"]));
    assert!(out.starts_with("saved and pinned snapshot "), "{out}");
    let id = out.split_whitespace().nth(4).unwrap().to_owned();
    assert!(id.ends_with("-manual"), "{id}");
    let out = stdout(&swofty(&root, &["saves", "backup"]));
    assert_eq!(
        out,
        format!("the profile save is unchanged since snapshot {id}; pinned it\n")
    );
    let vault = root.join("data").join("career-vault").join("steam-843502855");
    assert_eq!(std::fs::read_dir(vault.join("blobs")).unwrap().count(), 2);

    let list = stdout(&swofty(&root, &["saves", "list"]));
    let lines: Vec<&str> = list.lines().collect();
    assert!(lines[0].starts_with(&format!(
        "vault {}: 1 snapshot(s), 0.0 MB of 500.0 MB",
        vault.display()
    )));
    assert_eq!(
        lines[1],
        format!(
            "save {}: 2 file(s), 1316 bytes, Steam Cloud: waiting to upload",
            remote.display()
        )
    );
    assert_eq!(
        lines[2],
        format!("{id}  backup, profile vanilla, 2 file(s), 1316 bytes, pinned (backup)")
    );

    let show = stdout(&swofty(&root, &["saves", "show", &id]));
    assert!(
        show.contains("profile/NXDSMWW.BWW (savegame@profile#NXDSMWW.BWW): 900 bytes, sha256 "),
        "{show}"
    );
    assert!(
        show.contains("remotecache: size 416, sha 00, syncstate 1"),
        "{show}"
    );

    std::fs::write(remote.join(DATA), [2u8; 100]).unwrap();
    let cache = remote.parent().unwrap().join("remotecache.vdf");
    let cache_before = std::fs::read(&cache).unwrap();
    let out = swofty(&root, &["saves", "restore", &id]);
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    match out.status.success() {
        true => {
            let text = String::from_utf8_lossy(&out.stdout);
            assert!(
                text.contains(&format!("restored {id} into {}", remote.display())),
                "{text}"
            );
            assert_eq!(std::fs::read(remote.join(DATA)).unwrap(), [1u8; 900]);
        }
        false => {
            assert!(
                err.contains("Steam is running") || err.contains("F1 25 is running"),
                "{err}"
            );
            assert_eq!(std::fs::read(remote.join(DATA)).unwrap(), [2u8; 100]);
        }
    }
    assert_eq!(std::fs::read(&cache).unwrap(), cache_before);
    assert_eq!(std::fs::read(remote.join(INDEX)).unwrap(), [7u8; 416]);

    let out = swofty(&root, &["saves", "restore", "nope"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("no snapshot named nope") || err.contains("is running"),
        "{err}"
    );
}

#[test]
fn moves_old_backups_into_the_vault() {
    let root = root("migrate");
    let remote = steam(&root);
    let b = save_backup::take(
        &root.join("data"),
        &remote,
        "default",
        &["swofty.progressive".into()],
        "20261027-010203",
        "session",
    )
    .unwrap()
    .unwrap();
    let out = stdout(&swofty(&root, &["saves", "list"]));
    assert!(
        out.lines().any(|l| l
            == "20261027-010203-migrated  from save-backups, profile default, without swofty.progressive, 2 file(s), \
                1316 bytes, pinned (migrated)"),
        "{out}"
    );
    assert!(b.dir.is_dir(), "the old backup stays until the first prune");
}

#[test]
fn sets_the_cap() {
    let root = root("cap");
    assert_eq!(stdout(&swofty(&root, &["saves", "cap"])), "500 MB\n");
    stdout(&swofty(&root, &["saves", "cap", "800"]));
    assert_eq!(stdout(&swofty(&root, &["saves", "cap"])), "800 MB\n");
    assert!(!swofty(&root, &["saves", "cap", "0"]).status.success());
}

fn index(root: &Path, careers: &[(&str, &str, i64)]) {
    let list: Vec<serde_json::Value> = careers
        .iter()
        .map(|(id, status, round)| {
            serde_json::json!({
                "career": id, "status": status, "label": format!("{id} label"), "mode": "progressive",
                "driver": "Verstappen", "team": "Red Bull", "season": 2016, "round": round,
                "addons": [{"id": "swofty.progressive"}]
            })
        })
        .collect();
    let path = root.join("data").join("careers").join("index.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        serde_json::json!({ "version": 1, "careers": list }).to_string(),
    )
    .unwrap();
}

#[test]
fn lists_careers_and_versions_and_pins() {
    let root = root("careers");
    let remote = steam(&root);
    let out = stdout(&swofty(&root, &["saves", "careers"]));
    assert!(out.starts_with("no careers"), "{out}");

    index(&root, &[("p:1", "present", 7), ("p:2", "present", 1)]);
    let out = stdout(&swofty(&root, &["saves", "backup"]));
    let first = out.split_whitespace().nth(4).unwrap().to_owned();
    std::fs::write(remote.join(DATA), [2u8; 910]).unwrap();
    index(&root, &[("p:2", "present", 2)]);
    let out = stdout(&swofty(&root, &["saves", "backup"]));
    let second = out.split_whitespace().nth(4).unwrap().to_owned();

    let careers = stdout(&swofty(&root, &["saves", "careers"]));
    let lines: Vec<&str> = careers.lines().collect();
    assert_eq!(
        lines[0],
        format!(
            "p:1  p:1 label, progressive, Verstappen, Red Bull, season 2016, round 7, needs swofty.progressive, \
             missing-since:{second}"
        )
    );
    assert_eq!(lines[1], format!("    last in the save at snapshot {first}"));
    assert!(lines[2].ends_with(", present"), "{careers}");

    let versions = stdout(&swofty(&root, &["saves", "versions"]));
    assert!(versions.contains("1 career(s): p:2 label"), "{versions}");
    assert!(
        versions.contains("2 career(s): p:1 label, p:2 label"),
        "{versions}"
    );
    assert!(
        versions.contains("    restoring it gets back: p:1 label (season 2016, round 7)"),
        "{versions}"
    );
    assert!(
        versions.contains(
            "    restoring it loses progress in p:2 label: season 2016, round 2 back to season 2016, round 1"
        ),
        "{versions}"
    );

    let out = stdout(&swofty(&root, &["saves", "pin", &second]));
    assert_eq!(out, format!("pinned snapshot {second}\n"));
    let out = stdout(&swofty(&root, &["saves", "unpin", &second]));
    assert_eq!(
        out,
        format!("unpinned snapshot {second}; it stays kept for backup\n")
    );
    assert!(!swofty(&root, &["saves", "pin", "nope"]).status.success());
}
