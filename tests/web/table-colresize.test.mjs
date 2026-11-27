// The resize-math behind a dragged-wider table column. The grid strings, the floor block (trackMin) and the
// colresize block in the dashboard scripts (web/*.js) are read as text, so the test needs no browser. Whether
// dragging a real handle feels right, and the drag/keyboard wiring in sortHeader, is checked by hand in a browser.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html } from './page.mjs';
const floor = html.match(/\/\* floor:begin \*\/([\s\S]*?)\/\* floor:end \*\//);
const colresize = html.match(/\/\* colresize:begin \*\/([\s\S]*?)\/\* colresize:end \*\//);
assert.ok(floor, 'floor block not found in the dashboard scripts (web/*.js)');
assert.ok(colresize, 'colresize block not found in the dashboard scripts (web/*.js)');
const { isFlexTrack, applyColumnWidths } = new Function(floor[1] + '\n' + colresize[1] + '\nreturn { isFlexTrack, applyColumnWidths };')();

test('colresize: isFlexTrack is true only for a track with fr', () => {
  assert.equal(isFlexTrack('minmax(120px,1.6fr)'), true);
  assert.equal(isFlexTrack('96px'), false);
  assert.equal(isFlexTrack('56px'), false);
});

test('colresize: a dragged-wider flexible column becomes a fixed px track; a fixed column never changes', () => {
  const tracks = ['minmax(120px,1.6fr)', '56px', 'minmax(80px,1.4fr)'];
  const ids = ['name', 'cpu', 'exe'];
  assert.deepEqual(applyColumnWidths(tracks, ids, { name: 240 }), ['240px', '56px', 'minmax(80px,1.4fr)']);
  assert.deepEqual(applyColumnWidths(tracks, ids, { cpu: 200 }), tracks, 'cpu is a fixed track; dragging it is a no-op');
});

test('colresize: a width below the track minimum is clamped, never shrinks past it', () => {
  const tracks = ['minmax(120px,1.6fr)'];
  assert.deepEqual(applyColumnWidths(tracks, ['name'], { name: 40 }), ['120px']);
});

test('colresize: a stale width (a column id no longer in this table) is ignored, applied to nothing', () => {
  const tracks = ['minmax(120px,1.6fr)', '56px'];
  const ids = ['name', 'cpu'];
  assert.deepEqual(applyColumnWidths(tracks, ids, { oldColumnThatWasRenamed: 500 }), tracks);
});

test('colresize: a fractional or non-integer width rounds to a whole px', () => {
  assert.deepEqual(applyColumnWidths(['minmax(120px,1.6fr)'], ['name'], { name: 180.6 }), ['181px']);
});

test('colresize: decorateTables widens the surviving minimum, the same way setFloor expects', () => {
  const body = html.slice(html.indexOf('function decorateTables()'), html.indexOf('let decoRaf'));
  assert.match(body, /applyColumnWidths\(tracksOf\(head\), ids, widthsOf\(key\)\)/);
  assert.match(body, /applyColumnWidths\(tracksOf\(r\), ids, widthsOf\(key\)\)/);
});

test('colresize: every resizable column in sortHeader gets a grip, and a fixed one gets none', () => {
  assert.match(html, /function colGrip\(key, id, label, track\) \{\n\s*if \(!track \|\| !isFlexTrack\(track\)\) return null;/);
  assert.match(html, /class: 'colgrip'/);
});

test('colresize: the grip is reachable by keyboard and resets on double-click', () => {
  assert.match(html, /onKeydown: \(e\) => colHandleKeys\(e, key, id\)/);
  assert.match(html, /onDblclick: \(e\) => \{ e\.stopPropagation\(\); setWidth\(key, id, null\); render\(\); \}/);
});

test('colresize: widthsOf and setWidth use their own localStorage key, separate from the column chooser', () => {
  assert.match(html, /const COLW_STORE = 'table-colw:';/);
  assert.notEqual(html.match(/const COLW_STORE = '([^']+)'/)[1], html.match(/const COLS_STORE = '([^']+)'/)[1]);
});
