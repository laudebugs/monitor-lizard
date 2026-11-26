# A14: Notifications for high-severity findings

| Field | Value |
|---|---|
| ID | `A14` |
| Status | `ready` |
| Plan | [system-monitor](../../plans/system-monitor.md) |
| Depends on | None |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/a14-notifications-for-high-findings` |
| PR title | `feat(notify): send deduplicated macOS notifications for high findings` |
| Size | M (200-400) |
| PR | — |

## Goal

Tell the owner when something high-severity appears, without a window open: a native macOS notification, off by default, generic text by default, each finding at most once per 24 hours.

## Context

- Plan: "notifications for high findings only" (system-monitor plan, phase 8 notes).
- Status as of 2026-11-26: not researched. `osascript display notification` needs no bundle or entitlement and shows under the Script Editor name without actions; native actions and click-through need a signed app bundle (the later menu-bar todo). Both claims are from general knowledge and unverified here.

## Scope

**In scope:**

- `src/notify.rs`: `notify(title, body)` via `osascript` with the text passed as arguments (`on run argv`), never interpolated into the script.
- Settings in `~/.sys-monitor/settings.json`: `notifications` = off | high | all (default off), quiet hours, `generic_text` (default true).
- Dedupe table `notified(finding_id, ts)` in the history database; findings present when notifications are first enabled are seeded as seen.
- A Settings control in the dashboard using the existing custom dropdown.

**Out of scope** (name the todo that covers it):

- Click-through and action buttons: need the menu-bar app (not yet written).
- Web Notification API in the browser tab: out of scope.

## Target files

| File | Change |
|---|---|
| `src/notify.rs` | (new) |
| `src/history.rs` | `notified` table and `seen_before`, `mark_notified` |
| `src/snapshot.rs` | call `notify::dispatch(&findings)` after rules run |
| `src/server.rs` | `GET/POST /api/settings` (token-guarded write) |
| `web/index.html` | Settings dropdown in the sidebar footer |

## Steps

- [ ] Write the dedupe and seeding tests first.
- [ ] Implement `notify` and verify with a real notification on this Mac; record the macOS permission prompt behaviour and what the notification shows as its source.
- [ ] Add the settings endpoint and the dropdown.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Process name contains `"` or `\` or AppleScript syntax | Delivered as literal text; no script injection (test with a name like `"; do shell script "x`) |
| Notifications first enabled while 10 high findings exist | None sent; all seeded as seen |
| Same finding on the next 100 polls | One notification |
| Same finding 25 hours later | Notified again |
| Quiet hours active | Nothing sent; not marked notified, so it is sent when quiet hours end if still present |
| `osascript` fails or permission denied | Logged once to stderr; the app keeps running |

## Acceptance criteria

- With `high` selected, a new unsigned networked app produces exactly one notification with generic text.
- No setting change is possible without the per-run token.

## Tests

Rust tests for dedupe, seeding, quiet hours and argument passing (5 minimum); a manual check on a real Mac.

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`
- Plan section 2

## Risks and open questions

- macOS may ask for permission the first time and may attribute the notification to Script Editor.
- Notification text can appear on the lock screen and in screen sharing: keep it generic by default.

## Definition of done

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
