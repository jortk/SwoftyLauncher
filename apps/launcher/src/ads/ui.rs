//! Where the shell shows its ad slots: under the news page's release notes, a
//! banner over the Discover cards and under the Online filters, the side column
//! of an addon's page, and a floating video while the game builds and starts.
//! Signed-in supporters, admins, Swofty Launcher Support and addon creators,
//! as the server's support status says, see none. `SWOFTY_ADS_VIDEO=1` shows
//! the video without pressing Play, even while [`VIDEO_LIVE`] is off.

use super::Unit;
use crate::scale::{Hit, px};
use crate::shell::{PlayState, Shell};
use crate::theme::Palette;
use crate::time_trial::Load;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Disableable, WindowExt, h_flex, v_flex};
use gpui_kit::*;
use std::time::{Duration, Instant};
use swofty_i18n::tr;

/// The room each side of a banner in a page header keeps for the title and the
/// buttons, in logical pixels.
const HEADER_SIDE: f32 = 500.;

/// Off until RevIQ sets up the `video` unit, which would otherwise leave the
/// card empty.
const VIDEO_LIVE: bool = false;

/// How long the video plays before it can be closed.
const VIDEO_SKIP: Duration = Duration::from_secs(5);

/// The video for one press of Play.
pub(crate) struct AdVideo {
    since: Instant,
    closed: bool,
}

fn label(p: Palette) -> Div {
    div()
        .text_size(px(11.))
        .text_color(p.text_placeholder)
        .child(tr("Advertisement"))
}

impl Shell {
    /// Whether an overlay could be over a slot: slots draw over everything in
    /// the window, so they hide while one is open.
    pub(crate) fn ads_covered(&self, window: &mut Window, cx: &mut App) -> bool {
        self.popover.is_some()
            || self.tasks.is_some()
            || (self.crash_open && self.crash_view.is_some())
            || self.ban.is_some()
            || self.moving.is_some()
            || self.found_data.is_some()
            || self.dropping
            || window.has_active_dialog(cx)
            || window.has_active_sheet(cx)
    }

    /// Nobody signed in sees ads until their support status is known.
    pub(crate) fn ads_allowed(&self) -> bool {
        self.ads.on() && (self.mp.is_none() || matches!(&self.support.me, Load::Ready(me) if me.sees_ads()))
    }

    fn ad_uid(&self) -> Option<&str> {
        match &self.support.me {
            Load::Ready(me) => me.ad_uid.as_deref(),
            _ => None,
        }
    }

    /// Fetches the signed-in user's support once per sign-in, which decides their ads.
    pub(crate) fn sync_support(&mut self, cx: &mut Context<Self>) {
        if self.mp.is_none() {
            self.support.me = Load::Idle;
        } else if matches!(self.support.me, Load::Idle | Load::Failed(_)) {
            self.support_fetch_me(cx);
        }
    }

    fn ad_fits(unit: Unit, width: f32) -> bool {
        width * crate::scale::current() >= f32::from(unit.size().width)
    }

    fn ad_box(&self, unit: Unit, background: Hsla, p: Palette) -> Option<Div> {
        if !self.ads_allowed() {
            return None;
        }
        let slot = self.ads.slot(unit, background, self.ad_uid())?;
        Some(
            v_flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(6.))
                .child(label(p))
                .child(slot),
        )
    }

    /// The width, in layout units, the news page's side column needs for its ad.
    pub(crate) fn ad_rail_width(&self) -> Option<f32> {
        self.ads_allowed()
            .then(|| f32::from(Unit::Home.size().width) / crate::scale::current() + 2.)
    }

    /// Steps aside while the video is up, which floats over the page.
    pub(crate) fn ad_rail(&self, p: Palette) -> Option<AnyElement> {
        if self.ad_video.as_ref().is_some_and(|v| !v.closed) {
            return None;
        }
        Some(self.ad_box(Unit::Home, p.background, p)?.into_any_element())
    }

    /// Whether `unit` fits in the page header, centred between the title and the buttons.
    pub(crate) fn ad_header_room(&self, unit: Unit) -> bool {
        self.content_width * crate::scale::current() >= f32::from(unit.size().width) + 2. * HEADER_SIDE
    }

    /// `unit` for the middle of the page header, when it fits there.
    pub(crate) fn ad_header(&self, unit: Unit, p: Palette) -> Option<AnyElement> {
        if !self.ad_header_room(unit) {
            return None;
        }
        Some(self.ad_box(unit, p.background, p)?.into_any_element())
    }

    /// A banner centred in `width` layout units, when it fits.
    pub(crate) fn ad_banner(&self, unit: Unit, width: f32, p: Palette) -> Option<AnyElement> {
        if !Self::ad_fits(unit, width) {
            return None;
        }
        let ad = self.ad_box(unit, p.background, p)?;
        Some(h_flex().w_full().justify_center().child(ad).into_any_element())
    }

    /// At the top of an addon page's side column, `width` layout units wide.
    pub(crate) fn ad_addon(&self, width: f32, p: Palette) -> Option<AnyElement> {
        if !Self::ad_fits(Unit::Addon, width) {
            return None;
        }
        Some(self.ad_box(Unit::Addon, p.background, p)?.into_any_element())
    }

    /// The video from pressing Play until the game is running.
    pub(crate) fn ad_video(
        &mut self,
        state: &PlayState,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let starting = VIDEO_LIVE
            && matches!(
                state,
                PlayState::Preparing | PlayState::Building | PlayState::Launching
            );
        let starting = starting || std::env::var_os("SWOFTY_ADS_VIDEO").is_some_and(|v| v == "1");
        if !self.ads_allowed() || !starting {
            self.ad_video = None;
            return None;
        }
        let video = self.ad_video.get_or_insert_with(|| {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(VIDEO_SKIP).await;
                this.update(cx, |_, cx| cx.notify()).ok();
            })
            .detach();
            AdVideo {
                since: Instant::now(),
                closed: false,
            }
        });
        if video.closed {
            return None;
        }
        let closable = video.since.elapsed() >= VIDEO_SKIP;
        let slot = self.ads.slot(Unit::Video, p.surface, self.ad_uid())?;
        let close = Button::new("ad-video-close")
            .ghost()
            .tap_sm()
            .icon(IconName::X)
            .disabled(!closable)
            .on_click(cx.listener(|this, _, _, cx| {
                if let Some(video) = &mut this.ad_video {
                    video.closed = true;
                }
                cx.notify();
            }));
        Some(
            v_flex()
                .absolute()
                .right(px(16.))
                .bottom(px(crate::play_bar::HEIGHT + 16.))
                .p(px(8.))
                .gap(px(6.))
                .rounded(px(10.))
                .border_1()
                .border_color(p.border)
                .bg(p.surface)
                .shadow_lg()
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(label(p))
                        .child(close),
                )
                .child(slot)
                .into_any_element(),
        )
    }
}
