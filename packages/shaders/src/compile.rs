//! Compiling effect passes with dxc to DXIL (the game) or SPIR-V (the preview).

use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::{Effect, PROFILE, Target, params_hlsl};

/// The flags that place the pass contract's registers at fixed Vulkan bindings (DESIGN.md §7).
const SPIRV_FLAGS: [&str; 13] = [
    "-spirv",
    "-fspv-target-env=vulkan1.1",
    "-fvk-use-dx-layout",
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
];
const SPIRV_S_SHIFT: [&str; 2] = ["40", "0"];

/// An effect with its compiled passes. `images` holds `(texture name, width, height, RGBA8 rows)`
/// for the effect's file textures; the caller fills it.
#[derive(Debug, Clone)]
pub struct CompiledEffect {
    pub effect: Effect,
    pub blobs: Vec<Vec<u8>>,
    pub images: Vec<(String, u32, u32, Vec<u8>)>,
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

/// How a pass's source is compiled: which function is the effect, or the compute entry point of a
/// raw pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PassEntry<'a> {
    pub name: Option<&'a str>,
    pub raw: bool,
}

impl PassEntry<'_> {
    fn args(&self) -> Vec<String> {
        match (self.raw, self.name) {
            (true, name) => vec![
                "-D".into(),
                "SWOFTY_RAW=1".into(),
                "-E".into(),
                name.unwrap_or("main").into(),
            ],
            (false, Some(name)) => vec![
                "-D".into(),
                format!("SWOFTY_EFFECT={name}"),
                "-E".into(),
                "main".into(),
            ],
            (false, None) => vec!["-E".into(), "main".into()],
        }
    }
}

