//! The stock screens a `hud.element` record may target with `screen`: where the record's elements are
//! hosted, which stock scene instances the host, the template node that controls it, what it masks
//! and which archives carry the result. Every fact here is asserted by `swofty hud check`.
//!
//! The standings, replay and spectator facts are Project Halo's `tower_native.py` measurements
//! (`build_stacked`, `build_replay_scene`, `build_spectator_scene` and their templates); Uses work
//! from Project Halo by nyannoying (https://github.com/nyannoying1337/f1-25-project-halo).

use serde::{Deserialize, Serialize};

use super::model::{Bind, HudElement};
use crate::{Error, Result};

fn err(msg: impl std::fmt::Display) -> Error {
    Error::Other(format!("hud: {msg}"))
}

pub const UI_ERP: &str = "2025_asset_groups/ui_package/";
pub const FLOW_RACE: &str = "2025_asset_groups/ui_package/flow_race.erp";
pub const FLOW_REPLAY: &str = "2025_asset_groups/ui_package/flow_replay.erp";
pub const FLOW_FRONTEND: &str = "2025_asset_groups/ui_package/flow_frontend.erp";
pub const REPLAY_SCREEN_ERP: &str = "2025_asset_groups/ui_package/screens/replays_and_highlights.erp";
pub const SPECTATOR_ERP: &str = "2025_asset_groups/ui_package/screens/hud_spectator_2.erp";

/// The host of every screen record: the bottom cap of the standings board, the one unit scene of the
/// board with a quad mesh of its own.
pub const CAP: &str = "eaid://ui_package/art/osd/units/driver_position_table/driver_position_bottom_cap.idf";
pub const CAP_MESH: &str = "pill_base";
pub const CAP_NULL: &str = "driver_group";
pub const CAP_ANIM: &str = "pill_rotation_anim";
pub const CAP_MESH_SIZE: (f64, f64) = (3.04, 0.12);

pub const STACKED: &str =
    "eaid://ui_package/art/osd/components/driver_position_table/driver_position_table_stacked.idf";
pub const STACKED_UIPRE: &str =
    "eaid://ui_package/art/osd/components/driver_position_table/driver_position_table_stacked.uipre";
pub const STACKED_CAP_REF: &str = "driver_position_bottom_cap";
pub const STACKED_STACKER: &str = "full_stacker";
pub const STACKED_MASK: &str = "mask";
pub const STANDINGS_PARENT: &str = "hud_track_position.osd_visibility_option.track_position";
pub const STANDINGS_CAP_NODE: &str = "hud_track_position.osd_visibility_option.track_position.bottom_cap";

pub const LAPC_SCENE: &str = "eaid://ui_package/art/osd/components/lap_counter/lap_counter.idf";
pub const LAPC_PARENT: &str = "hud_lap_counter.osd_visibility_option";
pub const LAPC_STOCK_KIDS: [&str; 8] = [
    "car_category_logo",
    "edge_base",
    "edge_border",
    "holder_formation",
    "holder_standard",
    "lap_bg",
    "lockup",
    "session_icon",
];
pub const LAPC_HIDE: [&str; 10] = [
    "car_category_logo",
    "edge_base",
    "edge_border",
    "lap_bg",
    "lockup",
    "session_icon",
    "centre_lap_value",
    "separator_bg",
    "separator_bg_top",
    "centre_formation",
];
pub const HUD_SERIES_LOGO: &str = "#view_model_data.hud.content.lap_counter.logo_texture";

pub const REPLAY_UI: &str = "eaid://ui_package/screens/replays_and_highlights.ui";
pub const REPLAY_SCENE: &str = "eaid://ui_package/art/screens/theatre/replay_highlights.idf";
pub const REPLAY_ROOT: &str = "replays_and_highlights";
pub const REPLAY_GROUP: &str = "elements_null";
pub const REPLAY_TABLE: &str = "driver_table";
pub const REPLAY_HEADER: &str = "session_timer";
pub const REPLAY_TABLE_REF: &str = "eaid://ui_package/art/components/theatre/the_driver_position_table.idf";
pub const REPLAY_HEADER_REF: &str = "eaid://ui_package/art/components/theatre/the_spec_session_timer.idf";
pub const REPLAY_POS: [f64; 2] = [-9.2, 5.0];
pub const REPLAY_BOARD_SCALE: f64 = 0.9;
pub const REPLAY_SCALE: f64 = 0.78;
pub const REPLAY_VM: &str = "#replay_flow.replays_and_highlights.";
pub const REPLAY_SHOW: &str = "#replay_flow.replays_and_highlights.show_drivers_list";
pub const REPLAY_STING: &str = "#replay_flow.replays_and_highlights.is_video_sting_enabled";
pub const REPLAY_SERIES_LOGO: &str = "#replay_flow.replays_and_highlights.logo_texture";

