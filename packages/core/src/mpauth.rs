//! Launcher side of the Discord sign-in.
//!
//! The server (`/v1/auth/launcher/*`) runs the OAuth code exchange; the launcher
//! only opens the browser and catches the redirect back to a loopback port. The
//! GPUI layer supplies the browser-open (`cx.open_url`) and runs [`sign_in`] off
//! the UI thread; everything here is plain blocking I/O.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const WAIT: Duration = Duration::from_secs(180);
const BANNED_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Not signed in</title>\
<body style=\"font:16px system-ui;display:grid;place-items:center;height:100vh;margin:0;\
background:#0b0b0d;color:#eee\"><div>This account is banned. Swofty Launcher shows the reason. You can close this tab.</div>";
const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Signed in</title>\
<body style=\"font:16px system-ui;display:grid;place-items:center;height:100vh;margin:0;\
background:#0b0b0d;color:#eee\"><div>Signed in to Swofty Launcher. You can close this tab.</div>";

/// Why the server refuses an account, as the moderator wrote it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Ban {
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub by: Option<String>,
    #[serde(default)]
    pub at: Option<u64>,
}

/// Why a sign-in did not give a session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignInError {
    Banned(Ban),
    Failed(String),
}

impl std::fmt::Display for SignInError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Banned(b) => write!(f, "this account is banned: {}", b.reason),
            Self::Failed(e) => f.write_str(e),
        }
    }
}

impl From<String> for SignInError {
    fn from(e: String) -> Self {
        Self::Failed(e)
    }
}

impl From<&str> for SignInError {
    fn from(e: &str) -> Self {
        Self::Failed(e.to_owned())
    }
}

/// What the server says of a stored session.
#[derive(Clone, Debug)]
pub enum Refreshed {
    Session(Session),
    Gone,
    Banned(Ban),
}

/// A persisted sign-in. `allowed` is whether the server cleared this user for
/// multiplayer (beta-tester role); it is re-checked on each launch via [`refresh`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub token: String,
    pub name: String,
    pub allowed: bool,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub admin: bool,
}

/// The CDN link for a user's avatar, from the server's `avatar` value: a full link, or a Discord avatar hash.
pub fn avatar_link(id: Option<&str>, avatar: Option<&str>) -> Option<String> {
    let avatar = avatar.filter(|a| !a.is_empty())?;
    if avatar.starts_with("https://") {
        return Some(avatar.to_owned());
    }
    let id = id.filter(|i| !i.is_empty() && i.bytes().all(|b| b.is_ascii_digit()))?;
    let hash_ok = avatar.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    hash_ok.then(|| format!("https://cdn.discordapp.com/avatars/{id}/{avatar}.png?size=128"))
}

fn base_url() -> String {
    std::env::var(crate::catalog::URL_ENV)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::catalog::DEFAULT_URL.to_owned())
        .trim_end_matches('/')
        .to_owned()
}

fn session_path(data_dir: &Path) -> PathBuf {
    data_dir.join("multiplayer").join("session.json")
}

/// Load the saved session, if any.
pub fn load(data_dir: &Path) -> Option<Session> {
    let text = std::fs::read_to_string(session_path(data_dir)).ok()?;
    serde_json::from_str(&text).ok()
}

fn save(data_dir: &Path, session: &Session) -> std::io::Result<()> {
    let path = session_path(data_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(session).unwrap_or_default())
}

fn forget(data_dir: &Path) {
    let _ = std::fs::remove_file(session_path(data_dir));
}

fn query_pairs(request_line: &str) -> Vec<(String, String)> {
    let path = request_line.split_whitespace().nth(1).unwrap_or("");
    let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_owned(), urldecode(v)))
        .collect()
}

