function apps() {
  const q = S.q.toLowerCase();
  const all = S.snap.groups.filter((g) => g.conns.length);
  const list = all.filter((g) => (S.filter === 'all' || status(g) === S.filter) && (!q || g.label.toLowerCase().includes(q) || g.conns.some((c) => String(c.local_port).includes(q) || (c.remote_addr || '').includes(q))));
  const count = (f) => all.filter((g) => f === 'all' || status(g) === f).length;
  const chips = [['all', 'All'], ['review', 'Needs review'], ['approved', 'Approved'], ['macOS', 'macOS']].map(([id, l]) => h('button', { class: 'chip ' + (S.filter === id ? 'on' : ''), onClick: () => rec({ filter: id, page: {} }) }, l + ' ', count(id)));
  const listen = listenText, sig = signText;
  const fLabel = fv('apps-label'), fSig = fv('apps-sig');
  const shown = sortRows(list.filter((g) => (fLabel === 'all' || g.trust === fLabel) && (fSig === 'all' || sigKind(g) === fSig)), 'apps', {
    app: (g) => g.label, procs: (g) => g.procs.length, out: (g) => g.conns.filter((c) => c.direction === 'outbound').length,
    listening: (g) => new Set(g.conns.filter((c) => c.direction === 'listen').map((c) => c.local_port)).size, signing: (g) => sig(g), status: (g) => status(g), label: (g) => TRUST_RANK[g.trust],
  });
  const pg = paginate(shown, 'apps');
  const filters = [
    dropdown('f-apps-label', fLabel, [['all', 'All labels'], ['trusted', 'Trusted'], ['unknown', 'Unknown'], ['suspicious', 'Suspicious']], (v) => setFil('apps-label', v), 'Label filter', true),
    dropdown('f-apps-sig', fSig, [['all', 'Any signing'], ['apple', 'Apple'], ['signed', 'Signed (third party)'], ['unsigned', 'Unsigned'], ['unchecked', 'Not checked']], (v) => setFil('apps-sig', v), 'Signing filter', true),
  ];
  const table = h('section', { class: 'card tbl', style: 'flex:999 1 640px;min-width:0' },
    sortHeader('apps', [{ label: 'App', sort: 'app' }, { label: 'Procs', sort: 'procs', cls: 'num' }, { label: 'Out', sort: 'out', cls: 'num' }, { label: 'Listening', sort: 'listening', title: 'Sort by number of listening ports' }, { label: 'Signing', sort: 'signing' }, { label: 'Status', sort: 'status' }, { label: 'Label', sort: 'label', title: 'Sort by label, most worrying first' }], '', 'ell'),
    pg.slice.map((g) => h('button', { class: 'tr ell ' + (S.sel === g.key ? 'on' : ''), onClick: () => rec({ sel: g.key }) },
      h('span', nameAttrs(g.label), g.label), h('span', { class: 'num' }, g.procs.length), h('span', { class: 'num' }, g.conns.filter((c) => c.direction === 'outbound').length),
      h('span', { class: 'mut small' }, listen(g)), h('span', { class: 'mut small' }, sig(g)), h('span', {}, h('span', { class: 'tag ' + tagOf(status(g)) }, status(g))), h('span', {}, h('span', { class: 'tag ' + trustCls(g.trust), title: (g.rules_fired || []).join(', ') }, g.trust)))),
    shown.length ? null : h('div', { class: 'item' }, h('p', {}, 'No apps match the current filters. '), clearBtn()), pg.pager);
  const g = S.snap.groups.find((x) => x.key === S.sel);
  const handle = g ? h('div', { class: 'handle', role: 'separator', 'aria-orientation': 'vertical', 'aria-label': 'Resize the app detail panel', 'aria-valuenow': S.drawerW || 380, 'aria-valuemin': DRAWER_MIN, 'aria-valuemax': DRAWER_MAX, tabindex: '0', onMousedown: startDrawerResize, onKeydown: drawerHandleKeys, onDblclick: () => { S.drawerW = null; localStorage.removeItem('drawer-width'); render(); } }) : null;
  return pageShell({ title: verdict('apps', { review: reviewCount() }), subtitle: plural(all.length, 'app', 'apps') + ' with network activity', filters: [chips, h('span', { class: 'grow' }), filters], body: table, inspector: g ? [handle, drawer(g, listen(g), sig(g))] : null });
}

