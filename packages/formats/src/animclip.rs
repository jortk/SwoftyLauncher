//! UI animation clips: `AnimClip` resources and the scene's `ACStringTable`.
//!
//! A clip animates one scene element (`<scene>.idf/<element>?context=animation`). Its `temp`
//! fragment is a `!pkg` (`ANIM`) with one packet per channel (position, rotation or scale
//! keys); its `syst` fragment holds the key times and key values the packets point into. The
//! scene's string table (`<scene>.idf?context=animstringtable&file=0`, a `!pkg` `ANST`) lists
//! the element names the clips use, sorted; a clip refers to its element by index. A scene's
//! `UIXA` node then plays a clip from a view-model fraction.
//!
//! Clips are built from a stock clip of the same scene (any `AnimClip` resource), which gives
//! the packet shape. The writer reproduces a stock clip's `temp` byte for byte when it is given
//! the clip's own channels back ([`read_clip`] then [`build_clip`]), and its `syst` up to the zero
//! padding that ends every written `syst` on a 16-byte multiple ([`SYST_ALIGN`]).
//!
//! ```no_run
//! use swofty_formats::animclip::{Channel, build_clip};
//!
//! # fn demo(stock_temp: &[u8]) -> swofty_formats::Result<()> {
//! // Slide a mask from x = -1 to 1 over 1.5 seconds, node index 18 of the string table.
//! let slide = Channel::position(vec![0.0, 1.5], vec![[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
//! let (temp, syst) = build_clip(
//!     stock_temp, "zh_fill_anim", 18, "eaid://ui_package/art/x.idf?context=animstringtable&file=0",
//!     &[slide],
//! )?;
//! # let _ = (temp, syst); Ok(()) }
//! ```

use crate::pkg::{Node, Pkg, Val, Value};
use crate::{Error, Result};

/// What a channel animates. The number is the packet's type field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Quaternion x, y, z, w keys.
    Rotation = 1,
    /// Scale x, y, z keys.
    Scale = 2,
    /// Position x, y, z keys.
    Position = 3,
}

impl Target {
    /// Floats per key.
    pub fn width(self) -> usize {
        match self {
            Target::Rotation => 4,
            Target::Scale | Target::Position => 3,
        }
    }

    fn of(v: u32) -> Option<Self> {
        match v {
            1 => Some(Target::Rotation),
            2 => Some(Target::Scale),
            3 => Some(Target::Position),
            _ => None,
        }
    }
}

/// One animated channel of a clip.
#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    pub target: Target,
    /// The packet's interpolator: 5 for position and scale, 2 for rotation in every clip seen.
    pub interpolator: u32,
    /// The channel flags: 0x80, or 0x84 for rotation.
    pub flags: u32,
    /// The packet's `maxtime`: the last key time in every stock clip (0 for a one key channel).
    pub max_time: f32,
    /// The packet's `mintime`: 0 unless the clip starts later than its first key.
    pub min_time: f32,
    pub times: Vec<f32>,
    /// `times.len()` keys of `target.width()` floats, back to back.
    pub keys: Vec<f32>,
}

impl Channel {
    /// A position channel: one `[x, y, z]` per time.
    pub fn position(times: Vec<f32>, keys: Vec<[f32; 3]>) -> Self {
        Self::make(Target::Position, 5, 0x80, times, keys.concat())
    }

    /// A scale channel: one `[x, y, z]` per time.
    pub fn scale(times: Vec<f32>, keys: Vec<[f32; 3]>) -> Self {
        Self::make(Target::Scale, 5, 0x80, times, keys.concat())
    }

    /// A rotation channel: one quaternion `[x, y, z, w]` per time.
    pub fn rotation(times: Vec<f32>, keys: Vec<[f32; 4]>) -> Self {
        Self::make(Target::Rotation, 2, 0x84, times, keys.concat())
    }

    fn make(target: Target, interpolator: u32, flags: u32, times: Vec<f32>, keys: Vec<f32>) -> Self {
        Channel {
            target,
            interpolator,
            flags,
            max_time: times.last().copied().unwrap_or(0.0),
            min_time: 0.0,
            times,
            keys,
        }
    }
}

/// A clip as stored: the element it animates and its channels.
#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    /// The element's name, as in the scene.
    pub node: String,
    /// The element's index in the scene's string table.
    pub name_index: i32,
    /// The resource name of that string table.
    pub string_table: String,
    pub channels: Vec<Channel>,
}

