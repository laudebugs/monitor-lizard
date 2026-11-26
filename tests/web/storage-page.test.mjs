// Pure logic for the Storage page: scan state, "at least" wording, treemap tiles, table rows. The test lifts the
// `storage-page` block out of web/index.html with the blocks it reads (url-state, disk-net, page-shell).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../../web/index.html', import.meta.url), 'utf8');
const block = (name) => {
  const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'));
  assert.ok(m, name + ' block not found in web/index.html');
  return m[1];
};
const names = 'storageState atLeast staleScan measuredText coverageText storageFacts scanElapsed scanningText rescanInfo shownLabel storageRows sizeText shareText bySize storageTiles treemapColumns resolveTile tileCategories visibleRows labelCounts safeTotal nextTile verdictTitle ST_CATEGORIES ST_TILES'.split(' ');
const P = new Function(block('url-state') + block('disk-net') + block('page-shell') + block('storage-page') + '\nreturn { ' + names.join(', ') + ' };')();

const GB = 1e9;
const val = (v) => ({ value: v });
const gone = (why) => ({ unavailable: why });
const item = (path, category, bytes, extra = {}) => ({ id: path.replace(/\W/g, ''), path, category, label: 'review', rule: 'app-cache', size: val(bytes), apparent_bytes: bytes, entries: 10, complete: true, unreadable: 0, ...extra });
const cat = (category, bytes, extra = {}) => ({ category, bytes, items: 1, unavailable_items: 0, unmeasured: 0, lower_bound: false, ...extra });
const scan = (extra = {}) => ({
  scanning: false, next_scan_allowed_at: 1060, scanned_at: 1000, scan_ms: 19000, complete: true, refused: null, not_reached: [],
  coverage: { locations_total: 3, locations_measured: 3, bytes_measured: 60 * GB }, progress: { done: 3, total: 3 },
  capacity: { total: val(1000 * GB), used: val(400 * GB), purgeable: val(50 * GB), free: val(550 * GB) },
  categories: [cat('developer', 40 * GB), cat('caches', 20 * GB)],
  items: [item('/Users/me/Library/Developer/Xcode/DerivedData', 'developer', 40 * GB, { label: 'safe', rule: 'xcode-derived-data' }), item('/Users/me/Library/Caches/com.x', 'caches', 20 * GB)],
  snapshots: { count: val(0), dates: [], size: gone('no tool reports a snapshot size without root') }, notes: [], ...extra,
});

test('state: nothing held is loading or failed; the first scan running has no result yet', () => {
  assert.equal(P.storageState({ data: null, failed: false }), 'loading');
  assert.equal(P.storageState({ data: null, failed: true }), 'failed');
  assert.equal(P.storageState({ data: { scanning: true, next_scan_allowed_at: 1060 } }), 'scanning');
  assert.equal(P.storageState({ data: { scanning: false } }), 'empty');
  assert.equal(P.storageState({ data: { refused: 'HOME is not set' } }), 'refused');
});

test('state: complete only when the server says complete is true; anything else is partial', () => {
  assert.equal(P.storageState({ data: scan() }), 'complete');
  assert.equal(P.storageState({ data: scan({ complete: false }) }), 'partial');
  for (const v of [undefined, null, 'true', 1, 0]) assert.equal(P.storageState({ data: scan({ complete: v }) }), 'partial', String(v));
});

test('partial scan: no safe-to-remove total, only the reason; complete scan: the total', () => {
  const rows = P.storageRows(scan({ complete: false }));
  assert.deepEqual(P.safeTotal(scan({ complete: false }), rows), { shown: false, text: 'Not shown until the scan is complete.' });
  const done = P.safeTotal(scan(), P.storageRows(scan()));
  assert.equal(done.shown, true);
  assert.equal(done.bytes, 40 * GB);
  assert.equal(done.text, '40.0 GB');
});

