// Pure logic for the Persistence page: filter counts, hidden Apple items, the change list and the reasons an item is flagged.
// The test lifts the `persistence-page` block with the `page-shell` block it reads (plural).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { html } from './page.mjs';
const block = (name) => {
  const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'));
  assert.ok(m, name + ' block not found in the dashboard scripts (web/*.js)');
  return m[1];
};
const P = new Function(block('page-shell') + block('persistence-page') + '\nreturn { persMatches, persVisible, persCounts, persChangeRows, persWhy, persSigning, persSubtitle };')();

const item = (label, extra = {}) => ({ path: '/p/' + label + '.plist', label, scope: 'user', kind: 'LaunchAgent', program: '/bin/' + label, sig: 'signed', is_new: false, expected: false, was_expected: false, keep_alive: false, program_missing: false, unreadable: null, ...extra });
const items = [item('sync', { is_new: true, sig: 'unsigned', keep_alive: true }), item('keystone'), item('daemon', { scope: 'system' }), item('apple1', { scope: 'system', sig: 'apple' }), item('apple2', { sig: 'apple' })];

test('Apple items are hidden by default, shown on request, and always reached by a search', () => {
  assert.deepEqual(P.persVisible(items, false, '').map((r) => r.label), ['sync', 'keystone', 'daemon']);
  const newApple = item('newapple', { sig: 'apple', is_new: true });
  assert.ok(P.persVisible([...items, newApple], false, '').some((r) => r.label === 'newapple'), 'a new Apple item is never hidden');
  assert.equal(P.persVisible(items, true, '').length, 5);
  assert.equal(P.persVisible(items, false, 'apple').length, 5);
});
test('each filter count equals the rows under that filter', () => {
  const pool = P.persVisible(items, false, ''), c = P.persCounts(pool);
  assert.deepEqual(c, { all: 3, new: 1, user: 2, system: 1, unsigned: 1 });
  for (const id of Object.keys(c)) assert.equal(pool.filter((r) => P.persMatches(r, id)).length, c[id]);
});
test('the title count equals the New filter rows', () => {
  assert.equal(items.filter((r) => P.persMatches(r, 'new')).length, 1);
});
test('the change list keeps added, changed, restored and removed, and drops items marked expected', () => {
  const changes = [{ path: '/p/sync.plist', label: 'sync', kind: 'added', at: 5 }, { path: '/p/keystone.plist', label: 'keystone', kind: 'changed', at: 4 }, { path: '/p/gone.plist', label: 'gone', kind: 'removed', at: 3 }, { path: '/p/back.plist', label: 'back', kind: 'restored', at: 2 }];
  const rows = P.persChangeRows(changes, items);
  assert.deepEqual(rows.map((r) => r.chip), ['ADDED', 'CHANGED', 'REMOVED', 'RESTORED']);
  const marked = P.persChangeRows(changes, [item('sync', { expected: true })]);
  assert.deepEqual(marked.map((r) => r.label), ['keystone', 'gone', 'back']);
  assert.deepEqual(P.persChangeRows([], items), []);
});
test('an unknown change kind is shown, not hidden', () => {
  assert.equal(P.persChangeRows([{ path: '/x', label: 'x', kind: 'odd', at: 1 }], [])[0].chip, 'ODD');
});
test('the reasons come only from the item fields', () => {
  assert.deepEqual(P.persWhy(items[0]), ['It appeared after your baseline was set.', 'Its program is not signed.', 'It is restarted if it stops.']);
  assert.deepEqual(P.persWhy(items[1]), ['Nothing is flagged for this item.']);
  assert.ok(P.persWhy(item('x', { is_new: true, was_expected: true }))[0].startsWith('You marked an earlier version'));
});
test('an unreadable plist and a missing program give their reason', () => {
  const w = P.persWhy(item('y', { unreadable: 'permission denied', program_missing: true }));
  assert.ok(w.includes('Its plist could not be read: permission denied'));
  assert.ok(w.includes('The program it starts is not on disk.'));
});
test('signing text names the team, Apple, or says there is none', () => {
  assert.equal(P.persSigning(item('a', { signing: { team: 'T1' } })), 'Signed, team T1');
  assert.equal(P.persSigning(item('b', { sig: 'apple' })), 'Signed by Apple');
  assert.equal(P.persSigning(item('c', { sig: 'unsigned' })), 'Unsigned');
  assert.equal(P.persSigning(item('d', { sig: null, trust_note: 'program missing' })), 'program missing');
});
test('the subtitle counts yours and the system ones', () => {
  assert.equal(P.persSubtitle(items), '5 items run at login or boot: 3 user, 2 system');
});
