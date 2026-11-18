//! `GfxMatBindRes` (`.emb`): which `.material` each material slot of a mesh uses.
//!
//! ```text
//! u32 n_slots, u32 n_unique_materials, u32 material_block_offset (relative to 0x0c)
//! n_slots x { u32 slot_name_offset (in the slot block), u32 material_offset (in the material block) }
//! slot block:     NUL terminated slot names, in first-use order, each once
//! material block: NUL terminated material paths, in first-use order, each once
//! ```
//!
//! Worked out from the stock files; writing a parsed stock binding gives the stock bytes.
//!
//! ```
//! use swofty_formats::emb::{parse, write};
//!
//! let bytes = write(&[("central_bg_fill".into(), "art/osd/materials/mine/dot.material".into())]);
//! assert_eq!(parse(&bytes).unwrap()[0].0, "central_bg_fill");
//! ```

use crate::{Error, Result};

fn word(b: &[u8], at: usize) -> Result<usize> {
    b.get(at..at + 4)
        .map(|s| u32::from_le_bytes(s.try_into().expect("4 bytes")) as usize)
        .ok_or(Error::Eof(at))
}

fn cstr(b: &[u8], at: usize) -> Result<String> {
    let rest = b.get(at..).ok_or(Error::Eof(at))?;
    let end = rest
        .iter()
        .position(|&c| c == 0)
        .ok_or_else(|| Error::Format(format!("unterminated string at {at:#x}")))?;
    Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
}

/// The (slot name, material path) pairs of a binding, in file order.
pub fn parse(b: &[u8]) -> Result<Vec<(String, String)>> {
    let n = word(b, 0)?;
    let moff = word(b, 8)?;
    if n > b.len() / 8 {
        return Err(Error::Format(format!(
            "{n} slots do not fit in {} bytes",
            b.len()
        )));
    }
    let (sbase, mbase) = (12 + 8 * n, 12 + moff);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (s, m) = (word(b, 12 + 8 * i)?, word(b, 16 + 8 * i)?);
        out.push((cstr(b, sbase + s)?, cstr(b, mbase + m)?));
    }
    Ok(out)
}

/// Writes a binding from (slot name, material path) pairs. Names and paths used twice are stored
/// once.
pub fn write(pairs: &[(String, String)]) -> Vec<u8> {
    fn pool<'a>(
        items: impl Iterator<Item = &'a String>,
    ) -> (Vec<u8>, std::collections::HashMap<&'a str, u32>) {
        let (mut block, mut at) = (Vec::new(), std::collections::HashMap::new());
        for s in items {
            if !at.contains_key(s.as_str()) {
                at.insert(s.as_str(), block.len() as u32);
                block.extend_from_slice(s.as_bytes());
                block.push(0);
            }
        }
        (block, at)
    }
    let (sblock, soffs) = pool(pairs.iter().map(|p| &p.0));
    let (mblock, moffs) = pool(pairs.iter().map(|p| &p.1));
    let n = pairs.len() as u32;
    let mut out = Vec::new();
    for v in [n, moffs.len() as u32, 8 * n + sblock.len() as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for (slot, mat) in pairs {
        out.extend_from_slice(&soffs[slot.as_str()].to_le_bytes());
        out.extend_from_slice(&moffs[mat.as_str()].to_le_bytes());
    }
    out.extend_from_slice(&sblock);
    out.extend_from_slice(&mblock);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(a: &str, b: &str) -> (String, String) {
        (a.into(), b.into())
    }

    #[test]
    fn round_trips_with_shared_names() {
        let pairs = vec![p("a", "m1"), p("b", "m1"), p("a", "m2")];
        let bytes = write(&pairs);
        assert_eq!(&bytes[..4], &3u32.to_le_bytes());
        assert_eq!(&bytes[4..8], &2u32.to_le_bytes());
        assert_eq!(parse(&bytes).unwrap(), pairs);
        assert_eq!(write(&parse(&bytes).unwrap()), bytes);
    }

    #[test]
    fn one_slot_has_a_known_layout() {
        let bytes = write(&[p("slot", "mat")]);
        let mut want = Vec::new();
        for v in [1u32, 1, 8 + 5, 0, 0] {
            want.extend_from_slice(&v.to_le_bytes());
        }
        want.extend_from_slice(b"slot\0mat\0");
        assert_eq!(bytes, want);
    }

    #[test]
    fn damaged_files_are_errors() {
        assert!(parse(&[]).is_err());
        assert!(parse(&[0xff; 12]).is_err());
        let mut b = write(&[p("slot", "mat")]);
        b.pop();
        assert!(parse(&b).is_err());
    }
}
