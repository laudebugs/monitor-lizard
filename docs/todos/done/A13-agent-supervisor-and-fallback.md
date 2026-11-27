# A13: Supervise llama-server; fall back to rules

| Field | Value |
|---|---|
| ID | `A13` |
| Status | `done` |
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

- [x] Spawn with `env_clear()`; bind loopback only.
- [x] Add timeouts and restart backoff; a kill switch.
- [x] Return the rule-only answer on any failure.

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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27.

- **Built:** `src/llm_server.rs` (new): `LlmServer`/`Inner` supervises one `llama-server` child behind `settings.agent_enabled` (the kill switch). A background thread (`start()`) ticks every 500 ms: `preflight()` (a pure function, no IO) decides, in priority order, whether to run at all -- disabled, memory pressure at level 4 (`sampler::pressure_level()`, a new cheap one-shot reading added alongside `one_shot()`), the model not installed and hash-verified (`model_store::status`), or `llama-server` not found (Homebrew's two usual prefixes, then every `PATH` entry). On a clean preflight it picks a free loopback port, spawns the binary with `env_clear()` plus an allowlist (`PATH`, a private scratch `HOME`, `TMPDIR`, `LANG` -- never the real `HOME`, see Review below), and health-checks `GET /health` before marking itself ready. A crash (detected via `try_wait()`) triggers a capped exponential backoff (1 s to 30 s) before retrying; the kill switch (`agent off`) kills and reaps the child immediately via the same tick loop, not a separate code path. `src/main.rs` adds `agent on`/`agent off` (`settings::set_agent_enabled`). `src/server.rs` adds `POST /api/classify?group_key=` (token-guarded like every other write route): one snapshot finds the group, `classify::allowed_actions`/`evidence` and `llm::classify_with_timeout` (now `pub`, see Review) feed `classify::validate_verdict`, and the reply carries `rule_label`, `allowed_actions`, the clamped `verdict`, and `agent_reason` (one of `disabled`/`model-not-installed`/`memory-pressure`/`binary-not-found`/`starting`/`crashed`, present only when the agent wasn't used for this answer).
- **Tests:** 8 tests in `src/llm_server.rs` (exceeding the todo's 4-case minimum): `preflight`'s priority ordering across all four reasons plus the "warn, not critical" pressure case, a still-running vs. an exited child (via real, trivial `/bin/sh` processes standing in for `llama-server` -- the todo's own "fake child process"), `kill_and_wait` ending a long-running child promptly, the cleared environment dropping an arbitrary variable while keeping `PATH` usable, the real `HOME` never reaching the child, and `/health`'s 2xx-only readiness check against a `tiny_http` fixture server (the same pattern `src/llm.rs`'s own tests use). `make test` is 318 Rust (was 309) and 214 web (unchanged by this todo; the jump from 191 seen earlier in the session is S2b's own already-merged tests, not this one's).
- **Real run:** built the release binary and ran a live `serve`. Verified: `agent off`/`agent on` toggle `~/.monitor-lizard/settings.json`'s `agent_enabled` and nothing else; a live `POST /api/classify` against a real group (disabled) returns `rule_label`, `allowed_actions`, a rule-only `verdict`, and `agent_reason: "disabled"`; turning the agent on and re-polling within ~2 s shows `agent_reason` move to `"model-not-installed"` (no model installed on this machine, confirming the preflight chain actually runs); a request with no token is 403; a request for an unknown group is 404. No real `llama-server` binary was available in this session, so the spawn/health-check/crash-restart path was not exercised against the real thing -- see Open.
- **Review (2026-11-27):** `/code-review high 5` ran a standards and a spec review in parallel. Standards findings fixed: a todo-ID citation in a `settings.rs` doc comment (AGENTS.md rule 9); the child's `HOME` was the real one, allowlisted on the reasoning that the model lives under it -- but the model is always passed to `llama-server` by absolute path, so the real `HOME` was never actually needed and only widened what a third-party binary could read (SSH keys, cloud credentials, dotfiles); fixed to a private, per-run scratch directory instead (`scratch_home()`), with a new test asserting the real `HOME` is never handed over. Spec findings fixed: the HTTP response was missing the plain rule label and the full allowed-actions list the Scope line asks for (`"returning rule label, allowed actions and, when available, the validated verdict"`) -- a caller previously had no way to tell "nothing else was allowed" from "the model's suggestions were filtered out"; added `rule_label`/`allowed_actions` alongside `verdict`. A second, more structural finding: this server handles one HTTP request at a time on a single thread, so `/api/classify` calling `llm::classify`'s default 20 s timeout would let one slow-but-alive model server stall every other open dashboard tab for up to 20 s -- `llm::classify_with_timeout` was made `pub` and `/api/classify` now calls it with a 6 s bound instead. This is a mitigation, not a fix for the server's single-threaded design (out of scope here); see Open.
- **Deviation:** none beyond what Context/Scope already asked for.
- **Open:** no real `llama-server` binary was available to spawn in this session; the full crash-and-restart and health-check path is covered by unit tests against stand-in processes and servers, not a real one. The HTTP server's one-request-at-a-time design (pre-existing, not introduced here) means a classify call can still stall other routes for up to the new 6 s bound; a real concurrency fix (one thread per request, or moving `/api/classify` off the main loop) is a larger change than this todo's scope. Memory use on 8 GB Macs (the todo's own Risks line) was not measured, for the same reason: no real model or binary to run.
- **CI:** pending on the pull request.
