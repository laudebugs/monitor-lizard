# A17: Tauri menu-bar shell

> **On hold until [ADR 0004](../../decisions/0004-tauri-menu-bar-shell.md) is Accepted.** The A17a spike is done ([report](../../research/tauri-shell-spike/README.md)). Acceptance needs the owner's Full Disk Access test below. Do not build the shell before it passes.

| Field | Value |
|---|---|
| ID | `A17` |
| Status | `ready` |
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
- [ ] **Owner gate, Full Disk Access test:** follow the steps in the report, section "Full Disk Access test". Grant Full Disk Access to the bundle, confirm it can read `~/Library/Mail`, rebuild, and confirm the grant still applies. Repeat with a Developer ID signature. Record the result in the ADR. If the grant does not survive a rebuild, reject ADR 0004 and close this todo.
- [ ] Click-test the tray menu (Open dashboard, Rescan, Quit) and clipboard writes from a click; the spike could not.
- [ ] Check that `invoke` is denied from the page and record how.
- [ ] Only after the gate passes: create the shell with the smallest capability set that works, reusing the spike's `bind_free()` and `serve_on()` split in `src/server.rs`.
- [ ] Prove that a request from another origin or without the token is still refused when made from inside the webview.

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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
