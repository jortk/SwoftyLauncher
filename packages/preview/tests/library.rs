use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use swofty_preview::{ActiveInstance, Frame, Preview};
use swofty_shaders::{CompiledEffect, Effect, Origin, Target, compile, pack, parse_effect};

fn dxc() -> Option<PathBuf> {
    let path = std::env::var_os("SWOFTY_DXC")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/swofty/SwoftyLauncher/dxc/bin/dxc".into());
    path.exists().then_some(path)
}

fn library() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/swoftymod/shaders/effects")
}

fn effects(dxc: &Path, cache: &Path) -> Vec<CompiledEffect> {
    let header = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../sdk/include/swofty/postfx.hlsli"),
    )
    .unwrap();
    let mut out = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(library())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".effect.json"))
        .collect();
    files.sort();
    for file in files {
        let effect: Effect = parse_effect(&std::fs::read_to_string(&file).unwrap(), Origin::Library).unwrap();
        let blobs = compile(&effect, &library(), Target::Spirv, dxc, &header, cache)
            .unwrap_or_else(|e| panic!("{e}"));
        out.push(CompiledEffect {
            effect,
            blobs,
            images: Vec::new(),
        });
    }
    out
}

fn scene(index: usize, w: u32, h: u32) -> Frame {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let bright = if (fx - 0.7).abs() < 0.05 && (fy - 0.3).abs() < 0.05 {
                1.0
            } else {
                0.0
            };
            let r = (0.15 + 0.6 * fx + bright).min(1.0);
            let g = (0.2 + 0.5 * fy + bright).min(1.0);
            let b = (0.35 + 0.2 * (1.0 - fx) + bright).min(1.0);
            rgba.extend_from_slice(&[(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255]);
        }
    }
    Frame {
        index,
        width: w,
        height: h,
        rgba,
    }
}

fn instance(fx: &CompiledEffect) -> ActiveInstance {
    ActiveInstance {
        effect: fx.effect.id.clone(),
        label: String::new(),
        params: pack(&fx.effect, &BTreeMap::new()),
    }
}

fn differs(a: &Frame, b: &Frame) -> usize {
    a.rgba.iter().zip(&b.rgba).filter(|(x, y)| x != y).count()
}

#[test]
fn every_library_effect_renders() {
    let Some(dxc) = dxc() else {
        eprintln!("skipping: no dxc");
        return;
    };
    let cache = tempfile::tempdir().unwrap();
    let all = effects(&dxc, cache.path());
    assert!(all.len() >= 12, "{} effects", all.len());
    let passes: usize = all.iter().map(|e| e.blobs.len()).sum();
    eprintln!("{} effects, {passes} passes compiled to SPIR-V", all.len());
    let mut preview = match Preview::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    preview.set_effects(all.clone());
    let base = scene(0, 640, 360);
    for fx in &all {
        preview.set_instances(vec![instance(fx)]);
        let mut last = None;
        for i in 0..4 {
            let mut f = base.clone();
            f.index = i;
            last = Some(
                preview
                    .render(&f)
                    .unwrap_or_else(|e| panic!("{}: {e}", fx.effect.id)),
            );
        }
        let out = last.unwrap();
        let changed = differs(&out, &base);
        eprintln!(
            "{:16} changed {changed} of {} bytes",
            fx.effect.id,
            base.rgba.len()
        );
        assert!(
            out.rgba
                .chunks_exact(4)
                .any(|p| p[3] == 255 && (p[0] | p[1] | p[2]) != 0),
            "{} rendered black",
            fx.effect.id
        );
    }
}

#[test]
fn all_library_effects_together_stay_under_a_frame() {
    let Some(dxc) = dxc() else {
        eprintln!("skipping: no dxc");
        return;
    };
    let cache = tempfile::tempdir().unwrap();
    let all = effects(&dxc, cache.path());
    let Ok(mut preview) = Preview::new() else {
        return;
    };
    preview.set_effects(all.clone());
    preview.set_instances(all.iter().map(instance).collect());
    let mut frame = scene(0, 1280, 720);
    preview.render(&frame).unwrap();
    let start = Instant::now();
    let n = 30;
    for i in 1..=n {
        frame.index = i;
        preview.render(&frame).unwrap();
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / n as f64;
    eprintln!("1280x720, all library effects: {ms:.2} ms per frame");
    let adapter = preview.adapter_name().to_lowercase();
    if ["llvmpipe", "lavapipe", "swiftshader"]
        .iter()
        .any(|s| adapter.contains(s))
    {
        eprintln!("{adapter} renders on the CPU; the frame time is not checked");
        return;
    }
    assert!(ms < 33.0, "{ms} ms per frame");
}
