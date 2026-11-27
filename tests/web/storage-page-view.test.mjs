// Storage page code that sits outside the marked blocks: the Files fact, the Partial banner, the failed-load note
// and the visibilitychange wiring. Each function is cut out of its script by name and run against stand-ins,
// so no browser is needed.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { webFile, html } from './page.mjs';

const storageJs = webFile('page-storage.js');
const shellJs = webFile('shell.js');
const block = (name) => html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'))[1];
// A top-level function's source: from its keyword to the first closing brace in column 0.
const fn = (src, name) => {
  const m = new RegExp('^(async )?function ' + name + '\\(', 'm').exec(src);
  assert.ok(m, 'function ' + name + ' not found');
  const end = src.indexOf('\n}\n', m.index);
  return src.slice(m.index, end + 2);
};
// Stand-in element: text() joins every string below it.
const h = (tag, attrs, ...kids) => ({ tag, kids: kids.flat(Infinity).filter((k) => k != null && k !== false) });
const text = (n) => (typeof n === 'string' ? n : n.kids.map(text).join(' '));

test('Files fact: a missing, null or NaN entry count reads "not reported"; a number is unchanged', () => {
  const make = new Function('h', 'sizeText', 'timeCell', 'formatBytes', 'plural', 'ST_CATEGORIES', 'stKv',
    fn(storageJs, 'stItemFacts') + '\nreturn stItemFacts;');
  const stKv = (...pairs) => pairs.filter(Boolean);
  const facts = make(h, () => '1 GB', () => '-', (n) => n + ' B', (n, a, b) => n + ' ' + (n === 1 ? a : b), { caches: 'Caches' }, stKv);
  const files = (entries) => facts({ kind: 'item', path: '/p', category: 'caches', bytes: 1, entries, at: 1 }).find(([k]) => k === 'Files')[1];
  for (const bad of [undefined, null, NaN, Infinity, '12']) assert.equal(files(bad), 'not reported', String(bad));
  assert.equal(files(0), '0');
  assert.equal(files(12345), '12345');
  assert.equal(facts({ kind: 'pending', path: '/p' }).find(([k]) => k === 'Files'), undefined, 'a folder not yet scanned has no Files row');
});

test('Partial banner: three short sentences, and the HTML report says the same thing', () => {
  const make = new Function('h', 'coverageText', fn(storageJs, 'stBanner') + '\nreturn stBanner;');
  const banner = make(h, () => '2 of 3 locations measured');
  assert.equal(banner({}, 'complete'), null);
  const t = text(banner({}, 'partial'));
  const said = 'Sizes marked "at least" can only grow. The treemap is not final. No safe-to-remove total appears until a scan completes.';
  assert.ok(t.includes(said), t);
  assert.ok(block('storage-cleanups').includes(said), 'the report banner uses the same words');
});

test('a failed load clears the note of the earlier one', async () => {
  const src = fn(storageJs, 'loadStorage');
  const S = { view: 'storage', stData: { scanning: false }, stFailed: false, stNote: 'old note' };
  const quiet = (o) => Object.assign(S, o);
  const env = {
    S, document: { hidden: false }, clearTimeout() {}, setTimeout() { return 1; }, Date, TOKEN: 't', quiet, announce() {},
    loadStorageHistory() {}, stHistAt: 0, staleReply: () => false, rescanInfo: () => ({}), storageAnnouncement: () => null,
    retryDelay: () => 0, fetch: async () => { throw new Error('offline'); },
  };
  const run = new Function(...Object.keys(env), 'let stTimer = 0, stSeq = 0, stFails = 0;\n' + src + '\nreturn loadStorage;')(...Object.values(env));
  await run(false);
  assert.equal(S.stFailed, true);
  assert.equal(S.stNote, null);
});

test('visibilitychange: the Storage listener passes false, never the Event (an Event would read as a fresh Rescan)', () => {
  const lines = shellJs.split('\n').filter((l) => l.includes("addEventListener('visibilitychange'"));
  assert.ok(lines.length >= 3, 'the listeners are still in shell.js');
  assert.ok(lines.some((l) => /\(\)\s*=>\s*loadStorage\(false\)/.test(l)));
  const seen = [];
  const listeners = [];
  const document = { addEventListener: (t, f) => listeners.push(f) };
  new Function('document', 'loadMetrics', 'loadStorage', 'loadVolume', lines.join('\n'))(
    document, () => {}, (...a) => seen.push(a), () => {});
  const ev = { type: 'visibilitychange' };
  for (const f of listeners) f(ev);
  assert.deepEqual(seen, [[false]], 'loadStorage ran once, with exactly false');
  assert.ok(!seen.flat().includes(ev));
});

