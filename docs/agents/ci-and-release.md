# CI and release

## Principles

- CI calls the same `make` targets developers run.
- **One aggregate gate job** (`gate`) is the only required status check in branch protection. Individual jobs can then change without editing the protection rule.
- Permissions default to `contents: read`; a job asks for more only when it needs it.
- Concurrency cancels superseded runs on a PR. Deploys are serialized and never cancelled.
- Every job has `timeout-minutes`.
- No retries in PR lanes: report flaky tests, do not hide them.
- Hangs are caught by the test runner's timeout, not by wall-clock assertions.

## Supply chain (turn these on when the project has dependencies)

- Pin actions: `gh actions-lock` records each tag's commit in `.github/workflows/actions.lock`; a CI job verifies it.
- Dependabot weekly with a cooldown, and the same minimum release age in your package manager.
- Every audit or licence ignore carries a reason and a review date.
- Workflows that handle untrusted PR code use `pull_request_target` only to read, never to run PR code; pass PR fields through `env:`, never `${{ }}` inside `run:`.
- Build with no secrets; deploy from `main` with the secrets.

## Releases

Nothing is released yet. The binary is built from source (`cargo build --release`). When releases start: attach notarized binaries to GitHub Releases from a workflow using OIDC, not long-lived tokens, and serialize releases as described in [git-workflow.md](../guidelines/git-workflow.md).

## Status of this repo

- `gate` is the aggregate job. **Branch protection is not enabled yet** (an owner setting): require pull requests and the `gate` check on `main`.
- Third-party actions (`dtolnay/rust-toolchain`, `Swatinem/rust-cache`) are not pinned to commits yet; do that with `actions.lock` as above.
