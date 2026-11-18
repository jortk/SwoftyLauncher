//! Downloads to a file that survive a broken connection: a body cut off
//! part-way is asked for again from the bytes already written, with an HTTP
//! Range request, and the SHA-256 covers the whole file.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::mpsc::{RecvTimeoutError, sync_channel};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::{Error, Result, io_err};

/// Attempts in a row that may end without a new byte before a download gives up.
const TRIES: u32 = 4;
/// Attempts one download may make in all.
const MOST: u32 = 12;
/// The wait before the first retry; each retry in a row without a new byte
/// waits twice as long as the one before.
const BACKOFF: Duration = if cfg!(test) {
    Duration::from_millis(20)
} else {
    Duration::from_secs(2)
};

/// The longest a body may go without a byte, and the span in which it must
/// deliver [`WINDOW_BYTES`], before the connection counts as stalled.
const WINDOW: Duration = if cfg!(test) {
    Duration::from_millis(800)
} else {
    Duration::from_secs(60)
};
/// The fewest bytes a body may deliver in one [`WINDOW`] after its first byte.
const WINDOW_BYTES: u64 = if cfg!(test) { 64 } else { 4 * 1024 };
/// Stalled connections one download reconnects through before it gives up.
const STALLS: u32 = 5;

/// Chunk size for transport reads and disk streaming.
/// 256 KB balances low syscall / context-switch overhead with smooth progress updates,
/// matching modern TCP window scaling and NVMe / SSD write characteristics.
const CHUNK_SIZE: usize = 256 * 1024;

/// Capacity of the buffer pool and transfer channel.
/// 16 buffers of 256 KB provide a 4 MB pipelined cushion. This absorbs disk flush
/// spikes and hashing latency without blocking the network reader thread, keeping the
/// TCP receive window open and preventing TCP ZeroWindow throttling by remote CDNs.
const CHANNEL_CAPACITY: usize = 16;

/// What a download that kept breaking off says to do.
pub const ADVICE: &str = "the connection kept breaking during the download, usually from antivirus HTTPS or web \
     scanning, a VPN or a proxy; turn those off or try another network, such as a phone hotspot, then try again";

/// A file to download.
pub(crate) struct Fetch<'a> {
    pub url: &'a str,
    pub size: u64,
    /// What lists `size`, such as "the registry", for the errors.
    pub lister: &'a str,
    /// The error for a body longer than `size`.
    pub too_long: &'a dyn Fn() -> String,
}

/// Why a request got no usable answer.
pub(crate) enum Opening {
    Call(ureq::Error),
    Refused(Error),
}

enum Attempt {
    Broke(String),
    Stalled(u64),
    Failed(Error),
}

/// Downloads `f` to `to` with `open`, which sends the request, with a
/// `Range` from the given byte when it isn't 0, and turns an HTTP error into
/// [`Opening::Refused`]. A connection or TLS failure is tried again, resuming
/// where the file stops when the server answers the Range. Returns the length
/// and SHA-256 of the file.
pub(crate) fn fetch(
    f: &Fetch,
    to: &Path,
    open: &mut dyn FnMut(u64) -> std::result::Result<ureq::http::Response<ureq::Body>, Opening>,
    progress: &dyn Fn(u64),
) -> Result<(u64, String)> {
    let mut file = std::fs::File::create(to).map_err(io_err(to))?;
    let mut got = Got {
        hash: Sha256::new(),
        len: 0,
        resumable: true,
    };
    let (mut quiet, mut tries, mut stalls) = (0, 0, 0);
    loop {
        tries += 1;
        let before = got.len;
        let broke = match attempt(f, &mut file, to, &mut got, open, progress) {
            Ok(()) => break,
            Err(Attempt::Failed(e)) => return Err(e),
            Err(Attempt::Broke(e)) => e,
            Err(Attempt::Stalled(rate)) => {
                stalls += 1;
                if stalls >= STALLS {
                    return Err(Error::Other(format!(
                        "the download from {} stayed below {} KB in {} s, {STALLS} times; check your connection or VPN",
                        host(f.url),
                        WINDOW_BYTES / 1024,
                        WINDOW.as_secs()
                    )));
                }
                eprintln!(
                    "[download] stalled at {} KB ({rate} B/s for {} s); reconnecting, attempt {}/{STALLS}",
                    got.len / 1024,
                    WINDOW.as_secs(),
                    stalls + 1
                );
                std::thread::sleep(BACKOFF * 2u32.pow(stalls - 1));
                continue;
            }
        };
        quiet = if got.len > before { 0 } else { quiet + 1 };
        if quiet >= TRIES || tries >= MOST {
            return Err(Error::Other(format!("{ADVICE}\n({}: {broke})", f.url)));
        }
        eprintln!(
            "swofty-core: {}: {broke}; trying again from byte {}",
            f.url,
            if got.resumable { got.len } else { 0 }
        );
        std::thread::sleep(BACKOFF * 2u32.pow(quiet.saturating_sub(1)));
    }
    file.sync_all().map_err(io_err(to))?;
    let digest = got.hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Ok((got.len, digest))
}

