# trash-moved-files-without-audit-or-on-a-typo

`POST /api/trash` moved real files for `dry=true` or a typo, and wrote its audit row only after the move. Fixed by the removal contract in plan D15.

| File | What it holds |
|---|---|
| [detail.md](detail.md) | What was seen, how to reproduce, the cause |
| [resolution.md](resolution.md) | The fix and how it was checked |
