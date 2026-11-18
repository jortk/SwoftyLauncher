use super::*;
use crate::timetrial::integrity::tests::{Route, b64, json, pair, serve};
use ring::signature::{Ed25519KeyPair, KeyPair};

const BYTES: &[u8] = b"MZ not really a dll";
const NAME: &str = "tt-game";
const FILE: &str = "swofty_tt.dll";
const LIST: &str = "/v1/modules?";
const DOWNLOAD: &str = "/v1/modules/file?";

fn offer_of(key: &Ed25519KeyPair, version: &str, abi: u32, bytes: &[u8]) -> serde_json::Value {
    let sha = sha256_bytes(bytes);
    let msg = format!("swofty-module-v1\0{NAME}\0{WINDOWS}\0{version}\0{abi}\0{sha}");
    serde_json::json!({
        "name": NAME, "platform": WINDOWS, "version": version, "abi": abi, "sha256": sha,
        "size": bytes.len(), "signature": b64(key.sign(msg.as_bytes()).as_ref()), "file": FILE,
        "url": format!("/v1/modules/file?name={NAME}&platform={WINDOWS}&version={version}"),
    })
}

fn listing(offers: &[serde_json::Value]) -> Vec<u8> {
    json(&serde_json::json!({"modules": offers, "refused": []}))
}

fn refused(why: &str) -> Vec<u8> {
    json(&serde_json::json!({"modules": [], "refused": [{"name": NAME, "why": why}]}))
}

