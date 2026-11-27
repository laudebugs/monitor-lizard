# T2: Split the dashboard page into files

| Field | Value |
|---|---|
| ID | `T2` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D9 |
| Depends on | S4 |
| ADRs | None |
| Branch | `chore/t2-split-web-page` |
| PR title | `chore(web): split the dashboard page into CSS and script files` |
| Size | L (a move; see Risks) |
| PR | — |

## Goal

Split `web/index.html` (about 2,400 lines, one `<style>`, one `<script>`) into an HTML shell, one stylesheet and several scripts. No file then passes the 800-line budget, and R1, R2 and R3b each edit a small file.

## Context

- The page is embedded with `include_str!("../web/index.html")` in `src/server.rs`; the binary is one offline file and must stay one.
- Fourteen marked blocks (`storage-cleanups`, `storage-page`, `url-state` and so on) exist; tests lift them by name, and others (`table-floor`, `memory-tab`) read `web/index.html` as text.
- `src/server.rs` replaces `__TOKEN__` in the page with the per-run write token (`PAGE.replace`), and `const TOKEN = "__TOKEN__"` sits in the script block. That line moves with the script, so the replacement must run on the file that holds it, or the token must stay in the shell. The token must never reach the URL hash.
- The code shares globals (`S`, `h`, `rec`, `render`, `writeHash`); it is not written as modules.
- [docs/guidelines/maintainability.md](../../guidelines/maintainability.md) sets the 800-line budget.

## Scope

**In scope:**

- `web/app.css`: the stylesheet, unchanged.
- Classic scripts that keep sharing globals, loaded in a fixed order from `index.html`. Proposed split; adjust to the real seams:
  - `core.js`: state, `h`, `render`, router, `writeHash`, url-state
  - `table.js`: tables, export, widths
  - `shell.js`: page shell, sidebar, search
  - one file per page: `page-overview.js`, `page-apps.js`, `page-network.js`, `page-storage.js`, `page-guide.js`, `page-persistence.js`, and so on
- `src/server.rs` serves each file from an `include_str!`, from the same loopback origin, with the right content type, and injects the token as described in Context. No remote asset, no new dependency, no build step.
- `index.html` keeps the theme bootstrap script, the markup, and the script tags.
- Node tests read the new files; the marker blocks stay where they are in their new file.
- `docs/agents/layout.md` describes the new layout and where a new page goes.

**Out of scope** (name the todo that covers it):

- Any behaviour or style change: none, by design.
- ES modules or a bundler: not planned.
- Splitting `src/history.rs`: not planned here.

## Target files

| File | Change |
|---|---|
| `web/index.html` | shell only |
| `web/app.css`, `web/*.js` | new, moved from the page |
| `src/server.rs` | one route per file |
| `tests/web/*.test.mjs` | read the new paths |
| `docs/agents/layout.md` | new layout |

## Steps

- [x] Check what the server sends for the page today: headers, any Content-Security-Policy, caching. Record it.
- [x] Save the old script block and stylesheet as a committed fixture from `main`, write a test that fails if the new files, concatenated in load order, differ from it byte for byte (the token line excepted), then move the code until it passes. The fixture and that test stay in this PR and are deleted in the next one after merge.
- [x] Add the routes; confirm every file loads with 200 and the right content type, and an unknown path still returns 404.
- [x] Update the Node tests to the new paths.
- [x] Open every page in a real browser on a scratch HOME, in both themes, with the console open.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Script load order wrong | A test fails: `core.js` loads before anything that uses its globals |
| A script file is missing from the binary | The build or a test fails |
| Stale browser cache after an upgrade | Each file is served with the same cache headers as the page is today (recorded in step 1); a changed binary never serves an old script |
| Request for a path not in the list | 404, no file read from disk |
| Token line moved into a script | The served script carries the real token, the HTML does not leak it elsewhere, and the hash never contains it |
| Offline | The page works with no network |

## Acceptance criteria

- No behaviour change: every page looks and works as before, in both themes, with no console error.
- No file in `web/` is over 800 lines.
- Write actions still work (the token reaches the page).
- The page makes no request outside `127.0.0.1`.

## Tests

The byte-equality test for the move, a test for each route, a test that the served script carries the token, and the existing Node tests.

## Verification

```sh
make check
make test
```

## Docs to update

`docs/agents/layout.md`; plan table row.

## Risks and open questions

