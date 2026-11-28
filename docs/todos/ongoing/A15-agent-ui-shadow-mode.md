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
| `web/page-agent.js` | (new) the suggestion block and when to ask; `web/page-apps.js` places it in the drawer |
| `src/history/verdicts.rs` | (new) the `verdicts` table for shadow recording |
| `src/settings.rs`, `src/main.rs`, `src/llm_server.rs` | `agent_mode` (off, shadow, on) replaces `agent_enabled` |
| `src/server.rs` | `/api/classify` records the verdict and returns `agent_mode` |

## Steps

- [x] Record verdicts server-side in shadow mode.
- [x] Render the block when the setting is On.
- [x] Show "Unsure, investigate" and no quit suggestion when confidence is low or rules and model disagree.

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

## Implementation Record (2026-11-28)

**Decisions (the owner, 2026-11-28).** Shadow records a verdict when `POST /api/classify` runs, for any caller; there is no background worker. The setting is server-side and three-valued.

**Built.**

- `settings.agent_mode` (`off`, `shadow`, `on`) replaces `agent_enabled`. `monitor-lizard agent on|shadow|off` sets it; no web route can. A file with the old `agent_enabled: true` loads as `shadow`, which keeps its model running and shows nothing new; the old key is not written back. `llm_server` runs the model whenever the mode is not `off`.
- `verdicts` table (`src/history/verdicts.rs`): time, group key, mode, rule label, checked label, confidence, actions and rationale. Written only when the model answered. Rows older than 14 days are deleted on every write.
- `POST /api/classify` writes the row in shadow and on mode and returns `agent_mode`.
- `web/page-agent.js`: the "Suggestion (AI-generated, unverified)" block under the rule evidence in the Apps drawer, with the raw name, key and command in a code box. Shown only when the reply says `on`, the model server was ready and a rationale came back, so an unavailable model shows no block and no banner. A low-confidence verdict, or a label that differs from the rules', reads "Unsure, investigate" and offers only Investigate. Approve and Quit open the existing flows; Quit opens the existing Stop dialog, where Force quit is its own button. Rationale text is set as a text node.
- The page asks about an app at most once per ten minutes, and only when the mode is shadow or on.

**Deviations.**

- The attention-card line is not built. The drawer is the one place the page asks the server; a card line would need a verdict per card on the Overview, which the owner's trigger decision (on `/api/classify` only) does not cover.
- "Ignore" is shown as a label, not a button: nothing runs it until the mark-as-expected-or-suspicious todo.
- The todo asked for a jsdom smoke test. The repo has none; the logic is a pure block tested in Node (`tests/web/agent-suggestion.test.mjs`) like the other pages.

**Risk to know.** `/api/classify` handles one request at a time and waits up to `CLASSIFY_TIMEOUT` for the model. In shadow or on mode, opening an app's drawer starts that call, so a slow model can delay other tabs for that long. Off, the default, never calls it.

**Not verified.**

- The Rust was not compiled or run here (macOS-only crate); CI's `code` job is the first compile.
- No real browser and no real llama-server: the drawer layout, the block's look in both themes, and a live shadow row are pending. Marked for the owner.
- `agent on`, `shadow` and `off` against a running `serve` were not run.
