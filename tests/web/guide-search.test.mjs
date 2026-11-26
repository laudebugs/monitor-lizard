// Logic tests for the Guide's search and Copy flow in web/index.html. Like the URL-state tests, this lifts the blocks
// between the `guide-search` and `guide-copy` markers out of the page, so the search block must stay free of DOM
// access. What the page looks like and how the keyboard works are checked by hand in a browser. The fixtures are the
// real table (tests/fixtures/guide-table.json, checked against the Rust table by `cargo test`) and the results both
// the page and `help` must give (tests/fixtures/guide-search.json).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../../web/index.html', import.meta.url), 'utf8');
const block = (name) => { const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/')); assert.ok(m, name + ' block not found in web/index.html'); return m[1]; };
const search = block('guide-search');
const G = new Function(search + '\nreturn { guideWords, guideRanked, guideSearch, guideTopics, guideGroups, topicLabel, guidePick, guideSig, guideSummary, GUIDE_CUTOFF };')();
const { copyFlow } = new Function(block('guide-copy') + '\nreturn { copyFlow };')();
const json = (f) => JSON.parse(readFileSync(new URL('../fixtures/' + f, import.meta.url), 'utf8'));
const table = json('guide-table.json'), cases = json('guide-search.json');
const ids = (rows) => rows.map((r) => r.id);
const rows = (items, q, topic = 'all', all = false) => G.guideSearch(items, q, topic, all).shown;

const cmd = (id, topic, title, command, description, tags = []) => ({ id, topic, title, command, description, tags, safety: 'read-only' });
const commands = [
  cmd('sm-snapshot', 'monitor-lizard', 'Print the grouped snapshot', 'monitor-lizard snapshot', 'Lists every process grouped by app.', ['groups']),
  cmd('tmutil-list', 'storage', 'List local Time Machine snapshots', 'tmutil listlocalsnapshots /', 'Lists the dates of local snapshots.', ['snapshot', 'time machine']),
  cmd('tmutil-delete', 'storage', 'Delete one local snapshot', 'tmutil deletelocalsnapshots <date>', 'Deletes a Time Machine snapshot.', ['snapshot', 'time machine', 'delete']),
  cmd('docker-df', 'developer', 'Docker disk use', 'docker system df', 'Shows the space used by images and containers.', ['docker', 'images']),
  cmd('docker-prune', 'developer', 'Remove unused Docker data', 'docker system prune', 'Deletes stopped containers and dangling images.', ['docker', 'prune']),
];
const terms = [
  { id: 'apfs', term: 'APFS', meaning: 'The disk format. A local snapshot lives here.', tags: ['volume'] },
  { id: 'listener', term: 'Listener', meaning: 'A process waiting on a port.', tags: ['port'] },
];

test('every word must match: "time machine" needs both words', () => {
  assert.deepEqual(ids(rows(commands, 'time machine', 'all', true)), ['tmutil-list', 'tmutil-delete']);
  assert.deepEqual(rows(commands, 'time docker'), []);
});

test('title, command, description and tags are all searched, ignoring case', () => {
  assert.deepEqual(ids(rows(commands, 'listlocalsnapshots')), ['tmutil-list']);
  assert.deepEqual(ids(rows(commands, 'dangling')), ['docker-prune']);
  assert.deepEqual(ids(rows(commands, 'prune')), ['docker-prune']);
  assert.deepEqual(ids(rows(commands, 'DOCKER')), ['docker-df', 'docker-prune']);
});

test('a plural finds the singular, and a title hit outranks a description hit', () => {
  assert.deepEqual(G.guideWords('Snapshots'), ['snapshot']);
  assert.deepEqual(G.guideWords('dns'), ['dns'], 'short words keep their s');
  const r = ids(G.guideRanked(commands, 'snapshot').map((x) => x.e));
  assert.deepEqual(r.slice(0, 2), ['tmutil-list', 'tmutil-delete']);
  assert.ok(r.indexOf('tmutil-list') < r.indexOf('sm-snapshot'), 'tags and command beat a title-only hit');
});

test('words under three characters match whole words in title, command and tags only', () => {
  assert.deepEqual(ids(rows(commands, 'df')), ['docker-df']);
  assert.deepEqual(rows(commands, 'a'), [], '"a" is not a word in any title, command or tag, though "a" is in descriptions');
  assert.deepEqual(rows(commands, 'ck'), [], 'not inside "docker"');
  assert.deepEqual(rows(terms, 'is'), []);
});

test('a lone punctuation or space query is an empty query', () => {
  for (const q of ['', ' ', '/', '-', ' / - ', '...']) {
    assert.deepEqual(G.guideWords(q), [], JSON.stringify(q));
    assert.equal(rows(commands, q).length, commands.length, JSON.stringify(q));
  }
  assert.deepEqual(G.guideWords('/ docker'), ['docker']);
});

test('an empty search lists everything grouped by topic in table order', () => {
  const groups = G.guideGroups(rows(commands, ''));
  assert.deepEqual(groups.map(([t, r]) => [t, r.length]), [['monitor-lizard', 1], ['storage', 2], ['developer', 2]]);
  assert.deepEqual(G.guideTopics(commands), ['monitor-lizard', 'storage', 'developer']);
});

test('the topic filter narrows, and the weak cutoff does not move with it', () => {
  assert.equal(rows(commands, '', 'storage').length, 2);
  assert.deepEqual(rows(commands, 'docker', 'storage'), []);
  const real = table.commands;
  assert.deepEqual(ids(rows(real, 'snapshots', 'monitor-lizard')), [], 'the weak own-snapshot entry stays hidden under its own chip');
  assert.deepEqual(ids(G.guideSearch(real, 'snapshots', 'monitor-lizard', false).hidden), ['sm-snapshot', 'sm-json']);
});

test('weaker matches are hidden behind a count and shown on request', () => {
  const s = G.guideSearch(table.commands, 'snapshots', 'all', false);
  assert.deepEqual(ids(s.shown), ['tmutil-list', 'tmutil-delete']);
  assert.deepEqual(ids(s.hidden), ['sm-snapshot', 'sm-json']);
  const all = G.guideSearch(table.commands, 'snapshots', 'all', true);
  assert.deepEqual(ids(all.shown), ['tmutil-list', 'tmutil-delete', 'sm-snapshot', 'sm-json']);
  assert.deepEqual(all.hidden, []);
  assert.equal(G.guideSummary(2, 2, 'command'), '2 commands, 2 weaker hidden');
  assert.equal(G.guideSummary(1, 0, 'term'), '1 term');
  assert.equal(G.guideSummary(0, 0, 'command'), 'Nothing matches that');
});

test('the real table: the results in the shared fixture', () => {
  assert.ok(cases.length >= 15);
  for (const c of cases) {
    const ranked = G.guideRanked(table.commands, c.q);
    assert.deepEqual(ids(ranked.map((x) => x.e)), c.commands, 'commands for ' + JSON.stringify(c.q));
    assert.deepEqual(ids(G.guideRanked(table.terms, c.q).map((x) => x.e)), c.terms, 'terms for ' + JSON.stringify(c.q));
    assert.equal(G.guideSearch(table.commands, c.q, 'all', false).shown.length, c.shown, 'shown for ' + JSON.stringify(c.q));
  }
});

test('the real table: the entry a user expects comes first', () => {
  const first = { snapshots: 'tmutil-list', snapshot: 'tmutil-list', docker: 'docker-df', prune: 'docker-prune', sudo: 'sm-sudo-log', simulators: 'simctl-delete-unavailable', 'free space': 'df-root', df: 'df-root', delete: 'tmutil-delete', brew: 'brew-cleanup-n', 'Time Machine': 'tmutil-list', privilege: 'sm-privilege' };
  for (const [q, id] of Object.entries(first)) assert.equal(rows(table.commands, q)[0].id, id, q);
  assert.deepEqual(ids(rows(table.commands, 'snapshots')), ['tmutil-list', 'tmutil-delete']);
});

test('the real table has the three topics, and Developer tools has the Docker, Xcode and Homebrew entries', () => {
  assert.deepEqual(G.guideTopics(table.commands), ['monitor-lizard', 'storage', 'developer']);
  assert.deepEqual(table.commands.filter((c) => c.topic === 'developer').map((c) => c.id).sort(), ['brew-cleanup-n', 'docker-df', 'docker-prune', 'simctl-delete-unavailable']);
});

test('a selected entry the filters hide gives way to the first result; none gives null', () => {
  const r = rows(commands, 'docker');
  assert.equal(G.guidePick(r, 'docker-prune').id, 'docker-prune');
  assert.equal(G.guidePick(r, 'tmutil-list').id, 'docker-df');
  assert.equal(G.guidePick(r, null).id, 'docker-df');
  assert.equal(G.guidePick([], 'x'), null);
});

test('no results gives an empty list, and glossary terms search by term, meaning and tag', () => {
  assert.deepEqual(rows(commands, 'zzzz'), []);
  assert.deepEqual(ids(rows(terms, 'apfs')), ['apfs']);
  assert.deepEqual(ids(rows(terms, 'waiting')), ['listener']);
  assert.deepEqual(ids(rows(terms, 'port')), ['listener']);
});

test('topic labels come from the topic text, so a renamed product needs no edit here', () => {
  assert.equal(G.topicLabel('storage'), 'Storage');
  assert.equal(G.topicLabel('developer'), 'Developer tools');
  assert.equal(G.topicLabel('monitor-lizard'), 'Monitor Lizard');
});

test('the Guide code never names the binary', () => {
  const view = html.slice(html.indexOf('function guideDetail'), html.indexOf('const NAV = ['));
  assert.ok(view && !/sys-monitor|monitor-lizard/.test(view + search.replace(/\/\/.*$/gm, '')));
});

test('a poll changes nothing the Guide is built from, so the content is kept', () => {
  const s = { q: 'docker', gtab: 'commands', gtopic: 'all', gsel: 'docker-df', gall: null, gcopy: null, guide: { state: 'ready', data: {} } };
  const sig = G.guideSig(s);
  assert.equal(G.guideSig({ ...s, snap: { groups: [] }, priv: [1], pers: [{ is_new: true }], seen: [2] }), sig);
  for (const change of [{ q: 'x' }, { gtab: 'glossary' }, { gtopic: 'storage' }, { gsel: 'docker-prune' }, { gall: 'docker' }, { gcopy: { id: 'a', state: 'copied' } }, { guide: { state: 'error', data: null } }]) {
    assert.notEqual(G.guideSig({ ...s, ...change }), sig, JSON.stringify(change));
  }
  assert.match(html, /guideSig\(S\) !== S\.gsig/, 'render() compares the signature before rebuilding');
  assert.ok(!/guideHolds/.test(html), 'polls are never held back');
});

const flow = (write, byId) => { const log = []; return { log, deps: { write, show: (s) => log.push('show:' + s), byId, select: (el) => log.push('select:' + el.id) } }; };
const el = (id) => ({ id, focus() { this.focused = true; } });

test('Copy: success shows "copied" and focuses the button', async () => {
  const button = el('guide-copy'), code = el('guide-cmd'), { log, deps } = flow(async () => {}, (id) => ({ 'guide-copy': button, 'guide-cmd': code })[id]);
  assert.equal(await copyFlow('docker system df', deps), 'copied');
  assert.deepEqual(log, ['show:copied']);
  assert.ok(button.focused);
});

test('Copy: a blocked clipboard, or none at all, selects the command', async () => {
  for (const write of [async () => { throw new DOMException('no', 'NotAllowedError'); }, () => { throw new TypeError('clipboard is undefined'); }]) {
    const button = el('guide-copy'), code = el('guide-cmd'), { log, deps } = flow(write, (id) => ({ 'guide-copy': button, 'guide-cmd': code })[id]);
    assert.equal(await copyFlow('x', deps), 'manual');
    assert.deepEqual(log, ['show:manual', 'select:guide-cmd']);
  }
});

test('Copy: when the user has left the page by the time the clipboard answers, nothing throws', async () => {
  for (const write of [async () => {}, async () => { throw new Error('blocked'); }]) {
    const { log, deps } = flow(write, () => null);
    await assert.doesNotReject(() => copyFlow('x', deps));
    assert.ok(!log.some((l) => l.startsWith('select')));
  }
});

test('choosing an entry, a tab or a topic clears the Copy result', () => {
  for (const key of ['gtab: id', 'gtopic: id', 'gsel: e.id']) assert.match(html, new RegExp('\\{ ' + key + ', gcopy: null'), key);
});

test('the live region has its own id, outside anything render() rebuilds, and the static ids are unique', () => {
  const start = html.indexOf('<body'), body = html.slice(start, html.indexOf('<script', start));
  const idsInMarkup = [...body.matchAll(/\sid="([^"]+)"/g)].map((x) => x[1]);
  assert.equal(new Set(idsInMarkup).size, idsInMarkup.length, 'duplicate id in ' + idsInMarkup);
  assert.ok(idsInMarkup.includes('announcer'));
  assert.match(html, /function announce\(msg\) \{ const el = \$\('announcer'\)/);
});
