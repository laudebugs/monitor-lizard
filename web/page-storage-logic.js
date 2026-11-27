/* storage-page:begin */
// Pure logic for the Storage page: which state the scan is in, the "at least" wording, the treemap tiles and
// their columns, and the table rows. No DOM or storage access, so a test loads it with disk-net and page-shell.
const ST_CATEGORIES = { developer: 'Developer', caches: 'Caches', logs: 'Logs', downloads: 'Downloads', mail: 'Mail', applications: 'Applications' };
const ST_LABELS = Object.values(ST_LABEL);
// The scanner's rules that say safe (src/storage/rules.rs); a test reads that file and fails if one is missing here.
const ST_SAFE_RULES = ['xcode-derived-data', 'homebrew-cache', 'package-manager-cache', 'idle-node-modules'];
// Folders that hold the person's own files. The scanner never labels these safe; the page does not trust that alone.
const ST_USER_PARTS = ['documents', 'desktop', 'pictures', 'movies', 'music', 'downloads', 'mail'];
const ST_STALE_SEC = 900;
// The server starts a scan at most this often (src/storage/cache.rs MIN_INTERVAL); it also says when the last one began.
const ST_SCAN_INTERVAL = 60;
// A section under this share of the disk joins "Other"; every tile gets at least this share of the treemap so a
// zero or tiny section stays a readable tile instead of vanishing.
const ST_OTHER_PCT = 1, ST_MIN_SHARE = 6;
const stNum = (v) => (v && typeof v === 'object' && 'value' in v ? goodBytes(v.value) : null);
const stReason = (v) => (v && typeof v === 'object' && typeof v.unavailable === 'string' ? v.unavailable : v === 'restricted' ? 'restricted' : DISK_REASON_UNKNOWN);

// loading: nothing asked yet. failed: the request failed and nothing is held. scanning: the first scan is running, no
// result yet. refused: the scanner could not start. partial: a result that is not complete (or does not say it is).
// complete: only when the server says so, exactly.
function storageState({ data, failed }) {
  if (!data) return failed ? 'failed' : 'loading';
  if (data.refused) return 'refused';
  if (typeof data.scanned_at !== 'number') return data.scanning ? 'scanning' : 'empty';
  return data.complete === true ? 'complete' : 'partial';
}
const atLeast = (bytes, lowerBound) => (goodBytes(bytes) == null ? 'unavailable' : (lowerBound ? 'at least ' : '') + formatBytes(bytes));
const staleScan = (scannedAt, now) => typeof scannedAt === 'number' && now - scannedAt > ST_STALE_SEC;
const measuredText = (scannedAt, now) => (typeof scannedAt === 'number' ? 'measured ' + relTime(scannedAt, now) : 'not measured yet');
const coverageText = (d) => {
  const c = d && d.coverage, missed = d && Array.isArray(d.not_reached) ? d.not_reached.length : 0;
  return c ? c.locations_measured + ' of ' + c.locations_total + ' locations measured' + (missed ? ', ' + missed + ' not yet scanned' : '') : 'coverage unknown';
};
// Facts for the page title (see verdictTitle); null until a scan result is held. `ago` is spoken ("5 minutes ago").
const storageFacts = (d, now) => (d && typeof d.scanned_at === 'number' && d.coverage
  ? { measured: d.coverage.locations_measured, total: d.coverage.locations_total, complete: d.complete === true, stale: staleScan(d.scanned_at, now), ago: relTime(d.scanned_at, now, true) } : null);
// Seconds since the running scan began (the server says when the next one may start, one interval after the start).
const scanElapsed = (d, now) => (d && typeof d.next_scan_allowed_at === 'number' ? Math.max(0, Math.round(now - (d.next_scan_allowed_at - ST_SCAN_INTERVAL))) : null);
const scanningText = (d, now) => { const s = scanElapsed(d, now); return s == null ? 'Scanning' : 'Scanning, ' + s + ' s so far'; };
// What Rescan may do now: refuse while a scan runs or inside the server's minute, and say how long to wait.
function rescanInfo(d, now) {
  if (d && d.scanning) return { ok: false, wait: 0, text: 'A scan is running.' };
  const wait = d && typeof d.next_scan_allowed_at === 'number' ? Math.ceil(d.next_scan_allowed_at - now) : 0;
  return wait > 0 ? { ok: false, wait, text: 'A scan started less than a minute ago. The next one can start in ' + wait + ' s.' } : { ok: true, wait: 0, text: '' };
}
// Refresh failures: wait 2 s, 4 s, 8 s ... up to 30 s between tries. A reply that arrives after a newer request began is dropped.
const retryDelay = (failures) => Math.min(30000, 2000 * 2 ** Math.max(0, Math.min(failures, 10) - 1));
const staleReply = (mine, latest) => mine !== latest;
const refreshFailure = (d, now) => (d && typeof d.scanned_at === 'number'
  ? 'Could not refresh. Showing the scan from ' + relTime(d.scanned_at, now, true) + '.' : 'Could not read the storage scan. Trying again.');
