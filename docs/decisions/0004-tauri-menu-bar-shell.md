# 0004. A Tauri menu-bar shell runs the existing server and page

- **Status:** Proposed
- **Date:** 2026-11-26
- **Plan:** [storage-and-guide](../plans/storage-and-guide.md)
- **Amended 2026-11-26:** the A17a spike ran ([report](../research/tauri-shell-spike/README.md)). The shell is feasible, so the technical risks below are mostly resolved. Acceptance now depends on one owner-run test, the Full Disk Access test, and two checks are added to the decision. The status stays Proposed.

## Context

[ADR 0002](0002-rust-core-polling-collectors-local-web-ui.md) chose a Rust core and a loopback web page, and noted that "a Tauri menu-bar app can wrap the same server and page later". Nothing has been planned since. Three things now ask for an app: notifications cannot offer a click-through or action buttons without one (A14), the Storage scanner needs Full Disk Access for Mail and Messages, and a menu-bar item could show free space and the finding count without a browser tab open.

Spike result, 2026-11-26 (details and labels in the [report](../research/tauri-shell-spike/README.md)):

- **Verified:** a Tauri 2.12.7 shell builds on macOS 27; the server runs in-process; the webview loads the loopback page; the Host, Origin and token checks work unchanged (a request with no token or a wrong token got 403); the tray title and tooltip update every 10 s; the app hides from the Dock; the bundle is 8.8 MiB. No capability and no `invoke` command was needed.
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
- **Acceptance gate:** this ADR becomes Accepted only if the owner's Full Disk Access test passes, meaning a grant given to the bundle still applies after a rebuild with a Developer ID signature. If it does not, reject the ADR: the tray alone does not justify the memory and the signing cost.

## Consequences

- One UI stays. The page is not forked, which was the reason ADR 0002 rejected SwiftUI.
- The shell needs a signing and notarization setup and a release job. That is real work and a recurring cost.
- A bundle may give permissions such as Full Disk Access one identity, instead of whatever terminal ran the binary. This is the claim the gate tests; it is not yet shown.
- The shell uses about 133 MB while running, nearly 20 times `sys-monitor serve`, and the tray runs a second snapshot every 10 s.
- Todo A17 follows once this is accepted. Nothing else in the storage plan depends on it.

## Alternatives considered

- **Stay a command line tool plus browser tab.** No new cost, but no tray, no click-through, and Full Disk Access attaches to the terminal.
- **Native SwiftUI app.** Better menu-bar integration; rejected in ADR 0002 because it forks the UI.
- **Electron.** Works, but ships its own browser; Tauri uses the system web view.