fn urldecode(s: &str) -> String {
    let raw = s.replace('+', " ");
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = (bytes[i] == b'%' && i + 2 < bytes.len())
            .then(|| u8::from_str_radix(&raw[i + 1..i + 3], 16).ok())
            .flatten();
        match hex {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Run the full sign-in: open the browser at the server's start URL and wait for
/// the loopback redirect, then persist and return the session. `open` is called
/// with the URL to open; it must not block (e.g. hand it to `cx.open_url`).
/// A loopback waiting for the sign-in redirect. [`begin`] returns it together
/// with the URL to open in the browser; [`Pending::wait`] blocks for the result,
/// so the browser-open can happen on the UI thread and the wait on a worker.
pub struct Pending {
    listener: TcpListener,
    nonce: String,
}

/// Open a loopback port and build the URL the browser should visit. Call this on
/// the UI thread, open the returned URL, then run [`Pending::wait`] off-thread.
pub fn begin() -> Result<(Pending, String), String> {
    let listener =
        TcpListener::bind("127.0.0.1:0").map_err(|e| format!("couldn't open a local port: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("couldn't read the local port: {e}"))?
        .port();
    listener.set_nonblocking(true).map_err(|e| format!("{e}"))?;
    let nonce = {
        let mut raw = [0u8; 18];
        getrandom(&mut raw);
        base64url(&raw)
    };
    let url = format!("{}/v1/auth/launcher/start?port={port}&nonce={nonce}", base_url());
    Ok((Pending { listener, nonce }, url))
}

/// Run the whole flow on one thread: open a port, hand the URL to `open`, and
/// block for the result. Convenient for the CLI; the GPUI layer uses
/// [`begin`] + [`Pending::wait`] instead.
pub fn sign_in(data_dir: &Path, open: impl FnOnce(&str)) -> Result<Session, String> {
    let (pending, url) = begin()?;
    open(&url);
    pending.wait(data_dir).map_err(|e| e.to_string())
}

impl Pending {
    /// Block until the browser hits the loopback with a matching nonce, persist
    /// the session, and return it.
    pub fn wait(self, data_dir: &Path) -> Result<Session, SignInError> {
        let Pending { listener, nonce } = self;
        let deadline = Instant::now() + WAIT;
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0u8; 4096];
                    let n = stream.read(&mut buf).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]);
                    let first = request.lines().next().unwrap_or("");
                    let pairs = query_pairs(first);
                    let get = |k: &str| pairs.iter().find(|(p, _)| p == k).map(|(_, v)| v.clone());

                    let ok = get("nonce").as_deref() == Some(nonce.as_str());
                    let banned = ok && get("error").as_deref() == Some("banned");
                    let body = if banned {
                        BANNED_PAGE
                    } else if ok {
                        DONE_PAGE
                    } else {
                        "sign-in could not be verified"
                    };
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = stream.flush();
                    let _ = stream.shutdown(Shutdown::Both);

                    if !ok {
                        continue;
                    }
                    if banned {
                        return Err(SignInError::Banned(Ban {
                            reason: get("reason").unwrap_or_default(),
                            by: get("by").filter(|v| !v.is_empty()),
                            at: get("at").and_then(|v| v.parse().ok()),
                        }));
                    }
                    let token = get("token").ok_or("the server sent no session token")?;
                    let session = Session {
                        token,
                        name: get("name").unwrap_or_else(|| "Player".into()),
                        allowed: get("allowed").as_deref() == Some("1"),
                        avatar: avatar_link(get("id").as_deref(), get("avatar").as_deref()),
                        admin: false,
                    };
                    let _ = save(data_dir, &session);
                    return Ok(session);
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err("sign-in timed out; try again".into());
                    }
                    std::thread::sleep(Duration::from_millis(120));
                }
                Err(e) => return Err(format!("sign-in failed: {e}").into()),
            }
        }
    }
}

/// Re-check the current session against the server (user still valid, still
/// cleared for multiplayer). Returns the refreshed session, or `None` once the
/// server says the session is gone (401), in which case it's forgotten. When
/// the server can't be reached, answers with another error, or can't tell
/// whether the user has the role, the session stays as it was.
pub fn refresh(data_dir: &Path, session: &Session) -> Option<Session> {
    refresh_at(&base_url(), data_dir, session)
}