fn target_args(target: Target) -> Vec<String> {
    let mut args: Vec<String> = ["-nologo", "-T", PROFILE, "-HV", "2021", "-O3"]
        .map(String::from)
        .into();
    if target == Target::Spirv {
        args.extend(SPIRV_FLAGS.map(String::from));
        args.extend(SPIRV_S_SHIFT.map(String::from));
    }
    args
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unique() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

fn signed(dxil: &[u8]) -> bool {
    dxil.len() >= 20 && dxil.starts_with(b"DXBC") && dxil[4..20].iter().any(|&b| b != 0)
}

fn valid(blob: &[u8], target: Target) -> bool {
    match target {
        Target::Dxil => signed(blob),
        Target::Spirv => {
            blob.len() >= 4 && blob.len().is_multiple_of(4) && blob[..4] == 0x0723_0203u32.to_le_bytes()
        }
    }
}

fn local_includes(file: &Path, seen: &mut Vec<PathBuf>) {
    if seen.len() > 64 {
        return;
    }
    let Ok(text) = std::fs::read_to_string(file) else {
        return;
    };
    let dir = file.parent().unwrap_or(Path::new("."));
    for line in text.lines() {
        let Some(rest) = line.trim_start().strip_prefix("#include") else {
            continue;
        };
        let Some(name) = rest.trim().strip_prefix('"').and_then(|r| r.split('"').next()) else {
            continue;
        };
        if name.starts_with("swofty/") {
            continue;
        }
        let path = dir.join(name);
        if path.is_file() && !seen.contains(&path) {
            seen.push(path.clone());
            local_includes(&path, seen);
        }
    }
}

fn key(
    source: &Path,
    entry: PassEntry,
    target: Target,
    dxc: &Path,
    includes: &[(&str, &str)],
) -> Result<String, String> {
    let mut h = Sha256::new();
    h.update(format!(
        "{target:?};{:?};{:?};",
        target_args(target),
        entry.args()
    ));
    h.update(dxc.as_os_str().as_encoded_bytes());
    if let Ok(meta) = std::fs::metadata(dxc) {
        h.update(meta.len().to_le_bytes());
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
        h.update(mtime.map_or(0, |d| d.as_nanos()).to_le_bytes());
    }
    for (name, content) in includes {
        h.update(name.as_bytes());
        h.update((content.len() as u64).to_le_bytes());
        h.update(content.as_bytes());
    }
    let mut files = vec![source.to_path_buf()];
    local_includes(source, &mut files);
    for (i, f) in files.iter().enumerate() {
        let body = std::fs::read(f).map_err(|e| format!("{}: {e}", f.display()))?;
        if i == 0 {
            h.update(f.file_name().unwrap_or_default().as_encoded_bytes());
        } else {
            h.update(
                f.strip_prefix(source.parent().unwrap_or(Path::new(".")))
                    .unwrap_or(f)
                    .as_os_str()
                    .as_encoded_bytes(),
            );
        }
        h.update((body.len() as u64).to_le_bytes());
        h.update(body);
    }
    Ok(hex(&h.finalize()))
}

/// Compiles one pass source, reusing `cache` when nothing that changes the output has changed.
/// `includes` are `(path under the include dir, content)`, normally `swofty/postfx.hlsli` and the
/// effect's `swofty/params.hlsli`.
pub fn compile_pass(
    source: &Path,
    entry: PassEntry,
    target: Target,
    dxc: &Path,
    includes: &[(&str, &str)],
    cache: &Path,
) -> Result<Vec<u8>, CompileError> {
    let key = key(source, entry, target, dxc, includes)?;
    let ext = if target == Target::Dxil { "dxil" } else { "spv" };
    let cached = cache.join(format!("{key}.{ext}"));
    if let Ok(blob) = std::fs::read(&cached)
        && valid(&blob, target)
    {
        return Ok(blob);
    }
    let mut inc_hash = Sha256::new();
    for (name, content) in includes {
        inc_hash.update(name.as_bytes());
        inc_hash.update(content.as_bytes());
    }
    let include = cache.join(format!("include-{}", &hex(&inc_hash.finalize())[..16]));
    for (name, content) in includes {
        let path = include.join(name);
        std::fs::create_dir_all(path.parent().expect("has parent"))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if std::fs::read_to_string(&path).ok().as_deref() != Some(*content) {
            let tmp = path.with_extension(format!("tmp{}", unique()));
            std::fs::write(&tmp, content).map_err(|e| format!("{}: {e}", tmp.display()))?;
            std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    let out = cache.join(format!("{key}.tmp{}", unique()));
    let mut cmd = Command::new(dxc);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let run = cmd
        .args(target_args(target))
        .args(entry.args())
        .arg("-I")
        .arg(&include)
        .arg("-Fo")
        .arg(&out)
        .arg(source)
        .output()
        .map_err(|e| format!("{}: {e}", dxc.display()))?;
    let log = String::from_utf8_lossy(&run.stderr).trim().to_owned();
    if !run.status.success() {
        let _ = std::fs::remove_file(&out);
        let first = log.lines().find(|l| l.contains("error")).unwrap_or(&log).trim();
        return Err(CompileError {
            summary: format!("dxc: {first}"),
            log,
        });
    }
    let blob = std::fs::read(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    if target == Target::Dxil && !blob.starts_with(b"DXBC") {
        let _ = std::fs::remove_file(&out);
        return Err("dxc wrote no DXIL container".to_owned().into());
    }
    if !valid(&blob, target) {
        let _ = std::fs::remove_file(&out);
        return Err(format!(
            "{} wrote an unusable {ext} file; for DXIL its validator ({}) is missing beside it",
            dxc.display(),
            if cfg!(windows) { "dxil.dll" } else { "libdxil.so" }
        )
        .into());
    }
    std::fs::rename(&out, &cached).map_err(|e| format!("{}: {e}", cached.display()))?;
    Ok(blob)
}

/// Compiles an effect's passes, one blob per pass. `dir` holds the effect's HLSL; `dxc` is the
/// compiler (`swofty_core::postfx::dxc()`); `header` is `postfx.hlsli`.
pub fn compile(
    effect: &Effect,
    dir: &Path,
    target: Target,
    dxc: &Path,
    header: &str,
    cache: &Path,
) -> Result<Vec<Vec<u8>>, String> {
    std::fs::create_dir_all(cache).map_err(|e| format!("{}: {e}", cache.display()))?;
    let params = params_hlsl(effect);
    let includes = [
        ("swofty/postfx.hlsli", header),
        ("swofty/params.hlsli", params.as_str()),
    ];
    effect
        .passes
        .iter()
        .map(|pass| {
            let entry = PassEntry {
                name: pass.entry.as_deref(),
                raw: pass.raw,
            };
            compile_pass(&dir.join(&pass.shader), entry, target, dxc, &includes, cache)
                .map_err(|e| format!("effect `{}` pass `{}`: {}", effect.id, pass.name, e.summary))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Origin, parse_effect};

    const HEADER: &str = include_str!("../../sdk/include/swofty/postfx.hlsli");
    const DXC: &str = "/home/swofty/SwoftyLauncher/dxc/bin/dxc";

    fn dxc() -> Option<PathBuf> {
        let p = std::env::var_os("SWOFTY_DXC").map_or_else(|| PathBuf::from(DXC), PathBuf::from);
        p.is_file().then_some(p)
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-shaders-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const JSON: &str = r#"{"id":"two_pass","name":"Two pass","params":[
        {"id":"gain","type":"float","default":1.5,"min":0,"max":4},
        {"id":"mode","type":"enum","values":["add","mul"],"default":"mul"},
        {"id":"tint","type":"color3","default":[1,1,1]}],
      "textures":[{"name":"half","scale":0.5,"format":"rgba16f"}],
      "passes":[{"name":"bright","shader":"two_pass.hlsl","entry":"bright","output":"half"},
                {"name":"apply","shader":"two_pass.hlsl","entry":"apply","inputs":["half"]}]}"#;

    const HLSL: &str = r#"#include "swofty/params.hlsli"
#include "common.hlsli"
float4 bright(float2 uv, uint2 pixel) { return scene(uv) * param_gain(); }
float4 apply(float2 uv, uint2 pixel)
{
    float3 h = input(0, uv).rgb * param_tint();
    return float4(param_mode() == MODE_ADD ? scene(uv).rgb + h : scene(uv).rgb * h, mix_alpha());
}
"#;

    fn setup(name: &str) -> (Effect, PathBuf, PathBuf) {
        let dir = scratch(name);
        std::fs::write(dir.join("two_pass.hlsl"), HLSL).unwrap();
        std::fs::write(dir.join("common.hlsli"), "float mix_alpha() { return 1.0; }\n").unwrap();
        let cache = dir.join("cache");
        (parse_effect(JSON, Origin::Library).unwrap(), dir, cache)
    }

    #[test]
    fn a_two_pass_effect_compiles_to_dxil_and_spirv_and_caches() {
        let Some(dxc) = dxc() else { return };
        let (effect, dir, cache) = setup("both");
        let dxil = compile(&effect, &dir, Target::Dxil, &dxc, HEADER, &cache).unwrap();
        assert_eq!(dxil.len(), 2);
        assert!(dxil.iter().all(|b| signed(b)));
        assert_ne!(dxil[0], dxil[1]);
        let spv = compile(&effect, &dir, Target::Spirv, &dxc, HEADER, &cache).unwrap();
        assert_eq!(spv.len(), 2);
        assert!(spv.iter().all(|b| valid(b, Target::Spirv)));
        let cached = std::fs::read_dir(&cache)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|x| x == "dxil" || x == "spv")
            })
            .count();
        assert_eq!(cached, 4);
        let stamp = |p: &Path| std::fs::metadata(p).unwrap().modified().unwrap();
        let first = std::fs::read_dir(&cache)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|x| x == "dxil"))
            .unwrap();
        let before = stamp(&first);
        let again = compile(&effect, &dir, Target::Dxil, &dxc, HEADER, &cache).unwrap();
        assert_eq!(again, dxil);
        assert_eq!(stamp(&first), before);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn changing_a_param_declaration_or_an_include_changes_the_output_key() {
        let Some(dxc) = dxc() else { return };
        let (effect, dir, cache) = setup("keys");
        let a = compile(&effect, &dir, Target::Dxil, &dxc, HEADER, &cache).unwrap();
        std::fs::write(dir.join("common.hlsli"), "float mix_alpha() { return 0.5; }\n").unwrap();
        let b = compile(&effect, &dir, Target::Dxil, &dxc, HEADER, &cache).unwrap();
        assert_ne!(a[1], b[1]);
        let mut shifted = effect.clone();
        shifted.params.swap(0, 2);
        shifted.params[0].slot = 0;
        shifted.params[2].slot = 4;
        let c = compile(&shifted, &dir, Target::Dxil, &dxc, HEADER, &cache).unwrap();
        assert_ne!(b[1], c[1]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_broken_pass_names_the_effect_the_pass_and_dxc() {
        let Some(dxc) = dxc() else { return };
        let (effect, dir, cache) = setup("broken");
        std::fs::write(dir.join("two_pass.hlsl"), "#include \"swofty/params.hlsli\"\nfloat4 bright(float2 uv, uint2 p) { return nope; }\nfloat4 apply(float2 uv, uint2 p) { return 0; }\n").unwrap();
        let err = compile(&effect, &dir, Target::Dxil, &dxc, HEADER, &cache).unwrap_err();
        assert!(
            err.starts_with("effect `two_pass` pass `bright`: dxc: ") && err.contains("nope"),
            "{err}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_raw_pass_uses_its_own_entry() {
        let Some(dxc) = dxc() else { return };
        let dir = scratch("raw");
        let cache = dir.join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        let src = dir.join("raw.hlsl");
        std::fs::write(&src, "#include \"swofty/postfx.hlsli\"\n[numthreads(8, 8, 1)] void go(uint3 id : SV_DispatchThreadID) { swofty_output[id.xy] = swofty_scene.Load(int3(id.xy, 0)); }\n").unwrap();
        let blob = compile_pass(
            &src,
            PassEntry {
                name: Some("go"),
                raw: true,
            },
            Target::Dxil,
            &dxc,
            &[("swofty/postfx.hlsli", HEADER)],
            &cache,
        );
        assert!(signed(&blob.unwrap()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_missing_compiler_is_an_error_not_a_panic() {
        let (effect, dir, cache) = setup("nodxc");
        let err = compile(
            &effect,
            &dir,
            Target::Dxil,
            Path::new("/nonexistent/dxc"),
            HEADER,
            &cache,
        )
        .unwrap_err();
        assert!(err.contains("two_pass") && err.contains("dxc"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_spirv_flags_are_the_contract_ones() {
        let args = target_args(Target::Spirv).join(" ");
        assert!(args.contains("-spirv -fspv-target-env=vulkan1.1 -fvk-use-dx-layout -fvk-b-shift 0 0 -fvk-t-shift 10 0 -fvk-u-shift 30 0 -fvk-s-shift 40 0"), "{args}");
        assert!(!target_args(Target::Dxil).join(" ").contains("spirv"));
    }
}

#[cfg(test)]
mod unique_tests {
    #[test]
    fn temp_names_differ_within_one_process() {
        assert_ne!(super::unique(), super::unique());
    }
}
