//! F1 25 `GfxMaterialRes` (`.material`) constants: read them and change their values in place.
//!
//! Layout: `re/db/subsystems/gfx_material.md`. A record is 9 u32s after a 0x20-byte header; a
//! constant's value sits in the value blob at `base + at`, where `base = 0x20 + 36 * nparams`.

use std::fmt;

const HEADER: usize = 0x20;
const RECORD: usize = 36;
const CONSTANT: u32 = 1;
const FLOAT: u32 = 3;
const UINT: u32 = 6;
const VALUE: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    Truncated,
    NoConstant(String),
    NotAValue(String),
    Width {
        name: String,
        want: usize,
        got: usize,
    },
    /// The bytes are not a canonical F1 25 material: what is wrong.
    Invalid(String),
    /// The material has no parameter of this name.
    NoParam(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated => f.write_str("the material is truncated or not an F1 25 material"),
            Error::NoConstant(n) => write!(f, "the material has no constant {n}"),
            Error::NotAValue(n) => write!(f, "{n} is a bindless heap index, not a value"),
            Error::Width { name, want, got } => {
                write!(f, "{name} takes {want} value(s), not {got}")
            }
            Error::Invalid(why) => write!(f, "the material is not a valid F1 25 material: {why}"),
            Error::NoParam(n) => write!(f, "the material has no parameter {n}"),
        }
    }
}

impl std::error::Error for Error {}

/// One constant of a material.
#[derive(Debug, Clone, PartialEq)]
pub struct Constant {
    pub name: String,
    /// True for floats, false for uints.
    pub float: bool,
    pub width: usize,
    /// Where its value starts in the material's bytes.
    pub at: usize,
}

fn u32_at(b: &[u8], off: usize) -> Result<u32, Error> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes(s.try_into().expect("4 bytes")))
        .ok_or(Error::Truncated)
}

fn cstr(b: &[u8], off: usize) -> Result<String, Error> {
    let rest = b.get(off..).ok_or(Error::Truncated)?;
    let end = rest.iter().position(|&c| c == 0).ok_or(Error::Truncated)?;
    Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
}

/// The value constants of an F1 25 material, in record order.
pub fn constants(b: &[u8]) -> Result<Vec<Constant>, Error> {
    let n = u32_at(b, 0)? as usize;
    let value_size = u32_at(b, 8)? as usize;
    let base = HEADER
        .checked_add(n.checked_mul(RECORD).ok_or(Error::Truncated)?)
        .ok_or(Error::Truncated)?;
    let names = base + u32_at(b, 0x10)? as usize;
    let mut out = Vec::new();
    for i in 0..n {
        let r = HEADER + i * RECORD;
        if u32_at(b, r + 4)? != CONSTANT {
            continue;
        }
        let ty = u32_at(b, r + 8)?;
        let class = u32_at(b, r + 12)?;
        let size = u32_at(b, r + 16)? as usize;
        let at = u32_at(b, r + 28)? as usize;
        let name = cstr(b, names + u32_at(b, r)? as usize)?;
        if at + size > value_size || base + at + size > b.len() {
            return Err(Error::Truncated);
        }
        let width = ((ty >> 16) as usize).max(1);
        let float = ty & 0xffff == FLOAT;
        if class != VALUE || !(float || ty & 0xffff == UINT) || size != width * 4 {
            out.push(Constant {
                name,
                float,
                width: 0,
                at: base + at,
            });
            continue;
        }
        out.push(Constant {
            name,
            float,
            width,
            at: base + at,
        });
    }
    Ok(out)
}

