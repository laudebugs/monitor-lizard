// Pure logic for the Overview: the last-visit window, the feed and the Machine strip. The test lifts the `overview-page` block.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { html } from './page.mjs';
const m = html.match(/\/\* overview-page:begin \*\/([\s\S]*?)\/\* overview-page:end \*\//);
assert.ok(m, 'overview-page block not found in the dashboard scripts (web/*.js)');
const P = new Function(m[1] + '\nreturn { lookedSince, feedWindowText, feedRows, machineRows, feedWhen, cpuAverage, bySeverity, LOOK_FALLBACK_SECS };')();
const NOW = 2_000_000_000, DAY = 86400;
const label = (k) => k.toUpperCase();

test('first visit covers the last 24 hours and says so', () => {
  for (const stored of [null, '', 'abc', '0', '-5']) {
    const l = P.lookedSince(stored, NOW);
    assert.equal(l.first, true);
    assert.equal(l.since, NOW - DAY);
  }
  assert.match(P.feedWindowText({ first: true }), /No earlier visit.*24 hours/);
});
test('a stored visit is used, a future one is ignored, an old one is cut to 30 days', () => {
  assert.deepEqual(P.lookedSince(String(NOW - 100), NOW), { since: NOW - 100, first: false });
  assert.equal(P.lookedSince(String(NOW + 50), NOW).first, true);
  assert.equal(P.lookedSince(String(NOW - 90 * DAY), NOW).since, NOW - 30 * DAY);
  assert.match(P.feedWindowText({ first: false }), /since your last visit/);
});
const input = {
  seen: [{ group_key: 'a', remote: '1.2.3.4', first_seen: NOW - 50 }, { group_key: 'a', remote: '5.6.7.8', first_seen: NOW - 10 }, { group_key: 'b', remote: '9.9.9.9', first_seen: NOW - 5000 }],
  pers: [{ label: 'Agent', first_seen: NOW - 20, is_new: true }, { label: 'Old', first_seen: NOW - 9000, is_new: true }, { label: 'Baseline', first_seen: NOW - 15, is_new: false }],
  sudo: [{ ts: NOW - 30, args: 'sudo ls' }, { ts: NOW - 9000, args: 'sudo old' }],
};
test('the feed holds only changes after the last visit, newest first', () => {
  const rows = P.feedRows(input, NOW - 100, label);
  assert.deepEqual(rows.map((r) => r.kind), ['startup', 'sudo', 'first']);
  assert.deepEqual(rows.map((r) => r.ts), [NOW - 20, NOW - 30, NOW - 50]);
  assert.ok(rows.every((r) => r.ts > NOW - 100));
});
test('a feed row names its app and links to the item', () => {
  const rows = P.feedRows(input, NOW - 100, label);
  const host = P.feedRows({ seen: [{ group_key: 'a', remote: '5.6.7.8', first_seen: NOW - 10 }, { group_key: 'a', remote: 'old', first_seen: NOW - 9000 }] }, NOW - 100, label)[0];
  assert.equal(host.text, 'A contacted a new host: 5.6.7.8');
  assert.deepEqual(host.go, { view: 'apps', sel: 'a' });
  assert.deepEqual(rows.find((r) => r.kind === 'sudo').go, { view: 'priv' });
  assert.deepEqual(rows.find((r) => r.kind === 'startup').go, { view: 'persist', filter: 'new' });
});
test('an app with an older recorded host is not reported as a first connection', () => {
  const rows = P.feedRows({ seen: [{ group_key: 'a', remote: 'x', first_seen: NOW - 5000 }, { group_key: 'a', remote: 'y', first_seen: NOW - 5 }] }, NOW - 100, label);
  assert.deepEqual(rows.map((r) => r.kind), ['host']);
});
test('an empty or missing input gives an empty feed', () => {
  assert.deepEqual(P.feedRows({}, NOW - 100, label), []);
  assert.deepEqual(P.feedRows({ seen: [], pers: [], sudo: [] }, NOW - 100, label), []);
});
test('a long sudo command is cut', () => {
  const r = P.feedRows({ sudo: [{ ts: NOW, args: 'x'.repeat(500) }] }, NOW - 1, label);
  assert.ok(r[0].text.length < 150);
});
test('machine rows show a value or the reason, never 0', () => {
  const rows = P.machineRows({ freeText: '120 GB free', memoryText: null, memoryReason: 'restricted', cpuText: '12% total', cpuReason: null });
  const by = Object.fromEntries(rows.map((r) => [r.id, r]));
  assert.equal(by.free.value, '120 GB free');
  assert.equal(by.free.reason, null);
  assert.equal(by.memory.value, null);
  assert.equal(by.memory.reason, 'restricted');
  assert.equal(by.cpu.value, '12% total');
  assert.ok(!('forecast' in by));
});
test('the forecast row appears only with a forecast, and every row links somewhere', () => {
  const rows = P.machineRows({ freeText: null, freeReason: 'not read yet', forecastText: 'Full in about 90 days', memoryText: 'Normal', cpuText: null });
  assert.deepEqual(rows.map((r) => r.id), ['free', 'forecast', 'memory', 'cpu']);
  assert.equal(rows[0].reason, 'not read yet');
  assert.equal(rows[3].reason, 'no reason recorded');
  assert.ok(rows.every((r) => r.href && r.href.view));
});
test('a startup item that is not new since the baseline stays out of the feed', () => {
  const r = P.feedRows({ pers: [{ label: 'Baseline', first_seen: NOW - 15, is_new: false }] }, NOW - 100, label);
  assert.deepEqual(r, []);
});
test('an app that is new on the network gets one row, not one per host', () => {
  const r = P.feedRows({ seen: [{ group_key: 'a', remote: 'x', first_seen: NOW - 50 }, { group_key: 'a', remote: 'y', first_seen: NOW - 40 }] }, NOW - 100, label);
  assert.deepEqual(r.map((x) => x.kind), ['first']);
});
test('the forecast row shows its reason when there is no forecast yet', () => {
  const rows = P.machineRows({ freeText: 'x', forecastText: null, forecastReason: 'Collecting history', memoryText: 'Normal', cpuText: '1%' });
  const f = rows.find((r) => r.id === 'forecast');
  assert.equal(f.value, null);
  assert.equal(f.reason, 'Collecting history');
});
test('the memory row links to the Memory tab', () => {
  const rows = P.machineRows({ freeText: 'x', memoryText: 'Normal', cpuText: '1%' });
  assert.deepEqual(rows.find((r) => r.id === 'memory').href, { view: 'performance', ex: { perfTab: 'memory' } });
});

test('a feed time is a clock time today, a weekday this week and a date after that', () => {
  const now = new Date(2026, 10, 27, 15, 0, 0).getTime() / 1000;
  assert.equal(P.feedWhen(new Date(2026, 10, 27, 9, 5, 0).getTime() / 1000, now), '09:05');
  assert.equal(P.feedWhen(new Date(2026, 10, 24, 9, 5, 0).getTime() / 1000, now), 'Tue');
  assert.equal(P.feedWhen(new Date(2026, 9, 2, 9, 5, 0).getTime() / 1000, now), 'Oct 2');
});
const cpu = (ts, v) => ({ ts, cpu: { total_pct: v == null ? { unavailable: 'x' } : { value: v } } });
test('the CPU average covers the last five minutes and skips unreadable samples', () => {
  const samples = [cpu(100, 100), cpu(1000, 10), cpu(1100, null), cpu(1200, 30)];
  assert.equal(P.cpuAverage(samples, 300), 20);
  assert.equal(P.cpuAverage([], 300), null);
  assert.equal(P.cpuAverage([cpu(5, null)], 300), null);
});
test('decisions are sorted by severity and keep their order within a severity', () => {
  const list = [{ sev: 'low', n: 1 }, { sev: 'high', n: 2 }, { sev: 'medium', n: 3 }, { sev: 'high', n: 4 }];
  assert.deepEqual(P.bySeverity(list).map((x) => x.n), [2, 4, 3, 1]);
});
