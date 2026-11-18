use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::{Disableable, Icon, Sizable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::time::Duration;
use swofty_core::support::{BIO_MAX, Goal, Kind, Supporter, format_date, tier_name};
use swofty_i18n::{tr, trf};

use crate::pages::GUTTER;
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::{Palette, ScrollArea};
use crate::time_trial::Load;
use crate::ui::Overlay;

const LIST_WIDTH: f32 = 470.;
const YELLOW: u32 = 0xffc933;
const PINK: u32 = 0xff5fa2;
const HOT_PINK: u32 = 0xff2d95;
const PURPLE: u32 = 0x9b30ff;
const PLUS_NAME: u32 = 0xd45af0;

fn gradient() -> Background {
    linear_gradient(
        90.,
        linear_color_stop(rgb(HOT_PINK), 0.),
        linear_color_stop(rgb(PURPLE), 1.),
    )
}

fn money(cents: u64) -> String {
    match cents % 100 {
        0 => format!("${}", cents / 100),
        c => format!("${}.{c:02}", cents / 100),
    }
}

fn section() -> Div {
    v_flex().gap(px(10.))
}

fn heading(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.into())
}

fn muted(text: impl Into<SharedString>, p: Palette) -> Div {
    div()
        .text_size(px(12.))
        .text_color(p.text_muted)
        .child(text.into())
}

/// A soft white streak that sweeps left to right across its parent, which must be relative and clip.
fn glint(id: impl Into<ElementId>, every: Duration) -> impl IntoElement {
    let half = |from: u32, to: u32| {
        div().h_full().w(px(9.)).bg(linear_gradient(
            90.,
            linear_color_stop(rgba(from), 0.),
            linear_color_stop(rgba(to), 1.),
        ))
    };
    h_flex()
        .absolute()
        .top_0()
        .bottom_0()
        .child(half(0xffffff00, 0xffffff99))
        .child(half(0xffffff99, 0xffffff00))
        .with_animation(id, Animation::new(every).repeat(), |el, t| {
            el.left(relative(t * 2.4 - 0.4))
        })
}

fn badge(id: impl Into<ElementId>, plus: bool) -> Div {
    div()
        .relative()
        .overflow_hidden()
        .flex_shrink_0()
        .px(px(8.))
        .py(px(2.))
        .rounded_full()
        .text_size(px(11.))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffffff))
        .map(|d| match plus {
            true => d.bg(gradient()).child(glint(id, Duration::from_millis(3200))),
            false => d.bg(rgb(PINK)),
        })
        .child(tier_name(if plus { "supporter_plus" } else { "supporter" }))
}

