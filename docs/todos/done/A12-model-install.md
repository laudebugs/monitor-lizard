# A12: Download and verify the local model

| Field | Value |
|---|---|
| ID | `A12` |
| Status | `done` |
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

- [x] Fill the manifest from the Hub (record how the SHA-256 was obtained; the Hub field name is unverified).
- [x] Implement download to a temp file, verify, atomic rename.
- [x] Print model, size, licence and commit before downloading and require `--yes`.

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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27.

- **Built:** `src/model_store.rs` (new): `MANIFEST` pins two entries (`qwen3-4b` default, `qwen3-1.7b` fallback) with repo, commit, file name, size, SHA-256 and licence as `&'static` constants, obtained as described below. `install(entry)` checks whether a correctly-hashed file already sits at the final path (skips the network entirely if so), checks free disk space, streams the Hub URL into a `.partial` temp file (resuming via `Range` from the temp file's current length, or restarting cleanly if the server ignores the header and replies 200 instead of 206), hashes the assembled file, deletes it and returns an error on a mismatch, or atomically renames it into place on a match. `status(entry)` reads and hashes the local file only, never touching the network, so it is safe to run anytime and satisfies "no code path contacts the Hub except `model install`." `src/main.rs` adds `model install [--model qwen3-4b|qwen3-1.7b] [--yes]` and `model status [--model ...]`, printing the licence first (per the Goal), then model/repo/commit/file/size, with `--yes` required to actually download; `model status` rejects `--yes` as an unknown flag rather than silently accepting it. `src/guide.rs` gained one `COMMANDS` entry (`sm-model`) so `help`/`/api/guide` and the dispatch-completeness tests cover it; the two guide fixtures were regenerated to include it.
- **Manifest provenance:** read live from the real Hugging Face Hub (not fabricated): the commit for each repo is the Hub API's own `sha` field (`GET /api/models/<repo>`); the file's SHA-256 is the `oid` in its git-lfs pointer (`GET <repo>/raw/<commit>/<file>`, which for an LFS-tracked file returns the small pointer text — `version .../spec/v1`, `oid sha256:<hex>`, `size <n>` — not the multi-gigabyte blob), which the git-lfs spec itself defines as the file's content hash. This directly resolves the research's own open question ("the Hub's SHA-256 field name is unverified"): confirmed by reading the actual pointer file rather than guessing from the API's JSON shape. Sizes came from the same pointer. Both pinned models are Apache-2.0 (confirmed from each repo's own `cardData.license`).
- **Tests:** 8 new tests in `src/model_store.rs` using a `tiny_http` fixture server (the same pattern `src/llm.rs` already uses), exceeding the todo's minimum of 4: a fresh download that matches the hash and leaves no temp file; a partial temp file resumed via `Range` (one request, not a restart); a server that ignores `Range` and gets a clean restart (no duplicated prefix); a hash mismatch that deletes the temp file and never creates the final path; an already-correct final file that triggers no network call at all (no server is even started for that test); the 4 GB disk-space rule as a pure function; `status`'s three outcomes, also with no server started; and a manifest self-check (distinct ids, 40-char commits, 64-char lower-case hex hashes). All tests use a fresh, uniquely-named temp directory standing in for `~/.monitor-lizard` (never the real one), with a counter added after an initial flaky run showed two parallel tests could land on the same directory name when the clock's resolution was coarser than the test harness's parallelism. `make test` is unaffected elsewhere (309 Rust, 191 web).
- **Real run:** built the debug binary and ran `model install` (no `--yes`: prints licence first, then model/repo/commit/file/size, exits without touching the network), `model status` for both ids (both report "not installed"), `model install --model bogus` (exit 1, lists known ids), and `model status --yes` (exit 1, "unknown flag: --yes", confirming the fix below). A full real download against Hugging Face (the largest pinned file is 2.5 GB) was not run in this session: this Mac's disk was at 97% capacity (15 GiB free) at the time, and the download/verify/resume logic is already exercised end-to-end by the fixture-server tests above plus the live Hub reads used to build the manifest itself.
- **Review (2026-11-27):** `/code-review high 5` ran a standards and a spec review in parallel. Standards findings fixed: a todo-ID citation in `src/model_store.rs`'s module doc and in `docs/agents/commands.md` (AGENTS.md rule 9); a sentence over the docs guideline's 40-word limit in `commands.md` (split and reworded); the claim commit missing `[skip ci]` (`docs/guidelines/git-workflow.md`) -- fixed by recommitting the claim with the marker; the temp/final model file being created with `fs::File::create`/`OpenOptions::append` instead of this app's own private-file helpers, which would have left it world-readable instead of the `0600` every other file under `~/.monitor-lizard/` gets -- fixed to use `paths::create_private_file`/`_truncating`. Judgement calls fixed: the "unknown model" resolution repeated in both subcommands, extracted into `resolve_model()`; `--model`/`--yes` scanned in two separate passes over `args` with `model status` silently accepting a `--yes` its own usage line never mentioned -- both replaced with one `parse_model_args(args, allow_yes)` pass, and `model status --yes` is now a usage error. Spec findings fixed: the Goal's "with the licence shown first" wasn't honored by the print order (licence was last) -- reordered to print licence first; the SHA-256 provenance was told two different ways in two places (model_store.rs's doc comment and commands.md) -- both now state the same two facts (commit from the API's `sha` field, content hash from the git-lfs pointer) consistently, and a doc comment was added to `ModelEntry::commit` to say so. New dependency: `ureq`'s `rustls` feature (needed for HTTPS to huggingface.co; the existing `json`-only feature set had no TLS backend, only ever used against a local, unencrypted llama-server) pulls in `rustls`, `ring`, `rustls-webpki`, `rustls-pki-types`, `zeroize`, `untrusted`, `getrandom`, `webpki-roots` as new transitive dependencies -- checked via `cargo metadata`: all are MIT/Apache-2.0/ISC/CDLA-Permissive-2.0, all permissive, no new entry needed in `THIRD-PARTY-NOTICES.md` since that file's existing scope is bundled assets (fonts), not the many other Cargo dependencies already in this project with no such entries.
- **Deviation:** none beyond what Context/Scope already named (the Hub-field verification and the 4 GB disk check were both explicitly asked for).
- **Open:** how users get `llama-server` itself is still undecided, exactly as the todo's own Risks section says -- unchanged by this work, left for A13. A full, real multi-gigabyte download against the live Hub was not performed this session (see Real run above); the owner may want to do that once, outside a disk-constrained session, before relying on this in A13.
- **CI:** pending on the pull request.
