//! New GPU textures for F1 25: `GfxSurfaceRes` and `GfxSRVResource` fragment writers.
//!
//! A texture is a pair of resources. The surface `<name>.tif.image` holds a 40 byte header
//! fragment, a pixel fragment (every mip level, largest first) and optionally a stream
//! descriptor; the view `<name>.tif` holds a 24 byte header and the surface's name. Mirrors
//! `native/src/core/apply/content/garage_build.hpp` (`surface_header`, `srv_data`, `build`).
//!
//! ```
//! use swofty_formats::surface::{Surface, TextureResources};
//!
//! let rgba = vec![255u8; 16 * 16 * 4];
//! let surface = Surface::from_rgba(&rgba, 16, 16, true, false).unwrap();
//! let res = TextureResources::new("art/osd/textures/mine/dot", &surface);
//! assert_eq!(res.surface_name, "art/osd/textures/mine/dot.tif.image");
//! assert_eq!(res.surface_fragments[0].len(), 40);
//! ```

use crate::bc7_enc;
use crate::{Error, Result};

/// The format ids of `GfxSurfaceRes` headers (the engine's own numbering, not DXGI).
pub const FMT_BC1: u32 = 52;
pub const FMT_BC1_SRGB: u32 = 54;
pub const FMT_BC3: u32 = 55;
pub const FMT_BC3_SRGB: u32 = 57;
pub const FMT_BC5: u32 = 65;
pub const FMT_BC7: u32 = 69;
pub const FMT_BC7_SRGB: u32 = 70;

/// The ERP type argument of a `GfxSurfaceRes` resource.
pub const SURFACE_ARG: u32 = 9;
/// The ERP type argument of a `GfxSRVResource` resource.
pub const SRV_ARG: u32 = 2;

/// Bytes of one level of `format`; 0 for a format the writer does not know.
pub fn level_bytes(format: u32, w: u32, h: u32) -> usize {
    let block = match format {
        FMT_BC1 | FMT_BC1_SRGB => 8,
        FMT_BC3 | FMT_BC3_SRGB | FMT_BC5 | FMT_BC7 | FMT_BC7_SRGB => 16,
        _ => 0,
    };
    w.div_ceil(4) as usize * h.div_ceil(4) as usize * block
}

/// Bytes of a whole mip chain, largest level first.
pub fn chain_bytes(format: u32, mut w: u32, mut h: u32, levels: u32) -> usize {
    let mut total = 0;
    for _ in 0..levels {
        total += level_bytes(format, w, h);
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }
    total
}

/// A block compressed texture ready to be written as resource fragments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surface {
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub levels: u32,
    /// Every mip level, largest first, each a whole number of blocks.
    pub pixels: Vec<u8>,
}

