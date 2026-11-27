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
const feedWindowText = (look) => (look.first ? 'No earlier visit is recorded, so this covers the last 24 hours.' : 'Changes since your last visit.');
// One row per change newer than `since`, newest first. `label` turns an app key into its name.
function feedRows(input, since, label) {
  const rows = [], seen = input.seen || [], byApp = new Map();
  for (const x of seen) {
    if (!(x.first_seen > since)) continue;
    rows.push({ kind: 'host', app: x.group_key, ts: x.first_seen, text: label(x.group_key) + ' contacted a new host: ' + x.remote, go: { view: 'apps', sel: x.group_key } });
  }
  // An app counts as new on the network when none of its recorded hosts is older than the last visit.
  for (const x of seen) { const e = byApp.get(x.group_key) || { first: Infinity }; e.first = Math.min(e.first, x.first_seen); byApp.set(x.group_key, e); }
  for (const [key, e] of byApp) if (e.first > since) rows.push({ kind: 'first', app: key, ts: e.first, text: label(key) + ' made its first outbound connection', go: { view: 'network', filter: 'new' } });
  for (const i of input.pers || []) if (i.is_new && i.first_seen > since) rows.push({ kind: 'startup', ts: i.first_seen, text: 'New startup item: ' + i.label, go: { view: 'persist', filter: 'new' } });
  for (const e of input.sudo || []) if (e.ts > since) rows.push({ kind: 'sudo', ts: e.ts, text: 'sudo ran: ' + String(e.args || '').slice(0, 120), go: { view: 'priv' } });
  const firsts = new Set(rows.filter((r) => r.kind === 'first').map((r) => r.app));
  return rows.filter((r) => r.kind !== 'host' || !firsts.has(r.app)).sort((a, b) => b.ts - a.ts);
}
// The Machine strip. Every row shows a value or the reason it has none, never 0.
function machineRows(m) {
  const row = (id, label, v, reason, href) => ({ id, label, value: v == null ? null : v, reason: v == null ? (reason || 'no reason recorded') : null, href });
  return [
    row('free', 'Free space', m.freeText, m.freeReason, { view: 'storage' }),
    ...(m.forecastText || m.forecastReason ? [row('forecast', 'Forecast', m.forecastText, m.forecastReason, { view: 'storage' })] : []),
    row('memory', 'Memory pressure', m.memoryText, m.memoryReason, { view: 'performance', ex: { perfTab: 'memory' } }),
    row('cpu', 'CPU, 5 min average', m.cpuText, m.cpuReason, { view: 'performance' }),
  ];
}
// How long ago a change happened, as the feed shows it: a time today, a weekday this week, else a date.
const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'], MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
function feedWhen(ts, now) {
  const d = new Date(ts * 1000), n = new Date(now * 1000), two = (v) => String(v).padStart(2, '0');
  if (d.toDateString() === n.toDateString()) return two(d.getHours()) + ':' + two(d.getMinutes());
  if (now - ts < 6 * 86400) return WEEKDAYS[d.getDay()];
  return MONTHS[d.getMonth()] + ' ' + d.getDate();
}
// The average of the readings that are values, over the last `secs` before the newest sample; null when there are none.
function cpuAverage(samples, secs) {
  if (!samples.length) return null;
  const end = samples[samples.length - 1].ts, vals = [];
  for (const x of samples) { const v = x.cpu && x.cpu.total_pct && 'value' in x.cpu.total_pct ? x.cpu.total_pct.value : null; if (x.ts > end - secs && Number.isFinite(v)) vals.push(v); }
  return vals.length ? vals.reduce((a, b) => a + b, 0) / vals.length : null;
}
const SEV_RANK = { high: 0, medium: 1, low: 2 };
// Most severe first; entries of one severity keep the order they were found in.
const bySeverity = (list) => list.map((x, i) => [x, i]).sort((a, b) => (SEV_RANK[a[0].sev] ?? 3) - (SEV_RANK[b[0].sev] ?? 3) || a[1] - b[1]).map((p) => p[0]);
/* overview-page:end */

