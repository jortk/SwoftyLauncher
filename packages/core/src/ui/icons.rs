//! The icons of ADDON SETTINGS' tiles: the launcher's mark for SwoftyMod, Team Simplified's for
//! Simplified for Swofty, the launcher's track badge for tracks (track addons and track packages), and
//! for every other addon or mod the box the launcher shows for an addon without its own icon. They are one texture array of 144 px slices drawn like the game's settings category
//! icons (white, fading from 202 alpha at the top to 102 at the bottom), added to persistent.erp so it
//! is loaded wherever the tiles show, and the overrides entry settings.swofty_addons that names a
//! slice per icon, a copy of settings.button. icons.bc7 holds the slices in SLICES order, BC7 with the
//! top mip only, drawn from the SVGs in icons/ by tools/addon-icons (its header has the command).

use swofty_formats::pkg::Obj;

use super::node::*;
use super::pages::Kind;
use crate::Result;

pub const LIST: &str = "swofty_addons";
pub const SLICES: [&str; 4] = ["swoftymod", "simplified", "addon", "track"];
pub const SIZE: u32 = 144;
pub const PIXELS: &[u8] = include_bytes!("icons.bc7");
/// The stock array whose shader resource view and surface header the icons' copy.
pub const STOCK: &str =
    "eaid://ui_package/art/texture_arrays/overrides.uidb/overrides.settings.button.texarray";
const FROM: &str = "button";
const ARRAYS: &str = "ui_package/art/texture_arrays/overrides.uidb";
const TEXTURES: &str = "ui_package/art/textures/swofty/addon_icons";
const SRV_ARRAY_SIZE: usize = 20;
const SRV_HEADER: usize = 24;
const SURFACE_W: usize = 12;
const SURFACE_H: usize = 16;
const SURFACE_LAYERS: usize = 28;

pub fn array() -> String {
    format!("eaid://{ARRAYS}/overrides.settings.{LIST}.texarray")
}

/// The texture a tile shows: for an addon, by its id, unless it is a track.
pub fn of(kind: Kind, id: &str, track: bool) -> &'static str {
    match (kind, id) {
        (Kind::Map, _) => "#overrides.settings.swofty_addons.track",
        (Kind::Addon, _) if track => "#overrides.settings.swofty_addons.track",
        (Kind::Addon, crate::builtin::CORE_ID) => "#overrides.settings.swofty_addons.swoftymod",
        (Kind::Addon, crate::managed::ID) => "#overrides.settings.swofty_addons.simplified",
        (Kind::Addon | Kind::Mod, _) => "#overrides.settings.swofty_addons.addon",
    }
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().expect("4 bytes")))
}

