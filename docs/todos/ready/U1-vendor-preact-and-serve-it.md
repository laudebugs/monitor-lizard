# U1: Vendor Preact and serve it

| Field | Value |
|---|---|
| ID | `U1` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D1, D10 |
| Depends on | None |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `feat/u1-vendor-preact` |
| PR title | `feat(web): vendor Preact 11.2.0 and serve it with a checksum check` |
| Size | M (200-400) |
| PR | — |

## Goal

Put the pinned Preact files in `web/vendor/`, serve them through `src/web.rs` like every other dashboard file, and fail the build when a vendored byte changes. No page uses Preact yet and no behaviour changes. Only meaningful once ADR 0005 is Accepted (merged).

## Context

- [ADR 0005](../../decisions/0005-preact-vendored-no-build-step.md): what is vendored and how it is verified.
- `src/web.rs`: `SOURCES`, `reply()`, and the tests that compare `web/` on disk with `SOURCES` and check the token appears only in the shell.
- `docs/agents/ci-and-release.md` and `.github/dependabot.yml`: dependabot lists `github-actions` and `cargo` only, so vendored files need a manual rule.
- `THIRD-PARTY-NOTICES.md`: the font entry is the model for a notice.
- Plan section 7: the spike's `web/vendor/` layout and import map worked in Chrome.

## Scope

**In scope:**

- Copy `dist/preact.mjs` from the `preact@11.2.0` npm tarball (npm integrity `sha512-/HfHI/7N84e6GrWBJ3J280z3mLaRJiDcSvj/A+y/4AbLnND/1UlGHe1sR6oYuTGwId0nP7dqDU9OnLxMPZ7xBg==`; the 11.2.0 tarball was published 2026-11-06) and `LICENSE` (as `PREACT-LICENSE`). Add `hooks/dist/hooks.mjs` as `preact-hooks.mjs` only if U5 will use a hook; if unsure leave it out and let U5 add it.
- `web/vendor/VENDORED.md`: name, exact version, source URL, tarball integrity, date vendored, who reviewed, next review date (90 days), how to upgrade.
- `web/vendor/SHA256SUMS` for every file in the folder.
- `Source` rows in `src/web.rs` (`/vendor/preact.mjs`, `/vendor/PREACT-LICENSE` as `text/plain`) and the import map in `web/index.html`: `{"imports":{"preact":"/vendor/preact.mjs"}}`.
- A Rust test that every `/vendor/` source is listed in `SHA256SUMS` and that no vendored text contains `__TOKEN__`. Computing the hash in Rust would need a hash crate; add none (agreement 10) unless `Cargo.lock` already has one, and say which in the PR.
- A Node check `scripts/check-vendor.mjs` run by `make check` (add to `check-docs` or a new `check-vendor` target in `check`): recomputes SHA-256 for each file and compares; fails on a missing, extra or changed file and on any `__TOKEN__`.
- The upgrade and review rule in `docs/agents/ci-and-release.md`, a row in `docs/agents/layout.md`, the notice in `THIRD-PARTY-NOTICES.md`, one sentence in `SECURITY.md`.

**Out of scope** (name the todo that covers it):

- Importing Preact from any script (U4). Any change to `h` (U3, U4).

## Target files

| File | Change |
|---|---|
| `web/vendor/preact.mjs`, `web/vendor/PREACT-LICENSE` | (new) unmodified from the tarball |
| `web/vendor/VENDORED.md`, `web/vendor/SHA256SUMS` | (new) provenance and checksums |
| `src/web.rs` | two `Source` rows; test for the checksum file |
| `web/index.html` | the import map (inline script, before the stylesheet link) |
| `scripts/check-vendor.mjs`, `Makefile` | (new) the check and the target in `check` |
| `docs/agents/layout.md`, `docs/agents/ci-and-release.md`, `THIRD-PARTY-NOTICES.md`, `SECURITY.md`, `AGENTS.md` | rows and notices listed in ADR 0005 |

## Steps

- [ ] Download the tarball into a new, empty scratch folder (not the repo); compare its integrity to `npm view preact@11.2.0 dist.integrity`; copy only the files above.
- [ ] Write `SHA256SUMS` with `shasum -a 256`; write `VENDORED.md`.
- [ ] Add the sources and import map; `cargo test` must still agree `web/` and `SOURCES`.
- [ ] Write the check script and wire it into `make check`; prove it fails by editing one byte of a vendored file locally (do not commit the edit).
- [ ] Load the page in a real browser; confirm no console error and `import('/vendor/preact.mjs')` works from the console.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| A vendored file edited by one byte | `make check` fails and names the file |
| A file in `web/vendor/` not listed in `SHA256SUMS` | fails (and `cargo test` fails: not in `SOURCES`) |
| `/vendor/preact.mjs` requested with `Sec-Fetch-Site: cross-site` | 403, as for any dashboard script |
| `/vendor/preact.mjs` requested with a foreign `Host` | 403 |
| `/vendor/other.mjs` | 404 |
| Vendored file containing `__TOKEN__` | the existing test and the check fail |
| Response headers for the `.mjs` file | `content-type: text/javascript`, `cache-control: no-store`, `x-content-type-options: nosniff` |

## Acceptance criteria

- `make check` and `make test` green; the dashboard behaves exactly as before in a browser.
- `import('/vendor/preact.mjs')` returns `h` and `render` from the page.
- The binary grows by about 12 kB.
- `VENDORED.md` names the next review date.

## Tests

Rust: the served-file tests already loop over `SOURCES`; add cases for the two new paths (headers, Host, Sec-Fetch). Node: the check script has its own test with a temporary folder (a changed byte, a missing line, an extra file). Minimum 5 cases.

## Verification

```sh
make check
make test
node scripts/check-vendor.mjs
```

## Docs to update

`AGENTS.md` (the two web lines), `docs/agents/layout.md`, `docs/agents/ci-and-release.md`, `THIRD-PARTY-NOTICES.md`, `SECURITY.md`, `docs/guidelines/maintainability.md` (one link), ADR 0005 status when the owner accepts.

## Risks and open questions

- A scripted Rust hash needs a hash crate; do not add a dependency for it (agreement 10). If none is present, the Node check is the authority and the Rust test checks presence only. Decide in the PR and say why.
- The import map is an inline script. If a CSP exists by then, add its hash.

## Rollback and how the page keeps working

- **Rollback:** Revert the PR. Nothing else depends on the files yet.
- **Why the page works at every step:** No script imports Preact; two extra files are served and the import map is unused, so every page draws as before.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
