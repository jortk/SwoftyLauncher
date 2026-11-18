//! Post-process passes: HLSL compute shaders compiled with dxc against `swofty/postfx.hlsli`.

use std::path::{Path, PathBuf};

/// The pass contract, on the include path of every pass as `swofty/postfx.hlsli`.
pub const HEADER: &str = include_str!("../../sdk/include/swofty/postfx.hlsli");
/// The shader model passes compile to.
pub const PROFILE: &str = swofty_shaders::PROFILE;

/// The folder of a release that holds dxc, beside `bin/` and `loader/`.
pub const DXC_DIR: &str = "dxc";

fn exe_name() -> &'static str {
    if cfg!(windows) { "dxc.exe" } else { "dxc" }
}

/// The dxc the build compiles with: `SWOFTY_DXC`, then the release's `dxc/bin/`, then `PATH`.
pub fn dxc() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SWOFTY_DXC").filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    let bin = std::env::current_exe()
        .ok()
        .and_then(|e| Some(e.parent()?.to_path_buf()));
    if let Some(bundled) = bin
        .and_then(|b| Some(b.parent()?.join(DXC_DIR).join("bin").join(exe_name())))
        .filter(|p| p.is_file())
    {
        return Some(bundled);
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|d| d.join(exe_name()))
        .find(|p| p.is_file())
}

/// Whether a DXIL container carries the validator's hash, which D3D12 on Windows requires.
pub fn signed(dxil: &[u8]) -> bool {
    dxil.len() >= 20 && dxil.starts_with(b"DXBC") && dxil[4..20].iter().any(|&b| b != 0)
}

/// A compiled pass, with what dxc printed (empty when it came from the cache).
#[derive(Debug, Clone)]
pub struct Compiled {
    pub dxil: Vec<u8>,
    pub log: String,
}

/// Why a pass did not compile: one line for build output, and dxc's whole output.
#[derive(Debug, Clone)]
pub struct CompileError {
    pub summary: String,
    pub log: String,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.summary)
    }
}

impl From<String> for CompileError {
    fn from(summary: String) -> Self {
        Self {
            summary,
            log: String::new(),
        }
    }
}

/// Converts a PNG to the core's `.tex`: `SWTX`, width, height, format 0 (RGBA8), then the rows
/// top-down.
pub fn png_to_tex(png: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("the image is too large")?];
    let frame = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (frame.width, frame.height);
    if w == 0 || h == 0 || w > 8192 || h > 8192 {
        return Err(format!("{w}x{h} is not from 1x1 to 8192x8192"));
    }
    let px = &buf[..frame.buffer_size()];
    let mut out = Vec::with_capacity(16 + w as usize * h as usize * 4);
    out.extend_from_slice(b"SWTX");
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    match frame.color_type {
        png::ColorType::Rgba => out.extend_from_slice(px),
        png::ColorType::Rgb => px
            .chunks_exact(3)
            .for_each(|c| out.extend_from_slice(&[c[0], c[1], c[2], 255])),
        png::ColorType::Grayscale => px.iter().for_each(|&g| out.extend_from_slice(&[g, g, g, 255])),
        png::ColorType::GrayscaleAlpha => px
            .chunks_exact(2)
            .for_each(|c| out.extend_from_slice(&[c[0], c[0], c[0], c[1]])),
        png::ColorType::Indexed => return Err("an indexed image was not expanded".into()),
    }
    Ok(out)
}

/// How a pass's source is compiled: which function is the effect, or the compute entry point of
/// a raw pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Entry<'a> {
    pub name: Option<&'a str>,
    pub raw: bool,
}

impl<'a> Entry<'a> {
    pub fn of(pass: &'a crate::manifest::PostFx) -> Self {
        Self {
            name: pass.entry.as_deref(),
            raw: pass.raw,
        }
    }
}