// What a screen reader hears when the scan starts, ends or the read fails. Polls that change nothing say nothing.
function storageAnnouncement(before, after, now) {
  if (after.failed && !before.failed) return refreshFailure(after.data, now);
  const was = !!(before.data && before.data.scanning), is = !!(after.data && after.data.scanning);
  if (!was && is) return 'Scan started.';
  if (was && !is && after.data && !after.failed) return 'Scan finished: ' + coverageText(after.data) + (after.data.complete === true ? '.' : '. The result is partial.');
  return null;
}
// A folder is shown SAFE only when the server says exactly safe, by a rule on the safe list, for an absolute path under
// /Users/<name>/ that is not in a user-content folder. Anything else is REVIEW, so a wrong or missing field never
// tells a person to delete their own files. YOURS is kept as sent.
function shownLabel(item) {
  const sent = item && item.label;
  if (sent === ST_LABEL.yours) return ST_LABEL.yours;
  if (sent !== ST_LABEL.safe) return ST_LABEL.review;
  const path = item && typeof item.path === 'string' ? item.path : '';
  const parts = path.toLowerCase().split('/');
  const sane = /^\/Users\/[^/]+\/./.test(path) && !parts.slice(1).some((p) => p === '' || p === '.' || p === '..');
  const userContent = item.rule === 'user-content' || item.rule === 'mail' || item.category === 'downloads' || item.category === 'mail' || parts.some((p) => ST_USER_PARTS.includes(p));
  return sane && !userContent && ST_SAFE_RULES.includes(item.rule) ? ST_LABEL.safe : ST_LABEL.review;
}
const homeRelative = (p) => String(p).replace(/^\/Users\/[^/]+(?=\/|$)/, '~');
// How a folder's path reads in a cell: home-relative, no trailing slash, split so the last two segments (`tail`) stay whole
// and only the front (`head`) is ever cut. head + tail is always text. The full path stays in the title and the export.
function pathDisplay(path) {
  const text = homeRelative(path == null ? '' : path).replace(/(.)\/+$/, '$1'), parts = text.split('/');
  if (parts.length <= 3) return { text, head: '', tail: text };
  const tail = '/' + parts.slice(-2).join('/');
  return { text, head: text.slice(0, text.length - tail.length), tail };
}
// FNV-1a over the path: the stable fallback id when the server sends none, or sends one twice.
function pathId(path) {
  let h = 0x811c9dc5;
  for (let i = 0; i < path.length; i++) { h ^= path.charCodeAt(i); h = Math.imul(h, 0x01000193) >>> 0; }
  return 'p' + h.toString(16).padStart(8, '0');
}
// One table row per measured folder, then one per location the scan did not reach. No two rows share an id.
function storageRows(d) {
  const total = d && d.capacity ? stNum(d.capacity.total) : null, at = d && d.scanned_at, seen = new Set();
  const uniqueId = (given, path) => {
    let id = isId(given) && !seen.has(given) ? given : pathId(path);
    for (let n = 2; seen.has(id); n++) id = pathId(path) + '-' + n;
    seen.add(id);
    return id;
  };
  const items = (d && Array.isArray(d.items) ? d.items : []).map((it) => {
    const path = typeof it.path === 'string' ? it.path : '', bytes = stNum(it.size), share = bytes != null && total ? bytes / total * 100 : null;
    return { kind: 'item', id: uniqueId(it.id, path), path, name: pathDisplay(path).text || '(no path)', head: pathDisplay(path).head, tail: pathDisplay(path).tail || '(no path)', category: it.category, label: shownLabel(it), rule: it.rule, bytes, reason: bytes == null ? stReason(it.size) : null,
      lowerBound: it.complete === false, apparent: goodBytes(it.apparent_bytes), entries: it.entries, unreadable: it.unreadable | 0, share, at };
  });
  const pending = (d && Array.isArray(d.not_reached) ? d.not_reached : []).map((p) => ({ kind: 'pending', id: 'nr-' + p, path: String(p), name: pathDisplay(p).text, head: pathDisplay(p).head, tail: pathDisplay(p).tail, category: null, label: null, rule: null, bytes: null, reason: 'not yet scanned', lowerBound: false, apparent: null, entries: 0, unreadable: 0, share: null, at: null }));
  return items.concat(pending);
}
const sizeText = (r) => (r.kind === 'pending' ? 'not yet scanned' : r.bytes == null ? 'unavailable: ' + r.reason : atLeast(r.bytes, r.lowerBound));
const shareText = (r) => (r.share == null ? '-' : r.share < 0.1 ? '<0.1%' : (r.share < 10 ? r.share.toFixed(1) : Math.round(r.share)) + '%');
// The default order: biggest first, unmeasured and unavailable last.
const bySize = (a, b) => (a.bytes == null) - (b.bytes == null) || (b.bytes || 0) - (a.bytes || 0);
// The folder search: every word must appear in the name, the path or the label.
function searchRows(rows, q) {
  const words = String(q || '').toLowerCase().split(/\s+/).filter(Boolean);
  return words.length ? rows.filter((r) => words.every((w) => (r.path + ' ' + r.name + ' ' + (r.label || '')).toLowerCase().includes(w))) : rows;
}