const LOOK = lookedSince(localStorage.getItem('monitor-lizard-last-looked'), Date.now() / 1000);
// The visit is stamped when the page goes away, so the feed stays the same while it is open.
const stampVisit = () => { if (S.view !== 'overview') return; try { localStorage.setItem('monitor-lizard-last-looked', String(Math.floor(Date.now() / 1000))); } catch (e) {} };
window.addEventListener('pagehide', stampVisit);
document.addEventListener('visibilitychange', () => { if (document.hidden) stampVisit(); });

function cards() {
  const g = S.snap.groups, f = S.snap.findings, out = [];
  const grp = (label) => g.find((x) => x.label === label);
  const add = (c) => { if (!S.muted.has(c.id)) out.push(c); };
  const appPage = (G) => ({ view: 'apps', sel: G.key });
  for (const x of f) {
    const G = grp(x.group);
    if (x.rule === 'exposed-listener' && G) add({ id: 'exp:' + G.key, sev: 'high', rule: x.rule, evidence: x.evidence, href: appPage(G), title: G.label + ' listens on ports open to your network', sub: x.detail + '. Anyone on your Wi-Fi can reach these.', acts: [!G.approved && ['Approve ' + G.label, 'btn-acc', () => approve(G)], x.mark_key && ['Mark these ports expected', G.approved ? 'btn-acc' : '', () => mark(findingCalls(x), true, 'the open ports of ' + G.label)], ['Mute', '', () => mute('exp:' + G.key)]] });
    if (x.rule === 'debug-port' && G) add({ id: 'dbg:' + G.key, sev: 'high', rule: x.rule, evidence: x.evidence, href: appPage(G), title: G.label + ' has its debug port open', sub: x.detail, acts: [['Quit ' + G.label + '...', 'btn-danger quiet', () => askStop(G.label, G.procs.filter((p) => p.user !== 'root'))], ['Mute', '', () => mute('dbg:' + G.key)]] });
    if (x.rule === 'unsigned-network' && G) add({ id: 'uns:' + G.key, sev: 'high', rule: x.rule, evidence: x.evidence, href: appPage(G), title: G.label + ' is unsigned and uses the network', sub: x.detail, acts: [['Quit ' + G.label + '...', 'btn-danger quiet', () => askStop(G.label, G.procs.filter((p) => p.user !== 'root'))], !G.approved && ['Approve ' + G.label, '', () => approve(G)]] });
    if (x.rule === 'new-remote' && G) add({ id: 'new:' + G.key + x.detail, sev: 'medium', rule: x.rule, evidence: x.evidence, href: appPage(G), title: G.label + ' contacted new hosts', sub: x.detail, acts: [['Mark these hosts expected', 'btn-acc', () => mark(findingCalls(x), true, 'the new hosts of ' + G.label)], ['Mute', '', () => mute('new:' + G.key + x.detail)]] });
  }
  const pending = g.filter((x) => status(x) === 'review');
  if (pending.length) add({ id: 'review', sev: 'medium', rule: 'unreviewed', evidence: pending.map((x) => x.label).join(', '), href: { view: 'apps', filter: 'review' }, title: pending.length + ' apps use the network but are not approved', sub: 'Approve the ones you recognise.', acts: [['Review apps', 'btn-acc', () => rec({ view: 'apps', filter: 'review' })], ['Mute', '', () => mute('review')]] });
  const failed = failedSudo();
  if (failed >= 5) add({ id: 'sudo', sev: 'medium', rule: 'failed-sudo', evidence: failed + ' failed sudo requests', href: { view: 'priv' }, title: failed + ' failed sudo attempts in the last 2 hours', sub: 'A retrying script often causes this. Open Privilege to see which program asked.', acts: [['Investigate', 'btn-acc', () => rec({ view: 'priv' })], ['Mark as mine', '', () => mute('sudo')]] });
  const ap = f.filter((x) => x.rule === 'exposed-listener-apple');
  if (ap.length) add({ id: 'apple', sev: 'low', rule: 'exposed-listener-apple', evidence: ap.map((x) => x.group).join(', '), href: { view: 'listeners' }, title: 'Apple services reachable from your network', sub: 'These are part of macOS.', acts: [['Review listeners', 'btn-acc', () => rec({ view: 'listeners', lfilter: 'exposed' })], ['Mute', '', () => mute('apple')]] });
  for (const i of S.pers.filter((x) => x.is_new)) add({ id: 'pers:' + i.path, sev: psig(i) === 'unsigned' ? 'high' : 'medium', rule: 'new-startup-item', evidence: i.path, href: { view: 'persist', filter: 'new' }, title: 'New startup item: ' + i.label, sub: i.program + ' (' + i.kind + ', ' + i.scope + ')', acts: [['Review', 'btn-acc', () => rec({ view: 'persist', q: '', pfilter: 'new' })], ['Mark expected', '', () => mark([['startup', { key: i.mark_key }]], true, i.label)]] });
  return bySeverity(out);
}

