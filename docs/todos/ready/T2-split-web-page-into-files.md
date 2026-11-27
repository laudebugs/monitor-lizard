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
| Size | L (about 2,400 lines move and none change; not split because a half-moved page does not run. The review is the byte-equality test, not a line read) |
| PR | — |

## Goal

Move `web/index.html` (about 2,400 lines: one `<style>` and one `<script>`) into a small HTML shell, one stylesheet and several script files, so no file passes the 800-line module budget and the redesign todos (R1, R2, R3b) each edit a small file instead of the same one.

## Context

- The page is embedded with `include_str!("../web/index.html")` in `src/server.rs`; the binary is one offline file and must stay one.
- Twelve marked blocks (`storage-cleanups`, `storage-page`, `url-state` and so on) are lifted by name into Node tests, and some tests read `web/index.html` as text (`table-floor`, `memory-tab`).
- The code shares globals (`S`, `h`, `rec`, `render`, `writeHash`); it is not written as modules.
- [docs/guidelines/maintainability.md](../../guidelines/maintainability.md) suggests modules of at most 800 lines; the page is about three times that.

## Scope

**In scope:**

- `web/app.css`: the stylesheet, unchanged.
- Classic scripts that keep sharing globals, loaded in a fixed order from `index.html`. A proposed split (adjust to the real seams): `core.js` (state, `h`, `render`, router, `writeHash`, url-state), `table.js` (tables, export, widths), `shell.js` (page shell, sidebar, search), and one file per page (`page-overview.js`, `page-apps.js`, `page-network.js`, `page-storage.js`, `page-guide.js`, `page-persistence.js`, and so on). No file over 800 lines.
- `src/server.rs` serves each file from an `include_str!`, from the same loopback origin, with the right content type. No remote asset, no new dependency, no build step.
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
- [ ] Write a test that fails if the concatenated scripts differ from the old script block, byte for byte, then move the code until it passes.
- [ ] Add the routes; confirm every file loads with 200 and the right content type, and an unknown path still returns 404.
- [ ] Update the Node tests to the new paths.
- [ ] Open every page in a real browser on a scratch HOME, in both themes, with the console open.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Script load order wrong | A test fails: `core.js` loads before anything that uses its globals |
| A script file is missing from the binary | The build fails or a test fails; the page never half loads |
| Stale browser cache after an upgrade | Files are not served with a long cache lifetime, or are versioned |
| Request for a path not in the list | 404, no file read from disk |
| Offline | The page works with no network |

## Acceptance criteria

- No behaviour change: every page looks and works as before, in both themes, with no console error.
- No file in `web/` is over 800 lines.
- The page makes no request outside `127.0.0.1`.

## Tests

The byte-equality test for the move, a test for each route, and the existing 214+ Node tests unchanged in what they assert.

## Verification

```sh
make check
make test
```

## Docs to update

`docs/agents/layout.md`; plan table row.

## Risks and open questions

- A global defined in one file and used before its file loads fails only in a browser. The load-order test and the page-by-page check are the guard.
- This lands after S4 and before S3a, S5 and the redesigns; every open branch that edits `web/index.html` must rebase on it. Tell the other session before merging.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
