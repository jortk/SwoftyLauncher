//! The guard against losing an addon career to a game that does not know its mode. F1 25 started
//! without the addons (Steam's Play button, with the game folder rolled back) drops the career
//! containers no loaded game mode claims when it saves, and Steam Cloud uploads that save. The
//! profile save is encrypted, so the launcher cannot list its containers; it reads the signs
//! instead: the newest save a launcher session left held addon careers in its career index, an F1 25
//! the launcher did not start has saved since, and the save is smaller than it was.

use super::careers::{self, Brief};
use super::{EventKind, Reason, Vault};

const DISMISSED: &str = "loss-dismissed";

/// Addon careers a save probably lost, and the version that still holds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loss {
    pub source: String,
    pub snapshot: String,
    pub at: i64,
    pub careers: Vec<Brief>,
}

impl Loss {
    pub fn names(&self) -> Vec<String> {
        self.careers
            .iter()
            .map(|b| match &b.place {
                Some(p) => format!("{}, {p}", b.name),
                None => b.name.clone(),
            })
            .collect()
    }
}

fn left_by_launcher(r: Reason) -> bool {
    matches!(r, Reason::AfterSession | Reason::BeforeSession | Reason::Restored)
}

fn left_outside(r: Reason) -> bool {
    matches!(r, Reason::ChangedOutside | Reason::ForeignExit)
}

impl Vault {
    pub fn probable_loss(&self) -> Option<Loss> {
        let snaps = self.list().ok()?;
        let newest = snaps.first()?;
        let last = snaps.iter().position(|s| left_by_launcher(s.reason))?;
        let good = &snaps[last];
        let touched = self
            .events()
            .iter()
            .filter(|e| matches!(e.kind, EventKind::Snapshot | EventKind::Unchanged))
            .filter(|e| e.reason.is_some_and(left_by_launcher))
            .map(|e| e.at)
            .max()
            .unwrap_or(good.at);
        if !snaps.iter().any(|s| left_outside(s.reason) && s.at > touched) {
            return None;
        }
        if newest.total() >= good.total() {
            return None;
        }
        if std::fs::read_to_string(self.dir.join(DISMISSED)).is_ok_and(|d| d.trim() == good.id) {
            return None;
        }
        let held = careers::addon_careers(&self.index_of(good)?, None);
        let known = self.careers();
        let careers: Vec<Brief> = held
            .into_iter()
            .filter(|b| {
                known
                    .find(&b.career)
                    .is_none_or(|r| !r.status.is_missing() && r.status != careers::Status::Archived)
            })
            .collect();
        (!careers.is_empty()).then(|| Loss {
            source: self.source.clone(),
            snapshot: good.id.clone(),
            at: good.at,
            careers,
        })
    }

    pub fn dismiss_loss(&self) -> crate::Result<()> {
        let _lock = self.lock()?;
        let Some(loss) = self.probable_loss() else {
            return Ok(());
        };
        let path = self.dir.join(DISMISSED);
        std::fs::write(&path, loss.snapshot).map_err(crate::io_err(&path))
    }
}

/// Whether a profile's Saves page warns that its addon careers live in the Steam save.
pub fn steam_save_warning(vanilla: bool, launcher_saves: bool, addon_careers: &[Brief]) -> bool {
    !vanilla && !launcher_saves && !addon_careers.is_empty()
}

