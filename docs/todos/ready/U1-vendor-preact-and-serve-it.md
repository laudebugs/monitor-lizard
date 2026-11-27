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

Put the pinned Preact file in `web/vendor/`, serve it through `src/web.rs` like every other dashboard file, check it in CI, and prove it loads in the Tauri window. No page uses Preact yet and no behaviour changes. Start only after the owner merges ADR 0005.

## Context

- [ADR 0005](../../decisions/0005-preact-vendored-no-build-step.md): what is vendored, the review steps, the platform floor.
- `src/web.rs`: `SOURCES`, `reply()`, and three tests that this todo must adjust: `the_shell_loads_exactly_the_listed_files_in_the_listed_order` (compares `<script src=` tags with every JavaScript source; `/vendor/preact.mjs` has no tag), the test that compares `web/` on disk with `SOURCES` (reads the top level only, so it never sees `web/vendor/`), and the token test.
- `tests/web/split.test.mjs` allows exactly one inline script and compares `web/*.js` with the tags; `import()` adds neither.
- `.github/dependabot.yml` lists `github-actions` and `cargo` only.

## Scope

**In scope:**

- `web/vendor/preact.mjs`: the `dist/preact.mjs` file of the `preact@11.2.0` tarball, unmodified (tarball integrity `sha512-/HfHI/7N84e6GrWBJ3J280z3mLaRJiDcSvj/A+y/4AbLnND/1UlGHe1sR6oYuTGwId0nP7dqDU9OnLxMPZ7xBg==`). `PREACT-LICENSE`: the tarball's `LICENSE`. No hooks file (hooks import the bare name `preact`, which needs an import map).
- `web/vendor/VENDORED.md`: name, version, source URL, tarball integrity, date vendored, reviewer, next review date, the upgrade steps from the ADR with their exact commands, and the result of the recorded grep.
- `web/vendor/SHA256SUMS` for every file in the folder.
- `Source` rows for `/vendor/preact.mjs` (`text/javascript`) and `/vendor/PREACT-LICENSE` (`text/plain`). Adjust the three Rust tests: the shell-tags test skips `/vendor/` paths, the disk test also walks `web/vendor/` and compares `vendor/<file>`, and the token test already covers every source.
- `scripts/check-vendor.mjs`, run by a new Makefile target `check-vendor` that `check` depends on (so `make check` and the CI job that runs it execute it): recomputes SHA-256 for each file and compares with `SHA256SUMS`; fails on a missing, extra or changed file and on `__TOKEN__`; runs the recorded grep and fails on a match; **prints a warning (exit 0) when the review date in `VENDORED.md` is more than 90 days old**.
- A Tauri window check: with the file served, run `import('/vendor/preact.mjs')` in the menu-bar shell's webview (developer tools or a temporary one-line script) and record the macOS version and result. The shell belongs to Amber: if you cannot run it, ask them and leave this step pending in the PR.
- Notices and rules: `docs/agents/ci-and-release.md` (dependabot cannot watch vendored files; the 90-day review and the steps), a `web/vendor/` row in `docs/agents/layout.md`, `THIRD-PARTY-NOTICES.md`, one sentence in `SECURITY.md`, the `AGENTS.md` web lines, and a link from `docs/guidelines/maintainability.md`.

**Out of scope** (name the todo that covers it):

- Any use of Preact by a page (U4). Any change to `h` (U3, U4). Declaring a macOS floor (the owner decides; the ADR asks).

## Target files

| File | Change |
|---|---|
| `web/vendor/preact.mjs`, `PREACT-LICENSE`, `VENDORED.md`, `SHA256SUMS` | (new) |
| `src/web.rs` | two `Source` rows; three test adjustments |
| `scripts/check-vendor.mjs`, `Makefile` | (new) the check, target `check-vendor`, added to `check` |
| `docs/agents/layout.md`, `docs/agents/ci-and-release.md`, `THIRD-PARTY-NOTICES.md`, `SECURITY.md`, `AGENTS.md`, `docs/guidelines/maintainability.md` | rows and notices listed in ADR 0005 |

## Steps

- [ ] Download the tarball into a new, empty scratch folder (not the repo). Run the ADR's upgrade steps: compare the integrity with `npm view preact@11.2.0 dist.integrity`, confirm `npm view preact@11.2.0 dist.attestations` lists provenance, run the recorded grep (0 matches expected). Copy only the files above.
- [ ] Write `SHA256SUMS` with `shasum -a 256` and `VENDORED.md`.
- [ ] Add the sources and fix the three Rust tests; `cargo test` must pass.
- [ ] Write `scripts/check-vendor.mjs` with tests; prove it fails by editing one byte locally (do not commit the edit) and that it warns, without failing, when the date is 91 days old (test with a temporary folder).
- [ ] Load the page in Chrome: no console error, `import('/vendor/preact.mjs')` returns `h` and `render`.
- [ ] Run the Tauri window check (or record it pending, and say who was asked).

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| A vendored file edited by one byte | `make check` fails and names the file |
| A file in `web/vendor/` not in `SHA256SUMS`, or the reverse | fails |
| Review date 91 days old | a warning on stderr, exit 0 |
| Review date missing or unparsable | fails (a missing date is a bug, an old one is a warning) |
| `/vendor/preact.mjs` with `Sec-Fetch-Site: cross-site` or a foreign `Host` | 403, as for any dashboard script |
| `/vendor/other.mjs` | 404 |
| Vendored text containing `__TOKEN__` or a grep match | fails |
| Response headers for the `.mjs` | `content-type: text/javascript`, `cache-control: no-store`, `x-content-type-options: nosniff` |
| `import()` in the Tauri window | resolves, or the result is recorded and the ADR's fallback (10.29.8 UMD) is raised with the owner |

## Acceptance criteria

- `make check` and `make test` green; the dashboard behaves as before in a browser.
- `check-vendor` runs inside `make check`.
- `VENDORED.md` names the next review date and holds the exact commands.
- The binary grows by about 12 kB.
- The Tauri window result is recorded or marked pending with the person asked.

## Tests

Rust: header and status cases for the new path. Node: `check-vendor` with a temporary folder (changed byte, missing line, extra file, `__TOKEN__`, grep match, date at 89 and 91 days, missing date). At least 8 cases.

## Verification

Run `make check` and `make test` (the standard pair), plus:

```sh
make check-vendor
node --test tests/web/*.test.mjs
```

## Docs to update

The files listed in Scope.

## Risks and open questions

- `SHA256SUMS` in the same PR as the file detects accidental drift only; a malicious PR would change both. The controls are the upgrade steps, which are manual. Say so in `VENDORED.md`.
- No hash crate: add none (agreement 10); the Node check is the authority and the Rust test checks presence and listing.

## Rollback and how the dashboard keeps working

- **Rollback:** Revert the PR. Nothing else depends on the file yet.
- **Why the dashboard works at every step:** No script loads Preact; one extra file is served and `import()` is not called.

## Definition of done

The standard list in [TEMPLATE.md](../TEMPLATE.md#definition-of-done), plus a browser check recorded for every UI change.
