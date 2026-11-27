// ---- Storage page. One GET when the page opens and one per Rescan; while a scan runs it is read every 2 s so the
// page can say how long it has taken and show the result the moment it lands. A failed read is retried with a growing pause.
let stTimer = 0, stSeq = 0, stFails = 0, stHistAt = 0;
async function loadStorage(fresh) {
  if (S.view !== 'storage' || (document.hidden && !fresh)) return;
  clearTimeout(stTimer);
  if (fresh || Date.now() - stHistAt > 60000) loadStorageHistory();
  const mine = ++stSeq, before = { data: S.stData, failed: S.stFailed };
  let after, note = null;
  try {
    const r = await fetch('/api/storage' + (fresh ? '?fresh=1' : ''),
      { headers: { 'X-MonitorLizard-Token': TOKEN } });
    if (!r.ok) throw new Error('storage read failed');
    const data = await r.json();
    if (!data || typeof data !== 'object' || Array.isArray(data)) throw new Error('empty reply');
    if (staleReply(mine, stSeq)) return;
    stFails = 0;
    // A Rescan inside the server's minute gets 200, the old result and scanning false: say so, do not pretend a scan began.
    if (fresh && !data.scanning) note = rescanInfo(data,
      Date.now() / 1000).text || 'The server did not start a scan.';
    after = { data, failed: false };
    quiet({ stData: data, stFailed: false, stNote: note });
  } catch (e) {
    if (staleReply(mine, stSeq)) return;
    stFails++;
    after = { data: S.stData, failed: true };
    quiet({ stFailed: true, stNote: null });
  }
  const say = storageAnnouncement(before, after, Date.now() / 1000) || note;
  if (say) announce(say);
  const wait = after.failed ? retryDelay(stFails) : after.data.scanning ? 2000 : 0;
  if (wait) stTimer = setTimeout(() => loadStorage(false), wait);
}
function rescanNow() {
  const info = rescanInfo(S.stData, Date.now() / 1000);
  if (info.ok) { loadStorage(true); return; }
  announce(info.text);
}
// Picking the tile that is already picked clears it. Focus goes back to the tile, since render() rebuilds it.
function pickTile(id) {
  rec({ stTile: id === S.stTile ? 'all' : id, stItem: null, page: {} });
  const el = document.querySelector('[data-tile="' + id + '"]'); if (el) el.focus();
}
function tileKeys(e) {
  const ids = [...e.currentTarget.querySelectorAll('.tm-tile')].map((el) => el.dataset.tile);
  const to = nextTile(ids, document.activeElement && document.activeElement.dataset.tile, e.key);
  if (!to) return;
  e.preventDefault();
  e.currentTarget.querySelector('[data-tile="' + to + '"]').focus();
}
const labelTag = (label) => (label
  ? h('span', { class: 'tag ' + ({ [ST_LABEL.safe]: 'ok', [ST_LABEL.review]: 'warn' }[label] || ''), style: 'flex:none' }, label)
  : h('span', { class: 'mut small' }, '-'));