test('loadStorage asks for ?fresh=1 only when the argument is truthy', async () => {
  const src = fn(storageJs, 'loadStorage');
  const urls = [];
  const S = { view: 'storage', stData: null, stFailed: false };
  const env = {
    S, document: { hidden: false }, clearTimeout() {}, setTimeout() { return 1; }, Date, TOKEN: 't', quiet() {}, announce() {},
    loadStorageHistory() {}, stHistAt: 0, staleReply: () => false, rescanInfo: () => ({}), storageAnnouncement: () => null,
    retryDelay: () => 0, fetch: async (u) => { urls.push(u); return { ok: true, json: async () => ({ scanning: true }) }; },
  };
  const run = new Function(...Object.keys(env), 'let stTimer = 0, stSeq = 0, stFails = 0;\n' + src + '\nreturn loadStorage;')(...Object.values(env));
  await run(false); await run(new Event('visibilitychange')); await run(true);
  assert.deepEqual(urls, ['/api/storage', '/api/storage?fresh=1', '/api/storage?fresh=1'], 'the third case is why the listener must not pass the Event');
});

// ---- a partial scan states no safe total, and a button that cannot act is aria-disabled, not disabled
const node = (tag, attrs, ...kids) => ({ tag, attrs: attrs || {}, kids: kids.flat(Infinity).filter((k) => k != null && k !== false) });
const walk = (n) => (typeof n === 'string' ? [] : [n, ...n.kids.flatMap(walk)]);
const click = (b) => { let prevented = false; b.attrs.onClick({ preventDefault: () => { prevented = true; } }); return prevented; };
const sub = { safeSubtotalText: () => '40.0 GB', safeSubtotalFor: (d) => (d.complete ? '40.0 GB' : 'Not shown until the scan is complete.') };

function cleanups(complete, safeCount, extra = {}) {
  const said = [], recs = [];
  const S = { stData: { complete }, q: '', stTile: 'all', stfilter: 'all', stAll: false, stMultiSig: '' };
  const env = {
    h: node, S, $: () => ({ value: '' }), announce: (t) => said.push(t), rec: (m) => recs.push(m), plural: (n, a, b) => n + ' ' + (n === 1 ? a : b),
    storageState: ({ data }) => (data.complete ? 'complete' : 'partial'), cleanupView: (l) => ({ shown: l, hidden: 0 }),
    safeIds: () => Array.from({ length: safeCount }, (_, i) => 'id' + i), cleanupSummary: () => '3 folders, at least 65.0 GB',
    safeCleanupLine: () => (safeCount ? safeCount + ' safe folders' : 'Nothing safe to remove'), ST_ALL: 'all', ST_CLEANUP_TOP: 10,
    stPickSig: () => 'sig', scrollBox: node, rowFloor: () => 0, CLEAN_TRACKS: [], CLEAN_COLS: '', listReset: '', cleanupRow: () => null, ...sub, ...extra,
  };
  const src = fn(storageJs, 'storageCleanups');
  const run = new Function(...Object.keys(env), src + '\nreturn storageCleanups;')(...Object.values(env));
  const tree = run(safeCount ? [{ id: 'a' }] : [], null, []);
  return { tree, said, recs, S, all: walk(tree) };
}

test('clean-ups summary: a partial scan states no safe total, a complete scan keeps it', () => {
  const sum = (c) => text(c.all.find((n) => n.attrs.id === 'st-cleanups-sum'));
  const partial = sum(cleanups(false, 2));
  assert.ok(partial.includes('Safe total: not shown until the scan is complete.'), partial);
  assert.ok(!/\d GB, \d/.test(partial) && !partial.includes('40.0 GB'), partial);
  assert.ok(sum(cleanups(true, 2)).includes('2 safe folders, 40.0 GB.'));
  assert.ok(sum(cleanups(false, 0)).endsWith('Nothing safe to remove.'), 'no safe folder, nothing to withhold');
});

test('Review all safe: the announcement carries no total on a partial scan', () => {
  const p = cleanups(false, 2), btn = p.all.find((n) => n.tag === 'button' && text(n) === 'Review all safe');
  click(btn);
  assert.equal(p.said.length, 1);
  assert.ok(!p.said[0].includes('GB') && /not shown until the scan is complete/.test(p.said[0]), p.said[0]);
  const c = cleanups(true, 2); click(c.all.find((n) => n.tag === 'button' && text(n) === 'Review all safe'));
  assert.ok(c.said[0].includes('40.0 GB'));
});

