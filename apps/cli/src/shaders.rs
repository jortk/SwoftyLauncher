use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use swofty_core::shaders::{self, ConfigRef};
use swofty_core::store::Store;
use swofty_shaders::{ParamKind, Severity, Value};

#[derive(Subcommand)]
pub enum ShadersCommand {
    #[command(about = "List the shader configs, personal first, and mark the active one")]
    List,
    #[command(about = "List the effects, or show one effect's params")]
    Effects { id: Option<String> },
    #[command(about = "Print a config")]
    Show {
        #[arg(help = "personal:<slug>, addon:<id>/<file>, or a config's name")]
        config: String,
    },
    #[command(about = "Check a config file or a listed config; exits 1 on errors")]
    Lint {
        #[arg(help = "A .swfx file, or a config as in `show`")]
        config: String,
    },
    #[command(name = "use", about = "Make a config the profile's shader config, or `off`")]
    Use { config: String },
    #[command(about = "Create a personal config from the starter template")]
    New { name: String },
    #[command(about = "Copy a config to a personal one")]
    Copy { config: String },
    #[command(about = "Rename a personal config")]
    Rename { config: String, name: String },
    #[command(about = "Delete a personal config")]
    Delete { config: String },
    #[command(about = "Add a .swfx file to the personal configs")]
    Import { file: PathBuf },
    #[command(about = "Write a config to a file")]
    Export { config: String, file: PathBuf },
    #[command(about = "Show or set the in-game on/off key: A-Z, 0-9, F1-F24 or None")]
    Key { key: Option<String> },
}

