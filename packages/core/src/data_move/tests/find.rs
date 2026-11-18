use super::*;
use crate::data_move::find::{Found, Kind as Source, candidates_on, looks_like_data};

fn data_with(at: &Path, addons: &[&str]) {
    for id in addons {
        write(
            &at.join("addons").join(id).join("swofty.json"),
            &format!("{{\"id\":\"{id}\",\"name\":\"Name of {id}\"}}"),
        );
    }
    write(&at.join("profiles/default.json"), "{}");
    write(&at.join("config.json"), "{}");
}

fn find(l: &Location, installs: &[PathBuf], drives: &[PathBuf]) -> Vec<Found> {
    candidates_on(l, installs, &["swofty.bundled".to_owned()], drives)
}

fn paths(found: &[Found]) -> Vec<PathBuf> {
    found.iter().map(|f| f.path.clone()).collect()
}

#[test]
fn finds_an_aside_folder_with_addons_when_the_default_is_empty() {
    let dir = temp("find-aside");
    let default = dir.join("Swofty");
    std::fs::create_dir_all(&default).unwrap();
    data_with(&dir.join("Swofty.old"), &["a.one", "a.two"]);
    data_with(&dir.join("Swofty.old-2"), &["a.one"]);
    data_with(&dir.join("Swofty.fresh-20261108-1010"), &["b.new"]);
    write(&dir.join("Swofty.old-3/notes.txt"), "nothing here");
    let found = find(&loc(&default), &[], &[]);
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found.iter().all(|f| f.kind == Source::Aside));
    let two = found.iter().find(|f| f.path.ends_with("Swofty.old")).unwrap();
    assert_eq!((two.addons, two.profiles), (2, 1));
    assert_eq!(two.names, ["Name of a.one", "Name of a.two"]);
    assert!(two.bytes > 0 && !two.bytes_partial);
}

#[test]
fn finds_data_beside_the_previous_install_and_in_the_install_root() {
    let dir = temp("find-install");
    let default = dir.join("Local/Swofty");
    std::fs::create_dir_all(&default).unwrap();
    let (old, shared) = (dir.join("D/Old Launcher"), dir.join("D/Shared"));
    data_with(&old.join("SwoftyData"), &["a.one"]);
    data_with(&shared, &["a.two"]);
    write(&shared.join("bin/swofty-launcher"), "x");
    let found = find(&loc(&default), &[old.clone(), shared.clone()], &[]);
    assert_eq!(paths(&found).len(), 2, "{found:?}");
    assert!(paths(&found).contains(&old.join("SwoftyData")));
    assert!(paths(&found).contains(&shared));
    assert!(found.iter().all(|f| f.kind == Source::OldInstall));
}

#[test]
fn finds_a_swofty_folder_on_another_drive_root_but_not_a_folder_without_addons() {
    let dir = temp("find-drive");
    let default = dir.join("Local/Swofty");
    std::fs::create_dir_all(&default).unwrap();
    let drive = dir.join("D");
    data_with(&drive.join("SwoftyData"), &["a.one"]);
    data_with(&drive.join("Games/swofty-stuff"), &["a.two"]);
    write(&drive.join("Swofty Launcher/bin/swofty-launcher"), "x");
    write(&drive.join("Swofty2/profiles/default.json"), "{}");
    data_with(&drive.join("Other/Swofty"), &["deep.one"]);
    let found = find(&loc(&default), &[], std::slice::from_ref(&drive));
    let got = paths(&found);
    assert_eq!(got.len(), 2, "{got:?}");
    assert!(got.contains(&drive.join("SwoftyData")));
    assert!(got.contains(&drive.join("Games/swofty-stuff")));
    assert!(found.iter().all(|f| f.kind == Source::DriveRoot));
}

