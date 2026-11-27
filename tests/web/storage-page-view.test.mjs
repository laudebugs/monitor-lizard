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
