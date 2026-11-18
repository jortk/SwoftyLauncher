//! Ad slots: native webviews over reserved rectangles of the window, from the
//! swofty-ads crate. The Windows host keeps them in this process; on Linux the
//! `swofty-ads` helper hosts them inside the window when it runs on X11.
//!
//! A slot is a [`canvas`] that reserves its unit's size and places the webview
//! over its bounds as it prepaints. [`Ads::sweep`], the last element the shell
//! paints, hides every slot that wasn't placed that frame, and every slot
//! while an overlay could be under one. Builds without `SWOFTY_ADS_URL` show
//! no slots.

use gpui_kit::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};
use swofty_ads::Consent;

/// How long the game runs before the slots close. A game that stops sooner,
/// such as one that crashes as it starts, leaves them as they were.
pub(crate) const RELEASE_AFTER: Duration = Duration::from_secs(30);

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::{Backend, Opener, View};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::{Backend, Opener, View};
#[cfg(not(any(windows, target_os = "linux")))]
mod none;
#[cfg(not(any(windows, target_os = "linux")))]
use none::{Backend, Opener, View};
mod ui;

pub(crate) use ui::AdVideo;

/// Where a slot sits. Each has its own webview, even two that show the same unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Unit {
    Home,
    Discover,
    Addon,
    Online,
    Video,
    /// RevIQ's consent form, which onboarding shows full size.
    Consent,
}

impl Unit {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Discover => "discover",
            Self::Addon => "addon",
            Self::Online => "online",
            Self::Video => "video",
            Self::Consent => "consent",
        }
    }

    /// The RevIQ unit the slot shows.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Home => "right-rail-1",
            Self::Addon => "left-rail-1",
            Self::Discover | Self::Online => "leaderboard-1",
            Self::Video => "video",
            Self::Consent => "consent",
        }
    }

    /// In CSS pixels, which match the window's logical pixels whatever the UI scale.
    pub(crate) fn size(self) -> Size<Pixels> {
        match self {
            Self::Home | Self::Addon => size(px(300.), px(250.)),
            Self::Discover | Self::Online => size(px(728.), px(90.)),
            Self::Video => size(px(400.), px(225.)),
            Self::Consent => size(px(680.), px(420.)),
        }
    }
}

/// A slot's rectangle in physical pixels inside the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Area {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Area {
    pub(crate) fn of(bounds: Bounds<Pixels>, scale: f32) -> Self {
        let edge = |v: Pixels| (f32::from(v) * scale).round();
        let (left, top) = (edge(bounds.origin.x), edge(bounds.origin.y));
        let (right, bottom) = (
            edge(bounds.origin.x + bounds.size.width),
            edge(bounds.origin.y + bounds.size.height),
        );
        Self {
            x: left as i32,
            y: top as i32,
            width: (right - left).max(1.) as u32,
            height: (bottom - top).max(1.) as u32,
        }
    }
}

struct Placed {
    url: String,
    area: Area,
    scale: f32,
    shown: bool,
    painted: u64,
}

pub(crate) struct Host {
    base: String,
    backend: Backend,
    slots: HashMap<Unit, Placed>,
    frame: u64,
    covered: bool,
    paused: bool,
    game_since: Option<Instant>,
    consent: Option<Consent>,
}

impl Host {
    /// Shows `unit` over `bounds`. A slot that isn't open yet comes back as an
    /// [`Opener`] to run once this borrow is over: WebView2 pumps the window's
    /// messages while it starts, and the window may paint in between.
    /// A slot cut off by its container hides, since the webview can't be clipped.
    fn place(
        &mut self,
        unit: Unit,
        url: String,
        bounds: Bounds<Pixels>,
        clip: Bounds<Pixels>,
        scale: f32,
    ) -> Option<Opener> {
        let area = Area::of(bounds, scale);
        let frame = self.frame;
        let covered = self.covered || !inside(bounds, clip);
        let slot = self.slots.entry(unit).or_insert_with(|| Placed {
            url: String::new(),
            area,
            scale,
            shown: false,
            painted: frame,
        });
        slot.painted = frame;
        if covered {
            if slot.shown {
                slot.shown = false;
                self.backend.hide(unit);
            }
            return None;
        }
        if slot.shown && slot.area == area && slot.url == url && slot.scale == scale {
            return None;
        }
        slot.area = area;
        slot.url = url;
        slot.scale = scale;
        slot.shown = true;
        self.backend.show(unit, &slot.url, area, scale)
    }

    fn sweep(&mut self) {
        for (unit, slot) in &mut self.slots {
            if slot.shown && slot.painted != self.frame {
                slot.shown = false;
                self.backend.hide(*unit);
            }
        }
        self.frame += 1;
    }

