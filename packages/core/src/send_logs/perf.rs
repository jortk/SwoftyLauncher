pub const KEPT: [&str; 3] = ["Summary", "Pacing", "Addons"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section<'a> {
    pub header: &'a str,
    pub title: &'a str,
    pub lines: Vec<&'a str>,
}

/// The `== <Title>` sections of a `perf.log`, each with its lines up to the
/// next header, trailing blank lines dropped.
pub fn sections(text: &str) -> Vec<Section<'_>> {
    let mut out: Vec<Section> = Vec::new();
    for line in text.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if let Some(rest) = line.strip_prefix("== ") {
            out.push(Section {
                header: line,
                title: rest.trim().trim_end_matches('=').trim_end(),
                lines: Vec::new(),
            });
        } else if let Some(s) = out.last_mut() {
            s.lines.push(line);
        }
    }
    for s in &mut out {
        while s.lines.last().is_some_and(|l| l.trim().is_empty()) {
            s.lines.pop();
        }
    }
    out
}

/// A section's title without its bracketed note, as `Pacing` for `Pacing (last 3 minute(s))`.
fn name(title: &str) -> &str {
    title.split_once(" (").map_or(title, |(n, _)| n).trim()
}

/// The `Summary`, `Pacing` and `Addons` sections verbatim, else the start of
/// the file, in at most `max` lines.
pub fn excerpt(text: &str, max: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for s in sections(text)
        .into_iter()
        .filter(|s| KEPT.iter().any(|k| name(s.title).eq_ignore_ascii_case(k)))
    {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.push(s.header.to_owned());
        lines.extend(s.lines.iter().map(|l| (*l).to_owned()));
    }
    if lines.is_empty() {
        lines = text
            .lines()
            .map(|l| l.strip_suffix('\r').unwrap_or(l).to_owned())
            .collect();
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
    }
    if lines.is_empty() {
        return vec!["(the report is empty)".into()];
    }
    if lines.len() > max {
        let more = lines.len() - max + 1;
        lines.truncate(max - 1);
        lines.push(format!("[{more} more lines in the file]"));
    }
    lines
}

/// The `[perf]` minute lines the core logs before each verdict, in the order it logs them.
pub const MINUTE_LINES: [&str; 6] = [
    " s: verdict: ",
    " s: pacing: ",
    " s: presents: ",
    " s: gpu ",
    " s: cpu: ",
    " s: other processes: ",
];

/// Once-a-session `[perf]` notes about what the pacing lines could not read, and overlay modules.
const NOTES: [&str; 3] = [
    "[perf] gpu engine use: ",
    ": the driver gives no ",
    "[perf] overlay module in the game: ",
];

/// How many minutes of a `loader.log` got each verdict.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Verdicts {
    pub judged: u32,
    pub unjudged: u32,
    pub smooth: u32,
    pub stutter: u32,
    pub judder: u32,
    pub gpu: u32,
    pub cpu: u32,
    pub display: u32,
    pub background: u32,
}

impl Verdicts {
    pub fn add(&mut self, verdict: &str) {
        if verdict.starts_with("too few frames") {
            self.unjudged += 1;
            return;
        }
        self.judged += 1;
        for part in verdict.split("; ") {
            let hit = |p: &str| part.starts_with(p);
            self.smooth += u32::from(hit("smooth in frame times"));
            self.stutter += u32::from(hit("stutter"));
            self.judder += u32::from(hit("judder"));
            self.gpu += u32::from(hit("GPU bound"));
            self.cpu += u32::from(hit("CPU bound"));
            self.display += u32::from(hit("display mismatch"));
            self.background += u32::from(hit("background load"));
        }
    }

    pub fn text(&self) -> String {
        let mut out = format!(
            "{} minute(s) judged: {} smooth in frame times, {} stutter, {} judder; {} GPU bound, {} CPU bound; {} display \
             mismatch; {} background load",
            self.judged,
            self.smooth,
            self.stutter,
            self.judder,
            self.gpu,
            self.cpu,
            self.display,
            self.background
        );
        if self.unjudged > 0 {
            out.push_str(&format!(" ({} with too few frames)", self.unjudged));
        }
        out
    }
}

/// The number just before `marker` in `line`, as `3` in `3 spike(s)`.
fn number_before(line: &str, marker: &str) -> Option<f64> {
    let at = line.find(marker)?;
    let head = &line[..at];
    let start = head
        .rfind(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map_or(0, |i| i + 1);
    head[start..].parse().ok()
}

/// How rough a pacing line's minute was: frames over 50 ms count ten times a spike.
fn roughness(pacing: &str) -> f64 {
    number_before(pacing, " over 50 ms").unwrap_or(0.0) * 10.0
        + number_before(pacing, " spike(s) over 2x").unwrap_or(0.0)
}

/// The Smoothness lines for one `loader.log` tail: how many minutes got each verdict, the newest minute's pacing,
/// presents, GPU, CPU, other processes and verdict lines, the roughest minute's pacing line and the once-a-session
/// notes.
pub fn smoothness(tail: &str) -> Vec<String> {
    let mut verdicts = Verdicts::default();
    let mut newest: [Option<&str>; MINUTE_LINES.len()] = [None; MINUTE_LINES.len()];
    let mut roughest: Option<(f64, &str)> = None;
    let mut notes: Vec<&str> = Vec::new();
    let mut display: Option<&str> = None;
    for line in tail.lines().map(str::trim_end) {
        if line.contains("[perf] display: ") {
            display = Some(line);
            continue;
        }
        if let Some(note) = NOTES.iter().find(|n| line.contains(*n)) {
            let key = &line[line.find(note).unwrap_or(0)..];
            if notes.len() < 8 && !notes.iter().any(|n| n.ends_with(key)) {
                notes.push(line);
            }
            continue;
        }
        if !line.contains("[perf] last ") {
            continue;
        }
        let Some(kind) = MINUTE_LINES.iter().position(|m| line.contains(m)) else {
            continue;
        };
        newest[kind] = Some(line);
        if kind == 0 {
            let at = line.find(MINUTE_LINES[0]).unwrap_or(0) + MINUTE_LINES[0].len();
            verdicts.add(&line[at..]);
        }
        if kind == 1 {
            let score = roughness(line);
            if score > 0.0 && roughest.is_none_or(|(best, _)| score > best) {
                roughest = Some((score, line));
            }
        }
    }
    if newest[0].is_none() {
        return vec![
            "  no [perf] verdict lines (a core older than this launcher, or the game ran for under a minute)"
                .to_owned(),
        ];
    }
    let mut out = vec![format!("  verdicts: {}", verdicts.text())];
    if let Some(d) = display {
        out.push(format!("  newest display: {d}"));
    }
    out.push("  newest minute:".to_owned());
    out.extend(newest.iter().flatten().map(|l| format!("    {l}")));
    if let Some((_, l)) = roughest.filter(|(_, l)| Some(*l) != newest[1]) {
        out.push(format!("  roughest minute: {l}"));
    }
    out.extend(notes.iter().map(|l| format!("  {l}")));
    out
}
