//! The stock dials the composer grows: scenes, quads, template parents, parts and the typed table
//! of the game's own values. Every fact here is asserted against the game files by
//! [`super::check`] (`swofty hud check`), so a game update that moves one fails the build.

use super::model::Dial;

pub const UI: &str = "2025_asset_groups/ui_package/";
pub const SCENE_ERPS: [&str; 4] = [
    "2025_asset_groups/ui_package/flow_race.erp",
    "2025_asset_groups/ui_package/flow_options.erp",
    "2025_asset_groups/ui_package/persistent.erp",
    "2025_asset_groups/ui_package/screens/hud_spectator_2.erp",
];
pub const TEMPLATE_ERPS: [&str; 2] = [
    "2025_asset_groups/ui_package/flow_race.erp",
    "2025_asset_groups/ui_package/flow_options.erp",
];
pub const SPECTATOR_ERPS: [&str; 2] = [
    "2025_asset_groups/ui_package/flow_race.erp",
    "2025_asset_groups/ui_package/screens/hud_spectator_2.erp",
];
/// The archive every stock resource is read from.
pub const STOCK_ERP: &str = "2025_asset_groups/ui_package/flow_race.erp";

pub const TEMPLATE: &str = "eaid://ui_package/templates/hud_dynamic_templates.ui";
pub const SPECTATOR: &str = "eaid://ui_package/screens/hud_spectator_2.ui";
const DIAL_DIR: &str = "eaid://ui_package/art/osd/components/dial/";
pub const MODS: &str = "eaid://ui_package/art/osd/components/dial/dial_mods.idf";
pub const BAR: &str = "eaid://ui_package/art/osd/components/dial/dial_om_bar_complete.idf";
pub const FILL_MATERIAL: &str =
    "eaid://ui_package/art/osd/materials/dial/dial_frame/dial_central_bg_fill.material";
pub const CLIP_TEMPLATE: &str =
    "eaid://ui_package/art/osd/components/dial/dial_mods.idf/mod_1_icon_fill_mask_anim?context=animation";
pub const CUSTOMISATION: &str = "boundless_customisation";
pub const STOCK_KIDS: [&str; 4] = ["frame", "fuel", "holder_central", "pit_indicator"];
pub const TEMPLATE_UIPRE_KEYS: (u32, u32) = (4_045_306_521, 839_438_211);

#[derive(Debug, Clone, Copy)]
pub struct DialFacts {
    pub dial: Dial,
    pub scene: &'static str,
    pub strtab: &'static str,
    pub uipre: &'static str,
    pub quad: &'static str,
    pub race_parent: &'static str,
    pub spectator_parent: &'static str,
    pub bottom_y: f64,
    pub quad_size: Option<(f64, f64)>,
}

macro_rules! scene {
    ($n:literal) => {
        concat!("eaid://ui_package/art/osd/components/dial/", $n, ".idf")
    };
}

pub const F126: DialFacts = DialFacts {
    dial: Dial::F126,
    scene: scene!("dial_om_xm"),
    strtab: concat!(scene!("dial_om_xm"), "?context=animstringtable&file=0"),
    uipre: "eaid://ui_package/art/osd/components/dial/dial_om_xm.uipre",
    quad: "central_bg_fill",
    race_parent: "hud_f1_dial_om_xm.osd_visibility_option.f1_om_xm_dial_child",
    spectator_parent: "hud_spectator_2.content.bottom_right.f1_26_dial",
    bottom_y: -0.28837,
    quad_size: Some((4.48, 1.92)),
};

pub const F125: DialFacts = DialFacts {
    dial: Dial::F125,
    scene: scene!("dial_complete"),
    strtab: concat!(scene!("dial_complete"), "?context=animstringtable&file=0"),
    uipre: "eaid://ui_package/art/osd/components/dial/dial_complete.uipre",
    quad: "central_bg_fill",
    race_parent: "hud_f1_dial.osd_visibility_option.f1_dial_child",
    spectator_parent: "hud_spectator_2.content.bottom_right.modern_dial",
    bottom_y: -0.28837,
    quad_size: Some((4.48, 1.92)),
};

