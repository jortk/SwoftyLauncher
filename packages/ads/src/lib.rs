//! Ad slots in the launcher. Each slot is a native webview over a reserved
//! rectangle of the launcher window, loading `<base>/app/<unit>`.
//!
//! On Windows the launcher hosts the webviews itself. On Linux they live in the
//! `swofty-ads` helper process, which the launcher drives with [`Command`]
//! lines on its stdin, so the launcher never links WebKitGTK.

mod protocol;
#[cfg(feature = "view")]
pub mod view;

pub use protocol::{Command, Consent, Event};

/// The page for one ad unit at `placement`, which the page reports to RevIQ,
/// with `background` behind it while the ad loads. `dev` asks for RevIQ's test
/// placements, which never count as real impressions.
pub fn slot_url(base: &str, unit: &str, placement: &str, background: [u8; 3], dev: bool) -> String {
    let [r, g, b] = background;
    format!(
        "{}/app/{unit}?bg={r:02x}{g:02x}{b:02x}&p={placement}{}",
        base.trim_end_matches('/'),
        if dev { "&dev=1" } else { "" }
    )
}

/// `url` with the user's hashed email in its fragment, which the page hands to
/// RevIQ. The fragment never reaches the server. Anything but a SHA-256 hex
/// digest is left out.
pub fn with_uid(url: String, uid: Option<&str>) -> String {
    match uid {
        Some(uid)
            if uid.len() == 64
                && uid
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) =>
        {
            format!("{url}#e={uid}")
        }
        _ => url,
    }
}

/// The page that runs RevIQ's consent form full size, with `background`
/// behind it. `reopen` opens the form again over a stored answer.
pub fn consent_url(base: &str, background: [u8; 3], reopen: bool, dev: bool) -> String {
    let [r, g, b] = background;
    format!(
        "{}/app/consent?bg={r:02x}{g:02x}{b:02x}{}{}",
        base.trim_end_matches('/'),
        if reopen { "&show=1" } else { "" },
        if dev { "&dev=1" } else { "" }
    )
}

/// The background a [`slot_url`] asks for.
pub fn slot_background(url: &str) -> Option<[u8; 3]> {
    let hex = url.split_once("bg=")?.1.get(..6)?;
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// Whether `url` is on the same origin as `home`, so the slot may navigate to it.
pub fn same_origin(home: &str, url: &str) -> bool {
    origin(home).is_some_and(|o| origin(url) == Some(o))
}

/// Whether the launcher may hand `url` to the default browser.
pub fn openable(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && url.len() <= 4096
        && !url.contains(char::is_whitespace)
}

fn origin(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let scheme = url.len() - rest.len();
    Some(&url[..scheme + end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slot_url_names_the_unit_and_background() {
        assert_eq!(
            slot_url(
                "https://ads.swofty.net/",
                "right-rail-1",
                "home",
                [0x1a, 0x1b, 0x0c],
                false
            ),
            "https://ads.swofty.net/app/right-rail-1?bg=1a1b0c&p=home"
        );
        assert_eq!(
            slot_url("https://ads.swofty.net", "video", "video", [0, 0, 0], true),
            "https://ads.swofty.net/app/video?bg=000000&p=video&dev=1"
        );
    }

    #[test]
    fn only_a_sha256_digest_goes_in_the_fragment() {
        let digest = "0123456789abcdef".repeat(4);
        let url = "https://ads.swofty.net/app/right-rail-1?bg=000000&p=home".to_owned();
        assert_eq!(with_uid(url.clone(), Some(&digest)), format!("{url}#e={digest}"));
        assert_eq!(with_uid(url.clone(), None), url);
        assert_eq!(with_uid(url.clone(), Some("ABC")), url);
        assert_eq!(with_uid(url.clone(), Some(&digest.to_uppercase())), url);
        assert_eq!(with_uid(url.clone(), Some("someone@example.com")), url);
    }

    #[test]
    fn a_consent_url_can_reopen_the_form() {
        assert_eq!(
            consent_url("https://ads.swofty.net/", [0x1f, 0x23, 0x2d], false, false),
            "https://ads.swofty.net/app/consent?bg=1f232d"
        );
        assert_eq!(
            consent_url("https://ads.swofty.net", [0, 0, 0], true, true),
            "https://ads.swofty.net/app/consent?bg=000000&show=1&dev=1"
        );
        assert_eq!(
            slot_background(&consent_url("https://ads.swofty.net", [1, 2, 3], true, false)),
            Some([1, 2, 3])
        );
    }

    #[test]
    fn a_slot_url_gives_its_background_back() {
        let url = slot_url(
            "https://ads.swofty.net",
            "video",
            "video",
            [0x0b, 0xfe, 0x40],
            false,
        );
        assert_eq!(slot_background(&url), Some([0x0b, 0xfe, 0x40]));
        assert_eq!(slot_background("https://ads.swofty.net/app/video"), None);
        assert_eq!(
            slot_background("https://ads.swofty.net/app/video?bg=zz0000"),
            None
        );
    }

    #[test]
    fn only_the_home_origin_counts_as_same_origin() {
        let home = "https://ads.swofty.net/app/video?bg=000000";
        assert!(same_origin(home, "https://ads.swofty.net/app/video"));
        assert!(same_origin(home, "https://ads.swofty.net"));
        assert!(!same_origin(
            home,
            "https://ads.swofty.net.evil.example/app/video"
        ));
        assert!(!same_origin(home, "http://ads.swofty.net/app/video"));
        assert!(!same_origin(
            home,
            "https://example.com/?u=https://ads.swofty.net"
        ));
        assert!(!same_origin(home, "about:blank"));
    }

    #[test]
    fn only_web_links_open_in_the_browser() {
        assert!(openable("https://example.com/landing?id=1"));
        assert!(openable("http://example.com"));
        assert!(!openable("file:///etc/passwd"));
        assert!(!openable("javascript:alert(1)"));
        assert!(!openable("steam://run/1"));
        assert!(!openable("https://example.com/a b"));
    }
}
