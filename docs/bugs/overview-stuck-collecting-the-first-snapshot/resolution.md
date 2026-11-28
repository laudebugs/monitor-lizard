# Resolution: the dashboard sticks on "Collecting the first snapshot..." until reloaded

Detail: [detail.md](detail.md)

## The fix

`load()` in `web/data.js` (new `snapshot-load` marked block) now tells the two cases apart: a `null` reply retries in 500 ms (`SNAPSHOT_RETRY_MS`) instead of waiting for the next 10 s poll, and stops retrying as soon as a real snapshot arrives. A `null` reply also no longer sets the "live, refreshed ..." text, which was misleading when there was nothing live yet. No server-side change: `cached()` still returns `null` immediately rather than blocking the request thread, which is what keeps the rest of the page (CSS, JS, other API calls) loading promptly while the first snapshot is still being collected.

## How it was checked

| Check | Before | After |
|---|---|---|
| `tests/web/snapshot-load.test.mjs`, a `null` reply | block does not exist yet (test fails to find it) | passes: one retry scheduled at 500 ms, not 10000 ms |
| Same test, a real snapshot | not applicable | passes: no retry scheduled, "live, refreshed ..." set |
| Same test, `null` then a real snapshot | not applicable | passes: the scheduled retry runs once, gets real data, and schedules no further retry |
| Fresh `HOME`, Tauri shell, cold launch, timed by hand | "Collecting the first snapshot..." stays up for up to 10 s | the real snapshot renders within about 1-2 s, matching the measured server-side timing in detail.md |
| Fresh `HOME`, `monitor-lizard serve` in a browser tab, cold launch | same stuck message | same quick recovery |
| `node --test tests/web/*.test.mjs` | -- | 367 tests, all pass |
| `cargo test --locked` | -- | unaffected (no Rust change); still green |

Regression test: `tests/web/snapshot-load.test.mjs`.

## Left for later

The same `null`-means-"not ready yet" gap exists for `/api/privilege` and `/api/persistence` (`loadPriv`, `loadPers` in `web/data.js`), each with its own slower poll interval (30 s and 60 s). Neither blocks the Overview page like the snapshot did and neither was reported as a symptom, so this fix is scoped to the one the owner actually hit; the same retry pattern would apply if one of those is ever reported stuck the same way.
