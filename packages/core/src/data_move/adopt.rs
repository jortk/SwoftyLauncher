//! Pointing the launcher at an addons-and-saves folder it found, instead of the
//! empty one it made. The empty folder is set aside, never deleted.
//!
//! [`adopt`] checks the folder and writes a journal; [`recover`](super::recover)
//! does the switch at the next start, before the log opens. A folder inside an
//! install folder is moved out to the default path instead of linked to, so an
//! update can never find the launcher's own folder holding it.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use super::{
    Error, Journal, Kind, Location, Ops, Recovered, Spot, copy, exists, history, link, spot, within,
};
use crate::process::same_path;

/// What [`adopt`] set up, for the player to read before the restart.
#[derive(Debug, Clone, PartialEq)]
pub struct Adoption {
    pub found: PathBuf,
    pub fresh: Option<PathBuf>,
    pub moves_out: bool,
    pub kept: Vec<String>,
}

pub fn adopt(
    loc: &Location,
    found: &Path,
    installs: &[PathBuf],
    bundled: &[String],
) -> Result<Adoption, Error> {
    if let Some(env) = &loc.env {
        return Err(Error::EnvSet(env.clone()));
    }
    if !found.is_absolute() {
        return Err(Error::NotAbsolute);
    }
    if super::journal_pending(&loc.default) {
        return Err(Error::Io(
            "A move of your addons and saves is not finished. Restart Swofty Launcher to finish it.".into(),
        ));
    }
    if super::find::looks_like_data(found, bundled).is_none() {
        return Err(Error::NoAddons(found.to_path_buf()));
    }
    let linked = link::target(&loc.default);
    let current = loc.current();
    if same_path(found, &current) || crate::copies::same_dir(found, &current) {
        return Err(Error::AlreadyThere(current));
    }
    if within(&loc.default, found) && !same_path(found, &loc.default) {
        return Err(Error::ContainsCurrent(loc.default.clone()));
    }
    if within(found, &loc.default) && !same_path(found, &loc.default) && linked.is_none() {
        return Err(Error::InsideCurrent(loc.default.clone()));
    }
    let moves_out = installs
        .iter()
        .chain(loc.install.iter())
        .any(|root| within(found, root) || same_path(found, root));
    if moves_out && found.parent().is_none() {
        return Err(Error::DriveRoot(found.to_path_buf()));
    }
    let fresh = (linked.is_none() && exists(&loc.default)).then(|| fresh_name(&loc.default));
    let kept = fresh
        .as_ref()
        .map(|_| unmatched(&loc.default, found, bundled))
        .unwrap_or_default();
    Journal {
        from: linked.clone().unwrap_or_else(|| loc.default.clone()),
        to: found.to_path_buf(),
        default: loc.default.clone(),
        linked: linked.is_some(),
        aside: fresh.clone().unwrap_or_else(|| fresh_name(&loc.default)),
        to_existed: false,
        kind: if moves_out { Kind::Relocate } else { Kind::Adopt },
    }
    .save()?;
    history::record(&loc.default, &current, "before using a found folder");
    history::record(&loc.default, found, "found folder");
    Ok(Adoption {
        found: found.to_path_buf(),
        fresh,
        moves_out,
        kept,
    })
}

fn unmatched(fresh: &Path, found: &Path, bundled: &[String]) -> Vec<String> {
    let here = super::find::looks_like_data(fresh, bundled);
    let there = super::find::looks_like_data(found, bundled);
    let ids = |f: &Option<super::find::Found>| -> Vec<String> {
        f.as_ref().map(|f| f.names.clone()).unwrap_or_default()
    };
    let there = ids(&there);
    ids(&here).into_iter().filter(|n| !there.contains(n)).collect()
}

