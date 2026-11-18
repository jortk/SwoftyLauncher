//! The in-game bridge's mailbox pump. The core writes a request to `<mailbox>/outbox/<seq>.json`; the
//! pump asks the server and writes `<mailbox>/inbox/<seq>.json` (`{seq, status, body}`) so the game never
//! waits for its own timeout. The mailbox is the data folder's `tt`, or `<game>/swofty/tt` when the core
//! can't write the data folder ([`mailboxes`]); the pump serves both.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use super::api::{Api, ApiError};

pub const MAILBOX_DIR: &str = "swofty/tt";
const POLL_EVERY: Duration = Duration::from_millis(100);
const STALE_AFTER: Duration = Duration::from_secs(60);
const PER_SWEEP: usize = 4;

/// The folders the core keeps the mailbox in: the data folder's `tt`, which the game can always write
/// (it can't create files in `<game>/swofty` of an EA app copy under Program Files), then the game
/// folder's.
pub fn mailboxes(data: &Path, game: &Path) -> [PathBuf; 2] {
    [data.join("tt"), game.join(MAILBOX_DIR)]
}

pub fn outbox(mailbox: &Path) -> PathBuf {
    mailbox.join("outbox")
}

pub fn inbox(mailbox: &Path) -> PathBuf {
    mailbox.join("inbox")
}

/// Empty both folders, for the start and the end of a session.
pub fn clean(mailbox: &Path) {
    for dir in [outbox(mailbox), inbox(mailbox)] {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if entry.path().is_file() {
                std::fs::remove_file(entry.path()).ok();
            }
        }
    }
}

/// One request file as the pump reads it: the sequence number and the body to send, which is the request
/// without `seq`.
fn parse(path: &Path, bytes: &[u8]) -> Option<(u64, Value)> {
    let mut doc: Value = serde_json::from_slice(bytes).ok()?;
    let object = doc.as_object_mut()?;
    let seq = object
        .remove("seq")
        .and_then(|s| s.as_u64())
        .or_else(|| path.file_stem()?.to_str()?.parse().ok())?;
    object.contains_key("method").then_some((seq, doc))
}

fn older_than(path: &Path, limit: Duration) -> bool {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age > limit)
}

