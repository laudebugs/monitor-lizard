# UI components with Preact (vendored, no build step)

- **Status:** Draft
- **Date:** 2026-11-27
- **Baseline:** `main` at `e80bce1`, inspected 2026-11-27
- **Asked for by:** the owner (chose Preact, vendored, no build step)
- **Builds on:** [ADR 0002](../decisions/0002-rust-core-polling-collectors-local-web-ui.md), [ADR 0005](../decisions/0005-preact-vendored-no-build-step.md), [T2](../todos/done/T2-split-web-page-into-files.md)
- **Todos:** [U1](../todos/ready/U1-vendor-preact-and-serve-it.md), [U2](../todos/ready/U2-vnode-test-harness.md), [U3](../todos/ready/U3-make-h-call-sites-renderer-neutral.md), [U4](../todos/ready/U4-render-shell-through-preact.md), [U5](../todos/ready/U5-trash-dialog-and-focus-as-components.md), [U6a](../todos/ready/U6a-migrate-small-pages.md), [U6b](../todos/ready/U6b-migrate-medium-pages.md), [U6c](../todos/ready/U6c-migrate-performance-page.md), [U6d](../todos/ready/U6d-migrate-storage-page.md), [U7](../todos/ready/U7-delete-old-renderer-and-hacks.md)

## 1. Goal

Draw the dashboard with components, so a state change updates only what changed, focus and dialogs stop being special cases, and a new page is a function of state. Constraints from the owner: Preact, vendored, no build step, no npm or bundler in the repository, no remote assets. Success: the focus-restore code, `quiet()` and the `hadReq` lookup are deleted; every page is checked in a real browser at each step; the token, Host and Sec-Fetch rules are unchanged; and at no step is the dashboard broken.

## 2. What the system does today

Dated 2026-11-27, `main` at `e80bce1`. See also [layout.md](../agents/layout.md).

- `web/core.js` holds `h(tag, attrs, ...kids)`: it creates a real element (SVG namespace for `svg` only), sets `class` with `setAttribute`, wires any `on*` attribute with `addEventListener(name.slice(2).toLowerCase())`, and appends children, turning strings into text nodes. It returns a live `Node`. `rec(m)` merges into the state `S` and calls `render()`. `quiet(m)` does the same unless a menu, the Stop dialog or the Trash dialog is open.
- `web/shell.js` `render()`: prunes tables, `drawNav()` (rebuilt only when its signature changes, with a hand-written focus restore), rewrites `#notif` and `#theme`, builds the page (`page = overview()` and so on, each returning `{ crumb, nodes }` from `pageShell()`), `replaceChildren` on `#crumb` and `#content`, builds the layer (Stop dialog, Trash dialog, toast, copy box) into `#layer`, runs `decorateTables()`, then restores focus from `data-autofocus` or `data-req`, then `syncHash`. The Guide skips the rebuild unless `guideSig` changed.
- Counts on `main` (grep over `web/*.js`): about **787** `h(` calls (page-storage 178, page-performance 129, page-apps 67, page-privilege 55, page-guide 45, page-overview 42, page-history 42, shell 42, table 21, page-listeners 16, page-persistence 16, core 15, page-network 13, data 2); **72** `onClick`, 5 `onKeydown`, 1 `onChange`; about 225 `style:` strings; **71** `rec(` and **16** `quiet(` calls; **22** `.focus()` calls and 10 `document.activeElement` reads; 6 `replaceChildren`; 5 `.innerHTML` writes; 20 `querySelector` calls.
- **Call-site patterns that treat an `h()` result as a real Node** (the contract the spike tested):
  1. `svg.innerHTML = '<path .../>'` on an `h('svg', ...)` result: **5 sites** (`shell.js` nav icon; `page-performance.js` sparkline, multi-line chart and stacked chart; `page-storage.js` capacity chart). On a vnode this fails **silently**.
  2. `document.body.append(a); a.click(); a.remove()` on an `h('a')` result in `download()` (`table.js`): 1 site. Needs a real element.
  3. `sec.append(h('div', { class: 'rowcopy-layer' ... }))` in `decorateTables()` (`table.js`): 1 site; it builds Copy buttons after render from measured positions (`offsetTop`).
  4. `nodesOf(...)`: used inside `h()` and `pageShell()` (`nodesOf([filters]).length` asks whether a filter row has children) and by `show()`: 4 sites besides its definition. It must keep meaning "drop null and false" for vnodes too.
  5. Layout read during a build: `document.querySelector('.spark')?.clientWidth` in `page-performance.js`: 1 site. With diffing the old DOM stays in place while the page builds, so the read returns the previous width.
  6. `h()` result passed to `replaceChildren(...)` or `.append(...)`: `drawNav`, `show`, the notif and theme rows, the layer: 6 sites, all in `shell.js`. These are the renderer itself and are replaced in U4.
  7. Event handlers that read `e.currentTarget` and then query inside it (`tileKeys`, `trashKeys`, the column menu, `dropdown` keys): 5 sites. Works unchanged (spike).
  Everything else (about 770 of 787 calls) only builds elements and hands them to a parent `h()`.
