use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use sha2::Digest;

use swofty_preview::{ActiveInstance, Clip, ClipInfo, CompiledEffect, Frame, Preview};
use swofty_shaders::{Effect, Origin, PassDecl, TextureDecl};

fn dxc() -> Option<PathBuf> {
    let path = std::env::var_os("SWOFTY_DXC")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/home/swofty/SwoftyLauncher/dxc/bin/dxc".into());
    path.exists().then_some(path)
}

fn spirv(dxc: &Path, dir: &Path, name: &str, source: &str) -> Vec<u8> {
    let include = Path::new(env!("CARGO_MANIFEST_DIR")).join("../sdk/include");
    let file = dir.join(format!("{name}.hlsl"));
    let out = dir.join(format!("{name}.spv"));
    std::fs::write(&file, source).unwrap();
    let mut command = Command::new(dxc);
    if let Some(lib) = dxc.parent().and_then(|p| p.parent()).map(|p| p.join("lib")) {
        command.env("LD_LIBRARY_PATH", lib);
    }
    let result = command
        .args([
            "-T",
            "cs_6_0",
            "-E",
            "main",
            "-spirv",
            "-fspv-target-env=vulkan1.1",
            "-fvk-use-dx-layout",
        ])
        .args([
            "-fvk-b-shift",
            "0",
            "0",
            "-fvk-t-shift",
            "10",
            "0",
            "-fvk-u-shift",
            "30",
            "0",
            "-fvk-s-shift",
            "40",
            "0",
        ])
        .arg("-I")
        .arg(&include)
        .arg(&file)
        .arg("-Fo")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "dxc failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::read(out).unwrap()
}

fn pass(name: &str, inputs: &[&str], output: Option<&str>) -> PassDecl {
    PassDecl {
        name: name.into(),
        shader: format!("{name}.hlsl"),
        entry: None,
        raw: false,
        threads: None,
        inputs: inputs.iter().map(|s| s.to_string()).collect(),
        output: output.map(str::to_string),
        when: None,
        budget_ms: None,
    }
}

fn texture(name: &str, persistent: bool) -> TextureDecl {
    TextureDecl {
        name: name.into(),
        scale: None,
        width: None,
        height: None,
        format: Some("rgba16f".into()),
        persistent,
        file: None,
        mips: None,
    }
}

fn effect(id: &str, textures: Vec<TextureDecl>, passes: Vec<PassDecl>) -> Effect {
    Effect {
        id: id.into(),
        name: id.into(),
        description: String::new(),
        params: Vec::new(),
        textures,
        passes,
        slots: 1,
        origin: Origin::Library,
        warnings: Vec::new(),
    }
}

const COUNT: &str = r#"#include "swofty/postfx.hlsli"
float4 effect(float2 uv, uint2 pixel) { return input(0, uv) + float4(0.1, 0, 0, 0); }"#;
const PREP: &str = r#"#include "swofty/postfx.hlsli"
float4 effect(float2 uv, uint2 pixel) { return float4(1.0 - scene(uv).rgb, 1); }"#;
const APPLY: &str = r#"#include "swofty/postfx.hlsli"
float4 effect(float2 uv, uint2 pixel)
{
    float3 c = input(0, uv).rgb * 0.5;
    if (has_previous())
        c += input(1, uv).rgb * 0.5;
    c.r += input(2, uv).r;
    c += option(0);
    return float4(c, 1);
}"#;

fn compiled(dxc: &Path, dir: &Path) -> CompiledEffect {
    let fx = effect(
        "temporal",
        vec![texture("tmp", false), texture("acc", true)],
        vec![
            pass("count", &["acc"], Some("acc")),
            pass("prep", &[], Some("tmp")),
            pass("apply", &["tmp", "previous", "acc"], None),
        ],
    );
    let spirv = vec![
        spirv(dxc, dir, "count", COUNT),
        spirv(dxc, dir, "prep", PREP),
        spirv(dxc, dir, "apply", APPLY),
    ];
    CompiledEffect {
        effect: fx,
        blobs: spirv,
        images: Vec::new(),
    }
}

fn solid(index: usize, w: u32, h: u32, rgb: [u8; 3]) -> Frame {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for _ in 0..w * h {
        rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
    }
    Frame {
        index,
        width: w,
        height: h,
        rgba,
    }
}

