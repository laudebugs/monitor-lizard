# A Tauri v2 menu-bar shell works, but its payoff is small and its fixed cost is signing

As of 2026-11-26 (spike A17a, branch `spike/a17-tauri-menu-bar-shell`). Ran on macOS 27 (Darwin 27.0.0), Apple silicon. Code that ran is committed under `src-tauri/` on a local branch that is not pushed; it is a throwaway spike, not the shell. This report did not go through the full five-step pipeline in [../README.md](../README.md): there is no fact-check log. Each answer is labelled VERIFIED (I ran it, or read the primary page named) or UNVERIFIED.

**Thesis:** the technical risks in [ADR 0004](../../decisions/0004-tauri-menu-bar-shell.md) are mostly gone. The server runs in-process, the page and its guards work unchanged in the webview, and the tray works. What remains is cost (signing, a release job, about 19 times the resident memory (133 MB against 7 MB)) against a benefit that is still unproven (Full Disk Access identity).

## Versions and build

| Item | Value |
|---|---|
| rustc / cargo | 1.99.1 (2026-10-13) / 1.99.1 |
| tauri crate | 2.12.7 (`tray-icon` feature) |
| tauri-cli | 2.12.6, installed by me with `cargo install tauri-cli --locked` (4m32s, it was missing) |
| Release build of the shell, cold | 3m59s (ran while `cargo install` competed for CPU, so an upper bound); `cargo tauri build --bundles app`, incremental, 1m32s |
| Release build of `sys-monitor` alone | 45s (warm dependency cache) |

## Answers

