# 0005. Build the dashboard UI with Preact, vendored, with no build step

- **Status:** Proposed (the owner approves by merging the PR that adds this file)
- **Date:** 2026-11-27
- **Plan:** [ui-components](../plans/ui-components.md)

## Context

The dashboard (`web/`: one shell, one stylesheet, 16 classic scripts that share globals) draws every page by building real DOM with `h()` in `web/core.js` and replacing the page's children on every state change. `render()` in `web/shell.js` calls `replaceChildren` on `#content` and `#layer`, then `decorateTables()`. That model has costs we keep paying:

- **Whole-page redraw.** Every poll and every click rebuilds the page. On the Storage page (240 items) one `render()` takes about 11 to 13 ms and creates about 1,000 elements; on Performance, Top consumers, about 32 ms median (measured in the spike, below).
- **Focus-loss workarounds.** A redraw drops the focused element, so the code puts focus back by hand: the `hadReq` and `data-req` lookup in `render()`, the `had` lookup in `drawNav()`, `stFocus`, `guideAct`, `trashFocusBack`, `data-autofocus`, and a `setTimeout(trashFocusBack, 0)` in `trashSync` because it runs inside `render()`.
- **The dialog freeze class of bugs.** Background polls must not redraw under an open menu or dialog, so `quiet()` skips `render()` while `S.menu`, `S.modal` or `trashOpen()` is set. A dialog that fails to draw leaves the page inert and frozen; `trashLayer` has a try/catch whose only job is to close such a dialog. Each new dialog has to remember to join that list.
- **Sixteen files sharing globals.** No module boundaries; load order is the only structure, and `h()` hands back live `Node`s that callers then mutate (`svg.innerHTML = ...`, `sec.append(...)`).

