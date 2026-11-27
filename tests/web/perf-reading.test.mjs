// rv and rReason read a Reading<T> from the Rust side. The Memory chart maps them over every sample, so one odd value
// must never throw and take the whole page's render with it.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { html, webFile } from './page.mjs';

const m = html.match(/\/\* perf-reading:begin \*\/([\s\S]*?)\/\* perf-reading:end \*\//);
assert.ok(m, 'perf-reading block not found in the dashboard scripts (web/*.js)');
const { rv, rReason } = new Function(m[1] + '\nreturn { rv, rReason };')();

test('rv reads a value and is null for everything else, including the bare "restricted" string', () => {
  assert.equal(rv({ value: 5 }), 5);
  assert.equal(rv({ value: 0 }), 0);
  assert.equal(rv({ unavailable: 'no gpu' }), null);
  assert.equal(rv('restricted'), null);
  assert.equal(rv('anything else'), null);
  assert.equal(rv(null), null);
  assert.equal(rv(undefined), null);
  assert.equal(rv(7), null);
});

test('rReason says why, for each shape a reading can have', () => {
  assert.equal(rReason('restricted'), 'restricted');
  assert.equal(rReason({ unavailable: 'no gpu' }), 'no gpu');
  assert.equal(rReason(null), 'not sampled yet');
  assert.equal(rReason('surprise'), 'unavailable');
  assert.equal(rReason({}), 'unavailable');
});

test('a poll-time focus restore does not scroll the page back to the chart', () => {
  const shell = webFile('shell.js');
  const restore = shell.split('\n').filter((l) => /hadReq|if \(had\)/.test(l) && /\.focus\(/.test(l));
  assert.equal(restore.length, 2, 'the two restore lines moved');
  for (const l of restore) assert.match(l, /\.focus\(\{ preventScroll: true \}\)/);
  assert.match(webFile('table.js'), /\.handle'\)\?\.focus\(\{ preventScroll: true \}\)/);
});
