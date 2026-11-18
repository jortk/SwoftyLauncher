//! The gallery on an addon's registry page: pictures, videos and video links
//! its publishers add, shown in the Addon Browser.

use serde::{Deserialize, Deserializer, Serialize};

use crate::home_menu::https_host_in;

/// The registry's public file host.
pub const CDN_HOST: &str = "cdn.swofty.net";
/// The most items a gallery lists.
pub const ITEMS_MAX: usize = 12;
/// The path the registry serves gallery files from when it keeps them on its own disk.
const LOCAL_PATH: &str = "/v1/media/";
const LINK_HOSTS: [&str; 6] = [
    "www.youtube.com",
    "youtube.com",
    "vimeo.com",
    "www.vimeo.com",
    "streamable.com",
    "www.streamable.com",
];
const LOOPBACK: [&str; 3] = ["localhost", "127.0.0.1", "[::1]"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Image,
    Video,
    Link,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Media {
    pub id: String,
    pub kind: Kind,
    /// The file for pictures and videos, the video's page for links.
    pub url: String,
    /// The video's poster or the link's thumbnail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumb: Option<String>,
    #[serde(default)]
    pub caption: String,
    #[serde(default)]
    pub order: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// `youtube`, `vimeo` or `streamable` for links.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
}

impl Media {
    /// The picture to show for the item, when the launcher may load it from `registry`.
    pub fn preview(&self, registry: &str) -> Option<&str> {
        let url = match self.kind {
            Kind::Image => Some(self.url.as_str()),
            _ => self.thumb.as_deref(),
        }?;
        fetchable(url, registry).then_some(url)
    }

    /// The link the system browser opens for the item.
    pub fn opens(&self, registry: &str) -> Option<&str> {
        let ok = match self.kind {
            Kind::Image | Kind::Video => fetchable(&self.url, registry),
            Kind::Link => https_host_in(&self.url, &LINK_HOSTS),
            Kind::Unknown => false,
        };
        ok.then_some(self.url.as_str())
    }

    /// The video site's name for links.
    pub fn host_name(&self) -> Option<&'static str> {
        match self.host.as_deref()? {
            "youtube" => Some("YouTube"),
            "vimeo" => Some("Vimeo"),
            "streamable" => Some("Streamable"),
            _ => None,
        }
    }

    pub fn playable(&self) -> bool {
        matches!(self.kind, Kind::Video | Kind::Link)
    }
}

/// Whether `url` is a gallery file on the CDN, or on the registry at `registry`
/// itself (plain http only for a registry on this machine).
pub fn fetchable(url: &str, registry: &str) -> bool {
    if https_host_in(url, &[CDN_HOST]) {
        return true;
    }
    let registry = registry.trim_end_matches('/');
    let Some((scheme, authority)) = origin(registry) else {
        return false;
    };
    let local = scheme == "http" && loopback(authority);
    (scheme == "https" || local)
        && registry.len() == scheme.len() + 3 + authority.len()
        && url
            .strip_prefix(registry)
            .and_then(|rest| rest.strip_prefix(LOCAL_PATH))
            .is_some_and(|rest| !rest.is_empty() && !rest.contains(['?', '#', '\\']) && !rest.contains(".."))
}

fn origin(url: &str) -> Option<(&str, &str)> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    (matches!(scheme, "https" | "http") && !authority.is_empty() && !authority.contains('@'))
        .then_some((scheme, authority))
}

fn loopback(authority: &str) -> bool {
    let host = match authority.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
        _ => authority,
    };
    LOOPBACK.contains(&host.to_ascii_lowercase().as_str())
}

