//! How the composer sits in a profile build: it runs last among the archive changes, starts from the
//! resources as the other addons and the launcher leave them, and its overlay replaces those
//! resources with the same content plus its nodes.

use std::collections::BTreeSet;
use std::path::PathBuf;

use swofty_addon::asset::{AssetOp, Op, Origin};
use swofty_addon::ops::ErpOp;

use super::{Input, Layered, Output, Stock, TemplateBase, compose_with, dials, screen_build, screens};
use crate::erpbuild::ErpBuild;
use crate::store::Store;
use crate::{Error, Result};

/// The display line of the composer's origin in conflict reports and the runtime layering.
pub const ORIGIN: &str = "HUD composer";

pub struct Context<'a> {
    pub store: &'a Store,
    pub stock: &'a dyn Stock,
    pub base: &'a dyn Fn(&str) -> PathBuf,
    pub baseline: &'a dyn Fn(&str) -> PathBuf,
    pub puts: &'a [String],
    pub name: &'a dyn Fn(&str) -> String,
}

pub struct Composed {
    pub output: Output,
    pub ops: Vec<Op>,
    pub lines: Vec<String>,
    pub warnings: Vec<String>,
}

/// The archives screen records write beyond the dials' (`hud/screens.rs`).
pub const SCREEN_ERPS: [&str; 4] = [
    screens::FLOW_REPLAY,
    screens::FLOW_FRONTEND,
    screens::REPLAY_SCREEN_ERP,
    "2025_asset_groups/ui_package/flow_options.erp",
];

fn touched(op: &ErpOp, written: &BTreeSet<&str>, source: &dyn Fn() -> Option<PathBuf>) -> bool {
    let ours = |erps: &[&str]| {
        erps.iter()
            .any(|e| dials::SCENE_ERPS.contains(e) || SCREEN_ERPS.contains(e))
    };
    match op {
        ErpOp::PutResource {
            erp,
            resource,
            rename,
            ..
        }
        | ErpOp::CopyResource {
            erp,
            resource,
            rename,
            ..
        } => ours(&[erp]) && written.contains(rename.as_deref().unwrap_or(resource)),
        ErpOp::ReplaceResource { erp, resource, .. }
        | ErpOp::RemoveResource { erp, resource }
        | ErpOp::AddResource { erp, resource, .. }
        | ErpOp::PatchPkg { erp, resource, .. } => ours(&[erp]) && written.contains(resource.as_str()),
        ErpOp::Overlay { erps, .. } => {
            let erps: Vec<&str> = erps.iter().map(String::as_str).collect();
            ours(&erps)
                && source()
                    .and_then(|p| crate::erpbuild::overlay_names(&p).ok())
                    .is_some_and(|names| names.iter().any(|n| written.contains(n.as_str())))
        }
        _ => false,
    }
}

/// Composes `inputs` for a build: the archive changes in `erp_ops` that touch what the composer writes
/// are applied first to a scratch build, and the composer reads its resources from that. `template`
/// edits the template the composer starts from.
pub fn compose_in_build(
    cx: &Context,
    erp_ops: &[Op],
    inputs: &[Input],
    template: Option<TemplateBase>,
) -> Result<Composed> {
    let mut written: BTreeSet<&str> = dials::written().into_iter().collect();
    written.extend(inputs.iter().filter_map(|i| i.record.scene.as_deref()));
    written.extend(
        inputs
            .iter()
            .flat_map(|i| i.record.stock_nodes.iter())
            .filter(|s| !s.wrap.is_empty())
            .filter_map(|s| s.scene.as_deref()),
    );
    if inputs.iter().any(|i| i.record.screen.is_some()) {
        written.extend(screen_build::written());
    }
    let mut scratch = ErpBuild::new(cx.base).with_baseline(cx.baseline);
    let mut lines = Vec::new();
    let mut warnings = Vec::new();
    let huds: Vec<String> = {
        let mut v: Vec<String> = inputs.iter().map(|i| (cx.name)(&i.addon)).collect();
        v.dedup();
        v.sort();
        v.dedup();
        v
    };
    for rel in cx
        .puts
        .iter()
        .filter(|r| dials::SCENE_ERPS.contains(&r.as_str()) || SCREEN_ERPS.contains(&r.as_str()))
    {
        scratch.load(rel)?;
        lines.push(format!(
            "{rel} is replaced whole by an addon; the composed overlay starts from that archive's resources"
        ));
    }
    for op in erp_ops {
        let AssetOp::Erp(e) = &op.op else { continue };
        let origin = op.origin.to_string();
        let resolve = |s: &swofty_addon::asset::Source| {
            Ok(s.resolve(&crate::build::source_dir(cx.store, &op.origin.addon, s.root))?)
        };
        let path = || match e {
            ErpOp::Overlay { source, .. } => resolve(source).ok(),
            _ => None,
        };
        if !touched(e, &written, &path) {
            continue;
        }
        match scratch.apply_for(e, &origin, &resolve) {
            Ok(()) => lines.push(format!(
                "{} changes a resource the composer writes; the composed overlay is built on top of that change",
                (cx.name)(&origin)
            )),
            Err(err) => warnings.push(format!(
                "{}: its change to a HUD resource cannot be applied under {}'s composed HUD ({err}); the composed overlay replaces that resource with a copy of the game's own",
                (cx.name)(&origin),
                huds.join(", ")
            )),
        }
    }
    let layered = Layered {
        inner: cx.stock,
        build: &scratch,
    };
    let output = compose_with(&layered, inputs, template)
        .map_err(|e| Error::Other(format!("the HUD elements do not compose: {e}")))?;
    let left = super::check::uncovered(&layered, &output.archives)?;
    if !left.is_empty() {
        return Err(Error::Other(format!(
            "the composed HUD of {} would leave the game's own copy of a resource beside the composed one, and the game crashes loading the HUD when it mixes them: {}. Turn {} off in this profile and use Send Logs.",
            huds.join(", "),
            left.join("; "),
            huds.join(", ")
        )));
    }
    let rotations = super::check::constant_rotations(&output.archives)?;
    if !rotations.is_empty() {
        return Err(Error::Other(format!(
            "the composed HUD of {} has clips with a one-key rotation the game can read misaligned and crash on: {}",
            huds.join(", "),
            rotations.join("; ")
        )));
    }
    let unpadded = super::check::unpadded_clips(&output.archives)?;
    if !unpadded.is_empty() {
        return Err(Error::Other(format!(
            "the composed HUD of {} has clips whose key data the game can place misaligned and crash on: {}",
            huds.join(", "),
            unpadded.join("; ")
        )));
    }
    let owner = inputs.first().map(|i| i.addon.clone()).unwrap_or_default();
    let ops = output
        .ops("hud")
        .into_iter()
        .map(|op| Op {
            op: AssetOp::Erp(op),
            origin: Origin {
                addon: owner.clone(),
                item: Some(ORIGIN.into()),
            },
        })
        .collect();
    lines.extend(output.lines.iter().cloned());
    Ok(Composed {
        output,
        ops,
        lines,
        warnings,
    })
}