fn answer_for(result: Result<Value, ApiError>) -> (u16, String) {
    match result {
        Ok(v) => {
            let status = v.get("status").and_then(Value::as_u64).unwrap_or(200) as u16;
            let body = match v.get("body") {
                Some(Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
                None => "{}".to_owned(),
            };
            (status, body)
        }
        Err(e) => (503, json!({"Message": e.message, "Data": null}).to_string()),
    }
}

fn write_answer(mailbox: &Path, seq: u64, status: u16, body: &str) -> std::io::Result<()> {
    let dir = inbox(mailbox);
    std::fs::create_dir_all(&dir)?;
    let tmp = dir.join(format!("{seq}.json.tmp"));
    let done = dir.join(format!("{seq}.json"));
    let doc = json!({"seq": seq, "status": status, "body": body});
    let written = std::fs::write(&tmp, doc.to_string()).and_then(|()| std::fs::rename(&tmp, &done));
    if written.is_err() {
        std::fs::remove_file(&tmp).ok();
    }
    written
}

/// One pass over the outbox: answer at most four requests and drop what is stale or unreadable. Returns
/// the log line of each request answered.
pub fn sweep(mailbox: &Path, api: &Api, stop: &AtomicBool) -> Vec<String> {
    let mut lines = Vec::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(outbox(mailbox))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    let mut handled = 0;
    for path in files {
        if older_than(&path, STALE_AFTER) {
            std::fs::remove_file(&path).ok();
            continue;
        }
        if handled == PER_SWEEP || stop.load(Ordering::Relaxed) {
            break;
        }
        let Some((seq, request)) = std::fs::read(&path).ok().and_then(|b| parse(&path, &b)) else {
            if older_than(&path, Duration::from_secs(2)) {
                std::fs::remove_file(&path).ok();
            }
            continue;
        };
        handled += 1;
        let method = request["method"].as_str().unwrap_or("?").to_owned();
        let (status, body) = answer_for(api.game_f1os(&request));
        match write_answer(mailbox, seq, status, &body) {
            Ok(()) => lines.push(format!("bridge: {method} #{seq} -> {status}")),
            Err(e) => lines.push(format!("bridge: {method} #{seq}: couldn't write the answer: {e}")),
        }
        std::fs::remove_file(&path).ok();
    }
    for path in std::fs::read_dir(inbox(mailbox))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
    {
        if older_than(&path, STALE_AFTER) {
            std::fs::remove_file(&path).ok();
        }
    }
    lines
}

/// Polls the mailboxes every 100 ms while the game runs.
pub struct Pump {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    mailboxes: Vec<PathBuf>,
}

impl Pump {
    pub fn start(mailboxes: &[PathBuf], api: Api, say: impl Fn(&str) + Send + 'static) -> Self {
        mailboxes.iter().for_each(|m| clean(m));
        let stop = Arc::new(AtomicBool::new(false));
        let (flag, dirs) = (stop.clone(), mailboxes.to_vec());
        let thread = std::thread::Builder::new()
            .name("tt-bridge".into())
            .spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    for dir in &dirs {
                        for line in sweep(dir, &api, &flag) {
                            say(&line);
                        }
                    }
                    std::thread::sleep(POLL_EVERY);
                }
            })
            .ok();
        Self {
            stop,
            thread,
            mailboxes: mailboxes.to_vec(),
        }
    }

    pub fn finish(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.join().ok();
        }
        self.mailboxes.iter().for_each(|m| clean(m));
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use super::*;

    struct Server {
        url: String,
        seen: Arc<std::sync::Mutex<Vec<String>>>,
    }

    fn serve(status: &'static str, body: &'static str) -> Server {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut c) = stream else { return };
                let mut got = Vec::new();
                let mut buf = [0u8; 8192];
                loop {
                    let n = c.read(&mut buf).unwrap_or(0);
                    got.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&got).into_owned();
                    if let Some(h) = text.find("\r\n\r\n") {
                        let len = text[..h]
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .and_then(|v| v.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if got.len() >= h + 4 + len || n == 0 {
                            break;
                        }
                    } else if n == 0 {
                        break;
                    }
                }
                log.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&got).into_owned());
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                c.write_all(reply.as_bytes()).ok();
            }
        });
        Server { url, seen }
    }

    fn game(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-ttb-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(outbox(&dir)).unwrap();
        std::fs::create_dir_all(inbox(&dir)).unwrap();
        dir
    }

    fn request(game: &Path, seq: u64) -> PathBuf {
        let path = outbox(game).join(format!("{seq}.json"));
        let doc = json!({"seq": seq, "method": "GetTTLeaderboard", "args": {"LeaderboardID": "LBV0605D"},
            "board": {"track": "bahrain"}, "player": {}});
        std::fs::write(&path, doc.to_string()).unwrap();
        path
    }

    fn answer(game: &Path, seq: u64) -> Value {
        serde_json::from_slice(&std::fs::read(inbox(game).join(format!("{seq}.json"))).unwrap()).unwrap()
    }

    fn run(game: &Path, url: &str) -> Vec<String> {
        sweep(game, &Api::with_server(url, "tok"), &AtomicBool::new(false))
    }

    fn age(path: &Path, secs: u64) {
        let t = SystemTime::now() - Duration::from_secs(secs);
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn a_request_is_forwarded_without_seq_and_the_answer_keeps_its_seq() {
        let server = serve("200 OK", r#"{"status":200,"body":"{\"Data\":{\"Rows\":[]}}"}"#);
        let game = game("happy");
        let path = request(&game, 12);
        let lines = run(&game, &server.url);
        assert_eq!(lines, vec!["bridge: GetTTLeaderboard #12 -> 200"]);
        let a = answer(&game, 12);
        assert_eq!(
            a,
            json!({"seq": 12, "status": 200, "body": "{\"Data\":{\"Rows\":[]}}"})
        );
        assert!(!path.exists());
        assert!(!inbox(&game).join("12.json.tmp").exists());
        let sent = server.seen.lock().unwrap()[0].clone();
        assert!(sent.starts_with("POST /v1/tt/game/f1os HTTP/1.1"));
        assert!(sent.to_ascii_lowercase().contains("authorization: bearer tok"));
        assert!(sent.contains(r#""method":"GetTTLeaderboard""#));
        assert!(!sent.contains(r#""seq""#));
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn an_api_failure_answers_503_at_once() {
        let server = serve("503 Service Unavailable", r#"{"error":"boards are down"}"#);
        let game = game("503");
        let path = request(&game, 3);
        run(&game, &server.url);
        let a = answer(&game, 3);
        assert_eq!(a["status"], 503);
        let body: Value = serde_json::from_str(a["body"].as_str().unwrap()).unwrap();
        assert_eq!(body, json!({"Message": "boards are down", "Data": null}));
        assert!(!path.exists());
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn an_unreachable_server_answers_503() {
        let game = game("down");
        request(&game, 4);
        run(&game, "http://127.0.0.1:1");
        assert_eq!(answer(&game, 4)["status"], 503);
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn no_token_goes_over_plain_http_to_another_host() {
        let game = game("plain");
        request(&game, 5);
        run(&game, "http://10.0.0.5:1");
        let a = answer(&game, 5);
        assert_eq!(a["status"], 503);
        assert!(a["body"].as_str().unwrap().contains("plain http"));
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn malformed_and_half_written_files_are_skipped_without_a_panic() {
        let server = serve("200 OK", r#"{"status":200,"body":"{}"}"#);
        let game = game("bad");
        let half = outbox(&game).join("1.json");
        std::fs::write(&half, r#"{"seq": 1, "meth"#).unwrap();
        let notobject = outbox(&game).join("2.json");
        std::fs::write(&notobject, "[1,2]").unwrap();
        age(&half, 5);
        age(&notobject, 5);
        let fresh = outbox(&game).join("3.json");
        std::fs::write(&fresh, "{").unwrap();
        assert!(run(&game, &server.url).is_empty());
        assert!(!half.exists() && !notobject.exists());
        assert!(fresh.exists());
        assert!(server.seen.lock().unwrap().is_empty());
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn stale_files_go_unanswered_and_tmp_files_are_ignored() {
        let server = serve("200 OK", r#"{"status":200,"body":"{}"}"#);
        let game = game("stale");
        let old = request(&game, 7);
        age(&old, 120);
        let tmp = outbox(&game).join("8.json.tmp");
        std::fs::write(&tmp, r#"{"seq":8,"method":"X"}"#).unwrap();
        let old_answer = inbox(&game).join("9.json");
        std::fs::write(&old_answer, "{}").unwrap();
        age(&old_answer, 120);
        assert!(run(&game, &server.url).is_empty());
        assert!(!old.exists() && !old_answer.exists());
        assert!(tmp.exists());
        assert!(!inbox(&game).join("7.json").exists());
        assert!(server.seen.lock().unwrap().is_empty());
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn a_sweep_answers_four_and_leaves_the_rest_for_the_next() {
        let server = serve("200 OK", r#"{"status":200,"body":"{}"}"#);
        let game = game("cap");
        for seq in 1..=6 {
            request(&game, seq);
        }
        assert_eq!(run(&game, &server.url).len(), 4);
        assert_eq!(run(&game, &server.url).len(), 2);
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn the_pump_serves_the_data_folder_and_the_game_folder_mailboxes() {
        let server = serve("200 OK", r#"{"status":200,"body":"{}"}"#);
        let root = game("both");
        let (data, game_dir) = (root.join("data"), root.join("game"));
        let boxes = mailboxes(&data, &game_dir);
        assert_eq!(boxes[0], data.join("tt"));
        assert_eq!(boxes[1], game_dir.join("swofty").join("tt"));
        for b in &boxes {
            std::fs::create_dir_all(outbox(b)).unwrap();
        }
        let pump = Pump::start(&boxes, Api::with_server(&server.url, "tok"), |_| {});
        request(&boxes[0], 31);
        request(&boxes[1], 32);
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while !(inbox(&boxes[0]).join("31.json").exists() && inbox(&boxes[1]).join("32.json").exists())
            && std::time::Instant::now() < until
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(answer(&boxes[0], 31)["status"], 200);
        assert_eq!(answer(&boxes[1], 32)["status"], 200);
        assert!(!inbox(&boxes[0]).join("32.json").exists());
        pump.finish();
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_pump_answers_while_running_and_cleans_up_at_both_ends() {
        let server = serve("200 OK", r#"{"status":200,"body":"{\"ok\":1}"}"#);
        let game = game("pump");
        std::fs::write(inbox(&game).join("99.json"), "{}").unwrap();
        let pump = Pump::start(
            std::slice::from_ref(&game),
            Api::with_server(&server.url, "tok"),
            |_| {},
        );
        assert!(!inbox(&game).join("99.json").exists());
        request(&game, 21);
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while !inbox(&game).join("21.json").exists() && std::time::Instant::now() < until {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(answer(&game, 21)["status"], 200);
        pump.finish();
        assert!(!inbox(&game).join("21.json").exists());
        std::fs::remove_dir_all(&game).ok();
    }
}
