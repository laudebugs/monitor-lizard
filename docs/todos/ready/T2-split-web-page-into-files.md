# T2: Split the dashboard page into files

| Field | Value |
|---|---|
| ID | `T2` |
| Status | `ready` |
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
- Twelve marked blocks (`storage-cleanups`, `storage-page`, `url-state` and so on) exist; tests lift them by name, and others (`table-floor`, `memory-tab`) read `web/index.html` as text.
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

- [ ] Check what the server sends for the page today: headers, any Content-Security-Policy, caching. Record it.
- [ ] Save the old script block and stylesheet as a committed fixture from `main`, write a test that fails if the new files, concatenated in load order, differ from it byte for byte, then move the code until it passes. Delete the fixture in the last commit once reviewed, or keep it if the review wants it.
- [ ] Add the routes; confirm every file loads with 200 and the right content type, and an unknown path still returns 404.
- [ ] Update the Node tests to the new paths.
- [ ] Open every page in a real browser on a scratch HOME, in both themes, with the console open.

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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
