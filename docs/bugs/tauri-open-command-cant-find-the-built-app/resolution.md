# Resolution: `open "target/release/bundle/macos/Monitor Lizard.app"` doesn't open the app

Detail: [detail.md](detail.md)

## The fix

Every `open`/`codesign` command that follows `cd src-tauri && cargo tauri build --bundles app` in `README.md` and `docs/agents/commands.md` now points one level up: `"../target/release/bundle/macos/Monitor Lizard.app"`, with a comment saying why (the bundle lands in the workspace's `target/`, not `src-tauri/target/`). No code changed; this was a docs-only bug.

## How it was checked

| Check | Before | After |
|---|---|---|
| `cd src-tauri && cargo tauri build --bundles app` then `open "target/release/bundle/macos/Monitor Lizard.app"`, as one shell session | `The file .../src-tauri/target/release/bundle/macos/Monitor Lizard.app does not exist.`, exit code 1 | not the documented command any more |
| Same, with the corrected `open "../target/release/bundle/macos/Monitor Lizard.app"` | not applicable | the app opens: its window and tray item appear |
| `grep -rn 'open "target/release/bundle' . --include="*.md"` excluding the frozen spike report in `docs/research/` | three live-instruction hits (`README.md` x2, `docs/agents/commands.md`) | zero |

No regression test: these are prose code blocks in Markdown, not something `tests/web` or `cargo test` reads. The check above is the owner's own repro, run against both the broken and fixed commands.

## Left for later

`docs/research/tauri-shell-spike/README.md` keeps the same unprefixed path in its own transcript (a different, now-deleted spike worktree that may not have been a Cargo workspace member the same way). It is a frozen record of what the owner actually ran during that spike, not live instructions, so it was left as-is.
