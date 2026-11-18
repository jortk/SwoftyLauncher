//! The floating download dock: when it shows, and how far it has slid in.

use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, h_flex, v_flex};
use gpui_kit::{
    Animation, AnimationExt, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, relative,
};

use crate::shell::{Page, Shell};
use crate::theme::Palette;
use swofty_i18n::{tr, trf};

pub(crate) const LINGER: Duration = Duration::from_secs(4);
pub(crate) const SLIDE: Duration = Duration::from_millis(260);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Input {
    pub(crate) pending: usize,
    pub(crate) failed: usize,
    pub(crate) on_downloads: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Frame {
    /// 0 is fully shown, 1 is fully tucked away.
    pub(crate) out: f32,
    pub(crate) animating: bool,
    pub(crate) wake: Option<Duration>,
}

#[derive(Debug, Default)]
pub(crate) struct Dock {
    shown: bool,
    moved: Option<Instant>,
    idle_since: Option<Instant>,
    had_work: bool,
    dismissed: usize,
    base: Option<usize>,
}

impl Dock {
    pub(crate) fn failure_shown(&self, failed: usize) -> bool {
        failed > self.dismissed
    }

    pub(crate) fn batch(&mut self, finished: usize, pending: usize) -> (usize, usize) {
        if pending > 0 && self.base.is_none() {
            self.base = Some(finished);
        }
        batch_position(self.base.unwrap_or(finished), finished, pending)
    }

    pub(crate) fn dismiss(&mut self, failed: usize) {
        self.dismissed = failed;
    }

    fn wants_shown(&mut self, now: Instant, input: Input) -> bool {
        if input.on_downloads {
            self.dismissed = input.failed;
            self.idle_since = None;
            self.had_work = false;
            return false;
        }
        self.dismissed = self.dismissed.min(input.failed);
        if input.pending > 0 {
            self.idle_since = None;
            self.had_work = true;
            return true;
        }
        if self.had_work && self.idle_since.is_none() {
            self.idle_since = Some(now);
        }
        let lingering = self
            .idle_since
            .is_some_and(|since| now.saturating_duration_since(since) < LINGER);
        if !lingering {
            self.had_work = false;
            self.idle_since = None;
        }
        lingering || input.failed > self.dismissed
    }

    pub(crate) fn frame(&mut self, now: Instant, input: Input) -> Option<Frame> {
        let target = self.wants_shown(now, input);
        if target != self.shown {
            self.shown = target;
            self.moved = Some(now);
        }
        let progress = self.moved.map_or(1., |at| {
            (now.saturating_duration_since(at).as_secs_f32() / SLIDE.as_secs_f32()).clamp(0., 1.)
        });
        let eased = 1. - (1. - progress).powi(3);
        let out = if self.shown { 1. - eased } else { eased };
        if !self.shown && progress >= 1. {
            self.base = None;
            return None;
        }
        let wake = self
            .idle_since
            .map(|since| LINGER.saturating_sub(now.saturating_duration_since(since)))
            .filter(|_| self.shown && input.pending == 0);
        Some(Frame {
            out,
            animating: progress < 1.,
            wake,
        })
    }
}

pub(crate) fn batch_position(base: usize, finished: usize, pending: usize) -> (usize, usize) {
    let done = finished.saturating_sub(base);
    let total = done + pending;
    ((done + 1).min(total.max(1)), total.max(1))
}

pub(crate) fn download_title(position: usize, total: usize, pending: bool) -> String {
    match pending {
        true => trf!(
            "Downloading {position}/{total}",
            position = position,
            total = total
        ),
        false => tr("Download complete").to_owned(),
    }
}

pub(crate) fn bar_fraction(fraction: Option<f32>) -> Option<f32> {
    fraction.map(|f| f.clamp(0., 1.))
}

#[derive(Clone, Debug)]
pub(crate) struct Line {
    pub(crate) name: String,
    pub(crate) fraction: Option<f32>,
    pub(crate) failed: bool,
}

const WIDTH: f32 = 340.;
pub(crate) const NOTCH_HEIGHT: f32 = 46.;

impl Shell {
    pub(crate) fn download_notch(
        &mut self,
        p: Palette,
        layout_width: f32,
        has_sub: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.notch_beside = false;
        let now = Instant::now();
        let input = self.dock_input();
        let fake = cfg!(debug_assertions) && std::env::var_os("SWOFTY_FAKE_DOWNLOAD").is_some();
        let frame = self.dock.frame(now, input);
        if frame.as_ref().is_some_and(|f| f.animating) {
            window.request_animation_frame();
        }
        if let Some(wake) = frame.as_ref().and_then(|f| f.wake)
            && !self.dock_wake
        {
            self.dock_wake = true;
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(wake + Duration::from_millis(20))
                    .await;
                this.update(cx, |this, cx| {
                    this.dock_wake = false;
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        let (title, name, fraction) = if fake {
            let unknown = std::env::var("SWOFTY_FAKE_DOWNLOAD").is_ok_and(|v| v == "unknown");
            (
                download_title(2, 3, true),
                "Project Halo".to_owned(),
                (!unknown).then_some(0.45),
            )
        } else if let Some(frame) = frame.as_ref().filter(|f| f.out < 0.95) {
            let _ = frame;
            let line = self.dock_line()?;
            if line.failed {
                self.dock.dismiss(input.failed);
                return None;
            }
            let finished = self.finished_downloads();
            let (position, total) = self.dock.batch(finished, input.pending);
            (
                download_title(position, total, input.pending > 0),
                line.name,
                if input.pending > 0 {
                    bar_fraction(line.fraction)
                } else {
                    Some(1.)
                },
            )
        } else if let Some((done, text)) = self.progress.clone() {
            (download_title(1, 1, true), text, bar_fraction(Some(done)))
        } else {
            return None;
        };
        let beside = has_sub && layout_width >= 1150.;
        self.notch_beside = beside;
        let top = if has_sub && !beside {
            crate::shell::BAR_HEIGHT + crate::shell::SUB_HEIGHT
        } else {
            crate::shell::BAR_HEIGHT
        };
        let row = h_flex()
            .id("download-notch")
            .relative()
            .w(crate::scale::fixed(WIDTH))
            .h(crate::scale::fixed(NOTCH_HEIGHT))
            .px(crate::scale::fixed(16.))
            .gap(crate::scale::fixed(10.))
            .items_center()
            .overflow_hidden()
            .rounded_b(crate::scale::fixed(16.))
            .bg(p.chrome)
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.open_page(Page::Downloads, cx)))
            .child(
                Icon::new(IconName::Download)
                    .size(crate::scale::fixed(18.))
                    .text_color(p.accent),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(crate::scale::fixed(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(crate::scale::fixed(12.))
                            .text_color(p.text_muted)
                            .child(name),
                    ),
            )
            .child(match fraction {
                Some(f) => div()
                    .absolute()
                    .left_0()
                    .bottom_0()
                    .h(crate::scale::fixed(3.))
                    .w(relative(f))
                    .bg(p.accent)
                    .into_any_element(),
                None => div()
                    .absolute()
                    .left_0()
                    .bottom_0()
                    .h(crate::scale::fixed(3.))
                    .w_full()
                    .child(div().h_full().w(relative(0.3)).bg(p.accent).with_animation(
                        "download-indeterminate",
                        Animation::new(Duration::from_millis(1400)).repeat(),
                        |el, t| el.ml(relative(t * 0.7)),
                    ))
                    .into_any_element(),
            });
        let placed = if beside {
            div()
                .absolute()
                .top(crate::scale::fixed(top))
                .right(crate::scale::fixed(24.))
                .child(row)
        } else {
            div()
                .absolute()
                .top(crate::scale::fixed(top))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(row)
        };
        Some(placed.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn busy() -> Input {
        Input {
            pending: 1,
            ..Input::default()
        }
    }

    fn idle() -> Input {
        Input::default()
    }

    #[test]
    fn hidden_until_a_download_starts() {
        let mut dock = Dock::default();
        assert_eq!(dock.frame(Instant::now(), idle()), None);
    }

    #[test]
    fn visible_while_queued_or_active() {
        let mut dock = Dock::default();
        let t = Instant::now();
        assert!(dock.frame(t, busy()).is_some());
        let settled = dock.frame(t + SLIDE + Duration::from_secs(60), busy()).unwrap();
        assert_eq!(settled.out, 0.);
        assert!(!settled.animating);
        assert_eq!(settled.wake, None);
    }

    #[test]
    fn slides_in_from_tucked_away() {
        let mut dock = Dock::default();
        let t = Instant::now();
        assert_eq!(dock.frame(t, busy()).unwrap().out, 1.);
        let mid = dock.frame(t + SLIDE / 2, busy()).unwrap();
        assert!(mid.out > 0. && mid.out < 1. && mid.animating);
    }

    #[test]
    fn lingers_then_slides_out() {
        let mut dock = Dock::default();
        let t = Instant::now();
        dock.frame(t, busy());
        let done = t + Duration::from_secs(10);
        let lingering = dock.frame(done, idle()).unwrap();
        assert_eq!(lingering.wake, Some(LINGER));
        assert!(
            dock.frame(done + LINGER - Duration::from_millis(1), idle())
                .is_some()
        );
        let leaving = dock.frame(done + LINGER, idle()).unwrap();
        assert!(leaving.animating);
        assert_eq!(dock.frame(done + LINGER + SLIDE, idle()), None);
    }

    #[test]
    fn new_work_cancels_the_linger() {
        let mut dock = Dock::default();
        let t = Instant::now();
        dock.frame(t, busy());
        dock.frame(t + Duration::from_secs(10), idle());
        let again = t + Duration::from_secs(12);
        dock.frame(again, busy());
        let frame = dock.frame(again + LINGER * 3, busy()).unwrap();
        assert_eq!(frame.out, 0.);
    }

    #[test]
    fn failure_stays_until_dismissed() {
        let mut dock = Dock::default();
        let t = Instant::now();
        let failed = Input {
            failed: 1,
            ..Input::default()
        };
        dock.frame(t, busy());
        dock.frame(t + Duration::from_secs(1), failed);
        let late = t + Duration::from_secs(600);
        assert_eq!(dock.frame(late, failed).unwrap().out, 0.);
        dock.dismiss(1);
        dock.frame(late + Duration::from_secs(1), failed);
        assert_eq!(dock.frame(late + Duration::from_secs(2), failed), None);
    }

    #[test]
    fn a_new_failure_shows_after_a_dismissal() {
        let mut dock = Dock::default();
        let t = Instant::now();
        dock.dismiss(1);
        let two = Input {
            failed: 2,
            ..Input::default()
        };
        assert!(dock.frame(t, two).is_some());
    }

    #[test]
    fn clearing_failures_resets_the_dismissal() {
        let mut dock = Dock::default();
        let t = Instant::now();
        dock.dismiss(2);
        let two = Input {
            failed: 2,
            ..Input::default()
        };
        assert!(dock.frame(t, two).is_none());
        let one = Input {
            failed: 1,
            ..Input::default()
        };
        assert!(dock.frame(t + Duration::from_secs(1), one).is_none());
        let two = Input {
            failed: 2,
            ..Input::default()
        };
        assert!(dock.frame(t + Duration::from_secs(2), two).is_some());
    }

    #[test]
    fn hidden_on_the_downloads_page_and_failures_count_as_seen() {
        let mut dock = Dock::default();
        let t = Instant::now();
        let failed_here = Input {
            failed: 1,
            on_downloads: true,
            ..Input::default()
        };
        let busy_here = Input {
            pending: 2,
            on_downloads: true,
            ..Input::default()
        };
        dock.frame(t, busy());
        assert!(dock.frame(t + SLIDE * 2, busy_here).unwrap().animating);
        assert_eq!(dock.frame(t + SLIDE * 5, busy_here), None);
        dock.frame(t + SLIDE * 6, failed_here);
        let away = Input {
            failed: 1,
            ..Input::default()
        };
        assert_eq!(dock.frame(t + SLIDE * 8, away), None);
    }

    #[test]
    fn the_batch_counts_from_its_start() {
        assert_eq!(batch_position(0, 0, 3), (1, 3));
        assert_eq!(batch_position(0, 1, 2), (2, 3));
        assert_eq!(batch_position(5, 6, 1), (2, 2));
        assert_eq!(batch_position(0, 3, 0), (3, 3));
        assert_eq!(batch_position(4, 4, 0), (1, 1));
    }

    #[test]
    fn a_dock_remembers_where_its_batch_began() {
        let mut dock = Dock::default();
        assert_eq!(dock.batch(7, 2), (1, 2));
        assert_eq!(dock.batch(8, 1), (2, 2));
    }

    #[test]
    fn the_title_and_fraction() {
        assert_eq!(download_title(1, 3, true), "Downloading 1/3");
        assert_eq!(download_title(3, 3, false), "Download complete");
        assert_eq!(bar_fraction(Some(1.4)), Some(1.));
        assert_eq!(bar_fraction(Some(-0.2)), Some(0.));
        assert_eq!(bar_fraction(None), None);
    }
}
