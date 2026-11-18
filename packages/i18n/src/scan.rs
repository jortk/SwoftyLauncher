//! Reads the launcher's source for the tests: which texts go through `tr`, and which
//! user-visible literals don't.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const LAUNCHER_SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/launcher/src");

/// Every `.rs` file under `dir`, sorted.
pub fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    out
}

pub fn launcher_source() -> String {
    rust_files(Path::new(LAUNCHER_SRC))
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect()
}

/// A string literal in the source: where it sits and what it says.
pub struct Literal {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

fn unescape(body: &str) -> String {
    let mut text = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('n') => text.push('\n'),
                Some('t') => text.push('\t'),
                Some('u') => {
                    let hex: String = chars.by_ref().skip(1).take_while(|c| *c != '}').collect();
                    if let Some(c) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                        text.push(c);
                    }
                }
                Some('\n') => {
                    while chars.clone().next().is_some_and(char::is_whitespace) {
                        chars.next();
                    }
                }
                Some(other) => text.push(other),
                None => break,
            },
            _ => text.push(c),
        }
    }
    text
}

/// Every string literal in `source`, skipping comments and `'x'` characters.
pub fn literals(source: &str) -> Vec<Literal> {
    let b = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'\'' => {
                let rest = &source[i + 1..];
                let mut it = rest.char_indices();
                match it.next() {
                    Some((_, '\\')) => {
                        i += 1 + rest.find('\'').map_or(1, |e| e + 1);
                    }
                    Some((_, c)) if rest[c.len_utf8()..].starts_with('\'') => i += 2 + c.len_utf8(),
                    _ => i += 1,
                }
            }
            b'r' if matches!(b.get(i + 1), Some(b'"' | b'#'))
                && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_')) =>
            {
                let hashes = b[i + 1..].iter().take_while(|c| **c == b'#').count();
                if b.get(i + 1 + hashes) != Some(&b'"') {
                    i += 1;
                    continue;
                }
                let open = i + 2 + hashes;
                let close = format!("\"{}", "#".repeat(hashes));
                let end = source[open..].find(&close).map_or(b.len(), |e| open + e);
                out.push(Literal {
                    start: i,
                    end: (end + close.len()).min(b.len()),
                    text: source[open..end].to_owned(),
                });
                i = end + close.len();
            }
            b'"' => {
                let start = i;
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                let end = (i + 1).min(b.len());
                out.push(Literal {
                    start,
                    end,
                    text: unescape(&source[start + 1..i.min(b.len())]),
                });
                i = end;
            }
            _ => i += 1,
        }
    }
    out
}

pub const WRAPPERS: [&str; 4] = ["trf!(", "trn(", "trc(", "tr("];

fn is_call(source: &str, at: usize) -> bool {
    !source[..at].ends_with(|c: char| c.is_alphanumeric() || c == '_')
}

/// Where each wrapper call in `source` starts and ends, as byte spans of its arguments.
pub fn wrapper_calls(source: &str, lits: &[Literal]) -> Vec<(&'static str, usize, usize)> {
    let mut calls = Vec::new();
    for name in WRAPPERS {
        for (at, _) in source.match_indices(name) {
            if !is_call(source, at) || lits.iter().any(|l| l.start < at && at < l.end) {
                continue;
            }
            let open = at + name.len();
            let mut depth = 1;
            let mut i = open;
            let bytes = source.as_bytes();
            while i < bytes.len() && depth > 0 {
                if let Some(l) = lits.iter().find(|l| l.start == i) {
                    i = l.end;
                    continue;
                }
                match bytes[i] {
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            calls.push((name, open, i));
        }
    }
    calls
}

/// The literal arguments of each wrapper call, by argument position.
fn call_literals(lits: &[Literal], open: usize, close: usize) -> Vec<&Literal> {
    lits.iter()
        .filter(|l| l.start >= open && l.end <= close)
        .collect()
}

/// The locale-file key of every `tr`, `trf!`, `trn` and `trc` call: the English text,
/// the singular form for `trn`, and `context\u{4}text` for `trc`.
pub fn used_keys(source: &str) -> BTreeSet<String> {
    let lits = literals(source);
    let mut keys = BTreeSet::new();
    for (name, open, close) in wrapper_calls(source, &lits) {
        let inside = call_literals(&lits, open, close);
        let first_is_arg = inside
            .first()
            .is_some_and(|l| source[open..l.start].trim().is_empty());
        match name {
            "trn(" => {
                if let Some(one) = inside.first() {
                    keys.insert(one.text.clone());
                }
            }
            "trc(" => {
                if let [ctx, text, ..] = inside.as_slice() {
                    keys.insert(format!("{}\u{4}{}", ctx.text, text.text));
                }
            }
            _ => {
                if let Some(text) = inside.first().filter(|_| first_is_arg) {
                    keys.insert(text.text.clone());
                }
            }
        }
    }
    keys
}
