use std::collections::HashMap;
use std::time::Duration;

use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use swofty_core::shaders::ConfigRef;
use swofty_i18n::{tr, trf};
use swofty_shaders::rules::{Op, Rule, Signals};
use swofty_shaders::signals::{Signal, SignalKind, signal};

use super::ShadersView;
use crate::theme::Palette;
use crate::ui::Chip;

const FADE_TICK: Duration = Duration::from_millis(33);
const SLIDER_WIDTH: f32 = 120.;

#[derive(Clone, Copy)]
struct Used {
    sig: &'static Signal,
    magnitude: bool,
}

#[derive(Default)]
pub(super) struct Triggers {
    pub rules: Vec<Rule>,
    pub weights: Vec<f64>,
    config: Option<ConfigRef>,
    used: Vec<Used>,
    on: HashMap<&'static str, bool>,
    name: HashMap<&'static str, usize>,
    wheels: usize,
    num: HashMap<&'static str, f64>,
    mag: HashMap<&'static str, f64>,
    sliders: HashMap<String, Entity<SliderState>>,
    subs: Vec<Subscription>,
    pub busy: bool,
    tick: Option<Task<()>>,
}

fn used_by(rules: &[Rule]) -> Vec<Used> {
    let mut out: Vec<Used> = Vec::new();
    let mut add = |id: &str, magnitude: bool| {
        let Some((_, sig)) = signal(id) else { return };
        match out.iter_mut().find(|u| u.sig.id == sig.id) {
            Some(u) => u.magnitude |= magnitude,
            None => out.push(Used { sig, magnitude }),
        }
    };
    for r in rules {
        if let Some(ramp) = &r.ramp {
            add(&ramp.signal, false);
        }
        for c in &r.when {
            add(
                &c.signal,
                r.event && matches!(c.op, Op::Above | Op::Below | Op::Between),
            );
        }
    }
    out
}

fn clamp_to(sig: &Signal, x: f64) -> f64 {
    x.clamp(sig.min, sig.max)
}

fn smooth(w: f64) -> f64 {
    w * w * (3.0 - 2.0 * w)
}

impl Triggers {
    fn signals(&self) -> Signals {
        let mut s = Signals::new();
        for u in &self.used {
            let id = u.sig.id;
            match u.sig.kind {
                SignalKind::Bool => s.set_bool(id, self.on.get(id).copied().unwrap_or(false)),
                SignalKind::Number => {
                    s.set_number(id, self.num.get(id).copied().unwrap_or(clamp_to(u.sig, 0.0)))
                }
                SignalKind::Name => {
                    let Some(&i) = self.name.get(id) else { continue };
                    if u.sig.per_wheel() {
                        let value = u.sig.values[i];
                        let mut wheels = ["tarmac"; 4];
                        for w in wheels.iter_mut().take(self.wheels.clamp(1, 4)) {
                            *w = value;
                        }
                        s.set_surface(wheels);
                    } else {
                        s.set_name(id, u.sig.values[i]);
                    }
                }
                SignalKind::Event => {}
            }
        }
        s
    }

    fn magnitude(&self, u: &Used) -> f64 {
        let (lo, hi) = u.sig.magnitude.unwrap_or((0.0, 1.0));
        self.mag.get(u.sig.id).copied().unwrap_or((lo + hi) / 2.0)
    }
}

impl ShadersView {
    pub(super) fn trigger_rules(&mut self, rules: Vec<Rule>, cx: &mut Context<Self>) -> bool {
        let config = self.editor.loaded.clone();
        let same_config = self.preview.triggers.config == config;
        if same_config && self.preview.triggers.rules == rules {
            return false;
        }
        let t = &mut self.preview.triggers;
        let used = used_by(&rules);
        if !same_config {
            t.on.clear();
            t.name.clear();
            t.num.clear();
            t.mag.clear();
            t.wheels = 4;
            t.config = config;
        }
        if t.wheels == 0 {
            t.wheels = 4;
        }
        let changed = used.iter().map(|u| (u.sig.id, u.magnitude)).collect::<Vec<_>>()
            != t.used.iter().map(|u| (u.sig.id, u.magnitude)).collect::<Vec<_>>();
        t.rules = rules;
        t.weights = vec![0.0; t.rules.len()];
        t.used = used;
        if changed || !same_config {
            t.sliders.clear();
            t.subs.clear();
            let used = t.used.clone();
            for u in used {
                self.trigger_slider(u, cx);
            }
        }
        true
    }

