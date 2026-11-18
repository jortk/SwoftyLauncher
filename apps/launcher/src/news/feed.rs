use gpui_kit::*;
use serde::Deserialize;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use swofty_i18n::{tr, trf};

pub const FEED_URL: &str = "https://f1.swofty.net/news.json";

pub(super) const EMBEDDED: &str = include_str!(concat!(env!("OUT_DIR"), "/news.json"));

pub(super) const CACHE_DIR: &str = "cache/news";

pub(super) const FEED_FILE: &str = "feed.json";

pub(super) const TIMEOUT: Duration = Duration::from_secs(10);

pub(super) const STALE: Duration = Duration::from_secs(30 * 60);

pub(super) const IMAGES: usize = 8;

pub(super) const IMAGE_LIMIT: u64 = 8 * 1024 * 1024;

pub(super) const FEED_LIMIT: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Post {
    pub id: String,
    #[serde(default)]
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub body: String,
}

impl Post {
    pub(super) fn release(&self) -> bool {
        self.kind == "release"
    }

    pub(super) fn version(&self) -> &str {
        self.title.strip_prefix("Swofty Launcher ").unwrap_or(&self.title)
    }
}

#[derive(Deserialize)]
pub(super) struct Feed {
    pub(super) posts: Vec<Post>,
}

pub fn parse(text: &str) -> Option<Vec<Post>> {
    serde_json::from_str::<Feed>(text).ok().map(|f| f.posts)
}

pub(super) fn version(id: &str) -> Vec<u64> {
    let slug = id.rsplit('/').next().unwrap_or(id);
    slug.split('.').map(|n| n.parse().unwrap_or(0)).collect()
}

/// Every post of `base` and `newer` once, `newer`'s copy where both have it
/// (keeping `base`'s body when `newer`'s has none), newest first.
pub fn merge(mut base: Vec<Post>, newer: Vec<Post>) -> Vec<Post> {
    let mut posts = Vec::with_capacity(base.len() + newer.len());
    for mut post in newer {
        if let Some(at) = base.iter().position(|b| b.id == post.id) {
            let old = base.swap_remove(at);
            if post.body.is_empty() {
                post.body = old.body;
            }
        }
        posts.push(post);
    }
    posts.extend(base);
    posts.sort_by(|a, b| {
        b.date
            .cmp(&a.date)
            .then(a.release().cmp(&b.release()))
            .then_with(|| version(&b.id).cmp(&version(&a.id)))
    });
    posts
}

pub fn embedded() -> Vec<Post> {
    merge(parse(EMBEDDED).unwrap_or_default(), Vec::new())
}

pub fn format_date(date: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let mut parts = date.splitn(3, '-').map(|n| n.parse::<usize>().ok());
    match (
        parts.next().flatten(),
        parts.next().flatten(),
        parts.next().flatten(),
    ) {
        (Some(y), Some(m), Some(d)) if (1..=12).contains(&m) => format!("{d} {} {y}", MONTHS[m - 1]),
        _ => date.to_owned(),
    }
}

pub(crate) fn image_format(bytes: &[u8]) -> Option<ImageFormat> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some(ImageFormat::Png),
        [0xff, 0xd8, ..] => Some(ImageFormat::Jpeg),
        [b'G', b'I', b'F', ..] => Some(ImageFormat::Gif),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some(ImageFormat::Webp),
        _ => None,
    }
}

pub(crate) fn image(dir: &Path, url: &str) -> Option<Arc<Image>> {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let path = dir.join(format!("{:016x}", hasher.finish()));
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(_) => {
            let bytes = swofty_core::update::get(url, Some(TIMEOUT))
                .ok()?
                .with_config()
                .limit(IMAGE_LIMIT)
                .read_to_vec()
                .ok()?;
            std::fs::create_dir_all(dir).ok();
            std::fs::write(&path, &bytes).ok();
            bytes
        }
    };
    Some(Arc::new(Image::from_bytes(image_format(&bytes)?, bytes)))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Trouble {
    Offline,
    Failed,
}

pub(super) fn download() -> Result<String, (Trouble, String)> {
    let mut response = ureq::get(FEED_URL)
        .header(
            tr("User-Agent"),
            concat!("swofty-launcher/", env!("CARGO_PKG_VERSION")),
        )
        .config()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .build()
        .call()
        .map_err(|e| (Trouble::Offline, format!("{FEED_URL}: {e}")))?;
    let status = response.status().as_u16();
    if status >= 400 {
        return Err((
            Trouble::Failed,
            trf!("{feed_url}: HTTP {status}", feed_url = FEED_URL, status = status),
        ));
    }
    response
        .body_mut()
        .with_config()
        .limit(FEED_LIMIT)
        .read_to_string()
        .map_err(|e| (Trouble::Failed, format!("{FEED_URL}: {e}")))
}

pub(super) struct Fetched {
    pub(super) posts: Option<Vec<Post>>,
    pub(super) images: Vec<(String, Arc<Image>)>,
    pub(super) error: Option<(Trouble, String)>,
}

pub(super) fn fetch(root: &Path, shown: Vec<Post>) -> Fetched {
    let dir = root.join(CACHE_DIR);
    let remote = download().and_then(|text| {
        parse(&text).map(|p| (text, p)).ok_or_else(|| {
            (
                Trouble::Failed,
                trf!("{feed_url}: not a news feed", feed_url = FEED_URL),
            )
        })
    });
    let (posts, error) = match remote {
        Ok((text, posts)) => {
            std::fs::create_dir_all(&dir).ok();
            if let Err(e) = std::fs::write(dir.join(FEED_FILE), text) {
                eprintln!("swofty-launcher: could not save the news: {e}");
            }
            (Some(merge(embedded(), posts)), None)
        }
        Err(e) => (None, Some(e)),
    };
    let images = posts
        .as_ref()
        .unwrap_or(&shown)
        .iter()
        .filter(|p| !p.release())
        .take(IMAGES)
        .filter_map(|p| p.image.clone())
        .filter(|url| url.starts_with("https://"))
        .filter_map(|url| image(&dir, &url).map(|i| (url, i)))
        .collect();
    Fetched { posts, images, error }
}
