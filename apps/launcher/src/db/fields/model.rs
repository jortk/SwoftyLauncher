use swofty_core::dbedit::{Kind, NodeId};
use swofty_i18n::tr;

use super::FieldModel;
use super::value::range;
use super::{Nav, clean, kind_of};
use crate::shell::Shell;

impl Shell {
    pub(crate) fn db_row_fields(&self) -> Vec<FieldModel> {
        let (Some(slot), Some(row)) = (self.db.view.as_ref(), self.db.row.as_ref()) else {
            return Vec::new();
        };
        if self.db.nav != Nav::Module(slot.module.clone()) {
            return Vec::new();
        }
        let Some(module) = self.db.module(&slot.module) else {
            return Vec::new();
        };
        let Some(r) = slot.view.rows.iter().find(|r| r.label == *row) else {
            return Vec::new();
        };
        let rows = module.field_rows();
        module
            .fields
            .iter()
            .zip(&r.cells)
            .enumerate()
            .map(|(i, (f, c))| {
                let row = rows
                    .get(i)
                    .copied()
                    .flatten()
                    .and_then(|spec| Some((spec, r.backing.get(spec)?.clone())));
                let kind = kind_of(f.kind, c.tag);
                let options = match kind {
                    Kind::Choice => f
                        .options
                        .iter()
                        .map(|o| (o.value.clone(), o.label.clone()))
                        .collect(),
                    Kind::Ref | Kind::RefList => slot.choices.get(&i).cloned().unwrap_or_default(),
                    _ => Vec::new(),
                };
                FieldModel {
                    id: c.key.map_or_else(|| format!("m{i}"), |k| format!("m{i}-{k:x}")),
                    key: c.key,
                    tag: c.tag,
                    kind,
                    label: f.label.clone(),
                    section: f.section.clone(),
                    help: f.help.clone(),
                    value: c.value.clone(),
                    stock: Some(c.stock.clone()),
                    edited: c.edited,
                    min: f.min,
                    max: f.max,
                    step: f.step,
                    unit: f.unit.clone(),
                    options,
                    text: c.text.as_deref().map(clean),
                    readonly: f.readonly,
                    optional: f.optional,
                    shared: f.path.contains('[') || f.path.contains('{'),
                    others: c.key.and_then(|k| self.db_others(k)),
                    row,
                    all: (c.key.is_some() && module.sets_all(i, c.tag)).then_some(i),
                }
            })
            .collect()
    }

    pub(crate) fn db_key_field(&self, node: NodeId) -> Option<FieldModel> {
        let catalog = self.db.catalog()?;
        let record = catalog.record(catalog.node(node).record?);
        let key = record.key;
        let meta = catalog.meta(key);
        let (min, max) = meta.map_or((None, None), |m| range(&m.range));
        let edit = self.db.edit(key);
        let value = edit
            .map(|e| e.value.clone().unwrap_or_default())
            .unwrap_or_else(|| record.value.to_owned());
        let name = &catalog.node(node).name;
        Some(FieldModel {
            id: format!("k{key:x}"),
            key: Some(key),
            tag: record.tag,
            kind: kind_of(None, record.tag),
            label: tr("Value").into(),
            section: None,
            help: None,
            text: name
                .starts_with("lng_")
                .then(|| catalog.text(&value).map(clean))
                .flatten(),
            value,
            stock: Some(record.value.to_owned()),
            edited: edit.is_some(),
            min,
            max,
            step: None,
            unit: meta.map(|m| m.unit.clone()).filter(|u| !u.is_empty()),
            options: Vec::new(),
            readonly: false,
            optional: false,
            shared: false,
            others: self.db_others(key),
            row: None,
            all: None,
        })
    }

    pub(super) fn db_detail_fields(&self) -> Vec<FieldModel> {
        match (&self.db.nav, self.db.key) {
            (Nav::Module(_), _) => self.db_row_fields(),
            (Nav::Raw(_) | Nav::Unnamed, Some(node)) => self.db_key_field(node).into_iter().collect(),
            _ => Vec::new(),
        }
    }
}
