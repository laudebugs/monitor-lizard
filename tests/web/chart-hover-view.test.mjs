// The chart marker layer (`chart-hover-view` block) drawn against a small stand-in DOM, and the 5 min / 1 hour window
// (`perf-window` block): the highlight kept across the page's every-poll redraw, focus and blur, Escape, a resting pointer,
// touch, the tooltip's bounds. Pixel layout and real focus-visible are checked by hand in a browser.
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { html } from './page.mjs';
const block = (name) => {
  const m = html.match(new RegExp('/\\* ' + name + ':begin \\*/([\\s\\S]*?)/\\* ' + name + ':end \\*/'));
  assert.ok(m, name + ' block not found in the dashboard scripts (web/*.js)');
  return m[1];
};
const pure = block('chart-hover'), view = block('chart-hover-view'), win = block('perf-window');

class El {
  constructor(tag, doc) { this.tag = tag; this.doc = doc; this.attrs = {}; this.kids = []; this.on = {}; this.style = {}; this.hidden = false; this.connected = true; this.offsetWidth = 100; this.focusVisible = true; this.classes = new Set(); }
  setAttribute(k, v) { this.attrs[k] = String(v); if (k === 'hidden') this.hidden = true; if (k === 'class') this.classes = new Set(String(v).split(' ')); }
  getAttribute(k) { return k in this.attrs ? this.attrs[k] : null; }
  addEventListener(t, f) { (this.on[t] ||= []).push(f); }
  fire(t, ev = {}) { const e = { type: t, preventDefault() { this.prevented = true; }, ...ev }; for (const f of this.on[t] || []) f(e); return e; }
  append(...k) { this.kids.push(...k); }
  replaceChildren(...k) { this.kids = k; }
  get isConnected() { return this.connected; }
  matches(sel) { return sel === ':focus-visible' && this.focusVisible && this.doc.activeElement === this; }
  focus() { this.doc.activeElement = this; this.fire('focus'); }
  getBoundingClientRect() { return { left: 0, width: this.width ?? 1000 }; }
  find(cls) { const out = []; const walk = (e) => { for (const k of e.kids) if (k instanceof El) { if (k.classes.has(cls)) out.push(k); walk(k); } }; walk(this); return out; }
  get text() { return this.kids.map((k) => (typeof k === 'string' ? k : k.text)).join(' '); }
}
function world() {
  const doc = { activeElement: null }, timers = [], said = [];
  let now = 1_000_000;
  const h = (tag, attrs = {}, ...kids) => {
    const e = new El(tag, doc);
    for (const [k, v] of Object.entries(attrs)) { if (v == null || v === false) continue; if (k.startsWith('on')) e.addEventListener(k.slice(2).toLowerCase(), v); else e.setAttribute(k, v === true ? '' : v); }
    e.append(...kids.flat(Infinity).filter((k) => k != null && k !== false));
    return e;
  };
  const clock = { now: () => now };
  const make = new Function('h', 'document', 'announce', 'setTimeout', 'Date', pure + '\n' + view + '\nreturn { chartHost, chartFrame, CH };');
  const api = make(h, doc, (m) => said.push(m), (f, ms) => timers.push({ f, at: now + ms }), clock);
  const svg = () => { const s = new El('svg', doc); s.width = 1000; return s; };
  // Ten samples two seconds apart; the newest is at `now`.
  const spec = (over = {}) => ({ id: 'c1', name: 'CPU, last 5 minutes', keys: true, max: 100, fmt: (v) => v + '%', ts: Array.from({ length: 10 }, (_, i) => now / 1000 - 18 + i * 2),
    series: [{ label: 'CPU', color: 'var(--acc)', values: Array.from({ length: 10 }, (_, i) => i * 10), reason: () => 'why' }], ...over });
  const build = (over) => { const sp = spec(over); const wrap = api.chartHost(svg(), sp); return { wrap, sp, tip: wrap.find('chart-tip')[0], guide: wrap.find('chart-guide')[0], dots: wrap.find('chart-dot') }; };
  const advance = (ms) => { now += ms; for (const t of timers.splice(0)) { if (t.at <= now) t.f(); else timers.push(t); } };
  return { ...api, doc, said, build, spec, advance, tick: (ms) => { now += ms; }, h };
}
const key = (wrap, k) => wrap.fire('keydown', { key: k });

test('with no samples the svg comes back untouched, with no handlers', () => {
  const w = world();
  const svg = new El('svg', w.doc);
  assert.equal(w.chartHost(svg, { ...w.spec(), ts: [] }), svg);
  assert.deepEqual(svg.on, {});
});

test('the marker layer starts hidden, and the tooltip is a visual copy hidden from screen readers', () => {
  const w = world();
  const c = w.build();
  assert.ok(c.tip.hidden && c.guide.hidden && c.dots.every((d) => d.hidden));
  assert.equal(c.tip.getAttribute('aria-hidden'), 'true');
});

