# Commands

`make help` is the source of truth. CI runs the same targets, so green locally means green in CI.

| Command | What it runs |
|---|---|
| `make check` | `check-docs` + `check-code` |
| `make check-docs` | `scripts/check-todos.mjs` (board consistency), `scripts/check-doc-indexes.mjs` (every doc is indexed) |
| `make check-code` | `cargo check --locked --all-targets` with warnings as errors. Clippy, duplication (`.jscpd.json` is ready: `npx jscpd .`) and size budgets are not wired yet |
| `make test` | `cargo test --locked` (macOS only), then `node --test tests/web/*.test.mjs` for the dashboard's logic (URL hash state, table export). Node 22 or newer. Use the glob: `node --test tests/web` (a directory) fails on Node 24. The tests lift a marked block out of `web/index.html` and do not cover layout |
| `make fmt` | `cargo fmt`. Not in `check`: see the 2026-11-26 note in `AGENTS.md` |
| `make todos` | Prints the derived todo board |
| `make doctor` | Verifies required tools are installed |
| `make ci` | `check` + `test`: the whole fast lane |

## Pin tool versions in one place

Pin runtimes in a file CI also reads. Not done yet: CI uses `dtolnay/rust-toolchain@stable` and Node 22 for the doc scripts. Add `rust-toolchain.toml` when a release needs a reproducible build.

## Run the app

```sh
cargo run --release -- serve   # dashboard at http://127.0.0.1:4747
cargo run --release            # grouped snapshot in the terminal
```

## Run the menu-bar shell

```sh
cd src-tauri && cargo tauri build --bundles app
open "target/release/bundle/macos/Monitor Lizard.app"
```

The shell (`src-tauri/`, package `monitor-lizard-shell`) is a second workspace member that links against this crate as a library (`src/lib.rs`) instead of recompiling the modules; `src/main.rs` is the thin CLI binary. It starts the same server in-process on a free port (`server::bind_free()` / `server::serve_on()`), loads the dashboard in a window, and adds a tray item (see ADR 0004 and `docs/todos/done/A17-tauri-menu-bar-shell.md`). Full Disk Access needs a Developer ID signature to survive a rebuild (confirmed by the owner's test, ADR 0004) -- an ad-hoc build loses any grant on its next rebuild.

## Look up a command

```sh
cargo run --release -- help [word]   # commands and terms whose title, command or tags contain the word
curl http://127.0.0.1:4747/api/guide # the same table as JSON
```

The table is `COMMANDS` and `TERMS` in `src/guide.rs`. A new `monitor-lizard` subcommand needs an entry there (topic `monitor-lizard`); `make test` fails without one. Examples carry the macOS version they were captured on, and show placeholders such as `<date>` instead of real paths.
