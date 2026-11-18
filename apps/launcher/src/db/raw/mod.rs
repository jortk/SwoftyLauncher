mod add;
mod browse;
mod changes;
mod unnamed;

use crate::scale::px;
use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::*;
use swofty_core::dbedit::{Catalog, Edit, NodeId, NodeKind, RowCopy};

use super::{ADDABLE, Nav, hex, type_name};
use crate::format::title_case;
use crate::theme::Palette;

pub(super) const LIST_H: f32 = 44.;
pub(super) const GRID_COL: f32 = 150.;
pub(super) const GRID_LABEL: f32 = 200.;
pub(super) const GRID_HEADER: f32 = 32.;

pub(super) fn kind_icon(kind: &NodeKind) -> IconName {
    match kind {
        NodeKind::Root | NodeKind::Container => IconName::Folder,
        NodeKind::Table { .. } => IconName::Table2,
        NodeKind::Row { .. } => IconName::Rows3,
        NodeKind::Key => IconName::KeyRound,
        NodeKind::Unnamed => IconName::Binary,
    }
}

pub(super) fn icon_tile(icon: IconName, p: Palette) -> Div {
    div()
        .size(px(26.))
        .flex_shrink_0()
        .rounded(px(5.))
        .bg(p.elevated)
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(icon).size(px(13.)).text_color(p.text_muted))
}

// The row a key belongs to and the key's path under it, for a friendly name.
pub(super) fn friendly(catalog: &Catalog, node: NodeId) -> (String, Option<NodeId>) {
    let mut parts = vec![catalog.node(node).name.clone()];
    let mut at = catalog.node(node).parent;
    while let Some(id) = at {
        let n = catalog.node(id);
        if matches!(n.kind, NodeKind::Row { .. }) {
            let row = n
                .display
                .clone()
                .or_else(|| n.label.clone())
                .unwrap_or_else(|| n.name.clone());
            parts.reverse();
            return (format!("{row} › {}", parts.join(".")), Some(id));
        }
        if matches!(n.kind, NodeKind::Root | NodeKind::Unnamed) {
            break;
        }
        parts.push(n.name.clone());
        at = n.parent;
    }
    parts.reverse();
    (parts.join("."), None)
}

pub(super) fn table_of(catalog: &Catalog, node: NodeId) -> Option<NodeId> {
    let mut at = catalog.node(node).parent;
    while let Some(id) = at {
        if matches!(catalog.node(id).kind, NodeKind::Table { .. }) {
            return Some(id);
        }
        at = catalog.node(id).parent;
    }
    None
}

pub(super) fn shape(path: &str) -> String {
    let mut out = String::new();
    let mut depth = 0;
    for c in path.chars() {
        match c {
            '[' => depth += 1,
            ']' => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split('.')
        .map(|seg| match seg.rsplit_once('_') {
            Some((head, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => head,
            _ => seg,
        })
        .collect::<Vec<_>>()
        .join(".")
}

pub(crate) struct ChangeItem {
    pub group: Option<String>,
    pub edit: Option<Edit>,
    pub row: Option<RowCopy>,
    pub name: String,
    pub path: Option<String>,
    pub node: Option<NodeId>,
}
