use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Disableable, Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::catalog::{self, Trouble};
use swofty_i18n::{tr, trf, trn};

use super::*;
use crate::pages::GUTTER;
use crate::scale::{Hit, px};
use crate::shell::Shell;
use crate::theme::Palette;

fn activity_row(i: usize, e: &Activity, p: Palette) -> AnyElement {
    let color = if e.ok { p.text_muted } else { p.error };
    h_flex()
        .px(px(14.))
        .py(px(9.))
        .gap(px(10.))
        .items_start()
        .when(i > 0, |d| d.border_t_1().border_color(p.border_variant))
        .child(
            div().pt(px(2.)).child(
                Icon::new(if e.ok { e.op.icon() } else { IconName::CircleAlert })
                    .size(px(14.))
                    .text_color(color),
            ),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(
                    h_flex()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if e.ok { p.text } else { p.error })
                                .child(e.title.clone()),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(11.))
                                .text_color(p.text_placeholder)
                                .child(format!("{} · {}", since_time(e.at), took_text(e.took))),
                        ),
                )
                .when(!e.detail.is_empty(), |d| {
                    d.child(
                        div()
                            .text_size(px(12.))
                            .line_height(px(17.))
                            .text_color(p.text_muted)
                            .child(e.detail.clone()),
                    )
                }),
        )
        .into_any_element()
}

impl Shell {
    pub(super) fn stars_status(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let s = &self.browser.stars;
        let waiting = s.pending.len();
        let (icon, color, text) = if self.browser.syncing {
            (IconName::RefreshCw, p.text_muted, tr("Syncing stars…").to_owned())
        } else if s.error.is_some() {
            (
                IconName::CircleAlert,
                p.warning,
                match waiting {
                    0 => tr("Stars not synced").to_owned(),
                    n => trf!("Stars not synced · {count} waiting", count = n),
                },
            )
        } else if waiting > 0 {
            (
                IconName::Clock,
                p.text_muted,
                trn(
                    waiting,
                    "{count} star change waiting",
                    "{count} star changes waiting",
                ),
            )
        } else if let Some(at) = s.synced_at.as_deref().and_then(since) {
            (
                IconName::Star,
                p.text_muted,
                trf!("Stars synced {when}", when = at),
            )
        } else {
            (
                IconName::Star,
                p.text_muted,
                tr("Stars not synced yet").to_owned(),
            )
        };
        let tip = match (&s.error, &s.synced_at) {
            (Some(e), last) => trf!(
                "The last sync failed: {error}\nLast synced: {when}\nClick to try again.",
                error = e,
                when = last
                    .as_deref()
                    .and_then(since)
                    .unwrap_or_else(|| tr("never").to_owned())
            ),
            (None, _) => trf!(
                "{count} starred by you · client {client}…\nClick to sync now.",
                count = s.starred.len(),
                client = &s.client[..8.min(s.client.len())]
            ),
        };
        h_flex()
            .id("browser-stars-status")
            .h(px(28.))
            .px(px(8.))
            .gap(px(6.))
            .rounded(px(5.))
            .cursor_pointer()
            .hover(move |s| s.bg(p.hover))
            .text_size(px(12.))
            .text_color(color)
            .child(Icon::new(icon).size(px(13.)))
            .child(text)
            .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
            .on_click(cx.listener(|this, _, _, cx| this.sync_stars(cx)))
    }

    pub(super) fn trouble_banner(&self, p: Palette, cx: &mut Context<Self>) -> Option<Div> {
        let trouble = self.browser.trouble.as_ref()?;
        let what = match (trouble, self.browser.saved) {
            (Trouble::Offline(_), true) => tr("Couldn't reach the registry · showing the saved catalog"),
            (Trouble::Offline(_), false) => tr("Couldn't reach the registry · nothing saved to show"),
            (Trouble::Failed(_), true) => {
                tr("The registry answered with an error · showing the saved catalog")
            }
            (Trouble::Failed(_), false) => tr("The registry answered with an error · nothing saved to show"),
        };
        Some(
            h_flex()
                .mx(px(GUTTER))
                .mb(px(12.))
                .px(px(12.))
                .py(px(8.))
                .gap(px(10.))
                .flex_shrink_0()
                .items_center()
                .rounded(px(8.))
                .bg(p.warning.opacity(0.08))
                .border_1()
                .border_color(p.warning.opacity(0.4))
                .child(Icon::new(IconName::CloudOff).size(px(15.)).text_color(p.warning))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(what),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(p.text_muted)
                                .truncate()
                                .child(trouble.to_string()),
                        ),
                )
                .child(
                    Button::new("browser-retry")
                        .outline()
                        .tap_sm()
                        .icon(IconName::RefreshCw)
                        .label(tr("Retry"))
                        .loading(self.browser.fetching)
                        .disabled(self.browser.fetching)
                        .on_click(cx.listener(|this, _, _, cx| this.fetch_catalog(cx))),
                ),
        )
    }

    pub(super) fn activity_panel(&self, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let rows: Vec<AnyElement> = self
            .browser
            .activity
            .iter()
            .enumerate()
            .map(|(i, e)| activity_row(i, e, p))
            .collect();
        let empty = rows.is_empty().then(|| {
            div()
                .p(px(16.))
                .text_size(px(13.))
                .text_color(p.text_placeholder)
                .child(tr(
                    "Nothing yet. Catalog fetches, star syncs and installs show here.",
                ))
        });
        v_flex()
            .id("browser-activity-panel")
            .absolute()
            .top(px(64.))
            .right(px(GUTTER))
            .w(px(460.))
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.elevated)
            .shadow_lg()
            .occlude()
            .overflow_hidden()
            .child(
                h_flex()
                    .px(px(14.))
                    .h(px(44.))
                    .flex_shrink_0()
                    .gap(px(8.))
                    .items_center()
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        Icon::new(IconName::Activity)
                            .size(px(14.))
                            .text_color(p.text_muted),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("Activity")),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_placeholder)
                            .child(trf!(
                                "last {count} · {file}",
                                count = self.browser.activity.len(),
                                file = catalog::LOG_FILE
                            )),
                    )
                    .child(
                        Button::new("browser-activity-close")
                            .ghost()
                            .tap_xs()
                            .icon(IconName::X)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.browser.activity_open = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .id("browser-activity-rows")
                    .max_h(px(470.))
                    .overflow_y_scrollbar()
                    .children(rows)
                    .children(empty),
            )
    }
}