pub(super) fn fresh_name(default: &Path) -> PathBuf {
    let name = default
        .file_name()
        .map_or_else(|| "swofty".to_owned(), |n| n.to_string_lossy().into_owned());
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let stamp = crate::send_logs::utc_stamp(secs);
    let base = format!("{name}.fresh-{}", &stamp[..13]);
    std::iter::once(default.with_file_name(&base))
        .chain((2..).map(|i| default.with_file_name(format!("{base}-{i}"))))
        .find(|p| p.symlink_metadata().is_err())
        .unwrap_or_else(|| default.with_file_name(base))
}

pub(super) fn switch(j: &Journal, ops: &Ops) -> Result<Recovered, Error> {
    match j.kind {
        Kind::Relocate => relocate(j, ops),
        _ => point(j, ops),
    }
}

fn clear_default(j: &Journal) -> Result<(), Error> {
    match spot(&j.default) {
        Spot::Folder if !exists(&j.aside) => std::fs::rename(&j.default, &j.aside)
            .map_err(|e| Error::io(format!("Could not set {} aside", j.default.display()), e)),
        Spot::Link(_) => link::remove(&j.default)
            .map_err(|e| Error::io(format!("Could not replace {}", j.default.display()), e)),
        _ => Ok(()),
    }
}

fn restore(j: &Journal, ops: &Ops) {
    if exists(&j.default) {
        return;
    }
    if exists(&j.aside) {
        std::fs::rename(&j.aside, &j.default).ok();
    } else if j.linked && exists(&j.from) {
        (ops.link)(&j.from, &j.default).ok();
    }
}

fn point(j: &Journal, ops: &Ops) -> Result<Recovered, Error> {
    if matches!(spot(&j.default), Spot::Link(t) if same_path(&t, &j.to)) {
        return Ok(done(j));
    }
    if !exists(&j.to) {
        return Err(Error::Io(format!("{} is missing.", j.to.display())));
    }
    clear_default(j)?;
    if let Some(parent) = j.default.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Err(e) = (ops.link)(&j.to, &j.default) {
        restore(j, ops);
        return Err(Error::io(
            format!("Could not point {} to {}", j.default.display(), j.to.display()),
            e,
        ));
    }
    Ok(done(j))
}

fn relocate(j: &Journal, ops: &Ops) -> Result<Recovered, Error> {
    if !exists(&j.to) {
        return match spot(&j.default) {
            Spot::Folder => Ok(done(j)),
            _ => Err(Error::Io(format!("{} is missing.", j.to.display()))),
        };
    }
    let started = j.linked && matches!(spot(&j.default), Spot::Folder)
        || !j.linked && exists(&j.aside) && matches!(spot(&j.default), Spot::Folder);
    if !started {
        clear_default(j)?;
    }
    if let Some(parent) = j.default.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let moved = if exists(&j.default) {
        None
    } else {
        (ops.rename)(&j.to, &j.default).ok()
    };
    if moved.is_none() {
        let never = AtomicBool::new(false);
        let copied = copy::sync(&j.to, &j.default, &never, &mut |_, _| {})
            .map_err(|e| match e {
                copy::Stop::Cancelled => Error::Io("The move was cancelled.".into()),
                copy::Stop::Io(e) => Error::Io(format!(
                    "Could not copy {} to {}: {e}",
                    j.to.display(),
                    j.default.display()
                )),
            })
            .and_then(|()| {
                copy::verify(&j.to, &j.default).map_err(|reason| Error::Mismatch {
                    from: j.to.clone(),
                    reason,
                })
            });
        if let Err(e) = copied {
            copy::remove(&j.default).ok();
            restore(j, ops);
            return Err(e);
        }
    }
    let left = exists(&j.to)
        .then(|| copy::remove(&j.to).err().map(|_| j.to.clone()))
        .flatten();
    let mut finished = done(j);
    finished.to = j.default.clone();
    finished.leftover = left;
    Ok(finished)
}

fn done(j: &Journal) -> Recovered {
    j.clear();
    Recovered {
        to: j.to.clone(),
        leftover: None,
        adopted: true,
    }
}
