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
const L = new Function(m[1] + '\nreturn { parseHash, writeHash, historyMode, hashStateOf, patchOf, syncHash, crumbParts };')();

const base = { view: 'apps', filter: 'all', sel: null, q: '', sorts: {}, failed: false };
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
  const s = { view: 'apps', filter: 'review', sel: 'dev:/Applications/Foo.app', q: 'foo', sorts: { apps: { col: 'procs', dir: 'desc' } }, failed: false };
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
  assert.equal(L.parseHash('#priv?failed=1').failed, true);
  assert.equal(L.parseHash('#apps?failed=1').failed, false);
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
  const allBig = L.writeHash({ ...base, filter: 'review', sel: big, q: big });
  assert.equal(allBig, '#apps');
  assert.doesNotThrow(() => L.parseHash('#apps?q=' + 'y'.repeat(100000)));
});

test('a lone surrogate cannot make writeHash throw', () => {
  assert.doesNotThrow(() => L.writeHash({ ...base, q: '\uD800' }));
  assert.equal(L.writeHash({ ...base, q: '\uD800' }), '#apps');
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

test('hashStateOf and patchOf map the page-specific S fields', () => {
  const S = { view: 'listeners', filter: 'all', lfilter: 'exposed', pfilter: 'all', nfilter: 'all', sel: null, q: '22', sorts: { listen: { col: 'port', dir: 'asc' } }, fil: {} };
  const st = L.hashStateOf(S);
  assert.equal(st.filter, 'exposed');
  const patch = L.patchOf(L.parseHash(L.writeHash(st)), S);
  assert.equal(patch.view, 'listeners');
  assert.equal(patch.lfilter, 'exposed');
  assert.equal(patch.q, '22');
  assert.deepEqual(patch.sorts, { listen: { col: 'port', dir: 'asc' } });
});

test('patchOf keeps other pages sorts and the other fil entries', () => {
  const S = { view: 'apps', sorts: { net: { col: 'app', dir: 'asc' }, apps: { col: 'app', dir: 'asc' } }, fil: { 'apps-sig': 'apple', 'auth-result': 'ok' } };
  const patch = L.patchOf(L.parseHash('#priv?failed=1'), S);
  assert.deepEqual(patch.sorts.net, { col: 'app', dir: 'asc' });
  assert.deepEqual(patch.sorts.apps, { col: 'app', dir: 'asc' });
  assert.equal(patch.fil['auth-result'], 'failed');
  assert.equal(patch.fil['apps-sig'], 'apple');
});

test('crumbParts: Sys Monitor > page > selected item, each a link', () => {
  const parts = L.crumbParts({ ...base, sel: 'dev:/x/Foo.app' }, 'Foo');
  assert.deepEqual(parts.map((p) => p.text), ['Sys Monitor', 'Apps', 'Foo']);
  assert.equal(parts[0].href, '#overview');
  assert.equal(parts[1].href, '#apps');
  assert.equal(L.parseHash(parts[2].href).sel, 'dev:/x/Foo.app');
  assert.deepEqual(L.crumbParts({ ...base, view: 'overview' }, null).map((p) => p.text), ['Sys Monitor', 'Overview']);
});
