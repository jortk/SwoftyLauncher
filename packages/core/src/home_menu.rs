use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub const STATE_FILE: &str = "swofty/home_menu.json";
pub const OPEN_FILE: &str = "swofty/open_url.json";
const CACHE_DIR: &str = "cache/home_menu";
const VIDEO_CACHE: &str = "video.json";
const THUMBNAIL: &str = "thumbnail.jpg";
const AVATAR: &str = "multiplayer/avatar.png";
const VIDEO_ROUTE: &str = "/v1/video/latest";
const STATS_ROUTE: &str = "/v1/stats";
const REFRESH: Duration = Duration::from_secs(300);
const THUMBNAIL_LIMIT: u64 = 2 * 1024 * 1024;
const IMAGE_HOSTS: [&str; 1] = ["i.ytimg.com"];
const AVATAR_HOSTS: [&str; 1] = ["cdn.discordapp.com"];
const AVATAR_LIMIT: u64 = 1024 * 1024;
const HOSTS: [&str; 3] = ["www.youtube.com", "youtube.com", "youtu.be"];
const TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub url: String,
    pub thumbnail_url: String,
    #[serde(default)]
    pub published_at: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VideoCard {
    pub title: String,
    pub url: String,
    pub thumbnail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Discord {
    pub signed_in: bool,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct State {
    pub schema: u32,
    pub launcher_version: String,
    pub discord: Discord,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<VideoCard>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct Stats {
    online: StatsOnline,
}

#[derive(Debug, Deserialize)]
struct StatsOnline {
    total: u64,
}

pub fn fetch_online(registry: &str) -> Option<u64> {
    let text = get(&format!("{registry}{STATS_ROUTE}"))?
        .with_config()
        .limit(64 * 1024)
        .read_to_string()
        .ok()?;
    serde_json::from_str::<Stats>(&text).ok().map(|s| s.online.total)
}

#[derive(Debug, Deserialize)]
struct OpenRequest {
    url: String,
}

pub fn video_url_allowed(url: &str) -> bool {
    https_host_in(url, &HOSTS)
}

pub fn https_host_in(url: &str, hosts: &[&str]) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    !authority.contains('@')
        && !authority.contains(':')
        && hosts.contains(&authority.to_ascii_lowercase().as_str())
}

pub fn parse_open_request(text: &str) -> Option<String> {
    let request: OpenRequest = serde_json::from_str(text).ok()?;
    video_url_allowed(&request.url).then_some(request.url)
}

pub fn take_open_request(game: &Path) -> Option<String> {
    let path = game.join(OPEN_FILE);
    let text = std::fs::read_to_string(&path).ok()?;
    std::fs::remove_file(&path).ok();
    parse_open_request(&text)
}

fn base_url() -> String {
    std::env::var(crate::catalog::URL_ENV)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| crate::catalog::DEFAULT_URL.to_owned())
        .trim_end_matches('/')
        .to_owned()
}

fn get(url: &str) -> Option<ureq::Body> {
    let response = ureq::get(url)
        .header("User-Agent", crate::catalog::agent_name())
        .config()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .build()
        .call()
        .ok()?;
    response.status().is_success().then(|| response.into_body())
}

fn fetch_video(registry: &str) -> Option<Video> {
    let text = get(&format!("{registry}{VIDEO_ROUTE}"))?
        .with_config()
        .limit(64 * 1024)
        .read_to_string()
        .ok()?;
    let video: Video = serde_json::from_str(&text).ok()?;
    (video_url_allowed(&video.url) && !video.id.is_empty() && !video.thumbnail_url.is_empty())
        .then_some(video)
}

fn fetch_image(url: &str, to: &Path, limit: u64) -> bool {
    let Some(mut body) = get(url) else {
        return false;
    };
    let Ok(bytes) = body.with_config().limit(limit).read_to_vec() else {
        return false;
    };
    let looks_like_image = bytes.starts_with(&[0xff, 0xd8]) || bytes.starts_with(b"\x89PNG");
    if !looks_like_image {
        return false;
    }
    std::fs::create_dir_all(to.parent().unwrap_or(Path::new("."))).is_ok()
        && std::fs::write(to, bytes).is_ok()
}

fn cached_video(data: &Path) -> Option<Video> {
    let dir = data.join(CACHE_DIR);
    let video: Video = serde_json::from_slice(&std::fs::read(dir.join(VIDEO_CACHE)).ok()?).ok()?;
    (video_url_allowed(&video.url) && dir.join(THUMBNAIL).is_file()).then_some(video)
}

pub fn latest_video(data: &Path, registry: &str) -> Option<VideoCard> {
    latest_video_from(data, registry, &|url| https_host_in(url, &IMAGE_HOSTS))
}

pub fn fetch_avatar(data: &Path) -> bool {
    static TOLD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let why = |text: &str| {
        if !TOLD.swap(true, std::sync::atomic::Ordering::Relaxed) {
            eprintln!("home menu: no Discord avatar for the main menu: {text}");
        }
        false
    };
    let Some(mut session) = crate::mpauth::load(data) else {
        return why("not signed in");
    };
    if session.avatar.is_none()
        && let Some(fresh) = crate::mpauth::refresh(data, &session)
    {
        session = fresh;
    }
    let Some(url) = session.avatar else {
        return why(
            "the account has no avatar (the server's /v1/auth/launcher/me returned no user.avatar, and the launcher keeps no avatar picture on disk)",
        );
    };
    if !https_host_in(&url, &AVATAR_HOSTS) {
        return why("the avatar link is not on Discord's CDN");
    }
    if fetch_image(&url, &data.join(AVATAR), AVATAR_LIMIT) {
        true
    } else {
        why("downloading the avatar failed")
    }
}

fn latest_video_from(data: &Path, registry: &str, trusted: &dyn Fn(&str) -> bool) -> Option<VideoCard> {
    let dir = data.join(CACHE_DIR);
    let fresh = fetch_video(registry).filter(|v| {
        trusted(&v.thumbnail_url) && fetch_image(&v.thumbnail_url, &dir.join(THUMBNAIL), THUMBNAIL_LIMIT)
    });
    if let Some(video) = &fresh {
        std::fs::write(dir.join(VIDEO_CACHE), serde_json::to_vec(video).ok()?).ok()?;
    }
    let video = fresh.or_else(|| cached_video(data))?;
    Some(VideoCard {
        title: video.title,
        url: video.url,
        thumbnail: dir.join(THUMBNAIL).to_string_lossy().into_owned(),
    })
}

pub fn discord(data: &Path) -> Discord {
    match crate::mpauth::load(data) {
        Some(session) => {
            let avatar: PathBuf = data.join(AVATAR);
            Discord {
                signed_in: true,
                name: session.name,
                avatar: avatar.is_file().then(|| avatar.to_string_lossy().into_owned()),
            }
        }
        None => Discord {
            signed_in: false,
            name: String::new(),
            avatar: None,
        },
    }
}

pub fn state(data: &Path, version: &str, video: Option<VideoCard>, online: Option<u64>) -> State {
    State {
        schema: 1,
        launcher_version: version.to_owned(),
        discord: discord(data),
        video,
        online,
    }
}

const STAGED_DIR: &str = "swofty/home_menu";

fn stage(game: &Path, from: &str, name: &str) -> Option<String> {
    let src = Path::new(from);
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("img");
    let file = format!("{name}.{ext}");
    let dir = game.join(STAGED_DIR);
    std::fs::create_dir_all(&dir).ok()?;
    std::fs::copy(src, dir.join(&file)).ok()?;
    Some(format!("home_menu/{file}"))
}

pub fn write(game: &Path, state: &State) -> crate::Result<()> {
    let mut staged = state.clone();
    staged.discord.avatar = state
        .discord
        .avatar
        .as_deref()
        .and_then(|a| stage(game, a, "avatar"));
    staged.video = state.video.clone().and_then(|mut v| {
        v.thumbnail = stage(game, &v.thumbnail, "thumbnail")?;
        Some(v)
    });
    crate::write_json(&game.join(STATE_FILE), &staged)
}

pub struct Refresh(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Drop for Refresh {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Writes `home_menu.json` at launch and refreshes it while the game runs. With `show_video` off the file carries
/// no video and the latest video is not fetched, so the home screen shows no video card.
pub fn publish(data: &Path, game: &Path, version: &str, show_video: bool) -> Refresh {
    publish_from(data, game, version, show_video, base_url(), REFRESH)
}

fn card_of(data: &Path, video: Video) -> VideoCard {
    VideoCard {
        title: video.title,
        url: video.url,
        thumbnail: data
            .join(CACHE_DIR)
            .join(THUMBNAIL)
            .to_string_lossy()
            .into_owned(),
    }
}

fn publish_from(
    data: &Path,
    game: &Path,
    version: &str,
    show_video: bool,
    registry: String,
    every: Duration,
) -> Refresh {
    let cached = || {
        cached_video(data)
            .filter(|_| show_video)
            .map(|v| card_of(data, v))
    };
    let _ = write(game, &state(data, version, cached(), None));
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = stop.clone();
    let (data, game, version) = (data.to_path_buf(), game.to_path_buf(), version.to_owned());
    std::thread::spawn(move || {
        let mut online: Option<u64> = None;
        let mut card: Option<VideoCard> = cached_video(&data)
            .filter(|_| show_video)
            .map(|v| card_of(&data, v));
        let mut first = true;
        while !flag.load(std::sync::atomic::Ordering::Relaxed) {
            if first {
                fetch_avatar(&data);
            }
            if show_video && let Some(fresh) = latest_video(&data, &registry) {
                card = Some(fresh);
            }
            if let Some(n) = fetch_online(&registry) {
                online = Some(n);
            }
            let _ = write(&game, &state(&data, &version, card.clone(), online));
            first = false;
            let until = std::time::Instant::now() + every;
            while std::time::Instant::now() < until && !flag.load(std::sync::atomic::Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    });
    Refresh(stop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-home-menu-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn video_json(thumb: &str, url: &str) -> Vec<u8> {
        format!(
            r#"{{"id":"abc123","title":"Latest","url":"{url}","thumbnail_url":"{thumb}","published_at":"2026-11-09T10:00:00Z"}}"#
        )
        .into_bytes()
    }

    #[test]
    fn only_youtube_hosts_over_https_are_opened() {
        assert!(video_url_allowed("https://www.youtube.com/watch?v=abc"));
        assert!(video_url_allowed("https://youtu.be/abc"));
        assert!(video_url_allowed("https://YOUTUBE.com/watch?v=abc"));
        assert!(!video_url_allowed("http://www.youtube.com/watch?v=abc"));
        assert!(!video_url_allowed("https://www.youtube.com.evil.example/watch"));
        assert!(!video_url_allowed(
            "https://evil.example/https://www.youtube.com/"
        ));
        assert!(!video_url_allowed("https://www.youtube.com@evil.example/"));
        assert!(!video_url_allowed("https://www.youtube.com:8443/"));
        assert!(!video_url_allowed("file:///etc/passwd"));
        assert!(!video_url_allowed(""));
    }

    #[test]
    fn an_open_request_is_taken_once_and_validated() {
        let game = temp("open");
        std::fs::create_dir_all(game.join("swofty")).unwrap();
        std::fs::write(game.join(OPEN_FILE), r#"{"url":"https://youtu.be/abc"}"#).unwrap();
        assert_eq!(take_open_request(&game).as_deref(), Some("https://youtu.be/abc"));
        assert_eq!(take_open_request(&game), None);
        std::fs::write(game.join(OPEN_FILE), r#"{"url":"https://evil.example/"}"#).unwrap();
        assert_eq!(take_open_request(&game), None);
        assert!(!game.join(OPEN_FILE).exists());
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn a_fetch_saves_the_thumbnail_and_a_dead_server_falls_back_to_the_cache() {
        let data = temp("cache");
        let jpeg = vec![0xff, 0xd8, 0xff, 0xe0, 9, 9];
        let registry = serve(jpeg.clone(), "https://youtu.be/abc");
        let card = latest_video_from(&data, &registry, &|_| true).expect("fresh card");
        assert_eq!(card.url, "https://youtu.be/abc");
        assert_eq!(std::fs::read(&card.thumbnail).unwrap(), jpeg);
        let dead = "http://127.0.0.1:9";
        let again = latest_video_from(&data, dead, &|_| true).expect("cached card");
        assert_eq!(again, card);
        let empty = temp("none");
        assert!(latest_video_from(&empty, dead, &|_| true).is_none());
        std::fs::remove_dir_all(&data).ok();
        std::fs::remove_dir_all(&empty).ok();
    }

    fn serve(jpeg: Vec<u8>, url: &'static str) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let registry = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let thumb = format!("{registry}/t.jpg");
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).is_ok_and(|n| n == 1) {
                    head.push(byte[0]);
                }
                let head = String::from_utf8_lossy(&head).into_owned();
                let (kind, body) = if head.contains(VIDEO_ROUTE) {
                    ("application/json", video_json(&thumb, url))
                } else {
                    ("image/jpeg", jpeg.clone())
                };
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(reply.as_bytes()).ok();
                stream.write_all(&body).ok();
            }
        });
        registry
    }

    #[test]
    fn a_numeric_published_date_parses() {
        let text = r#"{"id":"abc","title":"T","url":"https://www.youtube.com/watch?v=abc","thumbnail_url":"https://i.ytimg.com/vi/abc/hqdefault.jpg","published_at":1794210804}"#;
        let video: Video = serde_json::from_str(text).unwrap();
        assert_eq!(video.id, "abc");
    }

    #[test]
    fn a_video_with_a_foreign_url_is_refused() {
        let data = temp("foreign");
        let registry = serve(vec![0xff, 0xd8, 1], "https://evil.example/x");
        assert!(latest_video_from(&data, &registry, &|_| true).is_none());
        std::fs::remove_dir_all(&data).ok();
    }

    #[test]
    fn images_come_only_from_the_expected_cdns() {
        assert!(https_host_in(
            "https://i.ytimg.com/vi/abc/hqdefault.jpg",
            &IMAGE_HOSTS
        ));
        assert!(!https_host_in(
            "https://i.ytimg.com.evil.example/x.jpg",
            &IMAGE_HOSTS
        ));
        assert!(!https_host_in("http://i.ytimg.com/x.jpg", &IMAGE_HOSTS));
        assert!(https_host_in(
            "https://cdn.discordapp.com/avatars/1/a.png?size=128",
            &AVATAR_HOSTS
        ));
        assert!(!https_host_in("https://i.ytimg.com/x.png", &AVATAR_HOSTS));
        let data = temp("untrusted");
        let registry = serve(vec![0xff, 0xd8, 1], "https://youtu.be/abc");
        assert!(
            latest_video(&data, &registry).is_none(),
            "a loopback thumbnail host is refused"
        );
        std::fs::remove_dir_all(&data).ok();
    }

    fn serve_json(body: &'static str) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") && stream.read(&mut byte).is_ok_and(|n| n == 1) {
                    head.push(byte[0]);
                }
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(reply.as_bytes()).ok();
            }
        });
        url
    }

    #[test]
    fn the_online_count_is_the_stats_total_and_a_dead_server_gives_none() {
        let registry = serve_json(r#"{"online":{"total":1234,"in_game":56},"other":1}"#);
        assert_eq!(fetch_online(&registry), Some(1234));
        let odd = serve_json(r#"{"online":{"in_game":56}}"#);
        assert_eq!(fetch_online(&odd), None);
        assert_eq!(fetch_online("http://127.0.0.1:9"), None);
    }

    #[test]
    fn the_published_file_gains_the_online_count_and_keeps_it_when_a_refresh_fails() {
        let data = temp("online");
        let game = temp("online-game");
        let registry = serve_json(r#"{"online":{"total":77}}"#);
        let guard = publish_from(&data, &game, "1.8.37", true, registry, Duration::from_millis(100));
        let mut seen = None;
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if let Ok(bytes) = std::fs::read(game.join(STATE_FILE))
                && let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes)
                && v.get("online").is_some()
            {
                seen = Some(v["online"].clone());
                break;
            }
        }
        drop(guard);
        assert_eq!(seen, Some(serde_json::json!(77)));
        std::fs::remove_dir_all(&data).ok();
        std::fs::remove_dir_all(&game).ok();
    }

    #[test]
    fn a_cached_video_is_published_only_while_the_home_video_setting_is_on() {
        for show in [true, false] {
            let data = temp(&format!("show-{show}"));
            let game = temp(&format!("show-{show}-game"));
            let cache = data.join(CACHE_DIR);
            std::fs::create_dir_all(&cache).unwrap();
            std::fs::write(cache.join(THUMBNAIL), b"jpg").unwrap();
            std::fs::write(
                cache.join(VIDEO_CACHE),
                video_json(
                    "https://i.ytimg.com/vi/abc123/hqdefault.jpg",
                    "https://youtu.be/abc123",
                ),
            )
            .unwrap();
            let guard = publish_from(
                &data,
                &game,
                "1.9.19",
                show,
                "http://127.0.0.1:9".into(),
                Duration::from_secs(60),
            );
            let written: serde_json::Value =
                serde_json::from_slice(&std::fs::read(game.join(STATE_FILE)).unwrap()).unwrap();
            drop(guard);
            assert_eq!(written.get("video").is_some(), show, "show_video {show}");
            std::fs::remove_dir_all(&data).ok();
            std::fs::remove_dir_all(&game).ok();
        }
    }

    #[test]
    fn the_state_file_carries_version_sign_in_and_video() {
        let data = temp("state");
        let game = temp("game");
        let signed_out = state(&data, "1.8.37", None, None);
        assert!(!signed_out.discord.signed_in);
        let json = serde_json::to_value(&signed_out).unwrap();
        assert_eq!(json["launcher_version"], "1.8.37");
        assert_eq!(json["discord"]["signed_in"], false);
        assert!(json.get("video").is_none());
        let with = state(
            &data,
            "1.8.37",
            Some(VideoCard {
                title: "t".into(),
                url: "https://youtu.be/a".into(),
                thumbnail: "x.jpg".into(),
            }),
            Some(1234),
        );
        std::fs::write(data.join("thumb.jpg"), [0xff, 0xd8, 1]).unwrap();
        let with = State {
            video: with.video.clone().map(|mut v| {
                v.thumbnail = data.join("thumb.jpg").to_string_lossy().into_owned();
                v
            }),
            ..with
        };
        write(&game, &with).unwrap();
        assert!(game.join("swofty/home_menu/thumbnail.jpg").is_file());
        let back: serde_json::Value =
            serde_json::from_slice(&std::fs::read(game.join(STATE_FILE)).unwrap()).unwrap();
        assert_eq!(back["video"]["url"], "https://youtu.be/a");
        assert_eq!(back["video"]["thumbnail"], "home_menu/thumbnail.jpg");
        assert_eq!(back["online"], 1234);
        assert!(json.get("online").is_none());
        std::fs::remove_dir_all(&data).ok();
        std::fs::remove_dir_all(&game).ok();
    }
}
