use super::drag::{Dragged, NESTED};
use crate::library::{AddonItem, Source, Type};
use crate::pages::{addon_icon, badge, logo, track_badge};
use crate::scale::{Hit, px};
use crate::shell::Shell;
use crate::theme::Palette;
use crate::ui::facet::checkbox;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::manifest::Category;
use swofty_core::profile::VANILLA;
use swofty_i18n::{tr, trf, trn};

/// The experimental features an addon turns on, as "Overlay".
pub(super) fn experimental_labels(m: &AddonItem) -> String {
    m.manifest
        .experimental_features
        .iter()
        .map(|f| f.label())
        .collect::<Vec<_>>()
        .join(", ")
}

/// A tag for what an addon does in game, as "Draws over the game".
pub(super) fn effect_tag(label: &'static str, p: Palette) -> Div {
    div()
        .h(px(16.))
        .px(px(5.))
        .flex()
        .items_center()
        .flex_shrink_0()
        .rounded(px(3.))
        .border_1()
        .border_color(p.border)
        .text_size(px(10.5))
        .text_color(p.text_muted)
        .child(label)
}

/// The icon an addon without its own shows: a race track for tracks, a palette for shader
/// packs, else a box.
pub(super) fn fallback_icon(m: &AddonItem, size: f32, p: Palette) -> Div {
    match m.category() {
        Some(Category::Tracks) => track_badge(size, p),
        Some(Category::Shaders) => badge(IconName::Palette, size, p),
        _ => badge(IconName::Package, size, p),
    }
}

pub(super) fn stock_removals_text(n: usize) -> String {
    trf!(
        "{amount} of stock database records skipped. The game crashes when a record it reads is missing.",
        amount = trn(n, "{count} removal", "{count} removals")
    )
}

pub(super) fn pill(icon: IconName, text: impl Into<SharedString>, color: Hsla) -> Div {
    h_flex()
        .h(px(20.))
        .px(px(6.))
        .gap(px(4.))
        .flex_shrink_0()
        .rounded(px(4.))
        .bg(color.opacity(0.14))
        .text_color(color)
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .child(Icon::new(icon).size(px(11.)))
        .child(text.into())
}

impl Shell {
    pub(super) fn list_row(
        &self,
        m: &AddonItem,
        position: Option<usize>,
        nested: bool,
        fade: Option<f32>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let man = &m.manifest;
        let id = man.id.clone();
        let selected = self.open_addon.as_ref() == Some(&id);
        let draggable = self.movable(m);
        let shader_only = man.is_shader_only();
        let off = !self.vanilla() && !self.addon_on(m) && !shader_only;
        let trailing = self.row_trailing(m, p, cx);
        let icon = match &m.icon {
            _ if man.is_builtin() => logo(28.),
            Some(icon) => addon_icon(icon, 28., p),
            None => fallback_icon(m, 28., p),
        };
        let ticking = self.addons.ticking() && !self.vanilla();
        let ticked = self.addons.selection.contains(&id);
        let grip = div()
            .w(px(14.))
            .flex_shrink_0()
            .when(draggable && !ticking, |d| {
                d.child(
                    Icon::new(IconName::GripVertical)
                        .size(px(14.))
                        .text_color(p.text_placeholder),
                )
            })
            .when(ticking, |d| {
                let id = id.clone();
                d.child(
                    div()
                        .id(SharedString::from(format!("tick-{id}")))
                        .child(checkbox(ticked, p))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.addons.selection.toggle(&id);
                            cx.notify();
                        })),
                )
            });
        let row = h_flex()
            .id(SharedString::from(format!("addon-{id}")))
            .relative()
            .h(px(52.))
            .pl(px(if nested { 6. + NESTED } else { 6. }))
            .pr(px(12.))
            .gap(px(10.))
            .flex_shrink_0()
            .border_t_1()
            .border_color(p.border_variant)
            .cursor_pointer()
            .map(|d| match (fade, selected || ticked) {
                (Some(k), _) => d.bg(p.warning.opacity(0.06 + 0.3 * k)),
                (None, true) => d.bg(p.selected),
                (None, false) => d.hover(move |s| s.bg(p.hover)),
            })
            .when_some(fade, |d, k| d.child(accent_bar(p.warning.opacity(k))))
            .when(selected && fade.is_none(), |d| d.child(accent_bar(p.accent)))
            .child(grip)
            .child(
                div()
                    .w(px(14.))
                    .flex_shrink_0()
                    .text_size(px(11.))
                    .text_color(p.text_placeholder)
                    .children(position.map(|n| n.to_string())),
            )
            .child(div().flex_shrink_0().when(off, |d| d.opacity(0.55)).child(icon))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(row_title(m, off, self.status_icon(&id, p), p))
                    .child(self.row_tags(m, p)),
            )
            .child(self.row_pills(m, p))
            .child(
                div()
                    .w(px(84.))
                    .flex()
                    .justify_end()
                    .flex_shrink_0()
                    .child(trailing),
            )
            .on_click(cx.listener({
                let id = id.clone();
                move |this, ev: &ClickEvent, _, cx| {
                    if !this.vanilla() && ev.modifiers().secondary(cx) {
                        this.addons.selection.toggle(&id);
                        cx.notify();
                        return;
                    }
                    this.open_addon = (this.open_addon.as_ref() != Some(&id)).then(|| id.clone());
                    cx.notify();
                }
            }));
        let row = match draggable {
            true => {
                let dragged = Dragged {
                    item: self.drag_item(&id),
                    name: man.name.clone().into(),
                };
                row.on_drag(dragged, |d, _, _, cx| cx.new(|_| d.clone()))
            }
            false => row,
        };
        row.into_any_element()
    }
}

