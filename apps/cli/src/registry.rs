//! `swofty registry` and `swofty careers`: the 1.6 standard library's
//! dependency check, the core's last conflicts report and status lines, the
//! stat ids an addon's added rows keep, and the career index.

use std::collections::BTreeMap;
use std::process::ExitCode;

use anyhow::Result;
use clap::Subcommand;
use swofty_core::ops::{IdFrom, UNLOCK_FRESH_FROM, is_unlock_ids};
use swofty_core::row_ids;
use swofty_core::stdlib::deps::{self, Level};
use swofty_core::stdlib::{careers, reports};
use swofty_core::store::Store;

#[derive(Subcommand)]
pub enum RegistryCommand {
    #[command(
        about = "Check the profile's addon dependencies (registry.json, registry.lock.json) as Play does; exits 3 when Play would stop"
    )]
    Check,
    #[command(about = "Show the core's conflicts report from your last session")]
    Conflicts,
    #[command(about = "Show what didn't apply for each addon in your last session")]
    Status,
    #[command(about = "List the stat ids an addon's added rows keep for careers, and where each came from")]
    RowIds {
        #[arg(help = "The addon's id")]
        addon: String,
    },
    #[command(
        about = "Show each box's track list from your last session: its calendar, more tracks and what it leaves out"
    )]
    Tracks,
}

fn game_dir(store: &Store) -> Option<std::path::PathBuf> {
    crate::build::game(store, None).ok().map(|g| g.dir)
}

