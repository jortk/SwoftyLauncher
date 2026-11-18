//! On Linux the slots live in the `swofty-ads` helper, as child windows of the
//! launcher's X11 window, so the launcher itself never links WebKitGTK. With no
//! helper, no WebKitGTK or a Wayland window there are no slots.

use super::{Area, Unit};
use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use swofty_ads::{Command as Line, Event};

/// The helper opens its slots itself, so nothing is opened here.
pub(super) enum Opener {}

impl Opener {
    pub(super) fn open(self) -> Option<View> {
        match self {}
    }
}

pub(super) enum View {}

pub(super) struct Backend {
    child: Child,
    stdin: Option<ChildStdin>,
    events: Arc<Mutex<Vec<Event>>>,
}

impl Backend {
    pub(super) fn start(window: &Window, data: &Path) -> Option<Self> {
        let parent = match HasWindowHandle::window_handle(window).ok()?.as_raw() {
            RawWindowHandle::Xcb(h) => u64::from(h.window.get()),
            RawWindowHandle::Xlib(h) => h.window,
            _ => {
                crate::app_log::note("no ad slots: the window isn't on X11");
                return None;
            }
        };
        let helper = helper()?;
        let dir = data.join("ads");
        std::fs::create_dir_all(&dir).ok()?;
        let log = std::fs::File::create(dir.join("helper.log")).map(Stdio::from);
        let mut child = Command::new(&helper)
            .arg("--parent")
            .arg(parent.to_string())
            .arg("--data")
            .arg(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(log.unwrap_or_else(|_| Stdio::null()))
            .spawn()
            .inspect_err(|e| {
                crate::app_log::note(&format!("no ad slots: {} didn't start: {e}", helper.display()))
            })
            .ok()?;
        let stdin = child.stdin.take();
        let events = Arc::new(Mutex::new(Vec::new()));
        if let Some(stdout) = child.stdout.take() {
            let events = events.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if let Some(event) = Event::parse(&line)
                        && let Ok(mut events) = events.lock()
                    {
                        events.push(event);
                    }
                }
            });
        }
        Some(Self { child, stdin, events })
    }

    pub(super) fn events(&mut self) -> Vec<Event> {
        self.events
            .lock()
            .map(|mut e| std::mem::take(&mut *e))
            .unwrap_or_default()
    }

    pub(super) fn show(&mut self, unit: Unit, url: &str, area: Area, scale: f32) -> Option<Opener> {
        self.send(&Line::Show {
            slot: unit.key().into(),
            url: url.into(),
            x: area.x,
            y: area.y,
            width: area.width,
            height: area.height,
            scale: f64::from(scale),
        });
        None
    }

    pub(super) fn hide(&mut self, unit: Unit) {
        self.send(&Line::Hide {
            slot: unit.key().into(),
        });
    }

    pub(super) fn close(&mut self, unit: Unit) {
        self.send(&Line::Close {
            slot: unit.key().into(),
        });
    }

    pub(super) fn adopt(&mut self, _: Unit, view: Option<View>) {
        if let Some(view) = view {
            match view {}
        }
    }

    /// Child windows move with the launcher's window.
    pub(super) fn moved(&mut self, _: impl Iterator<Item = (Unit, Area)>) {}

    fn send(&mut self, line: &Line) {
        let Some(stdin) = &mut self.stdin else {
            return;
        };
        if stdin
            .write_all(line.line().as_bytes())
            .and_then(|()| stdin.flush())
            .is_err()
        {
            self.stdin = None;
            crate::app_log::note("the ad helper stopped");
        }
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        self.stdin = None;
        let _ = self.child.try_wait();
    }
}

/// `SWOFTY_ADS_HELPER`, else `swofty-ads` beside the launcher.
fn helper() -> Option<PathBuf> {
    let path = match std::env::var_os("SWOFTY_ADS_HELPER") {
        Some(path) => PathBuf::from(path),
        None => std::env::current_exe().ok()?.with_file_name("swofty-ads"),
    };
    if path.is_file() {
        Some(path)
    } else {
        crate::app_log::note(&format!("no ad slots: {} is missing", path.display()));
        None
    }
}