// The treemap's tiles. A section under ST_OTHER_PCT of the disk joins "Other" (never all of them: at least one stays),
// an unavailable section stays its own tile with its reason, and "rest" is the used space this scan did not measure,
// so the tiles add up to the used total when capacity is readable. The rest is only an estimate while any section is
// unavailable, a lower bound or unfinished: those bytes are missing from the measured sum, so they land in the rest.
function storageTiles(d) {
  const cats = d && Array.isArray(d.categories) ? d.categories : [], items = d && Array.isArray(d.items) ? d.items : [];
  const cap = d && d.capacity ? d.capacity : {}, total = stNum(cap.total), used = stNum(cap.used);
  const made = cats.map((c) => {
    const mine = items.filter((it) => it.category === c.category), bytes = goodBytes(c.bytes);
    const allGone = mine.length > 0 && mine.every((it) => stNum(it.size) == null);
    const unreached = !mine.length && (c.unmeasured | 0) > 0 && bytes === 0;
    const known = hasOwn(ST_CATEGORIES, c.category);
    const reason = allGone ? stReason(mine[0].size) : unreached ? 'not yet scanned' : bytes == null ? 'bad reading' : null;
    return { id: String(c.category), realId: String(c.category), kind: 'category', name: known ? ST_CATEGORIES[c.category] : String(c.category), bytes: reason ? null : bytes, reason, lowerBound: c.lower_bound === true, unmeasured: c.unmeasured | 0, count: c.items | 0, known };
  });
  const measured = made.reduce((n, t) => n + (t.bytes || 0), 0), base = total || measured;
  const small = (t) => !t.known || (t.bytes != null && (base > 0 ? t.bytes / base * 100 : 0) < ST_OTHER_PCT);
  const merge = made.filter(small), keep = made.filter((t) => !small(t));
  const other = { id: 'other', kind: 'other', name: 'Other', bytes: merge.reduce((n, t) => n + (t.bytes || 0), 0), reason: null, lowerBound: merge.some((t) => t.lowerBound), unmeasured: merge.reduce((n, t) => n + t.unmeasured, 0), count: merge.reduce((n, t) => n + t.count, 0), members: merge };
  const tiles = merge.length && keep.length ? keep.concat([other]) : made.slice();
  if (used != null && used > measured) {
    const why = [];
    if (made.some((t) => t.bytes == null)) why.push('some sections are unavailable');
    if (made.some((t) => t.lowerBound)) why.push('some sections are only a lower bound');
    if (d && d.complete !== true) why.push('the scan is not complete');
    tiles.push({ id: 'rest', kind: 'rest', name: 'Not measured', bytes: used - measured, reason: null, lowerBound: false, unmeasured: 0, count: 0, approx: why.length > 0, why: why.join(' and ') });
  }
  const raw = tiles.reduce((n, t) => n + (t.bytes || 0), 0);
  // The unmeasured rest is often most of the disk; sharing the treemap with it would squeeze every real section to a sliver,
  // so it gets no share (pct 0) and the page draws it as its own full-width strip.
  const sized = tiles.filter((t) => t.kind !== 'rest'), rawSized = sized.reduce((n, t) => n + (t.bytes || 0), 0), floorSized = rawSized * ST_MIN_SHARE / 100;
  const weights = tiles.map((t) => (t.kind === 'rest' ? 0 : rawSized > 0 ? Math.max(t.bytes || 0, floorSized) : 1)), sum = weights.reduce((a, b) => a + b, 0);
  return tiles.map((t, i) => Object.assign(t, { pct: sum > 0 ? weights[i] / sum * 100 : 0, diskPct: t.bytes != null && (total || raw) ? t.bytes / (total || raw) * 100 : null }));
}
// The size a tile shows. The unmeasured rest says "about" while it is an estimate, and never reads as free space.
const tileSizeText = (t) => (t.bytes == null ? 'unavailable' : t.kind === 'rest' ? (t.approx ? 'about ' : '') + formatBytes(t.bytes) : atLeast(t.bytes, t.lowerBound));
const REST_NOT_FREE = 'This is not free space.';
// Flex columns for the treemap: the largest tiles first, each placed in the lightest column, columns largest first.
function treemapColumns(tiles) {
  const n = tiles.length, count = n <= 2 ? Math.max(1, n) : n <= 4 ? 2 : 3;
  const cols = Array.from({ length: count }, () => ({ pct: 0, tiles: [] }));
  for (const t of [...tiles].sort((a, b) => b.pct - a.pct)) { const c = cols.reduce((m, x) => (x.pct < m.pct ? x : m), cols[0]); c.tiles.push(t); c.pct += t.pct; }
  return cols.filter((c) => c.tiles.length).sort((a, b) => b.pct - a.pct);
}
// A selected id that is not a tile now falls back: a merged section selects "Other", anything else selects all.
function resolveTile(id, tiles) {
  if (id === 'all' || tiles.some((t) => t.id === id)) return id;
  return tiles.some((t) => t.kind === 'other' && t.members.some((m) => m.realId === id)) ? 'other' : 'all';
}
// The categories a tile stands for: null is every row, an empty list is no folder row (the unmeasured rest).
function tileCategories(id, tiles) {
  if (id === 'all') return null;
  const t = tiles.find((x) => x.id === id);
  return !t || t.kind === 'rest' ? [] : t.kind === 'other' ? t.members.map((m) => m.realId) : [t.realId];
}
// Rows for the table: the selected tile, then the label chip. A location not reached has no section and no label,
// so it shows only with no tile and no label chosen.
function visibleRows(rows, tiles, tileId, label) {
  const cats = tileCategories(tileId, tiles);
  return rows.filter((r) => (cats == null ? true : r.kind === 'item' && cats.includes(r.category)) && (label === 'all' || r.label === label));
}
const labelCounts = (rows) => Object.assign({ all: rows.length }, ...ST_LABELS.map((l) => ({ [l]: rows.filter((r) => r.label === l).length })));
// The "safe to remove" total exists only for a complete scan. Before that there is no number to misread.
function safeTotalText(d, rows) {
  if (storageState({ data: d }) !== 'complete') return 'Not shown until the scan is complete.';
  const safe = rows.filter((r) => r.kind === 'item' && r.label === ST_LABEL.safe);
  return atLeast(safe.reduce((n, r) => n + (r.bytes || 0), 0), safe.some((r) => r.bytes == null || r.lowerBound));
}
// Arrow keys walk the tiles in reading order; the ends stay put.
function nextTile(ids, current, key) {
  const i = ids.indexOf(current);
  if (i < 0 || !ids.length) return null;
  if (key === 'Home') return ids[0];
  if (key === 'End') return ids[ids.length - 1];
  return ids[Math.min(ids.length - 1, Math.max(0, i + (key === 'ArrowLeft' || key === 'ArrowUp' ? -1 : key === 'ArrowRight' || key === 'ArrowDown' ? 1 : 0)))];
}