/// Sets constant `name` to `values`, written as f32 or u32 as the constant is declared. The
/// material keeps its size and every other byte.
pub fn set(b: &mut [u8], name: &str, values: &[f64]) -> Result<(), Error> {
    let c = constants(b)?
        .into_iter()
        .find(|c| c.name == name)
        .ok_or_else(|| Error::NoConstant(name.to_owned()))?;
    if c.width == 0 {
        return Err(Error::NotAValue(name.to_owned()));
    }
    if values.len() != c.width {
        return Err(Error::Width {
            name: name.to_owned(),
            want: c.width,
            got: values.len(),
        });
    }
    for (k, v) in values.iter().enumerate() {
        let bytes = if c.float {
            (*v as f32).to_le_bytes()
        } else {
            (v.max(0.0) as u32).to_le_bytes()
        };
        b[c.at + k * 4..c.at + k * 4 + 4].copy_from_slice(&bytes);
    }
    Ok(())
}

/// The current value of constant `name`.
pub fn get(b: &[u8], name: &str) -> Result<Vec<f64>, Error> {
    let c = constants(b)?
        .into_iter()
        .find(|c| c.name == name)
        .ok_or_else(|| Error::NoConstant(name.to_owned()))?;
    if c.width == 0 {
        return Err(Error::NotAValue(name.to_owned()));
    }
    Ok((0..c.width)
        .map(|k| {
            let raw = u32_at(b, c.at + k * 4).expect("checked by constants");
            if c.float {
                f32::from_bits(raw) as f64
            } else {
                raw as f64
            }
        })
        .collect())
}

const KIND_TEXTURE: u32 = 2;

/// What a [`Param`] holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParamValue {
    /// A constant's bytes, as the shader reads them.
    Bytes(Vec<u8>),
    /// A texture slot's resource path.
    Texture(String),
}

/// One parameter record of a [`Material`], with the values the writer needs to lay it out again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub value: ParamValue,
    /// Record type: constants low16 3 float or 6 uint, high16 vector width; textures 2 or 6.
    pub ty: u32,
    /// Record class: 1 value, 3 bindless heap index.
    pub class: u32,
    pub cbuffer: u32,
    pub offset: u32,
    /// For a heap index constant (`me_XHeapIndex`): the texture it stands for.
    pub link: Option<String>,
}

impl Param {
    /// True for a bindless heap index constant, the ones that carry a [`Param::link`].
    pub fn is_heap(&self) -> bool {
        matches!(self.value, ParamValue::Bytes(_)) && (self.ty, self.class) == (6, 3)
    }
}

/// A whole F1 25 `GfxMaterialRes`: shader, parameter records (texture slots and their heap
/// links included) and constant buffers. [`Material::to_bytes`] of a parsed stock material equals
/// the stock bytes; port of `tools/gfx_material.py` `parse25`/`write25`. Layout:
/// `re/db/subsystems/gfx_material.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Material {
    /// The shader path, `eaid://shader_package/<group>/<name>.nefx2`.
    pub shader: String,
    pub params: Vec<Param>,
    /// Constant buffer (name, size) pairs.
    pub cbuffers: Vec<(String, u32)>,
}

#[derive(Default)]
struct Pool {
    buf: Vec<u8>,
    seen: std::collections::HashMap<String, u32>,
}

impl Pool {
    fn add(&mut self, s: &str) -> u32 {
        if let Some(&o) = self.seen.get(s) {
            return o;
        }
        let o = self.buf.len() as u32;
        self.seen.insert(s.to_owned(), o);
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(0);
        o
    }
}

fn invalid(why: impl Into<String>) -> Error {
    Error::Invalid(why.into())
}

