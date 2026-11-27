/* overview-page:begin */
// What the Overview shows besides the decisions: the time since the owner last looked, the feed of what changed
// since then, and the Machine strip. All pure, so the tests can read them without a browser.
const LOOK_FALLBACK_SECS = 86400, LOOK_MAX_SECS = 30 * 86400;
// The last visit comes from the browser's own storage. Anything missing, unreadable or in the future counts as a first visit,
// which covers the last 24 hours; a very old visit is cut to 30 days so the feed stays short.
function lookedSince(stored, now) {
  const t = stored == null || stored === '' ? NaN : Number(stored);
  if (!Number.isFinite(t) || t <= 0 || t > now) return { since: now - LOOK_FALLBACK_SECS, first: true };
  return { since: Math.max(t, now - LOOK_MAX_SECS), first: false };
}
const feedWindowText = (look) => (look.first ? 'This is your first visit, so this covers the last 24 hours.' : 'Changes since your last visit.');
// One row per change newer than `since`, newest first. `label` turns an app key into its name.
function feedRows(input, since, label) {
  const rows = [], seen = input.seen || [], byApp = new Map();
  for (const x of seen) {
    if (!(x.first_seen > since)) continue;
    rows.push({ kind: 'host', ts: x.first_seen, text: label(x.group_key) + ' contacted a new host: ' + x.remote, go: { view: 'apps', sel: x.group_key } });
  }
  // An app counts as new on the network when none of its recorded hosts is older than the last visit.
  for (const x of seen) { const e = byApp.get(x.group_key) || { first: Infinity }; e.first = Math.min(e.first, x.first_seen); byApp.set(x.group_key, e); }
  for (const [key, e] of byApp) if (e.first > since) rows.push({ kind: 'first', ts: e.first, text: label(key) + ' made its first outbound connection', go: { view: 'network', filter: 'new' } });
  for (const i of input.pers || []) if (i.first_seen > since) rows.push({ kind: 'startup', ts: i.first_seen, text: 'New startup item: ' + i.label, go: { view: 'persist', filter: 'new' } });
  for (const e of input.sudo || []) if (e.ts > since) rows.push({ kind: 'sudo', ts: e.ts, text: 'sudo ran: ' + String(e.args || '').slice(0, 120), go: { view: 'priv' } });
  return rows.sort((a, b) => b.ts - a.ts);
}
// The Machine strip. Every row shows a value or the reason it has none, never 0.
function machineRows(m) {
  const row = (id, label, v, reason, href) => ({ id, label, value: v == null ? null : v, reason: v == null ? (reason || 'unavailable') : null, href });
  return [
    row('free', 'Free space', m.freeText, m.freeReason, { view: 'storage' }),
    ...(m.forecastText ? [row('forecast', 'Forecast', m.forecastText, null, { view: 'storage' })] : []),
    row('memory', 'Memory pressure', m.memoryText, m.memoryReason, { view: 'performance', ex: { tab: 'memory' } }),
    row('cpu', 'CPU', m.cpuText, m.cpuReason, { view: 'performance' }),
  ];
}
/* overview-page:end */

const LOOK = lookedSince(localStorage.getItem('monitor-lizard-last-looked'), Date.now() / 1000);
// The visit is stamped when the page goes away, so the feed stays the same while it is open.
const stampVisit = () => { try { localStorage.setItem('monitor-lizard-last-looked', String(Math.floor(Date.now() / 1000))); } catch (e) {} };
window.addEventListener('pagehide', stampVisit);
document.addEventListener('visibilitychange', () => { if (document.hidden) stampVisit(); });