fn raw(n: &mut Node, key: &str, bytes: Vec<u8>) -> Result<()> {
    let v = n
        .as_obj_mut()
        .and_then(|o| o.get_mut(key))
        .and_then(Node::as_val_mut)
        .ok_or_else(|| Error::Format(format!("the clip has no {key}")))?;
    v.value = Value::Raw(bytes);
    Ok(())
}

fn field<'a>(n: &'a Node, key: &str) -> Result<&'a Val> {
    n.as_obj()
        .and_then(|o| o.get(key))
        .and_then(Node::as_val)
        .ok_or_else(|| Error::Format(format!("the clip has no {key}")))
}

fn num(n: &Node, key: &str, width: usize) -> Result<Vec<u8>> {
    match &field(n, key)?.value {
        Value::Raw(b) if b.len() >= width => Ok(b.clone()),
        _ => Err(Error::Format(format!("{key} is not a {width} byte number"))),
    }
}

fn u32_of(n: &Node, key: &str) -> Result<u32> {
    Ok(u32::from_le_bytes(
        num(n, key, 4)?[..4].try_into().expect("4 bytes"),
    ))
}

fn f32_of(n: &Node, key: &str) -> Result<f32> {
    Ok(f32::from_bits(u32_of(n, key)?))
}

fn str_of(n: &Node, key: &str) -> Result<String> {
    field(n, key)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::Format(format!("{key} is not a string")))
}

fn packets(p: &mut Pkg) -> Result<&mut crate::pkg::Arr> {
    p.root
        .as_obj_mut()
        .and_then(|o| o.get_mut("packets"))
        .and_then(Node::as_arr_mut)
        .ok_or_else(|| Error::Format("the clip has no packets".into()))
}

/// The length every written `syst` is a multiple of. The game's `AnimClip` handler asks the
/// `EGO_ANIMATION_CLIPDATA` heap for the `syst` length plus 16 when that length is a multiple of 16,
/// and plus 7 otherwise. Requests up to 128 bytes come from 8-byte small-block size classes laid
/// back to back, which ignore the 16-byte alignment asked for, so a class that is 8 more than a
/// multiple of 16 puts every other buffer 8 bytes past a 16-byte boundary. A multiple of 16 always
/// lands in a 16-byte class, and the quaternion keys the game reads with aligned loads then sit
/// on 16 bytes.
pub const SYST_ALIGN: usize = 16;

fn pad_to(syst: &mut Vec<u8>, align: usize) {
    while !syst.len().is_multiple_of(align) {
        syst.push(0);
    }
}

/// Builds an `AnimClip`'s two fragments, `(temp, syst)`, animating element `node` (index
/// `name_index` in the scene's string table `string_table`).
/// `template` is the `temp` fragment of any stock clip; its first packet is the model. Every key
/// array starts on a 16-byte offset and the `syst` length is a multiple of [`SYST_ALIGN`].
pub fn build_clip(
    template: &[u8],
    node: &str,
    name_index: i32,
    string_table: &str,
    channels: &[Channel],
) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut p = Pkg::parse(template)?;
    let model = packets(&mut p)?
        .items
        .first()
        .cloned()
        .ok_or_else(|| Error::Format("the template clip has no packet".into()))?;
    let mut syst = Vec::new();
    let mut out = Vec::new();
    for c in channels {
        if c.keys.len() != c.times.len() * c.target.width() {
            return Err(Error::Format(format!(
                "{} time(s) need {} key float(s), not {}",
                c.times.len(),
                c.times.len() * c.target.width(),
                c.keys.len()
            )));
        }
        let times_at = syst.len();
        for t in &c.times {
            syst.extend_from_slice(&t.to_le_bytes());
        }
        pad_to(&mut syst, SYST_ALIGN);
        let keys_at = syst.len();
        for k in &c.keys {
            syst.extend_from_slice(&k.to_le_bytes());
        }
        let mut pk = model.clone();
        raw(&mut pk, "maxtime", c.max_time.to_le_bytes().to_vec())?;
        raw(&mut pk, "mintime", c.min_time.to_le_bytes().to_vec())?;
        raw(&mut pk, "interpolator", c.interpolator.to_le_bytes().to_vec())?;
        let mut ch = pk
            .as_obj()
            .and_then(|o| o.get("channels"))
            .and_then(Node::as_arr)
            .and_then(|a| a.items.first().cloned())
            .ok_or_else(|| Error::Format("the template packet has no channel".into()))?;
        raw(&mut ch, "name_index", name_index.to_le_bytes().to_vec())?;
        raw(&mut ch, "mins", vec![0; 8])?;
        let misc = (0x80210u64 << 32)
            | (u64::from(c.flags) << 24)
            | ((c.target as u64) << 16)
            | c.times.len() as u64;
        raw(&mut ch, "misc", misc.to_le_bytes().to_vec())?;
        ch.as_obj_mut()
            .and_then(|o| o.get_mut("name"))
            .and_then(Node::as_val_mut)
            .ok_or_else(|| Error::Format("the channel has no name".into()))?
            .set_str(node);
        raw(&mut ch, "keyptr", (keys_at as u32).to_le_bytes().to_vec())?;
        raw(&mut ch, "timesptr", (times_at as u32).to_le_bytes().to_vec())?;
        let arr = pk
            .as_obj_mut()
            .and_then(|o| o.get_mut("channels"))
            .and_then(Node::as_arr_mut)
            .expect("read above");
        arr.items = vec![ch];
        out.push(pk);
    }
    pad_to(&mut syst, SYST_ALIGN);
    packets(&mut p)?.items = out;
    p.root
        .as_obj_mut()
        .and_then(|o| o.get_mut("string_table"))
        .and_then(Node::as_val_mut)
        .ok_or_else(|| Error::Format("the clip has no string_table".into()))?
        .set_str(string_table);
    Ok((p.to_bytes()?, syst))
}

