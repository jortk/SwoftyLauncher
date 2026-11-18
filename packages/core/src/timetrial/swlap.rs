//! The prefix of a `.swlap` lap file: the magic and the header JSON.

use std::io::Read;
use std::path::Path;

use serde::Deserialize;

pub const MAGIC: &[u8; 8] = b"SWLAP\0\x01\0";
const HEADER_MAX: usize = 1 << 20;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Header {
    #[serde(default)]
    pub format: u32,
    #[serde(default)]
    pub core: Option<String>,
    #[serde(default)]
    pub game_build: Option<String>,
    #[serde(default)]
    pub board: Board,
    #[serde(default)]
    pub car: Car,
    #[serde(default)]
    pub lap: Lap,
    #[serde(default)]
    pub nonce: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Board {
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub track: String,
    #[serde(default)]
    pub weather: String,
    #[serde(default)]
    pub game_board: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Car {
    #[serde(default)]
    pub team: Option<String>,
    #[serde(default)]
    pub vehicle: Option<String>,
    #[serde(default)]
    pub driver: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Lap {
    #[serde(default)]
    pub time_ms: u64,
    #[serde(default)]
    pub sectors_ms: Vec<u64>,
    #[serde(default)]
    pub valid: bool,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub lap_index: Option<u32>,
    #[serde(default)]
    pub started_utc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreadable {
    TooShort,
    BadMagic,
    HeaderTooBig(u32),
    BadHeader(String),
}

impl std::fmt::Display for Unreadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => f.write_str("the lap file ends inside its header"),
            Self::BadMagic => f.write_str("not a lap file"),
            Self::HeaderTooBig(n) => write!(f, "the lap file's header claims {n} bytes"),
            Self::BadHeader(e) => write!(f, "the lap file's header is not valid: {e}"),
        }
    }
}

impl std::error::Error for Unreadable {}

/// The header of the lap file in `bytes`, which only has to hold the magic and the header.
pub fn parse(bytes: &[u8]) -> Result<Header, Unreadable> {
    if bytes.len() < 12 {
        return Err(Unreadable::TooShort);
    }
    if &bytes[..8] != MAGIC {
        return Err(Unreadable::BadMagic);
    }
    let len = u32::from_le_bytes(bytes[8..12].try_into().expect("4 bytes"));
    if len as usize > HEADER_MAX {
        return Err(Unreadable::HeaderTooBig(len));
    }
    let end = 12 + len as usize;
    let json = bytes.get(12..end).ok_or(Unreadable::TooShort)?;
    serde_json::from_slice(json).map_err(|e| Unreadable::BadHeader(e.to_string()))
}

/// [`parse`] on the start of a file, reading no more than the header needs.
pub fn read(path: &Path) -> Result<Header, ReadError> {
    let mut file = std::fs::File::open(path).map_err(ReadError::Io)?;
    let mut prefix = [0u8; 12];
    let n = read_full(&mut file, &mut prefix).map_err(ReadError::Io)?;
    if n < 12 {
        return Err(ReadError::Unreadable(Unreadable::TooShort));
    }
    let len = u32::from_le_bytes(prefix[8..12].try_into().expect("4 bytes")) as usize;
    let mut bytes = prefix.to_vec();
    if &prefix[..8] == MAGIC && len <= HEADER_MAX {
        bytes.resize(12 + len, 0);
        let got = read_full(&mut file, &mut bytes[12..]).map_err(ReadError::Io)?;
        bytes.truncate(12 + got);
    }
    parse(&bytes).map_err(ReadError::Unreadable)
}

fn read_full(file: &mut std::fs::File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match file.read(&mut buf[n..])? {
            0 => break,
            k => n += k,
        }
    }
    Ok(n)
}

#[derive(Debug)]
pub enum ReadError {
    Io(std::io::Error),
    Unreadable(Unreadable),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Unreadable(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ReadError {}

#[cfg(test)]
pub(crate) fn sample(time_ms: u64, valid: bool) -> Vec<u8> {
    let header = serde_json::json!({
        "format": 1,
        "board": {"content": "game/f1_2025", "track": "bahrain", "weather": "dry"},
        "lap": {"time_ms": time_ms, "valid": valid, "sectors_ms": [1, 2, 3]},
    })
    .to_string();
    let mut out = MAGIC.to_vec();
    out.extend((header.len() as u32).to_le_bytes());
    out.extend(header.as_bytes());
    out.extend(b"END\0\0\0\0\0");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_header() {
        let h = parse(&sample(91234, true)).unwrap();
        assert_eq!(h.lap.time_ms, 91234);
        assert!(h.lap.valid);
        assert_eq!(h.board.track, "bahrain");
        assert_eq!(h.lap.sectors_ms, [1, 2, 3]);
    }

    #[test]
    fn refuses_what_is_not_a_lap() {
        assert_eq!(parse(b"short"), Err(Unreadable::TooShort));
        assert_eq!(parse(b"NOTALAP\0\0\0\0\0"), Err(Unreadable::BadMagic));
        let mut cut = sample(1, true);
        cut.truncate(20);
        assert_eq!(parse(&cut), Err(Unreadable::TooShort));
        let mut big = MAGIC.to_vec();
        big.extend(u32::MAX.to_le_bytes());
        assert_eq!(parse(&big), Err(Unreadable::HeaderTooBig(u32::MAX)));
        let mut bad = MAGIC.to_vec();
        bad.extend(3u32.to_le_bytes());
        bad.extend(b"{{{");
        assert!(matches!(parse(&bad), Err(Unreadable::BadHeader(_))));
    }

    #[test]
    fn reads_a_file_prefix() {
        let dir = std::env::temp_dir().join(format!("swofty-swlap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let ok = dir.join("a.swlap");
        std::fs::write(&ok, sample(5, false)).unwrap();
        assert_eq!(read(&ok).unwrap().lap.time_ms, 5);
        let bad = dir.join("b.swlap");
        std::fs::write(&bad, b"xx").unwrap();
        assert!(matches!(
            read(&bad),
            Err(ReadError::Unreadable(Unreadable::TooShort))
        ));
        assert!(matches!(read(&dir.join("none")), Err(ReadError::Io(_))));
        std::fs::remove_dir_all(&dir).ok();
    }
}
