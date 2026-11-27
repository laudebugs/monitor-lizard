# 0005. Build the dashboard UI with Preact, vendored, with no build step

- **Status:** Proposed (the owner approves by merging the PR that adds this file)
- **Date:** 2026-11-27
- **Plan:** [ui-components](../plans/ui-components.md)

**Decision:** vendor Preact 11.2.0 (11,970 bytes, 4,997 gzip, MIT) as one unmodified ES module file, load it from the existing classic scripts with `import()`, and add no build step, no npm and no CDN.
**Why: maintainability.** Today's dashboard keeps focus and open dialogs alive with hand-written bookkeeping that exists only because `render()` rebuilds the page; a component renderer removes that class of code and its bugs. Speed is not the reason: once table decoration is counted the spike showed no real speed win outside Storage, and focus preservation is a side effect of the same change.
**Cost:** it is the first third-party code in the page that holds the write token, a manual review is due at least every 90 days, and the migration is about ten pull requests.
**Approve by merging.** The ask is repeated at the end.

## Context

The dashboard (`web/`) draws every page by building real DOM nodes with `h()` in `web/core.js` and replacing the children of `#crumb`, `#content` and `#layer` on every state change (`render()` in `web/shell.js`). Terms used here: the *old renderer* is that `render()`; the *Preact renderer* is the same `render()` handing the page to Preact; a *component* is a plain function that takes state and returns `h(...)` output; the *HTML shell* is `web/index.html`, and `shell.js` is a script.

### Maintainability evidence (counts on `main` at `e80bce1`, grep over `web/*.js`)

Each row is code that exists because the old renderer discards the DOM on every update. "Removed" means a component renderer, which patches the DOM in place, makes it unnecessary; "stays" means it does not.

| Workaround | Where, how much | Component model |
|---|---|---|
| Suppress redraws under open UI | `quiet()` in `core.js:371` with its guard `!S.menu && !S.modal && !trashOpen()`; 16 call sites in 4 files; the comment says polls "must not rebuild the DOM under an open menu or dialog" | **Partly.** Dialogs: removed in U5 (spike: a poll through `quiet()` kept focus and the same dialog node). Menus: `quiet()` stays through U4 to U6 and goes in U7, after a test shows a poll leaves an open menu open. |
| Restore focus after a redraw by hand | `hadReq` and `data-req` (`shell.js:51,97`, `page-privilege.js:16`), `had` and `data-nav` in `drawNav()`, `stFocus` (page-storage), `guideAct` (page-guide): 7 call sites; `trashFocusBack` (4 references) and `data-autofocus` (4) besides. 22 `.focus()` calls and 10 `document.activeElement` reads in all | Removed for these; a few `.focus()` calls (first focus of a dialog, return after close) stay, as `ref` callbacks. |
| Skip rebuilding to keep state | Nav signature `navSig` (3 uses), Guide signature `gsig` (7 uses): "a poll must not drop focus or the selected command" (`shell.js:72`) | Removed: diffing is the skip. |
| Layout driven by the redraw | The breadcrumb and search sit outside `#content` because "the search box must keep focus while a poll redraws the page" (`shell.js:10-11`) | Not forced any more; a later page layout could change. Not done in this plan. |
| State cached on DOM nodes | `el.dataset.grid` (`table.js:355`: the first-seen grid, so a reused node keeps a stale value), `data-cols`, `data-table`, the Copy layer rebuilt after every render from measured `offsetTop` (`decorateTables`, 4 refs) | **Stays** (decision D6): `decorateTables()` remains post-processing on the DOM. It gets cheaper to run, not simpler. |
| Dialog plumbing | `inert` on `.shell` (`page-storage.js:515`), the try/catch whose only job is to close a dialog that cannot be drawn, three `{ do: 'focus' }` effects in the pure state machine, `trashKeys` focus trap | `inert`, focus return and the focus effects: removed (refs). The focus trap and the try/catch: **stay**. |
| Manual DOM building | 683 `h(` calls (`\bh(`), 72 `onClick`, about 225 `style:` strings | **Stays**: components are still written with `h(...)`. What goes is the contract that `h()` returns a live Node (5 `svg.innerHTML` writes, one `append` for the download link, one for the Copy layer) which blocks any refactor of the builder. |
| Global coupling | `S.` read in 14 of 16 scripts, `rec(` 71 calls in 13 files, `render(` 22 references in 7, `pageShell(` 18 in 11 | **Stays.** Components do not change that `S`, `rec` and `render` are globals shared by classic scripts, nor the 2 s poll. |
| File size | 2,973 lines and 259,696 bytes of script; largest `page-storage-logic.js` 547, `page-storage.js` 518, `table.js` 473, `core.js` 406 lines (budget 800) | Not a goal; the migration is expected to shrink the page files slightly (the workarounds above), not restructure them. |