/// [`refresh`], telling a ban apart from a plain end of the session.
pub fn refresh_checked(data_dir: &Path, session: &Session) -> Refreshed {
    check_at(&base_url(), data_dir, session)
}

fn refresh_at(base: &str, data_dir: &Path, session: &Session) -> Option<Session> {
    match check_at(base, data_dir, session) {
        Refreshed::Session(s) => Some(s),
        Refreshed::Gone | Refreshed::Banned(_) => None,
    }
}

fn check_at(base: &str, data_dir: &Path, session: &Session) -> Refreshed {
    let url = format!("{base}/v1/auth/launcher/me");
    let response = ureq::get(&url)
        .header("Authorization", &format!("Bearer {}", session.token))
        .header("User-Agent", "swofty-launcher")
        .config()
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .build()
        .call();
    let Ok(mut response) = response else {
        return Refreshed::Session(session.clone());
    };
    match response.status().as_u16() {
        200 => {}
        401 => {
            forget(data_dir);
            return Refreshed::Gone;
        }
        403 => {
            let ban = response
                .body_mut()
                .read_to_string()
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .filter(|d| d.get("error").and_then(serde_json::Value::as_str) == Some("banned"))
                .and_then(|d| serde_json::from_value::<Ban>(d).ok());
            return match ban {
                Some(ban) => {
                    forget(data_dir);
                    Refreshed::Banned(ban)
                }
                None => Refreshed::Session(session.clone()),
            };
        }
        _ => return Refreshed::Session(session.clone()),
    }
    let body = response
        .body_mut()
        .read_to_string()
        .ok()
        .and_then(|text| serde_json::from_str::<MeResponse>(&text).ok());
    let Some(body) = body else {
        return Refreshed::Session(session.clone());
    };
    let fresh = Session {
        token: session.token.clone(),
        name: body.user.name,
        allowed: match body.multiplayer_known {
            Some(false) => session.allowed,
            _ => body.multiplayer_allowed,
        },
        avatar: avatar_link(body.user.id.as_deref(), body.user.avatar.as_deref())
            .or_else(|| session.avatar.clone()),
        admin: body.admin,
    };
    let _ = save(data_dir, &fresh);
    Refreshed::Session(fresh)
}

/// End the session on the server and locally.
pub fn sign_out(data_dir: &Path, session: &Session) {
    let url = format!("{}/v1/auth/launcher/logout", base_url());
    let _ = ureq::post(&url)
        .header("Authorization", &format!("Bearer {}", session.token))
        .config()
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .build()
        .send_empty();
    forget(data_dir);
}

#[derive(Deserialize)]
struct MeResponse {
    user: MeUser,
    multiplayer_allowed: bool,
    #[serde(default)]
    admin: bool,
    /// Whether the server could look up the role; servers before this field
    /// always could.
    #[serde(default)]
    multiplayer_known: Option<bool>,
}

#[derive(Deserialize)]
struct MeUser {
    #[serde(default)]
    id: Option<String>,
    name: String,
    #[serde(default)]
    avatar: Option<String>,
}

fn base64url(bytes: &[u8]) -> String {
    const SET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..chunk.len() + 1 {
            out.push(SET[((n >> (18 - 6 * i)) & 0x3f) as usize] as char);
        }
    }
    out
}