// Capacity history (GET /api/storage/history): the weekly change, the forecast sentence and the chart's geometry.
// The forecast is a straight line over the last 14 days; every sentence says "about" and every range comes from the fit.
const CH_W = 600, CH_H = 140, CH_PAD = 10, CH_MAX_AHEAD = 90, CH_FAR_DAYS = 730;
const chDays = (d) => (Number.isFinite(d) && d >= 0 ? (d < 1 ? 'less than a day' : d >= CH_FAR_DAYS ? 'more than 2 years' : plural(Math.round(d), 'day', 'days')) : null);
const chCount = (d) => (d < 1 ? 'less than a day' : Math.round(d) + (Math.round(d) === 1 ? ' day' : ' days'));
// "Grew 14.2 GB this week". The server sends null until the samples span six days.
function weekText(history) {
  if (!history || history.available !== true) return null;
  if (history.stale === true) return 'Weekly change: no recent samples';
  const b = history.week_change_bytes;
  if (typeof b !== 'number' || !Number.isFinite(b)) return 'Weekly change: collecting history';
  if (b === 0) return 'No change in used space this week';
  return (b > 0 ? 'Grew ' : 'Shrank ') + formatBytes(Math.abs(b)) + ' this week';
}
// What the strip says about the disk filling. kind: loading, failed, unavailable, collecting, flat, growing.
// `headline` is the field, `detail` the range or the reason, `note` what the number rests on.
// `fmt` turns a unix time into a date for display; the page passes the local format, a test passes its own.
const chDate = (ts) => new Date(ts * 1000).toISOString().slice(0, 10);
// "2026-11-26, today" / "2026-11-20, 6 days ago": when the newest sample was taken, so a forecast never hides its age.
function lastSampleText(history, fmt) {
  if (!history || history.available !== true || typeof history.last_sample_ts !== 'number') return null;
  const n = history.days_since_last_sample, when = (fmt || chDate)(history.last_sample_ts);
  return Number.isFinite(n) ? when + (n <= 0 ? ', today' : n === 1 ? ', yesterday' : ', ' + n + ' days ago') : when;
}
function capacityOutlook(history, failed, fmt) {
  if (!history) return failed ? { kind: 'failed', headline: 'Capacity history could not be read', detail: 'The current capacity above is not affected.', note: '' } : { kind: 'loading', headline: 'Reading capacity history', detail: '', note: '' };
  if (history.available !== true) return { kind: 'unavailable', headline: 'History unavailable', detail: 'The history database could not be opened, so there is no forecast. The current capacity above is not affected.', note: '' };
  const f = history.forecast || {}, win = Number.isFinite(history.window_days) ? history.window_days : 14;
  if (f.status === 'collecting') return { kind: 'collecting', headline: 'Collecting history', detail: (f.days_have | 0) + ' of ' + (f.days_needed | 0) + ' days so far; a forecast needs ' + (f.days_needed | 0) + '.', note: '' };
  if (f.status === 'not_growing') return { kind: 'flat', headline: 'Not growing', detail: 'Used space was flat or shrinking over the last ' + win + ' days.', note: '' };
  if (f.status !== 'growing' || !Number.isFinite(f.days_until_full)) return { kind: 'unavailable', headline: 'History unavailable', detail: 'The forecast could not be read.', note: '' };
  const days = chDays(f.days_until_full), lo = f.days_low, hi = f.days_high;
  const far = f.days_until_full >= CH_FAR_DAYS;
  const same = Number.isFinite(lo) && typeof hi === 'number' && Math.round(lo) === Math.round(hi);
  const range = far || same || !Number.isFinite(lo) ? '' : typeof hi === 'number' ? 'Range ' + chCount(lo).replace(/ days?$/, '') + ' to ' + chCount(hi) : 'Range ' + chCount(lo) + ' or more';
  const note = 'A straight line over the last ' + win + ' days of used space, purgeable included. The range is about one standard error of the daily growth: roughly two chances in three that the real date falls inside it, if growth stays steady. A change in habits changes it.';
  const lead = f.days_until_full < 1 || far ? '' : 'about ';
  if (history.stale === true) {
    const when = typeof history.last_sample_ts === 'number' ? (fmt || chDate)(history.last_sample_ts) : 'an earlier day';
    return { kind: 'stale', headline: 'Forecast out of date: last sample ' + when,
      detail: 'Counted from then, the disk would have filled in ' + lead + days + (range ? ' (' + range.toLowerCase() + ')' : '') + '. It may be full sooner now. The monitor was not running; the forecast updates after new daily samples.', note };
  }
  return { kind: 'growing', headline: 'Full in ' + lead + days, detail: range, note };
}
// The chart: used space (purgeable included) as a solid line, the fitted line ahead as a dashed one, the disk size as the top rule.
// Returns null with fewer than two samples. All numbers are plain, so a test can check them.
function capacityChart(history) {
  const ok = (d) => d && Number.isFinite(d.day) && goodBytes(d.used) != null && goodBytes(d.purgeable) != null;
  const s = history && history.available === true && Array.isArray(history.samples) ? history.samples.filter(ok) : [];
  if (s.length < 2) return null;
  const occ = s.map((d) => d.used + d.purgeable), total = goodBytes(history.total);
  const f = history.forecast && history.forecast.status === 'growing' && total != null ? history.forecast : null;
  const first = s[0].day, last = s[s.length - 1].day;
  const ahead = f ? Math.min(CH_MAX_AHEAD, Math.max(3, f.days_until_full)) : 0;
  const top = Math.max(total != null ? total : 0, ...occ), floor = Math.min(...occ);
  const yMin = Math.max(0, floor - (top - floor) * 0.08), xEnd = last + ahead;
  const x = (day) => CH_PAD + (xEnd > first ? (day - first) / (xEnd - first) : 0) * (CH_W - 2 * CH_PAD);
  const y = (v) => CH_PAD + (1 - (top > yMin ? (v - yMin) / (top - yMin) : 0)) * (CH_H - 2 * CH_PAD);
  const pt = (day, v) => x(day).toFixed(1) + ' ' + y(v).toFixed(1);
  const line = 'M' + s.map((d, i) => pt(d.day, occ[i])).join(' L');
  let dash = '';
  if (f) {
    const fitted = total - f.days_until_full * f.bytes_per_day, end = Math.min(top, fitted + f.bytes_per_day * ahead);
    dash = 'M' + pt(last, fitted) + ' L' + pt(last + ahead, end);
  }
  return { w: CH_W, h: CH_H, line, dash, topY: total != null ? +y(total).toFixed(1) : null, from: s[0].ts, to: s[s.length - 1].ts, count: s.length, fromBytes: occ[0], toBytes: occ[occ.length - 1], ahead };
}
// The sentence a screen reader gets for the chart: the data and the forecast, since the picture alone says neither.
function capacityChartLabel(chart, outlook) {
  if (!chart) return '';
  return 'Used space over ' + plural(chart.count, 'daily sample', 'daily samples') + ', from ' + formatBytes(chart.fromBytes) + ' to ' + formatBytes(chart.toBytes) + '. Forecast: ' + outlook.headline + (outlook.detail ? '. ' + outlook.detail : '') + '.';
}
/* storage-page:end */
/* storage-cleanups:begin */
// Pure logic for the Storage page's suggested clean-ups, the Inside System Data breakdown and the exported report.
// No DOM or storage access, so a test loads it with url-state, disk-net, page-shell and storage-page.
const ST_RULE_TEXT = {
  'user-content': 'Your own files', mail: 'Mail data', 'xcode-derived-data': 'Xcode rebuilds this build cache',
  'homebrew-cache': 'Homebrew downloads this again',
  'package-manager-cache': 'A package manager downloads this again',
  'app-cache': 'An app cache; some apps keep offline data here', 'docker-data': 'Docker images and containers',
  simulators: 'Simulator runtimes and devices', logs: 'Log files',
  'idle-node-modules': 'node_modules of a project untouched for 90 days',
  'node-modules': 'node_modules of a project that is in use or has no lockfile', applications: 'An installed app',
  'outside-home': 'Outside your home folder', 'unclear-path': 'The path could not be checked',
};
const ST_RULE_FALLBACK = 'Not on the safe list, so it needs a look first.';
const ST_CLEANUP_TOP = 50;
const stRuleText = (rule) => (hasOwn(ST_RULE_TEXT, rule) ? ST_RULE_TEXT[rule] : ST_RULE_FALLBACK);
// Every measured folder with a label, biggest first; a size that could not be read goes last and keeps its reason.
// A location the scan did not reach has no label and is not listed.
function stCleanups(rows) {
  return rows.filter((r) => r.kind === 'item' && ST_LABELS.includes(r.label)).sort(bySize)
    .map((r) => Object.assign({}, r, { why: stRuleText(r.rule) }));
}
// The list the page draws: the top ST_CLEANUP_TOP, or everything once "Show all" is on. `hidden` says how many are left out.
function cleanupView(list, showAll) {
  const shown = showAll ? list : list.slice(0, ST_CLEANUP_TOP);
  return { shown, hidden: list.length - shown.length };
}
const safeIds = (list) => list.filter((r) => r.label === ST_LABEL.safe).map((r) => r.id);
// The sum of some folders. `lowerBound` is true when any size is a floor, `unreadable` counts sizes that could not be read
// (they add nothing and the text says so, instead of pretending to be 0).
function sumSizes(list) {
  return { count: list.length, bytes: list.reduce((n, r) => n + (r.bytes || 0), 0), lowerBound: list.some((r) => r.lowerBound),
    unreadable: list.filter((r) => r.bytes == null).length };
}
function sumText(s) {
  const base = atLeast(s.bytes, s.lowerBound || s.unreadable > 0);
  return s.unreadable ? base + ' (' + s.unreadable + ' ' + (s.unreadable === 1 ? 'size' : 'sizes') + ' unavailable)' : base;
}
// A picked set only keeps ids that are still safe folders in the latest scan: a rescan can drop or relabel one.
const pickedRows = (list, ids) => { const want = new Set(ids || []); return list.filter((r) => want.has(r.id) && r.label === ST_LABEL.safe); };
const cleanupSummary = (list) => (list.length ? plural(list.length, 'folder', 'folders') + ', ' + sumText(sumSizes(list)) : 'Nothing to list');
// The safe subtotal and the inspector's total for "Review all safe" both come from sumText over the same rows, so the
// wording ("at least", unavailable sizes) cannot differ between them.
const safeSubtotalText = (list) => sumText(sumSizes(pickedRows(list, safeIds(list))));
const safeCleanupLine = (list) => { const ids = safeIds(list); return ids.length ? plural(ids.length, 'safe folder', 'safe folders') : 'Nothing safe to remove'; };

