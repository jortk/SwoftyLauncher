//! Time Trial boards: the launcher's side of uploading laps the core records.
//!
//! The core writes one `.swlap` file per completed lap into `<game>/swofty/laps`, or into the data folder's
//! `laps` when it can't write the game folder (an EA app copy under Program Files). [`queue`] moves them into
//! the outbox and uploads them through [`api`]; [`upload`] is the loop the wrapper and the launcher run for it.

pub mod api;
pub mod barred;
pub mod boards;
pub mod bridge;
pub mod fingerprint;
pub mod game_replay;
pub mod integrity;
pub mod queue;
pub mod swlap;
pub mod toast;
pub mod upload;
pub mod watch;

use std::path::{Path, PathBuf};

use crate::session::Config;

pub const MAILBOX_TOAST: &str = "toast";
pub const LAPS_DIR: &str = "swofty/laps";
/// Where the wrapper hands the core its Time Trial boards switches. Not `swofty/session.json`: the core
/// treats that file's existence as a mixin manifest and refuses every mixin it does not declare.
pub const SESSION_FILE: &str = "swofty/tt.json";

/// Where the core writes laps: the game folder, where the integrity seal finds the session's `tt.json`, then the
/// data folder when the game folder refuses them (an EA app copy under Program Files).
pub fn laps_dirs(data: &Path, game: &Path) -> [PathBuf; 2] {
    [data.join("laps"), game.join(LAPS_DIR)]
}

/// The page that shows the boards.
pub fn boards_url() -> String {
    std::env::var("SWOFTY_TT_URL")
        .ok()
        .map(|u| u.trim().trim_end_matches('/').to_owned())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| "https://tt.f1.swofty.net".to_owned())
}

/// The token of the sign-in when Time Trial boards is on: the setting is on and the player is signed in.
pub fn token(data: &Path, config: &Config) -> Option<String> {
    config
        .time_trial_boards
        .then(|| crate::mpauth::load(data))
        .flatten()
        .map(|s| s.token)
}

const KEYS: [&str; 4] = ["tt_boards", "tt_nonce", "tt_bridge", "tt_ghost"];

/// Held while [`SESSION_FILE`] is read and written again, so the nonce retries and the barred list refresh,
/// each on a thread of its own, do not drop each other's keys.
pub(crate) static SESSION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Tell the core, through [`SESSION_FILE`], that laps are to be recorded for the boards, which
/// session nonce they carry and whether the in-game bridge is armed. Other keys in the file stay as they are.
pub fn write_session_keys(game: &Path, nonce: Option<&str>, bridge: bool) -> std::io::Result<()> {
    let _held = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = game.join(SESSION_FILE);
    let mut doc = read_object(&path);
    doc.insert("tt_boards".into(), true.into());
    match nonce {
        Some(n) => doc.insert("tt_nonce".into(), n.into()),
        None => doc.remove("tt_nonce"),
    };
    if bridge {
        doc.insert("tt_bridge".into(), true.into());
        doc.insert("tt_ghost".into(), true.into());
    } else {
        doc.remove("tt_bridge");
        doc.remove("tt_ghost");
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&doc).unwrap_or_default())
}

/// Take [`write_session_keys`]'s keys back out; the file goes if nothing else is in it.
pub fn clear_session_keys(game: &Path) {
    let _held = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = game.join(SESSION_FILE);
    let mut doc = read_object(&path);
    if KEYS.iter().all(|k| !doc.contains_key(*k)) {
        return;
    }
    KEYS.iter().for_each(|k| {
        doc.remove(*k);
    });
    if doc.is_empty() {
        std::fs::remove_file(&path).ok();
    } else {
        std::fs::write(&path, serde_json::to_vec_pretty(&doc).unwrap_or_default()).ok();
    }
}

fn read_object(path: &Path) -> serde_json::Map<String, serde_json::Value> {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_keys_merge_into_the_file_and_come_back_out() {
        let game = std::env::temp_dir().join(format!("swofty-ttk-{}", std::process::id()));
        std::fs::remove_dir_all(&game).ok();
        write_session_keys(&game, Some("abc"), true).unwrap();
        let doc: serde_json::Value =
            serde_json::from_slice(&std::fs::read(game.join(SESSION_FILE)).unwrap()).unwrap();
        assert_eq!(
            doc,
            serde_json::json!({"tt_boards": true, "tt_nonce": "abc", "tt_bridge": true, "tt_ghost": true})
        );
        clear_session_keys(&game);
        assert!(!game.join(SESSION_FILE).exists());

        std::fs::write(
            game.join(SESSION_FILE),
            r#"{"game_log":{"level":"warn"},"tt_nonce":"old"}"#,
        )
        .unwrap();
        write_session_keys(&game, None, false).unwrap();
        let doc: serde_json::Value =
            serde_json::from_slice(&std::fs::read(game.join(SESSION_FILE)).unwrap()).unwrap();
        assert_eq!(
            doc,
            serde_json::json!({"game_log":{"level":"warn"},"tt_boards": true})
        );
        clear_session_keys(&game);
        let doc: serde_json::Value =
            serde_json::from_slice(&std::fs::read(game.join(SESSION_FILE)).unwrap()).unwrap();
        assert_eq!(doc, serde_json::json!({"game_log":{"level":"warn"}}));
        std::fs::remove_dir_all(&game).ok();
    }
}