- Size: about 2,400 lines move and none change. The template asks for L todos to be split first, but a half-moved page does not run, so this ships as one PR. The review is the byte-equality test, not a line read.
- A global defined in one file and used before its file loads fails only in a browser. The load-order test and the page-by-page check are the guard.
- This lands after S4 and before S3a, S5 and the redesigns; every open branch that edits `web/index.html` must rebase on it. Tell the other session before merging.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md` (as a spec review and a standards review); findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record (2026-11-26)

**Built.** `web/index.html` (2,827 lines) is now a 44-line shell plus `web/app.css` (164) and 16 classic scripts (`core` 406, `table` 471, `data` 93, ten `page-*.js` of 26 to 370, `helpers` 79, `page-storage-logic` 372, `shell` 121). The split is contiguous line ranges taken at top-level seams, so the scripts joined in load order equal the old script block byte for byte except one line (below). A new `src/web.rs` holds the file table and the serving decision. `docs/agents/layout.md` lists what each file holds.

**What the server sent before** (step 1): the page, `/` only, with `content-type: text/html; charset=utf-8`; no `cache-control`, no Content-Security-Policy, no other header. Only the fonts carry `public, max-age=31536000, immutable`.

**The one deliberate code change.** The first version put `const TOKEN = "<secret>"` in `/js/core.js`. Review found that a script is a fixed URL: any website the owner visits could load it with `<script src="http://127.0.0.1:4747/js/core.js">` (it passes the Host check) and read `TOKEN` from its own scripts, because classic scripts share one global scope. The old page kept the token in the body of `/`, which another origin cannot read. Now the shell holds `<meta name="ml-token" content="__TOKEN__">`, the server replaces it in the shell only, and the first line of `core.js` reads it. The byte-equality test asserts that exactly that one line differs.

**Added guards.** For `/app.css` and `/js/*` the server answers 403 to `Sec-Fetch-Site: cross-site` or `same-site` (an absent header, `same-origin` and `none` pass). Every web file is sent with `cache-control: no-store` (this changes the page's old no-header behaviour on purpose: it carries a per-run secret, and a stale shell next to a cached script after a restart would make every write fail with 403) and `x-content-type-options: nosniff`. `src/web.rs::reply` is the single function `serve_on` and the socket tests both call, so the guard cannot be missing from the real route while a test passes.

**Deviations.** The Node tests read the page through `tests/web/page.mjs` instead of each naming a file. `storage-page` and `storage-cleanups` sit in `page-storage-logic.js`, not in `page-storage.js`, because the bytes between them (the Guide and the page-shell block) must stay in place for the equality proof. The fixture in `tests/fixtures/web-before/` and the byte-equality test exist only to prove this move: they are deleted in the next PR after merge, and nothing else depends on them.

**Tests.** `tests/web/split.test.mjs`: the equality test (token line exempt), the 800-line budget, no unlisted script, `core.js` first, a stand-in-browser run of every script in order (and a reversed order that must fail), the token only in the shell's meta tag. `src/web.rs`: token only in the shell, every file in `web/` is in `SOURCES`, the shell's tags equal `SOURCES` in order, 200 with type, `no-store` and `nosniff` for each file over a socket, 404 for unknown paths, 403 on a foreign Host, 403 for cross-site and same-site, no served file but `/` holds the secret. `src/server.rs`: a write is refused without the token whatever the Origin.

**Checked in Chrome** (scratch HOME, no resize, both themes, console open), twice: once before and once after the token and header changes. After them: all 10 pages in both themes with no page error (the only console exceptions come from the browser extension); Storage after a Rescan showed the clean-ups list, Inside System Data and Growth; Privilege with a requester detail open; all five Performance tabs including Top consumers; Guide. A settings write through the page returned 200 with the token and the same write without it returned 403; no script file contains the secret and the shell holds it once; every request went to `127.0.0.1`. With curl against the scratch server: `/app.css` and `/js/core.js` return 403 for `Sec-Fetch-Site: cross-site`, 200 with no header or `same-origin`, `/` returns 200 either way, a POST without the token is 403, a foreign Host is 403, and the files carry `cache-control: no-store` and `x-content-type-options: nosniff`. The one write checked is the settings write.

**Not verified.** Narrow widths, forced colors and reduced motion; the Tauri shell; the Stop and Quit dialogs and approve and trash writes (the browser check covers one write, the settings write, not every write action); that a Tauri webview sends `same-origin` or no `Sec-Fetch-Site` (Chrome was checked, the shell was not run); cross-file function hoisting is gone and a top-level throw in one script no longer stops the scripts after it, which only the stand-in-browser test and the page-by-page check guard; `/writing-review` on the docs prose; CI.
