use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_ads::Consent;
use swofty_i18n::tr;

use super::steps::heading;
use crate::ads::{Ads, Unit};
use crate::scale::{Hit, px};
use crate::shell::{Shell, offline_terms};
use crate::theme::Palette;

const TERMS_URL: &str = "https://ads.swofty.net/terms";
const PRIVACY_URL: &str = "https://ads.swofty.net/privacy";
const WIDTH: f32 = 620.;

fn card(p: Palette) -> Div {
    div()
        .w_full()
        .p(px(20.))
        .rounded(px(14.))
        .bg(p.surface)
        .border_1()
        .border_color(p.border)
}

pub(super) fn eula(p: Palette) -> Div {
    v_flex()
        .items_center()
        .gap(px(24.))
        .w(px(WIDTH))
        .child(heading(
            tr("Swofty Launcher EULA"),
            tr("Read this before you start. You need to accept it to play."),
            p,
        ))
        .child(card(p).child(offline_terms(p)))
}

/// The consent page, loading out of sight during the earlier steps, so the ads
/// step knows at once whether anything has to be asked.
pub(super) fn preload(ads: &Ads, p: Palette) -> Option<impl IntoElement> {
    let form = ads.consent_form(p.surface, false, size(gpui_kit::px(1.), gpui_kit::px(1.)))?;
    Some(div().absolute().left_0().bottom_0().child(form))
}

fn link(id: &'static str, label: &'static str, url: &'static str) -> Button {
    Button::new(id)
        .ghost()
        .tap_sm()
        .icon(IconName::ExternalLink)
        .label(label)
        .on_click(move |_, _, cx| cx.open_url(url))
}

fn point(text: &'static str, p: Palette) -> impl IntoElement {
    h_flex()
        .gap(px(10.))
        .items_start()
        .child(
            div()
                .mt(px(7.))
                .size(px(6.))
                .flex_shrink_0()
                .rounded_full()
                .bg(p.accent),
        )
        .child(div().flex_1().text_size(px(14.)).text_color(p.text).child(text))
}

fn note(icon: IconName, colour: Hsla, text: &'static str, p: Palette) -> impl IntoElement {
    h_flex()
        .gap(px(8.))
        .items_center()
        .child(Icon::new(icon).size(px(16.)).text_color(colour))
        .child(div().text_size(px(13.)).text_color(p.text_muted).child(text))
}

impl Shell {
    pub(super) fn ads_step(&self, privacy_only: bool, reopen: bool, p: Palette) -> Div {
        let consent = self.ads.consent();
        let loading = consent.is_none() && !self.consent_settled();
        let asking = loading || consent == Some(Consent::Asking);
        let (title, subtitle) = if privacy_only {
            (
                tr("Ad privacy choices"),
                tr("Choose how ads in the launcher may use your data."),
            )
        } else {
            (
                tr("Ads keep the launcher free"),
                tr("Read how ads work in the launcher, then accept to continue."),
            )
        };
        let links = h_flex()
            .gap(px(4.))
            .justify_center()
            .child(link("onboarding-ad-terms", tr("Terms of Service"), TERMS_URL))
            .child(link("onboarding-ad-privacy", tr("Privacy Policy"), PRIVACY_URL));
        let status = match consent {
            Some(Consent::Given) => Some(note(
                IconName::CircleCheck,
                p.success,
                tr("Your ad privacy choices are saved. You can change them in Settings."),
                p,
            )),
            Some(Consent::Unavailable) | None if !loading => Some(note(
                IconName::Info,
                p.text_muted,
                tr("Ad privacy choices didn't load. You can set them later in Settings."),
                p,
            )),
            _ => None,
        };
        let step = v_flex()
            .items_center()
            .gap(px(20.))
            .child(heading(title, subtitle, p));
        if asking {
            let form = self.ads.consent_form(p.surface, reopen, Unit::Consent.size());
            return step
                .when(!privacy_only, |d| {
                    d.child(
                        div()
                            .max_w(px(WIDTH))
                            .text_center()
                            .text_size(px(13.))
                            .text_color(p.text_muted)
                            .child(tr(
                                "Ads come from RevIQ and its partners, including Google. Where the law asks for it, \
                                 choose below how they may use your data.",
                            )),
                    )
                })
                .children(form.map(|form| {
                    div()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(p.border)
                        .bg(p.surface)
                        .overflow_hidden()
                        .child(form)
                }))
                .child(links);
        }
        step.child(
            card(p).w(px(WIDTH)).child(
                v_flex()
                    .gap(px(12.))
                    .child(point(
                        tr("A few ads show on the Home, Discover, Online and addon pages. They close while you play."),
                        p,
                    ))
                    .child(point(
                        tr("Ads come from RevIQ and its partners, including Google, which may use cookies, your IP \
                            address and your approximate location to choose and measure them."),
                        p,
                    ))
                    .child(point(tr("Supporters see no ads."), p)),
            ),
        )
        .children(status)
        .child(links)
    }
}
