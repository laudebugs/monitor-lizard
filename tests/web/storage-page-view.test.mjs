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
const logicBlock = (name) => html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'))[1];
const L = new Function(['url-state', 'disk-net', 'page-shell', 'storage-page', 'storage-cleanups'].map(logicBlock).join('')
  + '\nreturn { storageState, safeIds, safeSummaryTail, safeSelectedLine, safeSumFor, cleanupSummary, safeCleanupLine, plural };')();
const node = (tag, attrs, ...kids) => ({ tag, attrs: attrs || {}, kids: kids.flat(Infinity).filter((k) => k != null && k !== false) });
const walk = (n) => (typeof n === 'string' ? [] : [n, ...n.kids.flatMap(walk)]);
const click = (b) => { let prevented = false; b.attrs.onClick({ preventDefault: () => { prevented = true; } }); return prevented; };
const safeRow = (n) => ({ id: 'id' + n, label: 'safe', bytes: 1e9, lowerBound: false });

function cleanups(complete, safeCount) {
  const said = [], recs = [];
  const S = { stData: { scanned_at: 1, complete }, q: '', stTile: 'all', stfilter: 'all', stAll: false, stMultiSig: '' };
  const env = {
    h: node, S, $: () => ({ value: '' }), announce: (t) => said.push(t), rec: (m) => recs.push(m), ...L,
    cleanupView: (l) => ({ shown: l, hidden: 0 }), ST_ALL: 'all', ST_CLEANUP_TOP: 10,
    stPickSig: () => 'sig', scrollBox: node, rowFloor: () => 0, CLEAN_TRACKS: [], CLEAN_COLS: '', listReset: '', cleanupRow: () => null,
  };
  const src = fn(storageJs, 'storageCleanups');
  const run = new Function(...Object.keys(env), src + '\nreturn storageCleanups;')(...Object.values(env));
  const list = Array.from({ length: safeCount }, (_, i) => safeRow(i));
  const tree = run(list, null, []);
  return { tree, said, recs, S, all: walk(tree) };
}
const reviewAll = (c) => c.all.find((n) => n.tag === 'button' && text(n) === 'Review all safe');

test('clean-ups summary: a partial scan states no safe total, a complete scan keeps it', () => {
  const sum = (c) => text(c.all.find((n) => n.attrs.id === 'st-cleanups-sum'));
  assert.equal(sum(cleanups(false, 2)), '2 folders, 2.0 GB. 2 safe folders. Safe total: not shown until the scan is complete.');
  assert.ok(sum(cleanups(true, 2)).includes('2 safe folders, 2.0 GB.'));
  assert.ok(sum(cleanups(false, 0)).endsWith('Nothing safe to remove.'), 'no safe folder, nothing to withhold');
});

test('Review all safe: the announcement carries no total on a partial scan', () => {
  const p = cleanups(false, 2);
  click(reviewAll(p));
  assert.deepEqual(p.said, ['2 safe folders selected. Total not shown until the scan is complete. Details are in the inspector.']);
  const c = cleanups(true, 2); click(reviewAll(c));
  assert.deepEqual(c.said, ['2 safe folders selected: 2.0 GB. Details are in the inspector.']);
});

test('Review all safe with nothing safe: aria-disabled, no disabled attribute, a click only announces', () => {
  const c = cleanups(true, 0), btn = reviewAll(c);
  assert.equal(btn.attrs['aria-disabled'], 'true');
  assert.ok(btn.attrs.disabled == null);
  assert.equal(btn.attrs['aria-describedby'], 'st-cleanups-sum');
  assert.equal(click(btn), true);
  assert.deepEqual(c.recs, [], 'the selection did not change');
  assert.deepEqual(c.said, ['Nothing safe to remove']);
  assert.equal(reviewAll(cleanups(true, 2)).attrs['aria-disabled'], null);
});

function actions(d, now = 1000) {
  const calls = { loads: 0 }, said = [];
  const S = { stData: d, stNote: null };
  const env = {
    h: node, S, scanningText: () => 'Scanning, 3 s so far', announce: (t) => said.push(t), rec: (m) => Object.assign(S, m),
    loadStorage: () => { calls.loads++; },
    rescanInfo: (x) => (x && x.scanning ? { ok: false, wait: 0, text: 'A scan is running.' } : x && x.next_scan_allowed_at > now
      ? { ok: false, wait: 5, text: 'A scan started less than a minute ago. The next one can start in 5 s.' } : { ok: true, wait: 0, text: '' }),
  };
  const make = new Function(...Object.keys(env), fn(storageJs, 'rescanNow') + '\n' + fn(storageJs, 'stActions') + '\nreturn stActions;')(...Object.values(env));
  const kids = make(d, now).filter(Boolean), all = walk({ tag: 'x', attrs: {}, kids });
  return { calls, said, S, all, btn: all.find((n) => n.tag === 'button' && text(n) === 'Rescan'), exp: all.find((n) => n.attrs.id === 'st-export-btn') };
}

