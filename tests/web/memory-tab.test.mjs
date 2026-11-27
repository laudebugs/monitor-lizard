// Pure logic for the Memory tab. The test lifts the `memory-tab` block out of web/index.html, so the
// block must stay free of DOM and storage access.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../../web/index.html', import.meta.url), 'utf8');
const m = html.match(/\/\* memory-tab:begin \*\/([\s\S]*?)\/\* memory-tab:end \*\//);
assert.ok(m, 'memory-tab block not found in web/index.html');
const { pressureTag, swapState } = new Function(m[1] + '\nreturn { pressureTag, swapState };')();

test('pressure level 4 is a bad tag', () => {
  assert.deepEqual(pressureTag(4), { text: 'Critical', cls: 'bad' });
});

test('pressure levels 1 and 2 are normal and warning', () => {
  assert.deepEqual(pressureTag(1), { text: 'Normal', cls: 'ok' });
  assert.deepEqual(pressureTag(2), { text: 'Warning', cls: 'warn' });
});

test('an unrecognized pressure level reads as unknown, not a guessed severity', () => {
  assert.deepEqual(pressureTag(3), { text: 'Unknown', cls: '' });
  assert.deepEqual(pressureTag(null), { text: 'Unknown', cls: '' });
});

test('swap total 0 reads as no swap in use, never a divide by zero', () => {
  assert.deepEqual(swapState(0, 0), { kind: 'none' });
});

test('swap unavailable when either reading is missing', () => {
  assert.deepEqual(swapState(null, 8e9), { kind: 'unavailable' });
  assert.deepEqual(swapState(1e9, null), { kind: 'unavailable' });
});

test('swap in use reports a percentage capped at 100', () => {
  assert.deepEqual(swapState(2e9, 8e9), { kind: 'ok', pct: 25 });
  assert.equal(swapState(9e9, 8e9).pct, 100);
});
