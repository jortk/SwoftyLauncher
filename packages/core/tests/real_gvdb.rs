use std::collections::BTreeMap;
use std::path::PathBuf;

use swofty_core::heads;
use swofty_core::livery_kits;
use swofty_core::manifest::AddonManifest;
use swofty_core::ops::{DbOps, apply_db_ops};
use swofty_core::owned;
use swofty_core::profile::Profile;
use swofty_core::settings::{OnAddons, render_ops};
use swofty_formats::gvdb::Gvdb;

#[test]
fn template_db_ops_resolve_against_the_real_database() {
    let Some(dir) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let Ok(data) = std::fs::read(dir.join("game_package/gamedatabases/gamedatabases.2025.bin")) else {
        eprintln!("no gamedatabases.2025.bin under F1_25_DIR; skipping");
        return;
    };
    let mut db = Gvdb::parse(&data).unwrap();
    let template = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../templates/addon");
    let m = AddonManifest::load(&template).unwrap();
    let mut doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(template.join("db/example.json")).unwrap()).unwrap();
    render_ops(&mut doc, &m, &Profile::new("test"), &OnAddons::default()).unwrap();
    let ops: DbOps = serde_json::from_value(doc).unwrap();
    apply_db_ops(&mut db, &ops, &BTreeMap::<String, Vec<String>>::new()).unwrap();
    assert_eq!(
        db.get_text("rules.stop_go_penalty.penalty_duration").as_deref(),
        Some("5")
    );
    assert_eq!(db.records.len(), Gvdb::parse(&data).unwrap().records.len());
}

#[test]
fn added_se_livery_unlock_is_owned_against_the_real_database() {
    let Some(dir) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let Ok(data) = std::fs::read(dir.join("game_package/gamedatabases/gamedatabases.2025.bin")) else {
        eprintln!("no gamedatabases.2025.bin under F1_25_DIR; skipping");
        return;
    };
    let pristine = Gvdb::parse(&data).unwrap();
    let ops: DbOps = serde_json::from_str(
        r#"{"schema_version":1,
        "fields":{
          "customisation.custom_se_liveries.se_liveries.se_livery":["_metadata_","id","compatible_vehicle","wep_override","team_customisation","name_loc_tag","desc_loc_tag","ui_priority"],
          "unlockable_content.unlockable_items.unlockable_item_list_2.folder_23.unlockable_se_livery":["_metadata_","customisation_item","is_new_on_boot","rarity","statId","type_loctag"]
        },
        "ops":[
          {"op":"add-row","table":"customisation.custom_se_liveries.se_liveries",
           "from":"customisation.custom_se_liveries.se_liveries.se_livery[_metadata_=toro_rosso_miami_2026]",
           "set":{"_metadata_":"toro_rosso_rm_alphatauri_2026","id":"22","wep_override":"_barcelona_26",
                  "name_loc_tag":"lng_team_tor_hashtag","desc_loc_tag":"lng_team_tor_hashtag","ui_priority":"22"}},
          {"op":"add-row","table":"unlockable_content.unlockable_items.unlockable_item_list_2.folder_23",
           "from":"unlockable_content.unlockable_items.unlockable_item_list_2.folder_23.unlockable_se_livery[_metadata_=toro_rosso_miami_26]",
           "set":{"_metadata_":"toro_rosso_rm_alphatauri_2026","customisation_item":"toro_rosso_rm_alphatauri_2026","statId":"5280"}}
        ]}"#,
    )
    .unwrap();
    let mut db = pristine.clone();
    let before = owned::counts(&db);
    apply_db_ops(&mut db, &ops, &BTreeMap::<String, Vec<String>>::new()).unwrap();
    let added = owned::added(&db, "rmdesigns.alphatauri-orlen-livery", &before);
    let found = owned::owned_items(&db, &pristine, &added);
    assert_eq!(
        found.items,
        vec![owned::OwnedItem {
            key: "0xf01d03c7".into(),
            addon: "rmdesigns.alphatauri-orlen-livery".into(),
            what: "se_livery 22".into(),
        }]
    );
    let stock = std::collections::HashSet::from([owned::item_key("eSELivery", "toro_rosso_miami_26")]);
    assert_eq!(owned::stock_collisions(&pristine, &stock), stock);
}

