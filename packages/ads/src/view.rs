//! The webview a slot runs in, for the launcher on Windows and the Linux helper.
//!
//! A slot only ever shows its own page. Popups, downloads and navigations away
//! from it are refused, and a link opens in the default browser only after a
//! real click: `clicks.js` runs in every frame, catches clicked links and
//! `window.open`, and posts the address up to the slot's page, which hands it
//! to the launcher while the click's user activation lasts.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use wry::dpi::{PhysicalPosition, PhysicalSize};
use wry::{NewWindowResponse, PageLoadEvent, Rect, WebContext, WebViewBuilder};

use crate::Consent;

pub use wry;

/// A standard Chrome user agent for WebKitGTK, whose own reads as Safari on
/// Linux. WebView2 already sends Edge's.
pub const LINUX_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";

const CLICKS: &str = include_str!("clicks.js");

/// The least time between two links the slot opens.
const OPEN_GAP: Duration = Duration::from_millis(800);

pub fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
    Rect {
        position: PhysicalPosition::new(x, y).into(),
        size: PhysicalSize::new(width.max(1), height.max(1)).into(),
    }
}

/// A builder for the slot showing `home`, without a URL yet. `strayed` turns true when the main
/// frame leaves `home`'s origin, so the owner can load `home` again: WebKitGTK
/// asks the same navigation question for iframes, so on Linux a navigation
/// can't be refused up front without refusing the ads themselves.
/// `consent` hears what the consent page reports.
pub fn builder<'a>(
    context: &'a mut WebContext,
    home: &str,
    bounds: Rect,
    background: [u8; 3],
    strayed: Rc<Cell<bool>>,
    consent: impl Fn(Consent) + 'static,
) -> WebViewBuilder<'a> {
    let last = Cell::new(None::<Instant>);
    let [r, g, b] = background;
    let builder = WebViewBuilder::new_with_web_context(context)
        .with_bounds(bounds)
        .with_background_color((r, g, b, 255))
        .with_initialization_script_for_main_only(CLICKS, false)
        .with_ipc_handler(move |request| {
            let url = request.body();
            if let Some(state) = Consent::from_page(url) {
                consent(state);
                return;
            }
            if !crate::openable(url) {
                return;
            }
            let now = Instant::now();
            if last.get().is_some_and(|t| now.duration_since(t) < OPEN_GAP) {
                return;
            }
            last.set(Some(now));
            let _ = open::that_detached(url);
        })
        .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
        .with_download_started_handler(|_, _| false)
        .with_devtools(false)
        .with_hotkeys_zoom(false)
        .with_autoplay(true)
        .with_focused(false);
    let page_home = home.to_owned();
    let builder = builder.with_on_page_load_handler(move |event, url| {
        if matches!(event, PageLoadEvent::Started)
            && !crate::same_origin(&page_home, &url)
            && url != "about:blank"
        {
            strayed.set(true);
        }
    });
    #[cfg(windows)]
    let builder = {
        let home = home.to_owned();
        builder.with_navigation_handler(move |to| to == "about:blank" || crate::same_origin(&home, &to))
    };
    #[cfg(target_os = "linux")]
    let builder = builder.with_user_agent(LINUX_USER_AGENT);
    builder
}

/// Turns WebView2's tracking prevention off for the slot, which otherwise
/// blocks the cookies ad bidders read. Takes effect from the next navigation.
#[cfg(windows)]
pub fn allow_trackers(webview: &wry::WebView) -> windows_core::Result<()> {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_NONE, ICoreWebView2_13, ICoreWebView2Profile3,
    };
    use windows_core::Interface;
    use wry::WebViewExtWindows;
    unsafe {
        let profile = webview.webview().cast::<ICoreWebView2_13>()?.Profile()?;
        profile
            .cast::<ICoreWebView2Profile3>()?
            .SetPreferredTrackingPreventionLevel(COREWEBVIEW2_TRACKING_PREVENTION_LEVEL_NONE)
    }
}
