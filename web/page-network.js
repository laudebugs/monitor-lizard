// One key shape for both the live-connection map and the history join, so the delimiter and
// field order can't drift between the two call sites that build it and the one that looks it up.
const netKey = (groupKey, remote) => groupKey + '|' + remote;
/* network-page:begin */
const NET_GROUPS = ['app', 'host', 'port'], NET_NEW_SECS = 3600;
// One test for every filter chip, the verdict and the inspector, so they cannot disagree.
const netMatches = (r, id) => id === 'all' || (id === 'new' ? r.isNew : id === 'unsigned' ? r.unsigned : r.scope === id);
const netCounts = (rows) => Object.fromEntries(['all', 'new', 'public', 'lan', 'unsigned'].map((id) => [id, rows.filter((r) => netMatches(r, id)).length]));
const minOrNull = (xs) => { const v = xs.filter((x) => typeof x === 'number'); return v.length ? Math.min(...v) : null; };
const uniq = (xs) => [...new Set(xs)];
// Rows are one per app and host. Host grouping merges the apps that reached one address, port grouping merges every
// host behind one port; each merged row keeps the fields of an app row so sorting and export read it the same way.
function netGroup(rows, by) {
  if (by !== 'host' && by !== 'port') return rows;
  const keyed = new Map();
  for (const r of rows) {
    const keys = by === 'host' ? [[r.remote, r.sockets]] : r.ports.map((p) => [String(p), r.byPort[p] || 0]);
    for (const [k, n] of keys) {
      let m = keyed.get(k);
      if (!m) { m = { key: by + ':' + k, g: r.g, apps: [], hosts: [], ports: [], scope: r.scope, sockets: 0, firsts: [], isNew: false, unsigned: false, expected: true, byPort: {} }; keyed.set(k, m); }
      if (!m.apps.some((g) => g.key === r.g.key)) m.apps.push(r.g);
      m.hosts.push(r.remote); m.sockets += n; m.firsts.push(r.first_seen);
      m.ports.push(...(by === 'host' ? r.ports : [Number(k)]));
      if (m.scope !== r.scope) m.scope = 'mixed';
      m.isNew = m.isNew || r.isNew; m.unsigned = m.unsigned || r.unsigned; m.expected = m.expected && r.expected;
    }
  }
  return [...keyed.values()].map((m) => { const hosts = uniq(m.hosts); return Object.assign(m, { hosts, remote: by === 'host' ? hosts[0] : hosts.join(', '), ports: uniq(m.ports).sort((a, b) => a - b), first_seen: minOrNull(m.firsts) }); });
}
// Default order: what was first contacted most recently, then the busiest.
const netOrder = (rows) => rows.slice().sort((a, b) => (b.first_seen ?? -1) - (a.first_seen ?? -1) || b.sockets - a.sockets);
// What the inspector says about one address, from the rows of every app that reached it.
function netFacts(hostRows, now) {
  const apps = [], first = minOrNull(hostRows.map((r) => r.first_seen));
  for (const r of hostRows) if (!apps.some((g) => g.key === r.g.key)) apps.push(r.g);
  const isNew = hostRows.some((r) => r.isNew), unsigned = hostRows.some((r) => r.unsigned), names = apps.map((g) => g.label).join(', ');
  const why = isNew ? names + ' is approved, but had not contacted this address before. Hosts for approved apps are flagged for one hour after first contact.'
    : unsigned ? names + ' is not signed.' : 'Nothing about this address is flagged.';
  return { host: hostRows[0].remote, apps, first, ago: first == null ? null : Math.max(0, now - first), sockets: hostRows.reduce((n, r) => n + r.sockets, 0),
    ports: uniq(hostRows.flatMap((r) => r.ports)).sort((a, b) => a - b), scope: hostRows[0].scope, isNew, unsigned, expected: hostRows.every((r) => r.expected), why };
}
// "Before monitoring" only once history has loaded, so an unloaded history is not reported as old.
const netFirstSeenText = (first, histOk) => (typeof first === 'number' ? null : histOk ? 'before monitoring' : 'not known yet');
// An IPv6 address is shown plain in the table and with brackets and the port when copied.
const netCopyAddress = (remote, port) => (remote.includes(':') ? '[' + remote + ']' : remote) + (port == null ? '' : ':' + port);
const netSubtitle = (rows, sockets) => plural(sockets, 'outbound socket', 'outbound sockets') + ' to ' + plural(uniq(rows.map((r) => r.remote)).length, 'host', 'hosts') + ' across ' + plural(uniq(rows.map((r) => r.g.key)).length, 'app', 'apps');
/* network-page:end */


