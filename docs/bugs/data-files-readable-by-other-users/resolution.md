# Resolution: data folder and files readable by other local users

Detail: [detail.md](detail.md)

## The fix

`paths::data_dir()` is the one place every entry point reads the folder through. Once per process it now makes an existing folder 0700 and each regular file directly inside it 0600, including the WAL and SHM files.

An entry is changed only when the current user owns it, it is not a symlink, and its mode is wider than the target. The mode is set on an opened handle (`O_NOFOLLOW`), never by name, so a link swapped in after the check is refused. Narrower modes are left alone and no file content is touched.

One line on stderr gives the count and the folder. A chmod failure is reported in one line and the program carries on.

New files are owner-only from the start. `history.db` is created at 0600 before SQLite opens it, so the WAL and SHM files inherit that mode. The settings file and its folder are created owner-only. `copy_dir` creates folders 0700 and files 0600 whatever the old modes were.

The first run of this build against a real data folder tightens it. That is expected.

## How it was checked

| Check | Before | After |
|---|---|---|
| Existing 755 folder with 644 files | stays 755 and 644 | 700 and 600, contents unchanged |
| Symlink inside the folder, target outside | not applicable | link and target unchanged |
| Entry owned by another user | not applicable | skipped (predicate test, and a pass run as another uid) |
| Fresh database, WAL and SHM | database 644 | all 600 |
| Migration copy | old modes kept | folder 700, files 600, old folder untouched |
| Already-tight modes | not applicable | no chmod, no log line |
| chmod failure, read-only parent | not applicable | one line, no panic, pass continues |
| Second run | not applicable | no change, no line |

The new tests were run against the old code with no-op stubs for the new functions. Nine fail there: existing folder, second run, predicate, fresh database, settings, migration copy, private file, chmod failure and read-only parent. The symlink, other-owner and already-tight tests pass there because they assert that nothing changes.

## Left for later

Subfolders of the data folder are not tightened; the app creates none. The guide and the README are public text and need no protection.
