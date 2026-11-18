use std::path::{Path, PathBuf};

use swofty_core::launcher_saves;
use swofty_core::vault::Location;

pub(crate) fn awaiting_import(vanilla: bool, has_save: bool, steam_has_save: bool) -> bool {
    !vanilla && !has_save && steam_has_save
}

pub(crate) fn steam_save_missing(vanilla: bool, has_save: bool, steam_has_save: bool, pending: bool) -> bool {
    !vanilla && !has_save && !steam_has_save && pending
}

pub(crate) fn steam_save_in(folder: &Path) -> Option<Location> {
    let app = swofty_core::STEAM_APP_ID.to_string();
    let accounts = |dir: PathBuf| {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|u| u.path().join(&app).join("remote"))
            .collect::<Vec<_>>()
    };
    [
        folder.to_path_buf(),
        folder.join("remote"),
        folder.join(&app).join("remote"),
    ]
    .into_iter()
    .chain(accounts(folder.to_path_buf()))
    .chain(accounts(folder.join("userdata")))
    .filter_map(|d| Location::steam(&d))
    .find(launcher_saves::has_files)
}

pub(crate) fn source_of_profile(root: &Path, profile: &str) -> Option<String> {
    Location::source_of_profile(root, profile)
}
