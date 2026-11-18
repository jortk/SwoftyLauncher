use std::path::Path;

use swofty_core::profile::VANILLA;
use swofty_core::vault::{self, Notice, Vault};
use swofty_i18n::{fill, tr, trf, trn};

use super::labels::row_name;

pub(crate) struct Missing {
    pub(super) profile: String,
    pub(super) source: String,
    pub(super) snapshot: String,
    pub(super) kept_at: i64,
    pub(super) names: Vec<String>,
}

pub(crate) enum Banner {
    Dropped(Missing),
    Lost(Missing),
    Hint(Notice),
}

pub(super) struct Shown {
    pub(super) text: String,
    pub(super) source: String,
    pub(super) snapshot: String,
    pub(super) kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Dropped,
    Lost,
    Hint,
}

impl Banner {
    pub(crate) fn lost(&self) -> Option<(&[String], &str, &str, i64)> {
        match self {
            Self::Lost(m) => Some((&m.names, &m.source, &m.snapshot, m.kept_at)),
            _ => None,
        }
    }

    pub(super) fn shown(&self) -> Shown {
        match self {
            Self::Dropped(m) => Shown {
                text: fill(
                    &trn(
                        m.names.len(),
                        "{count} career is missing from {whose}: {names}. It was last in the save on {when}. Restoring that version brings it back; the restore shows what else changes first.",
                        "{count} careers are missing from {whose}: {names}. They were last in the save on {when}. Restoring that version brings them back; the restore shows what else changes first.",
                    ),
                    &[
                        ("whose", whose(&m.profile)),
                        ("names", m.names.join("; ")),
                        ("when", vault::human(m.kept_at)),
                    ],
                ),
                source: m.source.clone(),
                snapshot: m.snapshot.clone(),
                kind: Kind::Dropped,
            },
            Self::Lost(m) => Shown {
                text: fill(
                    &trn(
                        m.names.len(),
                        "{count} addon career may have been removed from your Steam save when F1 25 started without the launcher: {names}. The last version that held it is from {when}.",
                        "{count} addon careers may have been removed from your Steam save when F1 25 started without the launcher: {names}. The last version that held them is from {when}.",
                    ),
                    &[("names", m.names.join("; ")), ("when", vault::human(m.kept_at))],
                ),
                source: m.source.clone(),
                snapshot: m.snapshot.clone(),
                kind: Kind::Lost,
            },
            Self::Hint(n) => Shown {
                text: n.headline(),
                source: n.source.clone(),
                snapshot: n.snapshot.clone(),
                kind: Kind::Hint,
            },
        }
    }
}

pub(super) fn whose(profile: &str) -> String {
    match profile == VANILLA {
        true => tr("your Steam save").to_owned(),
        false => trf!("profile {profile}'s save", profile = profile),
    }
}

pub(super) fn banner_from(root: &Path, source: String, profile: String) -> Option<Banner> {
    let v = Vault::open(root, &source).ok()?;
    let table = v.careers();
    let shown = table.drop_to_show();
    if let Some(newest) = shown.iter().max_by_key(|r| r.last_seen_at) {
        return Some(Banner::Dropped(Missing {
            profile,
            source,
            snapshot: newest.last_seen.clone(),
            kept_at: newest.last_seen_at,
            names: shown.iter().map(|r| row_name(r)).collect(),
        }));
    }
    if let Some(loss) = v.probable_loss() {
        return Some(Banner::Lost(Missing {
            profile,
            source,
            snapshot: loss.snapshot.clone(),
            kept_at: loss.at,
            names: loss.names(),
        }));
    }
    Notice::read(root).map(Banner::Hint)
}