test('lower_bound totals read "at least"; a total that is not a bound does not', () => {
  assert.equal(P.atLeast(12 * GB, true), 'at least 12.0 GB');
  assert.equal(P.atLeast(12 * GB, false), '12.0 GB');
  assert.equal(P.atLeast(0, true), 'at least 0 B');
  for (const bad of [null, undefined, NaN, -1, 1e19, 'x']) assert.equal(P.atLeast(bad, true), 'unavailable', String(bad));
  const tiles = P.storageTiles(scan({ complete: false, categories: [cat('developer', 40 * GB, { lower_bound: true, unmeasured: 2 }), cat('caches', 20 * GB)] }));
  assert.equal(tiles.find((t) => t.id === 'developer').lowerBound, true);
  assert.equal(tiles.find((t) => t.id === 'caches').lowerBound, false);
  const rows = P.storageRows(scan({ items: [item('/Users/me/a', 'caches', 5 * GB, { complete: false })] }));
  assert.equal(P.sizeText(rows[0]), 'at least 5.0 GB');
});

test('not_reached locations become "not yet scanned" rows, never a 0 size, and only the unfiltered view shows them', () => {
  const d = scan({ complete: false, not_reached: ['/Users/me/Downloads', '/Applications'] });
  const rows = P.storageRows(d), pending = rows.filter((r) => r.kind === 'pending');
  assert.equal(pending.length, 2);
  for (const r of pending) { assert.equal(r.bytes, null); assert.equal(P.sizeText(r), 'not yet scanned'); assert.equal(P.shareText(r), '-'); }
  assert.equal(pending[0].name, '~/Downloads');
  const tiles = P.storageTiles(d);
  assert.equal(P.visibleRows(rows, tiles, 'all', 'all').length, 4);
  assert.equal(P.visibleRows(rows, tiles, 'caches', 'all').filter((r) => r.kind === 'pending').length, 0);
  assert.equal(P.visibleRows(rows, tiles, 'all', 'safe').filter((r) => r.kind === 'pending').length, 0);
  assert.match(P.coverageText(d), /3 of 3 locations measured, 2 not yet scanned/);
});

test('an unavailable size shows its reason, never 0', () => {
  const d = scan({ items: [item('/Users/me/Library/Mail', 'mail', 0, { size: gone('permission denied') })], categories: [{ ...cat('mail', 0), items: 1, unavailable_items: 1 }] });
  const [r] = P.storageRows(d);
  assert.equal(r.bytes, null);
  assert.equal(P.sizeText(r), 'unavailable: permission denied');
  const mail = P.storageTiles(d).find((t) => t.id === 'mail');
  assert.equal(mail.bytes, null);
  assert.equal(mail.reason, 'permission denied');
});

test('a section that is entirely unavailable stays its own tile even though it measures 0 bytes', () => {
  const d = scan({ categories: [cat('developer', 400 * GB), cat('mail', 0, { items: 1 })], items: [item('/Users/me/Library/Mail', 'mail', 0, { size: gone('permission denied') }), item('/Users/me/x', 'developer', 400 * GB)] });
  const ids = P.storageTiles(d).map((t) => t.id);
  assert.ok(ids.includes('mail') && !ids.includes('other'));
});

test('a section with zero bytes keeps a tile or a place in Other, and no weight is zero or NaN', () => {
  const d = scan({ categories: [cat('developer', 40 * GB), cat('logs', 0)], items: [] });
  const tiles = P.storageTiles(d), other = tiles.find((t) => t.id === 'other');
  assert.ok(other && other.members.some((m) => m.realId === 'logs' && m.bytes === 0), 'zero-byte logs is listed in Other');
  for (const t of tiles.filter((x) => x.kind !== 'rest')) assert.ok(Number.isFinite(t.pct) && t.pct > 0, t.id);
  assert.ok(Math.abs(tiles.reduce((n, t) => n + t.pct, 0) - 100) < 1e-9);
  assert.equal(tiles.find((t) => t.kind === 'rest').pct, 0, 'the unmeasured rest takes no treemap share');
  // Every section is zero: equal weights, nothing divides by zero.
  const zeros = P.storageTiles(scan({ capacity: { total: val(0), used: val(0), purgeable: val(0), free: val(0) }, categories: [cat('developer', 0), cat('caches', 0), cat('logs', 0)], items: [] }));
  assert.deepEqual(zeros.map((t) => t.id).sort(), ['caches', 'developer', 'logs']);
  for (const t of zeros) assert.ok(Math.abs(t.pct - 100 / 3) < 1e-9);
});

