use crate::scale::Hit;
use std::collections::BTreeSet;

use gpui_kit::assets::IconName;
use gpui_kit::component::Disableable;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::*;
use swofty_core::catalog::Addon;
use swofty_core::catalog::requires::{self, Have, Plan};
use swofty_core::update::current_version;
use swofty_i18n::{tr, trf};

use super::*;
use crate::library::Source;
use crate::shell::{Page, Shell};

impl Shell {
    pub(super) fn offer(&self, a: &Addon) -> Offer {
        if let Some(job) = self.browser.working.get(&a.id) {
            return Offer::Working(job.progress());
        }
        if self
            .browser
            .queue
            .jobs()
            .iter()
            .any(|job| job.id == a.id && job.state == crate::download_queue::State::Queued)
        {
            return Offer::Queued;
        }
        if self.browser.failed.contains_key(&a.id) {
            return Offer::Failed;
        }
        let installed = self
            .library
            .addons
            .iter()
            .find(|x| x.manifest.id == a.id)
            .map(|x| (x.source, x.manifest.version.clone()));
        let standing = standing(
            a.offered(&current_version()),
            self.installed_version(&a.id),
            || self.needs_of(a).1.is_some(),
        );
        browser_offer(standing, installed, self.update_skipped(&a.id))
    }

    /// What the Addon Browser says about the import that has the id of `a`.
    pub(super) fn separate_text(&self, a: &Addon) -> String {
        trf!(
            "This import has the same id as {name} in the Addon Browser; it is kept separate. Remove the import on the Addons page to install {name} from here.",
            name = a.name
        )
    }

    /// What installing the version of `a` this launcher would get also
    /// installs, from the requirements the registry lists; none when it doesn't
    /// list them all.
    pub(super) fn requirements(&self, a: &Addon) -> Option<swofty_core::Result<Plan>> {
        let launcher = current_version();
        let v = a.offered(&launcher)?;
        // An unknown root requirement list can never produce a complete preview.
        v.requires.as_ref()?;
        let builtin: BTreeSet<&str> = self
            .library
            .addons
            .iter()
            .filter(|x| x.source == Source::Builtin)
            .map(|x| x.manifest.id.as_str())
            .collect();
        let have = Have::new(self.library.addons.iter().map(|x| &x.manifest), |id| {
            builtin.contains(id)
        });
        requires::preview(a, v, &self.browser.addons, &have, &launcher)
    }

    /// The other addons installing `a` brings, as `Name 1.2.0`, and why it
    /// can't be installed when the registry's requirements already say.
    pub(super) fn needs_of(&self, a: &Addon) -> (Vec<String>, Option<String>) {
        if let Some(job) = self.browser.working.get(&a.id) {
            return (job.also(), None);
        }
        if self.installed_version(&a.id).is_some() {
            return (Vec::new(), None);
        }
        if let Some(cached) = self.browser.needs_cache.borrow().get(&a.id) {
            return cached.clone();
        }
        let needs = match self.requirements(a) {
            Some(Ok(plan)) => (plan.also().iter().map(|s| s.label()).collect(), None),
            Some(Err(e)) => (Vec::new(), Some(e.to_string())),
            None => (Vec::new(), None),
        };
        self.browser
            .needs_cache
            .borrow_mut()
            .insert(a.id.clone(), needs.clone());
        needs
    }

    /// A version's requirements as `Requires Name >=1.0.0, other.id`.
    pub(super) fn requires_text(&self, reqs: &[swofty_core::manifest::Requirement]) -> String {
        let named: Vec<String> = reqs
            .iter()
            .map(|r| {
                let name = self
                    .browser
                    .find(&r.id)
                    .map_or(r.id.as_str(), |a| a.name.as_str());
                match r.version == semver::VersionReq::STAR {
                    true => name.to_owned(),
                    false => format!("{name} {}", r.version),
                }
            })
            .collect();
        trf!("Requires {names}", names = named.join(", "))
    }

