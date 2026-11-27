// The detail under a row: the app (a guess), the terminal hint and every hop with the facts the collector read.
// Text only, and every value a stranger chose has been through `visible`; <bdi> keeps a right-to-left name from
// reordering its neighbours.
function requesterDetail(id, chain, note) {
  const app = requesterApp(chain), cut = chainCutNote(chain);
  return h('div', { class: 'reqdetail', id, role: 'group', 'aria-label': 'Requester detail' },
    h('p', { class: 'sum' }, h('strong', {}, 'App: '), h('bdi', {}, app.hint), h('span', { class: 'mut' }, '  ·  '), h('strong', {}, 'Terminal: '), h('bdi', {}, terminalNote(chain))),
    note ? h('p', { class: 'mut sum' }, h('bdi', {}, note)) : null,
    chain.length ? h('ol', {}, chain.map((hop, i) => h('li', {}, h('div', { class: 'hophead' }, h('span', { class: 'mut' }, i === 0 ? 'ran sudo' : 'started by the one above'), ' ', h('strong', { class: 'mono' }, h('bdi', {}, hopLabel(hop)))),
      h('dl', {}, hopFacts(hop, fmtWhen).flatMap(([k, v]) => [h('dt', {}, k), h('dd', {}, h('bdi', {}, v))])))), cut ? h('li', { class: 'mut' }, cut) : null) : h('p', {}, NO_PARENT));
}
// A collision-free element id for a row id: every character outside [A-Za-z0-9-] becomes _<hex>_, and `_` itself is one of them.
const reqDomId = (id) => 'req-' + [...id].map((c) => (/[A-Za-z0-9-]/.test(c) ? c : '_' + c.codePointAt(0).toString(16) + '_')).join('');
// The time cell with a button that opens the row's detail. `id` names the row (a time and a pid, never a command).
// The button has its own name (the time alone says nothing to a screen reader), points at the detail only while
// it exists, and carries `data-req` so render() can give it focus back after a poll redraws the page.
function reqToggle(id, cell, when) {
  const open = !!S.reqOpen[id];
  return h('button', { class: 'disc', type: 'button', 'data-req': id, 'aria-expanded': String(open), 'aria-controls': open ? reqDomId(id) : null, 'aria-label': (open ? 'Hide who ran this, ' : 'Show who ran this, ') + when,
    title: open ? 'Hide who ran this' : 'Show every program and script that led to this',
    onClick: () => rec({ reqOpen: Object.assign({}, S.reqOpen, { [id]: !open }) }) }, h('span', { 'aria-hidden': 'true', class: 'caret' }, open ? '▾' : '▸'), cell);
}
function priv() {
  const failed = failedSudo();
  const T = 'grid-template-columns:100px 70px minmax(180px,1.4fr) minmax(220px,2fr)';
  const TS = 'grid-template-columns:100px minmax(180px,1.2fr) minmax(130px,.8fr) minmax(220px,2fr)';
  const fRes = fv('auth-result');
  const sp = paginate(sortRows(S.sudo, 'sudo', { time: (e) => e.ts, command: (e) => e.args }), 'sudo');
  const authRows = S.priv.filter((e) => fRes === 'all' || (fRes === 'ok' ? e.ok : !e.ok));
  const ap = paginate(sortRows(authRows, 'auth', { time: (e) => e.time, result: (e) => (e.ok ? 'ok' : 'failed'), right: (e) => e.right, requester: (e) => base(e.client) }), 'auth');
  const wrap = { style: 'min-width:0;overflow-wrap:anywhere' };
  // Keep the open state of the rows on this page only, so it cannot grow with every row ever opened.
  const onPage = new Set([...sp.slice.map((e) => 'sudo:' + e.ts + ':' + e.pid), ...ap.slice.map((e) => 'auth:' + e.time + ':' + e.pid + ':' + e.right)]);
  if (Object.keys(S.reqOpen).some((k) => !onPage.has(k))) S.reqOpen = Object.fromEntries(Object.entries(S.reqOpen).filter(([k]) => onPage.has(k)));
  const sudoRow = (e) => {
    const id = 'sudo:' + e.ts + ':' + e.pid, has = Array.isArray(e.chain) && e.chain.length > 0;
    return [h('div', { class: 'tr', style: TS + ';cursor:default' }, has ? reqToggle(id, timeCell(e.ts), fmtWhen(e.ts)) : h('span', {}, timeCell(e.ts)), h('span', { class: 'small', ...wrap }, h('bdi', {}, cutText(e.args))), h('span', { class: 'small', ...wrap }, h('bdi', {}, requesterApp(e.chain).hint)), h('span', { class: 'mut small', ...wrap }, h('bdi', {}, chainLabel(e.chain)))),
      has && S.reqOpen[id] ? requesterDetail(reqDomId(id), e.chain) : null];
  };
  const authRow = (e) => {
    const id = 'auth:' + e.time + ':' + e.pid + ':' + e.right, cap = e.source === 'captured' && Array.isArray(e.chain) && e.chain.length > 0, has = cap || (e.source === 'ps' && !!e.parent);
    return [h('div', { class: 'tr', style: T + ';cursor:default' }, has ? reqToggle(id, h('span', {}, e.time.slice(11, 19)), e.time.slice(0, 19)) : h('span', {}, e.time.slice(11, 19)), h('span', {}, h('span', { class: 'tag ' + (e.ok ? 'ok' : 'bad') }, e.ok ? 'ok' : 'failed')), h('span', { class: 'mut small', ...wrap }, e.right + (e.error ? ' ' + e.error : '')), h('span', { class: 'small', ...wrap }, h('bdi', {}, authRequester(e)))),
      has && S.reqOpen[id] ? requesterDetail(reqDomId(id), cap ? e.chain : [], authNote(e)) : null];
  };
  return pageShell({ title: verdict('priv', { failed: failedAuth() }), body: [
    failed ? h('section', { class: 'card', style: 'margin-bottom:20px' }, h('div', { class: 'item' }, h('span', { class: 'sev medium' }), h('div', { class: 'grow' }, h('h3', {}, failed + ' failed sudo authorizations in the last 2 hours'), h('p', {}, 'Rapid repeats suggest a script retrying, not a person. Open a row to see every program and script that led to it. A request marked "no matching launch recorded" was launched before Monitor Lizard was running, exited before capture, or is older than the records kept.')))) : null,
    h('h2', { class: 'sec' }, 'sudo / su launches (live capture)'),
    h('div', { class: 'card tbl', style: 'margin-bottom:20px' }, sortHeader('sudo', [{ label: 'Time', sort: 'time' }, { label: 'Command', sort: 'command' }, { label: 'App (a guess)', id: 'app' }, { label: 'Requester chain', id: 'requester_chain' }, { label: 'Full chain', id: 'requester_detail', exportOnly: true }], TS),
      S.sudo.length ? sp.slice.map(sudoRow) : h('div', { class: 'item' }, h('p', {}, 'None captured yet. Anything that runs sudo or su while Monitor Lizard is serving shows up here.')), sp.pager),
    h('div', { style: 'display:flex;align-items:center;gap:12px;margin-bottom:8px' }, h('h2', { class: 'sec', style: 'margin:0' }, 'Authorization requests (system log)'), h('span', { class: 'grow' }),
      dropdown('f-auth-result', fRes, [['all', 'Any result'], ['failed', 'Failed'], ['ok', 'OK']], (v) => setFil('auth-result', v), 'Result filter', true)),
    h('div', { class: 'card tbl' }, sortHeader('auth', [{ label: 'Time', sort: 'time' }, { label: 'Result', sort: 'result' }, { label: 'Right', sort: 'right' }, { label: 'Requester', sort: 'requester' }, { label: 'App (a guess)', id: 'app', exportOnly: true }, { label: 'Full chain', id: 'requester_detail', exportOnly: true }, { label: 'Requester note', id: 'requester_note', exportOnly: true }], T),
      ap.slice.map(authRow), ap.slice.length ? null : h('div', { class: 'item' }, h('p', {}, authRows.length ? 'No requests match the filter. ' : 'No authorization requests yet.'), authRows.length ? clearBtn() : null), ap.pager),
  ] });
}

