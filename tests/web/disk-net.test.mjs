// Pure logic for the Disk and network tab and the shared capacity bar. The test lifts the `disk-net` block out of
// web/index.html, so the block must stay free of DOM and storage access.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../../web/index.html', import.meta.url), 'utf8');
const m = html.match(/\/\* disk-net:begin \*\/([\s\S]*?)\/\* disk-net:end \*\//);
assert.ok(m, 'disk-net block not found in web/index.html');
const { capacitySegments, formatBytes, formatRate, formatOps, networkState, rateChartMax } =
  new Function(m[1] + '\nreturn { capacitySegments, formatBytes, formatRate, formatOps, networkState, rateChartMax };')();

const GB = 1e9;
test('used, purgeable and free segments add up to the whole bar', () => {
  const r = capacitySegments({ used: 300 * GB, purgeable: 100 * GB, free: 600 * GB, unavailable: null });
  assert.deepEqual(r.segments.map((s) => s.key), ['used', 'purgeable', 'free']);
  assert.deepEqual(r.segments.map((s) => s.pct), [30, 10, 60]);
  assert.equal(r.total, 1000 * GB);
  assert.equal(r.note, null);
});

test('purgeable unavailable: the bar has used and free only, with a note that says why', () => {
  const r = capacitySegments({ used: 400 * GB, purgeable: null, free: 600 * GB, unavailable: 'volume capacity keys unreadable' });
  assert.deepEqual(r.segments.map((s) => s.key), ['used', 'free']);
  assert.deepEqual(r.segments.map((s) => s.pct), [40, 60]);
  assert.match(r.note, /used and free only/);
  assert.match(r.note, /volume capacity keys unreadable/);
});

test('a purgeable value of zero is a real value and stays as a segment', () => {
  const r = capacitySegments({ used: 500, purgeable: 0, free: 500, unavailable: null });
  assert.deepEqual(r.segments.map((s) => s.key), ['used', 'purgeable', 'free']);
  assert.equal(r.note, null);
});

test('no used or free reading gives no bar and the reason, never an empty full bar', () => {
  for (const args of [{ used: null, purgeable: null, free: null, unavailable: 'statfs failed' }, { used: 5, purgeable: 1, free: null, unavailable: 'statfs failed' }]) {
    const r = capacitySegments(args);
    assert.deepEqual(r.segments, []);
    assert.equal(r.total, null);
    assert.equal(r.note, 'statfs failed');
  }
  assert.equal(capacitySegments({ used: null, free: null }).note, 'not read yet');
});

test('an empty disk total does not divide by zero', () => {
  assert.deepEqual(capacitySegments({ used: 0, purgeable: 0, free: 0, unavailable: null }).segments.map((s) => s.pct), [0, 0, 0]);
});

test('rates use decimal units and never turn an unavailable value into 0', () => {
  assert.equal(formatRate(0), '0 B/s');
  assert.equal(formatRate(1500), '1.5 KB/s');
  assert.equal(formatRate(12_340_000), '12.3 MB/s');
  assert.equal(formatRate(250_000_000), '250 MB/s');
  assert.equal(formatRate(null), 'unavailable');
  assert.equal(formatRate(undefined), 'unavailable');
  assert.equal(formatRate(NaN), 'unavailable');
  assert.equal(formatBytes(null), 'unavailable');
  assert.equal(formatBytes(994 * GB), '994 GB');
  assert.equal(formatOps(1234.6), '1,235 ops/s');
  assert.equal(formatOps(null), 'unavailable');
});

test('no interface up reads as no connection, not as an idle zero', () => {
  assert.deepEqual(networkState({ interfacesUp: 0, down: 0, up: 0 }), { kind: 'no-connection', text: 'No connection' });
  assert.equal(networkState({ interfacesUp: 0, down: null, up: null }).kind, 'no-connection');
});

test('an unreadable interface list is unavailable, and a connected one with real rates is ok', () => {
  assert.equal(networkState({ interfacesUp: null, down: null, up: null }).kind, 'unavailable');
  assert.equal(networkState({ interfacesUp: 2, down: 0, up: 0 }).kind, 'ok');
  assert.equal(networkState({ interfacesUp: null, down: 10, up: null }).kind, 'ok');
});

test('the chart top is the largest value in view but never below the floor', () => {
  assert.equal(rateChartMax([[1, null, 5], [2]], 10), 10);
  assert.equal(rateChartMax([[1, null, 50], [20]], 10), 50);
  assert.equal(rateChartMax([[null], []], 7), 7);
});
