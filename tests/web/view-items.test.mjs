// A view returns an array of top-level sections, and some entries are `cond ? h(...) : null`. The page
// hands the array to Element.replaceChildren, which writes a null argument as the text "null" where h()
// would have dropped it. The test lifts the `view-items` block out of web/index.html and checks it against
// a stand-in element that stringifies non-Node arguments the way the DOM does.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../../web/index.html', import.meta.url), 'utf8');
const m = html.match(/\/\* view-items:begin \*\/([\s\S]*?)\/\* view-items:end \*\//);
assert.ok(m, 'view-items block not found in web/index.html');
const { viewItems } = new Function(m[1] + '\nreturn { viewItems };')();

const fakeEl = () => ({ out: [], replaceChildren(...n) { this.out = n.map((x) => (typeof x === 'object' && x !== null ? x : String(x))); } });
const sect = { tag: 'section' };

test('null, undefined and false never reach replaceChildren as text', () => {
  const el = fakeEl();
  el.replaceChildren(...viewItems([null, sect, undefined, false, sect]));
  assert.deepEqual(el.out, [sect, sect]);
});

test('without the filter the stand-in shows what the page showed', () => {
  const el = fakeEl();
  el.replaceChildren(...[null, sect]);
  assert.deepEqual(el.out, ['null', sect]);
});

test('real nodes and empty strings and zero are kept', () => {
  assert.deepEqual(viewItems([sect, 0, '']), [sect, 0, '']);
});

test('every view in render() goes through viewItems', () => {
  const calls = html.match(/c\.replaceChildren\(\.\.\.(?!viewItems)\w+\(\)\)/g) || [];
  assert.deepEqual(calls, [], 'a view bypasses viewItems: ' + calls.join(' '));
});
