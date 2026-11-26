// Logic tests for the URL-hash state in web/index.html. The page has no build step and no module
// exports, so the test lifts the block between the `url-state` markers out of the file and runs it
// on its own: the block must stay free of DOM and global access (it takes `location` and `history`
// as arguments) for that to work. Layout and real Back/Forward are not covered here.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../../web/index.html', import.meta.url), 'utf8');
const m = html.match(/\/\* url-state:begin \*\/([\s\S]*?)\/\* url-state:end \*\//);
assert.ok(m, 'url-state block not found in web/index.html');
const L = new Function(m[1] + '\nreturn { parseHash, writeHash, historyMode, toHashState, patchFromHash, selPatch, syncHash, crumbParts, PAGE_SORTS };')();

const base = { view: 'apps', filter: 'all', sel: null, q: '', sorts: {}, dd: {} };
const fakeEnv = (hash) => {
  const calls = [];
  const loc = { hash };
  const hist = {
    pushState: (_s, _t, u) => { calls.push(['push', u]); loc.hash = u; },
    replaceState: (_s, _t, u) => { calls.push(['replace', u]); loc.hash = u; },
  };
  return { loc, hist, calls };
};

test('empty, unknown and malformed hashes fall back to Overview', () => {
  for (const h of ['', '#', '#nonsense', '#nonsense?filter=review', '#?sel=x', '#%E0%A4%A', '#__proto__']) {
    assert.equal(L.parseHash(h).view, 'overview', h);
  }
});

test('page, filter, search, sort and selection round-trip', () => {
  const s = { view: 'apps', filter: 'review', sel: 'dev:/Applications/Foo.app', q: 'foo', sorts: { apps: { col: 'procs', dir: 'desc' } }, dd: { 'apps-label': 'unknown', 'apps-sig': 'apple' } };
  assert.deepEqual(L.parseHash(L.writeHash(s)), s);
});

test('values containing & = # ? and non-ASCII survive encoding', () => {
  const nasty = 'a&b=c#d?e é/f%g+h';
  const h = L.writeHash({ ...base, sel: nasty, q: nasty });
  assert.equal(h.slice(1).split('#').length, 1, 'no raw # after the first');
  const back = L.parseHash(h);
  assert.equal(back.sel, nasty);
  assert.equal(back.q, nasty);
});

test('unknown keys and bad values are ignored', () => {
  const p = L.parseHash('#apps?bogus=1&filter=nope&sort=apps.app.sideways,zzz.x.asc,apps.app&sel=');
  assert.equal(p.filter, 'all');
  assert.equal(p.sel, null);
  assert.deepEqual(p.sorts, {});
  assert.equal(p.bogus, undefined);
});

test('a filter value is only valid on the page that owns it', () => {
  assert.equal(L.parseHash('#apps?filter=exposed').filter, 'all');
  assert.equal(L.parseHash('#listeners?filter=exposed').filter, 'exposed');
  assert.equal(L.parseHash('#overview?filter=review').filter, 'all');
});

test('pre-filtered links: host, port, exposed and failed', () => {
  assert.equal(L.parseHash('#network?host=203.0.113.44').q, '203.0.113.44');
  assert.equal(L.parseHash('#listeners?port=4747').q, '4747');
  assert.equal(L.parseHash('#listeners?port=abc').q, '');
  assert.equal(L.parseHash('#listeners?exposed=1').filter, 'exposed');
  assert.equal(L.parseHash('#priv?failed=1').dd['auth-result'], 'failed');
  assert.equal(L.parseHash('#apps?failed=1').dd['auth-result'], undefined);
});

test('sorts keep only keys that belong to the page', () => {
  const p = L.parseHash('#priv?sort=sudo.time.asc,auth.result.desc,apps.app.asc');
  assert.deepEqual(p.sorts, { sudo: { col: 'time', dir: 'asc' }, auth: { col: 'result', dir: 'desc' } });
  const w = L.writeHash({ ...base, view: 'priv', sorts: { sudo: { col: 'time', dir: 'asc' }, apps: { col: 'app', dir: 'asc' } } });
  assert.ok(!w.includes('apps.app'));
});

test('defaults are omitted so the URL stays short', () => {
  assert.equal(L.writeHash(base), '#apps');
  assert.equal(L.writeHash({ ...base, view: 'overview' }), '#overview');
});

test('selection exists only on Apps', () => {
  assert.equal(L.writeHash({ ...base, view: 'network', sel: 'x' }), '#network');
  assert.equal(L.parseHash('#network?sel=x').sel, null);
});

test('a hash over 2 KB drops the selection first, then filters, and never throws', () => {
  const big = 'x'.repeat(2100);
  const withSel = L.writeHash({ ...base, filter: 'review', sel: big, q: 'ok' });
  assert.ok(withSel.length <= 2048);
  assert.ok(!withSel.includes('sel='));
  assert.ok(withSel.includes('filter=review') && withSel.includes('q=ok'));
  const sort = { apps: { col: 'procs', dir: 'desc' } };
  const allBig = L.writeHash({ ...base, filter: 'review', sel: big, q: big, sorts: sort });
  assert.equal(allBig, '#apps?sort=apps.procs.desc', 'selection, then filters go; the sort stays');
  assert.equal(L.writeHash({ ...base, filter: 'review', sel: 'small', q: big, sorts: sort }), '#apps?sort=apps.procs.desc', 'a small selection is dropped before the filters');
  assert.equal(L.writeHash({ ...base, sel: 'small', sorts: sort, dd: { 'apps-sig': 'apple' } }), '#apps?sel=small&signing=apple&sort=apps.procs.desc');
  assert.doesNotThrow(() => L.parseHash('#apps?q=' + 'y'.repeat(100000)));
});

test('a lone surrogate cannot make writeHash throw', () => {
  assert.doesNotThrow(() => L.writeHash({ ...base, q: '\uD800' }));
  const sort = { apps: { col: 'procs', dir: 'desc' } };
  assert.equal(L.writeHash({ ...base, sel: 'a\uD800', q: 'ok', sorts: sort, filter: 'review' }), '#apps?filter=review&q=ok&sort=apps.procs.desc', 'only the bad value is dropped');
  assert.equal(L.writeHash({ ...base, q: '\uD800', sel: 'fine' }), '#apps?sel=fine');
});

test('the write token never appears in a written hash and is ignored on read', () => {
  const h = L.writeHash({ ...base, token: 'secret123', writeToken: 'secret123' });
  assert.ok(!h.includes('secret123'));
  const p = L.parseHash('#apps?token=secret123');
  assert.ok(!JSON.stringify(p).includes('secret123'));
});

test('historyMode: page and selection changes push; filter, sort and search replace', () => {
  assert.equal(L.historyMode('#apps', '#network'), 'push');
  assert.equal(L.historyMode('#apps', '#apps?sel=a'), 'push');
  assert.equal(L.historyMode('#apps?sel=a', '#apps?sel=b'), 'push');
  assert.equal(L.historyMode('#apps', '#apps?filter=review'), 'replace');
  assert.equal(L.historyMode('#apps?filter=review', '#apps?filter=review&q=a'), 'replace');
  assert.equal(L.historyMode('#apps', '#apps'), 'none');
});

test('syncHash writes a filter change with replaceState, not a new history entry', () => {
  const { loc, hist, calls } = fakeEnv('#apps');
  L.syncHash({ view: 'apps', filter: 'review', sel: null, q: '', sorts: {}, fil: {} }, loc, hist);
  assert.deepEqual(calls, [['replace', '#apps?filter=review']]);
});

test('syncHash pushes on page change and skips when nothing changed', () => {
  const { loc, hist, calls } = fakeEnv('#apps');
  const S = { view: 'network', filter: 'all', nfilter: 'all', sel: null, q: '', sorts: {}, fil: {} };
  L.syncHash(S, loc, hist);
  L.syncHash(S, loc, hist);
  assert.deepEqual(calls, [['push', '#network']]);
});

test('syncHash survives a history API that throws', () => {
  const loc = { hash: '' };
  const hist = { pushState() { throw new Error('rate limited'); }, replaceState() { throw new Error('rate limited'); } };
  assert.doesNotThrow(() => L.syncHash({ view: 'apps', filter: 'all', sel: null, q: '', sorts: {}, fil: {} }, loc, hist));
});

test('toHashState and patchFromHash map the page-specific S fields', () => {
  const S = { view: 'listeners', filter: 'all', lfilter: 'exposed', pfilter: 'all', nfilter: 'all', sel: null, q: '22', sorts: { listen: { col: 'port', dir: 'asc' } }, fil: {} };
  const st = L.toHashState(S);
  assert.equal(st.filter, 'exposed');
  const patch = L.patchFromHash(L.parseHash(L.writeHash(st)), S);
  assert.equal(patch.view, 'listeners');
  assert.equal(patch.lfilter, 'exposed');
  assert.equal(patch.q, '22');
  assert.deepEqual(patch.sorts, { listen: { col: 'port', dir: 'asc' } });
});

test('patchFromHash keeps other pages sorts and the other fil entries', () => {
  const S = { view: 'apps', sorts: { net: { col: 'app', dir: 'asc' }, apps: { col: 'app', dir: 'asc' } }, fil: { 'apps-sig': 'apple', 'auth-result': 'ok' } };
  const patch = L.patchFromHash(L.parseHash('#priv?failed=1'), S);
  assert.deepEqual(patch.sorts.net, { col: 'app', dir: 'asc' });
  assert.deepEqual(patch.sorts.apps, { col: 'app', dir: 'asc' });
  assert.equal(patch.fil['auth-result'], 'failed');
  assert.equal(patch.fil['apps-sig'], 'apple');
});

test('crumbParts: Monitor Lizard > page > selected item, each a link', () => {
  const parts = L.crumbParts({ ...base, sel: 'dev:/x/Foo.app' }, 'Foo');
  assert.deepEqual(parts.map((p) => p.text), ['Monitor Lizard', 'Apps', 'Foo']);
  assert.equal(parts[0].href, '#overview');
  assert.equal(parts[1].href, '#apps');
  assert.equal(L.parseHash(parts[2].href).sel, 'dev:/x/Foo.app');
  assert.deepEqual(L.crumbParts({ ...base, view: 'overview' }, null).map((p) => p.text), ['Monitor Lizard', 'Overview']);
});

test('the page defines no global named history, which would replace window.history', () => {
  assert.ok(!/^(async\s+)?function\s+(history|location)\s*\(/m.test(html));
  assert.ok(!/^(const|let|var)\s+(history|location)\b/m.test(html));
});

const POISON = ['__proto__', 'constructor', 'toString', 'hasOwnProperty', 'valueOf', 'prototype', 'isPrototypeOf'];

test('Object.prototype names are never a page, filter, sort key, column or direction', () => {
  for (const w of POISON) {
    assert.equal(L.parseHash('#' + w).view, 'overview', 'page ' + w);
    assert.equal(L.parseHash('#' + w + '?filter=review').view, 'overview');
    for (const page of ['apps', 'listeners', 'persist', 'network', 'priv', 'history']) {
      const p = L.parseHash('#' + page + '?filter=' + w + '&label=' + w + '&signing=' + w + '&proto=' + w + '&result=' + w);
      assert.equal(p.filter, 'all', page + ' filter ' + w);
      assert.deepEqual(p.dd, {}, page + ' dropdown ' + w);
      for (const key of Object.keys(L.PAGE_SORTS[page])) {
        for (const bad of [key + '.' + w + '.asc', w + '.' + L.PAGE_SORTS[page][key][0] + '.asc', key + '.' + L.PAGE_SORTS[page][key][0] + '.' + w]) {
          assert.deepEqual(L.parseHash('#' + page + '?sort=' + bad).sorts, {}, page + ' sort ' + bad);
        }
      }
      assert.doesNotThrow(() => L.parseHash('#' + page + '?' + w + '=' + w + '&' + w + '&sort=' + w));
    }
    assert.doesNotThrow(() => L.writeHash({ view: w, filter: w, sel: w, q: w, sorts: { [w]: { col: w, dir: w } }, dd: { [w]: w } }));
    assert.equal(L.writeHash({ view: 'apps', filter: w, sorts: { apps: { col: w, dir: 'asc' } }, dd: { 'apps-label': w } }), '#apps');
  }
});

test('the exact crash link: apps sort on __proto__ is dropped', () => {
  assert.deepEqual(L.parseHash('#apps?sort=apps.__proto__.asc').sorts, {});
});

test('sort allow-list matches the sortHeader calls in the page', () => {
  const found = {};
  for (const mm of html.matchAll(/sortHeader\('(\w+)', \[([^\]]*)\]/g)) found[mm[1]] = [...mm[2].matchAll(/sort: '(\w+)'/g)].map((x) => x[1]);
  const listed = {};
  for (const cols of Object.values(L.PAGE_SORTS)) Object.assign(listed, cols);
  assert.deepEqual(Object.keys(found).sort(), Object.keys(listed).sort());
  for (const k of Object.keys(found)) assert.deepEqual([...listed[k]].sort(), found[k].sort(), k);
});

test('sortRows reads getters by own property only', () => {
  const src = html.match(/function sortRows[\s\S]*?\n}\n/)[0];
  const S = { sorts: { apps: { col: '__proto__', dir: 'asc' } } };
  const sortRows = new Function('S', 'isEmpty', src + '\nreturn sortRows;')(S, (v) => v == null || v === '');
  const rows = [3, 1, 2];
  assert.deepEqual(sortRows(rows, 'apps', { app: (r) => r }), rows);
  for (const w of POISON) { S.sorts.apps.col = w; assert.doesNotThrow(() => sortRows(rows, 'apps', { app: (r) => r })); }
  S.sorts.apps.col = 'app';
  assert.deepEqual(sortRows(rows, 'apps', { app: (r) => r }), [1, 2, 3]);
});

test('dropdown filters round-trip with allow-listed keys and values', () => {
  const s = { ...base, dd: { 'apps-label': 'suspicious', 'apps-sig': 'unsigned' } };
  const h = L.writeHash(s);
  assert.equal(h, '#apps?label=suspicious&signing=unsigned');
  assert.deepEqual(L.parseHash(h).dd, s.dd);
  assert.equal(L.writeHash({ ...base, dd: { 'apps-label': 'all' } }), '#apps');
  assert.deepEqual(L.parseHash('#apps?label=evil&signing=apple').dd, { 'apps-sig': 'apple' });
  assert.deepEqual(L.parseHash('#listeners?proto=UDP').dd, { 'listen-proto': 'UDP' });
  assert.deepEqual(L.parseHash('#apps?proto=UDP').dd, {});
  assert.equal(L.writeHash({ view: 'priv', dd: { 'auth-result': 'failed' } }), '#priv?result=failed');
  assert.equal(L.writeHash({ view: 'network', dd: { 'apps-label': 'trusted' } }), '#network');
});

test('dropdown filters travel from S to the address and back to S', () => {
  const S = { view: 'apps', filter: 'review', sel: null, q: '', sorts: {}, fil: { 'apps-label': 'unknown', 'apps-sig': 'all', 'listen-proto': 'TCP' } };
  const h = L.writeHash(L.toHashState(S));
  assert.equal(h, '#apps?filter=review&label=unknown');
  const fresh = { view: 'overview', sorts: {}, fil: {} };
  const patch = L.patchFromHash(L.parseHash(h), fresh);
  assert.equal(patch.fil['apps-label'], 'unknown');
  assert.equal(patch.fil['apps-sig'], 'all');
});

const SNAP = { groups: [{ key: 'a' }] };

test('selPatch: a missing selection from the address clears with replace and a notice', () => {
  const p = L.selPatch({ view: 'apps', sel: 'gone', selFromUrl: true }, SNAP);
  assert.equal(p.sel, null);
  assert.equal(p.hashReplace, true);
  assert.match(p.toast, /no longer running/);
});

test('selPatch: an app that quits during polling clears with replace and no notice', () => {
  const p = L.selPatch({ view: 'apps', sel: 'gone', selFromUrl: false }, SNAP);
  assert.equal(p.sel, null);
  assert.equal(p.hashReplace, true);
  assert.equal('toast' in p, false);
});

test('selPatch: a found selection stops counting as from the address; no snapshot means no change', () => {
  assert.deepEqual(L.selPatch({ view: 'apps', sel: 'a', selFromUrl: true }, SNAP), { selFromUrl: false });
  assert.equal(L.selPatch({ view: 'apps', sel: 'a', selFromUrl: false }, SNAP), null);
  assert.equal(L.selPatch({ view: 'apps', sel: 'a', selFromUrl: true }, null), null);
  assert.equal(L.selPatch({ view: 'network', sel: 'x', selFromUrl: true }, SNAP), null);
});

test('syncHash with forceReplace replaces even when the selection changed', () => {
  const { loc, hist, calls } = fakeEnv('#apps?sel=gone');
  L.syncHash({ view: 'apps', filter: 'all', sel: null, q: '', sorts: {}, fil: {} }, loc, hist, true);
  assert.deepEqual(calls, [['replace', '#apps']]);
});

test('fuzz: parseHash, writeHash and the apply path never throw and settle in one step', () => {
  const pool = [...POISON, 'apps', 'listeners', 'priv', 'network', 'history', 'persist', 'overview', 'filter', 'sel', 'q', 'sort', 'label', 'signing', 'proto', 'result', 'failed', 'host', 'port', 'exposed',
    'all', 'review', 'asc', 'desc', 'app', 'procs', 'apps.app.asc', 'sudo.time.desc', '1', '', '%', '%E0%A4', '%ZZ', '#', '?', '&', '=', ',', '.', '..', 'é', '\uD800', ' '];
  let seed = 12345;
  const rnd = (n) => { seed = (seed * 1103515245 + 12345) & 0x7fffffff; return seed % n; };
  for (let i = 0; i < 3000; i++) {
    const parts = Array.from({ length: 1 + rnd(14) }, () => pool[rnd(pool.length)]);
    const hash = '#' + parts.join(['', '?', '&', '=', '.'][rnd(5)]);
    let p, w, again;
    assert.doesNotThrow(() => {
      p = L.parseHash(hash);
      const S = { view: 'apps', sorts: {}, fil: {}, filter: 'all', lfilter: 'all', pfilter: 'all', nfilter: 'all', sel: null, q: '' };
      Object.assign(S, L.patchFromHash(p, S));
      w = L.writeHash(L.toHashState(S));
      again = L.writeHash(L.toHashState(Object.assign(S, L.patchFromHash(L.parseHash(w), S))));
    }, hash);
    assert.equal(again, w, 'writing is stable for ' + JSON.stringify(hash));
    assert.ok(w.length <= 2048);
  }
});
