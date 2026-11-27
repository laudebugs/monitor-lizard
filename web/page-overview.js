function cards() {
  const g = S.snap.groups, f = S.snap.findings, out = [];
  const grp = (label) => g.find((x) => x.label === label);
  const add = (c) => { if (!S.muted.has(c.id)) out.push(c); };
  for (const x of f) {
    const G = grp(x.group);
    if (x.rule === 'exposed-listener' && G) add({ id: 'exp:' + G.key, sev: 'high', title: G.label + ' listens on ports open to your network', sub: x.detail + '. Anyone on your Wi-Fi can reach these.', acts: [!G.approved && ['Approve ' + G.label, 'btn-acc', () => approve(G)], x.mark_key && ['Mark these ports expected', '', () => mark(findingCalls(x), true, 'the open ports of ' + G.label)], ['Details', '', () => rec({ view: 'apps', sel: G.key })]] });
    if (x.rule === 'debug-port' && G) add({ id: 'dbg:' + G.key, sev: 'high', title: G.label + ' has its debug port open', sub: x.detail, acts: [['Quit ' + G.label + '...', 'btn-danger quiet', () => askStop(G.label, G.procs.filter((p) => p.user !== 'root'))], ['Details', '', () => rec({ view: 'apps', sel: G.key })]] });
    if (x.rule === 'unsigned-network' && G) add({ id: 'uns:' + G.key, sev: 'high', title: G.label + ' is unsigned and uses the network', sub: x.detail, acts: [['Details', 'btn-acc', () => rec({ view: 'apps', sel: G.key })]] });
    if (x.rule === 'new-remote' && G) add({ id: 'new:' + G.key + x.detail, sev: 'medium', title: G.label + ' contacted new hosts', sub: x.detail, acts: [['Details', 'btn-acc', () => rec({ view: 'apps', sel: G.key })], ['Mark these hosts expected', '', () => mark(findingCalls(x), true, 'the new hosts of ' + G.label)], ['Mute', '', () => mute('new:' + G.key + x.detail)]] });
  }
  const pending = g.filter((x) => status(x) === 'review');
  if (pending.length) add({ id: 'review', sev: 'medium', title: pending.length + ' apps use the network but are not approved', sub: pending.map((x) => x.label).join(', '), acts: [['Review apps', 'btn-acc', () => rec({ view: 'apps', filter: 'review' })]] });
  const failed = failedSudo();
  if (failed >= 5) add({ id: 'sudo', sev: 'medium', title: failed + ' failed sudo attempts in the last 2 hours', sub: 'Repeated failures like this are usually a script retrying. Check who ran them.', acts: [['Investigate', 'btn-acc', () => rec({ view: 'priv' })], ['Mark as mine', '', () => mute('sudo')]] });
  const ap = f.filter((x) => x.rule === 'exposed-listener-apple');
  if (ap.length) add({ id: 'apple', sev: 'low', title: 'Apple services reachable from your network', sub: ap.map((x) => x.group).join(', ') + '. These are part of macOS.', acts: [['Mute', '', () => mute('apple')]] });
  for (const i of S.pers.filter((x) => x.is_new)) add({ id: 'pers:' + i.path, sev: psig(i) === 'unsigned' ? 'high' : 'medium', title: 'New startup item: ' + i.label, sub: i.program + ' (' + i.kind + ', ' + i.scope + ')', acts: [['Details', 'btn-acc', () => rec({ view: 'persist', q: '', pfilter: 'new' })], ['Mark expected', '', () => mark([['startup', { key: i.mark_key }]], true, i.label)]] });
  return out;
}

