use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::{Image, ImageFormat};

const LIMIT: u64 = 4 * 1024 * 1024;

#[derive(Clone)]
pub(crate) enum Art {
    Raster(Arc<Image>),
    Svg(Arc<[u8]>),
}

pub(crate) enum Slot {
    Loading,
    Ready(Art),
    Failed,
}

pub(crate) struct Images {
    slots: HashMap<String, Slot>,
    dir: PathBuf,
}

impl Images {
    pub(crate) fn new(data: &std::path::Path) -> Self {
        Self {
            slots: HashMap::new(),
            dir: data.join("cache").join("tt-images"),
        }
    }

    pub(crate) fn dir(&self) -> PathBuf {
        self.dir.clone()
    }

    pub(crate) fn get(&self, url: &str) -> Option<&Art> {
        match self.slots.get(url) {
            Some(Slot::Ready(art)) => Some(art),
            _ => None,
        }
    }

    pub(crate) fn wanted(&mut self, urls: impl IntoIterator<Item = String>) -> Vec<String> {
        let mut out = Vec::new();
        for url in urls {
            if !self.slots.contains_key(&url) {
                self.slots.insert(url.clone(), Slot::Loading);
                out.push(url);
            }
        }
        out
    }

    pub(crate) fn finish(&mut self, url: String, art: Option<Art>) {
        self.slots.insert(url, art.map_or(Slot::Failed, Slot::Ready));
    }
}

pub(crate) fn cache_name(url: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in url.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

pub(crate) fn decode(bytes: Vec<u8>) -> Option<Art> {
    let head = bytes.iter().take(256).copied().collect::<Vec<_>>();
    let text = String::from_utf8_lossy(&head);
    if text.contains("<svg") || text.trim_start().starts_with("<?xml") {
        return Some(Art::Svg(Arc::from(bytes)));
    }
    let format = if bytes.starts_with(b"\x89PNG") {
        ImageFormat::Png
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        ImageFormat::Jpeg
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        ImageFormat::Webp
    } else if bytes.starts_with(b"GIF8") {
        ImageFormat::Gif
    } else {
        return None;
    };
    Some(Art::Raster(Arc::new(Image::from_bytes(format, bytes))))
}

pub(crate) fn load(dir: &std::path::Path, url: &str) -> Option<Art> {
    let cached = dir.join(cache_name(url));
    if let Ok(bytes) = std::fs::read(&cached)
        && let Some(art) = decode(bytes)
    {
        return Some(art);
    }
    let bytes = if let Some(name) = url.strip_prefix("mock:") {
        if !cfg!(debug_assertions) || name.contains("..") || name.contains('/') {
            return None;
        }
        let root = std::env::var_os("SWOFTY_ONLINE_MOCK_ART")?;
        std::fs::read(PathBuf::from(root).join(name)).ok()?
    } else {
        if !url.starts_with("https://") && !cfg!(debug_assertions) {
            return None;
        }
        let mut response = ureq::get(url)
            .header("User-Agent", "swofty-launcher")
            .config()
            .timeout_global(Some(Duration::from_secs(12)))
            .max_redirects(3)
            .http_status_as_error(false)
            .build()
            .call()
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        response
            .body_mut()
            .with_config()
            .limit(LIMIT)
            .read_to_vec()
            .ok()?
    };
    let art = decode(bytes.clone())?;
    if !url.starts_with("mock:") && std::fs::create_dir_all(dir).is_ok() {
        std::fs::write(&cached, bytes).ok();
    }
    Some(art)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cache_name_is_stable_and_distinct() {
        assert_eq!(cache_name("https://a/x.png"), cache_name("https://a/x.png"));
        assert_ne!(cache_name("https://a/x.png"), cache_name("https://a/y.png"));
        assert_eq!(cache_name("a").len(), 16);
    }

    #[test]
    fn decoding_recognises_svg_and_rejects_noise() {
        assert!(matches!(
            decode(b"<svg xmlns=\"x\"></svg>".to_vec()),
            Some(Art::Svg(_))
        ));
        assert!(matches!(
            decode(b"\x89PNG\r\n\x1a\n....".to_vec()),
            Some(Art::Raster(_))
        ));
        assert!(decode(b"hello".to_vec()).is_none());
    }

    #[test]
    fn a_url_is_only_requested_once() {
        let mut images = Images::new(std::path::Path::new("/nonexistent"));
        assert_eq!(images.wanted(["a".to_owned(), "b".to_owned()]), ["a", "b"]);
        assert!(images.wanted(["a".to_owned()]).is_empty());
        images.finish("a".into(), None);
        assert!(images.get("a").is_none());
    }
}
