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
- `web/vendor/VENDORED.md`: name, version, source URL, tarball integrity, date vendored, reviewer, next review date, the upgrade steps from the ADR with their exact commands, and a `Reviewed on: YYYY-MM-DD` line (the one date the check reads; use this term everywhere), and the result of the recorded grep.
- `web/vendor/SHA256SUMS` lists `preact.mjs` and `PREACT-LICENSE` only (not itself, not `VENDORED.md`).
- `Source` rows for `/vendor/preact.mjs` and `/vendor/PREACT-LICENSE` (`text/plain`). The `.mjs` row reuses the existing JavaScript constant, so its header is `text/javascript; charset=utf-8` (the todo's test asserts exactly that). Adjust the Rust tests: the shell-tags test skips `/vendor/` paths (they have no `<script>` tag); the disk test (today it lists only `.js`, `.css` and `.html` at the top level of `web/`) also walks `web/vendor/`, **includes `.mjs`**, and treats `VENDORED.md` and `SHA256SUMS` as files that are on disk and deliberately not in `SOURCES` and not served (a named exception, so a new unlisted file still fails); the token test already covers every source. Keep `preact.mjs` byte-identical to the tarball, including its last line `//# sourceMappingURL=preact.mjs.map`; the map is not served, so DevTools logs a 404 for it (harmless; say so in `VENDORED.md`).
- `scripts/check-vendor.mjs`, run by a new Makefile target `check-vendor` that **`check-docs` runs**, because the CI `docs` job runs `make check-docs` on every pull request, while the `code` job is skipped for a PR that only changes docs (a `VENDORED.md`-only PR must still be checked). It recomputes SHA-256 for each file in `SHA256SUMS` and compares; it fails on a missing, extra or changed file and on `__TOKEN__`; it runs the recorded grep and fails on any match beyond the recorded `createElement` and `innerHTML` ones; and when today is more than 90 days after `Reviewed on` it prints a GitHub annotation `::warning::web/vendor is due for review (last reviewed YYYY-MM-DD)` on stdout and exits 0. "Today" is the environment variable `CHECK_VENDOR_TODAY` (`YYYY-MM-DD`) when set, otherwise the real date, so tests fake it without touching the clock. **Limit, stated in the script header and in `VENDORED.md`:** offline, it only compares the files with `SHA256SUMS` from the same PR; it cannot tie `preact.mjs` to the npm tarball. The upgrade routine does that with `npm view preact@X dist.integrity`.
- A Tauri window check: with the file served, run `import('/vendor/preact.mjs')` in the menu-bar shell's webview (developer tools or a temporary one-line script) and record the macOS version and result. The shell belongs to Amber: if you cannot run it, ask them and leave this step pending in the PR.
- Notices and rules: `docs/agents/ci-and-release.md` (dependabot cannot watch vendored files; the 90-day review and the steps), a `web/vendor/` row in `docs/agents/layout.md`, `THIRD-PARTY-NOTICES.md`, one sentence in `SECURITY.md`, the `AGENTS.md` web lines, and a link from `docs/guidelines/maintainability.md`.

**Out of scope** (name the todo that covers it):

- Any use of Preact by a page (U4). Any change to `h` (U3, U4). Declaring a macOS floor (the owner decides; the ADR asks).

## Target files

| File | Change |
|---|---|
| `web/vendor/preact.mjs`, `PREACT-LICENSE`, `VENDORED.md`, `SHA256SUMS` | (new) |
| `src/web.rs` | two `Source` rows; three test adjustments |
| `scripts/check-vendor.mjs`, `Makefile` | (new) the check; target `check-vendor`, called by `check-docs` |
| `docs/agents/layout.md`, `docs/agents/ci-and-release.md`, `THIRD-PARTY-NOTICES.md`, `SECURITY.md`, `AGENTS.md`, `docs/guidelines/maintainability.md` | rows and notices listed in ADR 0005 |

## Steps

- [ ] Download the tarball into a new, empty scratch folder (not the repo). Run the ADR's upgrade steps: compare the integrity with `npm view preact@11.2.0 dist.integrity`, confirm `npm view preact@11.2.0 dist.attestations` lists provenance (a CI build, not proof of safe code), read the source diff against nothing (first vendoring: read `src/`), run the recorded grep (only `createElement` ×2 and `innerHTML` ×2 expected). Copy only the files above.
- [ ] Write `SHA256SUMS` with `shasum -a 256` and `VENDORED.md`.
- [ ] Add the sources and fix the three Rust tests; `cargo test` must pass.
- [ ] Write `scripts/check-vendor.mjs` with tests; prove it fails by editing one byte locally (do not commit the edit) and that it emits `::warning::` and exits 0 at 91 days (`CHECK_VENDOR_TODAY`), using a temporary folder.
- [ ] Load the page in Chrome: no console error, `import('/vendor/preact.mjs')` returns `h` and `render`.
- [ ] Run the Tauri window check (or record it pending, and say who was asked).

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| A vendored file edited by one byte | `make check` fails and names the file |
| A file in `web/vendor/` not in `SHA256SUMS`, or the reverse | fails |
| `Reviewed on` 89 days before `CHECK_VENDOR_TODAY` | no warning, exit 0 |
| `Reviewed on` 91 days before it | stdout has a `::warning::` line, exit 0 |
| `Reviewed on` missing or unparsable | fails (a missing date is a bug, an old one is a warning) |
| A PR that changes only `VENDORED.md` | the CI `docs` job still runs the check (via `make check-docs`) |
| `/vendor/preact.mjs` with `Sec-Fetch-Site: cross-site` or a foreign `Host` | 403, as for any dashboard script |
| `/vendor/other.mjs` | 404 |
| Vendored text containing `__TOKEN__` or a grep match | fails |
| Response headers for the `.mjs` | `content-type: text/javascript; charset=utf-8`, `cache-control: no-store`, `x-content-type-options: nosniff` |
| DevTools open | a 404 for `preact.mjs.map` is expected and harmless |
| `import()` in the Tauri window | resolves, or the result is recorded and the ADR's fallback (10.29.8 UMD) is raised with the owner |

## Acceptance criteria

- `make check` and `make test` green; the dashboard behaves as before in a browser.
- `check-vendor` runs inside `make check-docs`, so the CI `docs` job executes it on every pull request.
- `VENDORED.md` names the next review date and holds the exact commands.
- The binary grows by about 12 kB.
- The Tauri window result is recorded or marked pending with the person asked.

## Tests

Rust: header and status cases for the new path. Node: `check-vendor` with a temporary folder (changed byte, missing line, extra file, `__TOKEN__`, grep match, date at 89 and 91 days, missing date). At least 8 cases.

## Verification

Run `make check` and `make test` (the standard pair), plus:

```sh
make check-vendor
CHECK_VENDOR_TODAY=2027-03-01 make check-vendor   # expect a ::warning:: line and exit 0
node --test tests/web/*.test.mjs
```

## Docs to update

The files listed in Scope.

## Risks and open questions

- `SHA256SUMS` in the same PR as the file detects accidental drift only; a malicious PR would change both. The controls are the upgrade steps, which are manual, and a person reading the source diff is the real one. Say so in `VENDORED.md`.
- No hash crate: add none (agreement 10); the Node check is the authority and the Rust test checks presence and listing.

## Rollback and how the dashboard keeps working

- **Rollback:** Revert the PR. Nothing else depends on the file yet.
- **Why the dashboard works at every step:** No script loads Preact; one extra file is served and `import()` is not called.

## Definition of done

The standard list in [TEMPLATE.md](../TEMPLATE.md#definition-of-done), plus a browser check recorded for every UI change.
