# Commands

`make help` is the source of truth. CI runs the same targets, so green locally means green in CI.

| Command | What it runs |
|---|---|
| `make check` | `check-docs` + `check-code` |
| `make check-docs` | `scripts/check-todos.mjs` (board consistency), `scripts/check-doc-indexes.mjs` (every doc is indexed) |
| `make check-code` | `cargo check --locked --all-targets` with warnings as errors. Clippy, duplication (`.jscpd.json` is ready: `npx jscpd .`) and size budgets are not wired yet |
| `make test` | `cargo test --locked` (macOS only) |
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