function cards() {
  const g = S.snap.groups, f = S.snap.findings, out = [];
  const grp = (label) => g.find((x) => x.label === label);
  const add = (c) => { if (!S.muted.has(c.id)) out.push(c); };
  const appPage = (G) => ({ view: 'apps', sel: G.key });
  for (const x of f) {
    const G = grp(x.group);
    if (x.rule === 'exposed-listener' && G) add({ id: 'exp:' + G.key, sev: 'high', rule: x.rule, evidence: x.evidence, href: appPage(G), title: G.label + ' listens on ports open to your network', sub: x.detail + '. Anyone on your Wi-Fi can reach these.', acts: [!G.approved && ['Approve ' + G.label, 'btn-acc', () => approve(G)], x.mark_key && ['Mark these ports expected', '', () => mark(findingCalls(x), true, 'the open ports of ' + G.label)]] });
    if (x.rule === 'debug-port' && G) add({ id: 'dbg:' + G.key, sev: 'high', rule: x.rule, evidence: x.evidence, href: appPage(G), title: G.label + ' has its debug port open', sub: x.detail, acts: [['Quit ' + G.label + '...', 'btn-danger quiet', () => askStop(G.label, G.procs.filter((p) => p.user !== 'root'))]] });
    if (x.rule === 'unsigned-network' && G) add({ id: 'uns:' + G.key, sev: 'high', rule: x.rule, evidence: x.evidence, href: appPage(G), title: G.label + ' is unsigned and uses the network', sub: x.detail, acts: [] });
    if (x.rule === 'new-remote' && G) add({ id: 'new:' + G.key + x.detail, sev: 'medium', rule: x.rule, evidence: x.evidence, href: appPage(G), title: G.label + ' contacted new hosts', sub: x.detail, acts: [['Mark these hosts expected', '', () => mark(findingCalls(x), true, 'the new hosts of ' + G.label)], ['Mute', '', () => mute('new:' + G.key + x.detail)]] });
  }
  const pending = g.filter((x) => status(x) === 'review');
  if (pending.length) add({ id: 'review', sev: 'medium', rule: 'unreviewed', evidence: pending.map((x) => x.label).join(', '), href: { view: 'apps', filter: 'review' }, title: pending.length + ' apps use the network but are not approved', sub: 'Approve the ones you recognise.', acts: [] });
  const failed = failedSudo();
  if (failed >= 5) add({ id: 'sudo', sev: 'medium', rule: 'failed-sudo', evidence: failed + ' failed sudo requests', href: { view: 'priv' }, title: failed + ' failed sudo attempts in the last 2 hours', sub: 'Repeated failures like this are usually a script retrying. Check who ran them.', acts: [['Mark as mine', '', () => mute('sudo')]] });
  const ap = f.filter((x) => x.rule === 'exposed-listener-apple');
  if (ap.length) add({ id: 'apple', sev: 'low', rule: 'exposed-listener-apple', evidence: ap.map((x) => x.group).join(', '), href: { view: 'listeners' }, title: 'Apple services reachable from your network', sub: 'These are part of macOS.', acts: [['Mute', '', () => mute('apple')]] });
  for (const i of S.pers.filter((x) => x.is_new)) add({ id: 'pers:' + i.path, sev: psig(i) === 'unsigned' ? 'high' : 'medium', rule: 'new-startup-item', evidence: i.path, href: { view: 'persist', filter: 'new' }, title: 'New startup item: ' + i.label, sub: i.program + ' (' + i.kind + ', ' + i.scope + ')', acts: [['Mark expected', '', () => mark([['startup', { key: i.mark_key }]], true, i.label)]] });
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
  const dp = paginate(c, 'decisions');
  const decisionRow = (x) => h('div', { class: 'item' }, h('span', { class: 'sev ' + x.sev, title: x.sev }),
    h('div', { class: 'grow', style: 'min-width:0;overflow-wrap:anywhere' }, h('h3', {}, h('a', { href: writeHash(x.href) }, x.title)), h('p', {}, x.sub),
      x.evidence ? h('p', { class: 'small' }, h('span', { class: 'mut' }, 'Evidence: '), x.evidence) : null,
      h('p', { class: 'small mut' }, 'Rule: ' + (RULE_TEXT[x.rule] || x.rule) + ' (', h('code', {}, x.rule), ')'),
      x.acts.filter(Boolean).length ? h('div', { style: 'display:flex;gap:8px;margin-top:10px;flex-wrap:wrap' }, x.acts.filter(Boolean).slice(0, 2).map((a) => h('button', { class: 'btn sm ' + a[1], onClick: a[2] }, a[0]))) : null),
    h('span', { class: 'tag ' + ({ high: 'bad', medium: 'warn' }[x.sev] || ''), style: 'flex:none;max-width:none;align-self:flex-start' }, x.sev));
  const label = (key) => { const G = g.find((x) => x.key === key); return G ? G.label : key; };
  const feed = feedRows({ seen: S.seen, pers: S.pers, sudo: S.sudo }, LOOK.since, label);
  const last = S.metrics.length ? S.metrics[S.metrics.length - 1] : null, vol = S.volume;
  const free = vol && rv(vol.free) != null ? formatBytes(rv(vol.free)) + ' free' : null;
  const outlook = S.stHist ? capacityOutlook(S.stHist, S.stHistFailed, null) : null;
  const level = rv(last && last.memory && last.memory.pressure_level), cpuNow = rv(last && last.cpu && last.cpu.total_pct);
  const machine = machineRows({ freeText: free, freeReason: vol && vol.free ? rReason(vol.free) : S.volumeFailed ? 'could not be read' : 'not read yet',
    forecastText: outlook && (outlook.kind === 'growing' || outlook.kind === 'flat') ? outlook.headline : null,
    memoryText: level == null ? null : pressureTag(level).text, memoryReason: rReason(last && last.memory && last.memory.pressure_level),
    cpuText: cpuNow == null ? null : Math.round(cpuNow) + '% total', cpuReason: rReason(last && last.cpu && last.cpu.total_pct) });
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
      h('section', { style: 'flex:999 1 520px;min-width:0' }, h('h2', { class: 'sec' }, 'Needs a decision' + (expectedF.length ? ' (' + expectedF.length + ' marked expected, listed below)' : '')),
        h('div', { class: 'card' }, c.length ? dp.slice.map(decisionRow) : h('div', { class: 'item' }, h('p', {}, 'Nothing needs a decision. Everything on this Mac is accounted for.')), c.length > S.size ? dp.pager : null),
        h('h2', { class: 'sec', style: 'margin-top:20px' }, 'Since you last looked'), h('p', { class: 'mut small', style: 'margin:0 0 8px' }, feedWindowText(LOOK)),
        h('div', { class: 'card' }, feed.length ? feed.slice(0, 20).map((r) => h('div', { class: 'item' }, h('div', { class: 'grow', style: 'min-width:0;overflow-wrap:anywhere' }, h('a', { href: writeHash(r.go) }, r.text), h('p', { class: 'mut small' }, fmtWhen(r.ts))))) : h('div', { class: 'item' }, h('p', {}, 'Nothing new since then.')))),
      h('section', { style: 'flex:1 1 300px;min-width:0' }, h('h2', { class: 'sec' }, 'Machine'),
        h('div', { class: 'card', style: 'padding:6px 16px;margin-bottom:20px' }, machine.map((r) => h('div', { style: 'padding:10px 0;border-bottom:1px solid var(--line);display:flex;justify-content:space-between;gap:12px;font-size:13px' },
          h('a', { href: writeHash(r.href) }, r.label), r.value != null ? h('span', {}, r.value) : h('span', { class: 'mut', style: 'overflow-wrap:anywhere;text-align:right' }, 'Unavailable: ' + r.reason)))),
        h('h2', { class: 'sec' }, 'Outbound sockets by app'),
        h('div', { class: 'card', style: 'padding:6px 16px' }, talk.map((t) => h('div', { style: 'padding:10px 0;border-bottom:1px solid var(--line);display:flex;flex-direction:column;gap:6px' },
          h('div', { style: 'display:flex;justify-content:space-between;font-size:13px' }, h('a', { href: writeHash({ view: 'apps', q: t[0] }) }, t[0]), h('span', { class: 'mut' }, t[1])), h('div', { class: 'bar' }, h('i', { style: 'width:' + Math.round(t[1] / max * 100) + '%' }))))))),
    markedSection,
  ] });
}

