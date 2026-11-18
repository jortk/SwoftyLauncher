use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::{App, BorrowAppContext, Global, SharedString, Window};

pub(crate) const CAP: usize = 50;
pub(crate) const POPUP_FOR: Duration = Duration::from_secs(6);
pub(crate) const POPUPS_SHOWN: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Info,
    Success,
    Warning,
    Error,
}

type Action = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
pub(crate) struct Notification {
    pub kind: Kind,
    pub title: Option<SharedString>,
    pub message: SharedString,
    pub sticky: bool,
    pub action: Option<(SharedString, Action)>,
}

impl Notification {
    fn of(kind: Kind, message: impl Into<SharedString>) -> Self {
        Self {
            kind,
            title: None,
            message: message.into(),
            sticky: kind == Kind::Error,
            action: None,
        }
    }

    pub(crate) fn info(message: impl Into<SharedString>) -> Self {
        Self::of(Kind::Info, message)
    }

    pub(crate) fn success(message: impl Into<SharedString>) -> Self {
        Self::of(Kind::Success, message)
    }

    pub(crate) fn warning(message: impl Into<SharedString>) -> Self {
        Self::of(Kind::Warning, message)
    }

    pub(crate) fn error(message: impl Into<SharedString>) -> Self {
        Self::of(Kind::Error, message)
    }

    pub(crate) fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub(crate) fn autohide(mut self, autohide: bool) -> Self {
        self.sticky = !autohide;
        self
    }

    pub(crate) fn action(
        mut self,
        label: impl Into<SharedString>,
        run: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), Rc::new(run)));
        self
    }
}

#[derive(Clone)]
pub(crate) struct Entry {
    pub id: u64,
    pub note: Notification,
    pub at: Instant,
    pub read: bool,
    pub popped: bool,
}

#[derive(Default)]
pub(crate) struct Notices {
    entries: Vec<Entry>,
    next: u64,
}

impl Global for Notices {}

impl Notices {
    pub(crate) fn add(&mut self, note: Notification, at: Instant) -> u64 {
        let id = self.next;
        self.next += 1;
        self.entries.insert(
            0,
            Entry {
                id,
                note,
                at,
                read: false,
                popped: false,
            },
        );
        self.entries.truncate(CAP);
        id
    }

    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub(crate) fn unread(&self) -> usize {
        self.entries.iter().filter(|e| !e.read).count()
    }

    pub(crate) fn mark_read(&mut self) {
        for e in &mut self.entries {
            e.read = true;
        }
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn dismiss_popup(&mut self, id: u64) {
        for e in self.entries.iter_mut().filter(|e| e.id == id) {
            e.popped = true;
        }
    }

    pub(crate) fn dismiss_popups(&mut self) {
        for e in &mut self.entries {
            e.popped = true;
        }
    }

    pub(crate) fn popups(&self, now: Instant) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| !e.popped && !e.read)
            .filter(|e| e.note.sticky || now.duration_since(e.at) < POPUP_FOR)
            .take(POPUPS_SHOWN)
            .collect()
    }
}

pub(crate) trait Notify {
    fn notify(&mut self, note: impl Into<Notification>, cx: &mut App);
}

impl Notify for Window {
    fn notify(&mut self, note: impl Into<Notification>, cx: &mut App) {
        let note = note.into();
        cx.update_global(|n: &mut Notices, _| {
            n.add(note, Instant::now());
        });
        cx.refresh_windows();
    }
}

impl From<String> for Notification {
    fn from(s: String) -> Self {
        Self::info(s)
    }
}

impl From<&str> for Notification {
    fn from(s: &str) -> Self {
        Self::info(s.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(text: &str) -> Notification {
        Notification::info(text.to_owned())
    }

    #[test]
    fn newest_first_and_unread_counted() {
        let mut n = Notices::default();
        let t = Instant::now();
        n.add(note("a"), t);
        n.add(note("b"), t);
        assert_eq!(n.entries()[0].note.message.as_ref(), "b");
        assert_eq!(n.unread(), 2);
        n.mark_read();
        assert_eq!(n.unread(), 0);
        n.add(note("c"), t);
        assert_eq!(n.unread(), 1);
    }

    #[test]
    fn the_list_is_capped() {
        let mut n = Notices::default();
        let t = Instant::now();
        for i in 0..CAP + 10 {
            n.add(note(&i.to_string()), t);
        }
        assert_eq!(n.entries().len(), CAP);
        assert_eq!(n.entries()[0].note.message.as_ref(), (CAP + 9).to_string());
    }

    #[test]
    fn clear_empties_the_list() {
        let mut n = Notices::default();
        n.add(note("a"), Instant::now());
        n.clear();
        assert!(n.entries().is_empty());
        assert_eq!(n.unread(), 0);
    }

    #[test]
    fn popups_expire_unless_sticky_and_stop_once_read() {
        let mut n = Notices::default();
        let t = Instant::now();
        n.add(note("quick"), t);
        n.add(Notification::error("bad".to_owned()), t);
        assert_eq!(n.popups(t).len(), 2);
        let later = t + POPUP_FOR + Duration::from_secs(1);
        let left = n.popups(later);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].note.message.as_ref(), "bad");
        n.mark_read();
        assert!(n.popups(t).is_empty());
    }
}