Bug history that came from these (evidence, not adjectives):

- The review round of PR #69 (commit `9ed786c`, "fix(web): escape names, list Trash moves in History, close stale dialogs") fixed: "Selection change, leaving the page and a dialog that cannot be drawn close the dialog with an announcement and focus return; autofocus is scoped to the dialog; one Escape closes one layer." Each is dialog plumbing around a redraw model.
- In the spike on the old renderer, an unrelated `rec({ toast: null })` while the Trash dialog was open rebuilt the dialog node and pulled focus off the button the person had tabbed to; the same sequence on the Preact renderer kept the node and the focus.
- The in-code comments quoted in the table (`shell.js:11`, `shell.js:26`, `shell.js:72`, `shell.js:97`, `core.js:370`) are the same constraint written five times.

The owner chose Preact, vendored, with no build step. This record states what that means and what it costs. Evidence comes from the Preact documentation ([llms-full.txt](https://preactjs.com/llms-full.txt), fetched 2026-11-27), the npm registry, and a throwaway spike in Chrome (branch `spike/preact-h-compat`).

### Facts about Preact

| Fact | Source |
|---|---|
| Current stable release is **11.2.0**, published 2026-11-06, licence **MIT** (Copyright Jason Miller). The last 10.x, 10.29.8, is from 2026-08-01; six 11.x releases (11.0.0 to 11.2.0) came between 2026-09-30 and 2026-11-06. | `npm view preact`; GitHub releases, 2026-11-27 |
| `h(type, props, ...children)`. `class` works; `className` also. Event props follow the DOM (`onClick`, `onInput`); an unknown `onTabChange` listens for `TabChange` as written. | llms-full.txt |
| Lowercase `onclick` fires. The documentation does not say so; the spike checked it. | spike |
| `style` strings work. In 11, automatic `px` for numbers moved to `preact/compat`; we pass strings only. | llms-full.txt, spike |
| 11 ships **ES modules only** (`.mjs`): no UMD, no global build. Checked in the 11.2.0 tarball: `dist/` holds `preact.mjs` and its map only. 10.29.8 does ship `dist/preact.umd.js` (11,443 bytes, 4,870 gzip). | tarballs, llms-full.txt |
| Browsers for 11 without polyfills: Chrome 71, Safari 12.1, Firefox 69, Edge 79. | llms-full.txt |
| `render()` lost its `replaceNode` argument. The text is silent on rendering into a container that has children, on focus, and on `inert`; the spike measured all three. | llms-full.txt, spike |
| htm (template strings) is optional; npm lists its licence as **Apache-2.0**. | llms-full.txt, `npm view htm` |
| 11.2.0 has a registry **SLSA provenance** attestation. It shows the tarball was built by a CI pipeline from a named repository; it says nothing about whether the code is benign. | `npm view preact@11.2.0 dist.attestations` |

## Decision

Use **Preact 11.2.0** for the dashboard's components.

1. **One vendored module.** `web/vendor/preact.mjs`, copied unmodified from the npm tarball, with `web/vendor/PREACT-LICENSE` (the MIT text as published), `VENDORED.md` and `SHA256SUMS`. No source map is served. `preact/hooks`, `compat` and signals are not vendored: hooks import the bare name `preact`, which would need an import map, and the code keeps its state in `S` and `rec()`.
2. **Loaded by `import()` from a classic script, with no import map and no module script.** All 16 scripts stay classic. `shell.js` starts the page exactly as today. If the switch is on, it runs `import('/vendor/preact.mjs').then(ok).catch(err)`. Until it resolves `h()` builds Nodes, so the first paint is unchanged. Readiness is a module-scoped flag in `shell.js` set by `ok`, not a `typeof window.preact` test. The first Preact render replaces everything the old renderer drew, so it happens only when nothing is focused or open (no focused element in the page, no menu, dialog, copy box or Trash state); otherwise the old renderer draws again and the next `render()` retries. On handover the Guide's `gsig` is reset so the Guide redraws. If `ok` or the first Preact render throws, `err` hands the containers back to the old renderer. Details are in todo U4.
3. **The switch** is a `localStorage` key, `monitor-lizard-ui`, set to `preact` in the browser's developer tools. There is no visible control. It is read once when `shell.js` loads, so setting or removing it takes effect on the next page load; removing it restores the old renderer. U7 deletes the key and the old renderer.
4. **Served like any other file.** One more `Source` in `src/web.rs` (`include_str!`), so the Host check, the `Sec-Fetch-Site` refusal, `no-store` and `nosniff` apply unchanged. The `Source` reuses the existing JavaScript type, so the header is `text/javascript; charset=utf-8`. The file stays byte-identical to the tarball, including its last line `//# sourceMappingURL=preact.mjs.map`; the map is not served, so DevTools logs a 404 for it when open (harmless).
5. **No remote assets, no CDN, no npm and no bundler in the repository.** No `package.json`. A human downloads a tarball only during a review.
6. **htm and signals are not adopted.** The code has about 680 `h(` calls (683 by `grep -o '\bh('`); they already read as hyperscript. htm would add a second licence (Apache-2.0) and a template dialect for no gain.
7. **`h` keeps its name and signature** (`h(tag, attrs, ...kids)`, `class`, `onClick`, `style` strings). Migration is incremental behind the switch; both renderers coexist until the last page moves (todos U3 to U7).
8. **Unchanged rules.** The per-run token stays in `<meta name="ml-token" content="__TOKEN__">` in the HTML shell and nowhere else. The Host check, the `Sec-Fetch-Site` refusal and the `no-store` and `nosniff` headers stay. A vendored file must not contain `__TOKEN__` (the existing Rust test checks every source).

## Evidence (spike, 2026-11-27)

Chrome, a scratch build of the server on port 47931, `HOME` pointing at a scratch folder, no resize, nothing from the real home folder. The spike's second round matches this design: **all app scripts classic**, no import map, `import()` of the vendored file, a switch in `localStorage`, `h` swapped for Preact's `h` while a page is built, the page committed with `preact.render`. Medians over 40 runs of the whole `render()` (nav, layer, `decorateTables()` and `syncHash` included), two rounds each, node counts checked equal after every loop:

| Page | Old renderer | Preact renderer | What the loop does |
|---|---|---|---|
| Storage, 240 items, 1,061 elements | 11.5 (p90 14.3) and 10.9 (11.7) ms | 2.5 (4.7) and 2.3 (2.8) ms | `render()` with the same data |
| Storage | 10.1 (11.9) and 9.8 (11.3) ms | 2.4 (6.3) and 4.0 (4.5) ms | page size toggled 10/25, so rows change |
| Storage | 12.2 (17.9) ms | 12.8 (14.0) ms | switch to another view and back (a full first mount) |
| Top consumers, 574 elements | 2.4 (7.1) and 3.7 (5.9) ms | 1.5 (7.1) and 1.4 (2.4) ms | `render()` with live data |

What the numbers show and do not show:

- **The gain is maintainability and state preservation, not speed.** Focus survived two `render()` calls and a click on the focused tile (the handler ran `rec()`); today's `render()` drops focus to `body`.
- On Storage the Preact renderer is faster for a specific reason: the old renderer builds new nodes and then `decorateTables()` forces layout on them (about 8 ms of the 11.4); with Preact the DOM is unchanged, so `decorateTables()` is cheap (0.2 ms in the steady state). When rows change the cost rises (2.4 to 4.0 ms), and a full first mount costs the same as today (12.8 against 12.2 ms). On Top consumers the old renderer was already about 2 to 4 ms.
- An earlier round of this spike reported 32 ms for Top consumers and a 2 ms update that left out `decorateTables()`. That comparison was unfair and the 32 ms did not reproduce; both are withdrawn.
- Size: the app's scripts are 259,696 bytes (82,005 gzip); Preact adds 11,970 (4,997 gzip), +4.6 percent raw and +6.1 percent gzip.

| Question | Verdict |
|---|---|
| Attribute shapes: `class`, `style` strings, `onClick`, lowercase `onclick`, `onKeydown`, `onInput`, `aria-*`, `data-*`, `disabled`, `inert`, SVG with kebab-case attributes and the SVG namespace | All correct. `e.currentTarget` is the handler's element. |
| Whole-page DOM equality with today's output (Storage, five Performance tabs), `style` text normalised | The only differences are `svg` bodies written with `svg.innerHTML`, which fails silently on a vnode (5 sites). |
| `decorateTables()` on Preact's DOM | Works. It must run after every commit, because the Copy layer goes stale when rows change. Export and row copy read data, not the DOM. |
| `render()` into a container that already has children | Old children stay after the new ones. A container has one owner; handing it back needs `render(null, container)` first. |
| Classic scripts only, real clicks, the whole app | Worked: a tile click changed state through `rec()` and kept focus. No console errors. |

Not tested: the Tauri window, Safari and Firefox, the Trash dialog and every page but Storage and Performance on the Preact renderer, `svg` bodies on the Preact renderer (U3 fixes them), and the nav, crumb and layer on the Preact renderer (they stayed on the old one in the spike).

## Consequences

**Two costs the owner decides on.**

1. **Third-party code in the page that holds the token.** About 12 kB of someone else's JavaScript will run in the page that holds the per-run write token. Any script in the page could read the token before, as now; this is the first script we did not write. The vendored file is minified and CI has no network. The offline `check-vendor` compares the file only with `SHA256SUMS` in the same PR, so it detects accidental drift; it cannot tie `preact.mjs` to the npm tarball, and a malicious PR would change both files. The upgrade routine below, run by a person with network access, is what ties the file to the registry (`npm view preact@X dist.integrity`) and reads the source.
2. **A manual review at least every 90 days, and at once on a security advisory.** Dependabot has no ecosystem for vendored files (`.github/dependabot.yml` lists `github-actions` and `cargo`). `VENDORED.md` carries a `Reviewed on: YYYY-MM-DD` line. `scripts/check-vendor.mjs`, run by `make check-docs` (so the always-on `docs` CI job executes it, including in a PR that only changes `VENDORED.md`), prints a GitHub `::warning::` annotation, and exits 0, when today is more than 90 days after `Reviewed on`. A missing or unparsable date fails.

**Review steps at every upgrade** (written in `web/vendor/VENDORED.md`, so the commands live next to the file):

- Choose a release at least a week old (`docs/guidelines/maintainability.md`).
- Recompute the tarball integrity: `npm view preact@X dist.integrity`, then `npm pack preact@X` in a new empty folder, and compare the sha512 of the tarball.
- Check provenance: `npm view preact@X dist.attestations` lists a SLSA provenance. That shows a CI build from the expected repository; it does not show the code is safe. Stop if a release that had one no longer does.
- **Read the shipped `src/` diff between the old and new tarball, and the release notes.** This is the real control: the minified file is not reviewable by eye, and the grep below is a denylist that obfuscated code can evade.
- Confirm `web/vendor/preact.mjs` is byte-identical to the tarball's `dist/preact.mjs`.
- Run the recorded grep: `grep -En 'fetch|XMLHttpRequest|eval\(|Function\(|document\.cookie|localStorage|sessionStorage|sendBeacon|WebSocket|import\(|\.src=|\.href=|createElement|innerHTML' web/vendor/preact.mjs`. For 11.2.0 on 2026-11-27 the only matches are `createElement` (2) and `.innerHTML` (2), both Preact building and filling DOM elements. Any other match, or a different count, means stop and read the diff.
- Replace `preact.mjs`, `PREACT-LICENSE`, `VENDORED.md` and `SHA256SUMS` in one PR and set `Reviewed on` to today. `SHA256SUMS` lists `preact.mjs` and `PREACT-LICENSE` only (not itself and not `VENDORED.md`).

**What gets easier.** Components keep focus and DOM state across updates, so the focus-restore code and `quiet()` can go, one at a time, each with its own browser check. A dialog can be a component whose open state lives in `S`; if it fails to draw, the page stays usable and the dialog closes. The "no build step" rule is kept: the files in `web/vendor/` are what the browser runs.

**Other consequences.**

- The binary grows by about 12 kB (`include_str!`).
- Preact adds no WebKit requirement beyond what the page already has; see the platform floor below.
- The token, meta tag and `Sec-Fetch` rules do not change. If a Content-Security-Policy is added later it needs `script-src 'self'`; `import()` of a same-origin file needs nothing else, since there is no inline script besides the theme bootstrap that already exists.
- The first paint is unchanged because the Preact renderer starts after `import()` resolves.

## Platform floor

The repository declares no minimum macOS version, and `src-tauri/tauri.conf.json` has no `minimumSystemVersion` (Tauri's documented default is `10.13`, [config reference](https://v2.tauri.app/reference/config/), fetched 2026-11-27). The Tauri window uses the system WebKit, so the floor is the WebKit of the macOS release.

The page already needs newer WebKit than Preact does. `web/app.css` uses `color-mix()` (2 uses) and `:has()` (1), and `page-storage.js` sets `inert`. From MDN's browser-compat-data (fetched 2026-11-27) Safari first supports `:has()` in 15.4, `inert` in 15.5 and `color-mix()` in 16.2. Preact 11 needs Safari 12.1 and `import()` needs Safari 11.1, so **the real floor is already about Safari 16.2 and Preact changes nothing.** Apple's security notes for Safari 16.2 (released 2022-12-13) list it for macOS Big Sur and Monterey; macOS Ventura (13) ships Safari 16.x. Whether a Tauri window on Big Sur or Monterey receives the Safari update's WebKit is not verified. A reasonable declared floor would be macOS 13 (Ventura); the owner decides.

Adding `minimumSystemVersion` to `tauri.conf.json` is a separate small change in Amber's shell, proposed as its own todo; it is not part of this plan. U1 includes a step to load the vendored file in the Tauri window and record the result.

## Tests

The Node tests have no DOM library: `tests/web/storage-trash-view.test.mjs` uses a hand-written `El` stand-in (about 25 lines) and lifts marked blocks by name. Options considered:

1. *jsdom or linkedom as a dev dependency.* Real DOM semantics, but a large tree of third-party code in CI and on developers' machines, a `package.json` and lockfile the repository does not have, and a dependabot ecosystem to add. Rejected.
2. *`preact-render-to-string` (MIT, 6.8.3) vendored for tests only.* It imports the bare name `preact`, so Node needs a resolve hook, and it renders strings only, so it cannot test focus, events or `decorateTables`. Rejected for now.
3. **Chosen: no new dependency.** Under test, `h` is a *vnode stand-in* that returns plain `{ type, props, children }` objects, with a walker (about 40 lines, `tests/web/vnode.mjs`, todo U2) that renders text and finds elements by attribute. Behaviour that needs real Preact diffing or a real focus is checked in a real browser, as `AGENTS.md` rule 7 requires, and recorded in each todo. The `El` stand-in retires in U7.

## Alternatives considered

Judged by maintainability first (what a future agent or person must learn, how much surface they depend on, whether it fits no build step, classic scripts and 800-line files, how much existing code is rewritten, how hard it is to leave), then by the costs weighed against it: third-party code in the token page, the review routine, and migration effort. "Spiked?" says whether a measurement backs a statement; the rest is judgement from how the tools work, and sizes of tools other than Preact were not measured.

| Option | Spiked? | Learning curve and surface | Fit: no build, classic scripts, 800-line files | Rewrite | Lock-in | Verdict |
|---|---|---|---|---|---|---|
| **Preact 11 (chosen)** | Yes | Three functions to learn (`h`, `render`, `Fragment`) plus `key` and `ref`; 11,970 bytes | Loads with `import()` from classic scripts; no tooling | About 650 of 683 `h(` calls unchanged; five sites change contract | Low: we keep our own `h` name, and the API is the React-style subset | Chosen |
| Stay vanilla, add a convention (`component(props) -> Node`, own `patch()`) | No | Nothing new to learn, but every future person learns our diff | Fits | Moderate: pages adopt the convention | None | Loses on the thing we want to remove: we would own a keyed diff, the code that drops focus and state. It is also the fallback if this record is declined |
| Svelte | No | New template syntax and a compiler to learn; large conceptual surface | Needs a compiler: breaks the no-build rule | Every page template | High (compiled output, own syntax) | Lost on the no-build rule |
| React with Vite | No | Largest surface: JSX, hooks, bundler config | Needs npm and a bundler: breaks the no-build rule | Every page, plus tooling | High | Lost on the no-build rule |
| Lit and web components | No | Web-component and template-literal concepts; shadow DOM rules | Light-DOM Lit would fit; shadow DOM breaks `app.css` and `decorateTables()` (`document.querySelectorAll('.tr.h[data-table]')` does not enter shadow roots) | Every page template | Medium | Lost on fit with the global stylesheet and table code |
| Alpine | No (from memory, not rechecked) | Directive attributes in HTML, evaluated at run time | Our markup is built in JavaScript, not written in HTML; the standard build uses `new Function`, which blocks a strict CSP later | Every page | Medium | Lost on fit |
| Mithril | No | Hyperscript like our `h` (`m(tag, attrs, kids)`), plus its own redraw and lifecycle model | Fits (no build) | Close to Preact for the markup | Medium | Close second. Its auto-redraw and own vnode shape would mean the incremental path of this plan does not carry over unchanged, and fewer people and tools know it |
| Preact 10.29.8 with its UMD file as a plain `<script>` | Size only | Same as Preact 11 | Fits, and avoids `import()` | Same | Same | **Fallback** if `import()` fails in the Tauri window. Not chosen first because 10.x has had no release since 2026-08-01 while 11.x had six releases in five weeks |
| Preact with htm or `@preact/signals` | No | Adds a template dialect or a second state model | Fits | More | Higher | Deferred, not rejected (Decision 6) |

Costs weighed against the maintainability gain: third-party code in the token page (see Consequences); the review routine (about one review per quarter); migration effort of ten pull requests (3 of size L and 7 of size M in the plan, roughly 3,500 changed lines by those size classes, an estimate).

What the spike measured on maintainability, for the most workaround-heavy part (the Trash dialog, 133 lines in the `storage-trash-view` block). The spike **deleted nothing** from the block: it added `!vnodeMode()` guards (so the old renderer still works) and two small `ref` functions. Reading the diff, about **5 or 6 lines** become obsolete once the old renderer is gone: the `setTimeout(trashFocusBack, 0)` in `trashSync`, the focus branch in `trashDispatch`, the `data-autofocus` query in `render()`, the `shell.inert` assignment in `trashLayer`, and the `!trashOpen()` clause in `quiet()`. An earlier figure of "about 14 lines" was an estimate that also counted the `trashFocusBack` definition and the three `{ do: 'focus' }` effects in the state machine; treat it as an upper bound. The line gain is small. What stays: the markup functions (about 60 lines), the focus trap `trashKeys`, the try/catch, and the pure state machine.

The gain is behavioural. On the old renderer an unrelated `rec()` while the dialog is open rebuilds the dialog and pulls focus off the button the person tabbed to; on the Preact renderer the dialog node and the focus are kept, and a poll cannot disturb them, which removes the reason `quiet()` has to know about dialogs. A test can build the dialog as a plain vnode object with no fake DOM: the spike's `trash-vnode.test.mjs` is 29 lines with 4 helper lines, where `storage-trash-view.test.mjs` carries about 40 lines of fake DOM and selector support (the `El` stand-in). The vnode test cannot check focus or `inert`, because refs run only in a browser.

## Documents that change when this is accepted

These edits ship in the todos, not in this PR.

| File and place | Change | Todo |
|---|---|---|
| `AGENTS.md`, "What this project is" and the layout block line for `web/` | Say Preact is vendored in `web/vendor/`; still no build step and no remote assets | U1, final wording U7 |
| `docs/agents/layout.md`, `src/web.rs` row | `SOURCES` also lists `/vendor/<file>`; the vendor checks | U1 |
| `docs/agents/layout.md`, `web/*.js` row | One `import()` in `shell.js`; scripts stay classic | U4 |
| `docs/agents/layout.md`, "Adding a dashboard page" | Pages return components | U6a to U6d |
| `docs/agents/layout.md`, `tests/web/` rows | The vnode stand-in; the `El` stand-in retires | U2, U7 |
| `docs/agents/layout.md`, new `web/vendor/` row beside `web/fonts/` | What is vendored; checksums; the review rule | U1 |
| `docs/agents/ci-and-release.md` | Dependabot cannot watch vendored files; the review steps | U1 |
| `Makefile` | New target `check-vendor`, run by `check-docs` (the CI `docs` job runs `make check-docs` on every PR) | U1 |
| `THIRD-PARTY-NOTICES.md`, `SECURITY.md`, `docs/guidelines/maintainability.md` | Notice; one sentence on the one vendored library; one link | U1 |

## The ask

Approve by merging this PR: vendor Preact 11.2.0 as one unmodified file loaded with `import()`, for the maintainability gain in the table above; accept the token-page and 90-day review costs; and decide whether to declare a macOS floor.
