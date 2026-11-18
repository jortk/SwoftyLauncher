//! Read-only round trips of the HUD format writers on stock F1 25 files. Skipped without
//! `F1_25_DIR`.

use std::path::PathBuf;

use swofty_formats::animclip::{self, append_strings, build_clip, read_clip, read_strings};
use swofty_formats::emb;
use swofty_formats::erp::{Erp, Resource};
use swofty_formats::material::{Material, ParamValue};
use swofty_formats::pkg::Pkg;
use swofty_formats::surface::{FMT_BC7, FMT_BC7_SRGB, Surface};
use swofty_formats::uipre::{KEY_VISIBLE, Uipre, element_id};

const UI: &str = "2025_asset_groups/ui_package";

fn archive(rel: &str, kinds: &[&str]) -> Option<Vec<Resource>> {
    let Some(dir) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return None;
    };
    let path = dir.join(UI).join(rel);
    if !path.exists() {
        eprintln!("{} is missing; skipping", path.display());
        return None;
    }
    let kinds: Vec<String> = kinds.iter().map(|k| (*k).to_owned()).collect();
    let list = Erp::read_matching(&path, |_| true).ok()?;
    Some(list.into_iter().filter(|r| kinds.contains(&r.kind())).collect())
}

fn frag(r: &Resource, i: usize) -> Vec<u8> {
    r.fragments[i].data().unwrap()
}

#[test]
fn stock_anim_clips_rebuild_byte_for_byte_up_to_the_syst_padding() {
    let Some(clips) = archive("flow_race.erp", &["AnimClip"]) else {
        return;
    };
    assert!(!clips.is_empty());
    let (mut same, mut total) = (0, 0);
    for r in &clips {
        let (temp, syst) = (frag(r, 0), frag(r, 1));
        let Ok(clip) = read_clip(&temp, &syst) else {
            continue;
        };
        // the zh_* clips are Halo's, laid out with 16 byte aligned key times
        if r.name.contains(".idf/zh_") {
            continue;
        }
        total += 1;
        let again = build_clip(
            &temp,
            &clip.node,
            clip.name_index,
            &clip.string_table,
            &clip.channels,
        )
        .unwrap();
        let padded = {
            let mut s = syst.clone();
            s.resize(s.len().next_multiple_of(animclip::SYST_ALIGN), 0);
            s
        };
        if again == (temp, padded) {
            same += 1;
        } else {
            eprintln!("differs: {}", r.name);
        }
    }
    eprintln!("{same} of {total} stock clips rebuild identically");
    assert!(total > 50, "{total} readable clips");
    assert_eq!(same, total);
}

#[test]
fn stock_string_tables_append_and_keep_their_names() {
    let Some(tables) = archive("flow_race.erp", &["ACStringTable"]) else {
        return;
    };
    assert!(!tables.is_empty());
    for r in &tables {
        let data = frag(r, 0);
        assert_eq!(Pkg::parse(&data).unwrap().to_bytes().unwrap(), data, "{}", r.name);
        let have = read_strings(&data).unwrap();
        let (out, first) = append_strings(&data, &["~zz_one", "~zz_two"]).unwrap();
        assert_eq!(first, have.len());
        let grown = read_strings(&out).unwrap();
        assert_eq!(&grown[..have.len()], &have[..]);
        assert_eq!(&grown[have.len()..], ["~zz_one", "~zz_two"]);
    }
}

#[test]
fn stock_uipre_files_round_trip_and_take_setters() {
    let Some(files) = archive("flow_race.erp", &["UIPR"]) else {
        return;
    };
    assert!(!files.is_empty());
    for r in &files {
        let data = frag(r, 0);
        let mut p = Uipre::parse(&data).unwrap();
        assert_eq!(p.to_bytes().unwrap(), data, "{}", r.name);
        p.set_visible("zz_new_element", false).unwrap();
        p.set_text("zz_new_element", "88").unwrap();
        let back = Uipre::parse(&p.to_bytes().unwrap()).unwrap();
        let id = element_id("zz_new_element");
        assert!(back.setters(id).iter().any(|(k, _)| *k == KEY_VISIBLE));
        assert_eq!(
            back.element_ids().len(),
            Uipre::parse(&data).unwrap().element_ids().len() + 1
        );
    }
}

#[test]
fn stock_materials_round_trip_and_relink() {
    let Some(mats) = archive("flow_race.erp", &["GfxMaterialRes"]) else {
        return;
    };
    assert!(!mats.is_empty());
    let mut linked = 0;
    for r in &mats {
        let data = frag(r, 0);
        let mut m = Material::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", r.name));
        assert_eq!(m.to_bytes(), data, "{}", r.name);
        if m.link_texture("me_diffuseTex", "eaid://ui_package/art/osd/textures/zz/zz.tif")
            .is_ok()
        {
            linked += 1;
            let back = Material::parse(&m.to_bytes()).unwrap();
            assert_eq!(
                back.texture("me_diffuseTex"),
                Some("eaid://ui_package/art/osd/textures/zz/zz.tif")
            );
            let heap = back.param("me_diffuseTexHeapIndex").unwrap();
            assert_eq!(heap.link.as_deref(), back.texture("me_diffuseTex"));
            assert!(matches!(heap.value, ParamValue::Bytes(_)));
        }
    }
    eprintln!("{} materials, {linked} with me_diffuseTex", mats.len());
    assert!(linked > 0);
}

#[test]
fn stock_material_bindings_round_trip() {
    let Some(embs) = archive("flow_race.erp", &["GfxMatBindRes"]) else {
        return;
    };
    assert!(!embs.is_empty());
    for r in &embs {
        let data = frag(r, 0);
        assert_eq!(emb::write(&emb::parse(&data).unwrap()), data, "{}", r.name);
    }
}

#[test]
fn stock_bc7_headers_and_views_match_the_writer() {
    let Some(res) = archive("flow_race.erp", &["GfxSurfaceRes", "GfxSRVResource"]) else {
        return;
    };
    let views: std::collections::HashMap<String, Vec<u8>> = res
        .iter()
        .filter(|r| r.kind() == "GfxSRVResource")
        .map(|r| (r.name.clone(), frag(r, 0)))
        .collect();
    let (mut headers, mut srvs) = (0, 0);
    for r in res.iter().filter(|r| r.kind() == "GfxSurfaceRes") {
        let head = frag(r, 0);
        let Ok((format, w, h, levels)) = Surface::parse_header(&head) else {
            continue;
        };
        if format != FMT_BC7 && format != FMT_BC7_SRGB {
            continue;
        }
        let s = Surface {
            format,
            width: w,
            height: h,
            levels,
            pixels: Vec::new(),
        };
        if s.header().as_slice() == head.as_slice() {
            headers += 1;
        }
        if let Some(view) = r.name.strip_suffix(".image").and_then(|n| views.get(n))
            && *view == s.srv_data(&r.name)
        {
            srvs += 1;
        }
    }
    eprintln!("{headers} BC7 headers and {srvs} views equal the writer's");
    assert!(headers > 0 && srvs > 0);
}