// The pages that know about this app, each with how many rows it opens. Network, Listeners and History filter by the
// app's name; startup items by its path. The Persistence page has no inspector of its own.
function related(g) {
  const path = unprefixed(g.key), isPath = path.startsWith('/');
  const hosts = new Set(g.conns.filter((c) => c.direction === 'outbound' && c.remote_addr && c.scope !== 'loopback').map((c) => c.remote_addr)).size;
  const counts = { hosts, sockets: g.conns.filter((c) => c.direction === 'listen').length, startup: isPath ? S.pers.filter((i) => i.program.toLowerCase().includes(path.toLowerCase())).length : 0, newHosts: S.seen.filter((x) => x.group_key === g.key).length };
  const links = relatedLinks(counts, g.label, path);
  if (!links.length) return null;
  return h('div', { class: 'insp-sec' }, h('h3', { class: 'sec' }, 'Related'),
    h('ul', { style: 'list-style:none;margin:0;padding:0;display:flex;flex-direction:column;gap:6px;font-size:13px' }, links.map((l) => h('li', {}, h('a', { href: l.hash, title: 'Filtered by this app. An app with a similar name may also show.' }, l.text)))));
}
function drawer(g, listen, sig) {
  const st = status(g), flags = S.snap.findings.filter((f) => f.group === g.label);
  const mine = g.procs.filter((p) => p.user !== 'root');
  const outs = g.conns.filter((c) => c.direction === 'outbound');
  return h('aside', { class: 'drawer card' + (S.drawerW ? ' sized' : ''), style: S.drawerW ? '--dw:' + S.drawerW + 'px' : '', 'aria-label': 'App detail' },
    h('div', { class: 'insp-head' }, h('div', { class: 'grow' }, h('div', { style: 'font-size:16px;font-weight:600' }, g.label), h('div', { class: 'mut small', style: 'word-break:break-all' }, unprefixed(g.key))),
      h('span', { class: 'tag ' + tagOf(st) }, st), h('span', { class: 'tag ' + trustCls(g.trust) }, g.trust), h('button', { class: 'btn sm', onClick: () => rec({ sel: null }) }, 'Close')),
    h('div', { style: 'display:flex;gap:8px;padding:0 16px 14px;flex-wrap:wrap' },
      h('button', { class: 'btn sm', title: 'Copies this app\'s row as JSON', onClick: () => copyRow('apps', g) }, 'Copy as JSON'),
      !g.approved && !apple(g) ? h('button', { class: 'btn btn-acc sm', onClick: () => approve(g) }, 'Approve app') : null,
      mine.length ? h('button', { class: 'btn btn-danger quiet sm', onClick: () => askStop(g.label, mine) }, 'Quit app...') : null,
      h('button', { class: 'btn sm', disabled: true, title: 'Needs the network extension (later phase)' }, 'Block network (soon)')),
    (g.rules_fired || []).length ? h('div', { class: 'insp-sec' }, h('h3', { class: 'sec' }, 'Label: ' + g.trust + ' (decided by rules)'), g.rules_fired.map((r) => h('p', { style: 'margin:0 0 4px;font-size:13px' }, h('span', { class: 'mut' }, r + ': '), RULE_TEXT[r] || r))) : null,
    flags.length ? h('div', { class: 'insp-sec' }, h('h3', { class: 'sec' }, 'Why it is flagged'), flags.map((f) => h('p', { style: 'margin:0 0 6px;font-size:13px;line-height:1.45' }, f.detail))) : null,
    h('dl', { class: 'kv' }, h('dt', {}, 'Signing'), h('dd', {}, sig), h('dt', {}, 'Listening'), h('dd', {}, listen), h('dt', {}, 'Outbound'), h('dd', {}, outs.length + ' sockets to ' + new Set(outs.map((c) => c.remote_addr)).size + ' hosts')),
    related(g),
    h('div', { style: 'border-top:1px solid var(--line)' }, h('div', { style: 'padding:12px 16px 8px' }, h('h3', { class: 'sec', style: 'margin:0' }, 'Processes (' + g.procs.length + ')')),
      (S.expand.procs ? g.procs : g.procs.slice(0, 12)).map((p) => h('div', { class: 'pr' }, h('span', { class: 'mut' }, p.pid), h('span', { style: 'overflow:hidden;text-overflow:ellipsis;white-space:nowrap', title: p.command }, base(p.exe)),
        h('span', { class: 'num' }, p.cpu.toFixed(1) + '%'), h('span', { class: 'num' }, Math.round(p.rss_kb / 1024) + 'M'),
        p.user === 'root' ? h('span', { class: 'tag', title: 'Monitor Lizard never signals root processes' }, 'root') : h('button', { class: 'btn btn-danger quiet sm', onClick: () => askStop(base(p.exe) + ' ' + p.pid, [p]) }, 'Stop'))),
      g.procs.length > 12 ? h('button', { class: 'pr mut small expander', onClick: () => rec({ expand: Object.assign({}, S.expand, { procs: !S.expand.procs }) }) }, S.expand.procs ? 'Show fewer' : 'Show all ' + g.procs.length) : null),
    h('div', { style: 'border-top:1px solid var(--line)' }, h('div', { style: 'padding:12px 16px 8px' }, h('h3', { class: 'sec', style: 'margin:0' }, 'Connections (' + g.conns.length + ')')),
      (S.expand.conns ? g.conns : g.conns.slice(0, 20)).map((c) => h('div', { class: 'pr', style: 'grid-template-columns:70px minmax(0,1fr) auto' }, h('span', { class: 'tag' }, c.direction), h('span', { class: 'small', style: 'word-break:break-all' }, c.proto + ' ' + c.local_addr + ':' + c.local_port + (c.remote_addr ? ' -> ' + c.remote_addr + ':' + c.remote_port : '')), h('span', { class: 'mut small' }, c.scope + (c.exposed ? ', EXPOSED' : '')))),
      g.conns.length > 20 ? h('button', { class: 'pr mut small expander', onClick: () => rec({ expand: Object.assign({}, S.expand, { conns: !S.expand.conns }) }) }, S.expand.conns ? 'Show fewer' : 'Show all ' + g.conns.length) : null));
}