pub const F2: DialFacts = DialFacts {
    dial: Dial::F2,
    scene: scene!("dial_f2_complete"),
    strtab: concat!(scene!("dial_f2_complete"), "?context=animstringtable&file=0"),
    uipre: "eaid://ui_package/art/osd/components/dial/dial_f2_complete.uipre",
    quad: "central_bg_edge",
    race_parent: "hud_f2_dial.osd_visibility_option.f2_dial_child",
    spectator_parent: "hud_spectator_2.content.bottom_right.f2_dial",
    bottom_y: -0.28837,
    quad_size: None,
};

/// The facts of a record's scene target: no dial parts, quad or strings, only the scene and the
/// template node that instances it. One leaked copy per distinct target for the life of the process.
pub fn target_facts(scene: &str, parent: &str) -> &'static DialFacts {
    use std::collections::BTreeMap;
    use std::sync::{Mutex, OnceLock};
    static KNOWN: OnceLock<Mutex<BTreeMap<(String, String), &'static DialFacts>>> = OnceLock::new();
    let mut known = KNOWN
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    known
        .entry((scene.to_owned(), parent.to_owned()))
        .or_insert_with(|| {
            let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
            Box::leak(Box::new(DialFacts {
                dial: Dial::F126,
                scene: leak(scene.to_owned()),
                strtab: leak(format!("{scene}?context=animstringtable&file=0")),
                uipre: leak(scene.replace(".idf", ".uipre")),
                quad: "central_bg_fill",
                race_parent: leak(parent.to_owned()),
                spectator_parent: "",
                bottom_y: 0.0,
                quad_size: None,
            }))
        })
}

pub fn facts(dial: Dial) -> &'static DialFacts {
    match dial {
        Dial::F126 => &F126,
        Dial::F125 => &F125,
        Dial::F2 => &F2,
    }
}

pub fn dial_dir() -> &'static str {
    DIAL_DIR
}

/// A value tag the stock template binds.
pub type Tag = [u8; 4];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Use {
    Property,
    DecisionOnly,
}

#[derive(Debug, Clone, Copy)]
pub struct GameValue {
    pub name: &'static str,
    pub db: &'static str,
    pub tag: Tag,
    pub usage: Use,
    /// False when no stock template node binds it, so the build can't confirm the tag against the
    /// template and trusts this table.
    pub in_stock: bool,
}

const fn v(name: &'static str, db: &'static str, tag: Tag, usage: Use, in_stock: bool) -> GameValue {
    GameValue {
        name,
        db,
        tag,
        usage,
        in_stock,
    }
}

/// The resources the composer writes whole: another addon's edit of one of them reaches the game only
/// through the composer's base.
pub fn written() -> Vec<&'static str> {
    let mut out = vec![TEMPLATE, SPECTATOR];
    for d in Dial::ALL {
        let f = facts(d);
        out.extend([f.scene, f.strtab, f.uipre]);
    }
    out
}

/// The game's own values an element may bind, by the name records use. A property's tag must be
/// the tag of the value it binds: the game takes it on trust, and a `stri` bound to the ui32
/// `#session.current_car.position` crashed the game while creating the HUD.
pub const GAME_VALUES: &[GameValue] = &[
    v("speed_str", "speed_str", *b"stri", Use::Property, true),
    v("revs", "revs", *b"fp32", Use::Property, true),
    v("ers_stored", "ers_stored", *b"fp32", Use::Property, true),
    v("om_active", "om_active", *b"bool", Use::Property, false),
    v("is_xm_active", "is_xm_active", *b"bool", Use::Property, true),
    v("has_ers", "has_ers", *b"bool", Use::Property, true),
    v("ers_mode", "ers_mode", *b"ui32", Use::DecisionOnly, true),
    v(
        "drs_pit_state",
        "drs_pit_state",
        *b"stri",
        Use::DecisionOnly,
        true,
    ),
];

