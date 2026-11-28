# A15: Show the rationale and ranked actions, in shadow mode first

| Field | Value |
|---|---|
| ID | `A15` |
| Status | `done` |
| Plan | [classifier-agent](../../plans/classifier-agent.md) |
| Depends on | A13 |
| ADRs | [0003](../../decisions/0003-classifier-agent-rust-loop-buildanagent-evals.md) |
| Branch | `feat/a15-agent-ui-shadow-mode` |
| PR title | `feat(web): show the agent verdict in the dashboard, shadow mode first` |
| Size | M (200-400) |
| PR | [#97](https://github.com/laudebugs/monitor-lizard/pull/97) |

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
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on any prose
- [x] `make check` green; `cargo test` ran in CI on macOS
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record (2026-11-28)

**Decisions (the owner, 2026-11-28).** Shadow records a verdict when `POST /api/classify` runs, for any caller; there is no background worker. The setting is server-side and three-valued.

**Built.**

- `settings.agent_mode` (`off`, `shadow`, `on`) replaces `agent_enabled`. `monitor-lizard agent on|shadow|off` sets it; no web route can. The old `agent_enabled` key is ignored, so an upgrade starts at `off` and nothing is recorded until the owner runs `agent on` or `agent shadow`. Carrying `true` over as `shadow` would have recorded without an opt-in. `llm_server` runs the model whenever the mode is not `off`.
- `verdicts` table (`src/history/verdicts.rs`): time, group key, mode, rule label, checked label, confidence, actions and rationale. Written only when the model answered; a failed write is logged to the terminal. Rows older than 14 days are deleted on every write and every database open. `monitor-lizard agent verdicts [count]` prints the newest rows, with names escaped for the terminal.
- `POST /api/classify` writes the row in shadow and on mode and returns `agent_mode`.
- `web/page-agent.js`: the "Suggestion (AI-generated, unverified)" block under the rule evidence in the Apps drawer, with the raw name, key and command in a code box. It shows only when all three hold: the reply says `on`, the model server was ready, and a rationale came back. Otherwise there is no block and no banner. A low-confidence verdict, or a label that differs from the rules', reads "Unsure, investigate" and offers only Investigate. Approve and Quit open the existing flows; Quit opens the existing Stop dialog, where Force quit is its own button. Rationale text is set as a text node.
- The page asks about an app at most once per ten minutes (a failed or not-ready answer after one minute), one ask in flight at a time, and only when the mode is shadow or on. It re-reads the mode at most every 30 seconds, and a cached suggestion is hidden as soon as the mode is no longer `on`.

**Deviations.**

- The attention-card line is not built. The drawer is the one place the page asks the server; a card line would need a verdict per Overview card, and the owner's trigger decision (record on `/api/classify` only) gives the Overview no such call.
- "Ignore" is a disabled button: nothing runs it until the mark-as-expected-or-suspicious todo.
- The todo asked for a jsdom smoke test. The repo has none; the logic is a pure block tested in Node (`tests/web/agent-suggestion.test.mjs`) like the other pages.

**Risk to know, left open.** `/api/classify` handles one request at a time and waits up to `CLASSIFY_TIMEOUT` for the model. Opening an app's drawer in shadow or on mode starts that call. While the model is slow, other tabs, polls and Stop wait up to `CLASSIFY_TIMEOUT`. The page now allows one ask at a time, but the fix is to take the call off the server's accept loop, a server change that needs its own todo. Off, the default, never calls it.

**Verification.** The Rust was not compiled here (macOS-only crate); CI's `code` job compiled and tested it. Owed by the owner, on a Mac with the model installed:

- The drawer layout and the block's look in both themes in a real browser.
- A live shadow row from a real llama-server.
- `agent on`, `shadow`, `off` and `verdicts` against a running `serve`.
