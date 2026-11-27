// Pure logic for the Storage page's clean-ups, Inside System Data and exported report. The test lifts the
// `storage-cleanups` block out of the dashboard scripts (web/*.js) with the blocks it reads.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html } from './page.mjs';
const block = (name) => {
  const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'));
  assert.ok(m, name + ' block not found in the dashboard scripts (web/*.js)');
  return m[1];
};
const names = 'stCleanups cleanupView safeIds sumSizes sumText pickedRows cleanupSummary safeCleanupLine systemData systemText stSectionReading escHtml reportHtml storageRows ST_RULE_TEXT ST_CLEANUP_TOP safeSubtotalText visibleText homeRelative'.split(' ');
const P = new Function(block('url-state') + block('disk-net') + block('page-shell') + block('storage-page') + block('storage-cleanups') + '\nreturn { ' + names.join(', ') + ' };')();

const GB = 1e9;
const val = (v) => ({ value: v });
const gone = (why) => ({ unavailable: why });
const item = (path, category, bytes, extra = {}) => ({ id: 'i' + path.replace(/\W/g, ''), path, category, label: 'review', rule: 'app-cache', size: typeof bytes === 'number' ? val(bytes) : bytes, apparent_bytes: 1, entries: 10, complete: true, unreadable: 0, ...extra });
const cat = (category, bytes, extra = {}) => ({ category, bytes, items: 1, unavailable_items: 0, unmeasured: 0, lower_bound: false, ...extra });
const safeItem = (path, bytes, extra = {}) => item(path, 'developer', bytes, { label: 'safe', rule: 'xcode-derived-data', ...extra });
const scan = (extra = {}) => ({
  scanning: false, next_scan_allowed_at: 1060, scanned_at: 1000, scan_ms: 19000, complete: true, refused: null, not_reached: [],
  coverage: { locations_total: 3, locations_measured: 3, bytes_measured: 60 * GB }, progress: { done: 3, total: 3 },
  capacity: { total: val(1000 * GB), used: val(400 * GB), purgeable: val(50 * GB), free: val(550 * GB) },
  categories: [cat('developer', 40 * GB), cat('caches', 20 * GB)],
  items: [safeItem('/Users/me/Library/Developer/Xcode/DerivedData', 40 * GB), item('/Users/me/Library/Caches/com.x', 'caches', 20 * GB),
    item('/Users/me/Documents/Taxes', 'downloads', 5 * GB, { label: 'yours', rule: 'user-content' })],
  snapshots: { count: val(2), dates: [], size: gone('no tool reports a snapshot size without root') }, notes: [], ...extra,
});
const clean = (d) => P.stCleanups(P.storageRows(d));

test('clean-ups: every labelled folder, largest first, each with a reason', () => {
  const list = clean(scan());
  assert.deepEqual(list.map((r) => r.label), ['safe', 'review', 'yours']);
  assert.deepEqual(list.map((r) => r.bytes), [40 * GB, 20 * GB, 5 * GB]);
  assert.equal(list[0].why, 'Xcode rebuilds this build cache');
});

test('clean-ups: an unknown rule gets the fallback reason, never undefined or a prototype property', () => {
  const list = clean(scan({ items: [item('/Users/me/Library/Caches/a', 'caches', 1, { rule: 'constructor' }), item('/Users/me/Library/Caches/b', 'caches', 2, { rule: 'made-up' })] }));
  for (const r of list) assert.equal(r.why, 'Not on the safe list, so it needs a look first.');
});

test('clean-ups: an unreadable size goes last and is not counted as zero', () => {
  const d = scan({ items: [item('/Users/me/Library/Caches/gone', 'caches', gone('permission denied')), item('/Users/me/Library/Caches/ok', 'caches', 10)] });
  const list = clean(d);
  assert.equal(list[0].path, '/Users/me/Library/Caches/ok');
  assert.equal(list[1].bytes, null);
  assert.match(P.sumText(P.sumSizes(list)), /1 size unavailable/);
});

test('clean-ups: locations the scan did not reach are not listed', () => {
  assert.equal(clean(scan({ not_reached: ['/Users/me/Library/Caches'] })).length, 3);
});

test('safe: a server safe label the page does not trust shows review, so it is not in the safe set', () => {
  const d = scan({ items: [safeItem('/Users/me/Documents/Build', 5 * GB), safeItem('/opt/cache', 5 * GB), safeItem('/Users/me/Library/Developer/Xcode/DerivedData', 5 * GB, { rule: 'made-up' })] });
  const list = clean(d);
  assert.deepEqual(P.safeIds(list), []);
  assert.ok(list.every((r) => r.label === 'review'));
});

