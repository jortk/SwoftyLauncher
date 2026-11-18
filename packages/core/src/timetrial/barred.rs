//! Addons the Time Trial boards server does not allow: `GET /v1/tt/barred`. The server rejects a lap
//! recorded with one of them enabled. The list is public; it is kept in the data folder's
//! `cache/tt-barred.json` and asked for again once that copy is [`MAX_AGE_S`] old. A launch never waits
//! for it: the wrapper hands the core the cached list through [`super::SESSION_FILE`]'s `tt_barred` and
//! refreshes it on a thread of its own.

use std::path::{Path, PathBuf};
use std::time::Duration;

use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::api::ApiError;
use super::{SESSION_FILE, SESSION_LOCK};

pub const MAX_AGE_S: u64 = 300;
pub const SESSION_KEY: &str = "tt_barred";
const ROUTE: &str = "/v1/tt/barred";
const TIMEOUT: Duration = Duration::from_secs(4);
const LIMIT: u64 = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Barred {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub reason: String,
    /// Only versions lower than this one are barred; without it every version is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub below: Option<String>,
}

impl Barred {
    pub fn title(&self) -> &str {
        match self.name.trim() {
            "" => &self.id,
            name => name,
        }
    }

    /// Whether the installed `version` of the addon is barred. A `below` that is not a version bars
    /// every version, and so does an installed version that is unknown or not a version.
    pub fn bars(&self, version: Option<&str>) -> bool {
        let Some(below) = self.below.as_deref().map(str::trim).filter(|b| !b.is_empty()) else {
            return true;
        };
        match (semver(below), version.and_then(semver)) {
            (Some(below), Some(installed)) => installed < below,
            _ => true,
        }
    }
}

fn semver(text: &str) -> Option<Version> {
    let text = text.trim();
    Version::parse(text.strip_prefix(['v', 'V']).unwrap_or(text)).ok()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cached {
    pub fetched: u64,
    pub addons: Vec<Barred>,
}

impl Cached {
    pub fn fresh(&self, now: u64) -> bool {
        now >= self.fetched && now - self.fetched < MAX_AGE_S
    }
}

#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    addons: Vec<Value>,
}

/// The server's answer: `{"addons": [{"id", "name", "reason", "below"?}]}`. An entry without an id is
/// left out, and so is one that is not an object. A `below` that is not a string is kept as its JSON
/// text, which is not a version, so it bars every version.
pub fn parse(bytes: &[u8]) -> Result<Vec<Barred>, serde_json::Error> {
    let reply: Reply = serde_json::from_slice(bytes)?;
    let text = |v: &Value, key: &str| {
        v.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_owned()
    };
    Ok(reply
        .addons
        .iter()
        .filter(|v| v.is_object())
        .map(|v| Barred {
            id: text(v, "id"),
            name: text(v, "name"),
            reason: text(v, "reason"),
            below: match v.get("below") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.trim().to_owned()),
                Some(other) => Some(other.to_string()),
            },
        })
        .filter(|b| !b.id.is_empty())
        .collect())
}

pub fn cache_path(data: &Path) -> PathBuf {
    data.join("cache").join("tt-barred.json")
}

pub fn cached(data: &Path) -> Option<Cached> {
    crate::read_json(&cache_path(data)).ok()
}

pub fn save(data: &Path, addons: &[Barred], now: u64) -> crate::Result<()> {
    crate::write_json_shared(
        &cache_path(data),
        &Cached {
            fetched: now,
            addons: addons.to_vec(),
        },
    )
}

/// The list from the boards server at `server`, without a sign-in.
pub fn fetch(server: &str) -> Result<Vec<Barred>, ApiError> {
    let server = server.trim().trim_end_matches('/');
    let sent = ureq::get(&format!("{server}{ROUTE}"))
        .header("User-Agent", "swofty-launcher")
        .header("Accept-Encoding", "identity")
        .config()
        .timeout_global(Some(TIMEOUT))
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .call();
    let mut response = sent.map_err(|e| ApiError::network(format!("couldn't reach {server}: {e}")))?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .with_config()
        .limit(LIMIT)
        .read_to_vec()
        .map_err(|e| ApiError::network(format!("reading the answer to GET {ROUTE}: {e}")))?;
    if !(200..300).contains(&status) {
        return Err(ApiError::from_body(status, &String::from_utf8_lossy(&body)));
    }
    parse(&body).map_err(|e| ApiError {
        status,
        message: format!("the server's answer to GET {ROUTE} is not valid: {e}"),
        retry_after: None,
    })
}

