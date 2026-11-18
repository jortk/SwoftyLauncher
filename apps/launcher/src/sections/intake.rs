use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::notices::{Notification, Notify};
use crate::scale::px;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::{Icon, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::import::{self, Route};
use swofty_core::profile::Profile;
use swofty_core::registry::Registry;
use swofty_core::store::Store;
use swofty_i18n::{tr, trf, trn};

use super::*;
use crate::game::Install;
use crate::pages::badge;
use crate::shell::{Page, Shell};
use crate::theme::Palette;

// When each installed addon's folder last changed, by addon id.
pub(super) fn installed(store: &Store) -> BTreeMap<String, SystemTime> {
    let Ok(dirs) = std::fs::read_dir(store.addons_dir()) else {
        return BTreeMap::new();
    };
    dirs.flatten()
        .filter(|e| e.path().join(swofty_core::manifest::MANIFEST_FILE).is_file())
        .filter_map(|e| {
            Some((
                e.file_name().into_string().ok()?,
                e.metadata().ok()?.modified().ok()?,
            ))
        })
        .collect()
}

// The addons installed or replaced between two snapshots, newest first.
pub(super) fn installed_since(
    before: &BTreeMap<String, SystemTime>,
    after: &BTreeMap<String, SystemTime>,
) -> Vec<String> {
    let mut fresh: Vec<(&String, &SystemTime)> = after
        .iter()
        .filter(|(id, at)| before.get(*id) != Some(at))
        .collect();
    fresh.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    fresh.into_iter().map(|(id, _)| id.clone()).collect()
}

pub(super) const ADDON_DOCS: &str = "https://f1.swofty.net/reference/addons/";

pub(super) fn drop_choice(
    icon: IconName,
    title: &str,
    text: &str,
    action: impl IntoElement,
    p: Palette,
) -> impl IntoElement {
    h_flex()
        .p(px(14.))
        .gap(px(14.))
        .items_start()
        .rounded(px(10.))
        .border_1()
        .border_color(p.border_variant)
        .bg(p.background)
        .child(
            div()
                .size(px(34.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(8.))
                .bg(p.selected)
                .child(Icon::new(icon).size(px(17.)).text_color(p.accent_hover)),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(4.))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(p.text)
                        .child(title.to_owned()),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .line_height(px(19.))
                        .text_color(p.text_muted)
                        .child(text.to_owned()),
                )
                .child(h_flex().pt(px(6.)).child(action)),
        )
}

pub(super) fn import_all(
    store: &Store,
    registry: &Registry,
    profile: &Profile,
    install: Option<&Install>,
    paths: &[PathBuf],
    progress: &Reporter,
) -> (Vec<String>, Vec<String>, Vec<PathBuf>) {
    let (batches, unclaimed) = import::route_all(registry, profile, paths);
    let mut done = Vec::new();
    let mut failed = Vec::new();
    let mut game_files = None;
    for (route, paths) in batches {
        match route {
            Route::Addon => {
                for path in paths {
                    for result in store.install_addons(&path) {
                        match result {
                            Ok(m) => done.push(trf!(
                                "Installed {name} {version}",
                                name = m.name,
                                version = m.version
                            )),
                            Err(e) => {
                                crate::app_log::action("install addon").file(&path).failed(&e);
                                failed.push(format!("{}: {e}", file_name(&path)))
                            }
                        }
                    }
                }
            }
            Route::Importer(id) => {
                let Some(importer) = registry.get(&id).and_then(|e| e.addon.importer()) else {
                    continue;
                };
                let game = game_files.get_or_insert_with(|| game(store, install));
                let result = host(store, &id, profile, game.as_ref()).and_then(|h| {
                    importer
                        .import(&h.cx(), &paths, progress)
                        .map_err(|e| e.to_string())
                });
                match result {
                    Ok(lines) => done.extend(lines),
                    Err(e) => {
                        crate::app_log::action("import")
                            .addon(&id)
                            .files(&paths)
                            .failed(&e);
                        failed.push(e)
                    }
                }
            }
        }
    }
    (done, failed, unclaimed)
}