pub fn command(store: &Store, profile: &str, cmd: RegistryCommand) -> Result<ExitCode> {
    match cmd {
        RegistryCommand::Check => check(store, profile),
        RegistryCommand::Conflicts => {
            conflicts(store);
            Ok(ExitCode::SUCCESS)
        }
        RegistryCommand::Status => {
            let lines = status(store);
            if lines.is_empty() {
                println!("nothing to report from your last session");
            }
            let installed = store.addons()?;
            for (id, lines) in lines {
                let name = installed.get(&id).map_or(id.as_str(), |m| m.name.as_str());
                for l in lines {
                    println!("{name:30} {l}");
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        RegistryCommand::RowIds { addon } => {
            for line in row_id_lines(&store.root, &addon)? {
                println!("{line}");
            }
            Ok(ExitCode::SUCCESS)
        }
        RegistryCommand::Tracks => {
            for l in track_lines(store, profile) {
                println!("{l}");
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// Each row of `addon` with a stat id kept in `data`'s row-ids.json: its table and label, its id,
/// and where the id came from.
pub fn row_id_lines(data: &std::path::Path, addon: &str) -> Result<Vec<String>> {
    let rows = row_ids::kept_rows(data, addon)?;
    if rows.is_empty() {
        return Ok(vec![format!(
            "{addon}: no added rows with a kept stat id; build a profile with it first"
        )]);
    }
    let names: Vec<String> = rows.iter().map(|r| format!("{}/{}", r.table, r.label)).collect();
    let width = names.iter().map(String::len).max().unwrap_or(0);
    let mut out = vec![format!(
        "{addon}: {} added row(s) keep these stat ids",
        rows.len()
    )];
    let unlock_next =
        format!("the lowest free id from {UNLOCK_FRESH_FROM} in its unlock list when it was added");
    for (name, r) in names.iter().zip(&rows) {
        let from = match r.from {
            IdFrom::Seeded => "seeded from a profile's last build or a game session from before 1.6.10",
            IdFrom::Hashed => "hashed from the row's key by 1.6.10 or 1.6.11",
            IdFrom::Next if is_unlock_ids(&r.table) => unlock_next.as_str(),
            IdFrom::Next => "the next free id in load order when it was added",
            IdFrom::Moved => "the next free id after its kept id was taken",
            IdFrom::Reused => "reused from an addon that was off or removed",
        };
        out.push(format!("  {name:width$}  {:>5}  kept, {from}", r.id));
    }
    Ok(out)
}

/// The box track lists of the core's last `catalog.json` for `profile`, a few lines per box and mode.
fn track_lines(store: &Store, profile: &str) -> Vec<String> {
    let game = game_dir(store);
    let Some(catalog) = reports::read_catalog(&store.root, game.as_deref()) else {
        return vec![format!(
            "no box track lists yet: the core writes them in {} when you pick a box",
            reports::path(&store.root, reports::CATALOG).display()
        )];
    };
    catalog_lines(&catalog, profile)
}

fn catalog_lines(catalog: &reports::Catalog, profile: &str) -> Vec<String> {
    if !catalog.is_for(profile) {
        return vec![format!(
            "no box track lists for profile {profile} yet: catalog.json holds the boxes of profile {}; the core rewrites it when you pick a box in a session of {profile}",
            catalog.profile.as_deref().unwrap_or_default()
        )];
    }
    let header = match (&catalog.profile, &catalog.build) {
        (Some(p), Some(b)) => format!("as of the last session of profile {p} (build {b}):"),
        (Some(p), None) => format!("as of the last session of profile {p}:"),
        _ => "as of your last session (an older core wrote these lists; they can include boxes from earlier sessions and other profiles):".to_owned(),
    };
    box_lines(header, &catalog.boxes)
}

fn box_lines(header: String, boxes: &reports::BoxTracks) -> Vec<String> {
    let mut out = vec![header];
    for (id, modes) in boxes {
        for (mode, list) in modes {
            out.push(format!(
                "box {id} ({}): calendar {}, more tracks {}, left out {}",
                mode.replace('_', " "),
                list.calendar.len(),
                list.more.len(),
                list.left_out.len()
            ));
            if !list.calendar.is_empty() {
                out.push(format!("  calendar: {}", list.calendar.join(", ")));
            }
            if !list.more.is_empty() {
                out.push(format!("  more tracks: {}", list.more.join(", ")));
            }
            for l in &list.left_out {
                out.push(format!("  left out {}: {}", l.track, l.reason));
            }
        }
    }
    out
}

/// The profile's dependency check, as the build runs it before Play.
pub fn report(store: &Store, profile: &str) -> Result<deps::Report> {
    let p = store.profile(profile)?;
    let installed = store.addons()?;
    if p.is_vanilla() {
        return Ok(deps::Report::default());
    }
    let order = swofty_core::resolver::resolve(&p, &installed)?.order;
    let all = deps::read_all(store, &installed);
    Ok(deps::for_order(profile, &order, &installed, &all))
}

fn check(store: &Store, profile: &str) -> Result<ExitCode> {
    let r = report(store, profile)?;
    if r.is_empty() {
        println!("profile {profile}: every dependency is met");
    }
    for f in &r.findings {
        let tag = match f.level {
            Level::Stop => "stops Play",
            Level::Warn => "warning",
            Level::Hint => "hint",
        };
        println!("{tag}: {}", f.block());
    }
    for n in &r.notes {
        println!("note: {n}");
    }
    Ok(match r.stops() {
        true => ExitCode::from(deps::STOP_EXIT),
        false => ExitCode::SUCCESS,
    })
}

fn conflicts(store: &Store) {
    let game = game_dir(store);
    let Some(c) = reports::Conflicts::read(&store.root, game.as_deref()) else {
        println!(
            "no conflicts report yet: the core writes {} at each session load",
            reports::path(&store.root, reports::CONFLICTS).display()
        );
        return;
    };
    let installed = store.addons().unwrap_or_default();
    let name = |id: &str| {
        installed
            .get(id)
            .map_or_else(|| id.to_owned(), |m| m.name.clone())
    };
    if c.registry.is_empty() {
        println!("as of your last session: no registry conflicts");
    } else {
        println!("as of your last session:");
    }
    for e in &c.registry {
        println!("  {}", e.line(&name));
    }
}

/// The status lines of every addon from the last session's reports.
pub fn status(store: &Store) -> BTreeMap<String, Vec<String>> {
    let game = game_dir(store);
    reports::status_lines(
        reports::Conflicts::read(&store.root, game.as_deref()).as_ref(),
        reports::Room::read(&store.root, game.as_deref()).as_ref(),
    )
}

pub fn careers(store: &Store, profile: &str) -> Result<()> {
    let list = careers::sync(&store.root, game_dir(store).as_deref(), None)?;
    if list.is_empty() {
        println!("no careers in the index yet: the core lists each career it creates or loads");
        return Ok(());
    }
    let installed = store.addons()?;
    let p = store.profile(profile)?;
    let order = match p.is_vanilla() {
        true => Vec::new(),
        false => swofty_core::resolver::resolve(&p, &installed)
            .map(|r| r.order)
            .unwrap_or_default(),
    };
    for c in &list {
        let started = c.profile.as_deref().unwrap_or("unknown profile");
        let label = if c.label.is_empty() {
            c.career.as_str()
        } else {
            c.label.as_str()
        };
        println!("{label} [{}] (started in {started})", c.career);
        let missing = careers::missing(c, &order, &installed);
        if missing.is_empty() {
            println!("  loads in profile {profile}");
        }
        for m in missing {
            println!("  won't load in profile {profile}: {m}");
        }
    }
    Ok(())
}

/// The careers that need `id`, for the warning before it's turned off.
pub fn careers_needing(store: &Store, id: &str) -> Vec<careers::Career> {
    let list = careers::sync(&store.root, game_dir(store).as_deref(), None).unwrap_or_default();
    careers::needing(&list, id).into_iter().cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_ids_lists_each_row_with_its_id_and_where_it_came_from() {
        let data = std::env::temp_dir().join(format!("swofty-cli-row-ids-{}", std::process::id()));
        std::fs::remove_dir_all(&data).ok();
        let path = row_ids::path(&data);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"version":1,"rows":{
                "mod.cars/entrants.vehicles.vehicle/car_a":{"id":512,"from":"seeded"},
                "mod.cars/entrants.vehicles.vehicle/car_b":{"id":77,"from":"hashed"},
                "mod.other/entrants.vehicles.vehicle/car_c":{"id":600,"from":"hashed"}}}"#,
        )
        .unwrap();
        assert_eq!(
            row_id_lines(&data, "mod.cars").unwrap(),
            [
                "mod.cars: 2 added row(s) keep these stat ids",
                "  entrants.vehicles.vehicle/car_a    512  kept, seeded from a profile's last build or a game session from before 1.6.10",
                "  entrants.vehicles.vehicle/car_b     77  kept, hashed from the row's key by 1.6.10 or 1.6.11",
            ]
        );
        assert_eq!(
            row_id_lines(&data, "mod.none").unwrap(),
            ["mod.none: no added rows with a kept stat id; build a profile with it first"]
        );
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn box_lines_list_each_box_and_mode() {
        let text = r#"{ "tracks": { "game/f1_2026": {
            "grand_prix": { "calendar": ["game/melbourne"], "more": ["mod.okayama/okayama"],
                            "left_out": [ { "track": "mod.x/x", "reason": "no Time Trial support" } ] },
            "time_trial": { "calendar": [], "more": [], "left_out": [] } } } }"#;
        let boxes = reports::box_tracks(text).unwrap();
        assert_eq!(
            box_lines("as of your last session:".to_owned(), &boxes),
            [
                "as of your last session:",
                "box game/f1_2026 (grand prix): calendar 1, more tracks 1, left out 1",
                "  calendar: game/melbourne",
                "  more tracks: mod.okayama/okayama",
                "  left out mod.x/x: no Time Trial support",
                "box game/f1_2026 (time trial): calendar 0, more tracks 0, left out 0",
            ]
        );
    }

    #[test]
    fn track_lines_show_only_the_profiles_build() {
        let text = r#"{ "profile": "Classic Seasons", "build": "9d7a5f9177e1436d",
            "tracks": { "swofty.classic-seasons-2017/f1_2017": { "time_trial": { "calendar": ["game/monaco"] } } } }"#;
        let c = reports::catalog(text).unwrap();
        assert_eq!(
            catalog_lines(&c, "Classic Seasons"),
            [
                "as of the last session of profile Classic Seasons (build 9d7a5f9177e1436d):",
                "box swofty.classic-seasons-2017/f1_2017 (time trial): calendar 1, more tracks 0, left out 0",
                "  calendar: game/monaco",
            ]
        );
        assert_eq!(
            catalog_lines(&c, "Tails what-if"),
            [
                "no box track lists for profile Tails what-if yet: catalog.json holds the boxes of profile Classic Seasons; the core rewrites it when you pick a box in a session of Tails what-if"
            ]
        );
        let old = reports::catalog(r#"{ "tracks": { "game/bdx_s5_2012": { "grand_prix": {} } } }"#).unwrap();
        let lines = catalog_lines(&old, "Classic Seasons");
        assert!(lines[0].starts_with("as of your last session (an older core wrote these lists"));
        assert_eq!(
            lines[1],
            "box game/bdx_s5_2012 (grand prix): calendar 0, more tracks 0, left out 0"
        );
    }
}
