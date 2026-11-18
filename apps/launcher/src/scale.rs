use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use gpui_kit::component::Theme;
use gpui_kit::{App, Pixels, Window};
use swofty_core::session::{UiMode, clamp_ui_scale};

pub const TV_SCALE: f32 = 1.3;
const BASE_REM: f32 = 16.;
const BASE_FONT: f32 = 14.;

const MIN_CONTENT: f32 = 320.;

static SCALE: AtomicU32 = AtomicU32::new(1.0_f32.to_bits());
static TV: AtomicBool = AtomicBool::new(false);

pub fn scale_for(mode: UiMode) -> f32 {
    match mode {
        UiMode::Pc => 1.,
        UiMode::Tv => TV_SCALE,
    }
}

/// The scale for a display mode and an interface scale percent: Auto is the mode's own scale,
/// a percent multiplies it.
pub fn scale_with(mode: UiMode, percent: Option<u16>) -> f32 {
    let base = scale_for(mode);
    match percent {
        None => base,
        Some(percent) => base * f32::from(clamp_ui_scale(percent)) / 100.,
    }
}

/// The scale in effect now.
pub fn current() -> f32 {
    f32::from_bits(SCALE.load(Ordering::Relaxed))
}

fn scaled(value: f32, scale: f32) -> f32 {
    value * scale
}

pub const TV_CONTROL: f32 = 1.5;

pub fn is_tv() -> bool {
    TV.load(Ordering::Relaxed)
}

pub fn control() -> f32 {
    if is_tv() { TV_CONTROL } else { 1. }
}

pub fn fixed(value: f32) -> Pixels {
    gpui_kit::px(value)
}

pub fn ctl(value: f32) -> Pixels {
    gpui_kit::px(scaled(value, current()) * control())
}

pub trait Hit: gpui_kit::component::Sizable + Sized {
    fn tap_xs(self) -> Self {
        if is_tv() { self.large() } else { self.xsmall() }
    }

    fn tap_sm(self) -> Self {
        if is_tv() { self.large() } else { self.small() }
    }
}

impl<T: gpui_kit::component::Sizable> Hit for T {}

pub fn px(value: f32) -> Pixels {
    gpui_kit::px(scaled(value, current()))
}

/// The width a page has for its content, in layout units: the window's width in
/// pixels, divided back down by the scale, less the page gutters.
pub fn content_width(viewport_px: f32, scale: f32) -> f32 {
    (viewport_px / scale - 2. * crate::ui::tokens::space::GUTTER).max(MIN_CONTENT)
}

pub fn apply(mode: UiMode, percent: Option<u16>, window: &mut Window, cx: &mut App) {
    let scale = scale_with(mode, percent);
    SCALE.store(scale.to_bits(), Ordering::Relaxed);
    TV.store(mode.is_tv(), Ordering::Relaxed);
    window.set_rem_size(gpui_kit::px(BASE_REM * scale));
    Theme::update(cx, |t| {
        t.font_size = gpui_kit::px(BASE_FONT * scale);
        t.notification.margins.bottom = gpui_kit::px((crate::play_bar::HEIGHT + 16.) * scale);
    });
    Theme::sync_base(cx);
    if mode.is_tv() {
        widen_scrollbars(scale / TV_SCALE, cx);
    }
    window.refresh();
}

fn widen_scrollbars(relative: f32, cx: &mut App) {
    let w = |value: f32| gpui_kit::px(value * relative);
    let mut base = cx.global::<gpui_kit::base::Theme>().clone();
    let styles = base
        .scrollbar
        .styles()
        .clone()
        .track(|s| s.width(w(32.)))
        .track_hover(|s| s.width(w(32.)))
        .track_active(|s| s.width(w(32.)))
        .thumb(|s| s.width(w(14.)).inset(w(9.)).min_length(w(96.)))
        .thumb_hover(|s| s.width(w(16.)).inset(w(8.)).min_length(w(96.)))
        .thumb_active(|s| s.width(w(18.)).inset(w(7.)).min_length(w(96.)));
    base.scrollbar = base.scrollbar.with_styles(styles);
    cx.set_global(base);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pc_keeps_the_layout_and_tv_scales_it_up() {
        assert_eq!(scale_for(UiMode::Pc), 1.);
        assert!(scale_for(UiMode::Tv) > 1.);
    }

    #[test]
    fn content_width_is_in_layout_units_in_both_modes() {
        assert_eq!(content_width(1440., 1.), 1440. - 56.);
        let tv = content_width(1440., TV_SCALE);
        assert!((tv - (1440. / TV_SCALE - 56.)).abs() < 0.01);
        assert!(tv < content_width(1440., 1.));
        assert_eq!(content_width(400., TV_SCALE), 320.);
    }

    #[test]
    fn controls_grow_only_in_tv() {
        assert_eq!(control(), 1.);
        SCALE.store(TV_SCALE.to_bits(), Ordering::Relaxed);
        TV.store(true, Ordering::Relaxed);
        assert_eq!(control(), TV_CONTROL);
        assert!(f32::from(ctl(26.)) >= 44.);
        SCALE.store(1.0_f32.to_bits(), Ordering::Relaxed);
        TV.store(false, Ordering::Relaxed);
    }

    #[test]
    fn auto_keeps_the_mode_scale_and_a_percent_multiplies_it() {
        assert_eq!(scale_with(UiMode::Pc, None), 1.);
        assert_eq!(scale_with(UiMode::Tv, None), TV_SCALE);
        assert_eq!(scale_with(UiMode::Pc, Some(100)), 1.);
        assert_eq!(scale_with(UiMode::Pc, Some(150)), 1.5);
        assert!((scale_with(UiMode::Tv, Some(75)) - TV_SCALE * 0.75).abs() < 1e-6);
        assert_eq!(scale_with(UiMode::Pc, Some(9999)), 4.);
        assert_eq!(scale_with(UiMode::Pc, Some(0)), 0.01);
        assert_eq!(scale_with(UiMode::Pc, Some(1)), 0.01);
        assert_eq!(scale_with(UiMode::Pc, Some(237)), 2.37);
    }

    #[test]
    fn scaling_multiplies_lengths() {
        assert_eq!(scaled(10., 1.), 10.);
        assert_eq!(scaled(10., 1.5), 15.);
        assert_eq!(scaled(0., TV_SCALE), 0.);
    }
}