// One reading of a section for Inside System Data: a size, or the reason there is none. Never 0 for a section that could not be read.
function stSectionReading(d, category) {
  const cats = d && Array.isArray(d.categories) ? d.categories : [], items = d && Array.isArray(d.items) ? d.items : [];
  const c = cats.find((x) => x.category === category), mine = items.filter((it) => it.category === category);
  if (!c && !mine.length) return { bytes: null, reason: 'not yet scanned', lowerBound: false, count: 0 };
  const bytes = c ? goodBytes(c.bytes) : null;
  if (mine.length && mine.every((it) => stNum(it.size) == null)) return { bytes: null, reason: stReason(mine[0].size), lowerBound: false, count: mine.length };
  if (!mine.length && c && (c.unmeasured | 0) > 0 && bytes === 0) return { bytes: null, reason: 'not yet scanned', lowerBound: false, count: 0 };
  if (bytes == null) return { bytes: null, reason: 'bad reading', lowerBound: false, count: mine.length };
  return { bytes, reason: null, lowerBound: !!(c && c.lower_bound === true) || mine.some((it) => it.complete === false), count: mine.length };
}
const ST_SYSTEM_OWN = ['caches', 'logs', 'downloads', 'mail', 'applications'];
// Caches, local snapshots, indexes, logs and everything else this scan measures that macOS counts as System Data. The scan reads
// no Spotlight index, so that row always shows why. `share` is the part of the rows that could be read, for the bar.
function systemData(d) {
  const cats = d && Array.isArray(d.categories) ? d.categories : [];
  const row = (id, name, r, extra) => Object.assign({ id, name, bytes: r.bytes, reason: r.reason, lowerBound: r.lowerBound, detail: null, share: null }, extra);
  const snap = d && d.snapshots ? d.snapshots : {}, n = stNum(snap.count);
  const snapSize = stNum(snap.size);
  const snapRow = row('snapshots', 'Local snapshots', { bytes: snapSize, reason: snapSize == null ? stReason(snap.size) : null, lowerBound: false },
    { detail: n == null ? 'count unavailable: ' + stReason(snap.count) : n + (n === 1 ? ' snapshot' : ' snapshots') });
  const others = cats.filter((c) => !ST_SYSTEM_OWN.includes(c.category)).map((c) => stSectionReading(d, c.category));
  const readable = others.filter((r) => r.bytes != null);
  const otherRow = row('other', 'Other', readable.length ? { bytes: readable.reduce((a, r) => a + r.bytes, 0), reason: null, lowerBound: readable.some((r) => r.lowerBound) || others.some((r) => r.bytes == null) }
    : { bytes: null, reason: others.length ? others[0].reason : 'not yet scanned', lowerBound: false }, { detail: 'Developer tools and any other section this scan measures' });
  const rows = [row('caches', 'Caches', stSectionReading(d, 'caches')), snapRow,
    row('indexes', 'Indexes', { bytes: null, reason: 'this scan does not read Spotlight indexes', lowerBound: false }),
    row('logs', 'Logs', stSectionReading(d, 'logs')), otherRow];
  const total = rows.reduce((a, r) => a + (r.bytes || 0), 0);
  return rows.map((r) => Object.assign(r, { share: r.bytes != null && total > 0 ? r.bytes / total * 100 : r.bytes != null ? 0 : null }));
}
const systemText = (r) => (r.bytes == null ? 'unavailable: ' + r.reason : atLeast(r.bytes, r.lowerBound));

