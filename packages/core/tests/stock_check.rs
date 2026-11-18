use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};
use swofty_core::baseline::{Baseline, Identity, Objects, Source, sha256_file};
use swofty_core::install::Game;
use swofty_core::placed::Placed;
use swofty_core::stock::{self, Depots, Entry};

const DB: &str = "2025_asset_groups/ui_package/databases.erp";
const UI: &str = "2025_asset_groups/ui_package/common_front_end.erp";
const EAC: &str = "EAAntiCheat.GameServiceLauncher.exe";

struct Fixture {
    root: PathBuf,
    data: PathBuf,
    game: Game,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("swofty-stock-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let data = root.join("data");
        let dir = root.join("game");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        let game = Game {
            dir,
            user_reg: None,
            identity: Identity {
                build_id: "test".into(),
                exe_sha256: "0".repeat(64),
            },
        };
        Self { root, data, game }
    }

    fn write(&self, rel: &str, body: &[u8]) {
        let p = self.game.dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }

    fn record_first_sight(&self, rel: &str) {
        let mut b = Baseline::load_or_new(&self.data, &self.game.identity).unwrap();
        b.record(&self.objects(), &self.game.dir, rel, &()).unwrap();
        b.save(&self.data).unwrap();
    }

    fn objects(&self) -> Objects {
        Objects {
            root: self.data.join("baseline/objects"),
        }
    }

