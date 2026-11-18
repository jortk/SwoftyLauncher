//! Which `F1_25.exe` builds the launcher can load add-ons into. The exe's own size and PE link timestamp
//! identify a build (the loader's analysed-build check and `re/db/builds/*.json` use the same pair); a Steam
//! build id alone never decides it.

use std::collections::HashMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

pub const ALLOW_ENV: &str = "SWOFTY_ALLOW_UNSUPPORTED_BUILD";

pub const BUILD_1_0_145: (u64, u32) = (391_613_312, 1_794_312_636);

const SUPPORTED: &[(u64, u32)] = &[
    (408_063_872, 1_789_118_616),
    (342_936_448, 1_789_128_892),
    BUILD_1_0_145,
    (355_003_776, 1_794_315_435),
];

pub fn identity(exe: &Path) -> Option<(u64, u32)> {
    let size = std::fs::metadata(exe).ok()?.len();
    let stamp = swofty_formats::pe::timestamp(&mut BufReader::new(std::fs::File::open(exe).ok()?)).ok()?;
    Some((size, stamp))
}

pub fn is_supported(size: u64, pe_timestamp: u32) -> bool {
    SUPPORTED.contains(&(size, pe_timestamp))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Support {
    Supported,
    Unsupported { version: Option<String> },
    Unreadable,
}

pub fn bypassed() -> bool {
    std::env::var(ALLOW_ENV).is_ok_and(|v| !matches!(v.trim(), "" | "0"))
}

type Stamp = (u64, Option<SystemTime>);
static SEEN: Mutex<Option<HashMap<PathBuf, (Stamp, Support)>>> = Mutex::new(None);

pub fn check(exe: &Path) -> Support {
    let Ok(meta) = std::fs::metadata(exe) else {
        return Support::Unreadable;
    };
    let stamp = (meta.len(), meta.modified().ok());
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((s, support)) = seen.as_ref().and_then(|m| m.get(exe))
        && *s == stamp
    {
        return support.clone();
    }
    let support = match std::fs::File::open(exe)
        .ok()
        .and_then(|f| swofty_formats::pe::timestamp(&mut BufReader::new(f)).ok())
    {
        None => Support::Unreadable,
        Some(t) if is_supported(meta.len(), t) => Support::Supported,
        Some(_) => Support::Unsupported {
            version: crate::exe_version::read(exe),
        },
    };
    seen.get_or_insert_with(HashMap::new)
        .insert(exe.to_path_buf(), (stamp, support.clone()));
    support
}

/// The version label of an exe the launcher must not load add-ons into: unsupported, and not bypassed.
pub fn blocked(exe: &Path) -> Option<String> {
    blocked_with(check(exe), bypassed())
}

fn blocked_with(support: Support, bypass: bool) -> Option<String> {
    match support {
        Support::Unsupported { version } if !bypass => Some(version.unwrap_or_else(|| "unknown".to_owned())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exe(stamp: u32) -> Vec<u8> {
        let mut out = vec![0; 0x200];
        out[..2].copy_from_slice(b"MZ");
        out[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        out[0x80..0x84].copy_from_slice(b"PE\0\0");
        out[0x88..0x8c].copy_from_slice(&stamp.to_le_bytes());
        out
    }

    #[test]
    fn the_steam_and_ea_builds_are_supported() {
        assert!(is_supported(408_063_872, 1_789_118_616));
        assert!(is_supported(342_936_448, 1_789_128_892));
        assert!(is_supported(391_613_312, 1_794_312_636));
        assert!(is_supported(355_003_776, 1_794_315_435));
    }

    #[test]
    fn a_new_build_is_not_supported_and_neither_is_half_a_match() {
        assert!(!is_supported(400_000_000, 1_800_000_000));
        assert!(!is_supported(408_063_872, 1_800_000_000));
        assert!(!is_supported(400_000_000, 1_789_118_616));
    }

    #[test]
    fn an_unknown_exe_is_unsupported_and_a_missing_one_is_unreadable() {
        let dir = std::env::temp_dir().join(format!("swofty-build-support-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("F1_25.exe");
        std::fs::write(&path, exe(1_800_000_000)).unwrap();
        assert!(matches!(check(&path), Support::Unsupported { .. }));
        assert_eq!(check(&dir.join("none.exe")), Support::Unreadable);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_bypass_lifts_the_block_only_for_unsupported_builds() {
        let bad = Support::Unsupported {
            version: Some("1.0.145.2472".into()),
        };
        assert_eq!(blocked_with(bad.clone(), false).as_deref(), Some("1.0.145.2472"));
        assert_eq!(blocked_with(bad, true), None);
        assert_eq!(blocked_with(Support::Supported, false), None);
        assert_eq!(blocked_with(Support::Unreadable, false), None);
    }
}
