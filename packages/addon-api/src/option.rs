use serde_json::Value;

use crate::manifest::{AddonOption, OptionType};

pub const MAX_ID_LEN: usize = 40;
pub const MAX_SLIDER_STEPS: usize = 100;
pub const MAX_CHOICES: usize = 16;

pub fn valid_option_id(id: &str) -> bool {
    id.len() <= MAX_ID_LEN
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub value: Value,
    pub text: String,
}

struct Grid {
    min: f64,
    step: f64,
    steps: usize,
    integer: bool,
    decimals: usize,
}

fn decimals(x: f64) -> usize {
    format!("{x}").split_once('.').map_or(0, |(_, f)| f.len())
}

impl Grid {
    fn value(&self, k: usize) -> f64 {
        let v = self.min + k as f64 * self.step;
        if self.integer {
            return v.round();
        }
        let scale = 10f64.powi(self.decimals as i32);
        (v * scale).round() / scale
    }

    fn json(&self, v: f64) -> Value {
        if self.integer {
            Value::from(v as i64)
        } else {
            serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
        }
    }

    fn text(&self, v: f64) -> String {
        if self.integer {
            format!("{}", v as i64)
        } else {
            format!("{v:.*}", self.decimals)
        }
    }
}

impl AddonOption {
    pub fn title(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.id)
    }

    pub(crate) fn validate(&self) -> std::result::Result<(), String> {
        if !valid_option_id(&self.id) {
            return Err(format!(
                "id must match [a-z][a-z0-9_]* and be at most {MAX_ID_LEN} characters"
            ));
        }
        if self.id == "addon" {
            return Err("the id addon is reserved: an op's when.addon names addons".into());
        }
        if !self.needs.is_empty() && self.kind != OptionType::Toggle {
            return Err("needs is only for a toggle".into());
        }
        let range = self.min.is_some() || self.max.is_some() || self.step.is_some();
        match self.kind {
            OptionType::Toggle => {
                if range || !self.values.is_empty() {
                    return Err("a toggle takes no values, min, max or step".into());
                }
                if !self.default.is_boolean() {
                    return Err("default must be true or false".into());
                }
            }
            OptionType::Slider => {
                if !self.values.is_empty() {
                    return Err("a slider takes min, max and step, not values".into());
                }
                self.grid()?;
                if self.index(&self.default).is_none() {
                    return Err(format!("default {} is not on the slider's grid", self.default));
                }
            }
            OptionType::Choice => {
                if range {
                    return Err("a choice takes values, not min, max or step".into());
                }
                if self.values.is_empty() || self.values.len() > MAX_CHOICES {
                    return Err(format!("a choice needs 1 to {MAX_CHOICES} values"));
                }
                for (i, c) in self.values.iter().enumerate() {
                    if c.value.is_empty() {
                        return Err("choice values must not be empty".into());
                    }
                    if self.values[..i].iter().any(|d| d.value == c.value) {
                        return Err(format!("value {} is listed twice", c.value));
                    }
                }
                if self.index(&self.default).is_none() {
                    return Err(format!("default {} is not one of the values", self.default));
                }
            }
        }
        Ok(())
    }

    fn grid(&self) -> std::result::Result<Grid, String> {
        let (Some(min), Some(max), Some(step)) = (self.min, self.max, self.step) else {
            return Err("a slider needs min, max and step".into());
        };
        if !(min.is_finite() && max.is_finite() && step.is_finite()) || min >= max || step <= 0.0 {
            return Err("a slider needs min < max and step > 0".into());
        }
        let span = (max - min) / step;
        let steps = span.round();
        if (span - steps).abs() > 1e-6 {
            return Err("max - min must be a whole number of steps".into());
        }
        if steps > MAX_SLIDER_STEPS as f64 {
            return Err(format!("a slider has at most {MAX_SLIDER_STEPS} steps"));
        }
        Ok(Grid {
            min,
            step,
            steps: steps as usize,
            integer: [min, max, step].iter().all(|v| v.fract() == 0.0),
            decimals: decimals(step).max(decimals(min)),
        })
    }

    pub fn range(&self) -> Option<(f64, f64, f64)> {
        self.grid().ok().map(|g| (g.min, g.value(g.steps), g.step))
    }

    pub fn states(&self) -> Vec<State> {
        match self.kind {
            OptionType::Toggle => vec![
                State {
                    value: false.into(),
                    text: "OFF".into(),
                },
                State {
                    value: true.into(),
                    text: "ON".into(),
                },
            ],
            OptionType::Slider => {
                let Ok(g) = self.grid() else {
                    return Vec::new();
                };
                (0..=g.steps)
                    .map(|k| {
                        let v = g.value(k);
                        State {
                            value: g.json(v),
                            text: g.text(v),
                        }
                    })
                    .collect()
            }
            OptionType::Choice => self
                .values
                .iter()
                .map(|c| State {
                    value: c.value.clone().into(),
                    text: c.label.clone().unwrap_or_else(|| c.value.clone()),
                })
                .collect(),
        }
    }

    pub fn index(&self, v: &Value) -> Option<usize> {
        match self.kind {
            OptionType::Toggle => v.as_bool().map(usize::from),
            OptionType::Slider => {
                let g = self.grid().ok()?;
                let x = v.as_f64()?;
                let k = ((x - g.min) / g.step).round();
                let ok = k >= 0.0 && k <= g.steps as f64 && (g.value(k as usize) - x).abs() <= g.step * 1e-6;
                ok.then_some(k as usize)
            }
            OptionType::Choice => {
                let s = v.as_str()?;
                self.values.iter().position(|c| c.value == s)
            }
        }
    }

    pub fn normalise(&self, v: &Value) -> Option<Value> {
        let i = self.index(v)?;
        match self.kind {
            OptionType::Toggle => Some(Value::Bool(i == 1)),
            OptionType::Slider => self.grid().ok().map(|g| g.json(g.value(i))),
            OptionType::Choice => Some(Value::String(self.values[i].value.clone())),
        }
    }

    pub fn default_value(&self) -> Value {
        self.normalise(&self.default)
            .unwrap_or_else(|| self.default.clone())
    }

    pub fn text(&self, v: &Value) -> String {
        match (self.kind, self.index(v)) {
            (OptionType::Toggle, Some(i)) => (if i == 1 { "On" } else { "Off" }).into(),
            (_, Some(i)) => self.states().swap_remove(i).text,
            (_, None) => v.to_string(),
        }
    }

    pub fn placeholder(&self, v: &Value) -> String {
        match v {
            Value::String(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            _ => self.text(v),
        }
    }
}