test('a keyboard chart carries a hidden text summary of the newest sample and is not a live region', () => {
  const w = world();
  const c = w.build();
  const sr = c.wrap.find('sr-only')[0];
  assert.match(sr.text, /^CPU, last 5 minutes, latest: now: CPU 90%$/);
  assert.ok(!/aria-live|role/.test(Object.keys(sr.attrs).join(' ')));
  assert.equal(c.wrap.getAttribute('tabindex'), '0');
  assert.equal(c.wrap.getAttribute('data-req'), 'c1');
  const tile = w.build({ keys: false, id: 't1', tile: true });
  assert.equal(tile.wrap.getAttribute('tabindex'), null);
  assert.equal(tile.wrap.find('sr-only').length, 0);
});

test('a pointer over the chart shows the nearest sample, and leaving hides it', () => {
  const w = world();
  const c = w.build();
  c.wrap.fire('pointermove', { clientX: 500, clientY: 5 });
  assert.ok(!c.tip.hidden && !c.guide.hidden);
  assert.equal(c.guide.style.left, '55.56%');
  assert.match(c.tip.text, /8s ago/);
  c.wrap.fire('pointerleave', { pointerType: 'mouse' });
  assert.ok(c.tip.hidden);
  assert.equal(w.CH.c1, undefined);
});

test('the highlight is read back by the chart the next poll builds', () => {
  const w = world();
  w.build().wrap.fire('pointermove', { clientX: 500, clientY: 5 });
  w.tick(2000);
  const next = w.build();
  assert.ok(!next.tip.hidden);
  assert.equal(next.guide.style.left, '55.56%');
});

test('an unavailable value shows its reason and gets no dot', () => {
  const w = world();
  const c = w.build({ series: [{ label: 'CPU', color: 'x', values: [null, 1, 2, 3, 4, 5, 6, 7, 8, 9], reason: () => 'restricted' }] });
  c.wrap.fire('pointermove', { clientX: 0, clientY: 5 });
  assert.match(c.tip.text, /unavailable \(restricted\)/);
  assert.ok(c.dots[0].hidden);
});

test('a resting pointer under a rebuilt chart does not undo a key press, a moved one does', () => {
  const w = world();
  let c = w.build();
  c.wrap.fire('pointermove', { clientX: 300, clientY: 5 });
  c.wrap.focus();
  key(c.wrap, 'ArrowLeft');
  assert.equal(w.CH.c1.ptr, null);
  c = w.build();
  c.wrap.fire('pointerenter', { clientX: 300, clientY: 5 });
  assert.equal(w.CH.c1.ptr, null, 'same spot, no movement');
  c.wrap.fire('pointermove', { clientX: 320, clientY: 5 });
  assert.notEqual(w.CH.c1.ptr, null);
});

test('focus by keyboard shows the newest sample; a key press moves it and speaks once; a poll never speaks', () => {
  const w = world();
  const c = w.build();
  c.wrap.focus();
  assert.match(c.tip.text, /now/);
  assert.equal(w.said.length, 0);
  key(c.wrap, 'ArrowLeft');
  assert.match(c.tip.text, /2s ago/);
  assert.equal(w.said.length, 1);
  assert.match(w.said[0], /^CPU, last 5 minutes, 2 seconds ago: CPU 80%$/);
  w.tick(2000); w.build(); w.build();
  assert.equal(w.said.length, 1);
});

test('arrow keys stop the browser scrolling, and a modified key or another key is left alone', () => {
  const w = world();
  const c = w.build();
  c.wrap.focus();
  assert.equal(key(c.wrap, 'ArrowRight').prevented, true);
  assert.equal(key(c.wrap, 'a').prevented, undefined);
  assert.equal(c.wrap.fire('keydown', { key: 'ArrowLeft', metaKey: true }).prevented, undefined);
});

test('a click that only focuses the chart shows nothing until a key is pressed', () => {
  const w = world();
  const c = w.build();
  c.wrap.focusVisible = false;
  c.wrap.focus();
  assert.ok(c.tip.hidden);
  key(c.wrap, 'ArrowLeft');
  assert.match(c.tip.text, /2s ago/);
});

test('Escape clears the highlight and it stays cleared across a rebuild that gets focus back', () => {
  const w = world();
  let c = w.build();
  c.wrap.focus();
  key(c.wrap, 'Escape');
  assert.ok(c.tip.hidden);
  c.wrap.connected = false;
  c = w.build();
  c.wrap.focus();
  assert.ok(c.tip.hidden, 'the restore of focus must not bring the tooltip back');
  assert.equal(w.said.length, 0);
});

test('a poll that removes the focused chart and focuses its replacement keeps the key highlight; a real blur drops it', () => {
  const w = world();
  const a = w.build();
  a.wrap.focus();
  key(a.wrap, 'ArrowLeft');
  const keyTs = w.CH.c1.key;
  a.wrap.connected = false;
  const b = w.build();
  b.wrap.focus();
  a.wrap.fire('blur');
  w.advance(1);
  assert.equal(w.CH.c1.key, keyTs);
  assert.ok(!b.tip.hidden);
  w.doc.activeElement = null;
  b.wrap.fire('blur');
  w.advance(1);
  assert.equal(w.CH.c1, undefined);
  assert.ok(b.tip.hidden);
});

