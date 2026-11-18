//! Ask the running game to play a lap from the boards as the ghost in Time Trial.
//!
//! The lap file goes next to `play.json` in `<game>/swofty/tt/ghost/`, and `play.json` names it. The core
//! picks the request up within half a second of a Time Trial session going live and arms it on the lap's track.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::api::Api;

const KEEP: usize = 8;

pub fn dir(game: &Path) -> PathBuf {
    game.join("swofty").join("tt").join("ghost")
}

pub(super) fn valid_id(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The request document for a lap whose details are `meta` (the server's lap reply) and whose file is `file`.
pub fn document(meta: &Value, file: &str) -> Result<Value, String> {
    let text = |v: &Value| v.as_str().unwrap_or_default().to_owned();
    let board = &meta["board"];
    let track = text(&board["track"]);
    if track.is_empty() {
        return Err("the lap has no track".to_owned());
    }
    let driver = text(&meta["user"]["name"]);
    let mut doc = json!({
        "lap": file,
        "track": track,
        "mode": "ghost",
        "label": driver,
        "driver": driver,
        "team": text(&meta["car"]["team"]),
    });
    for key in ["content", "weather"] {
        if let Some(v) = board[key].as_str().filter(|v| !v.is_empty()) {
            doc[key] = v.into();
        }
    }
    Ok(doc)
}

/// Write the lap file and the request into the game folder. Older lap files beyond a handful are removed.
pub fn place(game: &Path, id: &str, meta: &Value, lap: &[u8]) -> Result<PathBuf, String> {
    if !valid_id(id) {
        return Err("not a lap id".to_owned());
    }
    let dir = dir(game);
    let say = |what: &str, e: std::io::Error| format!("couldn't {what} in {}: {e}", dir.display());
    std::fs::create_dir_all(&dir).map_err(|e| say("make the folder", e))?;
    let name = format!("{id}.swlap");
    let doc = document(meta, &name)?;
    std::fs::write(dir.join(&name), lap).map_err(|e| say("write the lap", e))?;
    let mut old: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "swlap") && p.file_name().is_some_and(|n| n != name.as_str())
        })
        .filter_map(|p| Some((p.metadata().ok()?.modified().ok()?, p)))
        .collect();
    old.sort();
    old.reverse();
    for (_, path) in old.into_iter().skip(KEEP - 1) {
        std::fs::remove_file(path).ok();
    }
    let (tmp, play) = (dir.join("play.json.tmp"), dir.join("play.json"));
    std::fs::write(&tmp, doc.to_string()).map_err(|e| say("write the request", e))?;
    std::fs::rename(&tmp, &play).map_err(|e| say("place the request", e))?;
    Ok(play)
}

/// Download lap `id` and ask the game in `game` to play it.
pub fn request(api: &Api, game: &Path, id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Err("not a lap id".to_owned());
    }
    let meta: Value = api
        .get_json(&format!("/v1/tt/laps/{id}"))
        .map_err(|e| e.message)?;
    let lap = api.lap_file(id).map_err(|e| e.message)?;
    if lap.is_empty() {
        return Err("the lap has no replay file".to_owned());
    }
    place(game, id, &meta, &lap).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Game(PathBuf);

    impl Game {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("swofty-watch-{name}-{}", std::process::id()));
            std::fs::remove_dir_all(&dir).ok();
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Game {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    fn meta() -> Value {
        json!({
            "board": {"track": "monaco", "content": "a.b/sw_2019", "weather": "dry"},
            "user": {"name": "Driver"},
            "car": {"team": "sw19_mclaren"},
        })
    }

    #[test]
    fn the_request_names_the_lap_file_beside_it_and_the_board() {
        let game = Game::new("request");
        let play = place(game.path(), "2949b7d7eaf78422", &meta(), b"lap bytes").unwrap();
        let doc: Value = serde_json::from_slice(&std::fs::read(&play).unwrap()).unwrap();
        assert_eq!(
            doc,
            json!({"lap": "2949b7d7eaf78422.swlap", "track": "monaco", "content": "a.b/sw_2019",
                   "weather": "dry", "mode": "ghost", "label": "Driver", "driver": "Driver", "team": "sw19_mclaren"})
        );
        assert_eq!(
            std::fs::read(dir(game.path()).join("2949b7d7eaf78422.swlap")).unwrap(),
            b"lap bytes"
        );
        assert!(!dir(game.path()).join("play.json.tmp").exists());
    }

    #[test]
    fn a_bad_id_or_a_lap_without_a_track_writes_nothing() {
        let game = Game::new("refused");
        assert!(place(game.path(), "../../etc/passwd", &meta(), b"x").is_err());
        assert!(place(game.path(), "2949b7d7eaf78422", &json!({"board": {}}), b"x").is_err());
        assert!(!dir(game.path()).join("play.json").exists());
        assert!(!dir(game.path()).join("2949b7d7eaf78422.swlap").exists());
    }

    #[test]
    fn old_lap_files_are_pruned() {
        let game = Game::new("pruned");
        for i in 0..12u64 {
            place(game.path(), &format!("{i:016x}"), &meta(), b"x").unwrap();
        }
        let laps = std::fs::read_dir(dir(game.path()))
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "swlap"))
            .count();
        assert!(laps <= KEEP && dir(game.path()).join(format!("{:016x}.swlap", 11)).exists());
    }
}
