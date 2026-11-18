//! Server modules: signed files the server publishes and the launcher downloads at runtime.
//!
//! `GET /v1/modules` lists, per module name, the newest version that speaks the abi the launcher asks for.
//! [`ensure`] keeps a verified copy under `<data>/modules/<name>/<platform>/<version>/` and hands out a
//! [`Module`] only after the file's size, sha256 and Ed25519 signature check out against the key pinned in
//! [`crate::timetrial::integrity`]. The file comes from the offer's `cdn_url` when it has one and that copy
//! verifies, else from the server. Every failure is an [`Unavailable`] with one sentence for the log or the
//! page; the launcher goes on without the module.

pub mod library;

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ring::signature::{ED25519, UnparsedPublicKey};
use serde::{Deserialize, Serialize};

use crate::baseline::sha256_bytes;
use crate::timetrial::api::{Api, ApiError};
use crate::timetrial::integrity::{PUBLIC_KEY, decode, valid_version};

const SIGNED_PREFIX: &str = "swofty-module-v1";
const MAX_SIZE: u64 = 64 * 1024 * 1024;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const CDN_TIMEOUT: Duration = Duration::from_secs(60);
const CDN_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const KEEP: usize = 2;
const OFFER_FILE: &str = "offer.json";
const ENV_PREFIX: &str = "SWOFTY_MODULE_";
/// The version of a module [`local`] names.
pub const LOCAL_VERSION: &str = "local";
pub const LINUX: &str = "linux-x86_64";
pub const WINDOWS: &str = "windows-x86_64";
/// The platform of everything that loads inside the game, whatever the launcher runs on.
pub const GAME_PLATFORM: &str = WINDOWS;

/// An in-game plugin the server publishes: its module name, the addon id it loads under and the abi this
/// launcher speaks. The id is not a user addon: no profile, addon list or fingerprint has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plugin {
    pub name: &'static str,
    pub id: &'static str,
    pub abi: u32,
}

pub const TT_GAME: Plugin = Plugin {
    name: "tt-game",
    id: "swofty.tt",
    abi: 1,
};

/// Placed only for a game started with the session module's environment.
pub const MP_GAME: Plugin = Plugin {
    name: "mp-game",
    id: "swofty.mp",
    abi: 1,
};

/// The platform string of the running launcher.
pub fn platform() -> &'static str {
    if cfg!(windows) { WINDOWS } else { LINUX }
}

/// A verified file of a server module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub sha256: String,
}

impl Module {
    /// Whether this is a local build named by the environment, unsigned.
    pub fn is_local(&self) -> bool {
        self.version == LOCAL_VERSION
    }

    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// Why a module is not there for this session, in one sentence. A refusal carries the server's own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unavailable {
    pub why: String,
}

impl Unavailable {
    fn new(why: impl Into<String>) -> Self {
        Self { why: why.into() }
    }
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.why)
    }
}

impl std::error::Error for Unavailable {}

impl From<String> for Unavailable {
    fn from(why: String) -> Self {
        Self { why }
    }
}

