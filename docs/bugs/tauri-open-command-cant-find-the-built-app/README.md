# tauri-open-command-cant-find-the-built-app

Following the documented build steps verbatim, `open "target/release/bundle/macos/Monitor Lizard.app"` silently failed to open the app. The bundle actually lands in the workspace's `target/`, not `src-tauri/target/`, because the preceding `cd src-tauri` changed the directory the `open` command's relative path resolved against.

| File | What it holds |
|---|---|
| [detail.md](detail.md) | What was seen, how to reproduce, the cause |
| [resolution.md](resolution.md) | The fix and how it was checked |
