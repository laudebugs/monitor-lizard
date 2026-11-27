// Logic tests for the page shell in the dashboard scripts (web/*.js): sidebar groups and counts, the verdict sentence that titles
// each page, relative time and the export note. Like the other web tests, they lift marked blocks out of the page
// and run them on their own, so those blocks stay free of DOM and storage access. Layout is checked by hand in a
// browser.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html } from './page.mjs';
const block = (name) => { const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/')); assert.ok(m, name + ' block not found in the dashboard scripts (web/*.js)'); return m[1]; };
// page-shell reads TITLES, which the url-state block defines.
const P = new Function(block('url-state') + block('page-shell') + '\nreturn { TITLES, NAV, NAV_GROUPS, countText, plural, navBadges, verdictTitle, relTime, countReview, titleReady, TITLE_NEEDS, relatedLinks, writeHash, parseHash };')();
const X = new Function(block('export') + '\nreturn { exportNote, csvCell };')();

test('every NAV entry has a group that is listed, and every group has an entry', () => {
  for (const { id, group } of P.NAV) assert.ok(P.NAV_GROUPS.includes(group), id + ' has no known group: ' + group);
  for (const g of P.NAV_GROUPS) assert.ok(P.NAV.some((n) => n.group === g), 'empty group ' + g);
});

test('NAV ids are the pages the address knows, once each', () => {
  const ids = P.NAV.map((n) => n.id);
  assert.equal(new Set(ids).size, ids.length);
  assert.deepEqual([...ids].sort(), Object.keys(P.TITLES).sort());
});

test('sidebar groups hold the pages the design names, in order', () => {
  const by = (g) => P.NAV.filter((n) => n.group === g).map((n) => n.label);
  assert.deepEqual(by('Security'), ['Overview', 'Apps', 'Network', 'Listeners', 'Privilege', 'Persistence', 'History']);
  assert.deepEqual(by('Machine'), ['Performance', 'Storage']);
  assert.deepEqual(by('Help'), ['Guide']);
});

test('a count above 99 reads 99+', () => {
  assert.equal(P.countText(99), '99');
  assert.equal(P.countText(100), '99+');
  assert.equal(P.navBadges({ review: 250 }).apps.text, '99+');
  assert.equal(P.navBadges({ review: 250 }).apps.n, 250, 'the label keeps the real number');
  assert.equal(P.navBadges({ review: 120 }).apps.label, '99+ apps need review, 120 in total', 'the name starts with what is shown');
  assert.equal(P.navBadges({ review: 100 }).apps.label, '99+ apps need review, 100 in total');
  assert.equal(P.navBadges({ review: 99 }).apps.label, '99 apps need review');
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
  assert.equal(P.verdictTitle('overview', { attention: 2 }), '2 things need a decision');
  assert.equal(P.verdictTitle('overview', { attention: 0 }), 'Nothing needs a decision');
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
  assert.match(html, /const failedAuth = \(\) => S\.priv\.filter\(\(e\) => !e\.ok\)\.length/, 'the Privilege count is every failed request, as the Failed filter shows');
  assert.match(html, /failed: failedAuth\(\)/, 'the badge uses it');
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
  const fn = html.slice(html.indexOf('function timeCell'), html.indexOf('const crumbNodes'));
  assert.match(fn, /h\('time'/);
  assert.match(fn, /datetime: new Date\(sec \* 1000\)\.toISOString\(\), title: abs/);
  assert.match(fn, /class: 'sr-only' \}, abs \+ ', ' \+ relTime\(sec, now, true\)/, 'a screen reader gets the exact time and the long relative text');
  assert.match(fn, /'aria-hidden': 'true' \}, relTime\(sec, now\)/, 'the short text is not read twice');
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

test('the page title is not a static heading any more: the shell draws it', () => {
  const body = html.slice(html.indexOf('<body'), html.indexOf('<script', html.indexOf('<body')));
  assert.ok(!/<h1/.test(body));
  assert.match(body, /id="skip"/);
  assert.ok(body.indexOf('id="crumb"') < body.indexOf('id="q"'), 'breadcrumb comes before search in the focus order');
});

test('relative time, long form for a screen reader', () => {
  const now = 1_000_000;
  assert.equal(P.relTime(now - 300, now, true), '5 minutes ago');
  assert.equal(P.relTime(now - 60, now, true), '1 minute ago');
  assert.equal(P.relTime(now - 7200, now, true), '2 hours ago');
  assert.equal(P.relTime(now - 86400, now, true), '1 day ago');
  assert.equal(P.relTime(now - 5, now, true), 'just now');
});

// ---- one count of apps that need review
test('the review count is one function over all groups, with or without connections', () => {
  const statusOf = (g) => g.s;
  const groups = [{ s: 'review', conns: [] }, { s: 'review', conns: [1] }, { s: 'idle', conns: [] }, { s: 'approved', conns: [] }];
  assert.equal(P.countReview(groups, statusOf), 2);
  assert.equal(P.countReview([], statusOf), 0);
  const apps = html.slice(html.indexOf('function apps()'), html.indexOf('function drawer('));
  assert.match(apps, /review: reviewCount\(\)/, 'the Apps title');
  assert.match(html, /const badges = navBadges\(\{ review: reviewCount\(\)/, 'the badge');
  assert.match(html, /const review = reviewCount\(\);/, 'the Overview tile');
  assert.ok(!/count\('review'\)/.test(apps));
});

// ---- a title never says "nothing" before the data it counts has loaded
test('every verdict title names the data it waits for, and waits for it', () => {
  for (const v of ['overview', 'apps', 'priv', 'listeners', 'persist', 'network', 'storage']) assert.ok(P.TITLE_NEEDS[v] && P.TITLE_NEEDS[v].length, v);
  assert.deepEqual(P.TITLE_NEEDS.network, ['snap', 'hist'], 'new hosts need the host history');
  assert.ok(!P.titleReady('network', { snap: true, hist: false }));
  assert.ok(P.titleReady('network', { snap: true, hist: true }));
  assert.ok(!P.titleReady('overview', { snap: true, priv: true, pers: false }));
  assert.ok(!P.titleReady('priv', {}) && !P.titleReady('persist', null));
  assert.ok(!P.titleReady('storage', { snap: true }), 'the Storage title waits for a scan result');
  assert.ok(P.titleReady('storage', { stor: true }));
  assert.ok(P.titleReady('history', {}), 'a page with no verdict has nothing to wait for');
});

test('the page sets each load flag when its data arrives', () => {
  for (const [flag, loader] of [['privOk', 'loadPriv'], ['persOk', 'loadPers'], ['histOk', 'loadHist']]) {
    const m = html.match(new RegExp('const ' + loader + ' = .*'));
    assert.ok(m && m[0].includes(flag + ': true'), loader + ' sets ' + flag);
  }
  assert.match(html, /const verdict = \(view, facts\) => verdictTitle\(view, titleReady\(view, loadedNow\(\)\) \? facts : null\)/);
  assert.match(html, /hist: S\.histOk/);
});

test('a page title is drawn through verdict(), never verdictTitle() with its own guard', () => {
  const pages = html.slice(html.indexOf('function overview()'), html.indexOf('function perfOverviewTab_'));
  for (const m of pages.matchAll(/title: (\w+)\('(\w+)'/g)) if (['overview', 'apps', 'priv', 'listeners', 'persist', 'network', 'storage'].includes(m[2])) assert.equal(m[1], 'verdict', m[2]);
});

// ---- related links
test('related links: only those that open rows, built by writeHash', () => {
  const l = P.relatedLinks({ hosts: 3, sockets: 0, startup: 1, newHosts: 0 }, 'Google Chrome', '/Applications/Google Chrome.app');
  assert.deepEqual(l.map((x) => x.text), ['3 remote hosts on Network', '1 startup item on Persistence']);
  assert.equal(l[0].hash, P.writeHash({ view: 'network', q: 'Google Chrome' }));
  assert.equal(P.parseHash(l[0].hash).view, 'network');
  assert.equal(P.parseHash(l[0].hash).q, 'Google Chrome', 'the allow-listed search text carries the app');
  assert.equal(P.parseHash(l[1].hash).q, '/Applications/Google Chrome.app');
  assert.deepEqual(P.relatedLinks({ hosts: 0, sockets: 0, startup: 0, newHosts: 0 }, 'x', 'y'), []);
  assert.deepEqual(P.relatedLinks(null, 'x', 'y'), []);
  assert.equal(P.relatedLinks({ sockets: 1 }, 'x', 'y')[0].text, '1 listening socket on Listeners');
});

test('related link hashes survive odd names', () => {
  for (const name of ['a&b=c', 'x#y', 'é ü', '?sort=a', 'a'.repeat(300)]) {
    const l = P.relatedLinks({ hosts: 1 }, name, name)[0];
    assert.equal(P.parseHash(l.hash).q, name);
  }
});

// ---- counts in sentences go through plural()
test('page subtitles never join a count to a literal noun by hand', () => {
  const hand = (e) => /(?:\.length|\w)\s*\+\s*' [a-z]/.test(e.replace(/plural\([^)]*\)/g, ''));
  const subs = [...html.matchAll(/subtitle: (.*?), (?:filters|body|inspector|actions):/g)].map((m) => m[1]);
  assert.ok(subs.length >= 5);
  for (const e of subs) assert.ok(!hand(e), e);
  assert.ok(hand("g.length + ' apps, ' + n"), 'the check catches the old Overview subtitle');
  assert.ok(!hand("plural(g.length, 'app', 'apps') + ' right now'"));
});

// ---- every return of every renderer is the shell
const RENDERERS = ['overview', 'apps', 'priv', 'listeners', 'persistence', 'network', 'historyView', 'performanceView_', 'guide'];
// The `return` statements in a function's own body (not in functions nested inside it), each with the text after it.
function topReturns(src, name) {
  const start = src.indexOf('function ' + name + '(');
  if (start < 0) throw new Error('no function ' + name);
  const text = src.slice(start).replace(/'(?:\\.|[^'\\\n])*'|"(?:\\.|[^"\\\n])*"/g, "''").replace(/\/\/.*$/gm, '');
  let i = text.indexOf('{', text.indexOf(')')), depth = 0;
  const out = [];
  for (let j = i; j < text.length; j++) {
    const ch = text[j];
    if (ch === '{') depth++;
    else if (ch === '}') { depth--; if (!depth) break; }
    else if (depth === 1 && text.startsWith('return', j) && /\W/.test(text[j - 1]) && /[\s(\[{]/.test(text[j + 6])) out.push(text.slice(j + 6, j + 40).trim());
  }
  return out;
}
const bad = (src) => RENDERERS.flatMap((n) => topReturns(src, n).filter((r) => !r.startsWith('pageShell(')).map((r) => n + ': ' + r));

test('every return in every renderer is the shell', () => {
  for (const n of RENDERERS) assert.ok(topReturns(html, n).length >= 1, n + ' has a return');
  assert.equal(topReturns(html, 'guide').length, 5, 'the Guide has five return paths');
  assert.deepEqual(bad(html), []);
});

test('mutation: one bare return in a copy of the page is caught', () => {
  const i = html.indexOf('function guide()'), j = html.indexOf('return pageShell(', i);
  const mutated = html.slice(0, j) + 'return [' + html.slice(j + 'return pageShell('.length);
  assert.equal(bad(mutated).length, 1);
  assert.match(bad(mutated)[0], /^guide: /);
  const k = html.indexOf('return pageShell(', html.indexOf('function priv()')), mutated2 = html.slice(0, k) + 'return h(' + html.slice(k + 'return pageShell('.length);
  assert.match(bad(mutated2)[0], /^priv: /);
});

test('performance puts its live toggle in the title actions', () => {
  assert.match(html.slice(html.indexOf('function performanceView_'), html.indexOf('const SAFETY_TAG')), /actions: \[/);
});

test('the Overview tile says what it counts and links to the failed filter', () => {
  assert.match(html, /tile\('Failed sudo', failed, 'sudo only, last 2 hours', \(\) => rec\(\{ view: 'priv', fil: Object\.assign\(\{\}, S\.fil, \{ 'auth-result': 'failed' \}\)/);
});
