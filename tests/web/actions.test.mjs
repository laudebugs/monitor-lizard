// Pure logic for the Stop/Quit confirm dialog. The test lifts the `actions` block out of
// the dashboard scripts (web/*.js), so the block must stay free of DOM and fetch access.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html } from './page.mjs';
const m = html.match(/\/\* actions:begin \*\/([\s\S]*?)\/\* actions:end \*\//);
assert.ok(m, 'actions block not found in the dashboard scripts (web/*.js)');
const { confirmable, evidenceLine, justStoppedRows } = new Function(m[1] + '\nreturn { confirmable, evidenceLine, justStoppedRows };')();

test('a pid with a confirmed start time is kept', () => {
  const r = confirmable([1], new Map([[1, 1700000000]]), new Map([[1, 12.5]]));
  assert.deepEqual(r, { pids: [1], starts: [1700000000], cpuPct: [12.5] });
});

test('a pid missing from the fresh read (already exited) is dropped, not guessed at', () => {
  const r = confirmable([1, 2], new Map([[1, 1700000000]]), new Map([[1, 12.5]]));
  assert.deepEqual(r, { pids: [1], starts: [1700000000], cpuPct: [12.5] });
});

test('a pid whose start time came back unavailable is dropped', () => {
  const r = confirmable([1, 2], new Map([[1, 1700000000], [2, null]]), new Map([[1, 12.5], [2, 0.1]]));
  assert.deepEqual(r, { pids: [1], starts: [1700000000], cpuPct: [12.5] });
});

test('none confirmable returns null, never an empty-but-truthy result', () => {
  assert.equal(confirmable([1, 2], new Map(), new Map()), null);
  assert.equal(confirmable([1], new Map([[1, null]]), new Map()), null);
});

test('a missing cpu reading falls back to null, not undefined, in the result', () => {
  const r = confirmable([1], new Map([[1, 100]]), new Map());
  assert.deepEqual(r.cpuPct, [null]);
});

test('evidence line: one process with a known percentage names it', () => {
  assert.equal(evidenceLine('Notes', [12]), 'Notes is using 12% CPU right now, from a fresh sample.');
});

test('evidence line: one process with an unknown percentage still confirms it is running', () => {
  assert.equal(evidenceLine('Notes', [null]), 'Confirmed running right now, from a fresh sample.');
});

test('evidence line: several processes sum their known percentages into one sentence', () => {
  assert.equal(evidenceLine('Chrome', [10, 5, null]), 'Together using 15% CPU right now, from a fresh sample.');
});

test('evidence line: several processes with nothing known names the count instead of a percentage', () => {
  assert.equal(evidenceLine('Chrome', [null, null]), '2 processes confirmed running right now, from a fresh sample.');
});

test('a pid missing from a fresh read is marked stopped, not silently dropped', () => {
  const prev = [{ pid: 1, command: 'a' }, { pid: 2, command: 'b' }];
  const fresh = [{ pid: 1, command: 'a' }];
  assert.deepEqual(justStoppedRows(prev, fresh), [{ pid: 2, command: 'b', stopped: true }]);
});

test('a row already marked stopped is never re-added on the next read', () => {
  const prev = [{ pid: 2, command: 'b', stopped: true }];
  assert.deepEqual(justStoppedRows(prev, []), [], 'it already had its one extra render; it must not reappear again');
});

test('nothing is marked stopped when every pid is still present', () => {
  const rows = [{ pid: 1, command: 'a' }];
  assert.deepEqual(justStoppedRows(rows, rows), []);
});
