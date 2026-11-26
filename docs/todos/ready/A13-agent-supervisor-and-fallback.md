# A13: Supervise the sidecar and the model server; fall back to rules

| Field | Value |
|---|---|
| ID | `A13` |
| Status | `ready` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | A10, A12 |
| ADRs | [0003](../../decisions/0003-classifier-agent-as-node-sidecar-on-buildanagent.md) (Proposed) |
| Branch | `feat/a13-agent-supervisor-and-fallback` |
| PR title | `feat(agent): supervise the agent sidecar and fall back to the rule label` |
| Size | M (200-400) |
| PR | — |

## Goal

Rust starts and supervises `llama-server` and the Node sidecar, with a cleared environment, timeouts and a kill switch, and answers every classification with the rule label alone whenever either is unavailable.

## Context

- [buildanagent adoption research](../../research/buildanagent-adoption/README.md)
- The sidecar runs with no network except `127.0.0.1` and a cleared environment; the model runs only when the user enabled it.

## Scope

**In scope:**

- `src/agent.rs`: spawn, health check, restart with backoff, per-request timeout (default 20 s), `ownwatch agent off`.
- `POST /api/classify?group_key=` (token-guarded) returning rule label, allowed actions and, when available, the validated verdict.

**Out of scope** (name the todo that covers it):

- UI: A15. Packaging Node: A19.

## Target files

| File | Change |
|---|---|
| `src/agent.rs` | (new) supervisor |
| `src/server.rs` | the route |
| `src/main.rs` | `agent` subcommand |
| `docs/agents/layout.md` | update |

## Steps

- [ ] Spawn both children with `env_clear()` and an allowlist.
- [ ] Add timeouts and restart backoff; a kill switch that stops both.
- [ ] Return the rule-only answer on any failure.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Sidecar crashes mid-request | Rule-only answer within the timeout; restart with backoff |
| Model server not installed | Rule-only answer; `/api/classify` says why |
| Request without the token | 403 |
| Kill switch pressed | Both children stop; later requests are rule-only |

## Acceptance criteria

- With no model installed the dashboard behaves exactly as before.
- Killing the sidecar never blocks the dashboard.

## Tests

Rust tests with a fake child process (4 minimum).

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/commands.md`
- `docs/agents/layout.md`

## Risks and open questions

- Memory pressure on 8 GB Macs: measure and document in the Implementation Record.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
