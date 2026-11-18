//! PNG sprites to the four resources each quad needs: a BC7 surface, its view, a material that
//! links the texture and a material binding for the quad mesh's slot.
//!
//! Built on `swofty_formats::{surface, emb, material}`: the surface header, view and binding layouts
//! follow `native/src/core/apply/content/garage_build.hpp` and Halo's `resources.py` (formats and
//! game facts).

use swofty_formats::surface::{Surface, TextureResources};
use swofty_formats::{emb, material};

use super::Stock;
use crate::{Error, Result};

/// The game's own textures a sprite may draw without a PNG (present in flow_race, flow_options and
/// persistent): the F1 logo of the lap counter and the FIA badge of the message system.
pub const F1_LOGO: &str = "eaid://ui_package/art/osd/textures/lap_counter/osd_customisation/lc_f1_logo.tif";
pub const FIA_LOGO: &str = "eaid://ui_package/art/osd/textures/message_system/ms_fia_logo.tif";
pub const STOCK_TEXTURES: [&str; 2] = [F1_LOGO, FIA_LOGO];

fn err(msg: impl std::fmt::Display) -> Error {
    Error::Other(format!("hud: {msg}"))
}

/// One decoded sprite.
#[derive(Debug, Clone)]
pub struct Art {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn decode_png(data: &[u8]) -> Result<Art> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(data));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info().map_err(|e| err(format!("png: {e}")))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or_else(|| err("png: too large"))?];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| err(format!("png: {e}")))?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
        png::ColorType::Indexed => return Err(err("png: indexed colour is not expanded")),
    };
    Ok(Art {
        width: info.width,
        height: info.height,
        rgba,
    })
}

impl Art {
    pub fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Self {
        Self {
            width: w,
            height: h,
            rgba: rgba.iter().copied().cycle().take((w * h * 4) as usize).collect(),
        }
    }

    pub fn tinted(&self, colour: [u8; 3]) -> Self {
        let mut out = self.clone();
        for p in out.rgba.chunks_exact_mut(4) {
            for c in 0..3 {
                p[c] = (p[c] as u32 * colour[c] as u32 / 255) as u8;
            }
        }
        out
    }

    pub fn check(&self, name: &str) -> Result<()> {
        if self.width == 0
            || self.height == 0
            || !self.width.is_multiple_of(4)
            || !self.height.is_multiple_of(4)
        {
            return Err(err(format!(
                "sprite {name}: {}x{} is not a multiple of 4 on both sides",
                self.width, self.height
            )));
        }
        Ok(())
    }
}

pub fn hex_colour(s: &str) -> Result<[u8; 3]> {
    let h = s.trim_start_matches('#');
    if h.len() < 6 {
        return Err(err(format!("colour {s}")));
    }
    let b = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).map_err(|_| err(format!("colour {s}")));
    Ok([b(0)?, b(2)?, b(4)?])
}

/// `#RRGGBB` or `#RRGGBBAA` (alpha 255 when left out).
pub fn hex_rgba(s: &str) -> Result<[u8; 4]> {
    let h = s.trim_start_matches('#');
    if !(h.len() == 6 || h.len() == 8) || !h.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(err(format!("colour {s}: #RRGGBB or #RRGGBBAA")));
    }
    let [r, g, b] = hex_colour(s)?;
    let a = if h.len() == 8 {
        u8::from_str_radix(&h[6..8], 16).map_err(|_| err(format!("colour {s}")))?
    } else {
        255
    };
    Ok([r, g, b, a])
}

/// The stock fill material with its diffuse texture linked to `tex`, `me_colour` set to `tint`
/// (white without one: the material multiplies the texture by it) and `me_finalColourTint` white.
pub fn material_with(stock: &[u8], tex: &str, tint: Option<[u8; 4]>) -> Result<Vec<u8>> {
    let mut m = material::Material::parse(stock).map_err(|e| err(format!("fill material: {e}")))?;
    let colour = tint.map_or([1.0; 4], |t| t.map(|c| c as f32 / 255.0));
    if m.param("me_colour").is_some() || tint.is_some() {
        m.set_floats("me_colour", &colour)
            .map_err(|e| err(format!("me_colour: {e}")))?;
    }
    if m.param("me_finalColourTint").is_some() {
        m.set_floats("me_finalColourTint", &[1.0; 4])
            .map_err(|e| err(format!("me_finalColourTint: {e}")))?;
    }
    m.link_texture("me_diffuseTex", tex)
        .map_err(|e| err(format!("fill material: {e}")))?;
    Ok(m.to_bytes())
}

