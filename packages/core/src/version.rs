//! The one rule for whether a version meets an addon requirement.
//!
//! semver's `VersionReq::matches` rejects every pre-release unless a comparator names the same
//! `major.minor.patch` with a pre-release tag, so `*` rejects `0.2.8-beta`. Addon dependencies
//! use this rule instead: `*` and an empty requirement accept every version, and for any other
//! requirement a pre-release counts as its base version (`1.1.0-beta` is `1.1.0`), so
//! `>=1.0.0` accepts `1.1.0-beta` and `<1.1.0` rejects it. A comparator that names a
//! pre-release is compared against the pre-release itself when the versions share
//! `major.minor.patch`, and an `=` pin stays exact.

use semver::{Comparator, Op, Version, VersionReq};

pub fn satisfies(req: &VersionReq, v: &Version) -> bool {
    req.comparators.iter().all(|c| comparator_satisfies(c, v))
}

fn comparator_satisfies(c: &Comparator, v: &Version) -> bool {
    if v.pre.is_empty() || c.op == Op::Exact {
        return c.matches(v);
    }
    let same_core = c.minor == Some(v.minor) && c.patch == Some(v.patch) && c.major == v.major;
    if same_core && !c.pre.is_empty() {
        return c.matches(v);
    }
    let base = Version::new(v.major, v.minor, v.patch);
    c.matches(&base)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(req: &str, v: &str) -> bool {
        satisfies(&VersionReq::parse(req).unwrap(), &Version::parse(v).unwrap())
    }

    #[test]
    fn star_and_empty_accept_everything() {
        assert!(ok("*", "1.0.0"));
        assert!(ok("*", "0.2.8-native-radius-base"));
        assert!(satisfies(&VersionReq::STAR, &Version::parse("0.0.1-a").unwrap()));
        assert!(satisfies(
            &VersionReq::default(),
            &Version::parse("3.0.0-rc.1").unwrap()
        ));
    }

    #[test]
    fn greater_equal() {
        assert!(ok(">=1.0.0", "1.1.0"));
        assert!(ok(">=1.0.0", "1.1.0-beta"));
        assert!(ok(">=1.0.0", "1.0.0-beta"));
        assert!(!ok(">=1.0.0", "0.9.0-beta"));
        assert!(ok(">=0.2.8-native-radius-base", "0.2.8-native-radius-base"));
        assert!(!ok(">=0.2.8-b", "0.2.8-a"));
        assert!(ok(">=0.2.8-a", "0.2.8-b"));
        assert!(ok(">=0.2.8-a", "0.2.8"));
    }

    #[test]
    fn caret() {
        assert!(ok("^1.2.0", "1.3.0"));
        assert!(ok("^1.2.0", "1.3.0-beta"));
        assert!(!ok("^1.2.0", "2.0.0-beta"));
        assert!(!ok("^1.2.0", "1.1.0-beta"));
        assert!(ok("^0.2.8", "0.2.9-alpha"));
        assert!(!ok("^0.2.8", "0.3.0-alpha"));
    }

    #[test]
    fn tilde() {
        assert!(ok("~1.2.0", "1.2.5-beta"));
        assert!(!ok("~1.2.0", "1.3.0-beta"));
        assert!(ok("~1.2.0", "1.2.5"));
    }

    #[test]
    fn exact_pins_stay_strict() {
        assert!(ok("=1.0.0", "1.0.0"));
        assert!(!ok("=1.0.0", "1.0.0-beta"));
        assert!(ok("=1.0.0-beta", "1.0.0-beta"));
        assert!(!ok("=1.0.0-beta", "1.0.0-alpha"));
        assert!(!ok("=1.0.0-beta", "1.0.0"));
    }

    #[test]
    fn upper_bounds_use_the_base_version() {
        assert!(ok("<1.1.0", "1.0.5-beta"));
        assert!(!ok("<1.1.0", "1.1.0-beta"));
        assert!(ok(">=1.0.0, <2.0.0", "1.5.0-rc.1"));
        assert!(!ok(">=1.0.0, <2.0.0", "2.0.0-rc.1"));
    }
}
