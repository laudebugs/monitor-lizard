# A2: Persistence page

| Field | Value |
|---|---|
| ID | `A2` |
| Status | `ongoing` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | A1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/a2-persistence-page` |
| PR title | `feat(web): add the persistence page and new-item attention card` |
| Size | M (200-400) |
| PR | — |

## Goal

Replace the Persistence placeholder with a page listing every startup item, flag new and unsigned ones, and surface new items on the Overview.

## Context

- [A1](../done/A1-backend-persistence-history-api.md) provides `/api/persistence`.
- The Apps page in `web/index.html` is the pattern: chips, search, sticky header, pagination, drawer-style detail.

## Scope

**In scope:**

- Nav item "Persistence" with a badge of new items.
- Table: label, kind, scope, program, trust, added. Chips: All, New, User, System.
- Attention card on Overview for new items (high if the program is unsigned, else medium).

**Out of scope** (name the todo that covers it):

- Disabling or removing a startup item: a later todo, needs its own safety review.
- Login items: not covered.

## Target files

| File | Change |
|---|---|
| `web/index.html` | remove the `persist` entry from `SOON`; add `persistence()` view, loader (every 60 s via `quiet`), `cards()` rule, nav badge |

## Steps

- [ ] Add `S.pers`, `loadPers` and the view function using `paginate(list, "pers")`.
- [ ] Render `label` and `program` with `textContent` only (h() already does).
- [ ] Add the card: "New startup item: <label>" with a Details button that opens the Persistence view.
- [ ] Make search (`S.q`) filter label and program on this page.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| No items | Empty-state row, no pager |
| Item with `signing: null` (program missing on disk) | Trust column shows "-"; not counted as unsigned |
| `signing.signed === false` | Trust shows UNSIGNED with the bad tag; card severity high |
| Very long program path | Ellipsis with `title` showing the full path |
| Label containing `<script>` or HTML | Shown as literal text |

## Acceptance criteria

- Persistence page lists items from `/api/persistence` with correct counts.
- A new item (simulate by deleting its row from the `persistence` table and restarting) appears as "New" with a badge and an Overview card.
- UI layout and interaction checked in a real browser at 1440 px and below 1100 px (record what was tested, or mark pending if no browser was available)

## Tests

A jsdom smoke test (kept out of the repo) that renders the page against a running server, opens the view, applies each chip and checks row counts, and asserts no script errors. The layout check needs a real browser.

## Verification

```sh
make check\nmake test\ncargo run --release -- serve
```

## Docs to update

- `docs/agents/layout.md` if files change
- Plan section 2

## Risks and open questions

- No real browser may be available in the agent environment; record layout checks as pending then.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
