# A5: History page

| Field | Value |
|---|---|
| ID | `A5` |
| Status | `ready` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | A1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/a5-history-page` |
| PR title | `feat(web): add the history page` |
| Size | S (under 200) |
| PR | — |

## Goal

Replace the History placeholder with what changed over time: hosts first seen per app, and an audit table of every Stop/Quit the dashboard performed.

## Context

- `/api/history` returns `seen` and `actions` ([A1](../done/A1-backend-persistence-history-api.md)).
- Plan decision D3 requires the audit log to be visible to the user.

## Scope

**In scope:**

- Two tables: "New hosts" (when, app, host, hits) and "Actions you took" (when, signal, pids sent, refused).

**Out of scope** (name the todo that covers it):

- Retention and deletion of history: not covered.

## Target files

| File | Change |
|---|---|
| `web/index.html` | remove `history` from `SOON`; add `history()` view with `paginate(..., "hist")` and `paginate(..., "act")` |

## Steps

- [ ] Map `group_key` to the app label using the current snapshot; fall back to the key's basename.
- [ ] Render `refused` JSON as short reasons.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Empty history | Empty-state rows |
| `group_key` for an app no longer running | Basename shown |
| `sent` empty and `refused` non-empty | Row shows the refusal reason |

## Acceptance criteria

- An action performed through the dashboard appears here within one refresh.
- UI layout and interaction checked in a real browser at 1440 px and below 1100 px (record what was tested, or mark pending if no browser was available)

## Tests

jsdom smoke test as in A2.

## Verification

```sh
make check\nmake test
```

## Docs to update

- Plan section 2

## Risks and open questions

- None known.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
