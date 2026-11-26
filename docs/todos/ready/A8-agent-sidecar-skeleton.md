# A8: Node sidecar skeleton on the buildanagent SDK

| Field | Value |
|---|---|
| ID | `A8` |
| Status | `ready` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | A7 |
| ADRs | [0003](../../decisions/0003-classifier-agent-as-node-sidecar-on-buildanagent.md) (Proposed) |
| Branch | `feat/a8-agent-sidecar-skeleton` |
| PR title | `feat(agent): add the agent sidecar with a scripted-model smoke test` |
| Size | M (200-400) |
| PR | — |

## Goal

Create the `agent/` Node package: a stdio JSON-lines loop that builds a model with `createModel` over `openaiChat` pointed at a local URL, runs the SDK `run()` with a scripted model in tests, and never touches the network except that URL.

## Context

- [buildanagent adoption research](../../research/buildanagent-adoption/README.md)
- Pinned exactly: `@buildanagent/gateway` and `@buildanagent/sdk` at one alpha version, installed with the `alpha` tag; Node 22.19 or later.
- Do not start Node with `--conditions=source` (sdk README line 16).

## Scope

**In scope:**

- `agent/package.json` with exact pins, `agent/src/main.mjs`, `agent/src/model.mjs`.
- Message format: one JSON object per line on stdin (`{id, group_key, rule_label, rule_reasons, allowed_actions}`), one result per line on stdout.
- A smoke test with the SDK scripted model.

**Out of scope** (name the todo that covers it):

- The tools: A9.
- The verdict schema and validator: A10.
- Spawning from Rust: A13.

## Target files

| File | Change |
|---|---|
| `agent/package.json` | (new) exact versions, `"type": "module"`, `engines.node >= 22.19` |
| `agent/src/main.mjs` | (new) stdio loop |
| `agent/src/model.mjs` | (new) `createModel({adapter: openaiChat({baseURL, apiKey: "local", compat}), id})` |
| `agent/test/smoke.test.mjs` | (new) scripted-model run |
| `.github/workflows/ci.yml` | Node setup and `npm test` for `agent/` in the `code` job |
| `docs/agents/layout.md` | add `agent/` |

## Steps

- [ ] Install the two packages with the `alpha` tag in a scratch directory first and record the resolved versions.
- [ ] Write the stdio loop and the model factory; base URL comes from argv, default `http://127.0.0.1:8080/v1`.
- [ ] Write the smoke test using the SDK scripted model; no network.
- [ ] Add a test that fails if any module imports `node:http`, `node:https` or `fetch` outside `model.mjs`.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Malformed JSON line | One error line on stdout, loop continues |
| Model server unreachable | Result line with `error: "model_unavailable"`, no crash, no retry storm |
| stdin closes | Process exits 0 |
| Environment variables set | None are read except an allowlist (`PATH`) |

## Acceptance criteria

- `node agent/src/main.mjs` answers a scripted request.
- No outbound connection other than the configured base URL (assert in test).

## Tests

Node test runner; at least 4 cases from the table.

## Verification

```sh
make check
make test
(cd agent && npm test)
```

## Docs to update

- `docs/agents/layout.md`
- `docs/agents/commands.md`

## Risks and open questions

- Node dependencies are new to this repo: the dependabot config needs an `npm` ecosystem entry with the same cooldown.
- Alpha packages move weekly: pin exactly.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
