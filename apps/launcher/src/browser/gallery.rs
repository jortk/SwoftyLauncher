use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::catalog::media::{Kind, Media};
use swofty_core::catalog::{self, Addon};
use swofty_i18n::{tr, trf, trn};

use super::*;
use crate::pages::label;
use crate::scale::px;
use crate::shell::Shell;
use crate::theme::Palette;

const MEDIA_DIR: &str = "media";
/// The most bytes of gallery previews kept on disk.
const MEDIA_CACHE_BYTES: u64 = 256 * 1024 * 1024;
const THUMB_W: f32 = 112.;

fn media_dir(root: &Path) -> PathBuf {
    root.join(catalog::CACHE_DIR).join(MEDIA_DIR)
}

fn open_tip(m: &Media) -> String {
    match m.kind {
        Kind::Image => tr("Open the full picture in your browser").to_owned(),
        Kind::Video => tr("Play the video in your browser").to_owned(),
        _ => match m.host_name() {
            Some(host) => trf!("Watch on {site}", site = host),
            None => tr("Watch in your browser").to_owned(),
        },
    }
}

fn play_badge(size: f32) -> Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(px(size))
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .bg(black().opacity(0.6))
                .child(
                    Icon::new(IconName::Play)
                        .size(px(size * 0.46))
                        .text_color(white()),
                ),
        )
}

impl Shell {
    /// Loads the previews of `a`'s gallery into memory, from the disk cache or the registry.
    pub(super) fn load_media(&mut self, a: &Addon, cx: &mut Context<Self>) {
        let registry = catalog::base_url();
        let urls: Vec<String> = a
            .media
            .iter()
            .filter_map(|m| m.preview(&registry))
            .filter(|url| !self.browser.media.contains_key(*url))
            .map(str::to_owned)
            .collect();
        if urls.is_empty() {
            return;
        }
        let dir = media_dir(&self.store.root);
        cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move {
                    let loaded = images(&dir, &urls, &BTreeSet::new());
                    swofty_core::cache::prune_files(&dir, MEDIA_CACHE_BYTES);
                    loaded
                })
                .await;
            this.update(cx, |s, cx| {
                s.browser.media.extend(loaded);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The addon's pictures, videos and video links whose previews loaded.
    pub(super) fn browser_gallery(&self, a: &Addon, p: Palette, cx: &mut Context<Self>) -> Option<Div> {
        let registry = catalog::base_url();
        let shown: Vec<(&Media, Arc<Image>)> = a
            .media
            .iter()
            .filter_map(|m| Some((m, self.browser.media.get(m.preview(&registry)?)?.clone())))
            .collect();
        if shown.is_empty() {
            return None;
        }
        let pick = self
            .browser
            .media_pick
            .get(&a.id)
            .copied()
            .unwrap_or(0)
            .min(shown.len() - 1);
        let (current, picture) = &shown[pick];
        let tip = open_tip(current);
        let viewer = div()
            .id("browser-media-viewer")
            .relative()
            .w_full()
            .aspect_ratio(16. / 9.)
            .rounded(px(8.))
            .overflow_hidden()
            .bg(black())
            .child(img(picture.clone()).size_full().object_fit(ObjectFit::Contain))
            .when(current.playable(), |d| d.child(play_badge(64.)))
            .when_some(current.opens(&registry).map(str::to_owned), |d, url| {
                d.cursor_pointer()
                    .on_click(move |_, _, cx| cx.open_url(&url))
                    .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
            });
        let kind = match current.kind {
            Kind::Image => tr("Picture").to_owned(),
            Kind::Video => tr("Video").to_owned(),
            _ => current
                .host_name()
                .map_or_else(|| tr("Video link").to_owned(), str::to_owned),
        };
        let about = h_flex()
            .gap(px(12.))
            .items_start()
            .text_size(px(13.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(p.text.opacity(0.84))
                    .child(current.caption.clone()),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap(px(4.))
                    .items_center()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .when(current.opens(&registry).is_some(), |d| {
                        d.child(Icon::new(IconName::ExternalLink).size(px(12.)))
                    })
                    .child(kind),
            );
        let strip = (shown.len() > 1).then(|| {
            h_flex()
                .flex_wrap()
                .gap(px(8.))
                .children(shown.iter().enumerate().map(|(i, (m, picture))| {
                    let id = a.id.clone();
                    div()
                        .id(SharedString::from(format!("browser-media-{i}")))
                        .relative()
                        .w(px(THUMB_W))
                        .aspect_ratio(16. / 9.)
                        .rounded(px(6.))
                        .overflow_hidden()
                        .border_2()
                        .border_color(if i == pick { p.accent } else { p.border })
                        .bg(black())
                        .cursor_pointer()
                        .hover(move |s| s.border_color(p.accent.opacity(0.7)))
                        .child(img(picture.clone()).size_full().object_fit(ObjectFit::Cover))
                        .when(m.playable(), |d| d.child(play_badge(26.)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.browser.media_pick.insert(id.clone(), i);
                            cx.notify();
                        }))
                }))
        });
        Some(
            v_flex()
                .p(px(16.))
                .gap(px(10.))
                .rounded(px(10.))
                .border_1()
                .border_color(p.border)
                .bg(p.surface)
                .child(
                    h_flex()
                        .items_center()
                        .child(div().flex_1().child(label(tr("MEDIA"), p)))
                        .child(div().text_size(px(12.)).text_color(p.text_placeholder).child(trn(
                            shown.len(),
                            "{count} item",
                            "{count} items",
                        ))),
                )
                .child(viewer)
                .child(about)
                .children(strip),
        )
    }
}
