use crate::scale::px;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::view::{Fit, Image as ImageBlock, Meter, Tile};

use super::widgets::*;
use super::*;
use crate::sections::{icon, tone};
use crate::shell::Shell;
use crate::theme::Palette;

impl Shell {
    pub(super) fn tiles(&self, at: At, tiles: &[Tile], p: Palette) -> AnyElement {
        let cols = tile_columns(tiles.len(), at.width);
        let tile = |t: &Tile| {
            let value_color = tone(t.tone, p);
            v_flex()
                .flex_1()
                .flex_basis(px(0.))
                .min_w_0()
                .px(px(16.))
                .pt(px(14.))
                .pb(px(if t.progress.is_some() { 12. } else { 14. }))
                .gap(px(6.))
                .rounded(px(8.))
                .border_1()
                .border_color(p.border_variant)
                .bg(p.surface)
                .child(
                    h_flex()
                        .gap(px(8.))
                        .child(eyebrow(&t.label, p).flex_1().min_w_0().truncate())
                        .when_some(t.icon, |d, i| {
                            d.child(Icon::new(icon(i)).size(px(14.)).text_color(p.text_placeholder))
                        }),
                )
                .child(
                    h_flex()
                        .gap(px(8.))
                        .items_baseline()
                        .child(
                            div()
                                .text_size(px(24.))
                                .line_height(px(30.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(value_color)
                                .truncate()
                                .child(t.value.clone()),
                        )
                        .when_some(t.delta.clone(), |d, delta| {
                            d.child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(tone(t.delta_tone(), p))
                                    .whitespace_nowrap()
                                    .child(delta),
                            )
                        }),
                )
                .when_some(t.hint.clone(), |d, h| {
                    d.child(
                        div()
                            .text_size(px(12.))
                            .text_color(p.text_muted)
                            .truncate()
                            .child(h),
                    )
                })
                .when_some(t.progress, |d, f| {
                    let color = fill_tone(Some(t.tone), p);
                    d.child(div().pt(px(4.)).child(bar(f, color, 3., p)))
                })
                .into_any_element()
        };
        v_flex()
            .gap(px(12.))
            .children(tiles.chunks(cols).map(|row| {
                h_flex()
                    .items_stretch()
                    .gap(px(12.))
                    .children(row.iter().map(tile))
                    .children((row.len()..cols).map(|_| div().flex_1().flex_basis(px(0.))))
            }))
            .into_any_element()
    }

    pub(super) fn image(&self, at: At, i: &ImageBlock, p: Palette) -> AnyElement {
        let width = i.width.clamp(0., at.width.max(0.));
        let height = i.height.max(0.);
        let border = if i.transparent { 0. } else { 2. };
        let inner = ((width - border).max(1.), (height - border).max(1.));
        let fit = match i.fit {
            Fit::Cover => ObjectFit::Cover,
            Fit::Contain => ObjectFit::Contain,
        };
        v_flex()
            .w(px(width))
            .gap(px(6.))
            .child(
                div()
                    .w(px(width))
                    .h(px(height))
                    .rounded(px(6.))
                    .overflow_hidden()
                    .when(!i.transparent, |d| {
                        d.bg(p.surface).border_1().border_color(p.border_variant)
                    })
                    .when_some(self.image_roots(at.addon).allow(&i.path), |d, path| {
                        d.child(
                            img(path)
                                .w(px(inner.0))
                                .h(px(inner.1))
                                .aspect_ratio(inner.0 / inner.1)
                                .object_fit(fit),
                        )
                    }),
            )
            .when_some(i.caption.clone(), |d, c| {
                d.child(div().text_size(px(12.)).text_color(p.text_muted).child(c))
            })
            .into_any_element()
    }

    pub(super) fn meters(&self, meters: &[Meter], p: Palette) -> AnyElement {
        v_flex()
            .gap(px(14.))
            .children(meters.iter().map(|m| {
                v_flex()
                    .gap(px(7.))
                    .child(
                        h_flex()
                            .gap(px(12.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(13.))
                                    .text_color(p.text)
                                    .child(m.label.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(p.text_muted)
                                    .whitespace_nowrap()
                                    .child(m.text()),
                            ),
                    )
                    .child(bar(m.fraction(), fill_tone(m.tone, p), 6., p))
            }))
            .into_any_element()
    }
}