test('duplicate timestamps do not trap the Right arrow', () => {
  const w = world();
  const t = w.build().sp.ts.at(-1);
  const c = w.build({ ts: [t - 6, t - 4, t - 4, t - 2, t], series: [{ label: 'CPU', color: 'x', values: [1, 2, 3, 4, 5], reason: () => '' }] });
  c.wrap.focus();
  for (let i = 0; i < 4; i++) key(c.wrap, 'ArrowLeft');
  assert.match(c.tip.text, /6s ago/);
  const seen = [];
  for (let i = 0; i < 4; i++) { key(c.wrap, 'ArrowRight'); seen.push(w.said.at(-1).split(': ')[1]); }
  assert.deepEqual(seen, ['CPU 2%', 'CPU 3%', 'CPU 4%', 'CPU 5%']);
});

test('a key highlight held through a change of window lands on the nearest sample of the new window', () => {
  const w = world();
  const first = w.build();
  first.wrap.focus();
  key(first.wrap, 'ArrowLeft'); key(first.wrap, 'ArrowLeft');
  const held = w.CH.c1.key;
  const t = first.sp.ts.at(-1);
  first.wrap.connected = false;
  const thinned = [t - 18, t - 12, t - 6, t];
  const c = w.build({ ts: thinned, series: [{ label: 'CPU', color: 'x', values: [5, 6, 7, 8], reason: () => '' }] });
  c.wrap.focus();
  const nearest = thinned.reduce((b, x) => (Math.abs(x - held) < Math.abs(b - held) ? x : b));
  assert.match(c.tip.text, new RegExp(Math.round(t - nearest) + 's ago'));
});

test('a touch that lifts keeps the reading for 1.5 s, and a new touch cancels the clearing', () => {
  const w = world();
  let c = w.build();
  c.wrap.fire('pointerdown', { clientX: 400, clientY: 5, pointerType: 'touch' });
  c.wrap.fire('pointerleave', { pointerType: 'touch' });
  assert.ok(!c.tip.hidden);
  w.advance(1000);
  assert.ok(!c.tip.hidden);
  w.advance(600);
  assert.ok(c.tip.hidden);
  c.wrap.fire('pointerdown', { clientX: 400, clientY: 5, pointerType: 'touch' });
  c.wrap.fire('pointerleave', { pointerType: 'touch' });
  w.advance(1000);
  c.wrap.fire('pointerdown', { clientX: 600, clientY: 5, pointerType: 'touch' });
  w.advance(1000);
  assert.ok(!c.tip.hidden, 'the first lift must not clear the second touch');
});

test('the tooltip stays inside the chart: right of the guide, else left of it, never past an edge', () => {
  const w = world();
  const c = w.build();
  c.tip.offsetWidth = 200;
  c.wrap.fire('pointermove', { clientX: 100, clientY: 5 });
  assert.equal(c.tip.style.left, (1000 / 9 + 10).toFixed(1) + 'px');
  c.wrap.fire('pointermove', { clientX: 990, clientY: 5 });
  assert.equal(c.tip.style.left, '790.0px');
  c.tip.offsetWidth = 2000;
  c.wrap.fire('pointermove', { clientX: 500, clientY: 5 });
  assert.equal(c.tip.style.left, '0.0px');
});

test('chartFrame drops a highlight whose chart was not drawn last render, and everything after a 5 s gap', () => {
  const w = world();
  w.chartFrame();
  w.build({ id: 'a' }).wrap.fire('pointermove', { clientX: 100, clientY: 5 });
  w.build({ id: 'b' }).wrap.fire('pointermove', { clientX: 100, clientY: 5 });
  w.tick(2000); w.chartFrame();
  assert.deepEqual(Object.keys(w.CH).sort(), ['a', 'b']);
  w.build({ id: 'a' });
  w.tick(2000); w.chartFrame();
  assert.deepEqual(Object.keys(w.CH), ['a'], 'b was not drawn in the render before');
  w.tick(6000); w.chartFrame();
  assert.deepEqual(Object.keys(w.CH), [], 'a long absence clears all');
});

test('perfWindowed: 5 minutes keeps every sample in range, 1 hour thins to about one point per two pixels, never under 10', () => {
  const metrics = Array.from({ length: 1800 }, (_, i) => ({ ts: 10_000 + i * 2 }));
  const make = (S, width) => new Function('S', 'document', win + '\nreturn perfWindowed;')(S, { querySelector: () => ({ clientWidth: width }) });
  const five = make({ perfWin: '5m', metrics }, 600)();
  assert.equal(five.length, 150);
  assert.equal(five.at(-1).ts, metrics.at(-1).ts);
  const hour = make({ perfWin: '1h', metrics }, 600)();
  assert.equal(hour.length, 300);
  assert.equal(hour.at(-1).ts, metrics.at(-1).ts, 'the newest sample is always kept');
  assert.equal(make({ perfWin: '1h', metrics }, 8)().length, 10);
  assert.equal(make({ perfWin: '1h', metrics: [] }, 600)().length, 0);
});
