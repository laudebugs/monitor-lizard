# 0004. A Tauri menu-bar shell runs the existing server and page

- **Status:** Accepted
- **Date:** 2026-11-26
- **Plan:** [storage-and-guide](../plans/storage-and-guide.md)
- **Amended 2026-11-26:** the A17a spike ran ([report](../research/tauri-shell-spike/README.md)). The shell is feasible, so the technical risks below are mostly resolved. Acceptance now depends on one owner-run test, the Full Disk Access test, and two checks are added to the decision. The status stays Proposed. Todo A17 carries the gate.
- **Amended 2026-11-26 (later the same day):** the owner ran the Full Disk Access test (checklist in the [spike report](../research/tauri-shell-spike/README.md)). Three outcomes: (1) a grant to the default, ad-hoc-signed build does not survive a rebuild (new code hash, same ad-hoc signing) -- `Operation not permitted` after relaunch without re-adding; (2) a brand-new Developer ID identity that was never granted FDA is of course denied until added, same as any new app; (3) once a Developer-ID-signed build is granted, a later rebuild signed with the **same** Developer ID identity keeps the grant without re-adding it -- `Ok(ReadDir(...))` after relaunch. This confirms the one claim the gate was testing: a stable Developer ID signing identity, not the ad-hoc default, is what gives the shell a Full Disk Access grant that survives normal rebuilds. **Gate passed. Status: Accepted.** Todo A17 is unblocked.

## Context

[ADR 0002](0002-rust-core-polling-collectors-local-web-ui.md) chose a Rust core and a loopback web page, and noted that "a Tauri menu-bar app can wrap the same server and page later". Nothing has been planned since. Three things now ask for an app. Notifications cannot offer a click-through or action buttons without one (A14). The Storage scanner needs Full Disk Access for Mail and Messages. A menu-bar item could show free space and the finding count without a browser tab open.

Spike result, 2026-11-26 (details and labels in the [report](../research/tauri-shell-spike/README.md)):

- **Verified, build and server:** a Tauri 2.12.7 shell builds on macOS 27, and the server runs in-process. The webview loads the loopback page, and the Host, Origin and token checks work unchanged (a request with no token or a wrong token got 403).
- **Verified, tray and bundle:** the tray title and tooltip update every 10 s, the app hides from the Dock, and the bundle is 8.8 MiB. No capability and no `invoke` command was needed.
- **Cost, measured:** about 133 MB resident against 7 MB for `sys-monitor serve`, a 99 USD a year developer program (the owner already holds a Developer ID identity) and a signing and notarization release job.
- **Not verified:** whether a bundle keeps Full Disk Access across rebuilds (needs the owner), the tray menu items (could not be clicked), that `invoke` is denied from the page (`window.__TAURI_INTERNALS__` is still injected), and clipboard writes from a click.

## Decision

Proposed: add a Tauri v2 shell as a second binary in the same workspace. It starts the existing server in-process on 127.0.0.1, opens a window onto it, and adds a tray item. Specifically:

- The webview loads the page from the loopback server. The Host and Origin checks and the per-run token stay as they are.
- No Tauri `invoke` commands for anything that writes. Writes keep one path, the guarded HTTP routes, so there is one place to audit.
- The tray shows free space and the number of high findings, and opens the dashboard. Notification click-through is a follow-up.
- The webview is given no capability. Before acceptance, check that the external page has no `invoke` path (`withGlobalTauri` and the injected `__TAURI_INTERNALS__`) and record the result.
- The shell binds a free port instead of 4747; `src/server.rs` is split into `bind_free()` and `serve_on()` for it.
- Command line use (`sys-monitor serve`) keeps working and does not need the shell.
- **Acceptance gate (passed):** this ADR required the owner's Full Disk Access test to pass, meaning a grant given to the bundle still applies after a rebuild with a Developer ID signature. It passed (see the amendment above); the build and release job must therefore always sign with the Developer ID identity, never ship an ad-hoc-signed bundle, since only the Developer ID signature keeps the grant across rebuilds.

## Consequences

- One UI stays. The page is not forked, which was the reason ADR 0002 rejected SwiftUI.
- The shell needs a signing and notarization setup and a release job. That is real work and a recurring cost.
- A bundle gives permissions such as Full Disk Access to one stable identity instead of whatever terminal ran the binary, confirmed by the Full Disk Access test above -- but only when every release is signed with the Developer ID identity; an ad-hoc or unsigned build loses the grant on its very next rebuild.
- The shell uses about 133 MB while running, nearly 20 times `sys-monitor serve`, and the tray runs a second snapshot every 10 s.
- Todo A17 is unblocked now that this ADR is Accepted.

## Alternatives considered

- **Stay a command line tool plus browser tab.** No new cost, but no tray, no click-through, and Full Disk Access attaches to the terminal.
- **Native SwiftUI app.** Better menu-bar integration; rejected in ADR 0002 because it forks the UI.
- **Electron.** Works, but ships its own browser; Tauri uses the system web view.