struct Got {
    hash: Sha256,
    len: u64,
    resumable: bool,
}

fn attempt(
    f: &Fetch,
    file: &mut std::fs::File,
    to: &Path,
    got: &mut Got,
    open: &mut dyn FnMut(u64) -> std::result::Result<ureq::http::Response<ureq::Body>, Opening>,
    progress: &dyn Fn(u64),
) -> std::result::Result<(), Attempt> {
    let from = if got.resumable { got.len } else { 0 };
    let response = match open(from) {
        Ok(r) => r,
        Err(Opening::Call(e)) if transport(&e) => return Err(Attempt::Broke(e.to_string())),
        Err(Opening::Call(e)) => return Err(Attempt::Failed(Error::Other(format!("{}: {e}", f.url)))),
        Err(Opening::Refused(e)) => return Err(Attempt::Failed(e)),
    };
    let headers = response.headers();
    let header = |name: &str| headers.get(name).and_then(|h| h.to_str().ok());
    let partial = response.status().as_u16() == 206;
    if partial && (from == 0 || resumes_at(header("content-range"), f.size) != Some(from)) {
        return Err(Attempt::Failed(Error::Other(format!(
            "{}: the server sent another part of the file than byte {from} on",
            f.url
        ))));
    }
    if !partial && got.len > 0 {
        file.set_len(0)
            .and_then(|()| file.seek(SeekFrom::Start(0)).map(drop))
            .map_err(|e| Attempt::Failed(io_err(to)(e)))?;
        got.hash = Sha256::new();
        got.len = 0;
        progress(0);
    }
    let encoded = headers.contains_key("content-encoding");
    got.resumable = !encoded;
    let left = f.size - got.len;
    let sent = header("content-length").and_then(|h| h.trim().parse::<u64>().ok());
    if let Some(sent) = sent.filter(|s| !encoded && *s != left) {
        let after = if partial {
            format!(" after byte {from}")
        } else {
            String::new()
        };
        return Err(Attempt::Failed(Error::Other(format!(
            "{}: the server sends {sent} bytes{after}, {} lists {}",
            f.url, f.lister, f.size
        ))));
    }
    let (tx, rx) = sync_channel::<std::io::Result<Vec<u8>>>(CHANNEL_CAPACITY);
    let (free_tx, free_rx) = sync_channel::<Vec<u8>>(CHANNEL_CAPACITY);
    for _ in 0..CHANNEL_CAPACITY {
        let _ = free_tx.send(vec![0; CHUNK_SIZE]);
    }
    std::thread::spawn(move || {
        let mut body = response.into_body();
        let mut reader = body.with_config().limit(left + 1).reader();
        loop {
            let mut buf = free_rx.recv().unwrap_or_else(|_| vec![0; CHUNK_SIZE]);
            if buf.len() < CHUNK_SIZE {
                buf.resize(CHUNK_SIZE, 0);
            }
            let read = reader.read(&mut buf).map(|n| {
                buf.truncate(n);
                buf
            });
            let end = !matches!(&read, Ok(b) if !b.is_empty());
            if tx.send(read).is_err() || end {
                break;
            }
        }
    });
    let (mut window_at, mut window_bytes, mut started) = (Instant::now(), 0u64, false);
    loop {
        let wait = WINDOW.saturating_sub(window_at.elapsed());
        match rx.recv_timeout(wait) {
            Ok(Err(e)) if broken(&e) => return Err(Attempt::Broke(e.to_string())),
            Ok(Err(e)) => return Err(Attempt::Failed(Error::Other(format!("{}: {e}", f.url)))),
            Ok(Ok(chunk)) if chunk.is_empty() => break,
            Ok(Ok(chunk)) => {
                started = true;
                window_bytes += chunk.len() as u64;
                got.len += chunk.len() as u64;
                if got.len > f.size {
                    return Err(Attempt::Failed(Error::Other((f.too_long)())));
                }
                got.hash.update(&chunk);
                file.write_all(&chunk)
                    .map_err(|e| Attempt::Failed(io_err(to)(e)))?;
                progress(got.len);
                let _ = free_tx.try_send(chunk);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err(Attempt::Broke("the download thread ended".into()));
            }
        }
        if window_at.elapsed() >= WINDOW {
            if window_bytes < if started { WINDOW_BYTES } else { 1 } {
                return Err(Attempt::Stalled(window_bytes / WINDOW.as_secs().max(1)));
            }
            (window_at, window_bytes) = (Instant::now(), 0);
        }
    }
    if got.len < f.size && !encoded {
        return Err(Attempt::Broke(format!(
            "the connection closed after {} of {} bytes",
            got.len, f.size
        )));
    }
    Ok(())
}

fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?']).next().unwrap_or(rest)
}

/// The first byte of a `Content-Range: bytes <first>-<last>/<size>` that
/// runs to the end of a `size`-byte file.
fn resumes_at(range: Option<&str>, size: u64) -> Option<u64> {
    let (span, total) = range?.trim().strip_prefix("bytes ")?.split_once('/')?;
    let (first, last) = span.split_once('-')?;
    let (first, last) = (
        first.trim().parse::<u64>().ok()?,
        last.trim().parse::<u64>().ok()?,
    );
    let whole = total.trim() == "*" || total.trim().parse::<u64>().ok() == Some(size);
    (whole && last + 1 == size && first <= last).then_some(first)
}

const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(3),
    Duration::from_secs(8),
];

/// A server answer worth asking again for: a 5xx or a rate limit.
pub(crate) fn retryable_status(status: u16) -> bool {
    status == 429 || (500..600).contains(&status)
}

/// A call that never got an answer for want of time or a connection.
pub(crate) fn retryable_error(e: &ureq::Error) -> bool {
    matches!(
        e,
        ureq::Error::Timeout(_) | ureq::Error::ConnectionFailed | ureq::Error::Io(_)
    )
}

/// How long to wait before the attempt after `attempt` (counted from 1), or
/// none when the retries are used up.
pub(crate) fn retry_delay(attempt: u32) -> Option<Duration> {
    RETRY_DELAYS.get(attempt.checked_sub(1)? as usize).copied()
}