impl From<&str> for Unavailable {
    fn from(why: &str) -> Self {
        Self::new(why)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Offer {
    name: String,
    platform: String,
    version: String,
    abi: u32,
    sha256: String,
    size: u64,
    signature: String,
    file: String,
    /// Where the same bytes are served without the sign-in; the size, sha256 and signature are checked the same.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cdn_url: Option<String>,
}

#[derive(Deserialize)]
struct Refusal {
    name: String,
    why: String,
}

#[derive(Deserialize)]
struct Listing {
    #[serde(default)]
    modules: Vec<Offer>,
    #[serde(default)]
    refused: Vec<Refusal>,
}

/// `<data>/modules/<name>/<platform>`, the folder that holds one folder per cached version.
pub fn dir(data: &Path, name: &str, platform: &str) -> PathBuf {
    data.join("modules").join(name).join(platform)
}

/// The environment variable that names a local build of `name`: `SWOFTY_MODULE_` and the name in upper case
/// with `_` for `-`.
pub fn local_env(name: &str) -> String {
    format!("{ENV_PREFIX}{}", name.to_ascii_uppercase().replace('-', "_"))
}

/// The file [`local_env`] names, when it is set: `None` when it is unset or empty, else that file as a module
/// with the version `local`, or why it cannot be used. No server is asked and no signature is checked. Only
/// debug builds read the variable; a release build always takes the server's signed module.
pub fn local(name: &str) -> Option<Result<Module, Unavailable>> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let path = std::env::var_os(local_env(name)).filter(|p| !p.is_empty())?;
    Some(local_at(name, Path::new(&path)))
}

fn local_at(name: &str, path: &Path) -> Result<Module, Unavailable> {
    let bytes = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_SIZE || path.file_name().is_none() {
        return Err(format!("{} is not a module file under 64 MiB", path.display()).into());
    }
    Ok(Module {
        name: name.to_owned(),
        version: LOCAL_VERSION.to_owned(),
        path: path.to_path_buf(),
        sha256: sha256_bytes(&bytes),
    })
}

/// The verified module `name` for `platform` that speaks `abi`: the local build when [`local_env`] names
/// one, else the cached copy when it has the hash the server offers, else a download. When the server can't
/// be reached or the download is refused as busy, the newest cached copy of that abi that still verifies.
pub fn ensure(data: &Path, api: &Api, name: &str, platform: &str, abi: u32) -> Result<Module, Unavailable> {
    if let Some(found) = local(name) {
        return found;
    }
    let key = decode(PUBLIC_KEY).ok_or("the pinned module key is not valid base64")?;
    ensure_with(data, api, name, platform, abi, &key)
}

fn ensure_with(
    data: &Path,
    api: &Api,
    name: &str,
    platform: &str,
    abi: u32,
    key: &[u8],
) -> Result<Module, Unavailable> {
    if !valid_name(name) || !valid_name(platform) {
        return Err("the module name is not a plain name".into());
    }
    let listing = match api.modules(platform, &[(name, abi)]) {
        Ok(v) => v,
        Err(e) if unreachable(&e) => {
            return cached(data, name, platform, abi, key).ok_or(Unavailable::new(e.message));
        }
        Err(e) if e.status == 404 => return Err("the server does not offer modules".into()),
        Err(e) => return Err(e.message.into()),
    };
    let listing: Listing =
        serde_json::from_value(listing).map_err(|e| format!("the module listing is not valid: {e}"))?;
    let Some(offer) = listing.modules.into_iter().find(|m| m.name == name) else {
        return Err(listing
            .refused
            .into_iter()
            .find(|r| r.name == name && !r.why.trim().is_empty())
            .map_or_else(
                || Unavailable::new("the server did not offer it"),
                |r| Unavailable::new(r.why.trim()),
            ));
    };
    if offer.platform != platform || offer.abi != abi {
        return Err("the offer is for another platform or abi".into());
    }
    check(key, &offer)?;
    let folder = dir(data, name, platform).join(&offer.version);
    let path = folder.join(&offer.file);
    let module = Module {
        name: name.to_owned(),
        version: offer.version.clone(),
        path: path.clone(),
        sha256: offer.sha256.clone(),
    };
    if !matches(&path, &offer) {
        let bytes = match from_cdn(&offer) {
            Some(bytes) => bytes,
            None => match api.module_file(name, platform, &offer.version, offer.size, DOWNLOAD_TIMEOUT) {
                Ok(bytes) => bytes,
                Err(e) if unreachable(&e) => {
                    return cached(data, name, platform, abi, key).ok_or(Unavailable::new(e.message));
                }
                Err(e) => return Err(e.message.into()),
            },
        };
        if bytes.len() as u64 != offer.size {
            return Err(format!(
                "the download is {} bytes, the offer says {}",
                bytes.len(),
                offer.size
            )
            .into());
        }
        if sha256_bytes(&bytes) != offer.sha256 {
            return Err("the download does not have the offered sha256".into());
        }
        store(&folder, &offer.file, &bytes).map_err(|e| format!("storing the module: {e}"))?;
    }
    let noted = serde_json::to_vec_pretty(&offer)
        .map_err(std::io::Error::other)
        .and_then(|doc| store(&folder, OFFER_FILE, &doc));
    if let Err(e) = noted {
        return Err(format!("storing the module: {e}").into());
    }
    prune(&dir(data, name, platform), &offer.version);
    Ok(module)
}

/// Whether the server gave no answer about the module, as opposed to refusing it.
fn unreachable(e: &ApiError) -> bool {
    matches!(e.status, 0 | 408 | 429) || e.status >= 500
}

/// The offered file from its `cdn_url`, when the offer has one the launcher follows and the bytes there have
/// the offered size and sha256. No sign-in is sent. `None` sends the launcher to the server instead.
fn from_cdn(offer: &Offer) -> Option<Vec<u8>> {
    let url = offer.cdn_url.as_deref().filter(|url| cdn_link(url))?;
    let mut response = ureq::get(url)
        .header("User-Agent", "swofty-launcher")
        .header("Accept-Encoding", "identity")
        .config()
        .timeout_global(Some(CDN_TIMEOUT))
        .timeout_connect(Some(CDN_CONNECT_TIMEOUT))
        .max_redirects(0)
        .build()
        .call()
        .ok()?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(offer.size + 1)
        .read_to_vec()
        .ok()?;
    (bytes.len() as u64 == offer.size && sha256_bytes(&bytes) == offer.sha256).then_some(bytes)
}

/// A CDN link the launcher follows: https, or plain http on this machine.
fn cdn_link(url: &str) -> bool {
    url.starts_with("https://") || (url.starts_with("http://") && crate::http::bearer_allowed(url))
}

/// The newest cached version of `abi` whose stored offer and file still verify.
fn cached(data: &Path, name: &str, platform: &str, abi: u32, key: &[u8]) -> Option<Module> {
    let base = dir(data, name, platform);
    let mut found: Vec<(Vec<u64>, Module)> = std::fs::read_dir(&base)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let folder = entry.path();
            let offer: Offer = serde_json::from_slice(&std::fs::read(folder.join(OFFER_FILE)).ok()?).ok()?;
            let same = offer.name == name
                && offer.platform == platform
                && offer.abi == abi
                && entry.file_name().to_str() == Some(offer.version.as_str());
            if !same || check(key, &offer).is_err() {
                return None;
            }
            let path = folder.join(&offer.file);
            matches(&path, &offer).then(|| {
                (
                    order(&offer.version),
                    Module {
                        name: name.to_owned(),
                        version: offer.version,
                        path,
                        sha256: offer.sha256,
                    },
                )
            })
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.version.cmp(&b.1.version)));
    found.pop().map(|(_, m)| m)
}

