# U8: Declare the minimum macOS version in the Tauri shell

| Field | Value |
|---|---|
| ID | `U8` |
| Status | `ready` |
| Plan | [ui-components](../../plans/ui-components.md), D1 |
| Depends on | None |
| ADRs | [0005](../../decisions/0005-preact-vendored-no-build-step.md) |
| Branch | `chore/u8-minimum-macos` |
| PR title | `chore(shell): declare the minimum macOS version the dashboard needs` |
| Size | S (under 200) |
| PR | — |

## Goal

Set `minimumSystemVersion` in `src-tauri/tauri.conf.json` and say the floor in the README, so the dashboard never loads in a WebKit that lacks what `web/app.css` and the scripts already use. This is independent of Preact (it changes no web file) and belongs to the menu-bar shell.

## Context

- ADR 0005, "Platform floor": the page needs `:has()` (Safari 15.4), `inert` (15.5) and `color-mix()` (16.2) from MDNs browser-compat-data; Tauris default `minimumSystemVersion` is `10.13`; Apple lists Safari 16.2 for macOS Big Sur and Monterey, and Ventura ships Safari 16.
- The shell is owned by Amber; ask them to run the check below.

## Scope

**In scope:**

- Choose the floor with the owner (a reasonable candidate is macOS 13, Ventura); set `bundle.macOS.minimumSystemVersion`; state it in `README.md`; check that a Mac below it is refused by the bundle, if one is available.

**Out of scope** (name the todo that covers it):

- Any web change (U1 to U7).

## Target files

| File | Change |
|---|---|
| `src-tauri/tauri.conf.json` | `minimumSystemVersion` |
| `README.md` | the floor in one sentence |

## Steps

- [ ] Ask the owner for the floor; record the answer in the PR.
- [ ] Build the bundle and read `LSMinimumSystemVersion` from its `Info.plist`.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Floor higher than the build machine | the build still succeeds; say what was tested |
| `tauri dev` | the setting is ignored (documented by Tauri); not a failure |

## Acceptance criteria

- The bundle's `Info.plist` carries the chosen version.
- The README states it.

## Tests

No code; check the `Info.plist` value.

## Verification

Run `make check` and `make test` (the standard pair), plus:

```sh
make check
```

## Docs to update

`README.md`.

## Risks and open questions

- Whether a Tauri window on Big Sur or Monterey gets the updated WebKit from a Safari update is not verified; the floor the owner picks should be one that was run.

## Rollback and how the dashboard keeps working

- **Rollback:** Revert the PR.
- **Why the dashboard works at every step:** No dashboard file changes.

## Definition of done

The standard list in [TEMPLATE.md](../TEMPLATE.md#definition-of-done), plus a browser check recorded for every UI change.
