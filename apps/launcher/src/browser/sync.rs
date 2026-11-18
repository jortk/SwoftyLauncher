use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use gpui_kit::*;
use swofty_core::catalog::publishers::{self, Creator};
use swofty_core::catalog::stars::{self};
use swofty_core::catalog::{self, Addon, Trouble, Update};
use swofty_core::update::current_version;
use swofty_i18n::{tr, trf, trn};

use super::authors::View;
use super::*;
use crate::library::Source;
use crate::shell::Shell;

pub(super) struct Fetched {
    pub(super) addons: Option<Vec<Addon>>,
    pub(super) icons: Vec<(String, Arc<Image>)>,
    pub(super) error: Option<Trouble>,
    pub(super) creators: std::result::Result<Vec<Creator>, Trouble>,
    pub(super) avatars: Vec<(String, Arc<Image>)>,
}

pub(super) fn images<'a>(
    dir: &Path,
    urls: impl IntoIterator<Item = &'a String>,
    known: &BTreeSet<String>,
) -> Vec<(String, Arc<Image>)> {
    let mut wanted: Vec<&String> = urls
        .into_iter()
        .filter(|url| web_url(url) && !known.contains(*url))
        .collect();
    wanted.sort();
    wanted.dedup();
    wanted
        .into_iter()
        .filter_map(|url| crate::news::image(dir, url).map(|i| (url.clone(), i)))
        .collect()
}

pub(super) fn avatar_dir(root: &Path) -> std::path::PathBuf {
    root.join(catalog::CACHE_DIR).join(AVATARS_DIR)
}

pub(super) fn fetch(root: &Path, shown: Vec<Addon>, known: BTreeSet<String>) -> Fetched {
    let (addons, error) = match catalog::fetch(root) {
        Ok(addons) => (Some(addons), None),
        Err(e) => (None, Some(e)),
    };
    let creators = publishers::fetch(root);
    let listed = addons.as_ref().unwrap_or(&shown);
    let icons = images(
        &root.join(catalog::CACHE_DIR).join(ICONS_DIR),
        listed.iter().filter_map(|a| a.icon.as_ref()),
        &known,
    );
    let faces = listed
        .iter()
        .flat_map(|a| a.owner().into_iter().chain(&a.publishers))
        .filter_map(|p| p.avatar.as_ref())
        .chain(creators.iter().flatten().filter_map(|c| c.avatar.as_ref()));
    let avatars = images(&avatar_dir(root), faces, &known);
    Fetched {
        addons,
        icons,
        error,
        creators,
        avatars,
    }
}

