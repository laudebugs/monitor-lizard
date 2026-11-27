# A12: Download and verify the local model

| Field | Value |
|---|---|
| ID | `A12` |
| Status | `ongoing` |
| Plan | [classifier-agent](../../plans/classifier-agent.md) |
| Depends on | A6 |
| ADRs | [0003](../../decisions/0003-classifier-agent-rust-loop-buildanagent-evals.md) |
| Branch | `feat/a12-model-install` |
| PR title | `feat(model): add a pinned, checksum-verified model install command` |
| Size | M (200-400) |
| PR | — |

## Goal

A command that downloads one pinned GGUF from Hugging Face, verifies its SHA-256, and refuses anything else, with the licence shown first.

## Context

- [local classifier research](../../research/local-classifier/README.md): Qwen3-4B Q4_K_M (Apache-2.0, 2.5 GB), fallback Qwen3-1.7B.
- Safety: pin the commit SHA, verify after download and at load, accept only GGUF, no pickle.

## Scope

**In scope:**

- `monitor-lizard model install [--model qwen3-4b|qwen3-1.7b]` and `monitor-lizard model status`, storing under `~/.monitor-lizard/models/`.
- A manifest in the binary: repo, commit SHA, file name, size, SHA-256, licence.
- Resume of interrupted downloads; a disk-space check (3 GB cap).

**Out of scope** (name the todo that covers it):

- Running the model: A13 and A14. Obtaining llama.cpp's `llama-server`: open question below.

## Target files

| File | Change |
|---|---|
| `src/model_store.rs` | (new) manifest, download, verify |
| `src/main.rs` | `model` subcommand |
| `Cargo.toml` | an HTTP client crate; check its licence first |

## Steps

- [ ] Fill the manifest from the Hub (record how the SHA-256 was obtained; the Hub field name is unverified).
- [ ] Implement download to a temp file, verify, atomic rename.
- [ ] Print model, size, licence and commit before downloading and require `--yes`.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Hash mismatch | Delete the temp file, exit non-zero |
| Interrupted download | Resumes or restarts; never leaves a partial file at the final path |
| Disk under 4 GB free | Refuse with the numbers |
| File exists with correct hash | No download |
| No network | Clear error; the app keeps working with rules only |

## Acceptance criteria

- After install the file verifies at load and `model status` shows the hash.
- No code path contacts the Hub except `model install`.

## Tests

Rust tests with a local fake server for the hash, resume and mismatch cases (4 minimum).

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/commands.md`
- Plan section 2

## Risks and open questions

- How users get `llama-server` (Homebrew `llama.cpp`, or a pinned release binary with its own hash) is undecided.
- New dependency: record its licence in the PR.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
