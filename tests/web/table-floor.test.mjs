// Table column floors. The grid strings and the floor block in web/index.html are read as text, so the test
// needs no browser. Whether a wide table really scrolls inside its card is checked by hand in a browser.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../../web/index.html', import.meta.url), 'utf8');
const m = html.match(/\/\* floor:begin \*\/([\s\S]*?)\/\* floor:end \*\//);
assert.ok(m, 'floor block not found in web/index.html');
const { trackMin, tracksMin } = new Function(m[1] + '\nreturn { trackMin, tracksMin };')();

const MIN_FLEX = 80;
const split = (s) => { const out = []; let d = 0, cur = ''; for (const ch of s) { if (ch === '(') d++; else if (ch === ')') d--; if (ch === ' ' && !d) { if (cur) out.push(cur); cur = ''; } else cur += ch; } return cur ? out.concat(cur) : out; };
const flexible = (t) => /fr\)?$/.test(t) || t === 'auto';
// Every grid a table row uses: the stylesheet default, the script default and the per-table strings.
// The drawer's .pr rows are lists, not tables, so they are left out.
const grids = [...html.matchAll(/(?:grid-template-columns:|GRID_DEFAULT = ')([^;'}]+)[;'}]/g)].filter((x) => !/\.pr\{/.test(html.slice(Math.max(0, x.index - 30), x.index)))
  .map((x) => x[1].trim()).filter((g) => /\d(px|fr)/.test(g) && split(g).length >= 4);

test('floor: the page defines the table grids this test covers', () => {
  assert.ok(grids.length >= 9, 'found ' + grids.length + ' grids');
});

test('floor: every flexible column has a readable minimum', () => {
  for (const g of grids) for (const t of split(g)) if (flexible(t)) assert.ok(trackMin(t) >= MIN_FLEX, t + ' in "' + g + '" has minimum ' + trackMin(t));
});

test('floor: a row minimum is the sum of its fixed widths and the first argument of each minmax', () => {
  assert.equal(tracksMin(split('80px minmax(80px,1fr) minmax(100px,1.5fr) 56px')), 80 + 80 + 100 + 56);
  assert.equal(tracksMin(split('minmax(0,1fr) 60px')), 60);
});

test('floor: the Network row cannot be narrower than 4 fixed tracks, 3 flexible minimums, gaps and padding', () => {
  const net = grids.find((g) => g.includes('190px') && g.includes('90px 110px'));
  assert.equal(tracksMin(split(net)), 80 + 90 + 110 + 190 + 3 * MIN_FLEX);
  // 6 gaps of 12 px and 16 + 64 px of padding come from the CSS rule for .tbl.floor>.tr
  assert.match(html, /\.tbl\.floor>\.tr\{min-width:calc\(var\(--tracks-min\) \+ \(var\(--cols\) - 1\) \* var\(--gap\) \+ var\(--pl\) \+ var\(--pr\)\)\}/);
  assert.match(html, /\.has-copy>\.tr\{--pr:64px\}/);
});

test('floor: a table that opts into the floor sets both variables', () => {
  assert.match(html, /setProperty\('--tracks-min'/);
  assert.match(html, /setProperty\('--cols'/);
});
