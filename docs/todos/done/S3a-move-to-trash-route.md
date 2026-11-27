# S3a: Move to Trash route and audit log

| Field | Value |
|---|---|
| ID | `S3a` |
| Status | `done` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D5, D10 |
| Depends on | S1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/s3a-move-to-trash-route` |
| PR title | `feat(storage): move scanned items to the Trash with an audit entry` |
| Size | M (200-400) |
| PR | [#60](https://github.com/laudebugs/monitor-lizard/pull/60) |

## Goal

Let the server move scanned items to the Trash behind guards, with a dry run and an audit entry. It is the only way the app removes files. The page side is S3b.

## Context

- `POST /api/kill` in `src/server.rs` and `src/actions.rs`: same guards, same audit pattern (`History::record_action`).
- Plan D5: ids only, never paths from the browser.

## Scope

**In scope:**

- `POST /api/trash?ids=a,b` with the Origin check and the per-run token.
- The server re-resolves each id from the latest scan. It then refuses symlinks, paths outside the allowlist or `$HOME`, and items whose size or modified time changed since the scan. It also refuses items labelled `yours` unless the request carries `confirm_yours=1`, sent after a second confirmation.
- A dry-run `POST /api/trash?ids=...&dry=1` that returns what would move, with sizes and the reason for any refusal.
- One audit entry per request in a new `trash_moves` table (time, ids, paths, bytes, refusals), created with `CREATE TABLE IF NOT EXISTS`, so no migration is needed. The existing `actions` table stores only pids and cannot hold paths.
- `/api/history` returns the trash moves so the History page can list them.

**Out of scope** (name the todo that covers it):

- The preview dialog and enabling the page's buttons: S3b.
- Permanent deletion, `docker prune`, `tmutil deletelocalsnapshots`: not planned; the Guide explains them instead.

## Target files

| File | Change |
|---|---|
| `src/storage.rs` | `resolve(ids)`, refusal rules |
| `src/actions.rs` | `trash(paths)` |
| `src/server.rs` | route |
| `src/history.rs` | `trash_moves` table, writer and reader |

## Steps

- [x] Spike first (result in the Implementation Record): compare the `trash` crate (check its licence and version), `NSFileManager trashItem` through FFI, and `osascript` telling Finder to delete. Pick the one that needs no new permission prompt and keeps "Put Back".
- [x] Write the refusal tests first.
- [x] Implement the route and the audit entry.
- [x] Try the full flow on a real folder you created for the purpose.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Id not in the latest scan | 400, nothing moved |
| Path replaced by a symlink after the scan | Refused, reported |
| Item changed after the scan | Refused with "changed since scan, rescan" |
| `yours` item without the second confirmation | Refused |
| Item in use by a running app | Moved or refused with the OS reason; reported either way |
| Missing token or wrong Origin | 403 |
| Trash move fails halfway through a batch | Moved items are reported; the rest are listed as not moved |

## Acceptance criteria

- A moved item appears in the Trash and can be put back.
- Every move is returned by `/api/history` with its path and size.
- No request can move a path the last scan did not list.

## Tests

Rust tests, 10 minimum, using temp directories. Include a test that a crafted id cannot reach `/etc` or `~/Documents`.

## Verification

```sh
make check
make test
```

## Docs to update

`SECURITY.md` (new write path), `docs/agents/`, the README safety paragraph, plan status.

## Risks and open questions

- The Finder route may raise an Automation permission prompt. If so the spike must choose another route.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27.

### Spike: how to move to the Trash

`trash` 5.4.0 (MIT) has two macOS methods. Finder (osascript) keeps "Put Back" but asks for an Automation permission the first time. `NSFileManager` `trashItemAtURL` asks for nothing and, per the crate's own table, may not show "Put Back". Neither has both, and the todo rules out a new prompt, so the route uses `NSFileManager`. `trash` is built with `default-features = false`. The restore path is the Trash folder itself, and the `trash_moves` row keeps each original path. Not checked: whether Finder shows "Put Back" on this Mac. `~/.Trash` cannot be listed from a shell without Full Disk Access, so the real-Trash test (`cargo test --lib real_trash -- --ignored`) shows the file left its place and that a second move of it fails.

### Built

- `POST /api/trash?ids=a,b[&dry=1][&confirm_yours=1]` in `src/server.rs`, guards in `src/storage/trash.rs`, the mover in `src/actions.rs`, the `trash_moves` table and reader in `src/history.rs`, `GET /api/history` returns `trash_moves`.
- `Cache::find` looks ids up in the latest scan; `Item` gained a `modified_ns` that is not sent to the page.
- Tried on a throwaway `HOME` with the server running: no token and a foreign Origin gave 403, an unlisted id and a path given as an id gave 400, a dry run listed both folders and moved nothing, a real request moved them and `/api/history` returned the row.

### Deviations and limits

- The change check also compares exact length and entry count. The first live run showed that a 5-byte append inside one disk block changed neither the allocated size nor the folder's modification time. A same-length overwrite still passes.
- An item whose scan was cut short (entry cap, time, unreadable subfolder) is refused as not fully measured, so a huge `node_modules` over 200,000 entries cannot be moved yet. This is deliberate; the alternative is moving something whose size nobody knows.
- A failed move stops the batch and lists the remaining items as not attempted. A refusal does not stop the batch.
- A real request needs the history database to open; otherwise nothing moves. The reply carries `audit_error` if the row could not be written after the move.
- A folder swapped for a link between the check and the move is a race the check cannot close.
- Item "in use by a running app" is not detected; the OS reason is reported if the move fails.