    fn baseline(&self) -> Baseline {
        Baseline::load_or_new(&self.data, &self.game.identity).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn entry(body: &[u8]) -> Entry {
    Entry {
        size: body.len() as u64,
        sha1: Sha1::digest(body).into(),
    }
}

fn depots(files: &[(&str, &[u8])]) -> Depots {
    Depots::from_entries(files.iter().map(|(rel, body)| ((*rel).to_owned(), entry(body))))
}

#[test]
fn originals_that_match_steam_are_proven_once() {
    let f = Fixture::new("proven");
    f.write(DB, b"stock db");
    f.record_first_sight(DB);
    let d = depots(&[(DB, b"stock db")]);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert!(c.foreign.is_empty());
    assert_eq!(c.proven, 1);
    assert_eq!(f.baseline().files[DB].source, Source::Depot);
    assert!(f.baseline().stock_checked);
    let again = stock::check(&f.data, &f.game, &d).unwrap();
    assert_eq!(
        (again.proven, again.swept),
        (0, None),
        "proven originals and a passed sweep are not redone"
    );
}

#[test]
fn a_broken_baseline_stops_until_the_game_is_verified_then_backs_up_steams_copy() {
    let f = Fixture::new("broken");
    f.write(DB, b"db with three extra tracks");
    f.record_first_sight(DB);
    let d = depots(&[(DB, b"stock db")]);

    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert_eq!(c.foreign, [DB]);
    assert_eq!(f.baseline().files[DB].source, Source::FirstSight);
    assert!(!f.baseline().stock_checked);
    let text = stock::Report::new(c.foreign, &f.game.dir).text();
    assert!(text.starts_with("F1 25 was not started. databases.erp was changed outside the launcher"));
    assert!(text.contains("Verify integrity of game files"));
    assert!(text.contains(stock::STEAM_VERIFY_URL));

    f.write(DB, b"stock db");
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert!(c.foreign.is_empty());
    assert_eq!(c.rerecorded, [DB]);
    let rec = &f.baseline().files[DB];
    assert_eq!(rec.source, Source::Depot);
    assert_eq!(rec.sha256, sha256_file(&f.game.dir.join(DB)).unwrap());
    assert_eq!(
        std::fs::read(f.objects().path(&rec.sha256)).unwrap(),
        b"stock db",
        "the original on record is now Steam's"
    );
    assert!(f.baseline().stock_checked);
}

#[test]
fn the_first_check_sweeps_every_steam_file_by_size() {
    let f = Fixture::new("sweep");
    f.write(UI, b"stock ui");
    f.write("F1_25.exe", b"exe!");
    let d = depots(&[(UI, b"stock ui"), ("F1_25.exe", b"exe"), ("missing.erp", b"x")]);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert_eq!(c.foreign, ["F1_25.exe", "missing.erp"]);
    assert_eq!(c.swept, Some(3));
    assert!(!f.baseline().stock_checked, "a failed sweep runs again");

    f.write("F1_25.exe", b"exe");
    f.write("missing.erp", b"x");
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert!(c.foreign.is_empty());
    assert!(f.baseline().stock_checked);
}

#[test]
fn the_parked_ea_launcher_and_the_launchers_own_leftovers_are_not_foreign() {
    let f = Fixture::new("parked");
    f.write(EAC, b"offline play stub, longer");
    f.write(&format!("{EAC}.bak"), b"real eac");
    f.write(UI, b"a session's ui");
    let mut placed = Placed::load(&f.data).unwrap();
    placed.insert(UI, sha256_file(&f.game.dir.join(UI)).unwrap(), true);
    placed.save(&f.data).unwrap();
    let d = depots(&[(EAC, b"real eac"), (UI, b"stock ui")]);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert!(c.foreign.is_empty(), "{:?}", c.foreign);
}

const EAC_DLL: &str = "EAAntiCheat.GameServiceLauncher.dll";

#[test]
fn ea_anticheat_launcher_files_drift_without_blocking() {
    let f = Fixture::new("eac-drift");
    f.write(EAC, b"ea rewrote the launcher exe on a normal start");
    f.write(EAC_DLL, b"ea rewrote the dll too");
    let d = depots(&[(EAC, b"stock exe"), (EAC_DLL, b"stock dll")]);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert!(c.foreign.is_empty(), "{:?}", c.foreign);
    assert_eq!(c.eac.len(), 2, "both drifted files are reported, neither blocks");
    assert!(
        f.baseline().stock_checked,
        "a drifted anti-cheat file still lets the sweep finish"
    );
}

const FFX: &str = "amd_fidelityfx_dx12.dll";
const THIRD_PARTY: &[&str] = &[
    "openvr_api.dll",
    FFX,
    "amd_fidelityfx_upscaler_dx12.dll",
    "NVNGX_DLSS.DLL",
    "bin/vendor_overlay.dll",
];

fn third_party_rels(c: &stock::Checked) -> Vec<&str> {
    c.third_party.iter().map(|d| d.rel.as_str()).collect()
}

#[test]
fn changed_third_party_dlls_are_logged_and_never_block() {
    let f = Fixture::new("third-party-dlls");
    let mut listed: Vec<(&str, &[u8])> = vec![("F1_25.exe", b"stock exe bytes")];
    f.write("F1_25.exe", b"stock exe bytes");
    for rel in THIRD_PARTY {
        f.write(rel, b"a third-party build of this dll");
        listed.push((rel, b"steam's dll"));
    }
    let d = depots(&listed);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert!(c.foreign.is_empty(), "{:?}", c.foreign);
    assert!(c.detail.is_empty(), "{:?}", c.detail);
    assert!(c.missing.is_empty(), "{:?}", c.missing);
    assert!(c.eac.is_empty());
    let mut want = THIRD_PARTY.to_vec();
    want.sort();
    assert_eq!(third_party_rels(&c), want);
    let ffx = c.third_party.iter().find(|x| x.rel == FFX).unwrap();
    assert_eq!(ffx.size, Some(b"a third-party build of this dll".len() as u64));
    assert_eq!(ffx.steam, b"steam's dll".len() as u64);
    assert!(
        f.baseline().stock_checked,
        "third-party dlls let the sweep finish"
    );
    let again = stock::check(&f.data, &f.game, &d).unwrap();
    assert!(again.foreign.is_empty());
}

#[test]
fn a_changed_third_party_dll_in_a_first_sight_record_is_logged() {
    let f = Fixture::new("third-party-first-sight");
    f.write(FFX, b"a drop-in fsr 4 loader");
    f.record_first_sight(FFX);
    let d = depots(&[(FFX, b"stock fsr 3.1 loader")]);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert!(c.foreign.is_empty(), "{:?}", c.foreign);
    assert_eq!(third_party_rels(&c), [FFX]);
    assert_eq!(f.baseline().files[FFX].source, Source::FirstSight);
}

#[test]
fn extra_dlls_steam_does_not_list_never_block() {
    let f = Fixture::new("extra-dlls");
    f.write(DB, b"stock db");
    for rel in ["dxgi.dll", "d3d12.dll", "winmm.dll", "reshade-shaders/addon.dll"] {
        f.write(rel, b"a third-party addition");
    }
    let d = depots(&[(DB, b"stock db")]);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert_eq!(
        c,
        stock::Checked {
            swept: Some(1),
            ..Default::default()
        }
    );
    let inst = swofty_core::install::Installer {
        depot: &d,
        ..swofty_core::install::Installer::new(&f.data, &f.game)
    };
    inst.apply(&replace_plan(&f)).unwrap();
    assert_eq!(std::fs::read(f.game.dir.join(DB)).unwrap(), b"modded db");
    assert_eq!(
        std::fs::read(f.game.dir.join("dxgi.dll")).unwrap(),
        b"a third-party addition",
        "the extra dll is left as the player put it"
    );
}

#[test]
fn changed_game_files_that_are_not_dlls_still_block_beside_third_party_dlls() {
    for name in ["F1_25.exe", UI, "amd_fidelityfx_dx12.dll.bak", "readme.dll.txt"] {
        let f = Fixture::new(&format!("third-party-and-{}", name.replace(['/', '.'], "-")));
        f.write(FFX, b"a drop-in fsr 4 loader");
        f.write(name, b"modded bytes");
        let d = depots(&[(FFX, b"stock fsr 3.1 loader"), (name, b"stock bytes, longer")]);
        let c = stock::check(&f.data, &f.game, &d).unwrap();
        assert_eq!(c.foreign, vec![name.to_owned()], "{name}");
        assert_eq!(third_party_rels(&c), [FFX], "{name}");
        assert!(!f.baseline().stock_checked, "{name}");
    }
}

#[test]
fn a_missing_game_dll_still_blocks_with_the_missing_files_fix() {
    let f = Fixture::new("missing-dll");
    f.write("F1_25.exe", b"stock exe bytes");
    let d = depots(&[(FFX, b"stock fsr 3.1 loader"), ("F1_25.exe", b"stock exe bytes")]);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert_eq!(c.foreign, [FFX]);
    assert!(c.third_party.is_empty());
    assert!(!f.baseline().stock_checked);
    let (changed, missing) = stock::split_absent(&c.detail);
    assert!(changed.is_empty());
    assert_eq!(missing, [FFX]);
    let current = stock::Report::missing(missing.clone(), &f.game.dir, Some("1.0.145.2472".into()));
    assert_eq!(current.fix(), stock::Fix::Verify);
    let older = stock::Report::missing(missing, &f.game.dir, Some("1.0.138.5601".into()));
    assert_eq!(older.fix(), stock::Fix::Update);
}

#[test]
fn the_steam_api_dll_is_still_checked_by_the_ownership_check() {
    let f = Fixture::new("steam-api");
    f.write("F1_25.exe", b"stock exe bytes");
    f.write("steam_api64.dll", b"an emulator's steam api");
    let d = depots(&[
        ("F1_25.exe", b"stock exe bytes"),
        ("steam_api64.dll", b"steam's steam api"),
    ]);
    assert_eq!(
        swofty_core::entitlement::changed_files(&f.data, &f.game.dir, &d).unwrap(),
        ["steam_api64.dll"]
    );
}

fn versioned_exe(major_minor: u32, build_revision: u32) -> Vec<u8> {
    let mut out = vec![0; 0x200];
    out[..2].copy_from_slice(b"MZ");
    out[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    out[0x80..0x84].copy_from_slice(b"PE\0\0");
    out[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
    out[0x94..0x96].copy_from_slice(&0xf0u16.to_le_bytes());
    out[0x98..0x9a].copy_from_slice(&0x20bu16.to_le_bytes());
    let dir = 0x98 + 112 + 2 * 8;
    out[dir..dir + 4].copy_from_slice(&0x1000u32.to_le_bytes());
    out[dir + 4..dir + 8].copy_from_slice(&0x100u32.to_le_bytes());
    let section = 0x98 + 0xf0;
    out[section + 8..section + 12].copy_from_slice(&0x100u32.to_le_bytes());
    out[section + 12..section + 16].copy_from_slice(&0x1000u32.to_le_bytes());
    out[section + 16..section + 20].copy_from_slice(&0x100u32.to_le_bytes());
    out[section + 20..section + 24].copy_from_slice(&0x200u32.to_le_bytes());
    let mut res = vec![0; 0x100];
    res[0x20..0x24].copy_from_slice(&0xFEEF_04BDu32.to_le_bytes());
    res[0x28..0x2c].copy_from_slice(&major_minor.to_le_bytes());
    res[0x2c..0x30].copy_from_slice(&build_revision.to_le_bytes());
    out.extend(res);
    out
}

#[test]
fn a_changed_game_exe_still_blocks_whatever_its_size() {
    let f = Fixture::new("exe-changed");
    f.write(FFX, b"a drop-in fsr 4 loader");
    f.write("F1_25.exe", b"a patched exe, longer");
    let d = depots(&[(FFX, b"stock fsr 3.1 loader"), ("F1_25.exe", b"stock exe bytes")]);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert_eq!(c.foreign, ["F1_25.exe"]);
    assert_eq!(third_party_rels(&c), [FFX]);

    f.write("F1_25.exe", b"STOCK EXE BYTES");
    assert_eq!(
        swofty_core::entitlement::changed_files(&f.data, &f.game.dir, &d).unwrap(),
        ["F1_25.exe"],
        "a same-size edit is caught by its SHA-1"
    );
    f.write("F1_25.exe", b"stock exe bytes");
    assert!(
        swofty_core::entitlement::changed_files(&f.data, &f.game.dir, &d)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_missing_game_exe_still_blocks_and_is_told_to_update_first() {
    let f = Fixture::new("exe-missing");
    f.write(FFX, b"a drop-in fsr 4 loader");
    let d = depots(&[(FFX, b"stock fsr 3.1 loader"), ("F1_25.exe", b"stock exe bytes")]);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert_eq!(c.foreign, ["F1_25.exe"]);
    let (changed, missing) = stock::split_absent(&c.detail);
    assert!(changed.is_empty());
    assert_eq!(missing, ["F1_25.exe"]);
    let version = swofty_core::exe_version::read(&f.game.dir.join(swofty_core::GAME_EXE));
    assert_eq!(version, None);
    let report = stock::Report::missing(missing, &f.game.dir, version);
    assert_eq!(report.fix(), stock::Fix::Both);
    let text = report.text();
    assert!(
        text.contains("F1_25.exe is missing. Update F1 25 in Steam or the EA app, then verify"),
        "{text}"
    );
}

#[test]
fn an_older_game_exe_routes_to_the_update_fix_with_third_party_dlls_in_place() {
    let f = Fixture::new("exe-older");
    let older = versioned_exe(1 << 16, (138 << 16) | 5601);
    f.write("F1_25.exe", &older);
    f.write(FFX, b"a drop-in fsr 4 loader");
    let d = depots(&[
        ("F1_25.exe", &older),
        (FFX, b"stock fsr 3.1 loader"),
        ("character_package/heads/head.mipmaps", b"a head"),
    ]);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert_eq!(c.foreign, ["character_package/heads/head.mipmaps"]);
    assert_eq!(third_party_rels(&c), [FFX]);
    let version = swofty_core::exe_version::read(&f.game.dir.join(swofty_core::GAME_EXE));
    assert_eq!(version.as_deref(), Some("1.0.138.5601"));
    let (_, missing) = stock::split_absent(&c.detail);
    let report = stock::Report::missing(missing, &f.game.dir, version.clone());
    assert_eq!(report.fix(), stock::Fix::Update);
    assert!(
        report
            .text()
            .contains("F1 25 1.0.138.5601 is older than 1.0.145.2472; update F1 25 in Steam or the EA app"),
        "{}",
        report.text()
    );
    let outdated = swofty_core::known_problems::outdated_exe(version.as_deref()).unwrap();
    assert_eq!(outdated.id, "outdated_exe");

    let mut current = versioned_exe(1 << 16, (145 << 16) | 2472);
    current.extend([0; 16]);
    f.write("F1_25.exe", &current);
    let version = swofty_core::exe_version::read(&f.game.dir.join(swofty_core::GAME_EXE));
    assert_eq!(
        version.as_deref(),
        Some(swofty_core::known_problems::LATEST_GAME_VERSION)
    );
    assert_eq!(stock::fix_for(version.as_deref()), stock::Fix::Verify);
    assert!(swofty_core::known_problems::outdated_exe(version.as_deref()).is_none());
}

#[test]
fn a_stock_launcher_exe_with_a_stale_backup_is_not_foreign() {
    let f = Fixture::new("eac-stale-bak");
    f.write(EAC, b"stock exe");
    f.write(&format!("{EAC}.bak"), b"a stale backup of a different size");
    let d = depots(&[(EAC, b"stock exe")]);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert!(c.foreign.is_empty());
    assert!(c.eac.is_empty(), "the live exe matches Steam, nothing to report");
    assert!(f.baseline().stock_checked);
}

#[test]
fn modified_game_data_still_blocks_even_with_eac_drift() {
    let f = Fixture::new("eac-and-data");
    f.write(EAC, b"ea rewrote this");
    f.write("F1_25.exe", b"modded exe");
    f.write(UI, b"modded ui");
    let d = depots(&[
        (EAC, b"stock exe"),
        ("F1_25.exe", b"stock exe bytes"),
        (UI, b"stock ui"),
    ]);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    let mut want = vec!["F1_25.exe".to_owned(), UI.to_owned()];
    want.sort();
    assert_eq!(c.foreign, want, "real game data still stops the launch");
    assert_eq!(c.eac.len(), 1, "the anti-cheat file is reported, not blocked");
    assert!(
        !f.baseline().stock_checked,
        "a foreign game file keeps the sweep open"
    );
    let exe = c.detail.iter().find(|x| x.rel == "F1_25.exe").unwrap();
    assert_eq!(exe.measured, "F1_25.exe");
    assert_eq!(exe.size, Some(b"modded exe".len() as u64));
    assert_eq!(exe.steam, b"stock exe bytes".len() as u64);
}

#[test]
fn files_steam_does_not_list_are_left_alone() {
    let f = Fixture::new("unlisted");
    f.write(DB, b"anything");
    f.record_first_sight(DB);
    f.write("bypass.exe", b"an extra file");
    let c = stock::check(&f.data, &f.game, &depots(&[])).unwrap();
    assert!(c.foreign.is_empty());
    assert_eq!(f.baseline().files[DB].source, Source::FirstSight);
}

#[test]
fn a_new_original_is_checked_before_it_is_backed_up() {
    let f = Fixture::new("record");
    f.write(DB, b"modded db");
    let d = depots(&[(DB, b"stock db")]);
    let mut b = f.baseline();
    let err = b.record(&f.objects(), &f.game.dir, DB, &d).unwrap_err();
    assert!(matches!(&err, swofty_core::Error::NotStock(files) if files == &[DB.to_owned()]));
    assert!(
        err.to_string()
            .contains("databases.erp is not the copy Steam installed")
    );
    assert!(err.to_string().contains("open steam://validate/3059520"));
    assert!(
        !f.objects().has(&sha256_file(&f.game.dir.join(DB)).unwrap()),
        "nothing was backed up"
    );

    f.write(DB, b"stock db");
    assert_eq!(
        b.record(&f.objects(), &f.game.dir, DB, &d).unwrap().source,
        Source::Depot
    );
}

fn varint(mut v: u64, out: &mut Vec<u8>) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn field(n: u64, body: &[u8], out: &mut Vec<u8>) {
    varint(n << 3 | 2, out);
    varint(body.len() as u64, out);
    out.extend_from_slice(body);
}

fn int(n: u64, v: u64, out: &mut Vec<u8>) {
    varint(n << 3, out);
    varint(v, out);
}

fn mapping(name: &str, body: &[u8], flags: u64) -> Vec<u8> {
    let mut m = Vec::new();
    field(1, name.as_bytes(), &mut m);
    int(2, body.len() as u64, &mut m);
    int(3, flags, &mut m);
    field(4, &[0; 20], &mut m);
    field(5, &Sha1::digest(body), &mut m);
    m
}

fn section(magic: u32, body: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(&magic.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
}

fn manifest(encrypted: bool) -> Vec<u8> {
    let mut payload = Vec::new();
    field(
        1,
        &mapping("2025_asset_groups\\ui_package\\databases.erp", b"stock db", 0),
        &mut payload,
    );
    field(
        1,
        &mapping("2025_asset_groups\\ui_package", b"", 0x40),
        &mut payload,
    );
    let mut meta = Vec::new();
    int(1, 3059521, &mut meta);
    int(4, u64::from(encrypted), &mut meta);
    let mut out = Vec::new();
    section(0x71F6_17D0, &payload, &mut out);
    section(0x1F48_12BE, &meta, &mut out);
    section(0x1B81_B817, &[], &mut out);
    section(0x32C4_15AB, &[], &mut out);
    out
}

#[test]
fn steams_manifest_lists_files_with_forward_slashes_and_skips_directories() {
    let files = stock::parse_manifest(&manifest(false)).unwrap();
    assert_eq!(files, [(DB.to_owned(), entry(b"stock db"))]);
    assert!(
        stock::parse_manifest(&manifest(true))
            .unwrap_err()
            .contains("encrypted")
    );
}

#[test]
fn the_real_install_matches_steams_manifests() {
    let Some(found) = swofty_core::steam::find_game() else {
        eprintln!("no Steam install of F1 25; skipping");
        return;
    };
    let Some(data) = std::env::var_os("SWOFTY_STOCK_DATA").map(PathBuf::from) else {
        eprintln!("SWOFTY_STOCK_DATA is not set; skipping");
        return;
    };
    let d = Depots::load(&found).unwrap();
    eprintln!("Steam lists {} files", d.len());
    let b: Baseline = serde_json::from_slice(&std::fs::read(baseline_json(&data)).unwrap()).unwrap();
    let objects = Objects {
        root: data.join("baseline/objects"),
    };
    let mut checked = 0;
    for (rel, rec) in &b.files {
        let e = d
            .get(rel)
            .unwrap_or_else(|| panic!("{rel} is not in Steam's list"));
        assert_eq!(
            stock::sha1_file(&objects.path(&rec.sha256)).unwrap(),
            e.sha1,
            "{rel}"
        );
        checked += 1;
    }
    eprintln!("{checked} originals on record match Steam's copy");
}

fn baseline_json(data: &Path) -> PathBuf {
    std::fs::read_dir(data.join("baseline"))
        .unwrap()
        .flatten()
        .map(|e| e.path().join("baseline.json"))
        .find(|p| p.is_file())
        .unwrap()
}

#[test]
fn the_verify_link_opens_steams_check_for_f1_25() {
    assert_eq!(
        stock::STEAM_VERIFY_URL,
        format!("steam://validate/{}", swofty_core::STEAM_APP_ID)
    );
}

#[test]
fn missing_files_are_told_to_update_an_older_game_and_verify_a_current_one() {
    let files = vec!["character_package/heads/nico_rosberg_head_d.tif.2024.mipmaps".to_owned()];
    let older = stock::missing_text(&files, Some("1.0.138.5601"));
    assert!(
        older.starts_with("nico_rosberg_head_d.tif.2024.mipmaps is missing from the game folder;"),
        "{older}"
    );
    assert!(
        older.contains("F1 25 1.0.138.5601 is older than 1.0.145.2472; update F1 25 in Steam or the EA app"),
        "{older}"
    );
    assert!(!older.contains("verify"), "{older}");
    let current = stock::missing_text(&files, Some("1.0.145.2472"));
    assert!(
        current.contains("verify the game files in Steam (open steam://validate/3059520"),
        "{current}"
    );
    assert!(!current.contains("update F1 25"), "{current}");
    let unknown = stock::missing_text(&files, None);
    let (update, verify) = (
        unknown.find("update F1 25").unwrap(),
        unknown.find("verify").unwrap(),
    );
    assert!(update < verify, "{unknown}");
    assert_eq!(stock::fix_for(Some("1.0.138.5601")), stock::Fix::Update);
    assert_eq!(stock::fix_for(Some("1.0.141.7865")), stock::Fix::Update);
    assert_eq!(stock::fix_for(Some("1.0.145.2472")), stock::Fix::Verify);
    assert_eq!(stock::fix_for(None), stock::Fix::Both);
    let mut r = stock::Report::missing(files, Path::new("/games/F1 25"), Some("1.0.138.5601".into()));
    r.at = 1_793_144_157.5;
    let back: stock::Report = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(back, r);
    assert_eq!(back.fix(), stock::Fix::Update);
}

#[test]
fn a_missing_anti_cheat_launcher_with_no_backup_is_reported() {
    let f = Fixture::new("eac-missing");
    f.write(UI, b"stock ui");
    let d = depots(&[(EAC, b"stock exe"), (UI, b"stock ui")]);
    let c = stock::check(&f.data, &f.game, &d).unwrap();
    assert_eq!(c.missing, vec![EAC.to_owned()]);
    assert!(c.foreign.is_empty(), "{:?}", c.foreign);
}

#[test]
fn a_parked_or_present_anti_cheat_launcher_is_not_missing() {
    let f = Fixture::new("eac-not-missing");
    let d = depots(&[(EAC, b"stock exe")]);
    f.write(EAC, b"ea rewrote it");
    assert!(stock::check(&f.data, &f.game, &d).unwrap().missing.is_empty());
    std::fs::remove_file(f.game.dir.join(EAC)).unwrap();
    f.write(&format!("{EAC}.bak"), b"stock exe");
    assert!(stock::check(&f.data, &f.game, &d).unwrap().missing.is_empty());
}

fn replace_plan(f: &Fixture) -> swofty_core::install::Plan {
    let src = f.root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("db"), b"modded db").unwrap();
    swofty_core::install::Plan {
        profile: "MODDED".into(),
        data: std::collections::BTreeMap::from([(DB.to_owned(), src.join("db"))]),
        added: Default::default(),
        loader: Default::default(),
        registry_override: false,
    }
}

fn stale_baseline(f: &Fixture) {
    f.write(DB, b"old recorded db");
    f.record_first_sight(DB);
}

#[test]
fn a_stale_baseline_whose_file_matches_steam_is_rerecorded_and_the_install_goes_on() {
    let f = Fixture::new("stale-match");
    stale_baseline(&f);
    f.write(DB, b"stock db");
    let d = depots(&[(DB, b"stock db")]);
    let inst = swofty_core::install::Installer {
        depot: &d,
        ..swofty_core::install::Installer::new(&f.data, &f.game)
    };
    inst.apply(&replace_plan(&f)).unwrap();
    let rec = &f.baseline().files[DB];
    assert_eq!(rec.source, Source::Depot);
    assert_eq!(std::fs::read(f.objects().path(&rec.sha256)).unwrap(), b"stock db");
    assert_eq!(std::fs::read(f.game.dir.join(DB)).unwrap(), b"modded db");
}

#[test]
fn a_file_that_differs_from_steam_and_the_baseline_stops_with_the_not_stock_error() {
    let f = Fixture::new("stale-mismatch");
    stale_baseline(&f);
    f.write(DB, b"a mod copied in by hand");
    let d = depots(&[(DB, b"stock db")]);
    let log = std::cell::RefCell::new(Vec::new());
    let out = |s: &str| log.borrow_mut().push(s.to_owned());
    let inst = swofty_core::install::Installer {
        depot: &d,
        out: &out,
        ..swofty_core::install::Installer::new(&f.data, &f.game)
    };
    let err = inst.apply(&replace_plan(&f)).unwrap_err();
    assert!(matches!(&err, swofty_core::Error::NotStock(files) if files == &[DB.to_owned()]));
    assert!(err.to_string().contains("verify the game files"));
    let want = sha256_file(&f.game.dir.join(DB)).unwrap();
    assert!(
        log.borrow()
            .iter()
            .any(|l| l.contains(&want) && l.contains("size 23"))
    );
    assert_eq!(
        std::fs::read(f.game.dir.join(DB)).unwrap(),
        b"a mod copied in by hand",
        "the file is left alone"
    );
    assert_eq!(f.baseline().files[DB].source, Source::FirstSight);
}

#[test]
fn without_steams_list_the_conflict_is_still_refused() {
    let f = Fixture::new("stale-unknown");
    stale_baseline(&f);
    f.write(DB, b"stock db");
    let inst = swofty_core::install::Installer::new(&f.data, &f.game);
    let err = inst.apply(&replace_plan(&f)).unwrap_err();
    assert!(matches!(&err, swofty_core::Error::Leftovers(c) if c.len() == 1 && !c[0].owned));
    assert_eq!(std::fs::read(f.game.dir.join(DB)).unwrap(), b"stock db");
}

fn foreign(rel: &str, size: Option<u64>) -> stock::Foreign {
    stock::Foreign {
        rel: rel.to_owned(),
        measured: rel.to_owned(),
        size,
        steam: 10,
        modified: None,
    }
}

#[test]
fn many_missing_files_are_counted_not_named() {
    let files: Vec<String> = (0..7772).map(|i| format!("a/f{i}.erp")).collect();
    let text = stock::missing_text(&files, Some("1.0.145.2472"));
    assert!(
        text.starts_with("7772 game files are missing from the game folder;"),
        "{text}"
    );
    assert!(text.contains("verify the game files in Steam"), "{text}");
    assert!(!text.contains("copied into the game folder by hand"), "{text}");
}

#[test]
fn absent_files_are_split_from_changed_ones() {
    let detail = vec![
        foreign("a.erp", None),
        foreign("b.erp", Some(3)),
        foreign("c.erp", None),
    ];
    let (changed, missing) = stock::split_absent(&detail);
    assert_eq!(changed, ["b.erp"]);
    assert_eq!(missing, ["a.erp", "c.erp"]);
}

#[test]
fn a_report_of_only_absent_files_says_missing_and_a_mixed_one_says_both() {
    let game = Path::new("/games/F1 25");
    let only = stock::Report::new(vec!["a.erp".into(), "c.erp".into()], game)
        .with_detail(vec![foreign("a.erp", None), foreign("c.erp", None)]);
    let text = only.text();
    assert!(text.contains("a.erp and c.erp are missing"), "{text}");
    assert!(!text.contains("changed outside"), "{text}");
    assert!(!only.mixed());

    let mixed = stock::Report::new(vec!["a.erp".into(), "b.erp".into()], game)
        .with_detail(vec![foreign("a.erp", None), foreign("b.erp", Some(3))]);
    assert!(mixed.mixed());
    let text = mixed.text();
    assert!(
        text.contains("1 game file was changed outside the launcher"),
        "{text}"
    );
    assert!(text.contains("a.erp is missing"), "{text}");

    let edited = stock::Report::new(vec!["b.erp".into()], game).with_detail(vec![foreign("b.erp", Some(3))]);
    assert!(edited.text().contains("b.erp was changed outside the launcher"));
}

#[test]
fn a_mixed_error_names_both_counts() {
    let err = swofty_core::Error::NotStockAndMissing {
        changed: vec!["b.erp".into()],
        missing: (0..5).map(|i| format!("m{i}.erp")).collect(),
        version: Some("1.0.145.2472".into()),
    };
    let text = err.to_string();
    assert!(
        text.starts_with("b.erp is not the copy Steam installed"),
        "{text}"
    );
    assert!(text.contains("5 game files are missing"), "{text}");
}