/// Ask the server and keep its answer in the cache.
pub fn refresh(data: &Path, server: &str, now: u64) -> Result<Vec<Barred>, ApiError> {
    let addons = fetch(server)?;
    save(data, &addons, now).ok();
    Ok(addons)
}

/// The list: the cached one while it is fresh, else what `ask` gets (which is then cached), else the
/// cached one however old. `None` when nothing was ever cached and `ask` fails.
pub fn list(
    data: &Path,
    now: u64,
    ask: impl FnOnce() -> Result<Vec<Barred>, ApiError>,
) -> Option<Vec<Barred>> {
    let kept = cached(data);
    if let Some(c) = kept.as_ref().filter(|c| c.fresh(now)) {
        return Some(c.addons.clone());
    }
    match ask() {
        Ok(addons) => {
            save(data, &addons, now).ok();
            Some(addons)
        }
        Err(_) => kept.map(|c| c.addons),
    }
}

/// The barred addons among `enabled`, in the order of `enabled`: each enabled addon's id and installed
/// version. Ids compare without regard to case.
pub fn enabled<'a, 'b>(
    barred: &'a [Barred],
    enabled: impl IntoIterator<Item = (&'b str, Option<String>)>,
) -> Vec<&'a Barred> {
    let mut out: Vec<&Barred> = Vec::new();
    for (id, version) in enabled {
        let id = id.trim();
        if let Some(b) = barred
            .iter()
            .find(|b| b.id.eq_ignore_ascii_case(id) && b.bars(version.as_deref()))
            && !out.iter().any(|o| std::ptr::eq(*o, b))
        {
            out.push(b);
        }
    }
    out
}

/// [`list`], asking the boards server [`super::api::Api::new`] talks to.
pub fn current(data: &Path, now: u64) -> Option<Vec<Barred>> {
    list(data, now, || fetch(&crate::catalog::base_url()))
}

/// [`enabled`] for a profile, with the installed versions `version_of` gives.
pub fn in_profile<'a>(
    barred: &'a [Barred],
    profile: &crate::profile::Profile,
    version_of: impl Fn(&str) -> Option<String>,
) -> Vec<&'a Barred> {
    enabled(barred, profile.enabled().map(|id| (id, version_of(id))))
}

/// [`in_profile`] for the profile named `profile`, with the versions the registry has installed.
pub fn for_store(store: &crate::store::Store, profile: &str, barred: &[Barred]) -> Vec<Barred> {
    if barred.is_empty() {
        return Vec::new();
    }
    let (Ok(p), Ok(registry)) = (store.profile(profile), store.registry()) else {
        return Vec::new();
    };
    in_profile(barred, &p, |id| {
        registry.get(id).map(|e| e.addon.manifest().version.to_string())
    })
    .into_iter()
    .cloned()
    .collect()
}

/// The line the game and the launcher show, or `None` with nothing barred.
pub fn warning(addons: &[&Barred]) -> Option<String> {
    (!addons.is_empty()).then(|| {
        format!(
            "Not allowed in Time Trial, laps won't count: {}",
            addons.iter().map(|b| b.title()).collect::<Vec<_>>().join(", ")
        )
    })
}

