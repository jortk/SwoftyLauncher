use std::collections::BTreeMap;
use std::path::PathBuf;

use swofty_core::asset::{AssetOp, FileKind, OpFile};
use swofty_core::dbedit::deps::{self, Copy};
use swofty_core::manifest::AddonManifest;
use swofty_core::ops::{CopyMode, DbOp, DbOps, DbReport, apply_db_ops_report};
use swofty_core::profile::Profile;
use swofty_core::rowkeys::{Names, RowKeys};
use swofty_core::settings::{OnAddons, render_ops};
use swofty_formats::gvdb::Gvdb;

fn arg(args: &[String], name: &str) -> Option<PathBuf> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let game = arg(&args, "--game").expect("--game <F1 25 folder>");
    let addon = arg(&args, "--addon").expect("--addon <addon folder>");
    let listed = args.iter().any(|a| a == "--listed");
    let strict = args.iter().any(|a| a == "--strict");
    let db_path = game.join("game_package/gamedatabases/gamedatabases.2025.bin");
    let mut db =
        Gvdb::parse(&std::fs::read(db_path).expect("read the database")).expect("parse the database");
    let raw = std::fs::read_to_string(addon.join("swofty.json")).expect("read swofty.json");
    let manifest = AddonManifest::parse(&raw).expect("parse swofty.json");
    let profile: Profile = serde_json::from_str(r#"{"schema_version":1,"name":"check"}"#).expect("profile");
    let files = manifest.operations.database.clone();
    let none = BTreeMap::new();
    let names = Names {
        fields: &none,
        keys: RowKeys::builtin(),
        stock: None,
        tracks: None,
    };
    let (mut warnings, mut gaps) = (0, 0);
    let mut copies = Vec::new();
    for rel in files {
        let text = std::fs::read(addon.join(&rel)).expect("read the op file");
        let mut doc: serde_json::Value = serde_json::from_slice(&text).expect("op file json");
        let file = render_ops(&mut doc, &manifest, &profile, &OnAddons::default())
            .map_err(|e| e.to_string())
            .and_then(|()| OpFile::from_value(doc, FileKind::Database).map_err(|e| e.to_string()));
        let file = match file {
            Ok(file) => file,
            Err(e) => {
                println!("== {rel}: not a database op file ({e})");
                continue;
            }
        };
        let mut ops = DbOps {
            schema_version: file.schema_version,
            fields: file.fields,
            strict: file.strict,
            side_data: file.side_data,
            ops: file
                .ops
                .into_iter()
                .filter_map(|op| match op {
                    AssetOp::Db(op) => Some(op),
                    _ => None,
                })
                .collect(),
        };
        ops.strict |= strict;
        if listed {
            for op in &mut ops.ops {
                if let DbOp::AddRow { copy, .. } = op {
                    copy.get_or_insert(CopyMode::Listed);
                }
            }
        }
        let mut report = DbReport::default();
        let result = apply_db_ops_report(&mut db, &ops, &names, &mut report);
        println!("== {rel}");
        for l in report
            .lines
            .iter()
            .filter(|l| l.contains("doesn't name") || l.contains("stat_id"))
        {
            gaps += usize::from(l.contains("doesn't name"));
            println!("  {l}");
        }
        for w in &report.warnings {
            warnings += 1;
            println!("  warning: {w}");
        }
        if let Err(e) = result {
            println!("  error: {e}");
        }
        copies.extend(report.added_rows.into_iter().map(|r| Copy {
            addon: manifest.name.clone(),
            from: r.from,
            row: r.row,
            ignore: ops.side_data.ignore.clone(),
            quiet: false,
        }));
    }
    println!("== label-keyed side data of {} add-row(s)", copies.len());
    let side = deps::check(&db, &copies, &|id| id.to_owned());
    for w in &side.warnings {
        warnings += 1;
        println!("  warning: {w}");
    }
    for f in &side.findings {
        println!("  {f}");
    }
    println!("{warnings} warning(s), {gaps} fields list gap(s) closed by the full copy");
}
