# Resolution: `POST /api/trash` moved files on a typo and wrote its audit row after the move

Detail: [detail.md](detail.md)

## The fix

The contract is plan D15. In short: only `dry=0` moves, and only with a confirmation token from a preview of the same ids, labels and scan; one audit row per item is written as `pending` before anything moves, set to `moving` just before each move, and given its result after; a failed write stops the item and the rest. The route runs on its own thread with a 45 s budget and one request at a time; the path checks repeat right before the mover; `HOME` must be the user's own folder two levels down; items must be owned by the user and contain no mounted volume. The mover error is a typed enum, the audit keeps 10,000 rows, `/api/history` shows `~` paths, and the reply gains `ok`, `history_id`, per-item `status` and `code`, `counts` and a Put Back note.

## How it was checked

Regression tests that cannot pass on the old code (they use the new API and the old code lacked the behaviour): `only_dry_0_moves_and_every_other_spelling_is_a_preview_or_a_400`, `an_audit_row_that_cannot_be_inserted_moves_nothing_and_is_a_500`, `an_audit_row_that_cannot_be_updated_before_a_move_stops_that_item_and_the_rest`, `a_crash_mid_batch_leaves_a_row_for_every_item`, `the_audit_rows_exist_before_each_move_and_say_moving`, `a_path_swapped_for_a_symlink_between_the_check_and_the_move_is_refused`, `an_item_owned_by_someone_else_is_refused_up_front_with_a_reason`, `an_item_with_a_volume_mounted_inside_it_is_refused` (a real disk image attached in a temp folder), `a_path_in_a_different_letter_case_is_refused`, and the token tests. The default tests use a scratch home, database and a fake mover; none touches the real Trash.

| Check | Before | After |
|---|---|---|
| `dry=true` | moves | 400 `bad_dry` |
| Audit row | after the move | before each move, one per item |
| Locked audit table | items moved, row lost | 500, nothing moved |
| Real move without a preview | allowed | 400 `confirmation_required` |
| Worst-case request time | about 500 s on the request loop | 45 s on its own thread |

The real Trash was exercised once with the ignored test `real_trash_takes_a_file_and_refuses_a_missing_one`: it passed, so the mover returned success and the throwaway file left its place. Whether Finder offers "Put Back" for it could not be checked: `~/.Trash` is unreadable from the shell without Full Disk Access.

## Left for later

- A file overwritten with the same length passes the change check; the Trash is the backstop.
- A swap in the instant between the second check and the move cannot be closed.
- `GET /api/storage` still returns full paths without a token (it is read-only and Host-checked; see SECURITY.md).
- S3b: the page must use the preview, token and `ok`/`status`/`code` fields.
