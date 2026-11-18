use super::images::Art;
use super::model::*;
use super::{Load, Online, Target};
use crate::scale::{Hit, px};
use crate::shell::Shell;
use crate::theme::Palette;
use crate::ui::{Callout, EmptyState, Segmented};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Disableable, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_i18n::tr;

pub(super) fn tone(seed: &str) -> Hsla {
    let mut h: u32 = 2166136261;
    for b in seed.bytes() {
        h = (h ^ u32::from(b)).wrapping_mul(16777619);
    }
    hsla((h % 360) as f32 / 360., 0.42, 0.34, 1.)
}

pub(super) fn medal(rank: u32, p: &Palette) -> Hsla {
    match rank {
        1 => hsla(0.125, 0.88, 0.58, 1.),
        2 => hsla(0.6, 0.12, 0.76, 1.),
        3 => hsla(0.07, 0.55, 0.55, 1.),
        _ => p.text_muted,
    }
}

pub(super) fn panel(p: Palette) -> Div {
    v_flex()
        .rounded(px(12.))
        .border_1()
        .border_color(p.border_variant)
        .bg(p.surface)
        .overflow_hidden()
}

pub(super) fn section_title(text: impl Into<SharedString>, p: Palette) -> Div {
    div()
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.text_muted)
        .child(text.into())
}

pub(super) fn chip(label: impl Into<SharedString>, p: Palette) -> Div {
    div()
        .px(px(7.))
        .py(px(1.))
        .rounded(px(5.))
        .bg(p.elevated)
        .text_size(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.text_muted)
        .child(label.into())
}

impl Online {
    pub(super) fn ringed(&self, index: usize) -> bool {
        self.cursor == Some(index)
    }
}

impl Shell {
    pub(super) fn online_art(&self, url: Option<&String>) -> Option<&Art> {
        url.and_then(|u| self.online.images.get(u))
    }

