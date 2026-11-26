# A14: Notifications for high-severity findings

| Field | Value |
|---|---|
| ID | `A14` |
| Status | `done` |
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

- [x] Write the dedupe and seeding tests first.
- [x] Implement `notify` and verify with a real notification on this Mac; record the macOS permission prompt behaviour and what the notification shows as its source.
- [x] Add the settings endpoint and the dropdown.

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

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose (no new user-facing prose beyond one short notification body and a sidebar dropdown label; not in this skill's scope)
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-26.

- **Built:** `src/notify.rs` (sends via `osascript`, title and body always passed as separate `argv` elements via `on run argv`, never built into the script text -- the whole injection-safety property); `src/settings.rs` (`~/.sys-monitor/settings.json`: `notifications` off/high/all, `generic_text` default true, `quiet_start`/`quiet_end` as minutes since local midnight, `seeded`); a `notified(finding_id, ts)` table in `src/history.rs` with `notified_recently`/`mark_notified`; `notify::dispatch(&findings)` wired into `src/snapshot.rs` right after findings are finalized; a token-guarded `GET`/`POST /api/settings` in `src/server.rs`; a "Notify" dropdown in the dashboard sidebar footer, next to Theme, using the existing custom dropdown component.
- **Naming deviation from the todo's own Target-files table (deliberate):** the todo names the send function `notify` and the dedupe check `seen_before`. Implemented as `send` and `notified_recently` instead: a function named `notify::notify` is awkward to call and read; `seen_before` risked confusion with the unrelated `seen` table this codebase already has for network hosts (A4/A5). Logic matches the spec exactly; only the two internal names differ.
- **Tests:** 75 Rust tests pass (66 before; +9 in `notify.rs`, including a full set exercising `dispatch`'s testable core (`dispatch_with`) end to end: off sends nothing, first-enable seeds every finding (not just ones the current level would notify for) and sends nothing, quiet hours sends nothing and does not mark notified, a fresh finding sends exactly once, the same finding across 100 polls sends exactly once, the same finding 25 hours later sends again, High-only correctly ignores Medium findings, generic text hides the real app name and detail by default, and a failed send is not marked notified so it legitimately retries. `now` and `now_minutes` are both passed into `dispatch_with` rather than computed internally, so every test is fully deterministic regardless of the machine's timezone or the moment it runs -- no reliance on a real clock or a narrow time window. +5 in `settings.rs` for quiet-hour math (same-day, wrapping past midnight, equal-means-unset) and the off-to-on reseed rule. +1 in `history.rs` for the dedupe table itself. An injection-safety test (`untrusted_title_and_body_are_passed_as_literal_argv_elements_not_script_text`) exercises the exact case from the todo's edge-case table against a pure `osascript_args()` function, so it is checked without ever shelling out.
- **Real verification on this Mac:** ran `osascript` directly with the same script and arguments `send` uses -- exit code 0, no error, no visible permission prompt in the terminal (notification permission prompts are a GUI dialog, which a terminal session cannot observe). Then exercised the real end-to-end path through the built binary against an isolated fake-`HOME` (never the real `~/.sys-monitor/`): turned `notifications` on via a real `POST /api/settings` call with a real per-run token, watched the server's own background poll seed all 16 real findings present on this Mac as already-notified with no notification sent, confirmed the `notified` table stayed stable (no re-sends) across a second poll, then deleted one real finding's dedupe row and watched the next poll re-notify it for real -- the `notified` table's timestamp for that finding updated, consistent with a real `osascript` call firing with the default generic text ("Sys Monitor" / "A finding needs your attention..."), not the real app name. Could not visually confirm the notification banner appeared on screen: no desktop-screenshot tool was available in this session, only browser automation. The exit-code and `notified`-table evidence is the same signal `dispatch` itself uses to decide delivery succeeded, so it is taken as sufficient, but a human visually confirming the banner (and what it shows as its source, e.g. attributed to Script Editor or Terminal) is still open -- noted below.
- **Dashboard verification:** no Claude-in-Chrome extension was connected this session, so headless Chrome was driven directly over the DevTools Protocol. Confirmed the Notify dropdown loads the real current server-side setting on page load, and that clicking a different option in it actually changes the real setting on the server (checked via a direct `/api/settings` fetch after the click), not just local UI state. Confirmed `POST /api/settings` without the per-run token returns 403.
- **Review (2026-11-26):** `/code-review high 5` ran a standards and a spec review in parallel. Fixed: the dedupe/seeding/quiet-hours/failure logic inside `dispatch` was correct but entirely untested at the integration level -- only its lower-level building blocks (`notified_recently`, `in_quiet_hours`, the `seeded` reset) had tests, which the todo's "write the dedupe and seeding tests first" step calls for more directly. Refactored `dispatch` into a thin real-IO wrapper around a new `dispatch_with(history, settings, findings, now, now_minutes, sender)`, with `sender` and `now_minutes` both injected, so every edge case in the todo's table now has a direct, deterministic test that sends zero real notifications during `cargo test`. Also added a doc comment on `finding_id` explaining what "the same finding" means for dedupe and why a collision is not realistically possible, which review flagged as a decision with real consequences that had no comment explaining it. Not changed, with reasons: a `Settings`-fields-as-one-struct refactor of `settings::update`'s four `Option<&str>` parameters was flagged as a mild Data Clump, but it mirrors the codebase's existing stringly-typed convention (`actions::parse`), so kept; a more granular per-FFI-call SAFETY comment in `local_minutes_since_midnight` was flagged as less granular than `sampler.rs`'s style, but the single comment already covers the one invariant that matters (the zeroed starting value), so kept.
- **Deviation:** the two internal function/method names above. Also a process deviation worth naming: the claim commit (moving ready/ -> ongoing/) ended up bundled with the full first-draft implementation instead of being its own commit first, unlike every other todo this session -- a sequencing slip, not a functional issue.
- **Open:** a human has not yet visually confirmed what the real notification banner looks like on screen or what source macOS attributes it to (Script Editor, Terminal, or `sys-monitor` itself) -- the Risks section's own open question. The exit-code and dedupe-table evidence strongly suggest delivery succeeded, but this specific visual confirmation needs a person at the keyboard or a session with desktop-screenshot access.
- **CI:** pending on the pull request.
