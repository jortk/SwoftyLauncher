use super::*;

const BUNDLED: &[String] = &[];

fn data_with(at: &Path, id: &str) {
    write(
        &at.join("addons").join(id).join("swofty.json"),
        &format!("{{\"id\":\"{id}\"}}"),
    );
    write(&at.join("profiles/default.json"), "{}");
    write(&at.join("config.json"), "{}");
}

fn setup(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = temp(name);
    let (default, found) = (dir.join("Local/Swofty"), dir.join("D/Old"));
    data_with(&found, "mine.progressive");
    write(&default.join("config.json"), "{\"fresh\":true}");
    (dir, default, found)
}

fn fresh_of(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir.join("Local"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().contains(".fresh-"))
        .collect()
}

fn adopt_it(l: &Location, found: &Path) -> Result<Adoption, Error> {
    adopt(l, found, &[], BUNDLED)
}

#[test]
fn adopt_sets_the_fresh_folder_aside_and_points_the_default_at_the_found_one() {
    let (dir, default, found) = setup("adopt-basic");
    write(
        &default.join("addons/new.one/swofty.json"),
        "{\"id\":\"new.one\",\"name\":\"New One\"}",
    );
    let done = adopt_it(&loc(&default), &found).unwrap();
    assert!(!done.moves_out);
    assert_eq!(done.kept, ["New One"]);
    assert!(journal_left(&default));
    assert!(
        link::target(&default).is_none(),
        "nothing changes before the restart"
    );
    let recovered = recover(&default, &SAME_VOLUME).unwrap().unwrap();
    assert_eq!(recovered.to, found);
    assert!(!journal_left(&default));
    assert!(same_path(&link::target(&default).unwrap(), &found));
    assert!(default.join("addons/mine.progressive/swofty.json").is_file());
    let fresh = fresh_of(&dir);
    assert_eq!(fresh.len(), 1, "{fresh:?}");
    assert_eq!(Some(&fresh[0]), done.fresh.as_ref());
    assert_eq!(read(&fresh[0].join("config.json")), "{\"fresh\":true}");
    assert!(
        history::entries(&default)
            .iter()
            .any(|e| same_path(&e.path, &found))
    );
}

#[test]
fn adopt_replaces_a_link_to_an_empty_folder_and_leaves_that_folder() {
    let (dir, default, found) = setup("adopt-link");
    std::fs::remove_dir_all(&default).unwrap();
    let empty = dir.join("E/Empty");
    write(&empty.join("config.json"), "{}");
    link::create(&empty, &default).unwrap();
    let done = adopt_it(&loc(&default), &found).unwrap();
    assert_eq!(done.fresh, None);
    recover(&default, &SAME_VOLUME).unwrap().unwrap();
    assert!(same_path(&link::target(&default).unwrap(), &found));
    assert!(empty.join("config.json").is_file());
}

#[test]
fn adopt_rolls_back_when_the_link_cannot_be_made() {
    let (dir, default, found) = setup("adopt-rollback");
    adopt_it(&loc(&default), &found).unwrap();
    let failing = Ops {
        link: |_, _| Err(std::io::Error::from(ErrorKind::PermissionDenied)),
        ..SAME_VOLUME
    };
    let err = recover(&default, &failing).unwrap().unwrap_err();
    assert!(err.to_string().contains("Could not point"), "{err}");
    assert!(!journal_left(&default));
    assert_eq!(read(&default.join("config.json")), "{\"fresh\":true}");
    assert!(fresh_of(&dir).is_empty());
    data_found_intact(&found);
}

fn data_found_intact(found: &Path) {
    assert!(found.join("addons/mine.progressive/swofty.json").is_file());
    assert!(found.join("profiles/default.json").is_file());
}

#[test]
fn adopt_finishes_after_a_crash_at_every_journal_step() {
    let (dir, default, found) = setup("adopt-crash");
    let done = adopt_it(&loc(&default), &found).unwrap();
    let fresh = done.fresh.unwrap();
    recover(&default, &SAME_VOLUME).unwrap().unwrap();
    for step in 0..3 {
        let journal = Journal {
            from: default.clone(),
            to: found.clone(),
            default: default.clone(),
            linked: false,
            aside: fresh.clone(),
            to_existed: false,
            kind: Kind::Adopt,
        };
        link::remove(&default).ok();
        if step == 0 {
            std::fs::rename(&fresh, &default).unwrap();
        }
        journal.save().unwrap();
        let recovered = recover(&default, &SAME_VOLUME).unwrap();
        assert!(recovered.is_ok(), "step {step}: {recovered:?}");
        assert!(same_path(&link::target(&default).unwrap(), &found), "step {step}");
        assert_eq!(
            read(&fresh.join("config.json")),
            "{\"fresh\":true}",
            "step {step}"
        );
        data_found_intact(&found);
        std::fs::remove_file(history::path(&default)).ok();
    }
    assert!(dir.exists());
}

