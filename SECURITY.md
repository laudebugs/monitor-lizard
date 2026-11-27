# Security

Report vulnerabilities through GitHub's private vulnerability reporting (Security tab, "Report a vulnerability"). Do not open a public issue.

You can expect an acknowledgement within a few days. Say in the report what is affected and how to reproduce it.

## In scope

system-monitor can stop processes on your Mac, so these matter most:

- A way for a web page or another local process to stop a process, approve an app, or read data without the per-run token (CSRF, DNS rebinding, token leakage).
- A way to make `POST /api/trash` move anything the latest scan did not list, a symbolic link, or a path outside your home folder and `/Applications`, or to skip the second confirmation for an item labelled yours.
- A way to make it signal processes it refuses: root-owned, launchd, itself, or another user's.
- The server listening on anything other than 127.0.0.1.
- Injection through process names, command lines, or other attacker-controlled strings shown in the dashboard.

## Design guarantees

Loopback only, Host and Origin checks on every request, a random per-run token on every write, own-user processes only, an audit log of every action, no telemetry. Tests need no credentials and make no network calls.

`POST /api/kill` requires a `start=<unix-seconds>` for every pid in `pids=`, read moments earlier from `GET /api/processes`. Before signalling, the server re-reads that pid's start time fresh and refuses if it no longer matches, so a pid recycled by a new, unrelated process between the read and the click is never signalled. A request missing `start`, or with a different count of `start` values than `pids`, is refused outright rather than falling back to the old pid-only check.

## What `/api/storage` exposes

`POST /api/trash` is the one way the app removes files, and it only moves them to your Trash. Finder's "Put Back" may not be offered for those items; History keeps the original path of each one, so you can move it back by hand. The route needs the Origin check and the per-run token, and it takes ids from the latest scan, never paths. Only `dry=0` moves anything: no `dry` or `dry=1` is a preview, and any other value is rejected. A preview returns a confirmation that is good for ten minutes and one use, kept in memory only, and tied to those exact items, their labels and that scan; a real move must send it back. The server checks each item again against the disk, and once more just before the move: not a symbolic link, no symbolic link above it, inside your home folder or `/Applications`, owned by you (a root-owned app is refused with a reason), no other volume mounted inside it, size, entry count and modification time unchanged since the scan, and fully measured. `HOME` must be a folder of yours at least two levels below the root. An item labelled yours also needs `confirm_yours=1`. A request writes an audit row for each item to the history database before anything moves, and if that cannot be written nothing moves. The history database keeps the newest 10,000 such rows. `GET /api/history` needs no token, so it shows those paths with your home folder written as `~`. Limits: a file overwritten with the same length passes the change check, and a folder swapped for a link in the instant between the second check and the move is a race no check can close. The Trash keeps the item either way. One removal runs at a time and a request has a 45 second budget.

`GET /api/storage` lists the names of app caches, the paths of developer projects that have a `node_modules` folder, and how big each is. It needs no token, only the Host check. The Host check and the browser's cross-origin rules stop a web page you open from reading the reply. They do not stop that page from sending the request, so a page could ask for a new scan. To limit that, a request starts a scan at most once a minute. Any process on your Mac can read the reply, because it can reach `127.0.0.1`. Cloud folders such as `~/Dropbox` are not user-content folders, so an idle `node_modules` inside one can be labelled safe.

## What the baseline means

Marking an item as expected hides it from the Overview and the New filters. It does not make the item safe.

- A mark never raises a trust label. The rules still see every finding, so an app keeps the label it had before the mark.
- A mark on open ports covers exactly the ports you saw. A new port brings the finding back, and you mark it again.
- New hosts are marked host by host, so a new host is reported even after others were marked.
- An unsigned program using the network, and an open remote-debugging port, can never be marked. The server refuses the request with a 400.
- Marked findings stay listed in a "Marked as expected" section on the Overview, with an Unmark button. A marked host keeps its row with an "expected" chip.
- A mark on a startup item covers the version you saw: it records the item's fingerprint (its plist and its program's signing identity). If the plist or the program changes, the item is no longer expected, is reported again as changed, and shows as needing attention. An entry that is only a path, written by hand, matches every version and is not recommended.
- Every mark and unmark is written to the history database before it takes effect, and the History page lists them. If that write fails, nothing is changed. If the baseline file cannot then be written, a second row says so.
- The baseline holds at most 5,000 entries and the history keeps the newest 10,000 changes.
- A corrupt `baseline.json` is moved aside without overwriting older copies, and the Overview shows a banner saying earlier marks are not in effect. An unreadable one pauses marking until it can be read.
- Anyone who can write `~/.monitor-lizard/baseline.json` can hide things from you. That is the same access that is out of scope below.

## What is protected on disk

The data folder `~/.monitor-lizard` is meant to be mode 0700, and each file the app keeps in it 0600: the history database and its WAL and SHM files, the lock file, the backup, `baseline.json` and `settings.json`. New ones are created that way. Other local users then cannot read the sudo and su launch history.

The first time a process reads the data folder location, it also tightens an existing folder and its files. That covers folders made by an older version or copied from `~/.sys-monitor` or `~/.ownwatch`. It prints one line for a change and one for anything it could not do.

The pass changes a mode only when you own the entry and its mode is wider than the target. It skips symlinks, special files, and files with more than one hard link, and it counts them in the report. It never changes file contents.

## What is not covered

- Root and admin users can always read the folder.
- Earlier copies, such as Time Machine and backups made before the fix, keep their old modes.
- An ACL or the immutable flag can still grant access. The pass does not remove them.
- The mode of your home folder is not changed.
- A folder or file owned by someone else, for example after running with sudo, is not changed. The report names the folder and says how to fix it.
- The older folders `~/.sys-monitor` and `~/.ownwatch` are never changed.
- Files outside this folder are not covered. The in-app guide and the project files are public text, not secrets.
- If a mode change fails, the app says so once and carries on.

## Out of scope

Findings that need local root, or that assume the attacker already runs code as you and can read `~/.monitor-lizard/` (or an older `~/.sys-monitor/` or `~/.ownwatch/`, kept for migration).