function stWaiting(state, d, now) {
  const row = (w) => h('div', { class: 'tr', style: 'min-width:0;grid-template-columns:1fr 90px 90px' },
    h('span', { class: 'bar' }, h('i', { style: 'width:' + w + '%;opacity:.5' })), h('span', { class: 'bar' }),
      h('span', { class: 'bar' }));
  const skeleton = h('section', { class: 'card', 'aria-hidden': 'true' }, [70, 45, 30, 20].map(row));
  const say = {
    loading: 'Reading the storage scan...', failed: refreshFailure(null, now), empty: 'No scan result yet.',
    refused: 'No scan was made: ' + (d && d.refused), scanning: scanningText(d, now),
  }[state];
  return h('div', {}, h('p', { class: 'mut' }, say), state === 'refused' || state === 'failed' ? null : skeleton);
}
// The capacity history under the startup disk bar: the weekly change, the forecast, and the chart. The chart is a picture of
// numbers the text already states, so the svg carries the same sentence as its label.
async function loadStorageHistory() {
  stHistAt = Date.now();
  try {
    const r = await fetch('/api/storage/history');
    if (!r.ok) throw new Error('history read failed');
    const data = await r.json();
    if (!data || typeof data !== 'object' || Array.isArray(data)) throw new Error('empty reply');
    quiet({ stHist: data, stHistFailed: false });
  } catch (e) { quiet({ stHistFailed: true }); }
}
function stHistory() {
  const hist = S.stHist, fmt = (t) => new Date(t * 1000).toLocaleDateString(undefined, { month: 'short', day: 'numeric' }), out = capacityOutlook(hist, S.stHistFailed && !hist, fmt), week = weekText(hist), chart = capacityChart(hist), last = lastSampleText(hist, fmt);
  const label = capacityChartLabel(chart, out);
  const svg = chart ? h('svg', { class: 'cap-chart', viewBox: '0 0 ' + chart.w + ' ' + chart.h, role: 'img', 'aria-label': label, style: 'width:100%;height:auto;max-height:160px;display:block;margin-top:8px' }) : null;
  if (svg) svg.innerHTML = (chart.topY == null ? '' : '<line x1="0" x2="' + chart.w + '" y1="' + chart.topY + '" y2="' + chart.topY + '" stroke="var(--mut)" stroke-width="1" vector-effect="non-scaling-stroke"></line>')
    + '<path d="' + chart.line + '" fill="none" stroke="var(--acc)" stroke-width="2" vector-effect="non-scaling-stroke"></path>'
    + (chart.dash ? '<path d="' + chart.dash + '" fill="none" stroke="var(--warn)" stroke-width="2" stroke-dasharray="6 5" vector-effect="non-scaling-stroke"></path>' : '');
  const when = (t) => new Date(t * 1000).toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
  return h('div', { class: 'cap-history', style: 'margin-top:14px;border-top:1px solid var(--line);padding-top:12px' },
    h('h3', { class: 'sec', style: 'margin:0 0 6px' }, 'Growth'),
    h('dl', { class: 'kv', style: 'margin:0;padding:0;border-top:0' },
      h('dt', {}, 'This week'), h('dd', { id: 'cap-week', style: 'word-break:normal' }, week || 'Not available'),
      h('dt', {}, 'Disk full'), h('dd', { id: 'cap-forecast', style: 'word-break:normal' }, out.headline + (out.detail ? '. ' + out.detail : '')),
      last ? [h('dt', {}, 'Last sample'), h('dd', { id: 'cap-last', style: 'word-break:normal' }, last)] : null),
    out.note ? h('p', { class: 'small mut', style: 'margin:6px 0 0' }, out.note) : null,
    svg, svg ? h('p', { class: 'small mut', style: 'margin:4px 0 0', 'aria-hidden': 'true' }, 'Used space, ' + when(chart.from) + ' to ' + when(chart.to) + '.'
      + (chart.dash ? ' Dashed line: the forecast.' : '') + ' Top line: the disk size.') : null);
}
function stStrip(d, state, now) {
  const snaps = d.snapshots || {}, n = rv(snaps.count);
  const size = rv(snaps.size) == null ? ', size unavailable: ' + rReason(snaps.size) : ', '
    + formatBytes(rv(snaps.size));
  const snapText = n == null ? 'unavailable: ' + rReason(snaps.count) : plural(n, 'snapshot', 'snapshots') + size;
  const done = state === 'complete';
  const stale = staleScan(d.scanned_at, now) ? h('span', { class: 'tag warn',
    title: 'Older than 15 minutes; Rescan reads it again' }, 'Stale') : null;
  return card(h('h2', { class: 'sec', style: 'margin:0 0 10px' }, 'Startup disk'),
    capacityBar(volumeBarArgs(d.capacity)),
    h('p', { class: 'small', style: 'margin:12px 0 0;display:flex;gap:8px;align-items:center;flex-wrap:wrap' },
      h('span', { class: 'tag ' + (done ? 'ok' : 'warn') }, done ? 'Complete' : 'Partial'), stale,
      h('span', {}, coverageText(d) + ', ' + measuredText(d.scanned_at, now))),
    h('p', { class: 'small mut', style: 'margin:6px 0 0' }, 'Local snapshots: ' + snapText),
    stHistory());
}
function stTileNote(t) {
  if (t.bytes == null) return t.reason;
  if (t.kind === 'rest') return REST_NOT_FREE + ' Used space outside the folders this scan reads' + (t.approx
    ? ', roughly' : '');
  if (t.kind === 'other') return plural(t.members.length, 'small section', 'small sections');
  return t.unmeasured ? plural(t.unmeasured, 'folder', 'folders') + ' not fully measured' : null;
}
function stTileButton(t, tile, state) {
  const na = t.bytes == null, on = tile === t.id, size = tileSizeText(t), note = stTileNote(t);
  const share = t.diskPct == null ? null : shareText({ share: t.diskPct }) + ' of the disk';
  const spoken = t.name + ': ' + size + (share ? ', ' + share : '') + (note ? '. ' + note : '')
    + (state === 'partial' ? '. Scan is partial' : '') + (on ? '. Selected' : '')
      + '. Select to filter the folder table.';
  return h('button', { class: 'tm-tile' + (na || t.kind === 'rest' ? ' na' : '') + (on ? ' on' : ''), type: 'button',
    'data-tile': t.id, 'aria-pressed': String(on), style: 'flex:' + t.pct.toFixed(3) + ' 1 0', 'aria-label': spoken,
      onClick: () => pickTile(t.id) },
  h('span', { class: 'tm-name' }, t.name), h('span', { class: 'tm-size' }, size),
  share ? h('span', { class: 'small mut' }, share) : null, note ? h('span', { class: 'small mut' }, note) : null,
  h('span', { class: 'bar tm-bar', 'aria-hidden': 'true' }, h('i', { style: 'width:' + Math.min(100,
    t.pct).toFixed(1) + '%' })));
}
const stKv = (...pairs) => h('dl', { class: 'kv' }, pairs.filter(Boolean).map(([k, v]) => [h('dt', {}, k), h('dd', {},
  v)]));
const stPart = (title, ...kids) => h('div', { class: 'insp-sec' }, h('h3', { class: 'sec' }, title), ...kids);
const stRelated = () => stPart('Related', h('a', { href: writeHash({ view: 'performance',
  ex: { perfTab: 'disknet' } }) }, 'Disk throughput on Performance'));
