//! The Time Trial boards server's side of the launcher: `POST /v1/tt/sessions` and `POST /v1/tt/laps`.

use std::fmt;
use std::io::Write;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::http::{self, Field};

const CALL_TIMEOUT: Duration = Duration::from_secs(4);
const GAME_TIMEOUT: Duration = Duration::from_secs(5);
const BOARD_TIMEOUT: Duration = Duration::from_secs(15);
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(60);
const REPLAY_TIMEOUT: Duration = Duration::from_secs(600);
const REPLAY_MAX: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
    pub retry_after: Option<u64>,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

impl ApiError {
    pub fn network(message: impl Into<String>) -> Self {
        Self {
            status: 0,
            message: message.into(),
            retry_after: None,
        }
    }

    pub fn from_body(status: u16, body: &str) -> Self {
        let doc: Value = serde_json::from_str(body).unwrap_or(Value::Null);
        let banned = doc.get("error").and_then(Value::as_str) == Some("banned");
        let said = doc
            .get("message")
            .and_then(Value::as_str)
            .filter(|m| !banned && !m.is_empty());
        let message = doc
            .get("error")
            .and_then(Value::as_str)
            .map(|e| said.unwrap_or(e))
            .map(|e| match (banned, doc.get("reason").and_then(Value::as_str)) {
                (true, Some(r)) if !r.is_empty() => format!("your account is banned from Time Trial: {r}"),
                (true, _) => "your account is banned from Time Trial".to_owned(),
                _ => e.to_owned(),
            })
            .unwrap_or_else(|| match status {
                401 => format!("your sign-in expired; sign in again (HTTP {status})"),
                413 => format!("the lap is too big for the server (HTTP {status})"),
                429 => format!("too many laps sent; try again shortly (HTTP {status})"),
                _ => format!("the Time Trial boards server refused the lap (HTTP {status})"),
            });
        Self {
            status,
            message,
            retry_after: doc.get("retry_after").and_then(Value::as_u64),
        }
    }

    /// Whether the same lap can succeed later: the server was unreachable, busy, rate limiting, or the
    /// sign-in has to be renewed first.
    pub fn transient(&self) -> bool {
        matches!(self.status, 0 | 401 | 408 | 429) || self.status >= 500
    }

