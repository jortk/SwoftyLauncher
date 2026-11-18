//! Standalone gallery using the same renderer as addon Chart blocks. No game,
//! profile, helper, or application store is opened in this mode.
use crate::{
    blocks::chart,
    scale::px,
    theme::{self, ScrollArea},
};
use gpui_kit::component::{TitleBar, h_flex, v_flex};
use gpui_kit::*;
use swofty_core::view::*;
use swofty_i18n::tr;

const GROUPS: [&str; 5] = [
    "Lines & bands",
    "Bars & ranges",
    "Distribution",
    "Sector heatmap",
    "Edge cases",
];

enum Probe {
    Single(Vec<Chart>),
    Pages(Vec<Vec<Chart>>),
}
impl Probe {
    fn decode(bytes: &[u8]) -> serde_json::Result<Self> {
        // Do not use an untagged enum here: serde would buffer the entire
        // nested sample tree before attempting either variant.
        if bytes.iter().copied().filter(|b| !b.is_ascii_whitespace()).nth(1) == Some(b'[') {
            serde_json::from_slice(bytes).map(Self::Pages)
        } else {
            serde_json::from_slice(bytes).map(Self::Single)
        }
    }
    fn charts(&self, tick: usize) -> &[Chart] {
        match self {
            Self::Single(charts) => charts,
            Self::Pages(pages) if !pages.is_empty() => &pages[(tick / 30) % pages.len()],
            Self::Pages(_) => &[],
        }
    }
}

pub(crate) fn run() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            theme::init(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1280.), px(900.)),
                    cx,
                ))),
                window_min_size: Some(size(px(600.), px(500.))),
                app_id: Some("net.swofty.NativeChartsDemo".into()),
                titlebar: Some(TitlebarOptions {
                    title: Some("Swofty — Native charts".into()),
                    ..TitleBar::title_bar_options()
                }),
                ..TitleBar::window_options()
            };
            let (main, _) = gpui_kit::open_window(options, cx, |_, cx| {
                cx.new(|cx: &mut Context<Gallery>| {
                    let probe = std::env::var("SWOFTY_CHART_PROBE").ok().map(|path| {
                        Probe::decode(&std::fs::read(path).expect("read probe")).expect("decode probe charts")
                    });
                    if probe.is_some() {
                        cx.spawn(async move |this, cx| {
                            loop {
                                cx.background_executor()
                                    .timer(std::time::Duration::from_millis(100))
                                    .await;
                                if this
                                    .update(cx, |this, cx| {
                                        this.tick += 1;
                                        cx.notify();
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        })
                        .detach();
                    }
                    Gallery {
                        group: 0,
                        alternate: false,
                        probe,
                        tick: 0,
                    }
                })
            })
            .expect("open native chart gallery");
            cx.on_window_closed(move |cx, id| {
                if id == main.window_id() {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
}

struct Gallery {
    group: usize,
    alternate: bool,
    probe: Option<Probe>,
    tick: usize,
}
impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let tabs = h_flex()
            .flex_wrap()
            .gap(px(8.))
            .children(GROUPS.into_iter().enumerate().map(|(i, name)| {
                div()
                    .id(("gallery-tab", i))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .bg(if self.group == i { p.accent } else { p.elevated })
                    .cursor_pointer()
                    .child(tr(name))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.group = i;
                        cx.notify();
                    }))
            }))
            .child(
                div()
                    .id("gallery-data")
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .bg(p.elevated)
                    .cursor_pointer()
                    .child(tr("Change sample data"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.alternate = !this.alternate;
                        cx.notify();
                    })),
            );
        let examples;
        let charts = if let Some(probe) = &self.probe {
            probe.charts(self.tick)
        } else {
            examples = self::examples(self.group, self.alternate);
            &examples
        };
        v_flex()
            .size_full()
            .bg(p.background)
            .text_color(p.text)
            .child(TitleBar::new().bg(p.chrome))
            .child(
                v_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .scroll_y("chart-gallery-scroll")
                    .p(px(24.))
                    .gap(px(16.))
                    .child(
                        div()
                            .text_size(px(24.))
                            .font_weight(FontWeight::BOLD)
                            .child(tr("Native charts")),
                    )
                    .child(div().text_color(p.text_muted).child(tr(
                        "Rendered directly by GPUI. Sample data only. No chart image files are created.",
                    )))
                    .child(tabs)
                    .children(
                        charts
                            .iter()
                            .map(|c| chart::render(format!("gallery/{}", c.id), c, p, None)),
                    ),
            )
    }
}