#[test]
fn adopt_inside_the_install_folder_moves_out_instead_of_linking() {
    let (dir, default, _) = setup("adopt-relocate");
    let install = dir.join("D/Launcher");
    write(&install.join("bin/swofty-launcher"), "x");
    let found = install.join("Swofty");
    data_with(&found, "mine.progressive");
    let mut l = loc(&default);
    l.install = Some(install.clone());
    let done = adopt(&l, &found, &[], BUNDLED).unwrap();
    assert!(done.moves_out);
    let recovered = recover(&default, &SAME_VOLUME).unwrap().unwrap();
    assert_eq!(recovered.to, default);
    assert!(link::target(&default).is_none());
    assert!(default.join("addons/mine.progressive/swofty.json").is_file());
    assert!(!found.exists());
    assert_eq!(fresh_of(&dir).len(), 1);
    assert!(install.join("bin/swofty-launcher").is_file());
}

#[test]
fn adopt_across_drives_copies_out_of_the_install_and_checks_it() {
    let (dir, default, _) = setup("adopt-relocate-copy");
    let install = dir.join("D/Launcher");
    let found = install.join("Swofty");
    data_with(&found, "mine.progressive");
    write(&found.join("saves/career/one.bin"), &"y".repeat(2 << 20));
    let mut l = loc(&default);
    l.install = Some(install);
    adopt(&l, &found, &[], BUNDLED).unwrap();
    recover(&default, &OTHER_DRIVE).unwrap().unwrap();
    assert_eq!(read(&default.join("saves/career/one.bin")).len(), 2 << 20);
    assert!(!found.exists());
    assert_eq!(fresh_of(&dir).len(), 1);
}

#[test]
fn adopt_refuses_with_the_env_variable_set() {
    let (dir, default, found) = setup("adopt-env");
    let mut l = loc(&default);
    l.env = Some(dir.join("E"));
    assert_eq!(adopt_it(&l, &found), Err(Error::EnvSet(dir.join("E"))));
    assert!(!journal_left(&default));
}

#[test]
fn adopt_refuses_a_folder_without_addons_the_current_folder_and_a_pending_move() {
    let (dir, default, found) = setup("adopt-refuse");
    let empty = dir.join("D/Empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert!(matches!(
        adopt_it(&loc(&default), &empty),
        Err(Error::NoAddons(_))
    ));
    data_with(&default, "mine.here");
    assert!(matches!(
        adopt_it(&loc(&default), &default),
        Err(Error::AlreadyThere(_))
    ));
    assert!(matches!(
        adopt_it(&loc(&default), Path::new("relative")),
        Err(Error::NotAbsolute)
    ));
    adopt_it(&loc(&default), &found).unwrap();
    assert!(matches!(adopt_it(&loc(&default), &found), Err(Error::Io(_))));
}

#[test]
fn never_deletes_the_fresh_folder() {
    let (dir, default, found) = setup("adopt-never-deletes");
    write(&default.join("profiles/mine.json"), "{\"p\":1}");
    adopt_it(&loc(&default), &found).unwrap();
    recover(&default, &SAME_VOLUME).unwrap().unwrap();
    let fresh = fresh_of(&dir);
    assert_eq!(read(&fresh[0].join("profiles/mine.json")), "{\"p\":1}");
    temp_second(&dir, &default).unwrap();
    let after = fresh_of(&dir);
    assert_eq!(after, fresh);
    assert_eq!(read(&after[0].join("profiles/mine.json")), "{\"p\":1}");
    assert!(same_path(&link::target(&default).unwrap(), &dir.join("E/Other")));
}

fn temp_second(dir: &Path, default: &Path) -> Result<(), Error> {
    let second = dir.join("E/Other");
    data_with(&second, "mine.other");
    let l = loc(default);
    adopt_it(&l, &second)?;
    recover(default, &SAME_VOLUME).unwrap().map(|_| ())
}

#[test]
fn a_failed_link_never_removes_the_found_folder() {
    let (_dir, default, found) = setup("adopt-keeps-found");
    adopt_it(&loc(&default), &found).unwrap();
    let failing = Ops {
        link: |_, _| Err(std::io::Error::from(ErrorKind::PermissionDenied)),
        ..OTHER_DRIVE
    };
    recover(&default, &failing).unwrap().unwrap_err();
    data_found_intact(&found);
}
