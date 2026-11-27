# B8: Top consumers table and a guarded Stop with fresh evidence

| Field | Value |
|---|---|
| ID | `B8` |
| Status | `done` |
| Plan | [resource-monitoring](../../plans/resource-monitoring.md) |
| Depends on | B3, B4 |
| ADRs | [0002](../../decisions/0002-rust-core-polling-collectors-local-web-ui.md) |
| Branch | `feat/b8-top-consumers-and-guarded-stop` |
| PR title | `feat(web): add the top consumers table and re-check pid and start time on stop` |
| Size | M (200-400) |
| PR | — |

## Goal

Add the Top consumers table (sort by CPU, memory or disk I/O, restricted rows marked) and make Stop re-check pid and start time against a fresh sample, so a recycled pid is never signalled and the dialog shows current evidence.

## Context

- [resource monitoring research](../../research/resource-monitoring/README.md)
- Plan decision D6. Today `src/actions.rs` signals by pid only and cannot detect a recycled pid.
- The confirm dialog in the approved design shows "<name> is using N% CPU right now, from a fresh sample".

## Scope

**In scope:**

- `/api/kill` accepts `start=<unix-seconds>` per pid and refuses when the process's start time differs.
- The table from the design, with the Stop button hidden for root and other users' processes and a "restricted" tag.
- The dialog fetches one fresh row before opening.

**Out of scope** (name the todo that covers it):

- Any automatic action from metrics: never (D6).

## Target files

| File | Change |
|---|---|
| `src/actions.rs` | start-time check |
| `src/server.rs` | accept `start` |
| `web/index.html` | table, dialog evidence |
| `src/procusage.rs` | expose start time |

## Steps

- [x] Write the start-time tests first (pid reused, matching start, missing start).
- [x] Implement and wire the UI.

## Edge cases (must be covered)

| Input | Expected |
|---|---|
| Pid recycled before confirm | Refused with "process changed"; nothing signalled |
| `start` omitted by an old client | Refused |
| Root or other user's process | No Stop button; the server also refuses |
| Process already gone | Row marked stopped; no error |

## Acceptance criteria

- A recycled-pid test passes; the dialog shows fresh evidence.
- UI layout and interaction checked in a real browser against the approved design at 1440 px and below 1100 px (record what was tested, or mark pending)

## Tests

Rust tests for the start-time check (4 minimum); jsdom smoke test for the dialog.

## Verification

```sh
make check
make test
```

## Docs to update

- `docs/agents/layout.md`
- `SECURITY.md` if the write contract changes

## Risks and open questions

- This changes the `/api/kill` contract: update the README and security notes in the same PR.

## Definition of done

- [x] Claimed: first commit moved this file `ready/` -> `ongoing/`
- [x] Follows the applicable guidelines
- [x] `/code-review` run per `AGENTS.md`; findings and fixes in commit messages
- [x] `/writing-review` run on any prose
- [x] `make check` and `make test` green
- [x] Moved `ongoing/` -> `done/` in the last work commit; plan updated; Implementation Record added

## Implementation Record

2026-11-27.

