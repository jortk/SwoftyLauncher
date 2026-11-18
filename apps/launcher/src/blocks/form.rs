use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::notices::{Notification, Notify};
use crate::scale::{Hit, px};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState, NumberInput};
use gpui_kit::component::select::{Select, SelectDelegate, SelectEvent, SelectState};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use serde_json::{Map, Value};
use swofty_core::view::{Field, Form, colour_hex, parse_colour};
use swofty_i18n::{tr, trf, trn};

use super::select::*;
use super::state::*;
use super::widgets::*;
use super::*;
use crate::shell::Shell;
use crate::theme::Palette;

#[derive(Clone, Copy)]
pub(super) struct FieldCtx<'a> {
    pub(super) at: At<'a>,
    pub(super) key: &'a str,
    pub(super) form: &'a Form,
    pub(super) dirty: bool,
    pub(super) busy: bool,
}

impl Shell {
    pub(super) fn sync_form(
        &mut self,
        addon: &str,
        section: &str,
        form: &Form,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = format!("{addon}/{section}/{}", form.id);
        let roots = self.image_roots(addon);
        if !self.blocks.forms.contains_key(&key) {
            self.blocks.forms.insert(
                key.clone(),
                FormSlot {
                    inputs: HashMap::new(),
                    sliders: HashMap::new(),
                    selects: HashMap::new(),
                    picks: HashMap::new(),
                    synced: HashMap::new(),
                    reset: Reset::No,
                    seen: true,
                    _subs: Vec::new(),
                },
            );
        }
        let force = match self.blocks.forms[&key].reset {
            Reset::Now => true,
            Reset::After(g) => generation > g,
            Reset::No => false,
        };
        let mut subs = Vec::new();
        for field in &form.fields {
            let id = field.id().to_owned();
            let value = field.value();
            let current = self.field_value(&key, field, cx);
            let slot = self.blocks.forms.get_mut(&key).expect("inserted above");
            let synced = slot.synced.get(&id).cloned();
            let edited = synced.as_ref().is_some_and(|s| current.as_ref() != Some(s));
            let apply = force || synced.is_none() || (!edited && synced.as_ref() != Some(&value));
            match field {
                Field::Number(_) | Field::Text(_) if !slot.inputs.contains_key(&id) => {
                    let text = field_text(field, &value);
                    let state = cx.new(|cx| {
                        let mut s = InputState::new(window, cx).default_value(text);
                        if let Field::Number(nf) = field {
                            s = s.step(nf.step());
                            if let Some(min) = nf.min {
                                s = s.min(min);
                            }
                            if let Some(max) = nf.max {
                                s = s.max(max);
                            }
                        }
                        if let Field::Text(tf) = field
                            && let Some(ph) = tf.placeholder.clone()
                        {
                            s = s.placeholder(ph);
                        }
                        s
                    });
                    let (field_id, addon, section, form_id) =
                        (id.clone(), addon.to_owned(), section.to_owned(), form.id.clone());
                    subs.push(cx.subscribe_in(
                        &state,
                        window,
                        move |this, _, ev: &InputEvent, window, cx| {
                            let commit = matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur);
                            if matches!(ev, InputEvent::Change) {
                                cx.notify();
                            }
                            if !commit {
                                return;
                            }
                            let Some(form) = this.find_form(&addon, &section, &form_id) else {
                                return;
                            };
                            if form.live {
                                this.submit_form(&addon, &section, &form, Some(&field_id), window, cx);
                            } else if matches!(ev, InputEvent::PressEnter { .. }) {
                                this.submit_form(&addon, &section, &form, None, window, cx);
                            }
                        },
                    ));
                    slot.inputs.insert(id.clone(), state);
                }
                Field::Number(_) | Field::Text(_) => {
                    if apply && let Some(state) = slot.inputs.get(&id).cloned() {
                        let text = field_text(field, &value);
                        state.update(cx, |s, cx| s.set_value(text, window, cx));
                    }
                }
                Field::Slider(nf) => {
                    let at = value.as_f64().unwrap_or(0.) as f32;
                    if let Some(state) = slot.sliders.get(&id).cloned() {
                        if apply {
                            state.update(cx, |s, cx| s.set_value(at, window, cx));
                        }
                    } else {
                        let state = cx.new(|_| {
                            SliderState::new()
                                .min(nf.min.unwrap_or(0.) as f32)
                                .max(nf.max.unwrap_or(100.) as f32)
                                .step(nf.step() as f32)
                                .default_value(at)
                        });
                        let (addon, section, form_id, field_id) =
                            (addon.to_owned(), section.to_owned(), form.id.clone(), id.clone());
                        subs.push(cx.subscribe_in(
                            &state,
                            window,
                            move |this, _, ev: &SliderEvent, window, cx| {
                                cx.notify();
                                if !matches!(ev, SliderEvent::Release(_)) {
                                    return;
                                }
                                if let Some(form) =
                                    this.find_form(&addon, &section, &form_id).filter(|f| f.live)
                                {
                                    this.submit_form(&addon, &section, &form, Some(&field_id), window, cx);
                                }
                            },
                        ));
                        slot.sliders.insert(id.clone(), state);
                    }
                }
                Field::Select(sf) => {
                    if force || slot.picks.get(&id) == Some(&value) {
                        slot.picks.remove(&id);
                    }
                    let current = slot.picks.get(&id).unwrap_or(&value);
                    let current = current.as_str().unwrap_or_default().to_owned();
                    if let Some(select) = slot.selects.get_mut(&id) {
                        let state = select.state.clone();
                        if select.options != sf.options {
                            select.options = sf.options.clone();
                            let items = opts(sf, &roots, &select.shown);
                            state.update(cx, |s, cx| s.set_items(items, window, cx));
                        }
                        if state.read(cx).selected_value() != Some(&current) {
                            state.update(cx, |s, cx| s.set_selected_value(&current, window, cx));
                        }
                    } else {
                        let shown = Rc::new(RefCell::new(shown_for(sf, &roots, &current)));
                        let items = opts(sf, &roots, &shown);
                        let at = items.position(&current);
                        let state = cx.new(|cx| {
                            let mut s = SelectState::new(items, at, window, cx).searchable(sf.search);
                            s.set_selected_index(at, window, cx);
                            s
                        });
                        let (key, field_id, addon, section, form_id) = (
                            key.clone(),
                            id.clone(),
                            addon.to_owned(),
                            section.to_owned(),
                            form.id.clone(),
                        );
                        subs.push(cx.subscribe_in(
                            &state,
                            window,
                            move |this, state, ev: &SelectEvent<Opts>, window, cx| {
                                let SelectEvent::Confirm(Some(v)) = ev else {
                                    return;
                                };
                                state.update(cx, |s, cx| s.set_selected_value(v, window, cx));
                                let live = this
                                    .find_form(&addon, &section, &form_id)
                                    .filter(|f| f.live)
                                    .map(|f| (addon.clone(), section.clone(), f));
                                this.pick(&key, &field_id, Value::String(v.clone()), live, window, cx);
                            },
                        ));
                        slot.selects.insert(
                            id.clone(),
                            SelectSlot {
                                state,
                                options: sf.options.clone(),
                                shown,
                            },
                        );
                    }
                }
                Field::Toggle(_) | Field::Colour(_) => {
                    if force || slot.picks.get(&id) == Some(&value) {
                        slot.picks.remove(&id);
                    }
                }
            }
            let slot = self.blocks.forms.get_mut(&key).expect("inserted above");
            if apply {
                slot.synced.insert(id, value);
            }
        }
        let slot = self.blocks.forms.get_mut(&key).expect("inserted above");
        if force {
            slot.reset = Reset::No;
        }
        slot.seen = true;
        slot._subs.extend(subs);
    }

    pub(super) fn field_value(&self, key: &str, field: &Field, cx: &App) -> Option<Value> {
        let slot = self.blocks.forms.get(key)?;
        let id = field.id();
        match field {
            Field::Number(nf) => {
                let text = slot.inputs.get(id)?.read(cx).value().to_string();
                if text.trim().is_empty() {
                    return nf.value.is_none().then_some(Value::Null);
                }
                number_text(&text).map(|v| nf.json(nf.clamp(v)))
            }
            Field::Slider(nf) => {
                let v = slot.sliders.get(id)?.read(cx).value().start() as f64;
                Some(nf.json(nf.clamp(v)))
            }
            Field::Text(tf) => {
                let mut text = slot.inputs.get(id)?.read(cx).value().to_string();
                if let Some(max) = tf.max_len {
                    text = text.chars().take(max).collect();
                }
                Some(Value::String(text))
            }
            Field::Select(_) | Field::Toggle(_) | Field::Colour(_) => {
                Some(slot.picks.get(id).cloned().unwrap_or_else(|| field.value()))
            }
        }
    }

    pub(super) fn form_changes(&self, key: &str, form: &Form, cx: &App) -> (Vec<String>, bool) {
        let mut changed = Vec::new();
        let mut valid = true;
        for f in &form.fields {
            match self.field_value(key, f, cx) {
                Some(v) if v != f.value() => changed.push(f.id().to_owned()),
                Some(_) => {}
                None if self
                    .blocks
                    .forms
                    .get(key)
                    .is_some_and(|s| !s.inputs.contains_key(f.id()) && !s.sliders.contains_key(f.id())) => {}
                None => {
                    valid = false;
                    changed.push(f.id().to_owned());
                }
            }
        }
        (changed, valid)
    }

    pub(crate) fn submit_form(
        &mut self,
        addon: &str,
        section: &str,
        form: &Form,
        only: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = format!("{addon}/{section}/{}", form.id);
        if self.busy.contains(&key) || form.disabled {
            return;
        }
        let mut values = Map::new();
        let mut changed = Vec::new();
        for f in form.fields.iter().filter(|f| only.is_none_or(|o| o == f.id())) {
            let Some(v) = self.field_value(&key, f, cx) else {
                let name = f.label().unwrap_or(f.id()).to_owned();
                window.notify(
                    Notification::error(trf!("{name} is not a number", name = name)),
                    cx,
                );
                return;
            };
            if v != f.value() {
                changed.push(f.id().to_owned());
            }
            values.insert(f.id().to_owned(), v);
        }
        if changed.is_empty() {
            return;
        }
        let event = swofty_core::view::Event::Submit {
            form: form.id.clone(),
            row: None,
            values,
            changed,
        };
        self.send(addon, section, key, event, window, cx);
    }

    pub(super) fn reset_form(&mut self, key: &str, cx: &mut Context<Self>) {
        if let Some(slot) = self.blocks.forms.get_mut(key) {
            slot.reset = Reset::Now;
        }
        cx.notify();
    }

    pub(crate) fn settled(&mut self, control: &str, effect: &Result<swofty_core::view::Effect, String>) {
        use swofty_core::view::Effect;
        self.blocks.pending_cells.remove(control);
        if let Some(slot) = self.blocks.forms.get_mut(control) {
            slot.reset = match effect {
                Ok(Effect::Reload | Effect::Notice(_)) => Reset::After(self.view_gen),
                Ok(_) => Reset::Now,
                Err(_) => Reset::No,
            };
        }
    }

    pub(super) fn pick(
        &mut self,
        key: &str,
        id: &str,
        value: Value,
        live: Option<(String, String, Form)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(slot) = self.blocks.forms.get_mut(key) {
            slot.picks.insert(id.to_owned(), value);
        }
        if let Some((addon, section, form)) = live {
            self.submit_form(&addon, &section, &form, Some(id), window, cx);
        }
        cx.notify();
    }

    pub(super) fn form(&self, at: At, f: &Form, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let key = at.key(&f.id);
        let busy = self.busy.contains(&key);
        let cols = f.columns.unwrap_or(1).clamp(1, 3);
        let cols = cols.min(((at.width + GAP) / (220. + GAP)).floor().max(1.) as usize);
        let (changed, valid) = self.form_changes(&key, f, cx);
        let fields: Vec<AnyElement> = f
            .fields
            .iter()
            .map(|field| {
                let dirty = changed.iter().any(|c| c == field.id());
                let ctx = FieldCtx {
                    at,
                    key: &key,
                    form: f,
                    dirty,
                    busy,
                };
                self.field(ctx, field, p, cx)
            })
            .collect();
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut fields = fields.into_iter().peekable();
        while fields.peek().is_some() {
            let row: Vec<AnyElement> = fields.by_ref().take(cols).collect();
            let pad = cols - row.len();
            rows.push(
                h_flex()
                    .items_start()
                    .gap(px(20.))
                    .children(row)
                    .children((0..pad).map(|_| div().flex_1().flex_basis(px(0.))))
                    .into_any_element(),
            );
        }
        let footer = (!f.live).then(|| {
            let (reset_key, apply_form) = (key.clone(), f.clone());
            let (addon, section) = (at.addon.to_owned(), at.section.to_owned());
            let count = changed.len();
            h_flex()
                .gap(px(8.))
                .pt(px(4.))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.))
                        .text_color(p.text_muted)
                        .child(match count {
                            0 => String::new(),
                            n => trn(n, "{count} unsaved change", "{count} unsaved changes"),
                        }),
                )
                .when(count > 0 && !busy, |d| {
                    d.child(
                        Button::new(SharedString::from(format!("reset-{key}")))
                            .ghost()
                            .tap_sm()
                            .label(tr("Reset"))
                            .on_click(cx.listener(move |this, _, _, cx| this.reset_form(&reset_key, cx))),
                    )
                })
                .child(
                    Button::new(SharedString::from(format!("apply-{key}")))
                        .primary()
                        .tap_sm()
                        .label(f.apply.clone().unwrap_or_else(|| tr("Apply").into()))
                        .loading(busy)
                        .disabled(busy || f.disabled || count == 0 || !valid)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.submit_form(&addon, &section, &apply_form, None, window, cx)
                        })),
                )
        });
        v_flex()
            .gap(px(16.))
            .when(cols == 1, |d| d.max_w(px(520.)))
            .children(rows)
            .children(footer)
            .into_any_element()
    }

    pub(super) fn field(
        &self,
        ctx: FieldCtx,
        field: &Field,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let FieldCtx {
            at,
            key,
            form,
            dirty,
            busy,
        } = ctx;
        let id = field.id().to_owned();
        let disabled = form.disabled || field.disabled() || busy;
        let slot = self.blocks.forms.get(key);
        let control_id = SharedString::from(format!("field-{key}/{id}"));
        let live = form
            .live
            .then(|| (at.addon.to_owned(), at.section.to_owned(), form.clone()));
        let control: AnyElement = match field {
            Field::Number(nf) => match slot.and_then(|s| s.inputs.get(&id)) {
                Some(state) => NumberInput::new(state)
                    .tap_sm()
                    .disabled(disabled)
                    .when_some(nf.unit.clone(), |n, u| {
                        n.suffix(div().text_size(px(12.)).text_color(p.text_placeholder).child(u))
                    })
                    .into_any_element(),
                None => div().into_any_element(),
            },
            Field::Text(_) => match slot.and_then(|s| s.inputs.get(&id)) {
                Some(state) => Input::new(state).tap_sm().disabled(disabled).into_any_element(),
                None => div().into_any_element(),
            },
            Field::Slider(nf) => match slot.and_then(|s| s.sliders.get(&id)) {
                Some(state) => {
                    let v = state.read(cx).value().start() as f64;
                    let text = format!(
                        "{}{}",
                        nf.text(nf.clamp(v)),
                        nf.unit.as_ref().map(|u| format!(" {u}")).unwrap_or_default()
                    );
                    h_flex()
                        .h(px(28.))
                        .gap(px(12.))
                        .child(Slider::new(state).disabled(disabled).flex_1())
                        .child(
                            div()
                                .min_w(px(64.))
                                .flex()
                                .justify_end()
                                .text_size(px(13.))
                                .text_color(p.text)
                                .whitespace_nowrap()
                                .child(text),
                        )
                        .into_any_element()
                }
                None => div().into_any_element(),
            },
            Field::Select(sf) => match slot.and_then(|s| s.selects.get(&id)) {
                Some(select) => {
                    let current = slot
                        .and_then(|s| s.picks.get(&id))
                        .cloned()
                        .unwrap_or_else(|| field.value());
                    let committed = shown_for(
                        sf,
                        &self.image_roots(at.addon),
                        current.as_str().unwrap_or_default(),
                    );
                    let label = committed.label.clone();
                    select.shown.replace(committed);
                    Select::new(&select.state)
                        .id(control_id)
                        .tap_sm()
                        .w_full()
                        .menu_max_h(px(SELECT_MENU_H))
                        .search_placeholder(tr("Search"))
                        .placeholder(label)
                        .disabled(disabled)
                        .into_any_element()
                }
                None => div().into_any_element(),
            },
            Field::Colour(cf) => {
                let colour_key = format!("{key}/{id}");
                let rgb = self.colour_shown(&colour_key, cx).or_else(|| {
                    slot.and_then(|s| s.picks.get(&id))
                        .cloned()
                        .unwrap_or_else(|| field.value())
                        .as_str()
                        .and_then(parse_colour)
                });
                let open = self.blocks.colour.as_ref().is_some_and(|c| c.key == colour_key);
                let overlay = self.colour_overlay(&colour_key, cx);
                let target = ColourTarget::Form {
                    key: key.to_owned(),
                    field: id.clone(),
                    addon: at.addon.to_owned(),
                    section: at.section.to_owned(),
                    form: form.id.clone(),
                };
                let start = rgb.unwrap_or([0xFF, 0xFF, 0xFF]);
                h_flex()
                    .id(control_id)
                    .relative()
                    .h(px(24.))
                    .px(px(9.))
                    .gap(px(8.))
                    .rounded(px(5.))
                    .border_1()
                    .border_color(if open { p.accent } else { p.border })
                    .bg(p.background)
                    .child(swatch(rgb, 12., p))
                    .when(cf.hex, |d| {
                        d.child(
                            div()
                                .text_size(px(13.))
                                .text_color(if rgb.is_some() { p.text } else { p.text_placeholder })
                                .child(rgb.map_or_else(|| tr("None").to_owned(), colour_hex)),
                        )
                    })
                    .map(|d| match disabled {
                        true => d.opacity(0.5),
                        false => d
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_colour(colour_key.clone(), target.clone(), start, window, cx)
                            })),
                    })
                    .children(overlay)
                    .into_any_element()
            }
            Field::Toggle(_) => {
                let on = slot
                    .and_then(|s| s.picks.get(&id))
                    .cloned()
                    .unwrap_or_else(|| field.value())
                    == Value::Bool(true);
                let (key, id) = (key.to_owned(), id.clone());
                h_flex()
                    .h(px(28.))
                    .gap(px(10.))
                    .child(
                        Switch::new(control_id)
                            .checked(on)
                            .tap_sm()
                            .disabled(disabled)
                            .on_change(cx.listener(move |this, on: &bool, window, cx| {
                                this.pick(&key, &id, Value::Bool(*on), live.clone(), window, cx)
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(if on { p.text } else { p.text_muted })
                            .child(if on { tr("On") } else { tr("Off") }),
                    )
                    .into_any_element()
            }
        };
        v_flex()
            .flex_1()
            .flex_basis(px(0.))
            .min_w_0()
            .gap(px(6.))
            .child(
                h_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.text_muted)
                            .child(field.label().unwrap_or(field.id()).to_owned()),
                    )
                    .when(dirty, |d| d.child(div().size(px(6.)).rounded_full().bg(p.accent))),
            )
            .child(control)
            .when_some(field.help().map(str::to_owned), |d, h| {
                d.child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(17.))
                        .text_color(p.text_placeholder)
                        .child(h),
                )
            })
            .into_any_element()
    }
}