test('huge and invalid sizes do not break the layout', () => {
  const huge = P.storageTiles(scan({ capacity: { total: val(9e17), used: val(9e17), purgeable: val(0), free: val(0) }, categories: [cat('developer', 8e17), cat('caches', 1e17)], items: [] }));
  for (const t of huge.filter((x) => x.kind !== 'rest')) assert.ok(Number.isFinite(t.pct) && t.pct > 0, t.id);
  assert.ok(Math.abs(huge.reduce((n, t) => n + t.pct, 0) - 100) < 1e-9);
  const bad = P.storageTiles(scan({ categories: [cat('developer', 1e30), cat('caches', -5), cat('logs', NaN)], items: [] }));
  for (const t of bad.filter((x) => x.kind === 'category')) assert.equal(t.bytes, null, t.id + ' is unavailable, not a number');
  for (const t of bad) assert.ok(Number.isFinite(t.pct), t.id);
  assert.deepEqual(P.storageTiles(scan({ categories: [], items: [], capacity: { total: gone('x'), used: gone('x'), purgeable: gone('x'), free: gone('x') } })), []);
});

test('a small section joins Other, but never every section, and the Other list stays reachable', () => {
  const d = scan({ categories: [cat('developer', 40 * GB), cat('logs', 3 * GB), cat('downloads', 1 * GB)], items: [] });
  const tiles = P.storageTiles(d), other = tiles.find((t) => t.id === 'other');
  assert.deepEqual(tiles.filter((t) => t.kind === 'category').map((t) => t.id), ['developer']);
  assert.deepEqual(other.members.map((m) => m.realId).sort(), ['downloads', 'logs']);
  assert.equal(other.bytes, 4 * GB);
  assert.equal(P.resolveTile('logs', tiles), 'other', 'a link to a merged section selects Other');
  assert.deepEqual(P.tileCategories('other', tiles).sort(), ['downloads', 'logs']);
  // On a disk where every section is under 1%, nothing merges: one big Other would hide them all.
  const tiny = P.storageTiles(scan({ capacity: { total: val(2000 * GB), used: val(1000 * GB), purgeable: val(0), free: val(1000 * GB) }, categories: [cat('developer', 3 * GB), cat('caches', 2 * GB)], items: [] }));
  assert.deepEqual(tiny.filter((t) => t.kind === 'category').map((t) => t.id).sort(), ['caches', 'developer']);
  assert.equal(P.resolveTile('nothing', tiles), 'all');
  assert.equal(P.resolveTile('all', tiles), 'all');
});

test('the tiles add up to the used total within 1 GB when capacity is readable', () => {
  const d = scan({ categories: [cat('developer', 40 * GB), cat('caches', 20 * GB), cat('logs', 1)], items: [] });
  const tiles = P.storageTiles(d), sum = tiles.reduce((n, t) => n + (t.bytes || 0), 0);
  assert.ok(Math.abs(sum - 400 * GB) < GB);
  assert.equal(tiles.find((t) => t.id === 'rest').bytes, 400 * GB - (60 * GB + 1));
  // Used unreadable: no invented remainder.
  assert.ok(!P.storageTiles(scan({ capacity: { total: val(1000 * GB), used: gone('statfs failed'), purgeable: gone('x'), free: gone('x') } })).some((t) => t.id === 'rest'));
  // More measured than used (clones are counted per copy): no negative remainder.
  assert.ok(!P.storageTiles(scan({ capacity: { total: val(1000 * GB), used: val(10 * GB), purgeable: val(0), free: val(990 * GB) } })).some((t) => t.id === 'rest'));
  assert.deepEqual(P.tileCategories('rest', tiles), []);
  assert.deepEqual(P.visibleRows(P.storageRows(d), tiles, 'rest', 'all'), []);
});