/// The stock fill material with white colour constants and its diffuse texture linked to `tex`.
pub fn material(stock: &[u8], tex: &str) -> Result<Vec<u8>> {
    material_with(stock, tex, None)
}

/// The pixel size of a stock texture, from the header of its surface (`<tex>.image`) in `erp`.
pub fn texture_size(stock: &dyn Stock, erp: &str, tex: &str) -> Result<(u32, u32)> {
    let name = format!("{tex}.image");
    let frags = stock.read(erp, &name)?;
    let head = frags
        .first()
        .ok_or_else(|| err(format!("{name}: no header fragment")))?;
    let (_, w, h, _) = Surface::parse_header(head).map_err(|e| err(format!("{name}: {e}")))?;
    if w == 0 || h == 0 {
        return Err(err(format!("{name}: {w} x {h} px")));
    }
    Ok((w, h))
}

/// The bounds (HUD units, centred on the origin) of a texture of pixel size `size` scaled into the
/// box `[w, h]` keeping its aspect.
pub fn fit(size: (u32, u32), bx: [f64; 2]) -> [f64; 4] {
    let a = size.0 as f64 / size.1 as f64;
    let (w, h) = if bx[0] / bx[1] < a {
        (bx[0], bx[0] / a)
    } else {
        (bx[1] * a, bx[1])
    };
    [-w / 2.0, -h / 2.0, w / 2.0, h / 2.0]
}

/// Re-encodes a material the way it is stored; equal to the input for every stock material.
pub fn reencode_material(stock: &[u8]) -> Result<Vec<u8>> {
    Ok(material::Material::parse(stock)
        .map_err(|e| err(format!("material: {e}")))?
        .to_bytes())
}

/// The names of one sprite's resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Names {
    pub tex: String,
    pub material: String,
    pub emb: String,
    pub emb_edge: String,
}

pub fn names(prefix: &str, sprite: &str) -> Names {
    let key = format!("{prefix}{sprite}");
    Names {
        tex: format!("eaid://ui_package/art/osd/textures/hud/{key}.tif"),
        material: format!("eaid://ui_package/art/osd/materials/hud/{key}.material"),
        emb: format!("eaid://ui_package/art/osd/components/hud/{key}.emb?context=default"),
        emb_edge: format!("eaid://ui_package/art/osd/components/hud/edge/{key}.emb?context=default"),
    }
}

/// The names of a sprite drawn by another scene's quad mesh: the same texture and material, its
/// `.emb` under `components/hud/<mesh>/`. A sprite a dial and the standings cap both draw keeps one
/// binding per mesh slot this way.
pub fn mesh_names(prefix: &str, sprite: &str, mesh: &str) -> Names {
    let key = format!("{prefix}{sprite}");
    Names {
        emb: format!("eaid://ui_package/art/osd/components/hud/{mesh}/{key}.emb?context=default"),
        ..names(prefix, sprite)
    }
}

/// The names of a sprite that draws the stock texture `tex`: its own material and bindings, the
/// game's texture.
pub fn stock_names(prefix: &str, sprite: &str, tex: &str) -> Names {
    Names {
        tex: tex.to_owned(),
        ..names(prefix, sprite)
    }
}

