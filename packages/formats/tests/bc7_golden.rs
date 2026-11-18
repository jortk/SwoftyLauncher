//! The Rust encoder against bytes the C++ encoder (`native/src/core/apply/content/bc7.hpp`) wrote.
//! `tests/fixtures/bc7/gen.sh` regenerates the first set; the second set is the native tests' own
//! fixtures, which the C++ and the Python pipeline agree on.

use std::path::PathBuf;

use swofty_formats::bc7_enc::{encode_logo, encode_rgba};

fn fixture(dir: &str, name: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = match dir {
        "own" => root.join("tests/fixtures/bc7").join(name),
        _ => root.join("../../native/test/content/bc7").join(name),
    };
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn image(dir: &str, name: &str) -> (Vec<u8>, usize, usize) {
    let raw = fixture(dir, &format!("{name}.rgba"));
    let nl = raw.iter().position(|&b| b == b'\n').unwrap();
    let head = std::str::from_utf8(&raw[..nl]).unwrap();
    let mut it = head.split_whitespace().map(|v| v.parse::<usize>().unwrap());
    let (w, h) = (it.next().unwrap(), it.next().unwrap());
    (raw[nl + 1..].to_vec(), w, h)
}

fn same(got: &[u8], want: &[u8], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    let bad: Vec<usize> = (0..want.len() / 16)
        .filter(|&b| got[b * 16..b * 16 + 16] != want[b * 16..b * 16 + 16])
        .collect();
    assert!(
        bad.is_empty(),
        "{what}: {} blocks differ, first {:?}",
        bad.len(),
        bad.first()
    );
}

#[test]
fn matches_the_cpp_encoder() {
    for name in [
        "opaque_noise",
        "alpha_noise",
        "transparent_random_rgb",
        "soft_edges",
        "gradient",
    ] {
        let (px, w, h) = image("own", name);
        same(
            &encode_rgba(&px, w, h).unwrap(),
            &fixture("own", &format!("{name}.bc7")),
            &format!("{name} encode_rgba"),
        );
        same(
            &encode_logo(&px, w, h).unwrap(),
            &fixture("own", &format!("{name}.logo.bc7")),
            &format!("{name} encode_logo"),
        );
    }
}

#[test]
fn matches_the_native_test_fixtures() {
    for name in [
        "shape64",
        "noise32",
        "alpha32",
        "gradient48",
        "text64x32",
        "flatmix32",
    ] {
        let (px, w, h) = image("native", name);
        same(
            &encode_logo(&px, w, h).unwrap(),
            &fixture("native", &format!("{name}.bc7")),
            name,
        );
    }
}
