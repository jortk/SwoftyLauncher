# Development

Run your build commands from the repository root. For current progress and what's planned, see the [design notes](/design/).

## Source folders

| Path | Contents |
|---|---|
| `apps/launcher` | Desktop app |
| `apps/cli` | `swofty` command |
| `packages/core` | Addon management, profiles, launch setup and restore |
| `packages/addon-api` | Addon manifests, asset operations and launcher plugin types (`swofty-addon`) |
| `packages/formats` | Game file readers and writers |
| `packages/sdk` | Plugin SDK (MIT) |
| `native/` | In-game loader |
| `addons/builtins` | The built-in addons compiled into the launcher |
| `addons/swoftymod` | Built-in addon |
| `templates/addon` | Sample data addon with an engine plugin (MIT) |
| `templates/shader-pack` | Sample shader pack with three looks (MIT) |
| `templates/launcher-addon` | Sample launcher plugin (MIT) |
| `installer/` | The installer that downloads the latest release |
| `tools/` | Python tools for game files |
| `re/` | Game research and signatures |
| `docs/` | This site |

## Build and check

If you're on NixOS, use `nix develop` to get the development tools and libraries.

Build and check the workspace with the commands below. The launcher isn't one of the default workspace members, so you'll need the last command to build it separately.

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all --check
cargo build -p swofty-launcher
```

## Release downloads

`.github/workflows/release.yml` publishes the GitHub release, then runs `tools/release/cdn-publish.sh`, which needs the repository secrets `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_ENDPOINT` and `R2_BUCKET` and skips with a warning when they are missing. It uploads the archives and `SHA256SUMS` to `launcher/<tag>/` in the `swoftylauncher` bucket, downloads each back from cdn.swofty.net to check its SHA-256, copies the installers to `launcher/latest/`, and writes `launcher/latest.json` last:

```json
{
  "schema": 1,
  "version": "1.8.29",
  "tag": "v1.8.29",
  "published_at": "2026-11-08T09:16:53Z",
  "notes_url": "https://github.com/Swofty-Developments/SwoftyLauncher/releases/tag/v1.8.29",
  "assets": [
    { "name": "swofty-launcher-v1.8.29-linux-x86_64.tar.gz", "url": "https://cdn.swofty.net/launcher/v1.8.29/swofty-launcher-v1.8.29-linux-x86_64.tar.gz", "size": 86573884, "sha256": "2594…" }
  ]
}
```

Versioned files are immutable; `latest.json` and `launcher/latest/` are cached for a minute and five minutes. A pre-release tag uploads its files and leaves `latest.json` alone. `cargo test -p swofty-core --lib live_cdn -- --ignored` fetches the live manifest and downloads and verifies the asset for this platform.

## Test updates

In a debug build, `SWOFTY_TEST_GITHUB=http://127.0.0.1:<port>` sends the launcher's and `swofty update`'s GitHub requests to a local server instead: `https://github.com/<path>` and `https://api.github.com/<path>` both become `<server>/<path>`. `https://cdn.swofty.net/<path>` becomes `<server>/cdn/<path>`, so a server with no `/cdn/launcher/latest.json` makes the launcher use its GitHub routes. Release builds ignore it. Combine it with `SWOFTY_UPDATE_PRETEND_VERSION` and a `SWOFTY_DATA_DIR` of your own to run an update end to end.

## Isolate Steam discovery

For tests or sandboxed runs, set `SWOFTY_STEAM_ROOT_ONLY` to a Steam root to restrict Steam and save discovery to that folder. Unlike `SWOFTY_STEAM_ROOT`, this disables fallback to registered installations, default locations and drive scans. A missing or invalid root produces no Steam roots; it does not fall back to the host.

## Test with game files

Point `F1_25_DIR` at your game files to run these tests; leave it unset and they're skipped. The tests only read the files.

```sh
F1_25_DIR="/path/to/F1 25" cargo test -p swofty-formats
```

To compare with the Python tools, first set `F1_25_DIR` in your shell. Then run:

```sh
python3 tools/gamedb.py roundtrip "$F1_25_DIR/game_package/gamedatabases/gamedatabases.2025.bin"
python3 tools/erpw.py rebuild "$F1_25_DIR/2025_asset_groups/ui_package/databases.erp" /tmp/databases.erp
```

To check the ADDON SETTINGS build, run the test below. It compares the result byte for byte with a reference build of the same session (`packages/core/tests/fixtures/menu_session.json`). Make sure `F1_25_DIR` holds unmodified UI archives.

```sh
F1_25_DIR="/path/to/F1 25" SWOFTY_MENU_ORACLE=/path/to/reference cargo test -p swofty-core --test real_menus
```

To check track edits alongside ADDON SETTINGS, set `SWOFTY_MAPS_DIR` to a folder holding an imported `bahrain24` package. The same test file then checks that both are written into a single `databases.erp` and a single `persistent.erp`.

## Native loader

Build the loader, then run the Wine test:

```sh
native/build.sh
native/test/run_wine_test.sh
```

The build uses Zig to produce Windows DLLs; you'll find them in `native/dist/`. The Wine test checks the loader and ADDON SETTINGS menu entry with a small test executable.

## Documentation

```sh
pnpm install
pnpm docs:dev
pnpm docs:build
```

## Commits

Write your commit messages using [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/). When you commit, the hook checks Rust formatting and clippy.
