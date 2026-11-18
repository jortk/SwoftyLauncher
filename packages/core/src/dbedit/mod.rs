//! The DB Editor: the stock game database named and browsable, a draft of
//! edits saved as data addons, and the friendly modules the launcher draws.

pub mod calendar;
mod catalog;
pub mod deps;
pub mod draft;
pub mod driver_manager;
mod edits;
mod ego;
mod modules;
pub mod package;
pub mod rows;
mod schema;
mod search;

pub use catalog::{Catalog, FieldMeta, Node, NodeId, NodeKind, RecordView, UNNAMED};
pub use draft::Draft;
pub use edits::{Edit, Edits, ImportReport, Origin, bool_text, validate, validate_like};
pub use modules::{
    Backing, Cell, Column, FieldSpec, Kind, LinkSpec, Module, ModuleRow, ModuleView, NameSpec, OptionSpec,
    RefSpec, RowSpec, Scope, module, modules,
};
pub use package::{GameExport, Imported, Incoming, Package, PackageInfo};
pub use rows::{Copied, RowCopy};
pub use search::{Hit, HitKind, Target};

use std::path::PathBuf;

use crate::baseline::DepotHashes;
use crate::install::Game;
use crate::store::Store;
use crate::{Error, Result};

/// The stock game database: the launcher's baseline copy when it has one,
/// else the game folder's file, refused when Steam's manifest says it was
/// changed (a database mod copied in by hand).
pub fn stock_database(store: &Store, game: &Game) -> Result<PathBuf> {
    let depots = crate::steam::find_game()
        .filter(|g| g.dir == game.dir)
        .and_then(|g| crate::stock::Depots::load(&g).ok());
    stock_database_with(store, game, depots.as_ref().map(|d| d as &dyn DepotHashes))
}

fn stock_database_with(store: &Store, game: &Game, depots: Option<&dyn DepotHashes>) -> Result<PathBuf> {
    let path = crate::build::original(&store.root, game, crate::build::GVDB);
    if path != game.dir.join(crate::build::GVDB) {
        return Ok(path);
    }
    match depots.map(|d| d.stock(crate::build::GVDB, &path)).transpose()? {
        Some(Some(false)) => Err(Error::Other(format!(
            "{} is not the copy Steam installed; verify the game files in Steam (F1 25 > Properties > \
             Installed Files > Verify integrity of game files), then open the DB Editor again",
            crate::build::GVDB
        ))),
        _ => Ok(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Steam(bool);

    impl DepotHashes for Steam {
        fn stock(&self, _rel: &str, _path: &std::path::Path) -> Result<Option<bool>> {
            Ok(Some(self.0))
        }
    }

    #[test]
    fn refuses_a_game_database_steam_did_not_install() {
        let root = std::env::temp_dir().join(format!("swofty-stockdb-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(dir.join("game_package/gamedatabases")).unwrap();
        std::fs::write(dir.join(crate::GAME_EXE), "exe").unwrap();
        std::fs::write(dir.join(crate::build::GVDB), "db").unwrap();
        let game = Game {
            identity: crate::baseline::Identity::detect(&dir, Some(1)).unwrap(),
            dir: dir.clone(),
            user_reg: None,
        };
        let store = Store::new(root.join("data"));
        let live = dir.join(crate::build::GVDB);
        assert_eq!(stock_database_with(&store, &game, None).unwrap(), live);
        assert_eq!(
            stock_database_with(&store, &game, Some(&Steam(true))).unwrap(),
            live
        );
        let err = stock_database_with(&store, &game, Some(&Steam(false))).unwrap_err();
        assert!(err.to_string().contains("Verify integrity"), "{err}");
        std::fs::remove_dir_all(&root).ok();
    }
}