/// The array's shader resource view and surface (header, pixels), from the stock array's.
pub fn resources(srv: &[u8], header: &[u8]) -> Result<[(String, Vec<Vec<u8>>); 2]> {
    let stock = format!("{STOCK}.image");
    if srv.len() != SRV_HEADER + stock.len() + 1 || &srv[SRV_HEADER..srv.len() - 1] != stock.as_bytes() {
        return Err(err(format!("{STOCK}: unexpected shader resource view")));
    }
    if header.len() != 40 || u32_at(header, SURFACE_LAYERS).is_none() {
        return Err(err(format!("{stock}: unexpected surface header")));
    }
    if PIXELS.len() != SLICES.len() * (SIZE * SIZE) as usize {
        return Err(err(format!(
            "icons.bc7 is {} bytes, not {} slices of {SIZE} px",
            PIXELS.len(),
            SLICES.len()
        )));
    }
    let image = format!("{}.image", array());
    let mut view = srv[..SRV_HEADER].to_vec();
    view[SRV_ARRAY_SIZE..SRV_ARRAY_SIZE + 4].copy_from_slice(&(SLICES.len() as u32).to_le_bytes());
    view.extend_from_slice(image.as_bytes());
    view.push(0);
    let mut head = header.to_vec();
    for (at, v) in [
        (SURFACE_W, SIZE),
        (SURFACE_H, SIZE),
        (SURFACE_LAYERS, SLICES.len() as u32),
    ] {
        head[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    Ok([(array(), vec![view]), (image, vec![head, PIXELS.to_vec()])])
}

fn settings(p: &mut swofty_formats::pkg::Pkg) -> Result<&mut Obj> {
    root_obj(p)?
        .at_mut("overrides")?
        .o_mut()?
        .at_mut("Contents")?
        .o_mut()?
        .at_mut("settings")?
        .o_mut()?
        .at_mut("Contents")?
        .o_mut()
}

/// overrides.uidb with settings.swofty_addons, a copy of settings.button naming the icons' array
/// and a slice per icon.
pub fn overrides(data: &[u8]) -> Result<Vec<u8>> {
    edit(data, |p| {
        let d = settings(p)?;
        if d.get(LIST).is_some() {
            return Err(err(format!("the overrides already have settings.{LIST}")));
        }
        let mut e = d.at(FROM)?.clone();
        let eo = e.o_mut()?;
        set_str(eo, "Name", LIST)?;
        set_str(
            eo,
            "TextureArrayName",
            &format!("{ARRAYS}/overrides.settings.{LIST}.texarray"),
        )?;
        let contents = eo.at_mut("Contents")?.a_mut()?;
        let like = contents
            .first()
            .ok_or_else(|| err(format!("settings.{FROM} has no slices")))?
            .clone();
        contents.clear();
        for slice in SLICES {
            let mut c = like.clone();
            let co = c.o_mut()?;
            set_str(co, "Name", slice)?;
            set_str(co, "Value", &format!("{TEXTURES}/{slice}.tif"))?;
            contents.push(c);
        }
        d.set(LIST, e);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swoftymod_simplified_and_tracks_have_their_own_icons_and_the_rest_the_box() {
        let slice = |kind, id, track| of(kind, id, track).rsplit('.').next().unwrap();
        assert_eq!(slice(Kind::Addon, crate::builtin::CORE_ID, false), "swoftymod");
        assert_eq!(slice(Kind::Addon, crate::managed::ID, false), "simplified");
        assert_eq!(slice(Kind::Addon, "rk.ai-special-liveries", false), "addon");
        assert_eq!(slice(Kind::Addon, "", false), "addon");
        assert_eq!(slice(Kind::Addon, "track.mugello", true), "track");
        assert_eq!(slice(Kind::Map, "mugello", false), "track");
        assert_eq!(slice(Kind::Mod, "a.mod", false), "addon");
        for k in [Kind::Addon, Kind::Map, Kind::Mod] {
            for track in [false, true] {
                let icon = of(k, crate::builtin::CORE_ID, track);
                assert!(
                    icon.starts_with(&format!("#overrides.settings.{LIST}.")),
                    "{icon}"
                );
                assert!(
                    SLICES.contains(&slice(k, crate::builtin::CORE_ID, track)),
                    "{icon}"
                );
            }
        }
    }

    #[test]
    fn the_array_copies_the_stock_view_and_header_with_its_own_size() {
        let stock = format!("{STOCK}.image");
        let mut srv: Vec<u8> = [6u32, 70, 0, 1, 0, 5]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        srv.extend_from_slice(stock.as_bytes());
        srv.push(0);
        let header: Vec<u8> = [2u32, 264, 70, 32, 32, 1, 1, 5, 0, 8]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let [(view_name, view), (image_name, image)] = resources(&srv, &header).unwrap();
        assert_eq!(view_name, array());
        assert_eq!(image_name, format!("{}.image", array()));
        assert_eq!(u32_at(&view[0], SRV_ARRAY_SIZE), Some(SLICES.len() as u32));
        assert_eq!(&view[0][..SRV_ARRAY_SIZE], &srv[..SRV_ARRAY_SIZE]);
        assert!(view[0].ends_with(format!("{image_name}\0").as_bytes()));
        let head: Vec<u32> = (0..10).map(|i| u32_at(&image[0], i * 4).unwrap()).collect();
        assert_eq!(head, [2, 264, 70, SIZE, SIZE, 1, 1, SLICES.len() as u32, 0, 8]);
        assert_eq!(image[1].len(), SLICES.len() * (SIZE * SIZE) as usize);

        let mut other = srv.clone();
        other[SRV_HEADER] = b'x';
        assert!(
            resources(&other, &header).is_err(),
            "another array's view is refused"
        );
        assert!(
            resources(&srv, &header[..36]).is_err(),
            "a short header is refused"
        );
    }
}
