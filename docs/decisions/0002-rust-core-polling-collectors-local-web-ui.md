# 0002. Rust core with polling collectors and a loopback web dashboard

- **Status:** Accepted (owner, 2026-11-26)
- **Date:** 2026-11-26
- **Plan:** [system-monitor.md](../plans/system-monitor.md), D1 to D3

## Context

The project began as a TypeScript prototype and moved to Rust on the owner's request. Real-time process and network events on macOS need Apple's Endpoint Security, a restricted entitlement with reported waits of 5 to 13 months ([prior art](../research/prior-art/README.md)). Polling needs no entitlement. The owner wants a UI with actions such as stopping a process.

## Decision

- The core is a Rust crate (`ownwatch`, edition 2021) that polls: `ps`, `lsof`, `codesign`, `plutil`, the unified log, and `libproc` for image paths and parent pids.
- History is SQLite (`rusqlite`, bundled) in `~/.ownwatch/history.db`; the approved-apps baseline is `~/.ownwatch/baseline.json`.
- The UI is one static HTML file served by the Rust binary on `127.0.0.1` only. It makes no remote requests (system fonts only).
- Writes (approve, Stop/Quit) require a random per-run token, loopback Host and Origin, own-user processes only, and are audited.
- No telemetry, and no network calls except an explicit, user-started model download if a classifier is added.

## Consequences

- Short-lived processes and connections can be missed between polls; Endpoint Security would close that gap and remains optional.
- The code is macOS only; CI's code job runs on `macos-latest`.
- A Tauri menu-bar app can wrap the same server and page later without changing this decision.
- Renaming the crate and data folder to match the repository name is a separate todo.

**Amended 2026-11-26 (todo A11):** the crate is `sys-monitor` and state lives in `~/.sys-monitor/`, migrated once from `~/.ownwatch/`. The product and repository are both named Sys Monitor / `sys-monitor`.

**Amended 2026-11-26 (todo X1):** the crate is `monitor-lizard` and state lives in `~/.monitor-lizard/`, migrated once from whichever of `~/.sys-monitor/` or `~/.ownwatch/` is found (newest name tried first). The product and repository are both named Monitor Lizard / `monitor-lizard`.

## Alternatives considered

- Keep the TypeScript prototype: faster to write, but a long-running security daemon is better as a single static binary, and the owner chose Rust.
- Endpoint Security first: real-time, but blocked on an entitlement that may never be granted to an individual.
- A native SwiftUI app: better menu-bar integration, but it forks the UI from the web dashboard we already have.
