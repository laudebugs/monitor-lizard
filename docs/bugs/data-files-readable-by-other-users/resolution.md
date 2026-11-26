# Resolution: data folder and files readable by other local users

Detail: [detail.md](detail.md)

## The fix

`paths::data_dir()` is the one place every entry point reads the folder through. It creates a missing folder 0700. The first time a process calls it, it makes an existing folder 0700 and each regular file directly inside it 0600, including the WAL and SHM files. No other module creates the folder.

An entry is changed only when the current user owns it and its mode is wider than the target. Setuid, setgid and sticky count as wider. Symlinks, FIFOs, sockets and files with more than one hard link are skipped. A hard link could reach a file outside the folder.

The mode is set on an opened handle. The open uses `O_NOFOLLOW` and `O_NONBLOCK`, so a swapped-in symlink is refused and a FIFO cannot hang it. The handle's device and inode must match the entry that was checked. No file content is touched.

Output is one line for a change, with the count of files and the folder. A second line covers what was not done: chmod failures, linked files, and entries owned by another user. The owner case names the folder and says `sudo chown -R $USER <folder>`.

New files are owner-only from the start through shared helpers in `paths.rs`. `history.db` is created 0600 before SQLite opens it, so the WAL and SHM files inherit that mode. `copy_dir` creates folders 0700 and files 0600, and copies only regular files.

The first run of this build against a real data folder tightens it. That is expected.

## How it was checked

| Check | Before | After |
|---|---|---|
| Existing 755 folder with 644 files | stays 755 and 644 | 700 and 600, contents unchanged |
| Symlink inside the folder, target outside | not applicable | link and target unchanged |
| Symlink swapped in between the check and the open | not applicable | target outside unchanged |
| Hard link to a file outside, FIFO, socket, setuid file | not applicable | outside file unchanged; FIFO and socket skipped; setuid bit cleared |
| Symlinked data folder | not applicable | not listed, not changed, one line says so |
| Entry owned by another user | not applicable | skipped and reported with the folder and a chown hint |
| Fresh machine, first call from any module | folder 755 when `approve` ran first | folder 700 |
| Fresh database, WAL and SHM | database 644 | all 600 |
| Migration copy | old modes kept | folder 700, files 600, old folder untouched |
| Already-tight modes | not applicable | no chmod, no log line |
| chmod failure, read-only parent | not applicable | one line, no panic, pass continues |
| Second run | not applicable | no change, no line |

The first set of new tests was run against the old code with no-op stubs, and nine failed there. The inode re-check also stops a swapped symlink, so removing `O_NOFOLLOW` fails only the test that opens a symlink directly. A throwaway worktree confirmed that.

## Left for later

Subfolders of the data folder are not tightened; the app creates none. The guide and the README are public text and need no protection.