impl Surface {
    /// Encodes straight RGBA8 to BC7 (`FMT_BC7_SRGB` when `srgb`, else `FMT_BC7`) with a full box
    /// filtered mip chain when `mips`, else one level. Sizes that are not multiples of 4 are
    /// padded by repeating the edge texels, as the native build does. The bytes equal what the
    /// native core's garage build writes.
    pub fn from_rgba(rgba: &[u8], width: u32, height: u32, srgb: bool, mips: bool) -> Result<Self> {
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return Err(Error::Format(format!(
                "the size {width}x{height} is not supported"
            )));
        }
        if rgba.len() != width as usize * height as usize * 4 {
            return Err(Error::Format(format!(
                "{width}x{height} RGBA is {} bytes, not {}",
                width as usize * height as usize * 4,
                rgba.len()
            )));
        }
        let mut levels = 1;
        if mips {
            let mut m = width.max(height);
            while m > 1 {
                m /= 2;
                levels += 1;
            }
        }
        let (mut level, mut lw, mut lh) = (rgba.to_vec(), width as usize, height as usize);
        let mut pixels = Vec::new();
        for i in 0..levels {
            let (pw, ph) = (lw.next_multiple_of(4), lh.next_multiple_of(4));
            let mut padded = vec![0u8; pw * ph * 4];
            for y in 0..ph {
                for x in 0..pw {
                    let at = (y.min(lh - 1) * lw + x.min(lw - 1)) * 4;
                    padded[(y * pw + x) * 4..][..4].copy_from_slice(&level[at..at + 4]);
                }
            }
            pixels.extend(bc7_enc::encode_rgba(&padded, pw, ph)?);
            if i + 1 == levels {
                break;
            }
            let (nw, nh) = ((lw / 2).max(1), (lh / 2).max(1));
            let mut next = vec![0u8; nw * nh * 4];
            for y in 0..nh {
                for x in 0..nw {
                    for c in 0..4 {
                        let (mut sum, mut n) = (0u32, 0u32);
                        for dy in 0..2 {
                            for dx in 0..2 {
                                let sx = (x * 2 + dx).min(lw - 1);
                                let sy = (y * 2 + dy).min(lh - 1);
                                sum += level[(sy * lw + sx) * 4 + c] as u32;
                                n += 1;
                            }
                        }
                        next[(y * nw + x) * 4 + c] = ((sum + n / 2) / n) as u8;
                    }
                }
            }
            level = next;
            lw = nw;
            lh = nh;
        }
        Ok(Surface {
            format: if srgb { FMT_BC7_SRGB } else { FMT_BC7 },
            width,
            height,
            levels,
            pixels,
        })
    }

    /// A surface from pixels that are already compressed; the length must match the chain.
    pub fn from_blocks(format: u32, width: u32, height: u32, levels: u32, pixels: Vec<u8>) -> Result<Self> {
        let want = chain_bytes(format, width, height, levels);
        if want == 0 || pixels.len() != want {
            return Err(Error::Format(format!(
                "format {format} at {width}x{height} with {levels} level(s) is {want} bytes, not {}",
                pixels.len()
            )));
        }
        Ok(Surface {
            format,
            width,
            height,
            levels,
            pixels,
        })
    }

    /// The 40 byte header fragment: `2, 0x108, format, width, height, 1, levels, 1, 0, 0` as u32s.
    pub fn header(&self) -> [u8; 40] {
        let mut h = [0u8; 40];
        let v = [2, 0x108, self.format, self.width, self.height, 1, self.levels, 1];
        for (i, w) in v.iter().enumerate() {
            h[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        h
    }

    /// Reads back a header written by [`Surface::header`] (or a stock one): format, size, levels.
    pub fn parse_header(h: &[u8]) -> Result<(u32, u32, u32, u32)> {
        if h.len() < 32 {
            return Err(Error::Format("a surface header is 40 bytes".into()));
        }
        let w = |i: usize| u32::from_le_bytes(h[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        Ok((w(2), w(3), w(4), w(6).max(1)))
    }

    /// The 24 byte `GfxSRVResource` header followed by the surface's resource name and a NUL.
    pub fn srv_data(&self, surface_name: &str) -> Vec<u8> {
        let mut d = Vec::with_capacity(25 + surface_name.len());
        for v in [5, self.format, 0, self.levels, 0, 0] {
            d.extend_from_slice(&u32::to_le_bytes(v));
        }
        d.extend_from_slice(surface_name.as_bytes());
        d.push(0);
        d
    }
}

/// The 288 byte stream descriptor fragment a streamed surface carries: the path of its
/// `.tif.2025.mipmaps` side file below the game root (`eaid://` stripped), then zeros and a 1.0.
pub fn stream_descriptor(root: &str, stem: &str) -> Vec<u8> {
    const STOCK_LEN: usize = 288;
    let root = root.strip_prefix("eaid://").unwrap_or(root);
    let path = format!("{root}{stem}.tif.2025.mipmaps");
    let shown = &path.as_bytes()[..path.len().min(255)];
    let mut d = vec![shown.len() as u8];
    d.extend_from_slice(shown);
    d.push(0);
    d.extend_from_slice(&[0; 8]);
    d.extend_from_slice(&1.0f32.to_le_bytes());
    if d.len() < STOCK_LEN {
        d.resize(STOCK_LEN, 0);
    }
    d
}

/// The two resources of one texture: names and fragment data, ready for
/// `erp::Resource::like` with a stock surface and view as templates (or for an ERP writer that
/// takes [`SURFACE_ARG`] and [`SRV_ARG`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureResources {
    /// `<name>.tif.image`, type `GfxSurfaceRes`.
    pub surface_name: String,
    /// Header, pixels (and a stream descriptor when built with [`TextureResources::with_descriptor`]).
    pub surface_fragments: Vec<Vec<u8>>,
    /// `<name>.tif`, type `GfxSRVResource`.
    pub view_name: String,
    /// The one fragment of the view.
    pub view_fragment: Vec<u8>,
}

impl TextureResources {
    /// The pair for texture `name` (its resource path without `.tif`), without a stream descriptor.
    pub fn new(name: &str, surface: &Surface) -> Self {
        let surface_name = format!("{name}.tif.image");
        TextureResources {
            surface_fragments: vec![surface.header().to_vec(), surface.pixels.clone()],
            view_fragment: surface.srv_data(&surface_name),
            view_name: format!("{name}.tif"),
            surface_name,
        }
    }

    /// As [`TextureResources::new`] with the stock third fragment. `root` and `stem` split the
    /// name into the folder prefix and the file stem, e.g. `ui_package/art/` and `osd/dot`.
    pub fn with_descriptor(root: &str, stem: &str, surface: &Surface) -> Self {
        let mut t = Self::new(&format!("{root}{stem}"), surface);
        t.surface_fragments.push(stream_descriptor(root, stem));
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_matches_the_halo_and_native_layout() {
        let s = Surface::from_rgba(&[255; 8 * 8 * 4], 8, 8, true, false).unwrap();
        let mut want = Vec::new();
        for v in [2u32, 0x108, 70, 8, 8, 1, 1, 1, 0, 0] {
            want.extend_from_slice(&v.to_le_bytes());
        }
        assert_eq!(s.header().to_vec(), want);
        assert_eq!(Surface::parse_header(&want).unwrap(), (70, 8, 8, 1));
    }

    #[test]
    fn srv_is_the_header_and_the_surface_name() {
        let s = Surface::from_rgba(&[255; 4 * 4 * 4], 4, 4, false, false).unwrap();
        let d = s.srv_data("a/b.tif.image");
        let mut want = Vec::new();
        for v in [5u32, 69, 0, 1, 0, 0] {
            want.extend_from_slice(&v.to_le_bytes());
        }
        want.extend_from_slice(b"a/b.tif.image\0");
        assert_eq!(d, want);
    }

    #[test]
    fn mip_chain_runs_to_one_texel_and_pads_odd_sizes() {
        let s = Surface::from_rgba(&[9; 10 * 6 * 4], 10, 6, true, true).unwrap();
        assert_eq!(s.levels, 4);
        assert_eq!(s.pixels.len(), chain_bytes(FMT_BC7_SRGB, 10, 6, 4));
        assert_eq!(s.pixels.len(), (3 * 2 + 2 + 1 + 1) * 16);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Surface::from_rgba(&[0; 3], 1, 1, true, false).is_err());
        assert!(Surface::from_rgba(&[], 0, 0, true, false).is_err());
        assert!(Surface::from_blocks(FMT_BC7, 4, 4, 1, vec![0; 15]).is_err());
        assert!(Surface::from_blocks(FMT_BC7, 4, 4, 1, vec![0; 16]).is_ok());
    }

    #[test]
    fn descriptor_is_288_bytes_with_the_path() {
        let d = stream_descriptor("eaid://ui_package/art/", "osd/dot");
        assert_eq!(d.len(), 288);
        assert_eq!(d[0] as usize, "ui_package/art/osd/dot.tif.2025.mipmaps".len());
        assert_eq!(&d[1..8], b"ui_pack");
        let t = TextureResources::with_descriptor(
            "ui_package/art/",
            "osd/dot",
            &Surface::from_rgba(&[0; 64], 4, 4, true, false).unwrap(),
        );
        assert_eq!(t.surface_fragments.len(), 3);
        assert_eq!(t.view_name, "ui_package/art/osd/dot.tif");
    }
}