// Not a disabled attribute: that would take the button out of the tab order, and a keyboard user would never learn why.
// `rows` are the selected rows; the button opens a preview and never moves anything itself.
function stTrashButton(rows) {
  const sel = trashSelection(rows || [], (rows || []).map((r) => r.id), !safeKnown(S.stData)), yours = sel.yours.length > 0;
  const help = sel.ok ? 'Shows what would move first. Nothing moves until you confirm.' + (yours ? ' Your own folder asks for a second confirmation.' : '') : sel.why;
  return h('div', { style: 'display:flex;gap:8px;align-items:center;flex-wrap:wrap' },
    h('button', { class: 'btn btn-danger quiet sm', id: 'st-trash-btn', 'data-req': 'st-trash-btn', type: 'button', 'aria-disabled': sel.ok ? null : 'true',
      'aria-describedby': 'st-trash-why', onClick: (e) => { if (!sel.ok) { e.preventDefault(); announce(sel.why); return; } openTrash(sel); } },
    'Move to Trash'),
    h('span', { class: 'small mut', id: 'st-trash-why' }, help));
}
function stItemFacts(sel) {
  const gone = sel.kind === 'pending', sparse = sel.apparent != null && sel.bytes != null && sel.apparent > sel.bytes;
  return stKv(['Path', sel.path], ['Size', sizeText(sel)],
    sparse && ['Apparent', formatBytes(sel.apparent) + ' (sparse: the files claim more than they use)'],
    !gone && ['Section', ST_CATEGORIES[sel.category] || sel.category], !gone && ['Files', (Number.isFinite(sel.entries) ? String(sel.entries) : 'not reported')],
    !gone && ['Measured', timeCell(sel.at, 'mut')],
    sel.unreadable > 0 && ['Unreadable', plural(sel.unreadable, 'subfolder',
      'subfolders') + ' could not be read, so the size is at least this']);
}
function stItemInspector(sel) {
  const gone = sel.kind === 'pending', yours = sel.label === ST_LABEL.yours;
  const why = gone ? stPart('Not yet scanned', h('p', { style: 'margin:0;font-size:13px' },
    'The scan ran out of time before this folder. Rescan reads it again.'))
    : stPart('Why ' + sel.label.toUpperCase(), h('p', { style: 'margin:0;font-size:13px' },
      ST_RULE_TEXT[sel.rule] || 'Not on the safe list, so it needs a look first.'));
  const copy = gone ? null : h('button', { class: 'btn sm', type: 'button', onClick: () => copyRow('store', sel) },
    'Copy as JSON');
  return h('aside', { class: 'drawer card', 'aria-label': 'Folder detail' },
    h('div', { class: 'insp-head' }, h('div', { class: 'grow',
      style: 'font-size:16px;font-weight:600;word-break:break-all' }, sel.name),
      labelTag(sel.label), h('button', { class: 'btn sm', type: 'button', onClick: () => rec({ stItem: null }) },
        'Close')),
    stItemFacts(sel), why,
    h('div', { style: 'display:flex;gap:8px;padding:0 16px 14px;flex-wrap:wrap' }, copy,
      gone ? null : stTrashButton([sel])),
    yours ? h('p', { class: 'small mut', style: 'margin:0;padding:0 16px 14px' },
      'These are your own files. Moving them to the Trash asks for a second confirmation that names the folder.') : null,
    stRelated());
}
function stSectionExtra(t) {
  if (!t) return null;
  const para = (text) => h('p', { style: 'margin:0;font-size:13px' }, text);
  if (t.kind === 'rest') {
    const why = t.approx ? ' It is a rough figure because ' + t.why + ', and those bytes land here.' : '';
    return stPart('What this is',
      para(REST_NOT_FREE + ' It is the used total minus what was measured: files this scan does not look inside.'
        + why));
  }
  if (t.kind !== 'other') return null;
  const line = (m) => h('li', {}, m.name + ': ' + (m.bytes == null ? 'unavailable: ' + m.reason : atLeast(m.bytes,
    m.lowerBound)));
  return stPart('In Other', h('ul', { style: 'margin:0;padding:0;list-style:none;font-size:13px' },
    t.members.map(line)));
}
function stSectionInspector(d, state, rows, tiles, tile) {
  const t = tiles.find((x) => x.id === tile), mine = visibleRows(rows, tiles, tile, ST_ALL);
  const c = labelCounts(mine), pending = mine.filter((r) => r.kind === 'pending').length;
  const measured = t ? tileSizeText(t) + (t.bytes == null ? ': ' + t.reason : '') : atLeast(d.coverage.bytes_measured,
    state !== 'complete');
  const folders = String(mine.length - pending) + (pending ? ', ' + plural(pending, 'folder',
    'folders') + ' not yet scanned' : '');
  const showAll = tile !== ST_ALL ? h('button', { class: 'btn sm', type: 'button',
    onClick: () => rec({ stTile: ST_ALL, stItem: null, page: {} }) }, 'Show all') : null;
  return h('aside', { class: 'drawer card', 'aria-label': 'Section detail' },
    h('div', { class: 'insp-head' }, h('h2', { class: 'grow', style: 'font-size:16px;font-weight:600;margin:0' },
      t ? t.name : 'All sections'), showAll),
    stKv(['Measured', measured], ['Folders', folders], ['Labels',
      c.safe + ' safe, ' + c.review + ' review, ' + c.yours + ' yours'],
      ['Safe to remove', safeTotalText(d, mine)]),
    stSectionExtra(t), stRelated());
}
const ST_ALL = 'all';
const stFilterChips = (counts) => [[ST_ALL, 'All'], [ST_LABEL.safe, 'Safe'], [ST_LABEL.review, 'Review'],
  [ST_LABEL.yours, 'Yours']].map(([id, l]) =>
  h('button', { class: 'chip ' + (S.stfilter === id ? 'on' : ''), type: 'button',
    'aria-pressed': String(S.stfilter === id),
    onClick: () => rec({ stfilter: id, page: {} }) }, l + ' ', counts[id]));
