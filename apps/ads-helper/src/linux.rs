use gtk::glib;
use gtk::prelude::*;
use std::cell::Cell;
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;
use swofty_ads::{Command, Consent, Event};
use wry::raw_window_handle as rwh;
use wry::{Rect, WebContext, WebView, WebViewExtUnix};

/// Sizes a slot exactly. Its GTK window otherwise keeps GTK's default height
/// of 200 pixels, which leaves a banner too tall and blank.
fn fit(view: &WebView, bounds: Rect, width: u32, height: u32) {
    let _ = view.set_bounds(bounds);
    let webview = view.webview();
    let Some(window) = webview.toplevel().and_then(|w| w.downcast::<gtk::Window>().ok()) else {
        return;
    };
    let scale = webview.scale_factor().max(1);
    let (width, height) = (width as i32 / scale, height as i32 / scale);
    window.set_default_size(width, height);
    window.resize(width, height);
}

/// The launcher's X11 window, which every slot is a child of.
struct Parent(u64);

impl rwh::HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let handle = rwh::XlibWindowHandle::new(self.0 as _);
        Ok(unsafe { rwh::WindowHandle::borrow_raw(handle.into()) })
    }
}

struct Slot {
    view: WebView,
    home: String,
    strayed: Rc<Cell<bool>>,
}

struct Host {
    parent: Parent,
    context: WebContext,
    slots: HashMap<String, Slot>,
}

impl Host {
    fn apply(&mut self, command: Command) {
        match command {
            Command::Show {
                slot,
                url,
                x,
                y,
                width,
                height,
                scale,
            } => {
                let bounds = swofty_ads::view::rect(x, y, width, height);
                if let Some(s) = self.slots.get_mut(&slot) {
                    if s.home != url {
                        let _ = s.view.load_url(&url);
                        s.home = url;
                    }
                    fit(&s.view, bounds, width, height);
                    let _ = s.view.zoom(scale);
                    let _ = s.view.set_visible(true);
                    return;
                }
                let strayed = Rc::new(Cell::new(false));
                let key = slot.clone();
                let built = swofty_ads::view::builder(
                    &mut self.context,
                    &url,
                    bounds,
                    swofty_ads::slot_background(&url).unwrap_or([0, 0, 0]),
                    strayed.clone(),
                    move |consent| report(&key, consent),
                )
                .with_url(&url)
                .build_as_child(&self.parent);
                match built {
                    Ok(view) => {
                        fit(&view, bounds, width, height);
                        let _ = view.zoom(scale);
                        self.slots.insert(
                            slot,
                            Slot {
                                view,
                                home: url,
                                strayed,
                            },
                        );
                    }
                    Err(e) => eprintln!("swofty-ads: couldn't open the {slot} slot: {e}"),
                }
            }
            Command::Hide { slot } => {
                if let Some(s) = self.slots.get(&slot) {
                    let _ = s.view.set_visible(false);
                }
            }
            Command::Close { slot } => {
                self.slots.remove(&slot);
            }
        }
    }

    fn bring_back_strays(&self) {
        for s in self.slots.values() {
            if s.strayed.replace(false) {
                let _ = s.view.load_url(&s.home);
            }
        }
    }
}

/// Tells the launcher, on stdout, what a slot's consent page reported.
fn report(slot: &str, consent: Consent) {
    let line = Event {
        slot: slot.to_owned(),
        consent,
    }
    .line();
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(line.as_bytes()).and_then(|()| out.flush());
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn parse_window(text: &str) -> Option<u64> {
    match text.strip_prefix("0x") {
        Some(hex) => u64::from_str_radix(hex, 16).ok(),
        None => text.parse().ok(),
    }
}

pub fn run() -> i32 {
    let args: Vec<String> = std::env::args().collect();
    let Some(parent) = arg(&args, "--parent").as_deref().and_then(parse_window) else {
        eprintln!("usage: swofty-ads --parent <x11 window> --data <dir>");
        return 2;
    };
    let data = arg(&args, "--data").map(PathBuf::from);
    // SAFETY: no other thread exists yet.
    unsafe {
        std::env::set_var("GDK_BACKEND", "x11");
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
    if let Err(e) = gtk::init() {
        eprintln!("swofty-ads: GTK didn't start: {e}");
        return 1;
    }
    let (send, commands) = mpsc::channel::<Option<Command>>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if let Some(command) = Command::parse(&line)
                && send.send(Some(command)).is_err()
            {
                return;
            }
        }
        let _ = send.send(None);
    });
    let mut host = Host {
        parent: Parent(parent),
        context: WebContext::new(data),
        slots: HashMap::new(),
    };
    glib::timeout_add_local(Duration::from_millis(16), move || {
        while let Ok(next) = commands.try_recv() {
            match next {
                Some(command) => host.apply(command),
                None => {
                    gtk::main_quit();
                    return glib::ControlFlow::Break;
                }
            }
        }
        host.bring_back_strays();
        glib::ControlFlow::Continue
    });
    gtk::main();
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_id_reads_in_hex_or_decimal() {
        assert_eq!(parse_window("0x3a00007"), Some(0x3a00007));
        assert_eq!(parse_window("60817415"), Some(60817415));
        assert_eq!(parse_window("window"), None);
    }
}
