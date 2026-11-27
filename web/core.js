const TOKEN = document.querySelector('meta[name=ml-token]').content;
const $ = (id) => document.getElementById(id);
/* disk-net:begin */
// Pure logic for the Disk and network tab and the capacity bar: no DOM, no storage, so a Node test can load it.
const DISK_REASON_UNKNOWN = 'not read yet';
// Nothing real is bigger than this; a larger number is a bad reading, not a capacity.
const MAX_BYTES = 1e18;
const goodBytes = (n) => (typeof n === 'number' && Number.isFinite(n) && n >= 0 && n <= MAX_BYTES ? n : null);
// Whole-number percentages that add up to 100 (largest remainder), so a legend never reads 101% or 99%.
function wholePercents(shares) {
  const floors = shares.map(Math.floor);
  let left = 100 - floors.reduce((a, b) => a + b, 0);
  shares.map((x, i) => [x - floors[i], i]).sort((a, b) => b[0] - a[0] || a[1] - b[1]).forEach(([, i]) => { if (left-- > 0) floors[i]++; });
  return floors;
}
// Sizes the segments of a capacity bar. `args` is { used, purgeable, free, unavailable }: bytes for each part,
// or null when that part could not be read; `unavailable` is the reason purgeable is null. A part that is not a
// finite, non-negative number counts as unreadable. Purgeable space is counted as in use by macOS but can be
// reclaimed, so it gets its own segment and is never folded into free. With used or free missing there is
// nothing true to draw, so the result has no segments and says why. `shown` is the whole-number percentage.
function capacitySegments(args) {
  const used = goodBytes(args.used), purgeable = goodBytes(args.purgeable), free = goodBytes(args.free), unavailable = args.unavailable;
  if (used == null || free == null) return { segments: [], total: null, note: unavailable || DISK_REASON_UNKNOWN };
  const parts = [['used', used], ['purgeable', purgeable], ['free', free]].filter(([, bytes]) => bytes != null);
  const total = parts.reduce((sum, [, bytes]) => sum + bytes, 0);
  const shares = parts.map(([, bytes]) => (total > 0 ? bytes / total * 100 : 0));
  const shown = total > 0 ? wholePercents(shares) : shares;
  const segments = parts.map(([key, bytes], i) => ({ key, bytes, pct: shares[i], shown: shown[i] }));
  return { segments, total, note: purgeable == null ? 'Purgeable space could not be read (' + (unavailable || DISK_REASON_UNKNOWN) + '), so the bar shows used and free only; used includes any purgeable space.' : null };
}
// Where the capacity card is: the first read has not come back (loading), the last read failed (failed), or there is data.
const capacityStatus = ({ volume, failed }) => (failed ? 'failed' : volume ? 'ok' : 'loading');
const SIZE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];
// Decimal units, matching what macOS shows. null is "unavailable", never 0.
function formatBytes(n) {
  if (n == null || !Number.isFinite(n)) return 'unavailable';
  let v = n, i = 0;
  while (v >= 1000 && i < SIZE_UNITS.length - 1) { v /= 1000; i++; }
  return (i === 0 || v >= 100 ? Math.round(v) : v.toFixed(1)) + ' ' + SIZE_UNITS[i];
}
const formatRate = (n) => (n == null || !Number.isFinite(n) ? 'unavailable' : formatBytes(n) + '/s');
const formatOps = (n) => (n == null || !Number.isFinite(n) ? 'unavailable' : Math.round(n).toLocaleString('en-US') + ' ops/s');
// What the network card and tile say. `interfacesUp` 0 is a real "no connection" and must not be drawn as an idle
// 0 B/s; null means the interface list could not be read, which is "unavailable", a different statement.
function networkState({ interfacesUp, down, up }) {
  if (interfacesUp === 0) return { kind: 'no-connection', text: 'No connection' };
  if (down == null && up == null) return { kind: 'unavailable', text: 'unavailable' };
  return { kind: 'ok', text: null };
}
// A chart's top: the largest value in view, at least `floor`, so a quiet disk does not fill the chart with noise.
const rateChartMax = (series, floor) => Math.max(floor, ...series.flat().filter((v) => v != null));
/* disk-net:end */
/* memory-tab:begin */
// Pure logic for the Memory tab: no DOM, so a Node test can load it.
// Raw kern.memorystatus_vm_pressure_level: 1 normal, 2 warn, 4 critical. Any other value (a future
// macOS, a misread) falls back to 'unknown' rather than guessing a severity it was never given.
const PRESSURE_TAG = { 1: { text: 'Normal', cls: 'ok' }, 2: { text: 'Warning', cls: 'warn' }, 4: { text: 'Critical', cls: 'bad' } };
const pressureTag = (level) => PRESSURE_TAG[level] || { text: 'Unknown', cls: '' };
// No swap in use is a true, common state on a Mac with plenty of RAM, so a total of 0 reads as that,
// never as a 0/0 divide drawn as an empty bar.
function swapState(used, total) {
  if (used == null || total == null) return { kind: 'unavailable' };
  if (total === 0) return { kind: 'none' };
  return { kind: 'ok', pct: Math.min(100, used / total * 100) };
}
/* memory-tab:end */
/* actions:begin */
// Pure logic for the Stop/Quit confirm dialog: no DOM, no fetch, so a Node test can load it. Every
// value here is already unwrapped from its Reading<T> shape by the caller -- this block only
// decides what to do with plain pids, start times and percentages, never how to read a Reading.
// Only pids with a confirmed, fresh start time are safe to show a dialog for: one whose row is
// missing (already exited) or whose start time came back unavailable (a rare systemic failure, see
// procusage::boottime_unix) is dropped rather than guessed at -- the dialog only ever offers to
// stop what it just confirmed is still running. `null` means none of the requested pids are
// confirmable right now, so there is no dialog to open at all.
function confirmable(pids, startByPid, cpuByPid) {
  const kept = pids.filter((pid) => startByPid.has(pid) && startByPid.get(pid) != null);
  if (!kept.length) return null;
  return { pids: kept, starts: kept.map((pid) => startByPid.get(pid)), cpuPct: kept.map((pid) => cpuByPid.get(pid) ?? null) };
}
// What the dialog says it confirmed, in the "from a fresh sample" language the design calls for.
// Several processes' percentages are summed, not listed, so the line stays one sentence regardless
// of how many processes a "Quit app" covers.
function evidenceLine(label, cpuPct) {
  const known = cpuPct.filter((v) => v != null);
  if (cpuPct.length === 1) {
    return known.length ? (label + ' is using ' + Math.round(known[0]) + '% CPU right now, from a fresh sample.') : 'Confirmed running right now, from a fresh sample.';
  }
  return known.length ? ('Together using ' + Math.round(known.reduce((a, b) => a + b, 0)) + '% CPU right now, from a fresh sample.') : (cpuPct.length + ' processes confirmed running right now, from a fresh sample.');
}
// Which rows from a previous read are missing from a fresh one: just exited (by Stop or on its
// own), shown once more marked `stopped` rather than vanishing with nothing to show that anything
// happened. A row already marked stopped is never re-added, so it drops off for good one read later.
function justStoppedRows(prevRows, freshRows) {
  const seen = new Set(freshRows.map((r) => r.pid));
  return prevRows.filter((r) => !r.stopped && !seen.has(r.pid)).map((r) => Object.assign({}, r, { stopped: true }));
}
/* actions:end */
/* view-items:begin */
// The one rule for "nothing to show". h() and render() both use it: Element.append and replaceChildren write a
// null argument as the text "null", so a conditional section (`cond ? h(...) : null`) must be dropped first.
const nodesOf = (items) => items.flat(Infinity).filter((c) => c != null && c !== false);
/* view-items:end */
let h = (tag, attrs = {}, ...kids) => {
  const e = tag === 'svg' ? document.createElementNS('http://www.w3.org/2000/svg', 'svg') : document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v == null || v === false) continue;
    // Plain assignment, not setAttribute: SVGElement.className is a read-only SVGAnimatedString, so
    // e.className = v silently does nothing there (first hit by this page's own sparkline svgs).
    if (k === 'class') e.setAttribute('class', v); else if (k.startsWith('on')) e.addEventListener(k.slice(2).toLowerCase(), v); else e.setAttribute(k, v === true ? '' : v);
  }
  e.append(...nodesOf(kids).map((c) => (c instanceof Node ? c : document.createTextNode(String(c)))));
  return e;
};
const S = { view: 'overview', sel: null, filter: 'all', lfilter: 'all', pfilter: 'all', nfilter: 'all', q: '', page: {}, sorts: {}, fil: {}, size: 25, menu: null, expand: { procs: false, conns: false }, drawerW: +localStorage.getItem('drawer-width') || null, theme: localStorage.getItem('monitor-lizard-theme') || 'system', modal: null, toast: null, snap: null, priv: [], sudo: [], pers: [], seen: [], acts: [], baseChanges: [], noticeHidden: false, settings: { notifications: 'off' }, muted: new Set(JSON.parse(localStorage.getItem('monitor-lizard-muted') || '[]')), metrics: [], metricsSince: 0, perfWin: '5m', perfLive: true, perfTab: 'overview', stData: null, stFailed: false, stHist: null, stHistFailed: false, stTile: 'all', stItem: null, stfilter: 'all', stNote: null, stMulti: [], stAll: false, stAsk: false, volume: null, volumeFailed: false, procRows: null, procFailed: false, procSort: 'cpu', guide: { state: 'idle', data: null }, gtab: 'commands', gtopic: 'all', gsel: null, gcopy: null, gall: null, gsig: null, gnote: null, xopen: {}, reqOpen: {}, privOk: false, persOk: false, histOk: false };
const base = (p) => String(p).split('/').pop();
// A group key's own category prefix (apple:/dev:/other:), stripped wherever the raw key would
// otherwise leak into view: the app bundle path itself never has one, so this is always safe.
const unprefixed = (key) => key.replace(/^(apple|dev|other):/, '');
const net = (g) => g.conns.some((c) => c.scope !== 'loopback');
const apple = (g) => !!(g.signing && g.signing.apple);
const status = (g) => (g.approved ? 'approved' : apple(g) ? 'macOS' : net(g) ? 'review' : 'idle');
const tagOf = (s) => ({ review: 'warn', approved: 'ok' }[s] || '');
// The trust label comes from rules in the server (trusted, unknown, suspicious); the page only colors it.
const trustCls = (t) => ({ trusted: 'ok', unknown: 'warn', suspicious: 'bad' }[t] || '');
const RULE_TEXT = { 'apple-signed': 'Signed by Apple', approved: 'You approved this app', unreviewed: 'Uses the network and is not approved yet', unverified: 'Not approved and not verified as an Apple program', 'unsigned-network': 'Unsigned program using the network', 'exposed-listener': 'Listening on all interfaces', 'exposed-listener-apple': 'Apple service reachable from your network', 'new-remote': 'Contacted hosts first seen in the last hour', 'debug-port': 'Remote-debugging port is open' };
/* url-state:begin */
/* The address bar is the page's saved state: #<page>?<key>=<value>&...  Keys are a public format once a
   link is shared, so the list stays short and every key keeps its meaning:
     filter   the page's chip filter (Apps, Listeners, Persistence, Network)
     sel      the selected app (Apps only)
     q        the search text (pages the search box filters)
     sort     comma-separated key.column.direction, e.g. apps.procs.desc
     label, signing   the Apps dropdown filters
     proto    the Listeners protocol filter
     result   the Privilege result filter (all, failed, ok)
     topic, tab, entry   the Guide's topic filter, Commands or Glossary, and selected entry (see PAGE_PARAMS)
     tab   on Performance, the open tab: overview, cpugpu, disknet, memory or topconsumers
     filter, tile, item, q   on Storage, the label chip (safe, review, yours), the selected treemap tile, the selected folder's id and the folder search
   Read-only link shortcuts for pre-filtered views: host=<addr> (Network), port=<n> and exposed=1 (Listeners),
   failed=1 (Privilege). Every key and value is checked against the lists below: an address is typed or pasted
   by anyone, so nothing in it is trusted as a property name, a column or a filter value.
   Never put the per-run write token here: a hash is copied, bookmarked and shared. */