test('treemap columns hold every tile once and add up to 100', () => {
  for (const n of [1, 2, 3, 5, 7]) {
    const tiles = Array.from({ length: n }, (_, i) => ({ id: 't' + i, pct: 100 / n }));
    const cols = P.treemapColumns(tiles);
    assert.deepEqual(cols.flatMap((c) => c.tiles.map((t) => t.id)).sort(), tiles.map((t) => t.id).sort());
    assert.ok(Math.abs(cols.reduce((s, c) => s + c.pct, 0) - 100) < 1e-9);
    assert.ok(cols.length <= 3 && cols.every((c) => c.tiles.length));
  }
  assert.deepEqual(P.treemapColumns([]), []);
  const cols = P.treemapColumns([{ id: 'a', pct: 70 }, { id: 'b', pct: 20 }, { id: 'c', pct: 10 }]);
  assert.equal(cols[0].tiles[0].id, 'a');
});

test('a user-content folder is never shown as safe, whatever label the server sent', () => {
  const sent = (path, extra = {}) => P.shownLabel({ path, label: 'safe', rule: 'xcode-derived-data', category: 'developer', ...extra });
  assert.equal(sent('/Users/me/Library/Developer/Xcode/DerivedData'), 'safe');
  for (const p of ['/Users/me/Documents/x', '/Users/me/Desktop', '/Users/me/Pictures/a', '/Users/me/Movies', '/Users/me/Music', '/Users/me/Downloads/big.iso', '/Users/me/DOCUMENTS/Proj/node_modules']) assert.equal(sent(p), 'review', p);
  assert.equal(sent('/Users/me/x', { rule: 'user-content' }), 'review');
  assert.equal(sent('/Users/me/x', { category: 'downloads' }), 'review');
  assert.equal(sent('/Users/me/x', { category: 'mail' }), 'review');
  for (const l of ['danger', '', null, undefined, 'SAFE', 7]) assert.equal(P.shownLabel({ path: '/Users/me/x', label: l }), 'review', String(l));
  assert.equal(P.shownLabel({ path: '/Users/me/Documents', label: 'yours' }), 'yours');
  const rows = P.storageRows(scan({ items: [item('/Users/me/Documents/Proj/node_modules', 'developer', GB, { label: 'safe', rule: 'idle-node-modules' })] }));
  assert.equal(rows[0].label, 'review');
  assert.equal(P.safeTotal(scan(), rows).bytes, 0, 'it adds nothing to the safe total');
});

test('label counts and the default order: biggest first, unavailable last', () => {
  const d = scan({ items: [item('/Users/me/a', 'caches', 1 * GB), item('/Users/me/b', 'caches', 9 * GB, { label: 'yours', rule: 'user-content' }), item('/Users/me/c', 'caches', 0, { size: gone('permission denied') }), item('/Users/me/d', 'caches', 0)] });
  const rows = P.storageRows(d);
  assert.deepEqual(P.labelCounts(rows), { all: 4, safe: 0, review: 3, yours: 1 });
  assert.deepEqual([...rows].sort(P.bySize).map((r) => r.path.slice(-1)), ['b', 'a', 'd', 'c']);
  assert.equal(P.shareText(rows[1]), '0.9%');
  assert.equal(P.shareText({ share: 0.01 }), '<0.1%');
  assert.equal(P.shareText({ share: 41.6 }), '42%');
});

