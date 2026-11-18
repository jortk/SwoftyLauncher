use std::process::ExitCode;
use std::time::{Duration, SystemTime};

use anyhow::{Result, bail};
use clap::Subcommand;
use swofty_core::install::Game;
use swofty_core::launcher_saves;
use swofty_core::save_backup;
use swofty_core::store::Store;
use swofty_core::vault::restore::{self, Running};
use swofty_core::vault::{
    self, Checked, Location, Meta, Notice, Reason, Snapshot, Taken, Vault, migrate, watch,
};

use crate::run::say;

#[derive(Subcommand)]
pub enum SavesCommand {
    #[command(
        about = "List the career vault's snapshots of the profile save, newest first, with the cloud status"
    )]
    List {
        #[arg(long, help = SOURCE_HELP)]
        source: Option<String>,
    },
    #[command(
        about = "Show one snapshot: its files with their hashes, Steam's record of them, and its events"
    )]
    Show {
        #[arg(help = "The snapshot, as `swofty saves list` prints it")]
        snapshot: String,
        #[arg(long, help = SOURCE_HELP)]
        source: Option<String>,
    },
    #[command(about = "Snapshot the profile save now and pin it, for example before playing from Steam")]
    Backup,
    #[command(
        about = "Copy a snapshot back over the profile save (F1 25 and Steam, or the EA app, must be closed); snapshots the current save first"
    )]
    Restore {
        #[arg(help = "The snapshot, as `swofty saves list` prints it")]
        snapshot: String,
        #[arg(long, help = SOURCE_HELP)]
        source: Option<String>,
    },
    #[command(
        about = "List the versions of the profile save with the careers each holds, its cloud status, and what restoring it gets back and loses"
    )]
    Versions {
        #[arg(long, help = SOURCE_HELP)]
        source: Option<String>,
    },
    #[command(about = "List the careers the vault has seen, with their status and the addons they need")]
    Careers {
        #[arg(long, help = SOURCE_HELP)]
        source: Option<String>,
    },
    #[command(about = "Keep a snapshot: retention never removes it")]
    Pin {
        #[arg(help = "The snapshot, as `swofty saves list` prints it")]
        snapshot: String,
        #[arg(long, help = SOURCE_HELP)]
        source: Option<String>,
    },
    #[command(about = "Undo `pin`; reasons other than your own pin keep the snapshot")]
    Unpin {
        #[arg(help = "The snapshot, as `swofty saves list` prints it")]
        snapshot: String,
        #[arg(long, help = SOURCE_HELP)]
        source: Option<String>,
    },
    #[command(about = "Show or set the career vault's size cap in MB (500 by default)")]
    Cap { mb: Option<u64> },
    #[command(
        about = "Copy a Steam save, or another profile's launcher saves, into a profile's launcher saves (F1 25 must be closed); keeps what they held in the vault first"
    )]
    Import {
        #[arg(
            long,
            value_name = "steam",
            conflicts_with = "from_profile",
            help = "Import the Steam save (the default)"
        )]
        from: Option<String>,
        #[arg(
            long,
            value_name = "PROFILE",
            help = "Import another profile's launcher saves"
        )]
        from_profile: Option<String>,
        #[arg(long, help = "The profile to import into (default: the active profile)")]
        profile: Option<String>,
    },
    #[command(
        about = "Copy a profile's launcher saves over the Steam save (F1 25 and Steam must be closed); keeps the Steam save in the vault first"
    )]
    Export {
        #[arg(long, value_name = "steam", help = "Where to copy to; only steam")]
        to: String,
        #[arg(long, help = "The profile to export (default: the active profile)")]
        profile: Option<String>,
    },
}

const PINNED: &str = "pinned";

const SOURCE_HELP: &str =
    "The vault to use, steam-<id>, ea-<user> or launcher-<profile> (default: the save of the copy Play uses)";

/// The snapshot a session's after-session check compares with.
pub struct Before {
    vault: Vault,
    loc: Location,
    snapshot: Option<Snapshot>,
    meta: Meta,
    steam: Option<std::path::PathBuf>,
}

