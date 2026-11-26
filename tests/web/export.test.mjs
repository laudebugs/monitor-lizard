// Logic tests for table export, the column chooser and the per-row copy in web/index.html. Like the
// URL-state tests, they lift the block between the `export` markers out of the page and run it on
// its own, so that block must stay free of DOM and storage access. Downloads, the menu and layout
// are checked by hand in a browser.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../../web/index.html', import.meta.url), 'utf8');
const m = html.match(/\/\* export:begin \*\/([\s\S]*?)\/\* export:end \*\//);
assert.ok(m, 'export block not found in web/index.html');
const X = new Function(m[1] + '\nreturn { csvCell, toCsv, toJson, rowObject, colId, hiddenFrom, visibleCols, toggleHidden, fileName };')();

const col = (id, get, extra) => Object.assign({ id, label: id.toUpperCase(), get: get || ((r) => r[id]) }, extra);
const csv = (cols, rows) => X.toCsv(cols, rows);

test('csv: header row, CRLF line ends, a final CRLF', () => {
  assert.equal(csv([col('a'), col('b')], [{ a: 1, b: 'x' }, { a: 2, b: 'y' }]), 'A,B\r\n1,x\r\n2,y\r\n');
});

test('csv: a comma, a quote or a line break forces quotes, and quotes are doubled', () => {
  assert.equal(X.csvCell('a,b'), '"a,b"');
  assert.equal(X.csvCell('say "hi"'), '"say ""hi"""');
  assert.equal(X.csvCell('one\ntwo'), '"one\ntwo"');
  assert.equal(X.csvCell('one\r\ntwo'), '"one\r\ntwo"');
  assert.equal(X.csvCell('plain'), 'plain');
});

test('csv: a cell that starts like a formula gets a leading apostrophe', () => {
  for (const f of ['=SUM(1,1)', '+1', '-2+3', '@SUM(A1)', '\tcmd', '\rcmd']) {
    const out = X.csvCell(f);
    assert.ok(out.startsWith("'") || out.startsWith('"\'' ), JSON.stringify(f) + ' -> ' + JSON.stringify(out));
  }
  assert.equal(X.csvCell('=1+1'), "'=1+1");
  assert.equal(X.csvCell('=SUM(1,1)'), '"\'=SUM(1,1)"', 'guard first, then quote because of the comma');
  assert.equal(X.csvCell('@a'), "'@a");
  assert.equal(X.csvCell('\tx'), "'\tx");
});

test('csv: only the first character decides, and plain text is left alone', () => {
  assert.equal(X.csvCell('a=b'), 'a=b');
  assert.equal(X.csvCell('x-ray'), 'x-ray');
  assert.equal(X.csvCell(" =already"), ' =already');
});

test('csv: real numbers are not formulas, empty values are empty', () => {
  assert.equal(X.csvCell(-5), '-5');
  assert.equal(X.csvCell(0), '0');
  assert.equal(X.csvCell(null), '');
  assert.equal(X.csvCell(undefined), '');
  assert.equal(X.csvCell(NaN), '');
  assert.equal(X.csvCell(true), 'true');
});

test('csv: a header label is escaped like any cell', () => {
  assert.equal(csv([{ id: 'a', label: 'A, "B"', get: () => 1 }], []), '"A, ""B"""\r\n');
});

test('csv: zero rows gives the header only', () => {
  assert.equal(csv([col('a'), col('b')], []), 'A,B\r\n');
});

test('csv: non-ASCII text survives as-is', () => {
  const s = csv([col('app')], [{ app: 'Café 日本語 😀' }]);
  assert.equal(s, 'APP\r\nCafé 日本語 😀\r\n');
  assert.equal(new TextDecoder().decode(new TextEncoder().encode(s)), s);
});

test('csv: time columns print an ISO string, and empty or bad times print nothing', () => {
  const t = col('seen', null, { time: true });
  assert.equal(csv([t], [{ seen: 0 }, { seen: null }, { seen: 1e20 }]), 'SEEN\r\n1970-01-01T00:00:00.000Z\r\n\r\n\r\n');
});

test('json: an array of objects keyed by column id; times are epoch seconds plus an ISO string', () => {
  const cols = [col('app'), col('seen', null, { time: true }), col('n')];
  const out = JSON.parse(X.toJson(cols, [{ app: 'A', seen: 86400, n: 3 }, { app: 'B', seen: null }]));
  assert.deepEqual(out, [{ app: 'A', seen: 86400, seen_iso: '1970-01-02T00:00:00.000Z', n: 3 }, { app: 'B', seen: null, seen_iso: null, n: null }]);
});

test('json: zero rows is an empty array', () => {
  assert.equal(X.toJson([col('a')], []), '[]');
});

test('rowObject builds one row the same way toJson does', () => {
  const cols = [col('a'), col('t', null, { time: true })];
  assert.deepEqual(X.rowObject(cols, { a: 'x', t: 60 }), JSON.parse(X.toJson(cols, [{ a: 'x', t: 60 }]))[0]);
});

test('column ids come from the sort key, then the label', () => {
  assert.equal(X.colId({ sort: 'procs', label: 'Procs' }), 'procs');
  assert.equal(X.colId({ label: 'Requester chain' }), 'requester_chain');
  assert.equal(X.colId({ id: 'x', sort: 'y', label: 'z' }), 'x');
});

test('visibleCols hides what is chosen and ignores ids that are not columns', () => {
  const cols = [col('a'), col('b'), col('c')];
  assert.deepEqual(X.visibleCols(cols, ['b', 'nope']).map((c) => c.id), ['a', 'c']);
  assert.deepEqual(X.visibleCols(cols, []).map((c) => c.id), ['a', 'b', 'c']);
});

test('hiding every column keeps the last one', () => {
  const cols = [col('a'), col('b'), col('c')];
  let hidden = [];
  for (const id of ['a', 'b', 'c']) hidden = X.toggleHidden(cols, hidden, id);
  assert.deepEqual(X.visibleCols(cols, hidden).map((c) => c.id), ['c']);
  assert.deepEqual(X.toggleHidden(cols, hidden, 'c'), hidden, 'cannot hide the last visible column');
  assert.deepEqual(X.toggleHidden(cols, hidden, 'a'), ['b'], 'showing one again works');
});

test('stored choices that are damaged or hostile fall back to showing everything', () => {
  const cols = [col('a'), col('b')];
  for (const raw of [null, '', '{', 'null', '"a"', '{"a":1}', '[1,2]', '["__proto__","constructor"]']) {
    assert.deepEqual(X.visibleCols(cols, X.hiddenFrom(raw)).map((c) => c.id), ['a', 'b'], String(raw));
  }
  assert.deepEqual(X.hiddenFrom('["a"]'), ['a']);
  assert.deepEqual(X.visibleCols(cols, ['a', 'b']).map((c) => c.id), ['a', 'b'], 'a stored choice that hides everything is ignored');
});

test('file names are page and local date, with no product name', () => {
  assert.equal(X.fileName('network', 'csv', new Date(2026, 10, 26, 23, 59)), 'network-2026-11-26.csv');
  assert.equal(X.fileName('apps', 'json', new Date(2026, 0, 5)), 'apps-2026-01-05.json');
});

test('50,000 rows export in well under a second', () => {
  const rows = Array.from({ length: 50000 }, (_, i) => ({ a: i, b: 'app ' + i + ', "x"', c: i % 7 === 0 ? '=cmd' : 'ok', t: 1700000000 + i }));
  const cols = [col('a'), col('b'), col('c'), col('t', null, { time: true })];
  let t0 = performance.now();
  const s = csv(cols, rows);
  const dt = performance.now() - t0;
  t0 = performance.now();
  const j = X.toJson(cols, rows);
  const dj = performance.now() - t0;
  assert.equal(s.split('\r\n').length, 50002);
  assert.equal(JSON.parse(j).length, 50000);
  assert.ok(dt < 1000 && dj < 1000, 'csv ' + dt.toFixed(0) + ' ms, json ' + dj.toFixed(0) + ' ms');
});

test('every table column the page draws has an export value', () => {
  const region = html.match(/\/\* export-cols:begin \*\/([\s\S]*?)\/\* export-cols:end \*\//);
  assert.ok(region, 'export-cols block not found');
  const have = {};
  for (const mm of region[1].matchAll(/^ {2}(\w+): \{(.*)\},?$/gm)) have[mm[1]] = [...mm[2].matchAll(/(\w+): \(/g)].map((x) => x[1]);
  let seen = 0;
  for (const mm of html.matchAll(/sortHeader\('(\w+)', \[([^\]]*)\]/g)) {
    seen++;
    const ids = [...mm[2].matchAll(/\{ label: '([^']+)'(?:, sort: '(\w+)')?/g)].map((x) => X.colId({ sort: x[2], label: x[1] }));
    assert.deepEqual([...(have[mm[1]] || [])].sort(), ids.sort(), 'table ' + mm[1]);
  }
  assert.equal(seen, 8);
});
