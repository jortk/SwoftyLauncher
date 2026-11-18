//! The integrity module: a closed-source native addon the Time Trial boards server publishes, which
//! appends an attestation to every lap while the player is signed in with Time Trial boards on.
//!
//! The server offers one module at a time at `GET /v1/tt/integrity`. [`ensure`] keeps a verified copy
//! under `<data>/timetrial/integrity/<version>/` and nothing is placed in the game folder unless the
//! offer's signature checks out against the key pinned here. Every failure is an `Err` with one line of
//! text for the log; play goes on without the module.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ring::signature::{ED25519, UnparsedPublicKey};
use serde::Deserialize;

use crate::baseline::sha256_bytes;
use crate::timetrial::api::Api;

/// The addon id the module loads under. It is not a user addon: no profile, addon list or fingerprint has it.
pub const PLUGIN_ID: &str = "swofty.integrity";
pub const DLL: &str = "swofty_integrity.dll";
/// Base64 of the Ed25519 public key whose private half signs published modules.
pub(crate) const PUBLIC_KEY: &str = "Y79gick8AdEJDzKoMDuiVdEV5UYuc2xgxJ+V7VGgZxg=";
const SIGNED_PREFIX: &str = "swofty-tt-integrity-v1";
const MAX_SIZE: u64 = 16 * 1024 * 1024;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60);
const KEEP: usize = 2;
/// Names a locally built module to place for the session in place of the server's, for testing a build
/// before it is published. Unset in normal use.
pub const LOCAL_ENV: &str = "SWOFTY_TT_INTEGRITY_DLL";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub version: String,
    pub dll: PathBuf,
    pub sha256: String,
}

#[derive(Deserialize)]
struct Offer {
    version: String,
    sha256: String,
    size: u64,
    signature: String,
}

pub fn dir(data: &Path) -> PathBuf {
    data.join("timetrial").join("integrity")
}

/// The file [`LOCAL_ENV`] names, when it is set: `None` when it is unset or empty, else that file as a
/// module with the version `local`, or why it cannot be used. No server is asked and no signature is checked.
pub fn local() -> Option<Result<Module, String>> {
    let path = std::env::var_os(LOCAL_ENV).filter(|p| !p.is_empty())?;
    Some(local_at(Path::new(&path)))
}

fn local_at(path: &Path) -> Result<Module, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    if bytes.len() as u64 > MAX_SIZE || !bytes.starts_with(b"MZ") {
        return Err(format!("{} is not a Windows DLL under 16 MiB", path.display()));
    }
    Ok(Module {
        version: "local".to_owned(),
        dll: path.to_path_buf(),
        sha256: sha256_bytes(&bytes),
    })
}

/// The verified module for this session: the cached copy when it has the offered hash, else a download.
pub fn ensure(data: &Path, api: &Api) -> Result<Module, String> {
    let key = decode(PUBLIC_KEY).ok_or("the pinned integrity key is not valid base64")?;
    ensure_with(data, api, &key)
}

fn ensure_with(data: &Path, api: &Api, key: &[u8]) -> Result<Module, String> {
    let offer = api.integrity().map_err(|e| e.message)?;
    let offer: Offer = serde_json::from_value(offer).map_err(|e| format!("the offer is not valid: {e}"))?;
    if !valid_version(&offer.version) {
        return Err("the offered version is not a plain version string".into());
    }
    if offer.sha256.len() != 64
        || !offer
            .sha256
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err("the offered hash is not a lowercase sha256".into());
    }
    if offer.size == 0 || offer.size > MAX_SIZE {
        return Err(format!(
            "the offered size of {} bytes is out of range",
            offer.size
        ));
    }
    verify(key, &offer)?;
    let folder = dir(data).join(&offer.version);
    let dll = folder.join(DLL);
    let module = Module {
        version: offer.version.clone(),
        dll: dll.clone(),
        sha256: offer.sha256.clone(),
    };
    if std::fs::read(&dll).is_ok_and(|b| b.len() as u64 == offer.size && sha256_bytes(&b) == offer.sha256) {
        prune(data, &offer.version);
        return Ok(module);
    }
    let bytes = api
        .integrity_file(&offer.version, offer.size, DOWNLOAD_TIMEOUT)
        .map_err(|e| e.message)?;
    if bytes.len() as u64 != offer.size {
        return Err(format!(
            "the download is {} bytes, the offer says {}",
            bytes.len(),
            offer.size
        ));
    }
    if sha256_bytes(&bytes) != offer.sha256 {
        return Err("the download does not have the offered sha256".into());
    }
    store(&folder, &dll, &bytes).map_err(|e| format!("storing the module: {e}"))?;
    prune(data, &offer.version);
    Ok(module)
}

