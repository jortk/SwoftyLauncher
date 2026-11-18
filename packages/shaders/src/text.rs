//! Text helpers shared by the lint, the parser and the editor support.

use std::ops::Range;

use crate::Pos;

/// Byte offsets of line starts, for offset to line/UTF-16 column conversion.
pub(crate) struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self { text, starts }
    }

    pub(crate) fn pos(&self, offset: usize) -> Pos {
        let mut offset = offset.min(self.text.len());
        while !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        let line = self.starts.partition_point(|&s| s <= offset) - 1;
        let character = self.text[self.starts[line]..offset].encode_utf16().count();
        Pos {
            line: line as u32,
            character: character as u32,
        }
    }

    pub(crate) fn range(&self, range: &Range<usize>) -> (Pos, Pos) {
        (self.pos(range.start), self.pos(range.end))
    }
}

/// The line containing `offset`: its start and its end without the line break.
pub(crate) fn line_at(text: &str, offset: usize) -> Range<usize> {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    let start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
    let end = text[offset..].find('\n').map_or(text.len(), |i| offset + i);
    let end = if text[start..end].ends_with('\r') {
        end - 1
    } else {
        end
    };
    start..end
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The candidate a mistyped word most likely meant: the only candidate it is a prefix of, or the
/// closest within two edits (three for words of ten characters or more), ties going to the longest
/// shared prefix.
pub(crate) fn did_you_mean<'a>(word: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let word = word.to_ascii_lowercase();
    if word.is_empty() {
        return None;
    }
    let candidates: Vec<&str> = candidates.collect();
    let mut prefixed = candidates
        .iter()
        .filter(|c| c.to_ascii_lowercase().starts_with(&word));
    if let (Some(first), None) = (prefixed.next(), prefixed.next()) {
        return Some(first);
    }
    let len = word.chars().count();
    let limit = if len >= 10 { 3 } else { 2 };
    candidates
        .into_iter()
        .map(|c| {
            let lower = c.to_ascii_lowercase();
            let shared = word
                .chars()
                .zip(lower.chars())
                .take_while(|(a, b)| a == b)
                .count();
            (distance(&word, &lower), std::cmp::Reverse(shared), c)
        })
        .filter(|(d, _, _)| *d <= limit && *d < len)
        .min_by_key(|(d, shared, _)| (*d, *shared))
        .map(|(_, _, c)| c)
}

/// A number the way a config states it: whole numbers without a fraction for ints, `1.0` for
/// whole floats.
pub(crate) fn number(v: f64, float: bool) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        if float {
            format!("{v:.1}")
        } else {
            format!("{}", v as i64)
        }
    } else {
        format!("{v}")
    }
}

pub(crate) fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_count_utf16_units() {
        let text = "héllo\n😀x = 1\nz";
        let lines = Lines::new(text);
        assert_eq!(
            lines.pos(0),
            Pos {
                line: 0,
                character: 0
            }
        );
        assert_eq!(
            lines.pos(text.find('l').unwrap()),
            Pos {
                line: 0,
                character: 2
            }
        );
        assert_eq!(
            lines.pos(text.find('x').unwrap()),
            Pos {
                line: 1,
                character: 2
            }
        );
        assert_eq!(
            lines.pos(text.find('z').unwrap()),
            Pos {
                line: 2,
                character: 0
            }
        );
        assert_eq!(
            lines.pos(text.len()),
            Pos {
                line: 2,
                character: 1
            }
        );
    }

    #[test]
    fn a_mid_character_offset_snaps_back() {
        let lines = Lines::new("é");
        assert_eq!(
            lines.pos(1),
            Pos {
                line: 0,
                character: 0
            }
        );
    }

    #[test]
    fn suggestions_use_prefix_then_distance() {
        let c = ["bloom", "blur", "grain"];
        assert_eq!(did_you_mean("gra", c.iter().copied()), Some("grain"));
        assert_eq!(did_you_mean("blom", c.iter().copied()), Some("bloom"));
        assert_eq!(did_you_mean("bl", c.iter().copied()), None);
        assert_eq!(did_you_mean("zzzzz", c.iter().copied()), None);
        assert_eq!(did_you_mean("GRAIN", c.iter().copied()), Some("grain"));
    }

    #[test]
    fn long_words_allow_three_edits() {
        let c = ["highlight_saturation", "highlight_tint", "shadow_saturation"];
        assert_eq!(
            did_you_mean("highlite_saturation", c.iter().copied()),
            Some("highlight_saturation")
        );
        assert_eq!(
            did_you_mean("shadw_saturtion", c.iter().copied()),
            Some("shadow_saturation")
        );
        assert_eq!(did_you_mean("hilite_sat", c.iter().copied()), None);
    }

    #[test]
    fn near_miss_effect_ids_resolve() {
        let c = [
            "ambient_light",
            "auto_hdr",
            "contrast_stretch",
            "curved_monitor",
            "color_mod",
        ];
        assert_eq!(
            did_you_mean("ambiant_light", c.iter().copied()),
            Some("ambient_light")
        );
        assert_eq!(
            did_you_mean("contrast_strech", c.iter().copied()),
            Some("contrast_stretch")
        );
        assert_eq!(
            did_you_mean("curved_monitr", c.iter().copied()),
            Some("curved_monitor")
        );
        assert_eq!(did_you_mean("colour_mod", c.iter().copied()), Some("color_mod"));
        assert_eq!(did_you_mean("autohdr", c.iter().copied()), Some("auto_hdr"));
        assert_eq!(did_you_mean("qqqq", c.iter().copied()), None);
    }

    #[test]
    fn numbers_keep_their_kind() {
        assert_eq!(number(2.0, true), "2.0");
        assert_eq!(number(2.0, false), "2");
        assert_eq!(number(0.25, true), "0.25");
        assert_eq!(number(-3.0, false), "-3");
    }

    #[test]
    fn line_at_drops_the_break() {
        let t = "ab\r\ncd\nef";
        assert_eq!(&t[line_at(t, 1)], "ab");
        assert_eq!(&t[line_at(t, 5)], "cd");
        assert_eq!(&t[line_at(t, 9)], "ef");
    }
}