    fn opened(&mut self, unit: Unit, view: Option<View>) {
        self.backend.adopt(unit, view);
        match self.slots.get(&unit) {
            Some(slot) if slot.shown && !self.covered => {
                let (url, area, scale) = (slot.url.clone(), slot.area, slot.scale);
                let _ = self.backend.show(unit, &url, area, scale);
            }
            Some(_) => self.backend.hide(unit),
            None => self.backend.close(unit),
        }
    }

    fn close(&mut self, unit: Unit) {
        if self.slots.remove(&unit).is_some() {
            self.backend.close(unit);
        }
    }

    fn release(&mut self) {
        for unit in self.slots.keys() {
            self.backend.close(*unit);
        }
        self.slots.clear();
    }
}

#[derive(Default)]
pub(crate) struct Ads {
    host: Option<Rc<RefCell<Host>>>,
}

impl Ads {
    /// `data` is where the slots keep their cookies.
    pub(crate) fn start(window: &Window, data: &Path) -> Self {
        let Some(base) = base_url() else {
            return Self::default();
        };
        let Some(backend) = Backend::start(window, data) else {
            return Self::default();
        };
        crate::app_log::step("ad slots ready");
        let host = Rc::new(RefCell::new(Host {
            base,
            backend,
            slots: HashMap::new(),
            frame: 0,
            covered: false,
            paused: false,
            game_since: None,
            consent: None,
        }));
        Self { host: Some(host) }
    }

    /// Off while paused, so pages lay out as if there were no ads.
    pub(crate) fn on(&self) -> bool {
        self.host.as_ref().is_some_and(|h| !h.borrow().paused)
    }

    /// Whether the game is running. Returns true as it starts, when the shell
    /// should call [`Ads::release_for_game`] after [`RELEASE_AFTER`]. The
    /// slots open again once it has stopped.
    pub(crate) fn game(&self, running: bool) -> bool {
        let Some(host) = &self.host else { return false };
        let host = &mut *host.borrow_mut();
        match (running, host.game_since) {
            (true, None) => {
                host.game_since = Some(Instant::now());
                true
            }
            (false, Some(_)) => {
                host.game_since = None;
                host.paused = false;
                false
            }
            _ => false,
        }
    }

    /// Closes every slot's webview, freeing its memory, once the game has run
    /// for [`RELEASE_AFTER`]. Returns whether it did.
    pub(crate) fn release_for_game(&self) -> bool {
        let Some(host) = &self.host else { return false };
        let host = &mut *host.borrow_mut();
        let long = host.game_since.is_some_and(|t| t.elapsed() >= RELEASE_AFTER);
        if !long || host.paused {
            return false;
        }
        host.release();
        host.paused = true;
        true
    }

    /// At the start of the shell's render: whether an overlay could be over a slot.
    pub(crate) fn begin(&self, covered: bool) {
        if let Some(host) = &self.host {
            host.borrow_mut().covered = covered;
        }
    }

    /// The window moved: slots that live in their own windows follow it.
    pub(crate) fn moved(&self) {
        if let Some(host) = &self.host {
            let host = &mut *host.borrow_mut();
            let shown = host
                .slots
                .iter()
                .filter(|(_, s)| s.shown)
                .map(|(u, s)| (*u, s.area));
            host.backend.moved(shown);
        }
    }

    /// Reads what the consent page reported. Returns whether that changed.
    pub(crate) fn poll_consent(&self) -> bool {
        let Some(host) = &self.host else { return false };
        let host = &mut *host.borrow_mut();
        let mut changed = false;
        for event in host.backend.events() {
            if event.slot == Unit::Consent.key() && host.consent != Some(event.consent) {
                host.consent = Some(event.consent);
                changed = true;
            }
        }
        changed
    }

    /// What the consent page last reported, `None` while it loads.
    pub(crate) fn consent(&self) -> Option<Consent> {
        self.host.as_ref().and_then(|h| h.borrow().consent)
    }

    /// Starts the consent form over, from a fresh page.
    pub(crate) fn restart_consent(&self) {
        if let Some(host) = &self.host {
            let host = &mut *host.borrow_mut();
            host.consent = None;
            host.close(Unit::Consent);
        }
    }

    /// Closes `unit`'s webview, freeing its memory.
    pub(crate) fn close(&self, unit: Unit) {
        if let Some(host) = &self.host {
            host.borrow_mut().close(unit);
        }
    }

    /// The reserved rectangle for `unit`, over which its webview shows, with
    /// `background` behind the ad while it loads.
    /// `uid` is the user's hashed email, which the page hands to RevIQ.
    pub(crate) fn slot(&self, unit: Unit, background: Hsla, uid: Option<&str>) -> Option<Canvas<()>> {
        let url = swofty_ads::slot_url(
            &self.host.as_ref()?.borrow().base,
            unit.name(),
            unit.key(),
            rgb_of(background),
            dev_placements(),
        );
        self.canvas(unit, swofty_ads::with_uid(url, uid), unit.size(), background)
    }

