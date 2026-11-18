//! The launcher's translations. The English text in the code is the key:
//! `tr("Settings")` looks it up in the chosen language, and text with no
//! translation stays English.
//!
//! Besides `tr` there is `trf!` for text with `{names}` in it, `trn` for counts
//! (`trn(n, "{count} addon", "{count} addons")`) and `trc` for the same English
//! meaning two things (`trc("verb", "Open")`).
//!
//! To add a language, add `locales/<code>.json` with its name under `_language`
//! and what it translates. It then shows up under Language in Settings.

use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicUsize, Ordering};

mod plural;
mod pseudo;

pub use plural::{Category, categories, category};
pub use pseudo::{CODE as PSEUDO, enabled as pseudo_enabled};

include!(concat!(env!("OUT_DIR"), "/locales.rs"));

pub const ENGLISH: &str = "en";
/// Where a locale file gives its language's own name.
const NAME_KEY: &str = "_language";
/// Between a context and its text in a locale file's key: `"verb\u0004Open"`.
const CONTEXT_MARK: char = '\u{4}';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Language {
    pub code: &'static str,
    pub name: &'static str,
}

/// What a locale file gives for one key: a text, or one text per plural form in
/// the order of [`categories`] for the language.
enum Entry {
    One(&'static str),
    Forms(Vec<&'static str>),
}

struct Loaded {
    language: Language,
    table: HashMap<&'static str, Entry>,
    pseudo: bool,
}

/// English first, so index 0 is what an unknown language falls back to, and the
/// pseudo-locale last. The text lives as long as the launcher, which lets `tr`
/// hand back a `&'static str`.
static LOADED: LazyLock<Vec<Loaded>> = LazyLock::new(|| {
    let mut all: Vec<Loaded> = FILES.iter().filter_map(|(code, text)| load(code, text)).collect();
    all.sort_by_key(|l| (l.language.code != ENGLISH, l.language.code));
    if pseudo::enabled() {
        all.push(Loaded {
            language: Language {
                code: pseudo::CODE,
                name: pseudo::NAME,
            },
            table: HashMap::new(),
            pseudo: true,
        });
    }
    all
});
static CURRENT: AtomicUsize = AtomicUsize::new(0);

fn entry(value: serde_json::Value) -> Option<Entry> {
    match value {
        serde_json::Value::String(s) => Some(Entry::One(s.leak())),
        serde_json::Value::Array(forms) => forms
            .into_iter()
            .map(|f| f.as_str().map(|s| &*s.to_owned().leak()))
            .collect::<Option<Vec<_>>>()
            .filter(|f| !f.is_empty())
            .map(Entry::Forms),
        _ => None,
    }
}

/// A file that can't be read leaves its language out rather than stopping the
/// launcher; the tests here fail on one.
fn load(code: &'static str, text: &str) -> Option<Loaded> {
    let mut raw: BTreeMap<String, serde_json::Value> = serde_json::from_str(text).ok()?;
    let name: &'static str = raw.remove(NAME_KEY)?.as_str()?.to_owned().leak();
    let table = raw
        .into_iter()
        .map(|(k, v)| Some((&*k.leak(), entry(v)?)))
        .collect::<Option<_>>()?;
    Some(Loaded {
        language: Language { code, name },
        table,
        pseudo: false,
    })
}

fn current() -> Option<&'static Loaded> {
    LOADED.get(CURRENT.load(Ordering::Relaxed))
}

pub fn languages() -> Vec<Language> {
    LOADED.iter().map(|l| l.language).collect()
}

/// `code` from now on; an unknown code, or `None`, is English.
pub fn set_language(code: Option<&str>) {
    let found = code.and_then(|code| LOADED.iter().position(|l| l.language.code == code));
    CURRENT.store(found.unwrap_or(0), Ordering::Relaxed);
}

pub fn language() -> &'static str {
    current().map_or(ENGLISH, |l| l.language.code)
}