#[test]
fn a_special_livery_of_an_unknown_team_kit_is_emptied_against_the_real_database() {
    let Some(dir) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let Ok(data) = std::fs::read(dir.join("game_package/gamedatabases/gamedatabases.2025.bin")) else {
        eprintln!("no gamedatabases.2025.bin under F1_25_DIR; skipping");
        return;
    };
    let mut db = Gvdb::parse(&data).unwrap();
    assert!(livery_kits::kits(&db).contains("2026-renault"));
    assert_eq!(livery_kits::keep_kits(&mut db), []);
    assert_eq!(db.to_bytes(), data);
    let ops: DbOps = serde_json::from_str(
        r#"{"schema_version":1,
        "fields":{
          "customisation.custom_se_liveries.se_liveries.se_livery":["_metadata_","id","compatible_vehicle","wep_override","team_customisation","name_loc_tag","desc_loc_tag","ui_priority"]
        },
        "ops":[
          {"op":"add-row","table":"customisation.custom_se_liveries.se_liveries",
           "from":"customisation.custom_se_liveries.se_liveries.se_livery[_metadata_=mclaren_monaco_2026]",
           "set":{"_metadata_":"lotus_lx02","id":"22","team_customisation":"2026-lotus","ui_priority":"22"}},
          {"op":"add-row","table":"customisation.custom_se_liveries.se_liveries",
           "from":"customisation.custom_se_liveries.se_liveries.se_livery[_metadata_=mclaren_monaco_2026]",
           "set":{"_metadata_":"lotus_lx09","id":"23","team_customisation":"2026-renault","ui_priority":"23"}}
        ]}"#,
    )
    .unwrap();
    apply_db_ops(&mut db, &ops, &BTreeMap::<String, Vec<String>>::new()).unwrap();
    let rows = "customisation.custom_se_liveries.se_liveries";
    let found = livery_kits::keep_kits(&mut db);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(
        (found[0].row.as_str(), found[0].kit.as_str()),
        ("lotus_lx02", "2026-lotus")
    );
    assert_eq!(
        db.get_text(&format!("{rows}.se_livery_21.team_customisation"))
            .as_deref(),
        Some("")
    );
    assert_eq!(
        db.get_text(&format!("{rows}.se_livery_22.team_customisation"))
            .as_deref(),
        Some("2026-renault")
    );
    assert_eq!(
        db.get_text(&format!("{rows}.se_livery_13.team_customisation"))
            .as_deref(),
        Some("2026-mclaren")
    );
}

#[test]
fn the_games_headless_driver_wears_the_stand_in_against_the_real_database() {
    let Some(dir) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let Ok(data) = std::fs::read(dir.join("game_package/gamedatabases/gamedatabases.2025.bin")) else {
        eprintln!("no gamedatabases.2025.bin under F1_25_DIR; skipping");
        return;
    };
    let shipped = Gvdb::parse(&data).unwrap();
    let mut db = shipped.clone();
    let found = heads::wear_stand_ins(&mut db, &shipped, &std::collections::BTreeSet::new());
    assert_eq!(
        found,
        [heads::Headless {
            row: "nico_rosberg".into(),
            head: "nico_rosberg".into(),
            worn: heads::STAND_IN.into(),
        }]
    );
    let rosberg = (0..db.instance_count("entrants.drivers", "driver").unwrap())
        .find(|i| {
            db.get_text(&format!("entrants.drivers.driver_{i}._metadata_"))
                .as_deref()
                == Some("nico_rosberg")
        })
        .unwrap();
    assert_eq!(
        db.get_text(&format!("entrants.drivers.driver_{rosberg}.head"))
            .as_deref(),
        Some(heads::STAND_IN)
    );
    assert!(heads::labels(&db).contains(heads::STAND_IN));
    assert_eq!(db.records.len(), shipped.records.len());
}

