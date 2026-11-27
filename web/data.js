async function api(path, method = 'GET') {
  const r = await fetch(path, { method, headers: { 'X-MonitorLizard-Token': TOKEN } });
  if (!r.ok) { const text = await r.text(), e = new Error(text); e.status = r.status; e.text = text; throw e; } // the Trash flow reads the status and the body of a refusal
  return r;
}
async function load(fresh) {
  try {
    const snap = await (await api('/api/snapshot' + (fresh ? '?fresh=1' : ''))).json();
    // A shared link can name an app that has quit since; open the page without it rather than with an empty panel.
    quiet(Object.assign({ snap }, selPatch(S, snap))); $('live').textContent = 'live, refreshed ' + new Date().toLocaleTimeString(); }
  catch (e) { $('live').textContent = 'offline: ' + e.message; }
}
const loadPriv = async () => { try { quiet({ priv: (await (await api('/api/privilege')).json()) || [], privOk: true }); } catch (e) {} };
const loadSudo = async () => { try { quiet({ sudo: (await (await api('/api/sudo')).json()) || [] }); } catch (e) {} };
// The reply is { items, changes }. Items that are no longer found stay in the data but not on this page, which lists what is installed now.
const loadPers = async () => { try { const d = await (await api('/api/persistence')).json(); quiet({ pers: (Array.isArray(d) ? d : (d && d.items) || []).filter((i) => !i.removed_at), persOk: true }); } catch (e) {} };
const loadHist = async () => { try { const h = (await (await api('/api/history')).json()) || {}; quiet({ seen: h.seen || [], acts: h.actions || [], baseChanges: h.baseline_changes || [], trashMoves: Array.isArray(h.trash_moves) ? h.trash_moves : [], histOk: true }); } catch (e) {} };
const expectedTag = () => h('span', { class: 'tag', title: 'You marked this as expected' }, 'expected');
// params is one of { key } (the server's own key for a finding or startup item) or { app, addr } for a host:
// the page never builds baseline keys. kind is app, host, startup or finding; the server audits each change first.
async function markOne(kind, params, on) {
  const q = new URLSearchParams(Object.assign({ kind }, params)).toString();
  await api('/api/' + (on ? 'approve' : 'unapprove') + '?' + q, 'POST');
}
async function mark(calls, on, what) {
  try {
    for (const [kind, params] of calls) await markOne(kind, params, on);
    toast((on ? 'Marked ' : 'Unmarked ') + what + (on ? ' as expected.' : '. It can be reported again.'));
    await Promise.all([load(true), loadPers(), loadHist()]);
  } catch (e) { toast('Could not ' + (on ? 'mark' : 'unmark') + ': ' + e.message); }
}
const markBtn = (calls, expected, what, cls) => h('button', { class: 'btn sm ' + (cls || ''), onClick: () => mark(calls, !expected, what) }, expected ? 'Unmark' : 'Mark expected');
// The calls that mark or unmark a finding: its own key for open ports, one host call per host for new hosts.
const findingCalls = (f) => (f.mark_key ? [['finding', { key: f.mark_key }]] : f.rule === 'new-remote' ? f.evidence.split(',').map((a) => ['host', { app: f.group_key, addr: a }]) : []);
const findingScope = (f) => (f.rule === 'new-remote' ? 'these hosts: ' : 'these ports: ') + f.evidence.split(',').join(', ');
const loadSettings = async () => { try { quiet({ settings: await (await api('/api/settings')).json() }); } catch (e) {} };
// `since` is the newest ts already held; the server returns only what's newer, so a tab backgrounded
// for a while still gets exactly its gap in one request, not a burst of polls for the time it missed.
async function loadMetrics() {
  if (document.hidden || (S.view !== 'performance' && S.view !== 'overview') || (S.view === 'performance' && !S.perfLive)) return;
  try {
    const got = await (await api('/api/metrics?since=' + S.metricsSince)).json();
    if (!got.length) return;
    // src/sampler.rs stamps every sample with the real wall clock, so a restart's samples keep
    // counting forward from roughly "now", not from zero -- the one thing a restart can never
    // produce is a ts at or before what the client already holds (the server's own ring has none
    // that old left to send), so that's the signal used here, narrow as it is: it fires on a real
    // system clock being set backward too, which is the same "don't trust the old data" case.
    const reset = S.metrics.length && got[0].ts <= S.metrics[S.metrics.length - 1].ts;
    const merged = (reset ? [] : S.metrics).concat(got);
    quiet({ metrics: merged.slice(-RING_CAPACITY), metricsSince: got[got.length - 1].ts });
  } catch (e) {}
}
const RING_CAPACITY = 1800; // mirrors src/sampler.rs RING_CAPACITY (1h at 2s): the client never holds more than the server does
async function setNotifications(level) {
  try { const s = await (await api('/api/settings?notifications=' + level, 'POST')).json(); rec({ settings: s }); toast('Notifications: ' + level + '.' + (level !== 'off' ? ' Findings already on this Mac will not notify; only new ones will.' : '')); }
  catch (e) { toast('Could not change notifications: ' + e.message); }
}
const toast = (t) => rec({ toast: t });
// Failed authorization requests: the same rows the Privilege page lists under its Failed filter.
const failedAuth = () => S.priv.filter((e) => !e.ok).length;
// Failed requests from sudo alone, in the Overview tile and its banner.
const failedSudo = () => S.priv.filter((e) => !e.ok && e.client.endsWith('/sudo')).length;
const reviewCount = () => (S.snap ? countReview(S.snap.groups, status) : 0);
// What each verdict title waits for (see TITLE_NEEDS).
const loadedNow = () => ({ stor: !!(S.stData && typeof S.stData.scanned_at === 'number'), snap: !!S.snap, priv: S.privOk, pers: S.persOk, hist: S.histOk });
const verdict = (view, facts) => verdictTitle(view, titleReady(view, loadedNow()) ? facts : null);
const mute = (id) => { S.muted.add(id); localStorage.setItem('monitor-lizard-muted', JSON.stringify([...S.muted])); toast('Muted. It stays visible in its own section.'); };
async function approve(g) { try { await api('/api/approve?key=' + encodeURIComponent(g.key), 'POST'); toast('Approved ' + g.label + '. It is now part of your baseline.'); await load(true); } catch (e) { toast('Could not approve: ' + e.message); } }
async function stop(sig) {
  const m = S.modal; rec({ modal: null });
  try {
    const o = await (await api('/api/kill?sig=' + sig + '&pids=' + m.pids.join(',') + '&start=' + m.starts.join(','), 'POST')).json();
    toast('Sent SIG' + sig + ' to ' + o.sent.length + ' process(es) of ' + m.label + (o.refused.length ? '. Refused ' + o.refused.length + ': ' + o.refused[0].reason : '.'));
    setTimeout(() => load(true), 700);
    setTimeout(loadHist, 700); // the audit row for this action lands on the server at the same moment
  } catch (e) { toast('Stop failed: ' + e.message); }
}
// Fetches one fresh row per pid before opening the dialog (never from whatever stale list the
// caller already had), so the confirm screen -- and the `start=` the eventual /api/kill call sends
// back -- both come from the same just-read sample. A pid that already exited, or a server that
// could not be reached at all, means fewer (or zero) confirmable pids, never a dialog built on guesses.
async function askStop(label, procs) {
  const pids = procs.map((p) => p.pid);
  let rows = [];
  try { rows = await (await api('/api/processes?pids=' + pids.join(','))).json(); } catch (e) { rows = []; }
  const startByPid = new Map(rows.map((r) => [r.pid, rv(r.start_unix)]));
  const cpuByPid = new Map(rows.map((r) => [r.pid, rv(r.cpu_pct)]));
  const c = confirmable(pids, startByPid, cpuByPid);
  if (!c) { toast(label + ' is no longer running, or its current state could not be confirmed.'); return; }
  rec({ modal: { label, pids: c.pids, starts: c.starts, cpuPct: c.cpuPct, count: c.pids.length } });
}

