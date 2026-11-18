//! Native GPU paths and text only: no raster files, webview, or helper process.
use std::sync::Arc;

use gpui_kit::base::plot::{PlotLabel, label::Text as PlotText};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use swofty_core::view::*;
use swofty_i18n::{tr, trf};

use crate::{scale::px, theme::Palette};

const COLORS: [u32; 10] = [
    0x57b7ff, 0xffa52f, 0x28cbb2, 0xf26787, 0xb99aff, 0xffdc72, 0x77c65e, 0xee9be0, 0x99aaba, 0xe18d60,
];

fn color(s: Option<&str>, fallback: Hsla) -> Hsla {
    s.and_then(|s| {
        u32::from_str_radix(&s[1..], 16).ok().map(|n| {
            if s.len() == 9 {
                let mut c: Hsla = rgb(n >> 8).into();
                c.a = (n & 255) as f32 / 255.;
                c
            } else {
                rgb(n).into()
            }
        })
    })
    .unwrap_or(fallback)
}

#[derive(Clone, Copy, Debug)]
struct Scale {
    low: f64,
    high: f64,
    reversed: bool,
}
impl Scale {
    fn map(self, v: f64) -> f32 {
        let n = ((v - self.low) / (self.high - self.low)).clamp(-4096., 4097.) as f32;
        if self.reversed { 1. - n } else { n }
    }
}

#[derive(Clone)]
enum Mark {
    Path {
        points: Vec<[f32; 2]>,
        color: Hsla,
        fill: bool,
        dashed: bool,
    },
    Dot {
        at: [f32; 2],
        color: Hsla,
        hollow: bool,
    },
    Label {
        at: [f32; 2],
        text: String,
        width: f32,
        color: Hsla,
    },
}
pub(crate) struct Scene {
    x: Scale,
    y: Scale,
    marks: Vec<Mark>,
    has_data: bool,
}

fn domains(c: &Chart) -> (Scale, Scale) {
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    let mut add = |x: f64, y: f64| {
        lo[0] = lo[0].min(x);
        hi[0] = hi[0].max(x);
        lo[1] = lo[1].min(y);
        hi[1] = hi[1].max(y);
    };
    for s in &c.series {
        match &s.data {
            ChartData::Line { points, .. } => {
                for p in points.iter().flatten() {
                    add(p.x, p.y);
                }
            }
            ChartData::Scatter { points, .. } => {
                for p in points {
                    add(p.x, p.y);
                }
            }
            ChartData::Band { points } => {
                for p in points.iter().flatten() {
                    add(p.x, p.low);
                    add(p.x, p.high);
                }
            }
            ChartData::Bars {
                values,
                width,
                horizontal,
            } => {
                for b in values {
                    if *horizontal {
                        add(b.base, b.position - width / 2.);
                        add(b.value, b.position + width / 2.);
                    } else {
                        add(b.position - width / 2., b.base);
                        add(b.position + width / 2., b.value);
                    }
                }
            }
            ChartData::Ranges { cells } | ChartData::Heatmap { cells } => {
                for r in cells {
                    add(r.x[0], r.y[0]);
                    add(r.x[1], r.y[1]);
                }
            }
            ChartData::Boxes { values, width } => {
                for b in values {
                    add(b.position - width / 2., b.min);
                    add(b.position + width / 2., b.max);
                }
            }
        }
    }
    let scales = std::array::from_fn::<_, 2, _>(|i| {
        let axis = if i == 0 { &c.x } else { &c.y };
        let [mut a, mut b] = axis.domain.unwrap_or([lo[i], hi[i]]);
        if !a.is_finite() || !b.is_finite() {
            a = 0.;
            b = 1.;
        }
        if b - a < 1e-9 {
            let d = (a.abs() * 0.05).max(1.);
            a -= d;
            b += d;
        }
        Scale {
            low: a,
            high: b,
            reversed: axis.reversed,
        }
    });
    (scales[0], scales[1])
}

