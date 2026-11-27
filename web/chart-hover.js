// Hover and keyboard reading for the Performance charts: which sample is under the pointer or the
// arrow keys, what it says, and the marker drawn for it. The page-performance.js helpers call chartHost().

/* chart-hover:begin */
// Samples sit evenly along the chart (sample i at i / (n - 1)), exactly as chartXY plots them.
// A pointer between two samples snaps to the nearer one; an exact tie takes the newer one.
// -1 means there is nothing to point at.
function chartIndexAt(n, frac) {
  if (!(n > 0) || typeof frac !== 'number' || !Number.isFinite(frac)) return -1;
  if (n === 1) return 0;
  return Math.round(Math.min(1, Math.max(0, frac)) * (n - 1));
}
// Left and Right move one sample, Home and End jump to the ends, and the ends stop (no wrap). With no
// highlight yet (cur is not a valid index) a step starts from the newest sample.
function chartStep(n, cur, key) {
  if (!(n > 0)) return -1;
  const at = Number.isInteger(cur) && cur >= 0 && cur < n ? cur : n - 1;
  if (key === 'ArrowLeft') return Math.max(0, at - 1);
  if (key === 'ArrowRight') return Math.min(n - 1, at + 1);
  if (key === 'Home') return 0;
  if (key === 'End') return n - 1;
  return at;
}
// A keyboard highlight is remembered as a time, not an index: every poll adds a sample and the
// 1 hour window thins them out again, so an index would drift. The nearest sample wins; one older
// than the whole window sticks to the oldest.
function chartIndexForTs(ts, t) {
  if (!ts.length || typeof t !== 'number' || !Number.isFinite(t)) return -1;
  let best = 0;
  for (let i = 1; i < ts.length; i++) if (Math.abs(ts[i] - t) < Math.abs(ts[best] - t)) best = i;
  return best;
}
// "32s ago", "4m 12s ago", "1h 2m ago"; `long` spells the units out for a screen reader.
function chartAgo(secs, long) {
  const s = Math.max(0, Math.round(secs));
  const unit = (n, short, one, many) => (long ? n + ' ' + (n === 1 ? one : many) : n + short);
  if (s < 1) return 'now';
  const parts = s < 60 ? [unit(s, 's', 'second', 'seconds')]
    : s < 3600 ? [unit(Math.floor(s / 60), 'm', 'minute', 'minutes'), s % 60 ? unit(s % 60, 's', 'second', 'seconds') : null]
    : [unit(Math.floor(s / 3600), 'h', 'hour', 'hours'), Math.floor((s % 3600) / 60) ? unit(Math.floor((s % 3600) / 60), 'm', 'minute', 'minutes') : null];
  return parts.filter(Boolean).join(' ') + ' ago';
}
// What sample i says. spec: { ts: [epoch seconds], max, stacked, fmt, series: [{ label, color, values,
// plot?, reason? }] }. `values` hold null where a reading was unavailable and `reason(i)` says why, so
// that sample reads "unavailable (reason)", never a blank or a 0. `plot` holds where a series is drawn
// when that is not its own value (a stacked layer sits on the ones below). A stacked chart skips a
// sample when any layer is missing, so no dot is placed on it. `y` is the dot's distance from the top
// of the chart as a fraction, the same mapping as chartXY.
function chartSample(spec, i, now) {
  const n = spec.ts.length;
  if (!Number.isInteger(i) || i < 0 || i >= n) return null;
  const max = spec.max > 0 ? spec.max : 1;
  const rows = spec.series.map((s) => {
    const v = s.values[i], ok = typeof v === 'number' && Number.isFinite(v);
    const p = s.plot ? s.plot[i] : v;
    return { label: s.label, color: s.color, unavailable: !ok,
      text: ok ? spec.fmt(v) : 'unavailable (' + ((s.reason && s.reason(i)) || 'no reading') + ')',
      y: ok && typeof p === 'number' ? 1 - Math.min(Math.max(p, 0), max) / max : null };
  });
  if (spec.stacked && rows.some((r) => r.unavailable)) for (const r of rows) r.y = null;
  const secs = now - spec.ts[i];
  return { i, ts: spec.ts[i], frac: n > 1 ? i / (n - 1) : 0, ago: chartAgo(secs, false), rows,
    say: chartAgo(secs, true) + ': ' + rows.map((r) => r.label + ' ' + r.text).join(', ') };
}
/* chart-hover:end */

