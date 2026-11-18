use std::path::PathBuf;

use swofty_core::ui::node::{NodeExt, ObjExt};
use swofty_formats::erp::Erp;
use swofty_formats::pkg::Pkg;

#[test]
fn the_leaderboard_screen_loses_its_shoulder_glyphs_and_nothing_else() {
    let Some(erp) = std::env::var_os("F1_FLOW_FRONTEND_ERP").map(PathBuf::from) else {
        eprintln!("F1_FLOW_FRONTEND_ERP is not set; skipping");
        return;
    };
    let res = Erp::read_matching(&erp, |n| {
        n == "eaid://ui_package/screens/leaderboard_container.ui"
    })
    .unwrap();
    let stock = res[0].fragments[0].data().unwrap();
    let patched = swofty_core::ui::patch::leaderboard_glyphs(&stock).unwrap();
    let visible = |data: &[u8], name: &str| {
        let mut scr = swofty_core::ui::node::first_screen_of(data).unwrap();
        let n = scr.child_mut(name).unwrap();
        let v = n.props().unwrap().get("visible").cloned();
        format!("{v:?}")
    };
    for glyph in ["tooltip_tab_left", "tooltip_tab_right"] {
        assert_ne!(
            visible(&stock, glyph),
            visible(&patched, glyph),
            "{glyph} is hidden"
        );
    }
    let names = |data: &[u8]| {
        let scr = swofty_core::ui::node::first_screen_of(data).unwrap();
        scr.slot(swofty_core::ui::node::CHILDREN)
            .unwrap()
            .iter()
            .filter(|c| c.is_node())
            .map(|c| c.name().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&stock), names(&patched), "no node is added or removed");
    assert!(
        names(&patched).contains(&"tab_controller".to_owned()),
        "the tab controller stays"
    );
    let input = |data: &[u8]| {
        let mut scr = swofty_core::ui::node::first_screen_of(data).unwrap();
        format!(
            "{:?}",
            scr.child_mut("tab_controller")
                .unwrap()
                .props()
                .unwrap()
                .get("inputAllowed")
                .cloned()
        )
    };
    assert_ne!(
        input(&stock),
        input(&patched),
        "the tab controller takes no input"
    );
    assert!(
        swofty_core::ui::patch::leaderboard_glyphs(&patched).is_ok(),
        "patching twice is harmless"
    );
}

#[test]
fn report_a_built_ui_data() {
    let Some(erp) = std::env::var_os("F1_BUILT_DATABASES_ERP").map(PathBuf::from) else {
        return;
    };
    let res = Erp::read_matching(&erp, |n| n == "eaid://ui_package/databases/ui_data.uidb").unwrap();
    let data = res[0].fragments[0].data().unwrap();
    let mut p = Pkg::parse(&data).unwrap();
    let o = p.root.o_mut().unwrap();
    let n = o
        .at_mut("leaderboard_data")
        .unwrap()
        .o_mut()
        .unwrap()
        .at_mut("Contents")
        .unwrap()
        .o_mut()
        .unwrap()
        .at_mut("table")
        .unwrap()
        .o_mut()
        .unwrap()
        .at_mut("Contents")
        .unwrap()
        .o_mut()
        .unwrap()
        .at_mut("view_type_list")
        .unwrap()
        .o_mut()
        .unwrap()
        .at_mut("Contents")
        .unwrap()
        .a_mut()
        .unwrap()
        .len();
    let swofty = o
        .at_mut("ui_menu_data")
        .unwrap()
        .o_mut()
        .unwrap()
        .at_mut("Contents")
        .unwrap()
        .o_mut()
        .unwrap()
        .get("swofty")
        .is_some();
    eprintln!("BUILT view_type_list entries {n}, ui_menu_data.swofty {swofty}");
}
