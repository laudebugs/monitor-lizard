// A view returns an array of top-level sections, and some entries are `cond ? h(...) : null`. Element.append and
// Element.replaceChildren write a null argument as the text "null", so h() and render() both go through nodesOf.
// The test lifts the `view-items` block out of web/index.html and checks it against a stand-in element that
// stringifies non-object arguments the way the DOM does.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../../web/index.html', import.meta.url), 'utf8');
const m = html.match(/\/\* view-items:begin \*\/([\s\S]*?)\/\* view-items:end \*\//);
assert.ok(m, 'view-items block not found in web/index.html');
const { nodesOf } = new Function(m[1] + '\nreturn { nodesOf };')();

const fakeEl = () => ({ out: [], replaceChildren(...n) { this.out = n.map((x) => (typeof x === 'object' && x !== null ? x : String(x))); } });
const sect = { tag: 'section' };

test('null, undefined and false never reach replaceChildren as text', () => {
  const el = fakeEl();
  el.replaceChildren(...nodesOf([null, sect, undefined, false, sect]));
  assert.deepEqual(el.out, [sect, sect]);
});

test('nested arrays are flattened the way h() flattens its children', () => {
  const el = fakeEl();
  el.replaceChildren(...nodesOf([[sect, null], [[false, sect]], null]));
  assert.deepEqual(el.out, [sect, sect]);
});

test('zero and the empty string are kept', () => {
  assert.deepEqual(nodesOf([sect, 0, '']), [sect, 0, '']);
});

test('h() uses the same function', () => {
  assert.match(html, /e\.append\(\.\.\.nodesOf\(kids\)/);
});

// Any replaceChildren( call on the content element that spreads something other than nodesOf(...).
const bypasses = (src) => src.match(/\bc\.replaceChildren\(\s*\.\.\.(?!nodesOf\()/g) || [];

test('every view in render() goes through nodesOf', () => {
  assert.deepEqual(bypasses(html), []);
});

test('the bypass check catches calls written other ways', () => {
  for (const bad of ["c.replaceChildren(...priv());", "c.replaceChildren(...[a, b]);", "c.replaceChildren(...(x || []));", "c.replaceChildren(...items)"]) {
    assert.equal(bypasses(bad).length, 1, bad);
  }
  assert.deepEqual(bypasses("c.replaceChildren(...nodesOf(priv()));"), []);
});