pub const SPECT_UI: &str = "eaid://ui_package/screens/hud_spectator_2.ui";
pub const SPECT_SCREEN: &str = "eaid://ui_package/art/osd/screens/spectator_mode_2.idf";
pub const SPECT_TABLE: &str =
    "eaid://ui_package/art/osd/components/spectator_mode/spec_driver_position_table.idf";
pub const SPECT_HEADER: &str = "eaid://ui_package/art/osd/components/spectator_mode/spec_session_timer.idf";
pub const SPECT_ROOT: &str = "hud_spectator_2";
pub const SPECT_PARENT: &str = "hud_spectator_2.content";
pub const SPECT_PREFIX: &str = "spectator_mode_2";
pub const SPECT_TABLE_EL: &str = "driver_position_table";
pub const SPECT_HEADER_EL: &str = "session_timer";
pub const SPECT_GROUP: &str = "transition";
pub const SPECT_CAP_SRC: &str = "bottom_cap";
pub const SPECT_SHOW: &str = "show_left_side";
pub const SPECT_SERIES_LOGO: &str = "logo_texture";
pub const SCENE_ROOT: &str = "Scene Root";

/// Where the instance of the host goes in the instance scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum At {
    /// Scene units.
    Point([f64; 2]),
    /// The position of another element of the instance scene.
    Of(String),
    /// `a`'s position minus `b`'s, both read from another stock scene (the spectator screen's).
    Offset { scene: String, a: String, b: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Instance {
    #[serde(rename = "in")]
    pub scene: String,
    pub clone: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub under: Option<String>,
    pub at: At,
    #[serde(default = "one")]
    pub scale: f64,
    #[serde(default)]
    pub retarget: bool,
}

fn one() -> f64 {
    1.0
}

/// A property on an element inside the instance that follows a value of the screen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenSwitch {
    pub layout: Vec<String>,
    pub db: String,
    pub negate: bool,
    pub default: bool,
}