/// One resource to add to a ready archive: name, type, fragments.
pub type Res = (String, &'static str, Vec<Vec<u8>>);

/// The material of a stock-texture sprite; no surface is written, the material links the game's.
pub fn stock_sprite_resources(n: &Names, fill_material: &[u8], tint: Option<[u8; 4]>) -> Result<Vec<Res>> {
    Ok(vec![(
        n.material.clone(),
        "GfxMaterialRes",
        vec![material_with(fill_material, &n.tex, tint)?],
    )])
}

/// The surface, view and material of one sprite.
pub fn sprite_resources(n: &Names, art: &Art, fill_material: &[u8]) -> Result<Vec<Res>> {
    let surface = Surface::from_rgba(&art.rgba, art.width, art.height, true, false)?;
    let tex = TextureResources::new(n.tex.trim_end_matches(".tif"), &surface);
    Ok(vec![
        (tex.surface_name, "GfxSurfaceRes", tex.surface_fragments),
        (tex.view_name, "GfxSRVResource", vec![tex.view_fragment]),
        (
            n.material.clone(),
            "GfxMaterialRes",
            vec![material(fill_material, &n.tex)?],
        ),
    ])
}

/// The binding of the quad mesh's `slot` to the sprite's material.
pub fn binding_resource(n: &Names, slot: &str, edge: bool) -> Res {
    let name = if edge { &n.emb_edge } else { &n.emb };
    (
        name.clone(),
        "GfxMatBindRes",
        vec![emb::write(&[(slot.to_owned(), n.material.clone())])],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surfaces_are_bc7_srgb_in_the_sprites_resources() {
        let art = Art::solid(8, 4, [255, 255, 255, 255]);
        let n = names("t_", "base");
        let tex = Surface::from_rgba(&art.rgba, 8, 4, true, false).unwrap();
        assert_eq!(tex.format, 70);
        assert_eq!(tex.pixels.len(), 8 * 4);
        assert!(Art::solid(6, 4, [0; 4]).check("x").is_err());
        assert_eq!(n.tex, "eaid://ui_package/art/osd/textures/hud/t_base.tif");
        assert_eq!(
            n.tex.trim_end_matches(".tif"),
            "eaid://ui_package/art/osd/textures/hud/t_base"
        );
    }

    #[test]
    fn bindings_pair_the_slot_with_the_material() {
        let n = names("t_", "base");
        let (name, kind, frags) = binding_resource(&n, "central_bg_fill", false);
        assert_eq!((kind, name.as_str()), ("GfxMatBindRes", n.emb.as_str()));
        let pairs = emb::parse(&frags[0]).unwrap();
        assert_eq!(pairs, [("central_bg_fill".to_owned(), n.material.clone())]);
        let (edge, _, _) = binding_resource(&n, "central_bg_edge", true);
        assert_eq!(edge, n.emb_edge);
    }

    #[test]
    fn png_decode_expands_to_rgba() {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, 4, 4);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()
                .unwrap()
                .write_image_data(&[200u8; 48])
                .unwrap();
        }
        let art = decode_png(&out).unwrap();
        assert_eq!((art.width, art.height, art.rgba.len()), (4, 4, 64));
        assert_eq!(&art.rgba[..4], &[200, 200, 200, 255]);
        assert_eq!(hex_colour("#27F4D2").unwrap(), [0x27, 0xf4, 0xd2]);
        assert!(material(&[0; 4], "x").is_err());
    }

    #[test]
    fn stock_texture_sprites_fit_their_box_and_keep_the_games_texture() {
        assert_eq!(hex_rgba("#00000000").unwrap(), [0, 0, 0, 0]);
        assert_eq!(hex_rgba("#27F4D2").unwrap(), [0x27, 0xf4, 0xd2, 255]);
        assert!(hex_rgba("#27F4D").is_err());
        assert!(hex_rgba("red").is_err());
        let wide = fit((400, 100), [34.2, 12.0]);
        assert!((wide[2] - 17.1).abs() < 1e-9 && (wide[3] - 17.1 / 4.0).abs() < 1e-9);
        let tall = fit((100, 400), [8.0, 8.0]);
        assert!((tall[2] - 1.0).abs() < 1e-9 && (tall[3] - 4.0).abs() < 1e-9);
        let n = stock_names("t_", "f1", F1_LOGO);
        assert_eq!(n.tex, F1_LOGO);
        assert_eq!(
            n.material,
            "eaid://ui_package/art/osd/materials/hud/t_f1.material"
        );
        assert!(stock_sprite_resources(&n, &[0; 4], None).is_err());
    }
}