/// A dotted version as numbers, so `1.10` sorts after `1.9`.
fn order(version: &str) -> Vec<u64> {
    version.split('.').map(|part| part.parse().unwrap_or(0)).collect()
}

fn matches(path: &Path, offer: &Offer) -> bool {
    std::fs::read(path).is_ok_and(|b| b.len() as u64 == offer.size && sha256_bytes(&b) == offer.sha256)
}

/// Everything about an offer that can be judged without the file: its fields are plain and its signature
/// is the pinned key's.
fn check(key: &[u8], offer: &Offer) -> Result<(), Unavailable> {
    if !valid_name(&offer.name) || !valid_name(&offer.platform) {
        return Err("the offered name is not a plain name".into());
    }
    if !valid_version(&offer.version) {
        return Err("the offered version is not a plain version string".into());
    }
    if !valid_version(&offer.file) || offer.file == OFFER_FILE {
        return Err("the offered file name is not a plain file name".into());
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
        return Err(format!("the offered size of {} bytes is out of range", offer.size).into());
    }
    let signature = decode(&offer.signature).ok_or("the signature is not valid base64")?;
    UnparsedPublicKey::new(&ED25519, key)
        .verify(&signed_message(offer), &signature)
        .map_err(|_| "the signature does not match the pinned key".into())
}

fn signed_message(offer: &Offer) -> Vec<u8> {
    [
        SIGNED_PREFIX,
        &offer.name,
        &offer.platform,
        &offer.version,
        &offer.abi.to_string(),
        &offer.sha256,
    ]
    .join("\0")
    .into_bytes()
}

/// A module or platform name becomes a folder name and part of a URL: lowercase words joined by dashes.
fn valid_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_'))
}

fn store(folder: &Path, file: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(folder)?;
    let tmp = folder.join(format!("{file}.{}.tmp", std::process::id()));
    let done = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, folder.join(file)));
    if done.is_err() {
        std::fs::remove_file(&tmp).ok();
    }
    done
}

/// Keep `current` and the newest other version; the rest go.
fn prune(base: &Path, current: &str) {
    let Ok(entries) = std::fs::read_dir(base) else {
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

/// Add `module` to a build's placed files and plugin list so the loader loads it as the addon `id`.
pub fn place(module: &Module, id: &str, loader: &mut BTreeMap<String, PathBuf>, plugins: &mut Vec<String>) {
    place_file(id, &module.file_name(), &module.path, loader, plugins);
}

/// Add the plugin file at `source` to a build as `<addons>/<id>/<file>`.
pub fn place_file(
    id: &str,
    file: &str,
    source: &Path,
    loader: &mut BTreeMap<String, PathBuf>,
    plugins: &mut Vec<String>,
) {
    let rel = format!("{id}/{file}");
    loader.insert(
        format!(
            "{}/{}/{rel}",
            crate::install::LOADER_DIR,
            crate::build::ADDONS_DIR
        ),
        source.to_path_buf(),
    );
    plugins.push(format!("{}/{rel}", crate::build::ADDONS_DIR));
}

#[cfg(test)]
mod tests;
