# `open "target/release/bundle/macos/Monitor Lizard.app"` doesn't open the app

Resolution: [resolution.md](resolution.md)

## What we saw

Following the documented build steps verbatim -- `cd src-tauri && cargo tauri build --bundles app` immediately followed by `open "target/release/bundle/macos/Monitor Lizard.app"` -- the `open` command does nothing (no window, no error visible in a typical terminal that scrolled past it). Reported by the owner: "running `open \"target/release/bundle/macos/Monitor Lizard.app\"` doesn't open the app."

## How to reproduce

```sh
cd src-tauri && cargo tauri build --bundles app
open "target/release/bundle/macos/Monitor Lizard.app"
```

Run as two lines in the same shell session, exactly as `README.md` and `docs/agents/commands.md` showed them.

## Measurements

```
$ (cd src-tauri && cargo tauri build --bundles app >/dev/null 2>&1; open "target/release/bundle/macos/Monitor Lizard.app")
The file /Users/laudebugs/git/ownwatch-main/src-tauri/target/release/bundle/macos/Monitor Lizard.app does not exist.
```

The real bundle is one level up:

```
$ ls "target/release/bundle/macos/Monitor Lizard.app"      # from the repo root
Contents
$ ls "src-tauri/target/release/bundle/macos/Monitor Lizard.app"   # from the repo root
ls: src-tauri/target/release/bundle/macos/Monitor Lizard.app: No such file or directory
```

## Cause

`Cargo.toml` at the repo root declares `[workspace] members = ["src-tauri"]`. Cargo always writes build artifacts, including a Tauri bundle, to the workspace root's `target/`, never a member crate's own `target/`, regardless of which directory you ran the build from. The documented sequence did `cd src-tauri` for the build step and then used a path relative to that same, now-changed directory for `open`, so it looked for the bundle at `src-tauri/target/release/bundle/macos/...` -- a path that has never existed. This is the same README section the owner's "add build/sign/run instructions" request added. The commands were copied from `docs/agents/commands.md`, which had carried the identical bug since the shell was first added (`#52`). Neither file was ever run as a literal copy-paste in one shell session before this.
