use swofty_formats::bxml::{Document, Element, Kind};

use crate::Result;

/// A wheel's render position, visual height or tyre size that moves by more than this many metres from the car it
/// replaces. The car's tyre models sit where its own handling file puts them: a render position 30 mm off left
/// every tyre of F1 1990 invisible in Time Trial.
pub const TOLERANCE: f64 = 0.005;

/// Marks a line a cached build log carries as a warning, not a log line.
pub const WARNING_PREFIX: &str = "warning: ";

const CHASSIS_FIELDS: &[&str] = &["FrontVisualHeightOffset", "RearVisualHeightOffset"];
const SUSPENSION_FIELDS: &[&str] = &[
    "LocalRenderPositionX",
    "LocalRenderPositionY",
    "LocalRenderPositionZ",
    "RideHeightRenderOffset",
];
const TYRE_FIELDS: &[&str] = &["TyreWidth", "TyreRadius", "AlloyRadius"];
const FLAGS: &[&str] = &["UseModelPositions"];

#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub field: String,
    pub stock: String,
    pub new: String,
}

impl Change {
    pub fn text(&self) -> String {
        format!("{} {} -> {}", self.field, self.stock, self.new)
    }
}

fn is_el(e: &Element, name: &str) -> bool {
    e.kind == Kind::Element && e.name == name.as_bytes()
}

fn value<'a>(node: &'a Element, field: &str) -> Option<&'a str> {
    let leaf = node.children.iter().find(|c| is_el(c, field))?;
    std::str::from_utf8(leaf.attr(b"value")?).ok()
}

fn named<'a>(node: &'a Element, element: &str) -> Vec<(String, &'a Element)> {
    node.children
        .iter()
        .filter(|c| is_el(c, element))
        .map(|c| {
            let name = c
                .attr(b"name")
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_default();
            (name, c)
        })
        .collect()
}

fn compare(
    path: &str,
    fields: &[&str],
    stock: &Element,
    new: &Element,
    numeric: bool,
    out: &mut Vec<Change>,
) {
    for field in fields {
        let (a, b) = (value(stock, field), value(new, field));
        let differs = match (a, b) {
            (Some(a), Some(b)) if numeric => match (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
                (Ok(x), Ok(y)) => (x - y).abs() > TOLERANCE,
                _ => a != b,
            },
            (a, b) => a != b,
        };
        if differs {
            out.push(Change {
                field: format!("{path}{field}"),
                stock: a.unwrap_or("missing").to_owned(),
                new: b.unwrap_or("missing").to_owned(),
            });
        }
    }
}

/// The wheel render positions, visual height offsets and tyre sizes `new` moves by more than [`TOLERANCE`] from
/// `stock`, the car it replaces.
pub fn changes(stock: &Element, new: &Element) -> Vec<Change> {
    let mut out = Vec::new();
    if let (Some(a), Some(b)) = (
        stock.children.iter().find(|c| is_el(c, "Chassis")),
        new.children.iter().find(|c| is_el(c, "Chassis")),
    ) {
        compare("Chassis/", CHASSIS_FIELDS, a, b, true, &mut out);
    }
    let system = |e: &'_ Element| e.children.iter().find(|c| is_el(c, "SuspensionSystem")).cloned();
    let (Some(a), Some(b)) = (system(stock), system(new)) else {
        return out;
    };
    for (element, fields) in [("Suspension", SUSPENSION_FIELDS), ("Tyre", TYRE_FIELDS)] {
        let theirs = named(&b, element);
        for (name, old) in named(&a, element) {
            let Some((_, now)) = theirs.iter().find(|(n, _)| *n == name) else {
                continue;
            };
            let path = format!("{element} {name}/");
            compare(&path, fields, old, now, true, &mut out);
            if element == "Suspension" {
                compare(&path, FLAGS, old, now, false, &mut out);
            }
        }
    }
    out
}

/// [`changes`] for two handling files' bytes.
pub fn changes_in_data(stock: &[u8], new: &[u8]) -> Result<Vec<Change>> {
    Ok(changes(
        &Document::parse(stock)?.root,
        &Document::parse(new)?.root,
    ))
}