    pub(crate) fn offer_button(&self, a: &Addon, id: String, cx: &mut Context<Self>) -> Button {
        self.offer_button_as(a, id, tr("Install"), cx)
    }

    /// The install button, reading `install` while the addon can be installed.
    pub(super) fn offer_button_as(
        &self,
        a: &Addon,
        id: String,
        install: &str,
        cx: &mut Context<Self>,
    ) -> Button {
        let target = a.id.clone();
        let button = Button::new(SharedString::from(id)).tap_sm();
        let button = match self.offer(a) {
            Offer::Install => button
                .primary()
                .icon(IconName::Download)
                .label(install.to_owned()),
            Offer::Update(from, to) => {
                return button
                    .outline()
                    .icon(IconName::CircleArrowUp)
                    .label(trf!(
                        "Installed v{from} · v{to} in the play bar",
                        from = from,
                        to = to
                    ))
                    .disabled(true);
            }
            Offer::UpdateHere(from, to) => {
                let label = trf!("Installed v{from} · Update to v{to}", from = from, to = to);
                button.outline().icon(IconName::CircleArrowUp).label(label)
            }
            Offer::Separate(v) => {
                let tip = SharedString::from(self.separate_text(a));
                return button
                    .outline()
                    .icon(IconName::TriangleAlert)
                    .label(trf!("Your import v{version} has this id", version = v))
                    .tooltip(tip)
                    .disabled(true);
            }
            Offer::Failed => {
                return button
                    .danger()
                    .icon(IconName::RotateCw)
                    .label(tr("Failed – Retry"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.retry_download(&target, cx);
                    }));
            }
            Offer::Installed(v) => {
                return button
                    .outline()
                    .icon(IconName::Check)
                    .label(trf!("Installed v{version}", version = v))
                    .disabled(true);
            }
            Offer::TooOld => {
                return button
                    .outline()
                    .label(tr("Needs a newer launcher"))
                    .disabled(true);
            }
            Offer::Blocked => {
                return button
                    .outline()
                    .icon(IconName::TriangleAlert)
                    .label(tr("Missing requirements"))
                    .disabled(true);
            }
            Offer::Queued => {
                return button
                    .outline()
                    .label(tr("Queued · View downloads"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.open_page(Page::Downloads, cx);
                    }));
            }
            Offer::Working(progress) => {
                return button
                    .primary()
                    .loading(true)
                    .label(progress.short())
                    .disabled(true);
            }
        };
        button.on_click(cx.listener(move |this, _, _, cx| {
            cx.stop_propagation();
            this.install_from_catalog(&target, cx);
        }))
    }
}

/// What the Addon Browser offers for an addon that stands at `standing`, with the copy installed under
/// its id when there is one: an import keeps its id to itself, and the page itself updates a version
/// the player skipped, which the play bar doesn't.
pub(super) fn browser_offer(
    standing: Offer,
    installed: Option<(crate::library::Source, semver::Version)>,
    skipped: bool,
) -> Offer {
    use crate::library::Source;
    match (standing, installed) {
        (_, Some((Source::Importer, version))) => Offer::Separate(version),
        (Offer::Update(from, to), Some(_)) if skipped => Offer::UpdateHere(from, to),
        (offer, _) => offer,
    }
}

/// Where an addon stands for this launcher: `offered` is the newest version it can install, `installed`
/// the version it has, and `blocked` whether the requirements of `offered` can't all be met.
pub(super) fn standing(
    offered: Option<&catalog::Version>,
    installed: Option<semver::Version>,
    blocked: impl FnOnce() -> bool,
) -> Offer {
    match (offered, installed) {
        (Some(v), Some(m)) if v.version > m => Offer::Update(m, v.version.clone()),
        (_, Some(m)) => Offer::Installed(m),
        (Some(_), None) if blocked() => Offer::Blocked,
        (Some(_), None) => Offer::Install,
        (None, None) => Offer::TooOld,
    }
}