impl Shell {
    pub(crate) fn watch_catalog(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let ticked = this.update(cx, |s, cx| {
                    if s.data_move_locked() {
                        return;
                    }
                    if s.browser.stale() {
                        s.fetch_catalog(cx);
                    } else {
                        s.sync_stars(cx);
                    }
                    cx.notify();
                });
                if ticked.is_err() {
                    break;
                }
            }
        })
    }

    pub(super) fn record(&mut self, op: Op, ok: bool, title: String, detail: String, took: Duration) {
        let mut line = format!(
            "{} {title} ({})",
            if ok { "ok" } else { "failed" },
            took_text(took)
        );
        if !detail.is_empty() {
            line.push_str(&format!(": {detail}"));
        }
        catalog::log(&self.store.root, &line);
        let b = &mut self.browser;
        b.activity.push_front(Activity {
            at: SystemTime::now(),
            op,
            ok,
            title,
            detail,
            took,
        });
        b.activity.truncate(ACTIVITY_KEPT);
    }

    pub(crate) fn fetch_catalog(&mut self, cx: &mut Context<Self>) {
        if self.browser.fetching {
            return;
        }
        self.browser.fetching = true;
        let root = self.store.root.clone();
        let shown = self.browser.addons.clone();
        let known = self
            .browser
            .icons
            .keys()
            .chain(self.browser.avatars.keys())
            .cloned()
            .collect();
        let started = Instant::now();
        cx.spawn(async move |this, cx| {
            let fetched = cx
                .background_executor()
                .spawn(async move { fetch(&root, shown, known) })
                .await;
            this.update(cx, |s, cx| {
                let took = started.elapsed();
                let from = host(&catalog::base_url()).to_owned();
                match (&fetched.error, &fetched.addons) {
                    (Some(e), _) => s.record(
                        Op::Catalog,
                        false,
                        trf!("Couldn't fetch the catalog from {host}", host = from),
                        e.to_string(),
                        took,
                    ),
                    (None, Some(addons)) => s.record(
                        Op::Catalog,
                        true,
                        tr("Fetched the catalog").to_owned(),
                        trf!(
                            "{addons} from {host}",
                            addons = trn(addons.len(), "{count} addon", "{count} addons"),
                            host = from
                        ),
                        took,
                    ),
                    (None, None) => {}
                }
                match &fetched.creators {
                    Ok(creators) => catalog::log(
                        &s.store.root,
                        &format!("ok fetched {} publishers from {from}", creators.len()),
                    ),
                    Err(e) => catalog::log(&s.store.root, &format!("failed to fetch the publishers: {e}")),
                }
                let b = &mut s.browser;
                if let Ok(creators) = fetched.creators {
                    b.creators = creators;
                }
                b.avatars.extend(fetched.avatars);
                b.fetching = false;
                b.checked = Some(Instant::now());
                b.trouble = fetched.error;
                if let Some(addons) = fetched.addons {
                    b.addons = addons;
                    b.live = true;
                    b.saved = true;
                    b.fetched_at = Some(SystemTime::now());
                }
                b.icons.extend(fetched.icons);
                s.sync_catalog_updates();
                s.sync_stars(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn sync_stars(&mut self, cx: &mut Context<Self>) {
        let b = &mut self.browser;
        if b.syncing {
            b.sync_again = true;
            return;
        }
        b.syncing = true;
        let sent = b.stars.outgoing();
        let client = b.stars.client.clone();
        let started = Instant::now();
        cx.spawn(async move |this, cx| {
            let job = sent.clone();
            let result = cx
                .background_executor()
                .spawn(async move { stars::sync(&client, &job) })
                .await;
            this.update(cx, |s, cx| {
                let took = started.elapsed();
                s.browser.syncing = false;
                match result {
                    Ok(reply) => {
                        let applied = reply.applied;
                        let b = &mut s.browser;
                        b.stars.synced(&sent, reply, catalog::iso_now());
                        b.catalog_revision = b.catalog_revision.wrapping_add(1);
                        let detail = trf!(
                            "{sent}, {applied} applied · {starred} starred · {user}",
                            sent = trn(sent.len(), "{count} change sent", "{count} changes sent"),
                            applied = applied,
                            starred = b.stars.starred.len(),
                            user = b.stars.user.as_deref().unwrap_or(tr("no user yet"))
                        );
                        s.record(Op::Stars, true, tr("Synced stars").to_owned(), detail, took);
                    }
                    Err(e) => {
                        s.browser.stars.failed(e.to_string());
                        let waiting = trn(
                            s.browser.stars.pending.len(),
                            "{count} change waiting",
                            "{count} changes waiting",
                        );
                        s.record(
                            Op::Stars,
                            false,
                            tr("Couldn't sync stars").to_owned(),
                            trf!("{error} · {waiting}", error = e, waiting = waiting),
                            took,
                        );
                    }
                }
                if let Err(e) = s.browser.stars.save(&s.store.root) {
                    eprintln!("swofty-launcher: could not save the stars: {e}");
                }
                if std::mem::take(&mut s.browser.sync_again) {
                    s.sync_stars(cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(super) fn toggle_star(&mut self, id: &str, cx: &mut Context<Self>) {
        let listed = self.browser.find(id).map_or(0, |a| a.stars);
        let on = self.browser.stars.toggle(id, listed, catalog::iso_now());
        if let Err(e) = self.browser.stars.save(&self.store.root) {
            eprintln!("swofty-launcher: could not save the stars: {e}");
        }
        catalog::log(
            &self.store.root,
            &format!(
                "{} {id}, {} waiting",
                if on { "starred" } else { "unstarred" },
                self.browser.stars.pending.len()
            ),
        );
        self.browser.catalog_revision = self.browser.catalog_revision.wrapping_add(1);
        self.browser.star_gen += 1;
        let ticket = self.browser.star_gen;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(STAR_DELAY).await;
            this.update(cx, |s, cx| {
                if s.browser.star_gen == ticket {
                    s.sync_stars(cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn sync_catalog_updates(&mut self) {
        // Catalog refreshes and library reloads both change cached filters and dependency previews.
        self.browser.catalog_revision = self.browser.catalog_revision.wrapping_add(1);
        self.browser.needs_cache.borrow_mut().clear();
        let installed = self
            .library
            .addons
            .iter()
            .filter(|a| !matches!(a.source, Source::Builtin | Source::Importer))
            .map(|a| &a.manifest);
        self.browser.updates = catalog::updates(installed, &self.browser.addons, &current_version());
    }

    pub(crate) fn catalog_update(&self, id: &str) -> Option<&Update> {
        self.browser.updates.iter().find(|u| u.id == id)
    }

    pub(super) fn installed_version(&self, id: &str) -> Option<semver::Version> {
        self.library
            .addons
            .iter()
            .find(|m| m.manifest.id == id)
            .map(|m| m.manifest.version.clone())
    }

    pub(super) fn tick_browser(&mut self, cx: &mut Context<Self>) {
        if self.browser.ticking {
            return;
        }
        self.browser.ticking = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(150)).await;
                let done = this
                    .update(cx, |s, cx| {
                        let done = s.browser.working.is_empty();
                        if done {
                            s.browser.ticking = false;
                        }
                        cx.notify();
                        done
                    })
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        })
        .detach();
    }

    pub(super) fn open_details(&mut self, id: &str, cx: &mut Context<Self>) {
        self.browse(View::Addon(id.to_owned()), cx);
    }

    pub(super) fn fetch_details(&mut self, id: &str, cx: &mut Context<Self>) {
        if !self.browser.loading.insert(id.to_owned()) {
            return;
        }
        let id = id.to_owned();
        let started = Instant::now();
        cx.spawn(async move |this, cx| {
            let job = id.clone();
            let result = cx
                .background_executor()
                .spawn(async move { catalog::fetch_one(&job) })
                .await;
            this.update(cx, |s, cx| {
                s.browser.loading.remove(&id);
                let took = started.elapsed();
                match result {
                    Ok(a) => {
                        let detail = trf!(
                            "{versions} · {trend}",
                            versions = trn(a.versions.len(), "{count} version", "{count} versions"),
                            trend = if a.trend.is_some() {
                                tr("30-day trend")
                            } else {
                                tr("no trend")
                            }
                        );
                        s.record(
                            Op::Details,
                            true,
                            trf!("Fetched {name}", name = a.name),
                            detail,
                            took,
                        );
                        s.load_media(&a, cx);
                        s.browser.details.insert(id, a);
                    }
                    Err(e) => s.record(
                        Op::Details,
                        false,
                        trf!("Couldn't fetch {id}", id = id),
                        e.to_string(),
                        took,
                    ),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}