/// The warning for a handling file of `car` that `by` ships, when it moves the wheels from the stock car's.
pub fn warning(by: &str, car: &str, found: &[Change]) -> Option<String> {
    let first = found.first()?;
    let more = match found.len() {
        1 => String::new(),
        n => format!(" and {} more", n - 1),
    };
    Some(format!(
        "{by}: {car} handling moves the wheels from the stock car's: {}{more} (beyond {} mm). The car's tyre \
         models sit where the stock handling puts them, so the tyres can disappear in some sessions. Tell the \
         addon's author",
        first.text(),
        (TOLERANCE * 1000.0).round()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn el(name: &str, attrs: &[(&str, &str)], children: Vec<Element>) -> Element {
        Element {
            kind: Kind::Element,
            pad: 0,
            name: name.as_bytes().to_vec(),
            attrs: attrs
                .iter()
                .map(|(k, v)| (k.as_bytes().to_vec(), v.as_bytes().to_vec()))
                .collect(),
            children,
            end_pad: 0,
        }
    }

    fn lf(name: &str, v: &str) -> Element {
        el(name, &[("value", v)], vec![])
    }

    fn car(render_y: &str, radius: &str, height: &str) -> Element {
        el(
            "VTF",
            &[],
            vec![
                el(
                    "Chassis",
                    &[],
                    vec![
                        lf("FrontVisualHeightOffset", height),
                        lf("RearVisualHeightOffset", "0.008"),
                    ],
                ),
                el(
                    "SuspensionSystem",
                    &[],
                    vec![
                        el(
                            "Tyre",
                            &[("name", "Rear")],
                            vec![lf("TyreWidth", "0.375"), lf("TyreRadius", radius)],
                        ),
                        el(
                            "Suspension",
                            &[("name", "Rear")],
                            vec![
                                lf("LocalRenderPositionY", render_y),
                                lf("UseModelPositions", "true"),
                            ],
                        ),
                    ],
                ),
            ],
        )
    }

    #[test]
    fn the_same_car_has_no_changes() {
        let c = car("0.003", "0.354", "0.008");
        assert!(changes(&c, &c.clone()).is_empty());
    }

    #[test]
    fn a_render_position_30_mm_lower_is_named() {
        let found = changes(&car("0.003", "0.354", "0.008"), &car("-0.027", "0.354", "0.008"));
        assert_eq!(
            found,
            vec![Change {
                field: "Suspension Rear/LocalRenderPositionY".into(),
                stock: "0.003".into(),
                new: "-0.027".into()
            }]
        );
        let w = warning("1990 handling", "ferrari", &found).unwrap();
        assert!(
            w.contains("ferrari") && w.contains("LocalRenderPositionY 0.003 -> -0.027"),
            "{w}"
        );
    }

    #[test]
    fn changes_inside_the_tolerance_are_ignored() {
        assert!(changes(&car("0.003", "0.354", "0.008"), &car("0.006", "0.3541", "0.0105")).is_empty());
    }

    #[test]
    fn tyre_size_and_visual_height_are_checked() {
        let found = changes(&car("0.003", "0.354", "0.008"), &car("0.003", "0.330", "0.040"));
        let fields: Vec<_> = found.iter().map(|c| c.field.as_str()).collect();
        assert_eq!(
            fields,
            ["Chassis/FrontVisualHeightOffset", "Tyre Rear/TyreRadius"]
        );
        assert!(warning("a", "b", &found).unwrap().contains("and 1 more"));
    }

    #[test]
    fn a_flipped_model_position_flag_is_a_change() {
        let mut new = car("0.003", "0.354", "0.008");
        new.children[1].children[1].children[1] = lf("UseModelPositions", "false");
        let found = changes(&car("0.003", "0.354", "0.008"), &new);
        assert_eq!(found[0].field, "Suspension Rear/UseModelPositions");
    }

    #[test]
    fn data_round_trips() {
        let bytes = |c: Element| {
            Document {
                root: c,
                tail: Vec::new(),
            }
            .to_bytes()
        };
        let found = changes_in_data(
            &bytes(car("0.003", "0.354", "0.008")),
            &bytes(car("-0.027", "0.354", "0.008")),
        )
        .unwrap();
        assert_eq!(found.len(), 1);
    }
}