#[test]
fn nested_selectors_resolve_against_the_real_database() {
    let Some(dir) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let Ok(data) = std::fs::read(dir.join("game_package/gamedatabases/gamedatabases.2025.bin")) else {
        eprintln!("no gamedatabases.2025.bin under F1_25_DIR; skipping");
        return;
    };
    let mut db = Gvdb::parse(&data).unwrap();
    let sets = "track_data.game_setup";
    assert_eq!(
        db.get_text(&format!("{sets}.TrackDataSet_16._metadata_"))
            .as_deref(),
        Some("f1_2026")
    );
    let ops: DbOps = serde_json::from_str(
        r#"{"schema_version":1,
        "fields":{"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData":["_metadata_","track"]},
        "ops":[
          {"op":"set","select":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026]","field":"_metadata_","value":"f1_2026"},
          {"op":"set","select":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=suzuka]",
           "field":"TrackNameData_0.title","value":"lng_nested"},
          {"op":"add-row","table":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026]",
           "from":"track_data.game_setup.TrackDataSet[_metadata_=f1_2026].TrackData[_metadata_=melbourne]",
           "set":{"_metadata_":"nested_test","track":"nested_test"}}
        ]}"#,
    )
    .unwrap();
    let before = db
        .instance_count(&format!("{sets}.TrackDataSet_16"), "TrackData")
        .unwrap();
    let replaced = apply_db_ops(&mut db, &ops, &BTreeMap::<String, Vec<String>>::new()).unwrap();
    assert_eq!(
        replaced[0],
        swofty_formats::hash::fnv1a64(format!("{sets}.TrackDataSet_16._metadata_"))
    );
    let suzuka = (0..before)
        .find(|j| {
            db.get_text(&format!("{sets}.TrackDataSet_16.TrackData_{j}._metadata_"))
                .as_deref()
                == Some("suzuka")
        })
        .unwrap();
    assert_eq!(
        db.get_text(&format!(
            "{sets}.TrackDataSet_16.TrackData_{suzuka}.TrackNameData_0.title"
        ))
        .as_deref(),
        Some("lng_nested")
    );
    assert_eq!(
        db.instance_count(&format!("{sets}.TrackDataSet_16"), "TrackData"),
        Some(before + 1)
    );
    assert_eq!(
        db.get_text(&format!("{sets}.TrackDataSet_16.TrackData_{before}.track"))
            .as_deref(),
        Some("nested_test")
    );
}