// The three labels the scanner gives a folder. Every use on the Storage page reads this one object.
const ST_LABEL = Object.freeze({ safe: 'safe', review: 'review', yours: 'yours' });
const TITLES = { overview: 'Overview', apps: 'Apps', priv: 'Privilege requests', listeners: 'Listeners', persist: 'Persistence', network: 'Network', history: 'History', performance: 'Performance', guide: 'Guide', storage: 'Storage' };
// Views whose rows the search box filters in place; typing anywhere else jumps to Apps.
const FILTERABLE = ['apps', 'listeners', 'persist', 'network', 'history', 'guide', 'storage'];
const PAGE_FILTER = { apps: ['filter', ['all', 'review', 'approved', 'macOS']], listeners: ['lfilter', ['all', 'exposed', 'local']], persist: ['pfilter', ['all', 'new', 'user', 'system']], network: ['nfilter', ['all', 'public', 'lan', 'new']], storage: ['stfilter', ['all', ...Object.values(ST_LABEL)]] };
// Dropdown filters per page: S.fil name -> [hash key, allowed values]. Must match the dropdowns the views draw.
const PAGE_DROPDOWNS = {
  apps: { 'apps-label': ['label', ['all', 'trusted', 'unknown', 'suspicious']], 'apps-sig': ['signing', ['all', 'apple', 'signed', 'unsigned', 'unchecked']] },
  listeners: { 'listen-proto': ['proto', ['all', 'TCP', 'UDP']] },
  priv: { 'auth-result': ['result', ['all', 'failed', 'ok']] },
};
// Plain-text keys per page: hash key -> [S field, default, check, what sheds it when the address is too long].
// Guide topics and entry ids come from the server's table, so the check is a shape, not a list; an id that no
// longer exists falls back to the first result when the page draws.
const isId = (v) => typeof v === 'string' && /^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/.test(v);
// A folder id is the scanner's hash (or the page's fallback); a location not reached yet is 'nr-' plus its path, which is
// length-limited and free of control characters.
const isItemId = (v) => isId(v) || (typeof v === 'string' && v.startsWith('nr-') && v.length > 3 && v.length <= 300 && !/[\u0000-\u001f\u007f]/.test(v));
const PERF_TABS = ['overview', 'cpugpu', 'disknet', 'memory', 'topconsumers'];
// The Storage page's treemap tiles: every section the scanner names, the merged small ones and the unmeasured rest.
const ST_TILES = ['all', 'developer', 'caches', 'logs', 'downloads', 'mail', 'applications', 'other', 'rest'];
const PAGE_PARAMS = {
  performance: { tab: ['perfTab', 'overview', (v) => PERF_TABS.includes(v), 'filters'] },
  storage: { tile: ['stTile', 'all', (v) => ST_TILES.includes(v), 'sel'], item: ['stItem', null, isItemId, 'sel'] },
  guide: { tab: ['gtab', 'commands', (v) => v === 'glossary', 'filters'], topic: ['gtopic', 'all', isId, 'filters'], entry: ['gsel', null, isId, 'sel'] },
};
// Sortable columns per page and table key. Must match the sortHeader calls; a test compares them.
const PAGE_SORTS = {
  apps: { apps: ['app', 'procs', 'out', 'listening', 'signing', 'status', 'label'] },
  listeners: { listen: ['port', 'proto', 'bind', 'app', 'pid', 'reach'] },
  persist: { pers: ['label', 'kind', 'scope', 'program', 'trust', 'added'] },
  network: { net: ['app', 'remote', 'scope', 'sockets', 'seen'] },
  priv: { sudo: ['time', 'command'], auth: ['time', 'result', 'right', 'requester'] },
  history: { hist: ['when', 'app', 'host', 'hits'], act: ['when', 'signal', 'sent', 'refused'], trash: ['when', 'request', 'folder', 'state', 'size'] },
  storage: { store: ['folder', 'section', 'label', 'size', 'share'] },
};
const HASH_MAX = 2048; // browsers and chat apps start truncating long addresses around here
const hasOwn = (o, k) => Object.prototype.hasOwnProperty.call(o, k);
const own = (o, k) => (hasOwn(o, k) ? o[k] : undefined); // never reaches Object.prototype ('constructor', '__proto__', ...)
const dec = (s) => { try { return decodeURIComponent(s); } catch (e) { return null; } };
const enc = (s) => { try { return encodeURIComponent(s); } catch (e) { return null; } }; // throws on a lone surrogate

