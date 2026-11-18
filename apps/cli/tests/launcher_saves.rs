use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const DATA: &str = "savegame@profile#NXDSMWW.BWW";
const INDEX: &str = "savegame@profile#QKRHMYXE";

fn root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("swofty-cli-launcher-saves-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(root.join("home")).unwrap();
    root
}

fn steam(root: &Path) -> PathBuf {
    let steam = root.join("steam");
    std::fs::create_dir_all(steam.join("steamapps")).unwrap();
    let remote = steam.join("userdata/843502855/3059520/remote");
    std::fs::create_dir_all(&remote).unwrap();
    std::fs::write(remote.join(DATA), [1u8; 900]).unwrap();
    std::fs::write(remote.join(INDEX), [7u8; 416]).unwrap();
    std::fs::write(remote.join("savegame@highlights001#QKRHMYXE"), b"replay").unwrap();
    std::fs::write(
        steam.join("userdata/843502855/3059520/remotecache.vdf"),
        "\"3059520\"\n{\n}\n",
    )
    .unwrap();
    remote
}

fn swofty(root: &Path, running: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_swofty"))
        .args(args)
        .env("SWOFTY_DATA_DIR", root.join("data"))
        .env("SWOFTY_STEAM_ROOT_ONLY", root.join("steam"))
        .env("SWOFTY_TEST_RUNNING", running)
        .env("HOME", root.join("home"))
        .env_remove("LOCALAPPDATA")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn ok(o: &Output) -> String {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn refused(o: &Output) -> String {
    assert!(!o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn store(root: &Path, profile: &str) -> PathBuf {
    root.join("data").join("saves").join(profile).join("remote")
}

#[test]
fn import_copies_the_steam_save_into_the_launcher_saves_and_leaves_steam_alone() {
    let root = root("import");
    let remote = steam(&root);
    let out = ok(&swofty(&root, "", &["saves", "import"]));
    assert!(out.contains("\nimported 2 file(s), 1316 bytes, from "), "{out}");
    let to = store(&root, "default");
    assert_eq!(std::fs::read(to.join(DATA)).unwrap(), vec![1u8; 900]);
    assert_eq!(std::fs::read(to.join(INDEX)).unwrap(), vec![7u8; 416]);
    assert!(!to.join("savegame@highlights001#QKRHMYXE").exists());
    assert_eq!(std::fs::read(remote.join(DATA)).unwrap(), vec![1u8; 900]);
    assert_eq!(
        std::fs::read_to_string(remote.parent().unwrap().join("remotecache.vdf")).unwrap(),
        "\"3059520\"\n{\n}\n"
    );

    let list = ok(&swofty(
        &root,
        "",
        &["saves", "list", "--source", "launcher-default"],
    ));
    assert!(
        list.contains("Imported from Steam saves, profile default, 2 file(s), 1316 bytes"),
        "{list}"
    );
    assert!(
        list.contains(&format!(
            "save {}: 2 file(s), 1316 bytes, Steam Cloud: local only",
            to.display()
        )),
        "{list}"
    );
}

#[test]
fn import_waits_for_the_game_to_close_and_keeps_what_the_store_held() {
    let root = root("running");
    steam(&root);
    let to = store(&root, "default");
    std::fs::create_dir_all(&to).unwrap();
    std::fs::write(to.join(DATA), b"old career").unwrap();
    let err = refused(&swofty(&root, "game", &["saves", "import"]));
    assert!(err.contains("F1 25 is running"), "{err}");
    assert_eq!(std::fs::read(to.join(DATA)).unwrap(), b"old career");

    ok(&swofty(&root, "steam", &["saves", "import"]));
    assert_eq!(std::fs::read(to.join(DATA)).unwrap(), vec![1u8; 900]);
    let list = ok(&swofty(
        &root,
        "",
        &["saves", "list", "--source", "launcher-default"],
    ));
    assert!(list.contains("before a restore"), "{list}");
    assert!(list.contains("pinned (before-import)"), "{list}");
}

#[test]
fn import_from_another_profile() {
    let root = root("profile");
    steam(&root);
    ok(&swofty(&root, "", &["saves", "import"]));
    std::fs::create_dir_all(root.join("data/profiles")).unwrap();
    std::fs::write(
        root.join("data/profiles/second.json"),
        r#"{"schema_version":2,"name":"second"}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("data/profiles/default.json"),
        r#"{"schema_version":2,"name":"default"}"#,
    )
    .unwrap();
    let out = ok(&swofty(
        &root,
        "",
        &[
            "saves",
            "import",
            "--from-profile",
            "default",
            "--profile",
            "second",
        ],
    ));
    assert!(out.contains("imported 2 file(s)"), "{out}");
    assert_eq!(
        std::fs::read(store(&root, "second").join(DATA)).unwrap(),
        vec![1u8; 900]
    );
    let list = ok(&swofty(
        &root,
        "",
        &["saves", "list", "--source", "launcher-second"],
    ));
    assert!(list.contains("Imported from another profile"), "{list}");
    let err = refused(&swofty(
        &root,
        "",
        &[
            "saves",
            "import",
            "--from-profile",
            "second",
            "--profile",
            "second",
        ],
    ));
    assert!(err.contains("is the profile to import into"), "{err}");
}

#[test]
fn vanilla_has_no_launcher_saves() {
    let root = root("vanilla");
    steam(&root);
    let err = refused(&swofty(&root, "", &["saves", "import", "--profile", "vanilla"]));
    assert!(err.contains("always uses Steam's saves"), "{err}");
}

#[test]
fn export_needs_steam_and_the_game_closed_and_spares_remotecache() {
    let root = root("export");
    let remote = steam(&root);
    let cache = remote.parent().unwrap().join("remotecache.vdf");
    let to = store(&root, "default");
    std::fs::create_dir_all(&to).unwrap();
    std::fs::write(to.join(DATA), b"launcher career").unwrap();
    std::fs::write(to.join(INDEX), b"index").unwrap();

    let err = refused(&swofty(&root, "steam", &["saves", "export", "--to", "steam"]));
    assert!(err.contains("Steam is running"), "{err}");
    let err = refused(&swofty(&root, "game", &["saves", "export", "--to", "steam"]));
    assert!(err.contains("F1 25 is running"), "{err}");
    assert_eq!(std::fs::read(remote.join(DATA)).unwrap(), vec![1u8; 900]);

    let out = ok(&swofty(&root, "", &["saves", "export", "--to", "steam"]));
    assert!(out.contains("the Steam save it replaced is snapshot "), "{out}");
    assert!(out.contains("keep the local files"), "{out}");
    assert_eq!(std::fs::read(remote.join(DATA)).unwrap(), b"launcher career");
    assert_eq!(std::fs::read(remote.join(INDEX)).unwrap(), b"index");
    assert_eq!(
        std::fs::read(remote.join("savegame@highlights001#QKRHMYXE")).unwrap(),
        b"replay"
    );
    assert_eq!(std::fs::read_to_string(&cache).unwrap(), "\"3059520\"\n{\n}\n");

    let list = ok(&swofty(
        &root,
        "",
        &["saves", "list", "--source", "steam-843502855"],
    ));
    assert!(list.contains("before a restore"), "{list}");
    let err = refused(&swofty(&root, "", &["saves", "export", "--to", "somewhere"]));
    assert!(err.contains("--to takes steam"), "{err}");
}

#[test]
fn export_of_an_empty_store_says_so() {
    let root = root("empty-export");
    steam(&root);
    let err = refused(&swofty(&root, "", &["saves", "export", "--to", "steam"]));
    assert!(err.contains("are empty; there is nothing to export"), "{err}");
}

#[test]
fn list_defaults_to_the_active_profiles_launcher_store() {
    let root = root("default-store");
    steam(&root);
    ok(&swofty(&root, "", &["saves", "import"]));
    let list = ok(&swofty(&root, "", &["saves", "list"]));
    let want = std::path::Path::new("career-vault").join("launcher-default");
    assert!(list.contains(&want.display().to_string()), "{list}");
    assert!(!list.contains("steam-843502855"), "{list}");
    std::fs::write(
        root.join("data").join("config.json"),
        r#"{"active_profile":"vanilla"}"#,
    )
    .unwrap();
    let list = ok(&swofty(&root, "", &["saves", "list"]));
    assert!(list.contains("steam-843502855"), "{list}");
}
