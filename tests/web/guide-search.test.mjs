// Logic tests for the Guide's search in web/index.html. Like the URL-state tests, this lifts the block between
// the `guide-search` markers out of the page, so the block must stay free of DOM access. What the page looks like
// and how the keyboard works are checked by hand in a browser.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const html = readFileSync(new URL('../../web/index.html', import.meta.url), 'utf8');
const m = html.match(/\/\* guide-search:begin \*\/([\s\S]*?)\/\* guide-search:end \*\//);
assert.ok(m, 'guide-search block not found in web/index.html');
const G = new Function(m[1] + '\nreturn { guideWords, guideRows, guideTopics, guideGroups, topicLabel, guidePick };')();

const cmd = (id, topic, title, command, description, tags = []) => ({ id, topic, title, command, description, tags, safety: 'read-only' });
const commands = [
  cmd('sm-snapshot', 'monitor-lizard', 'Print the grouped snapshot', 'monitor-lizard snapshot', 'Lists every process grouped by app.', ['groups']),
  cmd('tmutil-list', 'storage', 'List local Time Machine snapshots', 'tmutil listlocalsnapshots /', 'Lists the dates of local snapshots.', ['snapshot', 'time machine']),
  cmd('tmutil-delete', 'storage', 'Delete one local snapshot', 'tmutil deletelocalsnapshots <date>', 'Deletes a Time Machine snapshot.', ['snapshot', 'time machine', 'delete']),
  cmd('docker-df', 'storage', 'Docker disk use', 'docker system df', 'Shows the space used by images and containers.', ['docker', 'images']),
  cmd('docker-prune', 'storage', 'Remove unused Docker data', 'docker system prune', 'Deletes stopped containers and dangling images.', ['docker', 'prune']),
  cmd('sm-sudo', 'monitor-lizard', 'Show recent sudo events', 'monitor-lizard sudo-log', 'Prints events. A snapshot is not involved.', ['sudo']),
];
const terms = [
  { id: 'apfs', term: 'APFS', meaning: 'The disk format. A local snapshot lives here.', tags: ['volume'] },
  { id: 'listener', term: 'Listener', meaning: 'A process waiting on a port.', tags: ['port'] },
];
const ids = (rows) => rows.map((r) => r.id);

test('every word must match: "time machine" needs both words', () => {
  assert.deepEqual(ids(G.guideRows(commands, 'time machine', 'all')), ['tmutil-list', 'tmutil-delete']);
  assert.deepEqual(ids(G.guideRows(commands, 'time docker', 'all')), []);
});

test('title, command, description and tags are all searched', () => {
  assert.deepEqual(ids(G.guideRows(commands, 'listlocalsnapshots', 'all')), ['tmutil-list']);
  assert.deepEqual(ids(G.guideRows(commands, 'dangling', 'all')), ['docker-prune']);
  assert.deepEqual(ids(G.guideRows(commands, 'prune', 'all')), ['docker-prune']);
  assert.deepEqual(ids(G.guideRows(commands, 'DOCKER', 'all')), ['docker-df', 'docker-prune']);
});

test('"snapshots" shows the Time Machine entries first, then weaker matches', () => {
  const r = ids(G.guideRows(commands, 'snapshots', 'all'));
  assert.deepEqual(r.slice(0, 2), ['tmutil-list', 'tmutil-delete']);
  assert.ok(!r.includes('docker-df'));
});

test('a title hit outranks a description hit, and ties keep table order', () => {
  const r = ids(G.guideRows(commands, 'snapshot', 'all'));
  assert.ok(r.indexOf('sm-snapshot') < r.indexOf('sm-sudo'));
  assert.deepEqual(G.guideRows(commands, '  ', 'all'), commands, 'blank search is no search');
});

test('an empty search lists everything grouped by topic in table order', () => {
  const groups = G.guideGroups(G.guideRows(commands, '', 'all'));
  assert.deepEqual(groups.map(([t, r]) => [t, r.length]), [['monitor-lizard', 2], ['storage', 4]]);
  assert.deepEqual(G.guideTopics(commands), ['monitor-lizard', 'storage']);
});

test('the topic filter narrows before the search runs', () => {
  assert.deepEqual(ids(G.guideRows(commands, '', 'storage')).length, 4);
  assert.deepEqual(ids(G.guideRows(commands, 'snapshot', 'monitor-lizard')), ['sm-snapshot', 'sm-sudo']);
  assert.deepEqual(G.guideRows(commands, 'docker', 'monitor-lizard'), []);
});

test('a selected entry the filters hide gives way to the first result; none gives null', () => {
  const rows = G.guideRows(commands, 'docker', 'all');
  assert.equal(G.guidePick(rows, 'docker-prune').id, 'docker-prune');
  assert.equal(G.guidePick(rows, 'tmutil-list').id, 'docker-df');
  assert.equal(G.guidePick(rows, null).id, 'docker-df');
  assert.equal(G.guidePick([], 'x'), null);
});

test('no results gives an empty list, and glossary terms search by term, meaning and tag', () => {
  assert.deepEqual(G.guideRows(commands, 'zzzz', 'all'), []);
  assert.deepEqual(ids(G.guideRows(terms, 'apfs', 'all')), ['apfs']);
  assert.deepEqual(ids(G.guideRows(terms, 'waiting', 'all')), ['listener']);
  assert.deepEqual(ids(G.guideRows(terms, 'port', 'all')), ['listener']);
});

test('topic labels come from the topic text, so a renamed product needs no edit here', () => {
  assert.equal(G.topicLabel('storage'), 'Storage');
  assert.equal(G.topicLabel('developer'), 'Developer tools');
  assert.equal(G.topicLabel('monitor-lizard'), 'Monitor Lizard');
});

test('the Guide code never names the binary', () => {
  const view = html.slice(html.indexOf('function guideDetail'), html.indexOf('const NAV = ['));
  assert.ok(view && !/sys-monitor|monitor-lizard/.test(view + m[1].replace(/\/\/.*$/gm, '')));
});