fn find(store: &Store, text: &str) -> Result<ConfigRef> {
    if let Ok(r) = text.parse::<ConfigRef>() {
        shaders::read(store, &r).with_context(|| format!("shader config {text}"))?;
        return Ok(r);
    }
    let all = shaders::list(store);
    let wanted = text.to_lowercase();
    match all.iter().find(|e| e.name.to_lowercase() == wanted) {
        Some(e) => Ok(e.r#ref.clone()),
        None => bail!("no shader config {text:?}; `swofty shaders list` shows them"),
    }
}

pub fn command(store: &Store, profile: &str, cmd: ShadersCommand) -> Result<ExitCode> {
    match cmd {
        ShadersCommand::List => list(store, profile)?,
        ShadersCommand::Effects { id } => effects(store, id.as_deref())?,
        ShadersCommand::Show { config } => {
            print!("{}", shaders::read(store, &find(store, &config)?)?);
        }
        ShadersCommand::Lint { config } => return lint(store, &config),
        ShadersCommand::Use { config } => {
            let mut p = store.profile(profile)?;
            if config.eq_ignore_ascii_case("off") {
                shaders::set_active(store, &mut p, None)?;
                println!("shaders off in {profile}");
            } else {
                let r = find(store, &config)?;
                shaders::set_active(store, &mut p, Some(r.clone()))?;
                println!("{r} is the shader config of {profile}");
                if let Some((forced, addon)) = shaders::forced(store, &p) {
                    println!("{addon} forces {forced} while it is enabled");
                }
            }
        }
        ShadersCommand::New { name } => println!("{}", shaders::create(store, &name, None)?),
        ShadersCommand::Copy { config } => {
            println!("{}", shaders::copy_to_personal(store, &find(store, &config)?)?);
        }
        ShadersCommand::Rename { config, name } => {
            println!("{}", shaders::rename(store, &find(store, &config)?, &name)?);
        }
        ShadersCommand::Delete { config } => {
            let r = find(store, &config)?;
            shaders::delete(store, &r)?;
            println!("deleted {r}");
        }
        ShadersCommand::Import { file } => println!("{}", shaders::import(store, &file)?),
        ShadersCommand::Export { config, file } => {
            shaders::export(store, &find(store, &config)?, &file)?;
            println!("wrote {}", file.display());
        }
        ShadersCommand::Key { key } => match key {
            Some(key) => {
                shaders::set_toggle_key(store, &key)?;
                println!("{}", shaders::toggle_key(store));
            }
            None => println!("{}", shaders::toggle_key(store)),
        },
    }
    Ok(ExitCode::SUCCESS)
}

fn list(store: &Store, profile: &str) -> Result<()> {
    let p = store.profile(profile)?;
    let active = shaders::active(store, &p);
    let forced = shaders::forced(store, &p);
    for e in shaders::list(store) {
        let mark = match (&forced, &active) {
            (Some((r, _)), _) if *r == e.r#ref => "forced",
            (None, Some(r)) if *r == e.r#ref => "active",
            _ => "",
        };
        println!("{:44} {mark:7} {} ({})", e.r#ref.to_string(), e.name, e.source);
    }
    match (&forced, &active) {
        (Some((r, addon)), _) => println!("shaders: {r}, forced by {addon}"),
        (None, Some(r)) => println!("shaders: {r}"),
        (None, None) => println!("shaders: off"),
    }
    Ok(())
}

fn effects(store: &Store, id: Option<&str>) -> Result<()> {
    let catalog = shaders::catalog(store);
    let Some(id) = id else {
        for e in &catalog.effects {
            let source = match &e.origin {
                swofty_shaders::Origin::Library => "library".to_owned(),
                swofty_shaders::Origin::Addon { name, .. } => name.clone(),
            };
            println!("{:20} {:2} slots  {} ({source})", e.id, e.slots, e.name);
        }
        return Ok(());
    };
    let Some(e) = catalog.get(id) else {
        bail!("no effect {id:?}; `swofty shaders effects` lists them");
    };
    println!("{}: {}\n{}", e.id, e.name, e.description);
    for p in &e.params {
        println!(
            "  {:24} {:28} default {}  {}",
            p.id,
            kind(&p.kind),
            value(&p.default),
            p.label
        );
    }
    Ok(())
}

fn value(v: &Value) -> String {
    match v {
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Text(t) => format!("{t:?}"),
        Value::Vector(v) => format!("{v:?}"),
    }
}

fn range(min: Option<f64>, max: Option<f64>) -> String {
    match (min, max) {
        (Some(a), Some(b)) => format!(" {a}..{b}"),
        _ => String::new(),
    }
}

fn kind(k: &ParamKind) -> String {
    match k {
        ParamKind::Float { min, max, .. } => format!("float{}", range(*min, *max)),
        ParamKind::Int { min, max } => {
            format!("int{}", range(min.map(|v| v as f64), max.map(|v| v as f64)))
        }
        ParamKind::Bool => "bool".into(),
        ParamKind::Enum { values } => format!("enum {}", values.join("|")),
        ParamKind::Float2 { min, max } => format!("float2{}", range(*min, *max)),
        ParamKind::Color3 { min, max } => format!("color3{}", range(*min, *max)),
        ParamKind::Color4 { min, max } => format!("color4{}", range(*min, *max)),
    }
}

fn lint(store: &Store, config: &str) -> Result<ExitCode> {
    let (label, text) = if Path::new(config).is_file() {
        (
            config.to_owned(),
            std::fs::read_to_string(config).with_context(|| format!("reading {config}"))?,
        )
    } else {
        let r = find(store, config)?;
        (r.to_string(), shaders::read(store, &r)?)
    };
    let result = swofty_shaders::lint(&text, &shaders::catalog(store));
    for d in &result.diagnostics {
        let severity = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
            Severity::Hint => "hint",
        };
        println!(
            "{label}:{}:{}: {severity}: {}",
            d.start.line + 1,
            d.start.character + 1,
            d.message
        );
        if let Some(fix) = &d.fix {
            println!(
                "{label}:{}:{}: fix: {}",
                d.start.line + 1,
                d.start.character + 1,
                fix.title
            );
        }
    }
    Ok(if result.errors() > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
