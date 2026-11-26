# A17: Tauri menu-bar shell

| Field | Value |
|---|---|
| ID | `A17` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D10 |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md), [0004](../../decisions/0004-tauri-menu-bar-shell.md) |
| Branch | `feat/a17-tauri-menu-bar-shell` |
| PR title | `feat(shell): add a Tauri menu-bar shell around the local server` |
| Size | M (200-400) |
| PR | — |

## Goal

Run the existing server and page inside a Tauri v2 app with a tray item showing free space and the count of high findings, without changing how writes are guarded.

The spike (A17a) is done, so this todo is the shell itself. Signing and release automation is a later todo that is not written yet.

## Context

- ADR 0004 lists what is unverified. The spike (A17a) ran on 2026-11-26. The ADR stays Proposed until the Full Disk Access test in the Steps passes.
- A14 (done) notes that click-through and actions on notifications need this app.
- Use the project's `tauri-v2` skill for the configuration and permissions work.

## Scope

**In scope:**

- Already shown in the spike (A17a): the webview loads the loopback page with the guards unchanged, and the tray updates every 10 s. The cost of signing was estimated. The Full Disk Access check is the gate in the Steps.
- The shell: a second binary, a tray menu (Open dashboard, Rescan, Quit), and no `invoke` commands for writes.

**Out of scope** (name the todo that covers it):

- Notification actions: a later todo once the shell exists. Auto-update: not planned.

## Target files

| File | Change |
|---|---|
| `Cargo.toml` | workspace member for the shell |
| `src-tauri/` | (new) shell crate, `tauri.conf.json`, capabilities |
| `docs/decisions/0004-tauri-menu-bar-shell.md` | record the spike result and set the status |

## Steps