fn lookup(l: &'static Loaded, key: &str) -> Option<&'static str> {
    match l.table.get(key)? {
        Entry::One(text) => Some(text),
        Entry::Forms(_) => None,
    }
}

/// `text` in the chosen language.
pub fn tr(text: &'static str) -> &'static str {
    match current() {
        Some(l) if l.pseudo => pseudo::pseudo(text),
        Some(l) => lookup(l, text).unwrap_or(text),
        None => text,
    }
}

/// `text` in the chosen language, for when `context` tells apart two meanings of
/// the same English: `trc("verb", "Open")` and `trc("state", "Open")`. A locale
/// file keys it as `"verb\u0004Open"`; without a translation it is just `text`.
pub fn trc(context: &str, text: &'static str) -> &'static str {
    match current() {
        Some(l) if l.pseudo => pseudo::pseudo(text),
        Some(l) => in_context(l, context, text),
        None => text,
    }
}

fn in_context(l: &'static Loaded, context: &str, text: &'static str) -> &'static str {
    lookup(l, &format!("{context}{CONTEXT_MARK}{text}")).unwrap_or(text)
}

/// A whole number of things, from any of the integer types a count comes in.
pub trait Count {
    fn count(self) -> u64;
}

macro_rules! counts {
    ($($t:ty),*) => {$(
        impl Count for $t {
            fn count(self) -> u64 {
                u64::try_from(self).unwrap_or(0)
            }
        }
    )*};
}

counts!(u8, u16, u32, u64, usize, i32, i64);

fn english_form(n: u64, one: &'static str, other: &'static str) -> &'static str {
    if n == 1 { one } else { other }
}

/// The text for `n` things, with `{count}` filled in. `one` is the English form for
/// 1 and also the key in a locale file, where the entry is a list with one text per
/// plural form of the language, in the order of [`categories`]:
///
/// ```json
/// "{count} addon": ["{count} аддон", "{count} аддона", "{count} аддонов"]
/// ```
pub fn trn(n: impl Count, one: &'static str, other: &'static str) -> String {
    let n = n.count();
    let form = match current() {
        Some(l) if l.pseudo => pseudo::pseudo(english_form(n, one, other)),
        Some(l) => match l.table.get(one) {
            Some(Entry::Forms(forms)) => {
                let wanted = category(l.language.code, n);
                let at = categories(l.language.code).iter().position(|c| *c == wanted);
                forms[at.unwrap_or(forms.len() - 1).min(forms.len() - 1)]
            }
            Some(Entry::One(text)) => text,
            None => english_form(n, one, other),
        },
        None => english_form(n, one, other),
    };
    fill(form, &[("count", n.to_string())])
}

#[cfg(test)]
fn translate(code: &str, text: &'static str) -> &'static str {
    LOADED
        .iter()
        .find(|l| l.language.code == code)
        .and_then(|l| lookup(l, text))
        .unwrap_or(text)
}

/// `text` with each `{name}` replaced by its value, once: a value holding `{x}`
/// stays as it is. Any other `{` stays too.
pub fn fill(text: &str, args: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let found = after.find('}').and_then(|close| {
            let value = args.iter().find(|(name, _)| *name == &after[..close])?;
            Some((close, &value.1))
        });
        match found {
            Some((close, value)) => {
                out.push_str(value);
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// [`tr`] for text with values in it: `trf!("{count} addons", count = n)`.
#[macro_export]
macro_rules! trf {
    ($text:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::fill($crate::tr($text), &[$((stringify!($name), ($value).to_string())),*])
    };
}

#[cfg(test)]
mod scan;

#[cfg(test)]
mod ratchet;

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::scan::{launcher_source, used_keys};

    fn files() -> Vec<(&'static str, BTreeMap<String, Value>)> {
        FILES
            .iter()
            .map(|(code, text)| {
                let table = serde_json::from_str(text).unwrap_or_else(|e| panic!("{code}.json: {e}"));
                (*code, table)
            })
            .collect()
    }

    fn texts(value: &Value) -> Vec<&str> {
        match value {
            Value::String(s) => vec![s],
            Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
            _ => Vec::new(),
        }
    }

    fn placeholders(text: &str) -> BTreeSet<&str> {
        text.split('{')
            .skip(1)
            .filter_map(|rest| rest.split_once('}'))
            .map(|(name, _)| name)
            .collect()
    }

    #[test]
    fn a_language_shows_its_own_name_with_english_first() {
        let all = languages();
        assert_eq!(
            all[0],
            Language {
                code: ENGLISH,
                name: "English"
            }
        );
        assert!(
            all.contains(&Language {
                code: "de",
                name: "Deutsch"
            }),
            "{all:?}"
        );
        assert!(
            all.contains(&Language {
                code: "es",
                name: "Español"
            }),
            "{all:?}"
        );
        assert!(
            all.contains(&Language {
                code: "ko",
                name: "한국어"
            }),
            "{all:?}"
        );
        assert!(
            all.contains(&Language {
                code: "pt-BR",
                name: "Português (Brasil)"
            }),
            "{all:?}"
        );
        assert!(
            all.contains(&Language {
                code: "ru",
                name: "Русский"
            }),
            "{all:?}"
        );
        assert!(
            all.contains(&Language {
                code: "zh-CN",
                name: "简体中文"
            }),
            "{all:?}"
        );
    }

    #[test]
    fn text_is_translated_and_anything_unknown_stays_english() {
        assert_eq!(translate("ru", "Settings"), "Настройки");
        assert_eq!(translate("zh-CN", "Settings"), "设置");
        assert_eq!(translate(ENGLISH, "Settings"), "Settings");
        assert_eq!(
            translate("ru", "Text nobody translated"),
            "Text nobody translated"
        );
        assert_eq!(translate("xx", "Settings"), "Settings");
    }

    #[test]
    fn fill_replaces_each_name_once_and_leaves_the_rest() {
        let args = [("a", "{b}".to_owned()), ("b", "2".to_owned())];
        assert_eq!(fill("{a} and {b} and {c} {", &args), "{b} and 2 and {c} {");
    }

    #[test]
    fn every_locale_entry_is_a_text_or_a_list_of_texts_with_the_right_form_count() {
        for (code, table) in files() {
            for (key, value) in table.iter().filter(|(k, _)| *k != NAME_KEY) {
                let all = texts(value);
                assert!(!all.is_empty(), "{code}.json: {key:?} has no text");
                if let Value::Array(items) = value {
                    assert_eq!(all.len(), items.len(), "{code}.json: {key:?} holds a non-text");
                    assert_eq!(
                        items.len(),
                        categories(code).len(),
                        "{code}.json: {key:?} needs one text per plural form"
                    );
                }
            }
        }
    }

    #[test]
    fn every_locale_file_names_its_language_and_keeps_the_placeholders() {
        for (code, table) in files() {
            let name = table.get(NAME_KEY).and_then(Value::as_str).unwrap_or("");
            assert!(!name.is_empty(), "{code}.json has no {NAME_KEY}");
            for (text, value) in table.iter().filter(|(k, _)| *k != NAME_KEY) {
                let wanted = placeholders(text.rsplit(CONTEXT_MARK).next().unwrap_or(text));
                for translated in texts(value) {
                    if matches!(value, Value::Array(_)) {
                        assert!(
                            placeholders(translated).is_subset(&wanted),
                            "{code}.json: {text:?} -> {translated:?}"
                        );
                    } else {
                        assert_eq!(
                            wanted,
                            placeholders(translated),
                            "{code}.json: {text:?} -> {translated:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_translation_is_of_text_the_launcher_still_has() {
        let used = used_keys(&launcher_source());
        for (code, table) in files() {
            for text in table.keys().filter(|k| *k != NAME_KEY) {
                assert!(
                    used.contains(text.as_str()),
                    "{code}.json translates {text:?}, which no tr() uses"
                );
            }
        }
    }

    static LANGUAGE: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn in_language<T>(code: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _held = LANGUAGE.lock().unwrap_or_else(|e| e.into_inner());
        set_language(code);
        let out = f();
        set_language(None);
        out
    }

    #[test]
    fn choosing_a_language_changes_what_tr_returns() {
        in_language(Some("ru"), || {
            assert_eq!((language(), tr("Settings")), ("ru", "Настройки"));
            assert_eq!(trf!("Settings {n}", n = 2), "Settings 2");
        });
        in_language(Some("no-such-language"), || {
            assert_eq!((language(), tr("Settings")), (ENGLISH, "Settings"));
        });
        in_language(Some("zh-CN"), || assert_eq!(tr("Settings"), "设置"));
        in_language(None, || assert_eq!(language(), ENGLISH));
    }

    #[test]
    fn trn_picks_the_english_form_and_fills_the_count() {
        in_language(None, || {
            assert_eq!(trn(1usize, "{count} addon", "{count} addons"), "1 addon");
            assert_eq!(trn(0usize, "{count} addon", "{count} addons"), "0 addons");
            assert_eq!(trn(5u64, "{count} addon", "{count} addons"), "5 addons");
        });
    }

    #[test]
    fn trn_uses_the_plural_forms_of_the_language() {
        in_language(Some("ru"), || {
            let word = |n: u64| trn(n, "{count} minute ago", "{count} minutes ago");
            assert_eq!(word(1), "1 минуту назад");
            assert_eq!(word(3), "3 минуты назад");
            assert_eq!(word(11), "11 минут назад");
            assert_eq!(word(21), "21 минуту назад");
        });
        in_language(Some("zh-CN"), || {
            assert_eq!(trn(1u64, "{count} minute ago", "{count} minutes ago"), "1 分钟前");
            assert_eq!(trn(9u64, "{count} minute ago", "{count} minutes ago"), "9 分钟前");
        });
    }

    #[test]
    fn trc_looks_up_the_context_and_falls_back_to_the_text() {
        let file = r#"{"_language": "Test", "verb\u0004Open": "Ouvrir", "state\u0004Open": "Ouvert"}"#;
        let loaded: &'static Loaded = Box::leak(Box::new(load("xx", file).unwrap()));
        assert_eq!(in_context(loaded, "verb", "Open"), "Ouvrir");
        assert_eq!(in_context(loaded, "state", "Open"), "Ouvert");
        assert_eq!(in_context(loaded, "other", "Open"), "Open");
        in_language(None, || assert_eq!(trc("verb", "Open"), "Open"));
    }

    #[test]
    fn the_pseudo_locale_wraps_and_lengthens_text() {
        assert!(
            languages().iter().any(|l| l.code == PSEUDO),
            "debug builds offer it"
        );
        in_language(Some(PSEUDO), || {
            assert_eq!(tr("Settings"), "[Šéţţíñğš~~]");
            assert_eq!(trn(2u64, "{count} addon", "{count} addons"), "[2 áďďöñš~~]");
        });
    }
}
