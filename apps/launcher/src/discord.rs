use std::sync::{Arc, LazyLock};

use crate::scale::px;
use gpui_kit::assets::IconName;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_i18n::tr;

pub const INVITE: &str = "https://discord.gg/STWsgyJKZS";
pub const LABEL: &str = "Join the Discord";
pub(crate) const BANNER_CARD_ID: &str = "discord-banner";

const BODY: &str = "Get help from the developers and hear about new releases and addons first.";
pub(crate) const LOGO: &[u8] = include_bytes!("../assets/discord.svg");
pub(crate) const BLURPLE: u32 = 0x5865f2;
pub(crate) const BLURPLE_HOVER: u32 = 0x4752c4;

fn server_icon(size: f32) -> Div {
    static ICON: LazyLock<Arc<Image>> = LazyLock::new(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../assets/discord-server.png").to_vec(),
        ))
    });
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(size * 0.28))
        .overflow_hidden()
        .child(img(ICON.clone()).size_full())
}

/// The Discord invite at the top of the News page, shorter in `compact` windows.
pub(crate) fn banner(
    compact: bool,
    on_dismiss: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let white = |alpha: f32| hsla(0., 0., 1., alpha);
    h_flex()
        .id("discord-banner")
        .relative()
        .flex_shrink_0()
        .w_full()
        .min_h(px(if compact { 0. } else { 116. }))
        .pl(px(if compact { 20. } else { 24. }))
        .pr(px(if compact { 38. } else { 24. }))
        .py(px(if compact { 14. } else { 20. }))
        .gap(px(18.))
        .items_center()
        .rounded(px(10.))
        .overflow_hidden()
        .bg(rgb(BLURPLE))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(BLURPLE_HOVER)))
        .on_click(|_, _, cx| cx.open_url(INVITE))
        .child(
            svg()
                .data(LOGO)
                .absolute()
                .top(px(-18.))
                .right(px(150.))
                .size(px(150.))
                .text_color(white(0.08)),
        )
        .child(server_icon(if compact { 48. } else { 68. }))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(3.))
                .text_color(white(1.))
                .child(
                    h_flex()
                        .gap(px(6.))
                        .items_center()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(white(0.8))
                        .child(svg().data(LOGO).size(px(13.)).text_color(white(0.8)))
                        .child(tr("DISCORD SERVER")),
                )
                .child(
                    div()
                        .text_size(px(if compact { 17. } else { 20. }))
                        .font_weight(FontWeight::BOLD)
                        .child(tr("Swofty Developments")),
                )
                .when(!compact, |d| {
                    d.child(
                        div()
                            .text_size(px(13.))
                            .line_height(px(18.))
                            .text_color(white(0.82))
                            .child(BODY),
                    )
                }),
        )
        .child(
            h_flex()
                .id("discord-join")
                .flex_shrink_0()
                .h(px(38.))
                .px(px(16.))
                .gap(px(8.))
                .items_center()
                .rounded(px(8.))
                .bg(white(1.))
                .hover(|s| s.bg(white(0.9)))
                .text_size(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(BLURPLE))
                .child(svg().data(LOGO).size(px(16.)).text_color(rgb(BLURPLE)))
                .child(LABEL)
                .on_click(|_, _, cx| {
                    cx.stop_propagation();
                    cx.open_url(INVITE);
                }),
        )
        .child(
            div()
                .id("discord-banner-dismiss")
                .absolute()
                .top(px(if compact { 6. } else { 8. }))
                .right(px(if compact { 6. } else { 8. }))
                .p(px(4.))
                .rounded(px(4.))
                .cursor_pointer()
                .text_color(white(0.75))
                .hover(|s| s.bg(white(0.16)).text_color(white(1.)))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |ev, window, cx| {
                    cx.stop_propagation();
                    on_dismiss(ev, window, cx);
                })
                .tooltip(|window, cx| Tooltip::new(tr("Dismiss")).build(window, cx))
                .child(Icon::new(IconName::X).size(px(14.))),
        )
}