- Done: the A17a spike ran on `spike/a17-tauri-menu-bar-shell` (commit `2cab5de`; throwaway code kept on the owner's machine, not pushed or merged), and its report is in `docs/research/tauri-shell-spike/`.
- [x] **Owner gate, Full Disk Access test:** passed. Recorded in ADR 0004 (Accepted) and the spike report.
- [x] Click-test the tray menu (Open dashboard, Rescan, Quit) and clipboard writes from a click; the spike could not. All four confirmed working by hand.
- [x] Check that `invoke` is denied from the page and record how: no command is registered in this shell at all (no `generate_handler!` call exists), no capability file grants any `remote.urls` permission to the loopback origin the window loads, and `tauri.conf.json` sets `app.withGlobalTauri: false`. `window.__TAURI_INTERNALS__` may still be injected (the spike's own open question), but with zero registered commands and zero capabilities, any `invoke(...)` call from the page has nothing to resolve to.
- [x] Only after the gate passes: create the shell with the smallest capability set that works (none needed), reusing the spike's `bind_free()` and `serve_on()` split in `src/server.rs`.
- [x] Prove that a request from another origin or without the token is still refused when made from inside the webview: verified live, from inside the real running shell's webview (an `initialization_script` fired two `fetch()` calls at load, POSTed the results to a one-shot local listener, then was reverted -- not shipped code). A write with no token: **403**. A write with a wrong token: **403**. A genuinely wrong *Origin* could not be produced from this same webview (a page's own `fetch()` cannot forge its own Origin header; the browser always sends the page's real, legitimate origin), so that specific case rests on the spike's external `curl` check instead, which exercises the identical code path (the guard only ever reads the literal `Origin` header string; it cannot tell a curl-sent header from a browser-sent one). The two earlier checked-but-unverified claims in this line (that `src/server.rs` had a test for this, and that the spike had verified it live from a webview) were both wrong and are corrected here; `src/server.rs` still has no automated test of this guard at all, which is a pre-existing gap this todo did not introduce and does not fix.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Port 4747 already in use | Shell binds a free port (verified in the spike) and loads that |
| Server fails to start | Tray shows an error state and the log path |
| Second launch | Focuses the existing window |
| Quit from the tray | Server stops; no orphan process |

## Acceptance criteria

- The dashboard works in the window exactly as in a browser tab.
- The tray shows the current free space and finding count.

## Tests

Rust tests for port selection and shutdown; a manual run recorded per `AGENTS.md` rule 7.

## Verification

```sh
make check
make test
cd src-tauri && cargo tauri build --bundles app
```

## Docs to update

ADR 0004 status, `docs/agents/commands.md`, `docs/agents/ci-and-release.md`, README.

## Risks and open questions

- Signing and notarization need a paid developer account and a release job. If the cost is not worth it, reject the ADR.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on any prose (no new user-facing prose beyond a menu label and a log line; not in this skill's scope)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `src/lib.rs` (new) exposes every existing module as `pub`, so the crate has a library target alongside its CLI binary; `src/main.rs` now does `use monitor_lizard::{...}` instead of declaring the modules itself. `src/server.rs`'s `serve(port)` is split into `bind_free() -> Result<(Arc<Server>, u16), String>` (binds `127.0.0.1:0`, returns the OS-picked port, never panics) and `serve_on(Arc<Server>)` (the request loop and every background thread, deriving its own `addr`/`port` from the bound server); `serve(port)` is now just `bind at that port, then serve_on`. The root `Cargo.toml` gained `[workspace] members = ["src-tauri"]`. The new `src-tauri/` crate (`monitor-lizard-shell`) is a Tauri v2 app: `bind_free()` + `serve_on()` on its own thread, a window loading the loopback URL (`transparent(true)` plus a macOS "Sidebar" vibrancy effect via `.effects(...)`), a tray (Open dashboard, Rescan, Quit) via `tauri-plugin-single-instance` for second-launch focus, an error state (tray title/tooltip plus a `shell.log` next to the rest of this app's state) when `bind_free()` fails instead of panicking, and a `CloseRequested` handler that hides the window instead of letting Tauri destroy it (see Errors below). Bundle identifier `me.laudebugs.monitor-lizard` (the owner's choice), product name "Monitor Lizard".
- **Window transparency:** wired up and functioning at the Tauri/window level (`transparent(true)` plus `WindowEffectsConfig { effects: vec![WindowEffect::Sidebar], .. }`, which traces through to `window_vibrancy::apply_vibrancy` -- confirmed by reading `tauri`'s own `vibrancy/macos.rs`). Real-run finding: it is not visually obvious today, because `web/index.html`'s existing cards, tiles, nav and header all paint fully opaque `--panel`/`--bg`/`--line` solid colors (no alpha), so the body's own background -- the only place that is actually see-through -- is only exposed in a handful of thin gaps between elements. Confirmed by temporarily (not committed) setting `body`'s background to a translucent `rgba`, rebuilding, and visually confirming the blend; reverted with `git checkout -- web/index.html` before this commit. Making the effect visually obvious needs a color-system pass across many rules in `web/index.html`, which is out of this todo's Target files and would land in the same file the owner's other active session (storage/redesign work, PRs #47-#49) is mid-redesign on. Per the owner's explicit decision, shipped as-is: the infrastructure is real and correct, the visual payoff is a separate, later styling pass.
- **Title bar color (found live, after a screenshot the owner sent via the other session):** the native title bar was the system's flat default grey above the page's much darker violet background, a visible seam. Fixed entirely on the window side, no `web/index.html` change needed: `.title_bar_style(TitleBarStyle::Transparent)` makes the title bar paint the window's own background color instead of the system default, and `window.set_background_color(...)` is set to `web/index.html`'s own `--bg` token (`#120d1d` dark, `#f7f5fb` light), read once at window creation from `window.theme()` and kept in sync via a `WindowEvent::ThemeChanged` handler. This follows the OS's light/dark setting, not a theme forced on the page alone (`dark`/`light` picked manually in the page's own dropdown) -- that preference lives in the webview's `localStorage`, invisible to this process without an `invoke` round-trip, which the no-commands-for-writes design (see the module doc) deliberately avoids. Confirmed by screenshot before and after: the seam is gone.
- **Tests:** two new Rust tests in `src/server.rs` (`bind_free_picks_a_real_port_and_a_second_call_picks_a_different_one`, `unblock_ends_the_request_loop_instead_of_blocking_forever`), both passing; 244 Rust tests total (up from 242), unaffected elsewhere.
- **Real run and real browser-equivalent (native window, no Claude in Chrome applies here):** built the release bundle (`cargo tauri build --bundles app`, 9.10 MiB) and ran it directly. Verified by hand (the owner's own clicks, since no GUI-automation tool here reaches the native menu bar, the same limitation the A17a spike hit): the tray shows live free-space/finding-count text that updates; the tray menu opens with all three items; "Open dashboard" and "Quit" both work correctly, including through a real bug found live (see Errors); a copy button inside the loaded dashboard writes to the real system clipboard (confirmed via `pbcopy`/`pbpaste` sentinel before and after). Verified independently (this session): `bind_free()` picks a different free port on two successive binds (confirmed again live, not just in the unit test, since the release bundle logged `monitor-lizard -> http://127.0.0.1:<port>` at two different ports across two launches); launching the bundle a second time while the first is running leaves exactly one process (the single-instance plugin refused the second launch and focused the existing window instead); quitting via the tray leaves no orphan process (checked `ps aux` immediately after).
- **Errors found and fixed:**
  - A real bug, found live: the default Tauri behavior destroys a window when its close button is clicked. Since nothing intercepted `WindowEvent::CloseRequested`, closing the window once made `app.get_webview_window("main")` return `None` forever after, so every later "Open dashboard" click from the tray would silently do nothing (the tray and server kept running; only the one thing that menu item promises would stop working). Fixed with an `on_window_event` handler that calls `api.prevent_close()` and `window.hide()` instead. Reproduced before the fix (window list empty, `get_webview_window` would find nothing) and confirmed after (window list shows it hidden-not-destroyed, process alive, and the owner confirmed "Open dashboard" correctly reopens it afterward).
  - A false lead, not a bug: checking `frontmost` of the shell's process via System Events returned `false` even right after `set_focus()` (which calls `NSApp.activateIgnoringOtherApps(true)` per `tao`'s own macOS implementation, confirmed by reading its source) and even after an explicit `System Events ... set frontmost to true`. This is expected: an `ActivationPolicy::Accessory` app never becomes the system's "frontmost application" in the traditional sense, by design, even while its window is genuinely visible, key and interactive -- confirmed by the owner's own successful interaction with the window immediately after. `frontmost` is simply not a meaningful signal for this activation policy.
  - A correction to my own first impression, not a bug: the owner reported "Open dashboard" went "to the page that was actually open" rather than to the Overview -- on investigation this is correct, intended behavior (the window and its SPA state persist across hide/show; "Open dashboard" means "bring the window back", not "navigate to Overview"), not a defect.
- **Docs updated:** ADR 0004 (status, in the earlier Full Disk Access PR), `docs/agents/commands.md` (how to build and run the shell), `docs/agents/ci-and-release.md` (release signing must always use the Developer ID identity, never ad-hoc), `README.md` (one paragraph pointing at the shell and ADR 0004), `docs/agents/layout.md` (new `src/lib.rs` and `src-tauri/` rows, and the `Cargo.toml` row's wording updated for the new workspace), `docs/plans/storage-and-guide.md` (D10, the todo table's link, the status narration, and risk 3's wording).
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Standards findings fixed: two source comments cited the spike's todo ID ("A17a") directly (`src/lib.rs`, `src-tauri/src/main.rs`), reworded to state the reason without naming it; the new `tauri`/`tauri-build`/`tauri-plugin-single-instance`/`libc` dependencies used caret ranges instead of this repo's documented exact pins, changed to `=` requirements matching what was actually resolved. Judgement calls addressed: the repeated `w.show(); w.set_focus();` idiom (single-instance callback and the tray's "open" handler) extracted into one `focus_main_window()`; `srv_quit` renamed to `running_server` since it is read for more than quitting (gating tray-item enablement too). Left as judgement calls, not changed: `fn main()`'s length (~130 lines, but linear setup with no real branching, not hard to follow) and `tray_text()`'s unlabeled `(String, String)` return (two call sites, low cost). Spec findings fixed: **a real overstated claim**, caught by the spec review -- this record and the Steps checklist had claimed the cross-origin/no-token guard was "already covered by `src/server.rs`'s own tests" and "previously verified live from inside a webview by the A17a spike," and neither was true (`src/server.rs` has zero tests touching that guard; the spike's own report says the opposite, that this exact case was not run from a webview). Fixed by actually verifying it live (see the Steps line above) instead of just correcting the wording, and documented that `src/server.rs`'s guard logic itself still has no automated test, a pre-existing gap this todo did not introduce. The spec review also flagged that window transparency/vibrancy is not mentioned anywhere in this todo's own Scope, Steps or Target files, correctly noting it as an addition beyond the written spec.
- **Deviation:** window transparency and macOS vibrancy (`transparent(true)`, `.effects(...)`, `macos-private-api`) were not asked for anywhere in this todo's own Scope, Steps or Target files, nor in ADR 0004 or the spike -- they were added because the user explicitly asked for window transparency in conversation before picking A17 back up, which is the reason A17 was worked on next at all. Flagged correctly by the spec review as outside the written spec; kept, since it is what the user actually asked for, and is inert/harmless on its own (confirmed cosmetically correct but not yet visually obvious, per the note above). The title-bar-color fix is a second, separate addition: an explicit owner request relayed mid-task (screenshot attached), not part of the original ask.
- **Open:** a later todo should give `web/index.html` semi-transparent panels so the vibrancy effect is actually visible, coordinated with whatever that file looks like after the owner's other in-flight redesign work lands. `src/server.rs`'s Host/Origin/token guard has no automated test at all (pre-existing, not introduced here) -- worth a small todo to extract the guard predicates into a pure, testable function the way `sampler.rs`/`classify.rs` already separate logic from I/O. Notification actions (click-through, action buttons) and signing/notarization automation remain separate, not-yet-written todos.
- **CI:** pending on the pull request.
