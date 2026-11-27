// One key shape for both the live-connection map and the history join, so the delimiter and
// field order can't drift between the two call sites that build it and the one that looks it up.
const netKey = (groupKey, remote) => groupKey + '|' + remote;
function network() {
  const q = S.q.toLowerCase(), f = S.nfilter, now = S.snap.at;
  const byKey = new Map();
  for (const g of S.snap.groups) for (const c of g.conns) {
    // loopback isn't "the network" (apps.js's own net(g) uses the same test); nothing to show here.
    if (c.direction !== 'outbound' || !c.remote_addr || c.scope === 'loopback') continue;
    const key = netKey(g.key, c.remote_addr);
    let r = byKey.get(key);
    if (!r) { r = { g, remote: c.remote_addr, ports: new Set(), scope: c.scope, sockets: 0 }; byKey.set(key, r); }
    r.ports.add(c.remote_port);
    r.sockets++;
  }
  const seenBy = new Map(S.seen.map((s) => [netKey(s.group_key, s.remote), s.first_seen]));
  const all = [...byKey.values()].map((r) => Object.assign(r, { ports: [...r.ports].sort((a, b) => a - b), first_seen: seenBy.get(netKey(r.g.key, r.remote)) ?? null }));
  // Newest-contacted hosts first, so what you'd most want to check sits at the top by default;
  // baked into the base order (as listeners() does for its reach tiers) so it survives filtering
  // and is what sortRows falls back to once no column header is actively sorted.
  all.sort((a, b) => (b.first_seen ?? -1) - (a.first_seen ?? -1) || b.sockets - a.sockets);
  const hostExpected = (r) => (r.g.expected_hosts || []).includes(r.remote);
  const isNew = (r) => r.g.approved && r.first_seen != null && now - r.first_seen < 3600 && !hostExpected(r);
  const inFilter = (r, id) => id === 'all' || (id === 'new' ? isNew(r) : r.scope === id);
  const list = all.filter((r) => inFilter(r, f) && (!q || r.g.label.toLowerCase().includes(q) || r.remote.toLowerCase().includes(q)));
  const shown = sortRows(list, 'net', { app: (r) => r.g.label, remote: (r) => r.remote, scope: (r) => r.scope, sockets: (r) => r.sockets, seen: (r) => r.first_seen });
  const pg = paginate(shown, 'net');
  const chips = [['all', 'All'], ['public', 'Public'], ['lan', 'LAN'], ['new', 'New']].map(([id, l]) => h('button', { class: 'chip ' + (f === id ? 'on' : ''), onClick: () => rec({ nfilter: id, page: {} }) }, l + ' ', all.filter((r) => inFilter(r, id)).length));
  const cols = 'grid-template-columns:minmax(120px,1.1fr) minmax(80px,1.3fr) minmax(80px,1fr) 80px 90px 110px 190px';
  const table = h('section', { class: 'card tbl', style: 'min-width:0' },
    sortHeader('net', [{ label: 'App', sort: 'app' }, { label: 'Remote', sort: 'remote' }, { label: 'Ports' }, { label: 'Scope', sort: 'scope' }, { label: 'Sockets', sort: 'sockets', cls: 'num' }, { label: 'First seen', sort: 'seen', title: 'Sort by first seen, newest first' }, { label: 'Expected', id: 'actions', action: true }], cols, 'ell'),
    pg.slice.map((r) => h('div', { class: 'tr ell', style: cols },
      h('span', nameAttrs(r.g.label), r.g.label), h('span', { class: 'mut small', title: r.remote }, r.remote),
      h('span', { class: 'mut small' }, r.ports.join(', ')), h('span', {}, h('span', { class: 'tag' }, r.scope)),
      h('span', { class: 'num' }, r.sockets), timeCell(r.first_seen, 'mut small'), h('span', { style: 'display:flex;align-items:center;gap:6px' }, hostExpected(r) ? expectedTag() : null, markBtn([['host', { app: r.g.key, addr: r.remote }]], hostExpected(r), r.remote + ' for ' + r.g.label)))),
    shown.length ? null : h('div', { class: 'item' }, h('p', {}, all.length ? 'No connections match the current filters. ' : 'No outbound connections right now.'), all.length ? clearBtn() : null), pg.pager);
  return pageShell({ title: verdict('network', { newHosts: all.filter(isNew).length }), subtitle: plural(all.length, 'host', 'hosts') + ' contacted by apps right now', filters: chips, body: [table,
    h('p', { class: 'mut small', style: 'margin:12px 0 0' }, 'Remote hosts are shown as IP addresses only: no hostname lookup is done, since that would tell a DNS resolver which IPs you look up. "First seen" is this app\'s own history, not the connection\'s age.')] });
}


