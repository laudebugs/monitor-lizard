// Logic tests for the global search block in web/search.js: matching, ranking, caps, hostile text, the keys and the
// address changes. The block is lifted out of the page like the other web tests, so it must stay free of DOM access.
// The menu's look and focus handling are checked by hand in a browser.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html, webFile } from './page.mjs';
const block = (name) => { const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/')); assert.ok(m, name + ' block not found in the dashboard scripts (web/*.js)'); return m[1]; };
const unprefixedSrc = webFile('core.js').match(/^const unprefixed = .*;$/m)[0]; // the one definition, in core.js
const G = new Function(unprefixedSrc + block('requester') + block('url-state') + block('guide-search') + block('global-search')
  + '\nreturn { searchAll, searchKey, searchOpen, searchWords, searchFlat, searchSummary, searchWell, parseHash, writeHash, SEARCH_CAP, SEARCH_KEEP, SEARCH_QUERY_MAX };')();

const conn = (o) => Object.assign({ direction: 'outbound', scope: 'public', proto: 'TCP', exposed: false }, o);
const grp = (label, conns, key) => ({ key: key || 'other:/Applications/' + label + '.app', label, conns, trust: 'unknown' });
const snap = [
  grp('node', [conn({ direction: 'listen', local_port: 3000, scope: 'loopback' }), conn({ direction: 'listen', local_port: 3000, proto: 'TCP', exposed: true })]),
  grp('Postgres', [conn({ direction: 'listen', local_port: 5432 })]),
  grp('Chrome', [conn({ remote_addr: '142.250.80.46', remote_port: 443 }), conn({ remote_addr: '2001:db8::1', remote_port: 443 }), conn({ remote_addr: '127.0.0.1', scope: 'loopback' })]),
  grp('Docker Desktop', [conn({ direction: 'listen', local_port: 30001 })]),
];
const guide = { commands: [
  { id: 'tmutil-list', topic: 'storage', title: 'List local Time Machine snapshots', command: 'tmutil listlocalsnapshots /', description: 'Lists the dates of local snapshots.', tags: ['snapshot'], safety: 'read-only' },
  { id: 'docker-df', topic: 'developer', title: 'Docker disk use', command: 'docker system df', description: 'Space used.', tags: ['docker'], safety: 'read-only' },
], terms: [{ id: 'apfs', term: 'APFS', meaning: 'The disk format. A local snapshot lives here.', tags: [] }] };
const pers = [{ label: 'com.example.helper', program: '/Library/Helper/helper', kind: 'LaunchAgent', scope: 'user' }, { label: 'Dropbox', program: '/Applications/Dropbox.app', kind: 'LoginItem', scope: 'user' }];
const data = { groups: snap, seen: [{ group_key: 'x', label: 'Old', remote: '203.0.113.9' }, { group_key: 'y', label: 'Chrome', remote: '142.250.80.46' }], pers, guide };
const find = (res, id) => res.groups.find((g) => g.id === id);

test('empty and blank queries close the menu: no groups', () => {
  for (const q of ['', '   ', null, undefined, '\u200b\u200e']) assert.deepEqual(G.searchAll(q, data).groups, []);
});
test('a port number offers the Listeners entry (once for IPv4 and IPv6 sockets), and Enter opens it through writeHash', () => {
  const r = find(G.searchAll('3000', data), 'ports');
  assert.equal(r.items[0].label, 'Port 3000');
  assert.equal(r.items[0].page, 'Listeners');
  assert.equal(r.items[0].hash, '#listeners?q=3000');
  assert.equal(G.parseHash(r.items[0].hash).q, '3000');
});
test('ports match by prefix, the exact port first', () => {
  const labels = find(G.searchAll('3000', data), 'ports').items.map((i) => i.label);
  assert.deepEqual(labels, ['Port 3000', 'Port 30001']);
});
test('apps match by label and by key, every word', () => {
  assert.equal(find(G.searchAll('docker desk', data), 'apps').items[0].label, 'Docker Desktop');
  assert.equal(find(G.searchAll('applications/chrome', data), 'apps').items[0].label, 'Chrome');
  assert.equal(G.searchAll('docker zzz', data).groups.length, 0);
});
test('an app result opens the Apps page with the app selected and no filter', () => {
  const it = find(G.searchAll('postgres', data), 'apps').items[0], st = G.parseHash(it.hash);
  assert.equal(st.view, 'apps'); assert.equal(st.sel, 'other:/Applications/Postgres.app'); assert.equal(st.filter, 'all'); assert.equal(st.q, '');
});
test('apps without network activity are not offered (the Apps page does not list them)', () => {
  assert.equal(G.searchAll('idle', { groups: [grp('Idle', [])], seen: [], pers: [], guide: {} }).groups.length, 0);
});
test('hosts match by IP prefix, and an IPv6 address with or without brackets', () => {
  assert.equal(find(G.searchAll('142.250', data), 'hosts').items[0].label, '142.250.80.46');
  assert.equal(find(G.searchAll('2001:db8', data), 'hosts').items[0].label, '2001:db8::1');
  assert.equal(find(G.searchAll('[2001:db', data), 'hosts').items[0].label, '2001:db8::1');
  assert.equal(find(G.searchAll('[2001:db8::1]', data), 'hosts').items[0].label, '2001:db8::1');
});
test('loopback is not a host; a host only in the history opens History, a live one Network', () => {
  assert.equal(G.searchAll('127.0', data).groups.filter((g) => g.id === 'hosts').length, 0);
  assert.equal(find(G.searchAll('203.0', data), 'hosts').items[0].page, 'History');
  const live = find(G.searchAll('142.250', data), 'hosts').items;
  assert.equal(live.length, 1, 'one row for a host in both lists');
  assert.equal(live[0].page, 'Network'); assert.equal(live[0].hash, '#network?q=142.250.80.46');
});
test('startup items match label or program, every word', () => {
  assert.equal(find(G.searchAll('example helper', data), 'startup').items[0].label, 'com.example.helper');
  assert.equal(find(G.searchAll('library/helper', data), 'startup').items[0].page, 'Persistence');
});
test('"snapshots" offers the Guide entries, command and glossary, with the typed query in the address', () => {
  const res = G.searchAll('snapshots', data);
  assert.equal(res.groups[0].id, 'guide');
  const hashes = res.groups[0].items.map((i) => i.hash);
  assert.equal(G.parseHash(hashes[0]).ex.gsel, 'tmutil-list');
  assert.equal(G.parseHash(hashes[0]).q, 'snapshots');
  assert.equal(G.parseHash(hashes[1]).ex.gtab, 'glossary');
});
test('a group with the better match comes first, equal matches keep the fixed order', () => {
  const res = G.searchAll('docker', data);
  assert.deepEqual(res.groups.map((g) => g.id), ['apps', 'guide']);
  const exact = G.searchAll('apfs', data);
  assert.equal(exact.groups[0].id, 'guide');
});
test('at most five per group, and the best five', () => {
  const many = Array.from({ length: 40 }, (_, i) => grp('app' + String(i).padStart(2, '0'), [conn({ remote_addr: '10.0.0.' + i })]));
  many.push(grp('app', [conn({ remote_addr: '10.9.9.9' })]));
  const r = find(G.searchAll('app', { groups: many, seen: [], pers: [], guide: {} }), 'apps');
  assert.equal(r.items.length, G.SEARCH_CAP);
  assert.equal(r.items[0].label, 'app', 'the exact match beats 40 prefix matches');
});
test('the 200-per-tier keep: an exact match after 250 prefix matches is still offered first', () => {
  const many = Array.from({ length: 250 }, (_, i) => grp('app' + String(i).padStart(3, '0'), [conn({})]));
  many.push(grp('app', [conn({})]));
  const r = find(G.searchAll('app', { groups: many, seen: [], pers: [], guide: {} }), 'apps');
  assert.equal(r.items[0].label, 'app');
  assert.equal(r.kept, 201, 'one exact plus 200 of the 250 prefix matches');
});
test('a lone surrogate in an app key becomes the replacement character, so the selection is not silently dropped', () => {
  assert.equal(G.searchWell('a\ud800b'), 'a\ufffdb'); assert.equal(G.searchWell('a\udc00'), 'a\ufffd'); assert.equal(G.searchWell('\ud83d\ude00'), '\ud83d\ude00');
  const res = G.searchAll('bad', { groups: [grp('bad', [conn({})], 'other:/x\ud800')], seen: [], pers: [], guide: {} });
  assert.ok(G.parseHash(res.groups[0].items[0].hash).sel.endsWith('\ufffd'));
});
test('regex metacharacters are plain text', () => {
  for (const q of ['.*', '(', '[', '\\', '^$', '(?<a>', '+', '?', '{1,2}', '|', 'a++']) assert.doesNotThrow(() => G.searchAll(q, data));
  assert.equal(G.searchAll('.*', data).groups.length, 0);
  const dot = { groups: [grp('a.b', [conn({})]), grp('axb', [conn({})])], seen: [], pers: [], guide: {} };
  assert.deepEqual(find(G.searchAll('a.b', dot), 'apps').items.map((i) => i.label), ['a.b']);
});
test('a very long query is cut and still answers fast', () => {
  const long = 'x'.repeat(100000);
  const t = performance.now(); const res = G.searchAll(long, data);
  assert.ok(performance.now() - t < 50);
  assert.equal(res.groups.length, 0);
  assert.equal(G.searchWords(long + ' a b c d e f g h i j').length <= 8, true);
  assert.equal(G.searchWords(long)[0].length, G.SEARCH_QUERY_MAX);
  assert.equal(G.searchAll('Chrome' + ' '.repeat(5000) + 'zzz', data).count > 0, true, 'text past the cut is ignored');
});
test('hostile names stay text: markup, ampersands, control, zero-width and bidi characters are shown escaped', () => {
  const evil = '<img src=x onerror=alert(1)> & \u202Efdp.exe\u200b\u0007';
  const res = G.searchAll('img', { groups: [grp(evil, [conn({})])], seen: [], pers: [], guide: {} });
  const label = res.groups[0].items[0].label;
  assert.ok(label.includes('<img src=x onerror=alert(1)> &'), 'markup is kept as text for a text node, never parsed');
  assert.ok(label.includes('\\u{202e}') && label.includes('\\u{200b}') && label.includes('\\u{7}'));
  assert.ok(!/[\u202E\u200b\u0007]/.test(label));
  assert.ok(!/[<>&"]/.test(res.groups[0].items[0].hash.slice(1)), 'the address is percent-encoded');
});
test('invisible characters do not hide a name from a search for what it looks like', () => {
  const res = G.searchAll('evil', { groups: [grp('ev\u200bil\u202e', [conn({})])], seen: [], pers: [], guide: {} });
  assert.equal(res.count, 1);
  assert.equal(G.searchAll('e\u200bvil', { groups: [grp('evil', [conn({})])], seen: [], pers: [], guide: {} }).count, 1);
});
test('width and case variants match (NFKC, lower case)', () => {
  assert.equal(G.searchAll('\uFF23hrome', data).count > 0, true);
  assert.equal(G.searchAll('CHROME', data).count > 0, true);
});
test('10,000 apps, ports, hosts, history and startup items: the work is bounded, and a generous time limit is a smoke check', () => {
  const groups = Array.from({ length: 10000 }, (_, i) => grp('Application ' + i, [conn({ direction: 'listen', local_port: 1024 + (i % 60000) }), conn({ remote_addr: '10.' + (i >> 8 & 255) + '.' + (i & 255) + '.1' })]));
  const seen = Array.from({ length: 10000 }, (_, i) => ({ group_key: 'k' + i, label: 'Old ' + i, remote: '172.16.' + (i >> 8 & 255) + '.' + (i & 255) }));
  const big = { groups, seen, pers: Array.from({ length: 10000 }, (_, i) => ({ label: 'item.' + i, program: '/p/' + i, kind: 'LaunchAgent', scope: 'user' })), guide };
  const cases = [['application', 'apps'], ['application 99', 'apps'], ['10.1', 'hosts'], ['172.16', 'hosts'], ['10', 'ports'], ['item', 'startup']];
  for (const [q, id] of cases) {
    let ms = Infinity, res;
    for (let k = 0; k < 5; k++) { const t = performance.now(); res = G.searchAll(q, big); ms = Math.min(ms, performance.now() - t); }
    const g = find(res, id);
    assert.ok(g, q + ' finds ' + id);
    assert.equal(g.items.length, G.SEARCH_CAP, q + ': the menu shows five');
    assert.ok(g.kept <= 4 * G.SEARCH_KEEP, q + ': candidates kept stay bounded (' + g.kept + ')');
    assert.ok(ms < 300, q + ' took ' + ms.toFixed(1) + ' ms');
  }
  assert.equal(find(G.searchAll('application', big), 'apps').kept, G.SEARCH_KEEP, 'a tier keeps its first 200 and no more');
});
test('odd data does not throw: missing fields, odd types', () => {
  assert.doesNotThrow(() => G.searchAll('a', { groups: [null, { key: 'k', label: 'a', conns: [{}] }, grp(undefined, [conn({ remote_addr: 5 })])], seen: [{}], pers: [{ label: 'a' }], guide: { commands: [{ id: 'x' }] } }));
  assert.doesNotThrow(() => G.searchAll('a', null));
  assert.doesNotThrow(() => G.searchAll('a'));
});
test('the summary names the count and the groups, or says nothing matches', () => {
  assert.equal(G.searchSummary(G.searchAll('zzzz', data)), 'Nothing matches \u201czzzz\u201d');
  assert.equal(G.searchAll('<b>\u202e', data).query, '<b>\\u{202e}', 'the query read aloud is escaped like the results');
  assert.match(G.searchSummary(G.searchAll('3000', data)), /^2 results: Ports/);
  assert.match(G.searchSummary({ count: 1, groups: [{ title: 'Apps' }] }), /^1 result: Apps$/);
});

const st = (o) => Object.assign({ open: true, active: -1, count: 3, hasText: true }, o);
const key = (o, k, composing = false, mod = false) => G.searchKey(o, k, composing, mod);
test('keys: Down and Up cycle through the results and the "no selection" state', () => {
  let s = st({});
  const seq = []; for (let i = 0; i < 5; i++) { s = Object.assign(st({}), key(s, 'ArrowDown')); seq.push(s.active); }
  assert.deepEqual(seq, [0, 1, 2, -1, 0]);
  assert.equal(key(st({ active: 0 }), 'ArrowUp').active, -1);
  assert.equal(key(st({ active: -1 }), 'ArrowUp').active, 2);
  assert.equal(key(st({ count: 0 }), 'ArrowDown').active, -1);
});
test('keys: Down on a closed menu opens it with no row chosen; Up opens it on the last row; with no text nothing happens', () => {
  assert.deepEqual(key(st({ open: false }), 'ArrowDown'), { open: true, active: -1, act: null, handled: true });
  assert.deepEqual(key(st({ open: false }), 'ArrowUp'), { open: true, active: 2, act: null, handled: true });
  assert.equal(key(st({ open: false, hasText: false }), 'ArrowDown').handled, false);
});
test('keys: Enter opens the active result, or with none chosen asks for the page filter', () => {
  assert.deepEqual(key(st({ active: 1 }), 'Enter'), { open: false, active: 1, act: 'open', handled: true });
  assert.equal(key(st({ active: -1 }), 'Enter').act, 'filter');
  assert.equal(key(st({ open: false }), 'Enter').act, 'filter');
  assert.equal(key(st({ count: 0 }), 'Enter').act, 'filter', 'the "nothing matches" row is not a result');
});
test('keys: Escape closes an open menu, then empties the box, and passes through with no text', () => {
  assert.equal(key(st({ active: 1 }), 'Escape').act, 'close');
  assert.equal(key(st({ open: false }), 'Escape').act, 'clear');
  assert.equal(key(st({ open: false, hasText: false }), 'Escape').handled, false);
});
test('keys: while an input method is composing, or with a modifier held, no key is ours', () => {
  for (const k of ['Enter', 'ArrowDown', 'ArrowUp', 'Escape']) {
    assert.equal(key(st({ active: 1 }), k, true).handled, false);
    assert.equal(key(st({ active: 1 }), k, false, true).handled, false);
  }
});
test('keys: other keys are not ours', () => { assert.equal(key(st({ active: 1 }), 'a').handled, false); });

const fakeNav = (hash) => { const log = []; return { log, loc: { get hash() { return hash; }, set hash(v) { log.push(['push', v]); hash = v; } }, hist: { replaceState: (_, __, v) => { log.push(['replace', v]); hash = v; } } }; };
test('back button: a result on another page is a new history entry', () => {
  const n = fakeNav('#apps?q=no'); let applied = 0;
  assert.equal(G.searchOpen('#listeners?q=3000', n.loc, n.hist, () => applied++), 'push');
  assert.deepEqual(n.log, [['push', '#listeners?q=3000']]); assert.equal(applied, 0, 'the hashchange event does the work');
});
test('back button: an app on the Apps page is a new entry, a refinement on the same page replaces', () => {
  let n = fakeNav('#apps'); assert.equal(G.searchOpen('#apps?sel=a', n.loc, n.hist, () => {}), 'push');
  n = fakeNav('#persist?q=a'); let applied = 0;
  assert.equal(G.searchOpen('#persist?q=abc', n.loc, n.hist, () => applied++), 'replace');
  assert.deepEqual(n.log, [['replace', '#persist?q=abc']]); assert.equal(applied, 1, 'no event comes from replaceState');
});
test('back button: the address already there changes nothing', () => {
  const n = fakeNav('#listeners?q=3000');
  assert.equal(G.searchOpen('#listeners?q=3000', n.loc, n.hist, () => assert.fail()), 'none');
  assert.deepEqual(n.log, []);
});
test('every result address is one the allow-lists accept: it parses back to the same page and writes the same string', () => {
  for (const q of ['3000', 'chrome', '142.250', 'helper', 'snapshots', 'apfs']) for (const g of G.searchAll(q, data).groups) for (const it of g.items) {
    const s = G.parseHash(it.hash);
    assert.ok(['apps', 'listeners', 'network', 'history', 'persist', 'guide'].includes(s.view), it.hash);
    assert.ok(it.hash.length <= 2048);
  }
});