/// Put the barred addons into [`SESSION_FILE`] as `tt_barred` for the core, or take the key out when
/// there are none; the file goes when nothing else is in it.
pub fn write_session(game: &Path, addons: &[Barred]) -> std::io::Result<()> {
    let _held = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let path = game.join(SESSION_FILE);
    let mut doc: serde_json::Map<String, Value> = std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if addons.is_empty() {
        if doc.remove(SESSION_KEY).is_none() {
            return Ok(());
        }
        if doc.is_empty() {
            return std::fs::remove_file(&path).or_else(|e| match e.kind() {
                std::io::ErrorKind::NotFound => Ok(()),
                _ => Err(e),
            });
        }
    } else {
        let listed: Vec<Value> = addons
            .iter()
            .map(|b| json!({"id": b.id, "name": b.title()}))
            .collect();
        doc.insert(SESSION_KEY.into(), listed.into());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&doc).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-barred-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }

    fn barred(id: &str, name: &str, below: Option<&str>) -> Barred {
        Barred {
            id: id.into(),
            name: name.into(),
            reason: String::new(),
            below: below.map(str::to_owned),
        }
    }

    #[test]
    fn reads_the_servers_list_and_drops_entries_without_an_id() {
        let got = parse(
            br#"{"addons": [
                {"id": "kizikafuka.ai-drive", "name": "AI Drive", "reason": "the AI can drive your car"},
                {"id": " apoc.race-broadcast ", "name": "Race Broadcast", "reason": "x", "extra": 1},
                {"id": "swofty.rich-fuel-mix", "name": "Rich Fuel Mix", "reason": "y", "below": "1.0.3"},
                {"name": "no id"},
                {"id": ""},
                "kizikafuka.ai-drive"
            ]}"#,
        )
        .unwrap();
        assert_eq!(
            got.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(),
            [
                "kizikafuka.ai-drive",
                "apoc.race-broadcast",
                "swofty.rich-fuel-mix"
            ]
        );
        assert_eq!(got[0].reason, "the AI can drive your car");
        assert_eq!(got[2].below.as_deref(), Some("1.0.3"));
        assert_eq!(got[0].below, None);
        let odd =
            parse(br#"{"addons":[{"id":"a.b","name":7,"below":1},{"id":"c.d","below":null}]}"#).unwrap();
        assert_eq!(odd[0].title(), "a.b");
        assert!(
            odd[0].bars(Some("9.9.9")),
            "a below that is not a string bars every version"
        );
        assert_eq!(odd[1].below, None);
        assert!(parse(br#"{}"#).unwrap().is_empty());
        assert!(parse(b"<html>").is_err());
    }

    #[test]
    fn a_name_falls_back_to_the_id() {
        assert_eq!(barred("a.b", " ", None).title(), "a.b");
        assert_eq!(barred("a.b", "AI Drive", None).title(), "AI Drive");
    }

    #[test]
    fn below_bars_only_older_versions() {
        let b = barred("swofty.rich-fuel-mix", "Rich Fuel Mix", Some("1.0.3"));
        assert!(b.bars(Some("1.0.2")));
        assert!(b.bars(Some("0.9.0")));
        assert!(b.bars(Some("1.0.3-beta.1")));
        assert!(!b.bars(Some("1.0.3")));
        assert!(!b.bars(Some("v1.0.4")));
        assert!(!b.bars(Some("2.0.0")));
        assert!(b.bars(Some("one point oh")), "an unparseable version is barred");
        assert!(b.bars(None), "an unknown version is barred");
        assert!(barred("a.b", "", None).bars(Some("9.9.9")));
        assert!(barred("a.b", "", Some("not a version")).bars(Some("9.9.9")));
        assert!(barred("a.b", "", Some(" ")).bars(Some("9.9.9")));
    }

    #[test]
    fn finds_the_enabled_barred_addons_in_profile_order() {
        let list = vec![
            barred("kizikafuka.ai-drive", "AI Drive", None),
            barred("apoc.race-broadcast", "Race Broadcast", None),
            barred("swofty.rich-fuel-mix", "Rich Fuel Mix", Some("1.0.3")),
        ];
        let v = |s: &str| Some(s.to_owned());
        let got = enabled(
            &list,
            [
                ("Apoc.Race-Broadcast", v("2.0.0")),
                ("swofty.swofty-seasons-2024", v("1.0.0")),
                ("swofty.rich-fuel-mix", v("1.0.3")),
                ("kizikafuka.ai-drive", None),
                ("kizikafuka.ai-drive", v("1.0.0")),
            ],
        );
        assert_eq!(
            got.iter().map(|b| b.title()).collect::<Vec<_>>(),
            ["Race Broadcast", "AI Drive"]
        );
        let old = enabled(&list, [("swofty.rich-fuel-mix", v("1.0.2"))]);
        assert_eq!(old.len(), 1);
        assert!(enabled(&list, [("swofty.swofty-seasons-2024", None)]).is_empty());
        assert_eq!(
            warning(&got).as_deref(),
            Some("Not allowed in Time Trial, laps won't count: Race Broadcast, AI Drive")
        );
        assert_eq!(warning(&[]), None);
    }

    #[test]
    fn a_profile_counts_only_its_enabled_addons() {
        let list = vec![barred("kizikafuka.ai-drive", "AI Drive", None)];
        let mut p = crate::profile::Profile::new("p");
        p.add_addon("kizikafuka.ai-drive").unwrap();
        assert!(in_profile(&list, &p, |_| None).is_empty());
        p.set_enabled("kizikafuka.ai-drive", true).unwrap();
        assert_eq!(in_profile(&list, &p, |_| None).len(), 1);
        p.set_enabled("kizikafuka.ai-drive", false).unwrap();
        assert!(in_profile(&list, &p, |_| None).is_empty());
    }

    #[test]
    fn a_fresh_cache_is_used_without_asking() {
        let data = scratch("fresh");
        save(&data, &[barred("a.b", "A", None)], 1000).unwrap();
        let got = list(&data, 1000 + MAX_AGE_S - 1, || panic!("asked while fresh")).unwrap();
        assert_eq!(got[0].id, "a.b");
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_stale_cache_is_refreshed_and_kept_when_the_server_is_away() {
        let data = scratch("stale");
        save(&data, &[barred("a.b", "A", None)], 1000).unwrap();
        let now = 1000 + MAX_AGE_S;
        let got = list(&data, now, || Ok(vec![barred("c.d", "C", None)])).unwrap();
        assert_eq!(got[0].id, "c.d");
        assert_eq!(cached(&data).unwrap().fetched, now);
        let later = now + MAX_AGE_S * 10;
        let offline = list(&data, later, || Err(ApiError::network("down"))).unwrap();
        assert_eq!(offline[0].id, "c.d");
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn nothing_cached_and_offline_is_none() {
        let data = scratch("none");
        assert!(list(&data, 5, || Err(ApiError::network("down"))).is_none());
        assert!(!cache_path(&data).exists());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn a_cache_from_the_future_is_not_fresh() {
        let c = Cached {
            fetched: 2000,
            addons: Vec::new(),
        };
        assert!(c.fresh(2000) && c.fresh(2000 + MAX_AGE_S - 1));
        assert!(!c.fresh(1999) && !c.fresh(2000 + MAX_AGE_S));
    }

    #[test]
    fn fetches_without_a_sign_in() {
        let (url, t) = super::super::api::tests::serve(
            "200 OK",
            r#"{"addons":[{"id":"kizikafuka.ai-drive","name":"AI Drive","reason":"the AI can drive your car"}]}"#,
        );
        let data = scratch("fetch");
        let got = refresh(&data, &url, 77).unwrap();
        let request = String::from_utf8_lossy(&t.join().unwrap()).into_owned();
        assert!(request.starts_with("GET /v1/tt/barred HTTP/1.1"));
        assert!(!request.to_ascii_lowercase().contains("authorization"));
        assert_eq!(got[0].title(), "AI Drive");
        assert_eq!(cached(&data).unwrap().fetched, 77);
        let (url, t) = super::super::api::tests::serve("503 Service Unavailable", "");
        assert!(fetch(&url).unwrap_err().transient());
        t.join().unwrap();
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn the_session_file_carries_the_names_and_loses_them_again() {
        let game = scratch("session");
        write_session(&game, &[]).unwrap();
        assert!(!game.join(SESSION_FILE).exists());
        write_session(
            &game,
            &[
                barred("kizikafuka.ai-drive", "AI Drive", None),
                barred("x.y", "", None),
            ],
        )
        .unwrap();
        let read =
            || -> Value { serde_json::from_slice(&std::fs::read(game.join(SESSION_FILE)).unwrap()).unwrap() };
        assert_eq!(
            read(),
            json!({"tt_barred": [{"id": "kizikafuka.ai-drive", "name": "AI Drive"}, {"id": "x.y", "name": "x.y"}]})
        );
        write_session(&game, &[]).unwrap();
        assert!(!game.join(SESSION_FILE).exists());
        super::super::write_session_keys(&game, Some("n"), false).unwrap();
        write_session(&game, &[barred("a.b", "A", None)]).unwrap();
        super::super::clear_session_keys(&game);
        assert_eq!(read(), json!({"tt_barred": [{"id": "a.b", "name": "A"}]}));
        write_session(&game, &[]).unwrap();
        assert!(!game.join(SESSION_FILE).exists());
        std::fs::remove_dir_all(&game).ok();
    }
}