/* chart-hover-view:begin */
// What each chart currently highlights, by chart id: { ptr: fraction | null, key: sample time | null,
// focused }. It lives here, not on the chart, because the page rebuilds every chart on every poll; the
// new chart reads it back, so the marker stays where it was and the focused chart keeps the keyboard
// (render() gives focus back through data-req, as it does for buttons).
const CH = {};
let chPos = [-1, -1];
let chFrame = { at: 0, prev: new Set(), now: new Set() };
// Called once per Performance render. A highlight whose chart did not draw in the render before
// this one (another tab, or a long absence from the page) is dropped, so it cannot reappear later.
function chartFrame() {
  const t = Date.now();
  const keep = t - chFrame.at <= 5000 ? chFrame.now : new Set();
  for (const id of Object.keys(CH)) if (!keep.has(id)) delete CH[id];
  chFrame = { at: t, prev: chFrame.now, now: new Set() };
}
// Wraps a chart's svg with the marker layer: a guide line, a dot per series and a small tooltip, all
// absolutely positioned over the chart so showing them never resizes it or moves the page. spec is
// chartSample's, plus { id, name, keys }. With no samples the svg comes back unwrapped. keys: true makes
// the chart a tab stop with Left/Right/Home/End; a tile's sparkline sits inside a button, which is
// already the tab stop, so it answers the pointer only.
function chartHost(svg, spec) {
  if (!spec || !spec.ts || !spec.ts.length) return svg;
  const n = spec.ts.length, id = spec.id;
  chFrame.now.add(id);
  const guide = h('span', { class: 'chart-guide', hidden: true });
  const dots = spec.series.map((s) => h('span', { class: 'chart-dot', hidden: true, style: 'background:' + s.color }));
  const tip = h('span', { class: 'chart-tip', hidden: true });
  const keys = !!spec.keys;
  const wrap = h('span', Object.assign({ class: 'chart' + (spec.tile ? ' chart-tile' : '') }, keys
    ? { tabindex: '0', role: 'group', 'data-req': id, 'aria-label': spec.name + '. Left and right arrow keys read each sample.', 'aria-keyshortcuts': 'ArrowLeft ArrowRight Home End' } : {}),
    svg, guide, ...dots, tip);
  if (keys) svg.setAttribute('aria-hidden', 'true');
  const index = () => {
    const st = CH[id];
    if (!st) return -1;
    if (st.ptr != null) return chartIndexAt(n, st.ptr);
    return st.key != null ? chartIndexForTs(spec.ts, st.key) : -1;
  };
  const paint = () => {
    const sample = chartSample(spec, index(), Date.now() / 1000);
    for (const el of [guide, tip, ...dots]) el.hidden = !sample;
    if (!sample) return;
    const left = (sample.frac * 100).toFixed(2) + '%';
    guide.style.left = left;
    sample.rows.forEach((r, k) => { dots[k].hidden = r.y == null; dots[k].style.left = left; dots[k].style.top = r.y == null ? '' : (r.y * 100).toFixed(2) + '%'; });
    tip.style.left = left;
    tip.classList.toggle('flip', sample.frac > 0.5);
    tip.replaceChildren(h('span', { class: 'chart-tip-t' }, sample.ago),
      ...sample.rows.map((r) => h('span', { class: 'chart-tip-r' }, h('i', { class: 'chart-sw', style: 'background:' + r.color }), h('span', {}, r.label + ' '), h('b', {}, r.text))));
  };
  const at = (e) => { const r = svg.getBoundingClientRect(); return r.width > 0 ? (e.clientX - r.left) / r.width : null; };
  // A rebuilt chart under a resting pointer gets a pointer event with no movement; it must not undo a key press.
  const point = (e) => {
    const f = at(e);
    if (f == null || (e.clientX === chPos[0] && e.clientY === chPos[1] && CH[id])) return;
    chPos = [e.clientX, e.clientY];
    CH[id] = Object.assign(CH[id] || {}, { ptr: f });
    paint();
  };
  wrap.addEventListener('pointermove', point);
  wrap.addEventListener('pointerenter', point);
  const leave = () => { if (CH[id]) { CH[id].ptr = null; if (!CH[id].focused && CH[id].key == null) delete CH[id]; } paint(); };
  wrap.addEventListener('pointerleave', leave);
  wrap.addEventListener('pointercancel', leave);
  if (keys) {
    wrap.addEventListener('focus', () => {
      // Arriving by keyboard shows the newest sample at once; a click that merely focuses it does not.
      const first = !CH[id];
      CH[id] = Object.assign(CH[id] || { ptr: null, key: null }, { focused: true });
      if (first && CH[id].key == null && wrap.matches(':focus-visible')) CH[id].key = spec.ts[n - 1];
      paint();
    });
    // A poll removes the focused chart and render() focuses its replacement in the same task; only a
    // chart that is still on the page and no longer focused has really lost the keyboard.
    wrap.addEventListener('blur', () => setTimeout(() => { if (wrap.isConnected && document.activeElement !== wrap) { delete CH[id]; paint(); } }, 0));
    wrap.addEventListener('keydown', (e) => {
      if (e.altKey || e.ctrlKey || e.metaKey) return;
      if (e.key === 'Escape') { if (CH[id]) { CH[id].ptr = null; CH[id].key = null; paint(); } return; }
      if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) return;
      e.preventDefault();
      const next = chartStep(n, index(), e.key);
      CH[id] = Object.assign(CH[id] || {}, { ptr: null, key: spec.ts[next], focused: true });
      paint();
      const sample = chartSample(spec, next, Date.now() / 1000);
      // Only a key press speaks: a poll redraw or a pointer move never does.
      if (sample) announce(spec.name + ', ' + sample.say);
    });
  }
  paint();
  return wrap;
}
/* chart-hover-view:end */
