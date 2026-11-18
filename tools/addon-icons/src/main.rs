// addon-icons <out.bc7> <in.svg>...: the ADDON SETTINGS tile icons (packages/core/src/ui/icons.rs), one 144 px
// BC7 slice per SVG in order, drawn like the game's settings category icons: white, alpha fading from 202 at
// the glyph's top to 102 at its bottom, the glyph fitted to 110 px. Writes a preview of each over dark grey
// next to the output (<out>.<n>.png).
//
//   cargo run --release --manifest-path tools/addon-icons/Cargo.toml -- packages/core/src/ui/icons.bc7 \
//       packages/core/src/ui/icons/swoftymod.svg packages/core/src/ui/icons/simplified.svg \
//       packages/core/src/ui/icons/addon.svg
use resvg::{tiny_skia, usvg};

const SIZE: u32 = 144;
const GLYPH: f32 = 110.0;
const TOP_ALPHA: f32 = 202.0;
const BOTTOM_ALPHA: f32 = 102.0;
const WEIGHTS: [u32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

fn lerp(e0: u32, e1: u32, w: u32) -> u32 {
    ((64 - w) * e0 + w * e1 + 32) >> 6
}

// BC7 mode 6: endpoint 0 is (254,254,254,a0) with p-bit 0, endpoint 1 (255,255,255,a1) with
// p-bit 1, so only alpha varies.
fn block(alpha: &[u8; 16]) -> [u8; 16] {
    let lo = *alpha.iter().min().unwrap() as u32 & !1;
    let hi = (*alpha.iter().max().unwrap() as u32 | 1).min(255);
    let mut ends = [(127u32, lo >> 1, 0u32), (127, hi >> 1, 1)];
    let value = |e: (u32, u32, u32)| (e.1 << 1) | e.2;
    let mut idx = [0u32; 16];
    let pick = |ends: &[(u32, u32, u32); 2], a: u32| {
        (0..16)
            .min_by_key(|&i| (lerp(value(ends[0]), value(ends[1]), WEIGHTS[i]) as i32 - a as i32).abs())
            .unwrap() as u32
    };
    for (i, a) in alpha.iter().enumerate() {
        idx[i] = pick(&ends, *a as u32);
    }
    if idx[0] >= 8 {
        ends.swap(0, 1);
        for i in idx.iter_mut() {
            *i = 15 - *i;
        }
    }
    let mut bits: u128 = 1 << 6;
    let mut at = 7;
    let mut put = |v: u32, n: u32| {
        bits |= (v as u128) << at;
        at += n;
    };
    for _ in 0..3 {
        put(ends[0].0, 7);
        put(ends[1].0, 7);
    }
    put(ends[0].1, 7);
    put(ends[1].1, 7);
    put(ends[0].2, 1);
    put(ends[1].2, 1);
    put(idx[0], 3);
    for i in &idx[1..] {
        put(*i, 4);
    }
    bits.to_le_bytes()
}

fn slice(svg: &str, preview: &std::path::Path) -> Vec<u8> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).unwrap();
    let b = tree.root().abs_stroke_bounding_box();
    let s = GLYPH / b.width().max(b.height());
    let (dx, dy) = (
        (SIZE as f32 - b.width() * s) / 2.0 - b.x() * s,
        (SIZE as f32 - b.height() * s) / 2.0 - b.y() * s,
    );
    let mut pix = tiny_skia::Pixmap::new(SIZE, SIZE).unwrap();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_row(s, 0., 0., s, dx, dy),
        &mut pix.as_mut(),
    );
    let (top, bottom) = (b.y() * s + dy, (b.y() + b.height()) * s + dy);
    let n = SIZE as usize;
    let mut alpha = vec![0u8; n * n];
    for y in 0..n {
        let t = ((y as f32 + 0.5 - top) / (bottom - top)).clamp(0.0, 1.0);
        let ramp = BOTTOM_ALPHA + (TOP_ALPHA - BOTTOM_ALPHA) * (1.0 - t).powf(1.6);
        for x in 0..n {
            let cover = pix.pixels()[y * n + x].alpha() as f32 / 255.0;
            alpha[y * n + x] = (cover * ramp).round() as u8;
        }
    }
    let mut blocks = Vec::new();
    for by in 0..n / 4 {
        for bx in 0..n / 4 {
            let mut blk = [0u8; 16];
            for i in 0..16 {
                blk[i] = alpha[(by * 4 + i / 4) * n + bx * 4 + i % 4];
            }
            blocks.extend_from_slice(&block(&blk));
        }
    }
    let mut prev = tiny_skia::Pixmap::new(SIZE, SIZE).unwrap();
    for by in 0..n / 4 {
        for bx in 0..n / 4 {
            let k = u128::from_le_bytes(blocks[(by * n / 4 + bx) * 16..][..16].try_into().unwrap());
            let f = |at: u32, len: u32| ((k >> at) & ((1u128 << len) - 1)) as u32;
            let (a0, a1) = ((f(49, 7) << 1) | f(63, 1), (f(56, 7) << 1) | f(64, 1));
            for i in 0..16u32 {
                let ix = if i == 0 { f(65, 3) } else { f(68 + (i - 1) * 4, 4) };
                let av = lerp(a0, a1, WEIGHTS[ix as usize]);
                let v = ((255 * av + 30 * (255 - av)) / 255) as u8;
                let p = tiny_skia::ColorU8::from_rgba(v, v, v, 255).premultiply();
                prev.pixels_mut()[(by * 4 + i as usize / 4) * n + bx * 4 + i as usize % 4] = p;
            }
        }
    }
    prev.save_png(preview).unwrap();
    println!(
        "glyph {:.0}x{:.0}, {} bytes",
        b.width() * s,
        b.height() * s,
        blocks.len()
    );
    blocks
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: addon-icons <out.bc7> <in.svg>...");
        std::process::exit(2);
    }
    let out = std::path::Path::new(&a[1]);
    let mut all = Vec::new();
    for (i, p) in a[2..].iter().enumerate() {
        let svg = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{p}: {e}"));
        all.extend(slice(&svg, &out.with_extension(format!("{i}.png"))));
    }
    std::fs::write(out, &all).unwrap_or_else(|e| panic!("{}: {e}", out.display()));
}