test('Rescan: reachable by Tab (aria-disabled, never disabled) while a scan runs or inside the minute; a click sends nothing', () => {
  for (const d of [{ scanning: true, scanned_at: 900 }, { scanning: false, scanned_at: 900, next_scan_allowed_at: 1005 }]) {
    const a = actions(d);
    assert.equal(a.btn.attrs['aria-disabled'], 'true');
    assert.ok(a.btn.attrs.disabled == null, 'no disabled attribute');
    assert.equal(a.btn.attrs['aria-describedby'], 'st-rescan-why');
    const why = a.all.find((n) => n.attrs.id === 'st-rescan-why');
    assert.ok(text(why).length > 5, 'the reason is on the page');
    a.btn.attrs.onClick({ preventDefault() {} });
    a.btn.attrs.onClick({ preventDefault() {} });
    assert.equal(a.calls.loads, 0, 'no request');
    assert.equal(a.said.length, 2);
    assert.equal(a.S.stNote, null, 'the reason is already on the page, so no second note is kept that would go stale');
  }
  const ok = actions({ scanning: false, scanned_at: 900 });
  assert.equal(ok.btn.attrs['aria-disabled'], null);
  ok.btn.attrs.onClick({});
  assert.equal(ok.calls.loads, 1, 'one request per click');
});

test('a running scan says so once: the button keeps its name and the text beside it carries the progress', () => {
  const a = actions({ scanning: true, scanned_at: 900 });
  assert.equal(text(a.btn), 'Rescan');
  assert.equal(a.all.filter((n) => n.tag === 'span' && /Scanning/.test(text(n))).length, 1);
  assert.ok(!text({ tag: 'x', kids: [a.btn] }).includes('Scanning'));
});

test('the multi-select inspector shows the total it is given', () => {
  const env = { h: node, plural: (n, a, b) => n + ' ' + b, rec() {}, stKv: (...p) => p.filter(Boolean), stPart: (t, ...k) => node('div', {}, ...k), sizeText: () => '1 GB', stTrashButton: () => null };
  const make = new Function(...Object.keys(env), fn(storageJs, 'stMultiInspector') + '\nreturn stMultiInspector;')(...Object.values(env));
  assert.ok(JSON.stringify(make([{ name: 'a' }], 'Safe total: not shown until the scan is complete')).includes('Safe total: not shown until the scan is complete'));
});

// ---- focus survives a poll: render() restores it only for elements carrying data-req
test('Rescan, Review all safe, Export and Move to Trash carry data-req, so a quiet() poll gives focus back to them', () => {
  const a = actions({ scanning: false, scanned_at: 900 });
  assert.equal(a.btn.attrs['data-req'], 'st-rescan-btn');
  assert.equal(a.exp.attrs['data-req'], 'st-export-btn');
  assert.equal(reviewAll(cleanups(true, 2)).attrs['data-req'], 'st-review-all');
  assert.equal(reviewAll(cleanups(true, 0)).attrs['data-req'], 'st-review-all', 'the disabled one too');
  const trash = new Function('h', 'trashSelection', 'safeKnown', 'S', 'announce', 'openTrash', fn(storageJs, 'stTrashButton') + '\nreturn stTrashButton;')(node, () => ({ ok: true, yours: [] }), () => true, {}, () => {}, () => {});
  assert.equal(walk(trash([])).find((n) => n.attrs.id === 'st-trash-btn').attrs['data-req'], 'st-trash-btn');
  // The restore path, lifted from render(): the element active before a poll redraws the page is found again by data-req.
  const render = fn(shellJs, 'render');
  const grab = new Function('document', render.match(/const hadReq = [^\n]*;/)[0] + '\nreturn hadReq;');
  const restore = render.match(/else if \(hadReq\) \{.*?\}(?= \/\/)/)[0];
  const redo = new Function('document', 'hadReq', 'const af = null; if (af) af.focus();\n' + restore);
  const dom = (list) => { const doc = { activeElement: null }; doc.els = list.map((n) => ({ dataset: { req: n.attrs['data-req'] }, focus() { doc.activeElement = this; } }));
    doc.querySelectorAll = () => doc.els; return doc; };
  const buttons = () => [actions({ scanning: false, scanned_at: 900 }).btn, actions({ scanning: false, scanned_at: 900 }).exp, reviewAll(cleanups(true, 2))];
  for (const i of [0, 1, 2]) {
    const before = dom(buttons()); before.els[i].focus();
    const req = grab(before);
    const after = dom(buttons());                       // the poll rebuilt every element; focus is on the body
    redo(after, req);
    assert.equal(after.activeElement, after.els[i], 'button ' + i + ' has focus again after the redraw');
  }
});
