# On first launch the dashboard sticks on "Collecting the first snapshot...", and only a reload fixes it

Resolution: [resolution.md](resolution.md)

## What we saw

Right after starting the server or opening the Tauri shell for the first time, the Overview page (and every other page that needs a snapshot) stays on "Collecting the first snapshot..." for a long time -- long enough to look hung. Reloading the page fixes it immediately. Reported by the owner: "on first launch, the app isn't picking up the UI... looks like something's hanging, but on reload, it works. Same as a desktop app."

## How to reproduce

1. Build the Tauri shell: `cd src-tauri && cargo tauri build --bundles app`.
2. Launch it with a fresh, empty `HOME` so no snapshot has ever been cached: `HOME=/tmp/ml-coldstart "target/release/bundle/macos/Monitor Lizard.app/Contents/MacOS/monitor-lizard-shell"`.
3. Watch the window right after it opens: the nav and chrome render immediately, but the content area reads "Collecting the first snapshot..." Reload (Cmd+R) once it has been open a couple of seconds and it loads instantly.

The same reproduces with the plain CLI (`monitor-lizard serve`) in a browser tab with a fresh `HOME`; it is not Tauri-specific.

## Measurements

Timed against the real binary, fresh `HOME`, on this machine (423 apps, 227 outbound sockets):

| Elapsed since launch | What happened |
|---|---|
| 0.90 s | The server's loopback port is bound and accepting connections |
| 1.10 s | `GET /` returns 200 (the page itself loads fine) ; `GET /api/snapshot` returns the 4-byte body `null` |
| 1.47 s | `GET /api/snapshot` still returns `null` |
| 1.73 s | `GET /api/snapshot` returns the real snapshot (567 KB) |

So the real data is ready well under 2 seconds after launch. `web/shell.js` calls `load()` once at boot and then every 10 seconds (`setInterval(load, 10000)`). The boot-time call almost always lands inside that roughly 1-2 s window and gets `null`; the next attempt is not for another 10 seconds. The page is correctly showing "Collecting the first snapshot..." the whole time (`web/shell.js`'s `render()` already has that branch for `!S.snap`) -- it just has no reason to check again for 10 seconds, which reads as hung. A manual reload re-runs the boot sequence immediately, and by then (a few seconds in) the cache is long since warm, so it loads on the first try.

## Cause

`src/server.rs`'s `cached()` helper returns the JSON string `"null"` for a `Cache` whose background `refresh()` thread has not completed its first computation yet -- a deliberate, reasonable choice on the server side (nothing to serve yet, and it must not block the one request-handling thread, which would stall every other asset on the page too). `load()` in `web/data.js` couldn't tell "nothing yet, try again very soon" apart from "nothing yet, same as any other quiet poll" -- it used the same 10 s cadence for both.
