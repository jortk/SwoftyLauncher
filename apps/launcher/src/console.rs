use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::notices::{Notification, Notify};
use crate::scale::{Hit, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::Scrollbar;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use swofty_core::session::{LOADER_LOG, SessionRecord};
use swofty_core::store::Store;
use swofty_i18n::{tr, trn};

use crate::pages::badge;
use crate::play_bar::link;
use crate::shell::Shell;
use crate::theme::{Palette, palette};

const MAX_LINES: usize = 20_000;
const BACKLOG: u64 = 4 << 20;
const STEP: u64 = 8 << 20;
const CHECK: usize = 64;
const ROW: f32 = 18.;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Source {
    Launcher,
    Wrapper,
    Game,
}

const SOURCES: [Source; 3] = [Source::Launcher, Source::Wrapper, Source::Game];

impl Source {
    fn label(self) -> &'static str {
        match self {
            Self::Launcher => "Launcher",
            Self::Wrapper => "Wrapper",
            Self::Game => "Game",
        }
    }

    fn index(self) -> usize {
        self as usize
    }

    fn path(self, store: &Store, game: Option<&Path>) -> Option<PathBuf> {
        match self {
            Self::Launcher => Some(store.root.join("state/launcher-run.log")),
            Self::Wrapper => Some(store.root.join("state/wrapper-run.log")),
            Self::Game => game_log(store, game),
        }
    }
}

