# B8: Top consumers table and a guarded Stop with fresh evidence

| Field | Value |
|---|---|
| ID | `B8` |
| Status | `ongoing` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | B3, B4 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/b8-top-consumers-and-guarded-stop` |
| PR title | `feat(web): add the top consumers table and re-check pid and start time on stop` |
| Size | M (200-400) |
| PR | — |

## Goal

Add the Top consumers table (sort by CPU, memory or disk I/O, restricted rows marked) and make Stop re-check pid and start time against a fresh sample, so a recycled pid is never signalled and the dialog shows current evidence.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md)
- Plan decision D6. Today `src/actions.rs` signals by pid only and cannot detect a recycled pid.
- The confirm dialog in the approved design shows "<name> is using N% CPU right now, from a fresh sample".

## Scope

**In scope:**

- `/api/kill` accepts `start=<unix-seconds>` per pid and refuses when the process's start time differs.
- The table from the design, with the Stop button hidden for root and other users' processes and a "restricted" tag.
- The dialog fetches one fresh row before opening.

**Out of scope** (name the todo that covers it):

- Any automatic action from metrics: never (D6).

## Target files

| File | Change |
|---|---|
| `src/actions.rs` | start-time check |
| `src/server.rs` | accept `start` |
| `web/index.html` | table, dialog evidence |
| `src/procusage.rs` | expose start time |

## Steps

- [ ] Write the start-time tests first (pid reused, matching start, missing start).
- [ ] Implement and wire the UI.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Pid recycled before confirm | Refused with "process changed"; nothing signalled |
| `start` omitted by an old client | Refused |
| Root or other user's process | No Stop button; the server also refuses |
| Process already gone | Row marked stopped; no error |

## Acceptance criteria

- A recycled-pid test passes; the dialog shows fresh evidence.
- UI layout and interaction checked in a real browser against the approved design at 1440 px and below 1100 px (record what was tested, or mark pending)

## Tests

Rust tests for the start-time check (4 minimum); jsdom smoke test for the dialog.

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`
- `SECURITY.md` if the write contract changes

## Risks and open questions

- This changes the `/api/kill` contract: update the README and security notes in the same PR.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