fn report(lines: Vec<String>) {
    for l in lines {
        say("info", &l);
    }
}

/// Snapshot the profile save before a session, after moving in any old
/// `save-backups/`.
pub fn before_session(
    store: &Store,
    profile: &str,
    g: &Game,
    session: &str,
    build_blocked: Option<&str>,
    steam_saves_once: bool,
) -> Option<Before> {
    before_session_at(
        store,
        profile,
        g,
        session,
        build_blocked,
        steam_saves_once,
        Location::for_game(g),
    )
}

fn before_session_at(
    store: &Store,
    profile: &str,
    g: &Game,
    session: &str,
    build_blocked: Option<&str>,
    steam_saves_once: bool,
    game_loc: Option<Location>,
) -> Option<Before> {
    match migrate::migrate(&store.root) {
        Ok(notes) => report(notes),
        Err(e) => say(
            "warning",
            &format!("saves: could not move save-backups into the career vault: {e}"),
        ),
    }
    for line in swofty_core::save_stall::sweep(store, game_loc.as_ref(), Running::now(), vault::now()) {
        say("warning", &line);
    }
    let p = store.profile(profile);
    let launcher = game_loc.as_ref().and_then(|l| {
        let area = launcher_saves::area(launcher_saves::Choice {
            build_blocked,
            vanilla: p.as_ref().map_or(true, |p| p.is_vanilla()),
            profile_uses_launcher_saves: p.as_ref().is_ok_and(|p| p.launcher_saves),
            steam_saves_once,
            layout: l.layout,
        });
        match area {
            launcher_saves::Area::Launcher => launcher_saves::store_for(&store.root, profile, l),
            launcher_saves::Area::Own(launcher_saves::Why::EaCopy) => {
                say("info", &format!("saves: {}", launcher_saves::EA_NOTE));
                None
            }
            launcher_saves::Area::Own(_) => None,
        }
    });
    if let (Some(store_loc), Some(from)) = (&launcher, &game_loc) {
        seed_if_empty(store, profile, from, store_loc);
    }
    let steam = game_loc
        .as_ref()
        .filter(|l| l.layout.named_like_steam() && !l.layout.is_launcher())
        .map(|l| l.dir.clone());
    if let Some(dir) = &steam {
        say(
            "info",
            &format!(
                "saves: Steam remote folder at session start, {}",
                vault::folder_listing(dir)
            ),
        );
    }
    let Some(loc) = launcher.or(game_loc) else {
        say(
            "info",
            "saves: no profile save folder found for this copy of F1 25; nothing to snapshot",
        );
        return None;
    };
    if loc.layout.is_launcher() {
        say(
            "info",
            &format!(
                "saves: this session plays on the launcher saves of profile {profile} ({}); Steam's saves are not touched",
                loc.dir.display()
            ),
        );
    }
    let absent = match p
        .as_ref()
        .map_err(|e| e.to_string())
        .and_then(|p| save_backup::absent_in(store, p).map_err(|e| e.to_string()))
    {
        Ok(a) => a,
        Err(e) => {
            say(
                "warning",
                &format!("saves: could not check the profile's addons: {e}"),
            );
            Vec::new()
        }
    };
    let mut meta = Meta::new(Reason::BeforeSession).profile(profile);
    meta.session_id = (!session.is_empty()).then(|| session.to_owned());
    meta.absent = absent.clone();
    meta.exe_sha256 = Some(g.identity.exe_sha256.clone());
    if p.is_ok_and(|p| p.is_vanilla()) && !absent.is_empty() {
        meta = meta.pin("vanilla-session");
    }
    let vault = match Vault::of(&store.root, &loc) {
        Ok(v) => v,
        Err(e) => {
            say("warning", &format!("saves: {e}"));
            return None;
        }
    };
    let snapshot = match vault.before_session(&loc, meta.clone(), vault::now()) {
        Ok(c) => {
            if let Some(n) = &c.notice {
                say("warning", &format!("saves: {}", n.text()));
            }
            let snapshot = c.taken.snapshot().cloned();
            report(watch::describe(&Checked { notice: None, ..c }));
            snapshot
        }
        Err(e) => {
            say(
                "warning",
                &format!("saves: could not snapshot the profile save: {e}"),
            );
            None
        }
    };
    if let (Some(s), false) = (&snapshot, absent.is_empty()) {
        say(
            "info",
            &format!(
                "saves: this session runs without {}; snapshot {} holds the save from before it",
                absent.join(", "),
                s.id
            ),
        );
    }
    Some(Before {
        vault,
        loc,
        snapshot,
        meta,
        steam,
    })
}