fn transient(result: &std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> bool {
    match result {
        Ok(r) => retryable_status(r.status().as_u16()),
        Err(e) => retryable_error(e),
    }
}

/// Runs `call` again after a transient failure, with [`RETRY_DELAYS`]
/// between, and returns the last result with the number of attempts made.
pub(crate) fn retrying<T, E>(
    mut call: impl FnMut() -> std::result::Result<T, E>,
    transient: impl Fn(&std::result::Result<T, E>) -> bool,
    sleep: impl Fn(Duration),
) -> (std::result::Result<T, E>, u32) {
    let mut attempt = 1;
    loop {
        let result = call();
        match retry_delay(attempt).filter(|_| transient(&result)) {
            Some(delay) => {
                sleep(delay);
                attempt += 1;
            }
            None => return (result, attempt),
        }
    }
}

/// [`retrying`] for a ureq call made with `http_status_as_error(false)`.
pub(crate) fn send(
    call: impl FnMut() -> std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> (
    std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    u32,
) {
    retrying(call, transient, std::thread::sleep)
}

/// " (after N attempts)" when more than one was made.
pub(crate) fn attempts_note(attempts: u32) -> String {
    if attempts > 1 {
        format!(" (after {attempts} attempts)")
    } else {
        String::new()
    }
}

/// A failure of the connection or its TLS, as opposed to an answer from the
/// server.
pub(crate) fn transport(e: &ureq::Error) -> bool {
    matches!(
        e,
        ureq::Error::Io(_)
            | ureq::Error::Rustls(_)
            | ureq::Error::Tls(_)
            | ureq::Error::Timeout(_)
            | ureq::Error::HostNotFound
            | ureq::Error::ConnectionFailed
            | ureq::Error::ConnectProxyFailed(_)
    )
}

/// A read of a body that failed on the connection or its TLS. ureq hands its
/// own errors, such as a body over its limit, through `io::Error`.
pub(crate) fn broken(e: &std::io::Error) -> bool {
    match e.get_ref().and_then(|inner| inner.downcast_ref::<ureq::Error>()) {
        Some(inner) => transport(inner),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_only_server_trouble() {
        for s in [429, 500, 502, 503, 504] {
            assert!(retryable_status(s), "{s}");
        }
        for s in [200, 301, 400, 401, 403, 404, 410, 416] {
            assert!(!retryable_status(s), "{s}");
        }
        assert!(retryable_error(&ureq::Error::Timeout(ureq::Timeout::Connect)));
        assert!(retryable_error(&ureq::Error::Timeout(
            ureq::Timeout::RecvResponse
        )));
        assert!(retryable_error(&ureq::Error::ConnectionFailed));
        assert!(!retryable_error(&ureq::Error::StatusCode(404)));
        assert!(!retryable_error(&ureq::Error::HostNotFound));
        assert!(!retryable_error(&ureq::Error::BodyExceedsLimit(10)));
    }

    #[test]
    fn backs_off_then_gives_up() {
        assert_eq!(retry_delay(0), None);
        assert_eq!(retry_delay(1), Some(Duration::from_secs(1)));
        assert_eq!(retry_delay(2), Some(Duration::from_secs(3)));
        assert_eq!(retry_delay(3), Some(Duration::from_secs(8)));
        assert_eq!(retry_delay(4), None);
    }

    #[test]
    fn counts_attempts() {
        let slept = std::cell::RefCell::new(Vec::new());
        let mut n = 0;
        let (result, attempts) = retrying(
            || {
                n += 1;
                if n < 3 { Err(504) } else { Ok(n) }
            },
            |r: &std::result::Result<i32, i32>| r.is_err(),
            |d| slept.borrow_mut().push(d),
        );
        assert_eq!((result, attempts), (Ok(3), 3));
        assert_eq!(*slept.borrow(), [Duration::from_secs(1), Duration::from_secs(3)]);

        let (result, attempts) = retrying(|| Err::<i32, i32>(504), |r| r.is_err(), |_| {});
        assert_eq!((result, attempts), (Err(504), 4));

        let mut calls = 0;
        let (result, attempts) = retrying(
            || {
                calls += 1;
                Err::<i32, i32>(404)
            },
            |_| false,
            |_| panic!("no wait for a permanent failure"),
        );
        assert_eq!((result, attempts, calls), (Err(404), 1, 1));
        assert_eq!(attempts_note(1), "");
        assert_eq!(attempts_note(4), " (after 4 attempts)");
    }

    #[test]
    fn counts_tls_and_connection_failures_as_transport() {
        let decrypt = std::io::Error::new(std::io::ErrorKind::InvalidData, rustls::Error::DecryptError);
        assert!(broken(&decrypt));
        assert_eq!(decrypt.to_string(), "cannot decrypt peer's message");
        let reset = std::io::Error::from(std::io::ErrorKind::ConnectionReset);
        assert!(broken(&reset));
        let eof = ureq::Error::Io(std::io::Error::from(std::io::ErrorKind::UnexpectedEof)).into_io();
        assert!(broken(&eof));
        assert!(broken(&ureq::Error::Timeout(ureq::Timeout::RecvBody).into_io()));
        assert!(!broken(&ureq::Error::BodyExceedsLimit(10).into_io()));

        assert!(transport(&ureq::Error::Rustls(rustls::Error::DecryptError)));
        assert!(transport(&ureq::Error::ConnectionFailed));
        assert!(transport(&ureq::Error::HostNotFound));
        assert!(!transport(&ureq::Error::StatusCode(404)));
        assert!(!transport(&ureq::Error::BodyExceedsLimit(10)));
    }

    #[test]
    fn reads_where_a_range_answer_starts() {
        assert_eq!(resumes_at(Some("bytes 100-999/1000"), 1000), Some(100));
        assert_eq!(resumes_at(Some(" bytes 0-999/* "), 1000), Some(0));
        assert_eq!(resumes_at(Some("bytes 100-998/1000"), 1000), None);
        assert_eq!(resumes_at(Some("bytes 100-999/2000"), 1000), None);
        assert_eq!(resumes_at(Some("bytes */1000"), 1000), None);
        assert_eq!(resumes_at(Some("items 100-999/1000"), 1000), None);
        assert_eq!(resumes_at(None, 1000), None);
    }

    fn serve(
        size: usize,
        trickle: fn(usize) -> bool,
    ) -> (String, std::sync::Arc<std::sync::Mutex<Vec<u64>>>) {
        use std::io::BufRead;
        use std::sync::{Arc, Mutex};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file", listener.local_addr().unwrap());
        let ranges = Arc::new(Mutex::new(Vec::new()));
        let seen = ranges.clone();
        std::thread::spawn(move || {
            for (n, stream) in listener.incoming().enumerate() {
                let mut stream = stream.unwrap();
                let mut lines = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut from = 0u64;
                loop {
                    let mut line = String::new();
                    lines.read_line(&mut line).unwrap();
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some(r) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                        from = r.trim().trim_end_matches('-').parse().unwrap();
                    }
                }
                seen.lock().unwrap().push(from);
                let data: Vec<u8> = (from as usize..size).map(|i| (i % 251) as u8).collect();
                let head = if from > 0 {
                    format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {from}-{}/{size}\r\nContent-Length: {}\r\n\r\n",
                        size - 1,
                        data.len()
                    )
                } else {
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", data.len())
                };
                std::thread::spawn(move || {
                    if stream.write_all(head.as_bytes()).is_err() {
                        return;
                    }
                    if trickle(n) {
                        for b in data {
                            std::thread::sleep(Duration::from_millis(250));
                            if stream.write_all(&[b]).is_err() {
                                return;
                            }
                        }
                    } else {
                        let _ = stream.write_all(&data);
                    }
                });
            }
        });
        (url, ranges)
    }

    fn download(url: &str, size: u64, name: &str) -> (Result<(u64, String)>, Vec<u8>) {
        let to = std::env::temp_dir().join(format!("swofty-stall-{}-{name}", std::process::id()));
        let too_long = || "too long".to_string();
        let f = Fetch {
            url,
            size,
            lister: "the test",
            too_long: &too_long,
        };
        let mut open = |from: u64| {
            let mut request = ureq::get(url).header("Accept-Encoding", "identity");
            if from > 0 {
                request = request.header("Range", format!("bytes={from}-"));
            }
            request
                .config()
                .http_status_as_error(false)
                .build()
                .call()
                .map_err(Opening::Call)
        };
        let out = fetch(&f, &to, &mut open, &|_| {});
        let bytes = std::fs::read(&to).unwrap_or_default();
        let _ = std::fs::remove_file(&to);
        (out, bytes)
    }

    fn expected(size: usize) -> Vec<u8> {
        (0..size).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn a_trickling_connection_reconnects_and_resumes_with_a_range() {
        let (url, ranges) = serve(5000, |n| n == 0);
        let (out, bytes) = download(&url, 5000, "resume");
        let (len, _) = out.unwrap();
        assert_eq!(len, 5000);
        assert_eq!(bytes, expected(5000));
        let ranges = ranges.lock().unwrap();
        assert_eq!(ranges.len(), 2);
        assert_eq!(ranges[0], 0);
        assert!(ranges[1] > 0 && ranges[1] < 64);
    }

    #[test]
    fn a_connection_that_always_trickles_fails_with_the_speed_error() {
        let (url, ranges) = serve(5000, |_| true);
        let (out, _) = download(&url, 5000, "always");
        let Err(Error::Other(message)) = out else {
            panic!("expected the stall error");
        };
        assert!(message.contains("stayed below"), "{message}");
        assert!(message.contains("5 times"), "{message}");
        assert!(message.contains("check your connection or VPN"), "{message}");
        assert_eq!(ranges.lock().unwrap().len(), STALLS as usize);
    }

    #[test]
    fn a_normal_speed_download_is_untouched() {
        let (url, ranges) = serve(300_000, |_| false);
        let (out, bytes) = download(&url, 300_000, "fast");
        assert_eq!(out.unwrap().0, 300_000);
        assert_eq!(bytes, expected(300_000));
        assert_eq!(*ranges.lock().unwrap(), vec![0]);
    }

    #[test]
    fn names_the_host_of_a_url() {
        assert_eq!(host("https://api.f1.swofty.net/a/b?c"), "api.f1.swofty.net");
        assert_eq!(host("http://127.0.0.1:80/x"), "127.0.0.1:80");
    }
}
