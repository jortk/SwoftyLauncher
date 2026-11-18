use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use gpui_kit::assets::IconName;
use gpui_kit::*;
use swofty_core::dbedit::draft::DraftFile;
use swofty_core::dbedit::{Kind, Module};
use swofty_core::store::Store;

pub(crate) fn kind_of(spec: Option<Kind>, tag: u8) -> Kind {
    spec.unwrap_or(match tag {
        0 => Kind::Int,
        1 => Kind::Float,
        5 => Kind::Bool,
        6 => Kind::Time,
        9 => Kind::Curve,
        11 => Kind::Color,
        13 => Kind::List,
        14 => Kind::Vector,
        _ => Kind::Text,
    })
}

pub(crate) fn type_name(tag: u8) -> String {
    match tag {
        0 => "int".into(),
        1 => "float".into(),
        2 => "string".into(),
        3 => "enum".into(),
        4 => "id".into(),
        5 => "bool".into(),
        6 => "time".into(),
        9 => "curve".into(),
        11 => "colour".into(),
        13 => "list".into(),
        14 => "vector".into(),
        t => format!("t{t}"),
    }
}

pub(crate) const ADDABLE: [(u8, &str); 9] = [
    (0, "int"),
    (1, "float"),
    (2, "string"),
    (3, "enum"),
    (4, "id"),
    (5, "bool"),
    (6, "time"),
    (13, "list"),
    (14, "vector"),
];

pub(crate) fn icon_named(name: &str) -> IconName {
    static ICONS: LazyLock<HashMap<String, IconName>> = LazyLock::new(|| {
        IconName::ALL
            .iter()
            .map(|i| {
                let path = i.path().to_string();
                let stem = path
                    .trim_start_matches("icons/")
                    .trim_end_matches(".svg")
                    .to_owned();
                (stem, *i)
            })
            .collect()
    });
    ICONS.get(name).copied().unwrap_or(IconName::Table2)
}

/// Rows of these modules reorder by swapping one field with the neighbour row.
pub(crate) const SWAPS: [(&str, &str); 3] = [
    ("calendar", "event_name"),
    ("garage", "team"),
    ("lineups", "entrant_reference"),
];

pub(crate) fn swap_field(module: &Module) -> Option<usize> {
    let (_, field) = SWAPS.iter().find(|(m, _)| *m == module.id)?;
    module.fields.iter().position(|f| f.id == *field)
}

/// English text without the game's `{o:upper}…{/o}` markup.
pub(crate) fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

pub(crate) fn bool_text(tag: u8, stock: &str, on: bool) -> String {
    let (yes, no) = match (tag, stock) {
        (0, _) | (_, "1" | "0") => ("1", "0"),
        (_, "true" | "false") => ("true", "false"),
        _ => ("True", "False"),
    };
    if on { yes } else { no }.into()
}

pub(crate) fn short(v: &str) -> String {
    match v.chars().count() {
        0 => "empty".into(),
        n if n > 18 => format!("{}…", v.chars().take(17).collect::<String>()),
        _ => v.to_owned(),
    }
}

pub(crate) fn hex(key: u64) -> String {
    format!("0x{key:016x}")
}

pub(crate) fn guarded<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err("The database editor is not available in this build.".into()))
}

pub(crate) fn lock(file: &Mutex<DraftFile>) -> std::sync::MutexGuard<'_, DraftFile> {
    file.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(crate) fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

pub(crate) fn home(store: &Store) -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map_or_else(|| store.root.clone(), PathBuf::from)
}

/// Debug builds only: a file (`SWOFTY_DEBUG_IMPORT`) or folder (`SWOFTY_DEBUG_EXPORT`)
/// used instead of the file dialogs, which need a desktop portal.
pub(crate) fn debug_path(var: &str) -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        std::env::var_os(var).map(PathBuf::from)
    } else {
        None
    }
}