/// Reads a clip's fragments back into a [`Clip`]. Only the three channel kinds [`Channel`]
/// builds are understood (position, rotation, scale with one channel per packet); other stock
/// clips, with tangent keys and the like, are refused.
pub fn read_clip(temp: &[u8], syst: &[u8]) -> Result<Clip> {
    let mut p = Pkg::parse(temp)?;
    let string_table = str_of(&p.root, "string_table")?;
    let mut clip = Clip {
        node: String::new(),
        name_index: 0,
        string_table,
        channels: Vec::new(),
    };
    let short = || Error::Format("a clip pointer lies outside its syst fragment".into());
    let floats = |at: usize, n: usize| -> Result<Vec<f32>> {
        let bytes = syst.get(at..at + 4 * n).ok_or_else(short)?;
        Ok(bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().expect("4 bytes")))
            .collect())
    };
    for pk in &packets(&mut p)?.items {
        let ch = pk
            .as_obj()
            .and_then(|o| o.get("channels"))
            .and_then(Node::as_arr)
            .filter(|a| a.items.len() == 1)
            .and_then(|a| a.items.first())
            .ok_or_else(|| {
                Error::Format("unsupported clip: a packet does not have exactly one channel".into())
            })?;
        let misc = u64::from_le_bytes(num(ch, "misc", 8)?[..8].try_into().expect("8 bytes"));
        let interpolator = u32_of(pk, "interpolator")?;
        let flags = ((misc >> 24) & 0xff) as u32;
        let target = Target::of(((misc >> 16) & 0xff) as u32)
            .filter(|t| {
                matches!(
                    (interpolator, *t, flags),
                    (5, Target::Position, 0x80) | (2, Target::Rotation, 0x84) | (5, Target::Scale, 0x80)
                )
            })
            .ok_or_else(|| {
                Error::Format(format!(
                    "unsupported clip channel (interpolator {interpolator}, type {}, flags {flags:#x})",
                    (misc >> 16) & 0xff
                ))
            })?;
        let n = (misc & 0xffff) as usize;
        clip.node = str_of(ch, "name")?;
        clip.name_index = u32_of(ch, "name_index")? as i32;
        clip.channels.push(Channel {
            target,
            interpolator,
            flags,
            max_time: f32_of(pk, "maxtime")?,
            min_time: f32_of(pk, "mintime")?,
            times: floats(u32_of(ch, "timesptr")? as usize, n)?,
            keys: floats(u32_of(ch, "keyptr")? as usize, n * target.width())?,
        });
    }
    Ok(clip)
}

/// True when the clip carries a rotation channel of one key: a constant quaternion the game
/// copies with an aligned 16-byte load from the clip's `syst` buffer. A `syst` whose length is not
/// a multiple of [`SYST_ALIGN`] can land 8 bytes past a 16-byte boundary (a 92-byte one did), and
/// the load then faults.
pub fn has_constant_rotation(temp: &[u8], syst: &[u8]) -> Result<bool> {
    Ok(read_clip(temp, syst)?
        .channels
        .iter()
        .any(|c| c.target == Target::Rotation && c.times.len() == 1))
}