impl Shell {
    pub(crate) fn support_page(
        &mut self,
        p: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let refresh = Button::new("support-refresh")
            .ghost()
            .icon(IconName::ArrowRightLeft)
            .on_click(cx.listener(|this, _, _, cx| this.support_reload(cx)));
        let claim = Button::new("support-claim-open")
            .outline()
            .label(tr("Claim previous donation"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.support.claim_open = true;
                this.support.notice = None;
                cx.notify();
            }));
        let inputs = self.support_inputs(window, cx);
        if !self.support.bio_synced
            && let Load::Ready(me) = &self.support.me
        {
            let bio = me.bio.clone().unwrap_or_default();
            inputs.0.update(cx, |s, cx| s.set_value(bio, window, cx));
            self.support.bio_synced = true;
        }
        let list = self.support_list(p);
        let modal = self
            .support
            .claim_open
            .then(|| self.support_claim_modal(inputs.2.clone(), inputs.3.clone(), p, cx));
        let main = self.support_main(inputs, p, cx);
        let header = self.header(
            tr("Support"),
            tr("Keep Swofty Launcher free and open source").to_owned(),
            h_flex().gap(px(8.)).items_center().child(claim).child(refresh),
            p,
        );
        v_flex()
            .relative()
            .size_full()
            .child(header)
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .gap(px(24.))
                    .px(px(GUTTER))
                    .pb(px(16.))
                    .child(list)
                    .child(main),
            )
            .children(modal)
            .into_any_element()
    }

    fn support_claim_modal(
        &self,
        email: Entity<gpui_kit::component::input::InputState>,
        payment: Entity<gpui_kit::component::input::InputState>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let busy = self.support.busy;
        let weak = cx.entity().downgrade();
        let (e, pay) = (email.clone(), payment.clone());
        let notice = self.support.notice.as_ref().map(|(ok, text)| {
            div()
                .text_size(px(13.))
                .text_color(if *ok { rgb(0x4cc38a).into() } else { p.error })
                .child(text.clone())
        });
        let claimed = self.support.claimed.as_ref().map(|c| {
            let when = c.expires_at.as_ref().map(format_date).unwrap_or_default();
            let mut line = trf!(
                "You're {tier}: {months} months left (expires {date})",
                tier = tier_name(&c.tier),
                months = c.months_left,
                date = when
            );
            if c.role_granted {
                line.push_str(&format!(", {}", tr("role granted in Discord")));
            }
            div().text_size(px(13.)).text_color(rgb(0x4cc38a)).child(line)
        });
        let panel = v_flex()
            .w(px(440.))
            .gap(px(12.))
            .p(px(20.))
            .rounded(px(12.))
            .border_1()
            .border_color(p.border)
            .bg(p.background)
            .child(div().text_size(px(17.)).font_weight(FontWeight::BOLD).child(tr("Claim previous donation")))
            .child(muted(
                tr("Paid on Stripe outside the launcher? Enter the email you paid with and the invoice or payment ID to get your tier and Discord role."),
                p,
            ))
            .child(Input::new(&email).small())
            .child(Input::new(&payment).small())
            .children(notice)
            .children(claimed)
            .child(
                h_flex()
                    .gap(px(8.))
                    .justify_end()
                    .child(
                        Button::new("support-claim-close")
                            .small()
                            .ghost()
                            .label(tr("Close"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.support.claim_open = false;
                                this.support.notice = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("support-claim")
                            .small()
                            .primary()
                            .label(if self.mp.is_some() { tr("Claim") } else { tr("Sign in to claim") })
                            .disabled(busy || self.mp.is_none())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let (a, b) = (e.read(cx).value().to_string(), pay.read(cx).value().to_string());
                                if a.trim().is_empty() || b.trim().is_empty() {
                                    this.support.notice =
                                        Some((false, tr("Enter both the email and the payment ID.").to_owned()));
                                    cx.notify();
                                } else {
                                    this.support_claim(a, b, cx);
                                }
                            })),
                    ),
            );
        Overlay::new("support-claim-modal")
            .on_dismiss(move |_, cx| {
                weak.update(cx, |this, cx| {
                    this.support.claim_open = false;
                    cx.notify();
                })
                .ok();
            })
            .child(panel)
            .into_any_element()
    }

    fn support_list(&self, p: Palette) -> AnyElement {
        let frame = || v_flex().w(px(LIST_WIDTH)).flex_shrink_0().h_full().min_h_0();
        let note = |text: String| {
            frame()
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(text)
                .into_any_element()
        };
        let supporters = match &self.support.supporters {
            Load::Idle | Load::Loading => return note(tr("Loading supporters…").to_owned()),
            Load::Failed(e) => return note(e.clone()),
            Load::Ready(s) if s.is_empty() => return note(tr("No supporters yet. Be the first.").to_owned()),
            Load::Ready(s) => s,
        };
        let rows: Vec<AnyElement> = supporters
            .iter()
            .enumerate()
            .map(|(i, s)| self.supporter_row(i, s, p))
            .collect();
        frame()
            .child(
                h_flex()
                    .pb(px(6.))
                    .gap(px(8.))
                    .items_center()
                    .child(heading(tr("Supporters")))
                    .child(muted(supporters.len().to_string(), p)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .scroll_y("support-list")
                    .child(h_flex().flex_wrap().items_start().pr(px(8.)).children(rows)),
            )
            .into_any_element()
    }

    fn supporter_row(&self, i: usize, s: &Supporter, p: Palette) -> AnyElement {
        let name = match (&s.display.name, s.display.anonymous) {
            (Some(n), false) => n.clone(),
            _ => tr("Anonymous").to_owned(),
        };
        let face = match (&s.display.name, s.display.anonymous) {
            (Some(n), false) => self.face(n, s.display.avatar_url.as_deref(), 36., p),
            _ => div()
                .size(px(36.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(p.surface)
                .child(Icon::new(IconName::User).size(px(18.)).text_color(p.text_muted)),
        };
        let plus = s.plus();
        let face = face.when(plus, |d| d.border_2().border_color(rgb(PURPLE)));
        let bio = s.bio.clone().filter(|b| !b.trim().is_empty());
        h_flex()
            .id(SharedString::from(format!("supporter-{i}")))
            .w(relative(0.5))
            .py(px(8.))
            .pr(px(8.))
            .gap(px(10.))
            .items_center()
            .child(face)
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(3.))
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .items_center()
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(if plus { PLUS_NAME } else { PINK }))
                                    .child(name),
                            )
                            .child(badge(SharedString::from(format!("supporter-glint-{i}")), plus)),
                    )
                    .children(bio.map(|b| muted(b, p))),
            )
            .into_any_element()
    }

    fn support_main(
        &mut self,
        inputs: (
            Entity<gpui_kit::component::input::InputState>,
            Entity<gpui_kit::component::input::InputState>,
            Entity<gpui_kit::component::input::InputState>,
            Entity<gpui_kit::component::input::InputState>,
        ),
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (bio, amount, _, _) = inputs;
        let busy = self.support.busy;
        let goal: AnyElement = match &self.support.goal {
            Load::Idle | Load::Loading => muted(tr("Loading the goal…"), p).into_any_element(),
            Load::Failed(e) => muted(e.clone(), p).into_any_element(),
            Load::Ready(g) => goal_body(g, p).into_any_element(),
        };
        let typed = amount.read(cx).value().to_string();
        let column = || v_flex().items_center().gap(px(8.));
        let onetime = column()
            .child(
                div()
                    .id("support-onetime")
                    .flex()
                    .items_center()
                    .h(px(46.))
                    .px(px(22.))
                    .rounded_full()
                    .bg(rgb(YELLOW))
                    .text_color(rgb(0x1a1400))
                    .text_size(px(15.))
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .when(busy, |d| d.opacity(0.6))
                    .when(!busy, |d| d.cursor_pointer().hover(|d| d.opacity(0.9)))
                    .child(tr("ONE TIME"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.support_checkout(Kind::OneTime, Some(typed.clone()), cx)
                    })),
            )
            .child(muted(tr("Converts to whole months of a tier"), p))
            .child(
                div().w(px(200.)).child(
                    Input::new(&amount)
                        .small()
                        .prefix(div().text_color(p.text_muted).child("$")),
                ),
            );
        let perks = |lines: &[&'static str]| {
            v_flex().gap(px(6.)).pt(px(4.)).children(lines.iter().map(|t| {
                h_flex()
                    .gap(px(8.))
                    .items_start()
                    .child(
                        Icon::new(IconName::Check)
                            .size(px(14.))
                            .mt(px(2.))
                            .text_color(rgb(PINK)),
                    )
                    .child(div().text_size(px(13.)).child(*t))
            }))
        };
        let tiers = h_flex()
            .w_full()
            .justify_center()
            .items_start()
            .gap(px(28.))
            .flex_wrap()
            .child(
                column()
                    .w(px(280.))
                    .child(self.tier_pill(Kind::Supporter, "$5", false, busy, p, cx))
                    .child(perks(&[
                        tr("No ads in the launcher"),
                        tr("Access to #f1-supporter-leaks"),
                        tr("The #f1-supporter-general chat"),
                        tr("Discord embed links and external emoji permissions"),
                        tr("Supporting a free and open-source launcher"),
                    ])),
            )
            .child(
                column()
                    .w(px(280.))
                    .child(self.tier_pill(Kind::SupporterPlus, "$20", true, busy, p, cx))
                    .child(perks(&[
                        tr("All of Supporter"),
                        tr("Access to private betatesting"),
                    ])),
            )
            .child(onetime.w(px(280.)));
        let status = self.support_status(bio, p, cx);
        let notice = self
            .support
            .notice
            .as_ref()
            .filter(|_| !self.support.claim_open)
            .map(|(ok, text)| {
                div()
                    .text_size(px(13.))
                    .text_color(if *ok { rgb(0x4cc38a).into() } else { p.error })
                    .child(text.clone())
            });
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .min_h_0()
            .scroll_y("support-main")
            .child(
                v_flex()
                    .gap(px(26.))
                    .pr(px(4.))
                    .child(goal)
                    .children(status)
                    .child(tiers)
                    .children(notice),
            )
            .into_any_element()
    }

    fn tier_pill(
        &self,
        kind: Kind,
        price: &'static str,
        plus: bool,
        busy: bool,
        _p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let signed_in = self.mp.is_some();
        let name = tier_name(if plus { "supporter_plus" } else { "supporter" }).to_uppercase();
        div()
            .id(if plus { "support-plus" } else { "support-basic" })
            .relative()
            .overflow_hidden()
            .flex()
            .items_center()
            .gap(px(14.))
            .h(px(46.))
            .px(px(22.))
            .rounded_full()
            .text_color(rgb(0xffffff))
            .map(|d| match plus {
                true => d
                    .bg(gradient())
                    .child(glint("support-plus-glint", Duration::from_millis(2600))),
                false => d.bg(rgb(PINK)),
            })
            .when(busy, |d| d.opacity(0.6))
            .when(!busy, |d| d.cursor_pointer().hover(|d| d.opacity(0.9)))
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .child(name),
            )
            .child(div().w(px(1.)).h(px(18.)).bg(rgba(0xffffff66)))
            .child(
                div()
                    .text_size(px(14.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!("{price}{}", tr(" / month"))),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                if signed_in {
                    this.support_checkout(kind, None, cx);
                } else {
                    this.support.notice = Some((false, tr("Sign in with Discord to subscribe.").to_owned()));
                    cx.notify();
                }
            }))
            .into_any_element()
    }

    fn support_status(
        &mut self,
        bio: Entity<gpui_kit::component::input::InputState>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let Load::Ready(me) = &self.support.me else {
            return None;
        };
        if !me.active {
            return None;
        }
        let tier = me.tier.clone().unwrap_or_default();
        let plus = tier == "supporter_plus";
        let when = me.expires_at.as_ref().map(format_date).unwrap_or_default();
        let months = me.months_left.unwrap_or(0);
        let typed = bio.read(cx).value().to_string();
        let count = typed.chars().count();
        let mode = self.support.mode.clone();
        let busy = self.support.busy;
        let choice = |id: &'static str, key: &'static str, label: &'static str, cx: &mut Context<Self>| {
            Button::new(id)
                .small()
                .when_else(mode == key, |b| b.primary(), |b| b.outline())
                .label(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.support.mode = key.to_owned();
                    cx.notify();
                }))
        };
        Some(
            section()
                .child(
                    h_flex()
                        .gap(px(10.))
                        .items_center()
                        .child(badge("support-me-glint", plus))
                        .child(heading(trf!(
                            "{months} months left (expires {date})",
                            months = months,
                            date = when
                        ))),
                )
                .child(muted(tr("Your line next to your name in the supporter list"), p))
                .child(Input::new(&bio).small())
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(if count > BIO_MAX { p.error } else { p.text_muted })
                        .child(format!("{count} / {BIO_MAX}")),
                )
                .child(
                    h_flex()
                        .gap(px(8.))
                        .items_center()
                        .child(muted(tr("Show me as"), p))
                        .child(choice("mode-discord", "discord", tr("Discord name & avatar"), cx))
                        .child(choice("mode-anon", "anonymous", tr("Anonymous"), cx)),
                )
                .child(
                    h_flex().child(
                        Button::new("support-save")
                            .small()
                            .primary()
                            .label(tr("Save"))
                            .disabled(busy || count > BIO_MAX)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.support_save(typed.clone(), cx)),
                            ),
                    ),
                )
                .into_any_element(),
        )
    }
}

