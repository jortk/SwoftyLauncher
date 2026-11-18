//! The lap-finish toast: when the server accepts an uploaded lap that is the player's new best on its
//! board, the wrapper drops a message into the data folder's `tt/toast/<id>.json` and the core shows it
//! as a race HUD notification. See CONTRACT.md, addendum E.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::MAILBOX_TOAST;
use super::api::Uploaded;
use super::boards::content_listed;
use super::queue::{Folder, Queue};
use super::swlap::Board;

pub fn dir(data: &Path) -> PathBuf {
    data.join("tt").join(MAILBOX_TOAST)
}

/// The player's fastest accepted lap on `board` that was uploaded before the one named `id`.
pub fn previous_best(queue: &Queue, board: &Board, id: &str) -> Option<u64> {
    queue
        .list(Folder::Sent)
        .into_iter()
        .filter_map(|e| e.done)
        .filter(|d| {
            d.valid
                && d.content == board.content
                && d.track == board.track
                && d.weather == board.weather
                && d.answer
                    .as_ref()
                    .is_some_and(|a| a.status == "accepted" && a.id != id)
        })
        .map(|d| d.time_ms)
        .min()
}

/// The message for an upload reply, or `None` when no toast is due: a lap that is not accepted, not the
/// player's best, has no position, or is on content the launcher does not list
/// ([`content_listed`]). `previous_ms` is the best the launcher knew before this lap; a
/// reply that carries `previous_best_ms` wins over it.
pub fn message(
    board: &Board,
    lap_ms: u64,
    answer: &Uploaded,
    previous_ms: Option<u64>,
    now: u64,
) -> Option<Value> {
    if answer.status != "accepted" || answer.best != Some(true) || !content_listed(&board.content) {
        return None;
    }
    let position = answer.rank.filter(|r| *r >= 1)?;
    let mut doc = json!({
        "id": answer.id,
        "board": {"content": board.content, "track": board.track, "weather": board.weather},
        "lap_ms": lap_ms,
        "position": position,
        "record": position == 1,
        "created": now,
    });
    let object = doc.as_object_mut()?;
    if let Some(total) = answer.total.filter(|t| *t >= position) {
        object.insert("total".into(), total.into());
    }
    let gain = answer
        .previous_best_ms
        .or(previous_ms)
        .and_then(|p| p.checked_sub(lap_ms))
        .filter(|g| *g > 0);
    if let Some(gain) = gain {
        object.insert("improvement_ms".into(), gain.into());
    }
    Some(doc)
}

/// Write a message as `.tmp`, then rename. The id keeps a lap from showing twice.
pub fn write(data: &Path, doc: &Value) -> std::io::Result<()> {
    let dir = dir(data);
    std::fs::create_dir_all(&dir)?;
    let id: String = doc["id"]
        .as_str()
        .unwrap_or("toast")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();
    let tmp = dir.join(format!("{id}.json.tmp"));
    let done = dir.join(format!("{id}.json"));
    let written = std::fs::write(&tmp, doc.to_string()).and_then(|()| std::fs::rename(&tmp, &done));
    if written.is_err() {
        std::fs::remove_file(&tmp).ok();
    }
    written
}

/// A made-up message for `swofty laps toast-test`.
pub fn sample(now: u64, record: bool) -> Value {
    json!({
        "id": format!("test-{now}"),
        "board": {"content": "game/f1_2025", "track": "bahrain", "weather": "dry"},
        "lap_ms": 90_317,
        "position": if record { 1 } else { 12 },
        "total": 48,
        "improvement_ms": 386,
        "record": record,
        "created": now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board() -> Board {
        Board {
            content: "game/f1_2025".into(),
            track: "bahrain".into(),
            weather: "dry".into(),
            game_board: None,
        }
    }

    fn reply(status: &str, rank: Option<u32>, best: Option<bool>) -> Uploaded {
        Uploaded {
            id: "lap1".into(),
            status: status.into(),
            rank,
            best,
            total: None,
            previous_best_ms: None,
            reasons: vec![],
            url: None,
        }
    }

    #[test]
    fn an_accepted_personal_best_carries_position_time_and_gain() {
        let mut a = reply("accepted", Some(7), Some(true));
        a.total = Some(40);
        let m = message(&board(), 90_000, &a, Some(90_386), 1000).unwrap();
        assert_eq!(m["id"], "lap1");
        assert_eq!(m["position"], 7);
        assert_eq!(m["total"], 40);
        assert_eq!(m["lap_ms"], 90_000);
        assert_eq!(m["improvement_ms"], 386);
        assert_eq!(m["record"], false);
        assert_eq!(m["board"]["track"], "bahrain");
        assert_eq!(m["created"], 1000);
    }

    #[test]
    fn a_reply_without_total_or_previous_best_still_toasts() {
        let m = message(&board(), 90_000, &reply("accepted", Some(3), Some(true)), None, 5).unwrap();
        assert!(m.get("total").is_none() && m.get("improvement_ms").is_none());
    }

    #[test]
    fn the_replys_previous_best_wins_over_the_launchers() {
        let mut a = reply("accepted", Some(3), Some(true));
        a.previous_best_ms = Some(91_000);
        let m = message(&board(), 90_000, &a, Some(95_000), 5).unwrap();
        assert_eq!(m["improvement_ms"], 1000);
    }

    #[test]
    fn a_slower_previous_lap_gives_no_gain() {
        let m = message(
            &board(),
            90_000,
            &reply("accepted", Some(3), Some(true)),
            Some(89_000),
            5,
        )
        .unwrap();
        assert!(m.get("improvement_ms").is_none());
    }

    #[test]
    fn accepted_but_not_a_best_shows_nothing() {
        assert!(message(&board(), 1, &reply("accepted", Some(3), Some(false)), None, 0).is_none());
        assert!(message(&board(), 1, &reply("accepted", Some(3), None), None, 0).is_none());
    }

    #[test]
    fn pending_and_rejected_show_nothing() {
        assert!(message(&board(), 1, &reply("pending", Some(3), Some(true)), None, 0).is_none());
        assert!(message(&board(), 1, &reply("rejected", None, None), None, 0).is_none());
    }

    #[test]
    fn position_one_is_a_record() {
        let m = message(&board(), 1, &reply("accepted", Some(1), Some(true)), None, 0).unwrap();
        assert_eq!(m["record"], true);
    }

    #[test]
    fn a_best_on_unlisted_content_shows_nothing() {
        let classic = Board {
            content: "swofty.classic-seasons-2020/f1_2020".into(),
            ..board()
        };
        assert!(message(&classic, 1, &reply("accepted", Some(1), Some(true)), None, 0).is_none());
    }

    #[test]
    fn no_position_shows_nothing() {
        assert!(message(&board(), 1, &reply("accepted", None, Some(true)), None, 0).is_none());
    }

    #[test]
    fn write_renames_into_the_toast_folder() {
        let root = std::env::temp_dir().join(format!("swofty-toast-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        write(&root, &sample(9, false)).unwrap();
        let file = dir(&root).join("test-9.json");
        let doc: Value = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
        assert_eq!(doc["position"], 12);
        assert!(!dir(&root).join("test-9.json.tmp").exists());
        std::fs::remove_dir_all(&root).ok();
    }
}