/// Appends element names to a scene's `ACStringTable` and returns the new table with the index
/// of the first appended name. A clip refers to its element by that index. Names must be new.
/// Most scenes' tables are sorted, and Halo's scenes keep them so: when the table is sorted, the
/// appended `names` must be sorted and sort after its last string, so it stays sorted. (Some
/// stock tables are not sorted; those take any new names.)
pub fn append_strings(table: &[u8], names: &[&str]) -> Result<(Vec<u8>, usize)> {
    let mut p = Pkg::parse(table)?;
    let strings = p
        .root
        .as_obj_mut()
        .and_then(|o| o.get_mut("strings"))
        .and_then(Node::as_arr_mut)
        .ok_or_else(|| Error::Format("the string table has no strings".into()))?;
    let have: Vec<String> = strings
        .items
        .iter()
        .filter_map(|n| n.as_val().and_then(Val::as_str).map(str::to_owned))
        .collect();
    let sorted = have.windows(2).all(|w| w[0] <= w[1]);
    let mut last = have.last().map(String::as_str);
    for (i, n) in names.iter().enumerate() {
        if have.iter().any(|h| h == n) || names[..i].contains(n) {
            return Err(Error::Format(format!("the string table already lists {n}")));
        }
        if sorted && last.is_some_and(|l| *n < l) {
            return Err(Error::Format(format!(
                "{n} does not sort after {}: the table is sorted, append names in order",
                last.unwrap_or_default()
            )));
        }
        last = Some(n);
    }
    let first = have.len();
    for n in names {
        strings.items.push(Node::Val(Val::str(*n)));
    }
    Ok((p.to_bytes()?, first))
}

