use swofty_core::install::Source;
use swofty_core::launcher_saves::{self, Area, Choice, Why};
use swofty_core::profile::Profile;
use swofty_core::vault::Layout;
use swofty_i18n::{tr, trf};

pub(crate) struct Line {
    pub headline: String,
    pub reason: Option<&'static str>,
    pub warn: bool,
}

pub(crate) fn line(
    profile: &Profile,
    source: Source,
    build_blocked: Option<&str>,
    steam_saves_once: bool,
) -> Line {
    let ea = source == Source::EaApp;
    let area = launcher_saves::area(Choice {
        build_blocked,
        vanilla: profile.is_vanilla(),
        profile_uses_launcher_saves: profile.launcher_saves,
        steam_saves_once,
        layout: if ea { Layout::Ea } else { Layout::Steam },
    });
    match area {
        Area::Launcher => Line {
            headline: trf!("Saves: {profile} launcher save", profile = profile.name),
            reason: None,
            warn: false,
        },
        Area::Own(why) => Line {
            headline: match ea {
                true => tr("Saves: the EA app's own save (your launcher careers are not used on this run)"),
                false => tr("Saves: Steam's own save (your launcher careers are not used on this run)"),
            }
            .to_owned(),
            reason: Some(match why {
                Why::UnsupportedBuild(_) => tr("this game version is not supported yet"),
                Why::Vanilla => tr("Vanilla profile"),
                Why::LauncherSavesOff => tr("launcher saves are off"),
                Why::EaCopy => tr("launcher saves are not available on the EA app copy"),
                Why::SteamOnce => tr("you chose to play once with your Steam saves"),
            }),
            warn: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEAM_OWN: &str = "Saves: Steam's own save (your launcher careers are not used on this run)";

    fn profile(name: &str) -> Profile {
        Profile::new(name)
    }

    #[test]
    fn a_profile_on_launcher_saves_names_its_save_neutrally() {
        let l = line(&profile("grid"), Source::Steam, None, false);
        assert_eq!(l.headline, "Saves: grid launcher save");
        assert_eq!(l.reason, None);
        assert!(!l.warn);
    }

    #[test]
    fn the_vanilla_profile_plays_on_the_steam_save() {
        let l = line(&Profile::vanilla(), Source::Steam, None, false);
        assert_eq!(
            (l.headline.as_str(), l.reason, l.warn),
            (STEAM_OWN, Some("Vanilla profile"), true)
        );
    }

    #[test]
    fn launcher_saves_off_plays_on_the_steam_save() {
        let mut p = profile("grid");
        p.launcher_saves = false;
        let l = line(&p, Source::Steam, None, false);
        assert_eq!(
            (l.headline.as_str(), l.reason, l.warn),
            (STEAM_OWN, Some("launcher saves are off"), true)
        );
    }

    #[test]
    fn an_unsupported_build_plays_on_the_steam_save_whatever_the_profile() {
        for p in [profile("grid"), Profile::vanilla()] {
            let l = line(&p, Source::Steam, Some("1.0.145.2472"), false);
            assert_eq!(
                (l.headline.as_str(), l.reason, l.warn),
                (STEAM_OWN, Some("this game version is not supported yet"), true)
            );
        }
    }

    #[test]
    fn play_once_with_steam_saves_plays_on_the_steam_save() {
        let l = line(&profile("grid"), Source::Steam, None, true);
        assert_eq!(
            (l.headline.as_str(), l.reason, l.warn),
            (
                STEAM_OWN,
                Some("you chose to play once with your Steam saves"),
                true
            )
        );
    }

    #[test]
    fn the_ea_app_copy_uses_the_launcher_save_of_its_profile() {
        let l = line(&profile("grid"), Source::EaApp, None, false);
        assert_eq!(l.headline, "Saves: grid launcher save");
        assert_eq!(l.reason, None);
        assert!(!l.warn);
    }

    #[test]
    fn the_area_is_the_one_the_cli_acts_on() {
        let choice = |blocked, vanilla, on, once, layout| Choice {
            build_blocked: blocked,
            vanilla,
            profile_uses_launcher_saves: on,
            steam_saves_once: once,
            layout,
        };
        assert_eq!(
            launcher_saves::area(choice(None, false, true, false, Layout::Steam)),
            Area::Launcher
        );
        assert_eq!(
            launcher_saves::area(choice(Some("9"), false, true, false, Layout::Steam)),
            Area::Own(Why::UnsupportedBuild("9".into()))
        );
    }
}
