# A16: Labelled eval corpus and a pass^k runner

| Field | Value |
|---|---|
| ID | `A16` |
| Status | `ready` |
| Plan | [classifier-agent](../../plans/classifier-agent.md) |
| Depends on | A10 |
| ADRs | [0003](../../decisions/0003-classifier-agent-rust-loop-buildanagent-evals.md) |
| Branch | `test/a16-agent-eval-corpus-and-runner` |
| PR title | `test(classify): add the classifier eval corpus and runner` |
| Size | L (over 400: split before you start) |
| PR | — |

## Goal

Build the measurement the research says we lack: at least 50 labelled groups including injection cases, an offline replay run and a live k=5 run, with a do-nothing baseline. buildanagent tooling is used here, at development time only.

## Context

- [buildanagent adoption research](../../research/buildanagent-adoption/README.md)
- buildanagent guide: 20 to 50 tasks, k=5, code graders, must-not cases, a do-nothing baseline; safety cases fail on a single trial.
- `packages/evals` in buildanagent is an empty scaffold, so the runner is ours.
- [ADR 0003](../../decisions/0003-classifier-agent-rust-loop-buildanagent-evals.md): buildanagent is a dev-time dependency, never shipped.

## Scope

**In scope:**

- `evals/corpus.json` (labelled, no real hostnames or personal data) and a runner that drives `monitor-lizard classify --json` over it.
- Graders: the label equals the rule label or escalates; actions are a subset of the allowed set; no injection case ever approves.
- Spike first: can `@buildanagent/sdk` `replayPairs()` or the gateway test transport replay raw chat-completion pairs? If yes use it for the offline lane; if not, replay with a Rust fixture server.

**Out of scope** (name the todo that covers it):

- Fine-tuning: not planned.
- Any runtime use of buildanagent.

## Target files

| File | Change |
|---|---|
| `evals/corpus.json` | (new) |
| `evals/run.mjs or a Rust test` | (new) replay and live modes |
| `docs/research/buildanagent-adoption/README.md` | link the results |

## Steps

- [ ] Split this todo (A16a corpus, A16b runner) before starting if it exceeds 400 lines.
- [ ] Record the GGUF hash, prompt version and package versions with every result.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Any injection case approves or downgrades | Whole run fails |
| Live server absent | Live mode skips with a message; replay still runs |

## Acceptance criteria

- Replay passes in CI; a live k=5 result is recorded in the Implementation Record.

## Tests

The corpus is the test.

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/commands.md`

## Risks and open questions

- Labels are the owner's judgement: review the corpus before trusting scores.
- A JavaScript runner must run on Node: the owner found buildanagent does not work under Bun yet (see the 2026-11-26 note in `AGENTS.md`). Adding Node to the CI `code` job is optional; prefer a Rust runner if the spike shows no benefit.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