function machineCard() {
  const last = S.metrics.length ? S.metrics[S.metrics.length - 1] : null, vol = S.volume;
  const free = vol && rv(vol.free) != null ? formatBytes(rv(vol.free)) + ' free' : null;
  const outlook = S.stHist ? capacityOutlook(S.stHist, S.stHistFailed, null) : null;
  const level = rv(last && last.memory && last.memory.pressure_level), pctFree = rv(last && last.memory && last.memory.pressure_pct_available), cpuAvg = cpuAverage(S.metrics, 300);
  const machine = machineRows({ freeText: free, freeReason: vol && vol.free ? rReason(vol.free) : S.volumeFailed ? 'could not be read' : 'not read yet',
    forecastText: outlook && (outlook.kind === 'growing' || outlook.kind === 'flat') ? outlook.headline : null, forecastReason: outlook && !(outlook.kind === 'growing' || outlook.kind === 'flat') ? outlook.headline : null,
    memoryText: level == null ? null : pressureTag(level).text + (pctFree == null ? '' : ' \u00b7 ' + Math.round(pctFree) + '% free'), memoryReason: rReason(last && last.memory && last.memory.pressure_level),
    cpuText: cpuAvg == null ? null : Math.round(cpuAvg) + '%', cpuReason: rReason(last && last.cpu && last.cpu.total_pct) });
  return h('div', { class: 'card', id: 'machine-card', style: 'padding:6px 16px;margin-bottom:20px' }, machine.map((r) => h('div', { style: 'padding:10px 0;border-bottom:1px solid var(--line);display:flex;justify-content:space-between;gap:12px;font-size:13px' },
    h('a', { href: writeHash(r.href) }, r.label), r.value != null ? h('span', {}, r.value) : h('span', { class: 'mut', style: 'overflow-wrap:anywhere;text-align:right' }, 'Unavailable: ' + r.reason))));
}
// The Machine strip is refreshed in place: a full redraw every 2 s would drop keyboard focus and close the open "Marked as expected" list.
function refreshMachine() { const el = document.getElementById('machine-card'); if (el) el.replaceWith(machineCard()); }