/// Compiles `source` to signed DXIL, reusing `cache` when the source, the files it includes, the
/// contract and the entry are unchanged.
pub fn compile(source: &Path, cache: &Path, entry: Entry) -> Result<Compiled, CompileError> {
    let dxc = dxc().ok_or_else(|| "dxc was not found (set SWOFTY_DXC, or put dxc on PATH)".to_owned())?;
    std::fs::create_dir_all(cache).map_err(|e| format!("{}: {e}", cache.display()))?;
    let pass = swofty_shaders::PassEntry {
        name: entry.name,
        raw: entry.raw,
    };
    swofty_shaders::compile_pass(
        source,
        pass,
        swofty_shaders::Target::Dxil,
        &dxc,
        &[("swofty/postfx.hlsli", HEADER)],
        cache,
    )
    .map(|dxil| Compiled {
        dxil,
        log: String::new(),
    })
    .map_err(|e| CompileError {
        summary: e.summary,
        log: e.log,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_template_pass_compiles_when_dxc_is_at_hand() {
        if dxc().is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-postfx-{}", std::process::id()));
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../templates/postfx-pass/fx/vignette.hlsl");
        let dxil = compile(&source, &root, Entry::default()).unwrap().dxil;
        assert!(signed(&dxil), "the validator signs the container");
        assert_eq!(compile(&source, &root, Entry::default()).unwrap().dxil, dxil);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn entries_and_raw_passes_compile_to_their_own_dxil() {
        if dxc().is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-postfx-entry-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let cache = root.join("cache");
        let two = root.join("two.hlsl");
        std::fs::write(
            &two,
            "#include \"swofty/postfx.hlsli\"\n\
             float4 down(float2 uv, uint2 p) { return scene(uv) * 0.5; }\n\
             float4 up(float2 uv, uint2 p) { return input(0, uv) * 2.0; }\n",
        )
        .unwrap();
        let entry = |name| Entry {
            name: Some(name),
            raw: false,
        };
        let down = compile(&two, &cache, entry("down")).unwrap().dxil;
        let up = compile(&two, &cache, entry("up")).unwrap().dxil;
        assert!(signed(&down) && signed(&up));
        assert_ne!(down, up, "each entry is its own shader");
        assert_eq!(compile(&two, &cache, entry("down")).unwrap().dxil, down);
        let err = compile(&two, &cache, Entry::default()).unwrap_err();
        assert!(err.summary.starts_with("dxc: "), "no effect() in the file: {err}");

        let raw = root.join("raw.hlsl");
        std::fs::write(
            &raw,
            "#include \"swofty/postfx.hlsli\"\n\
             [numthreads(16, 16, 1)] void fill(uint3 id : SV_DispatchThreadID)\n\
             { if (all(id.xy < swofty_size)) swofty_output[id.xy] = float4(1, 0, 0, 1); }\n",
        )
        .unwrap();
        let compiled = compile(
            &raw,
            &cache,
            Entry {
                name: Some("fill"),
                raw: true,
            },
        )
        .unwrap();
        assert!(signed(&compiled.dxil));
        assert!(
            compile(
                &raw,
                &cache,
                Entry {
                    name: None,
                    raw: true
                }
            )
            .is_err(),
            "raw defaults to main"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_png_becomes_a_tex() {
        let mut png = Vec::new();
        {
            let mut e = png::Encoder::new(&mut png, 2, 1);
            e.set_color(png::ColorType::Rgb);
            e.set_depth(png::BitDepth::Eight);
            let mut w = e.write_header().unwrap();
            w.write_image_data(&[255, 0, 0, 0, 255, 0]).unwrap();
        }
        let tex = png_to_tex(&png).unwrap();
        assert_eq!(&tex[..4], b"SWTX");
        assert_eq!(tex[4..8], 2u32.to_le_bytes());
        assert_eq!(tex[8..12], 1u32.to_le_bytes());
        assert_eq!(tex[12..16], 0u32.to_le_bytes());
        assert_eq!(tex[16..], [255, 0, 0, 255, 0, 255, 0, 255]);
        assert!(png_to_tex(b"nope").is_err());
    }

    #[test]
    fn a_broken_pass_reports_the_dxc_error() {
        if dxc().is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!("swofty-postfx-bad-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("bad.hlsl");
        std::fs::write(
            &source,
            "#include \"swofty/postfx.hlsli\"\nfloat4 effect(float2 uv, uint2 p) { return nope; }\n",
        )
        .unwrap();
        let err = compile(&source, &root.join("cache"), Entry::default()).unwrap_err();
        assert!(
            err.summary.starts_with("dxc: ") && err.summary.contains("nope"),
            "{err}"
        );
        assert!(err.log.contains("bad.hlsl"), "{}", err.log);
        std::fs::remove_dir_all(&root).ok();
    }
}
