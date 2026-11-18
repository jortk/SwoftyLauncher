use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::{Image, ImageFormat};

const HOSTS: [&str; 1] = ["cdn.discordapp.com"];
const LIMIT: u64 = 1024 * 1024;

pub(crate) fn path(data: &Path) -> PathBuf {
    data.join("multiplayer").join("avatar.png")
}

pub(crate) fn trusted(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    !authority.contains('@')
        && !authority.contains(':')
        && HOSTS.contains(&authority.to_ascii_lowercase().as_str())
}

pub(crate) fn load(data: &Path) -> Option<Arc<Image>> {
    let bytes = std::fs::read(path(data)).ok()?;
    bytes
        .starts_with(b"\x89PNG")
        .then(|| Arc::new(Image::from_bytes(ImageFormat::Png, bytes)))
}

pub(crate) fn fetch(data: &Path, url: &str) -> bool {
    if !trusted(url) {
        return false;
    }
    let sent = ureq::get(url)
        .header("User-Agent", "swofty-launcher")
        .config()
        .timeout_global(Some(Duration::from_secs(10)))
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .call();
    let Ok(mut response) = sent else {
        return false;
    };
    if !response.status().is_success() {
        return false;
    }
    let Ok(bytes) = response.body_mut().with_config().limit(LIMIT).read_to_vec() else {
        return false;
    };
    if !bytes.starts_with(b"\x89PNG") {
        return false;
    }
    let to = path(data);
    to.parent().is_none_or(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(to, bytes).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_discords_cdn_over_https_is_trusted() {
        assert!(trusted("https://cdn.discordapp.com/avatars/1/a.png?size=128"));
        assert!(!trusted("http://cdn.discordapp.com/a.png"));
        assert!(!trusted("https://cdn.discordapp.com.evil.example/a.png"));
        assert!(!trusted("https://user@cdn.discordapp.com/a.png"));
        assert!(!trusted("https://example.com/a.png"));
    }

    #[test]
    fn a_foreign_link_is_never_fetched() {
        let dir = std::env::temp_dir().join(format!("swofty-avatar-{}", std::process::id()));
        assert!(!fetch(&dir, "https://example.com/a.png"));
        assert!(load(&dir).is_none());
    }
}