function parseHash(hash) {
  const raw = String(hash || '').replace(/^#/, ''), cut = raw.indexOf('?');
  const page = dec(cut < 0 ? raw : raw.slice(0, cut));
  const out = { view: 'overview', filter: 'all', sel: null, q: '', sorts: {}, dd: {}, ex: {} };
  if (page == null || !hasOwn(TITLES, page)) return out;
  out.view = page;
  const p = Object.create(null);
  if (cut >= 0) for (const kv of raw.slice(cut + 1).split('&')) {
    const eq = kv.indexOf('='), k = dec(eq < 0 ? kv : kv.slice(0, eq)), v = eq < 0 ? '' : dec(kv.slice(eq + 1));
    if (k != null && v != null && !(k in p)) p[k] = v;
  }
  const f = own(PAGE_FILTER, page);
  if (f && f[1].includes(p.filter)) out.filter = p.filter;
  if (page === 'listeners' && p.exposed === '1') out.filter = 'exposed';
  if (page === 'apps' && p.sel) out.sel = p.sel;
  if (FILTERABLE.includes(page)) {
    if (p.q) out.q = p.q;
    else if (page === 'network' && p.host) out.q = p.host;
    else if (page === 'listeners' && /^\d{1,5}$/.test(p.port || '')) out.q = p.port;
  }
  for (const [name, [key, values]] of Object.entries(own(PAGE_DROPDOWNS, page) || {})) if (values.includes(p[key])) out.dd[name] = p[key];
  if (page === 'priv' && p.failed === '1' && !out.dd['auth-result']) out.dd['auth-result'] = 'failed';
  for (const [key, [field, dflt, ok]] of Object.entries(own(PAGE_PARAMS, page) || {})) out.ex[field] = ok(p[key]) ? p[key] : dflt;
  const cols = own(PAGE_SORTS, page) || {};
  for (const part of (p.sort || '').split(',')) {
    const [key, col, dir, extra] = part.split('.');
    if (extra === undefined && hasOwn(cols, key) && cols[key].includes(col) && (dir === 'asc' || dir === 'desc')) out.sorts[key] = { col, dir };
  }
  return out;
}

// Pure: the same state always gives the same string. Over HASH_MAX it sheds, in order: the selection, then the
// filters (chip, dropdowns, search), then the sort. A value that cannot be encoded is skipped on its own.
function writeHash(st) {
  const page = hasOwn(TITLES, st.view) ? st.view : 'overview', f = own(PAGE_FILTER, page);
  const build = (withSel, withFilters, withSort) => {
    const kv = [], add = (k, v) => { const e = enc(v); if (e != null) kv.push(k + '=' + e); };
    if (withSel && page === 'apps' && st.sel) add('sel', st.sel);
    const param = (shed) => { for (const [key, [field, dflt, ok, when]] of Object.entries(own(PAGE_PARAMS, page) || {})) { const v = st.ex && own(st.ex, field); if (when === shed && v !== dflt && ok(v)) add(key, v); } };
    if (withSel) param('sel');
    if (withFilters) {
      if (f && f[1].includes(st.filter) && st.filter !== 'all') add('filter', st.filter);
      for (const [name, [key, values]] of Object.entries(own(PAGE_DROPDOWNS, page) || {})) { const v = st.dd && own(st.dd, name); if (v !== 'all' && values.includes(v)) add(key, v); }
      if (FILTERABLE.includes(page) && st.q) add('q', st.q);
      param('filters');
    }
    if (withSort) {
      const cols = own(PAGE_SORTS, page) || {};
      const sorts = Object.keys(cols).filter((k) => { const s = st.sorts && own(st.sorts, k); return s && cols[k].includes(s.col) && (s.dir === 'asc' || s.dir === 'desc'); }).map((k) => k + '.' + st.sorts[k].col + '.' + st.sorts[k].dir);
      if (sorts.length) add('sort', sorts.join(','));
    }
    return '#' + page + (kv.length ? '?' + kv.join('&') : '');
  };
  for (const [a, b, c] of [[true, true, true], [false, true, true], [false, false, true]]) { const h = build(a, b, c); if (h.length <= HASH_MAX) return h; }
  return '#' + page;
}

// Page and selection changes are places to come back to (push); filter, sort and search edits refine the
// current place, so Back must skip them (replace).
function historyMode(prev, next) {
  if (prev === next) return 'none';
  const a = parseHash(prev), b = parseHash(next);
  return a.view !== b.view || a.sel !== b.sel || a.ex.gsel !== b.ex.gsel || a.ex.perfTab !== b.ex.perfTab || a.ex.stTile !== b.ex.stTile || a.ex.stItem !== b.ex.stItem ? 'push' : 'replace';
}

const toHashState = (s) => {
  const f = own(PAGE_FILTER, s.view), dd = {}, ex = {};
  for (const name of Object.keys(own(PAGE_DROPDOWNS, s.view) || {})) if (s.fil && hasOwn(s.fil, name)) dd[name] = s.fil[name];
  for (const [field] of Object.values(own(PAGE_PARAMS, s.view) || {})) ex[field] = s[field];
  return { view: s.view, filter: f ? s[f[0]] : 'all', sel: s.sel, q: s.q, sorts: s.sorts || {}, dd, ex };
};

// The S fields a parsed hash sets. Other pages' sorts and filters stay as they were.
function patchFromHash(st, s) {
  const sorts = Object.assign({}, s.sorts);
  for (const k of Object.keys(own(PAGE_SORTS, st.view) || {})) delete sorts[k];
  const patch = { view: st.view, sel: st.sel, q: st.q, sorts: Object.assign(sorts, st.sorts), page: {}, selFromUrl: !!st.sel };
  if (own(PAGE_FILTER, st.view)) patch[PAGE_FILTER[st.view][0]] = st.filter;
  const dds = Object.keys(own(PAGE_DROPDOWNS, st.view) || {});
  if (dds.length) { patch.fil = Object.assign({}, s.fil); for (const name of dds) patch.fil[name] = st.dd[name] || 'all'; }
  for (const [field] of Object.values(own(PAGE_PARAMS, st.view) || {})) patch[field] = st.ex[field];
  if (own(PAGE_PARAMS, st.view)) patch.gcopy = null; // a Copy result belongs to the entry it was made on
  if (st.sel !== s.sel) patch.expand = { procs: false, conns: false };
  return patch;
}

// What a loaded snapshot means for the selection. A selection that came from the address and is gone gets a
// notice; one whose app quit while the page was open does not. Clearing it must not add a history entry.
function selPatch(s, snap) {
  if (!snap || s.view !== 'apps' || !s.sel) return null;
  if (snap.groups.some((g) => g.key === s.sel)) return s.selFromUrl ? { selFromUrl: false } : null;
  return Object.assign({ sel: null, selFromUrl: false, hashReplace: true }, s.selFromUrl ? { toast: 'That app is no longer running, so nothing is selected.' } : {});
}

function syncHash(s, loc, hist, forceReplace) {
  const next = writeHash(toHashState(s));
  if (next === loc.hash) return;
  const mode = forceReplace ? 'replace' : historyMode(loc.hash, next);
  try { hist[mode === 'push' ? 'pushState' : 'replaceState'](null, '', next); } catch (e) { /* the address bar is a convenience, never worth breaking the page */ }
}

function crumbParts(st, selLabel) {
  const parts = [{ text: 'Monitor Lizard', href: '#overview' }];
  if (st.view !== 'overview') parts.push({ text: TITLES[st.view], href: writeHash({ view: st.view }) });
  else parts.push({ text: TITLES.overview, href: '#overview' });
  if (st.view === 'apps' && st.sel) parts.push({ text: selLabel || st.sel, href: writeHash(st) });
  return parts;
}
/* url-state:end */
/* guide-search:begin */
// The Guide's search, kept free of DOM access so tests (and later a global search) can lift it out of the page.
// The Rust `search` behind `help` follows the same rules; tests/fixtures/guide-search.json holds results both give.
// Words: lower case; a word with no letter or digit ("/", "-") is dropped; a trailing "s" is cut from words longer
// than three letters, so "snapshots" also finds "snapshot". Every word must match.
const guideWords = (q) => String(q || '').toLowerCase().split(/\s+/).filter((w) => /[\p{L}\p{N}]/u.test(w)).map((w) => (w.length > 3 && w.endsWith('s') ? w.slice(0, -1) : w));
const isAlnum = (c) => c !== '' && /[\p{L}\p{N}]/u.test(c);
// A word under three characters must stand alone ("df" in "docker system df"), or it would match inside nearly every text.
function hasWord(hay, w) {
  for (let i = hay.indexOf(w); i >= 0; i = hay.indexOf(w, i + 1)) if (!isAlnum(hay.charAt(i - 1)) && !isAlnum(hay.charAt(i + w.length))) return true;
  return false;
}
// 0 when any word is missing. Per word: title 8, tags 4, command 4, description 1 (short words are not looked for in the description).
function guideScore(e, words) {
  const f = { title: e.title || e.term || '', cmd: e.command || '', tags: (e.tags || []).join(' '), desc: e.description || e.meaning || '' };
  for (const k in f) f[k] = f[k].toLowerCase();
  let score = 0;
  for (const w of words) {
    const short = w.length < 3, hit = (hay) => (short ? hasWord(hay, w) : hay.includes(w));
    const n = (hit(f.title) ? 8 : 0) + (hit(f.tags) ? 4 : 0) + (hit(f.cmd) ? 4 : 0) + (!short && hit(f.desc) ? 1 : 0);
    if (!n) return 0;
    score += n;
  }
  return score;
}
// Every match, best first and in table order among equals, each as { e, n }. Without words: everything, n = 0.
function guideRanked(items, q) {
  const words = guideWords(q);
  if (!words.length) return items.map((e) => ({ e, n: 0 }));
  return items.map((e, i) => ({ e, i, n: guideScore(e, words) })).filter((x) => x.n > 0).sort((a, b) => b.n - a.n || a.i - b.i);
}
// Weak matches score under this share of the best score. Tuned on the real table: "snapshots" scores 17, 17, 12, 8, so
// anything above 12/17 (0.71) drops the app's own snapshot entry and 0.75 does.
const GUIDE_CUTOFF = 0.75;
// What to list: matches in the topic, split at the cutoff measured against the best match over all topics so a topic
// chip does not change what counts as weak. `all` shows the weak ones too.
function guideSearch(items, q, topic, all) {
  const ranked = guideRanked(items, q), bar = ranked.length ? ranked[0].n * GUIDE_CUTOFF : 0;
  const inTopic = ranked.filter((x) => !topic || topic === 'all' || x.e.topic === topic);
  const strong = inTopic.filter((x) => x.n >= bar);
  const weak = inTopic.filter((x) => x.n < bar);
  return { shown: (all ? inTopic : strong).map((x) => x.e), hidden: all ? [] : weak.map((x) => x.e) };
}
const guideTopics = (commands) => [...new Set(commands.map((c) => c.topic))];
function guideGroups(rows) {
  const by = new Map();
  for (const r of rows) { if (!by.has(r.topic)) by.set(r.topic, []); by.get(r.topic).push(r); }
  return [...by];
}
// The product's own topic is named after its binary, so its label comes from the topic text, not from a list here.
const topicLabel = (t) => ({ storage: 'Storage', developer: 'Developer tools' })[t] || String(t).split('-').map((w) => w.charAt(0).toUpperCase() + w.slice(1)).join(' ');
const guidePick = (rows, id) => rows.find((r) => r.id === id) || rows[0] || null;
// What the Guide's content is built from. A poll changes none of it, so the content is not rebuilt (and keeps focus
// and the selected text) until one of these changes.
const guideSig = (s) => JSON.stringify([s.q, s.gtab, s.gtopic, s.gsel, s.gall, s.gcopy, s.guide && s.guide.state]);
// "5 commands", or the empty message, for the live region.
const guideSummary = (n, hidden, kind) => (n ? n + ' ' + kind + (n === 1 ? '' : 's') + (hidden ? ', ' + hidden + ' weaker hidden' : '') : 'Nothing matches that');
/* guide-search:end */

/* guide-copy:begin */
// The Copy flow with its page access passed in, so a test can run it without a DOM. By the time the clipboard
// answers the user may have left the page, so every element is looked up again and may be gone.
async function copyFlow(command, { write, show, byId, select }) {
  let state = 'copied';
  try { await write(command); } catch (err) { state = 'manual'; }
  show(state);
  const button = byId('guide-copy'), code = byId('guide-cmd');
  if (button) button.focus();
  if (state === 'manual' && code) select(code);
  return state;
}
/* guide-copy:end */

// Spike: Preact mode. window.preact is set by the import() in shell.js; until then (or when the opt-in is off) h() builds Nodes.
const UI_PREACT = (() => { try { return localStorage.getItem('monitor-lizard-ui') === 'preact'; } catch (e) { return false; } })();
const vnodeMode = () => UI_PREACT && typeof window.preact !== 'undefined';
function build(fn) {
  if (!vnodeMode()) return fn();
  const old = h; h = (tag, attrs, ...kids) => window.preact.h(tag, attrs || null, ...kids);
  try { return fn(); } finally { h = old; }
}
const rec = (m) => { if ('sel' in m && m.sel !== S.sel) m = Object.assign({ expand: { procs: false, conns: false }, selFromUrl: false }, m); Object.assign(S, m); render(); };
// Background polls must not rebuild the DOM under an open menu or dialog.
const quiet = (m) => { Object.assign(S, m); if (!S.menu && !S.modal && (vnodeMode() || !trashOpen())) render(); };
const media = matchMedia('(prefers-color-scheme: light)');
const applyTheme = () => { document.documentElement.dataset.theme = S.theme === 'system' ? (media.matches ? 'light' : 'dark') : S.theme; };
const setTheme = (t) => { localStorage.setItem('monitor-lizard-theme', t); S.theme = t; applyTheme(); render(); };
media.addEventListener('change', applyTheme);

/* Custom select: button + listbox. Arrow keys move, Enter/Space picks, Escape closes and returns focus. */
function menuKeys(e) {
  const items = [...e.currentTarget.querySelectorAll('.sel-opt')], i = items.indexOf(document.activeElement);
  if (e.key === 'ArrowDown') { e.preventDefault(); items[Math.min(i + 1, items.length - 1)].focus(); }
  else if (e.key === 'ArrowUp') { e.preventDefault(); items[Math.max(i - 1, 0)].focus(); }
  else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); const k = S.menu; rec({ menu: null }); const t = document.querySelector('[data-trigger="' + k + '"]'); if (t) t.focus(); }
  else if (e.key === 'Tab') { S.menu = null; render(); }
}
function dropdown(key, value, options, onPick, label, down) {
  const open = S.menu === key, cur = options.find((o) => o[0] === value) || options[0];
  return h('div', { class: 'sel' },
    h('button', { class: 'sel-btn', type: 'button', 'aria-haspopup': 'listbox', 'aria-expanded': String(open), 'aria-label': label + ': ' + cur[1], 'data-trigger': key, onClick: (e) => { e.stopPropagation(); rec({ menu: open ? null : key }); } }, h('span', {}, cur[1]), h('span', { class: 'caret', 'aria-hidden': 'true' }, '\u25BE')),
    open ? h('div', { class: 'sel-menu' + (down ? ' down' : ''), role: 'listbox', 'aria-label': label, onKeydown: menuKeys },
      options.map(([v, text]) => h('button', { class: 'sel-opt' + (v === value ? ' on' : ''), type: 'button', role: 'option', 'aria-selected': String(v === value), 'data-autofocus': v === value ? '' : null, onClick: (e) => { e.stopPropagation(); S.menu = null; onPick(v); } }, text))) : null);
}
function paginate(list, key) {
  const pages = Math.max(1, Math.ceil(list.length / S.size));
  const p = Math.min(Math.max(1, S.page[key] || 1), pages);
  const start = (p - 1) * S.size;
  const slice = list.slice(start, start + S.size);
  TABLES[key] = { rows: list, page: slice };
  const go = (n) => rec({ page: Object.assign({}, S.page, { [key]: n }) });
  const pager = list.length <= 10 ? null : h('div', { class: 'pager' },
    h('span', {}, (start + 1) + '-' + (start + slice.length) + ' of ' + list.length), h('span', { class: 'grow' }),
    h('span', { class: 'theme-row' }, 'Rows', dropdown('rows', S.size, [10, 25, 50, 100].map((n) => [n, String(n)]), (n) => rec({ size: n, page: {} }), 'Rows per page')),
    h('button', { class: 'btn sm', disabled: p <= 1, onClick: () => go(p - 1) }, 'Prev'), h('span', {}, 'Page ' + p + ' / ' + pages),
    h('button', { class: 'btn sm', disabled: p >= pages, onClick: () => go(p + 1) }, 'Next'));
  return { slice, pager };
}

