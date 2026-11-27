// Pure logic for the Network page: filters and counts, Group by, the inspector facts and the address copy text.
// The test lifts the `network-page` block with the `page-shell` block it reads (plural).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { html } from './page.mjs';
const block = (name) => {
  const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'));
  assert.ok(m, name + ' block not found in the dashboard scripts (web/*.js)');
  return m[1];
};
const P = new Function(block('page-shell') + block('network-page') + '\nreturn { netMatches, netCounts, netGroup, netOrder, netFacts, netFirstSeenText, netCopyAddress, netSubtitle };')();

const app = (key, label, approved = true) => ({ key, label, approved });
const row = (g, remote, ports, sockets, first, extra = {}) => ({ key: g.key + '|' + remote, g, apps: [g], remote, hosts: [remote], ports, byPort: Object.fromEntries(ports.map((p) => [p, Math.floor(sockets / ports.length)])), scope: 'public', sockets, first_seen: first, expected: false, unsigned: false, isNew: false, ...extra });
const notion = app('n', 'Notion'), slack = app('s', 'Slack'), syncd = app('d', 'syncd', false);
const rows = [
  row(notion, '203.0.113.44', [443], 3, 990, { isNew: true }),
  row(slack, '203.0.113.44', [443], 9, 100),
  row(syncd, '198.51.100.7', [8443], 2, 50, { unsigned: true }),
  row(slack, '192.168.1.20', [8009], 1, null, { scope: 'lan' }),
];

test('filter counts match what each filter shows', () => {
  const c = P.netCounts(rows);
  assert.deepEqual(c, { all: 4, new: 1, public: 3, lan: 1, unsigned: 1 });
  for (const id of Object.keys(c)) assert.equal(rows.filter((r) => P.netMatches(r, id)).length, c[id]);
});
test('app grouping leaves the rows alone', () => {
  assert.equal(P.netGroup(rows, 'app'), rows);
});
test('host grouping lists every app that reached the address', () => {
  const g = P.netGroup(rows, 'host'), shared = g.find((r) => r.remote === '203.0.113.44');
  assert.equal(g.length, 3);
  assert.deepEqual(shared.apps.map((a) => a.label), ['Notion', 'Slack']);
  assert.equal(shared.sockets, 12);
  assert.equal(shared.first_seen, 100);
  assert.equal(shared.isNew, true);
});
test('port grouping sums the sockets of every host behind a port', () => {
  const g = P.netGroup(rows, 'port'), p443 = g.find((r) => r.ports[0] === 443);
  assert.equal(p443.sockets, 12);
  assert.deepEqual(p443.hosts, ['203.0.113.44']);
  assert.equal(g.length, 3);
});
test('grouping keeps the filter result: a merged row is new when any of its rows is', () => {
  assert.equal(P.netGroup(rows.filter((r) => P.netMatches(r, 'new')), 'host').length, 1);
});
test('a merged row with no known first-seen time keeps null, not zero', () => {
  const g = P.netGroup([rows[3]], 'host');
  assert.equal(g[0].first_seen, null);
});
test('the default order is newest first contact, then the busiest', () => {
  assert.deepEqual(P.netOrder(rows).map((r) => r.remote), ['203.0.113.44', '203.0.113.44', '198.51.100.7', '192.168.1.20']);
  assert.equal(P.netOrder(rows)[0].g.label, 'Notion');
});
test('inspector facts name the apps, the reason and the first contact', () => {
  const f = P.netFacts(rows.slice(0, 2), 1000);
  assert.equal(f.host, '203.0.113.44');
  assert.equal(f.first, 100);
  assert.equal(f.sockets, 12);
  assert.equal(f.isNew, true);
  assert.match(f.why, /Notion, Slack is approved, but had not contacted this address before/);
});
test('an unsigned app is the stated reason when the host is not new', () => {
  const f = P.netFacts([rows[2]], 1000);
  assert.match(f.why, /syncd is not signed/);
  assert.equal(P.netFacts([rows[1]], 1000).why, 'Nothing about this address is flagged.');
});
test('a host seen before history existed reads before monitoring, once history has loaded', () => {
  assert.equal(P.netFirstSeenText(null, true), 'before monitoring');
  assert.equal(P.netFirstSeenText(null, false), 'not known yet');
  assert.equal(P.netFirstSeenText(5, true), null);
});
test('an IPv6 address is copied with brackets and the port', () => {
  assert.equal(P.netCopyAddress('2607:f8b0::2003', 443), '[2607:f8b0::2003]:443');
  assert.equal(P.netCopyAddress('203.0.113.44', 443), '203.0.113.44:443');
  assert.equal(P.netCopyAddress('203.0.113.44'), '203.0.113.44');
});
test('the subtitle counts sockets, hosts and apps', () => {
  assert.equal(P.netSubtitle(rows, 15), '15 outbound sockets to 3 hosts across 3 apps');
});
