//! What addons do over the game, from their manifests, and what a session's `loader.log` says
//! about the in-game overlay.

use crate::manifest::{AddonManifest, ExperimentalFeature};
use crate::store::Store;

/// What an addon does in game, from its manifest, as the Addons page tags it.
pub fn effects(m: &AddonManifest) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for f in &m.experimental_features {
        let e = f.effect();
        if !out.contains(&e) {
            out.push(e);
        }
    }
    out
}

pub fn draws_over_the_game(m: &AddonManifest) -> bool {
    m.experimental_features.contains(&ExperimentalFeature::Overlay)
}

/// The addons on in `profile` that draw over the game, as `(id, name)` in load order.
pub fn painters(store: &Store, profile: &str) -> Vec<(String, String)> {
    let (Ok(addons), Ok(p)) = (store.addons(), store.profile(profile)) else {
        return Vec::new();
    };
    p.enabled()
        .filter_map(|id| addons.get(id))
        .filter(|m| draws_over_the_game(m))
        .map(|m| (m.id.clone(), m.name.clone()))
        .collect()
}

/// What a session's `loader.log` says about the overlay, from its `[overlay]` lines.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionOverlay {
    pub painters: Vec<String>,
    pub armed: bool,
    pub drew: bool,
    pub frames: u64,
    pub target: Option<String>,
    pub off: Option<String>,
    pub removed: Option<String>,
    pub removal: Option<Removal>,
}

/// How long before the graphics device was lost the overlay last drew, as the core's
/// `the GPU was removed` line words it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    /// `4.8 s after the overlay last drew`: the overlay drew within the core's window.
    /// `at_present` when a Present the overlay drew into returned the removal (`present
    /// 0x887a0005`); `present 0x00000000` is the core finding the device already gone, before a
    /// draw or as the game aborts.
    AfterDraw { at_present: bool },
    /// `more than 10 s after the overlay last drew`.
    LongAfterDraw,
    /// `and the overlay had not drawn`.
    NeverDrew,
}

impl Removal {
    /// `rest` is the line from `the GPU was removed` on. `None` for a wording this doesn't know.
    fn parse(rest: &str) -> Option<Self> {
        let (_, said) = rest.split_once(") ")?;
        if said.starts_with("and the overlay had not drawn") {
            return Some(Self::NeverDrew);
        }
        let (age, _) = said.split_once(" s after the overlay last drew")?;
        if age.starts_with("more than ") {
            return Some(Self::LongAfterDraw);
        }
        let at_present = rest
            .split_once("(present 0x")
            .and_then(|(_, hr)| u32::from_str_radix(hr.get(..8)?, 16).ok())
            .is_some_and(|hr| hr != 0);
        age.parse::<f64>().ok().map(|_| Self::AfterDraw { at_present })
    }
}

const TAG: &str = "[overlay] ";

impl SessionOverlay {
    /// `None` when the log has no `[overlay]` line.
    pub fn scan(log: &str) -> Option<Self> {
        let mut s = Self::default();
        let mut any = false;
        for line in log.lines() {
            let Some(at) = line.find(TAG) else {
                continue;
            };
            any = true;
            let rest = &line[at + TAG.len()..];
            if let Some(api) = rest.strip_prefix("api: ") {
                if let Some((id, _)) = api.split_once(" on_paint -> ok")
                    && !id.is_empty()
                    && !s.painters.iter().any(|p| p == id)
                {
                    s.painters.push(id.to_owned());
                }
            } else if rest.starts_with("present hook armed") {
                s.armed = true;
            } else if let Some(up) = rest.strip_prefix("renderer up: ") {
                s.drew = true;
                s.target = Some(up.trim().to_owned());
            } else if rest.starts_with("first overlay frame submitted") {
                s.drew = true;
            } else if let Some(on) = rest.strip_prefix("drawing on swapchain ") {
                s.armed = true;
                s.target.get_or_insert_with(|| clip(on, 120));
            } else if let Some(n) = drawn_frames(rest) {
                s.frames += n;
                s.drew |= n > 0;
            } else if rest.starts_with("off: ")
                || rest.starts_with("the overlay feature is experimental and off")
            {
                s.off = Some(clip(rest.strip_prefix("off: ").unwrap_or(rest), 200));
            } else if rest.starts_with("the GPU was removed") {
                s.removed = Some(clip(rest, 200));
                s.removal = Removal::parse(rest);
            }
        }
        any.then_some(s)
    }

    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(off) = &self.off {
            parts.push(format!("off ({off})"));
        } else if self.drew {
            let mut drew = "drew".to_owned();
            if self.frames > 0 {
                drew.push_str(&format!(" {} frame(s)", self.frames));
            }
            if let Some(t) = &self.target {
                drew.push_str(&format!(" on {t}"));
            }
            parts.push(drew);
        } else if self.armed {
            parts.push("armed, never drew".to_owned());
        } else {
            parts.push("did not arm".to_owned());
        }
        if !self.painters.is_empty() {
            parts.push(format!("on_paint by {}", self.painters.join(", ")));
        }
        if let Some(r) = &self.removed {
            parts.push(r.clone());
        }
        parts.join("; ")
    }
}

fn drawn_frames(rest: &str) -> Option<u64> {
    let tail = rest.strip_prefix("last ")?;
    let (_, after) = tail.split_once(": ")?;
    let (n, more) = after.split_once(' ')?;
    more.starts_with("frame(s) drawn").then(|| n.parse().ok())?
}