impl Scene {
    fn new(c: &Chart) -> Self {
        let (x, y) = domains(c);
        let mut scene = Self {
            x,
            y,
            marks: Vec::new(),
            has_data: false,
        };
        for (i, s) in c.series.iter().enumerate() {
            let col = color(s.color.as_deref(), rgb(COLORS[i % COLORS.len()]).into());
            match &s.data {
                ChartData::Line {
                    points,
                    dashed,
                    interpolation,
                    dots,
                } => {
                    let mut run = Vec::new();
                    for p in points.iter().chain(std::iter::once(&None)) {
                        if let Some(p) = p {
                            let at = scene.at(p.x, p.y);
                            if *interpolation == ChartInterpolation::StepAfter
                                && let Some(last) = run.last()
                            {
                                let last: &[f32; 2] = last;
                                run.push([at[0], last[1]]);
                            }
                            run.push(at);
                            if *dots {
                                scene.marks.push(Mark::Dot {
                                    at,
                                    color: col,
                                    hollow: false,
                                });
                            }
                            scene.has_data = true;
                        } else if !run.is_empty() {
                            if run.len() == 1 {
                                scene.marks.push(Mark::Dot {
                                    at: run[0],
                                    color: col,
                                    hollow: false,
                                });
                            } else {
                                scene.marks.push(Mark::Path {
                                    points: std::mem::take(&mut run),
                                    color: col,
                                    fill: false,
                                    dashed: *dashed,
                                });
                            }
                            run.clear();
                        }
                    }
                }
                ChartData::Scatter { points, hollow } => {
                    for p in points {
                        scene.has_data = true;
                        scene.marks.push(Mark::Dot {
                            at: scene.at(p.x, p.y),
                            color: col,
                            hollow: *hollow,
                        });
                    }
                }
                ChartData::Band { points } => {
                    let mut run: Vec<&ChartBandPoint> = Vec::new();
                    for p in points.iter().chain(std::iter::once(&None)) {
                        if let Some(p) = p {
                            run.push(p);
                            scene.has_data = true;
                        } else if !run.is_empty() {
                            let pts = run
                                .iter()
                                .map(|p| scene.at(p.x, p.high))
                                .chain(run.iter().rev().map(|p| scene.at(p.x, p.low)))
                                .collect();
                            scene.marks.push(Mark::Path {
                                points: pts,
                                color: col,
                                fill: true,
                                dashed: false,
                            });
                            run.clear();
                        }
                    }
                }
                ChartData::Bars {
                    values,
                    width,
                    horizontal,
                } => {
                    for b in values {
                        if *horizontal {
                            scene.rect(
                                [b.base, b.value],
                                [b.position - width / 2., b.position + width / 2.],
                                col,
                            );
                        } else {
                            scene.rect(
                                [b.position - width / 2., b.position + width / 2.],
                                [b.base, b.value],
                                col,
                            );
                        }
                    }
                }
                ChartData::Ranges { cells } | ChartData::Heatmap { cells } => {
                    for r in cells {
                        scene.rect(r.x, r.y, color(r.color.as_deref(), col));
                        if !r.label.is_empty() {
                            scene.marks.push(Mark::Label {
                                at: scene.at((r.x[0] + r.x[1]) / 2., (r.y[0] + r.y[1]) / 2.),
                                text: r.label.clone(),
                                width: (scene.x.map(r.x[1]) - scene.x.map(r.x[0])).abs(),
                                color: {
                                    let c = color(r.color.as_deref(), col);
                                    if c.l > 0.55 {
                                        rgb(0x10141a).into()
                                    } else {
                                        rgb(0xffffff).into()
                                    }
                                },
                            });
                        }
                    }
                }
                ChartData::Boxes { values, width } => {
                    for b in values {
                        scene.rect(
                            [b.position - width / 2., b.position + width / 2.],
                            [b.q1, b.q3],
                            col,
                        );
                        for (a, z) in [
                            ((b.position, b.min), (b.position, b.q1)),
                            ((b.position, b.q3), (b.position, b.max)),
                            ((b.position - width / 4., b.min), (b.position + width / 4., b.min)),
                            ((b.position - width / 4., b.max), (b.position + width / 4., b.max)),
                        ] {
                            scene.line(a, z, col);
                        }
                        scene.line(
                            (b.position - width / 2., b.median),
                            (b.position + width / 2., b.median),
                            rgb(0x10141a).into(),
                        );
                    }
                }
            }
        }
        scene
    }
    fn at(&self, x: f64, y: f64) -> [f32; 2] {
        [self.x.map(x), 1. - self.y.map(y)]
    }
    fn line(&mut self, a: (f64, f64), b: (f64, f64), color: Hsla) {
        self.marks.push(Mark::Path {
            points: vec![self.at(a.0, a.1), self.at(b.0, b.1)],
            color,
            fill: false,
            dashed: false,
        });
    }
    fn rect(&mut self, x: [f64; 2], y: [f64; 2], color: Hsla) {
        self.has_data = true;
        self.marks.push(Mark::Path {
            points: vec![
                self.at(x[0], y[0]),
                self.at(x[1], y[0]),
                self.at(x[1], y[1]),
                self.at(x[0], y[1]),
            ],
            color,
            fill: true,
            dashed: false,
        });
    }
}

