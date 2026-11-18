use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::session::UiMode;
use swofty_i18n::tr;

use super::{Step, art};
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::Palette;

const ENTER: Duration = Duration::from_millis(420);

pub(super) fn enter(step: Step, body: impl IntoElement) -> impl IntoElement {
    div().child(body).with_animation(
        ("onboarding-step", step.order()),
        Animation::new(ENTER).with_easing(ease_out_quint()),
        |el, t| el.opacity(t).relative().top(px((1. - t) * 18.)),
    )
}

pub(super) fn dots(step: Step, steps: &[Step], p: Palette) -> impl IntoElement {
    h_flex()
        .gap(px(8.))
        .items_center()
        .children(steps.iter().map(|s| {
            let active = *s == step;
            div()
                .h(px(8.))
                .w(px(if active { 24. } else { 8. }))
                .rounded_full()
                .bg(if active { p.accent } else { p.switch_off })
        }))
}

pub(super) fn heading(title: &'static str, subtitle: &'static str, p: Palette) -> impl IntoElement {
    v_flex()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(30.))
                .font_weight(FontWeight::BOLD)
                .child(title),
        )
        .child(div().text_size(px(14.)).text_color(p.text_muted).child(subtitle))
}

fn checked(p: Palette) -> impl IntoElement {
    div()
        .absolute()
        .top(px(12.))
        .right(px(12.))
        .size(px(24.))
        .rounded_full()
        .bg(p.accent)
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(IconName::Check).size(px(14.)).text_color(p.on_accent))
}

fn choice(id: SharedString, selected: bool, p: Palette) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .cursor_pointer()
        .rounded(px(14.))
        .border_2()
        .border_color(if selected { p.accent } else { p.border })
        .bg(if selected { p.selected } else { p.surface })
        .hover(|s| s.bg(p.hover))
        .focus_visible(|s| s.border_color(p.accent_hover))
        .tab_index(0)
        .when(selected, |d| d.child(checked(p)))
}

fn english_name(code: &str) -> Option<&'static str> {
    match code {
        "de" => Some(tr("German")),
        "en" => Some(tr("English")),
        "es" => Some(tr("Spanish")),
        "ko" => Some(tr("Korean")),
        "pt-BR" => Some(tr("Portuguese (Brazil)")),
        "ru" => Some(tr("Russian")),
        "zh-CN" => Some(tr("Chinese (Simplified)")),
        _ => None,
    }
}

impl Shell {
    pub(super) fn language_step(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let current = swofty_i18n::language();
        let cards = swofty_i18n::languages().into_iter().map(|l| {
            let code = l.code;
            choice(format!("onboarding-language-{code}").into(), code == current, p)
                .w(px(220.))
                .h(px(80.))
                .px(px(20.))
                .flex()
                .items_center()
                .text_size(px(18.))
                .font_weight(FontWeight::MEDIUM)
                .child(v_flex().gap(px(2.)).child(l.name).when_some(
                    english_name(code).filter(|n| *n != l.name),
                    |d, n| {
                        d.child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::NORMAL)
                                .text_color(p.text_muted)
                                .child(n),
                        )
                    },
                ))
                .on_click(cx.listener(move |this, _, window, cx| this.set_language(code, window, cx)))
        });
        v_flex()
            .items_center()
            .gap(px(28.))
            .max_w(px(780.))
            .child(art::globe())
            .child(heading(
                tr("Choose your language"),
                tr("You can change it later in Settings."),
                p,
            ))
            .child(h_flex().flex_wrap().justify_center().gap(px(14.)).children(cards))
    }

    pub(super) fn display_step(&self, selected: UiMode, p: Palette, cx: &mut Context<Self>) -> Div {
        let option = |mode: UiMode, id: &'static str, title: &'static str, note: &'static str| {
            choice(id.into(), selected == mode, p)
                .w(px(art::WIDTH + 48.))
                .p(px(24.))
                .child(
                    v_flex()
                        .items_center()
                        .gap(px(16.))
                        .child(art::display(mode))
                        .child(
                            v_flex()
                                .items_center()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .text_size(px(20.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .h(px(40.))
                                        .text_center()
                                        .text_size(px(13.))
                                        .text_color(p.text_muted)
                                        .child(note),
                                ),
                        ),
                )
                .on_click(cx.listener(move |this, _, window, cx| this.onboarding_pick_mode(mode, window, cx)))
        };
        v_flex()
            .items_center()
            .gap(px(28.))
            .child(heading(
                tr("How do you play?"),
                tr("Pick the layout that suits your screen."),
                p,
            ))
            .child(
                h_flex()
                    .gap(px(24.))
                    .child(option(
                        UiMode::Pc,
                        "onboarding-mode-pc",
                        tr("PC"),
                        tr("The normal layout, for a mouse and keyboard."),
                    ))
                    .child(option(
                        UiMode::Tv,
                        "onboarding-mode-tv",
                        tr("TV or handheld"),
                        tr("Larger text and controls, for the couch or a handheld."),
                    )),
            )
    }
}

pub(super) fn done(p: Palette) -> Div {
    let badge = div()
        .size(px(96.))
        .rounded_full()
        .bg(p.success.opacity(0.16))
        .border_2()
        .border_color(p.success)
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(IconName::Check).size(px(44.)).text_color(p.success))
        .with_animation(
            "onboarding-done-badge",
            Animation::new(Duration::from_millis(700)).with_easing(ease_out_quint()),
            |el, t| el.opacity(t).size(px(96. * (0.5 + 0.5 * t))),
        );
    v_flex()
        .items_center()
        .gap(px(24.))
        .child(div().h(px(96.)).flex().items_center().child(badge))
        .child(heading(
            tr("You're all set"),
            tr("Find addons in the browser, then press Play."),
            p,
        ))
}