test('no safe items: the line says so and the id list is empty', () => {
  const list = clean(scan({ items: [item('/Users/me/Library/Caches/a', 'caches', 1)] }));
  assert.equal(P.safeCleanupLine(list), 'Nothing safe to remove');
  assert.deepEqual(P.safeIds(list), []);
  assert.equal(P.safeCleanupLine(clean(scan())), '1 safe folder');
});

test('top 50 with Show all: 10,000 items list 50 and count the rest', () => {
  const items = Array.from({ length: 10000 }, (_, i) => item('/Users/me/Library/Caches/c' + i, 'caches', i + 1));
  const list = clean(scan({ items }));
  assert.equal(list.length, 10000);
  const v = P.cleanupView(list, false);
  assert.equal(v.shown.length, 50);
  assert.equal(v.hidden, 9950);
  assert.equal(v.shown[0].bytes, 10000);
  assert.deepEqual([P.cleanupView(list, true).shown.length, P.cleanupView(list, true).hidden], [10000, 0]);
});

test('selection: "Review all safe" adds up to the safe items of the list, no more', () => {
  const d = scan({ items: [safeItem('/Users/me/Library/Developer/Xcode/DerivedData', 4 * GB), safeItem('/Users/me/Library/Caches/Homebrew', 1 * GB, { rule: 'homebrew-cache', category: 'caches' }), item('/Users/me/Library/Caches/com.x', 'caches', 9 * GB)] });
  const list = clean(d), picked = P.pickedRows(list, P.safeIds(list));
  assert.equal(picked.length, 2);
  assert.equal(P.sumSizes(picked).bytes, 5 * GB);
  assert.equal(P.sumSizes(picked).bytes, list.filter((r) => r.label === 'safe').reduce((n, r) => n + r.bytes, 0));
});

test('selection: an id that is gone or no longer safe after a rescan drops out', () => {
  const list = clean(scan());
  assert.equal(P.pickedRows(list, ['nope', list[1].id, list[2].id]).length, 0);
  assert.equal(P.pickedRows(list, null).length, 0);
});

test('selection: a lower-bound size makes the total "at least"', () => {
  const list = clean(scan({ items: [safeItem('/Users/me/Library/Developer/Xcode/DerivedData', 4 * GB, { complete: false })] }));
  assert.match(P.sumText(P.sumSizes(list)), /^at least /);
});

test('system data: caches, snapshots, indexes, logs and other, in that order', () => {
  const rows = P.systemData(scan({ categories: [cat('caches', 20 * GB), cat('logs', 1 * GB), cat('developer', 40 * GB)], items: [item('/Users/me/Library/Caches', 'caches', 20 * GB), item('/Users/me/Library/Logs', 'logs', 1 * GB), safeItem('/Users/me/Library/Developer/Xcode/DerivedData', 40 * GB)] }));
  assert.deepEqual(rows.map((r) => r.id), ['caches', 'snapshots', 'indexes', 'logs', 'other']);
  assert.equal(rows[0].bytes, 20 * GB);
  assert.equal(rows[4].bytes, 40 * GB);
  assert.ok(rows[0].share > rows[3].share);
});

test('system data: snapshot size unavailable shows the count and the reason, not 0', () => {
  const rows = P.systemData(scan());
  const snap = rows.find((r) => r.id === 'snapshots');
  assert.equal(snap.bytes, null);
  assert.equal(snap.share, null);
  assert.equal(snap.detail, '2 snapshots');
  assert.equal(P.systemText(snap), 'unavailable: no tool reports a snapshot size without root');
});

test('system data: indexes are never measured and always say why; a missing section is not 0', () => {
  const rows = P.systemData(scan({ categories: [], items: [], snapshots: { count: gone('tmutil not found'), size: gone('tmutil not found'), dates: [] } }));
  assert.match(P.systemText(rows.find((r) => r.id === 'indexes')), /^unavailable: .*Spotlight/);
  for (const id of ['caches', 'logs', 'other']) assert.equal(P.systemText(rows.find((r) => r.id === id)), 'unavailable: not yet scanned', id);
  assert.match(rows.find((r) => r.id === 'snapshots').detail, /count unavailable: tmutil not found/);
});

test('system data: a section whose every folder was unreadable shows that reason', () => {
  const rows = P.systemData(scan({ categories: [cat('caches', 0)], items: [item('/Users/me/Library/Caches', 'caches', gone('permission denied'))] }));
  assert.equal(P.systemText(rows[0]), 'unavailable: permission denied');
});

const report = (d, extra = {}) => { const rows = P.storageRows(d); return P.reportHtml({ d, rows, shownRows: rows, system: P.systemData(d), generated: 'today', filterText: 'All sections, all labels', ...extra }); };

