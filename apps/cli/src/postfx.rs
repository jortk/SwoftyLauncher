use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use swofty_core::manifest::{AddonManifest, MANIFEST_FILE};
use swofty_core::postfx;

#[derive(Subcommand)]
pub enum PostfxCommand {
    #[command(
        about = "Compile an addon's post-process passes with dxc the way the build does, and print dxc's errors and warnings"
    )]
    Compile {
        #[arg(help = "The addon folder, with swofty.json")]
        dir: PathBuf,
        #[arg(long, help = "Also write each pass's DXIL here, as <name>.dxil")]
        out: Option<PathBuf>,
    },
}

pub fn command(cmd: PostfxCommand) -> Result<ExitCode> {
    match cmd {
        PostfxCommand::Compile { dir, out } => compile(&dir, out.as_deref()),
    }
}

fn compile(dir: &Path, out: Option<&Path>) -> Result<ExitCode> {
    let path = dir.join(MANIFEST_FILE);
    let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let m = AddonManifest::parse(&text).with_context(|| format!("{}", path.display()))?;
    if m.postfx.is_empty() {
        bail!("{} lists no postfx passes", m.name);
    }
    match postfx::dxc() {
        Some(dxc) => println!("dxc: {}", dxc.display()),
        None => bail!("dxc was not found (set SWOFTY_DXC, or put dxc on PATH)"),
    }
    let cache = std::env::temp_dir().join(format!("swofty-postfx-check-{}", std::process::id()));
    if let Some(out) = out {
        std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    }
    let mut failed = 0;
    for pass in &m.postfx {
        let label = pass.label();
        let entry = postfx::Entry::of(pass);
        let shader = match (entry.raw, entry.name) {
            (true, name) => format!("{}, raw {}", pass.shader, name.unwrap_or("main")),
            (false, Some(name)) => format!("{}, {name}", pass.shader),
            (false, None) => pass.shader.clone(),
        };
        match postfx::compile(&dir.join(&pass.shader), &cache, entry) {
            Ok(c) => {
                println!("ok    {label} ({shader}): {} bytes of signed DXIL", c.dxil.len());
                for line in c.log.lines() {
                    println!("      {line}");
                }
                if let Some(out) = out {
                    let file = out.join(format!("{label}.dxil"));
                    std::fs::write(&file, &c.dxil).with_context(|| format!("writing {}", file.display()))?;
                }
            }
            Err(e) => {
                failed += 1;
                println!("FAIL  {label} ({shader}): {}", e.summary);
                for line in e.log.lines() {
                    println!("      {line}");
                }
            }
        }
    }
    std::fs::remove_dir_all(&cache).ok();
    Ok(if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