    fn trigger_slider(&mut self, u: Used, cx: &mut Context<Self>) {
        let id = u.sig.id;
        if u.sig.kind == SignalKind::Number {
            let at = self
                .preview
                .triggers
                .num
                .get(id)
                .copied()
                .unwrap_or(clamp_to(u.sig, 0.0));
            let step = ((u.sig.max - u.sig.min) / 200.0).max(0.001);
            let state = cx.new(|_| {
                SliderState::new()
                    .min(u.sig.min as f32)
                    .max(u.sig.max as f32)
                    .step(step as f32)
                    .default_value(at as f32)
            });
            let sub = cx.subscribe(&state, move |this, _, ev: &SliderEvent, cx| {
                let (SliderEvent::Change(v) | SliderEvent::Release(v)) = ev;
                this.preview.triggers.num.insert(id, f64::from(v.start()));
                this.trigger_changed(cx);
            });
            self.preview.triggers.subs.push(sub);
            self.preview.triggers.sliders.insert(id.to_owned(), state);
        }
        if u.magnitude
            && let Some((lo, hi)) = u.sig.magnitude
        {
            let at = self.preview.triggers.magnitude(&u);
            let state = cx.new(|_| {
                SliderState::new()
                    .min(lo as f32)
                    .max(hi as f32)
                    .step(((hi - lo) / 100.0).max(0.001) as f32)
                    .default_value(at as f32)
            });
            let sub = cx.subscribe(&state, move |this, _, ev: &SliderEvent, cx| {
                let (SliderEvent::Change(v) | SliderEvent::Release(v)) = ev;
                this.preview.triggers.mag.insert(id, f64::from(v.start()));
                cx.notify();
            });
            self.preview.triggers.subs.push(sub);
            self.preview.triggers.sliders.insert(format!("{id}#mag"), state);
        }
    }

    pub(super) fn trigger_signals(&self) -> Signals {
        self.preview.triggers.signals()
    }

    pub(super) fn trigger_changed(&mut self, cx: &mut Context<Self>) {
        self.preview_send_triggers(cx);
        self.preview.triggers.busy = true;
        if !self.preview.playing {
            self.preview_render_settle(cx);
            self.fade_tick(cx);
        }
        cx.notify();
    }

    fn trigger_fire(&mut self, id: &'static str, cx: &mut Context<Self>) {
        let magnitude = self
            .preview
            .triggers
            .used
            .iter()
            .find(|u| u.sig.id == id)
            .map_or(0.0, |u| self.preview.triggers.magnitude(u));
        self.preview_fire(id, magnitude, cx);
        self.trigger_changed(cx);
    }