/// The strings of an `ACStringTable`, in table order.
pub fn read_strings(table: &[u8]) -> Result<Vec<String>> {
    let p = Pkg::parse(table)?;
    let arr = p
        .root
        .as_obj()
        .and_then(|o| o.get("strings"))
        .and_then(Node::as_arr)
        .ok_or_else(|| Error::Format("the string table has no strings".into()))?;
    Ok(arr
        .items
        .iter()
        .filter_map(|n| n.as_val().and_then(Val::as_str).map(str::to_owned))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pkg::{Arr, Obj};

    fn raw(ty: &[u8; 4], b: &[u8]) -> Node {
        Node::Val(Val::raw(*ty, b))
    }

    fn pkg(name: &[u8; 4], items: Vec<(&str, Node)>) -> Pkg {
        Pkg {
            name: *name,
            root: Node::Obj(Obj::new(
                items.into_iter().map(|(k, v)| (Val::str(k), v)).collect(),
            )),
            vca: vec![],
            layout: None,
        }
    }

    fn template() -> Vec<u8> {
        let channel = Node::Obj(Obj::new(vec![
            (Val::str("name_index"), raw(b"si32", &[0; 4])),
            (Val::str("mins"), raw(b"ui64", &[0; 8])),
            (Val::str("misc"), raw(b"ui64", &[0; 8])),
            (Val::str("name"), Node::Val(Val::str("x"))),
            (Val::str("keyptr"), raw(b"ui32", &[0; 4])),
            (Val::str("timesptr"), raw(b"ui32", &[0; 4])),
        ]));
        let packet = Node::Obj(Obj::new(vec![
            (Val::str("maxtime"), raw(b"fp32", &[0; 4])),
            (Val::str("mintime"), raw(b"fp32", &[0; 4])),
            (Val::str("interpolator"), raw(b"ui32", &[0; 4])),
            (Val::str("channels"), Node::Arr(Arr::new(vec![channel]))),
        ]));
        pkg(
            b"ANIM",
            vec![
                ("packets", Node::Arr(Arr::new(vec![packet]))),
                ("string_table", Node::Val(Val::str("t"))),
            ],
        )
        .to_bytes()
        .unwrap()
    }

    fn channels() -> Vec<Channel> {
        vec![
            Channel::position(vec![0.0, 1.5], vec![[-1.0, 0.0, 0.5], [1.0, 2.0, 0.5]]),
            Channel::rotation(
                vec![0.0, 0.75, 1.5],
                vec![[0.0, 0.0, 0.0, -1.0], [0.0, 0.0, 0.5, -0.8], [0.0, 0.0, 1.0, 0.0]],
            ),
            Channel::scale(vec![0.0], vec![[1.0, 1.0, 1.0]]),
        ]
    }

    #[test]
    fn a_built_clip_reads_back_and_rebuilds_identically() {
        let (temp, syst) = build_clip(&template(), "zh_a", 7, "tbl", &channels()).unwrap();
        let clip = read_clip(&temp, &syst).unwrap();
        assert_eq!((clip.node.as_str(), clip.name_index), ("zh_a", 7));
        assert_eq!(clip.string_table, "tbl");
        assert_eq!(clip.channels, channels());
        let again = build_clip(
            &temp,
            &clip.node,
            clip.name_index,
            &clip.string_table,
            &clip.channels,
        )
        .unwrap();
        assert_eq!(again, (temp, syst));
    }

    #[test]
    fn a_one_key_rotation_is_told_apart() {
        let (temp, syst) = build_clip(&template(), "n", 0, "t", &channels()).unwrap();
        assert!(!has_constant_rotation(&temp, &syst).unwrap());
        let mut c = channels();
        c.push(Channel::rotation(vec![0.0], vec![[0.0, 0.0, 0.0, -1.0]]));
        let (temp, syst) = build_clip(&template(), "n", 0, "t", &c).unwrap();
        assert!(has_constant_rotation(&temp, &syst).unwrap());
    }

    fn key_offsets(temp: &[u8]) -> Vec<u32> {
        let mut p = Pkg::parse(temp).unwrap();
        packets(&mut p)
            .unwrap()
            .items
            .iter()
            .map(|pk| {
                let ch = pk
                    .as_obj()
                    .and_then(|o| o.get("channels"))
                    .and_then(Node::as_arr)
                    .and_then(|a| a.items.first())
                    .unwrap();
                u32_of(ch, "keyptr").unwrap()
            })
            .collect()
    }

    fn game_slot(syst_len: usize) -> usize {
        let request = syst_len + if syst_len.is_multiple_of(16) { 16 } else { 7 };
        let n = request - 1;
        let shift = if n < 64 { 3 } else { 3 + (n >> 6).ilog2() };
        ((n >> shift) + 1) << shift
    }

    fn needle(keys: usize) -> Vec<Channel> {
        let times = (0..keys).map(|i| i as f32).collect();
        vec![
            Channel::position(vec![0.0], vec![[0.1, 0.2, 0.0]]),
            Channel::rotation(times, vec![[0.0, 0.0, 0.0, -1.0]; keys]),
            Channel::scale(vec![0.0], vec![[1.0; 3]]),
        ]
    }

    fn slide(keys: usize) -> Vec<Channel> {
        let times = (0..keys).map(|i| i as f32).collect();
        vec![Channel::position(times, vec![[0.5, 0.25, 0.0]; keys])]
    }

    #[test]
    fn the_game_slot_model_matches_the_crashed_and_the_stock_buffers() {
        assert_eq!(game_slot(92), 104);
        assert_eq!(game_slot(108), 120);
        assert_eq!(game_slot(96), 112);
        assert_eq!(game_slot(124), 144);
        assert_eq!(game_slot(216), 224);
    }

    #[test]
    fn every_syst_is_padded_to_16_bytes_with_its_keys_on_16() {
        for keys in 1..=12 {
            for channels in [needle(keys), slide(keys), channels()] {
                let (temp, syst) = build_clip(&template(), "n", 0, "t", &channels).unwrap();
                assert!(
                    syst.len().is_multiple_of(SYST_ALIGN),
                    "{keys} keys: {}",
                    syst.len()
                );
                assert!(
                    game_slot(syst.len()).is_multiple_of(16),
                    "{keys} keys: {}",
                    syst.len()
                );
                assert!(key_offsets(&temp).iter().all(|k| k % 16 == 0));
                assert_eq!(read_clip(&temp, &syst).unwrap().channels, channels);
            }
        }
    }

    #[test]
    fn a_two_key_needle_is_no_longer_108_bytes() {
        let (_, syst) = build_clip(&template(), "n", 0, "t", &needle(2)).unwrap();
        assert_eq!(syst.len(), 112);
        assert_eq!(&syst[108..], &[0; 4]);
    }

    #[test]
    fn keys_must_match_times() {
        let mut c = channels();
        c[0].keys.pop();
        assert!(build_clip(&template(), "n", 0, "t", &c).is_err());
    }

    #[test]
    fn strings_append_in_order_only() {
        let t = pkg(
            b"ANST",
            vec![(
                "strings",
                Node::Arr(Arr::new(vec![
                    Node::Val(Val::str("Alpha")),
                    Node::Val(Val::str("Beta")),
                ])),
            )],
        )
        .to_bytes()
        .unwrap();
        let (out, first) = append_strings(&t, &["zh_a", "zh_b"]).unwrap();
        assert_eq!(first, 2);
        assert_eq!(read_strings(&out).unwrap(), ["Alpha", "Beta", "zh_a", "zh_b"]);
        assert!(append_strings(&t, &["Aaa"]).is_err());
        assert!(append_strings(&t, &["Alpha"]).is_err());
        assert!(append_strings(&t, &["zh_b", "zh_a"]).is_err());
    }
}