impl Shell {
    pub(crate) fn drop_paths(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        self.dropping = false;
        if paths.is_empty() {
            return;
        }
        let store = self.store.clone();
        let registry = self.library.registry.clone();
        let profile = self.library.profile.clone();
        let install = self.install.clone();
        let shared: Shared = Arc::new(Mutex::new(Some((
            0.,
            trf!(
                "Importing {items}",
                items = trn(paths.len(), "{count} item", "{count} items")
            ),
        ))));
        let reporter = Reporter(shared.clone());
        self.follow_progress(shared, cx);
        cx.spawn_in(window, async move |this, cx| {
            let (mut done, failed, unclaimed, fresh, left_out) = cx
                .background_executor()
                .spawn(async move {
                    let before = installed(&store);
                    let (done, failed, unclaimed) =
                        import_all(&store, &registry, &profile, install.as_ref(), &paths, &reporter);
                    let fresh = installed_since(&before, &installed(&store));
                    let addons = store.addons().unwrap_or_default();
                    let left_out: Vec<String> = fresh
                        .iter()
                        .filter_map(|id| addons.get(id))
                        .flat_map(|m| swofty_core::tracks::left_out_lines(&store, m))
                        .collect();
                    if let Ok(mut p) = reporter.0.lock() {
                        *p = None;
                    }
                    (done, failed, unclaimed, fresh, left_out)
                })
                .await;
            if fresh.len() > 1 {
                done.insert(
                    0,
                    trf!(
                        "Installed {addons}; Addons highlights the newest",
                        addons = trn(fresh.len(), "{count} addon", "{count} addons")
                    ),
                );
            }
            this.update_in(cx, |s, window, cx| {
                if !done.is_empty() {
                    window.notify(Notification::success(done.join("\n")), cx);
                }
                if !failed.is_empty() {
                    window.notify(Notification::error(failed.join("\n")), cx);
                }
                if !left_out.is_empty() {
                    window.notify(
                        Notification::warning(left_out.join("\n"))
                            .title(tr("Track can't be used"))
                            .autohide(false),
                        cx,
                    );
                }
                if !unclaimed.is_empty() {
                    s.show_not_addons(&unclaimed, window, cx);
                }
                s.rediscover = true;
                s.reload(cx);
                if let Some(id) = fresh.first() {
                    s.show_addon(id, cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(super) fn show_not_addons(&self, paths: &[PathBuf], window: &mut Window, cx: &mut Context<Self>) {
        const SHOWN: usize = 5;
        let count = paths.len();
        let names: Vec<String> = paths.iter().take(SHOWN).map(|p| file_name(p)).collect();
        let more = paths.len().saturating_sub(SHOWN);
        let shell = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, cx| {
            let p = crate::theme::palette(cx);
            let shell = shell.clone();
            let files = v_flex()
                .gap(px(8.))
                .child(trn(
                    count,
                    "The launcher can't install this file as an addon:",
                    "The launcher can't install these files as addons:",
                ))
                .child(
                    v_flex()
                        .px(px(12.))
                        .py(px(8.))
                        .gap(px(4.))
                        .rounded(px(8.))
                        .bg(p.background)
                        .border_1()
                        .border_color(p.border_variant)
                        .text_color(p.text)
                        .children(names.iter().map(|n| {
                            h_flex()
                                .gap(px(8.))
                                .items_center()
                                .child(
                                    Icon::new(IconName::File)
                                        .size(px(14.))
                                        .text_color(p.text_placeholder),
                                )
                                .child(div().min_w_0().truncate().child(n.clone()))
                        }))
                        .when(more > 0, |d| {
                            d.child(
                                div()
                                    .text_color(p.text_placeholder)
                                    .child(trf!("and {count} more", count = more)),
                            )
                        }),
                );
            let mods = trn(
                count,
                "Mods like this one go through the Simplified Importer, which turns them into addons.",
                "Mods like these go through the Simplified Importer, which turns them into addons.",
            );
            alert
                .width(px(540.))
                .title(trn(
                    count,
                    "That isn't a Swofty addon",
                    "Those aren't Swofty addons",
                ))
                .description(files)
                .child(
                    v_flex()
                        .gap(px(10.))
                        .child(drop_choice(
                            IconName::FolderInput,
                            tr("Trying to add a mod?"),
                            &mods,
                            Button::new("not-addon-importer")
                                .primary()
                                .icon(IconName::ArrowRight)
                                .label(tr("Go to Simplified Importer"))
                                .on_click(move |_, window, cx| {
                                    window.close_dialog(cx);
                                    shell.update(cx, |s, cx| s.open_page(Page::Importer, cx)).ok();
                                }),
                            p,
                        ))
                        .child(drop_choice(
                            IconName::BookOpen,
                            tr("Making a Swofty addon?"),
                            tr("An addon is a folder with a swofty.json."),
                            crate::tools::web_link(
                                "not-addon-docs",
                                tr("Read the addon docs"),
                                ADDON_DOCS,
                                p,
                            ),
                            p,
                        )),
                )
                .ok_text(tr("Close"))
                .ok_variant(ButtonVariant::Secondary)
        });
    }

    pub(crate) fn follow_progress(&mut self, shared: Shared, cx: &mut Context<Self>) {
        self.progress = shared.lock().ok().and_then(|p| p.clone());
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(100)).await;
                let now = shared.lock().ok().and_then(|p| p.clone());
                let done = now.is_none();
                if this
                    .update(cx, |s, cx| {
                        s.progress = now;
                        cx.notify();
                    })
                    .is_err()
                    || done
                {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn drop_overlay(&self, p: Palette) -> impl IntoElement {
        div()
            .absolute()
            .inset_0()
            .p(px(16.))
            .bg(p.background.opacity(0.85))
            .child(
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap(px(8.))
                    .rounded(px(10.))
                    .border_2()
                    .border_dashed()
                    .border_color(p.accent)
                    .child(badge(IconName::Upload, 44., p).text_color(p.accent))
                    .child(
                        div()
                            .pt(px(8.))
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("Drop to import")),
                    )
                    .child(
                        div()
                            .max_w(px(420.))
                            .text_center()
                            .text_size(px(13.))
                            .line_height(px(19.))
                            .text_color(p.text_muted)
                            .child(tr(
                                "Addon folders, .swoftyaddon files, zip and rar archives, track packages and the files your addons import.",
                            )),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::{Duration, SystemTime};

    use swofty_core::store::Store;

    use super::{installed, installed_since};

    #[test]
    fn an_import_shows_what_it_installed_or_replaced_newest_first() {
        let t = |s: u64| SystemTime::UNIX_EPOCH + Duration::from_secs(s);
        let snap = |v: &[(&str, u64)]| -> BTreeMap<String, SystemTime> {
            v.iter().map(|(id, s)| ((*id).to_owned(), t(*s))).collect()
        };
        let before = snap(&[("a.kept", 1), ("a.replaced", 2), ("a.removed", 3)]);
        let after = snap(&[("a.kept", 1), ("a.replaced", 9), ("b.new", 8), ("c.new", 9)]);
        assert_eq!(installed_since(&before, &after), ["a.replaced", "c.new", "b.new"]);
        assert!(installed_since(&before, &before).is_empty());
    }

    #[test]
    fn snapshots_list_only_folders_with_a_manifest() {
        let root = std::env::temp_dir().join(format!("swofty-launcher-installed-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let store = Store::new(&root);
        let addon = store.addons_dir().join("t.one");
        std::fs::create_dir_all(&addon).unwrap();
        std::fs::create_dir_all(store.addons_dir().join(".importing")).unwrap();
        std::fs::write(addon.join(swofty_core::manifest::MANIFEST_FILE), "{}").unwrap();
        let before = BTreeMap::new();
        assert_eq!(installed_since(&before, &installed(&store)), ["t.one"]);
        std::fs::remove_dir_all(&root).ok();
    }
}