/// Shared by addon blocks and the standalone chart gallery; IDs are scoped by caller.
pub(crate) fn prepare(c: &Chart) -> Arc<Scene> {
    Arc::new(Scene::new(c))
}

pub(crate) fn render(id: String, c: &Chart, p: Palette, prepared: Option<Arc<Scene>>) -> AnyElement {
    let mut root = v_flex()
        .id(id)
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .gap(px(8.))
        .p(px(16.))
        .rounded(px(10.))
        .border_1()
        .border_color(p.border_variant)
        .bg(p.surface)
        .text_color(p.text);
    if !c.title.is_empty() {
        root = root.child(
            div()
                .text_size(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(c.title.clone()),
        );
    }
    if let Err(error) = c.validate() {
        return root
            .child(
                div()
                    .text_color(p.error)
                    .child(trf!("Cannot display chart: {error}", error = error)),
            )
            .into_any_element();
    }
    let scene = prepared.unwrap_or_else(|| prepare(c));
    if !scene.has_data {
        return root
            .child(
                div()
                    .h(px(c.height))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(p.text_muted)
                    .child(tr("No chart data")),
            )
            .into_any_element();
    }
    let legend = h_flex().flex_wrap().gap(px(14.)).children(
        c.series
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.name.is_empty())
            .map(|(i, s)| {
                h_flex()
                    .gap(px(5.))
                    .child(
                        div()
                            .size(px(8.))
                            .rounded_full()
                            .bg(color(s.color.as_deref(), rgb(COLORS[i % COLORS.len()]).into())),
                    )
                    .child(div().text_size(px(11.)).child(s.name.clone()))
            }),
    );
    let axes = [c.x.clone(), c.y.clone()];
    root = root.child(legend).child(
        canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                paint(&scene, &axes, bounds, p, window, cx);
            },
        )
        .w_full()
        .flex_shrink_0()
        .h(px(c.height))
        .min_w(px(100.)),
    );
    if !c.caption.is_empty() {
        root = root.child(
            div()
                .text_size(px(11.))
                .text_color(p.text_muted)
                .child(c.caption.clone()),
        );
    }
    root.into_any_element()
}