    /// RevIQ's consent form at `size`. `reopen` opens it over a stored answer.
    pub(crate) fn consent_form(
        &self,
        background: Hsla,
        reopen: bool,
        size: Size<Pixels>,
    ) -> Option<Canvas<()>> {
        let mut url = swofty_ads::consent_url(
            &self.host.as_ref()?.borrow().base,
            rgb_of(background),
            reopen,
            dev_placements(),
        );
        if let Some(country) = test_country() {
            url.push_str(&format!("&country={country}&rev-geo={country}"));
        }
        self.canvas(Unit::Consent, url, size, background)
    }

    fn canvas(&self, unit: Unit, url: String, size: Size<Pixels>, background: Hsla) -> Option<Canvas<()>> {
        if !self.on() {
            return None;
        }
        let host = Rc::downgrade(self.host.as_ref()?);
        Some(
            canvas(
                move |bounds, window, cx| {
                    let Some(strong) = host.upgrade() else { return };
                    let clip = window.content_mask().bounds;
                    let opener = strong
                        .borrow_mut()
                        .place(unit, url, bounds, clip, window.scale_factor());
                    if let Some(opener) = opener {
                        cx.spawn(async move |_| {
                            let view = opener.open();
                            if let Some(host) = host.upgrade() {
                                host.borrow_mut().opened(unit, view);
                            }
                        })
                        .detach();
                    }
                },
                |_, _, _, _| {},
            )
            .w(size.width)
            .h(size.height)
            .flex_shrink_0()
            .bg(background),
        )
    }

    /// The last element the shell paints; see the module docs.
    pub(crate) fn sweep(&self) -> Option<Canvas<()>> {
        let host = Rc::downgrade(self.host.as_ref()?);
        Some(
            canvas(
                |_, _, _| {},
                move |_, _, _, _| {
                    if let Some(host) = host.upgrade() {
                        host.borrow_mut().sweep();
                    }
                },
            )
            .absolute()
            .size_0(),
        )
    }
}

/// RevIQ's test placements, for debug builds unless `SWOFTY_ADS_DEV=0`, or any
/// build with `SWOFTY_ADS_DEV=1`.
fn dev_placements() -> bool {
    match std::env::var("SWOFTY_ADS_DEV").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => cfg!(debug_assertions),
    }
}

/// `SWOFTY_ADS_COUNTRY=DE` makes the consent form act as if the player were in
/// that country, for testing.
fn test_country() -> Option<String> {
    std::env::var("SWOFTY_ADS_COUNTRY")
        .ok()
        .filter(|c| c.len() == 2 && c.bytes().all(|b| b.is_ascii_uppercase()))
}

fn base_url() -> Option<String> {
    if std::env::var_os("SWOFTY_ADS").is_some_and(|v| v == "0") {
        return None;
    }
    std::env::var("SWOFTY_ADS_URL")
        .ok()
        .or_else(|| option_env!("SWOFTY_ADS_URL").map(str::to_owned))
        .filter(|u| !u.trim().is_empty())
}

fn inside(bounds: Bounds<Pixels>, clip: Bounds<Pixels>) -> bool {
    let slack = px(0.5);
    bounds.left() + slack >= clip.left()
        && bounds.top() + slack >= clip.top()
        && bounds.right() <= clip.right() + slack
        && bounds.bottom() <= clip.bottom() + slack
}

fn rgb_of(colour: Hsla) -> [u8; 3] {
    let c = Rgba::from(colour);
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    [byte(c.r), byte(c.g), byte(c.b)]
}

#[cfg(test)]
mod tests {
    use super::{Area, inside, rgb_of};
    use gpui_kit::{Bounds, Rgba, point, px, size};

    #[test]
    fn an_area_rounds_to_whole_physical_pixels() {
        let bounds = Bounds::new(point(px(10.3), px(20.5)), size(px(300.), px(250.)));
        assert_eq!(
            Area::of(bounds, 1.5),
            Area {
                x: 15,
                y: 31,
                width: 450,
                height: 375
            }
        );
    }

    #[test]
    fn a_slot_cut_off_by_its_container_is_not_inside_it() {
        let clip = Bounds::new(point(px(0.), px(0.)), size(px(800.), px(600.)));
        let slot = |y: f32| Bounds::new(point(px(100.), px(y)), size(px(300.), px(250.)));
        assert!(inside(slot(350.), clip));
        assert!(!inside(slot(351.), clip));
        assert!(!inside(slot(-10.), clip));
    }

    #[test]
    fn a_colour_turns_into_its_bytes() {
        assert_eq!(
            rgb_of(
                Rgba {
                    r: 1.,
                    g: 0.,
                    b: 0.5,
                    a: 1.
                }
                .into()
            ),
            [255, 0, 128]
        );
    }
}
