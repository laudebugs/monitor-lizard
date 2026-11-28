// A null /api/snapshot reply means "the server's first collect+codesign pass is still running",
// not "poll again in 10 s like everything else": see docs/bugs/overview-stuck-collecting-the-first-snapshot/.
// This lifts the real load() out of web/data.js (no browser needed) and fakes fetch, the DOM and
// setTimeout so the retry can be observed without waiting on either a server or a real timer.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html } from './page.mjs';
const m = html.match(/\/\* snapshot-load:begin \*\/([\s\S]*?)\/\* snapshot-load:end \*\//);
assert.ok(m, 'snapshot-load block not found in the dashboard scripts (web/*.js)');

// One load() wired to fakes: `responses` is what /api/snapshot returns on each successive call
// (the last entry repeats once exhausted, so a test can call load() more times than it listed).
function makeLoad(responses) {
  let i = 0;
  const live = { textContent: '' };
  const scheduled = [];
  const api = async () => ({ json: async () => responses[Math.min(i++, responses.length - 1)] });
  const quiet = () => {};
  const selPatch = () => null;
  const $ = (id) => (id === 'live' ? live : null);
  const fakeSetTimeout = (fn, ms) => { scheduled.push({ fn, ms }); return scheduled.length; };
  const factory = new Function('api', 'quiet', 'selPatch', 'S', '$', 'setTimeout', m[1] + '\nreturn { load, SNAPSHOT_RETRY_MS };');
  const { load, SNAPSHOT_RETRY_MS } = factory(api, quiet, selPatch, {}, $, fakeSetTimeout);
  return { load, SNAPSHOT_RETRY_MS, live, scheduled };
}

test('snapshot-load: a null reply schedules a quick retry, not the 10 s poll', async () => {
  const { load, SNAPSHOT_RETRY_MS, live, scheduled } = makeLoad([null]);
  await load();
  assert.equal(scheduled.length, 1, 'exactly one retry scheduled');
  assert.ok(scheduled[0].ms < 10000, 'retry is sooner than the 10 s poll interval: ' + scheduled[0].ms);
  assert.equal(scheduled[0].ms, SNAPSHOT_RETRY_MS);
  assert.equal(live.textContent, '', 'a null reply must not be announced as "live, refreshed"');
});

test('snapshot-load: a real snapshot does not retry and announces itself as live', async () => {
  const { load, live, scheduled } = makeLoad([{ groups: [], findings: [] }]);
  await load();
  assert.equal(scheduled.length, 0, 'no retry once real data arrives');
  assert.match(live.textContent, /^live, refreshed /);
});

test('snapshot-load: the retry self-terminates as soon as a real snapshot is available', async () => {
  const { load, live, scheduled } = makeLoad([null, { groups: [], findings: [] }]);
  await load();
  assert.equal(scheduled.length, 1);
  await scheduled[0].fn(); // run the retry load() scheduled() captured, as the real timer would
  assert.equal(scheduled.length, 1, 'the retry does not schedule another retry once data is in');
  assert.match(live.textContent, /^live, refreshed /);
});