function overview() {
  const s = S.snap, g = s.groups, c = cards();
  const review = reviewCount();
  const lan = g.reduce((n, x) => n + new Set(x.conns.filter((k) => k.exposed).map((k) => k.local_port)).size, 0);
  const failed = failedSudo();
  const tile = (l, v, sub, to, col) => h('a', { class: 'tile', href: writeHash(to), style: 'text-decoration:none;color:inherit' }, h('span', { class: 'l' }, l), h('span', { class: 'v', style: col ? 'color:var(--' + col + ')' : '' }, v), h('span', { class: 's' }, sub));
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
  const banner = s.baseline_notice && !S.noticeHidden ? h('div', { class: 'card item', role: 'alert', style: 'margin-bottom:14px;flex-wrap:wrap' }, h('span', { class: 'tag bad', style: 'flex:none;max-width:none;align-self:flex-start' }, 'BASELINE'), h('div', { class: 'grow', style: 'min-width:min(100%,240px)' }, h('h3', {}, 'Your expected-items list has a problem'), h('p', { style: 'overflow-wrap:anywhere' }, s.baseline_notice)), h('button', { class: 'btn sm', onClick: () => rec({ noticeHidden: true }) }, 'Dismiss')) : null;
  const markedSection = expectedF.length ? h('details', { class: 'card', style: 'margin-top:20px;padding:10px 16px' }, h('summary', { style: 'cursor:pointer;font-weight:500' }, 'Marked as expected (' + expectedF.length + ')'),
    expectedF.map((f) => h('div', { class: 'item' }, h('span', { class: 'tag', style: 'flex:none;max-width:none;align-self:flex-start' }, 'expected'), h('div', { class: 'grow', style: 'min-width:0;overflow-wrap:anywhere' }, h('h3', {}, f.group + ': ' + (RULE_TEXT[f.rule] || f.rule)), h('p', {}, 'Applies only to ' + findingScope(f) + '. Anything new is reported again.')),
      h('button', { class: 'btn sm', onClick: () => mark(findingCalls(f), false, f.group + ' (' + f.rule + ')') }, 'Unmark')))) : null;
  return pageShell({ actions: h('button', { class: 'btn sm btn-acc', type: 'button', onClick: () => { load(true); toast('Rescanning.'); } }, 'Rescan'), title: verdict('overview', { attention: c.length }), subtitle: plural(g.length, 'app', 'apps') + ', ' + plural(s.totals.outbound, 'outbound socket', 'outbound sockets') + ' right now', body: [
    banner,
    h('div', { style: 'display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:14px' },
      tile('Needs review', review, 'apps not yet approved', { view: 'apps', filter: 'review' }, review ? 'warn' : 'ok'),
      tile('Open to LAN', lan, 'ports on all interfaces', { view: 'listeners', filter: 'exposed' }),
      tile('Failed sudo', failed, 'sudo only, last 2 hours', { view: 'priv', dd: { 'auth-result': 'failed' } }, failed ? 'bad' : ''),
      tile('Outbound', s.totals.outbound, 'sockets right now', { view: 'network' })),
    h('div', { style: 'display:flex;flex-wrap:wrap;gap:20px;align-items:flex-start;margin-top:20px' },
      h('section', { style: 'flex:999 1 520px;min-width:0' }, h('h2', { class: 'sec' }, 'Needs a decision \u00b7 ' + c.length + (expectedF.length ? ' (' + expectedF.length + ' marked expected, listed below)' : '')),
        h('p', { class: 'mut small', style: 'margin:0 0 8px' }, 'Sorted by severity.'),
        h('div', { class: 'card' }, c.length ? dp.slice.map(decisionRow) : h('div', { class: 'item' }, h('p', {}, 'Nothing needs a decision. Everything on this Mac is accounted for.')), c.length > S.size ? dp.pager : null),
        h('h2', { class: 'sec', style: 'margin-top:20px' }, 'Since you last looked'), h('p', { class: 'mut small', style: 'margin:0 0 8px' }, feedWindowText(LOOK)),
        h('div', { class: 'card' }, feed.length ? feed.slice(0, 20).map((r) => h('div', { class: 'item' }, h('span', { class: 'mut small', style: 'flex:none;width:52px;font-family:var(--mono,monospace)', title: fmtWhen(r.ts) }, feedWhen(r.ts, Date.now() / 1000)), h('div', { class: 'grow', style: 'min-width:0;overflow-wrap:anywhere' }, h('a', { href: writeHash(r.go) }, r.text)))) : h('div', { class: 'item' }, h('p', {}, 'Nothing new since then.'))),
        feed.length > 20 ? h('p', { class: 'mut small' }, (feed.length - 20) + ' older changes are not shown.') : null),
      h('section', { style: 'flex:1 1 300px;min-width:0' }, h('h2', { class: 'sec' }, 'Machine'),
        machineCard(),
        h('div', { style: 'display:flex;justify-content:space-between;align-items:baseline' }, h('h2', { class: 'sec' }, 'Outbound sockets by app'), h('a', { href: writeHash({ view: 'network' }), class: 'small' }, 'All on Network')),
        h('div', { class: 'card', style: 'padding:6px 16px' }, talk.map((t) => h('div', { style: 'padding:10px 0;border-bottom:1px solid var(--line);display:flex;flex-direction:column;gap:6px' },
          h('div', { style: 'display:flex;justify-content:space-between;font-size:13px' }, h('a', { href: writeHash({ view: 'apps', q: t[0] }) }, t[0]), h('span', { class: 'mut' }, t[1])), h('div', { class: 'bar' }, h('i', { style: 'width:' + Math.round(t[1] / max * 100) + '%' }))))))),
    markedSection,
  ] });
}