- Table machinery (`web/table.js`): `decorateTables()` runs after every render over the real DOM (`.tr.h[data-table]`), hides columns by writing `style.display`, writes `--tracks-min` and the `floor`, `xscroll`, `has-copy` classes, sets `tabindex`, `role` and `aria-label` on the scroll region, and appends the Copy layer. Export and row copy read `TABLES` and `HEADERS` (data), not the DOM.
- Tests: `tests/web/*.test.mjs` (3,182 lines) lift marked blocks by name from the assembled page (`tests/web/page.mjs`) and run them against hand-written stand-ins (an `El` class in `storage-trash-view.test.mjs`); there is no jsdom.
- Server: `src/web.rs` lists each file in `SOURCES` with `include_str!`; `reply()` applies the Host check, the `Sec-Fetch-Site` refusal for non-shell files, `no-store` and `nosniff`; tests check that `web/` on disk and `SOURCES` agree and that only the shell holds the token.
- In flight: `S2a`, `S2b`, `S3b`, `B7` are in `ongoing/` and touch `page-storage.js`, `page-performance.js` and `shell.js`. U6c and U6d must land after those; U1 and U2 touch no dashboard file those todos edit, so they can start now.

## 3. What others do

- Preact documentation ([llms-full.txt](https://preactjs.com/llms-full.txt), fetched 2026-11-27): `h`, `render`, hooks (`preact/hooks`), optional htm for build-less templates (`htm.bind(h)`), optional signals; import maps for bare specifiers; version 11 is ES modules only and removed `render`'s `replaceNode` argument. The text is silent on vendoring, on rendering into a non-empty container and on focus; the spike answered those.
- npm registry (`npm view preact`, 2026-11-27): 11.2.0 published 2026-11-06, MIT; htm 3.1.1 is Apache-2.0; preact-render-to-string 6.8.3 is MIT.
- Option comparison is in the ADR (labelled as opinion where not measured).

## 4. Decisions

| ID | Decision | Why | Evidence and status |
|---|---|---|---|
| D1 | Adopt Preact 11.2.0, vendored in `web/vendor/`, no build step, no npm, no CDN. | Owner's choice; see the [ADR](../decisions/0005-preact-vendored-no-build-step.md). | Proposed until the ADR merges. |
| D2 | `h` keeps its name and signature; components are functions calling `h`. In vnode mode `h` returns a Preact vnode; in DOM mode it returns a Node as today. A page is built in one mode per render. | About 770 of 787 call sites need no change; shared helpers (`labelTag`, `dropdown`, `timeCell`) work in both modes, so pages can move one at a time. | Spike (a): attribute shapes all pass; Storage and all five Performance tabs produce the same DOM once `style` text is normalised. **Verified.** |
| D3 | Remove the "`h()` returns a Node" contract first (U3), on the old renderer, with no behaviour change. | The one failure class is silent: `svg.innerHTML` on a vnode draws nothing. A neutral helper (`svgBody(svgAttrs, markup)`) uses `dangerouslySetInnerHTML` in vnode mode and `innerHTML` in DOM mode. | Spike: 5 sites, the only DOM difference found. **Verified; premise partly disproved** (a wide break was feared; it is narrow but silent). |
| D4 | Both renderers coexist. `#content` and `#layer` are each owned by exactly one renderer at a time; handing over calls `render(null, container)` then `replaceChildren()`. A localStorage opt-in (`monitor-lizard-ui=preact`) selects the Preact path per page during the migration; the default stays the old path until U7. | Preact's `render` does not clear a container with children (the old children stay after the new ones), so shared ownership corrupts the page. An opt-in lets every step ship with the old path as the default. | Spike: render into a non-empty container left `old` after `new`. **Verified.** |
| D5 | `shell.js` becomes the one `<script type="module">`; Preact is imported there through an import map in the shell. The other scripts stay classic. | Modules run after all classic scripts, and the page's own bootstrap runs `render()` at load. A module can read the classic scripts' globals. | Spike: a module read `TOKEN`, `S` and `nodesOf`; the bare `preact` specifier resolved through the import map. **Verified.** |
| D6 | `decorateTables()` stays as DOM post-processing and runs after every Preact commit (a call after `render()` in the one place that commits). It is not turned into components until a later plan. | It works on Preact's DOM; export and row copy read data. Rewriting it is out of scope and risky. | Spike (c): column hiding, scroll region and Copy layer all applied; the layer goes stale on row changes, so the post-commit call is required. **Verified.** |
| D7 | Focus: stop restoring by hand. Keep one stable DOM per component and key lists; delete `hadReq`/`data-req`, `had`/`data-nav`, `stFocus`, `guideAct`, `trashFocusBack`. Keep `data-autofocus` for a dialog's initial focus. | Preact patches in place, so a focused element survives. | Spike (b): a focused tile stayed focused across 5 re-renders and a state change; today's `render()` loses it. **Verified** for a stable tree; keyed lists still need keys (U6 checks each list). |
| D8 | Tests keep running in Node without a DOM library: a vnode stand-in for `h` plus a text walker, no jsdom, no linkedom, no vendored `preact-render-to-string`. Browser behaviour is checked by hand and recorded. | No new supply-chain surface in CI; see the ADR's test options. | **Decision, not measured.** Uncertain: whether the stand-in is enough for the dialog tests (U2 proves it on one). |
| D9 | No htm, no signals, no `preact/compat`, no hooks until a todo needs a hook. | Smaller surface; htm is Apache-2.0 and adds a template dialect. `preact/hooks` (1,596 bytes gzip) is vendored only when a component needs local state. | Decision. |
| D10 | Upgrades are manual and dated: review every 90 days, at once for an advisory; the reviewer replaces the files, `VENDORED.md` and `SHA256SUMS` in one PR. | Dependabot has no ecosystem for vendored files. | Decision (U1 writes the rule). |
| D11 | Rollback at every step is a revert of that step's PR; U4 to U6d also keep the old path selectable so a user can switch back without a rebuild. | Each todo is one PR and one squash commit. | See section 5. |

## 5. Order of work

| Todo | Summary | Depends on | Page works because | Rollback |
|---|---|---|---|---|
| U1 | Vendor Preact, serve it, checksum tests, licence, import map | None | Nothing imports it yet; two new files are served and an import map sits unused | Revert the PR; no page code changed |
| U2 | Vnode test stand-in and walker; port one dialog test | None | Test-only change | Revert the PR |
| U3 | Make every `h()` call site renderer-neutral (svg body helper, download anchor, `nodesOf`, layout read) | U2 | Still the old renderer; every change is behaviour-preserving and covered by tests plus a browser check | Revert the PR |
| U4 | `vh` mode for `h`; `shell.js` as a module; render `#content` and `#layer` through Preact when the opt-in is set; `decorateTables()` after commit | U1, U3 | Default path is untouched; the opt-in path renders every page through Preact with old-style components | Remove the opt-in (clear the localStorage key) or revert the PR |
| U5 | Trash dialog and Stop dialog as components; delete their focus workarounds on the Preact path | U4, S3b | Old path keeps its code until U7; the Preact path has the new components | Clear the opt-in or revert |
| U6a | Migrate Persistence, Listeners, History, Network | U4 | Pages are built by neutral `h`, so they render on both paths; this todo removes per-page DOM assumptions found in the check | Revert |
| U6b | Migrate Guide, Overview, Apps, Privilege | U6a | Same | Revert |
| U6c | Migrate Performance (sparklines, charts, live toggle) | U6b, B7 | Same | Revert |
| U6d | Migrate Storage (largest page, treemap, drawer, dialog) | U5, U6c, S2a, S2b, S3b | Same | Revert |
| U7 | Make Preact the default; delete the DOM-mode `h`, `quiet()`, `hadReq`, `drawNav` focus code, the `El` stand-in; update docs | U6d | Last step; the old path is deleted only after every page has run on the new path in a browser | Revert the PR (one squash commit restores the old path) |

Why this order: the risk is the contract change (U3) and the ownership of the containers (U4), so they come first, each behind no behaviour change or an opt-in. Pages then move from least to most risky: Persistence (26 lines), Listeners (29), History (56), Network (41), then Guide (has its own redraw skip, `gsig`), Overview, Apps, Privilege (detail drawer), then Performance (live charts, 2 s polling, 129 `h` calls), then Storage (518 lines, treemap, drawer, trash dialog, selection state). "Migrate" for U6 means: run the page on the Preact path in a browser, fix what breaks, delete that page's focus workarounds, and add its vnode tests; the page code itself changes little because of D2.

## 6. Risks and open questions

- **Silent failures.** A page can render wrong without throwing (the `svg.innerHTML` case). Each U6 todo diffs the DOM from both paths on real data in the browser (the spike's walk is the method) before the page is accepted.
- **`style` normalisation.** Preact writes `style` through the CSSOM, so attribute text differs from what we write (`flex: 999 1 640px; min-width: 0px;` for `flex:999 1 640px;min-width:0`). `tracksOf` reads `grid-template-columns:\s*...`, which tolerates it. Any other code or test that matches `style` text must be found in U3 (grep for `getAttribute('style')`; one site today).
- **Keys.** Unkeyed lists that reorder will move focus or reuse the wrong element. Each list that sorts or filters needs `key`; U6 lists them.
- **Dialogs outside Preact.** `closeTopLayer`, the global keydown and the document click handler stay outside until U5; they read `S`, not the DOM.
- **Third-party code in the page that holds the token.** Mitigated, not removed: see the ADR.
- **Preact 11 is three weeks old.** The ADR names 10.29.8 as the fallback; the API we use is identical.
- **Open:** whether the vnode stand-in (D8) can cover the Trash dialog's focus assertions (U2 will tell); whether `import()` of a module under the Tauri webview behaves like Chrome (A17's shell loads the same loopback page; U4 must check it, and it is **not verified**).
- **Open:** a Content-Security-Policy does not exist today. If one is added, it needs `script-src 'self'` and the import map's hash.

## 7. The spike (evidence)

**Branch** `spike/preact-h-compat` (commit `8e3d562`, pushed, never to be merged). It vendors `preact.mjs`, `hooks.mjs` and the licence, adds three `Source` rows to `src/web.rs`, an import map to `web/index.html`, and changes `const h` to `let h` in `web/core.js` so the experiment could swap in Preact's `h` from the console. The experiments themselves were run from the console and are described below; there is no committed harness. Not for merge: the Rust tests for the sources would also need `VENDORED.md` and the sums.

**Method.** Preact 11.2.0 from the npm tarball (integrity matched the registry: sha512 `/HfHI/7N...ZPx7xBg==`). A release build of the server from the spike branch, started with `HOME` set to a scratch folder, on port 47931; Chrome, a new tab, no resize. The scanner refused the scratch `HOME` ("not a folder of yours at least two levels below the root"), so the Storage page was fed 240 synthetic items shaped like the test fixtures; Performance used the live sampler of this machine. The real home folder, `~/.monitor-lizard`, `~/.sys-monitor`, `~/.ownwatch` and `~/.Trash` were not touched.

| | Experiment | Result |
|---|---|---|
| a | Attribute shapes through Preact `h`: `class`, `style` string, `onClick`, lowercase `onclick`, `onKeydown`, `onInput`, `onChange`, an unknown `onTabChange`, `aria-*`, `data-*`, `disabled`, `inert` true and false, `readonly`, `hidden:false`, SVG with `path`, `g`, `circle`, `stroke-width`, `vector-effect`, `viewBox` | All correct. `onClick` and `onclick` both fire; `onKeydown` listens for `keydown`; `onTabChange` listens for `TabChange` only. `disabled` and `inert` are set when true and removed when false. SVG children are in the SVG namespace. `e.currentTarget` is the handler's element. No console errors. |
| a2 | Whole-page equivalence: Storage page and each Performance tab built with Preact `h`, rendered, compared node by node with today's DOM (after normalising `style` through `cssText`) | Storage: **one** difference (the capacity chart's `svg` has 0 children, today 2). Performance, Overview: 5 `svg`s empty; CPU and GPU: 2; Disk and network: 3; Memory: 1; Top consumers: none. Every difference is an `svg.innerHTML` write. Nothing threw. |
| b | Focus: focus a Storage tile, re-render | Preact: still focused after 5 re-renders and after a state change (`S.q` set). Control, today's `render()`: focus moved to `body`. |
| c | `decorateTables()` after a Preact render of Storage (hide column `section`, then sort, then change page size) | Column hidden (1 header cell), Copy layer present once (25 buttons, then 10 after the page size change and a second decoration), `floor` and `has-copy` classes set, no "rows do not match" warning. With no second decoration the old Copy layer stays (stale) after rows change. `setHidden('store', [])` restored all columns. Export columns (6) and rows (240) and the CSV header came out right (data-driven). |
| d | Size | `preact.mjs` 11,970 bytes, 4,997 gzip; `hooks.mjs` 3,628 and 1,596 gzip. App scripts today: 259,696 bytes, 82,005 gzip (so core adds 4.6 percent raw, 6.1 percent gzip); `app.css` 21,735; fonts 85,723. `render-to-string` is not used. |
| d | Time, 40 runs each, median (p90), ms | Storage, 240 items, ~1,070 elements: today build only 3.1 (4.5), full rebuild with `decorateTables` 11.4 (14.0); Preact vnode build 1.7 (3.6), first mount 11.9 (14.5), update with the same data 2.0 (2.5), update with a changed page size 2.2 (2.5). Performance, Overview (53 elements): full rebuild 3.5 (10.2) vs update 0.5 (1.0). Performance, Top consumers (534 elements): full rebuild 32.2 (56.5) vs update 0.7 (1.3); vnode build 0.2 (0.5). The p90 values include the live poll running at the same time. |
| e | Loading | `<script type="module">` runs after the classic scripts. A module imported by blob URL (stand-in for `shell.js`) read `TOKEN`, `S` and `nodesOf` and resolved `preact` through the import map. |
| f | Containers | `render(vnode, container)` into a container that already had a child left that child in place, after the new one. |

**What broke.** (1) `svg.innerHTML` on a vnode: silent, 5 sites. (2) Preact's `style` serialisation differs from our text. (3) Nothing else in the 12 page views tried. **What was not tested:** the Trash dialog and the other pages under Preact, the `download()` and Copy-layer sites with vnodes (they require real elements and are in U3), keyboard behaviour in the live app, the Tauri webview, and Safari or Firefox. These are pending, not passed.

**Premises disproved or adjusted.** We expected `decorateTables` or the Node-return contract to be incompatible. Neither is: the contract breaks in five known places and `decorateTables` needs one post-commit call. Lowercase `onclick` works, although the documentation does not say so. The documentation's silence on non-empty containers hid a real hazard (D4).