function overview() {
  const s = S.snap, g = s.groups, c = cards();
  const review = reviewCount();
  const lan = g.reduce((n, x) => n + new Set(x.conns.filter((k) => k.exposed).map((k) => k.local_port)).size, 0);
  const failed = failedSudo();
  const tile = (l, v, sub, go, col) => h('button', { class: 'tile', onClick: go }, h('span', { class: 'l' }, l), h('span', { class: 'v', style: col ? 'color:var(--' + col + ')' : '' }, v), h('span', { class: 's' }, sub));
  const talk = g.filter((x) => x.conns.some((k) => k.direction === 'outbound')).map((x) => [x.label, x.conns.filter((k) => k.direction === 'outbound').length]).sort((a, b) => b[1] - a[1]).slice(0, 5);
  const max = talk.length ? talk[0][1] : 1;
  const expectedF = s.expected_findings || [];
  const banner = s.baseline_notice && !S.noticeHidden ? h('div', { class: 'card item', role: 'alert', style: 'margin-bottom:14px;flex-wrap:wrap' }, h('span', { class: 'tag bad', style: 'flex:none;max-width:none;align-self:flex-start' }, 'BASELINE'), h('div', { class: 'grow', style: 'min-width:min(100%,240px)' }, h('h3', {}, 'Your expected-items list has a problem'), h('p', { style: 'overflow-wrap:anywhere' }, s.baseline_notice)), h('button', { class: 'btn sm', onClick: () => rec({ noticeHidden: true }) }, 'Dismiss')) : null;
  const markedSection = expectedF.length ? h('details', { class: 'card', style: 'margin-top:20px;padding:10px 16px' }, h('summary', { style: 'cursor:pointer;font-weight:500' }, 'Marked as expected (' + expectedF.length + ')'),
    expectedF.map((f) => h('div', { class: 'item' }, h('span', { class: 'tag', style: 'flex:none;max-width:none;align-self:flex-start' }, 'expected'), h('div', { class: 'grow', style: 'min-width:0;overflow-wrap:anywhere' }, h('h3', {}, f.group + ': ' + (RULE_TEXT[f.rule] || f.rule)), h('p', {}, 'Applies only to ' + findingScope(f) + '. Anything new is reported again.')),
      h('button', { class: 'btn sm', onClick: () => mark(findingCalls(f), false, f.group + ' (' + f.rule + ')') }, 'Unmark')))) : null;
  return pageShell({ title: verdict('overview', { attention: c.length }), subtitle: plural(g.length, 'app', 'apps') + ', ' + plural(s.totals.outbound, 'outbound socket', 'outbound sockets') + ' right now', body: [
    banner,
    h('div', { style: 'display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:14px' },
      tile('Needs review', review, 'apps not yet approved', () => rec({ view: 'apps', filter: 'review' }), review ? 'warn' : 'ok'),
      tile('Open to LAN', lan, 'ports on all interfaces', () => rec({ view: 'apps' })),
      tile('Failed sudo', failed, 'sudo only, last 2 hours', () => rec({ view: 'priv', fil: Object.assign({}, S.fil, { 'auth-result': 'failed' }), page: {} }), failed ? 'bad' : ''),
      tile('Outbound', s.totals.outbound, 'sockets right now', () => rec({ view: 'apps' }))),
    h('div', { style: 'display:flex;flex-wrap:wrap;gap:20px;align-items:flex-start;margin-top:20px' },
      h('section', { style: 'flex:999 1 520px;min-width:0' }, h('h2', { class: 'sec' }, 'Needs your attention' + (expectedF.length ? ' (' + expectedF.length + ' marked expected, listed below)' : '')),
        h('div', { class: 'card' }, c.length ? c.map((x) => h('div', { class: 'item' }, h('span', { class: 'sev ' + x.sev, title: x.sev }),
          h('div', { class: 'grow' }, h('h3', {}, x.title), h('p', {}, x.sub), h('div', { style: 'display:flex;gap:8px;margin-top:10px;flex-wrap:wrap' }, x.acts.filter(Boolean).map((a) => h('button', { class: 'btn sm ' + a[1], onClick: a[2] }, a[0])))),
          h('span', { class: 'tag ' + ({ high: 'bad', medium: 'warn' }[x.sev] || ''), style: 'flex:none;max-width:none;align-self:flex-start' }, x.sev))) : h('div', { class: 'item' }, h('p', {}, 'Nothing needs attention. Everything on this Mac is accounted for.')))),
      h('section', { style: 'flex:1 1 300px;min-width:0' }, h('h2', { class: 'sec' }, 'Top talkers (outbound sockets)'),
        h('div', { class: 'card', style: 'padding:6px 16px' }, talk.map((t) => h('div', { style: 'padding:10px 0;border-bottom:1px solid var(--line);display:flex;flex-direction:column;gap:6px' },
          h('div', { style: 'display:flex;justify-content:space-between;font-size:13px' }, h('span', {}, t[0]), h('span', { class: 'mut' }, t[1])), h('div', { class: 'bar' }, h('i', { style: 'width:' + Math.round(t[1] / max * 100) + '%' }))))))),
    markedSection,
  ] });
}