pub const SESSION_VALUES: &[GameValue] = &[
    v(
        "current_car.position",
        "#session.current_car.position",
        *b"ui32",
        Use::Property,
        true,
    ),
    v(
        "current_car.driver.second_name",
        "#session.current_car.driver.second_name",
        *b"stri",
        Use::Property,
        false,
    ),
    v(
        "current_car.nub_colour",
        "#session.current_car.nub_colour",
        *b"rgba",
        Use::Property,
        true,
    ),
    v(
        "number_of_laps",
        "#session.number_of_laps",
        *b"si32",
        Use::Property,
        true,
    ),
    v(CLOCK_STR, CLOCK_STR, *b"stri", Use::Property, true),
];

pub const SESSION_CAR_VALUES: &[GameValue] = &[
    v(
        "position",
        "#session.cars.car{vid}.position",
        *b"ui32",
        Use::Property,
        true,
    ),
    v(
        "driver.second_name",
        "#session.cars.car{vid}.driver.second_name",
        *b"stri",
        Use::Property,
        false,
    ),
    v(
        "driver.name_code",
        "#session.cars.car{vid}.driver.name_code",
        *b"stri",
        Use::Property,
        false,
    ),
    v(
        "nub_colour",
        "#session.cars.car{vid}.nub_colour",
        *b"rgba",
        Use::Property,
        true,
    ),
];

/// The session clock: a text bound to `time_remaining_str` reads the stock session timer's
/// formatter, a UITF under this template that the composer clones into the record's group.
pub const CLOCK_MODEL: &str = "hud_session_timer_dynamic";
pub const CLOCK_FORMATTER: &str = "time_remaining_formatter";
pub const CLOCK_STR: &str = "time_remaining_str";

pub const TEAM_COLOUR: &str = "#session.current_car.nub_colour";

pub fn game_value(name: &str) -> Option<&'static GameValue> {
    GAME_VALUES.iter().find(|g| g.name == name)
}

pub fn session_value(path: &str) -> Option<&'static GameValue> {
    let path = path.trim_start_matches("#session.");
    SESSION_VALUES.iter().find(|g| g.name == path)
}

pub fn session_car_value(field: &str) -> Option<&'static GameValue> {
    SESSION_CAR_VALUES.iter().find(|g| g.name == field)
}

/// The text of a `Tag` for messages.
pub fn tag_name(t: &Tag) -> String {
    String::from_utf8_lossy(t).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dial_has_its_facts() {
        for d in Dial::ALL {
            let f = facts(d);
            assert_eq!(f.dial, d);
            assert!(f.scene.ends_with(".idf"));
            assert!(f.strtab.starts_with(f.scene));
            assert!(f.race_parent.split('.').count() >= 3);
        }
        assert_eq!(F2.quad, "central_bg_edge");
    }

    #[test]
    fn the_value_table_has_typed_names() {
        assert_eq!(game_value("revs").unwrap().tag, *b"fp32");
        assert_eq!(
            session_value("#session.current_car.position").unwrap().tag,
            *b"ui32"
        );
        assert_eq!(session_car_value("driver.second_name").unwrap().tag, *b"stri");
        assert_eq!(session_car_value("driver.name_code").unwrap().tag, *b"stri");
        assert_eq!(session_car_value("nub_colour").unwrap().tag, *b"rgba");
        assert_eq!(session_value("number_of_laps").unwrap().tag, *b"si32");
        assert_eq!(session_value(CLOCK_STR).unwrap().db, "time_remaining_str");
        assert!(game_value("nope").is_none());
        assert_eq!(game_value("ers_mode").unwrap().usage, Use::DecisionOnly);
    }
}