    pub(super) fn fade_tick(&mut self, cx: &mut Context<Self>) {
        if self.preview.triggers.tick.is_some() || self.preview.triggers.rules.is_empty() {
            return;
        }
        self.preview.triggers.tick = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(FADE_TICK).await;
                let again = this
                    .update(cx, |v, cx| {
                        let t = &v.preview.triggers;
                        if v.preview.playing || !t.busy {
                            return false;
                        }
                        v.preview_render_settle(cx);
                        true
                    })
                    .unwrap_or(false);
                if !again {
                    break;
                }
            }
            this.update(cx, |v, _| v.preview.triggers.tick = None).ok();
        }));
    }

    pub(super) fn triggers_strip(&mut self, p: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.preview.triggers.rules.is_empty() {
            return None;
        }
        let used = self.preview.triggers.used.clone();
        let mut controls = Vec::new();
        for u in &used {
            controls.push(self.trigger_control(u, p, cx));
        }
        let t = &self.preview.triggers;
        let rows: Vec<AnyElement> = t
            .rules
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let a = smooth(t.weights.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0));
                let name = if r.name.is_empty() {
                    trf!("rule {n}", n = i + 1)
                } else {
                    r.name.clone()
                };
                h_flex()
                    .w(px(230.))
                    .gap(px(8.))
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(if a > 0.0 { p.text } else { p.text_muted })
                            .child(name),
                    )
                    .child(
                        div()
                            .w(px(70.))
                            .h(px(4.))
                            .flex_shrink_0()
                            .rounded(px(2.))
                            .bg(p.elevated)
                            .overflow_hidden()
                            .child(div().h_full().w(relative(a as f32)).rounded(px(2.)).bg(p.accent)),
                    )
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .flex_shrink_0()
                .w_full()
                .gap(px(8.))
                .p(px(10.))
                .rounded(px(8.))
                .border_1()
                .border_color(p.border)
                .bg(p.surface)
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_x(px(14.))
                        .gap_y(px(6.))
                        .items_center()
                        .children(controls),
                )
                .child(h_flex().flex_wrap().gap_x(px(18.)).gap_y(px(4.)).children(rows))
                .into_any_element(),
        )
    }

    fn trigger_control(&mut self, u: &Used, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let id = u.sig.id;
        let label = div().text_size(px(11.)).text_color(p.text_muted).child(id);
        let weak = cx.entity().downgrade();
        let t = &self.preview.triggers;
        let control: AnyElement = match u.sig.kind {
            SignalKind::Bool => {
                let on = t.on.get(id).copied().unwrap_or(false);
                Chip::new(format!("trig-{id}"), if on { tr("yes") } else { tr("no") })
                    .on(on)
                    .filled(true)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let now = this.preview.triggers.on.get(id).copied().unwrap_or(false);
                        this.preview.triggers.on.insert(id, !now);
                        this.trigger_changed(cx);
                    }))
                    .into_any_element()
            }
            SignalKind::Name => {
                let current = t.name.get(id).map_or_else(|| tr("none"), |&i| u.sig.values[i]);
                let sig = u.sig;
                let picker = Button::new(SharedString::from(format!("trig-{id}")))
                    .outline()
                    .tap_sm()
                    .label(current)
                    .dropdown_caret(true)
                    .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                        let w = weak.clone();
                        menu = menu.item(PopupMenuItem::new(tr("none")).on_click(move |_, _, cx| {
                            w.update(cx, |v, cx| {
                                v.preview.triggers.name.remove(id);
                                v.trigger_changed(cx);
                            })
                            .ok();
                        }));
                        for (i, value) in sig.values.iter().enumerate() {
                            let w = weak.clone();
                            menu = menu.item(PopupMenuItem::new(*value).on_click(move |_, _, cx| {
                                w.update(cx, |v, cx| {
                                    v.preview.triggers.name.insert(id, i);
                                    v.trigger_changed(cx);
                                })
                                .ok();
                            }));
                        }
                        menu
                    });
                if u.sig.per_wheel() {
                    let n = t.wheels.clamp(1, 4);
                    let segments = (1..=4usize).map(|k| {
                        Chip::new(format!("trig-{id}-w{k}"), k.to_string())
                            .on(k == n)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.preview.triggers.wheels = k;
                                this.trigger_changed(cx);
                            }))
                    });
                    h_flex()
                        .gap(px(6.))
                        .items_center()
                        .child(picker)
                        .child(
                            h_flex()
                                .gap(px(3.))
                                .items_center()
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(p.text_muted)
                                        .child(tr("wheels")),
                                )
                                .children(segments),
                        )
                        .into_any_element()
                } else {
                    picker.into_any_element()
                }
            }
            SignalKind::Number => {
                let x = t.num.get(id).copied().unwrap_or(clamp_to(u.sig, 0.0));
                let text = if u.sig.unit.is_empty() {
                    format!("{x:.2}")
                } else {
                    format!("{x:.0} {}", u.sig.unit)
                };
                let slider = t
                    .sliders
                    .get(id)
                    .map(|s| div().w(px(SLIDER_WIDTH)).child(Slider::new(s)));
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .children(slider)
                    .child(
                        div()
                            .min_w(px(52.))
                            .text_size(px(11.))
                            .text_color(p.text)
                            .child(text),
                    )
                    .into_any_element()
            }
            SignalKind::Event => {
                let fire = Button::new(SharedString::from(format!("trig-{id}")))
                    .outline()
                    .tap_sm()
                    .icon(IconName::Zap)
                    .label(tr("Fire"))
                    .on_click(cx.listener(move |this, _, _, cx| this.trigger_fire(id, cx)));
                let slider = t
                    .sliders
                    .get(&format!("{id}#mag"))
                    .map(|s| div().w(px(SLIDER_WIDTH)).child(Slider::new(s)));
                let text = u.magnitude.then(|| {
                    let m = t.magnitude(u);
                    format!("{m:.1} {}", u.sig.unit)
                });
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(fire)
                    .children(slider)
                    .children(text.map(|text| {
                        div()
                            .min_w(px(44.))
                            .text_size(px(11.))
                            .text_color(p.text)
                            .child(text)
                    }))
                    .into_any_element()
            }
        };
        h_flex()
            .gap(px(8.))
            .items_center()
            .child(label)
            .child(control)
            .into_any_element()
    }
}