impl Material {
    /// Parses an F1 25 material and refuses anything the writer would not reproduce byte for byte.
    pub fn parse(b: &[u8]) -> Result<Self, Error> {
        if b.len() < HEADER {
            return Err(Error::Truncated);
        }
        let h: Vec<usize> = (0..8)
            .map(|i| u32_at(b, i * 4).map(|v| v as usize))
            .collect::<Result<_, _>>()?;
        let (n, ncb, vsz) = (h[0], h[1], h[2]);
        let (o_sh, o_nm, o_t1, o_t2, o_cb) = (h[3], h[4], h[5], h[6], h[7]);
        let base = HEADER + RECORD * n;
        let ordered = vsz + 8 * ncb == o_sh && o_sh <= o_nm && o_nm <= o_t1 && o_t1 <= o_t2 && o_t2 <= o_cb;
        if n > b.len() || ncb > b.len() || base + o_cb > b.len() || !ordered {
            return Err(invalid("bad header"));
        }
        let mut cbuffers = Vec::new();
        for i in 0..ncb {
            let at = base + vsz + i * 8;
            cbuffers.push((
                cstr(b, base + o_cb + u32_at(b, at)? as usize)?,
                u32_at(b, at + 4)?,
            ));
        }
        let mut params = Vec::new();
        for i in 0..n {
            let r = HEADER + i * RECORD;
            let f: Vec<u32> = (0..9).map(|k| u32_at(b, r + k * 4)).collect::<Result<_, _>>()?;
            let (nm, kind, ty, class, size, cbuffer, offset, at, link) = (
                f[0],
                f[1],
                f[2],
                f[3],
                f[4] as usize,
                f[5],
                f[6],
                f[7] as usize,
                f[8],
            );
            let name = cstr(b, base + o_nm + nm as usize)?;
            let mut p = Param {
                name: name.clone(),
                value: ParamValue::Bytes(Vec::new()),
                ty,
                class,
                cbuffer,
                offset,
                link: None,
            };
            if kind == CONSTANT {
                if at + size > vsz {
                    return Err(invalid(format!("{name}: value outside blob")));
                }
                p.value = ParamValue::Bytes(b[base + at..base + at + size].to_vec());
                if (ty, class) == (6, 3) {
                    p.link = Some(cstr(b, base + o_t2 + link as usize)?);
                } else if link != 0 {
                    return Err(invalid(format!("{name}: link on a non-heap constant")));
                }
            } else if kind == KIND_TEXTURE {
                let path = cstr(b, base + o_t1 + at)?;
                if size != path.len() + 1 || link != 0 {
                    return Err(invalid(format!("{name}: bad texture record")));
                }
                p.value = ParamValue::Texture(path);
            } else {
                return Err(invalid(format!("{name}: unknown kind {kind}")));
            }
            params.push(p);
        }
        let m = Material {
            shader: cstr(b, base + o_sh)?,
            params,
            cbuffers,
        };
        if m.to_bytes() != b {
            return Err(invalid("not canonical: re-encoding differs"));
        }
        Ok(m)
    }