pub(crate) fn getrandom(buf: &mut [u8]) {
    // Enough entropy for a one-shot loopback login nonce without adding a dependency:
    // RandomState is randomly seeded, mixed with the clock and a counter.
    use std::hash::{BuildHasher, Hasher, RandomState};
    let state = RandomState::new();
    let mut i = 0usize;
    let mut counter = 0u64;
    while i < buf.len() {
        let mut h = state.build_hasher();
        h.write_u64(counter);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        h.write_u128(now);
        for byte in h.finish().to_le_bytes() {
            if i < buf.len() {
                buf[i] = byte;
                i += 1;
            }
        }
        counter += 1;
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    fn serve(status: &str, body: &str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let reply = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    fn me(allowed: bool, known: Option<bool>) -> String {
        let known = known.map_or(String::new(), |k| format!(r#","multiplayer_known":{k}"#));
        format!(
            r#"{{"user":{{"id":"1","name":"xsoul","avatar":null}},"multiplayer_allowed":{allowed}{known}}}"#
        )
    }

    #[test]
    fn an_avatar_hash_becomes_a_discord_cdn_link() {
        assert_eq!(
            avatar_link(Some("123"), Some("abc_9")).as_deref(),
            Some("https://cdn.discordapp.com/avatars/123/abc_9.png?size=128")
        );
        assert_eq!(avatar_link(Some("123"), None), None);
        assert_eq!(avatar_link(None, Some("abc")), None);
        assert_eq!(avatar_link(Some("1/../2"), Some("abc")), None);
        assert_eq!(avatar_link(Some("1"), Some("a/b")), None);
        assert_eq!(
            avatar_link(None, Some("https://cdn.discordapp.com/a.png")).as_deref(),
            Some("https://cdn.discordapp.com/a.png")
        );
    }

    #[test]
    fn a_banned_callback_gives_the_reason_and_no_session() {
        let dir = std::env::temp_dir().join(format!("swofty-mpauth-ban-{}", std::process::id()));
        let (pending, _) = begin().unwrap();
        let port = pending.listener.local_addr().unwrap().port();
        let nonce = pending.nonce.clone();
        let client = std::thread::spawn(move || {
            let mut c = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            let line = format!(
                "GET /done?error=banned&reason=cheating%20on%20boards&by=mod&at=5&nonce={nonce} HTTP/1.1\r\n\r\n"
            );
            c.write_all(line.as_bytes()).unwrap();
            let mut out = String::new();
            c.read_to_string(&mut out).ok();
        });
        let got = pending.wait(&dir);
        client.join().unwrap();
        assert_eq!(
            got.unwrap_err(),
            SignInError::Banned(Ban {
                reason: "cheating on boards".into(),
                by: Some("mod".into()),
                at: Some(5)
            })
        );
        assert!(load(&dir).is_none());
    }

    #[test]
    fn only_a_401_ends_the_session_and_an_unknown_role_keeps_the_last_answer() {
        let dir = std::env::temp_dir().join(format!("swofty-mpauth-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let session = Session {
            token: "t".into(),
            name: "xsoul".into(),
            allowed: true,
            avatar: None,
            admin: false,
        };
        save(&dir, &session).unwrap();
        let closed = {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            format!("http://{}", listener.local_addr().unwrap())
        };

        assert!(refresh_at(&closed, &dir, &session).unwrap().allowed);
        assert!(
            refresh_at(&serve("500 Internal Server Error", ""), &dir, &session)
                .unwrap()
                .allowed
        );
        assert!(
            refresh_at(&serve("429 Too Many Requests", ""), &dir, &session)
                .unwrap()
                .allowed
        );
        assert!(
            refresh_at(&serve("200 OK", "not json"), &dir, &session)
                .unwrap()
                .allowed
        );
        assert!(
            refresh_at(&serve("200 OK", &me(false, Some(false))), &dir, &session)
                .unwrap()
                .allowed
        );
        assert!(load(&dir).is_some());

        assert!(
            refresh_at(&serve("200 OK", &me(true, None)), &dir, &session)
                .unwrap()
                .allowed
        );
        let denied = refresh_at(&serve("200 OK", &me(false, Some(true))), &dir, &session).unwrap();
        assert!(!denied.allowed);
        assert!(!load(&dir).unwrap().allowed);

        assert!(refresh_at(&serve("401 Unauthorized", "not signed in"), &dir, &session).is_none());
        assert!(load(&dir).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }
}