fn goal_body(g: &Goal, p: Palette) -> Div {
    let filled = g.fraction();
    // Thin lines that drift up and fade above the filled part of the bar.
    let floats = (0..9usize).map(|i| {
        let at = filled * ((i as f32 * 0.137 + 0.05) % 1.);
        let wide = 10. + (i % 4) as f32 * 7.;
        let period = Duration::from_millis(2200 + (i as u64 * 430) % 1900);
        div()
            .absolute()
            .left(relative(at))
            .h(px(2.))
            .w(px(wide))
            .rounded_full()
            .bg(rgb(if i % 2 == 0 { HOT_PINK } else { PURPLE }))
            .with_animation(
                SharedString::from(format!("goal-float-{i}")),
                Animation::new(period).repeat(),
                move |el, t| {
                    let t = (t + i as f32 * 0.21) % 1.;
                    el.top(px(30. - 30. * t))
                        .ml(px(18. * t))
                        .opacity((t * std::f32::consts::PI).sin() * 0.8)
                },
            )
    });
    // Bright sparks that run along the filled part.
    let sparks = (0..3usize).map(|i| {
        let half = |from: u32, to: u32| {
            div().h_full().w(px(22.)).bg(linear_gradient(
                90.,
                linear_color_stop(rgba(from), 0.),
                linear_color_stop(rgba(to), 1.),
            ))
        };
        h_flex()
            .absolute()
            .top_0()
            .bottom_0()
            .child(half(0xffffff00, 0xffffffcc))
            .child(half(0xffffffcc, 0xffffff00))
            .with_animation(
                SharedString::from(format!("goal-spark-{i}")),
                Animation::new(Duration::from_millis(1700 + i as u64 * 650)).repeat(),
                move |el, t| el.left(relative(((t + i as f32 * 0.37) % 1.) * 1.3 - 0.2)),
            )
    });
    v_flex()
        .gap(px(6.))
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(PINK))
                .child(tr("Monthly goal").to_uppercase()),
        )
        .child(
            h_flex()
                .items_end()
                .gap(px(10.))
                .child(
                    div()
                        .text_size(px(44.))
                        .line_height(px(46.))
                        .font_weight(FontWeight::EXTRA_BOLD)
                        .child(money(g.raised_cents)),
                )
                .child(
                    div()
                        .pb(px(6.))
                        .text_size(px(16.))
                        .text_color(p.text_muted)
                        .child(format!("{} {} {}", tr("of"), money(g.goal_cents), tr("/ month"))),
                ),
        )
        .child(div().relative().h(px(32.)).w_full().children(floats))
        .child(
            div()
                .h(px(20.))
                .w_full()
                .rounded_full()
                .bg(p.border_variant)
                .overflow_hidden()
                .child(
                    div()
                        .relative()
                        .overflow_hidden()
                        .h_full()
                        .w(relative(filled))
                        .rounded_full()
                        .bg(gradient())
                        .children(sparks),
                ),
        )
        .child(
            h_flex()
                .w_full()
                .pt(px(12.))
                .justify_center()
                .flex_wrap()
                .gap_x(px(28.))
                .gap_y(px(6.))
                .children(g.costs.iter().map(|c| {
                    h_flex()
                        .gap(px(10.))
                        .items_center()
                        .text_size(px(15.))
                        .child(div().size(px(6.)).rounded_full().bg(rgb(PINK)))
                        .child(c.clone())
                })),
        )
}