fn near(frame: &Frame, expect: [f32; 3]) {
    for (x, y) in [
        (0, 0),
        (frame.width / 2, frame.height / 2),
        (frame.width - 1, frame.height - 1),
    ] {
        let at = ((y * frame.width + x) * 4) as usize;
        for (c, e) in expect.iter().enumerate() {
            let got = f32::from(frame.rgba[at + c]);
            let want = (e.clamp(0.0, 1.0) * 255.0).round();
            assert!(
                (got - want).abs() <= 2.0,
                "pixel ({x},{y}) channel {c}: got {got}, want {want}"
            );
        }
    }
}

fn preview(effects: Vec<CompiledEffect>, params: f32) -> Option<Preview> {
    let mut preview = match Preview::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping: {e}");
            return None;
        }
    };
    eprintln!("adapter: {}", preview.adapter_name());
    preview.set_effects(effects);
    preview.set_instances(vec![ActiveInstance {
        effect: "temporal".into(),
        label: String::new(),
        params: vec![params],
    }]);
    Some(preview)
}

#[test]
fn passes_previous_and_persistent_textures() {
    let Some(dxc) = dxc() else {
        eprintln!("skipping: no dxc");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let Some(mut preview) = preview(vec![compiled(&dxc, dir.path())], 0.0) else {
        return;
    };
    let a = [51, 102, 153];
    let b = [204, 0, 102];
    let f0 = preview.render(&solid(0, 64, 48, a)).unwrap();
    let out0 = [0.5 * 0.8 + 0.1, 0.5 * 0.6, 0.5 * 0.4];
    near(&f0, out0);

    let f1 = preview.render(&solid(1, 64, 48, b)).unwrap();
    let out1 = [
        0.5 * 0.2 + 0.5 * out0[0] + 0.2,
        0.5 * 1.0 + 0.5 * out0[1],
        0.5 * 0.6 + 0.5 * out0[2],
    ];
    near(&f1, out1);

    let jump = preview.render(&solid(20, 64, 48, b)).unwrap();
    near(&jump, [0.5 * 0.2 + 0.1, 0.5 * 1.0, 0.5 * 0.6]);

    let again = preview.render(&solid(21, 64, 48, b)).unwrap();
    near(
        &again,
        [
            0.5 * 0.2 + 0.5 * (0.5 * 0.2 + 0.1) + 0.2,
            0.5 * 1.0 + 0.5 * 0.5,
            0.5 * 0.6 + 0.5 * 0.3,
        ],
    );
}

#[test]
fn param_slots_reach_the_shader() {
    let Some(dxc) = dxc() else {
        eprintln!("skipping: no dxc");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let Some(mut preview) = preview(vec![compiled(&dxc, dir.path())], 0.0) else {
        return;
    };
    let a = [51, 102, 153];
    let plain = preview.render(&solid(0, 32, 32, a)).unwrap();
    preview.set_instances(vec![ActiveInstance {
        effect: "temporal".into(),
        label: String::new(),
        params: vec![0.05],
    }]);
    let biased = preview.render(&solid(0, 32, 32, a)).unwrap();
    near(&biased, [0.5 * 0.8 + 0.1 + 0.05, 0.3 + 0.05, 0.2 + 0.05]);
    assert_ne!(plain.rgba, biased.rgba);
    preview.set_instances(Vec::new());
    let none = preview.render(&solid(0, 32, 32, a)).unwrap();
    assert_eq!(none.rgba, solid(0, 32, 32, a).rgba);
}

#[test]
fn clip_opens_and_decodes() {
    let dir = tempfile::tempdir().unwrap();
    let info = ClipInfo {
        title: "test".into(),
        fps: 30.0,
        width: 16,
        height: 8,
        frames: 3,
        view: "cockpit".into(),
    };
    std::fs::write(
        dir.path().join("clip.json"),
        serde_json::to_string(&info).unwrap(),
    )
    .unwrap();
    for i in 0..3u8 {
        let image = image::RgbaImage::from_pixel(16, 8, image::Rgba([i * 80, 10, 20, 255]));
        image.save(dir.path().join(format!("{i:04}.webp"))).unwrap();
    }
    let clip = Clip::open(dir.path()).unwrap();
    assert_eq!(clip.len(), 3);
    let f = clip.frame(2).unwrap();
    assert_eq!((f.width, f.height, f.rgba[0], f.rgba[1]), (16, 8, 160, 10));
    assert!(clip.frame(3).is_err());
}

#[test]
fn store_installs_a_checked_zip_and_lists_it() {
    use std::io::Write;
    let mut bytes = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut bytes));
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("clip.json", options).unwrap();
        zip.write_all(br#"{"title":"Lap","fps":30,"width":4,"height":4,"frames":1,"view":"cockpit"}"#)
            .unwrap();
        zip.start_file("0000.webp", options).unwrap();
        let mut webp = Vec::new();
        image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]))
            .write_to(&mut std::io::Cursor::new(&mut webp), image::ImageFormat::WebP)
            .unwrap();
        zip.write_all(&webp).unwrap();
        zip.finish().unwrap();
    }
    let sha: String = sha2::Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let cache = tempfile::tempdir().unwrap();
    let store = swofty_preview::ClipStore::new(cache.path());
    assert!(store.cached().is_empty());
    let mut entry = swofty_preview::ClipEntry {
        id: "lap".into(),
        title: "Lap".into(),
        url: String::new(),
        sha256: "0".repeat(64),
        size: bytes.len() as u64,
    };
    assert!(store.install(&entry, &bytes).is_err());
    entry.sha256 = sha;
    let dir = store.install(&entry, &bytes).unwrap();
    let listed = store.cached();
    assert_eq!(listed.len(), 1);
    assert_eq!((listed[0].id.as_str(), listed[0].title.as_str()), ("lap", "Lap"));
    let clip = Clip::open(&dir).unwrap();
    assert_eq!(clip.frame(0).unwrap().rgba[..3], [1, 2, 3]);
}