fn game_log(store: &Store, game: Option<&Path>) -> Option<PathBuf> {
    let session = SessionRecord::read(&store.latest_session()?);
    if session.exit_code.is_some() {
        return Some(session.dir.join("loader.log")).filter(|p| p.is_file());
    }
    let live = game?.join(LOADER_LOG);
    session.owns_loader_log(&live).then_some(live)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Level {
    Error,
    Warn,
    Info,
    Plain,
}

fn level(line: &str) -> Level {
    let l = line.to_ascii_lowercase();
    if ["error", "err:", "failed", "[crash]"]
        .iter()
        .any(|k| l.contains(k))
    {
        Level::Error
    } else if l.contains("warn") {
        Level::Warn
    } else if l.contains("[info]") {
        Level::Info
    } else {
        Level::Plain
    }
}

#[derive(Default, Debug, PartialEq)]
struct Chunk {
    reset: bool,
    lines: Vec<String>,
}

struct Tail {
    path: PathBuf,
    pos: u64,
    seen: Vec<u8>,
    partial: Vec<u8>,
    skip: bool,
}

impl Tail {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            pos: 0,
            seen: Vec::new(),
            partial: Vec::new(),
            skip: false,
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn rewind(&mut self) -> bool {
        let read = self.pos > 0;
        self.pos = 0;
        self.seen.clear();
        self.partial.clear();
        self.skip = false;
        read
    }

    fn fetch(&mut self, file: &mut File, len: u64) -> Option<Vec<u8>> {
        if self.pos == 0 && len > BACKLOG {
            self.pos = len - BACKLOG;
            self.skip = true;
        }
        let start = self.pos - self.seen.len() as u64;
        file.seek(SeekFrom::Start(start)).ok()?;
        let mut data = Vec::new();
        file.take(STEP).read_to_end(&mut data).ok()?;
        Some(data)
    }

    fn read(&mut self) -> Chunk {
        let mut chunk = Chunk::default();
        let Ok(mut file) = File::open(&self.path) else {
            chunk.reset = self.rewind();
            return chunk;
        };
        let len = file.metadata().map_or(0, |m| m.len());
        if len < self.pos {
            chunk.reset = self.rewind();
        }
        let mut data = self.fetch(&mut file, len);
        if data.as_ref().is_some_and(|d| !d.starts_with(&self.seen)) {
            chunk.reset |= self.rewind();
            data = self.fetch(&mut file, len);
        }
        let Some(data) = data else {
            return chunk;
        };
        let fresh = &data[self.seen.len()..];
        self.pos += fresh.len() as u64;
        self.seen.extend_from_slice(fresh);
        self.seen.drain(..self.seen.len().saturating_sub(CHECK));
        self.partial.extend_from_slice(fresh);
        let Some(end) = self.partial.iter().rposition(|&b| b == b'\n') else {
            return chunk;
        };
        let rest = self.partial.split_off(end + 1);
        let done = std::mem::replace(&mut self.partial, rest);
        chunk.lines = done[..end]
            .split(|&b| b == b'\n')
            .map(|l| String::from_utf8_lossy(l).trim_end().to_owned())
            .collect();
        if std::mem::take(&mut self.skip) {
            chunk.lines.remove(0);
        }
        chunk
    }
}

struct Line {
    text: SharedString,
    level: Level,
    width: usize,
}

#[derive(Default)]
struct Feed {
    tail: Option<Tail>,
    path: Option<PathBuf>,
    lines: VecDeque<Line>,
    widest: usize,
}

impl Feed {
    fn clear(&mut self) {
        self.lines.clear();
        self.widest = 0;
    }

    fn push(&mut self, lines: Vec<String>) {
        if lines.is_empty() {
            return;
        }
        let overflow = (self.lines.len() + lines.len()).saturating_sub(MAX_LINES);
        let drop = overflow.min(self.lines.len());
        self.lines.drain(..drop);
        self.lines
            .extend(lines.into_iter().skip(overflow - drop).map(|text| Line {
                level: level(&text),
                width: text.chars().count(),
                text: text.into(),
            }));
        self.widest = (0..self.lines.len())
            .max_by_key(|&i| (self.lines[i].width, std::cmp::Reverse(i)))
            .unwrap_or(0);
    }
}

type Polled = [(Option<Tail>, Chunk); 3];

fn poll(store: &Store, game: Option<&Path>, tails: [Option<Tail>; 3]) -> Polled {
    let mut sources = SOURCES.into_iter();
    tails.map(|tail| {
        let want = sources.next().and_then(|s| s.path(store, game));
        let mut tail = match tail {
            Some(t) if Some(t.path()) == want.as_deref() => Some(t),
            _ => want.map(Tail::new),
        };
        let chunk = tail.as_mut().map(Tail::read).unwrap_or_default();
        (tail, chunk)
    })
}

pub struct Console {
    store: Store,
    shell: WeakEntity<Shell>,
    feeds: [Feed; 3],
    source: Source,
    scroll: UniformListScrollHandle,
    _poll: Task<()>,
}

const TITLE: &str = "Swofty Launcher for F1 25: Console";

pub fn open(store: Store, shell: WeakEntity<Shell>, cx: &mut App) -> Option<AnyWindowHandle> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(980.), px(620.)),
            cx,
        ))),
        window_min_size: Some(size(px(560.), px(320.))),
        app_id: Some("net.swofty.ModLauncher".into()),
        titlebar: Some(TitlebarOptions {
            title: Some(TITLE.into()),
            ..TitleBar::title_bar_options()
        }),
        ..TitleBar::window_options()
    };
    gpui_kit::open_window(options, cx, |_, cx| cx.new(|cx| Console::new(store, shell, cx)))
        .ok()
        .map(|(window, _)| window)
}

impl Console {
    fn new(store: Store, shell: WeakEntity<Shell>, cx: &mut Context<Self>) -> Self {
        let poll = cx.spawn(async move |this, cx| {
            loop {
                let Ok((store, game, tails)) = this.update(cx, |c, cx| {
                    let game = c
                        .shell
                        .upgrade()
                        .and_then(|s| s.read(cx).install.as_ref().map(|i| i.dir.clone()));
                    (c.store.clone(), game, c.feeds.each_mut().map(|f| f.tail.take()))
                }) else {
                    break;
                };
                let polled = cx
                    .background_executor()
                    .spawn(async move { poll(&store, game.as_deref(), tails) })
                    .await;
                if this.update(cx, |c, cx| c.apply(polled, cx)).is_err() {
                    break;
                }
                cx.background_executor().timer(Duration::from_millis(250)).await;
            }
        });
        Self {
            store,
            shell,
            feeds: Default::default(),
            source: Source::Launcher,
            scroll: UniformListScrollHandle::new(),
            _poll: poll,
        }
    }

