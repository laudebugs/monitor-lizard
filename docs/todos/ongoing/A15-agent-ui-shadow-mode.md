# A15: Show the rationale and ranked actions, in shadow mode first

| Field | Value |
|---|---|
| ID | `A15` |
| Status | `ongoing` |
| Plan | [classifier-agent](../../plans/classifier-agent.md) |
| Depends on | A13 |
| ADRs | [0003](../../decisions/0003-classifier-agent-rust-loop-buildanagent-evals.md) |
| Branch | `feat/a15-agent-ui-shadow-mode` |
| PR title | `feat(web): show the agent verdict in the dashboard, shadow mode first` |
| Size | M (200-400) |
| PR | — |

## Goal

Show label, confidence band, rationale and suggested actions in the Apps drawer and attention cards. Shadow mode first: record verdicts and show nothing until the owner turns display on.

## Context

- [local classifier research](../../research/local-classifier/README.md) section "UI".
- Every suggested action opens the existing confirm dialog; focus defaults to Cancel; force quit needs a second step.

## Scope

**In scope:**

- A "Suggestion (AI-generated, unverified)" block under the rule evidence; the raw name and command in a code box.
- A setting: Off, Shadow, On.
- "Mark as expected / suspicious" is a later todo.

**Out of scope** (name the todo that covers it):

- The feedback baseline: later todo.

## Target files

| File | Change |
|---|---|
| `web/index.html` | drawer block, card line, setting stored in `localStorage` |
| `src/history.rs` | a `verdicts` table for shadow recording |

## Steps

- [ ] Record verdicts server-side in shadow mode.
- [ ] Render the block when the setting is On.
- [ ] Show "Unsure, investigate" and no quit suggestion when confidence is low or rules and model disagree.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Model server unavailable | Block not shown; no error banner |
| Verdict suggests quit for a root-only group | Not possible: filtered by A7/A10 |
| Rationale contains HTML | Shown as text |

## Acceptance criteria

- Off and Shadow show no difference in the UI; On shows the block.
- UI layout and interaction checked in a real browser, or marked pending

## Tests

jsdom smoke test with a stubbed `/api/classify`.

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`

## Risks and open questions

- Shadow data is a record of process names on disk; document retention.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