#[test]
fn ignores_bundled_addons_when_counting() {
    let dir = temp("find-bundled");
    data_with(&dir.join("d"), &["swofty.bundled"]);
    let bundled = ["swofty.bundled".to_owned()];
    assert!(looks_like_data(&dir.join("d"), &bundled).is_none());
    data_with(&dir.join("d"), &["mine.one"]);
    let found = looks_like_data(&dir.join("d"), &bundled).unwrap();
    assert_eq!(
        (found.addons, found.names.as_slice()),
        (1, &["Name of mine.one".to_owned()][..])
    );
    write(&dir.join("e/addons/mine/swofty.json"), "{}");
    assert!(
        looks_like_data(&dir.join("e"), &bundled).is_none(),
        "no config, profiles or saves"
    );
    std::fs::create_dir_all(dir.join("e/saves")).unwrap();
    assert!(looks_like_data(&dir.join("e"), &bundled).is_some());
}

#[test]
fn ignores_the_current_data_folder_and_folders_with_fewer_addons() {
    let dir = temp("find-current");
    let default = dir.join("Local/Swofty");
    let current = dir.join("D/Mine");
    data_with(&current, &["a.one", "a.two"]);
    std::fs::create_dir_all(default.parent().unwrap()).unwrap();
    link::create(&current, &default).unwrap();
    data_with(&dir.join("Local/Swofty.old"), &["a.one"]);
    data_with(&dir.join("Local/Swofty.old-2"), &["a.one", "a.two", "a.three"]);
    let found = find(&loc(&default), &[], &[]);
    assert_eq!(paths(&found), [dir.join("Local/Swofty.old-2")]);
    assert!(found.iter().all(|f| !same_path(&f.path, &current)));
}

#[test]
fn a_folder_inside_or_around_the_current_one_is_not_offered() {
    let dir = temp("find-nested");
    let default = dir.join("Swofty");
    data_with(&default, &["a.one"]);
    data_with(&default.join("backup"), &["a.nine", "a.ten"]);
    assert!(find(&loc(&default), std::slice::from_ref(&dir), &[]).is_empty());
}

#[test]
fn history_file_records_every_move_and_survives_the_default_being_replaced() {
    let dir = temp("history");
    let default = dir.join("Swofty");
    seed(&default);
    let (one, two) = (dir.join("D/One"), dir.join("E/Two"));
    let p = plan(&loc(&default), &one, &SAME_VOLUME).unwrap();
    move_data(&p, &SAME_VOLUME, &Quiet, &no_cancel()).unwrap();
    let p = plan(&loc(&default), &two, &SAME_VOLUME).unwrap();
    move_data(&p, &SAME_VOLUME, &Quiet, &no_cancel()).unwrap();
    let kept: Vec<PathBuf> = history::entries(&default).into_iter().map(|e| e.path).collect();
    assert_eq!(kept, [default.clone(), one.clone()]);
    assert!(history::path(&default).starts_with(&dir));
    assert!(!history::path(&default).starts_with(&default));
    std::fs::remove_dir_all(&default).ok();
    std::fs::remove_file(&default).ok();
    std::fs::create_dir_all(&default).unwrap();
    assert_eq!(history::entries(&default).len(), 2);
}

#[test]
fn a_recorded_place_is_found_again() {
    let dir = temp("history-find");
    let default = dir.join("Swofty");
    std::fs::create_dir_all(&default).unwrap();
    let old = dir.join("Z/Anywhere/Name");
    data_with(&old, &["a.one"]);
    history::record(&default, &old, "moved the data folder");
    history::record(&default, &old, "again");
    assert_eq!(history::entries(&default).len(), 1);
    let found = find(&loc(&default), &[], &[]);
    assert_eq!(
        (paths(&found), found[0].kind),
        (vec![old], Source::PreviousLocation)
    );
}

#[cfg(windows)]
#[test]
fn a_junction_default_is_not_offered_as_its_own_target() {
    let dir = temp("find-junction");
    let (default, target) = (dir.join("Swofty"), dir.join("D/Data"));
    data_with(&target, &["a.one"]);
    link::create(&target, &default).unwrap();
    assert!(find(&loc(&default), &[], &[]).is_empty());
    let mut with_env = loc(&default);
    with_env.env = Some(dir.join("Elsewhere"));
    assert_eq!(paths(&find(&with_env, &[], &[])), [default]);
}
