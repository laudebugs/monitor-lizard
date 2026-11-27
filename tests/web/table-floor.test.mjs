// Table column floors. The grid strings and the floor block in web/index.html are read as text, so the test
// needs no browser. Whether a wide table really scrolls inside its card is checked by hand in a browser.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../../web/index.html', import.meta.url), 'utf8');
const m = html.match(/\/\* floor:begin \*\/([\s\S]*?)\/\* floor:end \*\//);
assert.ok(m, 'floor block not found in web/index.html');
const { trackMin, tracksMin, scrollState, setFloor, nameAttrs, SIDE_BY_SIDE } = new Function(m[1] + '\nreturn { trackMin, tracksMin, scrollState, setFloor, nameAttrs, SIDE_BY_SIDE };')();

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

test('floor: the Network row cannot be narrower than 4 fixed tracks, 3 flexible minimums (one of 120 px), gaps and padding', () => {
  const net = grids.find((g) => g.includes('190px') && g.includes('90px 110px'));
  assert.equal(tracksMin(split(net)), 80 + 90 + 110 + 190 + 120 + 2 * MIN_FLEX);
  // 6 gaps of 12 px and 16 + 64 px of padding come from the CSS rule for .tbl.floor>.tr
  assert.match(html, /\.tbl\.floor>\.tr\{min-width:calc\(var\(--tracks-min\) \+ \(var\(--cols\) - 1\) \* var\(--gap\) \+ var\(--pl\) \+ var\(--pr\)\)\}/);
  assert.match(html, /\.has-copy>\.tr\{--pr:64px\}/);
});

// A stand-in for the table element: it records what setFloor does to it.
const fakeSec = (clientWidth) => {
  const attrs = new Map(), vars = new Map(), classes = new Set();
  return {
    clientWidth, attrs, vars, classes,
    style: { setProperty: (k, v) => vars.set(k, String(v)) },
    classList: { add: (c) => classes.add(c), toggle: (c, on) => (on ? classes.add(c) : classes.delete(c)) },
    setAttribute: (k, v) => attrs.set(k, String(v)), removeAttribute: (k) => attrs.delete(k),
  };
};

test('floor: scrollState uses the 1101 px breakpoint and a row exactly as wide as its card', () => {
  assert.equal(SIDE_BY_SIDE, 1101);
  assert.deepEqual(scrollState(900, 800, 1100), { region: true, xscroll: false });
  assert.deepEqual(scrollState(900, 800, 1101), { region: true, xscroll: true });
  assert.deepEqual(scrollState(800, 800, 1352), { region: false, xscroll: false });
  assert.deepEqual(scrollState(801, 800, 1352), { region: true, xscroll: true });
  assert.deepEqual(scrollState(700, 800, 400), { region: false, xscroll: false });
});

test('floor: setFloor sets the variables and the scroll region only while the row is wider than the card', () => {
  const sec = fakeSec(700);
  setFloor(sec, { offsetWidth: 902 }, ['120px', '80px', 'minmax(80px,1fr)'], 'Network connections', 1200);
  assert.equal(sec.vars.get('--tracks-min'), '280px');
  assert.equal(sec.vars.get('--cols'), '3');
  assert.ok(sec.classes.has('floor') && sec.classes.has('xscroll'));
  assert.equal(sec.attrs.get('tabindex'), '0');
  assert.equal(sec.attrs.get('role'), 'region');
  assert.equal(sec.attrs.get('aria-label'), 'Network connections, scrollable');
  // The card grows (a window resize): the same element loses the region and the class.
  sec.clientWidth = 1073;
  setFloor(sec, { offsetWidth: 902 }, ['120px'], 'Network connections', 1352);
  assert.ok(!sec.classes.has('xscroll'));
  for (const k of ['tabindex', 'role', 'aria-label']) assert.ok(!sec.attrs.has(k), k + ' should be removed');
  // Below the breakpoint the table scrolls without the class, but is still a labelled region.
  const narrow = fakeSec(342);
  setFloor(narrow, { offsetWidth: 902 }, ['120px'], 'Apps', 400);
  assert.ok(!narrow.classes.has('xscroll'));
  assert.equal(narrow.attrs.get('aria-label'), 'Apps, scrollable');
});

test('floor: decorateTables calls setFloor before and after it trims hidden columns, with a name for every table', () => {
  const body = html.slice(html.indexOf('function decorateTables()'), html.indexOf('let decoRaf'));
  assert.equal((body.match(/setFloor\(sec, head, /g) || []).length, 2);
  assert.match(body, /TABLE_NAME\[key\]/);
  const names = html.match(/const TABLE_NAME = \{([^}]*)\}/)[1];
  for (const k of [...html.matchAll(/sortHeader\('(\w+)'/g)].map((x) => x[1])) assert.match(names, new RegExp('\\b' + k + ':'), 'no name for table ' + k);
  assert.match(html, /\.tbl:focus-visible\{[^}]*outline:2px solid var\(--focus-edge\)/);
});

test('floor: a long name reaches the title of the name cell, and every name cell uses nameAttrs', () => {
  const long = 'com.example.a-very-long-launch-agent-name.helper.service';
  const a = nameAttrs(long, 'display:flex');
  assert.equal(a.title, long);
  assert.match(a.style, /display:flex/);
  assert.equal(nameAttrs(42).title, '42');
  assert.equal((html.match(/h\('span', nameAttrs\(/g) || []).length, 8); // Apps, Listeners, Persistence, Network, Storage, Storage clean-ups, Top consumers (running and stopped rows)
});

test('floor: a table that opts into the floor sets both variables', () => {
  assert.match(m[1], /setProperty\('--tracks-min'/);
  assert.match(m[1], /setProperty\('--cols'/);
});