    /// The material's bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let (mut names, mut list1, mut list2, mut cbn) =
            (Pool::default(), Pool::default(), Pool::default(), Pool::default());
        let mut blob = Vec::new();
        let mut recs: Vec<[u32; 9]> = Vec::new();
        for p in &self.params {
            let nm = names.add(&p.name);
            match &p.value {
                ParamValue::Bytes(v) => {
                    let link = match (&p.link, p.is_heap()) {
                        (Some(l), true) => list2.add(l),
                        _ => 0,
                    };
                    recs.push([
                        nm,
                        CONSTANT,
                        p.ty,
                        p.class,
                        v.len() as u32,
                        p.cbuffer,
                        p.offset,
                        blob.len() as u32,
                        link,
                    ]);
                    blob.extend_from_slice(v);
                }
                ParamValue::Texture(t) => {
                    let at = list1.add(t);
                    recs.push([
                        nm,
                        KIND_TEXTURE,
                        p.ty,
                        p.class,
                        t.len() as u32 + 1,
                        p.cbuffer,
                        p.offset,
                        at,
                        0,
                    ]);
                }
            }
        }
        let mut cbt = Vec::new();
        for (name, size) in &self.cbuffers {
            cbt.extend_from_slice(&cbn.add(name).to_le_bytes());
            cbt.extend_from_slice(&size.to_le_bytes());
        }
        let mut shader = self.shader.as_bytes().to_vec();
        shader.push(0);
        let o_sh = (blob.len() + cbt.len()) as u32;
        let o_nm = o_sh + shader.len() as u32;
        let o_t1 = o_nm + names.buf.len() as u32;
        let o_t2 = o_t1 + list1.buf.len() as u32;
        let o_cb = o_t2 + list2.buf.len() as u32;
        let mut out = Vec::new();
        for v in [
            recs.len() as u32,
            self.cbuffers.len() as u32,
            blob.len() as u32,
            o_sh,
            o_nm,
            o_t1,
            o_t2,
            o_cb,
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for r in &recs {
            for v in r {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.extend_from_slice(&blob);
        out.extend_from_slice(&cbt);
        out.extend_from_slice(&shader);
        for pool in [&names, &list1, &list2, &cbn] {
            out.extend_from_slice(&pool.buf);
        }
        out
    }

    /// The parameter called `name`.
    pub fn param(&self, name: &str) -> Option<&Param> {
        self.params.iter().find(|p| p.name == name)
    }

    /// Points texture slot `slot` (`me_diffuseTex`) at resource `texture` (the SRV's name, the
    /// `.tif`), setting the slot and its `<slot>HeapIndex` link, as the stock materials pair them.
    pub fn link_texture(&mut self, slot: &str, texture: &str) -> Result<(), Error> {
        let heap = format!("{slot}HeapIndex");
        if !self.params.iter().any(|p| p.name == heap && p.is_heap()) {
            return Err(Error::NoParam(heap));
        }
        let tex = self
            .params
            .iter_mut()
            .find(|p| p.name == slot && matches!(p.value, ParamValue::Texture(_)))
            .ok_or_else(|| Error::NoParam(slot.to_owned()))?;
        tex.value = ParamValue::Texture(texture.to_owned());
        let idx = self
            .params
            .iter_mut()
            .find(|p| p.name == heap)
            .expect("checked above");
        idx.link = Some(texture.to_owned());
        Ok(())
    }

    /// The texture a slot points at.
    pub fn texture(&self, slot: &str) -> Option<&str> {
        match &self.param(slot)?.value {
            ParamValue::Texture(t) => Some(t),
            ParamValue::Bytes(_) => None,
        }
    }

    /// Sets a float constant (`me_colour`) to `values`; the width must match.
    pub fn set_floats(&mut self, name: &str, values: &[f32]) -> Result<(), Error> {
        let p = self
            .params
            .iter_mut()
            .find(|p| p.name == name)
            .ok_or_else(|| Error::NoParam(name.to_owned()))?;
        let ParamValue::Bytes(v) = &mut p.value else {
            return Err(Error::NotAValue(name.to_owned()));
        };
        if p.ty & 0xffff != FLOAT || p.class != VALUE || v.len() != values.len() * 4 {
            return Err(Error::Width {
                name: name.to_owned(),
                want: v.len() / 4,
                got: values.len(),
            });
        }
        for (k, x) in values.iter().enumerate() {
            v[k * 4..k * 4 + 4].copy_from_slice(&x.to_le_bytes());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn material() -> Vec<u8> {
        let names = b"me_colour\0me_count\0me_mapHeapIndex\0";
        let n = 3;
        let mut b = Vec::new();
        for v in [n, 1, 20, 20, 20, 0, 0, 0] {
            b.extend_from_slice(&(v as u32).to_le_bytes());
        }
        let records: [[u32; 9]; 3] = [
            [0, 1, 0x30003, 1, 12, 0, 0, 0, 0],
            [10, 1, 6, 1, 4, 0, 12, 12, 0],
            [19, 1, 6, 3, 4, 0, 16, 16, 0],
        ];
        for r in records {
            for v in r {
                b.extend_from_slice(&v.to_le_bytes());
            }
        }
        for v in [1.0f32, 1.0, 1.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&7u32.to_le_bytes());
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(names);
        b
    }

    #[test]
    fn sets_a_float_vector_and_a_uint_in_place() {
        let mut m = material();
        let len = m.len();
        assert_eq!(get(&m, "me_colour").unwrap(), [1.0, 1.0, 1.0]);
        set(&mut m, "me_colour", &[0.25, 1.0, 0.5]).unwrap();
        set(&mut m, "me_count", &[3.0]).unwrap();
        assert_eq!(get(&m, "me_colour").unwrap(), [0.25, 1.0, 0.5]);
        assert_eq!(get(&m, "me_count").unwrap(), [3.0]);
        assert_eq!(m.len(), len);
    }

    #[test]
    fn refuses_unknown_names_heap_indices_and_wrong_widths() {
        let mut m = material();
        assert_eq!(
            set(&mut m, "me_nope", &[1.0]),
            Err(Error::NoConstant("me_nope".into()))
        );
        assert_eq!(
            set(&mut m, "me_mapHeapIndex", &[1.0]),
            Err(Error::NotAValue("me_mapHeapIndex".into()))
        );
        assert_eq!(
            set(&mut m, "me_colour", &[1.0]),
            Err(Error::Width {
                name: "me_colour".into(),
                want: 3,
                got: 1
            })
        );
        assert_eq!(constants(&m[..10]), Err(Error::Truncated));
    }

    fn full() -> Material {
        let tex = "eaid://ui_package/art/a.tif";
        let p = |name: &str, value, ty, class, cbuffer, offset, link: Option<&str>| Param {
            name: name.into(),
            value,
            ty,
            class,
            cbuffer,
            offset,
            link: link.map(Into::into),
        };
        Material {
            shader: "eaid://shader_package/x/y.nefx2".into(),
            params: vec![
                p(
                    "me_colour",
                    ParamValue::Bytes(
                        [1.0f32, 1.0, 1.0, 1.0]
                            .iter()
                            .flat_map(|f| f.to_le_bytes())
                            .collect(),
                    ),
                    0x40003,
                    1,
                    0,
                    0,
                    None,
                ),
                p(
                    "me_diffuseTexHeapIndex",
                    ParamValue::Bytes(vec![0; 4]),
                    6,
                    3,
                    0,
                    16,
                    Some(tex),
                ),
                p(
                    "me_diffuseTex",
                    ParamValue::Texture(tex.into()),
                    2,
                    1,
                    0xffff_ffff,
                    0,
                    None,
                ),
            ],
            cbuffers: vec![("PerMaterial".into(), 20)],
        }
    }

    #[test]
    fn a_whole_material_round_trips_and_links_a_texture() {
        let mut m = full();
        let bytes = m.to_bytes();
        assert_eq!(Material::parse(&bytes).unwrap(), m);
        m.link_texture("me_diffuseTex", "eaid://ui_package/art/b.tif")
            .unwrap();
        m.set_floats("me_colour", &[0.5, 0.25, 1.0, 1.0]).unwrap();
        let back = Material::parse(&m.to_bytes()).unwrap();
        assert_eq!(back.texture("me_diffuseTex"), Some("eaid://ui_package/art/b.tif"));
        assert_eq!(
            back.param("me_diffuseTexHeapIndex").unwrap().link.as_deref(),
            Some("eaid://ui_package/art/b.tif")
        );
        assert_eq!(get(&back.to_bytes(), "me_colour").unwrap(), [0.5, 0.25, 1.0, 1.0]);
    }

    #[test]
    fn linking_needs_the_slot_and_its_heap_index() {
        let mut m = full();
        assert_eq!(
            m.link_texture("me_nope", "x"),
            Err(Error::NoParam("me_nopeHeapIndex".into()))
        );
        m.params.remove(2);
        assert_eq!(
            m.link_texture("me_diffuseTex", "x"),
            Err(Error::NoParam("me_diffuseTex".into()))
        );
        assert!(Material::parse(&full().to_bytes()[..40]).is_err());
    }
}
