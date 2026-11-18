use std::path::Path;

use swofty_core::build_support;
use swofty_i18n::{tr, trf};

pub(crate) struct Notice {
    pub headline: String,
    pub detail: &'static str,
}

pub(crate) fn notice(exe: &Path) -> Option<Notice> {
    notice_for(build_support::blocked(exe)?)
}

fn notice_for(version: String) -> Option<Notice> {
    Some(Notice {
        headline: trf!(
            "This game version ({version}) isn't supported by Swofty Launcher yet.",
            version = version
        ),
        detail: tr("Add-ons are off until a launcher update, so the game runs without them."),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unsupported_build_names_its_version_and_says_add_ons_are_off() {
        let n = notice_for("1.0.145.2472".into()).unwrap();
        assert_eq!(
            n.headline,
            "This game version (1.0.145.2472) isn't supported by Swofty Launcher yet."
        );
        assert_eq!(
            n.detail,
            "Add-ons are off until a launcher update, so the game runs without them."
        );
    }

    #[test]
    fn a_missing_exe_has_no_notice() {
        assert!(notice(Path::new("/no/such/F1_25.exe")).is_none());
    }
}
