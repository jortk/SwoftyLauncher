use crate::scale::{Hit, px};
use crate::shell::{Shell, SliderSlot};
use crate::theme::Palette;
use crate::ui::{Toggle, label};
use gpui_kit::component::button::Button;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{Disableable, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::manifest::{AddonManifest, AddonOption, Binds, OptionType};
use swofty_core::settings::OnAddons;
use swofty_i18n::{fill, tr};

impl Shell {
    pub(crate) fn options_panel(
        &self,
        man: &AddonManifest,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let customised = !self.vanilla() && self.library.profile.options.contains_key(&man.id);
        let reset = customised.then(|| {
            let man = man.clone();
            div()
                .id(SharedString::from(format!("reset-{}", man.id)))
                .cursor_pointer()
                .text_size(px(12.))
                .text_color(p.text_muted)
                .hover(move |s| s.text_color(p.text))
                .child(tr("Reset to defaults"))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.update_profile(window, cx, |p| p.reset_options(&man).map(drop))
                }))
        });
        let on = self.library.on();
        let rows: Vec<AnyElement> = man
            .options
            .iter()
            .enumerate()
            .map(|(i, o)| self.setting_row(man, o, &on, i == 0, p, cx))
            .collect();
        v_flex()
            .gap(px(6.))
            .child(
                h_flex()
                    .max_w(px(760.))
                    .justify_between()
                    .child(label(tr("SETTINGS"), p))
                    .children(reset),
            )
            .child(
                v_flex()
                    .max_w(px(760.))
                    .rounded(px(5.))
                    .border_1()
                    .border_color(p.border_variant)
                    .children(rows),
            )
    }

    pub(super) fn setting_row(
        &self,
        man: &AddonManifest,
        o: &AddonOption,
        on: &OnAddons,
        first: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = format!("{}.{}", man.id, o.id);
        let pending = !self.vanilla() && self.library.rebuild.contains(&key);
        let needs = on.missing(o).map(|n| {
            let name = self
                .library
                .addons
                .iter()
                .find(|a| a.manifest.id == n.id)
                .map_or_else(|| n.id.clone(), |a| a.manifest.name.clone());
            let addon = match n.version.comparators.is_empty() {
                true => name,
                false => format!("{name} {}", n.version),
            };
            fill(tr("Off: needs {addon} on"), &[("addon", addon)])
        });
        let locked = needs.is_some();
        let (hint, hint_color) = match (needs, o.binds) {
            (Some(text), _) => (text, p.warning),
            (None, Binds::Native) => (tr("Applies live").to_owned(), p.text_placeholder),
            (None, Binds::Db) if pending => (tr("Rebuilds on next launch").to_owned(), p.warning),
            (None, Binds::Db) => (tr("Applies after a rebuild").to_owned(), p.text_placeholder),
        };
        let value = on.value(&self.library.profile, man, o);
        let control = match o.kind {
            OptionType::Toggle => {
                let (man, id) = (man.clone(), o.id.clone());
                Toggle::new(format!("setting-{key}"), value == true)
                    .disabled(self.vanilla() || locked)
                    .on_change(cx.listener(move |this, on: &bool, window, cx| {
                        this.update_profile(window, cx, |p| p.set_option(&man, &id, (*on).into()).map(drop))
                    }))
                    .into_any_element()
            }
            OptionType::Choice => self.choice(&key, man, o, &value, cx),
            OptionType::Slider => self.slider(&key, o, &value, p, cx),
        };
        h_flex()
            .px(px(12.))
            .py(px(10.))
            .flex_wrap()
            .gap_x(px(24.))
            .gap_y(px(8.))
            .when(!first, |d| d.border_t_1().border_color(p.border_variant))
            .child(
                v_flex()
                    .flex_1()
                    .flex_basis(px(220.))
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_x(px(10.))
                            .gap_y(px(2.))
                            .items_baseline()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(o.title().to_owned()),
                            )
                            .child(div().text_size(px(12.)).text_color(hint_color).child(hint)),
                    )
                    .when_some(o.description.clone(), |d, t| {
                        d.child(
                            div()
                                .text_size(px(12.))
                                .line_height(px(18.))
                                .text_color(p.text_muted)
                                .child(t),
                        )
                    }),
            )
            .child(div().flex_shrink_0().child(control))
            .into_any_element()
    }

    pub(super) fn choice(
        &self,
        key: &str,
        man: &AddonManifest,
        o: &AddonOption,
        value: &serde_json::Value,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let states = o.states();
        let current = o.index(value);
        let shell = cx.entity().downgrade();
        let (man, id) = (man.clone(), o.id.clone());
        Button::new(SharedString::from(format!("setting-{key}")))
            .outline()
            .tap_sm()
            .min_w(px(168.))
            .label(o.text(value))
            .dropdown_caret(true)
            .disabled(self.vanilla())
            .dropdown_menu_with_anchor(Anchor::TopRight, move |mut menu, _, _| {
                for (i, s) in states.iter().enumerate() {
                    let (shell, man, id, v) = (shell.clone(), man.clone(), id.clone(), s.value.clone());
                    menu = menu.item(
                        PopupMenuItem::new(s.text.clone())
                            .checked(current == Some(i))
                            .on_click(move |_, window, cx| {
                                let v = v.clone();
                                shell
                                    .update(cx, |this, cx| {
                                        this.update_profile(window, cx, |p| {
                                            p.set_option(&man, &id, v).map(drop)
                                        })
                                    })
                                    .ok();
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }

    pub(super) fn slider(
        &self,
        key: &str,
        o: &AddonOption,
        value: &serde_json::Value,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(slot) = self.sliders.get(key) else {
            return div().into_any_element();
        };
        let at = slot.state.read(cx).value().start().round().max(0.) as usize;
        let text = o
            .states()
            .get(at)
            .map_or_else(|| o.text(value), |s| s.text.clone());
        h_flex()
            .w(px(260.))
            .h(px(28.))
            .gap(px(12.))
            .child(Slider::new(&slot.state).disabled(self.vanilla()).flex_1())
            .child(
                div()
                    .w(px(48.))
                    .flex_shrink_0()
                    .flex()
                    .justify_end()
                    .text_size(px(13.))
                    .text_color(p.text)
                    .child(text),
            )
            .into_any_element()
    }

    pub(crate) fn sync_sliders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(m) = self
            .open_addon
            .as_ref()
            .and_then(|id| self.library.addons.iter().find(|m| &m.manifest.id == id))
        else {
            return;
        };
        let man = m.manifest.clone();
        for o in man.options.iter().filter(|o| o.kind == OptionType::Slider) {
            let key = format!("{}.{}", man.id, o.id);
            let at = o.index(&self.library.profile.option(&man, o)).unwrap_or(0);
            if let Some(slot) = self.sliders.get_mut(&key) {
                if slot.synced != at {
                    slot.synced = at;
                    slot.state.update(cx, |s, cx| s.set_value(at as f32, window, cx));
                }
                continue;
            }
            let last = o.states().len().saturating_sub(1) as f32;
            let state = cx.new(|_| {
                SliderState::new()
                    .min(0.)
                    .max(last)
                    .step(1.)
                    .default_value(at as f32)
            });
            let (man, id) = (man.clone(), o.id.clone());
            let changes = cx.subscribe_in(&state, window, move |this, _, ev: &SliderEvent, window, cx| {
                let SliderEvent::Release(v) = ev else {
                    return;
                };
                let at = v.start().round().max(0.) as usize;
                let Some(state) = man
                    .options
                    .iter()
                    .find(|o| o.id == id)
                    .and_then(|o| o.states().into_iter().nth(at))
                else {
                    return;
                };
                this.update_profile(window, cx, |p| p.set_option(&man, &id, state.value).map(drop));
            });
            self.sliders.insert(
                key,
                SliderSlot {
                    state,
                    synced: at,
                    _changes: changes,
                },
            );
        }
    }
}