pub const STEAM_SAVE_WARNING: &str = "This profile's addon careers live in your Steam save. Starting F1 25 from Steam without the launcher can remove them. Launcher saves keep them out of Steam's reach.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdlib::careers::parse;
    use crate::vault::{Layout, Location, Meta, Snapshot, Taken};

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vault-guard-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_index(data: &std::path::Path, json: &str) {
        let file = crate::stdlib::careers::data_path(data);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, json).unwrap();
    }

    const WITH: &str = r#"{"careers":[
        {"career":"driver_career:a1","slot":"a1","status":"present","mode":"driver_career"},
        {"career":"progressive:7f","slot":"7f","status":"present","mode":"gm_driver_career_progressive","label":"Progressive 2016","season":2016,"round":7,"addons":[{"id":"swofty.progressive"}]}
    ]}"#;

    struct World {
        data: std::path::PathBuf,
        loc: Location,
        vault: Vault,
    }

    fn world(name: &str) -> World {
        let data = dir(name);
        let remote = data
            .join("steam")
            .join("userdata")
            .join("843502855")
            .join("3059520")
            .join("remote");
        std::fs::create_dir_all(&remote).unwrap();
        std::fs::write(remote.join("savegame@profile#NXDSMWW.BWW"), vec![1u8; 4000]).unwrap();
        std::fs::write(remote.join("savegame@profile#QKRHMYXE"), vec![7u8; 416]).unwrap();
        let loc = Location::steam(&remote).unwrap();
        assert_eq!(loc.layout, Layout::Steam);
        let vault = Vault::of(&data, &loc).unwrap();
        World { data, loc, vault }
    }

    fn save(w: &World, bytes: usize) {
        std::fs::write(
            w.loc.dir.join("savegame@profile#NXDSMWW.BWW"),
            vec![(bytes % 251) as u8; bytes],
        )
        .unwrap();
    }

    fn take(w: &World, reason: Reason, at: i64) -> Snapshot {
        match w.vault.snapshot(&w.loc, Meta::new(reason), at).unwrap() {
            Taken::New(s) | Taken::Unchanged(s) => s,
            Taken::NoSave => panic!("no save"),
        }
    }

    #[test]
    fn a_save_that_shrank_after_a_foreign_run_loses_the_addon_career_of_the_last_session() {
        let w = world("lost");
        write_index(&w.data, WITH);
        let good = take(&w, Reason::AfterSession, 1_000);
        assert!(w.vault.probable_loss().is_none());
        save(&w, 3000);
        take(&w, Reason::ForeignExit, 2_000);
        let loss = w.vault.probable_loss().unwrap();
        assert_eq!(loss.snapshot, good.id);
        assert_eq!(loss.at, 1_000);
        assert_eq!(loss.names(), ["Progressive 2016, season 2016, round 7"]);
        std::fs::remove_dir_all(&w.data).ok();
    }

    #[test]
    fn a_launcher_start_that_finds_a_smaller_save_counts_as_outside() {
        let w = world("start");
        write_index(&w.data, WITH);
        take(&w, Reason::AfterSession, 1_000);
        save(&w, 100);
        w.vault
            .check(&w.loc, Meta::new(Reason::LauncherStart), 2_000)
            .unwrap();
        assert_eq!(w.vault.probable_loss().unwrap().careers.len(), 1);
        std::fs::remove_dir_all(&w.data).ok();
    }

    #[test]
    fn nothing_is_lost_without_addon_careers_a_foreign_run_or_a_smaller_save() {
        let w = world("quiet");
        write_index(
            &w.data,
            r#"{"careers":[{"career":"driver_career:a1","slot":"a1","status":"present","mode":"driver_career"}]}"#,
        );
        take(&w, Reason::AfterSession, 1_000);
        save(&w, 3000);
        take(&w, Reason::ForeignExit, 2_000);
        assert!(w.vault.probable_loss().is_none());

        let w2 = world("grew");
        write_index(&w2.data, WITH);
        take(&w2, Reason::AfterSession, 1_000);
        save(&w2, 5000);
        take(&w2, Reason::ForeignExit, 2_000);
        assert!(w2.vault.probable_loss().is_none());

        let w3 = world("inside");
        write_index(&w3.data, WITH);
        take(&w3, Reason::BeforeSession, 1_000);
        save(&w3, 3000);
        take(&w3, Reason::AfterSession, 2_000);
        assert!(w3.vault.probable_loss().is_none());
        for w in [w, w2, w3] {
            std::fs::remove_dir_all(&w.data).ok();
        }
    }

    #[test]
    fn a_restore_or_a_later_session_clears_it_and_a_dismissal_holds() {
        let w = world("clear");
        write_index(&w.data, WITH);
        take(&w, Reason::AfterSession, 1_000);
        save(&w, 3000);
        take(&w, Reason::ForeignExit, 2_000);
        assert!(w.vault.probable_loss().is_some());
        w.vault.dismiss_loss().unwrap();
        assert!(w.vault.probable_loss().is_none());
        save(&w, 2000);
        take(&w, Reason::ForeignExit, 2_500);
        assert!(w.vault.probable_loss().is_none());
        take(&w, Reason::AfterSession, 3_000);
        assert!(w.vault.probable_loss().is_none());
        std::fs::remove_dir_all(&w.data).ok();

        let w = world("session");
        write_index(&w.data, WITH);
        take(&w, Reason::AfterSession, 1_000);
        save(&w, 3000);
        take(&w, Reason::ForeignExit, 2_000);
        assert!(w.vault.probable_loss().is_some());
        take(&w, Reason::BeforeSession, 3_000);
        assert!(w.vault.probable_loss().is_none());
        std::fs::remove_dir_all(&w.data).ok();
    }

    #[test]
    fn a_career_the_table_already_calls_missing_is_not_named_twice() {
        let w = world("known");
        write_index(&w.data, WITH);
        take(&w, Reason::AfterSession, 1_000);
        save(&w, 3000);
        write_index(
            &w.data,
            r#"{"careers":[{"career":"driver_career:a1","slot":"a1","status":"present","mode":"driver_career"}]}"#,
        );
        take(&w, Reason::ForeignExit, 2_000);
        assert!(w.vault.careers().missing().next().is_some());
        assert!(w.vault.probable_loss().is_none());
        std::fs::remove_dir_all(&w.data).ok();
    }

    #[test]
    fn the_warning_needs_launcher_saves_off_and_an_addon_career() {
        let held = careers::addon_careers(&parse(WITH), None);
        assert_eq!(held.len(), 1);
        assert!(steam_save_warning(false, false, &held));
        assert!(!steam_save_warning(false, true, &held));
        assert!(!steam_save_warning(true, false, &held));
        assert!(!steam_save_warning(false, false, &[]));
        assert!(careers::addon_careers(&parse(WITH), Some("other")).len() == 1);
    }
}