fn data(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("swofty-modules-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    d
}

fn base(d: &Path) -> PathBuf {
    dir(d, NAME, WINDOWS)
}

fn ask(
    d: &Path,
    key: &Ed25519KeyPair,
    routes: Vec<Route>,
    requests: usize,
) -> (Result<Module, Unavailable>, Vec<String>) {
    let (url, seen, t) = serve(routes, requests);
    let got = ensure_with(
        d,
        &Api::with_server(&url, "tok"),
        NAME,
        WINDOWS,
        1,
        key.public_key().as_ref(),
    );
    t.join().unwrap();
    let seen = seen.lock().unwrap().clone();
    (got, seen)
}

fn run(
    name: &str,
    key: &Ed25519KeyPair,
    routes: Vec<Route>,
    requests: usize,
) -> (Result<Module, Unavailable>, PathBuf, Vec<String>) {
    let d = data(name);
    let (got, seen) = ask(&d, key, routes, requests);
    (got, d, seen)
}

fn offline(d: &Path, key: &Ed25519KeyPair, abi: u32) -> Result<Module, Unavailable> {
    ensure_with(
        d,
        &Api::with_server("http://127.0.0.1:1", "tok"),
        NAME,
        WINDOWS,
        abi,
        key.public_key().as_ref(),
    )
}

fn fetch(d: &Path, key: &Ed25519KeyPair, version: &str, abi: u32, bytes: &[u8]) -> Module {
    let (url, _, t) = serve(
        vec![
            (DOWNLOAD, "200 OK", bytes.to_vec()),
            (LIST, "200 OK", listing(&[offer_of(key, version, abi, bytes)])),
        ],
        2,
    );
    let got = ensure_with(
        d,
        &Api::with_server(&url, "tok"),
        NAME,
        WINDOWS,
        abi,
        key.public_key().as_ref(),
    );
    t.join().unwrap();
    got.unwrap()
}

#[test]
fn the_platform_is_one_the_server_knows() {
    assert!([LINUX, WINDOWS].contains(&platform()));
    assert_eq!(platform() == WINDOWS, cfg!(windows));
    assert_eq!(GAME_PLATFORM, "windows-x86_64");
}

/// The public key of the seed `00 01 .. 1f` and the signature the server's signing code makes with it.
const KAT_KEY: &str = "A6EHv/POEL4dcN0Y50vAmWfk1jCbpQ1fHdyGZBJVMbg=";
const KAT: &str = "T+TYC+DN9BXybTH6n671PBYVXJkF4+9K7IACspP5Zhy4QWkU2yiuCdxwFJmpgrcujuRPHN1JUZ+3KKSQge3mBw==";

#[test]
fn the_servers_signature_format_is_accepted() {
    let offer = Offer {
        name: "tt-game".into(),
        platform: "windows-x86_64".into(),
        version: "2026.11.17.1".into(),
        abi: 1,
        sha256: "0".repeat(64),
        size: 1,
        signature: KAT.into(),
        file: FILE.into(),
        cdn_url: Some("https://cdn.example/not-signed".into()),
    };
    assert_eq!(
        signed_message(&offer),
        format!(
            "swofty-module-v1\0tt-game\0windows-x86_64\x002026.11.17.1\x001\0{}",
            "0".repeat(64)
        )
        .into_bytes()
    );
    check(&decode(KAT_KEY).unwrap(), &offer).unwrap();
    assert!(check(&decode(PUBLIC_KEY).unwrap(), &offer).is_err());
    assert!(check(&decode(KAT_KEY).unwrap(), &Offer { abi: 2, ..offer }).is_err());
}

#[test]
fn downloads_verifies_and_stores_a_signed_module() {
    let key = pair();
    let routes = vec![
        (DOWNLOAD, "200 OK", BYTES.to_vec()),
        (
            LIST,
            "200 OK",
            listing(&[offer_of(&key, "2026.11.17.1", 1, BYTES)]),
        ),
    ];
    let (got, d, seen) = run("good", &key, routes, 2);
    let m = got.unwrap();
    let folder = base(&d).join("2026.11.17.1");
    assert_eq!(
        m,
        Module {
            name: NAME.into(),
            version: "2026.11.17.1".into(),
            path: folder.join(FILE),
            sha256: sha256_bytes(BYTES),
        }
    );
    assert!(!m.is_local());
    assert_eq!(m.file_name(), FILE);
    assert_eq!(std::fs::read(&m.path).unwrap(), BYTES);
    assert_eq!(
        seen,
        [
            "/v1/modules?platform=windows-x86_64&abi.tt-game=1",
            "/v1/modules/file?name=tt-game&platform=windows-x86_64&version=2026.11.17.1"
        ]
    );
    let mut files: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    files.sort();
    assert_eq!(files, [OFFER_FILE, FILE]);
    assert_eq!(folder, d.join("modules/tt-game/windows-x86_64/2026.11.17.1"));
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn a_cached_copy_with_the_offered_hash_is_not_downloaded_again() {
    let key = pair();
    let d = data("cache");
    fetch(&d, &key, "1.0", 1, BYTES);
    let (got, seen) = ask(
        &d,
        &key,
        vec![(LIST, "200 OK", listing(&[offer_of(&key, "1.0", 1, BYTES)]))],
        1,
    );
    assert_eq!(std::fs::read(got.unwrap().path).unwrap(), BYTES);
    assert_eq!(seen.len(), 1);
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn a_cached_copy_with_other_bytes_is_replaced() {
    let key = pair();
    let d = data("stale");
    let m = fetch(&d, &key, "1.0", 1, BYTES);
    std::fs::write(&m.path, b"MZ tampered with it").unwrap();
    let routes = vec![
        (DOWNLOAD, "200 OK", BYTES.to_vec()),
        (LIST, "200 OK", listing(&[offer_of(&key, "1.0", 1, BYTES)])),
    ];
    let (got, seen) = ask(&d, &key, routes, 2);
    assert_eq!(std::fs::read(got.unwrap().path).unwrap(), BYTES);
    assert_eq!(seen.len(), 2);
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn a_newer_offer_replaces_the_cached_version() {
    let key = pair();
    let d = data("newer");
    fetch(&d, &key, "1.0", 1, BYTES);
    let m = fetch(&d, &key, "1.1", 1, b"MZ the next build");
    assert_eq!(m.version, "1.1");
    assert_eq!(std::fs::read(&m.path).unwrap(), b"MZ the next build");
    assert!(base(&d).join("1.0").join(FILE).exists());
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn the_servers_refusal_is_the_reason() {
    let key = pair();
    for why in ["not signed in", "needs the beta role", "none published"] {
        let (got, d, seen) = run("refused", &key, vec![(LIST, "200 OK", refused(why))], 1);
        assert_eq!(got.unwrap_err().to_string(), why);
        assert_eq!(seen.len(), 1);
        assert!(!d.join("modules").exists());
    }
    let (got, ..) = run("silent", &key, vec![(LIST, "200 OK", listing(&[]))], 1);
    assert_eq!(got.unwrap_err().why, "the server did not offer it");
    let (got, ..) = run("old", &key, vec![], 1);
    assert_eq!(got.unwrap_err().why, "the server does not offer modules");
    let banned = br#"{"error":"too many requests; try again later"}"#.to_vec();
    let (got, ..) = run("http", &key, vec![(LIST, "403 Forbidden", banned)], 1);
    assert_eq!(got.unwrap_err().why, "too many requests; try again later");
}

#[test]
fn a_refusal_does_not_fall_back_to_the_cache() {
    let key = pair();
    let d = data("revoked");
    fetch(&d, &key, "1.0", 1, BYTES);
    let (got, _) = ask(
        &d,
        &key,
        vec![(LIST, "200 OK", refused("needs the beta role"))],
        1,
    );
    assert_eq!(got.unwrap_err().why, "needs the beta role");
    let gone = br#"{"error":"sign in with Discord in the launcher first"}"#.to_vec();
    let (got, _) = ask(&d, &key, vec![(LIST, "401 Unauthorized", gone)], 1);
    assert!(got.unwrap_err().why.contains("sign in"));
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn offline_uses_the_newest_cached_copy_of_the_abi() {
    let key = pair();
    let d = data("offline");
    assert!(offline(&d, &key, 1).unwrap_err().why.contains("couldn't reach"));
    assert!(!d.join("modules").exists());
    fetch(&d, &key, "1.9", 1, b"MZ nine");
    fetch(&d, &key, "1.10", 1, b"MZ ten");
    let m = offline(&d, &key, 1).unwrap();
    assert_eq!(
        (m.version.as_str(), m.sha256.clone()),
        ("1.10", sha256_bytes(b"MZ ten"))
    );
    assert_eq!(m.path, base(&d).join("1.10").join(FILE));
    assert!(offline(&d, &key, 2).unwrap_err().why.contains("couldn't reach"));
    fetch(&d, &key, "2.0", 2, b"MZ two");
    assert_eq!(offline(&d, &key, 2).unwrap().version, "2.0");
    assert_eq!(offline(&d, &key, 1).unwrap().version, "1.10");
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn a_busy_server_counts_as_unreachable() {
    let key = pair();
    let d = data("busy");
    fetch(&d, &key, "1.0", 1, BYTES);
    for status in ["502 Bad Gateway", "429 Too Many Requests"] {
        let (got, _) = ask(&d, &key, vec![(LIST, status, b"{}".to_vec())], 1);
        assert_eq!(got.unwrap().version, "1.0");
    }
    std::fs::remove_dir_all(d).ok();
}

fn with_cdn(mut offer: serde_json::Value, url: &str) -> serde_json::Value {
    offer["cdn_url"] = url.into();
    offer
}

/// One request answered with `status` and `body`; the whole request as it arrived.
fn cdn(status: &str, body: Vec<u8>) -> (String, std::thread::JoinHandle<String>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/modules/tt-game/{FILE}", listener.local_addr().unwrap());
    let status = status.to_owned();
    let t = std::thread::spawn(move || {
        let (mut c, _) = listener.accept().unwrap();
        let mut got = Vec::new();
        let mut buf = [0u8; 4096];
        while !got.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = c.read(&mut buf).unwrap();
            if n == 0 {
                break;
            }
            got.extend_from_slice(&buf[..n]);
        }
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        c.write_all(head.as_bytes()).ok();
        c.write_all(&body).ok();
        String::from_utf8_lossy(&got).into_owned()
    });
    (url, t)
}

#[test]
fn a_cdn_link_is_downloaded_without_the_sign_in_and_the_server_is_only_listed() {
    let key = pair();
    let (url, t) = cdn("200 OK", BYTES.to_vec());
    let offer = with_cdn(offer_of(&key, "1.0", 1, BYTES), &url);
    let (got, d, seen) = run("cdn", &key, vec![(LIST, "200 OK", listing(&[offer]))], 1);
    let asked = t.join().unwrap();
    let m = got.unwrap();
    assert_eq!(std::fs::read(&m.path).unwrap(), BYTES);
    assert_eq!(seen, ["/v1/modules?platform=windows-x86_64&abi.tt-game=1"]);
    assert!(asked.starts_with(&format!("GET /modules/tt-game/{FILE} ")));
    assert!(!asked.to_ascii_lowercase().contains("authorization"), "{asked}");
    assert!(!asked.contains("tok"), "{asked}");
    let stored: Offer =
        serde_json::from_slice(&std::fs::read(m.path.with_file_name(OFFER_FILE)).unwrap()).unwrap();
    assert_eq!(stored.cdn_url.as_deref(), Some(url.as_str()));
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn a_cdn_that_fails_or_serves_other_bytes_leaves_the_download_to_the_server() {
    let key = pair();
    for (label, status, body) in [
        ("cdn-404", "404 Not Found", b"gone".to_vec()),
        ("cdn-hash", "200 OK", b"MZ not really a dlL".to_vec()),
        ("cdn-long", "200 OK", [BYTES, b" and more"].concat()),
    ] {
        let (url, t) = cdn(status, body);
        let offer = with_cdn(offer_of(&key, "1.0", 1, BYTES), &url);
        let routes = vec![
            (DOWNLOAD, "200 OK", BYTES.to_vec()),
            (LIST, "200 OK", listing(&[offer])),
        ];
        let (got, d, seen) = run(label, &key, routes, 2);
        t.join().unwrap();
        assert_eq!(std::fs::read(got.unwrap().path).unwrap(), BYTES, "{label}");
        assert_eq!(seen.len(), 2, "{label}");
        std::fs::remove_dir_all(d).ok();
    }
}

#[test]
fn a_cdn_link_that_is_not_https_is_not_followed() {
    assert!(cdn_link("https://cdn.swofty.net/modules/x"));
    assert!(cdn_link("http://127.0.0.1:9/x"));
    assert!(!cdn_link("http://cdn.swofty.net/modules/x"));
    assert!(!cdn_link("file:///etc/passwd"));
    assert!(!cdn_link("ftp://cdn.swofty.net/x"));
    let key = pair();
    let offer = with_cdn(offer_of(&key, "1.0", 1, BYTES), "http://10.255.255.1:9/x");
    let routes = vec![
        (DOWNLOAD, "200 OK", BYTES.to_vec()),
        (LIST, "200 OK", listing(&[offer])),
    ];
    let (got, d, seen) = run("cdn-plain", &key, routes, 2);
    assert_eq!(std::fs::read(got.unwrap().path).unwrap(), BYTES);
    assert_eq!(seen.len(), 2);
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn a_busy_download_keeps_the_cached_copy_and_a_refused_one_does_not() {
    let key = pair();
    let d = data("busy-file");
    fetch(&d, &key, "1.0", 1, BYTES);
    let next = b"MZ the next build, longer than any error answer the server sends for it";
    let newer = || listing(&[offer_of(&key, "1.1", 1, next)]);
    let busy = br#"{"error":"too many requests; try again later"}"#.to_vec();
    for status in ["429 Too Many Requests", "503 Service Unavailable"] {
        let routes = vec![(DOWNLOAD, status, busy.clone()), (LIST, "200 OK", newer())];
        let (got, seen) = ask(&d, &key, routes, 2);
        assert_eq!(got.unwrap().version, "1.0", "{status}");
        assert_eq!(seen.len(), 2);
    }
    let routes = vec![
        (
            DOWNLOAD,
            "403 Forbidden",
            br#"{"error":"needs the beta role"}"#.to_vec(),
        ),
        (LIST, "200 OK", newer()),
    ];
    let (got, _) = ask(&d, &key, routes, 2);
    assert_eq!(got.unwrap_err().why, "needs the beta role");
    let routes = vec![
        (DOWNLOAD, "429 Too Many Requests", busy),
        (LIST, "200 OK", newer()),
    ];
    let (got, empty, _) = run("busy-empty", &key, routes, 2);
    assert_eq!(got.unwrap_err().why, "too many requests; try again later");
    std::fs::remove_dir_all(empty).ok();
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn offline_refuses_a_cached_copy_that_no_longer_verifies() {
    let key = pair();
    let d = data("offline-bad");
    fetch(&d, &key, "1.0", 1, BYTES);
    let newest = fetch(&d, &key, "1.1", 1, b"MZ the next build");
    std::fs::write(&newest.path, b"MZ the next builD").unwrap();
    assert_eq!(offline(&d, &key, 1).unwrap().version, "1.0");
    assert!(offline(&d, &pair(), 1).is_err());

    let folder = base(&d).join("1.0");
    let note = folder.join(OFFER_FILE);
    let good = std::fs::read(&note).unwrap();
    let mut doc: serde_json::Value = serde_json::from_slice(&good).unwrap();
    doc["abi"] = 2.into();
    std::fs::write(&note, json(&doc)).unwrap();
    assert!(offline(&d, &key, 2).is_err());
    assert!(offline(&d, &key, 1).is_err());
    std::fs::write(&note, &good).unwrap();
    std::fs::rename(&folder, base(&d).join("3.0")).unwrap();
    assert!(offline(&d, &key, 1).is_err());
    std::fs::rename(base(&d).join("3.0"), &folder).unwrap();
    assert_eq!(offline(&d, &key, 1).unwrap().version, "1.0");
    std::fs::write(folder.join(FILE), [BYTES, b"!"].concat()).unwrap();
    assert!(offline(&d, &key, 1).is_err());
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn a_bad_signature_is_refused_before_any_download() {
    let key = pair();
    let other = pair();
    let routes = vec![(LIST, "200 OK", listing(&[offer_of(&other, "1.0", 1, BYTES)]))];
    let (got, d, seen) = run("badsig", &key, routes, 1);
    assert!(got.unwrap_err().why.contains("signature"));
    assert_eq!(seen.len(), 1);
    assert!(!d.join("modules").exists());
}

#[test]
fn a_signature_covers_every_signed_field() {
    let key = pair();
    for (field, value) in [
        ("version", serde_json::json!("1.1")),
        ("sha256", serde_json::json!("0".repeat(64))),
    ] {
        let mut o = offer_of(&key, "1.0", 1, BYTES);
        o[field] = value;
        let (got, ..) = run("swapped", &key, vec![(LIST, "200 OK", listing(&[o]))], 1);
        assert!(got.unwrap_err().why.contains("signature"), "{field}");
    }
    let sha = sha256_bytes(BYTES);
    for (name, platform, abi) in [("other-game", WINDOWS, 1), (NAME, LINUX, 1), (NAME, WINDOWS, 2)] {
        let msg = format!("swofty-module-v1\0{name}\0{platform}\x001.0\0{abi}\0{sha}");
        let mut o = offer_of(&key, "1.0", 1, BYTES);
        o["signature"] = b64(key.sign(msg.as_bytes()).as_ref()).into();
        let (got, ..) = run("moved", &key, vec![(LIST, "200 OK", listing(&[o]))], 1);
        assert!(
            got.unwrap_err().why.contains("signature"),
            "{name} {platform} {abi}"
        );
    }
    let (got, ..) = run(
        "abi",
        &key,
        vec![(LIST, "200 OK", listing(&[offer_of(&key, "1.0", 2, BYTES)]))],
        1,
    );
    assert!(got.unwrap_err().why.contains("another platform or abi"));
}

#[test]
fn a_download_with_the_wrong_hash_or_size_is_not_stored() {
    let key = pair();
    for (label, body, why) in [
        ("badhash", b"MZ not really a dlL".to_vec(), "sha256"),
        ("long", [BYTES, b" and more"].concat(), ""),
        ("short", b"MZ".to_vec(), "bytes"),
    ] {
        let routes = vec![
            (DOWNLOAD, "200 OK", body),
            (LIST, "200 OK", listing(&[offer_of(&key, "1.0", 1, BYTES)])),
        ];
        let (got, d, _) = run(label, &key, routes, 2);
        assert!(got.unwrap_err().why.contains(why), "{label}");
        assert!(!base(&d).join("1.0").join(FILE).exists());
        assert!(offline(&d, &key, 1).is_err());
        std::fs::remove_dir_all(d).ok();
    }
}

#[test]
fn an_offer_that_is_not_plain_is_refused_unfetched() {
    let key = pair();
    let mut big = offer_of(&key, "1.0", 1, BYTES);
    big["size"] = (MAX_SIZE + 1).into();
    let (got, _, seen) = run("big", &key, vec![(LIST, "200 OK", listing(&[big]))], 1);
    assert!(got.unwrap_err().why.contains("out of range"));
    assert_eq!(seen.len(), 1);
    for v in ["../x", ".hidden", "a/b", "a\\b", "", "x y"] {
        let routes = vec![(LIST, "200 OK", listing(&[offer_of(&key, v, 1, BYTES)]))];
        let (got, d, seen) = run("trav", &key, routes, 1);
        assert!(got.unwrap_err().why.contains("version"), "{v}");
        assert_eq!(seen.len(), 1);
        assert!(!d.join("modules").exists());
    }
    for file in ["../x.dll", ".hidden", "a/b.dll", "", OFFER_FILE] {
        let mut o = offer_of(&key, "1.0", 1, BYTES);
        o["file"] = file.into();
        let (got, d, seen) = run("file", &key, vec![(LIST, "200 OK", listing(&[o]))], 1);
        assert!(got.unwrap_err().why.contains("file name"), "{file}");
        assert_eq!(seen.len(), 1);
        assert!(!d.join("modules").exists());
    }
    let d = data("name");
    for name in ["../x", "TT", "", "a b", "a&b=1"] {
        let api = Api::with_server("http://127.0.0.1:1", "tok");
        let e = ensure_with(&d, &api, name, WINDOWS, 1, key.public_key().as_ref()).unwrap_err();
        assert!(e.why.contains("plain name"), "{name}");
    }
}

#[test]
fn the_pinned_key_does_not_accept_a_test_signature() {
    let key = pair();
    let d = data("pinned");
    let (url, _, t) = serve(
        vec![(LIST, "200 OK", listing(&[offer_of(&key, "1.0", 1, BYTES)]))],
        1,
    );
    let e = ensure(&d, &Api::with_server(&url, "tok"), NAME, WINDOWS, 1).unwrap_err();
    t.join().unwrap();
    assert!(e.why.contains("pinned key"));
}

#[cfg(unix)]
#[test]
fn older_versions_are_pruned_to_two() {
    let key = pair();
    let d = data("prune");
    for (i, v) in ["0.1", "0.2", "0.3"].iter().enumerate() {
        let folder = base(&d).join(v);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(FILE), b"old").unwrap();
        let when = std::time::SystemTime::now() - Duration::from_secs(1000 - 100 * i as u64);
        std::fs::File::open(&folder).unwrap().set_modified(when).unwrap();
    }
    fetch(&d, &key, "1.0", 1, BYTES);
    let mut left: Vec<String> = std::fs::read_dir(base(&d))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    left.sort();
    assert_eq!(left, ["0.3", "1.0"]);
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn a_local_build_is_named_by_the_environment_and_used_as_it_is() {
    assert_eq!(local_env("tt-game"), "SWOFTY_MODULE_TT_GAME");
    assert_eq!(local_env("some-other-lib"), "SWOFTY_MODULE_SOME_OTHER_LIB");
    assert!(local("no-such-module-in-the-environment").is_none());
    let d = data("local");
    std::fs::create_dir_all(&d).unwrap();
    let file = d.join("swofty_other.dll");
    std::fs::write(&file, BYTES).unwrap();
    let m = local_at("other-game", &file).unwrap();
    assert_eq!(
        m,
        Module {
            name: "other-game".into(),
            version: "local".into(),
            path: file.clone(),
            sha256: sha256_bytes(BYTES),
        }
    );
    assert!(m.is_local());
    std::fs::write(&file, b"").unwrap();
    assert!(
        local_at("other-game", &file)
            .unwrap_err()
            .why
            .contains("not a module file")
    );
    assert!(
        local_at("other-game", &d.join("missing.dll"))
            .unwrap_err()
            .why
            .contains("reading")
    );
    std::fs::remove_dir_all(d).ok();
}

#[test]
fn placing_a_plugin_adds_one_file_and_one_plugin_under_its_id() {
    let m = Module {
        name: NAME.into(),
        version: "1.0".into(),
        path: PathBuf::from("/data/modules/tt-game/windows-x86_64/1.0").join(FILE),
        sha256: String::new(),
    };
    let mut loader = BTreeMap::new();
    let mut plugins = vec!["addons/other/o.dll".to_owned()];
    place(&m, TT_GAME.id, &mut loader, &mut plugins);
    assert_eq!(
        loader.into_iter().collect::<Vec<_>>(),
        [("swofty/addons/swofty.tt/swofty_tt.dll".to_owned(), m.path.clone())]
    );
    assert_eq!(plugins, ["addons/other/o.dll", "addons/swofty.tt/swofty_tt.dll"]);
}