    fn apply(&mut self, polled: Polled, cx: &mut Context<Self>) {
        let follow = self.scroll.is_scrolled_to_end().is_none_or(|end| end);
        let mut changed = false;
        for (feed, (tail, chunk)) in self.feeds.iter_mut().zip(polled) {
            let path = tail.as_ref().map(|t| t.path().to_path_buf());
            let moved = feed.path != path;
            feed.path = path;
            feed.tail = tail;
            if chunk.reset || moved {
                feed.clear();
            }
            changed |= chunk.reset || moved || !chunk.lines.is_empty();
            feed.push(chunk.lines);
        }
        if changed {
            if follow {
                self.scroll.scroll_to_bottom();
            }
            cx.notify();
        }
    }

    fn feed(&self) -> &Feed {
        &self.feeds[self.source.index()]
    }

    fn select(&mut self, source: Source, cx: &mut Context<Self>) {
        self.source = source;
        self.scroll.scroll_to_bottom();
        cx.notify();
    }

    fn copy_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let lines = &self.feed().lines;
        let text = lines
            .iter()
            .map(|l| l.text.as_ref())
            .collect::<Vec<_>>()
            .join("\n");
        let count = lines.len();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        window.notify(
            Notification::success(trn(count, "Copied {count} line", "Copied {count} lines")),
            cx,
        );
    }

    fn tab(&self, source: Source, p: Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let active = self.source == source;
        let count = self.feeds[source.index()].lines.len();
        h_flex()
            .id(source.label())
            .h(px(28.))
            .px(px(10.))
            .gap(px(8.))
            .rounded(px(5.))
            .cursor_pointer()
            .text_size(px(13.))
            .map(|d| {
                if active {
                    d.bg(p.selected)
                        .text_color(p.text)
                        .font_weight(FontWeight::MEDIUM)
                } else {
                    d.text_color(p.text_muted)
                        .hover(move |s| s.bg(p.hover).text_color(p.text))
                }
            })
            .child(source.label())
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(p.text_placeholder)
                    .child(count.to_string()),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.select(source, cx)))
    }

    fn toolbar(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let path = self.feed().path.clone().filter(|p| p.is_file());
        let empty = self.feed().lines.is_empty();
        h_flex()
            .h(px(44.))
            .flex_shrink_0()
            .px(px(10.))
            .gap(px(4.))
            .bg(p.chrome)
            .border_b_1()
            .border_color(p.border_variant)
            .children(SOURCES.map(|s| self.tab(s, p, cx)))
            .child(div().flex_1())
            .child(
                Button::new("copy-all")
                    .ghost()
                    .tap_sm()
                    .icon(IconName::Copy)
                    .label(tr("Copy all"))
                    .disabled(empty)
                    .on_click(cx.listener(|this, _, window, cx| this.copy_all(window, cx))),
            )
            .child(
                Button::new("clear")
                    .ghost()
                    .tap_sm()
                    .icon(IconName::Eraser)
                    .label(tr("Clear view"))
                    .disabled(empty)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.feeds[this.source.index()].clear();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("open-file")
                    .ghost()
                    .tap_sm()
                    .icon(IconName::FolderOpen)
                    .label(tr("Open file"))
                    .tooltip(tr("Show the log in the file manager"))
                    .disabled(path.is_none())
                    .on_click(move |_, _, cx| {
                        if let Some(path) = &path {
                            cx.reveal_path(path);
                        }
                    }),
            )
    }

    fn body(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let feed = self.feed();
        if feed.lines.is_empty() {
            let (title, body) = match &feed.path {
                Some(path) if path.is_file() => (
                    "Waiting for new lines",
                    "New lines appear here as they are written",
                ),
                _ if self.source == Source::Game => (
                    "No game log for this session yet",
                    "The loader writes this log when F1 25 starts; a launch that fails earlier has none",
                ),
                _ => ("Nothing logged yet", "This log is written on the next launch"),
            };
            return v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(px(8.))
                .pb(px(32.))
                .child(badge(IconName::SquareTerminal, 44., p))
                .child(
                    div()
                        .pt(px(8.))
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .max_w(px(520.))
                        .text_center()
                        .text_size(px(13.))
                        .line_height(px(19.))
                        .text_color(p.text_muted)
                        .child(body),
                )
                .into_any_element();
        }
        let list = uniform_list(
            "console-lines",
            feed.lines.len(),
            cx.processor(move |this, range: std::ops::Range<usize>, _, _| {
                let lines = &this.feed().lines;
                range
                    .filter_map(|i| lines.get(i))
                    .map(|line| {
                        div()
                            .h(px(ROW))
                            .px(px(14.))
                            .whitespace_nowrap()
                            .text_color(match line.level {
                                Level::Error => p.error,
                                Level::Warn => p.warning,
                                Level::Info => p.text_muted,
                                Level::Plain => p.text,
                            })
                            .child(line.text.clone())
                    })
                    .collect::<Vec<_>>()
            }),
        )
        .size_full()
        .pt(px(8.))
        .pb(px(18.))
        .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
        .with_width_from_item(Some(feed.widest))
        .track_scroll(&self.scroll);
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(12.))
            .line_height(px(ROW))
            .child(list)
            .child(Scrollbar::new(&self.scroll))
            .into_any_element()
    }

    fn footer(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let feed = self.feed();
        let paused = !feed.lines.is_empty() && self.scroll.is_scrolled_to_end() == Some(false);
        h_flex()
            .h(px(30.))
            .flex_shrink_0()
            .px(px(14.))
            .gap(px(12.))
            .bg(p.chrome)
            .border_t_1()
            .border_color(p.border_variant)
            .text_size(px(12.))
            .text_color(p.text_placeholder)
            .child(
                div().flex_1().min_w_0().truncate().child(
                    feed.path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                ),
            )
            .when(paused, |d| {
                d.child(
                    link("follow", tr("Paused · jump to latest"), p).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.scroll.scroll_to_bottom();
                            cx.notify();
                        },
                    )),
                )
            })
            .child(trn(feed.lines.len(), "{count} line", "{count} lines"))
    }
}