/// A stock screen, as the record's `screen` names one or spells one out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenFacts {
    pub key: String,
    pub template: String,
    pub parent_ui: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire: Option<(String, String)>,
    pub host: Host,
    pub instance: Instance,
    #[serde(default)]
    pub masks: Vec<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_switch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_hide: Option<ScreenSwitch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_logo: Option<String>,
    pub erps: Erps,
    pub stock_erp: String,
    pub template_erp: String,
    pub scene_file: String,
    pub template_file: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Host {
    pub scene: String,
    pub mesh: String,
    pub null: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Erps {
    pub scene: Vec<String>,
    pub template: Vec<String>,
}

fn s(v: &str) -> String {
    v.to_owned()
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

fn cap_host() -> Host {
    Host {
        scene: s(CAP),
        mesh: s(CAP_MESH),
        null: s(CAP_NULL),
    }
}

pub fn standings() -> ScreenFacts {
    ScreenFacts {
        key: s("standings"),
        template: s(super::dials::TEMPLATE),
        parent_ui: s(STANDINGS_PARENT),
        wire: Some((s("enable"), s("disable"))),
        host: cap_host(),
        instance: Instance {
            scene: s(STACKED),
            clone: s(STACKED_CAP_REF),
            under: None,
            at: At::Point([0.0, 0.0]),
            scale: 1.0,
            retarget: false,
        },
        masks: vec![vec![s(STACKED_MASK)]],
        root_switch: None,
        extra_hide: None,
        series_logo: Some(s(HUD_SERIES_LOGO)),
        erps: Erps {
            scene: strs(&super::dials::SCENE_ERPS),
            template: strs(&super::dials::TEMPLATE_ERPS),
        },
        stock_erp: s(FLOW_RACE),
        template_erp: s(FLOW_RACE),
        scene_file: s("hud_scene.erp"),
        template_file: s("hud_template.erp"),
    }
}

pub fn replay() -> ScreenFacts {
    ScreenFacts {
        key: s("replay"),
        template: s(REPLAY_UI),
        parent_ui: s(REPLAY_ROOT),
        wire: None,
        host: cap_host(),
        instance: Instance {
            scene: s(REPLAY_SCENE),
            clone: s(REPLAY_TABLE),
            under: Some(s(REPLAY_GROUP)),
            at: At::Of(s(REPLAY_HEADER)),
            scale: REPLAY_SCALE,
            retarget: true,
        },
        masks: vec![
            vec![s(REPLAY_TABLE), s(SCENE_ROOT)],
            vec![s(REPLAY_HEADER), s(SCENE_ROOT)],
        ],
        root_switch: Some(s(REPLAY_SHOW)),
        extra_hide: Some(ScreenSwitch {
            layout: vec![s(SCENE_ROOT)],
            db: s(REPLAY_STING),
            negate: true,
            default: true,
        }),
        series_logo: Some(s(REPLAY_SERIES_LOGO)),
        erps: Erps {
            scene: strs(&[FLOW_FRONTEND]),
            template: strs(&[FLOW_REPLAY, REPLAY_SCREEN_ERP]),
        },
        stock_erp: s(FLOW_REPLAY),
        template_erp: s(FLOW_REPLAY),
        scene_file: s("hud_replay_scene.erp"),
        template_file: s("hud_replay.erp"),
    }
}

pub fn spectator() -> ScreenFacts {
    ScreenFacts {
        key: s("spectator"),
        template: s(SPECT_UI),
        parent_ui: s(SPECT_PARENT),
        wire: None,
        host: cap_host(),
        instance: Instance {
            scene: s(SPECT_TABLE),
            clone: s(SPECT_CAP_SRC),
            under: None,
            at: At::Offset {
                scene: s(SPECT_SCREEN),
                a: s(SPECT_HEADER_EL),
                b: s(SPECT_TABLE_EL),
            },
            scale: REPLAY_SCALE,
            retarget: true,
        },
        masks: vec![
            vec![s(SPECT_PREFIX), s(SPECT_TABLE_EL), s(SPECT_GROUP)],
            vec![s(SPECT_PREFIX), s(SPECT_HEADER_EL), s(SCENE_ROOT)],
        ],
        root_switch: Some(s(SPECT_SHOW)),
        extra_hide: None,
        series_logo: Some(s(SPECT_SERIES_LOGO)),
        erps: Erps {
            scene: strs(&super::dials::SPECTATOR_ERPS),
            template: strs(&super::dials::SPECTATOR_ERPS),
        },
        stock_erp: s(FLOW_RACE),
        template_erp: s(FLOW_RACE),
        scene_file: s("hud_spectator.erp"),
        template_file: s("hud_spectator.erp"),
    }
}

pub const PRESETS: [&str; 3] = ["standings", "replay", "spectator"];

pub fn preset(key: &str) -> Option<ScreenFacts> {
    match key {
        "standings" => Some(standings()),
        "replay" => Some(replay()),
        "spectator" => Some(spectator()),
        _ => None,
    }
}

impl ScreenFacts {
    /// The facts of a `screen` value: a preset's name or the raw form.
    pub fn parse(v: &serde_json::Value) -> Result<Self> {
        match v {
            serde_json::Value::String(k) => preset(k).ok_or_else(|| {
                err(format!(
                    "screen {k:?} is none of {}; or spell the screen out as an object",
                    PRESETS.join(", ")
                ))
            }),
            serde_json::Value::Object(_) => {
                let f: ScreenFacts =
                    serde_json::from_value(v.clone()).map_err(|e| err(format!("screen: {e}")))?;
                f.check()?;
                Ok(f)
            }
            _ => Err(err("screen is a preset name or an object")),
        }
    }

    pub fn check(&self) -> Result<()> {
        let idf = |p: &str| p.starts_with("eaid://") && p.ends_with(".idf");
        if !self.template.starts_with("eaid://") || !self.template.ends_with(".ui") {
            return Err(err(format!("screen {}: template is an eaid://….ui", self.key)));
        }
        if !idf(&self.host.scene) || !idf(&self.instance.scene) {
            return Err(err(format!("screen {}: scenes are eaid://….idf paths", self.key)));
        }
        if self.parent_ui.split('.').any(str::is_empty) {
            return Err(err(format!(
                "screen {}: parent_ui is a dotted template path",
                self.key
            )));
        }
        if self.masks.iter().any(|m| m.is_empty()) {
            return Err(err(format!(
                "screen {}: a mask names at least one element",
                self.key
            )));
        }
        if self.erps.scene.is_empty() || self.erps.template.is_empty() {
            return Err(err(format!(
                "screen {}: erps.scene and erps.template name archives",
                self.key
            )));
        }
        if self.instance.scale <= 0.0 || self.instance.scale.is_nan() {
            return Err(err(format!(
                "screen {}: the instance scale is positive",
                self.key
            )));
        }
        Ok(())
    }

    /// The host instance's element name for a record.
    pub fn host_name(&self, pfx: &str) -> String {
        format!("{pfx}_host")
    }
}

/// One screen a record shows its elements on, with the leaf that shows it there.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub facts: ScreenFacts,
    pub show: Bind,
    /// Element ids hidden for good on this screen.
    pub hide: Vec<String>,
}

/// A record's `screen`, `show` and `also` fields as the screens it shows its elements on.
pub fn targets(rec: &HudElement) -> Result<Option<Vec<Target>>> {
    let Some(screen) = &rec.screen else {
        if rec.show.is_some() || !rec.also.is_empty() {
            return Err(err("show and also go with screen"));
        }
        return Ok(None);
    };
    if rec.scene.is_some() {
        return Err(err("a record has a screen or a scene target, not both"));
    }
    let show_of = |b: Option<&Bind>, what: &str| -> Result<Bind> {
        let b = b.ok_or_else(|| {
            err(format!(
                "{what}: a screen record needs show, the leaf that shows it"
            ))
        })?;
        b.check()?;
        if b.view_model.is_none() {
            return Err(err(format!("{what}: show is a view_model leaf")));
        }
        Ok(b.clone())
    };
    let mut out = vec![Target {
        facts: ScreenFacts::parse(screen)?,
        show: show_of(rec.show.as_ref(), "screen")?,
        hide: Vec::new(),
    }];
    for (k, v) in &rec.also {
        let facts = ScreenFacts::parse(&serde_json::Value::String(k.clone()))?;
        let show = show_of(v.show.as_ref(), &format!("also.{k}"))?;
        if out.iter().any(|t| t.facts.key == facts.key) {
            return Err(err(format!("also.{k}: the record already targets {k}")));
        }
        out.push(Target {
            facts,
            show,
            hide: v.hide.clone(),
        });
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_presets_have_the_measured_shape() {
        let st = standings();
        assert_eq!(st.parent_ui, STANDINGS_PARENT);
        assert_eq!(st.wire.as_ref().map(|w| w.0.as_str()), Some("enable"));
        assert_eq!(st.masks, vec![vec!["mask".to_owned()]]);
        assert_eq!(st.instance.at, At::Point([0.0, 0.0]));
        let rp = replay();
        assert_eq!(rp.masks[0], ["driver_table", "Scene Root"]);
        assert_eq!(rp.instance.scale, REPLAY_SCALE);
        assert!(rp.instance.retarget);
        assert_eq!(rp.erps.template, [FLOW_REPLAY, REPLAY_SCREEN_ERP]);
        assert_eq!(rp.erps.scene, [FLOW_FRONTEND]);
        let sp = spectator();
        assert_eq!(
            sp.masks[0],
            ["spectator_mode_2", "driver_position_table", "transition"]
        );
        assert_eq!(sp.root_switch.as_deref(), Some("show_left_side"));
        assert_eq!(sp.scene_file, sp.template_file);
        for k in PRESETS {
            let f = preset(k).unwrap();
            assert_eq!(f.key, k);
            f.check().unwrap();
            let back: ScreenFacts = serde_json::from_value(serde_json::to_value(&f).unwrap()).unwrap();
            assert_eq!(back, f);
        }
    }

    #[test]
    fn a_record_names_its_screens_with_also() {
        let v = serde_json::json!({
            "screen": "standings",
            "show": { "view_model": "view_model_data.hud_tt_show" },
            "also": { "replay": { "show": { "view_model": "view_model_data.hud_tt_on" } },
                      "spectator": { "show": { "view_model": "view_model_data.hud_tt_on" } } },
            "sprites": {}, "elements": []
        });
        let rec = HudElement::parse(&v).unwrap();
        let t = targets(&rec).unwrap().unwrap();
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].facts.key, "standings");
        assert_eq!(t[1].show.view_model.as_deref(), Some("view_model_data.hud_tt_on"));
        let mut bad = v.clone();
        bad["also"] = serde_json::json!({ "standings": { "show": { "view_model": "x" } } });
        assert!(targets(&HudElement::parse(&bad).unwrap()).is_err());
        let mut bad = v.clone();
        bad.as_object_mut().unwrap().remove("show");
        assert!(targets(&HudElement::parse(&bad).unwrap()).is_err());
        let mut raw = v.clone();
        raw.as_object_mut().unwrap().remove("also");
        raw["screen"] = serde_json::to_value(replay()).unwrap();
        assert_eq!(
            targets(&HudElement::parse(&raw).unwrap()).unwrap().unwrap()[0].facts,
            replay()
        );
        assert!(ScreenFacts::parse(&serde_json::json!("nope")).is_err());
    }
}