// ---- The exported report: one HTML file, inline styles only, no script, no remote address of any kind.
const escHtml = (v) => String(v == null ? '' : v).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');
const REPORT_CSS = 'body{font:14px/1.45 -apple-system,system-ui,sans-serif;margin:32px auto;max-width:960px;padding:0 16px;color:#1a1a1f}'
  + 'h1{font-size:22px;margin:0 0 4px}h2{font-size:16px;margin:28px 0 8px}p{margin:4px 0}.mut{color:#5b5b66}.note{border:1px solid #c9a227;padding:8px 12px;margin:12px 0}'
  + 'table{border-collapse:collapse;width:100%}th,td{text-align:left;padding:6px 8px;border-bottom:1px solid #d8d8e0;vertical-align:top}'
  + 'td.n,th.n{text-align:right;white-space:nowrap}.p{word-break:break-all}.tag{border:1px solid #5b5b66;padding:0 6px;font-size:12px;text-transform:uppercase}';
// Newlines and other control characters in a name show as \n, \t or \xNN, so a path cannot pass as two folders.
const visibleText = (v) => String(v == null ? '' : v).replace(/[\u0000-\u001f\u007f\u2028\u2029]/g, (c) => (c === '\n' ? '\\n' : c === '\r' ? '\\r' : c === '\t' ? '\\t' : '\\x' + c.charCodeAt(0).toString(16).padStart(2, '0')));
function reportHtml({ d, rows, shownRows, system, generated, filterText }) {
  const state = storageState({ data: d }), list = stCleanups(rows), safe = list.filter((r) => r.label === ST_LABEL.safe);
  const td = (v, cls) => '<td' + (cls ? ' class="' + cls + '"' : '') + '>' + escHtml(visibleText(v)) + '</td>';
  const rel = (r) => homeRelative(r.path);
  const folderRow = (r) => '<tr>' + td(rel(r), 'p') + td(ST_CATEGORIES[r.category] || r.category || '-') + '<td><span class="tag">' + escHtml(r.label || '-') + '</span></td>' + td(sizeText(r), 'n') + '</tr>';
  const cleanRow = (r) => '<tr>' + td(rel(r), 'p') + td(r.why) + '<td><span class="tag">' + escHtml(r.label) + '</span></td>' + td(sizeText(r), 'n') + '</tr>';
  const head = (cols) => '<tr>' + cols.map((c, i) => '<th' + (i === cols.length - 1 ? ' class="n"' : '') + '>' + escHtml(c) + '</th>').join('') + '</tr>';
  const partial = state === 'complete' ? '' : '<p class="note"><strong>Partial scan.</strong> ' + escHtml(coverageText(d)) + '. Sizes marked "at least" can only grow, and no safe-to-remove total is shown.</p>';
  return '<!doctype html><html lang="en"><head><meta charset="utf-8">'
    + '<meta http-equiv="Content-Security-Policy" content="default-src \'none\'; style-src \'unsafe-inline\'">'
    + '<meta name="viewport" content="width=device-width,initial-scale=1"><title>Storage report</title><style>' + REPORT_CSS + '</style></head><body>'
    + '<h1>Storage report</h1><p class="mut">' + escHtml(visibleText('Made ' + generated + '. Scan ' + (typeof d.scanned_at === 'number' ? 'finished at ' + new Date(d.scanned_at * 1000).toISOString() : 'not finished') + '. ' + coverageText(d))) + '.</p>'
    + '<p class="mut">This file lists folder paths, with your home folder shown as ~. It makes no network requests and has no script.</p>' + partial
    + '<h2>Suggested clean-ups</h2><p>' + escHtml(state === 'complete' ? 'Safe to remove: ' + safeTotalText(d, rows) + ' (' + safeCleanupLine(list) + '). ' : 'No safe-to-remove total until a scan completes. ') + escHtml(cleanupSummary(list)) + '.</p>'
    + (list.length ? '<table>' + head(['Folder', 'Why', 'Label', 'Size']) + list.map(cleanRow).join('') + '</table>' : '')
    + '<h2>Inside System Data</h2><table>' + head(['Part', 'Detail', 'Size'])
    + system.map((r) => '<tr>' + td(r.name) + td(r.detail || '') + td(systemText(r), 'n') + '</tr>').join('') + '</table>'
    + '<h2>Folders in this view</h2><p class="mut">' + escHtml(visibleText(filterText)) + '</p>'
    + '<table>' + head(['Folder', 'Section', 'Label', 'Size']) + shownRows.map(folderRow).join('') + '</table></body></html>';
}
/* storage-cleanups:end */