const NET_GROUP_LABEL = { app: 'App', host: 'Host', port: 'Port' };
function netBaseRows() {
  const byKey = new Map(), now = S.snap.at;
  for (const g of S.snap.groups) for (const c of g.conns) {
    // loopback isn't "the network" (apps.js's own net(g) uses the same test); nothing to show here.
    if (c.direction !== 'outbound' || !c.remote_addr || c.scope === 'loopback') continue;
    const key = netKey(g.key, c.remote_addr);
    let r = byKey.get(key);
    if (!r) { r = { key, g, apps: [g], remote: c.remote_addr, hosts: [c.remote_addr], ports: new Set(), byPort: {}, scope: c.scope, sockets: 0 }; byKey.set(key, r); }
    r.ports.add(c.remote_port);
    r.byPort[c.remote_port] = (r.byPort[c.remote_port] || 0) + 1;
    r.sockets++;
  }
  const seenBy = new Map(S.seen.map((s) => [netKey(s.group_key, s.remote), s.first_seen]));
  return [...byKey.values()].map((r) => {
    const first = seenBy.get(netKey(r.g.key, r.remote)) ?? null, expected = (r.g.expected_hosts || []).includes(r.remote);
    return Object.assign(r, { ports: [...r.ports].sort((a, b) => a - b), first_seen: first, expected, unsigned: sigKind(r.g) === 'unsigned',
      isNew: !!r.g.approved && first != null && now - first < NET_NEW_SECS && !expected });
  });
}
const netSelect = (host) => rec({ nhost: S.nhost === host ? null : host });
function netInspector(f, hostRows) {
  const kv = (...pairs) => h('dl', { class: 'kv' }, pairs.filter(Boolean).map(([k, v]) => [h('dt', {}, k), h('dd', {}, v)]));
  const first = f.first == null ? netFirstSeenText(f.first, S.histOk) : timeCell(f.first);
  const tags = [h('span', { class: 'tag' }, f.scope), f.isNew ? h('span', { class: 'tag warn' }, 'new') : null, f.unsigned ? h('span', { class: 'tag bad' }, 'unsigned app') : null, f.expected ? expectedTag() : null];
  const apps = f.apps.length > 1 ? f.apps.map((g) => g.label).join(', ') : 'No other app';
  return h('aside', { class: 'drawer card', 'aria-label': 'Host detail' },
    h('div', { class: 'insp-head' }, h('div', { class: 'grow', style: 'font-size:16px;font-weight:600;word-break:break-all' }, f.host), h('button', { class: 'btn sm', type: 'button', onClick: () => rec({ nhost: null }) }, 'Close')),
    h('div', { class: 'insp-sec', style: 'display:flex;gap:6px;flex-wrap:wrap' }, tags),
    h('p', { style: 'margin:0;padding:0 16px 12px' }, f.why),
    kv(['First seen', first], ['Sockets', f.sockets + ' now'], ['Ports', f.ports.join(', ')], ['Also used by', apps], ['Apps', f.apps.map((g) => g.label + (g.approved ? ' (approved)' : ' (not approved)')).join(', ')]),
    h('div', { class: 'insp-sec' }, h('h3', { class: 'sec' }, 'Related'),
      f.apps.map((g) => h('p', { style: 'margin:4px 0' }, h('a', { href: writeHash({ view: 'apps', sel: g.key }) }, 'Open app ' + g.label))),
      h('p', { style: 'margin:4px 0' }, h('a', { href: writeHash({ view: 'persist', q: f.apps[0].label }) }, 'Startup items for ' + f.apps[0].label)),
      h('p', { style: 'margin:4px 0' }, h('a', { href: writeHash({ view: 'history', q: f.host }) }, 'History of this host'))),
    h('div', { class: 'insp-sec', style: 'display:flex;gap:8px;flex-wrap:wrap' },
      hostRows.map((r) => h('button', { class: 'btn sm ' + (r.expected ? '' : 'btn-acc'), type: 'button', onClick: () => mark([['host', { app: r.g.key, addr: r.remote }]], !r.expected, r.remote + ' for ' + r.g.label) }, r.expected ? 'Unmark ' + r.g.label : (hostRows.length > 1 ? 'Mark expected for ' + r.g.label : 'Mark host as expected'))),
      h('button', { class: 'btn sm', type: 'button', onClick: () => copyText(netCopyAddress(f.host, f.ports[0])) }, 'Copy address')));
}
function network() {
  const q = S.q.toLowerCase(), f = S.nfilter, by = NET_GROUPS.includes(S.ngroup) ? S.ngroup : 'app', now = S.snap.at;
  const base = netBaseRows();
  const counts = netCounts(base);
  const searched = base.filter((r) => !q || r.g.label.toLowerCase().includes(q) || r.remote.toLowerCase().includes(q) || r.ports.some((p) => String(p) === q));
  const list = searched.filter((r) => netMatches(r, f));
  const grouped = netOrder(netGroup(list, by));
  const shown = sortRows(grouped, 'net', { app: (r) => r.apps.map((g) => g.label).join(', '), remote: (r) => r.remote, scope: (r) => r.scope, sockets: (r) => r.sockets, seen: (r) => r.first_seen });
  const pg = paginate(shown, 'net');
  const hostRows = S.nhost ? list.filter((r) => r.remote === S.nhost) : [];
  const facts = hostRows.length ? netFacts(hostRows, now) : null;
  const chips = [['all', 'All'], ['new', 'New'], ['public', 'Public'], ['lan', 'LAN'], ['unsigned', 'Unsigned']].map(([id, l]) => h('button', { class: 'chip ' + (f === id ? 'on' : ''), 'aria-pressed': String(f === id), onClick: () => rec({ nfilter: id, page: {} }) }, l + ' \u00b7 ' + counts[id]));
  const groupBy = h('div', { class: 'seg', role: 'group', 'aria-label': 'Group by' }, NET_GROUPS.map((id) => h('button', { class: 'chip ' + (by === id ? 'on' : ''), 'aria-pressed': String(by === id), onClick: () => rec({ ngroup: id, page: {} }) }, NET_GROUP_LABEL[id])));
  const cols = 'grid-template-columns:minmax(120px,1.1fr) minmax(80px,1.3fr) minmax(80px,1fr) 80px 90px 110px 190px';
  const firstCell = (r) => (r.first_seen == null ? h('span', { class: 'mut small' }, netFirstSeenText(null, S.histOk)) : timeCell(r.first_seen, 'mut small'));
  const nameCell = (r) => {
    const label = r.apps.map((g) => g.label).join(', '), pickable = by !== 'port', on = pickable && S.nhost === r.hosts[0];
    return h('span', nameAttrs(label), pickable ? h('button', { class: 'linkbtn', type: 'button', 'aria-pressed': String(on), title: label, style: 'background:none;border:0;padding:0;text-align:left;font:inherit;color:inherit;max-width:100%;overflow:hidden;text-overflow:ellipsis', onClick: () => netSelect(r.hosts[0]) }, label) : label);
  };
  const table = h('section', { class: 'card tbl', style: 'min-width:0' },
    sortHeader('net', [{ label: 'App', sort: 'app' }, { label: 'Remote', sort: 'remote' }, { label: 'Ports' }, { label: 'Scope', sort: 'scope' }, { label: 'Sockets', sort: 'sockets', cls: 'num' }, { label: 'First seen', sort: 'seen', title: 'Sort by first seen, newest first' }, { label: 'Expected', id: 'actions', action: true }], cols, 'ell'),
    pg.slice.map((r) => h('div', { class: 'tr ell' + (by !== 'port' && S.nhost === r.hosts[0] ? ' on' : ''), style: cols },
      nameCell(r), h('span', { class: 'mut small', title: r.remote }, r.remote),
      h('span', { class: 'mut small' }, r.ports.join(', ')), h('span', {}, h('span', { class: 'tag' }, r.scope), r.isNew ? h('span', { class: 'tag warn', style: 'margin-left:4px' }, 'new') : null),
      h('span', { class: 'num' }, r.sockets), firstCell(r),
      h('span', { style: 'display:flex;align-items:center;gap:6px' }, r.expected ? expectedTag() : null, by === 'app' ? markBtn([['host', { app: r.g.key, addr: r.remote }]], r.expected, r.remote + ' for ' + r.g.label) : null))),
    shown.length ? null : h('div', { class: 'item' }, h('p', {}, base.length ? 'No connections match the current filters. ' : 'No outbound connections right now.'), base.length ? clearBtn() : null), pg.pager);
  const sockets = base.reduce((n, r) => n + r.sockets, 0);
  const foot = h('p', { class: 'mut small', style: 'margin:12px 0 0' }, 'Hosts are shown as IP addresses only. A name lookup would tell a DNS server which hosts you contact. "First seen" is the app\'s own history on this Mac, not the age of the connection.');
  return pageShell({ title: base.length || !titleReady('network', loadedNow()) ? verdict('network', { newHosts: counts.new }) : 'No outbound connections right now', subtitle: base.length ? netSubtitle(base, sockets) : 'Nothing is connected to the network right now', filters: [chips, h('span', { class: 'mut small' }, 'Group by'), groupBy],
    body: [table, foot], inspector: facts ? netInspector(facts, hostRows) : null });
}