impl Render for Console {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        v_flex()
            .size_full()
            .bg(p.background)
            .text_color(p.text)
            .text_size(px(14.))
            .child(
                TitleBar::new().bg(p.chrome).child(
                    h_flex()
                        .gap(px(8.))
                        .text_color(p.text_muted)
                        .child(Icon::new(IconName::SquareTerminal).size(px(14.)))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .child("Console"),
                        ),
                ),
            )
            .child(self.toolbar(p, cx))
            .child(self.body(p, cx))
            .child(self.footer(p, cx))
            .on_scroll(cx.listener(|_, _, _, cx| cx.notify()))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::{Path, PathBuf};

    use swofty_core::session::{LOADER_LOG, LOADER_MARK, LogMark};
    use swofty_core::store::Store;

    use super::{Feed, Level, MAX_LINES, Tail, game_log, level};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("swofty-console-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("run.log")
    }

    fn append(path: &Path, text: &str) {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        f.write_all(text.as_bytes()).unwrap();
    }

    #[test]
    fn levels() {
        assert_eq!(
            level("[wine] c:0150:err:service:device_notify_proc"),
            Level::Error
        );
        assert_eq!(level("Failed to create process"), Level::Error);
        assert_eq!(level("[crash] access violation"), Level::Error);
        assert_eq!(level("WARNING: slow frame"), Level::Warn);
        assert_eq!(level("[info] no fatal-class exceptions"), Level::Info);
        assert_eq!(level("bahrain24: track_trivia: added bahrain24"), Level::Plain);
    }

    #[test]
    fn follows_appends_and_partial_lines() {
        let path = scratch("append");
        std::fs::write(&path, "one\ntwo\npar").unwrap();
        let mut tail = Tail::new(path.clone());
        assert_eq!(tail.read().lines, ["one", "two"]);
        assert!(tail.read().lines.is_empty());
        append(&path, "tial\r\nthree\n");
        let chunk = tail.read();
        assert!(!chunk.reset);
        assert_eq!(chunk.lines, ["partial", "three"]);
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn restarts_after_truncation_and_rewrites() {
        let path = scratch("rotate");
        std::fs::write(&path, "first run line one\nfirst run line two\n").unwrap();
        let mut tail = Tail::new(path.clone());
        assert_eq!(tail.read().lines.len(), 2);
        std::fs::write(&path, "short\n").unwrap();
        let chunk = tail.read();
        assert!(chunk.reset);
        assert_eq!(chunk.lines, ["short"]);
        std::fs::write(&path, "second run, longer than before so the size check passes\n").unwrap();
        let chunk = tail.read();
        assert!(chunk.reset);
        assert_eq!(
            chunk.lines,
            ["second run, longer than before so the size check passes"]
        );
        std::fs::remove_file(&path).unwrap();
        assert!(tail.read().reset);
        append(&path, "back\n");
        assert_eq!(tail.read().lines, ["back"]);
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn shows_only_the_latest_sessions_game_log() {
        let root = scratch("game-log").parent().unwrap().to_path_buf();
        let store = Store::new(&root);
        let game = root.join("game");
        let live = game.join(LOADER_LOG);
        std::fs::create_dir_all(live.parent().unwrap()).unwrap();
        std::fs::write(&live, "21:44:08.228 [  416] swofty loader attached, pid 412\n").unwrap();
        assert_eq!(game_log(&store, Some(&game)), None);
        let session = store.sessions_dir().join("20261017-111206");
        std::fs::create_dir_all(&session).unwrap();
        std::fs::write(
            store.sessions_dir().join("latest.txt"),
            session.display().to_string(),
        )
        .unwrap();
        std::fs::write(
            session.join(LOADER_MARK),
            serde_json::to_vec(&LogMark::read(&live)).unwrap(),
        )
        .unwrap();
        assert_eq!(game_log(&store, Some(&game)), None);
        std::fs::rename(&live, live.with_file_name("loader.prev.log")).unwrap();
        std::fs::write(&live, "22:12:08.004 [  420] swofty loader attached, pid 416\n").unwrap();
        assert_eq!(game_log(&store, Some(&game)), Some(live.clone()));
        std::fs::write(session.join("exit_code"), "1\n").unwrap();
        assert_eq!(game_log(&store, Some(&game)), None);
        std::fs::copy(&live, session.join("loader.log")).unwrap();
        assert_eq!(game_log(&store, Some(&game)), Some(session.join("loader.log")));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn caps_lines() {
        let mut feed = Feed::default();
        feed.push((0..MAX_LINES + 5).map(|i| format!("line {i}")).collect());
        assert_eq!(feed.lines.len(), MAX_LINES);
        assert_eq!(feed.lines[0].text.as_ref(), "line 5");
        assert_eq!(feed.lines[feed.widest].text.as_ref(), "line 10000");
        feed.push(vec!["x".repeat(40)]);
        assert_eq!(feed.lines.len(), MAX_LINES);
        assert_eq!(feed.widest, MAX_LINES - 1);
    }
}