fn seed_if_empty(store: &Store, profile: &str, from: &Location, to: &Location) {
    if launcher_saves::has_files(to) {
        return;
    }
    match launcher_saves::seed(&store.root, profile, from, Reason::Imported, vault::now()) {
        Ok(s) => say("info", &s.line(profile, from, to)),
        Err(e) => say(
            "warning",
            &format!(
                "saves: could not copy the Steam save into the launcher saves of profile {profile}: {e}"
            ),
        ),
    }
}

/// Snapshot the profile save once the game exited and the session's game
/// files are rolled back.
pub fn after_session(before: Option<Before>) {
    let Some(b) = before else {
        return;
    };
    if let Some(dir) = &b.steam {
        say(
            "info",
            &format!(
                "saves: Steam remote folder at session end, {}",
                vault::folder_listing(dir)
            ),
        );
    }
    let mut meta = b.meta;
    meta.reason = Reason::AfterSession;
    meta.pin = None;
    match b
        .vault
        .after_session(&b.loc, meta, b.snapshot.as_ref(), vault::now())
    {
        Ok(c) => {
            if let Some(n) = &c.notice {
                say("warning", &format!("saves: {}", n.text()));
            }
            report(watch::describe(&Checked { notice: None, ..c }));
        }
        Err(e) => say(
            "warning",
            &format!("saves: could not snapshot the profile save after the session: {e}"),
        ),
    }
}

/// The vault notice a session that started at `since` raised.
pub fn notice_since(store: &Store, since: f64) -> Option<String> {
    let at = std::fs::metadata(Notice::path(&store.root))
        .and_then(|m| m.modified())
        .ok()?;
    let since = SystemTime::UNIX_EPOCH + Duration::from_secs_f64(since.max(0.0));
    if at < since {
        return None;
    }
    Notice::read(&store.root).map(|n| n.text())
}

fn default_location(store: &Store, active: Option<Location>) -> Option<Location> {
    let name = store.active_profile();
    let uses_launcher_saves = store
        .profile(&name)
        .is_ok_and(|p| !p.is_vanilla() && p.launcher_saves);
    if !uses_launcher_saves {
        return active;
    }
    match &active {
        Some(l) if !launcher_saves::supported(l.layout) => active,
        Some(l) => launcher_saves::store_for(&store.root, &name, l),
        None => Some(launcher_saves::location(&store.root, &name)),
    }
}

fn open(store: &Store, source: Option<&str>) -> Result<(Vault, Option<Location>)> {
    let active = Location::active(&store.root);
    match source {
        Some(s) => Ok((
            Vault::open(&store.root, s)?,
            active
                .filter(|l| l.source == s)
                .or_else(|| Location::of_source(&store.root, s)),
        )),
        None => match default_location(store, active) {
            Some(l) => Ok((Vault::of(&store.root, &l)?, Some(l))),
            None => match Vault::sources(&store.root).as_slice() {
                [only] => Ok((Vault::open(&store.root, only)?, None)),
                [] => bail!(
                    "no F1 25 profile save found and the career vault in {} is empty",
                    Vault::root(&store.root).display()
                ),
                many => bail!(
                    "no F1 25 profile save found; pass --source with one of {}",
                    many.join(", ")
                ),
            },
        },
    }
}

fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
}

