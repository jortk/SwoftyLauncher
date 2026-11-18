//! Native, data-only charts. Coordinates are numeric, including categorical indices.
//! Helpers should send these blocks only when `Hello.capabilities` contains
//! `view.chart`. Older launchers cannot deserialize the new block variant.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chart {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub caption: String,
    #[serde(default = "default_height")]
    pub height: f32,
    #[serde(default)]
    pub x: ChartAxis,
    #[serde(default)]
    pub y: ChartAxis,
    #[serde(default)]
    pub series: Vec<ChartSeries>,
}

fn default_height() -> f32 {
    300.
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChartAxis {
    #[serde(default)]
    pub label: String,
    /// Explicit limits. Otherwise inferred from all series; equal limits are expanded.
    #[serde(default)]
    pub domain: Option<[f64; 2]>,
    #[serde(default)]
    pub reversed: bool,
    #[serde(default)]
    pub format: ChartFormat,
    /// Custom ticks for categories. Empty means automatically spaced numeric ticks.
    #[serde(default)]
    pub ticks: Vec<ChartTick>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartTick {
    pub value: f64,
    pub label: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartFormat {
    #[default]
    Number,
    Integer,
    Percent,
    Seconds,
    SignedSeconds,
    LapTime,
    Duration,
}

impl ChartFormat {
    /// Time values in the chart protocol are seconds, not milliseconds.
    pub fn format(self, value: f64) -> String {
        if !value.is_finite() {
            return "—".into();
        }
        match self {
            Self::Number => format!("{value:.2}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned(),
            Self::Integer => format!("{value:.0}"),
            Self::Percent => format!("{value:.0}%"),
            Self::Seconds => format!("{value:.2}s"),
            Self::SignedSeconds => format!("{value:+.2}s"),
            Self::LapTime => {
                let ms = (value.abs() * 1000.).round() as u64;
                format!(
                    "{}{}:{:02}.{:03}",
                    if value < 0. { "−" } else { "" },
                    ms / 60000,
                    ms / 1000 % 60,
                    ms % 1000
                )
            }
            Self::Duration => {
                let s = value.abs().round() as u64;
                format!(
                    "{}{:02}:{:02}:{:02}",
                    if value < 0. { "−" } else { "" },
                    s / 3600,
                    s / 60 % 60,
                    s % 60
                )
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartBandPoint {
    pub x: f64,
    pub low: f64,
    pub high: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartBar {
    pub position: f64,
    pub value: f64,
    #[serde(default)]
    pub base: f64,
}

/// An interval/tile in axis coordinates. Suitable for tyre timelines and heatmaps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartCell {
    pub x: [f64; 2],
    pub y: [f64; 2],
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartBox {
    pub position: f64,
    pub min: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartInterpolation {
    #[default]
    Linear,
    StepAfter,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartSeries {
    pub name: String,
    /// CSS-style #RRGGBB or #RRGGBBAA. Omit to use the launcher's chart palette.
    #[serde(default)]
    pub color: Option<String>,
    #[serde(flatten)]
    pub data: ChartData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChartData {
    Line {
        /// Null explicitly breaks a line; it is never joined across missing samples.
        points: Vec<Option<ChartPoint>>,
        #[serde(default)]
        dashed: bool,
        #[serde(default)]
        interpolation: ChartInterpolation,
        #[serde(default)]
        dots: bool,
    },
    Scatter {
        points: Vec<ChartPoint>,
        #[serde(default)]
        hollow: bool,
    },
    Band {
        points: Vec<Option<ChartBandPoint>>,
    },
    Bars {
        values: Vec<ChartBar>,
        width: f64,
        #[serde(default)]
        horizontal: bool,
    },
    Ranges {
        cells: Vec<ChartCell>,
    },
    Heatmap {
        cells: Vec<ChartCell>,
    },
    Boxes {
        values: Vec<ChartBox>,
        width: f64,
    },
}

fn valid_color(value: &Option<String>) -> bool {
    value.as_ref().is_none_or(|s| {
        (s.len() == 7 || s.len() == 9) && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
    })
}

// Keep arithmetic and tessellation bounded even for untrusted helper payloads.
fn coordinate(value: f64) -> bool {
    value.is_finite() && value.abs() <= 1e12
}

impl Chart {
    /// Bounded validation is shared by Rust addons and the native renderer. Bad chart
    /// data becomes an inline error; it must not take down the helper or UI.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty() || self.id.len() > 256 {
            return Err("Chart needs an id (1–256 bytes).".into());
        }
        if !self.height.is_finite() || !(120. ..=1600.).contains(&self.height) {
            return Err("Chart height must be between 120 and 1600.".into());
        }
        if self.series.len() > 128 {
            return Err("Chart supports at most 128 series.".into());
        }
        if self.title.len() > 512 || self.caption.len() > 2048 {
            return Err("Chart title or caption is too long.".into());
        }
        for a in [&self.x, &self.y] {
            if a.ticks.len() > 1024
                || a.label.len() > 128
                || a.ticks
                    .iter()
                    .any(|t| !coordinate(t.value) || t.label.len() > 128)
                || a.domain.is_some_and(|[a, b]| {
                    !coordinate(a) || !coordinate(b) || a > b || (a != b && b - a < 1e-9)
                })
            {
                return Err("Invalid chart axis.".into());
            }
        }
        let mut count = 0;
        for s in &self.series {
            if s.name.len() > 128 {
                return Err("Chart series name is too long.".into());
            }
            if !valid_color(&s.color) {
                return Err("Chart colours must use #RRGGBB or #RRGGBBAA.".into());
            }
            let ok = match &s.data {
                ChartData::Line { points, .. } => {
                    count += points.len();
                    points
                        .iter()
                        .flatten()
                        .all(|p| coordinate(p.x) && coordinate(p.y))
                }
                ChartData::Scatter { points, .. } => {
                    count += points.len();
                    points.iter().all(|p| coordinate(p.x) && coordinate(p.y))
                }
                ChartData::Band { points } => {
                    count += points.len();
                    points.iter().flatten().all(|p| {
                        coordinate(p.x) && coordinate(p.low) && coordinate(p.high) && p.low <= p.high
                    })
                }
                ChartData::Bars { values, width, .. } => {
                    count += values.len();
                    coordinate(*width)
                        && *width > 0.
                        && values
                            .iter()
                            .all(|p| coordinate(p.position) && coordinate(p.value) && coordinate(p.base))
                }
                ChartData::Ranges { cells } | ChartData::Heatmap { cells } => {
                    count += cells.len();
                    cells.iter().all(|c| {
                        c.x.into_iter().chain(c.y).all(coordinate)
                            && c.label.len() <= 128
                            && c.x[0] <= c.x[1]
                            && c.y[0] <= c.y[1]
                            && valid_color(&c.color)
                    })
                }
                ChartData::Boxes { values, width } => {
                    count += values.len();
                    coordinate(*width)
                        && *width > 0.
                        && values.iter().all(|b| {
                            [b.position, b.min, b.q1, b.median, b.q3, b.max]
                                .into_iter()
                                .all(coordinate)
                                && b.min <= b.q1
                                && b.q1 <= b.median
                                && b.median <= b.q3
                                && b.q3 <= b.max
                        })
                }
            };
            if !ok {
                return Err("Chart contains non-finite or unordered values.".into());
            }
        }
        if count > 100_000 {
            return Err("Chart supports at most 100,000 samples.".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Chart {
        serde_json::from_str(r##"{"id":"speed","series":[{"name":"Driver","type":"line","points":[{"x":0,"y":10},null,{"x":9,"y":20}],"dashed":true}]}"##).unwrap()
    }
    #[test]
    fn protocol_round_trip_preserves_breaks() {
        let chart = sample();
        chart.validate().unwrap();
        let block = super::super::Block::Chart(chart);
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("null"));
        assert_eq!(block, serde_json::from_str(&json).unwrap());
    }
    #[test]
    fn bad_values_are_rejected() {
        let mut c = sample();
        c.x.domain = Some([2., 1.]);
        assert!(c.validate().is_err());
        c.x.domain = None;
        c.series[0].color = Some("red".into());
        assert!(c.validate().is_err());
        c.series[0].color = None;
        c.height = f32::NAN;
        assert!(c.validate().is_err());
    }
    #[test]
    fn time_labels_round_at_boundaries() {
        assert_eq!(ChartFormat::LapTime.format(79.561), "1:19.561");
        assert_eq!(ChartFormat::LapTime.format(59.9996), "1:00.000");
        assert_eq!(ChartFormat::Duration.format(3723.), "01:02:03");
        assert_eq!(ChartFormat::SignedSeconds.format(-1.25), "-1.25s");
    }
}
