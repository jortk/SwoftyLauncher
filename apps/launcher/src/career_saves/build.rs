use std::path::Path;

use swofty_core::install::Game;
use swofty_core::launcher_saves;
use swofty_core::profile::VANILLA;
use swofty_core::store::Store;
use swofty_core::vault::career_names;
use swofty_core::vault::careers::{self};
use swofty_core::vault::{self, Location, Vault};
use swofty_i18n::tr;

use super::detail::{DETAILS, details, saves_print};
use super::labels::{imported_from, season_round};
use super::state::{Built, Entry, Imported, SavesPage, SavesView, Version};
use super::steam::{awaiting_import, steam_save_missing};

pub(super) fn card_careers(root: &Path, store: &Store, game: Option<&Game>, profile: &str) -> Vec<Entry> {
    let listing = match game {
        Some(g) => career_names::read_named_listing(root, store, g, Some(profile)),
        None => careers::read_listing(root, Some(profile)),
    };
    listing
        .into_iter()
        .filter(|l| !l.hidden)
        .filter(|l| l.profile.as_deref().is_none_or(|p| p == profile))
        .map(|l| Entry {
            place: season_round(&l),
            l,
        })
        .collect()
}

pub(super) fn build_saves(
    store: &Store,
    install: Option<&crate::game::Install>,
    name: &str,
    game_running: bool,
) -> Built {
    let started = std::time::Instant::now();
    let built = build_saves_untimed(store, install, name, game_running);
    crate::app_log::note(&format!(
        "saves: page loaded in {} ms ({} snapshots)",
        started.elapsed().as_millis(),
        built.view.versions.len()
    ));
    built
}

pub(super) fn build_saves_untimed(
    store: &Store,
    install: Option<&crate::game::Install>,
    name: &str,
    game_running: bool,
) -> Built {
    let print = saves_print(&store.root, name, game_running);
    let root = store.root.clone();
    let name = name.to_owned();
    let banner = SavesPage::banner_for(&root, &name);
    let active_loc = Location::active(&root);
    let vanilla = name == VANILLA;
    let loc = Location::of_profile(&root, &name);
    let own_store = loc
        .as_ref()
        .is_some_and(|l| matches!(l.layout, vault::Layout::Launcher | vault::Layout::LauncherEa));
    let vault = loc.as_ref().and_then(|l| Vault::of(&root, l).ok());
    let game = crate::sections::game(store, install);
    let entries = card_careers(&root, store, game.as_ref(), &name);
    let snaps = vault.as_ref().and_then(|v| v.list().ok()).unwrap_or_default();
    let has_save = loc.as_ref().is_some_and(launcher_saves::has_files);
    let imported = (has_save && entries.is_empty() && (vanilla || own_store)).then(|| match vanilla {
        true => Imported {
            title: tr("Your Steam save").to_owned(),
            at: None,
        },
        false => imported_from(&snaps),
    });
    let (versions, dir, usage, cap) = match vault {
        Some(v) => {
            let found = details(
                &v,
                &snaps,
                &swofty_core::stdlib::careers::data_path(&root),
                &DETAILS,
            );
            let versions = snaps
                .into_iter()
                .zip(found)
                .map(|(snap, d)| Version {
                    snap,
                    held: d.held,
                    diff: d.diff,
                    newest: d.newest,
                    missing: d.missing,
                })
                .collect();
            (versions, v.dir.clone(), v.usage(), v.policy.cap_bytes)
        }
        None => (Vec::new(), root.clone(), 0, 0),
    };
    let others = store
        .profile_names()
        .unwrap_or_default()
        .into_iter()
        .filter(|n| n != VANILLA && *n != name)
        .collect();
    let steam = active_loc.filter(|l| l.layout == vault::Layout::Steam);
    let awaiting_import = awaiting_import(
        vanilla,
        has_save,
        steam.as_ref().is_some_and(launcher_saves::has_files),
    );
    let steam_missing = steam_save_missing(
        vanilla,
        has_save,
        steam.as_ref().is_some_and(launcher_saves::has_files),
        launcher_saves::import_pending(&root),
    ) && launcher_saves::steam_save_pending(&root, &name);
    let steam_running = steam.as_ref().is_some_and(|_| {
        swofty_core::steam_client::status().state != swofty_core::steam_client::State::Stopped
    });
    let steam_warning = store.profile(&name).is_ok_and(|p| {
        swofty_core::vault::guard::steam_save_warning(
            vanilla,
            p.launcher_saves,
            &careers::addon_careers(
                &swofty_core::stdlib::careers::read(&swofty_core::stdlib::careers::data_path(&root)),
                Some(&name),
            ),
        )
    });
    let view = SavesView {
        steam,
        profile: name,
        loc,
        entries,
        imported,
        has_save,
        others,
        versions,
        dir,
        usage,
        cap,
        steam_running,
        awaiting_import,
        steam_missing,
        steam_warning,
    };
    Built {
        print,
        banner,
        running: game_running,
        view,
    }
}