| # | Question | Answer | Label |
|---|---|---|---|
| 1 | Does a minimal Tauri v2 app build here? | Yes, with the repo toolchain, no errors. The shell reuses the repo modules through `#[path]` includes. | VERIFIED (ran) |
| 2a | Server in-process, page loads in the webview? | Yes. The server runs on a thread; the window is `WebviewUrl::External("http://127.0.0.1:<port>/")`. The page rendered (screenshot) and `/api/snapshot`, `/api/metrics` and the font returned 200 from inside the webview (JS self-test). | VERIFIED (ran) |
| 2b | Host, Origin, token from inside the webview? | Yes, unchanged. From the webview: POST `/api/settings` with no token gave 403; with the page's token it passed the guard (400 from the validator, so no setting changed); with a wrong token 403. With curl against the same port: a wrong Host gave 403; an `Origin: http://evil.example` with a valid token gave 403; another server's token gave 403. The webview's own Origin is `http://127.0.0.1:<port>`, which the allow list contains. I did not drive a cross-origin request from inside the webview itself. | VERIFIED (ran), cross-origin from the webview not run |
| 2c | Port conflict? | Handled by binding port 0. 4747 was held by another process during the run and the shell took 61780. The token, `hosts` and `origins` are already derived from the port in `serve`; the only change needed was to split bind from the loop (`bind_free`, `serve_on(Arc<Server>)` in `src/server.rs`). | VERIFIED (ran) |
| 3a | Tray item, title and tooltip updated every 10 s? | Yes. Title `39 GB · 17` and tooltip `38.8 GiB free, 17 findings, 704 processes` are set from a Rust thread through `run_on_main_thread`. The log shows updates at 10 s spacing, and System Events read the live title and tooltip from the status item. The tray has no icon, only a title. | VERIFIED (ran) |
| 3b | Menu (Open dashboard, Rescan, Quit)? | Built with `Menu::with_items`. I could not open the menu and click items: System Events saw the status item but not its menu, and keyboard navigation did nothing. The handlers are untested. | UNVERIFIED |
| 3c | Hidden from the Dock? | Yes. `set_activation_policy(Accessory)` gives `type="UIElement"` in `lsappinfo`. The bundle also gets `LSUIElement = true` from `src-tauri/Info.plist` (checked in the built `Info.plist`). | VERIFIED (ran) |
| 3d | Quit stops the server, no orphan? | The Quit handler (`Server::unblock`, `app.exit(0)`) was not exercised (see 3b). SIGTERM to the shell ended the process and its three WebKit helpers; no listener remained on its port. Server and sampler are threads in the same process, so exit leaves no child. | Handler UNVERIFIED; process exit VERIFIED |
| 4 | Minimum capabilities, no `invoke` for writes? | No capability file was needed. The windows load a remote URL, and a remote origin gets IPC only if a capability lists it under `remote.urls`; I added none. The tray and menu are Rust-side, so they need no permission. The page has no `invoke` calls and writes use the HTTP routes. Caveat: `window.__TAURI_INTERNALS__` is still injected into the external page (typeof `object`); I did not test that `invoke` is denied from it. Add `"app.withGlobalTauri": false` and check in A17b. | Config VERIFIED (ran, no capability); denial of `invoke` UNVERIFIED. Rule for remote URLs from the Tauri v2 capabilities docs is from memory: UNVERIFIED |
| 5 | `navigator.clipboard.writeText`? | Context is secure (`isSecureContext` true). Called without a user gesture it rejects with `NotAllowedError`. A gesture-driven call was not tested (no click could be sent), and the Guide page is not on `main`, so it was not available to test. WKWebView normally allows it inside a click handler: UNVERIFIED. `document.queryCommandSupported('copy')` is true as a fallback. | No-gesture VERIFIED; gesture UNVERIFIED |
| 6a | Developer ID identity on this Mac? | Yes: `security find-identity -v -p codesigning` lists 3 valid identities, two "Developer ID Application" (same hash twice) and one "Apple Distribution". Name and team ID are left out here. | VERIFIED (ran) |
| 6b | What signing and notarization need and cost | Apple Developer Program, 99 USD per year; includes certificates and an Apple-verified Developer ID. Only the Account Holder creates Developer ID certificates. Tauri signs with `APPLE_SIGNING_IDENTITY` (or `bundle.macOS.signingIdentity`) and notarizes when `APPLE_API_ISSUER`/`APPLE_API_KEY`/`APPLE_API_KEY_PATH` or `APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID` are set. CI needs the certificate as a base64 `.p12` (`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`). | VERIFIED for cost and Tauri steps (pages read through a fetch tool, which summarises: treat as secondary); Apple's notarization page body did not load, so hardened runtime, timestamp, `notarytool` and stapling are UNVERIFIED (general knowledge) |
| 6c | Unsigned app? | Tauri's docs: on Apple silicon "code-signing is required for all apps from the Internet"; ad-hoc signing (`-`) does not stop macOS requiring the user to allow the app in Privacy & Security; a free account cannot notarize and the app "will still show up as not verified". The default `cargo tauri build` here produced an ad-hoc, linker-signed bundle. | VERIFIED (Tauri page); Gatekeeper wording is Tauri's, not Apple's |
| 7 | FDA to the bundle reads `~/Library/Mail`? | See the steps below. For a baseline, the spike binary run from this agent's session got `Operation not permitted` on `ls ~/Library/Mail`. | PENDING |
| 8 | Size, idle memory, CPU | See the table below. | VERIFIED (ran) |

## Measurements (question 8)

Idle for 60 s after 15 s warm-up, sampled with `ps`. The shell had its window open on the dashboard in the background; `sys-monitor serve` had no browser open, so the comparison favours serve. The shell also runs its own `snapshot::take` every 10 s for the tray, on top of the server's.

| | `sys-monitor serve` | Shell (main process) | WebKit helpers (3) |
|---|---|---|---|
| Resident memory | 7.1 MB | 81.2 MB | 51.8 MB |
| Shell total | | 133 MB | |
| CPU seconds in 60 s | 0.55 | 0.69 | 0.20 |
| Binary | 3.3 MB | 9.2 MB | system WebKit, not shipped |

The `.app` bundle is 8.8 MiB. Memory is about 19x the headless server (about 2.5x if you count the browser tab you would otherwise have open: not measured). CPU is negligible either way.

## What must change in the code