const ST_COLS = 'grid-template-columns:minmax(170px,2fr) 100px 84px minmax(150px,1fr) minmax(110px,.9fr) 96px';
const ST_HEAD = { style: 'flex:0 1 auto;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap' };
const ST_TAIL = { style: 'flex:0 0 auto;max-width:75%;overflow:hidden;text-overflow:ellipsis;white-space:nowrap' };
function stRow(r, sel) {
  const on = !!sel && sel.id === r.id, share = r.share == null ? null
    : h('span', { class: 'bar', style: 'flex:1 1 auto;min-width:30px' }, h('i', { style: 'width:' + Math.min(100,
      r.share).toFixed(1) + '%' }));
  return h('button', { class: 'tr ell' + (on ? ' on' : ''), type: 'button', style: ST_COLS,
    'aria-pressed': String(on), onClick: () => rec({ stItem: r.id, stMulti: [] }) },
    // decorateTables resets each cell's display, so the flex row is an inner element.
    h('span', nameAttrs(r.path), h('span', { style: 'display:flex;min-width:0' }, r.head ? h('span', ST_HEAD, r.head) : null, h('span', ST_TAIL, r.tail))), h('span', { class: 'mut small' }, ST_CATEGORIES[r.category] || '-'),
      h('span', {}, labelTag(r.label)),
    h('span', { class: 'num' + (r.bytes == null ? ' mut small' : '') }, sizeText(r)),
    h('span', { style: 'display:flex;align-items:center;gap:8px' }, share, h('span', { class: 'mut small' },
      shareText(r))),
    r.kind === 'pending' ? h('span', { class: 'mut small' }, '-') : timeCell(r.at, 'mut small'));
}
function stTable(shown, sel, tile) {
  const pg = paginate(shown, 'store');
  const reset = () => { S.q = ''; $('q').value = ''; rec({ stTile: ST_ALL, stfilter: ST_ALL, page: {} }); };
  const none = tile === 'rest' ? 'This part of the disk is not in any folder this scan reads, so there are no rows.'
    : 'No folders match the current filters. ';
  return h('section', { class: 'card tbl', style: 'min-width:0' },
    sortHeader('store', [{ label: 'Folder', sort: 'folder' }, { label: 'Section', sort: 'section' }, { label: 'Label', sort: 'label' },
      { label: 'Size', sort: 'size', cls: 'num' }, { label: 'Share of disk', sort: 'share' }, { label: 'Measured' }], ST_COLS, 'ell'),
    pg.slice.map((r) => stRow(r, sel)),
    shown.length ? null : h('div', { class: 'item' }, h('p', {}, none), h('button', { class: 'btn sm', type: 'button',
      onClick: reset }, 'Show all folders')), pg.pager);
}
function stTreemap(tiles, tile, state) {
  const rest = tiles.find((t) => t.kind === 'rest');
  const keys = 'Sections by size. Arrow keys move between sections; Enter selects one and filters the table.';
  const map = h('div', { role: 'group', 'aria-label': keys, onKeydown: tileKeys },
    h('div', { class: 'tm' }, treemapColumns(tiles.filter((t) => t.kind !== 'rest')).map((c) =>
      h('div', { class: 'tm-col', style: 'flex:' + c.pct.toFixed(3) + ' 1 0' }, c.tiles.map((t) => stTileButton(t,
        tile, state))))),
    rest ? h('div', { class: 'tm-rest' }, stTileButton(rest, tile, state)) : null);
  const heading = state === 'complete' ? 'Sections' : 'Sections measured so far, not final';
  return h('section', { class: 'card', style: 'padding:16px;margin-bottom:14px', 'aria-label': 'Storage by section' },
    h('div', { style: 'display:flex;align-items:center;gap:10px;flex-wrap:wrap;margin-bottom:10px' },
      h('h2', { class: 'sec', style: 'margin:0' }, heading), state === 'partial' ? h('span', { class: 'tag warn' },
        'Partial') : null),
    tiles.length ? map : h('p', { class: 'mut', style: 'margin:0' }, 'No section has been measured yet.'));
}
function stBanner(d, state) {
  if (state !== 'partial') return null;
  return h('section', { class: 'card', role: 'note', 'aria-label': 'Partial scan',
    style: 'padding:12px 16px;margin-bottom:14px;border-color:var(--warn)' },
    h('span', { class: 'tag warn' }, 'Partial'),
    ' This scan did not finish every folder (' + coverageText(d)
      + '). Sizes marked "at least" can only grow. The treemap is not final.'
    + ' No safe-to-remove total appears until a scan completes.');
}
function stActions(d, now) {
  const info = rescanInfo(d, now), busy = !!(d && d.scanning);
  const hint = info.ok ? 'Scans the known folders again. It only reads, and takes up to 20 seconds.' : info.text;
  // aria-disabled, not disabled: the button stays in the tab order and its reason is read from the text beside it.
  // rescanNow reads the live scan state, so a click while it cannot act announces why and sends nothing.
  // data-req: render() gives the keyboard back to this button after a poll or a click redraws the page.
  return [info.ok ? null : h('span', { class: 'small mut', id: 'st-rescan-why' }, busy ? scanningText(d, now) : info.text),
    h('button', { class: 'btn sm', id: 'st-rescan-btn', 'data-req': 'st-rescan-btn', type: 'button', 'aria-disabled': info.ok ? null : 'true',
      'aria-describedby': info.ok ? null : 'st-rescan-why', title: hint, onClick: rescanNow },
      'Rescan'),
    d && typeof d.scanned_at === 'number' ? h('button', { class: 'btn sm', id: 'st-export-btn', 'data-req': 'st-export-btn', type: 'button', 'aria-expanded': String(S.stAsk),
      'aria-controls': 'st-export', onClick: () => stFocus(S.stAsk ? 'st-export-btn' : 'st-export-go', { stAsk: !S.stAsk }) }, 'Export HTML report') : null];
}
// Clean-ups list: the folders the scan labelled, largest first, top ST_CLEANUP_TOP unless "Show all" is on.
// Each grid row has 12 px gaps and 16 px of padding on both sides (the `.tr` rule), so a list needs
// its track minimums + gaps + 32 px. Keep each track list in step with the column string beside it. A card narrower than that scrolls inside itself instead of widening the page.
const CLEAN_TRACKS = [120, 120, 74, 90, 84];
const CLEAN_COLS = 'grid-template-columns:minmax(120px,2fr) minmax(120px,2fr) 74px minmax(90px,.7fr) 84px';
const SYSTEM_TRACKS = [110, 80, 150];
const SYSTEM_COLS = 'grid-template-columns:minmax(110px,1fr) minmax(80px,2fr) minmax(150px,1fr)';
const rowFloor = (tracks) => tracks.reduce((a, b) => a + b, 0) + 12 * (tracks.length - 1) + 32;
// A box that scrolls sideways and can be reached by keyboard, named as table.js names its scrolling tables.
const scrollBox = (name, floor, ...kids) => h('div', { class: 'sbox', style: 'overflow-x:auto', tabindex: '0', role: 'region', 'aria-label': name + ', scrollable' },
  h('div', { style: 'min-width:' + floor + 'px' }, ...kids));