    pub(super) fn tt_face(&self, d: &Driver, size: f32, p: Palette) -> Div {
        let frame = div()
            .size(px(size))
            .flex_shrink_0()
            .rounded_full()
            .overflow_hidden()
            .border_1()
            .border_color(p.surface);
        match self.online_art(d.avatar.as_ref()) {
            Some(Art::Raster(image)) => {
                frame.child(img(image.clone()).size_full().object_fit(ObjectFit::Cover))
            }
            _ => frame
                .bg(tone(&d.name))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(size * 0.42))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(0xffffff))
                .child(
                    d.name
                        .chars()
                        .take(1)
                        .flat_map(char::to_uppercase)
                        .collect::<String>(),
                ),
        }
    }

    pub(super) fn season_icon(&self, id: &str, cover: Option<&String>, size: f32, p: Palette) -> Div {
        let addon = id.split('/').next().unwrap_or(id);
        if let Some(icon) = self
            .library
            .addons
            .iter()
            .find(|a| a.manifest.id == addon)
            .and_then(|a| a.icon.as_ref())
        {
            return crate::pages::addon_icon(icon, size, p);
        }
        if let Some(d) = self.catalog_icon_of(addon, size, p) {
            return d;
        }
        match self.online_art(cover) {
            Some(Art::Raster(image)) => div()
                .size(px(size))
                .flex_shrink_0()
                .rounded(px(size * 0.22))
                .overflow_hidden()
                .child(img(image.clone()).size_full().object_fit(ObjectFit::Cover)),
            _ => crate::pages::badge(IconName::Trophy, size, p),
        }
    }

    pub(super) fn filter_chip(
        &self,
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        on: bool,
        p: Palette,
        cx: &mut Context<Self>,
        change: impl Fn(&mut TrackFilter) + 'static,
    ) -> Stateful<Div> {
        let id: SharedString = id.into();
        h_flex()
            .id(id)
            .h(px(28.))
            .px(px(12.))
            .items_center()
            .rounded_full()
            .border_1()
            .cursor_pointer()
            .text_size(px(12.))
            .font_weight(FontWeight::MEDIUM)
            .map(|d| {
                if on {
                    d.bg(p.selected).border_color(p.accent).text_color(p.text)
                } else {
                    d.border_color(p.border)
                        .text_color(p.text_muted)
                        .hover(move |s| s.bg(p.hover))
                }
            })
            .child(label.into())
            .on_click(cx.listener(move |this, _, _, cx| {
                change(&mut this.online.filter_tracks);
                cx.notify();
            }))
    }

    pub(super) fn tt_category_bar(
        &self,
        cat: Category,
        local: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let set = move |this: &mut Self, next: Category, cx: &mut Context<Self>| {
            if local {
                this.online_set_board_category(next, cx);
            } else {
                this.online_set_category(next, cx);
            }
        };
        let handle = cx.entity().downgrade();
        let pick_assist = |label: &'static str, which: AssistFilter| {
            let handle = handle.clone();
            (
                label,
                cat.assists == which,
                move |_: &mut Window, cx: &mut App| {
                    handle
                        .update(cx, |s, cx| {
                            set(
                                s,
                                Category {
                                    assists: which,
                                    ..s.board_or_global(local)
                                },
                                cx,
                            )
                        })
                        .ok();
                },
            )
        };
        let pick_device = |label: &'static str, which: DeviceFilter| {
            let handle = handle.clone();
            (label, cat.device == which, move |_: &mut Window, cx: &mut App| {
                handle
                    .update(cx, |s, cx| {
                        set(
                            s,
                            Category {
                                device: which,
                                ..s.board_or_global(local)
                            },
                            cx,
                        )
                    })
                    .ok();
            })
        };
        let mut assists = Segmented::new(if local { "board-assists" } else { "cat-assists" }, p);
        for (l, on, f) in [
            pick_assist(tr("Either"), AssistFilter::Either),
            pick_assist(tr("No assists"), AssistFilter::None),
            pick_assist(tr("Assists"), AssistFilter::Assists),
        ] {
            assists = assists.item(l, on, f);
        }
        let mut device = Segmented::new(if local { "board-device" } else { "cat-device" }, p);
        for (l, on, f) in [
            pick_device(tr("All"), DeviceFilter::All),
            pick_device(tr("Wheel"), DeviceFilter::Wheel),
            pick_device(tr("Controller"), DeviceFilter::Controller),
            pick_device(tr("Keyboard"), DeviceFilter::Keyboard),
        ] {
            device = device.item(l, on, f);
        }
        h_flex()
            .gap(px(10.))
            .items_center()
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.text_placeholder)
                    .child(tr("ASSISTS")),
            )
            .child(assists)
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(p.text_placeholder)
                    .child(tr("INPUT")),
            )
            .child(device)
            .into_any_element()
    }

    pub(super) fn board_or_global(&self, local: bool) -> Category {
        if local {
            self.board_category()
        } else {
            self.online.category
        }
    }

    pub(super) fn input_icons(&self, r: &Row, p: Palette) -> Option<AnyElement> {
        let device = r.input.map(|d| match d {
            InputDevice::Wheel => IconName::Disc3,
            InputDevice::Controller => IconName::Gamepad2,
            InputDevice::Keyboard => IconName::Keyboard,
        });
        let aids = r.aids.as_ref();
        if device.is_none() && aids.is_none() {
            return None;
        }
        let icon_of = |a: Aid| match a {
            Aid::TractionControl => IconName::Gauge,
            Aid::Abs => IconName::CircleSlash,
            Aid::Gearbox => IconName::Cog,
            Aid::BrakingAssist => IconName::Hand,
            Aid::SteeringAssist => IconName::Navigation,
            Aid::RacingLine => IconName::Route,
            Aid::Ers => IconName::Zap,
        };
        let mut tip: Vec<String> = Vec::new();
        if let Some(d) = r.input {
            tip.push(tr(d.label()).to_owned());
        }
        if let Some(a) = aids {
            for aid in Aid::ALL {
                if let Some(level) = a.level(aid) {
                    tip.push(format!(
                        "{}: {}",
                        tr(aid.label()),
                        if level == 0 {
                            tr("Off").to_owned()
                        } else {
                            level.to_string()
                        }
                    ));
                }
            }
        }
        let tip = tip.join("\n");
        Some(
            h_flex()
                .id(SharedString::from(format!("icons-{}", r.lap_id)))
                .gap(px(5.))
                .items_center()
                .children(device.map(|i| Icon::new(i).size(px(15.)).text_color(p.text)))
                .children(aids.map(|a| {
                    if a.none() {
                        chip(tr("No assists"), p).into_any_element()
                    } else {
                        h_flex()
                            .gap(px(3.))
                            .children(Aid::TRIO.iter().map(|aid| {
                                Icon::new(icon_of(*aid)).size(px(13.)).text_color(if a.on(*aid) {
                                    p.accent_hover
                                } else {
                                    p.text_placeholder.opacity(0.45)
                                })
                            }))
                            .into_any_element()
                    }
                }))
                .tooltip(move |window, cx| {
                    gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx)
                })
                .into_any_element(),
        )
    }

    pub(super) fn tt_faces(&self, drivers: &[Driver], size: f32, p: Palette) -> Div {
        h_flex().children(
            drivers
                .iter()
                .enumerate()
                .map(|(i, d)| self.tt_face(d, size, p).when(i > 0, |f| f.ml(px(-size * 0.3)))),
        )
    }

    pub(super) fn layout_art(&self, url: Option<&String>, size: f32, color: Hsla, p: Palette) -> Div {
        let frame = div()
            .size(px(size))
            .flex_shrink_0()
            .rounded(px(10.))
            .bg(p.elevated)
            .flex()
            .items_center()
            .justify_center();
        match self.online_art(url) {
            Some(Art::Svg(data)) => frame.child(svg().data(data).size(px(size * 0.78)).text_color(color)),
            _ => frame.child(
                Icon::new(IconName::Route)
                    .size(px(size * 0.35))
                    .text_color(p.text_placeholder),
            ),
        }
    }

    pub(super) fn flag_chip(&self, code: &Option<String>, p: Palette) -> Option<Div> {
        code.as_ref().map(|c| chip(c.to_uppercase(), p))
    }

    /// The byline of each `shown` track, told apart from every custom track and loaded season track.
    pub(super) fn online_bylines(&self, shown: &[&TrackCard]) -> Vec<Option<String>> {
        let custom: &[TrackCard] = match &self.online.overview {
            Load::Ready(o) => &o.custom,
            _ => &[],
        };
        let seasons = self.online.seasons.values().flat_map(|s| match s {
            Load::Ready(page) => page.tracks.as_slice(),
            _ => &[],
        });
        let tracks: Vec<(&str, Option<&str>)> = shown
            .iter()
            .copied()
            .chain(custom)
            .chain(seasons)
            .map(|t| (t.name.as_str(), t.addon.as_deref()))
            .collect();
        let mut out = self.track_bylines(&tracks);
        out.truncate(shown.len());
        out
    }

    pub(super) fn crumb_bar(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let crumbs: Vec<(usize, String)> = self
            .online
            .trail
            .crumbs()
            .into_iter()
            .map(|(d, l)| (d, l.to_owned()))
            .collect();
        let last = crumbs.len();
        h_flex()
            .px(px(28.))
            .pt(px(18.))
            .pb(px(10.))
            .gap(px(8.))
            .items_center()
            .text_size(px(13.))
            .child(
                Button::new("trail-back")
                    .ghost()
                    .tap_sm()
                    .icon(IconName::ChevronLeft)
                    .disabled(self.online.trail.depth() <= 1)
                    .on_click(cx.listener(|this, _, _, cx| this.online_back(cx))),
            )
            .child(
                div()
                    .id("crumb-root")
                    .cursor_pointer()
                    .text_color(if last == 0 { p.text } else { p.text_muted })
                    .font_weight(if last == 0 {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .hover(move |s| s.text_color(p.text))
                    .child(tr("Time Trial"))
                    .on_click(cx.listener(|this, _, _, cx| this.online_jump(1, cx))),
            )
            .children(crumbs.into_iter().enumerate().flat_map(|(i, (depth, label))| {
                let current = i + 1 == last;
                [
                    Icon::new(IconName::ChevronRight)
                        .size(px(13.))
                        .text_color(p.text_placeholder)
                        .into_any_element(),
                    div()
                        .id(SharedString::from(format!("crumb-{depth}")))
                        .cursor_pointer()
                        .text_color(if current { p.text } else { p.text_muted })
                        .font_weight(if current {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .hover(move |s| s.text_color(p.text))
                        .child(label)
                        .on_click(cx.listener(move |this, _, _, cx| this.online_jump(depth, cx)))
                        .into_any_element(),
                ]
            }))
            .child(div().flex_1())
            .child(
                Button::new("online-refresh")
                    .ghost()
                    .tap_sm()
                    .icon(IconName::RefreshCw)
                    .label(tr("Refresh"))
                    .on_click(cx.listener(|this, _, _, cx| this.online_reload(cx))),
            )
            .into_any_element()
    }

    pub(super) fn sign_in_banner(&self, p: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.account_name()
            .is_none()
            .then(|| {
                div()
                    .px(px(28.))
                    .pb(px(8.))
                    .child(
                        Callout::info(tr("Log in with Discord to see your own place on the boards."))
                            .icon(IconName::User)
                            .actions(
                                h_flex().child(
                                    Button::new("online-login")
                                        .outline()
                                        .tap_sm()
                                        .label(tr("Log in with Discord"))
                                        .disabled(self.game_busy())
                                        .tooltip(if self.game_busy() {
                                            tr("Close the game to sign in or out")
                                        } else {
                                            tr("Log in with Discord")
                                        })
                                        .on_click(
                                            cx.listener(|this, _, window, cx| this.mp_sign_in(window, cx)),
                                        ),
                                ),
                            ),
                    )
                    .into_any_element()
            })
            .filter(|_| {
                let _ = p;
                true
            })
    }

    pub(super) fn skeleton(&self, p: Palette, rows: usize, height: f32) -> AnyElement {
        v_flex()
            .px(px(28.))
            .gap(px(12.))
            .children((0..rows).map(|_| {
                div()
                    .h(px(height))
                    .rounded(px(12.))
                    .bg(p.surface)
                    .border_1()
                    .border_color(p.border_variant)
            }))
            .into_any_element()
    }

    pub(super) fn failed_view(&self, e: &OnlineError, cx: &mut Context<Self>) -> AnyElement {
        EmptyState::new(IconName::CloudOff, tr("Can't load the boards"))
            .body(e.message.clone())
            .action(
                Button::new("online-retry")
                    .outline()
                    .tap_sm()
                    .icon(IconName::RefreshCw)
                    .label(tr("Try again"))
                    .on_click(cx.listener(|this, _, _, cx| this.online_reload(cx))),
            )
            .into_any_element()
    }

    pub(super) fn nothing_yet(&self) -> AnyElement {
        EmptyState::new(IconName::Trophy, tr("No laps yet"))
            .body(tr("Time Trial laps show up here once they are uploaded."))
            .into_any_element()
    }

    pub(super) fn no_laps_filtered(&self, cx: &mut Context<Self>) -> AnyElement {
        EmptyState::new(IconName::Funnel, tr("No laps with these filters"))
            .body(tr(
                "Nobody has uploaded a lap with the assists and input you picked. Change them above or show every lap.",
            ))
            .action(
                Button::new("online-show-all")
                    .outline()
                    .tap_sm()
                    .label(tr("Show all laps"))
                    .on_click(cx.listener(|this, _, _, cx| this.online_set_category(Category::default(), cx))),
            )
            .into_any_element()
    }

    pub(super) fn target_item(&mut self, items: &mut Vec<Target>, t: Target) -> usize {
        items.push(t);
        items.len() - 1
    }
}