fn paint(
    scene: &Scene,
    axes: &[ChartAxis; 2],
    bounds: Bounds<Pixels>,
    p: Palette,
    window: &mut Window,
    cx: &mut App,
) {
    // Scroll containers still paint canvas children outside their viewport. Reject
    // them before allocating projected points or tessellating any native paths.
    if bounds.intersect(&window.content_mask().bounds).is_empty() {
        return;
    }
    if std::env::var_os("SWOFTY_CHART_METRICS").is_some() {
        static PAINTS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        if PAINTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < 16 {
            eprintln!(
                "native chart painted: {} marks, {} x {} logical pixels",
                scene.marks.len(),
                bounds.size.width.as_f32(),
                bounds.size.height.as_f32()
            );
        }
    }
    let gutter = px(88.);
    let top = px(10.);
    let bottom = px(48.);
    let plot = Bounds::new(
        bounds.origin + point(gutter, top),
        size(
            (bounds.size.width - gutter - px(18.)).max(px(1.)),
            (bounds.size.height - top - bottom).max(px(1.)),
        ),
    );
    let at = |v: [f32; 2]| plot.origin + point(plot.size.width * v[0], plot.size.height * v[1]);
    let mut labels = Vec::new();
    for (axis, scale, vertical) in [(&axes[0], scene.x, false), (&axes[1], scene.y, true)] {
        let ticks = if axis.ticks.is_empty() {
            (0..=4)
                .map(|i| {
                    let v = scale.low + (scale.high - scale.low) * f64::from(i) / 4.;
                    ChartTick {
                        value: v,
                        label: axis.format.format(v),
                    }
                })
                .collect()
        } else {
            axis.ticks.clone()
        };
        let stride = if vertical {
            (ticks.len() as f32 / (plot.size.height.as_f32() / px(20.).as_f32()).max(1.)).ceil() as usize
        } else {
            (ticks.len() as f32 / (plot.size.width.as_f32() / px(65.).as_f32()).max(1.)).ceil() as usize
        }
        .max(1);
        for t in ticks.iter().step_by(stride) {
            let n = scale.map(t.value);
            if !(0. ..=1.).contains(&n) {
                continue;
            }
            let (a, b, origin, align) = if vertical {
                (
                    [0., 1. - n],
                    [1., 1. - n],
                    point(gutter - px(8.), top + plot.size.height * (1. - n) - px(6.)),
                    TextAlign::Right,
                )
            } else {
                (
                    [n, 0.],
                    [n, 1.],
                    point(gutter + plot.size.width * n, top + plot.size.height + px(8.)),
                    TextAlign::Center,
                )
            };
            path(&[at(a), at(b)], p.border_variant, false, window);
            labels.push(
                PlotText::new(t.label.clone(), origin, p.text_muted)
                    .font_size(px(10.))
                    .align(align),
            );
        }
    }
    labels.push(
        PlotText::new(
            axes[0].label.clone(),
            point(gutter + plot.size.width / 2., bounds.size.height - px(16.)),
            p.text_muted,
        )
        .font_size(px(11.))
        .align(TextAlign::Center),
    );
    labels
        .push(PlotText::new(axes[1].label.clone(), point(gutter, px(-3.)), p.text_muted).font_size(px(10.)));
    window.with_content_mask(Some(ContentMask { bounds: plot }), |window| {
        for mark in &scene.marks {
            match mark {
                Mark::Path {
                    points,
                    color,
                    fill,
                    dashed,
                } => {
                    let points = if *fill {
                        points.clone()
                    } else {
                        simplify_path(points, plot.size.width.as_f32(), plot.size.height.as_f32())
                    };
                    let points: Vec<_> = points.into_iter().map(at).collect();
                    if *dashed {
                        dashed_path(&points, *color, plot, window);
                    } else {
                        path(&points, *color, *fill, window);
                    }
                }
                Mark::Dot { at: v, color, hollow } => {
                    let center = at(*v);
                    let radius = px(3.);
                    let points: Vec<_> = (0..=16)
                        .map(|i| {
                            let a = i as f32 * std::f32::consts::TAU / 16.;
                            center + point(radius * a.cos(), radius * a.sin())
                        })
                        .collect();
                    path(&points, *color, !hollow, window);
                }
                Mark::Label {
                    at: v,
                    text,
                    width,
                    color,
                } => {
                    let origin = at(*v) - bounds.origin - point(px(0.), px(6.));
                    let text_width = gpui_kit::base::plot::label::measure_text_width(
                        &text.clone().into(),
                        px(10.),
                        window,
                    );
                    if text_width + px(8.).as_f32() > width * plot.size.width.as_f32() {
                        continue;
                    }
                    PlotLabel::new(vec![
                        PlotText::new(text.clone(), origin, *color)
                            .font_size(px(10.))
                            .align(TextAlign::Center),
                    ])
                    .paint(&bounds, window, cx);
                }
            }
        }
    });
    PlotLabel::new(labels).paint(&bounds, window, cx);
}