const listReset = 'list-style:none;margin:0;padding:0';
// Focus goes back to the control that opened a panel when it closes: render() rebuilds every control.
const stFocus = (id, m) => { rec(m); const el = $(id); if (el) el.focus(); };
function cleanupRow(r, on) {
  return h('li', { class: 'tr' + (on ? ' on' : ''), style: CLEAN_COLS + ';min-width:0' },
    h('span', nameAttrs(r.path), h('span', { style: 'display:flex;min-width:0' }, r.head ? h('span', ST_HEAD, r.head) : null, h('span', ST_TAIL, r.tail))),
    h('span', { class: 'small mut' }, r.why), h('span', {}, labelTag(r.label)),
    h('span', { class: 'num' + (r.bytes == null ? ' mut small' : '') }, sizeText(r)),
    h('button', { class: 'btn sm', type: 'button', 'aria-pressed': String(on), 'aria-label': 'Review ' + r.name + ', ' + r.label + ', ' + sizeText(r),
      onClick: () => rec({ stItem: r.id, stMulti: [] }) }, 'Review'));
}
const stPickSig = () => [S.stTile, S.stfilter, S.q].join('|');
function storageCleanups(list, sel, picked) {
  const v = cleanupView(list, S.stAll), safe = safeIds(list), none = !safe.length;
  const reviewAll = h('button', { class: 'btn sm', type: 'button', 'data-req': 'st-review-all', 'aria-disabled': none ? 'true' : null, 'aria-describedby': none ? 'st-cleanups-sum' : null,
    title: none ? 'Nothing safe to remove' : 'Select every safe folder in the inspector',
    onClick: (e) => {
      if (none) { e.preventDefault(); announce('Nothing safe to remove'); return; }
      // Every safe folder must be in the table the selection is tied to, so the filters go back to all.
      S.q = ''; $('q').value = ''; S.stTile = ST_ALL; S.stfilter = ST_ALL; S.stMultiSig = stPickSig();
      rec({ stMulti: safe, stItem: null, page: {} });
      announce(safeSelectedLine(S.stData, list));
    } }, 'Review all safe');
  const pickedIds = new Set(picked.map((r) => r.id));
  const colHead = h('div', { class: 'small mut', 'aria-hidden': 'true', style: CLEAN_COLS + ';display:grid;gap:12px;padding:0 16px 6px' },
    h('span', {}, 'Folder'), h('span', {}, 'Why'), h('span', {}, 'Label'), h('span', {}, 'Size'), h('span', {}));
  return h('section', { class: 'card', style: 'padding:16px;margin-bottom:14px', 'aria-label': 'Suggested clean-ups' },
    h('div', { style: 'display:flex;align-items:center;gap:10px;flex-wrap:wrap;margin-bottom:6px' },
      h('h2', { class: 'sec', style: 'margin:0' }, 'Suggested clean-ups'), h('span', { class: 'grow' }), reviewAll),
    h('p', { class: 'small mut', id: 'st-cleanups-sum', style: 'margin:0 0 10px' }, cleanupSummary(list) + '. ' + safeCleanupLine(list)
      + safeSummaryTail(S.stData, list)),
    list.length ? [scrollBox('Suggested clean-ups', rowFloor(CLEAN_TRACKS), colHead,
        h('ul', { style: listReset, 'aria-label': 'Folders, largest first' }, v.shown.map((r) => cleanupRow(r, (!!sel && sel.id === r.id) || pickedIds.has(r.id))))),
      v.hidden ? h('div', { style: 'padding-top:10px;display:flex;gap:10px;align-items:center' },
        h('span', { class: 'small mut' }, 'Showing the largest ' + v.shown.length + '; ' + v.hidden + ' more.'),
        h('button', { class: 'btn sm', type: 'button', onClick: () => rec({ stAll: true }) }, 'Show all')) : null,
      S.stAll && list.length > ST_CLEANUP_TOP ? h('div', { style: 'padding-top:10px' },
        h('button', { class: 'btn sm', type: 'button', onClick: () => rec({ stAll: false }) }, 'Show the top ' + ST_CLEANUP_TOP)) : null]
      : h('p', { class: 'mut', style: 'margin:0' }, 'No labelled folder has been measured yet.'));
}
function systemDataCard(d) {
  const rows = systemData(d);
  return h('section', { class: 'card', style: 'padding:16px;margin-bottom:14px', 'aria-label': 'Inside System Data' },
    h('h2', { class: 'sec', style: 'margin:0 0 10px' }, 'Inside System Data'),
    scrollBox('Inside System Data', rowFloor(SYSTEM_TRACKS), h('ul', { style: listReset, 'aria-label': 'Parts of System Data' }, rows.map((r) => h('li', { class: 'tr', style: SYSTEM_COLS + ';min-width:0;align-items:center' },
      h('span', {}, h('div', {}, r.name), r.detail ? h('div', { class: 'small mut' }, r.detail) : null),
      r.share == null ? h('span', { class: 'small mut', 'aria-hidden': 'true' }, 'No bar')
        : h('span', { class: 'bar', role: 'img', 'aria-label': r.name + ': ' + shareText({ share: r.share }) + ' of what is shown here' }, h('i', { style: 'width:' + Math.min(100, r.share).toFixed(1) + '%' })),
      h('span', { class: 'num' + (r.bytes == null ? ' mut small' : '') }, h('span', { class: 'sr-only' }, r.name + ': '), systemText(r)))))));
}
function stExportPanel(d, rows, shown, now, filterText) {
  if (!S.stAsk) return null;
  return h('section', { class: 'card', id: 'st-export', role: 'group', 'aria-label': 'Export HTML report', style: 'padding:12px 16px;margin-bottom:14px' },
    h('p', { style: 'margin:0 0 8px' }, 'The report lists folder paths, with your home folder shown as ~. It is one file with no network use, saved to your Downloads folder. It names your folders, so share it only with people you trust.'),
    h('div', { style: 'display:flex;gap:8px' },
      h('button', { class: 'btn btn-acc sm', id: 'st-export-go', type: 'button', onClick: () => exportReport(d, rows, shown, now, filterText) }, 'Download report'),
      h('button', { class: 'btn sm', type: 'button', onClick: () => stFocus('st-export-btn', { stAsk: false }) }, 'Cancel')));
}
function exportReport(d, rows, shown, now, filterText) {
  const html = reportHtml({ d, rows, shownRows: shown, system: systemData(d), generated: new Date(now * 1000).toLocaleString(), filterText });
  download(fileName('storage-report', 'html', new Date(now * 1000)), 'text/html', html);
  stFocus('st-export-btn', { stAsk: false, toast: 'Report downloaded.' });
}
function stMultiInspector(picked, total) { // `total` is already text: a partial scan passes the withheld line
  return h('aside', { class: 'drawer card', 'aria-label': 'Selected folders' },
    h('div', { class: 'insp-head' }, h('h2', { class: 'grow', style: 'font-size:16px;font-weight:600;margin:0' }, plural(picked.length, 'safe folder', 'safe folders') + ' selected'),
      h('button', { class: 'btn sm', type: 'button', onClick: () => rec({ stMulti: [] }) }, 'Clear')),
    stKv(['Total', total]),
    stPart('Folders', h('ul', { style: 'margin:0;padding:0;list-style:none;font-size:13px' }, picked.map((r) => h('li', { style: 'display:flex;gap:8px;justify-content:space-between;word-break:break-all' }, h('span', {}, r.name), h('span', { class: 'mut', style: 'flex:none' }, sizeText(r)))))),
    h('div', { style: 'padding:0 16px 14px' }, stTrashButton(picked)));
}
function storage() {
  const d = S.stData, now = Date.now() / 1000, state = storageState({ data: d, failed: S.stFailed && !S.stData });
  const title = verdict('storage', storageFacts(d, now)), actions = stActions(d, now);
  const note = S.stNote ? h('p', { class: 'small mut' }, S.stNote) : null;
  if (state !== 'partial' && state !== 'complete') return pageShell({ title, actions, body: [note, stWaiting(state, d,
    now)] });
  const failed = S.stFailed ? h('p', { class: 'small', style: 'color:var(--warn)' }, refreshFailure(d, now)) : null;
  const rows = storageRows(d), tiles = storageTiles(d), tile = resolveTile(S.stTile, tiles);
  for (const r of rows) r.measuredText = r.kind === 'pending' ? 'not yet scanned' : measuredText(r.at, now);
  const found = searchRows(visibleRows(rows, tiles, tile, ST_ALL), S.q);
  const list = found.filter((r) => S.stfilter === ST_ALL || r.label === S.stfilter).sort(bySize);
  const shown = sortRows(list, 'store', { folder: (r) => r.path, section: (r) => r.category, label: (r) => r.label,
    size: (r) => r.bytes, share: (r) => r.share });
  const sel = rows.find((r) => r.id === S.stItem) || null;
  // A selection belongs to the filters it was made under: changing the section, label or search drops it.
  if (S.stMulti.length && S.stMultiSig !== stPickSig()) S.stMulti = [];
  const cleanList = stCleanups(rows), picked = sel ? [] : pickedRows(cleanList, S.stMulti);
  if (trashOpen()) { const now = sel ? [sel] : picked; trashSync(trashSelection(now, now.map((r) => r.id), !safeKnown(S.stData))); }
  const filterText = (tile === ST_ALL ? 'All sections' : 'Section: ' + tile) + ', ' + (S.stfilter === ST_ALL ? 'all labels' : 'label ' + S.stfilter) + (S.q.trim() ? ', search "' + S.q.trim() + '"' : '');
  return pageShell({ title, subtitle: coverageText(d) + ', ' + measuredText(d.scanned_at, now), actions,
    filters: stFilterChips(labelCounts(found)),
    body: h('div', { style: 'flex:999 1 640px;min-width:0' }, note, failed, stStrip(d, state, now), stBanner(d,
      state), stExportPanel(d, rows, shown, now, filterText), stTreemap(tiles, tile, state),
      storageCleanups(cleanList, sel, picked), systemDataCard(d), stTable(shown, sel, tile)),
    inspector: sel ? stItemInspector(sel) : picked.length ? stMultiInspector(picked, safeSumFor(d, picked)) : stSectionInspector(d, state, rows, tiles, tile) });
}