fn row(s: &Snapshot) -> String {
    let mut out = format!("{}  {}", s.id, s.reason.text());
    if let Some(p) = &s.profile {
        out.push_str(&format!(", profile {p}"));
    }
    if !s.absent_addons.is_empty() {
        out.push_str(&format!(", without {}", s.absent_addons.join(", ")));
    }
    out.push_str(&format!(", {} file(s), {} bytes", s.files.len(), s.total()));
    if !s.pinned.is_empty() {
        out.push_str(&format!(", pinned ({})", s.pinned.join(", ")));
    }
    out
}

fn list(store: &Store, source: Option<&str>) -> Result<()> {
    if let Err(e) = migrate::migrate(&store.root).map(report) {
        say("warning", &format!("saves: {e}"));
    }
    let (v, loc) = match open(store, source) {
        Ok(x) => x,
        Err(e) if source.is_none() && Vault::sources(&store.root).is_empty() => {
            println!("no snapshots: {e}");
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    let snaps = v.list()?;
    println!(
        "vault {}: {} snapshot(s), {} of {}",
        v.dir.display(),
        snaps.len(),
        mb(v.usage()),
        mb(v.policy.cap_bytes)
    );
    if let Some(l) = &loc {
        match vault::save_status(l) {
            Ok(st) if st.files > 0 => println!(
                "save {}: {} file(s), {} bytes, Steam Cloud: {}",
                l.dir.display(),
                st.files,
                st.total,
                st.cloud.text()
            ),
            Ok(_) => println!("save {}: no profile save yet", l.dir.display()),
            Err(e) => println!("save {}: {e}", l.dir.display()),
        }
    }
    for s in snaps {
        println!("{}", row(&s));
    }
    Ok(())
}

fn show(store: &Store, id: &str, source: Option<&str>) -> Result<()> {
    let (v, _) = open(store, source)?;
    let s = v.find(id)?;
    println!("{}", row(&s));
    println!("taken {} by launcher {}", vault::human(s.at), s.launcher);
    println!("save folder {}", s.save_dir.display());
    if let Some(n) = &s.note {
        println!("note: {n}");
    }
    if let Some(sid) = &s.session_id {
        println!("session {sid}");
    }
    if let Some(exe) = &s.exe_sha256 {
        println!("F1_25.exe sha256 {exe}");
    }
    println!("Steam Cloud when taken: {}", s.cloud().text());
    for f in &s.files {
        println!(
            "  {} ({}): {} bytes, sha256 {}, sha1 {}",
            f.name, f.file, f.size, f.sha256, f.sha1
        );
        if let Some(r) = &f.remotecache {
            println!(
                "    remotecache: size {}, sha {}, syncstate {}",
                r.size, r.sha, r.syncstate
            );
        }
    }
    for e in v
        .events()
        .iter()
        .filter(|e| e.snapshot.as_deref() == Some(id) || e.from.as_deref() == Some(id))
    {
        println!(
            "  event {} {}{}",
            vault::human(e.at),
            serde_json::to_value(e.kind)
                .ok()
                .and_then(|k| k.as_str().map(str::to_owned))
                .unwrap_or_default(),
            e.detail.as_deref().map(|d| format!(": {d}")).unwrap_or_default()
        );
    }
    Ok(())
}

fn names(list: &[vault::careers::Brief]) -> String {
    list.iter()
        .map(|b| match &b.place {
            Some(p) => format!("{} ({p})", b.name),
            None => b.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn versions(store: &Store, source: Option<&str>) -> Result<()> {
    if let Err(e) = migrate::migrate(&store.root).map(report) {
        say("warning", &format!("saves: {e}"));
    }
    let (v, loc) = match open(store, source) {
        Ok(x) => x,
        Err(e) if source.is_none() && Vault::sources(&store.root).is_empty() => {
            println!("no versions: {e}");
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    let snaps = v.list()?;
    println!(
        "{} version(s) in {}, {} of {}",
        snaps.len(),
        v.dir.display(),
        mb(v.usage()),
        mb(v.policy.cap_bytes)
    );
    if let Some(st) = loc.as_ref().and_then(|l| vault::save_status(l).ok()) {
        println!("Steam Cloud now: {}", st.cloud.text());
    }
    for s in snaps {
        let held = v.index_of(&s).map(|i| {
            let held: Vec<String> = vault::careers::in_save(&i)
                .map(|c| match c.label.is_empty() {
                    true => c.career.clone(),
                    false => c.label.clone(),
                })
                .collect();
            match held.len() {
                0 => "no careers".to_owned(),
                n => format!("{n} career(s): {}", held.join(", ")),
            }
        });
        println!(
            "{}  {}, {}, {}, Steam Cloud: {}, {}{}",
            s.id,
            s.reason.text(),
            match &s.profile {
                Some(p) => format!("profile {p}"),
                None => "no profile".to_owned(),
            },
            mb(s.total()),
            s.cloud().text(),
            held.as_deref().unwrap_or("careers unknown"),
            match s.pinned.is_empty() {
                true => String::new(),
                false => format!(", pinned ({})", s.pinned.join(", ")),
            }
        );
        if let Some(d) = v.preview(&s).filter(|d| !d.is_empty()) {
            if !d.get_back.is_empty() {
                println!("    restoring it gets back: {}", names(&d.get_back));
            }
            if !d.lose.is_empty() {
                println!("    restoring it loses: {}", names(&d.lose));
            }
            for (now, then) in &d.progress {
                println!(
                    "    restoring it loses progress in {}: {} back to {}",
                    now.name,
                    now.place.as_deref().unwrap_or("now"),
                    then.place.as_deref().unwrap_or("then")
                );
            }
        }
    }
    Ok(())
}

fn careers(store: &Store, source: Option<&str>) -> Result<()> {
    let (v, _) = match open(store, source) {
        Ok(x) => x,
        Err(e) if source.is_none() && Vault::sources(&store.root).is_empty() => {
            println!("no careers: {e}");
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    let table = v.careers();
    if table.careers.is_empty() {
        println!(
            "no careers yet: the vault learns them from the career index F1 25 writes in a launcher session"
        );
        return Ok(());
    }
    for r in &table.careers {
        let mut line = format!("{}  {}", r.career, r.name());
        if let Some(m) = &r.mode {
            line.push_str(&format!(", {m}"));
        }
        if let Some(d) = &r.driver {
            line.push_str(&format!(", {d}"));
        }
        if let Some(t) = &r.team {
            line.push_str(&format!(", {t}"));
        }
        if let Some(p) = r.place() {
            line.push_str(&format!(", {p}"));
        }
        if let Some(at) = r.last_played {
            line.push_str(&format!(", last played {}", vault::human(at)));
        }
        let needs: Vec<&str> = r.needs().collect();
        if !needs.is_empty() {
            line.push_str(&format!(", needs {}", needs.join(" ")));
        }
        line.push_str(&format!(", {}", r.status.slug()));
        println!("{line}");
        if r.status.is_missing() {
            println!("    last in the save at snapshot {}", r.last_seen);
        }
    }
    Ok(())
}

fn pin_cmd(store: &Store, id: &str, source: Option<&str>, pin: bool) -> Result<()> {
    let (v, _) = open(store, source)?;
    let s = match pin {
        true => v.pin(id, PINNED, vault::now())?,
        false => v.unpin_why(id, PINNED, vault::now())?,
    };
    match (pin, s.pinned.is_empty()) {
        (true, _) => println!("pinned snapshot {}", s.id),
        (false, true) => println!("unpinned snapshot {}", s.id),
        (false, false) => println!(
            "unpinned snapshot {}; it stays kept for {}",
            s.id,
            s.pinned.join(", ")
        ),
    }
    Ok(())
}

fn backup(store: &Store) -> Result<()> {
    migrate::migrate(&store.root).map(report).ok();
    let Some(loc) = Location::active(&store.root) else {
        bail!("no F1 25 profile save found to back up");
    };
    let v = Vault::of(&store.root, &loc)?;
    let profile = store.active_profile();
    let meta = Meta::new(Reason::Manual).profile(profile).pin("backup");
    match v.check(&loc, meta, vault::now())?.taken {
        Taken::New(s) => println!("saved and pinned snapshot {} ({} bytes)", s.id, s.total()),
        Taken::Unchanged(s) => println!("the profile save is unchanged since snapshot {}; pinned it", s.id),
        Taken::NoSave => bail!("no profile save in {} yet", loc.dir.display()),
    }
    Ok(())
}

fn restore_cmd(store: &Store, id: &str, source: Option<&str>) -> Result<()> {
    let (v, loc) = open(store, source)?;
    let snap = v.find(id)?;
    let loc = match loc {
        Some(l) => l,
        None => Location {
            source: snap.source.clone(),
            layout: snap.layout,
            dir: snap.save_dir.clone(),
        },
    };
    let r = restore::restore(&v, id, &loc, Running::now(), vault::now())?;
    say("info", &r.log_line());
    if let Some(b) = &r.before {
        println!("the save it replaced is snapshot {b}");
    }
    println!("restored {id} into {}", r.dir.display());
    if !r.kept.is_empty() {
        println!("left as they are (not in the snapshot): {}", r.kept.join(", "));
    }
    if loc.layout.is_launcher() {
        println!("the launcher saves are restored; Steam's saves are not touched");
    }
    if loc.layout == vault::Layout::Steam {
        println!(
            "start Steam to upload the restored save; if it asks which copy to keep, keep the local files (this computer)"
        );
    }
    Ok(())
}

fn profile_arg(store: &Store, profile: Option<String>) -> Result<String> {
    let p = profile.unwrap_or_else(|| store.active_profile());
    let loaded = store.profile(&p)?;
    if loaded.is_vanilla() {
        bail!("{p} plays without the Swofty core and always uses Steam's saves");
    }
    Ok(p)
}

fn import_cmd(
    store: &Store,
    from: Option<String>,
    from_profile: Option<String>,
    profile: Option<String>,
) -> Result<()> {
    let profile = profile_arg(store, profile)?;
    let (src, reason) = match (from.as_deref(), from_profile) {
        (Some(other), _) if other != "steam" => {
            bail!("--from takes steam; use --from-profile for another profile")
        }
        (_, Some(p)) => {
            if p == profile {
                bail!("{p} is the profile to import into");
            }
            store.profile(&p)?;
            (launcher_saves::location(&store.root, &p), Reason::ImportedProfile)
        }
        _ => match Location::active(&store.root) {
            Some(l) if l.layout == vault::Layout::Steam => (l, Reason::Imported),
            Some(_) => bail!("{}", launcher_saves::EA_NOTE),
            None => bail!("no Steam save of F1 25 found to import"),
        },
    };
    let (files, bytes, snapshot) =
        launcher_saves::import(&store.root, &profile, &src, reason, Running::now(), vault::now())?;
    say(
        "info",
        &format!(
            "saves: imported {files} file(s) from {} into the launcher saves of profile {profile}",
            src.dir.display()
        ),
    );
    println!(
        "imported {files} file(s), {bytes} bytes, from {} into {}",
        src.dir.display(),
        launcher_saves::remote_dir(&store.root, &profile).display()
    );
    if let Some(s) = snapshot {
        println!("the vault holds it as snapshot {s}");
    }
    Ok(())
}

fn export_cmd(store: &Store, to: &str, profile: Option<String>) -> Result<()> {
    if to != "steam" {
        bail!("--to takes steam");
    }
    let profile = profile_arg(store, profile)?;
    let steam = match Location::active(&store.root) {
        Some(l) if l.layout == vault::Layout::Steam => l,
        Some(_) => bail!("{}", launcher_saves::EA_NOTE),
        None => bail!("no Steam save folder of F1 25 found to export to"),
    };
    let r = launcher_saves::export(&store.root, &profile, &steam, Running::now(), vault::now())?;
    say("info", &r.log_line());
    if let Some(b) = &r.before {
        println!("the Steam save it replaced is snapshot {b}");
    }
    println!(
        "exported the launcher saves of profile {profile} into {}",
        r.dir.display()
    );
    println!(
        "start Steam to upload the exported save; if it asks which copy to keep, keep the local files (this computer)"
    );
    Ok(())
}

pub fn command(store: &Store, cmd: SavesCommand) -> Result<ExitCode> {
    match cmd {
        SavesCommand::Import {
            from,
            from_profile,
            profile,
        } => import_cmd(store, from, from_profile, profile)?,
        SavesCommand::Export { to, profile } => export_cmd(store, &to, profile)?,
        SavesCommand::List { source } => list(store, source.as_deref())?,
        SavesCommand::Show { snapshot, source } => show(store, &snapshot, source.as_deref())?,
        SavesCommand::Versions { source } => versions(store, source.as_deref())?,
        SavesCommand::Careers { source } => careers(store, source.as_deref())?,
        SavesCommand::Pin { snapshot, source } => pin_cmd(store, &snapshot, source.as_deref(), true)?,
        SavesCommand::Unpin { snapshot, source } => pin_cmd(store, &snapshot, source.as_deref(), false)?,
        SavesCommand::Backup => backup(store)?,
        SavesCommand::Restore { snapshot, source } => restore_cmd(store, &snapshot, source.as_deref())?,
        SavesCommand::Cap { mb: None } => println!(
            "{} MB",
            store.config().vault_cap_mb.unwrap_or(vault::DEFAULT_CAP_MB)
        ),
        SavesCommand::Cap { mb: Some(n) } => {
            if n == 0 {
                bail!("the cap must be at least 1 MB");
            }
            let mut c = store.config();
            c.vault_cap_mb = Some(n);
            store.save_config(&c)?;
            println!("the career vault keeps at most {n} MB; the next snapshot prunes to it");
        }
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use swofty_core::GAME_EXE;
    use swofty_core::baseline::Identity;

    use super::*;

    const DATA: &str = "savegame@profile#NXDSMWW.BWW";
    const INDEX: &str = "savegame@profile#QKRHMYXE";

    struct Fixture {
        root: PathBuf,
        store: Store,
        game: Game,
        steam: Location,
    }

    fn fixture(name: &str) -> Fixture {
        let root = std::env::temp_dir().join(format!("swofty-cli-saves-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let dir = root.join("game");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(GAME_EXE), "exe").unwrap();
        let game = Game {
            identity: Identity::detect(&dir, Some(1)).unwrap(),
            dir,
            user_reg: None,
        };
        let remote = root.join("steam/userdata/843502855/3059520/remote");
        std::fs::create_dir_all(&remote).unwrap();
        std::fs::write(remote.join(DATA), [1u8; 900]).unwrap();
        std::fs::write(remote.join(INDEX), [7u8; 416]).unwrap();
        Fixture {
            store: Store::new(root.join("data")),
            steam: Location::steam(&remote).unwrap(),
            game,
            root,
        }
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }

    #[test]
    fn the_first_play_copies_the_steam_save_into_an_empty_store_and_plays_on_it() {
        let f = fixture("first");
        let b = before_session_at(
            &f.store,
            "default",
            &f.game,
            "s1",
            None,
            false,
            Some(f.steam.clone()),
        )
        .unwrap();
        let store = launcher_saves::remote_dir(&f.store.root, "default");
        assert_eq!(b.loc.layout, vault::Layout::Launcher);
        assert_eq!(b.loc.dir, store);
        assert_eq!(names(&store), [DATA, INDEX]);
        assert_eq!(std::fs::read(store.join(DATA)).unwrap(), vec![1u8; 900]);
        assert_eq!(names(&f.steam.dir), [DATA, INDEX]);
        let snaps = Vault::open(&f.store.root, "launcher-default")
            .unwrap()
            .list()
            .unwrap();
        assert!(
            snaps
                .iter()
                .any(|s| s.reason == Reason::Imported && s.layout == vault::Layout::Launcher)
        );
        assert!(snaps.iter().any(|s| s.reason == Reason::BeforeSession) || b.snapshot.is_some());
        std::fs::remove_dir_all(&f.root).ok();
    }

    #[test]
    fn a_store_with_files_is_not_reseeded() {
        let f = fixture("kept");
        let store = launcher_saves::remote_dir(&f.store.root, "default");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join(DATA), b"launcher career").unwrap();
        let b = before_session_at(
            &f.store,
            "default",
            &f.game,
            "s1",
            None,
            false,
            Some(f.steam.clone()),
        )
        .unwrap();
        assert_eq!(b.loc.layout, vault::Layout::Launcher);
        assert_eq!(names(&store), [DATA]);
        assert_eq!(std::fs::read(store.join(DATA)).unwrap(), b"launcher career");
        std::fs::remove_dir_all(&f.root).ok();
    }

    #[test]
    fn one_session_on_steam_saves_and_a_profile_with_the_setting_off_use_steam() {
        let f = fixture("steam");
        let b = before_session_at(
            &f.store,
            "default",
            &f.game,
            "s1",
            None,
            true,
            Some(f.steam.clone()),
        )
        .unwrap();
        assert_eq!(b.loc.layout, vault::Layout::Steam);
        assert!(!launcher_saves::remote_dir(&f.store.root, "default").exists());
        f.store
            .update_profile("default", |p| {
                p.launcher_saves = false;
                Ok(())
            })
            .unwrap();
        let b = before_session_at(
            &f.store,
            "default",
            &f.game,
            "s2",
            None,
            false,
            Some(f.steam.clone()),
        )
        .unwrap();
        assert_eq!(b.loc.layout, vault::Layout::Steam);
        assert!(!launcher_saves::remote_dir(&f.store.root, "default").exists());
        std::fs::remove_dir_all(&f.root).ok();
    }

    #[test]
    fn a_profile_with_launcher_saves_off_leaves_an_after_play_version_of_the_steam_save() {
        let f = fixture("after-steam");
        f.store
            .update_profile("default", |p| {
                p.launcher_saves = false;
                Ok(())
            })
            .unwrap();
        let b = before_session_at(
            &f.store,
            "default",
            &f.game,
            "s1",
            None,
            false,
            Some(f.steam.clone()),
        )
        .unwrap();
        std::fs::write(f.steam.dir.join(DATA), [9u8; 1200]).unwrap();
        after_session(Some(b));
        let snaps = Vault::of(&f.store.root, &f.steam).unwrap().list().unwrap();
        let last = snaps.iter().find(|s| s.reason == Reason::AfterSession).unwrap();
        assert_eq!(last.layout, vault::Layout::Steam);
        assert_eq!(last.session_id.as_deref(), Some("s1"));
        std::fs::remove_dir_all(&f.root).ok();
    }

    #[test]
    fn the_vanilla_profile_always_plays_on_steam_saves() {
        let f = fixture("vanilla");
        let b = before_session_at(
            &f.store,
            "vanilla",
            &f.game,
            "s1",
            None,
            false,
            Some(f.steam.clone()),
        )
        .unwrap();
        assert_eq!(b.loc.layout, vault::Layout::Steam);
        assert!(!launcher_saves::root(&f.store.root).exists());
        std::fs::remove_dir_all(&f.root).ok();
    }

    #[test]
    fn the_ea_copy_plays_on_the_launcher_saves_of_its_profile() {
        let f = fixture("ea");
        let ea_dir = f
            .root
            .join("pfx/drive_c/users/steamuser/AppData/Local/F1 25/savegame/ea/profile");
        std::fs::create_dir_all(&ea_dir).unwrap();
        std::fs::write(ea_dir.join("NXDSMWW.BWW"), [3u8; 64]).unwrap();
        let ea = Location::ea(&ea_dir, "steamuser");
        let b = before_session_at(&f.store, "default", &f.game, "s1", None, false, Some(ea)).unwrap();
        assert_eq!(b.loc.layout, vault::Layout::LauncherEa);
        let stored = launcher_saves::ea_dir(&f.store.root, "default");
        assert_eq!(std::fs::read(stored.join("NXDSMWW.BWW")).unwrap(), [3u8; 64]);
        std::fs::remove_dir_all(&f.root).ok();
    }
}
