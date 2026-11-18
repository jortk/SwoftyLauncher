use gpui_kit::*;

pub(super) struct Curve {
    pub(super) head: Vec<(String, String)>,
    pub(super) points: Vec<(String, String)>,
}

pub(super) fn parse_curve(v: &str) -> Option<Curve> {
    let rest = v.strip_prefix("@v2@")?;
    let mut c = Curve {
        head: Vec::new(),
        points: Vec::new(),
    };
    for part in rest.split(',').filter(|p| !p.is_empty()) {
        let (a, b) = part.split_once(':')?;
        if a.starts_with("min") || a.starts_with("max") {
            c.head.push((a.to_owned(), b.to_owned()));
        } else {
            c.points.push((a.to_owned(), b.to_owned()));
        }
    }
    Some(c)
}

impl Curve {
    pub(super) fn head(&self, name: &str, or: f64) -> f64 {
        self.head
            .iter()
            .find(|(a, _)| a == name)
            .and_then(|(_, b)| b.parse().ok())
            .unwrap_or(or)
    }

    pub(super) fn span(&self) -> (f64, f64) {
        (self.head("miny", 0.), self.head("maxy", 1.))
    }
}

pub(super) fn num(v: f64) -> String {
    let t = format!("{v:.4}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t == "-0" { "0".into() } else { t.to_owned() }
}

pub(super) fn shown_y(c: &Curve, y: &str) -> String {
    let (lo, hi) = c.span();
    y.parse::<f64>()
        .map_or_else(|_| y.to_owned(), |f| num(lo + f * (hi - lo)))
}

pub(super) fn stored_y(c: &Curve, shown: &str) -> Option<String> {
    let (lo, hi) = c.span();
    let v: f64 = shown.trim().parse().ok()?;
    (hi != lo).then(|| num((v - lo) / (hi - lo)))
}

/// A rating curve as a table cell: its first point on the curve's value scale.
pub(crate) fn curve_rating(v: &str) -> Option<String> {
    let c = parse_curve(v)?;
    let (_, y) = c.points.first()?;
    Some(shown_y(&c, y))
}

pub(crate) fn sep_of(v: &str) -> &'static str {
    if v.contains(", ") { ", " } else { "," }
}

pub(super) fn curve_text(c: &Curve) -> String {
    let parts: Vec<String> = c
        .head
        .iter()
        .chain(&c.points)
        .map(|(a, b)| format!("{a}:{b}"))
        .collect();
    format!("@v2@{}", parts.join(","))
}

pub(super) fn argb(v: &str) -> Option<u32> {
    v.trim().parse::<i64>().ok().map(|n| n as u32)
}

pub(crate) fn swatch_color(v: &str) -> Option<Hsla> {
    let c = argb(v)?;
    Some(rgba(((c & 0x00ff_ffff) << 8) | (c >> 24)).into())
}

pub(super) fn color_text(h: Hsla) -> String {
    let c = Rgba::from(h);
    let byte = |f: f32| (f.clamp(0., 1.) * 255.).round() as u32;
    let v = (byte(c.a) << 24) | (byte(c.r) << 16) | (byte(c.g) << 8) | byte(c.b);
    (v as i32).to_string()
}

pub(crate) fn hex_rgb(v: &str) -> Option<String> {
    argb(v).map(|c| format!("#{:06X}", c & 0x00ff_ffff))
}

pub(crate) fn is_on(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1")
}

pub(super) fn range(text: &str) -> (Option<f64>, Option<f64>) {
    match text.split_once("..") {
        Some((a, b)) => (a.trim().parse().ok(), b.trim().parse().ok()),
        None => (None, None),
    }
}

pub(crate) fn list_items(v: &str) -> Vec<String> {
    v.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
