# Data folder and files readable by other local users

Resolution: [resolution.md](resolution.md)

## What we saw

`~/.monitor-lizard` was mode 755. `history.db`, `history.db-wal`, `history.db-shm` and `baseline.json` were mode 644. The history database holds the commands and parent chains of every sudo and su launch, and the hosts each app has contacted. Any other user on the Mac could read them.

## How to reproduce

1. Make a scratch home with a `.monitor-lizard` folder at mode 755 and files at mode 644, or let the migration copy one from `.sys-monitor`.
2. Run `HOME=<scratch> monitor-lizard sudo-log`.
3. Run `stat -f '%Lp %N'` on the folder and its files. The old build leaves 755 and 644.

## Measurements

As of November 2026, on a real data folder: the folder was 755 and five files were 644. A fresh folder made by the old build was 700, but a fresh `history.db` in it was 644, so only the folder protected it. The migration copy kept the old modes, because `fs::copy` carries them over.

## Cause

Each newer feature set modes only on what it created itself: the folder, the backup, the lock file and the baseline temporary file. Nothing looked at what already existed. SQLite created `history.db` with the process umask, and the WAL and SHM files copy the mode of the database file. The migration copy used `fs::copy`, which keeps the source mode.

Two places defined "private": the creating code for new files, and nothing at all for old ones.
