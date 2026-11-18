use std::time::Duration;

use gpui_kit::component::v_flex;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_i18n::tr;

use super::Step;
use crate::pages::logo;
use crate::scale::px;
use crate::theme::Palette;

const INTRO: Duration = Duration::from_millis(1500);
/// The product name, which stays the same in every language.
const PRODUCT: &str = "Swofty Launcher";
const RING_CYCLE: Duration = Duration::from_millis(3200);
const LOGO_SIZE: f32 = 112.;
const STAGE: f32 = 260.;

struct Streak {
    top: f32,
    width: f32,
    seconds: u64,
    offset: f32,
    alpha: f32,
}

const STREAKS: [Streak; 6] = [
    Streak {
        top: 0.16,
        width: 0.30,
        seconds: 7,
        offset: 0.0,
        alpha: 0.30,
    },
    Streak {
        top: 0.28,
        width: 0.18,
        seconds: 5,
        offset: 0.5,
        alpha: 0.22,
    },
    Streak {
        top: 0.46,
        width: 0.24,
        seconds: 9,
        offset: 0.25,
        alpha: 0.18,
    },
    Streak {
        top: 0.62,
        width: 0.34,
        seconds: 6,
        offset: 0.75,
        alpha: 0.26,
    },
    Streak {
        top: 0.78,
        width: 0.20,
        seconds: 8,
        offset: 0.1,
        alpha: 0.2,
    },
    Streak {
        top: 0.88,
        width: 0.28,
        seconds: 10,
        offset: 0.6,
        alpha: 0.16,
    },
];

fn within(t: f32, start: f32, end: f32) -> f32 {
    ((t - start) / (end - start)).clamp(0., 1.)
}

fn ease_out(t: f32) -> f32 {
    1. - (1. - t).powi(4)
}

pub(super) fn backdrop(step: Step, p: Palette) -> impl IntoElement {
    let glow = div()
        .absolute()
        .top_0()
        .left_0()
        .w_full()
        .h(relative(0.6))
        .bg(linear_gradient(
            180.,
            linear_color_stop(p.accent.opacity(0.14), 0.),
            linear_color_stop(p.accent.opacity(0.), 1.),
        ));
    let streaks = STREAKS.iter().enumerate().map(move |(i, s)| {
        let Streak {
            top,
            width,
            seconds,
            offset,
            alpha,
        } = *s;
        div()
            .absolute()
            .top(relative(top))
            .h(px(2.))
            .w(relative(width))
            .rounded_full()
            .bg(linear_gradient(
                90.,
                linear_color_stop(p.accent.opacity(0.), 0.),
                linear_color_stop(p.accent_hover.opacity(alpha), 1.),
            ))
            .with_animation(
                ("onboarding-streak", i),
                Animation::new(Duration::from_secs(seconds)).repeat(),
                move |el, t| {
                    let travel = (t + offset).fract();
                    el.left(relative(-width + travel * (1. + width)))
                },
            )
    });
    div()
        .absolute()
        .size_full()
        .child(glow)
        .when(step == Step::Welcome, |d| d.children(streaks))
}

fn ring(index: usize, p: Palette) -> impl IntoElement {
    let phase = index as f32 * 0.5;
    div()
        .absolute()
        .rounded_full()
        .border_2()
        .border_color(p.accent)
        .with_animation(
            ("onboarding-ring", index),
            Animation::new(RING_CYCLE).repeat(),
            move |el, t| {
                let t = (t + phase).fract();
                let size = LOGO_SIZE * (1. + 0.9 * ease_out(t));
                let inset = (STAGE - size) / 2.;
                el.left(px(inset))
                    .top(px(inset))
                    .size(px(size))
                    .opacity(0.8 * (1. - t).powi(2))
            },
        )
}

fn mark(p: Palette) -> impl IntoElement {
    let halo = div()
        .absolute()
        .left(px((STAGE - LOGO_SIZE * 1.5) / 2.))
        .top(px((STAGE - LOGO_SIZE * 1.5) / 2.))
        .size(px(LOGO_SIZE * 1.5))
        .rounded_full()
        .bg(p.accent.opacity(0.18))
        .with_animation(
            "onboarding-halo",
            Animation::new(Duration::from_millis(2600))
                .repeat()
                .with_easing(pulsating_between(0.35, 1.)),
            |el, t| el.opacity(t),
        );
    let logo = div()
        .absolute()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(logo(LOGO_SIZE))
        .with_animation("onboarding-logo", Animation::new(INTRO), |el, t| {
            let pop = ease_out(within(t, 0., 0.6));
            let drift = (1. - ease_out(within(t, 0., 0.8))) * 24.;
            el.opacity(pop)
                .relative()
                .top(px(drift))
                .size(px(STAGE * (0.55 + 0.45 * pop)))
        });
    div()
        .relative()
        .flex_shrink_0()
        .size(px(STAGE))
        .child(halo)
        .child(ring(0, p))
        .child(ring(1, p))
        .child(logo)
}

fn reveal(id: &'static str, start: f32, rise: f32, line: Div) -> impl IntoElement {
    line.with_animation(id, Animation::new(INTRO), move |el, t| {
        let local = ease_out(within(t, start, (start + 0.4).min(1.)));
        el.opacity(local).relative().top(px((1. - local) * rise))
    })
}

pub(super) fn view(p: Palette) -> Div {
    let kicker = div()
        .text_size(px(15.))
        .text_color(p.text_muted)
        .child(tr("Welcome to"));
    let title = div()
        .text_size(px(46.))
        .font_weight(FontWeight::BOLD)
        .text_color(p.text)
        .child(PRODUCT);
    let tagline = div()
        .max_w(px(460.))
        .text_center()
        .text_size(px(15.))
        .text_color(p.text_muted)
        .child(tr("Addons, mods and tracks for F1 25, in one place."));
    v_flex()
        .items_center()
        .gap(px(6.))
        .child(mark(p))
        .child(reveal("onboarding-kicker", 0.35, 14., kicker))
        .child(reveal("onboarding-title", 0.45, 20., title))
        .child(div().h(px(8.)))
        .child(reveal("onboarding-tagline", 0.6, 14., tagline))
}