impl Shell {
    fn row_trailing(&self, m: &AddonItem, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let man = &m.manifest;
        let id = man.id.clone();
        let has_configs = self.shaders.read(cx).has_configs(&id);
        let shader_only = man.is_shader_only();
        if shader_only {
            let open = id.clone();
            h_flex()
                .gap(px(10.))
                .items_center()
                .child(crate::pages::tag(tr("Shader config"), p))
                .child(
                    Button::new(SharedString::from(format!("open-shaders-{id}")))
                        .ghost()
                        .tap_sm()
                        .icon(IconName::Palette)
                        .label(tr("Open in Shaders"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.open_shaders_for(&open, cx)
                        })),
                )
                .into_any_element()
        } else if m.locked {
            h_flex()
                .gap(px(5.))
                .text_size(px(12.))
                .text_color(p.text_placeholder)
                .whitespace_nowrap()
                .child(Icon::new(IconName::Lock).size(px(12.)))
                .child(if self.vanilla() {
                    trf!("Off in {vanilla}", vanilla = VANILLA)
                } else {
                    tr("Always on").to_owned()
                })
                .into_any_element()
        } else {
            let id = id.clone();
            let switch = self.toggle(
                format!("toggle-{id}"),
                m.enabled && !self.vanilla(),
                cx,
                move |this, on, window, cx| this.set_addon_enabled(&id, on, window, cx),
            );
            if has_configs {
                v_flex()
                    .items_end()
                    .gap(px(2.))
                    .child(switch)
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.text_placeholder)
                            .whitespace_nowrap()
                            .child(tr("Its shader config stays in Shaders while it's off")),
                    )
                    .into_any_element()
            } else {
                switch
            }
        }
    }

    fn row_tags(&self, m: &AddonItem, p: Palette) -> Div {
        let man = &m.manifest;
        let id = &man.id;
        let category = match m.category() {
            Some(c) => Some(c.label()),
            None if m.source == Source::Builtin => None,
            None => Some(tr("Uncategorised")),
        };
        let types = Type::list(m);
        h_flex()
            .gap(px(6.))
            .text_size(px(12.))
            .children(category.map(|c| {
                div()
                    .flex_shrink_0()
                    .text_color(if m.category().is_some() {
                        p.text_muted
                    } else {
                        p.text_placeholder
                    })
                    .child(c)
            }))
            .when(!types.is_empty(), |d| {
                d.when(category.is_some(), |d| {
                    d.child(div().text_color(p.text_placeholder).child("·"))
                })
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(p.text_placeholder)
                        .child(types),
                )
            })
            .child(
                div()
                    .h(px(16.))
                    .px(px(5.))
                    .flex()
                    .items_center()
                    .flex_shrink_0()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(p.border)
                    .text_size(px(10.5))
                    .text_color(p.text_muted)
                    .child(m.source.label()),
            )
            .children(
                swofty_core::overlay_guard::effects(man)
                    .into_iter()
                    .map(|e| effect_tag(e, p)),
            )
            .when(!man.experimental_features.is_empty(), |d| {
                let features = experimental_labels(m);
                d.child(
                    div()
                        .id(SharedString::from(format!("experimental-{id}")))
                        .h(px(16.))
                        .px(px(5.))
                        .flex()
                        .items_center()
                        .flex_shrink_0()
                        .rounded(px(3.))
                        .border_1()
                        .border_color(p.warning.opacity(0.6))
                        .text_size(px(10.5))
                        .text_color(p.warning)
                        .child(tr("Uses experimental features"))
                        .tooltip(move |window, cx| {
                            Tooltip::new(trf!("Turns on {features} while it's on", features = features))
                                .build(window, cx)
                        }),
                )
            })
    }

    fn row_pills(&self, m: &AddonItem, p: Palette) -> Div {
        let id = m.manifest.id.clone();
        let pairs = self.live_pairs(m);
        let overlaps = pairs.iter().filter(|pr| pr.real_items() > 0).count();
        let kept = self.stock_removals(m);
        h_flex()
            .flex_shrink_0()
            .gap(px(4.))
            .justify_end()
            .when_some(
                self.addons.status.get(&id).filter(|l| !l.is_empty()),
                |d, lines| {
                    let first = lines[0].clone();
                    let tip = trf!("As of your last session: {lines}", lines = lines.join("; "));
                    d.child(
                        pill(IconName::TriangleAlert, first, p.warning)
                            .id(SharedString::from(format!("status-{id}")))
                            .max_w(px(240.))
                            .truncate()
                            .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx)),
                    )
                },
            )
            .when(self.update_ready(m), |d| {
                let to = self.catalog_update(&id).map(|u| format!("v{}", u.to.version));
                let skipped = self.update_skipped(&id);
                d.child(
                    pill(IconName::CircleArrowUp, to.clone().unwrap_or_default(), p.accent)
                        .id(SharedString::from(format!("update-{id}")))
                        .tooltip(move |window, cx| {
                            Tooltip::new(match &to {
                                Some(v) if skipped => trf!(
                                    "{v} is available. You skipped it in the play bar; update it from its Addon Browser page.",
                                    v = v
                                ),
                                Some(v) => {
                                    trf!("{v} is available. Update addons from the play bar.", v = v)
                                }
                                None => tr("Update available").to_owned(),
                            })
                            .build(window, cx)
                        }),
                )
            })
            .when_some(self.kept_separate(m), |d, why| {
                d.child(
                    pill(IconName::TriangleAlert, tr("Kept separate"), p.warning)
                        .id(SharedString::from(format!("separate-{id}")))
                        .tooltip(move |window, cx| Tooltip::new(why.clone()).build(window, cx)),
                )
            })
            .when(!m.left_out.is_empty(), |d| {
                let why = m.left_out.join("\n");
                d.child(
                    pill(IconName::TriangleAlert, "Track left out", p.warning)
                        .id(SharedString::from(format!("left-out-{id}")))
                        .tooltip(move |window, cx| Tooltip::new(why.clone()).build(window, cx)),
                )
            })
            .when(kept > 0, |d| {
                d.child(
                    pill(
                        IconName::TriangleAlert,
                        trf!(
                            "{amount} skipped",
                            amount = trn(kept, "{count} removal", "{count} removals")
                        ),
                        p.warning,
                    )
                    .id(SharedString::from(format!("kept-{id}")))
                    .tooltip(move |window, cx| Tooltip::new(stock_removals_text(kept)).build(window, cx)),
                )
            })
            .when(overlaps > 0, |d| {
                d.child(
                    pill(
                        IconName::Layers,
                        trf!("Overlaps {count}", count = overlaps),
                        p.text_muted,
                    )
                    .id(SharedString::from(format!("overlaps-{id}")))
                    .tooltip(move |window, cx| {
                        Tooltip::new(trf!(
                            "{amount} also change some of this. The lower one in the list wins. Open the row for details.",
                            amount = trn(overlaps, "{count} addon", "{count} addons")
                        ))
                        .build(window, cx)
                    }),
                )
            })
    }
}

fn accent_bar(color: Hsla) -> Div {
    div().absolute().left_0().top_0().bottom_0().w(px(3.)).bg(color)
}

fn row_title(m: &AddonItem, off: bool, flag: Option<AnyElement>, p: Palette) -> Div {
    let man = &m.manifest;
    h_flex()
        .gap(px(8.))
        .items_baseline()
        .children(flag)
        .child(
            div()
                .min_w_0()
                .text_size(px(14.))
                .font_weight(FontWeight::MEDIUM)
                .when(off, |d| d.text_color(p.text_muted))
                .truncate()
                .child(man.name.clone()),
        )
        .child(
            div()
                .flex_shrink_0()
                .text_size(px(12.))
                .text_color(p.text_placeholder)
                .child(man.version.to_string()),
        )
}