test('Rescan: refused with the wait while the server minute runs, allowed after, refused while scanning', () => {
  assert.deepEqual(P.rescanInfo({ scanning: true }, 1000), { ok: false, wait: 0, text: 'A scan is running.' });
  const wait = P.rescanInfo({ scanning: false, next_scan_allowed_at: 1060 }, 1019.2);
  assert.equal(wait.ok, false);
  assert.equal(wait.wait, 41);
  assert.match(wait.text, /next one can start in 41 s/);
  assert.equal(P.rescanInfo({ scanning: false, next_scan_allowed_at: 1060 }, 1060).ok, true);
  assert.equal(P.rescanInfo(null, 5).ok, true);
});

test('scanning text counts from the scan start the server implies; stale and measured-ago words', () => {
  assert.equal(P.scanningText({ scanning: true, next_scan_allowed_at: 1060 }, 1012), 'Scanning, 12 s so far');
  assert.equal(P.scanningText({ scanning: true }, 1012), 'Scanning');
  assert.equal(P.scanningText({ scanning: true, next_scan_allowed_at: 1060 }, 900), 'Scanning, 0 s so far');
  assert.equal(P.measuredText(1000, 1300), 'measured 5m ago');
  assert.equal(P.measuredText(1000, 1010), 'measured just now');
  assert.equal(P.measuredText(undefined, 1010), 'not measured yet');
  assert.equal(P.staleScan(1000, 1000 + 900), false);
  assert.equal(P.staleScan(1000, 1000 + 901), true);
  assert.equal(P.staleScan(undefined, 5), false);
});

test('title: a verdict from counts, never a total, and the page name before data', () => {
  assert.equal(P.verdictTitle('storage', null), 'Storage');
  assert.equal(P.storageFacts({ scanning: true }), null);
  assert.equal(P.verdictTitle('storage', P.storageFacts(scan({ complete: false, coverage: { locations_total: 305, locations_measured: 270, bytes_measured: 1 } }))), 'Scan partial: 270 of 305 locations measured');
  assert.equal(P.verdictTitle('storage', P.storageFacts(scan())), 'Scan complete: 3 locations measured');
  assert.equal(P.verdictTitle('storage', { measured: 3, complete: true }), 'Scan complete: 3 locations measured');
});

test('arrow keys walk the tiles in reading order and stop at the ends', () => {
  const ids = ['a', 'b', 'c'];
  assert.equal(P.nextTile(ids, 'a', 'ArrowRight'), 'b');
  assert.equal(P.nextTile(ids, 'b', 'ArrowDown'), 'c');
  assert.equal(P.nextTile(ids, 'c', 'ArrowDown'), 'c');
  assert.equal(P.nextTile(ids, 'a', 'ArrowLeft'), 'a');
  assert.equal(P.nextTile(ids, 'c', 'ArrowUp'), 'b');
  assert.equal(P.nextTile(ids, 'b', 'Home'), 'a');
  assert.equal(P.nextTile(ids, 'b', 'End'), 'c');
  assert.equal(P.nextTile(ids, 'zz', 'ArrowRight'), null);
  assert.equal(P.nextTile([], 'a', 'ArrowRight'), null);
});

test('the tile ids the address accepts are the scanner sections plus all, other and rest', () => {
  assert.deepEqual([...P.ST_TILES].sort(), ['all', ...Object.keys(P.ST_CATEGORIES), 'other', 'rest'].sort());
});

test('page wiring: removal is disabled with its reason, and the page never writes the address itself', () => {
  const view = html.slice(html.indexOf('function stInspector('), html.indexOf('const SAFETY_TAG'));
  assert.match(view, /disabled: true, title: 'Available after the removal route ships'/);
  assert.match(view, /aria-label': 'Move to Trash, disabled: available after the removal route ships'/);
  assert.doesNotMatch(view, /location\.hash|pushState|replaceState/);
  assert.match(view, /writeHash\(\{ view: 'performance'/);
  const loader = html.slice(html.indexOf('async function loadStorage('), html.indexOf('function rescanNow'));
  assert.match(loader, /r\.status === 429/);
  assert.match(loader, /Retry-After/);
});
