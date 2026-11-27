# U6c: Migrate the Performance page

| Field | Value |
|---|---|
| ID | `U6c` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D2, D7 |
| Depends on | U6b |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `refactor/u6c-performance-page` |
| PR title | `refactor(web): run the Performance page on the Preact renderer` |
| Size | M (200-400) |
| PR | — |

## Goal

Performance is the live page: charts every 2 seconds, five tabs, 129 `h(` calls and three chart builders that write `svg` bodies. On the Preact renderer a chart update keeps the same DOM, so a focused control and the scroll position survive.

## Context

- The code from B7 (#50) is on `main`; the todo file stays in `ongoing/`. Spike: Top consumers `render()` 1.4 to 1.5 ms (Preact) against 2.4 to 3.7 ms (old); no large speed claim is made.
- `web/page-performance.js`; `loadMetrics` uses `quiet()` and the live toggle.

## Scope

**In scope:**

- Sparkline, multi-line and stacked charts through `svgBody`; keys on process rows; the live toggle keeps focus; `S.perfLive`, hidden-tab and view checks unchanged; chart width read from state or a `ResizeObserver`, not a live DOM read inside the builder; escaping tests for process names.

**Out of scope** (name the todo that covers it):

- Storage (U6d).

## Target files

| File | Change |
|---|---|
| `web/page-performance.js` | keys, chart sizing |
| `tests/web/*.test.mjs` | markup helpers |

## Steps

- [ ] Open each tab for 60 seconds with the key set: the charts update, nothing flickers, a focused control stays focused, the page does not jump.
- [ ] Record `render()` timings as in the ADR (for information, not a goal).

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Hidden tab, then visible | one catch-up, no burst |
| Live toggle off | polling stops; focus stays on the toggle |
| Metrics `unavailable` values | the existing "unavailable" text, never 0 |
| Window width change | chart width recomputed |
| 200 process rows | the page size caps the DOM |
| A hostile process name | escaped by `visible()` |

## Acceptance criteria

- Five tabs match the old renderer in DOM and behaviour.
- **Maintainability (the reason for this plan):** each page or component moved deletes the workarounds it needed (a `data-req` or `stFocus`-style focus restore, a `quiet()` guard, a dataset-cached state) and the PR description records before and after counts of those workarounds and of lines in the files touched. Logic that a page owns (labels, rows, text) is tested in Node against the vnode stand-in, with no browser needed; only behaviour that needs a real browser (focus, `inert`, scroll) is checked by hand.

## Tests

Markup, key and escaping cases; browser checks and timings recorded.

## Verification

Run `make check` and `make test` (the standard pair), plus:

```sh
node --test tests/web/*.test.mjs
```

## Docs to update

None unless a rule changes.

## Risks and open questions

- Chart widths come from the live DOM today; moving that to state changes first-paint behaviour and needs a look at narrow widths.

## Rollback and how the dashboard keeps working

- **Rollback:** Revert the PR.
- **Why the dashboard works at every step:** The default is the old renderer.

## Definition of done

The standard list in [TEMPLATE.md](../TEMPLATE.md#definition-of-done), plus a browser check recorded for every UI change.
