//! On Windows the slots live in this process, each in a small owned popup that
//! sits over its rectangle. A child window can't be used: GPUI draws through a
//! topmost DirectComposition target, which covers child windows.

use super::{Area, Unit};
use gpui_kit::Window;
use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroIsize;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Once;
use swofty_ads::view::{self, wry};
use swofty_ads::{Consent, Event};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, MA_NOACTIVATE, RegisterClassW, SW_HIDE, SWP_NOACTIVATE,
    SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowPos, ShowWindow, WM_MOUSEACTIVATE, WNDCLASSW, WS_CLIPCHILDREN,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{PCWSTR, w};

const CLASS: PCWSTR = w!("SwoftyLauncherAdSlot");

pub(super) struct Backend {
    owner: isize,
    data: PathBuf,
    views: HashMap<Unit, View>,
    opening: HashSet<Unit>,
    events: Rc<RefCell<Vec<Event>>>,
}

/// What a slot needs to open, taken out of the backend so it can run with no
/// borrow held.
pub(super) struct Opener {
    slot: &'static str,
    owner: isize,
    data: PathBuf,
    url: String,
    area: Area,
    events: Rc<RefCell<Vec<Event>>>,
}

impl Opener {
    pub(super) fn open(self) -> Option<View> {
        let (slot, events) = (self.slot, self.events.clone());
        let report = move |consent| {
            events.borrow_mut().push(Event {
                slot: slot.to_owned(),
                consent,
            })
        };
        View::open(self.owner, &self.data, &self.url, self.area, report)
            .inspect_err(|e| crate::app_log::note(&format!("the {} ad slot didn't open: {e}", self.slot)))
            .ok()
    }
}

pub(super) struct View {
    webview: wry::WebView,
    _context: wry::WebContext,
    popup: isize,
    url: String,
    size: (u32, u32),
    visible: bool,
}

impl Backend {
    pub(super) fn start(window: &Window, data: &Path) -> Option<Self> {
        let RawWindowHandle::Win32(handle) = HasWindowHandle::window_handle(window).ok()?.as_raw() else {
            return None;
        };
        if let Err(e) = wry::webview_version() {
            crate::app_log::note(&format!("no ad slots: WebView2 isn't available: {e}"));
            return None;
        }
        Some(Self {
            owner: handle.hwnd.get(),
            data: data.join("ads"),
            views: HashMap::new(),
            opening: HashSet::new(),
            events: Rc::default(),
        })
    }

    pub(super) fn events(&mut self) -> Vec<Event> {
        std::mem::take(&mut *self.events.borrow_mut())
    }

    pub(super) fn show(&mut self, unit: Unit, url: &str, area: Area, _scale: f32) -> Option<Opener> {
        if let Some(view) = self.views.get_mut(&unit) {
            if view.url != url {
                let _ = view.webview.load_url(url);
                view.url = url.to_owned();
            }
            view.place(self.owner, area);
            return None;
        }
        if !self.opening.insert(unit) {
            return None;
        }
        Some(Opener {
            slot: unit.key(),
            owner: self.owner,
            data: self.data.clone(),
            url: url.to_owned(),
            area,
            events: self.events.clone(),
        })
    }

    /// A hidden slot also tells WebView2, which then throttles the page.
    pub(super) fn hide(&mut self, unit: Unit) {
        if let Some(view) = self.views.get_mut(&unit) {
            view.visible = false;
            let _ = view.webview.set_visible(false);
            unsafe {
                let _ = ShowWindow(hwnd(view.popup), SW_HIDE);
            }
        }
    }

    pub(super) fn close(&mut self, unit: Unit) {
        if let Some(view) = self.views.remove(&unit) {
            let popup = view.popup;
            drop(view);
            unsafe {
                let _ = DestroyWindow(hwnd(popup));
            }
        }
    }

    pub(super) fn adopt(&mut self, unit: Unit, view: Option<View>) {
        self.opening.remove(&unit);
        if let Some(view) = view {
            self.views.insert(unit, view);
        }
    }

    pub(super) fn moved(&mut self, shown: impl Iterator<Item = (Unit, Area)>) {
        for (unit, area) in shown {
            if let Some(view) = self.views.get_mut(&unit) {
                view.place(self.owner, area);
            }
        }
    }
}

impl View {
    fn open(
        owner: isize,
        data: &Path,
        url: &str,
        area: Area,
        report: impl Fn(Consent) + 'static,
    ) -> anyhow::Result<Self> {
        register_class();
        let (x, y) = to_screen(owner, area);
        let popup = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                CLASS,
                w!(""),
                WS_POPUP | WS_CLIPCHILDREN,
                x,
                y,
                area.width as i32,
                area.height as i32,
                Some(hwnd(owner)),
                None,
                GetModuleHandleW(None).ok().map(Into::into),
                None,
            )?
        };
        let mut context = wry::WebContext::new(Some(data.to_path_buf()));
        let background = swofty_ads::slot_background(url).unwrap_or([0, 0, 0]);
        let bounds = view::rect(0, 0, area.width, area.height);
        let webview = view::builder(
            &mut context,
            url,
            bounds,
            background,
            Rc::new(Cell::new(false)),
            report,
        )
        .with_url("about:blank")
        .build_as_child(&Popup(popup.0 as isize))?;
        if let Err(e) = view::allow_trackers(&webview) {
            crate::app_log::note(&format!("an ad slot kept tracking prevention on: {e}"));
        }
        webview.load_url(url)?;
        let mut view = Self {
            webview,
            _context: context,
            popup: popup.0 as isize,
            url: url.to_owned(),
            size: (area.width, area.height),
            visible: false,
        };
        view.place(owner, area);
        Ok(view)
    }

    fn place(&mut self, owner: isize, area: Area) {
        let (x, y) = to_screen(owner, area);
        unsafe {
            let _ = SetWindowPos(
                hwnd(self.popup),
                None,
                x,
                y,
                area.width as i32,
                area.height as i32,
                SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
            );
        }
        if self.size != (area.width, area.height) {
            self.size = (area.width, area.height);
            let _ = self.webview.set_bounds(view::rect(0, 0, area.width, area.height));
        }
        if !self.visible {
            self.visible = true;
            let _ = self.webview.set_visible(true);
        }
    }
}

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut _)
}

fn to_screen(owner: isize, area: Area) -> (i32, i32) {
    let mut point = POINT { x: area.x, y: area.y };
    unsafe {
        let _ = ClientToScreen(hwnd(owner), &mut point);
    }
    (point.x, point.y)
}

fn register_class() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let class = WNDCLASSW {
            lpfnWndProc: Some(slot_proc),
            hInstance: GetModuleHandleW(None).map(Into::into).unwrap_or_default(),
            lpszClassName: CLASS,
            ..Default::default()
        };
        RegisterClassW(&class);
    });
}

/// Clicking a slot leaves the launcher's window active.
unsafe extern "system" fn slot_proc(window: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if message == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

struct Popup(isize);

impl HasWindowHandle for Popup {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = NonZeroIsize::new(self.0).ok_or(HandleError::Unavailable)?;
        Ok(unsafe { WindowHandle::borrow_raw(Win32WindowHandle::new(raw).into()) })
    }
}