test('report: app and folder names with < and & are escaped everywhere', () => {
  const evil = '/Users/me/Library/Caches/<img src=x onerror=alert(1)>&Co "q"';
  const html2 = report(scan({ items: [item(evil, 'caches', GB)] }));
  assert.ok(!html2.includes('<img'));
  assert.ok(html2.includes('&lt;img src=x onerror=alert(1)&gt;&amp;Co &quot;q&quot;'));
  assert.equal(P.escHtml(null), '');
  assert.equal(P.escHtml("a'b"), 'a&#39;b');
});

test('report: self-contained, with no script, no remote address and a locked-down policy', () => {
  const out = report(scan());
  assert.ok(!/<script|<link|<img|<iframe|@import|url\(/i.test(out));
  assert.ok(!/(https?:)?\/\/[a-z0-9]/i.test(out.replace(/<!doctype/i, '')));
  assert.match(out, /default-src 'none'/);
});

test('report: carries the same numbers as the page, and says it lists paths', () => {
  const d = scan();
  const out = report(d);
  assert.ok(out.includes('40.0 GB') && out.includes('20.0 GB'));
  assert.match(out, /Safe to remove: 40\.0 GB/);
  assert.match(out, /lists folder paths/);
  assert.match(out, /unavailable: no tool reports a snapshot size without root/);
});

test('report: a partial scan says so and shows no safe-to-remove total', () => {
  const out = report(scan({ complete: false }));
  assert.match(out, /Partial scan/);
  assert.match(out, /No safe-to-remove total until a scan completes/);
  assert.ok(out.includes('Sizes marked "at least" can only grow. The treemap is not final. No safe-to-remove total appears until a scan completes.'));
  assert.ok(!/Safe to remove: /.test(out));
});

test('report: the folder table follows the view it is given', () => {
  const d = scan(), rows = P.storageRows(d);
  const out = P.reportHtml({ d, rows, shownRows: rows.slice(0, 1), system: P.systemData(d), generated: 'now', filterText: 'Label: safe' });
  assert.match(out, /Label: safe/);
  const folders = out.slice(out.indexOf('Folders in this view'));
  assert.ok(folders.includes('DerivedData') && !folders.includes('com.x'));
});

test('subtotal: the safe subtotal in the list header is the inspector total for the same set, in every case', () => {
  const cases = {
    plain: scan(),
    lowerBound: scan({ items: [safeItem('/Users/me/Library/Developer/Xcode/DerivedData', 4 * GB, { complete: false }), safeItem('/Users/me/Library/Caches/Homebrew', GB, { rule: 'homebrew-cache', category: 'caches' })] }),
    unavailable: scan({ items: [safeItem('/Users/me/Library/Developer/Xcode/DerivedData', 4 * GB), safeItem('/Users/me/Library/Caches/Homebrew', gone('permission denied'), { rule: 'homebrew-cache', category: 'caches' })] }),
    none: scan({ items: [item('/Users/me/Library/Caches/a', 'caches', GB)] }),
  };
  for (const [name, d] of Object.entries(cases)) {
    const list = clean(d), inspector = P.sumText(P.sumSizes(P.pickedRows(list, P.safeIds(list))));
    assert.equal(P.safeSubtotalText(list), inspector, name);
  }
  assert.match(P.safeSubtotalText(clean(cases.lowerBound)), /^at least 5\.0 GB$/);
  assert.match(P.safeSubtotalText(clean(cases.unavailable)), /^at least 4\.0 GB \(1 size unavailable\)$/);
});

test('report: a newline in a path shows as \\n and cannot look like two folders; a single quote is escaped', () => {
  const out = report(scan({ items: [item("/Users/me/Library/Caches/a\n/Users/me/Library/Caches/b'c\r\t", 'caches', GB)] }));
  assert.ok(!/a\n/.test(out.slice(out.indexOf('<h2>Suggested'))) , 'a raw newline reached the table');
  assert.ok(out.includes('a\\n/Users/me/Library/Caches/b&#39;c\\r\\t') || out.includes('~/Library/Caches/a\\n'), 'visible escape');
  assert.equal(P.visibleText('a\nb\u0007'), 'a\\nb\\x07');
});

test('report: hostile filterText and generated text are escaped and flattened', () => {
  const d = scan(), rows = P.storageRows(d);
  const out = P.reportHtml({ d, rows, shownRows: rows, system: P.systemData(d), generated: '<b>x</b>', filterText: 'search "</p><script>alert(1)</script>"\nline2' });
  assert.ok(!/<script|<b>x/i.test(out));
  assert.ok(out.includes('&lt;/p&gt;&lt;script&gt;') && out.includes('\\nline2'));
});

test('report: paths are home-relative, so the user name is not in the file', () => {
  const out = report(scan());
  assert.ok(!out.includes('/Users/me'));
  assert.ok(out.includes('~/Library/Caches/com.x'));
});