fn axis(label: &str, format: ChartFormat) -> ChartAxis {
    ChartAxis {
        label: label.into(),
        format,
        ..Default::default()
    }
}
fn categories(names: &[&str]) -> ChartAxis {
    ChartAxis {
        domain: Some([-0.5, names.len() as f64 - 0.5]),
        ticks: names
            .iter()
            .enumerate()
            .map(|(i, n)| ChartTick {
                value: i as f64,
                label: (*n).into(),
            })
            .collect(),
        ..Default::default()
    }
}
fn series(name: &str, color: &str, data: ChartData) -> ChartSeries {
    ChartSeries {
        name: name.into(),
        color: Some(color.into()),
        data,
    }
}
fn base(id: &str, title: &str) -> Chart {
    Chart {
        id: id.into(),
        title: title.into(),
        caption: String::new(),
        height: 300.,
        x: ChartAxis::default(),
        y: ChartAxis::default(),
        series: vec![],
    }
}
fn line(points: Vec<Option<ChartPoint>>, dashed: bool) -> ChartData {
    ChartData::Line {
        points,
        dashed,
        interpolation: ChartInterpolation::Linear,
        dots: true,
    }
}

fn examples(group: usize, alternate: bool) -> Vec<Chart> {
    let offset = if alternate { 0.7 } else { 0. };
    match group {
        0 => {
            let mut pace = base(
                "pace",
                "Pace versus field · numeric lap axis, band, missing laps & dashed pit segment",
            );
            pace.x = axis("Lap", ChartFormat::Integer);
            pace.y = axis("Versus field median (s)", ChartFormat::SignedSeconds);
            pace.y.domain = Some([-3., 3.]);
            pace.y.reversed = true;
            pace.series.push(series(
                "Middle 50% of field",
                "#99AABA30",
                ChartData::Band {
                    points: (1..=30)
                        .map(|i| {
                            Some(ChartBandPoint {
                                x: i as f64,
                                low: -0.5 - (i as f64 / 5.).sin() * 0.2,
                                high: 0.6 + (i as f64 / 4.).cos() * 0.2,
                            })
                        })
                        .collect(),
                },
            ));
            pace.series.push(series(
                "Field median",
                "#D5DAE0",
                line(
                    vec![
                        Some(ChartPoint { x: 1., y: 0. }),
                        Some(ChartPoint { x: 30., y: 0. }),
                    ],
                    false,
                ),
            ));
            for (i, (name, col)) in [("NOR", "#FF9B24"), ("VER", "#57B7FF"), ("HAM", "#F26787")]
                .iter()
                .enumerate()
            {
                pace.series.push(series(
                    name,
                    col,
                    line(
                        (1..=30)
                            .map(|lap| {
                                if lap == 12 {
                                    None
                                } else {
                                    Some(ChartPoint {
                                        x: lap as f64,
                                        y: (lap as f64 / 4. + i as f64).sin() * 0.65 - i as f64 * 0.35
                                            + offset,
                                    })
                                }
                            })
                            .collect(),
                        i == 2,
                    ),
                ));
            }
            let mut speed = base("speed", "Speed trace · irregular distance samples");
            speed.x = axis("Lap distance (m)", ChartFormat::Integer);
            speed.y = axis("Speed (km/h)", ChartFormat::Integer);
            speed.series.push(series(
                "Driver A",
                "#57B7FF",
                line(
                    [0., 15., 25., 160., 250., 580., 600., 610., 900., 1300.]
                        .into_iter()
                        .map(|x| {
                            Some(ChartPoint {
                                x,
                                y: 160. + (x / 170_f64).sin() * 100. + offset * 25.,
                            })
                        })
                        .collect(),
                    false,
                ),
            ));
            let mut gear = base("gear", "Gear · step interpolation");
            gear.x = axis("Distance (m)", ChartFormat::Integer);
            gear.y = axis("Gear", ChartFormat::Integer);
            gear.height = 210.;
            gear.series.push(series(
                "Gear",
                "#28CBB2",
                ChartData::Line {
                    points: (0..10)
                        .map(|i| {
                            Some(ChartPoint {
                                x: i as f64 * 100.,
                                y: (i % 7 + 2) as f64,
                            })
                        })
                        .collect(),
                    dashed: false,
                    interpolation: ChartInterpolation::StepAfter,
                    dots: false,
                },
            ));
            vec![pace, speed, gear]
        }
        1 => {
            let mut pit = base("pits", "Average pit lane duration · horizontal lollipops");
            pit.x = axis("Seconds", ChartFormat::Seconds);
            pit.x.domain = Some([22., 27.]);
            pit.y = categories(&["NOR", "VER", "HAM"]);
            pit.y.reversed = true;
            for (i, col) in ["#FF9B24", "#57B7FF", "#F26787"].iter().enumerate() {
                let y = i as f64;
                let x = 23.6 + y * 0.8 + offset;
                pit.series.push(series(
                    ["NOR", "VER", "HAM"][i],
                    col,
                    line(
                        vec![Some(ChartPoint { x: 22., y }), Some(ChartPoint { x, y })],
                        false,
                    ),
                ));
            }
            let mut tyres = base("tyres", "Tyre strategy · labelled range timeline");
            tyres.x = axis("Lap", ChartFormat::Integer);
            tyres.y = categories(&["NOR", "VER", "HAM"]);
            tyres.y.reversed = true;
            tyres.height = 230.;
            tyres.series.push(series(
                "Stints",
                "#FFFFFF",
                ChartData::Ranges {
                    cells: (0..3)
                        .flat_map(|i| {
                            let split = 8. + i as f64 * 3. + offset;
                            [
                                ChartCell {
                                    x: [1., split],
                                    y: [i as f64 - 0.3, i as f64 + 0.3],
                                    color: Some("#E4C957".into()),
                                    label: "C4".into(),
                                },
                                ChartCell {
                                    x: [split + 0.15, 30.],
                                    y: [i as f64 - 0.3, i as f64 + 0.3],
                                    color: Some("#B7C5D8".into()),
                                    label: "C3".into(),
                                },
                            ]
                        })
                        .collect(),
                },
            ));
            let mut bars = base("bars", "Stint length by compound · stacked columns");
            bars.x = categories(&["NOR", "VER", "HAM"]);
            bars.y = axis("Laps", ChartFormat::Integer);
            for (j, col) in ["#E4C957", "#B7C5D8"].iter().enumerate() {
                bars.series.push(series(
                    ["C4", "C3"][j],
                    col,
                    ChartData::Bars {
                        values: (0..3)
                            .map(|i| {
                                let split = 8. + i as f64 * 3.;
                                ChartBar {
                                    position: i as f64,
                                    base: if j == 0 { 0. } else { split },
                                    value: if j == 0 { split } else { 30. },
                                }
                            })
                            .collect(),
                        width: 0.55,
                        horizontal: false,
                    },
                ));
            }
            let mut h = base("h2h", "Teammate head-to-head · horizontal bars");
            h.x = axis("Gap (s)", ChartFormat::Seconds);
            h.y = categories(&["NOR › PIA", "VER › HAD", "HAM › LEC"]);
            h.y.reversed = true;
            h.series.push(series(
                "Best lap gap",
                "#57B7FF",
                ChartData::Bars {
                    values: (0..3)
                        .map(|i| ChartBar {
                            position: i as f64,
                            base: 0.,
                            value: 0.12 + i as f64 * 0.15 + offset / 5.,
                        })
                        .collect(),
                    width: 0.5,
                    horizontal: true,
                },
            ));
            vec![pit, tyres, bars, h]
        }
        2 => {
            let mut c = base("boxes", "Lap time distribution · box and whisker");
            c.x = categories(&["NOR", "VER", "HAM"]);
            c.y = axis("Lap time", ChartFormat::LapTime);
            for (i, col) in ["#FF9B24", "#57B7FF", "#F26787"].iter().enumerate() {
                let d = i as f64 + offset;
                c.series.push(series(
                    ["NOR", "VER", "HAM"][i],
                    col,
                    ChartData::Boxes {
                        values: vec![ChartBox {
                            position: i as f64,
                            min: 75. + d,
                            q1: 77. + d,
                            median: 78. + d,
                            q3: 80. + d,
                            max: 84. + d,
                        }],
                        width: 0.45,
                    },
                ));
            }
            let mut stems = base(
                "quali",
                "Qualifying gaps · stems, cutoff and hollow eliminated drivers",
            );
            stems.x = categories(&["NOR", "VER", "HAM", "RUS", "LEC"]);
            stems.y = axis("Gap to pole", ChartFormat::Seconds);
            for i in 0..5 {
                let y = i as f64 * 0.21 + offset / 3.;
                stems.series.push(series(
                    ["NOR", "VER", "HAM", "RUS", "LEC"][i],
                    "#57B7FF",
                    line(
                        vec![
                            Some(ChartPoint { x: i as f64, y: 0. }),
                            Some(ChartPoint { x: i as f64, y }),
                        ],
                        false,
                    ),
                ));
                stems.series.push(series(
                    "",
                    "#57B7FF",
                    ChartData::Scatter {
                        points: vec![ChartPoint { x: i as f64, y }],
                        hollow: i >= 3,
                    },
                ));
            }
            stems.series.push(series(
                "Cutoff",
                "#F26787",
                line(
                    vec![
                        Some(ChartPoint { x: 2.5, y: 0. }),
                        Some(ChartPoint { x: 2.5, y: 1.2 }),
                    ],
                    true,
                ),
            ));
            vec![c, stems]
        }
        3 => {
            let mut c = base("heatmap", "Sector duel · whole-race heatmap");
            c.height = 230.;
            c.x = axis("Lap", ChartFormat::Integer);
            c.y = categories(&["Sector 1", "Sector 2", "Sector 3"]);
            c.y.reversed = true;
            let colors = ["#FF9B24", "#57B7FF", "#28CBB2"];
            for (i, col) in colors.iter().enumerate() {
                c.series.push(series(
                    ["NOR", "VER", "HAM"][i],
                    col,
                    ChartData::Heatmap {
                        cells: (1..=30)
                            .flat_map(|lap| {
                                (0..3).filter_map(move |sector| {
                                    if (lap + sector + usize::from(alternate)) % 3 != i || lap == 12 {
                                        None
                                    } else {
                                        Some(ChartCell {
                                            x: [lap as f64 - 0.45, lap as f64 + 0.45],
                                            y: [sector as f64 - 0.42, sector as f64 + 0.42],
                                            color: None,
                                            label: String::new(),
                                        })
                                    }
                                })
                            })
                            .collect(),
                    },
                ));
            }
            vec![c]
        }
        _ => {
            let empty = base("empty", "Empty chart");
            let mut invalid = base("invalid", "Invalid chart · contained error");
            invalid.x.domain = Some([9., 1.]);
            let mut single = base("single", "Single point · equal domain expansion");
            single.series.push(series(
                "One sample",
                "#28CBB2",
                ChartData::Scatter {
                    points: vec![ChartPoint { x: 4., y: 79.561 }],
                    hollow: false,
                },
            ));
            single.y.format = ChartFormat::LapTime;
            vec![empty, invalid, single]
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn gallery_payloads_validate_and_round_trip() {
        for group in 0..4 {
            for alternate in [false, true] {
                for c in super::examples(group, alternate) {
                    c.validate().unwrap();
                    let json = serde_json::to_string(&c).unwrap();
                    let parsed: super::Chart = serde_json::from_str(&json).unwrap();
                    parsed.validate().unwrap();
                    assert_eq!(c.id, parsed.id);
                    assert_eq!(c.series.len(), parsed.series.len());
                }
            }
        }
    }
}
