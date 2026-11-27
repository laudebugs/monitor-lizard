# P2: Freeze, resume or stop the process that requested sudo

| Field | Value |
|---|---|
| ID | `P2` |
| Status | `ongoing` |
| Plan | [system-monitor](../../plans/system-monitor.md), section 2 (`src/privilege.rs`, `src/watcher.rs`) and D3 (writes are guarded) |
| Depends on | P1 |
| ADRs | None |
| Branch | `feat/p2-freeze-requester` |
| PR title | `feat(privilege): freeze or stop the process that requested sudo` |
| Size | L (over 400), kept as one PR because the route, the dialogs and the audit share one contract |
| PR | — |

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

- Signalling anything that is not a hop of a captured chain.
- Unfreezing on exit or any timer: a frozen process stays frozen until the person resumes it.
- Endpoint Security, or acting on processes before they are captured.

## Target files

| File | Change |
|---|---|
| `src/requester_signal.rs` | (new) verification, ordering, confirmation, audit, reply |
| `src/server.rs`, `src/lib.rs`, `src/watcher.rs` | Route wiring; a public start-time read for a hop |
| `web/page-signal.js` | (new) severity classifier, dialog state machine, dialog view |
| `web/page-privilege.js`, `web/index.html`, `web/app.css`, `src/web.rs` | Hop buttons, badge, script tag, styles |
| `tests/web/signal.test.mjs`, `tests/web/signal-view.test.mjs` | (new) |
| `docs/agents/layout.md`, `README.md`, `SECURITY.md`, `docs/plans/system-monitor.md` | Updated |

## Steps

- [ ] Tests first: Rust unit tests with fake and real `sleep` children; Node tests for the state machine and classifier.
- [ ] Server module, then the route.
- [ ] Page: buttons, dialogs, polling of the status.
- [ ] Look at every dialog state in Chrome in both themes on a scratch server.

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
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
