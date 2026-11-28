# Commands

`make help` is the source of truth. CI runs the same targets, so green locally means green in CI.

| Command | What it runs |
|---|---|
| `make check` | `check-docs` + `check-code` |
| `make check-docs` | `scripts/check-todos.mjs` (board consistency), `scripts/check-doc-indexes.mjs` (every doc is indexed) |
| `make check-code` | `cargo check --locked --all-targets` with warnings as errors. Clippy, duplication (`.jscpd.json` is ready: `npx jscpd .`) and size budgets are not wired yet |
| `make test` | `cargo test --locked` (macOS only; the two hdiutil mount tests skip in CI, see below), then `node --test tests/web/*.test.mjs` for the dashboard's logic (URL hash state, table export). Node 22 or newer. Use the glob: `node --test tests/web` (a directory) fails on Node 24. The tests lift a marked block out of the dashboard scripts (`web/*.js`, assembled by `tests/web/page.mjs`) and do not cover layout |
| `make fmt` | `cargo fmt`. Not in `check`: see the 2026-11-26 note in `AGENTS.md` |
| `make todos` | Prints the derived todo board |
| `make doctor` | Verifies required tools are installed |
| `make ci` | `check` + `test`: the whole fast lane |

## Run the mount tests

Two tests in `src/storage/trash.rs` (`an_item_with_a_volume_mounted_inside_it_is_refused`, `an_item_that_is_itself_a_mount_point_is_refused`) attach a small disk image with `hdiutil` in a temp folder. A hosted macOS runner has frozen during `hdiutil detach -force`, so they skip themselves when `GITHUB_ACTIONS` or `CI` is set and print `SKIPPED IN CI` to stderr. The mount-crossing refusal is therefore verified on a developer's machine only. Run them locally with the skip lifted:

```sh
ML_RUN_MOUNT_TESTS=1 cargo test mount                       # the two trash tests
ML_RUN_MOUNT_TESTS=1 cargo test -- --ignored never_crosses  # the walker's own mount test (ignored by default)
hdiutil info                                                # afterwards: none of the tests' own images is attached
```

Every `hdiutil` call in them has a time limit, and a guard detaches with `-force` and deletes the image even when the test fails. A child that survives the kill for 5 s is abandoned and lives until the test binary exits. CI still runs a unit test of the device comparison (`crosses`).

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
open "../target/release/bundle/macos/Monitor Lizard.app"   # the bundle lands in the workspace's target/, not src-tauri/target/
```

The shell (`src-tauri/`, package `monitor-lizard-shell`) is a second workspace member that links against this crate as a library (`src/lib.rs`) instead of recompiling the modules; `src/main.rs` is the thin CLI binary. It starts the same server in-process on a free port (`server::bind_free()` / `server::serve_on()`), loads the dashboard in a window, and adds a tray item (see ADR 0004 and `docs/todos/done/A17-tauri-menu-bar-shell.md`). Full Disk Access needs a Developer ID signature to survive a rebuild (confirmed by the owner's test, ADR 0004) -- an ad-hoc build loses any grant on its next rebuild.

## Install the local classifier model

```sh
cargo run --release -- model install --model qwen3-4b --yes   # downloads and verifies
cargo run --release -- model status                            # local hash check, no network
```

Without `--yes`, `install` only prints the licence, pinned repo, commit, file and size. The commit came from the Hugging Face API's own `sha` field; the SHA-256 came from the file's git-lfs pointer (`<repo>/raw/<commit>/<file>`), whose `oid sha256:<hex>` line is the git-lfs spec's own content hash. Both are pinned in `src/model_store.rs`'s `MANIFEST`, not fetched at install time, so a later push to the repo cannot change what gets downloaded. `status` never contacts the Hub.

## Turn the local agent on or off

```sh
cargo run --release -- agent on    # kill switch: a running `serve` starts supervising llama-server
cargo run --release -- agent off   # the default; any running llama-server stops within a second or two
```

With the agent on, `serve` spawns `llama-server` (found on `PATH` or Homebrew's usual install prefixes, never bundled or downloaded by this repo) loopback-only with a cleared, allowlisted environment, once the pinned model is installed and memory pressure is not critical. `POST /api/classify?group_key=` (token-guarded, like every other write route) answers with a model-backed verdict when the agent is ready, or the rule label alone otherwise -- the response's `agent_reason` says why (`disabled`, `model-not-installed`, `memory-pressure`, `binary-not-found`, `starting` or `crashed`).

## Look up a command

```sh
cargo run --release -- help [word]   # commands and terms whose title, command or tags contain the word
curl http://127.0.0.1:4747/api/guide # the same table as JSON
```

The table is `COMMANDS` and `TERMS` in `src/guide.rs`. A new `monitor-lizard` subcommand needs an entry there (topic `monitor-lizard`); `make test` fails without one. Examples carry the macOS version they were captured on, and show placeholders such as `<date>` instead of real paths.