#[test]
fn core_db_ops_add_the_swofty_f1_modes_to_the_real_database() {
    use swofty_core::asset::AssetOp;
    use swofty_core::builtin::{CORE_MODE, core_db_ops};
    use swofty_core::ops::{DbReport, apply_db_ops_report};
    use swofty_core::rowkeys::{Names, RowKeys};

    let Some(dir) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let Ok(data) = std::fs::read(dir.join("game_package/gamedatabases/gamedatabases.2025.bin")) else {
        eprintln!("no gamedatabases.2025.bin under F1_25_DIR; skipping");
        return;
    };
    let pristine = Gvdb::parse(&data).unwrap();
    let mut db = pristine.clone();
    let file = core_db_ops();
    let ops = DbOps {
        schema_version: file.schema_version,
        fields: file.fields,
        strict: true,
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
    let none = BTreeMap::new();
    let stock = pristine.records.iter().map(|r| r.key).collect();
    let names = Names {
        fields: &none,
        keys: RowKeys::builtin(),
        stock: Some(&stock),
        tracks: None,
    };
    let mut report = DbReport::default();
    apply_db_ops_report(&mut db, &ops, &names, &mut report).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);

    let modes = "game_setup.game_setup_modes";
    let count = |db: &Gvdb, t: &str, row: &str| -> usize {
        db.get_text(&format!("{t}._{row}_instancecount_"))
            .unwrap()
            .parse()
            .unwrap()
    };
    let n = count(&db, modes, "game_setup_mode");
    assert_eq!(n, count(&pristine, modes, "game_setup_mode") + 1);
    let row = format!("{modes}.game_setup_mode_{}", n - 1);
    let text = |k: &str| db.get_text(&format!("{row}.{k}"));
    assert_eq!(text("_metadata_").as_deref(), Some(CORE_MODE));
    assert_eq!(text("stat_id").as_deref(), Some("20"));
    assert_eq!(text("entrant_setup_modes").as_deref(), Some(CORE_MODE));
    let mut ids: Vec<String> = (0..n)
        .map(|i| {
            db.get_text(&format!("{modes}.game_setup_mode_{i}.stat_id"))
                .unwrap()
        })
        .collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n, "game setup mode stat ids collide");

    let setups = "entrants.entrant_setup_modes";
    let n = count(&db, setups, "entrant_setup_mode");
    assert_eq!(n, count(&pristine, setups, "entrant_setup_mode") + 1);
    let row = format!("{setups}.entrant_setup_mode_{}", n - 1);
    assert_eq!(
        db.get_text(&format!("{row}._metadata_")).as_deref(),
        Some(CORE_MODE)
    );
    for k in ["entrant_lists", "selectable_entrant_lists"] {
        assert_eq!(db.get_text(&format!("{row}.{k}")).as_deref(), Some(""), "{k}");
    }
    let mut ids: Vec<String> = (0..n)
        .map(|i| {
            db.get_text(&format!("{setups}.entrant_setup_mode_{i}.stat_id"))
                .unwrap()
        })
        .collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), n, "entrant setup mode stat ids collide");

    let after: std::collections::HashMap<u64, &[u8]> =
        db.records.iter().map(|r| (r.key, r.value.as_slice())).collect();
    let appended = pristine
        .records
        .iter()
        .filter(|r| {
            let now = after[&r.key];
            now != r.value
                && String::from_utf8_lossy(now)
                    == format!("{},{CORE_MODE}", String::from_utf8_lossy(&r.value))
        })
        .count();
    assert_eq!(appended, 50);

    let championships = count(&db, "championships", "Championship");
    for i in 0..championships {
        let row = format!("championships.Championship_{i}");
        let modes = db
            .get_text(&format!("{row}.game_setup_modes"))
            .unwrap_or_default();
        assert_eq!(
            modes.split(',').count(),
            1,
            "championship {row} lists more than one setup mode: {modes}"
        );
    }
    assert_eq!(championships, 21);

    let addon: DbOps = serde_json::from_str(&format!(
        r#"{{"schema_version":1,"ops":[
          {{"op":"list-append","select":"{setups}.entrant_setup_mode[_metadata_={CORE_MODE}]","field":"entrant_lists","value":"f1_2025"}},
          {{"op":"list-append","select":"{setups}.entrant_setup_mode[_metadata_={CORE_MODE}]","field":"selectable_entrant_lists","value":"f1_2025"}}
        ]}}"#
    ))
    .unwrap();
    apply_db_ops(&mut db, &addon, &none).unwrap();
    assert_eq!(
        db.get_text(&format!("{row}.entrant_lists")).as_deref(),
        Some("f1_2025")
    );
}

#[test]
fn a_calendar_count_past_its_rows_comes_down_against_the_real_database() {
    let Some(dir) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let Ok(data) = std::fs::read(dir.join("game_package/gamedatabases/gamedatabases.2025.bin")) else {
        eprintln!("no gamedatabases.2025.bin under F1_25_DIR; skipping");
        return;
    };
    let cat = swofty_core::dbedit::Catalog::load(&data).unwrap();
    let mut db = Gvdb::parse(&data).unwrap();
    let count = "schedule.track_lists.track_event_list_19._fixed_event_reference_instancecount_";
    assert_eq!(db.get_text(count).as_deref(), Some("16"));
    db.set(count, "20", None).unwrap();
    let short = swofty_core::dbedit::rows::trim_row_counts(&cat, &mut db);
    assert_eq!(
        short
            .iter()
            .map(|t| (t.table.as_str(), t.count, t.rows))
            .collect::<Vec<_>>(),
        [(
            "schedule.track_lists.track_event_list_19.fixed_event_reference",
            20,
            16
        )]
    );
    assert_eq!(db.get_text(count).as_deref(), Some("16"));
}

#[test]
fn the_real_track_lists_hold_35_stock_tracks() {
    let Some(dir) = std::env::var_os("F1_25_DIR").map(PathBuf::from) else {
        eprintln!("F1_25_DIR is not set; skipping");
        return;
    };
    let Ok(data) = std::fs::read(dir.join("game_package/gamedatabases/gamedatabases.2025.bin")) else {
        eprintln!("no gamedatabases.2025.bin under F1_25_DIR; skipping");
        return;
    };
    let listed = swofty_core::tracks::ListedTracks::of(&Gvdb::parse(&data).unwrap());
    assert_eq!(listed.stock(), 35);
}
