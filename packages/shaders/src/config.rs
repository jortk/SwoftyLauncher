//! The tolerant `.swfx` parser: the TOML subset a shader config uses, one line at a time.
//!
//! Every problem becomes an error in [`RawDoc::errors`] and parsing goes on with the next line, so
//! one text can report several.

use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Lit {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Array(Vec<Elem>),
    Strs(Vec<StrElem>),
    Tables(Vec<RawInline>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StrElem {
    pub(crate) text: String,
    pub(crate) range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawInline {
    pub(crate) entries: Vec<RawEntry>,
    pub(crate) range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Elem {
    pub(crate) num: f64,
    pub(crate) int: bool,
    pub(crate) range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawValue {
    pub(crate) lit: Lit,
    pub(crate) range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawEntry {
    pub(crate) key: String,
    pub(crate) key_range: Range<usize>,
    pub(crate) value: RawValue,
    /// The whole line without indentation or comment.
    pub(crate) range: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawTable {
    pub(crate) effect: String,
    pub(crate) effect_range: Range<usize>,
    pub(crate) label: Option<String>,
    pub(crate) header: Range<usize>,
    pub(crate) entries: Vec<RawEntry>,
    pub(crate) span: Range<usize>,
    pub(crate) bad: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawRule {
    pub(crate) header: Range<usize>,
    pub(crate) entries: Vec<RawEntry>,
    pub(crate) sets: Vec<RawTable>,
    pub(crate) span: Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RawError {
    pub(crate) range: Range<usize>,
    pub(crate) message: String,
}

#[derive(Debug, Default)]
pub(crate) struct RawDoc {
    pub(crate) top: Vec<RawEntry>,
    pub(crate) tables: Vec<RawTable>,
    pub(crate) rules: Vec<RawRule>,
    pub(crate) errors: Vec<RawError>,
}

fn bare_key(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn digits(s: &str) -> Option<&str> {
    let mut prev_us = true;
    let mut any = false;
    for c in s.chars() {
        match c {
            '0'..='9' => {
                prev_us = false;
                any = true;
            }
            '_' if !prev_us => prev_us = true,
            _ => return None,
        }
    }
    (any && !prev_us).then_some(s)
}

/// TOML's integer and float grammar. Returns the value and whether it is an integer.
pub(crate) fn parse_number(tok: &str) -> Option<(f64, bool)> {
    let body = tok.strip_prefix(['+', '-']).unwrap_or(tok);
    let (int_part, rest) = match body.find(['.', 'e', 'E']) {
        Some(i) => (&body[..i], &body[i..]),
        None => (body, ""),
    };
    digits(int_part)?;
    if int_part.len() > 1 && int_part.starts_with('0') {
        return None;
    }
    let clean: String = tok.chars().filter(|&c| c != '_').collect();
    if rest.is_empty() {
        return clean.parse::<i64>().ok().map(|v| (v as f64, true));
    }
    let (frac, exp) = match rest.strip_prefix('.') {
        Some(r) => match r.find(['e', 'E']) {
            Some(i) => (Some(&r[..i]), &r[i..]),
            None => (Some(r), ""),
        },
        None => (None, rest),
    };
    if let Some(f) = frac {
        digits(f)?;
    }
    if !exp.is_empty() {
        digits(exp[1..].strip_prefix(['+', '-']).unwrap_or(&exp[1..]))?;
    }
    clean
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(|v| (v, false))
}

struct LineCtx<'a> {
    line: &'a str,
    start: usize,
}

impl LineCtx<'_> {
    fn at(&self, from: usize, to: usize) -> Range<usize> {
        self.start + from..self.start + to
    }
}

fn skip_ws(s: &str, mut i: usize) -> usize {
    while s[i..].starts_with([' ', '\t']) {
        i += 1;
    }
    i
}

fn skip_all(s: &str, mut i: usize) -> usize {
    loop {
        i = skip_ws(s, i);
        if s[i..].starts_with(['\n', '\r']) {
            i += 1;
        } else if s[i..].starts_with('#') {
            i = s[i..].find('\n').map_or(s.len(), |k| i + k);
        } else {
            return i;
        }
    }
}

fn string_basic(ctx: &LineCtx, open: usize) -> Result<(String, usize), RawError> {
    let line = ctx.line;
    let mut out = String::new();
    let mut chars = line[open + 1..].char_indices();
    while let Some((k, c)) = chars.next() {
        let at = open + 1 + k;
        match c {
            '"' => return Ok((out, at + 1)),
            '\n' | '\r' => break,
            '\\' => {
                let Some((_, e)) = chars.next() else { break };
                match e {
                    'b' => out.push('\u{8}'),
                    't' => out.push('\t'),
                    'n' => out.push('\n'),
                    'f' => out.push('\u{c}'),
                    'r' => out.push('\r'),
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    'u' | 'U' => {
                        let n = if e == 'u' { 4 } else { 8 };
                        let hex: String = chars.by_ref().take(n).map(|(_, c)| c).collect();
                        match u32::from_str_radix(&hex, 16)
                            .ok()
                            .filter(|_| hex.len() == n)
                            .and_then(char::from_u32)
                        {
                            Some(c) => out.push(c),
                            None => {
                                return Err(RawError {
                                    range: ctx.at(at, (at + 2 + hex.len()).min(line.len())),
                                    message: "Invalid unicode escape in string".into(),
                                });
                            }
                        }
                    }
                    other => {
                        return Err(RawError {
                            range: ctx.at(at, at + 1 + other.len_utf8()),
                            message: format!("Unknown escape `\\{other}` in string"),
                        });
                    }
                }
            }
            _ => out.push(c),
        }
    }
    Err(RawError {
        range: ctx.at(open, line.len()),
        message: "This string is not closed on its line".into(),
    })
}

fn number_token(s: &str, from: usize) -> usize {
    s[from..]
        .find([' ', '\t', '\n', '\r', ',', ']', '}', '#'])
        .map_or(s.len(), |i| from + i)
}

fn bad_token(ctx: &LineCtx, from: usize, to: usize) -> RawError {
    let tok = &ctx.line[from..to];
    let message = if tok.starts_with(|c: char| c.is_ascii_digit()) && tok.contains(['-', ':']) {
        format!("Dates and times (`{tok}`) are not supported in a shader config")
    } else if matches!(tok.trim_start_matches(['+', '-']), "inf" | "nan")
        || tok.starts_with("0x")
        || tok.starts_with("0o")
        || tok.starts_with("0b")
    {
        format!("`{tok}` is not supported in a shader config; write a decimal number")
    } else if tok.is_empty() {
        "Expected a value".to_owned()
    } else {
        format!("`{tok}` is not a valid value; use a number, true, false or a quoted string")
    };
    RawError {
        range: ctx.at(from, to.max(from)),
        message,
    }
}

fn value(ctx: &LineCtx, from: usize) -> Result<(RawValue, usize), RawError> {
    let line = ctx.line;
    let i = skip_ws(line, from);
    let rest = &line[i..];
    let unsupported = |end: usize, what: &str| RawError {
        range: ctx.at(i, end),
        message: format!("{what} is not supported in a shader config"),
    };
    if rest.starts_with("\"\"\"") || rest.starts_with("'''") {
        return Err(unsupported(line.len(), "A multi-line string"));
    }
    if rest.starts_with('{') {
        return Err(unsupported(line.len(), "An inline table"));
    }
    if rest.starts_with('"') {
        let (s, end) = string_basic(ctx, i)?;
        return Ok((
            RawValue {
                lit: Lit::Str(s),
                range: ctx.at(i, end),
            },
            end,
        ));
    }
    if let Some(inner) = rest.strip_prefix('\'') {
        return match inner.find('\'').filter(|&k| !inner[..k].contains('\n')) {
            Some(k) => {
                let end = i + 1 + k + 1;
                Ok((
                    RawValue {
                        lit: Lit::Str(line[i + 1..end - 1].to_owned()),
                        range: ctx.at(i, end),
                    },
                    end,
                ))
            }
            None => Err(RawError {
                range: ctx.at(i, line.len()),
                message: "This string is not closed on its line".into(),
            }),
        };
    }
    if rest.starts_with('[') {
        return array(ctx, i);
    }
    let end = number_token(line, i);
    let tok = &line[i..end];
    match tok {
        "true" => {
            return Ok((
                RawValue {
                    lit: Lit::Bool(true),
                    range: ctx.at(i, end),
                },
                end,
            ));
        }
        "false" => {
            return Ok((
                RawValue {
                    lit: Lit::Bool(false),
                    range: ctx.at(i, end),
                },
                end,
            ));
        }
        _ => {}
    }
    match parse_number(tok) {
        Some((v, true)) => Ok((
            RawValue {
                lit: Lit::Int(v as i64),
                range: ctx.at(i, end),
            },
            end,
        )),
        Some((v, false)) => Ok((
            RawValue {
                lit: Lit::Float(v),
                range: ctx.at(i, end),
            },
            end,
        )),
        None => Err(bad_token(ctx, i, end)),
    }
}

fn array(ctx: &LineCtx, open: usize) -> Result<(RawValue, usize), RawError> {
    let line = ctx.line;
    let mut i = open + 1;
    let mut nums = Vec::new();
    let mut strs = Vec::new();
    let mut tables = Vec::new();
    loop {
        i = skip_all(line, i);
        if i >= line.len() {
            return Err(RawError {
                range: ctx.at(open, (open + 1).min(line.len())),
                message: "This array is not closed with `]`".into(),
            });
        }
        if line[i..].starts_with(']') {
            let lit = if !strs.is_empty() {
                Lit::Strs(strs)
            } else if !tables.is_empty() {
                Lit::Tables(tables)
            } else {
                Lit::Array(nums)
            };
            return Ok((
                RawValue {
                    lit,
                    range: ctx.at(open, i + 1),
                },
                i + 1,
            ));
        }
        let kind_clash = |want_nums: bool, want_strs: bool, want_tables: bool| {
            (!want_nums && !nums.is_empty())
                || (!want_strs && !strs.is_empty())
                || (!want_tables && !tables.is_empty())
        };
        let end;
        if line[i..].starts_with(['"', '\'']) {
            let (v, e) = value(ctx, i)?;
            end = e;
            if kind_clash(false, true, false) {
                return Err(mixed(ctx, i, e));
            }
            if let Lit::Str(text) = v.lit {
                strs.push(StrElem { text, range: v.range });
            }
        } else if line[i..].starts_with('{') {
            let (t, e) = inline_table(ctx, i)?;
            end = e;
            if kind_clash(false, false, true) {
                return Err(mixed(ctx, i, e));
            }
            tables.push(t);
        } else {
            end = number_token(line, i);
            match parse_number(&line[i..end]) {
                Some((num, int)) => {
                    if kind_clash(true, false, false) {
                        return Err(mixed(ctx, i, end));
                    }
                    nums.push(Elem {
                        num,
                        int,
                        range: ctx.at(i, end),
                    });
                }
                None => {
                    let mut e = bad_token(ctx, i, end);
                    if line[i..].starts_with('[') || matches!(&line[i..end], "true" | "false") {
                        e.range = ctx.at(i, end.max(i + 1));
                        e.message =
                            "An array in a shader config holds numbers, strings or `{ }` tables only".into();
                    }
                    return Err(e);
                }
            }
        }
        i = skip_all(line, end);
        if i >= line.len() {
            continue;
        }
        if line[i..].starts_with(',') {
            i += 1;
        } else if !line[i..].starts_with(']') {
            let stop = number_token(line, i).max(i + 1).min(line.len());
            return Err(RawError {
                range: ctx.at(i.min(line.len()), stop),
                message: "Expected `,` or `]` in the array".into(),
            });
        }
    }
}

fn mixed(ctx: &LineCtx, from: usize, to: usize) -> RawError {
    RawError {
        range: ctx.at(from, to.max(from + 1).min(ctx.line.len())),
        message: "An array holds one kind of value: numbers, strings or `{ }` tables".into(),
    }
}

fn inline_table(ctx: &LineCtx, open: usize) -> Result<(RawInline, usize), RawError> {
    let line = ctx.line;
    let mut i = open + 1;
    let mut entries: Vec<RawEntry> = Vec::new();
    loop {
        i = skip_all(line, i);
        if i >= line.len() {
            return Err(RawError {
                range: ctx.at(open, (open + 1).min(line.len())),
                message: "This `{` is not closed with `}`".into(),
            });
        }
        if line[i..].starts_with('}') {
            return Ok((
                RawInline {
                    entries,
                    range: ctx.at(open, i + 1),
                },
                i + 1,
            ));
        }
        let key_end = line[i..]
            .find([' ', '\t', '\n', '\r', '=', ',', '}'])
            .map_or(line.len(), |k| i + k);
        let key = &line[i..key_end];
        if !bare_key(key) {
            return Err(RawError {
                range: ctx.at(i, key_end.max(i + 1).min(line.len())),
                message: "Expected `key = value` in the `{ }` table".into(),
            });
        }
        let eq = skip_ws(line, key_end);
        if !line[eq..].starts_with('=') {
            return Err(RawError {
                range: ctx.at(i, key_end),
                message: format!("Expected `=` after `{key}`"),
            });
        }
        let (v, end) = value(ctx, eq + 1)?;
        if entries.iter().any(|e| e.key == key) {
            return Err(RawError {
                range: ctx.at(i, key_end),
                message: format!("`{key}` is set twice"),
            });
        }
        entries.push(RawEntry {
            key: key.to_owned(),
            key_range: ctx.at(i, key_end),
            range: ctx.at(i, end),
            value: v,
        });
        i = skip_all(line, end);
        if i >= line.len() {
            continue;
        }
        if line[i..].starts_with(',') {
            i += 1;
        } else if !line[i..].starts_with('}') {
            return Err(RawError {
                range: ctx.at(i.min(line.len()), (i + 1).min(line.len())),
                message: "Expected `,` or `}` in the `{ }` table".into(),
            });
        }
    }
}

fn trailing(ctx: &LineCtx, from: usize, errors: &mut Vec<RawError>) {
    let i = skip_ws(ctx.line, from);
    if i < ctx.line.len() && !ctx.line[i..].starts_with('#') {
        errors.push(RawError {
            range: ctx.at(i, ctx.line.len()),
            message: "Unexpected text after the value".into(),
        });
    }
}

#[derive(Clone, Copy)]
enum Cur {
    Top,
    Table(usize),
    Rule(usize),
    Set(usize, usize),
}

fn split_segments(line: &str, from: usize, to: usize) -> Vec<(&str, Range<usize>)> {
    let inner = &line[from..to];
    let mut segments = Vec::new();
    let mut off = from;
    for part in inner.split('.') {
        let lead = part.len() - part.trim_start_matches([' ', '\t']).len();
        let trimmed = part.trim_matches([' ', '\t']);
        segments.push((trimmed, off + lead..off + lead + trimmed.len()));
        off += part.len() + 1;
    }
    segments
}

fn bad_table(doc: &mut RawDoc, ctx: &LineCtx, at: usize, end: usize) -> Cur {
    doc.tables.push(RawTable {
        effect: String::new(),
        effect_range: ctx.at(at, at),
        label: None,
        header: ctx.at(at, end),
        entries: Vec::new(),
        span: ctx.at(at, end),
        bad: true,
    });
    Cur::Table(doc.tables.len() - 1)
}

fn rule_header(ctx: &LineCtx, at: usize, doc: &mut RawDoc) -> Cur {
    let line = ctx.line;
    let body_end = line.trim_end().len();
    let inner_start = at + 2;
    let close = line[inner_start..].find("]]").map(|i| inner_start + i);
    let inner_end = close.unwrap_or_else(|| {
        line[inner_start..]
            .find('#')
            .map_or(body_end, |i| inner_start + i)
    });
    let name = line[inner_start..inner_end].trim();
    if name != "when" {
        doc.errors.push(RawError {
            range: ctx.at(at, body_end),
            message: "An array of tables (`[[x]]`) is not supported in a shader config; only `[[when]]` is"
                .into(),
        });
        return bad_table(doc, ctx, at, line.len());
    }
    let end = match close {
        Some(c) => {
            let mut errors = Vec::new();
            trailing(ctx, c + 2, &mut errors);
            doc.errors.append(&mut errors);
            c + 2
        }
        None => {
            doc.errors.push(RawError {
                range: ctx.at(at, inner_end),
                message: "This rule header is missing its `]]`".into(),
            });
            inner_end
        }
    };
    doc.rules.push(RawRule {
        header: ctx.at(at, end),
        entries: Vec::new(),
        sets: Vec::new(),
        span: ctx.at(at, end),
    });
    Cur::Rule(doc.rules.len() - 1)
}

fn header(ctx: &LineCtx, at: usize, doc: &mut RawDoc) -> Cur {
    let line = ctx.line;
    let mk = |bad: bool, effect: String, effect_range: Range<usize>, label: Option<String>, close: usize| {
        RawTable {
            effect,
            effect_range,
            label,
            header: ctx.at(at, close),
            entries: Vec::new(),
            span: ctx.at(at, close),
            bad,
        }
    };
    let Some(close) = line[at..].find(']').map(|i| at + i) else {
        doc.errors.push(RawError {
            range: ctx.at(at, line.len()),
            message: "This table header is missing its `]`".into(),
        });
        doc.tables
            .push(mk(true, String::new(), ctx.at(at, at), None, line.len()));
        return Cur::Table(doc.tables.len() - 1);
    };
    let mut errors = Vec::new();
    trailing(ctx, close + 1, &mut errors);
    doc.errors.append(&mut errors);
    let inner = &line[at + 1..close];
    let segments = split_segments(line, at + 1, close);
    let mut bad = false;
    let whole = ctx.at(at, close + 1);
    if inner.contains(['"', '\'']) {
        doc.errors.push(RawError {
            range: whole.clone(),
            message: "A quoted table name is not supported in a shader config".into(),
        });
        bad = true;
    } else if segments.iter().any(|(s, _)| !bare_key(s)) {
        doc.errors.push(RawError {
            range: whole.clone(),
            message: "A table name is letters, digits, `_` and `-`".into(),
        });
        bad = true;
    } else if segments[0].0 == "when" && segments.len() > 1 {
        let ok_shape = segments[1].0 == "set" && (3..=4).contains(&segments.len());
        if !ok_shape {
            doc.errors.push(RawError {
                range: whole.clone(),
                message: "A rule's table is `[when.set.<effect>]` or `[when.set.<effect>.<label>]`".into(),
            });
            bad = true;
        } else if doc.rules.is_empty() {
            doc.errors.push(RawError {
                range: whole.clone(),
                message: format!("`[{}]` must come after a `[[when]]` rule", inner.trim()),
            });
            bad = true;
        } else {
            let r = doc.rules.len() - 1;
            let mut t = mk(
                false,
                segments[2].0.to_owned(),
                segments[2].1.clone(),
                segments.get(3).map(|(s, _)| (*s).to_owned()),
                close + 1,
            );
            t.span = whole;
            doc.rules[r].span.end = t.span.end;
            doc.rules[r].sets.push(t);
            return Cur::Set(r, doc.rules[r].sets.len() - 1);
        }
    } else if segments.len() > 2 {
        doc.errors.push(RawError {
            range: whole.clone(),
            message: "A shader table is `[effect]` or `[effect.label]`".into(),
        });
        bad = true;
    }
    let (effect, er) = segments[0].clone();
    let label = segments.get(1).map(|(s, _)| (*s).to_owned()).filter(|_| !bad);
    let mut t = mk(bad, effect.to_owned(), ctx.at(er.start, er.end), label, close + 1);
    t.span = whole;
    doc.tables.push(t);
    Cur::Table(doc.tables.len() - 1)
}

fn entry(ctx: &LineCtx, at: usize, cur: Cur, doc: &mut RawDoc) {
    let line = ctx.line;
    let Some(eq) = line[at..].find('=').map(|i| at + i) else {
        doc.errors.push(RawError {
            range: ctx.at(at, line.trim_end().len()),
            message: "Expected `key = value`".into(),
        });
        return;
    };
    let raw_key = &line[at..eq];
    let key = raw_key.trim_end_matches([' ', '\t']);
    let key_range = ctx.at(at, at + key.len());
    if key.is_empty() {
        doc.errors.push(RawError {
            range: ctx.at(eq, eq + 1),
            message: "Missing the key before `=`".into(),
        });
        return;
    }
    if key.contains('"') || key.contains('\'') {
        doc.errors.push(RawError {
            range: key_range,
            message: "A quoted key is not supported in a shader config".into(),
        });
        return;
    }
    if key.contains('.') {
        doc.errors.push(RawError {
            range: key_range,
            message: "A dotted key is not supported in a shader config".into(),
        });
        return;
    }
    if !bare_key(key) {
        doc.errors.push(RawError {
            range: key_range,
            message: format!("`{key}` is not a valid key"),
        });
        return;
    }
    match value(ctx, eq + 1) {
        Ok((v, end)) => {
            let mut errors = Vec::new();
            trailing(ctx, end, &mut errors);
            doc.errors.append(&mut errors);
            let range = ctx.at(at, end);
            let e = RawEntry {
                key: key.to_owned(),
                key_range: key_range.clone(),
                value: v,
                range: range.clone(),
            };
            let target = match cur {
                Cur::Top => &mut doc.top,
                Cur::Table(i) => {
                    let t = &mut doc.tables[i];
                    t.span.end = range.end;
                    &mut t.entries
                }
                Cur::Rule(r) => {
                    let t = &mut doc.rules[r];
                    t.span.end = range.end;
                    &mut t.entries
                }
                Cur::Set(r, s) => {
                    let rule = &mut doc.rules[r];
                    rule.span.end = range.end;
                    let t = &mut rule.sets[s];
                    t.span.end = range.end;
                    &mut t.entries
                }
            };
            if target.iter().any(|x| x.key == e.key) {
                doc.errors.push(RawError {
                    range: key_range,
                    message: format!("`{key}` is set twice"),
                });
            } else {
                target.push(e);
            }
        }
        Err(e) => doc.errors.push(e),
    }
}

/// How many more `[`/`{` than `]`/`}` a line opens outside strings and comments.
fn depth_delta(s: &str) -> i32 {
    let (mut depth, mut quote, mut escaped) = (0, None, false);
    for c in s.chars() {
        match quote {
            Some('"') if escaped => escaped = false,
            Some('"') if c == '\\' => escaped = true,
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' => quote = Some(c),
                '#' => break,
                '[' | '{' => depth += 1,
                ']' | '}' => depth -= 1,
                _ => {}
            },
        }
    }
    depth
}

fn looks_like_entry(body: &str) -> bool {
    let key_len = body
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(body.len());
    key_len > 0 && body[key_len..].trim_start_matches([' ', '\t']).starts_with('=')
}

pub(crate) fn parse(text: &str) -> RawDoc {
    let mut doc = RawDoc::default();
    let mut cur = Cur::Top;
    let lines: Vec<(usize, &str)> = text
        .split_inclusive('\n')
        .scan(0, |off, raw| {
            let at = *off;
            *off += raw.len();
            Some((at, raw))
        })
        .collect();
    let mut n = 0;
    while n < lines.len() {
        let (start, raw) = lines[n];
        n += 1;
        let mut line_end = start + raw.len();
        let first = raw.trim_end_matches(['\n', '\r']);
        let at = skip_ws(first, 0);
        let body = &first[at..];
        if body.is_empty() || body.starts_with('#') {
            continue;
        }
        if !body.starts_with('[') {
            let mut depth = body.find('=').map_or(0, |eq| depth_delta(&body[eq + 1..]));
            while depth > 0 && n < lines.len() {
                let next = lines[n].1.trim_end_matches(['\n', '\r']).trim_start();
                if next.starts_with('[') || looks_like_entry(next) {
                    break;
                }
                depth += depth_delta(next);
                line_end = lines[n].0 + lines[n].1.len();
                n += 1;
            }
        }
        let line = text[start..line_end].trim_end_matches(['\n', '\r']);
        let ctx = LineCtx { line, start };
        if body.starts_with("[[") {
            cur = rule_header(&ctx, at, &mut doc);
        } else if body.starts_with('[') {
            cur = header(&ctx, at, &mut doc);
        } else {
            entry(&ctx, at, cur, &mut doc);
        }
    }
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn errs(text: &str) -> Vec<String> {
        parse(text).errors.into_iter().map(|e| e.message).collect()
    }

    #[test]
    fn numbers_follow_the_toml_grammar() {
        for ok in [
            "0", "1", "-3", "+4", "1_000", "0.5", "-0.25", "1e3", "1.5E-2", "6.02e+23", "1_0.2_5", "-0",
        ] {
            assert!(parse_number(ok).is_some(), "{ok}");
            ok.parse::<toml::Value>().ok();
        }
        for bad in [
            "", "01", "1__0", "_1", "1_", ".5", "5.", "1e", "1.e3", "0x10", "inf", "nan", "--1", "1.2.3",
            "1e+",
        ] {
            assert!(parse_number(bad).is_none(), "{bad}");
        }
        assert_eq!(parse_number("12"), Some((12.0, true)));
        assert_eq!(parse_number("1e3"), Some((1000.0, false)));
        assert_eq!(parse_number("1_000.5"), Some((1000.5, false)));
    }

    #[test]
    fn a_good_file_parses_to_tables_and_entries() {
        let text = "# hi\nname = \"A\"\n\n[bloom]\nintensity = 1.5 # c\ntint = [1, 2.0, 3,]\non = true\n[bloom.wide]\nx = 'lit'\n";
        let d = parse(text);
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        assert_eq!(d.top.len(), 1);
        assert_eq!(d.tables.len(), 2);
        assert_eq!(d.tables[0].entries.len(), 3);
        assert_eq!(d.tables[1].label.as_deref(), Some("wide"));
        let e = &d.tables[0].entries[0];
        assert_eq!(&text[e.key_range.clone()], "intensity");
        assert_eq!(&text[e.value.range.clone()], "1.5");
        assert_eq!(
            &text[d.tables[0].span.clone()],
            "[bloom]\nintensity = 1.5 # c\ntint = [1, 2.0, 3,]\non = true"
        );
        match &d.tables[0].entries[1].value.lit {
            Lit::Array(a) => assert_eq!(a.len(), 3),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn strings_decode_escapes() {
        let d = parse("name = \"a\\tb\\u00e9\\\"q\\\\\"\n");
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        assert_eq!(d.top[0].value.lit, Lit::Str("a\tbé\"q\\".into()));
        assert!(errs("n = \"a\\qb\"").iter().any(|m| m.contains("Unknown escape")));
        assert!(errs("n = \"abc").iter().any(|m| m.contains("not closed")));
        assert!(errs("n = \"a\\u12\"").iter().any(|m| m.contains("unicode")));
    }

    #[test]
    fn unsupported_toml_is_named() {
        assert!(errs("n = \"\"\"x\"\"\"")[0].contains("multi-line string"));
        assert!(errs("n = '''x'''")[0].contains("multi-line string"));
        assert!(errs("n = {a = 1}")[0].contains("inline table"));
        assert!(errs("n = 1979-05-27")[0].contains("Dates"));
        assert!(errs("n = [1,\n 2")[0].contains("not closed"));
        assert!(errs("[[x]]")[0].contains("array of tables"));
        assert!(errs("a.b = 1")[0].contains("dotted key"));
        assert!(errs("\"a\" = 1")[0].contains("quoted key"));
        assert!(errs("[a.b.c]")[0].contains("[effect.label]"));
        assert!(errs("[\"a\"]")[0].contains("quoted table"));
        assert!(errs("n = [1, \"a\"]")[0].contains("one kind"));
        assert!(errs("n = [true]")[0].contains("numbers, strings"));
        assert!(errs("n = yes")[0].contains("not a valid value"));
        assert!(errs("n = 0x10")[0].contains("decimal"));
    }

    #[test]
    fn it_keeps_going_after_a_bad_line() {
        let d = parse("a = \nb = 2\nc 3\nd = 4 5\n[x\ne = 1\n");
        let msgs: Vec<_> = d.errors.iter().map(|e| e.message.as_str()).collect();
        assert_eq!(msgs.len(), 4, "{msgs:?}");
        assert!(d.top.iter().any(|e| e.key == "b"));
        assert!(d.top.iter().any(|e| e.key == "d"));
        assert!(d.tables[0].bad);
    }

    #[test]
    fn duplicate_keys_are_errors_in_their_table() {
        assert!(errs("a = 1\na = 2")[0].contains("set twice"));
        assert!(errs("[x]\na = 1\n[y]\na = 2").is_empty());
    }

    #[test]
    fn crlf_and_indent_are_accepted() {
        let d = parse("name = \"a\"\r\n  [x]\r\n\ty = 1\r\n");
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        assert_eq!(d.tables[0].entries[0].key, "y");
    }

    #[test]
    fn trailing_garbage_is_flagged() {
        assert!(errs("a = 1 2")[0].contains("after the value"));
        assert!(errs("[x] y")[0].contains("after the value"));
        assert!(errs("a = [1] 2")[0].contains("after the value"));
    }

    #[test]
    fn rules_collect_their_entries_and_set_tables() {
        let text = "name = \"a\"\n[grain]\nsize = 1\n[[when]]\nname = \"k\"\non = \"car.speed\" # c\n[when.set.grain]\namount = 0.5\n[when.set.grain.heavy]\nsize = 2\n[[when]]\non = \"view.replay\"\n";
        let d = parse(text);
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        assert_eq!(d.tables.len(), 1);
        assert_eq!(d.rules.len(), 2);
        let r = &d.rules[0];
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.sets.len(), 2);
        assert_eq!(r.sets[0].effect, "grain");
        assert_eq!(r.sets[1].label.as_deref(), Some("heavy"));
        assert_eq!(r.sets[1].entries[0].key, "size");
        assert_eq!(&text[r.header.clone()], "[[when]]");
        assert!(text[r.span.clone()].ends_with("size = 2"));
        assert_eq!(d.rules[1].entries.len(), 1);
    }

    #[test]
    fn arrays_hold_strings_tables_and_span_lines() {
        let text = "[[when]]\nis = [\"grass\", 'gravel']\nall = [\n  { on = \"car.surface\", is = \"kerb\" }, # a\n  { on = \"car.speed\", above = 150 },\n]\npriority = 1\n";
        let d = parse(text);
        assert!(d.errors.is_empty(), "{:?}", d.errors);
        let r = &d.rules[0];
        assert_eq!(r.entries.len(), 3);
        match &r.entries[0].value.lit {
            Lit::Strs(s) => assert_eq!((s[0].text.as_str(), s[1].text.as_str()), ("grass", "gravel")),
            other => panic!("{other:?}"),
        }
        match &r.entries[1].value.lit {
            Lit::Tables(t) => {
                assert_eq!(t.len(), 2);
                assert_eq!(t[1].entries[1].key, "above");
                assert_eq!(t[1].entries[1].value.lit, Lit::Int(150));
                assert_eq!(&text[t[0].entries[0].key_range.clone()], "on");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(r.entries[2].key, "priority");
    }

    #[test]
    fn rule_header_problems_are_errors_and_parsing_goes_on() {
        let before = errs("[when.set.grain]\nx = 1\n");
        assert!(before[0].contains("must come after a `[[when]]`"));
        assert!(errs("[[when]]\n[when.foo.grain]\n")[0].contains("[when.set.<effect>]"));
        assert!(errs("[[when\n")[0].contains("missing its `]]`"));
        assert!(errs("[[nope]]\n")[0].contains("only `[[when]]`"));
        let d = parse("[[when]]\non = \n[[when]]\nname = \"x\"\n");
        assert_eq!(d.rules.len(), 2);
        assert_eq!(d.errors.len(), 1);
        assert_eq!(d.rules[1].entries.len(), 1);
    }

    #[test]
    fn an_unclosed_array_does_not_swallow_the_next_table() {
        let d = parse("[[when]]\nall = [\n  { on = \"a\" },\n[grain]\nsize = 1\n");
        assert!(
            d.errors.iter().any(|e| e.message.contains("not closed")),
            "{:?}",
            d.errors
        );
        assert_eq!(d.tables.len(), 1);
        assert_eq!(d.tables[0].entries.len(), 1);
    }
}
