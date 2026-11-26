# Adopting the buildanagent tooling for the classifier agent

As of November 2026. Gathered on 2026-11-26 by reading the public repo `laudebugs/buildanagent` (packages at `0.1.0-alpha.40`) from a shallow clone; nothing was installed or run. The agent read `docs/guides/agent.md` only in part and did not read most decision records, so see "Not verified". It did not go through the five-step pipeline in [../README.md](../README.md).

**Thesis:** build the classifier agent as a Node sidecar on `@buildanagent/gateway` and `@buildanagent/sdk` only, pointed at a local llama-server. Skip the harness and the CLI. Keep the label in Rust and the human confirm dialog as the only approver.

## What the packages offer (paths are in the buildanagent repo)

| Package | Relevant for us | Source |
|---|---|---|
| `gateway` | `createModel()`, `complete()`, adapters `anthropic` and `openaiChat`; `openaiChat` accepts a `baseURL` such as `http://127.0.0.1:8000/v1` and a dummy key | `packages/gateway/README.md` 242-290 |
| `sdk` | `run()` loop, `defineTool()`, `outputTool()` for a schema-checked final verdict, hooks, a scripted test model, `replayPairs()` for offline replay | `packages/sdk/README.md` 19-60, 405-435, 514, 548-618 |
| `agent-safety` | Typed write outcomes, signed approvals, plan guards: aimed at writes, not needed for read-only tools | `packages/agent-safety/README.md` 1-15 |
| `protocol` | Session-entry types, JSON-RPC messages, `JsonlFramer`, JSON Schemas for non-TypeScript clients | `packages/protocol/README.md` |
| `harness` | Session log, permission pipeline, sandboxes, MCP, built-in file/shell/fetch tools: heavier than we want | `packages/harness/README.md` |
| `cli` | `--model` accepts only `anthropic:` and `openai:` with the default OpenAI base URL, so it cannot reach llama-server | `packages/cli/src/model.ts` 12-41 |
| `evals` | Private empty scaffold; no pass^k runner exists | `packages/evals/README.md` 3 |

Licence: code is Apache-2.0 OR MIT; docs are CC BY 4.0 (`LICENSE`). Node 22.19 or later for the sdk. Packages install under the `alpha` tag; a plain install gives a deprecated placeholder (`packages/sdk/README.md` 14). Releases move weekly; pin exactly.

## Local model and network behaviour

- The `openaiChat` adapter posts to `{baseURL}/chat/completions` and needs an API key string (a dummy works). Set `compat` flags (`streamUsage: false`, `strictTools: false`) if llama-server rejects `stream_options` or `strict`; without usage, cost is unknown, so do not set `budgetUsd`.
- Structured output is a tool call (`outputTool`), not `response_format`. The adapter has no grammar support, but extra request fields can be passed through `providerOptions.openaiChat.extraBody`.
- No telemetry: the gateway only calls the configured `baseURL`; `otelObserver()` sends to the tracer you pass; the CLI sends no usage data. A grep of sdk, harness and agent-safety found `fetch` only in harness tools that run only if registered.

## Fit with our design

Matches "rules decide, model suggests, human approves": the buildanagent guide says to compute safety tiers from the worst case if an attacker wrote the input, and its decision record 0029 says no classifier approves and detectors only raise a tier. Prompt injection is "contained, not prevented": allowlisted tools, identity from trusted context, no mutating tools. There is no sanitiser for attacker-controlled strings such as process names, so we build that.

## Options compared

| Option | Fit | Problems |
|---|---|---|
| (a) Node sidecar on `sdk` + `gateway` | Direct local endpoint, typed loop, `outputTool`, offline test model | Node on user machines; alpha churn |
| (b) `cli` / `harness` as the agent | One shell-out | The CLI cannot target a local URL; the harness brings shell, file and fetch tools we do not want |
| (c) Rust calls llama-server directly; buildanagent for evals only | No Node at runtime; smallest attack surface; native grammars | We write the tool loop; tests only mirror production |
| (d) harness JSON-RPC and protocol events to the dashboard | Sessions, replay, approval wire | Too heavy for stateless classification; a session log of process names is a privacy surface |

**Decision (owner, 2026-11-26): the hybrid, option (c), with buildanagent used for evals only.** See [ADR 0003](../../decisions/0003-classifier-agent-rust-loop-buildanagent-evals.md).

## Proposed shape of (a)

Rust spawns `node agent.mjs` with a cleared environment and talks JSON lines over stdio. Rust sends only `{group_id, rule_label, rule_reasons}`. Four read-only tools (`get_group`, `get_connections`, `get_baseline`, `get_history`) call back to Rust, each scoped to that `group_id`, with attacker-controlled strings carried as JSON string values. The output is an `outputTool` verdict (`label`, `actions`, `rationale`); Rust re-validates it, clamps the label to the rule label and drops any action outside the rule-computed allowed set. The agent has no approval step: the dashboard's token-guarded Stop/Quit dialog stays the only approver.

## Evaluation (from the guide)

Label at least 50 groups across benign, unknown and suspicious, including injection-style process names and command lines as must-not cases; replay a recorded response offline with `replayPairs()` and run a live k=5 pass against the real model on a schedule; safety cases fail on a single trial; compare against a do-nothing agent and a single structured call without tools; record the GGUF hash, prompt version and package versions. Roll out offline gate, then shadow mode (record and show nothing), then canary.

## Not verified

`response_format` or grammar through `extraBody` working against llama-server; Qwen3-4B tool-calling and strict-schema behaviour and whether llama-server needs `--jinja`; llama-server's real context window against the 128k default; whether published npm artifacts match the repo source; Node single-file or SEA packaging; macOS sandbox-exec confinement of the sidecar; most decision records and several guides were not read; `packages/cli/README.md` says "no built-in tools" while `packages/harness/src/tools/builtins` exists, so the README is probably stale.
