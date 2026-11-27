# S3a: Move to Trash route and audit log

| Field | Value |
|---|---|
| ID | `S3a` |
| Status | `ongoing` |
| Plan | [storage-and-guide](../../plans/storage-and-guide.md), D5, D10 |
| Depends on | S1 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/s3a-move-to-trash-route` |
| PR title | `feat(storage): move scanned items to the Trash with an audit entry` |
| Size | M (200-400) |
| PR | — |

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

- [ ] Spike first (record the result here): compare the `trash` crate (check its licence and version), `NSFileManager trashItem` through FFI, and `osascript` telling Finder to delete. Pick the one that needs no new permission prompt and keeps "Put Back".
- [ ] Write the refusal tests first.
- [ ] Implement the route and the audit entry.
- [ ] Try the full flow on a real folder you created for the purpose.

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

- [ ] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [ ] Follows the applicable guidelines
- [ ] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [ ] `/writing-review` run on any prose
- [ ] `make check` and `make test` green
- [ ] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added
