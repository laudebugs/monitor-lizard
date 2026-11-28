// Pure logic for the Storage page's disk writes panel: which state to draw, the busiest apps, the hourly trace and
// the lifetime SSD figure. The test lifts the `storage-writes` block out of the dashboard scripts (web/*.js) with the
// `disk-net` (formatBytes) and `page-shell` (plural) blocks it reads.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html } from './page.mjs';
const block = (name) => {
  const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'));
  assert.ok(m, name + ' block not found in the dashboard scripts (web/*.js)');
  return m[1];
};
const names = 'writesState writesRows writesSpark writesSparkLabel writesLifetimeText writesNote writesHasRows WR_TOP WR_H'.split(' ');
const P = new Function(block('disk-net') + block('page-shell') + block('storage-writes') + '\nreturn { ' + names.join(', ') + ' };')();

const H0 = 1_800_000_000 - (1_800_000_000 % 3600);
const hours = Array.from({ length: 24 }, (_, i) => H0 + i * 3600);
const zeros = () => Array(24).fill(0);
const app = (name, total, at = 23) => { const hourly = zeros(); hourly[at] = total; return { app: name, total, hourly }; };
const reply = (extra = {}) => ({ enabled: true, available: true, error: null, hours, apps: [], rows_kept: 0, keep_days: 7, lifetime_written: { value: 2e14 }, ...extra });

test('state: loading, failed, unavailable, off, empty and data', () => {
  assert.equal(P.writesState(null, false), 'loading');
  assert.equal(P.writesState(null, true), 'failed');
  assert.equal(P.writesState(reply({ available: false, error: 'locked' }), false), 'unavailable');
  assert.equal(P.writesState(reply({ enabled: false, apps: [app('Mail', 5)] }), false), 'off', 'off wins even with rows left from before');
  assert.equal(P.writesState(reply(), false), 'empty');
  assert.equal(P.writesState(reply({ apps: [app('Mail', 0)] }), false), 'empty', 'a zero total is not data');
  assert.equal(P.writesState(reply({ apps: [app('Mail', 5)] }), false), 'data');
});

test('every state says what it means; off says how to turn it on', () => {
  for (const s of ['loading', 'failed', 'unavailable', 'off', 'empty', 'data']) assert.ok(P.writesNote(s, reply()).length > 10, s);
  assert.match(P.writesNote('off', reply()), /Turn it on/);
  assert.match(P.writesNote('data', reply({ keep_days: 7 })), /Kept 7 days/);
  assert.match(P.writesNote('unavailable', reply({ error: 'history unavailable: x' })), /history unavailable: x/);
});

test('rows: the busiest first, capped, with a share of the busiest and a count of the rest', () => {
  const apps = Array.from({ length: P.WR_TOP + 3 }, (_, i) => app('App' + i, (20 - i) * 1e6));
  const { rows, more } = P.writesRows(reply({ apps }));
  assert.equal(rows.length, P.WR_TOP);
  assert.equal(more, 3);
  assert.equal(rows[0].share, 100);
  assert.equal(rows[0].text, '20.0 MB');
  assert.ok(rows.every((r) => r.share >= 2), 'a small app still gets a visible bar');
});

test('rows: a malformed entry is skipped, and a bad hourly figure reads as zero', () => {
  const { rows } = P.writesRows(reply({ apps: [{ app: 7, total: 1 }, { app: 'A', total: NaN }, { app: 'B', total: 4, hourly: [1, null, -3, 'x', 3] }] }));
  assert.deepEqual(rows.map((r) => r.app), ['B']);
  assert.deepEqual(rows[0].hourly, [1, 0, 0, 0, 3]);
});

test('spark: scaled to the app\'s own busiest hour, flat at the bottom for zero', () => {
  const d = P.writesSpark([0, 5, 10]);
  const ys = d.slice(1).split(' L').map((p) => +p.split(' ')[1]);
  assert.equal(ys[0], P.WR_H - 1);
  assert.equal(ys[2], 1);
  assert.equal(P.writesSpark([0, 0]).includes('NaN'), false);
  assert.equal(P.writesSpark([5]), '', 'one point is no line');
});

test('spark label names the busiest hour', () => {
  const r = P.writesRows(reply({ apps: [app('Xcode', 3e9, 20)] })).rows[0];
  const label = P.writesSparkLabel(r, hours);
  assert.match(label, /^Xcode: 3\.0 GB in the last 24 hours, most in the hour from /);
  assert.match(label, /\(3\.0 GB\)$/);
});

test('lifetime: a value is shown, anything else is hidden with the reason', () => {
  assert.deepEqual(P.writesLifetimeText({ value: 2.079e14 }), { shown: true, text: '208 TB written to the internal SSD since it was made' });
  const hidden = P.writesLifetimeText({ unavailable: 'no drive on this Mac reports NVMe SMART data' });
  assert.equal(hidden.shown, false);
  assert.match(hidden.text, /hidden: no drive on this Mac reports NVMe SMART data/);
  assert.equal(P.writesLifetimeText(undefined).shown, false);
});

test('erase is offered only when rows are kept', () => {
  assert.equal(P.writesHasRows(reply({ rows_kept: 0 })), false);
  assert.equal(P.writesHasRows(reply({ rows_kept: 3 })), true);
  assert.equal(P.writesHasRows(null), false);
});
