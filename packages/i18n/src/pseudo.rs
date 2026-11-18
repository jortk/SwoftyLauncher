//! The pseudo-locale: English with accents and about 30% more length, with its
//! `{names}` untouched. It shows what clips or overflows before a translator does.

use std::collections::HashMap;
use std::sync::Mutex;

pub const CODE: &str = "en-XA";
pub const NAME: &str = "Pseudo (en-XA)";

/// Whether the pseudo-locale is offered: always in debug builds, and in release
/// builds only with `SWOFTY_PSEUDO_LOCALE` set.
pub fn enabled() -> bool {
    cfg!(debug_assertions) || std::env::var_os("SWOFTY_PSEUDO_LOCALE").is_some()
}

fn accent(c: char) -> char {
    match c {
        'a' => 'á',
        'b' => 'ƀ',
        'c' => 'ç',
        'd' => 'ď',
        'e' => 'é',
        'g' => 'ğ',
        'h' => 'ĥ',
        'i' => 'í',
        'k' => 'ķ',
        'l' => 'ł',
        'n' => 'ñ',
        'o' => 'ö',
        'r' => 'ŕ',
        's' => 'š',
        't' => 'ţ',
        'u' => 'ü',
        'w' => 'ŵ',
        'y' => 'ý',
        'z' => 'ž',
        'A' => 'Å',
        'C' => 'Ç',
        'E' => 'É',
        'I' => 'Î',
        'O' => 'Ö',
        'S' => 'Š',
        'U' => 'Û',
        'Y' => 'Ý',
        other => other,
    }
}

fn transform(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2 + 2);
    out.push('[');
    let mut in_name = false;
    let mut shown = 0;
    for c in text.chars() {
        match c {
            '{' => in_name = true,
            '}' => in_name = false,
            _ => {}
        }
        out.push(if in_name { c } else { accent(c) });
        if !in_name {
            shown += 1;
        }
    }
    out.extend(std::iter::repeat_n('~', (shown * 3 / 10).max(2)));
    out.push(']');
    out
}

static CACHE: Mutex<Option<HashMap<&'static str, &'static str>>> = Mutex::new(None);

/// `text` in the pseudo-locale. Each text is made once and kept for the run.
pub fn pseudo(text: &'static str) -> &'static str {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let cache = guard.get_or_insert_with(HashMap::new);
    cache.entry(text).or_insert_with(|| transform(text).leak())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_accents_pads_and_keeps_names() {
        assert_eq!(transform("Settings"), "[Šéţţíñğš~~]");
        assert_eq!(transform("{count} items"), "[{count} íţémš~~]");
    }
}