    pub fn signed_out(&self) -> bool {
        self.status == 401
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Meta {
    pub launcher: String,
    pub os: String,
    pub session: Option<String>,
    pub fingerprint: Value,
}

impl Meta {
    pub fn new(session: Option<String>, fingerprint: Value) -> Self {
        Self {
            launcher: env!("CARGO_PKG_VERSION").to_owned(),
            os: if cfg!(windows) { "windows" } else { "linux" }.to_owned(),
            session,
            fingerprint,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Uploaded {
    pub id: String,
    pub status: String,
    #[serde(default)]
    pub rank: Option<u32>,
    #[serde(default)]
    pub best: Option<bool>,
    #[serde(default)]
    pub total: Option<u32>,
    #[serde(default)]
    pub previous_best_ms: Option<u64>,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Nonce {
    pub nonce: String,
    #[serde(default)]
    pub expires: u64,
}

/// What the queue needs of the server, so the queue's logic can run against a fake.
pub trait Boards {
    fn upload(&self, lap: &[u8], meta: &Meta) -> Result<Uploaded, ApiError>;
}

#[derive(Clone)]
pub struct Api {
    server: String,
    token: String,
    upload_timeout: Duration,
}

impl fmt::Debug for Api {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Api")
            .field("server", &self.server)
            .finish_non_exhaustive()
    }
}

impl Api {
    pub fn new(token: &str) -> Self {
        Self::with_server(&crate::catalog::base_url(), token)
    }

    pub fn with_server(server: &str, token: &str) -> Self {
        Self {
            server: server.trim().trim_end_matches('/').to_owned(),
            token: token.to_owned(),
            upload_timeout: UPLOAD_TIMEOUT,
        }
    }

    /// Give up on an upload after `limit`, for a caller that cannot wait.
    pub fn within(mut self, limit: Duration) -> Self {
        self.upload_timeout = limit.max(Duration::from_secs(1));
        self
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    /// A nonce that ties the laps of one game session together.
    pub fn sessions(&self) -> Result<Nonce, ApiError> {
        self.send("POST", "/v1/tt/sessions", None, CALL_TIMEOUT)
    }

    /// One in-game request of the bridge: `{status, body}` as the server built it for the game.
    pub fn game_f1os(&self, request: &Value) -> Result<Value, ApiError> {
        let body = ("application/json".to_owned(), request.to_string().into_bytes());
        self.send("POST", "/v1/tt/game/f1os", Some(body), GAME_TIMEOUT)
    }

    /// What the server offers as the integrity module: its version, size, hash and signature.
    pub fn integrity(&self) -> Result<Value, ApiError> {
        let bytes = self.get("/v1/tt/integrity", CALL_TIMEOUT, 64 * 1024)?;
        serde_json::from_slice(&bytes).map_err(|e| ApiError {
            status: 200,
            message: format!("the server's answer to GET /v1/tt/integrity is not valid: {e}"),
            retry_after: None,
        })
    }

    /// The integrity module's bytes for `version`, at most `limit` of them.
    pub fn integrity_file(&self, version: &str, limit: u64, timeout: Duration) -> Result<Vec<u8>, ApiError> {
        self.get(
            &format!("/v1/tt/integrity/file?version={version}"),
            timeout,
            limit,
        )
    }

    /// What the server offers of the modules `asked` for (`name`, `abi`) on `platform`: the ones this sign-in
    /// may download and, for the others, why not.
    pub fn modules(&self, platform: &str, asked: &[(&str, u32)]) -> Result<Value, ApiError> {
        let mut path = format!("/v1/modules?platform={platform}");
        for (name, abi) in asked {
            path.push_str(&format!("&abi.{name}={abi}"));
        }
        let bytes = self.get(&path, CALL_TIMEOUT, 256 * 1024)?;
        serde_json::from_slice(&bytes).map_err(|e| ApiError {
            status: 200,
            message: format!("the server's answer to GET /v1/modules is not valid: {e}"),
            retry_after: None,
        })
    }

    /// The bytes of one module version, at most `limit` of them.
    pub fn module_file(
        &self,
        name: &str,
        platform: &str,
        version: &str,
        limit: u64,
        timeout: Duration,
    ) -> Result<Vec<u8>, ApiError> {
        self.get(
            &format!("/v1/modules/file?name={name}&platform={platform}&version={version}"),
            timeout,
            limit,
        )
    }

    /// A lap's `.swlap` file.
    pub fn lap_file(&self, id: &str) -> Result<Vec<u8>, ApiError> {
        self.get(
            &format!("/v1/tt/laps/{id}/file"),
            UPLOAD_TIMEOUT,
            64 * 1024 * 1024,
        )
    }

    /// A lap's game replay file.
    pub fn game_replay(&self, id: &str) -> Result<Vec<u8>, ApiError> {
        self.get(
            &format!("/v1/tt/laps/{id}/game-replay"),
            REPLAY_TIMEOUT,
            REPLAY_MAX,
        )
    }

    /// Store `file` as the game replay of lap `id`, recorded with `setup` (a compact JSON object).
    pub fn put_game_replay(&self, id: &str, setup: &str, file: &[u8]) -> Result<(), ApiError> {
        self.request::<Value>(
            "PUT",
            &format!("/v1/tt/laps/{id}/game-replay"),
            &[("X-Swofty-Replay-Setup", setup)],
            "application/octet-stream",
            file,
            REPLAY_TIMEOUT,
        )
        .map(|_| ())
    }

    /// A JSON document of the boards server, read with the sign-in.
    pub fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, ApiError> {
        let bytes = self.get(path, BOARD_TIMEOUT, 8 * 1024 * 1024)?;
        serde_json::from_slice(&bytes).map_err(|e| ApiError {
            status: 200,
            message: format!("the server's answer to GET {path} is not valid: {e}"),
            retry_after: None,
        })
    }

    /// A POST with a JSON body whose answer is read as JSON.
    pub fn post_value(&self, path: &str, body: &Value) -> Result<Value, ApiError> {
        let body = ("application/json".to_owned(), body.to_string().into_bytes());
        self.send("POST", path, Some(body), BOARD_TIMEOUT)
    }

    /// A moderation call: a POST with a JSON body whose answer is ignored.
    pub fn post_json(&self, path: &str, body: &Value) -> Result<(), ApiError> {
        let body = ("application/json".to_owned(), body.to_string().into_bytes());
        self.send::<Value>("POST", path, Some(body), BOARD_TIMEOUT)
            .map(|_| ())
    }

    fn get(&self, path: &str, timeout: Duration, limit: u64) -> Result<Vec<u8>, ApiError> {
        if !http::bearer_allowed(&self.server) {
            return Err(ApiError::network(format!(
                "refusing to send the session token to {} over plain http; use https",
                self.server
            )));
        }
        let url = format!("{}{path}", self.server);
        let sent = ureq::get(&url)
            .header("Authorization", &format!("Bearer {}", self.token))
            .header("User-Agent", "swofty-launcher")
            .header("Accept-Encoding", "identity")
            .config()
            .timeout_global(Some(timeout))
            .timeout_connect(Some(Duration::from_secs(10)))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .call();
        let mut response =
            sent.map_err(|e| ApiError::network(format!("couldn't reach {}: {e}", self.server)))?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let body = response
            .body_mut()
            .with_config()
            .limit(limit + 1)
            .read_to_vec()
            .map_err(|e| ApiError::network(format!("reading the answer to GET {path}: {e}")))?;
        if body.len() as u64 > limit {
            return Err(ApiError::network(format!(
                "the answer to GET {path} is longer than the {limit} bytes expected"
            )));
        }
        if !(200..300).contains(&status) {
            let mut e = ApiError::from_body(status, &String::from_utf8_lossy(&body));
            e.retry_after = e.retry_after.or(retry_after);
            return Err(e);
        }
        Ok(body)
    }

    fn send<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<(String, Vec<u8>)>,
        timeout: Duration,
    ) -> Result<T, ApiError> {
        let (content_type, bytes) = body.unwrap_or_else(|| ("application/json".into(), Vec::new()));
        self.request(method, path, &[], &content_type, &bytes, timeout)
    }

    fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        content_type: &str,
        bytes: &[u8],
        timeout: Duration,
    ) -> Result<T, ApiError> {
        if !http::bearer_allowed(&self.server) {
            return Err(ApiError::network(format!(
                "refusing to send the session token to {} over plain http; use https",
                self.server
            )));
        }
        let url = format!("{}{path}", self.server);
        let auth = format!("Bearer {}", self.token);
        let mut request = match method {
            "PUT" => ureq::put(&url),
            _ => ureq::post(&url),
        }
        .header("Authorization", &auth)
        .header("User-Agent", "swofty-launcher")
        .header("Content-Type", content_type);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let sent = request
            .config()
            .timeout_global(Some(timeout))
            .timeout_connect(Some(Duration::from_secs(10)))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .send(bytes);
        let mut response =
            sent.map_err(|e| ApiError::network(format!("couldn't reach {}: {e}", self.server)))?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| ApiError::network(format!("reading the answer to {method} {path}: {e}")))?;
        if !(200..300).contains(&status) {
            let mut e = ApiError::from_body(status, &text);
            e.retry_after = e.retry_after.or(retry_after);
            return Err(e);
        }
        let text = if text.trim().is_empty() {
            "null"
        } else {
            text.as_str()
        };
        serde_json::from_str(text).map_err(|e| ApiError {
            status,
            message: format!("the server's answer to {method} {path} is not valid: {e}"),
            retry_after: None,
        })
    }
}

impl Boards for Api {
    fn upload(&self, lap: &[u8], meta: &Meta) -> Result<Uploaded, ApiError> {
        let packed = gzip(lap).map_err(|e| ApiError::network(format!("compressing the lap: {e}")))?;
        let mut doc = serde_json::to_value(meta).expect("serialisable");
        doc["gzip"] = true.into();
        let meta = doc.to_string();
        let body = http::multipart(&[
            Field {
                name: "meta",
                file_name: None,
                content_type: "application/json",
                data: meta.as_bytes(),
            },
            Field {
                name: "lap",
                file_name: Some("lap.swlap.gz"),
                content_type: "application/octet-stream",
                data: &packed,
            },
        ]);
        self.send("POST", "/v1/tt/laps", Some(body), self.upload_timeout)
    }
}

fn gzip(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    enc.write_all(bytes)?;
    enc.finish()
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    #[test]
    fn reads_the_servers_error() {
        let e = ApiError::from_body(429, r#"{"error":"slow down","retry_after":7}"#);
        assert_eq!((e.message.as_str(), e.retry_after), ("slow down", Some(7)));
        assert!(e.transient());
        let e = ApiError::from_body(400, "<html>");
        assert!(e.message.contains("HTTP 400"));
        assert!(!e.transient());
        assert!(ApiError::from_body(401, "").signed_out());
        assert!(ApiError::from_body(503, "").transient());
        assert!(ApiError::network("down").transient());
        assert!(!ApiError::from_body(413, "").transient());
    }

    #[test]
    fn answers_parse_with_missing_optionals() {
        let u: Uploaded = serde_json::from_str(r#"{"id":"x","status":"pending"}"#).unwrap();
        assert_eq!(u.rank, None);
        assert!(u.reasons.is_empty());
        let u: Uploaded = serde_json::from_str(
            r#"{"id":"x","status":"ranked","rank":12,"best":true,"reasons":["a"],"url":"https://t/lap/x"}"#,
        )
        .unwrap();
        assert_eq!((u.rank, u.best), (Some(12), Some(true)));
    }

    #[test]
    fn refuses_a_token_over_plain_http() {
        let e = Api::with_server("http://10.0.0.5:1", "t").sessions().unwrap_err();
        assert!(e.message.contains("plain http"));
        let e = Api::with_server("http://127.0.0.1:1", "t")
            .sessions()
            .unwrap_err();
        assert!(e.message.contains("couldn't reach"));
    }

    #[test]
    fn a_lap_round_trips_through_gzip() {
        use std::io::Read;
        let packed = gzip(b"SWLAP lap bytes").unwrap();
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(&packed[..])
            .read_to_end(&mut out)
            .unwrap();
        assert_eq!(out, b"SWLAP lap bytes");
    }

    pub(in crate::timetrial) fn serve(
        status: &str,
        body: &'static str,
    ) -> (String, std::thread::JoinHandle<Vec<u8>>) {
        use std::io::Read;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let status = status.to_owned();
        let t = std::thread::spawn(move || {
            let (mut c, _) = listener.accept().unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                let n = c.read(&mut buf).unwrap();
                got.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&got);
                if let Some(h) = text.find("\r\n\r\n") {
                    let len = text[..h]
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if got.len() >= h + 4 + len {
                        break;
                    }
                }
            }
            let reply = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            c.write_all(reply.as_bytes()).unwrap();
            got
        });
        (url, t)
    }

    #[test]
    fn uploads_a_lap_as_multipart_with_the_bearer_token() {
        let (url, t) = serve("200 OK", r#"{"id":"l1","status":"ranked","rank":4,"best":true}"#);
        let meta = Meta::new(Some("n0nce".into()), serde_json::json!({"digest": "d"}));
        let got = Api::with_server(&url, "tok")
            .upload(b"SWLAP-bytes", &meta)
            .unwrap();
        assert_eq!((got.id.as_str(), got.rank), ("l1", Some(4)));
        let request = String::from_utf8_lossy(&t.join().unwrap()).into_owned();
        assert!(request.starts_with("POST /v1/tt/laps HTTP/1.1"));
        assert!(request.to_ascii_lowercase().contains("authorization: bearer tok"));
        assert!(request.contains("name=\"meta\""));
        assert!(request.contains(r#""gzip":true"#));
        assert!(request.contains(r#""session":"n0nce""#));
        assert!(request.contains("name=\"lap\"; filename="));
    }

    #[test]
    fn puts_a_game_replay_with_its_setup_header() {
        let (url, t) = serve("200 OK", r#"{"size":9}"#);
        Api::with_server(&url, "tok")
            .put_game_replay("2949b7d7eaf78422", r#"{"lap_ms":5}"#, b"FRR bytes")
            .unwrap();
        let request = String::from_utf8_lossy(&t.join().unwrap()).into_owned();
        assert!(request.starts_with("PUT /v1/tt/laps/2949b7d7eaf78422/game-replay HTTP/1.1"));
        let lower = request.to_ascii_lowercase();
        assert!(lower.contains("authorization: bearer tok"));
        assert!(lower.contains("content-type: application/octet-stream"));
        assert!(lower.contains(r#"x-swofty-replay-setup: {"lap_ms":5}"#));
        assert!(request.ends_with("FRR bytes"));
        let (url, t) = serve("413 Payload Too Large", "");
        let e = Api::with_server(&url, "tok")
            .put_game_replay("2949b7d7eaf78422", "{}", b"x")
            .unwrap_err();
        assert!(e.status == 413 && !e.transient());
        t.join().unwrap();
    }

    #[test]
    fn asks_for_a_session_nonce_and_reads_a_refusal() {
        let (url, t) = serve("200 OK", r#"{"nonce":"abcd","expires":99}"#);
        let n = Api::with_server(&url, "tok").sessions().unwrap();
        assert_eq!((n.nonce.as_str(), n.expires), ("abcd", 99));
        t.join().unwrap();
        let (url, t) = serve("429 Too Many Requests", r#"{"error":"slow down"}"#);
        let e = Api::with_server(&url, "tok")
            .upload(b"x", &Meta::new(None, Value::Null))
            .unwrap_err();
        assert_eq!(
            (e.status, e.message.as_str(), e.transient()),
            (429, "slow down", true)
        );
        t.join().unwrap();
    }
}
