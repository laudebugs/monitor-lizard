# B6: Memory detail page

| Field | Value |
|---|---|
| ID | `B6` |
| Status | `ready` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | B4 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/b6-memory-detail-page` |
| PR title | `feat(web): add the memory detail tab` |
| Size | M (200-400) |
| PR | — |

## Goal

The Memory tab: stacked area of App, Wired, Compressed and Cached, a pressure card colored on pressure level with the percentage available, and a swap bar.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md)
- One report says the pressure level stayed "normal" during swap thrash, so always show the available percentage next to the level [U].
- The "used = app + wired + compressed" mapping is inference: compare with Activity Monitor and record the result.

## Scope

**In scope:**

- Stacked chart, pressure card (level tag and percent), swap card, explanatory copy as in the design.

**Out of scope** (name the todo that covers it):

- Per-process memory: B8.

## Target files

| File | Change |
|---|---|
| `web/index.html` | Memory tab |

## Steps

- [ ] Build from the design.
- [ ] Record the side-by-side with Activity Monitor.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Pressure level 4 | Bad tag |
| Swap total 0 | Swap card says "no swap" instead of dividing by zero |
| Pressure unreadable | Unavailable with the reason |

## Acceptance criteria

- Matches the approved design.
- UI layout and interaction checked in a real browser against the approved design at 1440 px and below 1100 px (record what was tested, or mark pending)

## Tests

jsdom smoke test as in B4.

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`

## Risks and open questions

- None known.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
