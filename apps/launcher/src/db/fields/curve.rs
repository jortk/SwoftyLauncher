use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::{Disableable, h_flex, v_flex};
use gpui_kit::*;
use swofty_i18n::{tr, trf};

use super::FieldModel;
use super::detail::plain;
use super::value::{num, parse_curve};
use crate::shell::Shell;
use crate::theme::Palette;

#[derive(Clone, Copy)]
struct CurveEdit<'a> {
    f: &'a FieldModel,
    key: u64,
    locked: bool,
    n: usize,
}

impl Shell {
    fn db_curve_row(&self, curve: &CurveEdit, i: usize, p: Palette, cx: &mut Context<Self>) -> Div {
        let CurveEdit { f, key, locked, n } = *curve;
        let input = |axis: &str| {
            self.db
                .slots
                .inputs
                .get(&format!("{}/{axis}{i}", f.id))
                .map(|s| Input::new(&s.input).tap_xs().disabled(locked))
        };
        h_flex()
            .gap(px(6.))
            .child(
                div()
                    .w(px(18.))
                    .text_size(px(11.))
                    .text_color(p.text_placeholder)
                    .child((i + 1).to_string()),
            )
            .child(div().flex_1().min_w_0().children(input("x")))
            .child(div().flex_1().min_w_0().children(input("y")))
            .child(
                Button::new(SharedString::from(format!("{}-pt-{i}", f.id)))
                    .ghost()
                    .tap_xs()
                    .icon(IconName::X)
                    .disabled(locked || n <= 2)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.db_curve_edit(
                            key,
                            |pts| {
                                pts.remove(i);
                            },
                            cx,
                        )
                    })),
            )
    }

    pub(super) fn db_curve(
        &self,
        f: &FieldModel,
        key: u64,
        locked: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(c) = parse_curve(&f.value) else {
            return match self.db.slots.inputs.get(&f.id) {
                Some(s) => Input::new(&s.input).tap_sm().disabled(locked).into_any_element(),
                None => plain(&f.value, p),
            };
        };
        let head: Vec<String> = c.head.iter().map(|(a, b)| format!("{a} {b}")).collect();
        let n = c.points.len();
        let pts: Vec<(f32, f32)> = c
            .points
            .iter()
            .filter_map(|(x, y)| Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?)))
            .collect();
        let spark = sparkline(pts, p);
        let edit = CurveEdit { f, key, locked, n };
        let rows = (0..n).map(|i| self.db_curve_row(&edit, i, p, cx));
        v_flex()
            .gap(px(6.))
            .child(
                div()
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(p.border_variant)
                    .bg(p.background)
                    .child(spark),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .text_size(px(11.))
                    .text_color(p.text_placeholder)
                    .child(div().w(px(18.)))
                    .child(div().flex_1().child(tr("X")))
                    .child(div().flex_1().child(trf!(
                        "VALUE {min}–{max}",
                        min = num(c.span().0),
                        max = num(c.span().1)
                    )))
                    .child(div().w(px(22.))),
            )
            .children(rows)
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(
                        Button::new(SharedString::from(format!("{}-add-pt", f.id)))
                            .ghost()
                            .tap_xs()
                            .icon(IconName::Plus)
                            .label(tr("Add point"))
                            .disabled(locked)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.db_curve_edit(
                                    key,
                                    |pts| {
                                        let last = pts.last().cloned().unwrap_or(("1".into(), "1".into()));
                                        pts.push(last);
                                    },
                                    cx,
                                )
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.text_placeholder)
                            .child(head.join(" · ")),
                    ),
            )
            .into_any_element()
    }
}

fn sparkline(pts: Vec<(f32, f32)>, p: Palette) -> Canvas<()> {
    let line = p.accent;
    let grid = p.border_variant;
    canvas(
        |_, _, _| {},
        move |b, _, window, _| {
            let (x0, x1) = pts
                .iter()
                .fold((f32::MAX, f32::MIN), |(a, z), (x, _)| (a.min(*x), z.max(*x)));
            let (y0, y1) = pts
                .iter()
                .fold((0f32, 1f32), |(a, z), (_, y)| (a.min(*y), z.max(*y)));
            let at = |x: f32, y: f32| {
                let fx = if x1 > x0 { (x - x0) / (x1 - x0) } else { 0. };
                let fy = if y1 > y0 { (y - y0) / (y1 - y0) } else { 0. };
                point(
                    b.origin.x + b.size.width * fx,
                    b.origin.y + b.size.height * (1. - fy),
                )
            };
            let mut base = PathBuilder::stroke(px(1.));
            base.move_to(point(b.origin.x, b.origin.y + b.size.height));
            base.line_to(point(b.origin.x + b.size.width, b.origin.y + b.size.height));
            if let Ok(path) = base.build() {
                window.paint_path(path, grid);
            }
            let mut path = PathBuilder::stroke(px(1.5));
            for (i, (x, y)) in pts.iter().enumerate() {
                if i == 0 {
                    path.move_to(at(*x, *y));
                } else {
                    path.line_to(at(*x, *y));
                }
            }
            if pts.len() > 1
                && let Ok(path) = path.build()
            {
                window.paint_path(path, line);
            }
        },
    )
    .w_full()
    .h(px(44.))
}
