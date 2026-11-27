// Where a listening socket can be reached from: exposed (any machine on the network), lan (an Apple service on all interfaces), or local.
const reachOf = (r) => (r.c.exposed ? (apple(r.g) ? 'lan' : 'exposed') : 'local');
function listeners() {
  const q = S.q.toLowerCase(), f = S.lfilter;
  const rows = [];
  for (const g of S.snap.groups) for (const c of g.conns) if (c.direction === 'listen') rows.push({ g, c });
  // Exposed non-Apple first, then Apple services, then local-only; by port inside each tier.
  const tier = { exposed: 0, lan: 1, local: 2 };
  rows.sort((a, b) => tier[reachOf(a)] - tier[reachOf(b)] || a.c.local_port - b.c.local_port);
  const inFilter = (r, id) => id === 'all' || (id === 'exposed' ? r.c.exposed : !r.c.exposed);
  const proto = fv('listen-proto');
  const filtered = rows.filter((r) => inFilter(r, f) && (proto === 'all' || r.c.proto === proto) && (!q || String(r.c.local_port).includes(q) || r.g.label.toLowerCase().includes(q) || r.c.local_addr.toLowerCase().includes(q)));
  const shown = sortRows(filtered, 'listen', { port: (r) => r.c.local_port, proto: (r) => r.c.proto, bind: (r) => r.c.local_addr, app: (r) => r.g.label, pid: (r) => r.c.pid, reach: (r) => tier[reachOf(r)] });
  const pg = paginate(shown, 'listen');
  const chips = [['all', 'All'], ['exposed', 'Exposed'], ['local', 'Local only']].map(([id, l]) => h('button', { class: 'chip ' + (f === id ? 'on' : ''), onClick: () => rec({ lfilter: id, page: {} }) }, l + ' ', rows.filter((r) => inFilter(r, id)).length));
  const cols = 'grid-template-columns:80px 60px minmax(80px,1fr) minmax(80px,1.4fr) 80px 120px';
  const reach = (r) => { const k = reachOf(r); return k === 'exposed' ? h('span', { class: 'tag warn', title: 'Listening on all interfaces' }, 'EXPOSED') : k === 'lan' ? h('span', { class: 'tag', title: 'An Apple service reachable from your network' }, 'LAN') : h('span', { class: 'tag' }, 'local'); };
  const missed = S.snap.totals.listening - rows.length;
  const table = h('section', { class: 'card tbl', style: 'min-width:0' },
    sortHeader('listen', [{ label: 'Port', sort: 'port', cls: 'num' }, { label: 'Proto', sort: 'proto' }, { label: 'Bind address', sort: 'bind' }, { label: 'App', sort: 'app' }, { label: 'PID', sort: 'pid', cls: 'num' }, { label: 'Reach', sort: 'reach', title: 'Sort by reach, exposed first' }], cols, 'ell'),
    pg.slice.map((r) => h('button', { class: 'tr ell', style: cols, onClick: () => { $('q').value = ''; rec({ view: 'apps', sel: r.g.key, filter: 'all', q: '', page: {} }); } },
      h('span', { class: 'num', style: 'font-weight:500' }, r.c.local_port), h('span', { class: 'mut small' }, r.c.proto), h('span', { class: 'mut small' }, r.c.local_addr),
      h('span', nameAttrs(r.g.label), r.g.label), h('span', { class: 'num mut' }, r.c.pid), h('span', {}, reach(r)))),
    shown.length ? null : h('div', { class: 'item' }, h('p', {}, S.q ? 'No listeners match "' + S.q + '". ' : 'No listeners match the current filters. '), clearBtn()), pg.pager);
  return pageShell({ title: verdict('listeners', { exposed: rows.filter((r) => r.c.exposed).length }), subtitle: plural(rows.length, 'listening socket', 'listening sockets'), filters: [chips, h('span', { class: 'grow' }), dropdown('f-listen-proto', proto, [['all', 'Any protocol'], ['TCP', 'TCP'], ['UDP', 'UDP']], (v) => setFil('listen-proto', v), 'Protocol filter', true)], body: [table,
    h('p', { class: 'mut small', style: 'margin:12px 0 0' }, 'These are sockets, not distinct ports: one app can listen on IPv4 and IPv6 for the same port. Click a row to open the app.' + (missed > 0 ? ' ' + missed + ' more listening sockets belong to processes that exited before they could be read.' : ''))] });
}


