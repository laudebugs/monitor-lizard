# UI components with Preact (vendored, no build step)

- **Status:** Draft
- **Date:** 2026-11-27
- **Baseline:** `main` at `e80bce1`, inspected 2026-11-27
- **Asked for by:** the owner (chose Preact, vendored, no build step)
- **Builds on:** [ADR 0005](../decisions/0005-preact-vendored-no-build-step.md), [ADR 0002](../decisions/0002-rust-core-polling-collectors-local-web-ui.md), [T2](../todos/done/T2-split-web-page-into-files.md)
- **Todos:** [U1](../todos/ready/U1-vendor-preact-and-serve-it.md), [U2](../todos/ready/U2-vnode-test-harness.md), [U3](../todos/ready/U3-make-h-call-sites-renderer-neutral.md), [U4](../todos/ready/U4-render-shell-through-preact.md), [U5](../todos/ready/U5-trash-dialog-and-focus-as-components.md), [U6a](../todos/ready/U6a-migrate-small-pages.md), [U6b](../todos/ready/U6b-migrate-medium-pages.md), [U6c](../todos/ready/U6c-migrate-performance-page.md), [U6d](../todos/ready/U6d-migrate-storage-page.md), [U7](../todos/ready/U7-delete-old-renderer-and-hacks.md), [U8](../todos/ready/U8-declare-minimum-macos-version.md)

## 1. Goal

The reason is maintainability. Draw the dashboard with components so that the hand-written bookkeeping that exists only because `render()` rebuilds the page can be deleted: `quiet()` and its 16 call sites, the `hadReq`/`data-req`/`stFocus`/`guideAct`/`trashFocusBack` focus restores, the signature skips (`navSig`, `gsig`) and the per-dialog `inert` and focus code. The ADR's [maintainability table](../decisions/0005-preact-vendored-no-build-step.md#maintainability-evidence-counts-on-main-at-e80bce1-grep-over-webjs) has the counts and says what a component model does not remove (globals, `S`, the poll, `decorateTables()`). Speed is not a goal and the spike found no real speed win; focus preservation is a side effect. Constraints from the owner: Preact, vendored, no build step, no npm or bundler in the repository, no remote assets. Success means: every row the ADR marks "removed" is zero in U7's before and after table, and each migration PR shows the workarounds it deleted; every page is checked in a real browser at each step; the token, Host and `Sec-Fetch` rules are unchanged; and the dashboard works after every merged todo.

Terms (same as the ADR): the *old renderer* is today's `render()` building DOM nodes and replacing children; the *Preact renderer* is `render()` handing the page to Preact; a *component* is a function from state to `h(...)` output; the *switch* is the `localStorage` key `monitor-lizard-ui` set to `preact` in the browser's developer tools; the *HTML shell* is `web/index.html`. The *vnode stand-in* (new) and the *`El` stand-in* (existing) are the two test fakes.

## 2. What the system does today

Dated 2026-11-27, `main` at `e80bce1`. See also [layout.md](../agents/layout.md).

