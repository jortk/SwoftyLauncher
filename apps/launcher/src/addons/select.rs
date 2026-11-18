#[derive(Default)]
pub(super) struct Selection {
    ids: Vec<String>,
}

impl Selection {
    pub(super) fn toggle(&mut self, id: &str) {
        match self.ids.iter().position(|i| i == id) {
            Some(at) => {
                self.ids.remove(at);
            }
            None => self.ids.push(id.to_owned()),
        }
    }

    pub(super) fn contains(&self, id: &str) -> bool {
        self.ids.iter().any(|i| i == id)
    }

    pub(super) fn clear(&mut self) {
        self.ids.clear();
    }

    pub(super) fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub(super) fn len(&self) -> usize {
        self.ids.len()
    }

    pub(super) fn ids(&self) -> &[String] {
        &self.ids
    }

    pub(super) fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        self.ids.retain(|id| keep(id));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Bulk {
    On,
    Off,
    Uninstall,
}

pub(super) struct Pick {
    pub(super) id: String,
    pub(super) locked: bool,
    pub(super) builtin: bool,
    pub(super) shader_only: bool,
    pub(super) warns_on_off: bool,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Plan {
    pub(super) apply: Vec<String>,
    pub(super) skipped: usize,
    pub(super) warned: Vec<String>,
}

pub(super) fn plan(action: Bulk, picks: &[Pick]) -> Plan {
    let mut out = Plan::default();
    for pick in picks {
        let fixed = match action {
            Bulk::On | Bulk::Off => pick.locked || pick.shader_only,
            Bulk::Uninstall => pick.locked || pick.builtin,
        };
        if fixed {
            out.skipped += 1;
        } else if action == Bulk::Off && pick.warns_on_off {
            out.warned.push(pick.id.clone());
        } else {
            out.apply.push(pick.id.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(id: &str) -> Pick {
        Pick {
            id: id.to_owned(),
            locked: false,
            builtin: false,
            shader_only: false,
            warns_on_off: false,
        }
    }

    #[test]
    fn toggle_selects_then_deselects() {
        let mut s = Selection::default();
        s.toggle("a");
        s.toggle("b");
        assert_eq!(s.ids(), ["a", "b"]);
        assert!(s.contains("a"));
        s.toggle("a");
        assert_eq!(s.ids(), ["b"]);
        assert_eq!(s.len(), 1);
        assert!(!s.is_empty());
    }

    #[test]
    fn clear_empties_the_selection() {
        let mut s = Selection::default();
        s.toggle("a");
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn retain_drops_ids_that_are_gone() {
        let mut s = Selection::default();
        s.toggle("a");
        s.toggle("b");
        s.retain(|id| id == "b");
        assert_eq!(s.ids(), ["b"]);
    }

    #[test]
    fn turning_on_skips_always_on_and_shader_only() {
        let locked = Pick {
            locked: true,
            ..pick("core")
        };
        let shader = Pick {
            shader_only: true,
            ..pick("look")
        };
        let p = plan(Bulk::On, &[pick("a"), locked, shader, pick("b")]);
        assert_eq!(p.apply, ["a", "b"]);
        assert_eq!(p.skipped, 2);
        assert!(p.warned.is_empty());
    }

    #[test]
    fn turning_off_leaves_addons_that_warn_about_saves() {
        let warns = Pick {
            warns_on_off: true,
            ..pick("drivers")
        };
        let p = plan(Bulk::Off, &[pick("a"), warns]);
        assert_eq!(p.apply, ["a"]);
        assert_eq!(p.warned, ["drivers"]);
        assert_eq!(p.skipped, 0);
    }

    #[test]
    fn turning_on_ignores_the_saves_warning() {
        let warns = Pick {
            warns_on_off: true,
            ..pick("drivers")
        };
        assert_eq!(plan(Bulk::On, &[warns]).apply, ["drivers"]);
    }

    #[test]
    fn uninstall_skips_built_in_and_always_on() {
        let builtin = Pick {
            builtin: true,
            ..pick("simplified")
        };
        let locked = Pick {
            locked: true,
            ..pick("core")
        };
        let warns = Pick {
            warns_on_off: true,
            ..pick("drivers")
        };
        let p = plan(Bulk::Uninstall, &[builtin, pick("a"), locked, warns]);
        assert_eq!(p.apply, ["a", "drivers"]);
        assert_eq!(p.skipped, 2);
    }

    #[test]
    fn uninstall_allows_a_shader_only_addon() {
        let shader = Pick {
            shader_only: true,
            ..pick("look")
        };
        assert_eq!(plan(Bulk::Uninstall, &[shader]).apply, ["look"]);
    }
}