fn verify(key: &[u8], offer: &Offer) -> Result<(), String> {
    let signature = decode(&offer.signature).ok_or("the signature is not valid base64")?;
    let mut message = Vec::new();
    message.extend_from_slice(SIGNED_PREFIX.as_bytes());
    message.push(0);
    message.extend_from_slice(offer.version.as_bytes());
    message.push(0);
    message.extend_from_slice(offer.sha256.as_bytes());
    UnparsedPublicKey::new(&ED25519, key)
        .verify(&message, &signature)
        .map_err(|_| "the signature does not match the pinned key".to_owned())
}

/// A version becomes a folder name, so it is plain characters only.
pub fn valid_version(v: &str) -> bool {
    (1..=32).contains(&v.len())
        && !v.starts_with('.')
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn store(folder: &Path, dll: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(folder)?;
    let tmp = folder.join(format!("{DLL}.{}.tmp", std::process::id()));
    let done = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, dll));
    if done.is_err() {
        std::fs::remove_file(&tmp).ok();
    }
    done
}

/// Keep `current` and the newest other version; the rest go.
fn prune(data: &Path, current: &str) {
    let Ok(entries) = std::fs::read_dir(dir(data)) else {
        return;
    };
    let mut others: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| valid_version(n) && n != current)
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    others.sort_by_key(|o| std::cmp::Reverse(o.0));
    for (_, path) in others.into_iter().skip(KEEP - 1) {
        std::fs::remove_dir_all(path).ok();
    }
}

