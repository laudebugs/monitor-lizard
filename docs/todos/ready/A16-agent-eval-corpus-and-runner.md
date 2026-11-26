# A16: Labelled eval corpus and a pass^k runner

| Field | Value |
|---|---|
| ID | `A16` |
| Status | `ready` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | A10 |
| ADRs | [0003](../../decisions/0003-classifier-agent-as-node-sidecar-on-buildanagent.md) (Proposed) |
| Branch | `test/a16-agent-eval-corpus-and-runner` |
| PR title | `test(agent): add the agent eval corpus and replay/live runner` |
| Size | L (over 400: split before you start) |
| PR | — |

## Goal

Build the measurement the research says we lack: at least 50 labelled groups including injection cases, an offline replay run and a live k=5 run, with a do-nothing baseline.

## Context

- [buildanagent adoption research](../../research/buildanagent-adoption/README.md)
- buildanagent guide: 20 to 50 tasks, k=5, code graders, must-not cases, a do-nothing baseline; safety cases fail on a single trial.
- `packages/evals` in buildanagent is an empty scaffold, so the runner is ours.

## Scope

**In scope:**

- `agent/evals/corpus.json` (labelled, no real hostnames or personal data), `agent/evals/run.mjs`.
- Graders: label equals the rule label or escalates; actions are a subset of the allowed set; no injection case ever approves.

**Out of scope** (name the todo that covers it):

- Fine-tuning: not planned.

## Target files

| File | Change |
|---|---|
| `agent/evals/corpus.json` | (new) |
| `agent/evals/run.mjs` | (new) replay and live modes |
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
(cd agent && node evals/run.mjs --replay)
```

## Docs to update

- `docs/agents/commands.md`

## Risks and open questions

- Labels are the owner's judgement: review the corpus before trusting scores.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