fn path(points: &[Point<Pixels>], color: Hsla, fill: bool, window: &mut Window) {
    if points.len() < 2 {
        return;
    }
    if !fill && points.len() > 4096 {
        // Lyon's GPUI adapter uses u16 vertex indices. Bound each stroke even
        // when an unusually noisy series cannot be simplified further.
        for start in (0..points.len() - 1).step_by(4095) {
            path(
                &points[start..points.len().min(start + 4096)],
                color,
                false,
                window,
            );
        }
        return;
    }
    let mut builder = if fill {
        PathBuilder::fill()
    } else {
        PathBuilder::stroke(px(1.6))
    };
    builder.move_to(points[0]);
    for p in &points[1..] {
        builder.line_to(*p);
    }
    if fill {
        builder.close();
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// Fixed screen-space dashes, continuous through corners and irregular samples.
fn dashed_path(points: &[Point<Pixels>], color: Hsla, bounds: Bounds<Pixels>, window: &mut Window) {
    let mut builder = PathBuilder::stroke(px(1.6));
    let mut segments = 0;
    let mut phase = 0.;
    let dash = px(7.).as_f32();
    let period = dash + px(5.).as_f32();
    for pair in points.windows(2) {
        let delta = pair[1] - pair[0];
        let len = delta.x.as_f32().hypot(delta.y.as_f32());
        if !len.is_finite() || len == 0. {
            continue;
        }
        let a = [
            (pair[0].x - bounds.origin.x).as_f32() / bounds.size.width.as_f32(),
            (pair[0].y - bounds.origin.y).as_f32() / bounds.size.height.as_f32(),
        ];
        let b = [
            (pair[1].x - bounds.origin.x).as_f32() / bounds.size.width.as_f32(),
            (pair[1].y - bounds.origin.y).as_f32() / bounds.size.height.as_f32(),
        ];
        let Some((enter, leave)) = clip_segment(a, b) else {
            phase = (phase + len) % period;
            continue;
        };
        let previous_phase = phase;
        phase = (phase + enter * len) % period;
        let visible_start = pair[0] + delta * enter;
        let unit = delta * (1. / len);
        let mut done = 0.;
        let end = (leave - enter) * len;
        while done < end {
            let step = (if phase < dash {
                dash - phase
            } else {
                period - phase
            })
            .min(end - done)
            .max(0.001);
            if phase < dash {
                // All dash segments share one native path, rather than one
                // scene allocation and GPU draw entry for every tiny segment.
                builder.move_to(visible_start + unit * done);
                builder.line_to(visible_start + unit * (done + step));
                segments += 1;
                if segments == 1024 {
                    let batch = std::mem::replace(&mut builder, PathBuilder::stroke(px(1.6)));
                    if let Ok(path) = batch.build() {
                        window.paint_path(path, color);
                    }
                    segments = 0;
                }
            }
            done += step;
            phase = (phase + step) % period;
        }
        phase = (previous_phase + len) % period;
    }
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

/// Preserve the first, last, minimum and maximum in each consecutive pixel
/// column. No smoothing: short braking spikes and vertical gear steps survive.
/// Runs are already separated at missing samples, and non-monotonic x is safe.
fn simplify_path(points: &[[f32; 2]], width: f32, height: f32) -> Vec<[f32; 2]> {
    let points = pixel_envelope(points, width);
    if points.len() < 3 {
        return points;
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut pending = vec![(0, points.len() - 1)];
    let mut budget = points.len().saturating_mul(32);
    while let Some((start, end)) = pending.pop() {
        // Adversarial zigzags must not turn simplification into quadratic work.
        // Keeping the remaining samples preserves accuracy when the budget ends.
        let work = end - start;
        if work > budget {
            keep[start..=end].fill(true);
            continue;
        }
        budget -= work;
        let a = points[start];
        let b = points[end];
        let dx = (b[0] - a[0]) * width;
        let dy = (b[1] - a[1]) * height;
        let length2 = dx * dx + dy * dy;
        let mut farthest = None;
        let mut max_error = 0.35_f32.powi(2);
        for (i, p) in points.iter().enumerate().take(end).skip(start + 1) {
            let x = (p[0] - a[0]) * width;
            let y = (p[1] - a[1]) * height;
            let t = if length2 == 0. {
                0.
            } else {
                ((x * dx + y * dy) / length2).clamp(0., 1.)
            };
            let error = (x - t * dx).powi(2) + (y - t * dy).powi(2);
            if error > max_error {
                max_error = error;
                farthest = Some(i);
            }
        }
        if let Some(i) = farthest {
            keep[i] = true;
            pending.push((start, i));
            pending.push((i, end));
        }
    }
    points
        .into_iter()
        .zip(keep)
        .filter_map(|(p, keep)| keep.then_some(p))
        .collect()
}

fn pixel_envelope(points: &[[f32; 2]], width: f32) -> Vec<[f32; 2]> {
    let mut out = Vec::with_capacity(points.len().min((width.max(1.) as usize + 2) * 4));
    let mut start = 0;
    while start < points.len() {
        let column = (points[start][0] * width).floor();
        let mut end = start + 1;
        let mut lo = start;
        let mut hi = start;
        while end < points.len() && (points[end][0] * width).floor() == column {
            if points[end][1] < points[lo][1] {
                lo = end;
            }
            if points[end][1] > points[hi][1] {
                hi = end;
            }
            end += 1;
        }
        let mut indices = [start, lo, hi, end - 1];
        indices.sort_unstable();
        let mut previous = usize::MAX;
        for i in indices {
            if i != previous && out.last() != Some(&points[i]) {
                out.push(points[i]);
            }
            previous = i;
        }
        start = end;
    }
    out
}

/// Liang–Barsky clipping keeps dashed work proportional to visible geometry.
fn clip_segment(a: [f32; 2], b: [f32; 2]) -> Option<(f32, f32)> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let mut enter = 0_f32;
    let mut leave = 1_f32;
    for (p, q) in [(-d[0], a[0]), (d[0], 1. - a[0]), (-d[1], a[1]), (d[1], 1. - a[1])] {
        if p == 0. {
            if q < 0. {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0. {
                enter = enter.max(r);
            } else {
                leave = leave.min(r);
            }
            if enter > leave {
                return None;
            }
        }
    }
    Some((enter, leave))
}

#[cfg(test)]
mod tests {
    use super::{Mark, Scene, domains};
    use swofty_core::view::*;
    #[test]
    fn dense_straights_collapse_but_braking_spikes_and_steps_survive() {
        let mut points: Vec<_> = (0..100_000).map(|i| [i as f32 / 99_999., 0.5]).collect();
        assert_eq!(
            super::simplify_path(&points, 1000., 200.),
            vec![points[0], points[99_999]]
        );
        points[50_000][1] = 0.1;
        let simple = super::simplify_path(&points, 1000., 200.);
        assert!(simple.contains(&points[50_000]));
        assert!(simple.len() < 12);
        let step = [[0., 1.], [0.5, 1.], [0.5, 0.], [1., 0.]];
        assert_eq!(super::simplify_path(&step, 1000., 200.), step);
    }

    #[test]
    fn pixel_envelope_keeps_extrema_in_sample_order_and_handles_backtracking() {
        let points = [
            [0., 0.5],
            [0.0001, 0.1],
            [0.0002, 0.9],
            [0.0003, 0.5],
            [0.8, 0.2],
            [0., 0.7],
        ];
        assert_eq!(super::pixel_envelope(&points, 100.), points);
        assert!(super::simplify_path(&[], 100., 100.).is_empty());
        assert_eq!(super::simplify_path(&[[0., 0.]], 100., 100.), [[0., 0.]]);
    }
    #[test]
    fn dashed_work_is_clipped_to_the_visible_plot() {
        assert_eq!(super::clip_segment([-100., 0.5], [100., 0.5]), Some((0.5, 0.505)));
        assert_eq!(super::clip_segment([-2., -1.], [2., -1.]), None);
        assert_eq!(super::clip_segment([0., 0.], [1., 1.]), Some((0., 1.)));
    }
    fn chart(data: ChartData) -> Chart {
        Chart {
            id: "test".into(),
            title: String::new(),
            caption: String::new(),
            height: 300.,
            x: ChartAxis::default(),
            y: ChartAxis::default(),
            series: vec![ChartSeries {
                name: "driver".into(),
                color: None,
                data,
            }],
        }
    }
    #[test]
    fn missing_points_split_paths_and_step_reaches_final_value() {
        let c = chart(ChartData::Line {
            points: vec![
                Some(ChartPoint { x: 0., y: 0. }),
                Some(ChartPoint { x: 1., y: 1. }),
                None,
                Some(ChartPoint { x: 3., y: 2. }),
                Some(ChartPoint { x: 9., y: 4. }),
            ],
            dashed: false,
            interpolation: ChartInterpolation::StepAfter,
            dots: false,
        });
        let scene = Scene::new(&c);
        let paths: Vec<_> = scene
            .marks
            .iter()
            .filter_map(|m| {
                if let Mark::Path { points, .. } = m {
                    Some(points)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].len(), 3);
        assert_eq!(*paths[1].last().unwrap(), [1., 0.]);
        // Numeric distance spacing: x=3 is one third, not the third category.
        assert!((paths[1][0][0] - 1. / 3.).abs() < 0.0001);
    }
    #[test]
    fn bars_include_baseline_and_reversed_axis_is_respected() {
        let mut c = chart(ChartData::Bars {
            values: vec![ChartBar {
                position: 1.,
                value: 25.,
                base: 0.,
            }],
            width: 0.8,
            horizontal: true,
        });
        c.y.reversed = true;
        let (x, y) = domains(&c);
        assert_eq!(x.low, 0.);
        assert_eq!(x.high, 25.);
        assert_eq!(y.map(y.low), 1.);
    }
}