- **Built:** `src/procusage.rs`: every `ProcessUsage` row now carries `start_unix` (unix seconds), computed from `kern.boottime` plus the already-read mach-abstime start converted to nanoseconds; `live_start_unix(pid)` is a separate, always-fresh single-pid read (not the 5 s cache) for the recycled-pid defense. `/api/processes` gained a `pids=` filter (returns exactly those cached rows, ignoring sort/limit) for "fetch specific rows," alongside its existing `sort=&limit=` mode. `src/actions.rs`: `parse` now reads a `start=` comma list paired 1:1 with `pids=`, refusing the whole request if the counts differ (an old client that never sends `start` is refused, not silently downgraded); `check(pid, uid, expected_start)` adds a fresh `live_start_unix` re-read and refuses as `"process changed"` on any mismatch; a sibling `is_signalable(pid, uid)` shares the same ownership check for `classify::allowed_actions`' "can this ever be signalled" query, which has no specific instance's start time to defend. `web/index.html`: `askStop` is now async -- it fetches `/api/processes?pids=` for exactly the pids it was asked about before opening anything, keeps only the ones with a confirmed fresh start time (`confirmable`), and shows a toast instead of a dialog if none survive; `stop()` sends the matching `start=` list back; the dialog gained an evidence line (`evidenceLine`, the exact "is using N% CPU right now, from a fresh sample" wording the design calls for, summed across processes for a multi-pid Quit). A new Performance tab, "Top consumers" (`perfTopConsumersTab_`, `loadProcesses`), lists rows sorted by CPU/memory/disk I/O (chips, reusing the already-built `/api/processes?sort=` backend), a `restricted` tag instead of a Stop button for another user's processes, and a one-extra-render `stopped` tag (`justStoppedRows`) for a row that drops out of a fresh read, so Stop -- or the process exiting on its own -- has something to point to instead of the row just vanishing.
- **Tests:** 2 new Rust tests in `src/procusage.rs` for the boottime/abstime conversion plus `live_start_unix` against a real process and a nonexistent pid (on top of the existing suite); `src/actions.rs` has 6 tests covering parse, the count mismatch refusal, launchd/self, a real spawned child's start time matching/mismatching, and a nonexistent pid -- exceeding the todo's 4-test minimum. `tests/web/actions.test.mjs` (new) covers `confirmable` (kept/dropped/none-confirmable/missing-cpu cases), `evidenceLine` (one process, several, known/unknown CPU), and `justStoppedRows` (marked once, never re-added, nothing marked when all present) -- 12 cases. No `jsdom` package exists anywhere in this repo (checked): every prior "jsdom smoke test" todo (B4, B5, B6) was actually satisfied by this same pure-logic-block-extraction technique, which this follows.
- **Real run:** built the release binary, ran a live `serve`, and drove the new tab through the Claude in Chrome extension. Verified: the table renders real rows sorted by CPU with correct values; restricted rows show the `restricted` tag, not a Stop button; clicking Stop opens the dialog with the real, just-fetched CPU percentage in the evidence line ("/usr/libexec/hybridsearchd 24560 is using 19% CPU right now, from a fresh sample."); Cancel closes without sending anything. Checked at the default window width (**1375px**, above the 1100px breakpoint). Below 1100px was **not** checked this session: the browser-automation resize tool did not change the viewport in this environment (confirmed via `window.innerWidth` staying unchanged after two resize attempts) -- marked pending, not fabricated.
- **Review (2026-11-27):** `/code-review high 5` ran a standards and a spec review in parallel. Standards findings fixed: a sentence in `SECURITY.md` over the docs guideline's 40-word limit (split in two); `actions::check`'s `Option<u64>` "skip the recheck" parameter was a flag parameter mixing two responsibilities -- split into `is_signalable` (no start time, used by `classify::allowed_actions`) and `check` (always requires one, used only where a signal is about to be sent), both built on a shared private `basic_check`. Spec findings fixed: the edge case "Process already gone -> Row marked stopped; no error" was only half true -- a gone process just vanished on the next poll with no "stopped" marking anywhere; added `justStoppedRows` so a row that drops out of a fresh read is shown once more, tagged `stopped`, before it disappears for good. Judgement calls not adopted, with reasons: (1) the Top Consumers table does not join the app's `sortHeader`/`decorateTables`/`TABLES`/export machinery that every other `.tbl` table uses, so it gets no CSV/JSON export or column-hiding floor behavior -- that mechanism assumes a fully-loaded, client-resortable dataset, while this table asks the server for the correct top-50-*by-that-metric* rows (sorting a different, already-limited column client-side would not recover the true top 50 by memory from a CPU-sorted top-50 slice), so adopting it as-is would be the wrong design, not just more work; a dedicated resortable-without-relimiting variant is a real, larger follow-up, not attempted here. (2) `(pid, start)` travelling as a bare tuple (Rust) and parallel `pids`/`starts`/`cpuPct` arrays in the dialog's modal state (JS) were flagged as Data Clumps; left as tuples/arrays given the modest size of this change and that every call site already names what each position means in a doc comment, rather than introduce a new type for three or four call sites.
- **Deviation:** none beyond what Context/Scope already asked for.
- **Open:** below-1100px verification is pending (tooling limitation this session, see Real run). A resortable Top Consumers table that does not need re-limiting per column (the `sortHeader`/`decorateTables`/export integration noted above) is a reasonable follow-up, not this todo's scope.
- **CI:** pending on the pull request.
