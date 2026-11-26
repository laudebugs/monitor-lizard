# Security

Report vulnerabilities through GitHub's private vulnerability reporting (Security tab, "Report a vulnerability"). Do not open a public issue.

You can expect an acknowledgement within a few days. Say in the report what is affected and how to reproduce it.

## In scope

system-monitor can stop processes on your Mac, so these matter most:

- A way for a web page or another local process to stop a process, approve an app, or read data without the per-run token (CSRF, DNS rebinding, token leakage).
- A way to make it signal processes it refuses: root-owned, launchd, itself, or another user's.
- The server listening on anything other than 127.0.0.1.
- Injection through process names, command lines, or other attacker-controlled strings shown in the dashboard.

## Design guarantees

Loopback only, Host and Origin checks on every request, a random per-run token on every write, own-user processes only, an audit log of every action, no telemetry. Tests need no credentials and make no network calls.

## What the baseline means

Marking an item as expected hides it from the Overview and the New filters. It does not make the item safe, and it cannot be undone by anything except you.

- An app, remote host, startup item or single finding you mark stays in its table with an "expected" chip, and "Unmark" restores it.
- Every mark and unmark is written to the history database before it takes effect, and the History page lists them. If that write fails, nothing is changed.
- An unsigned program that uses the network cannot be marked as expected: it stays flagged and keeps its Suspicious label.
- Anyone who can write `~/.monitor-lizard/baseline.json` can hide things from you. That is the same access that is out of scope below.

## Out of scope

Findings that need local root, or that assume the attacker already runs code as you and can read `~/.monitor-lizard/` (or an older `~/.sys-monitor/` or `~/.ownwatch/`, kept for migration).
