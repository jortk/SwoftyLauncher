use super::Pill;
use crate::library::{AddonIcon, ProfileItem};
use crate::notices::{Notification, Notify};
use crate::scale::px;
use crate::shell::{Page, Prompt, Shell, SliderSlot};
use crate::theme::{Palette, ScrollArea, palette};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::scroll::Scrollable;
use gpui_kit::component::slider::{Slider, SliderEvent, SliderState};
use gpui_kit::component::{Disableable, Icon, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};
use swofty_core::manifest::{AddonManifest, AddonOption, Binds, OptionType};
use swofty_core::profile::VANILLA;
use swofty_core::store::ProfileDeletion;
use swofty_i18n::{tr, trf, trn};

pub(crate) fn tag(label: impl Into<SharedString>, _: Palette) -> Pill {
    Pill::new(label)
}

pub(crate) fn tile(size: f32, p: Palette) -> Div {
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(5.))
        .bg(p.elevated)
        .border_1()
        .border_color(p.border_variant)
        .flex()
        .items_center()
        .justify_center()
}

pub(crate) fn badge(icon: IconName, size: f32, p: Palette) -> Div {
    tile(size, p).child(Icon::new(icon).size(px(size * 0.5)).text_color(p.text_muted))
}

/// The race track mark (The Noun Project, RaceTrack 7280751, licensed).
pub(crate) const TRACK: &[u8] = include_bytes!("../../assets/track.svg");

/// The track mark for track addons without an icon of their own.
pub(crate) fn track_badge(size: f32, p: Palette) -> Div {
    tile(size, p).child(svg().data(TRACK).size(px(size * 0.55)).text_color(p.text_muted))
}

pub(crate) fn logo(size: f32) -> Div {
    static LOGO: LazyLock<Arc<Image>> = LazyLock::new(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../../assets/icon-256.png").to_vec(),
        ))
    });
    div()
        .w(px(size))
        .h(px(size))
        .flex_shrink_0()
        .child(img(LOGO.clone()).size_full())
}

pub(crate) fn wordmark(width: f32) -> Div {
    static MARK: LazyLock<Arc<Image>> = LazyLock::new(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../../assets/wordmark.png").to_vec(),
        ))
    });
    div()
        .w(px(width))
        .h(px(width * 238. / 440.))
        .flex_shrink_0()
        .child(img(MARK.clone()).size_full())
}

pub(crate) fn game_art(w: f32, h: f32) -> Div {
    static ART: LazyLock<Arc<Image>> = LazyLock::new(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Png,
            include_bytes!("../../assets/f1-25.png").to_vec(),
        ))
    });
    div()
        .w(px(w))
        .h(px(h))
        .flex_shrink_0()
        .overflow_hidden()
        .rounded(px(12.))
        .child(img(ART.clone()).size_full())
}

pub(crate) fn addon_icon(icon: &AddonIcon, size: f32, p: Palette) -> Div {
    match icon {
        AddonIcon::Glyph(data) => {
            tile(size, p).child(svg().data(data).size(px(size * 0.5)).text_color(p.text_muted))
        }
        AddonIcon::Image(image) => div()
            .w(px(size))
            .h(px(size))
            .flex_shrink_0()
            .rounded(px(size * 0.22))
            .overflow_hidden()
            .child(img(image.clone()).size_full()),
    }
}

pub(crate) fn monogram(text: &str, size: f32, p: Palette) -> Div {
    let letter: String = text.chars().take(1).flat_map(char::to_uppercase).collect();
    tile(size, p)
        .text_size(px(size * 0.42))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(p.text_muted)
        .child(letter)
}

pub(crate) fn avatar(name: &str, size: f32, p: Palette) -> Div {
    if name == VANILLA {
        badge(IconName::ShieldCheck, size, p)
    } else {
        monogram(name, size, p)
    }
}

pub(crate) fn label(title: impl Into<SharedString>, p: Palette) -> Div {
    div()
        .text_size(px(11.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.text_placeholder)
        .child(title.into())
}

pub(crate) fn fact(label: impl Into<SharedString>, value: impl IntoElement, p: Palette) -> Div {
    h_flex()
        .py(px(6.))
        .gap(px(12.))
        .border_b_1()
        .border_color(p.border_variant)
        .child(
            div()
                .w(px(104.))
                .flex_shrink_0()
                .text_size(px(13.))
                .text_color(p.text_muted)
                .child(label.into()),
        )
        .child(div().flex_1().min_w_0().text_size(px(13.)).child(value))
}

pub(crate) fn stat(
    title: impl Into<SharedString>,
    value: impl Into<SharedString>,
    color: Hsla,
    p: Palette,
) -> Div {
    v_flex()
        .gap(px(3.))
        .child(label(title, p))
        .child(div().text_size(px(13.)).text_color(color).child(value.into()))
}

pub(crate) fn chevron(open: bool, p: Palette) -> Icon {
    Icon::new(if open {
        IconName::ChevronUp
    } else {
        IconName::ChevronDown
    })
    .size(px(15.))
    .text_color(p.text_placeholder)
}

pub(crate) fn names(title: impl Into<SharedString>, items: &[String], p: Palette) -> Div {
    const SHOWN: usize = 6;
    let text = if items.is_empty() {
        tr("None").to_owned()
    } else {
        let mut t = items.iter().take(SHOWN).cloned().collect::<Vec<_>>().join(", ");
        if items.len() > SHOWN {
            t.push_str(&trf!(" and {count} more", count = items.len() - SHOWN));
        }
        t
    };
    v_flex().gap(px(3.)).child(label(title, p)).child(
        div()
            .text_size(px(13.))
            .line_height(px(19.))
            .text_color(if items.is_empty() {
                p.text_placeholder
            } else {
                p.text
            })
            .child(text),
    )
}
