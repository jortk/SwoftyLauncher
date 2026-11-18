use std::path::Path;

use swofty_core::vault::careers::{Brief, Listed, Row};
use swofty_core::vault::restore::Running;
use swofty_core::vault::{self, Reason, Snapshot};
use swofty_i18n::{tr, trf, trn};

use super::state::Imported;

pub(super) fn row_name(r: &Row) -> String {
    let mut out = r.name().to_owned();
    if let Some(p) = r.place() {
        out.push_str(&format!(", {p}"));
    }
    out
}

pub(super) fn list(items: &[Brief]) -> String {
    items
        .iter()
        .map(|b| match &b.place {
            Some(p) => format!("{} ({p})", b.name),
            None => b.name.clone(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn version_label(s: &Snapshot) -> &'static str {
    match s.pinned.iter().any(|p| p == "before-restore") {
        true => tr("Before restore"),
        false => reason_label(s.reason),
    }
}

pub(super) fn reason_label(r: Reason) -> &'static str {
    match r {
        Reason::BeforeSession => tr("Before Play"),
        Reason::AfterSession => tr("After Play"),
        Reason::Imported => tr("Imported from Steam"),
        Reason::ImportedProfile => tr("Copied from another profile"),
        Reason::LauncherStart => tr("Launcher start"),
        Reason::ChangedOutside => tr("Changed outside the launcher"),
        Reason::ForeignLaunch => tr("F1 25 started outside the launcher"),
        Reason::ForeignExit => tr("F1 25 closed outside the launcher"),
        Reason::BeforeRestore => tr("Before restore"),
        Reason::Restored => tr("Restored"),
        Reason::Migrated => tr("Old backup"),
        Reason::Manual => tr("Backup"),
        Reason::Other => tr("Saved"),
    }
}

pub(super) fn careers_held_line(count: usize, newest: Option<&str>, missing: usize) -> String {
    let mut out = trn(count, "{count} career", "{count} careers");
    if let Some(name) = newest {
        out.push_str(&trf!(", newest: {name}", name = name));
    }
    if missing > 0 {
        out.push_str(&trf!(", {missing} marked missing", missing = missing));
    }
    out
}

pub(super) fn missing_warning(missing: &[Brief]) -> String {
    trf!(
        "This version lists {careers} as missing, so restoring it does not bring them back.",
        careers = list(missing)
    )
}

pub(super) fn imported_from(snaps: &[Snapshot]) -> Imported {
    let found = snaps
        .iter()
        .find(|s| matches!(s.reason, Reason::Imported | Reason::ImportedProfile));
    let title = match found {
        Some(s) if s.reason == Reason::ImportedProfile => {
            let from = s
                .note
                .as_deref()
                .and_then(|n| n.strip_prefix("copied from "))
                .and_then(|d| Path::new(d).parent())
                .and_then(|d| d.file_name())
                .map(|n| n.to_string_lossy().into_owned());
            match from {
                Some(f) => trf!("Copied from {profile}", profile = f),
                None => tr("Copied from another profile").to_owned(),
            }
        }
        _ => tr("Imported Steam save").to_owned(),
    };
    Imported {
        title,
        at: found.map(|s| s.at),
    }
}

pub(super) fn season_round(l: &Listed) -> Option<String> {
    match (l.season_no, l.weekend) {
        (Some(s), Some(w)) => Some(trf!("Season {season} · Round {round}", season = s, round = w + 1)),
        (Some(s), None) => Some(trf!("Season {season}", season = s)),
        _ => None,
    }
}

pub(super) fn short_block(layout: vault::Layout, r: Running) -> Option<&'static str> {
    match (r.game, layout, r.steam) {
        (true, _, _) => Some(tr("Close F1 25 first")),
        (false, vault::Layout::Steam, true) => Some(tr("Close Steam first")),
        _ => None,
    }
}

pub(crate) fn notice_title() -> &'static str {
    tr("Your careers have their own saves now")
}

pub(crate) fn notice_body() -> &'static str {
    tr(
        "Each profile now has its own save, starting from a copy of your Steam save. Playing from Steam still uses your Steam save, and Vanilla plays on it too.",
    )
}

pub(crate) fn empty_save() -> &'static str {
    tr("This profile's launcher save is empty. Import from Steam brings your careers in.")
}

pub(crate) fn empty_save_short() -> &'static str {
    tr("Launcher save is empty · import from Steam on the Saves page")
}

pub(crate) fn steam_save_missing_text() -> &'static str {
    tr("Couldn't find your Steam save, so this profile's launcher saves are empty.")
}

pub(crate) fn listed_later() -> &'static str {
    tr("careers listed after you play this profile once")
}

pub(super) fn close_game_first() -> &'static str {
    tr("Close F1 25 first")
}