fn clip(s: &str, max: usize) -> String {
    let s = s.trim();
    match s.char_indices().nth(max) {
        Some((at, _)) => format!("{}…", &s[..at]),
        None => s.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const V150: &str = "\
2026-10-24 02:37:54.717 [31308] [assets] resources: 104 prebuilt overlay ERP(s) in swofty/assets.json
2026-10-24 02:37:56.256 [31308] [overlay] present hook armed: Present (vt+0x40), ResizeBuffers (vt+0x68), ExecuteCommandLists (vt+0x50) on the game's D3D12 swapchain
2026-10-24 02:38:01.760 [31308] [overlay] api: kizi.pitlane on_paint -> ok (token 1)
2026-10-24 02:38:01.800 [31308] [overlay] api: kizi.canned on_paint -> ok (token 2)
2026-10-24 02:38:01.800 [31308] [overlay] api: kizi.canned on_paint -> ok (token 3)
2026-10-24 02:38:06.370 [35760] [overlay] captured the game's direct command queue 00000000D1AE5A30
2026-10-24 02:38:06.753 [31308] [overlay] renderer up: 2560x1440, 4 buffer(s), format 24
";

    #[test]
    fn picture_features_share_one_effect_tag() {
        let m = AddonManifest::parse(
            r#"{ "schema_version": 1, "id": "a.fx", "name": "Fx", "version": "1.0.0", "author": "a",
                 "experimental_features": ["shaders", "postfx", "overlay"], "compat": { "game": "f1_25" } }"#,
        )
        .unwrap();
        assert_eq!(effects(&m), ["Changes the picture", "Draws over the game"]);
        assert!(draws_over_the_game(&m));
    }

    #[test]
    fn a_150_session_that_drew_names_its_painters() {
        let s = SessionOverlay::scan(V150).unwrap();
        assert!(s.armed && s.drew);
        assert_eq!(s.painters, ["kizi.pitlane", "kizi.canned"]);
        assert_eq!(
            s.describe(),
            "drew on 2560x1440, 4 buffer(s), format 24; on_paint by kizi.pitlane, kizi.canned"
        );
    }

    #[test]
    fn frame_counts_and_the_removal_line_are_read() {
        let log = "\
1 [overlay] api: kizi.pitlane on_paint -> ok (token 1)
2 [overlay] drawing on swapchain 0000000012345678 with the DIRECT queue 00000000AAAA (2560x1440, format 24, 3 buffer(s))
3 [overlay] first overlay frame submitted: 96 vertices
4 [overlay] last 60 s: 3600 frame(s) drawn, 2 skipped while the GPU still held the slot
5 [overlay] the GPU was removed (present 0x887a0005, reason 0x887a0006) 4.8 s after the overlay last drew; the overlay stops drawing
";
        let s = SessionOverlay::scan(log).unwrap();
        assert!(s.drew);
        assert_eq!(s.frames, 3600);
        assert!(
            s.removed
                .as_deref()
                .unwrap()
                .starts_with("the GPU was removed (present 0x887a0005")
        );
        assert!(
            s.describe().starts_with("drew 3600 frame(s) on 0000000012345678"),
            "{}",
            s.describe()
        );
    }

    #[test]
    fn the_removal_line_says_whether_the_overlay_had_just_drawn() {
        let at_present = SessionOverlay::scan(
            "1 [overlay] the GPU was removed (present 0x887a0005, reason 0x887a0006) 4.8 s after the overlay last \
             drew; the overlay stops drawing\n",
        )
        .unwrap()
        .removal;
        assert_eq!(at_present, Some(Removal::AfterDraw { at_present: true }));
        let removal = |said: &str| {
            let log = format!(
                "1 [overlay] the GPU was removed (present 0x00000000, reason 0x887a0006) {said}; the overlay stops \
                 drawing\n"
            );
            let s = SessionOverlay::scan(&log).unwrap();
            assert!(s.removed.is_some());
            s.removal
        };
        assert_eq!(
            removal("4.8 s after the overlay last drew"),
            Some(Removal::AfterDraw { at_present: false })
        );
        assert_eq!(
            removal("0.1 s after the overlay last drew (swapchain by sl.interposer.dll <- F1_25.exe)"),
            Some(Removal::AfterDraw { at_present: false })
        );
        assert_eq!(
            removal("more than 10 s after the overlay last drew"),
            Some(Removal::LongAfterDraw)
        );
        assert_eq!(removal("and the overlay had not drawn"), Some(Removal::NeverDrew));
        assert_eq!(removal("at some point"), None);
    }

    #[test]
    fn an_overlay_feature_left_off_says_so() {
        let log =
            "1 [overlay] the overlay feature is experimental and off; the HUD present hook is not armed\n";
        let s = SessionOverlay::scan(log).unwrap();
        assert!(!s.drew);
        assert!(
            s.describe()
                .starts_with("off (the overlay feature is experimental and off")
        );
    }

    #[test]
    fn erp_overlay_lines_are_not_the_overlay() {
        assert_eq!(
            SessionOverlay::scan("1 [assets] resources: overlay ERP swofty/assets/resources/p2.erp\n"),
            None
        );
        let armed = SessionOverlay::scan("1 [overlay] present hook armed: Present (vt+0x40)\n").unwrap();
        assert_eq!(armed.describe(), "armed, never drew");
    }
}
