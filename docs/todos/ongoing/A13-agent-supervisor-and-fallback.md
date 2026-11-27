# A13: Supervise llama-server; fall back to rules

| Field | Value |
|---|---|
| ID | `A13` |
| Status | `ongoing` |
| Plan | [classifier-agent](../../plans/classifier-agent.md) |
| Depends on | A10, A12 |
| ADRs | [0003](../../decisions/0003-classifier-agent-rust-loop-buildanagent-evals.md) |
| Branch | `feat/a13-agent-supervisor-and-fallback` |
| PR title | `feat(classify): supervise the local model server and fall back to the rule label` |
| Size | M (200-400) |
| PR | — |

## Goal

Rust starts and supervises `llama-server` with a cleared environment, loopback only, timeouts and a kill switch, and answers every classification with the rule label alone whenever the model is unavailable.

## Context

- [buildanagent adoption research](../../research/buildanagent-adoption/README.md)
- The model runs only when the user enabled it; the app must behave exactly as before when it is off or missing.

## Scope

**In scope:**

- `src/llm_server.rs`: spawn with `env_clear()` and an allowlist, `--host 127.0.0.1`, health check, restart with backoff, memory-pressure check before loading, `monitor-lizard agent off`.
- `POST /api/classify?group_key=` (token-guarded) returning rule label, allowed actions and, when available, the validated verdict.

**Out of scope** (name the todo that covers it):

- UI: A15.

## Target files

| File | Change |
|---|---|
| `src/llm_server.rs` | (new) supervisor |
| `src/server.rs` | the route |
| `src/main.rs` | `agent` subcommand |
| `docs/agents/layout.md` | update |

## Steps

- [ ] Spawn with `env_clear()`; bind loopback only.
- [ ] Add timeouts and restart backoff; a kill switch.
- [ ] Return the rule-only answer on any failure.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| llama-server crashes mid-request | Rule-only answer within the timeout; restart with backoff |
| Model not installed | Rule-only answer; the route says why |
| Request without the token | 403 |
| Kill switch used | The server stops; later requests are rule-only |
| Memory pressure level 4 | Do not start the model; rule-only |

## Acceptance criteria

- With no model installed the dashboard behaves exactly as before.
- Killing llama-server never blocks the dashboard.

## Tests

Rust tests with a fake child process, 4 cases minimum.

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/commands.md`
- `docs/agents/layout.md`

## Risks and open questions

- Memory use on 8 GB Macs: measure and record.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