const BLUR: &str = r#"#include "swofty/postfx.hlsli"
float4 effect(float2 uv, uint2 pixel)
{
    float4 sum = 0;
    for (int y = -1; y <= 1; y++)
        for (int x = -1; x <= 1; x++)
            sum += scene_at(int2(pixel) + int2(x, y));
    return sum / 9.0;
}"#;

#[test]
fn ten_passes_at_720p_stay_under_a_frame() {
    let Some(dxc) = dxc() else {
        eprintln!("skipping: no dxc");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let blob = spirv(&dxc, dir.path(), "blur", BLUR);
    let passes = (0..10).map(|i| pass(&format!("blur{i}"), &[], None)).collect();
    let fx = effect("temporal", Vec::new(), passes);
    let compiled = CompiledEffect {
        effect: fx,
        blobs: vec![blob; 10],
        images: Vec::new(),
    };
    let Some(mut preview) = preview(vec![compiled], 0.0) else {
        return;
    };
    let mut frame = solid(0, 1280, 720, [90, 120, 200]);
    preview.render(&frame).unwrap();
    let start = Instant::now();
    let n = 30;
    for i in 1..=n {
        frame.index = i;
        let out = preview.render(&frame).unwrap();
        assert_eq!(out.rgba.len(), frame.rgba.len());
        if i == 1 {
            assert_eq!(&out.rgba[..4], &[90, 120, 200, 255]);
        }
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / n as f64;
    eprintln!("1280x720, 10 passes: {ms:.2} ms per frame (render + readback)");
    assert!(ms < 33.0, "{ms} ms per frame");
}

#[test]
fn no_instances_returns_the_frame() {
    let Ok(mut preview) = Preview::new() else {
        return;
    };
    let f = solid(0, 33, 17, [51, 102, 153]);
    let out = preview.render(&f).unwrap();
    assert_eq!(out.rgba, f.rgba);
}

const RAW: &str = r#"#define SWOFTY_RAW
#include "swofty/postfx.hlsli"
[numthreads(8, 8, 1)] void main(uint3 id : SV_DispatchThreadID)
{
    swofty_output[id.xy] = float4(1, 0.5, 0.25, 1);
}"#;

#[test]
fn raw_passes_write_the_picture() {
    let Some(dxc) = dxc() else { return };
    let dir = tempfile::tempdir().unwrap();
    let blob = spirv(&dxc, dir.path(), "raw", RAW);
    let mut p = pass("raw", &[], None);
    p.raw = true;
    let compiled = CompiledEffect {
        effect: effect("temporal", Vec::new(), vec![p]),
        blobs: vec![blob],
        images: Vec::new(),
    };
    let Some(mut preview) = preview(vec![compiled], 0.0) else {
        return;
    };
    let out = preview.render(&solid(0, 32, 32, [1, 2, 3])).unwrap();
    assert_eq!(&out.rgba[..4], &[255, 127, 64, 255]);
}

const SHOW: &str = r#"#include "swofty/postfx.hlsli"
float4 effect(float2 uv, uint2 pixel) { return input_at(0, int2(uv * float2(input_size(0)))); }"#;

#[test]
fn image_textures_reach_the_shader_the_right_way_up() {
    let Some(dxc) = dxc() else {
        eprintln!("skipping: no dxc");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut pattern = texture("pattern", false);
    pattern.format = None;
    pattern.file = Some("pattern.png".into());
    let fx = effect("image", vec![pattern], vec![pass("show", &["pattern"], None)]);
    let rgba = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 255, 255],
    ]
    .concat();
    let compiled = CompiledEffect {
        effect: fx,
        blobs: vec![spirv(&dxc, dir.path(), "show", SHOW)],
        images: vec![("pattern".into(), 2, 2, rgba)],
    };
    let mut preview = match Preview::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    preview.set_effects(vec![compiled]);
    preview.set_instances(vec![ActiveInstance {
        effect: "image".into(),
        label: String::new(),
        params: Vec::new(),
    }]);
    let out = preview.render(&solid(0, 64, 48, [128, 128, 128])).unwrap();
    let at = |x: u32, y: u32| {
        let i = ((y * out.width + x) * 4) as usize;
        [out.rgba[i], out.rgba[i + 1], out.rgba[i + 2]]
    };
    assert_eq!(at(1, 1), [255, 0, 0], "top left");
    assert_eq!(at(62, 1), [0, 255, 0], "top right");
    assert_eq!(at(1, 46), [0, 0, 255], "bottom left");
    assert_eq!(at(62, 46), [255, 255, 255], "bottom right");
}

const FILL_R: &str = r#"#include "swofty/postfx.hlsli"
float4 effect(float2 uv, uint2 pixel) { return float4(0.25, 0.5, 0.75, 0.5); }"#;
const FILL_8: &str = r#"#include "swofty/postfx.hlsli"
float4 effect(float2 uv, uint2 pixel) { return float4(1.5, -0.5, 0.5, 1); }"#;
const SHOW_BOTH: &str = r#"#include "swofty/postfx.hlsli"
float4 effect(float2 uv, uint2 pixel)
{
    float4 r = input(0, uv), b = input(1, uv);
    return float4(r.r, r.g + r.b, b.r * 0.5, 1);
}"#;

#[test]
fn declared_formats_hold_what_the_games_textures_hold() {
    let Some(dxc) = dxc() else {
        eprintln!("skipping: no dxc");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut single = texture("single", false);
    single.format = Some("r16f".into());
    let mut bytes = texture("bytes", false);
    bytes.format = Some("rgba8".into());
    let fx = effect(
        "formats",
        vec![single, bytes],
        vec![
            pass("fill_r", &[], Some("single")),
            pass("fill_8", &[], Some("bytes")),
            pass("show_both", &["single", "bytes"], None),
        ],
    );
    let compiled = CompiledEffect {
        effect: fx,
        blobs: vec![
            spirv(&dxc, dir.path(), "fill_r", FILL_R),
            spirv(&dxc, dir.path(), "fill_8", FILL_8),
            spirv(&dxc, dir.path(), "show_both", SHOW_BOTH),
        ],
        images: Vec::new(),
    };
    let mut preview = match Preview::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    preview.set_effects(vec![compiled]);
    preview.set_instances(vec![ActiveInstance {
        effect: "formats".into(),
        label: String::new(),
        params: Vec::new(),
    }]);
    let out = preview.render(&solid(0, 32, 32, [128, 128, 128])).unwrap();
    near(&out, [0.25, 0.0, 0.5]);
}

const SLOTS: &str = r#"#include "swofty/postfx.hlsli"
float4 effect(float2 uv, uint2 pixel) { return float4(option(16), option(19), option(31), 1); }"#;

#[test]
fn all_32_option_slots_reach_the_shader() {
    let Some(dxc) = dxc() else {
        eprintln!("skipping: no dxc");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let compiled = CompiledEffect {
        effect: effect("slots", Vec::new(), vec![pass("slots", &[], None)]),
        blobs: vec![spirv(&dxc, dir.path(), "slots", SLOTS)],
        images: Vec::new(),
    };
    let mut preview = match Preview::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    preview.set_effects(vec![compiled]);
    preview.set_instances(vec![ActiveInstance {
        effect: "slots".into(),
        label: String::new(),
        params: (0..32).map(|i| i as f32 / 40.0).collect(),
    }]);
    let out = preview.render(&solid(0, 16, 16, [0, 0, 0])).unwrap();
    near(&out, [16.0 / 40.0, 19.0 / 40.0, 31.0 / 40.0]);
}

const FILL_PATTERN: &str = r#"#include "swofty/postfx.hlsli"
float4 effect(float2 uv, uint2 pixel) { return float4(float(pixel.x + 4 * pixel.y) / 16.0, 0.5, 0, 1); }"#;
const FILL_ODD: &str = r#"#include "swofty/postfx.hlsli"
float4 effect(float2 uv, uint2 pixel) { return float4(0.5, 0.5, 0.5, 1); }"#;
const SHOW_LODS: &str = r#"#include "swofty/postfx.hlsli"
float4 effect(float2 uv, uint2 pixel)
{
    float v = uv.x < 0.5
        ? input_lod(0, uv, 2).r
        : input_lod_point(0, float2(uv.x * 2 - 1, uv.y), 1).r;
    float level0 = input(0, float2(0.125, 0.125)).r;
    float g = float(input_size_lod(0, 1).x) / 4 + float(input_size_lod(0, 2).x) / 16;
    float b = float(input_size_lod(1, 1).x) / 4 + input_lod(1, uv, 2).r * 0.25;
    return float4(v, g, b, 1) + float4(0, 0, 0, level0 * 0);
}"#;

#[test]
fn mip_chains_hold_the_box_averages() {
    let Some(dxc) = dxc() else {
        eprintln!("skipping: no dxc");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let mut lens = texture("lens", false);
    lens.width = Some(4);
    lens.height = Some(4);
    lens.mips = Some(3);
    let mut odd = texture("odd", false);
    odd.width = Some(5);
    odd.height = Some(3);
    odd.mips = Some(9);
    let fx = effect(
        "mips",
        vec![lens, odd],
        vec![
            pass("fill_pattern", &[], Some("lens")),
            pass("fill_odd", &[], Some("odd")),
            pass("show_lods", &["lens", "odd"], None),
        ],
    );
    let compiled = CompiledEffect {
        effect: fx,
        blobs: vec![
            spirv(&dxc, dir.path(), "fill_pattern", FILL_PATTERN),
            spirv(&dxc, dir.path(), "fill_odd", FILL_ODD),
            spirv(&dxc, dir.path(), "show_lods", SHOW_LODS),
        ],
        images: Vec::new(),
    };
    let mut preview = match Preview::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("skipping: {e}");
            return;
        }
    };
    preview.set_effects(vec![compiled]);
    preview.set_instances(vec![ActiveInstance {
        effect: "mips".into(),
        label: String::new(),
        params: Vec::new(),
    }]);
    let out = preview.render(&solid(0, 64, 48, [128, 128, 128])).unwrap();
    let at = |x: u32, y: u32| {
        let i = ((y * out.width + x) * 4) as usize;
        [out.rgba[i], out.rgba[i + 1], out.rgba[i + 2]].map(|v| f32::from(v) / 255.0)
    };
    let close = |got: f32, want: f32, what: &str| {
        assert!((got - want).abs() < 2.0 / 255.0, "{what}: got {got}, want {want}");
    };
    close(at(10, 10)[0], 7.5 / 16.0, "level 2 is the mean of all 16");
    close(at(40, 12)[0], 2.5 / 16.0, "level 1 texel 0,0");
    close(at(56, 12)[0], 4.5 / 16.0, "level 1 texel 1,0");
    close(at(40, 36)[0], 10.5 / 16.0, "level 1 texel 0,1");
    close(at(56, 36)[0], 12.5 / 16.0, "level 1 texel 1,1");
    let [_, g, b] = at(10, 10);
    close(g, 0.5 + 1.0 / 16.0, "level sizes of the 4x4");
    close(b, 0.5 + 0.5 / 4.0, "the 5x3 rounds down to 2 wide and holds 0.5");
}