- `web/core.js` holds `h(tag, attrs, ...kids)`: it creates a real element, sets `class` with `setAttribute`, wires each `on*` attribute with `addEventListener`, and returns a live `Node`. `rec(m)` merges into the state `S` and calls `render()`; `quiet(m)` does the same unless a menu, the Stop dialog or the Trash dialog is open.
- `web/shell.js` `render()` prunes tables, redraws the nav (with a hand-written focus restore), rewrites `#notif` and `#theme`, builds the page (`overview()`, `apps()`, ... each returning `{ crumb, nodes }`), calls `replaceChildren` on `#crumb`, `#content` and `#layer`, runs `decorateTables()`, restores focus from `data-autofocus` or `data-req`, then `syncHash`.
- Counts on `main` (grep over `web/*.js`): 683 `h(` calls (`\bh(`: storage 178, performance 129, apps 67, privilege 55, guide 45, overview 42, history 42, shell 42, table 21, listeners 16, persistence 16, core 15, network 13, data 2); 72 `onClick`, 5 `onKeydown`, 1 `onChange`; about 225 `style:` strings; 71 `rec(` and 16 `quiet(` calls; 22 `.focus()` and 10 `document.activeElement`; 6 `replaceChildren`; 5 `.innerHTML` writes; 20 `querySelector`.
- **Sites that treat an `h()` result as a live Node** (the contract U3 removes):
  1. `svg.innerHTML = ...` on an `h('svg')` result, 5 sites: `shell.js` nav icon; `page-performance.js` sparkline, multi-line and stacked charts; `page-storage.js` capacity chart. On a vnode this fails silently.
  2. `download()` in `table.js` appends an `h('a')` result to `document.body` and clicks it.
  3. `decorateTables()` in `table.js` appends an `h('div')` Copy layer to a section after measuring `offsetTop`, writes `style.display` on cells, `--tracks-min`, classes, `tabindex`, `role` and `aria-label` on the section, and caches the original grid in `el.dataset.grid` (`table.js:355`).
  4. `nodesOf` inside `h()`, `pageShell()` (`nodesOf([filters]).length`) and `show()`.
  5. A layout read while building: `document.querySelector('.spark')?.clientWidth` (`page-performance.js`).
  6. `replaceChildren(...)` and `.append(...)` in the renderer itself: `drawNav`, `show`, the notif and theme rows, the layer (6 sites, all `shell.js`).
  7. `instanceof Node` inside `h()` (`core.js:112`).
  8. Live-node writes outside the renderer: `el.style.setProperty` and `classList.add` in the drawer resize (`table.js:455`), `shell.inert = ...` (`page-storage.js:506`), and the 22 `.focus()` calls.
  Everything else (about 650 of 683 calls) only builds elements and hands them to a parent `h()`.
