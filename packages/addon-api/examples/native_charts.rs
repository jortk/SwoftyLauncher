//! Minimal JSON-lines helper demonstrating native chart blocks on a real addon page.
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use swofty_addon::helper::VIEW_CHART;
use swofty_addon::view::*;

fn main() {
    let mut alternate = false;
    let mut supported = false;
    let fixture = std::env::var_os("SWOFTY_CHART_DEMO_VIEW");
    let mut out = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(call) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(id) = call.get("id") else { continue };
        if supported
            && call["method"] == "view"
            && let Some(path) = &fixture
        {
            let fixture = std::fs::read_to_string(path).expect("read chart demo view fixture");
            eprintln!("native chart fixture served: {} bytes", fixture.len());
            if writeln!(out, "{{\"id\":{id},\"result\":{fixture}}}")
                .and_then(|()| out.flush())
                .is_err()
            {
                break;
            }
            continue;
        }
        let result = match call["method"].as_str() {
            Some("hello") => {
                supported = call["params"]["capabilities"]
                    .as_array()
                    .is_some_and(|caps| caps.iter().any(|c| c.as_str() == Some(VIEW_CHART)));
                json!({"api":1})
            }
            Some("event") => {
                alternate = !alternate;
                serde_json::to_value(Effect::Reload).unwrap()
            }
            Some("tick") => {
                if supported
                    && fixture.is_some()
                    && writeln!(
                        out,
                        "{{\"method\":\"invalidate\",\"params\":{{\"section\":\"charts\"}}}}"
                    )
                    .is_err()
                {
                    break;
                }
                Value::Null
            }
            Some("view") => {
                if supported {
                    serde_json::to_value(view(alternate)).unwrap()
                } else {
                    json!({"subtitle":"Native charts sample","body":{"blocks":[{"text":{"text":"This launcher does not advertise the view.chart capability.","tone":"muted"}}]}})
                }
            }
            _ => Value::Null,
        };
        if writeln!(out, "{}", json!({"id":id,"result":result}))
            .and_then(|()| out.flush())
            .is_err()
        {
            break;
        }
    }
}

fn view(alternate: bool) -> View {
    let chart = Chart {
        id: "native-lap-times".into(),
        title: "Native lap progression".into(),
        caption: "Sample addon data. Change the sample to update this chart without creating an image."
            .into(),
        height: 330.,
        x: ChartAxis {
            label: "Lap".into(),
            format: ChartFormat::Integer,
            ..Default::default()
        },
        y: ChartAxis {
            label: "Lap time".into(),
            format: ChartFormat::LapTime,
            ..Default::default()
        },
        series: vec![ChartSeries {
            name: "Sample driver".into(),
            color: Some("#57B7FF".into()),
            data: ChartData::Line {
                points: (1..=20)
                    .map(|i| {
                        if i == 10 {
                            None
                        } else {
                            Some(ChartPoint {
                                x: i as f64,
                                y: 80. + (i as f64 / 3.).sin() * 1.3 + if alternate { 0.7 } else { 0. },
                            })
                        }
                    })
                    .collect(),
                dashed: false,
                interpolation: ChartInterpolation::Linear,
                dots: true,
            },
        }],
    };
    serde_json::from_value(json!({"subtitle":"Native chart block · helper protocol demo","body":{"blocks":[{"buttons":[{"id":"change-data","label":"Change sample data"}]},{"chart":chart}]}})).unwrap()
}
