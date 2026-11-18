//! Counts the user-visible literals each launcher file still has outside `tr`, and
//! fails when a file has more than `ratchet.json` allows. Wrap text and the count
//! drops; lower the number with `SWOFTY_RATCHET_UPDATE=1 cargo test -p swofty-i18n`.

use std::collections::BTreeMap;
use std::path::Path;

use crate::scan::{LAUNCHER_SRC, Literal, launcher_source, literals, rust_files, used_keys, wrapper_calls};

const BASELINE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/ratchet.json");

/// Calls whose literal arguments are shown to the player.
const SINKS: &[&str] = &[
    ".child(",
    ".label(",
    ".title(",
    ".tooltip(",
    ".placeholder(",
    ".description(",
    ".text(",
    ".body(",
    ".message(",
    "format!(",
    "SharedString::from(",
    "Tooltip::new(",
    "Button::new(",
    "Notification::",
    "AlertDialog",
];

/// Lines whose literals are for logs, paths and checks.
const QUIET: &[&str] = &[
    "println!",
    "eprintln!",
    "note(",
    "log::",
    "tracing::",
    "assert",
    "panic!",
    ".expect(",
    "anyhow!",
    "#[",
    "include_",
    "env!",
    "write!(",
    "writeln!(",
];

fn line_at(source: &str, at: usize) -> (usize, &str) {
    let start = source[..at].rfind('\n').map_or(0, |n| n + 1);
    let end = source[at..].find('\n').map_or(source.len(), |n| at + n);
    (start, &source[start..end])
}

fn reads_as_text(text: &str) -> bool {
    let mut outside = true;
    let letters = text
        .chars()
        .filter(|c| {
            match c {
                '{' => outside = false,
                '}' => outside = true,
                _ => {}
            }
            outside && c.is_alphabetic()
        })
        .count();
    letters >= 3
        && text.contains(' ')
        && !text.contains("://")
        && !text.starts_with(['/', '.', '#'])
        && !text.contains('/')
        && !text.contains('=')
}

fn visible(source: &str, lit: &Literal) -> bool {
    let (start, line) = line_at(source, lit.start);
    let mut context = line.to_owned();
    if source[start..lit.start].trim().is_empty() && start > 0 {
        context.push_str(line_at(source, start - 1).1);
    }
    SINKS.iter().any(|s| context.contains(s)) && !QUIET.iter().any(|q| context.contains(q))
}

/// How many user-visible literals in `source` are not inside a `tr` call. Test modules
/// don't count.
pub fn unwrapped(source: &str) -> usize {
    let source = source.find("#[cfg(test)]").map_or(source, |at| &source[..at]);
    let lits = literals(source);
    let calls = wrapper_calls(source, &lits);
    lits.iter()
        .filter(|l| reads_as_text(&l.text))
        .filter(|l| {
            !calls
                .iter()
                .any(|(_, open, close)| l.start >= *open && l.end <= *close)
        })
        .filter(|l| visible(source, l))
        .count()
}

fn counts() -> BTreeMap<String, usize> {
    let root = Path::new(LAUNCHER_SRC);
    rust_files(root)
        .into_iter()
        .filter(|path| path.file_name().is_none_or(|n| n != "tests.rs"))
        .filter_map(|path| {
            let count = unwrapped(&std::fs::read_to_string(&path).unwrap());
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (count > 0).then_some((name, count))
        })
        .collect()
}

fn baseline() -> BTreeMap<String, usize> {
    let text = std::fs::read_to_string(BASELINE).unwrap_or_else(|_| "{}".to_owned());
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("ratchet.json: {e}"))
}

#[test]
fn no_launcher_file_gains_unwrapped_text() {
    let now = counts();
    if std::env::var_os("SWOFTY_RATCHET_UPDATE").is_some() {
        let text = serde_json::to_string_pretty(&now).unwrap();
        std::fs::write(BASELINE, text + "\n").unwrap();
        return;
    }
    let allowed = baseline();
    let grown: Vec<String> = now
        .iter()
        .filter(|(file, count)| **count > allowed.get(*file).copied().unwrap_or(0))
        .map(|(file, count)| {
            format!(
                "{file}: {count} unwrapped, {} allowed",
                allowed.get(file).copied().unwrap_or(0)
            )
        })
        .collect();
    assert!(
        grown.is_empty(),
        "wrap new user-visible text in tr()/trf!()/trn()/trc():\n{}",
        grown.join("\n")
    );
    let shrunk = allowed
        .iter()
        .filter(|(file, count)| now.get(*file).copied().unwrap_or(0) < **count)
        .count();
    if shrunk > 0 {
        eprintln!(
            "{shrunk} files have fewer unwrapped texts than ratchet.json allows; lower it with SWOFTY_RATCHET_UPDATE=1"
        );
    }
}

#[test]
fn it_counts_text_outside_tr_and_leaves_the_rest() {
    let source = r#"
        fn view() {
            div().child("Not translated yet")
                .child(tr("Translated text"))
                .child(trf!("Hello {name}", name = n))
                .child(trn(n, "{count} thing", "{count} things"))
                .child("single");
            println!("a log line here");
            let path = format!("{dir}/some file.json");
        }
        #[cfg(test)]
        mod tests { fn t() { div().child("In a test module"); } }
    "#;
    assert_eq!(unwrapped(source), 1);
}

#[test]
#[ignore = "a report: cargo test -p swofty-i18n coverage -- --ignored --nocapture"]
fn coverage() {
    let used = used_keys(&launcher_source());
    for (code, text) in crate::FILES {
        let table: BTreeMap<String, serde_json::Value> = serde_json::from_str(text).unwrap();
        let missing = used.iter().filter(|k| !table.contains_key(k.as_str())).count();
        let padded = table
            .keys()
            .filter(|k| k.starts_with(' ') || k.ends_with(' '))
            .count();
        println!(
            "{code}: {} of {} texts translated, {padded} keys with edge spaces",
            used.len() - missing,
            used.len()
        );
    }
    let total: usize = counts().values().sum();
    println!("unwrapped user-visible literals: {total}");
}
