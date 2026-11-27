# P2: Freeze, resume or stop the process that requested sudo

| Field | Value |
|---|---|
| ID | `P2` |
| Status | `done` |
| Plan | [system-monitor](../../plans/system-monitor.md), section 2 (`src/privilege.rs`, `src/watcher.rs`) and D3 (writes are guarded) |
| Depends on | P1 |
| ADRs | None |
| Branch | `feat/p2-freeze-requester` |
| PR title | `feat(privilege): freeze or stop the process that requested sudo` |
| Size | L (over 400), kept as one PR because the route, the dialogs and the audit share one contract |
| PR | [#75](https://github.com/laudebugs/monitor-lizard/pull/75) |

## Goal

P1 shows every process between the person and `sudo`. The owner asked to act on what they see: pause a runaway script that keeps asking for sudo, resume it, or stop it. For each hop of a requester chain, offer Freeze (SIGSTOP), Resume (SIGCONT), Stop (SIGTERM) and Force stop (SIGKILL), on that one process and on the whole chain from that hop down to the process that ran `sudo`. The dialog says plainly what the action does to that process, and asks twice when the process is the owner's own session (a shell, `login`, the terminal app, `claude`).

## Context

- P1 (done): the requester detail on the Privilege page; `Hop` carries pid, executable, command, start time, terminal and working directory.
- Stop flow (`/api/kill`, `actions::check`): re-reads the start time before signalling.
- `src/trash_route.rs`: the model for preview, single-use confirmation token and audit before action.
- `SECURITY.md`: the signal guarantees (own user only, never launchd or itself) that this route must keep.

## Scope

**In scope:**

- `POST /api/requester/signal` (preview and commit) and `GET /api/requester/status` (what the page shows: frozen, exited, reused, children).
- A pure server module that verifies every pid against the captured hop immediately before signalling, with fakes in the tests.
- Audit rows in the existing `actions` table (signal names STOP, CONT, TERM, KILL), written before any signal.
- Dialogs: severity (danger, caution, plain), second confirmation for danger, whole-chain list, focus trap, announcer, Escape, one request per double click.
- A Frozen badge and the exited / reused / already-frozen availability states.

**Out of scope:**

- Signalling anything that is not a hop of a captured chain. A chain is a hop plus every hop closer to the process that ran `sudo`; side branches are listed, never signalled.
- Unfreezing on exit or any timer: a frozen process stays frozen until the person resumes it.
- Endpoint Security, or acting on processes before they are captured.

## Target files

| File | Change |
|---|---|
| `src/requester_signal.rs` | (new) verification, ordering, confirmation, audit, reply |
| `src/server.rs`, `src/lib.rs`, `src/watcher.rs` | Route wiring; a public start-time read for a hop |
| `tests/fixtures/danger-hops.json` | (new) the danger lists and cases, checked by the Rust and Node tests |
| `web/page-signal.js` | (new) severity classifier, dialog state machine, dialog view |
| `web/page-privilege.js`, `web/index.html`, `web/app.css`, `src/web.rs` | Hop buttons, badge, script tag, styles |
| `tests/web/signal.test.mjs`, `tests/web/signal-view.test.mjs` | (new) |
| `docs/agents/layout.md`, `README.md`, `SECURITY.md`, `docs/plans/system-monitor.md` | Updated |

## Steps

- [x] Tests first: Rust unit tests with fake and real `sleep` children; Node tests for the state machine and classifier.
- [x] Server module, then the route.
- [x] Page: buttons, dialogs, polling of the status.
- [x] Look at the dialog states in Chrome in both themes on a scratch server (scripted replies).

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Process already exited | Buttons disabled: "Already exited, nothing to freeze" (code `gone`) |
| Pid now belongs to another process (start time or executable differs) | Refused: "This pid now belongs to a different process" (`pid_reused`) |
| Already frozen | Frozen badge, Resume offered, a second freeze refused (`already_frozen`) |
| Root-owned or another user's process | Refused (`root_owned`, `not_yours`) |
| launchd, Monitor Lizard, its parents or its children | Refused (`launchd`, `monitor_lizard`) |
| Stop sent to a frozen process | SIGTERM then SIGCONT, so the TERM is delivered |
| Audit row cannot be written | Nothing is signalled (`audit_unavailable`) |
| Token reused, expired, or for another selection | Refused (`confirmation_*`) |
| More than 12 pids | Refused (`too_many_items`) |
| Chain | Freeze and resume run parent first, stop and force stop child first |
| Stopped by ^Z or a debugger, not by this app | Never continued: `not_frozen_by_us`; force stop still works |
| Items that are not parent and child | `not_a_chain` |
| Your own session (shell, login, terminal, Claude) | `needs_danger_confirmation` unless `confirm_danger=1` on preview and commit |
| Table of Monitor Lizard's own tree cannot be built | Every signal refused (`unverifiable`) |
| Status route without the token | 403, and no children listed for a refused process |

## Acceptance criteria

- Each hop of a requester detail offers the four actions on one process and on the chain, with the availability states above.
- The dialogs name the full command line, grade it danger, caution or plain, and ask twice for danger.
- Nothing is signalled without a preview token, an audit row and a fresh check of every pid.

## Tests

Rust: parse, verification refusals (fakes), ordering, token binding, audit-before-act, real freeze, resume and stop on `sleep` processes the test starts. Node: severity classifier and dialog state machine; dialog view against a stand-in DOM.

## Verification

```sh
make check
make test
make check-docs
```

## Docs to update

`docs/agents/layout.md`, `README.md`, `SECURITY.md`, `docs/plans/system-monitor.md`.

## Risks and open questions

- A frozen shell or terminal hangs the owner's session until it is resumed from another place; the danger dialog says so.
- A start-time check closes pid reuse but not a swap in the instant between the check and the signal.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`: two independent reviews of PR #75 (standards and spec) were run by the coordinator after the first commit; each finding and its fix is in the second commit's message. A review of that second commit has not been run.
- [x] `/writing-review` run on the dialog strings and docs, in both rounds
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27.

**Built**

- `src/requester_signal.rs`: `POST /api/requester/signal` (preview, single-use token, audit first) and `GET /api/requester/status`. `tests/fixtures/danger-hops.json` holds the danger lists that Rust and the page both check.
- Chain reading: "a hop plus every hop closer to the process that ran `sudo`". Other children of those processes are listed in the dialog and never signalled. At most 12 pids per request.
- Every pid is verified three times: at preview, before the audit rows, and right before its own signal (start time, executable, uid, state, parent from one `proc_pidinfo` call; not launchd, not Monitor Lizard or its parents and descendants).
- Stop on a frozen process sends TERM then CONT; each signal that will be sent has its own audit row (STOP, CONT, TERM, KILL), all written before the first signal.
- After review round 1: CONT only goes to a process the audit log shows this app froze (`not_frozen_by_us` otherwise; force stop still works); `scope=chain` checks parent and child links (`not_a_chain`; the check just before a signal accepts a child that is already gone, because stop goes child first); the table of Monitor Lizard's own tree fails closed (`unverifiable`); one signalling request at a time (`busy`); the binding is length-prefixed; the status route needs the token and lists children only for fully verified processes, with one `ps` per request.
- Danger is now enforced by the server: a hop that is a shell, `login`, `su`, `ssh`, a multiplexer, a terminal or editor app, or `claude` (also `claude` run by an interpreter) needs `confirm_danger=1` on preview and commit, bound into the token. The page sends it after the second confirmation, then sends the real request from that flagged preview. What only the page does: the warning text and the caution level for scripts and for processes with children; a hop whose children are not yet known is caution until the first live status.
- The page: buttons per hop and for the chain below it, a Frozen badge (and "Stopped by something else"), disabled states with the reason beside them, and the dialogs.

**Tests:** Rust `requester_signal` 30 tests (fakes for every refusal; real detached `sleep` and `sh` processes that the tests start themselves and kill in drop guards; a real parent-first freeze and child-first stop). Whole crate 477 passed, four ignored, five consecutive runs stable after one flake (a child-first stop failed its own chain check; fixed). Node: 36 tests in `signal.test.mjs` and `signal-view.test.mjs`; 350 web tests in all.

**Browser:** Chrome, scratch HOME, scripted `fetch` replies only (no real process signalled), light and dark, window not resized. Seen in round 2: the chain dialog, the second confirmation, the flagged preview then one real request despite a double click, the result list, and the "Stopped by something else" hop.

**Not verified:** forced colors; a screen reader; a narrow window; a full keyboard walk-through in a browser; a real sudo chain end to end; the dialog's error and refused-preview states by eye (Node tests only); the second confirmation against a real shell (tests use fakes and detached shells the tests start).

**Open:** the start time compares whole seconds (that is what the collector records); a process swapped in the instant between the last check and the signal remains a race. The page's `reqLive` is polled every five seconds, so a button can be stale until the next poll; the server re-checks anyway.

**Process note:** my first `make test` ran with the real `HOME` by mistake, before I set a scratch one. The tests use temporary directories, but it is unconfirmed whether any of them touched `~/.monitor-lizard`. Every later run used a scratch `HOME`.
