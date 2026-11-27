const labelOf = (key) => { const g = S.snap && S.snap.groups.find((x) => x.key === key); return g ? g.label : base(unprefixed(key)); };
function jsonOr(s, fallback) { try { return JSON.parse(s); } catch (e) { return fallback; } }
function refusedText(a) {
  const r = jsonOr(a.refused, []);
  return r.length ? r.length + ' refused: ' + r[0].reason + (r.length > 1 ? ', ...' : '') : '-';
}
// Every item of every request the Storage page sent to the Trash, newest first, as History recorded it (the audit rows
// are written before a move, so a request that stopped half way shows as interrupted, not as moved).
function trashMovesTable(q) {
  const all = trashHistoryRows(S.trashMoves), list = all.filter((r) => !q || (r.path + ' ' + r.stateText + ' ' + r.why + ' ' + r.requestText).toLowerCase().includes(q));
  const shown = sortRows(list, 'trash', { when: (r) => r.ts, request: (r) => r.request == null ? -1 : r.request, folder: (r) => r.path, state: (r) => r.stateText, size: (r) => r.bytes });
  const pg = paginate(shown, 'trash'), cols = 'grid-template-columns:140px 90px minmax(140px,2fr) minmax(130px,1fr) 84px minmax(100px,1.2fr)';
  return h('section', { class: 'card tbl', style: 'margin-bottom:20px' },
    sortHeader('trash', [{ label: 'When', sort: 'when' }, { label: 'Request', sort: 'request' }, { label: 'Folder', sort: 'folder' }, { label: 'State', sort: 'state' }, { label: 'Size', sort: 'size', cls: 'num' }, { label: 'Why' }], cols),
    pg.slice.map((r) => h('div', { class: 'tr', style: cols + ';cursor:default' },
      r.ts == null ? h('span', { class: 'mut small' }, '-') : timeCell(r.ts, 'mut small'), h('span', { class: 'mut small' }, r.requestText),
      h('span', nameAttrs(r.path), h('span', { style: 'display:flex;min-width:0' }, r.head ? h('span', ST_HEAD, r.head) : null, h('span', ST_TAIL, r.tail))),
      h('span', { class: 'small' }, r.stateText), h('span', { class: 'num' + (r.bytes == null ? ' mut small' : '') }, r.sizeText), h('span', { class: 'mut small', style: 'word-break:break-word' }, r.why || '-'))),
    shown.length ? null : h('div', { class: 'item' }, h('p', {}, all.length ? 'No moves match the search. ' : 'Nothing has been moved to the Trash from this page yet.'), all.length ? clearBtn() : null), pg.pager);
}
// Not named history: a global function of that name replaces window.history and breaks pushState.
function historyView() {
  const q = S.q.toLowerCase();
  const allHosts = [...S.seen].sort((a, b) => b.first_seen - a.first_seen); // newest first, baked into the base order
  const hostList = allHosts.filter((s) => !q || labelOf(s.group_key).toLowerCase().includes(q) || s.remote.toLowerCase().includes(q));
  const hostsShown = sortRows(hostList, 'hist', { when: (s) => s.first_seen, app: (s) => labelOf(s.group_key), host: (s) => s.remote, hits: (s) => s.hits });
  const hp = paginate(hostsShown, 'hist');
  const hostCols = 'grid-template-columns:140px minmax(80px,1fr) minmax(80px,1.2fr) 70px';
  const hostsTable = h('section', { class: 'card tbl', style: 'margin-bottom:20px' },
    sortHeader('hist', [{ label: 'When', sort: 'when' }, { label: 'App', sort: 'app' }, { label: 'Host', sort: 'host' }, { label: 'Hits', sort: 'hits', cls: 'num' }], hostCols),
    hp.slice.length ? hp.slice.map((s) => h('div', { class: 'tr', style: hostCols + ';cursor:default' },
      timeCell(s.first_seen, 'mut small'), h('span', { style: 'font-weight:500' }, labelOf(s.group_key)),
      h('span', { class: 'mut small', style: 'word-break:break-all' }, s.remote), h('span', { class: 'num' }, s.hits)))
      : h('div', { class: 'item' }, h('p', {}, allHosts.length ? 'No hosts match the search. ' : 'No new hosts recorded yet.'), allHosts.length ? clearBtn() : null), hp.pager);
  const allActs = [...S.acts].sort((a, b) => b.ts - a.ts); // newest first, baked into the base order
  const actList = allActs.filter((a) => !q || a.sig.toLowerCase().includes(q) || a.sent.includes(q) || a.refused.toLowerCase().includes(q));
  const actsShown = sortRows(actList, 'act', { when: (a) => a.ts, signal: (a) => a.sig, sent: (a) => jsonOr(a.sent, []).length, refused: (a) => refusedText(a) });
  const ap = paginate(actsShown, 'act');
  const actCols = 'grid-template-columns:140px 70px minmax(80px,1fr) minmax(80px,1.4fr)';
  const actsTable = h('section', { class: 'card tbl' },
    sortHeader('act', [{ label: 'When', sort: 'when' }, { label: 'Signal', sort: 'signal' }, { label: 'Sent', sort: 'sent', cls: 'num' }, { label: 'Refused', sort: 'refused' }], actCols),
    ap.slice.length ? ap.slice.map((a) => { const sent = jsonOr(a.sent, []); return h('div', { class: 'tr', style: actCols + ';cursor:default' },
      timeCell(a.ts, 'mut small'), h('span', {}, h('span', { class: 'tag', title: a.sig }, a.sig)),
      h('span', { class: 'small' }, sent.length ? sent.length + ' pid(s): ' + sent.join(', ') : '-'), h('span', { class: 'mut small' }, refusedText(a))); })
      : h('div', { class: 'item' }, h('p', {}, allActs.length ? 'No actions match the search. ' : 'No Stop/Quit actions recorded yet.'), allActs.length ? clearBtn() : null), ap.pager);
  const bp = paginate([...S.baseChanges], 'bc');
  const bcCols = 'grid-template-columns:140px 90px 80px minmax(80px,1fr)';
  const changesTable = h('section', { class: 'card tbl' },
    h('div', { class: 'tr h', style: bcCols }, ['When', 'Change', 'Kind', 'Item'].map((l) => h('span', {}, l))),
    bp.slice.length ? bp.slice.map((c) => h('div', { class: 'tr', style: bcCols + ';cursor:default' },
      timeCell(c.ts, 'mut small'), h('span', {}, h('span', { class: 'tag ' + (c.note ? 'bad' : '') , title: c.note || '' }, c.note ? 'not saved' : c.marked ? 'expected' : 'unmarked')),
      h('span', { class: 'mut small' }, c.kind), h('span', { class: 'mut small', style: 'word-break:break-all' }, c.key)))
      : h('div', { class: 'item' }, h('p', {}, 'Nothing marked as expected yet.')), bp.pager);
  return pageShell({ title: verdictTitle('history'), body: [h('h2', { class: 'sec' }, 'New hosts'), hostsTable, h('h2', { class: 'sec' }, 'Actions you took'), actsTable, h('h2', { class: 'sec' }, 'Moves to the Trash'), trashMovesTable(q), h('h2', { class: 'sec' }, 'Marked as expected'), changesTable] });
}

