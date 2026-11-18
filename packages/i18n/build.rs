//! Embeds every `locales/<code>.json`, so adding a language is adding a file.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=locales");
    let dir = Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo")).join("locales");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("locales folder")
        .map(|e| e.expect("locales entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    let mut out = String::from("const FILES: &[(&str, &str)] = &[\n");
    for path in files {
        let code = path.file_stem().expect("a file").to_str().expect("an ASCII code");
        let path = path.to_str().expect("a UTF-8 path");
        writeln!(out, "    ({code:?}, include_str!({path:?})),").expect("write to a string");
    }
    out.push_str("];\n");
    let target = Path::new(&std::env::var("OUT_DIR").expect("set by cargo")).join("locales.rs");
    std::fs::write(target, out).expect("write locales.rs");
}
