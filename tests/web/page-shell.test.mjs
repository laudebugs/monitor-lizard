// Logic tests for the page shell in web/index.html: sidebar groups and counts, the verdict sentence that titles
// each page, relative time and the export note. Like the other web tests, they lift marked blocks out of the page
// and run them on their own, so those blocks stay free of DOM and storage access. Layout is checked by hand in a
// browser.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../../web/index.html', import.meta.url), 'utf8');
const block = (name) => { const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/')); assert.ok(m, name + ' block not found in web/index.html'); return m[1]; };
// page-shell reads TITLES, which the url-state block defines.
const P = new Function(block('url-state') + block('page-shell') + '\nreturn { TITLES, NAV, NAV_GROUPS, countText, plural, navBadges, verdictTitle, relTime };')();
const X = new Function(block('export') + '\nreturn { exportNote, csvCell };')();

test('every NAV entry has a group that is listed, and every group has an entry', () => {
  for (const [id, , , group] of P.NAV) assert.ok(P.NAV_GROUPS.includes(group), id + ' has no known group: ' + group);
  for (const g of P.NAV_GROUPS) assert.ok(P.NAV.some((n) => n[3] === g), 'empty group ' + g);
});

test('NAV ids are the pages the address knows, once each', () => {
  const ids = P.NAV.map((n) => n[0]);
  assert.equal(new Set(ids).size, ids.length);
  assert.deepEqual([...ids].sort(), Object.keys(P.TITLES).sort());
});

test('sidebar groups hold the pages the design names, in order', () => {
  const by = (g) => P.NAV.filter((n) => n[3] === g).map((n) => n[1]);
  assert.deepEqual(by('Security'), ['Overview', 'Apps', 'Network', 'Listeners', 'Privilege', 'Persistence', 'History']);
  assert.deepEqual(by('Machine'), ['Performance']);
  assert.deepEqual(by('Help'), ['Guide']);
});

test('a count above 99 reads 99+', () => {
  assert.equal(P.countText(99), '99');
  assert.equal(P.countText(100), '99+');
  assert.equal(P.navBadges({ review: 250 }).apps.text, '99+');
  assert.equal(P.navBadges({ review: 250 }).apps.n, 250, 'the label keeps the real number');
});

test('badges appear only for a positive count, with a label that names the number', () => {
  assert.deepEqual(P.navBadges({ review: 0, failed: 0, fresh: 0 }), {});
  assert.deepEqual(P.navBadges(null), {});
  assert.deepEqual(P.navBadges({ review: NaN, failed: -1, fresh: undefined }), {});
  const b = P.navBadges({ review: 3, failed: 1, fresh: 2 });
  assert.deepEqual(Object.keys(b), ['apps', 'priv', 'persist']);
  assert.equal(b.apps.label, '3 apps need review');
  assert.equal(b.priv.label, '1 failed authorization request');
  assert.equal(b.persist.label, '2 new startup items');
  assert.equal(b.priv.tone, 'bad');
});

test('titles say what the page found, singular and plural', () => {
  assert.equal(P.verdictTitle('apps', { review: 3 }), '3 apps need review');
  assert.equal(P.verdictTitle('apps', { review: 1 }), '1 app needs review');
  assert.equal(P.verdictTitle('apps', { review: 0 }), 'Nothing needs review');
  assert.equal(P.verdictTitle('overview', { attention: 2 }), '2 items need your attention');
  assert.equal(P.verdictTitle('overview', { attention: 0 }), 'Nothing needs your attention');
  assert.equal(P.verdictTitle('priv', { failed: 1 }), '1 failed authorization request');
  assert.equal(P.verdictTitle('priv', { failed: 0 }), 'No failed authorization requests');
  assert.equal(P.verdictTitle('listeners', { exposed: 2 }), '2 listening sockets are open to your network');
  assert.equal(P.verdictTitle('listeners', { exposed: 1 }), '1 listening socket is open to your network');
  assert.equal(P.verdictTitle('listeners', { exposed: 0 }), 'No listening socket is open to your network');
  assert.equal(P.verdictTitle('persist', { fresh: 4 }), '4 new startup items');
  assert.equal(P.verdictTitle('persist', { fresh: 0 }), 'No new startup items');
  assert.equal(P.verdictTitle('network', { newHosts: 1 }), '1 new host in the last hour');
  assert.equal(P.verdictTitle('network', { newHosts: 0 }), 'No new hosts in the last hour');
});

test('a page with no verdict, or no data yet, is titled by its name', () => {
  for (const v of ['history', 'performance', 'guide']) assert.equal(P.verdictTitle(v, { review: 5 }), P.TITLES[v]);
  assert.equal(P.verdictTitle('apps', null), 'Apps');
  assert.equal(P.verdictTitle('apps', {}), 'Apps');
  assert.equal(P.verdictTitle('apps', { review: NaN }), 'Apps');
  assert.equal(P.verdictTitle('priv', { failed: -2 }), 'Privilege requests');
});

test('an unknown page name gives an empty title, and never reaches Object.prototype', () => {
  for (const v of ['nope', '__proto__', 'constructor', 'toString']) assert.equal(P.verdictTitle(v, { review: 1 }), '', v);
});

test('every page has a title, and the badges count the rows their link opens', () => {
  for (const v of Object.keys(P.TITLES)) assert.ok(P.verdictTitle(v, null).length > 0, v);
  assert.match(html, /const badgeHash = .*filter: 'review'.*'auth-result': 'failed'.*filter: 'new'/);
  assert.match(html, /failed: S\.priv\.filter\(\(e\) => !e\.ok\)\.length/, 'the Privilege count is every failed request, as the Failed filter shows');
});

test('relative time: short form for each span', () => {
  const now = 1_000_000;
  assert.equal(P.relTime(now, now), 'just now');
  assert.equal(P.relTime(now - 59, now), 'just now');
  assert.equal(P.relTime(now - 60, now), '1m ago');
  assert.equal(P.relTime(now - 3599, now), '59m ago');
  assert.equal(P.relTime(now - 3600, now), '1h ago');
  assert.equal(P.relTime(now - 86399, now), '23h ago');
  assert.equal(P.relTime(now - 86400, now), '1d ago');
  assert.equal(P.relTime(now - 86400 * 29, now), '29d ago');
  assert.equal(P.relTime(now - 86400 * 30, now), '1mo ago');
  assert.equal(P.relTime(now - 86400 * 365, now), '1y ago');
});

test('relative time: a time in the future (clock skew) and missing values never read as negative or NaN', () => {
  assert.equal(P.relTime(2_000_000, 1_000_000), 'just now');
  for (const bad of [null, undefined, NaN, Infinity, '12', {}]) assert.equal(P.relTime(bad, 1_000_000), '-');
});

test('the time cell keeps the absolute time in its title', () => {
  assert.match(html, /const timeCell = .*title: new Date\(sec \* 1000\)\.toLocaleString\(\).*relTime\(sec, Date\.now\(\) \/ 1000\)/);
});

test('export note: one short line, with the full text behind a button', () => {
  const n = X.exportNote({ info: 'app names and ports' });
  assert.ok(!/\n/.test(n.line));
  assert.ok(n.line.length <= 60, n.line);
  assert.ok(n.full.length > n.line.length);
  assert.match(html, /'aria-expanded': String\(open\), 'aria-controls': 'xinfo-' \+ key/, 'the disclosure is a button that names its panel');
  assert.match(html, /id: 'xinfo-' \+ key, hidden: !open/, 'the full text is always in the page');
  assert.match(html, /const more = h\('button'/);
});

test('export note: the full text keeps the facts a reader must not lose', () => {
  const n = X.exportNote({ info: 'the commands run with sudo', warn: 'Commands can contain secrets in their arguments.' });
  assert.match(n.full, /stays on this Mac until you share it/);
  assert.match(n.full, /Commands can contain secrets in their arguments\./);
  assert.match(n.full, /leading ' so spreadsheets do not run it/);
  assert.match(n.full, /the commands run with sudo/);
  assert.match(n.line, /secrets/, 'a table with a warning says so on the one line too');
  assert.match(n.line, /Mac until you share it/);
});

test('export note: a table with no warning has none, and a missing descriptor still gives text', () => {
  assert.doesNotMatch(X.exportNote({ info: 'x' }).line, /secrets/);
  assert.match(X.exportNote(undefined).full, /the rows shown/);
});

test('every table the page draws has a descriptor the note can use', () => {
  const keys = [...html.matchAll(/sortHeader\('(\w+)'/g)].map((m) => m[1]);
  const meta = html.match(/const TABLE_META = \{([\s\S]*?)\n\};/)[1];
  for (const k of new Set(keys)) assert.match(meta, new RegExp('\\b' + k + ': \\{ file:'), k);
});

test('every page renderer returns the shell', () => {
  for (const fn of ['overview', 'apps', 'priv', 'listeners', 'persistence', 'network', 'historyView', 'performanceView_', 'guide']) {
    const start = html.indexOf('function ' + fn + '(');
    assert.ok(start > 0, fn);
    const next = html.indexOf('\nfunction ', start + 10), body = html.slice(start, next > 0 ? next : undefined);
    assert.match(body, /pageShell\(/, fn + ' draws through pageShell');
  }
});

test('the page title is not a static heading any more: the shell draws it', () => {
  const body = html.slice(html.indexOf('<body'), html.indexOf('<script', html.indexOf('<body')));
  assert.ok(!/<h1/.test(body));
  assert.match(body, /id="skip"/);
  assert.ok(body.indexOf('id="crumb"') < body.indexOf('id="q"'), 'breadcrumb comes before search in the focus order');
});