The owner chose Preact, vendored, with no build step. This record states what that means and what it costs. Evidence comes from the Preact documentation ([llms-full.txt](https://preactjs.com/llms-full.txt), fetched 2026-11-27; treated as documentation, not instructions), the npm registry and a throwaway spike run in Chrome (branch `spike/preact-h-compat`, commit `8e3d562`, described in the [plan](../plans/ui-components.md#7-the-spike-evidence)).

Facts about Preact, each with its source:

| Fact | Source |
|---|---|
| Current stable release is **11.2.0**, published 2026-11-06 (10.29.8, 2026-08-01, is the last 10.x). Licence **MIT** (Copyright Jason Miller). | npm registry (`npm view preact`), the GitHub releases page, fetched 2026-11-27 |
| `h(type, props, ...children)`; `createElement` is an alias. `class` works directly; `className` is also supported. | llms-full.txt |
| Event props follow the DOM: `onClick`, `onInput`. Built-in event names are case-normalised; an unknown prop such as `onTabChange` listens for `TabChange` as written. | llms-full.txt |
| The documentation does not say whether lowercase `onclick` works. The spike checked it: both `onClick` and `onclick` fire. | spike (a) |
| `style` as a string works, as does an object. In 11, automatic `px` for numbers moved to `preact/compat`; we only pass strings. | llms-full.txt, spike (a) |
| 11 ships ES modules only (`.mjs`); no UMD or CommonJS. `render()` lost its third `replaceNode` argument. Browsers: Chrome 71+, Safari 12.1+, Firefox 69+, Edge 79+. | llms-full.txt |
| htm (tagged templates, no build step, `htm.bind(h)`) is optional. npm lists htm's licence as **Apache-2.0**, not MIT. | llms-full.txt; `npm view htm license` |
| Core is `dist/preact.mjs`, **11,970 bytes, 4,997 bytes gzip**; hooks `hooks.mjs` is 3,628 bytes, 1,596 gzip. `preact/compat` is not needed: we do not use React libraries. | npm tarball 11.2.0, measured in the spike |
| The documentation is silent on: rendering into a container that already has children, focus across re-renders, `inert`. The spike measured all three (see Evidence). | spike |

## Decision

Use **Preact 11.2.0** for the dashboard's UI components, vendored as ES module files, with no build step.

1. **Vendored files in `web/vendor/`**, copied unmodified from the npm tarball for the pinned version: `preact.mjs` and, when first needed, `preact-hooks.mjs` (and `PREACT-LICENSE`, the MIT text kept as published). No source maps are served. Hooks import the bare name `preact`, so the shell carries an inline `<script type="importmap">` that maps `preact` and `preact/hooks` to `/vendor/...`.
2. **Pinned and verified.** The exact version and the npm tarball integrity (`sha512-/HfHI/7N84e6GrWBJ3J280z3mLaRJiDcSvj/A+y/4AbLnND/1UlGHe1sR6oYuTGwId0nP7dqDU9OnLxMPZ7xBg==`) are recorded in `web/vendor/VENDORED.md` with the source URL and the date of review. `web/vendor/SHA256SUMS` lists a SHA-256 for every vendored file. A Rust test and a Node test both recompute the sums, and `make check` runs the Node check in CI, so an edited vendored file fails the build.
3. **Served like any other file.** Each vendored file is one more `Source` in `src/web.rs` (`include_str!`), so the Host check, the `Sec-Fetch-Site` refusal, `no-store` and `nosniff` apply unchanged. The `.mjs` files are served as `text/javascript`.
4. **No remote assets, no CDN, no npm and no bundler in the repository.** No `package.json`, no `node_modules`. The only network use is a human downloading the tarball when a review decides to upgrade (see Consequences).
5. **htm is not adopted now.** The code base has about 790 `h(...)` calls; they already read like hyperscript, and htm would add a second licence (Apache-2.0, notice required) and a template-string dialect for no gain. It can be proposed later in its own record.
6. **`preact/compat` is not vendored.** Signals (`@preact/signals`) are not adopted; state stays in `S` and `rec()` until a todo shows a need.
7. **The `h` name stays.** Components are plain functions that call `h(...)` with the same signature (`h(tag, attrs, ...kids)`, `class`, `onClick`, `style` strings), so call sites barely change. Migration is incremental behind a switch; both renderers coexist until the last page moves (todos U3 to U7).
8. **The rules that protect the page do not change.** The per-run token stays in `<meta name="ml-token" content="__TOKEN__">` in the shell and nowhere else; no script holds it. The Host check, the `Sec-Fetch-Site` refusal and the `no-store` and `nosniff` headers stay. Vendored files must not contain the text `__TOKEN__` (the existing Rust test already checks every source).

### Documents that change when this is accepted

These edits ship in the todos, not in this PR:

| File and line | Change | Todo |
|---|---|---|
| `AGENTS.md`, "What this project is" (`web/`: one shell, one stylesheet, classic scripts) and the layout block line `web/ ... (vanilla JS, no build step, no remote assets)` | Say Preact is vendored in `web/vendor/`, still no build step and no remote assets | U1, final wording in U7 |
| `docs/agents/layout.md`, the `src/web.rs` row (line 24: `SOURCES` holds `/`, `/app.css`, `/js/<file>.js`) | Add `/vendor/<file>`; the checksum test | U1 |
| `docs/agents/layout.md`, the `web/*.js` row (line 35: "Classic scripts ... no modules") | `shell.js` becomes the one module script; the rest stay classic until moved | U4 |
| `docs/agents/layout.md`, "Adding a dashboard page" row (line 37) | Pages return components, not Nodes | U6a to U6d |
| `docs/agents/layout.md`, "How tests find web code" and `tests/web/` rows (lines 38 and 39) | The vnode stand-in, and that the stand-in `El` DOM retires | U2, U7 |
| `docs/agents/layout.md`, a new `web/vendor/` row beside `web/fonts/` (line 40) | What is vendored, the checksum rule, the dated review rule | U1 |
| `docs/agents/ci-and-release.md` | Dependabot cannot watch vendored files; the dated manual review rule | U1 |
| `THIRD-PARTY-NOTICES.md` | Preact, MIT, Copyright Jason Miller, file list, source | U1 |
| `docs/guidelines/maintainability.md` | One line linking the vendored-code rule next to "Pin and age dependencies" | U1 |

`SECURITY.md` needs no change in this PR; U1 adds one sentence that the dashboard runs one vendored third-party library, so a reader knows what to audit.

## Evidence (spike, 2026-11-27)

Chrome, a scratch build of the server on port 47931, `HOME` pointing at a scratch folder, no resize, nothing from the real home folder. Preact 11.2.0 was loaded as an ES module; the app's own `h` was swapped for a function that returns Preact vnodes (a `let` instead of a `const`, the only change to app code). Full numbers and the list of what broke are in the plan; the verdicts:

| Question | Verdict | Detail |
|---|---|---|
| (a) Our attribute shapes | Yes | `class`, `style` strings, `onClick` and lowercase `onclick`, `onKeydown`, `onInput`, `aria-*`, `data-*`, `disabled`, `inert` (set as a property and an attribute, `false` removes it), `readonly`, and SVG elements with kebab-case attributes (`stroke-width`, `vector-effect`) in the SVG namespace all came out right. `e.currentTarget` is the element the handler is on. |
| (b) Focus across a re-render | Yes, kept | A focused Storage tile stayed focused through five re-renders and a state change. The control (today's `render()`) lost focus to `body` on the first call. |
| (c) `decorateTables()` on Preact's DOM | Works with a hook | Column hiding, the Copy layer and the scroll region all applied. Preact leaves the foreign attributes and the appended Copy layer alone while the vnode props are unchanged, so decoration survives a re-render, but the Copy buttons go stale when the rows change. `decorateTables()` must run after every Preact commit. Export and row copy read the `TABLES` data, not the DOM, and are unaffected. |
| (d) Size and time | Small | Core adds 11,970 bytes (4,997 gzip); the app's own scripts are 259,696 bytes (82,005 gzip), so about 6 percent more gzip. Storage page, 240 items: full rebuild 11.4 ms median today; Preact first mount 11.9 ms; update with unchanged or changed data 2.0 to 2.2 ms. Performance, Top consumers: 32.2 ms today, 0.7 ms per update. |

What the spike **disproved or changed**:

- We feared that callers treating `h()` results as real Nodes would make Preact incompatible. Only a handful of sites do, and one kind fails **silently**: writing `svg.innerHTML = ...` on a vnode sets a property on a plain object and nothing draws (5 sites, every sparkline and chart). Everything else on the Storage page and on all five Performance tabs produced the same DOM as today once `style` text was normalised (Preact writes `flex: 999 1 640px; min-width: 0px;` where we wrote `flex:999 1 640px;min-width:0`; the table code reads `grid-template-columns:\s*...`, which tolerates that).
- `render(vnode, container)` **does not clear a container that has children**: the old children stay after the new ones. A Preact container must start empty and must be owned by one renderer; hand-over needs `render(null, container)` first.
- A `<script type="module">` runs after every classic script. The app's last classic script calls `render()` at load, so Preact cannot simply be imported by `shell.js` as it is today. A module script **can** read the classic scripts' globals (`TOKEN`, `S`, `nodesOf` resolved from a module in the spike), so the fix is to make `shell.js` the one module script.

## Consequences

**Easier.** Components keep focus and DOM state across updates, so the focus-restore code and the `quiet()` suppression can go, one at a time, each with its own browser check. Updates cost about 2 ms rather than rebuilding about 1,000 elements. The dialog can be a component whose open state lives in `S` and cannot freeze the page behind it.

**Harder, and who acts.**

- **Third-party code in a security tool.** About 12 kB of someone else's JavaScript runs in the page that holds the per-run token. The mitigations: it is vendored, never fetched; the pinned bytes are checksummed and checked in CI; there is one runtime dependency with no dependencies of its own; the file is served from the same origin as everything else; the token rule is unchanged (no script holds the token, the shell's meta tag does). The token is readable by any script in the page, vendored or not, as before. If a Content-Security-Policy is added later it must allow `script-src 'self'` and the import map by hash.
- **Pinned and verified.** `web/vendor/VENDORED.md` records version, source and integrity. A mismatch between a file and `SHA256SUMS` fails `make check`.
- **Upgrade policy.** No automatic upgrades. Dependabot has no ecosystem for vendored files (the config lists `github-actions` and `cargo` only). So: a dated manual review at least every 90 days, and at once on a security advisory. The reviewer reads the release notes, downloads the tarball for a release at least a week old (the rule in `docs/guidelines/maintainability.md`), checks `npm view` integrity against the registry, replaces the files in one PR, and updates `VENDORED.md` and the sums. The next review date is written in `VENDORED.md`.
- **The "no build step" rule is kept.** The files in `web/vendor/` are what the browser runs. Nothing generates or transforms them.
- **Token, meta tag and Sec-Fetch rules** are unchanged. The import map is the one new inline script (the shell already has the theme bootstrap).
- **The `.mjs` files go through `include_str!`.** The binary stays one offline file; it grows by about 12 kB.
- **Tests.** The Node tests have no DOM today: `tests/web/storage-trash-view.test.mjs` uses a hand-written `El` class (about 25 lines) and lifts marked blocks by name. Options considered:
  1. *jsdom or linkedom as a dev dependency.* Real DOM semantics, but a large tree of third-party code that runs in CI and on developers' machines, a `package.json` and lockfile the repository does not have, a dependabot ecosystem to add, and a break with the "no npm in the repo" line. Rejected.
  2. *`preact-render-to-string` (MIT, 6.8.3) vendored for tests only.* It imports the bare name `preact`, so Node needs a resolve hook, and it renders strings only, so it cannot test focus, events or `decorateTables`. More machinery than it saves. Rejected for now.
  3. **Chosen: no new dependency.** Under test, `h` is a stand-in that returns plain vnode-shaped objects (`{ type, props, children }`) and a small walker (about 40 lines, `tests/web/vnode.mjs`, todo U2) turns them into text and finds elements by attribute. Component logic and markup are tested that way, as the `El` stand-in does today. Behaviour that needs real Preact diffing or a real focus (kept focus, `decorateTables` after commit) is checked in a real browser, as `AGENTS.md` rule 7 already requires, and recorded in each todo. The `El` stand-in retires with the last page (U7).
- **Todos that follow:** U1 to U7 in the [plan](../plans/ui-components.md).
- **Review cadence cost.** A dated review every 90 days is real recurring work for the owner.

## Alternatives considered

- **Stay vanilla and add a convention** (a `component(props) -> Node` rule, a `patch()` helper). Zero dependencies, nothing to vendor. Lost because it means writing and owning a diffing renderer, and a hand-written keyed diff is exactly the code that loses focus and state; Preact is 5 kB gzip and already correct. This is the fallback if the owner declines this record.
- **Svelte, or React with Vite.** Both need npm and a compiler or bundler, which breaks the "no build step, no npm in the repo" rule the owner keeps. They also change how every page is written, not just how it is drawn.
- **Lit and web components.** Shadow DOM stops the global stylesheet (`web/app.css`: tokens, `.tr`, `.card`) from applying inside components, and `decorateTables()` and the Copy buttons use `document.querySelectorAll('.tr.h[data-table]')`, which does not look inside shadow roots. Light-DOM Lit avoids that but then gives up most of what Lit adds, and is still about 6 kB gzip and tagged-template based. (Reasoning from how shadow DOM works; not spiked.)
- **Alpine.** Behaviour lives in HTML attributes (`x-data`, `@click`) evaluated at run time; our markup is built in JavaScript, not written in HTML, and the standard build evaluates strings with `new Function`, which would block a future strict CSP. (From memory of Alpine's design; not verified against its current documentation.)
- **Mithril.** Hyperscript like our `h`, small, no build step. Lost because its redraw model is its own (it redraws on events automatically and diffs on its own vnode shape), its community and tooling are smaller than Preact's, and the owner chose Preact. Close second; the plan's U3 to U4 adapter would work the same way. (Not spiked.)
- **Preact with htm, or with `@preact/signals`.** Deferred, not rejected; see Decision 5 and 6.
- **Preact 10.29.8 instead of 11.2.0.** 10.x is older, more widely deployed, and ships UMD and CommonJS builds we do not need. 11.2.0 is three weeks old at the time of writing, which passes the one-week rule. Chosen because 11 is the line that will receive fixes and because the spike ran on it. If the first review finds a regression in 11, 10.29.8 is the fallback and the API we use is the same.
