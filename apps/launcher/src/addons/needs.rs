use crate::library::AddonItem;
use crate::shell::Shell;
use std::collections::HashMap;

/// One of an addon's requirements that stops Play, or that the build loads before it though listed below it.
pub(super) struct Need {
    pub(super) id: String,
    pub(super) kind: NeedKind,
}

pub(super) enum NeedKind {
    Missing,
    Off,
    Version { found: String },
    Below,
}

impl Need {
    pub(super) fn blocks(&self) -> bool {
        !matches!(self.kind, NeedKind::Below)
    }
}

impl Shell {
    /// The requirements of an addon that's on which stop Play, and those listed below it, which the build
    /// loads first anyway.
    pub(super) fn needs(&self, m: &AddonItem) -> Vec<Need> {
        if self.vanilla() || !m.enabled {
            return Vec::new();
        }
        let order: HashMap<&str, usize> = self
            .library
            .profile
            .enabled()
            .enumerate()
            .map(|(i, id)| (id, i))
            .collect();
        m.manifest
            .requires
            .iter()
            .filter_map(|r| {
                let need = |kind| Need {
                    id: r.id.clone(),
                    kind,
                };
                let Some(dep) = self.library.addons.iter().find(|a| a.manifest.id == r.id) else {
                    return Some(need(NeedKind::Missing));
                };
                if !dep.enabled {
                    return Some(need(NeedKind::Off));
                }
                if !swofty_core::version::satisfies(&r.version, &dep.manifest.version) {
                    return Some(need(NeedKind::Version {
                        found: dep.manifest.version.to_string(),
                    }));
                }
                match (order.get(r.id.as_str()), order.get(m.manifest.id.as_str())) {
                    (Some(dep_at), Some(at)) if dep_at > at => Some(need(NeedKind::Below)),
                    _ => None,
                }
            })
            .collect()
    }
}