test('Review all safe with nothing safe: aria-disabled, no disabled attribute, a click only announces', () => {
  const c = cleanups(true, 0), btn = c.all.find((n) => n.tag === 'button' && text(n) === 'Review all safe');
  assert.equal(btn.attrs['aria-disabled'], 'true');
  assert.ok(!('disabled' in btn.attrs) || btn.attrs.disabled == null);
  assert.equal(btn.attrs['aria-describedby'], 'st-cleanups-sum');
  assert.equal(click(btn), true);
  assert.deepEqual(c.recs, [], 'the selection did not change');
  assert.deepEqual(c.said, ['Nothing safe to remove']);
  const live = cleanups(true, 2).all.find((n) => n.tag === 'button' && text(n) === 'Review all safe');
  assert.equal(live.attrs['aria-disabled'], null);
});

function actions(d, now = 1000) {
  const calls = { rescan: 0, loads: 0 }, said = [];
  const S = { stData: d, stNote: null };
  const env = {
    h: node, S, scanningText: () => 'Scanning, 3 s so far', announce: (t) => said.push(t), rec: (m) => Object.assign(S, m),
    loadStorage: () => { calls.loads++; },
    rescanInfo: (x) => (x && x.scanning ? { ok: false, wait: 0, text: 'A scan is running.' } : x && x.next_scan_allowed_at > now
      ? { ok: false, wait: 5, text: 'A scan started less than a minute ago. The next one can start in 5 s.' } : { ok: true, wait: 0, text: '' }),
    S_: S,
  };
  const make = new Function(...Object.keys(env), fn(storageJs, 'rescanNow') + '\n' + fn(storageJs, 'stActions') + '\nreturn stActions;')(...Object.values(env));
  const tree = make(d, now);
  return { tree, calls, said, S, btn: walk({ tag: 'x', attrs: {}, kids: tree.filter(Boolean) }).find((n) => n.tag === 'button' && /Rescan|Scanning/.test(text(n))) };
}

test('Rescan: reachable by Tab (aria-disabled, never disabled) while a scan runs or inside the minute; a click sends nothing', () => {
  for (const d of [{ scanning: true, scanned_at: 900 }, { scanning: false, scanned_at: 900, next_scan_allowed_at: 1005 }]) {
    const a = actions(d);
    assert.equal(a.btn.attrs['aria-disabled'], 'true');
    assert.ok(a.btn.attrs.disabled == null, 'no disabled attribute');
    assert.equal(a.btn.attrs['aria-describedby'], 'st-rescan-why');
    const why = walk({ tag: 'x', attrs: {}, kids: a.tree.filter(Boolean) }).find((n) => n.attrs.id === 'st-rescan-why');
    assert.ok(text(why).length > 5, 'the reason is on the page');
    a.btn.attrs.onClick({ preventDefault() {} });
    a.btn.attrs.onClick({ preventDefault() {} });
    assert.equal(a.calls.loads, 0, 'no request');
    assert.equal(a.said.length, 2);
  }
  const ok = actions({ scanning: false, scanned_at: 900 });
  assert.equal(ok.btn.attrs['aria-disabled'], null);
  ok.btn.attrs.onClick({});
  assert.equal(ok.calls.loads, 1, 'one request per click');
});

test('the multi-select inspector shows the total it is given, so a partial scan can pass the withheld line', () => {
  const env = { h: node, plural: (n, a, b) => n + ' ' + b, rec() {}, stKv: (...p) => p.filter(Boolean), stPart: (t, ...k) => node('div', {}, ...k), sizeText: () => '1 GB', stTrashButton: () => null };
  const make = new Function(...Object.keys(env), fn(storageJs, 'stMultiInspector') + '\nreturn stMultiInspector;')(...Object.values(env));
  const t = JSON.stringify(make([{ name: 'a' }], 'Not shown until the scan is complete.'));
  assert.ok(t.includes('Not shown until the scan is complete.'));
  const call = storageJs.match(/stMultiInspector\(picked, ([^\n]*)\) : stSectionInspector/)[1];
  assert.match(call, /state === 'complete'/);
  assert.match(call, /SAFE_WITHHELD/);
});

test('the HTML report withholds the safe total on a partial scan', () => {
  const b = block('storage-cleanups');
  assert.match(b, /state === 'complete' \? 'Safe to remove: '[^\n]*'No safe-to-remove total until a scan completes\. '/);
});
