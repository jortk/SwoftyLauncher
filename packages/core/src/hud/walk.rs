//! The budget of the game's UI tree walk (`0x144167620`, called by the job `0x144344970`).
//!
//! The game flattens a screen's node objects into 16-byte slots (`0x14416ac60`): one slot per node,
//! plus `(n + 4) / 8` more for the child list of a node with four or more children. Children are
//! named by a `u16` slot index, so a flattened tree holds at most 65,536 slots; past that the
//! indices wrap to the start of the tree and the walk never ends. The walk keeps a fixed stack of
//! 256 entries: a node with `n` children pushes itself and its other `n - 1` children, walks the
//! first child at once, and the game aborts (`Ne_FatalAbort`) when a push finds the stack full.

use swofty_formats::pkg::Node;

/// Slots of one flattened tree (`u16` child indices).
pub const SLOT_LIMIT: usize = 0x1_0000;
/// Entries of the walk's stack.
pub const STACK_LIMIT: usize = 0x100;

/// What a subtree costs the walk.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cost {
    pub nodes: usize,
    pub slots: usize,
    /// The most stack entries the subtree holds at once, walked from an empty stack.
    pub stack: usize,
    /// The largest child count of one node.
    pub widest: usize,
}

/// Slots a node with `children` children takes by itself.
pub fn own_slots(children: usize) -> usize {
    if children < 4 { 1 } else { 1 + (children + 4) / 8 }
}

/// Whether a node with `children` children can be walked with `held` entries on the stack.
pub fn fits(held: usize, children: usize) -> bool {
    held + children <= STACK_LIMIT
}

fn kids(e: &Node) -> impl Iterator<Item = &Node> {
    e.as_obj()
        .and_then(|o| o.get("children"))
        .and_then(Node::as_arr)
        .into_iter()
        .flat_map(|a| a.items.iter().filter(|c| c.as_obj().is_some()))
}

impl Cost {
    /// A node whose children cost `children`, in the scene's order.
    pub fn of(children: &[Cost]) -> Cost {
        let n = children.len();
        let mut out = Cost {
            nodes: 1,
            slots: own_slots(n),
            stack: n,
            widest: n,
        };
        for (i, c) in children.iter().enumerate() {
            out.nodes += c.nodes;
            out.slots += c.slots;
            out.widest = out.widest.max(c.widest);
            if c.stack > 0 {
                out.stack = out.stack.max(n - i + c.stack);
            }
        }
        out
    }

    /// Whether a tree that holds this subtree `instances` times is sure to stop the game.
    pub fn over(&self, instances: usize) -> bool {
        self.stack > STACK_LIMIT || self.slots * instances > SLOT_LIMIT
    }
}

/// The cost of the scene element `e` and everything under it.
pub fn cost(e: &Node) -> Cost {
    let children: Vec<Cost> = kids(e).map(cost).collect();
    Cost::of(&children)
}

/// The build lines for the record `pfx` of `addon`, whose live root costs `live` and whose editor
/// copy costs `preview`, in a host scene that every screen tree holds `instances` times.
pub fn lines(addon: &str, pfx: &str, live: Cost, preview: Cost, instances: usize) -> Vec<String> {
    let both = Cost {
        nodes: live.nodes + preview.nodes,
        slots: live.slots + preview.slots,
        stack: live.stack.max(preview.stack),
        widest: live.widest.max(preview.widest),
    };
    let mut out = vec![format!(
        "{pfx}: {addon} adds {} nodes to the host scene ({} walk slots, {} in the editor copy; walk stack {} of {STACK_LIMIT}); a screen holds the host scene {instances} times per board, {} of the {SLOT_LIMIT} slots of one UI tree",
        both.nodes,
        both.slots,
        preview.slots,
        both.stack,
        both.slots * instances
    )];
    if both.over(instances) {
        out.push(format!(
            "warning: {pfx}: {addon} is past the game's UI walk limits ({} walk slots in {instances} instances, walk stack {}); the game aborts on a screen that shows it",
            both.slots, both.stack
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEAF: Cost = Cost {
        nodes: 1,
        slots: 1,
        stack: 0,
        widest: 0,
    };

    fn wide(n: usize, child: Cost) -> Cost {
        Cost::of(&vec![child; n])
    }

    #[test]
    fn a_child_list_spills_into_more_slots_from_four_children() {
        assert_eq!([0, 3, 4, 11, 12, 26, 59].map(own_slots), [1, 1, 2, 2, 3, 4, 8]);
    }

    #[test]
    fn the_stack_holds_the_node_and_its_waiting_siblings_on_the_way_down() {
        assert_eq!(wide(26, LEAF).stack, 26);
        let chain = Cost::of(&[Cost::of(&[LEAF])]);
        assert_eq!(chain.stack, 2);
        // the last child is walked with only its parent's entry left
        assert_eq!(Cost::of(&[LEAF, LEAF, wide(5, LEAF)]).stack, 6);
        assert_eq!(Cost::of(&[wide(5, LEAF), LEAF, LEAF]).stack, 8);
    }

    /// Both field reports: a node with 26 children, each a node with one child (two slots apart),
    /// met the stack with 236 entries held, and the push of the 20th entry found it full.
    #[test]
    fn the_reported_abort_is_a_26_child_node_on_a_stack_of_236() {
        let node = wide(26, Cost::of(&[LEAF]));
        assert_eq!(node.slots, 4 + 26 * 2);
        assert_eq!(node.stack, 27);
        assert!(!fits(236, node.widest));
        assert!(fits(236, 20) && !fits(236, 21));
        assert!(fits(229, node.stack));
    }

    /// TV Timing Tower 0.7.2 as composed: 24 rows of 403 nodes (443 slots), 10,907 slots a host
    /// scene, twice per board. One board fits a tree; the report's tree held 66,306 slots.
    #[test]
    fn the_reported_tower_fits_one_board_and_not_three() {
        let row = Cost {
            nodes: 403,
            slots: 443,
            stack: 61,
            widest: 59,
        };
        let rows = wide(24, row);
        assert_eq!((rows.nodes, rows.slots, rows.stack), (9673, 10636, 85));
        let live = Cost {
            nodes: 9827,
            slots: 10806,
            stack: 88,
            widest: 59,
        };
        let preview = Cost {
            nodes: 97,
            slots: 101,
            stack: 14,
            widest: 5,
        };
        let out = lines("pablo.tv-timing-tower", "tvtt", live, preview, 2);
        assert_eq!(out.len(), 1, "{out:?}");
        assert!(
            out[0].contains("10907 walk slots") && out[0].contains("21814 of the 65536"),
            "{out:?}"
        );
        let host = Cost { slots: 10915, ..live };
        assert!(!host.over(2));
        assert_eq!(6 * host.slots + 816, 66306);
        assert!(6 * host.slots + 816 > SLOT_LIMIT);
    }

    #[test]
    fn a_record_past_a_limit_gets_a_warning_that_names_it() {
        let deep = wide(200, wide(60, LEAF));
        assert!(deep.stack > STACK_LIMIT);
        let out = lines("a.b", "ab", deep, LEAF, 2);
        assert_eq!(out.len(), 2);
        assert!(out[1].starts_with("warning: ab: a.b is past"), "{out:?}");
        let big = wide(100, wide(100, wide(4, LEAF)));
        assert!(big.stack <= STACK_LIMIT && big.slots * 2 > SLOT_LIMIT);
        assert_eq!(lines("a.b", "ab", big, LEAF, 2).len(), 2);
    }
}