/* storage-trash-view:begin */
// ---- Move to Trash: the dialog around the state machine in page-storage-logic.js (`storage-trash`).
// The page sends ids, never paths, through api() (the per-run token header) and keeps no token in the address.
// Every folder name, reason and note a stranger could have chosen goes through visible(), so a bidi override, a
// newline or a zero-width character shows as an escape instead of rearranging what the confirmation says.
const trashOpen = () => !!(S.trash && S.trash.phase !== 'idle');
const trashFocusBack = () => { const t = $('st-trash-btn') || $('main'); if (t) t.focus(); };
function openTrash(sel) { trashDispatch({ type: 'open', selection: sel }); }
// Runs the machine and its effects without drawing. Returns true when focus should go back to where the dialog began.
function trashApply(ev) {
  const r = trashStep(S.trash || trashIdle(), ev);
  S.trash = r.state;
  let focus = false;
  for (const fx of r.fx) {
    if (fx.do === 'announce') announce(fx.text);
    else if (fx.do === 'preview' || fx.do === 'commit') trashRequest(fx);
    else if (fx.do === 'rescan') loadStorage(true);
    else if (fx.do === 'focus') focus = true;
  }
  return focus;
}
function trashDispatch(ev) {
  const focus = trashApply(ev);
  render();
  if (focus) trashFocusBack();
}
async function trashRequest(fx) {
  let status = 0, body = null;
  try { const r = await api(trashUrl(fx), 'POST'); status = r.status; body = await r.json(); }
  catch (e) { if (e && e.status) { status = e.status; try { body = JSON.parse(e.text); } catch (_) { body = null; } } else body = null; }
  trashDispatch({ type: 'reply', seq: fx.seq, status, body });
}
// A rescan or the address can change what is selected, or a folder's label, while the dialog is open. It is called from
// inside render(), so focus is moved afterwards.
function trashSync(selection) {
  if (trashApply({ type: 'selection', key: selection.key })) setTimeout(trashFocusBack, 0);
}
const trashClose = () => trashDispatch({ type: 'close' });
// One Escape closes one layer: the Trash dialog if it is open, otherwise the Stop dialog, otherwise the
// collapsed-nav drawer (the widest-reaching layer, checked last since a dialog can be open over the drawer).
function closeTopLayer() {
  if (trashOpen()) { trashClose(); return 'trash'; }
  if (sigOpen()) { sigClose(); return 'signal'; }
  if (S.modal) { rec({ modal: null }); return 'stop'; }
  if (S.navOpen) { closeNav(); return 'nav'; }
  return null;
}
// Tab stays inside the dialog: it wraps at both ends, and Shift+Tab from the heading goes to the last button.
function trashKeys(e) {
  if (e.key !== 'Tab') return;
  const box = e.currentTarget, f = [...box.querySelectorAll('button:not([disabled]), a[href]')], a = document.activeElement;
  if (!f.length) { e.preventDefault(); box.focus(); return; }
  if (e.shiftKey && (a === f[0] || a === box || a.getAttribute('tabindex') === '-1' || !box.contains(a))) { e.preventDefault(); f[f.length - 1].focus(); }
  else if (!e.shiftKey && (a === f[f.length - 1] || !box.contains(a))) { e.preventDefault(); f[0].focus(); }
}
const TRASH_STATUS_TEXT = { would_move: 'Will move', moved: 'Moved', refused: 'Refused', failed: 'Failed', not_attempted: 'Not attempted' };
function trashItemRow(i) {
  const why = i.status === 'would_move' || i.status === 'moved' ? '' : ': ' + visible(i.reason || i.code || 'no reason given');
  return h('li', { class: 'tr-item', 'data-status': i.status },
    h('div', { class: 'tr-name' }, h('span', { class: 'tr-path', title: visible(i.path || i.name) }, visible(i.name)), labelTag(i.label)),
    h('div', { class: 'small tr-meta' }, h('strong', {}, TRASH_STATUS_TEXT[i.status] || 'Unknown'), why, i.bytes == null ? '' : ', ' + formatBytes(i.bytes)));
}
const trashCounts = (c) => ['moved', 'refused', 'failed', 'not_attempted'].map((k) => c[k] + ' ' + k.replace('_', ' ')).join(', ');
const trashNames = (list) => list.map((y) => '“' + visible(y.name) + '”').join(', ');
function trashBody(t) {
  const sum = t.summary, p = (text, cls) => h('p', { class: cls || '', style: 'margin:0;line-height:1.5' }, text);
  const known = () => Object.values(t.known).map((i) => trashItemRow({ id: i.id, status: 'would_move', name: i.name, path: i.path, label: i.label, bytes: i.bytes, reason: null }));
  if (t.phase === 'previewing') return [p('Checking ' + plural(t.ids.length, 'folder', 'folders') + ' with the server. Nothing moves yet.', 'mut'), h('ul', { class: 'tr-list' }, known())];
  if (t.phase === 'moving') return [p('Moving ' + plural(t.ids.length, 'folder', 'folders') + ' to the Trash. This can take up to 45 seconds and cannot be cancelled once it starts.', 'mut'), h('ul', { class: 'tr-list' }, known())];
  if (t.phase === 'error') return [p(t.message.text), t.message.retry ? p('Preview again to start over. Nothing carries across.', 'small mut') : null];
  if (t.phase === 'confirming-yours') {
    return [p('This selection includes ' + plural(t.yours.length, 'folder', 'folders') + ' labelled YOURS. ' + (t.yours.length === 1 ? 'It holds' : 'They hold') + ' your own files, not a cache:'),
      h('ul', { class: 'tr-list' }, t.yours.map((y) => h('li', { class: 'tr-item' }, h('div', { class: 'tr-name' }, h('strong', { class: 'tr-path', title: visible(y.path) }, visible(homeRelative(y.path))), labelTag(y.label)),
        h('div', { class: 'small tr-meta' }, y.bytes == null ? 'size unknown' : formatBytes(y.bytes))))),
      p('Nothing moves yet. You will see one more preview before the final Move.'),
      p('Anything that uses ' + (t.yours.length === 1 ? 'it' : 'them') + ' may stop working until you move ' + (t.yours.length === 1 ? 'it' : 'them') + ' back from the Trash by hand.', 'small'),
      p(TRASH_PUT_BACK, 'small mut')];
  }
  const done = t.phase === 'done', moved = done ? sum.items.filter((i) => i.status === 'moved') : [], rest = sum.items.filter((i) => i.status !== 'moved');
  const lines = [p(trashHeadline(sum, !done), 'tr-head')];
  if (sum.warning) lines.push(p(sum.warning, 'tr-warn'));
  if (done) lines.push(p(trashCounts(sum.counts), 'small mut'));
  if (done && sum.counts.failed + sum.counts.not_attempted > 0) lines.push(p('Check your Trash for folders that failed or were not attempted.', 'small'));
  if (sum.auditError) lines.push(p('History could not be written: ' + sum.auditError, 'small'));
  if (!done && sum.counts.refused) lines.push(p(plural(sum.counts.refused, 'folder is', 'folders are') + ' refused and stays where it is. The rest can still move.', 'small'));
  if (!done && t.yours.length && !t.confirmYours) lines.push(p('Your own folder ' + trashNames(t.yours) + ' needs a separate confirmation on the next step.', 'small'));
  lines.push(h('ul', { class: 'tr-list', 'aria-label': done ? 'Result per folder' : 'What would move' }, [...moved, ...rest].map(trashItemRow)));
  lines.push(p(sum.note, 'small mut'));
  if (done) {
    lines.push(p('Moved folders may stay listed for up to a minute. Wait a minute, then press Rescan.', 'small'));
    lines.push(p(sum.historyId == null ? 'The server did not report a History entry for this request.' : 'Recorded as request ' + sum.historyId + ' in History.', 'small'));
    lines.push(h('a', { class: 'small', href: writeHash({ view: 'history' }) }, 'Open History'));
  }
  return lines;
}
function trashButtons(t) {
  const sum = t.summary, cancel = (label) => h('button', { class: 'btn', type: 'button', onClick: trashClose }, label || 'Cancel');
  if (t.phase === 'previewing') return [cancel()];
  if (t.phase === 'moving') return [h('button', { class: 'btn', type: 'button', 'aria-disabled': 'true', onClick: (e) => e.preventDefault() }, 'Moving...')];
  if (t.phase === 'error') return [cancel('Close'), t.message.retry ? h('button', { class: 'btn btn-acc', type: 'button', onClick: () => trashDispatch({ type: 'again' }) }, 'Preview again') : null];
  if (t.phase === 'done') return [h('button', { class: 'btn btn-acc', type: 'button', onClick: trashClose }, 'Close')];
  if (t.phase === 'confirming-yours') {
    return [h('button', { class: 'btn', type: 'button', onClick: () => trashDispatch({ type: 'yours-back' }) }, 'Go back'),
      h('button', { class: 'btn btn-danger', type: 'button', onClick: () => trashDispatch({ type: 'yours-yes' }) }, 'Include ' + trashNames(t.yours) + ' and preview again')];
  }
  const needsYours = t.yours.length > 0 && !t.confirmYours, can = needsYours || !!t.token;
  return [cancel(), h('button', { class: 'btn btn-danger', id: 'trash-move-btn', type: 'button', 'aria-disabled': can ? null : 'true', 'aria-describedby': can ? null : 'trash-desc',
    onClick: (e) => { if (!can) { e.preventDefault(); announce('Nothing can be moved. The preview lists why.'); return; } trashDispatch({ type: 'move' }); } },
    needsYours ? 'Continue' : can ? 'Move ' + plural(sum.counts.would_move, 'folder', 'folders') + ' to the Trash' : 'Nothing can be moved')];
}
function trashDialog() {
  const t = S.trash;
  if (!t || t.phase === 'idle') return null;
  const title = { previewing: 'Checking before anything moves', previewed: 'Move to the Trash?', 'confirming-yours': t.yours.length === 1 ? 'Include your own folder in the preview?' : 'Include your own folders in the preview?',
    moving: 'Moving to the Trash', done: 'Result', error: 'Could not finish' }[t.phase];
  return h('div', { class: 'overlay' }, h('div', { class: 'modal trash-modal', role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'trash-title', 'aria-describedby': 'trash-desc',
    'aria-busy': t.phase === 'previewing' || t.phase === 'moving' ? 'true' : null, tabindex: '-1', onKeydown: trashKeys },
  h('h2', { id: 'trash-title', tabindex: '-1', 'data-autofocus': '' }, title),
  h('div', { id: 'trash-desc', class: 'trash-body' }, trashBody(t)),
  h('div', { class: 'trash-buttons' }, trashButtons(t))));
}
// What render() puts in the layer. A dialog that cannot be drawn is closed with a message, because a page that stays
// frozen under an invisible dialog (background updates are held while one is open) is worse than a lost preview.
function trashLayer() {
  const shell = document.querySelector('.shell');
  if (trashOpen() && S.view !== 'storage' && S.trash.phase !== 'moving' && trashApply({ type: 'leave' })) setTimeout(trashFocusBack, 0);
  let node = null;
  try { node = trashDialog(); }
  catch (e) {
    const moving = S.trash && S.trash.phase === 'moving';
    S.trash = trashIdle();
    announce('The dialog could not be drawn and was closed. ' + (moving ? 'A move may have started: open History to check.' : 'Nothing was moved.'));
  }
  if (shell) shell.inert = !!node; // the page behind a dialog is out of reach of the keyboard and screen readers
  return node;
}
/* storage-trash-view:end */