/// Standard base64, padded or not.
pub(crate) fn decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for b in text.trim_end_matches('=').bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// Add `module` to a build's placed files and plugin list so the loader loads it as [`PLUGIN_ID`].
pub fn place(
    module: &Module,
    loader: &mut std::collections::BTreeMap<String, PathBuf>,
    plugins: &mut Vec<String>,
) {
    crate::modules::place_file(PLUGIN_ID, DLL, &module.dll, loader, plugins);
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{Ed25519KeyPair, KeyPair};
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};

    const BYTES: &[u8] = b"MZ not really a dll";

    pub(crate) fn pair() -> Ed25519KeyPair {
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
        Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap()
    }

    pub(crate) fn b64(bytes: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for c in bytes.chunks(3) {
            let n = c
                .iter()
                .enumerate()
                .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
            for i in 0..=c.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            }
            out.extend(std::iter::repeat_n('=', 3 - c.len()));
        }
        out
    }

    fn sign(key: &Ed25519KeyPair, version: &str, sha: &str) -> String {
        let msg = format!("{SIGNED_PREFIX}\0{version}\0{sha}");
        b64(key.sign(msg.as_bytes()).as_ref())
    }

    fn offer(key: &Ed25519KeyPair, version: &str, bytes: &[u8]) -> serde_json::Value {
        let sha = sha256_bytes(bytes);
        serde_json::json!({
            "version": version, "sha256": sha, "size": bytes.len(),
            "signature": sign(key, version, &sha),
            "url": format!("/v1/tt/integrity/file?version={version}"),
        })
    }

    pub(crate) type Route = (&'static str, &'static str, Vec<u8>);

    pub(crate) fn serve(
        routes: Vec<Route>,
        requests: usize,
    ) -> (String, Arc<Mutex<Vec<String>>>, std::thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let t = std::thread::spawn(move || {
            for _ in 0..requests {
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
                let line = String::from_utf8_lossy(&got)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_owned();
                let path = line.split(' ').nth(1).unwrap_or("").to_owned();
                log.lock().unwrap().push(path.clone());
                let (status, body) = routes.iter().find(|(p, _, _)| path.starts_with(p)).map_or(
                    ("404 Not Found", br#"{"error":"nothing here"}"#.to_vec()),
                    |(_, s, b)| (*s, b.clone()),
                );
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                c.write_all(head.as_bytes()).ok();
                c.write_all(&body).ok();
            }
        });
        (url, seen, t)
    }

    pub(crate) fn json(v: &serde_json::Value) -> Vec<u8> {
        v.to_string().into_bytes()
    }

    fn data(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("swofty-integ-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&d).ok();
        d
    }

    fn run(
        name: &str,
        key: &Ed25519KeyPair,
        routes: Vec<Route>,
        requests: usize,
    ) -> (Result<Module, String>, PathBuf, Vec<String>) {
        let d = data(name);
        let (url, seen, t) = serve(routes, requests);
        let got = ensure_with(&d, &Api::with_server(&url, "tok"), key.public_key().as_ref());
        t.join().unwrap();
        let seen = seen.lock().unwrap().clone();
        (got, d, seen)
    }

    #[test]
    fn the_pinned_key_is_an_ed25519_public_key() {
        assert_eq!(decode(PUBLIC_KEY).unwrap().len(), 32);
        assert_eq!(decode("aGk=").unwrap(), b"hi");
        assert_eq!(decode("aGk").unwrap(), b"hi");
        assert!(decode("a$k=").is_none());
    }

    #[test]
    fn the_pinned_key_accepts_the_servers_signature_format() {
        let offer = Offer {
            version: "kat".to_owned(),
            sha256: "0".repeat(64),
            size: 1,
            signature:
                "zZcTrXukQHWeNKBjlHN+WEge6rdUdGhnGofCPjCQVXy4FyEs2Fs7H0Vugik9LDAeaHmV09oJP3/zQGEYwO2vBw=="
                    .to_owned(),
        };
        verify(&decode(PUBLIC_KEY).unwrap(), &offer).unwrap();
    }

    #[test]
    fn downloads_verifies_and_stores_a_signed_module() {
        let key = pair();
        let routes = vec![
            (
                "/v1/tt/integrity/file?version=2026.11.10.1",
                "200 OK",
                BYTES.to_vec(),
            ),
            (
                "/v1/tt/integrity",
                "200 OK",
                json(&offer(&key, "2026.11.10.1", BYTES)),
            ),
        ];
        let (got, d, seen) = run("good", &key, routes, 2);
        let m = got.unwrap();
        assert_eq!(m.version, "2026.11.10.1");
        assert_eq!(m.sha256, sha256_bytes(BYTES));
        assert_eq!(m.dll, dir(&d).join("2026.11.10.1").join(DLL));
        assert_eq!(std::fs::read(&m.dll).unwrap(), BYTES);
        assert_eq!(
            seen,
            ["/v1/tt/integrity", "/v1/tt/integrity/file?version=2026.11.10.1"]
        );
        assert_eq!(
            std::fs::read_dir(dir(&d).join("2026.11.10.1")).unwrap().count(),
            1
        );
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn a_cached_copy_with_the_offered_hash_is_not_downloaded_again() {
        let key = pair();
        let d = data("cache");
        let folder = dir(&d).join("v1");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(DLL), BYTES).unwrap();
        let (url, seen, t) = serve(
            vec![("/v1/tt/integrity", "200 OK", json(&offer(&key, "v1", BYTES)))],
            1,
        );
        let m = ensure_with(&d, &Api::with_server(&url, "tok"), key.public_key().as_ref()).unwrap();
        t.join().unwrap();
        assert_eq!(std::fs::read(m.dll).unwrap(), BYTES);
        assert_eq!(*seen.lock().unwrap(), ["/v1/tt/integrity"]);
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn a_cached_copy_with_other_bytes_is_replaced() {
        let key = pair();
        let d = data("stale");
        let folder = dir(&d).join("v1");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(DLL), b"tampered").unwrap();
        let routes = vec![
            ("/v1/tt/integrity/file", "200 OK", BYTES.to_vec()),
            ("/v1/tt/integrity", "200 OK", json(&offer(&key, "v1", BYTES))),
        ];
        let (url, _, t) = serve(routes, 2);
        let m = ensure_with(&d, &Api::with_server(&url, "tok"), key.public_key().as_ref()).unwrap();
        t.join().unwrap();
        assert_eq!(std::fs::read(m.dll).unwrap(), BYTES);
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn a_bad_signature_is_refused_before_any_download() {
        let key = pair();
        let other = pair();
        let routes = vec![("/v1/tt/integrity", "200 OK", json(&offer(&other, "v1", BYTES)))];
        let (got, d, seen) = run("badsig", &key, routes, 1);
        assert!(got.unwrap_err().contains("signature"));
        assert_eq!(seen.len(), 1);
        assert!(!dir(&d).exists());
    }

    #[test]
    fn a_signature_over_another_version_is_refused() {
        let key = pair();
        let mut o = offer(&key, "v1", BYTES);
        o["version"] = "v2".into();
        let (got, ..) = run("swapped", &key, vec![("/v1/tt/integrity", "200 OK", json(&o))], 1);
        assert!(got.unwrap_err().contains("signature"));
    }

    #[test]
    fn a_download_with_the_wrong_hash_is_not_stored() {
        let key = pair();
        let routes = vec![
            ("/v1/tt/integrity/file", "200 OK", b"MZ not really a dlL".to_vec()),
            ("/v1/tt/integrity", "200 OK", json(&offer(&key, "v1", BYTES))),
        ];
        let (got, d, _) = run("badhash", &key, routes, 2);
        assert!(got.unwrap_err().contains("sha256"));
        assert!(!dir(&d).join("v1").join(DLL).exists());
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn a_download_longer_than_offered_is_refused() {
        let key = pair();
        let routes = vec![
            ("/v1/tt/integrity/file", "200 OK", [BYTES, b" and more"].concat()),
            ("/v1/tt/integrity", "200 OK", json(&offer(&key, "v1", BYTES))),
        ];
        let (got, d, _) = run("long", &key, routes, 2);
        assert!(got.is_err());
        assert!(!dir(&d).join("v1").join(DLL).exists());
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn an_offer_over_the_size_limit_is_refused_unfetched() {
        let key = pair();
        let mut o = offer(&key, "v1", BYTES);
        o["size"] = (MAX_SIZE + 1).into();
        let (got, _, seen) = run("big", &key, vec![("/v1/tt/integrity", "200 OK", json(&o))], 1);
        assert!(got.unwrap_err().contains("out of range"));
        assert_eq!(seen.len(), 1);
    }

    #[test]
    fn a_version_that_is_not_a_plain_name_is_refused() {
        let key = pair();
        for v in ["../x", ".hidden", "a/b", "a\\b", "", "x y", &"v".repeat(33)] {
            let (got, d, seen) = run(
                "trav",
                &key,
                vec![("/v1/tt/integrity", "200 OK", json(&offer(&key, v, BYTES)))],
                1,
            );
            assert!(got.unwrap_err().contains("version"), "{v}");
            assert_eq!(seen.len(), 1);
            assert!(!dir(&d).exists());
        }
        assert!(valid_version("2026.11.10.1"));
        assert!(valid_version("a_b-c"));
    }

    #[test]
    fn nothing_is_stored_when_offline_or_unpublished() {
        let d = data("offline");
        let e = ensure(&d, &Api::with_server("http://127.0.0.1:1", "tok")).unwrap_err();
        assert!(e.contains("couldn't reach"));
        assert!(!dir(&d).exists());
        let key = pair();
        let routes = vec![(
            "/v1/tt/integrity",
            "404 Not Found",
            br#"{"error":"no integrity module is published"}"#.to_vec(),
        )];
        let (got, d, _) = run("none", &key, routes, 1);
        assert_eq!(got.unwrap_err(), "no integrity module is published");
        assert!(!dir(&d).exists());
    }

    #[test]
    fn the_pinned_key_does_not_accept_a_test_signature() {
        let key = pair();
        let d = data("pinned");
        let (url, _, t) = serve(
            vec![("/v1/tt/integrity", "200 OK", json(&offer(&key, "v1", BYTES)))],
            1,
        );
        let e = ensure(&d, &Api::with_server(&url, "tok")).unwrap_err();
        t.join().unwrap();
        assert!(e.contains("pinned key"));
    }

    #[cfg(unix)]
    #[test]
    fn older_versions_are_pruned_to_two() {
        let key = pair();
        let d = data("prune");
        for (i, v) in ["a1", "a2", "a3"].iter().enumerate() {
            let folder = dir(&d).join(v);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join(DLL), b"old").unwrap();
            let when = std::time::SystemTime::now() - Duration::from_secs(1000 - 100 * i as u64);
            std::fs::File::open(&folder).unwrap().set_modified(when).unwrap();
        }
        let routes = vec![
            ("/v1/tt/integrity/file", "200 OK", BYTES.to_vec()),
            ("/v1/tt/integrity", "200 OK", json(&offer(&key, "b1", BYTES))),
        ];
        let (url, _, t) = serve(routes, 2);
        ensure_with(&d, &Api::with_server(&url, "tok"), key.public_key().as_ref()).unwrap();
        t.join().unwrap();
        let mut left: Vec<String> = std::fs::read_dir(dir(&d))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(left, ["a3", "b1"]);
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn a_local_build_is_used_as_it_is_and_a_file_that_is_no_dll_is_refused() {
        let d = data("local");
        std::fs::create_dir_all(&d).unwrap();
        let dll = d.join(DLL);
        std::fs::write(&dll, BYTES).unwrap();
        let m = local_at(&dll).unwrap();
        assert_eq!(
            (m.version.as_str(), &m.dll, m.sha256),
            ("local", &dll, sha256_bytes(BYTES))
        );
        std::fs::write(&dll, b"not a dll").unwrap();
        assert!(local_at(&dll).unwrap_err().contains("not a Windows DLL"));
        assert!(local_at(&d.join("missing.dll")).unwrap_err().contains("reading"));
        std::fs::remove_dir_all(d).ok();
    }

    #[test]
    fn placing_the_module_adds_one_file_and_one_plugin() {
        let m = Module {
            version: "v1".into(),
            dll: PathBuf::from("/data/timetrial/integrity/v1").join(DLL),
            sha256: String::new(),
        };
        let mut loader = std::collections::BTreeMap::new();
        let mut plugins = vec!["addons/other/o.dll".to_owned()];
        place(&m, &mut loader, &mut plugins);
        assert_eq!(
            loader.into_iter().collect::<Vec<_>>(),
            [(
                "swofty/addons/swofty.integrity/swofty_integrity.dll".to_owned(),
                m.dll.clone()
            )]
        );
        assert_eq!(
            plugins,
            [
                "addons/other/o.dll",
                "addons/swofty.integrity/swofty_integrity.dll"
            ]
        );
    }
}
