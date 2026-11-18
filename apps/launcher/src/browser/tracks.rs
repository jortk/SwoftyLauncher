//! What the Time Trial pages ask the Addon Browser about a board's track: who made it, and a button
//! that downloads it when it isn't installed.

use gpui_kit::component::button::Button;
use gpui_kit::*;
use swofty_core::catalog::Addon;
use swofty_core::timetrial::boards::{self, Byline, Listed};
use swofty_i18n::{tr, trf};

use super::*;
use crate::shell::Shell;

/// Whether a board shows its track's download button for where the addon stands: until it is
/// installed, and only when this launcher can install a version of it.
pub(super) fn offers_track(offer: &Offer) -> bool {
    !matches!(
        offer,
        Offer::Installed(_) | Offer::Update(..) | Offer::UpdateHere(..) | Offer::Separate(_) | Offer::TooOld
    )
}

/// A byline as a track list shows it: `by Twilight Sparkle`, with the addon's id after it when that is
/// what tells two tracks apart.
pub(super) fn byline_text(b: &Byline) -> String {
    match (&b.author, &b.addon) {
        (Some(author), Some(id)) => format!("{} \u{b7} {id}", trf!("by {name}", name = author)),
        (Some(author), None) => trf!("by {name}", name = author),
        (None, Some(id)) => id.clone(),
        (None, None) => String::new(),
    }
}

impl Shell {
    /// Who made the addon `id`: the registry's author, else the installed addon's.
    pub(crate) fn addon_author(&self, id: &str) -> Option<String> {
        let listed = self.browser.find(id).map(Addon::author_name);
        let installed = self
            .library
            .addons
            .iter()
            .find(|x| x.manifest.id == id)
            .map(|x| x.manifest.author.as_str());
        listed
            .into_iter()
            .chain(installed)
            .map(str::trim)
            .find(|a| !a.is_empty())
            .map(str::to_owned)
    }

    /// The byline of each `(name, addon)` track, told apart from the others in the list.
    pub(crate) fn track_bylines(&self, tracks: &[(&str, Option<&str>)]) -> Vec<Option<String>> {
        let authors: Vec<Option<String>> = tracks
            .iter()
            .map(|(_, addon)| addon.and_then(|a| self.addon_author(a)))
            .collect();
        let listed: Vec<Listed<'_>> = tracks
            .iter()
            .zip(&authors)
            .map(|((name, addon), author)| Listed {
                name,
                addon: *addon,
                author: author.as_deref(),
            })
            .collect();
        boards::bylines(&listed)
            .iter()
            .map(|b| b.as_ref().map(byline_text))
            .collect()
    }

    /// The button that downloads a board's track addon through the Addon Browser, into the current
    /// profile. None when the registry doesn't list the addon, when it is installed, or when this
    /// launcher can't install it.
    pub(crate) fn track_download_button(
        &self,
        addon: &str,
        id: &str,
        cx: &mut Context<Self>,
    ) -> Option<Button> {
        let a = self.browser.find(addon)?;
        offers_track(&self.offer(a)).then(|| self.offer_button_as(a, id.to_owned(), tr("Download track"), cx))
    }
}