- Tests: `tests/web/*.test.mjs` lift marked blocks from the assembled page (`tests/web/page.mjs`) and run them against fakes (the `El` stand-in in `storage-trash-view.test.mjs`); there is no jsdom. `split.test.mjs` allows one inline script (the theme bootstrap) and checks the load order; `src/web.rs` compares `<script src=` tags with every JavaScript source and compares `web/` on disk (top level only) with `SOURCES`.
- The code from B7 (#50), S2a (#51), S2b (#54) and S3b (#69) is on `main`; their todo files sit in `ongoing/` only for owner-run checks and are not touched here.

## 3. Sources

- Preact documentation ([llms-full.txt](https://preactjs.com/llms-full.txt), fetched 2026-11-27): `h`, `render`, event naming, htm, import maps, the 10 to 11 changes. Silent on vendoring, non-empty containers and focus; the spike answered those.
- npm registry (`npm view`, 2026-11-27): preact 11.2.0 (MIT, no UMD, SLSA provenance), 10.29.8 (UMD, last release 2026-08-01), htm 3.1.1 (Apache-2.0), preact-render-to-string 6.8.3 (MIT).
- Tauri config reference, fetched 2026-11-27: `minimumSystemVersion` defaults to `10.13`.
- The option comparison and its labels are in the ADR.

## 4. Decisions

The evidence for each is in the [ADR](../decisions/0005-preact-vendored-no-build-step.md#evidence-spike-2026-11-27); this table says what to build.

| ID | Decision | Status |
|---|---|---|
| D1 | Adopt Preact 11.2.0, one vendored file in `web/vendor/`, no build step, no npm, no CDN. | Proposed until the ADR merges. |
| D2 | `h` keeps its name and signature. While a page is built for the Preact renderer, `h` returns Preact vnodes; otherwise Nodes as today. Shared helpers (`labelTag`, `dropdown`, `timeCell`) work in both, so pages move one at a time. | Spiked, verified on Storage and Performance. |
| D3 | Remove the "`h()` returns a Node" contract first (U3), on the old renderer, with no behaviour change. One helper, `svgBody`, is the only place that sets an SVG body, and the only place `dangerouslySetInnerHTML` appears. | Spiked: 5 silent sites. |
| D4 | The two renderers coexist. Each of `#crumb`, `#content` and `#layer` is owned by exactly one renderer at a time; handing back calls `preact.render(null, container)` and then `replaceChildren()`. The switch selects the Preact renderer; the default stays the old one until U7. | Spiked: `render()` into a non-empty container leaves the old children. |
| D5 | **All scripts stay classic.** No import map, no module script. `shell.js` loads Preact with `import('/vendor/preact.mjs').then(ok).catch(err)` when the switch is on. Readiness is a script-level flag set by `ok`. The first Preact commit replaces what the old renderer drew, so it waits until nothing is focused or open, keeps `#main`'s scroll position, and resets `S.gsig` for the Guide. The switch is read once at load. | Spiked in round two: classic scripts, real clicks, no console errors. The spike detected readiness with `typeof window.preact` and did not wait for a safe moment; U4 specifies both. Not yet tried in the Tauri window (U1). |
| D6 | `decorateTables()` stays DOM post-processing and runs after every Preact commit. It is not turned into components in this plan. | Spiked: works; the Copy layer goes stale without the call. |
| D7 | Focus: stop restoring by hand. One stable DOM per component and keys on lists; delete `hadReq`/`data-req`, `had`/`data-nav`, `stFocus`, `guideAct`, `trashFocusBack`. Keep `data-autofocus` for a dialog's first focus. Focus return and `inert` change in a post-commit step (a `ref` callback or a call after `preact.render`), never as a side effect inside a render function. | Spiked for a stable tree; keyed lists are checked in U6. |
| D8 | Tests keep running in Node without a DOM library: the vnode stand-in plus a text walker; no jsdom, linkedom or vendored `preact-render-to-string`. Browser behaviour is checked by hand and recorded. | Decision. Uncertain: whether the stand-in covers the dialog tests (U2 proves it on one). |
| D9 | No htm, signals, hooks or `preact/compat`. | Decision. |
| D10 | Upgrades are manual: a review at least every 90 days with the steps in the ADR. `scripts/check-vendor.mjs`, run by `make check-docs` (the CI `docs` job, on every PR), prints a `::warning::` annotation and exits 0 when today is more than 90 days after the `Reviewed on` date in `VENDORED.md`. | Decision; U1 builds it. |
| D11 | Rollback at every step is a revert of that step's PR. U4 to U6d also leave the old renderer selectable: **removing the switch** (delete the key in developer tools and reload: it is read once at load) restores it. U7 deletes both the switch and the old renderer, so its rollback is the revert. | Consistent with U4. |
| D12 | Each migration todo has maintainability acceptance criteria: the page deletes its workarounds, its logic tests need no browser, and the PR records before and after counts (workarounds and lines). U7 collects the totals. | Decision. |

## 5. Order of work

| Todo | Summary | Depends on | The dashboard works because | Rollback |
|---|---|---|---|---|
| U1 | Vendor Preact, serve it, `check-vendor` inside `check-docs`, notices, Tauri window check | None | Nothing loads it yet; one extra file is served | Revert the PR |
| U2 | Vnode stand-in and walker; port one dialog test | None | Test-only | Revert the PR |
| U3 | Make every `h()` site renderer-neutral; add the guards | U2 | Old renderer only; each change preserves behaviour | Revert the PR |
| U4 | `h` vnode mode; `import()` in `shell.js` with a safe-moment handover; the Preact renderer for `#crumb`, `#content` and `#layer` behind the switch; `decorateTables()` after each commit | U1, U3 | The default is untouched; a page that throws under Preact falls back to the old renderer for that render | Delete the switch key, or revert the PR (the revert removes the `import()` and `window.preact` use) |
| U5 | Stop and Trash dialogs, toast, copy box as components; focus return and `inert` as post-commit steps | U4 | The old renderer keeps its dialog code until U7 | Delete the switch key, or revert |
| U6a | Persistence, Listeners, History, Network | U4 | Pages build through neutral `h`, so they render on both renderers | Revert |
| U6b | Guide, Overview, Apps, Privilege | U6a | Same | Revert |
| U6c | Performance | U6b | Same | Revert |
| U6d | Storage | U5, U6c | Same | Revert |
| U7 | Preact becomes the default; delete the old renderer, `quiet()`, `hadReq` and the rest; update the docs | U6d | Last step; every page has run on the Preact renderer in a browser first | Revert the PR (one squash commit restores the old renderer and the switch) |

Why this order: the risks are the contract change (U3) and ownership of the containers (U4), so they come first, behind no behaviour change or the switch. Pages then move from least to most risky: Persistence, Listeners, History, Network (26 to 56 lines each); Guide (own redraw skip), Overview, Apps (drawer), Privilege; Performance (live charts every 2 s); Storage (518 lines, treemap, drawer, selection, the Trash flow). "Migrate" in U6 means: run the page on the Preact renderer in a browser, fix what breaks, delete that page's focus workarounds, and add its vnode-stand-in tests.

**U8** (declare the minimum macOS version in the Tauri shell) is independent of the rest and has no dependencies.

Because every page works on both renderers after U4, U6a to U6d can ship in any order that respects their dependencies and each is revertable on its own.

## 6. Risks and open questions

- **Silent failures.** A page can render wrong without throwing (the `svg.innerHTML` case). Each U6 todo diffs the DOM from both renderers on live data (the spike's walk) before the page is accepted.
- **`style` text.** Preact writes `style` through the CSSOM, so text differs from ours (`flex: 999 1 640px; min-width: 0px;`). `tracksOf` reads `grid-template-columns:\s*...`, which tolerates it; U3 greps for other `getAttribute('style')` readers (one today).
- **Keys.** Unkeyed lists that reorder reuse the wrong element or move focus. U6 lists each list and its key.
- **Foreign mutations on Preact's DOM.** `decorateTables()` and the drawer resize write attributes and children Preact does not know. They survive while vnode props are unchanged; a changed `style` prop resets what they wrote. The post-commit pass re-applies them; U6 tests it.
- **Third-party code in the token-holding page.** Mitigated by review steps, not removed; see the ADR.
- **Platform floor.** The repository declares no macOS minimum; see the ADR. U1 loads the vendored file in the Tauri window.
- **Open:** whether the vnode stand-in covers the dialog focus assertions (U2); whether `import()` works in the Tauri window (U1; not verified; fallback in the ADR is the 10.29.8 UMD file as a plain `<script>`).

## 7. Spike results

The branch `spike/preact-h-compat` (never merged; last commit `f30c84f`) vendors Preact, adds a `Source` row, and in round two changes `h` from `const` to `let`, adds a `build()` mode switch in `core.js` and the `import()` plus a `commit()` helper in `shell.js`. Its numbers and verdicts are in the [ADR](../decisions/0005-preact-vendored-no-build-step.md#evidence-spike-2026-11-27); this section records the method and what is left.

**Method.** Preact 11.2.0 from the npm tarball (integrity matched the registry). A release build of the server from the spike branch, started with `HOME` set to a scratch folder, on port 47931; Chrome, a new tab, no resize. The scanner refused the scratch `HOME` ("not a folder of yours at least two levels below the root"), so Storage was fed 240 synthetic items shaped like the test fixtures; Performance used this machine's live sampler. The real home folder, `~/.monitor-lizard`, `~/.sys-monitor`, `~/.ownwatch` and `~/.Trash` were not touched.

**Round one** (import map, `h` swapped from the console) covered attribute shapes, whole-page DOM equality, focus, `decorateTables()` and a first set of timings. Its design (module `shell.js` plus import map) was dropped because a module's top-level names are not globals, so the scripts that call `render()`, `pageShell`, `show` and `timeCell` would throw `ReferenceError`; the import map would also have broken the load-order tests. **Round two** ran the app with classic scripts only and measured the whole `render()`; its table is in the ADR.

**What broke.** `svg.innerHTML` on a vnode (5 sites, silent). A test harness error of mine: clearing a Preact container with `replaceChildren()` instead of `preact.render(null, container)` left stale internal state, so the next render drew nothing and its timing was meaningless; the numbers in the ADR come from a rerun with a correct handover and equal node counts.

**Round three** (maintainability) converted the Trash dialog to the Preact renderer on the spike branch (commit `f30c84f`): refs for `inert` and focus return, a keyed heading for focus on each phase, and the `!trashOpen()` clause in `quiet()` skipped. The spike **deleted nothing** there; it added `!vnodeMode()` guards and two ref functions. By reading the diff about 5 or 6 of the block's 133 lines become obsolete once the old renderer is gone (an earlier estimate of 14 was an upper bound), so the line gain is small. The gain is behavioural: an unrelated `rec()` and a poll no longer rebuild the open dialog or move focus (the old renderer does both). A 29-line vnode test checks the dialog's escaping, roles, key and refs with 4 helper lines where the existing test carries about 40 lines of fake DOM; it cannot check focus or `inert`, which need a browser. Details are in the ADR. The spike commits (`bb8259a`, `f30c84f`) are pushed to `origin/spike/preact-h-compat` so these claims can be reviewed.

**Not tested.** The Tauri window, Safari, Firefox, the Trash dialog and every page except Storage and Performance on the Preact renderer, the nav, crumb and layer on the Preact renderer, and `svg` bodies on the Preact renderer.