/// A gallery the launcher can't fully read keeps the items it can, in order;
/// an item of a kind it doesn't know is left out.
pub(super) fn listed<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Vec<Media>, D::Error> {
    let raw = Option::<serde_json::Value>::deserialize(d)?;
    let items = match raw {
        Some(serde_json::Value::Array(items)) => items,
        _ => return Ok(Vec::new()),
    };
    let mut out: Vec<Media> = items
        .into_iter()
        .filter_map(|v| serde_json::from_value::<Media>(v).ok())
        .filter(|m| m.kind != Kind::Unknown)
        .collect();
    out.sort_by_key(|m| m.order);
    out.truncate(ITEMS_MAX);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTRY: &str = "https://api.f1.swofty.net";

    fn item(kind: &str, url: &str, thumb: Option<&str>) -> Media {
        serde_json::from_value(serde_json::json!({
            "id": "0123456789abcdef",
            "kind": kind,
            "url": url,
            "thumb": thumb,
        }))
        .unwrap()
    }

    #[derive(Deserialize)]
    struct Holder {
        #[serde(default, deserialize_with = "listed")]
        media: Vec<Media>,
    }

    fn gallery(json: serde_json::Value) -> Vec<Media> {
        serde_json::from_value::<Holder>(json).unwrap().media
    }

    #[test]
    fn previews_come_only_from_the_registry_storage() {
        let pic = item("image", "https://cdn.swofty.net/media/demo.a/aa.webp", None);
        assert_eq!(
            pic.preview(REGISTRY),
            Some("https://cdn.swofty.net/media/demo.a/aa.webp")
        );
        assert_eq!(pic.opens(REGISTRY), Some(pic.url.as_str()));
        let clip = item(
            "video",
            "https://cdn.swofty.net/media/demo.a/bb.mp4",
            Some("https://cdn.swofty.net/media/demo.a/cc.jpg"),
        );
        assert_eq!(
            clip.preview(REGISTRY),
            Some("https://cdn.swofty.net/media/demo.a/cc.jpg")
        );
        assert!(clip.playable());
        let elsewhere = item("image", "https://i.ytimg.com/vi/x/hqdefault.jpg", None);
        assert_eq!(elsewhere.preview(REGISTRY), None);
        assert_eq!(elsewhere.opens(REGISTRY), None);
        let plain = item("image", "http://cdn.swofty.net/media/demo.a/aa.webp", None);
        assert_eq!(plain.preview(REGISTRY), None);
        let sneaky = item("image", "https://cdn.swofty.net@evil.test/a.png", None);
        assert_eq!(sneaky.preview(REGISTRY), None);
        let blank = item("video", "https://cdn.swofty.net/media/demo.a/bb.mp4", None);
        assert_eq!(blank.preview(REGISTRY), None);
    }

    #[test]
    fn a_registry_serves_its_own_files() {
        let own = "https://api.f1.swofty.net/v1/media/demo.a/aa.png";
        assert!(fetchable(own, REGISTRY));
        assert!(fetchable(own, "https://api.f1.swofty.net/"));
        assert!(!fetchable(
            "https://api.f1.swofty.net/v1/addons/demo.a/icon",
            REGISTRY
        ));
        assert!(!fetchable("https://api.f1.swofty.net/v1/media/", REGISTRY));
        assert!(!fetchable("https://api.f1.swofty.net/v1/media/../x", REGISTRY));
        assert!(!fetchable(
            "https://api.f1.swofty.net.evil.test/v1/media/a.png",
            REGISTRY
        ));
        assert!(fetchable(
            "http://localhost:8471/v1/media/demo.a/aa.png",
            "http://localhost:8471"
        ));
        assert!(fetchable(
            "http://127.0.0.1/v1/media/demo.a/aa.png",
            "http://127.0.0.1"
        ));
        assert!(!fetchable(
            "http://example.com/v1/media/demo.a/aa.png",
            "http://example.com"
        ));
        assert!(!fetchable(
            "http://localhost:8471/v1/media/demo.a/aa.png",
            "http://localhost:8471/registry"
        ));
    }

    #[test]
    fn links_open_their_video_page() {
        let mut link = item(
            "link",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            Some("https://cdn.swofty.net/media/demo.a/dd.jpg"),
        );
        link.host = Some("youtube".into());
        assert_eq!(
            link.opens(REGISTRY),
            Some("https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        );
        assert_eq!(link.host_name(), Some("YouTube"));
        assert!(link.playable());
        let vimeo = item("link", "https://vimeo.com/76979871", None);
        assert_eq!(vimeo.opens(REGISTRY), Some("https://vimeo.com/76979871"));
        let other = item("link", "https://evil.test/watch", None);
        assert_eq!(other.opens(REGISTRY), None);
        let plain = item("link", "http://www.youtube.com/watch?v=dQw4w9WgXcQ", None);
        assert_eq!(plain.opens(REGISTRY), None);
    }

    #[test]
    fn galleries_are_read_leniently_and_in_order() {
        let media = gallery(serde_json::json!({"media": [
            {"id": "b", "kind": "video", "url": "https://cdn.swofty.net/b.mp4", "order": 1},
            {"id": "a", "kind": "image", "url": "https://cdn.swofty.net/a.png", "order": 0, "caption": "Night"},
            {"id": "c", "kind": "hologram", "url": "https://cdn.swofty.net/c.holo", "order": 2},
            {"id": "d", "kind": "image", "order": 3},
            {"id": "e", "kind": "image", "url": "https://cdn.swofty.net/e.png", "order": "x"},
            "junk",
            {"id": "f", "kind": "link", "url": "https://vimeo.com/1", "order": 4, "future": true}
        ]}));
        let ids: Vec<&str> = media.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "f"]);
        assert_eq!(media[0].caption, "Night");
        assert!(gallery(serde_json::json!({})).is_empty());
        assert!(gallery(serde_json::json!({"media": null})).is_empty());
        assert!(gallery(serde_json::json!({"media": {"oops": 1}})).is_empty());
        let many: Vec<serde_json::Value> = (0..20)
            .map(|i| serde_json::json!({"id": i.to_string(), "kind": "image", "url": "https://cdn.swofty.net/a.png", "order": i}))
            .collect();
        assert_eq!(gallery(serde_json::json!({ "media": many })).len(), ITEMS_MAX);
    }
}