- `src/server.rs`: split `serve` into a bind step and `serve_on(Arc<Server>)`; add `bind_free()`. Done in this branch, with `serve(port)` behaving as before.
- The root crate is a binary, not a library. The spike includes modules by `#[path]`; A17b should instead make a library target or a workspace crate so there is one copy of the modules.
- Quit works only if the server loop can end: `Server::unblock` is enough, but the refresh threads, sampler and sudo watcher are detached and die with the process. Fine for Quit; not fine if the shell ever restarts the server without exiting.
- Todo A17 says "Server fails to start: tray shows an error state". The spike's `bind_free` returns `Result`, but the shell `expect`s it; A17b needs the error state.
- Second launch focusing the existing window needs `tauri-plugin-single-instance`: not tried, UNVERIFIED.

## Full Disk Access test (question 7), steps for the owner

Result: **PENDING**. Needs a human; macOS only grants it in System Settings.

```sh
cd src-tauri && cargo tauri build --bundles app
open "target/release/bundle/macos/Sys Monitor.app"      # dashboard opens; note the tray title
```

1. Open System Settings, Privacy & Security, Full Disk Access. Remove Terminal (or the app that runs your shell) from the list for this test, or run the test from a Terminal that has no FDA.
2. Baseline: in that Terminal run `ls ~/Library/Mail`. Expect `Operation not permitted`.
3. Click the `+` in Full Disk Access, add `Sys Monitor.app`, switch it on, then quit and reopen the app (FDA is read at launch).
4. The spike has no Mail reader, so add a throwaway `let r = std::fs::read_dir("/Users/<you>/Library/Mail")` to `src-tauri/src/main.rs` that writes `ok` or the error to `$TMPDIR/fda.txt` at startup, rebuild the bundle, relaunch (step 3), and read the file. Expect `ok` with the grant and `Operation not permitted` without.
5. Rebuild the bundle (new ad-hoc signature, different code hash) and relaunch without re-adding it. Record whether FDA still applies. This shows whether an unsigned or ad-hoc bundle keeps its grant across rebuilds, which is what the ADR claims a stable identity gives. Then repeat with `codesign --force --options runtime --sign "<your Developer ID Application identity>"` on the bundle (do this yourself; the spike did not sign).
6. Record all three outcomes (first grant, ad-hoc rebuild, Developer ID rebuild) in ADR 0004.

Expectation, not evidence: TCC keys a grant to the bundle identifier plus its code requirement, so a Developer ID signature should keep the grant across rebuilds and an ad-hoc one should not. UNVERIFIED.

## Surprises

- The page shows a stray `">` at the top left, in the shell window and in the owner's browser tab alike. It is in `web/index.html` on `main`, not caused by the shell.
- The process listening on 4747 when the run started was no longer there at the end of the run. The spike never signalled it (it only killed processes it started). Cause unknown; another session may have stopped it.

## Recommendation

**Amend ADR 0004 (keep it Proposed, do not accept yet). Build the shell only if the Full Disk Access test passes.** The shell is feasible and cheap to build. The in-process server, loopback page, guards, tray title and tooltip, Accessory policy and bundle (8.8 MiB) all worked with a small change to `server.rs`, and no `invoke` command or capability was needed. The costs are 99 USD a year, a signing and notarization release job, and about 130 MB resident against 7 MB. The owner already holds a Developer ID identity, which lowers the entry cost. The one benefit that cannot be bought another way is a stable identity for Full Disk Access, and it is still PENDING. The ADR should therefore record these results, mark that claim as untested, and make acceptance depend on the FDA test (step 5 above). Reject it if an ad-hoc or Developer ID rebuild does not keep the grant, because the tray alone does not justify the cost. Also amend two ADR statements: the webview needs no capability, and `withGlobalTauri` must be checked so the external page has no `invoke` path.

## Sources

- Tauri v2, macOS code signing: https://v2.tauri.app/distribute/sign/macos/ (read 2026-11-26 through a fetch tool, a summary).
- Apple Developer Program, what is included: https://developer.apple.com/programs/whats-included/ (same).
- Apple, notarizing macOS software before distribution: https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution (page body did not load; not used as evidence).
- Local runs: `cargo build --release`, `cargo tauri build --bundles app`, `lsappinfo`, `ps`, `security find-identity`, System Events accessibility reads, curl, a JS self-test in `src-tauri/src/selftest.js`.
