# swofty-i18n

The launcher's translations. The English text in the code is the key, and text with no translation stays in English.

## Add a language

Add `locales/<code>.json`, with the language's own name under `_language`:

```json
{
  "_language": "Deutsch",
  "Settings": "Einstellungen",
  "{name} is the active profile": "{name} ist das aktive Profil"
}
```

It then shows up under **Language** in Settings. You don't have to translate everything: copy `ru.json` for the full list, and leave out what you haven't done yet.

The launcher's layout runs left to right only, so a right-to-left language won't lay out correctly.

## Counts

`trn(n, one, other)` is for text that changes with a number. The English `one` form is the key, `{count}` is filled in, and a locale file gives one text per plural form of its language, in this order:

| Language | Forms |
|---|---|
| English, German, Dutch, Spanish, Italian and others | one, other |
| Russian, Ukrainian, Belarusian, Polish | one, few, many |
| Czech, Slovak | one, few, other |
| French, Portuguese | one, many, other |
| Arabic | zero, one, two, few, many, other |
| Chinese, Japanese, Korean, Vietnamese, Thai, Indonesian | other |

```rust
trn(n, "{count} addon", "{count} addons")
```

```json
"{count} addon": ["{count} аддон", "{count} аддона", "{count} аддонов"]
```

To give a language its rules, add it to `categories` and `category` in `src/plural.rs`.

## Same word, two meanings

`trc(context, text)` looks up `context` and `text` together. In a locale file the key is the context, the character `\u0004`, then the text:

```rust
trc("verb", "Open")
trc("state", "Open")
```

```json
"verb\u0004Open": "Ouvrir",
"state\u0004Open": "Ouvert"
```

Without an entry, the text is shown as it is.

## Pseudo-locale

`en-XA` is English with accents, about 30% longer, in brackets. Pick it under Language to find labels that clip or overflow. It is listed in debug builds, and in release builds only when `SWOFTY_PSEUDO_LOCALE` is set.

## Translate more of the launcher

Wrap the text where the code uses it, then add it to the locale files:

```rust
use swofty_i18n::{tr, trf};

.child(tr("Settings"))
.child(trf!("{name} is the active profile", name = name))
```

`tr` returns a `&'static str`, so it fits wherever a literal did. Keep the text in the call: `cargo test -p swofty-i18n` looks for it there, and fails if a locale file translates text the launcher no longer has or drops a `{name}`.

Text on `.child(`, `.label(`, `.title(`, `.tooltip(` and the like that is not wrapped is counted per file in `ratchet.json`, and the test fails when a file has more than its number. After wrapping text, lower the numbers with:

```sh
SWOFTY_RATCHET_UPDATE=1 cargo test -p swofty-i18n
```

`cargo test -p swofty-i18n coverage -- --ignored --nocapture` prints how much of the launcher each language translates.
